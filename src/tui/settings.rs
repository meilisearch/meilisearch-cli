use std::collections::HashMap;
use std::io;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use serde_json::Value;

use crate::client::MeiliClient;

// ─── Types ───────────────────────────────────────────────────

#[derive(Clone)]
enum EditorKind {
    FieldCheckbox,
    FieldOrdered,
    StringList,
    SingleSelect,
    EnumSelect(Vec<&'static str>),
    NumberInput,
    JsonEditor,
}

struct ResourceDef {
    slug: &'static str,
    key: &'static str,
    kind: EditorKind,
    group: &'static str,
}

fn resource_defs() -> Vec<ResourceDef> {
    vec![
        ResourceDef {
            slug: "displayed-attributes",
            key: "displayedAttributes",
            kind: EditorKind::FieldOrdered,
            group: "Field Attributes",
        },
        ResourceDef {
            slug: "searchable-attributes",
            key: "searchableAttributes",
            kind: EditorKind::FieldOrdered,
            group: "Field Attributes",
        },
        ResourceDef {
            slug: "filterable-attributes",
            key: "filterableAttributes",
            kind: EditorKind::FieldCheckbox,
            group: "Field Attributes",
        },
        ResourceDef {
            slug: "sortable-attributes",
            key: "sortableAttributes",
            kind: EditorKind::FieldCheckbox,
            group: "Field Attributes",
        },
        ResourceDef {
            slug: "ranking-rules",
            key: "rankingRules",
            kind: EditorKind::StringList,
            group: "Ranking",
        },
        ResourceDef {
            slug: "proximity-precision",
            key: "proximityPrecision",
            kind: EditorKind::EnumSelect(vec!["byWord", "byAttribute"]),
            group: "Ranking",
        },
        ResourceDef {
            slug: "search-cutoff-ms",
            key: "searchCutoffMs",
            kind: EditorKind::NumberInput,
            group: "Ranking",
        },
        ResourceDef {
            slug: "prefix-search",
            key: "prefixSearch",
            kind: EditorKind::EnumSelect(vec!["indexingTime", "disabled"]),
            group: "Ranking",
        },
        ResourceDef {
            slug: "stop-words",
            key: "stopWords",
            kind: EditorKind::StringList,
            group: "Tokens",
        },
        ResourceDef {
            slug: "dictionary",
            key: "dictionary",
            kind: EditorKind::StringList,
            group: "Tokens",
        },
        ResourceDef {
            slug: "non-separator-tokens",
            key: "nonSeparatorTokens",
            kind: EditorKind::StringList,
            group: "Tokens",
        },
        ResourceDef {
            slug: "separator-tokens",
            key: "separatorTokens",
            kind: EditorKind::StringList,
            group: "Tokens",
        },
        ResourceDef {
            slug: "distinct-attribute",
            key: "distinctAttribute",
            kind: EditorKind::SingleSelect,
            group: "Other",
        },
        ResourceDef {
            slug: "typo-tolerance",
            key: "typoTolerance",
            kind: EditorKind::JsonEditor,
            group: "Other",
        },
        ResourceDef {
            slug: "faceting",
            key: "faceting",
            kind: EditorKind::JsonEditor,
            group: "Other",
        },
        ResourceDef {
            slug: "pagination",
            key: "pagination",
            kind: EditorKind::JsonEditor,
            group: "Other",
        },
        ResourceDef {
            slug: "synonyms",
            key: "synonyms",
            kind: EditorKind::JsonEditor,
            group: "Other",
        },
        ResourceDef {
            slug: "embedders",
            key: "embedders",
            kind: EditorKind::JsonEditor,
            group: "Other",
        },
        ResourceDef {
            slug: "localized-attributes",
            key: "localizedAttributes",
            kind: EditorKind::JsonEditor,
            group: "Other",
        },
        ResourceDef {
            slug: "chat",
            key: "chat",
            kind: EditorKind::JsonEditor,
            group: "Other",
        },
    ]
}

// ─── Editor State ────────────────────────────────────────────

struct FieldItem {
    name: String,
    checked: bool,
}

enum EditorState {
    None,
    Field {
        items: Vec<FieldItem>,
        cursor: usize,
        ordered: bool,
        wildcard: bool,
    },
    StringList {
        items: Vec<String>,
        cursor: usize,
        input: String,
        adding: bool,
    },
    Select {
        options: Vec<String>,
        cursor: usize,
    },
    Number {
        input: String,
    },
}

// ─── Main State ──────────────────────────────────────────────

struct State {
    uid: String,
    settings: Value,
    changes: HashMap<String, Value>,
    fields: Vec<String>,
    resources: Vec<ResourceDef>,
    view: View,
    status_msg: String,
    confirm_quit: bool,
    res_cursor: usize,
    editing_idx: usize,
    editor: EditorState,
}

#[derive(PartialEq)]
enum View {
    ResourceList,
    Editor,
}

enum Action {
    Continue,
    Quit,
    Save,
    LaunchEditor(usize),
}

// ─── Helpers ─────────────────────────────────────────────────

fn get_value<'a>(settings: &'a Value, changes: &'a HashMap<String, Value>, key: &str) -> &'a Value {
    changes.get(key).unwrap_or_else(|| &settings[key])
}

fn summarize_value(value: &Value) -> String {
    match value {
        Value::Array(arr) if arr.len() == 1 && arr[0].as_str() == Some("*") => {
            "* (all fields)".into()
        }
        Value::Array(arr) if arr.is_empty() => "(none)".into(),
        Value::Array(arr) => {
            let items: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).take(3).collect();
            let s = items.join(", ");
            if arr.len() > 3 {
                format!("{s}, +{}", arr.len() - 3)
            } else {
                s
            }
        }
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "(default)".into(),
        Value::Object(obj) if obj.is_empty() => "(none)".into(),
        Value::Object(obj) => {
            let preview = serde_json::to_string(obj).unwrap_or_default();
            if preview.len() > 40 {
                format!("{}...", &preview[..40])
            } else {
                preview
            }
        }
    }
}

