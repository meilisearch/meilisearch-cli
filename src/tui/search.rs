use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use std::io;
use std::time::{Duration, Instant};

use crate::client::MeiliClient;

struct SearchState {
    query: String,
    results: Vec<serde_json::Value>,
    selected: usize,
    total_hits: u64,
    processing_time_ms: u64,
    last_search: Instant,
    expanded: Option<usize>,
}

pub async fn run_interactive_search(client: MeiliClient, uid: &str) -> Result<()> {
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = SearchState {
        query: String::new(),
        results: vec![],
        selected: 0,
        total_hits: 0,
        processing_time_ms: 0,
        last_search: Instant::now(),
        expanded: None,
    };

    let debounce = Duration::from_millis(150);
    let mut needs_search = false;

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(5),
                    Constraint::Length(1),
                ])
                .split(f.area());

            // Search input
            let input = Paragraph::new(state.query.as_str())
                .block(
                    Block::default()
                        .title(format!(" Search: {} ", uid))
                        .borders(Borders::ALL),
                )
                .style(Style::default().fg(Color::Yellow));
            f.render_widget(input, chunks[0]);

            // Results
            if let Some(idx) = state.expanded {
                if let Some(doc) = state.results.get(idx) {
                    let json = serde_json::to_string_pretty(doc).unwrap_or_default();
                    let para = Paragraph::new(json)
                        .block(
                            Block::default()
                                .title(" Document (Esc to close) ")
                                .borders(Borders::ALL),
                        )
                        .style(Style::default().fg(Color::White));
                    f.render_widget(para, chunks[1]);
                }
            } else {
                let items: Vec<ListItem> = state
                    .results
                    .iter()
                    .enumerate()
                    .map(|(i, doc)| {
                        let display = doc
                            .as_object()
                            .map(|obj| {
                                obj.iter()
                                    .take(3)
                                    .map(|(k, v)| {
                                        format!(
                                            "{}: {}",
                                            k,
                                            match v {
                                                serde_json::Value::String(s) => {
                                                    if s.len() > 60 {
                                                        format!("{}...", &s[..60])
                                                    } else {
                                                        s.clone()
                                                    }
                                                }
                                                other => other.to_string(),
                                            }
                                        )
                                    })
                                    .collect::<Vec<_>>()
                                    .join(" | ")
                            })
                            .unwrap_or_else(|| doc.to_string());

                        let style = if i == state.selected {
                            Style::default()
                                .fg(Color::Black)
                                .bg(Color::Cyan)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                        };
                        ListItem::new(Line::from(vec![Span::styled(display, style)]))
                    })
                    .collect();

                let list = List::new(items)
                    .block(Block::default().title(" Results ").borders(Borders::ALL));
                f.render_widget(list, chunks[1]);
            }

            // Status bar
            let status = Line::from(vec![Span::styled(
                format!(
                    " {} hits in {}ms | ↑↓ navigate | Enter expand | q quit ",
                    state.total_hits, state.processing_time_ms
                ),
                Style::default().fg(Color::DarkGray),
            )]);
            let status_bar = Paragraph::new(status);
            f.render_widget(status_bar, chunks[2]);
        })?;

        if event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
        {
            match key.code {
                KeyCode::Char('q') if state.expanded.is_none() => break,
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                KeyCode::Esc => {
                    if state.expanded.is_some() {
                        state.expanded = None;
                    } else {
                        break;
                    }
                }
                KeyCode::Char(c) if state.expanded.is_none() => {
                    state.query.push(c);
                    needs_search = true;
                    state.last_search = Instant::now();
                }
                KeyCode::Backspace if state.expanded.is_none() => {
                    state.query.pop();
                    needs_search = true;
                    state.last_search = Instant::now();
                }
                KeyCode::Up if state.selected > 0 => {
                    state.selected -= 1;
                }
                KeyCode::Up => {}
                KeyCode::Down if state.selected + 1 < state.results.len() => {
                    state.selected += 1;
                }
                KeyCode::Down => {}
                KeyCode::Enter => {
                    if state.expanded.is_some() {
                        state.expanded = None;
                    } else if !state.results.is_empty() {
                        state.expanded = Some(state.selected);
                    }
                }
                _ => {}
            }
        }

        // Debounced search
        if needs_search && state.last_search.elapsed() >= debounce {
            needs_search = false;
            if !state.query.is_empty() {
                if let Ok(result) = client
                    .search(
                        uid,
                        &state.query,
                        None,
                        None,
                        Some(20),
                        None,
                        None,
                        None,
                        None,
                    )
                    .await
                {
                    state.results = result["hits"].as_array().cloned().unwrap_or_default();
                    state.total_hits = result["estimatedTotalHits"].as_u64().unwrap_or(0);
                    state.processing_time_ms = result["processingTimeMs"].as_u64().unwrap_or(0);
                    state.selected = 0;
                    state.expanded = None;
                }
            } else {
                state.results.clear();
                state.total_hits = 0;
                state.processing_time_ms = 0;
            }
        }
    }

    terminal::disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}
