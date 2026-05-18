use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use futures_util::StreamExt;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::io;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

use crate::client::MeiliClient;

// ── SSE event types sent from background stream to TUI ───────

enum SseEvent {
    ContentDelta(String),
    ToolCall(String),
    Source(Source),
    RawLog(String),
    Done,
    Error(String),
}

#[derive(Clone)]
struct Source {
    index: String,
    title: String,
    snippet: String,
}

// ── Chat data model ──────────────────────────────────────────

struct ChatMessage {
    role: String,
    content: String,
    sources: Vec<Source>,
}

struct ChatState {
    input: String,
    messages: Vec<ChatMessage>,
    /// Partial content being streamed for the current assistant response.
    streaming_content: String,
    streaming_sources: Vec<Source>,
    raw_log: Vec<String>,
    scroll: u16,
    auto_scroll: bool,
    log_scroll: u16,
    loading: bool,
    show_log: bool,
    show_sources: bool,
    tick: u64,
    model: String,
    workspace: String,
    /// Input history for arrow-up/down recall.
    input_history: Vec<String>,
    /// Current position in history. `input_history.len()` means "new input".
    history_pos: usize,
    /// Stash for the in-progress input when browsing history.
    input_stash: String,
    /// When true, the next completed response replaces all messages (compaction).
    compacting: bool,
}

/// Returns `true` if the command triggers a /compact (needs async API call from main loop).
fn handle_slash_command(state: &mut ChatState, input: &str) -> bool {
    let parts: Vec<&str> = input.trim().splitn(2, ' ').collect();
    let cmd = parts[0];
    let arg = parts.get(1).map(|s| s.trim());

    let response = match cmd {
        "/help" => "/model <name>     — change LLM model (current: MODEL)\n\
             /workspace <name> — change chat workspace (current: WORKSPACE)\n\
             /sources           — toggle source display\n\
             /clear             — clear all history and logs\n\
             /compact           — summarize conversation into a fresh start\n\
             /log               — show raw SSE log\n\
             /help              — show this help"
            .replace("MODEL", &state.model)
            .replace("WORKSPACE", &state.workspace),
        "/model" => {
            if let Some(name) = arg {
                state.model = name.to_string();
                format!("Model set to: {name}")
            } else {
                format!("Current model: {}\nUsage: /model <name>", state.model)
            }
        }
        "/workspace" => {
            if let Some(name) = arg {
                state.workspace = name.to_string();
                format!("Workspace set to: {name}")
            } else {
                format!(
                    "Current workspace: {}\nUsage: /workspace <name>",
                    state.workspace
                )
            }
        }
        "/sources" => {
            state.show_sources = !state.show_sources;
            if state.show_sources {
                "Sources: visible".to_string()
            } else {
                "Sources: hidden".to_string()
            }
        }
        "/clear" => {
            state.messages.clear();
            state.raw_log.clear();
            state.scroll = 0;
            "Chat cleared.".to_string()
        }
        "/compact" => {
            let convo_messages: Vec<&ChatMessage> = state
                .messages
                .iter()
                .filter(|m| m.role == "user" || m.role == "assistant")
                .collect();

            if convo_messages.is_empty() {
                "Nothing to compact.".to_string()
            } else {
                return true;
            }
        }
        "/log" => {
            state.show_log = true;
            state.log_scroll = state.raw_log.len().saturating_sub(1) as u16;
            return false;
        }
        _ => format!("Unknown command: {cmd}\nType /help for available commands."),
    };

    state.messages.push(ChatMessage {
        role: "system".to_string(),
        content: response,
        sources: Vec::new(),
    });
    state.auto_scroll = true;
    false
}

const SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

// ── Background SSE stream reader ─────────────────────────────