fn build_field_items(value: &Value, fields: &[String], ordered: bool) -> (Vec<FieldItem>, bool) {
    let is_wildcard = value
        .as_array()
        .is_some_and(|arr| arr.len() == 1 && arr[0].as_str() == Some("*"));

    let checked_list: Vec<String> = if is_wildcard {
        fields.to_vec()
    } else {
        value
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect()
    };

    let mut items = Vec::new();

    if ordered {
        // Checked items first, in their order
        for name in &checked_list {
            items.push(FieldItem {
                name: name.clone(),
                checked: true,
            });
        }
        // Unchecked items, alphabetical
        let mut unchecked: Vec<&String> = fields
            .iter()
            .filter(|f| !checked_list.contains(f))
            .collect();
        unchecked.sort();
        for name in unchecked {
            items.push(FieldItem {
                name: name.clone(),
                checked: false,
            });
        }
    } else {
        // All fields alphabetical, with checkmarks
        let mut all_fields: Vec<String> = fields.to_vec();
        for f in &checked_list {
            if !all_fields.contains(f) {
                all_fields.push(f.clone());
            }
        }
        all_fields.sort();
        for name in &all_fields {
            items.push(FieldItem {
                name: name.clone(),
                checked: checked_list.contains(name),
            });
        }
    }

    (items, is_wildcard)
}

fn compute_field_value(items: &[FieldItem], ordered: bool, wildcard: bool) -> Value {
    if wildcard {
        return serde_json::json!(["*"]);
    }
    if ordered {
        // Checked items are already in order in the vec
        let names: Vec<&str> = items
            .iter()
            .filter(|i| i.checked)
            .map(|i| i.name.as_str())
            .collect();
        serde_json::json!(names)
    } else {
        let names: Vec<&str> = items
            .iter()
            .filter(|i| i.checked)
            .map(|i| i.name.as_str())
            .collect();
        serde_json::json!(names)
    }
}

