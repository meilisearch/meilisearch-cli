//! Process-wide output settings and helpers.
//!
//! Rules: stdout carries only the command result (JSON when piped or with
//! `--json`); progress and status text goes to stderr via [`status!`].

use std::io::{IsTerminal, Write};
use std::sync::OnceLock;

use serde_json::{Map, Value};

use crate::error::CliError;

#[derive(Debug, Clone)]
pub struct OutputCtx {
    /// Machine-readable output: compact JSON on stdout, JSON errors on stderr.
    pub json: bool,
    /// Suppress status messages on stderr.
    pub quiet: bool,
    /// Colorize pretty JSON.
    pub color: bool,
    /// Render results as a text table (`--table`).
    pub table: bool,
    /// Dot-path projection applied to JSON results (`--select`).
    pub select: Option<Vec<String>>,
}

impl OutputCtx {
    /// `json`/`pretty` are the explicit flags; without either, JSON is used
    /// whenever stdout is not a terminal.
    pub fn new(
        json: bool,
        pretty: bool,
        table: bool,
        quiet: bool,
        select: Option<Vec<String>>,
    ) -> Self {
        let stdout_tty = std::io::stdout().is_terminal();
        // An explicit --table or --pretty wins over JSON auto-detection.
        let json = json || (!pretty && !table && !stdout_tty);
        let no_color = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
        Self {
            json,
            quiet,
            color: !json && stdout_tty && !no_color,
            table: table && !json,
            select,
        }
    }
}

static CTX: OnceLock<OutputCtx> = OnceLock::new();

pub fn init(ctx: OutputCtx) {
    let _ = CTX.set(ctx);
}

pub fn ctx() -> &'static OutputCtx {
    CTX.get_or_init(|| OutputCtx::new(false, false, false, false, None))
}

pub fn is_json() -> bool {
    ctx().json
}

/// True when both stdin and stdout are attached to a terminal.
pub fn is_interactive() -> bool {
    std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
}

/// Fail fast instead of prompting or opening a TUI/editor without a terminal.
pub fn require_interactive(what: &str, alternative: &str) -> anyhow::Result<()> {
    if is_interactive() {
        return Ok(());
    }
    Err(CliError::usage(
        "non_interactive",
        format!("{what} needs an interactive terminal"),
    )
    .with_hint(alternative.to_string())
    .into())
}

/// Print a status/progress line to stderr. Silenced by `--quiet` and in JSON
/// mode, where stderr is reserved for the JSON error object.
#[macro_export]
macro_rules! status {
    ($($arg:tt)*) => {
        if !$crate::output::ctx().quiet && !$crate::output::ctx().json {
            eprintln!($($arg)*);
        }
    };
}

/// Print a JSON result to stdout, honoring `--select`, `--json` and colors.
pub fn print_json(value: &Value) {
    let ctx = ctx();
    let value = match &ctx.select {
        Some(paths) => select(value, paths),
        None => value.clone(),
    };
    let text = if ctx.table {
        table(&value)
    } else if ctx.json {
        serde_json::to_string(&value).unwrap_or_default()
    } else if ctx.color {
        colored_json::to_colored_json(&value, colored_json::ColorMode::On)
            .unwrap_or_else(|_| serde_json::to_string_pretty(&value).unwrap_or_default())
    } else {
        serde_json::to_string_pretty(&value).unwrap_or_default()
    };
    println!("{text}");
}

const MAX_CELL: usize = 48;

/// Render a value as a text table: lists (`results`, `hits`, arrays) become
/// one row per element; a single object becomes key/value rows.
pub fn table(value: &Value) -> String {
    let rows: Option<&Vec<Value>> = match value {
        Value::Array(items) => Some(items),
        Value::Object(map) => ["results", "hits"]
            .iter()
            .find_map(|k| map.get(*k).and_then(Value::as_array)),
        _ => None,
    };
    match rows {
        Some(rows) if rows.iter().all(Value::is_object) => {
            if rows.is_empty() {
                return "(no results)".to_string();
            }
            let mut columns: Vec<String> = vec![];
            for row in rows {
                for key in row.as_object().unwrap().keys() {
                    if !columns.contains(key) {
                        columns.push(key.clone());
                    }
                }
            }
            let cells: Vec<Vec<String>> = rows
                .iter()
                .map(|r| columns.iter().map(|c| cell(r.get(c))).collect())
                .collect();
            render(&columns, &cells)
        }
        Some(rows) => render(
            &["value".to_string()],
            &rows.iter().map(|v| vec![cell(Some(v))]).collect::<Vec<_>>(),
        ),
        None => match value.as_object() {
            Some(map) => render(
                &["key".to_string(), "value".to_string()],
                &map.iter()
                    .map(|(k, v)| vec![k.clone(), cell(Some(v))])
                    .collect::<Vec<_>>(),
            ),
            None => cell(Some(value)),
        },
    }
}

fn cell(value: Option<&Value>) -> String {
    let text = match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
    };
    let text = text.replace(['\n', '\t'], " ");
    if text.chars().count() > MAX_CELL {
        format!("{}…", text.chars().take(MAX_CELL - 1).collect::<String>())
    } else {
        text
    }
}