async fn stream_sse(resp: reqwest::Response, tx: mpsc::UnboundedSender<SseEvent>) {
    let mut stream = resp.bytes_stream();
    let mut buffer = String::new();
    // Accumulate tool call arguments by index
    let mut tool_args_acc: std::collections::HashMap<u64, String> =
        std::collections::HashMap::new();
    let mut tool_names: std::collections::HashMap<u64, String> = std::collections::HashMap::new();

    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(SseEvent::Error(e.to_string()));
                return;
            }
        };
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(line_end) = buffer.find('\n') {
            let line = buffer[..line_end].trim().to_string();
            buffer = buffer[line_end + 1..].to_string();

            if line.is_empty() {
                continue;
            }

            if line == "data: [DONE]" {
                let _ = tx.send(SseEvent::RawLog("[DONE]".to_string()));
                let _ = tx.send(SseEvent::Done);
                return;
            }

            if let Some(data) = line.strip_prefix("data: ") {
                let _ = tx.send(SseEvent::RawLog(data.to_string()));

                if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                    // Content delta
                    if let Some(delta) = json["choices"][0]["delta"]["content"].as_str() {
                        let _ = tx.send(SseEvent::ContentDelta(delta.to_string()));
                    }

                    // Tool calls — accumulate argument chunks
                    if let Some(calls) = json["choices"][0]["delta"]["tool_calls"].as_array() {
                        for call in calls {
                            let idx = call["index"].as_u64().unwrap_or(0);
                            if let Some(name) = call["function"]["name"].as_str() {
                                tool_names.insert(idx, name.to_string());
                                let _ = tx.send(SseEvent::ToolCall(name.to_string()));
                                let _ = tx.send(SseEvent::RawLog(format!("  [tool_call] {name}")));
                            }
                            if let Some(args) = call["function"]["arguments"].as_str() {
                                tool_args_acc.entry(idx).or_default().push_str(args);
                            }
                        }
                    }

                    // Check finish_reason to parse accumulated tool args
                    if json["choices"][0]["finish_reason"].as_str() == Some("tool_calls") {
                        for (idx, full_args) in &tool_args_acc {
                            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(full_args)
                            {
                                let name =
                                    tool_names.get(idx).map(|s| s.as_str()).unwrap_or("unknown");
                                let _ = tx.send(SseEvent::RawLog(format!(
                                    "  [tool_args] {}",
                                    serde_json::to_string(&parsed).unwrap_or_default()
                                )));

                                // Parse _meiliSearchSources
                                if name == "_meiliSearchSources"
                                    && let Some(docs) = parsed["documents"].as_array()
                                {
                                    for doc in docs {
                                        let source = Source {
                                            index: doc["_index"]
                                                .as_str()
                                                .or(doc["indexUid"].as_str())
                                                .unwrap_or("")
                                                .to_string(),
                                            title: doc["title"]
                                                .as_str()
                                                .or(doc["name"].as_str())
                                                .unwrap_or("(untitled)")
                                                .to_string(),
                                            snippet: {
                                                let s = doc["overview"]
                                                    .as_str()
                                                    .or(doc["description"].as_str())
                                                    .or(doc["content"].as_str())
                                                    .unwrap_or("");
                                                if s.len() > 120 {
                                                    format!("{}...", &s[..117])
                                                } else {
                                                    s.to_string()
                                                }
                                            },
                                        };
                                        let _ = tx.send(SseEvent::Source(source));
                                    }
                                }
                            }
                        }
                        tool_args_acc.clear();
                        tool_names.clear();
                    }
                }
            }
        }
    }

    // Stream ended without [DONE]
    let _ = tx.send(SseEvent::Done);
}

// ── Main TUI loop ────────────────────────────────────────────