fn enter_editor(state: &mut State, idx: usize) {
    state.editing_idx = idx;
    state.view = View::Editor;
    let def = &state.resources[idx];
    let value = get_value(&state.settings, &state.changes, def.key).clone();

    match &def.kind {
        EditorKind::FieldCheckbox => {
            let (items, _) = build_field_items(&value, &state.fields, false);
            state.editor = EditorState::Field {
                items,
                cursor: 0,
                ordered: false,
                wildcard: false,
            };
        }
        EditorKind::FieldOrdered => {
            let (items, wildcard) = build_field_items(&value, &state.fields, true);
            state.editor = EditorState::Field {
                items,
                cursor: 0,
                ordered: true,
                wildcard,
            };
        }
        EditorKind::StringList => {
            let items: Vec<String> = value
                .as_array()
                .unwrap_or(&vec![])
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            state.editor = EditorState::StringList {
                items,
                cursor: 0,
                input: String::new(),
                adding: false,
            };
        }
        EditorKind::SingleSelect => {
            let current = value.as_str().unwrap_or("").to_string();
            let mut options = vec!["(none)".to_string()];
            let mut sorted_fields = state.fields.clone();
            sorted_fields.sort();
            options.extend(sorted_fields);
            let cursor = options.iter().position(|o| o == &current).unwrap_or(0);
            state.editor = EditorState::Select { options, cursor };
        }
        EditorKind::EnumSelect(opts) => {
            let current = value.as_str().unwrap_or("");
            let options: Vec<String> = opts.iter().map(|s| s.to_string()).collect();
            let cursor = options.iter().position(|o| o == current).unwrap_or(0);
            state.editor = EditorState::Select { options, cursor };
        }
        EditorKind::NumberInput => {
            let input = match &value {
                Value::Number(n) => n.to_string(),
                Value::Null => String::new(),
                _ => String::new(),
            };
            state.editor = EditorState::Number { input };
        }
        EditorKind::JsonEditor => {
            // Handled externally via $EDITOR
            state.editor = EditorState::None;
        }
    }
}

fn exit_editor(state: &mut State) {
    // Compute new value and store in changes if different from original
    let def = &state.resources[state.editing_idx];
    let key = def.key.to_string();
    let original = &state.settings[&key];

    let new_value = match &state.editor {
        EditorState::Field {
            items,
            ordered,
            wildcard,
            ..
        } => compute_field_value(items, *ordered, *wildcard),
        EditorState::StringList { items, .. } => serde_json::json!(items),
        EditorState::Select { options, cursor } => {
            let selected = &options[*cursor];
            if selected == "(none)" {
                Value::Null
            } else {
                serde_json::json!(selected)
            }
        }
        EditorState::Number { input } => {
            if input.is_empty() {
                Value::Null
            } else if let Ok(n) = input.parse::<u64>() {
                serde_json::json!(n)
            } else {
                Value::Null
            }
        }
        EditorState::None => return,
    };

    if &new_value != original {
        state.changes.insert(key, new_value);
    } else {
        state.changes.remove(&key);
    }

    state.view = View::ResourceList;
    state.editor = EditorState::None;
}

// ─── Drawing ─────────────────────────────────────────────────

fn draw(f: &mut ratatui::Frame, state: &State) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(1)])
        .split(f.area());

    match state.view {
        View::ResourceList => draw_resource_list(f, chunks[0], state),
        View::Editor => draw_editor(f, chunks[0], state),
    }

    draw_status_bar(f, chunks[1], state);
}