fn render(columns: &[String], rows: &[Vec<String>]) -> String {
    let widths: Vec<usize> = columns
        .iter()
        .enumerate()
        .map(|(i, c)| {
            rows.iter()
                .map(|r| r[i].chars().count())
                .chain(std::iter::once(c.chars().count()))
                .max()
                .unwrap_or(0)
        })
        .collect();
    let line = |cells: &[String]| {
        cells
            .iter()
            .zip(&widths)
            .map(|(c, w)| format!("{c:<w$}"))
            .collect::<Vec<_>>()
            .join("  ")
            .trim_end()
            .to_string()
    };
    let mut out = vec![
        line(columns),
        line(&widths.iter().map(|w| "-".repeat(*w)).collect::<Vec<_>>()),
    ];
    out.extend(rows.iter().map(|r| line(r)));
    out.join("\n")
}

/// Print a result: JSON in machine mode, `human` text otherwise.
pub fn emit(value: &Value, human: impl FnOnce() -> String) {
    if is_json() {
        print_json(value);
    } else {
        println!("{}", human());
    }
}

/// Print one NDJSON event line to stdout and flush immediately.
pub fn event(value: &Value) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{}", serde_json::to_string(value).unwrap_or_default());
    let _ = out.flush();
}

/// Keep only the given dot-paths. Lists (`results`, `hits`, top-level arrays)
/// are projected element-wise, keeping pagination metadata.
pub fn select(value: &Value, paths: &[String]) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(|v| project(v, paths)).collect()),
        Value::Object(map) => {
            for key in ["results", "hits"] {
                if let Some(Value::Array(items)) = map.get(key) {
                    let mut out = map.clone();
                    out.insert(
                        key.to_string(),
                        Value::Array(items.iter().map(|v| project(v, paths)).collect()),
                    );
                    return Value::Object(out);
                }
            }
            project(value, paths)
        }
        other => other.clone(),
    }
}

fn project(value: &Value, paths: &[String]) -> Value {
    if !value.is_object() {
        return value.clone();
    }
    let mut out = Value::Object(Map::new());
    for path in paths {
        let parts: Vec<&str> = path.split('.').filter(|p| !p.is_empty()).collect();
        if parts.is_empty() {
            continue;
        }
        let mut cur = value;
        let mut found = true;
        for p in &parts {
            match cur.get(p) {
                Some(v) => cur = v,
                None => {
                    found = false;
                    break;
                }
            }
        }
        if found {
            insert_path(&mut out, &parts, cur.clone());
        }
    }
    out
}

fn insert_path(target: &mut Value, parts: &[&str], value: Value) {
    let mut cur = target;
    for (i, p) in parts.iter().enumerate() {
        let map = cur.as_object_mut().expect("projection target is an object");
        if i == parts.len() - 1 {
            map.insert(p.to_string(), value);
            return;
        }
        cur = map
            .entry(p.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn table_renders_lists_and_objects() {
        let v = json!({"results": [{"uid": "movies", "primaryKey": "id"}, {"uid": "books", "extra": [1, 2]}], "total": 2});
        let t = table(&v);
        let lines: Vec<&str> = t.lines().collect();
        assert_eq!(lines[0], "uid     primaryKey  extra");
        assert_eq!(lines[1], "------  ----------  -----");
        assert_eq!(lines[3], "books               [1,2]");

        let t = table(&json!({"status": "available"}));
        assert!(t.lines().nth(2).unwrap().starts_with("status"));
        assert_eq!(table(&json!([])), "(no results)");
    }

    fn paths(p: &[&str]) -> Vec<String> {
        p.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn select_projects_list_elements_and_keeps_metadata() {
        let v =
            json!({"results": [{"uid": "a", "primaryKey": "id", "x": 1}], "total": 1, "limit": 20});
        let out = select(&v, &paths(&["uid"]));
        assert_eq!(
            out,
            json!({"results": [{"uid": "a"}], "total": 1, "limit": 20})
        );
    }

    #[test]
    fn select_projects_search_hits() {
        let v = json!({"hits": [{"id": 1, "title": "t", "body": "b"}], "estimatedTotalHits": 1});
        let out = select(&v, &paths(&["id", "title"]));
        assert_eq!(out["hits"], json!([{"id": 1, "title": "t"}]));
        assert_eq!(out["estimatedTotalHits"], 1);
    }

    #[test]
    fn select_supports_nested_paths() {
        let v = json!({"uid": 3, "status": "failed", "error": {"code": "c", "message": "m"}});
        let out = select(&v, &paths(&["uid", "error.code", "missing"]));
        assert_eq!(out, json!({"uid": 3, "error": {"code": "c"}}));
    }

    #[test]
    fn select_on_arrays() {
        let v = json!([{"a": 1, "b": 2}, {"a": 3}]);
        assert_eq!(select(&v, &paths(&["a"])), json!([{"a": 1}, {"a": 3}]));
    }
}