pub async fn run_interactive_chat(
    client: MeiliClient,
    workspace: &str,
    model: Option<&str>,
) -> Result<()> {
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = ChatState {
        input: String::new(),
        messages: vec![ChatMessage {
            role: "system".to_string(),
            content: "Welcome to Meilisearch Chat. Type /help for commands.".to_string(),
            sources: Vec::new(),
        }],
        streaming_content: String::new(),
        streaming_sources: Vec::new(),
        raw_log: Vec::new(),
        scroll: 0,
        auto_scroll: true,
        log_scroll: 0,
        loading: false,
        show_log: false,
        show_sources: false,
        tick: 0,
        model: model.unwrap_or("gpt-4o-mini").to_string(),
        workspace: workspace.to_string(),
        input_history: Vec::new(),
        history_pos: 0,
        input_stash: String::new(),
        compacting: false,
    };

    // Channel for receiving SSE events from background stream
    let (tx, mut rx) = mpsc::unbounded_channel::<SseEvent>();
    let mut active_tx: Option<mpsc::UnboundedSender<SseEvent>> = None;
    let last_tick = Instant::now();
    let _ = last_tick;

    loop {
        // Drain any pending SSE events (non-blocking)
        while let Ok(evt) = rx.try_recv() {
            match evt {
                SseEvent::ContentDelta(delta) => {
                    state.streaming_content.push_str(&delta);
                    state.auto_scroll = true;
                }
                SseEvent::ToolCall(_name) => {
                    // Visible in log view
                }
                SseEvent::Source(src) => {
                    state.streaming_sources.push(src);
                }
                SseEvent::RawLog(entry) => {
                    state.raw_log.push(entry);
                }
                SseEvent::Done => {
                    let content = if state.streaming_content.is_empty() {
                        "(empty response — check log with Ctrl+O)".to_string()
                    } else {
                        std::mem::take(&mut state.streaming_content)
                    };
                    let sources = std::mem::take(&mut state.streaming_sources);

                    if state.compacting {
                        // Replace entire conversation with the compacted summary
                        state.messages.clear();
                        state.messages.push(ChatMessage {
                            role: "system".to_string(),
                            content: "Conversation compacted.".to_string(),
                            sources: Vec::new(),
                        });
                        state.messages.push(ChatMessage {
                            role: "assistant".to_string(),
                            content: format!("[Context]\n{content}"),
                            sources,
                        });
                        state.scroll = 0;
                        state.compacting = false;
                    } else {
                        state.messages.push(ChatMessage {
                            role: "assistant".to_string(),
                            content,
                            sources,
                        });
                    }
                    state.loading = false;
                    active_tx = None;
                }
                SseEvent::Error(e) => {
                    state.raw_log.push(format!("<<< STREAM ERROR: {e}"));
                    state.compacting = false;
                    let content = if state.streaming_content.is_empty() {
                        format!("Stream error: {e}")
                    } else {
                        // Preserve partial content and note the error
                        let partial = std::mem::take(&mut state.streaming_content);
                        format!("{partial}\n\n[stream error: {e}]")
                    };
                    let sources = std::mem::take(&mut state.streaming_sources);
                    state.messages.push(ChatMessage {
                        role: "assistant".to_string(),
                        content,
                        sources,
                    });
                    state.loading = false;
                    active_tx = None;
                }
            }
        }

        state.tick = state.tick.wrapping_add(1);

        if state.show_log {
            terminal.draw(|f| draw_log_view(f, &state))?;
        } else {
            terminal.draw(|f| draw_chat_view(f, &mut state))?;
        }

        if event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
        {
            if state.show_log {
                match key.code {
                    KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        state.show_log = false;
                    }
                    KeyCode::Esc => {
                        state.show_log = false;
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        state.log_scroll = state.log_scroll.saturating_sub(1);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        state.log_scroll = state.log_scroll.saturating_add(1);
                    }
                    KeyCode::Char('G') => {
                        state.log_scroll = state.raw_log.len().saturating_sub(1) as u16;
                    }
                    KeyCode::Char('g') => {
                        state.log_scroll = 0;
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                    _ => {}
                }
                continue;
            }

            match key.code {
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    state.show_log = true;
                    state.log_scroll = state.raw_log.len().saturating_sub(1) as u16;
                }
                KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    state.show_sources = !state.show_sources;
                }
                KeyCode::Esc => break,
                KeyCode::Enter if !state.loading && !state.input.is_empty() => {
                    {
                        // Push to history
                        state.input_history.push(state.input.clone());
                        state.history_pos = state.input_history.len();
                        state.input_stash.clear();

                        // Handle slash commands
                        if state.input.starts_with('/') {
                            let cmd = state.input.clone();
                            state.input.clear();
                            let needs_compact = handle_slash_command(&mut state, &cmd);
                            if needs_compact {
                                // Build compaction prompt from conversation
                                let mut convo_text = String::new();
                                for m in &state.messages {
                                    if m.role == "user" || m.role == "assistant" {
                                        convo_text
                                            .push_str(&format!("{}: {}\n\n", m.role, m.content));
                                    }
                                }

                                state.messages.push(ChatMessage {
                                    role: "system".to_string(),
                                    content: "Compacting conversation...".to_string(),
                                    sources: Vec::new(),
                                });
                                state.loading = true;
                                state.compacting = true;
                                state.streaming_content.clear();
                                state.streaming_sources.clear();

                                let compact_messages = vec![serde_json::json!({
                                    "role": "user",
                                    "content": format!(
                                        "Summarize the following conversation into a concise context paragraph \
                                         that preserves all important facts, decisions, and context. \
                                         Write it as a system-style briefing so the conversation can \
                                         continue from this summary.\n\n{convo_text}"
                                    )
                                })];

                                let body = serde_json::json!({
                                    "model": &state.model,
                                    "messages": compact_messages,
                                    "stream": true
                                });

                                state.raw_log.push(format!(
                                    ">>> COMPACT REQUEST: {}",
                                    serde_json::to_string(&body).unwrap_or_default()
                                ));

                                let new_tx = tx.clone();
                                let client_clone = client.clone();
                                let ws = state.workspace.clone();
                                active_tx = Some(new_tx.clone());
                                tokio::spawn(async move {
                                    match client_clone.chat_completions(&ws, &body).await {
                                        Ok(resp) => {
                                            let status = resp.status();
                                            let _ = new_tx.send(SseEvent::RawLog(format!(
                                                "<<< COMPACT HTTP {status}"
                                            )));
                                            if !status.is_success() {
                                                let text = resp.text().await.unwrap_or_default();
                                                let _ = new_tx.send(SseEvent::Error(format!(
                                                    "HTTP {status}: {text}"
                                                )));
                                            } else {
                                                stream_sse(resp, new_tx).await;
                                            }
                                        }
                                        Err(e) => {
                                            let _ = new_tx.send(SseEvent::Error(e.to_string()));
                                        }
                                    }
                                });
                                state.auto_scroll = true;
                            }
                            continue;
                        }

                        let user_msg = state.input.clone();
                        state.messages.push(ChatMessage {
                            role: "user".to_string(),
                            content: user_msg.clone(),
                            sources: Vec::new(),
                        });
                        state.input.clear();
                        state.loading = true;
                        state.streaming_content.clear();
                        state.streaming_sources.clear();

                        state.raw_log.push(format!(">>> USER: {user_msg}"));

                        let api_messages: Vec<serde_json::Value> = state
                            .messages
                            .iter()
                            .filter(|m| m.role == "user" || m.role == "assistant")
                            .map(|m| {
                                serde_json::json!({
                                    "role": m.role,
                                    "content": m.content
                                })
                            })
                            .collect();

                        let body = serde_json::json!({
                            "model": &state.model,
                            "messages": api_messages,
                            "stream": true
                        });

                        state.raw_log.push(format!(
                            ">>> REQUEST: {}",
                            serde_json::to_string(&body).unwrap_or_default()
                        ));

                        // Spawn background SSE reader
                        let new_tx = tx.clone();
                        let client_clone = client.clone();
                        let ws = state.workspace.clone();
                        active_tx = Some(new_tx.clone());
                        tokio::spawn(async move {
                            match client_clone.chat_completions(&ws, &body).await {
                                Ok(resp) => {
                                    let status = resp.status();
                                    let _ =
                                        new_tx.send(SseEvent::RawLog(format!("<<< HTTP {status}")));
                                    if !status.is_success() {
                                        let text = resp.text().await.unwrap_or_default();
                                        let _ = new_tx
                                            .send(SseEvent::RawLog(format!("<<< ERROR: {text}")));
                                        let _ = new_tx.send(SseEvent::Error(format!(
                                            "HTTP {status}: {text}"
                                        )));
                                    } else {
                                        stream_sse(resp, new_tx).await;
                                    }
                                }
                                Err(e) => {
                                    let _ = new_tx.send(SseEvent::Error(e.to_string()));
                                }
                            }
                        });

                        state.auto_scroll = true;
                    }
                }
                KeyCode::Up
                    if !state.loading
                        && !state.input_history.is_empty()
                        && state.history_pos > 0 =>
                {
                    // First time pressing up: stash current input
                    if state.history_pos == state.input_history.len() {
                        state.input_stash = state.input.clone();
                    }
                    state.history_pos -= 1;
                    state.input = state.input_history[state.history_pos].clone();
                }
                KeyCode::Up if !state.loading => {}
                KeyCode::Down
                    if !state.loading && state.history_pos < state.input_history.len() =>
                {
                    state.history_pos += 1;
                    if state.history_pos == state.input_history.len() {
                        // Restore stashed input
                        state.input = state.input_stash.clone();
                    } else {
                        state.input = state.input_history[state.history_pos].clone();
                    }
                }
                KeyCode::Down if !state.loading => {}
                KeyCode::Char(c) if !state.loading => {
                    state.input.push(c);
                }
                KeyCode::Backspace if !state.loading => {
                    state.input.pop();
                }
                KeyCode::Up => {
                    state.auto_scroll = false;
                    state.scroll = state.scroll.saturating_sub(1);
                }
                KeyCode::Down => {
                    state.auto_scroll = false;
                    state.scroll = state.scroll.saturating_add(1);
                }
                _ => {}
            }
        }
    }

    drop(active_tx);
    terminal::disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}