fn draw_resource_list(f: &mut ratatui::Frame, area: Rect, state: &State) {
    let mut items: Vec<ListItem> = Vec::new();
    let mut current_group = "";
    let mut item_to_res: Vec<Option<usize>> = Vec::new(); // map list index -> resource index

    for (i, def) in state.resources.iter().enumerate() {
        if def.group != current_group {
            current_group = def.group;
            items.push(ListItem::new(Line::from(vec![Span::styled(
                format!("  {current_group}"),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )])));
            item_to_res.push(None);
        }

        let value = get_value(&state.settings, &state.changes, def.key);
        let summary = summarize_value(value);
        let changed = state.changes.contains_key(def.key);
        let marker = if changed { "*" } else { " " };

        let is_selected = i == state.res_cursor;
        let pointer = if is_selected { ">" } else { " " };

        let name_style = if is_selected {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        let summary_style = if changed {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        items.push(ListItem::new(Line::from(vec![
            Span::styled(format!(" {marker}{pointer} {:<28}", def.slug), name_style),
            Span::styled(summary, summary_style),
        ])));
        item_to_res.push(Some(i));
    }

    let title = format!(" Settings: {} ", state.uid);
    let list = List::new(items).block(Block::default().title(title).borders(Borders::ALL));
    f.render_widget(list, area);
}

fn draw_editor(f: &mut ratatui::Frame, area: Rect, state: &State) {
    let def = &state.resources[state.editing_idx];
    let title = format!(" {} ", def.slug);

    match &state.editor {
        EditorState::Field {
            items,
            cursor,
            ordered,
            wildcard,
        } => draw_field_editor(f, area, items, *cursor, *ordered, *wildcard, &title),
        EditorState::StringList {
            items,
            cursor,
            input,
            adding,
        } => draw_string_list_editor(f, area, items, *cursor, input, *adding, &title),
        EditorState::Select { options, cursor } => {
            draw_select_editor(f, area, options, *cursor, &title)
        }
        EditorState::Number { input } => draw_number_editor(f, area, input, &title),
        EditorState::None => {}
    }
}

fn draw_field_editor(
    f: &mut ratatui::Frame,
    area: Rect,
    items: &[FieldItem],
    cursor: usize,
    ordered: bool,
    wildcard: bool,
    title: &str,
) {
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(2)])
        .split(area);

    let mut list_items: Vec<ListItem> = Vec::new();

    if wildcard {
        list_items.push(ListItem::new(Line::from(vec![Span::styled(
            "   [*] All fields (wildcard)",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::ITALIC),
        )])));
        for item in items {
            list_items.push(ListItem::new(Line::from(vec![Span::styled(
                format!("       {}", item.name),
                Style::default().fg(Color::DarkGray),
            )])));
        }
    } else {
        let mut checked_count = 0;
        for (i, item) in items.iter().enumerate() {
            let is_selected = i == cursor;
            let pointer = if is_selected { ">" } else { " " };

            let (checkbox, name_style) = if item.checked {
                checked_count += 1;
                let cb = if ordered {
                    format!("[{checked_count}]")
                } else {
                    "[x]".to_string()
                };
                let style = if is_selected {
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Green)
                };
                (cb, style)
            } else {
                let style = if is_selected {
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                ("[ ]".to_string(), style)
            };

            list_items.push(ListItem::new(Line::from(vec![Span::styled(
                format!(" {pointer} {checkbox} {}", item.name),
                name_style,
            )])));
        }
    }

    let list = List::new(list_items).block(
        Block::default()
            .title(title.to_string())
            .borders(Borders::ALL),
    );
    f.render_widget(list, inner[0]);

    let hints = if ordered {
        " Space toggle | Shift+Up/Down reorder | * wildcard | Esc back "
    } else {
        " Space toggle | Esc back "
    };
    let hint_bar = Paragraph::new(Line::from(vec![Span::styled(
        hints,
        Style::default().fg(Color::DarkGray),
    )]));
    f.render_widget(hint_bar, inner[1]);
}

fn draw_string_list_editor(
    f: &mut ratatui::Frame,
    area: Rect,
    items: &[String],
    cursor: usize,
    input: &str,
    adding: bool,
    title: &str,
) {
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(3),
            Constraint::Length(2),
        ])
        .split(area);

    // List
    let list_items: Vec<ListItem> = items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let is_selected = i == cursor && !adding;
            let pointer = if is_selected { ">" } else { " " };
            let style = if is_selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Line::from(vec![Span::styled(
                format!(" {pointer} {}. {item}", i + 1),
                style,
            )]))
        })
        .collect();

    let list = List::new(list_items).block(
        Block::default()
            .title(title.to_string())
            .borders(Borders::ALL),
    );
    f.render_widget(list, inner[0]);

    // Input field
    let input_style = if adding {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let input_widget = Paragraph::new(input)
        .block(
            Block::default()
                .title(if adding {
                    " Add item (Enter to confirm, Esc to cancel) "
                } else {
                    " Press 'a' to add "
                })
                .borders(Borders::ALL),
        )
        .style(input_style);
    f.render_widget(input_widget, inner[1]);

    if adding {
        let cursor_x = inner[1].x + input.len() as u16 + 1;
        let cursor_y = inner[1].y + 1;
        f.set_cursor_position((cursor_x, cursor_y));
    }

    // Hints
    let hints = if adding {
        " Enter confirm | Esc cancel "
    } else {
        " a add | d delete | Shift+Up/Down reorder | Esc back "
    };
    let hint_bar = Paragraph::new(Line::from(vec![Span::styled(
        hints,
        Style::default().fg(Color::DarkGray),
    )]));
    f.render_widget(hint_bar, inner[2]);
}

fn draw_select_editor(
    f: &mut ratatui::Frame,
    area: Rect,
    options: &[String],
    cursor: usize,
    title: &str,
) {
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(2)])
        .split(area);

    let list_items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, opt)| {
            let is_selected = i == cursor;
            let radio = if is_selected { "(*)" } else { "( )" };
            let style = if is_selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Line::from(vec![Span::styled(
                format!("  {radio} {opt}"),
                style,
            )]))
        })
        .collect();

    let list = List::new(list_items).block(
        Block::default()
            .title(title.to_string())
            .borders(Borders::ALL),
    );
    f.render_widget(list, inner[0]);

    let hints = " Up/Down navigate | Enter select | Esc back ";
    let hint_bar = Paragraph::new(Line::from(vec![Span::styled(
        hints,
        Style::default().fg(Color::DarkGray),
    )]));
    f.render_widget(hint_bar, inner[1]);
}

fn draw_number_editor(f: &mut ratatui::Frame, area: Rect, input: &str, title: &str) {
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(2)])
        .split(area);

    let display = if input.is_empty() {
        "(null / default)"
    } else {
        input
    };
    let content = Paragraph::new(vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            format!("  Value: {display}"),
            Style::default().fg(Color::Yellow),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Type a number, or leave empty for default/null.",
            Style::default().fg(Color::DarkGray),
        )]),
    ])
    .block(
        Block::default()
            .title(title.to_string())
            .borders(Borders::ALL),
    );
    f.render_widget(content, inner[0]);

    let hints = " Type digits | Backspace clear | Enter confirm | Esc back ";
    let hint_bar = Paragraph::new(Line::from(vec![Span::styled(
        hints,
        Style::default().fg(Color::DarkGray),
    )]));
    f.render_widget(hint_bar, inner[1]);
}

fn draw_status_bar(f: &mut ratatui::Frame, area: Rect, state: &State) {
    let n_changes = state.changes.len();
    let changes_text = if n_changes > 0 {
        format!(" | {n_changes} unsaved change(s) — press s to save")
    } else {
        String::new()
    };

    let msg = if !state.status_msg.is_empty() {
        format!(" {}{changes_text}", state.status_msg)
    } else if state.view == View::ResourceList {
        format!(" Up/Down navigate | Enter edit | s save | q quit{changes_text}")
    } else {
        changes_text
    };

    let bar = Paragraph::new(Line::from(vec![Span::styled(
        msg,
        Style::default().fg(Color::DarkGray),
    )]));
    f.render_widget(bar, area);
}

// ─── Input Handling ──────────────────────────────────────────