// ── Drawing ──────────────────────────────────────────────────

fn draw_chat_view(f: &mut ratatui::Frame, state: &mut ChatState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(f.area());

    let mut lines: Vec<Line> = Vec::new();

    for msg in &state.messages {
        render_message(&mut lines, msg, state.show_sources);
    }

    // Show streaming content with animated cursor
    if state.loading {
        let spinner = SPINNER[(state.tick / 3) as usize % SPINNER.len()];

        if state.streaming_content.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("AI: ", Style::default().fg(Color::Green)),
                Span::styled(
                    format!("{spinner} Searching and thinking..."),
                    Style::default().fg(Color::Yellow),
                ),
            ]));
        } else {
            // Show partial content being streamed
            let content_lines: Vec<&str> = state.streaming_content.split('\n').collect();
            for (i, content_line) in content_lines.iter().enumerate() {
                if i == 0 {
                    lines.push(Line::from(vec![
                        Span::styled("AI: ", Style::default().fg(Color::Green)),
                        Span::raw(*content_line),
                    ]));
                } else {
                    lines.push(Line::from(vec![
                        Span::raw("    "),
                        Span::raw(*content_line),
                    ]));
                }
            }
            // Blinking cursor at the end
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled(spinner, Style::default().fg(Color::Yellow)),
            ]));
        }
        lines.push(Line::from(""));
    }

    let view_height = chunks[0].height.saturating_sub(2); // minus borders
    let inner_width = chunks[0].width.saturating_sub(2); // minus borders

    // Build the paragraph so we can ask ratatui for the real wrapped line count
    let history = Paragraph::new(lines)
        .block(Block::default().title(" Chat ").borders(Borders::ALL))
        .wrap(Wrap { trim: false });

    let total_visual_rows = history.line_count(inner_width) as u16;

    // Compute auto-scroll: scroll to bottom so latest content is visible
    if state.auto_scroll {
        state.scroll = total_visual_rows.saturating_sub(view_height);
    }
    // Clamp manual scroll so it can't go past the content
    let max_scroll = total_visual_rows.saturating_sub(view_height);
    if state.scroll > max_scroll {
        state.scroll = max_scroll;
    }

    let history = history.scroll((state.scroll, 0));
    f.render_widget(history, chunks[0]);

    // Input
    let input_title = if state.loading {
        " Waiting... "
    } else {
        " Message "
    };
    let input = Paragraph::new(state.input.as_str())
        .block(Block::default().title(input_title).borders(Borders::ALL))
        .style(Style::default().fg(if state.loading {
            Color::DarkGray
        } else {
            Color::Yellow
        }));
    f.render_widget(input, chunks[1]);

    // Show blinking cursor in input box
    if !state.loading {
        let cursor_x = chunks[1].x + 1 + state.input.len() as u16;
        let cursor_y = chunks[1].y + 1;
        f.set_cursor_position((cursor_x, cursor_y));
    }

    // Status bar
    let sources_label = if state.show_sources {
        "Ctrl+S hide sources"
    } else {
        "Ctrl+S show sources"
    };
    let status = Paragraph::new(Line::from(vec![Span::styled(
        format!(" Enter send | Ctrl+C quit | Ctrl+O log | {sources_label} "),
        Style::default().fg(Color::DarkGray),
    )]));
    f.render_widget(status, chunks[2]);
}