fn handle_input(key: event::KeyEvent, state: &mut State) -> Action {
    // Global: Ctrl+C always quits
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Action::Quit;
    }

    state.status_msg.clear();

    match state.view {
        View::ResourceList => handle_resource_list_input(key, state),
        View::Editor => handle_editor_input(key, state),
    }
}

fn handle_resource_list_input(key: event::KeyEvent, state: &mut State) -> Action {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            if !state.changes.is_empty() && !state.confirm_quit {
                state.status_msg =
                    "Unsaved changes! Press q again to discard, or s to save.".into();
                state.confirm_quit = true;
                return Action::Continue;
            }
            Action::Quit
        }
        KeyCode::Char('s') => Action::Save,
        KeyCode::Up => {
            if state.res_cursor > 0 {
                state.res_cursor -= 1;
            }
            Action::Continue
        }
        KeyCode::Down => {
            if state.res_cursor + 1 < state.resources.len() {
                state.res_cursor += 1;
            }
            Action::Continue
        }
        KeyCode::Enter => {
            let idx = state.res_cursor;
            let kind = state.resources[idx].kind.clone();
            match kind {
                EditorKind::JsonEditor => Action::LaunchEditor(idx),
                _ => {
                    enter_editor(state, idx);
                    Action::Continue
                }
            }
        }
        _ => Action::Continue,
    }
}

fn handle_editor_input(key: event::KeyEvent, state: &mut State) -> Action {
    match &mut state.editor {
        EditorState::Field {
            items,
            cursor,
            ordered,
            wildcard,
        } => {
            if *wildcard {
                // In wildcard mode, only * or Esc work
                match key.code {
                    KeyCode::Char('*') => {
                        *wildcard = false;
                        // All items already checked from wildcard build
                    }
                    KeyCode::Esc => {
                        exit_editor(state);
                    }
                    _ => {}
                }
                return Action::Continue;
            }

            let shift = key.modifiers.contains(KeyModifiers::SHIFT);
            match key.code {
                KeyCode::Esc => {
                    exit_editor(state);
                }
                KeyCode::Up
                    if shift && *ordered
                        && items[*cursor].checked
                        && *cursor > 0
                        && items[*cursor - 1].checked =>
                {
                    items.swap(*cursor, *cursor - 1);
                    *cursor -= 1;
                }
                KeyCode::Up if shift && *ordered => {}
                KeyCode::Down
                    if shift && *ordered
                        && items[*cursor].checked
                        && *cursor + 1 < items.len()
                        && items[*cursor + 1].checked =>
                {
                    items.swap(*cursor, *cursor + 1);
                    *cursor += 1;
                }
                KeyCode::Down if shift && *ordered => {}
                KeyCode::Up if *cursor > 0 => {
                    *cursor -= 1;
                }
                KeyCode::Up => {}
                KeyCode::Down if *cursor + 1 < items.len() => {
                    *cursor += 1;
                }
                KeyCode::Down => {}
                KeyCode::Char(' ') => {
                    if *ordered {
                        let name = items[*cursor].name.clone();
                        let was_checked = items[*cursor].checked;

                        if was_checked {
                            // Uncheck: remove from checked, move to unchecked section
                            items.remove(*cursor);
                            // Re-insert alphabetically in unchecked section
                            let unchecked_start =
                                items.iter().position(|i| !i.checked).unwrap_or(items.len());
                            let insert_pos = items[unchecked_start..]
                                .iter()
                                .position(|i| i.name > name)
                                .map(|p| p + unchecked_start)
                                .unwrap_or(items.len());
                            items.insert(
                                insert_pos,
                                FieldItem {
                                    name,
                                    checked: false,
                                },
                            );
                            if *cursor >= items.len() {
                                *cursor = items.len().saturating_sub(1);
                            }
                        } else {
                            // Check: remove from unchecked, append to checked section
                            items.remove(*cursor);
                            let checked_count = items.iter().filter(|i| i.checked).count();
                            items.insert(
                                checked_count,
                                FieldItem {
                                    name,
                                    checked: true,
                                },
                            );
                            *cursor = checked_count; // Follow the item
                        }
                    } else {
                        items[*cursor].checked = !items[*cursor].checked;
                    }
                }
                KeyCode::Char('*') if *ordered => {
                    *wildcard = true;
                }
                _ => {}
            }
            Action::Continue
        }
        EditorState::StringList {
            items,
            cursor,
            input,
            adding,
        } => {
            if *adding {
                match key.code {
                    KeyCode::Enter => {
                        if !input.is_empty() {
                            items.push(input.clone());
                            *cursor = items.len().saturating_sub(1);
                            input.clear();
                        }
                        *adding = false;
                    }
                    KeyCode::Esc => {
                        input.clear();
                        *adding = false;
                    }
                    KeyCode::Char(c) => {
                        input.push(c);
                    }
                    KeyCode::Backspace => {
                        input.pop();
                    }
                    _ => {}
                }
            } else {
                let shift = key.modifiers.contains(KeyModifiers::SHIFT);
                match key.code {
                    KeyCode::Esc => {
                        exit_editor(state);
                    }
                    KeyCode::Up if shift && *cursor > 0 => {
                        items.swap(*cursor, *cursor - 1);
                        *cursor -= 1;
                    }
                    KeyCode::Up if shift => {}
                    KeyCode::Down if shift && *cursor + 1 < items.len() => {
                        items.swap(*cursor, *cursor + 1);
                        *cursor += 1;
                    }
                    KeyCode::Down if shift => {}
                    KeyCode::Up if *cursor > 0 => {
                        *cursor -= 1;
                    }
                    KeyCode::Up => {}
                    KeyCode::Down if *cursor + 1 < items.len() => {
                        *cursor += 1;
                    }
                    KeyCode::Down => {}
                    KeyCode::Char('a') => {
                        *adding = true;
                    }
                    KeyCode::Char('d') | KeyCode::Delete if !items.is_empty() => {
                        items.remove(*cursor);
                        if *cursor >= items.len() && !items.is_empty() {
                            *cursor = items.len() - 1;
                        }
                    }
                    _ => {}
                }
            }
            Action::Continue
        }
        EditorState::Select { options, cursor } => {
            match key.code {
                KeyCode::Esc => {
                    exit_editor(state);
                }
                KeyCode::Up if *cursor > 0 => {
                    *cursor -= 1;
                }
                KeyCode::Up => {}
                KeyCode::Down if *cursor + 1 < options.len() => {
                    *cursor += 1;
                }
                KeyCode::Down => {}
                KeyCode::Enter | KeyCode::Char(' ') => {
                    exit_editor(state);
                }
                _ => {}
            }
            Action::Continue
        }
        EditorState::Number { input } => {
            match key.code {
                KeyCode::Esc => {
                    exit_editor(state);
                }
                KeyCode::Enter => {
                    exit_editor(state);
                }
                KeyCode::Char(c) if c.is_ascii_digit() => {
                    input.push(c);
                }
                KeyCode::Backspace => {
                    input.pop();
                }
                _ => {}
            }
            Action::Continue
        }
        EditorState::None => {
            if key.code == KeyCode::Esc {
                state.view = View::ResourceList;
            }
            Action::Continue
        }
    }
}

// ─── Save ────────────────────────────────────────────────────

async fn save_changes(client: &MeiliClient, state: &State) -> Result<String> {
    if state.changes.is_empty() {
        return Ok("No changes to save.".into());
    }

    let mut settings = serde_json::Map::new();
    for (key, value) in &state.changes {
        settings.insert(key.clone(), value.clone());
    }

    let result = client
        .update_settings(&state.uid, &Value::Object(settings))
        .await?;

    if let Some(task_uid) = result["taskUid"].as_u64() {
        let task = client.wait_for_task(task_uid, 30_000).await?;
        let status = task["status"].as_str().unwrap_or("unknown");
        if status == "failed" {
            let msg = task["error"]["message"].as_str().unwrap_or("unknown error");
            return Ok(format!("Task failed: {msg}"));
        }
        Ok(format!("Saved! Task {task_uid} {status}."))
    } else {
        Ok("Settings update queued.".into())
    }
}