fn render_message<'a>(lines: &mut Vec<Line<'a>>, msg: &'a ChatMessage, show_sources: bool) {
    let (prefix, color) = match msg.role.as_str() {
        "user" => ("You: ", Color::Cyan),
        "assistant" => ("AI: ", Color::Green),
        _ => ("", Color::DarkGray),
    };

    let content_lines: Vec<&str> = msg.content.split('\n').collect();
    for (i, content_line) in content_lines.iter().enumerate() {
        if i == 0 {
            lines.push(Line::from(vec![
                Span::styled(prefix, Style::default().fg(color)),
                Span::raw(*content_line),
            ]));
        } else {
            let indent = " ".repeat(prefix.len());
            lines.push(Line::from(vec![
                Span::raw(indent),
                Span::raw(*content_line),
            ]));
        }
    }

    // Sources
    if !msg.sources.is_empty() {
        if show_sources {
            lines.push(Line::from(Span::styled(
                format!("    [{} sources]", msg.sources.len()),
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            )));
            for src in &msg.sources {
                let idx_label = if src.index.is_empty() {
                    String::new()
                } else {
                    format!("[{}] ", src.index)
                };
                lines.push(Line::from(vec![
                    Span::raw("      "),
                    Span::styled(idx_label, Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        src.title.as_str(),
                        Style::default()
                            .fg(Color::Blue)
                            .add_modifier(Modifier::UNDERLINED),
                    ),
                ]));
                if !src.snippet.is_empty() {
                    lines.push(Line::from(vec![
                        Span::raw("        "),
                        Span::styled(src.snippet.as_str(), Style::default().fg(Color::DarkGray)),
                    ]));
                }
            }
        } else {
            lines.push(Line::from(Span::styled(
                format!("    [{} sources — Ctrl+S to expand]", msg.sources.len()),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    lines.push(Line::from(""));
}

fn draw_log_view(f: &mut ratatui::Frame, state: &ChatState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(f.area());

    let lines: Vec<Line> = state
        .raw_log
        .iter()
        .map(|entry| {
            let color = if entry.starts_with(">>>") {
                Color::Cyan
            } else if entry.starts_with("<<<") {
                Color::Yellow
            } else if entry.starts_with("  [tool") {
                Color::Magenta
            } else if entry.starts_with("[DONE]") {
                Color::DarkGray
            } else {
                Color::White
            };
            Line::from(Span::styled(entry.as_str(), Style::default().fg(color)))
        })
        .collect();

    let log = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Raw SSE Log ")
                .title_style(Style::default().add_modifier(Modifier::BOLD))
                .borders(Borders::ALL),
        )
        .wrap(Wrap { trim: false })
        .scroll((state.log_scroll, 0));
    f.render_widget(log, chunks[0]);

    let status = Paragraph::new(Line::from(vec![Span::styled(
        " Ctrl+O / Esc close | j/k scroll | g top | G bottom ",
        Style::default().fg(Color::DarkGray),
    )]));
    f.render_widget(status, chunks[1]);
}