async fn launch_json_editor(
    _client: &MeiliClient,
    uid: &str,
    slug: &str,
    key: &str,
    state: &mut State,
) -> Result<()> {
    let value = get_value(&state.settings, &state.changes, key).clone();
    let pretty = serde_json::to_string_pretty(&value)?;

    let tmp_path = std::env::temp_dir().join(format!("msc-{slug}-{uid}.json"));
    std::fs::write(&tmp_path, &pretty)?;

    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| {
            if which::which("vim").is_ok() {
                "vim".to_string()
            } else if which::which("nano").is_ok() {
                "nano".to_string()
            } else {
                "vi".to_string()
            }
        });

    let status = std::process::Command::new(&editor)
        .arg(&tmp_path)
        .status()
        .with_context(|| format!("Failed to open editor: {editor}"))?;

    if !status.success() {
        let _ = std::fs::remove_file(&tmp_path);
        state.status_msg = "Editor exited with error.".into();
        return Ok(());
    }

    let new_content = std::fs::read_to_string(&tmp_path)?;
    let _ = std::fs::remove_file(&tmp_path);

    if new_content == pretty {
        state.status_msg = "No changes.".into();
        return Ok(());
    }

    let new_value: Value =
        serde_json::from_str(&new_content).context("Failed to parse edited JSON")?;

    let original = &state.settings[key];
    if &new_value != original {
        state.changes.insert(key.to_string(), new_value);
        state.status_msg = format!("Changed {slug}. Press s to save.");
    } else {
        state.changes.remove(key);
    }

    Ok(())
}

// ─── Entry Point ─────────────────────────────────────────────

pub async fn run_interactive_settings(client: MeiliClient, uid: &str) -> Result<()> {
    // Fetch data
    let stats = client.index_stats(uid).await?;
    let settings = client.get_settings(uid).await?;

    let mut fields: Vec<String> = stats["fieldDistribution"]
        .as_object()
        .map(|obj| obj.keys().cloned().collect())
        .unwrap_or_default();
    fields.sort();

    let resources = resource_defs();

    let mut state = State {
        uid: uid.to_string(),
        settings,
        changes: HashMap::new(),
        fields,
        resources,
        view: View::ResourceList,
        status_msg: String::new(),
        confirm_quit: false,
        res_cursor: 0,
        editing_idx: 0,
        editor: EditorState::None,
    };

    // Terminal setup
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    loop {
        terminal.draw(|f| draw(f, &state))?;

        if event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
        {
            match handle_input(key, &mut state) {
                Action::Continue => {}
                Action::Quit => break,
                Action::Save => {
                    state.status_msg = "Saving...".into();
                    terminal.draw(|f| draw(f, &state))?;

                    match save_changes(&client, &state).await {
                        Ok(msg) => {
                            state.status_msg = msg;
                            state.changes.clear();
                            if let Ok(s) = client.get_settings(&state.uid).await {
                                state.settings = s;
                            }
                        }
                        Err(e) => {
                            state.status_msg = format!("Error: {e}");
                        }
                    }
                }
                Action::LaunchEditor(idx) => {
                    // Leave TUI, launch $EDITOR, re-enter TUI
                    terminal::disable_raw_mode()?;
                    crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

                    let slug = state.resources[idx].slug.to_string();
                    let key = state.resources[idx].key.to_string();
                    let uid = state.uid.clone();
                    let _ = launch_json_editor(&client, &uid, &slug, &key, &mut state).await;

                    // Re-enter TUI
                    terminal::enable_raw_mode()?;
                    let mut stdout = io::stdout();
                    crossterm::execute!(stdout, EnterAlternateScreen)?;
                    terminal = Terminal::new(CrosstermBackend::new(stdout))?;
                }
            }
        }
    }

    // Cleanup
    terminal::disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}
