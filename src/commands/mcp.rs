//! `msc mcp`: a Model Context Protocol server over stdio (newline-delimited
//! JSON-RPC 2.0). Every runnable command becomes a tool, generated from the
//! same clap tree as `msc schema`; a tool call runs `msc` itself with `--json`
//! so behavior, output and errors are identical to the CLI.

use std::process::Stdio;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Args;
use serde_json::{Map, Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use super::Cli;
use super::schema::{self, ArgSpec, ArgType, CommandSpec};

#[derive(Args)]
pub struct McpArgs {
    /// Only expose read-only tools (list, get, search, …)
    #[arg(long)]
    pub read_only: bool,

    /// Only expose these tools (comma-separated names; a trailing * matches a prefix, e.g. index_*,search)
    #[arg(long, value_delimiter = ',')]
    pub tools: Option<Vec<String>>,
}

const PROTOCOL_VERSIONS: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];

/// Commands that need a terminal, never terminate, or make no sense as tools.
const EXCLUDED: &[&[&str]] = &[
    &["mcp"],
    &["schema"],
    &["completions"],
    &["self-update"],
    &["settings", "edit"],
    &["task", "watch"],
    &["log", "stream"],
    &["skill", "list"],
    &["skill", "show"],
    &["skill", "install"],
    &["skill", "uninstall"],
];

/// Arguments dropped from tools (interactive or streaming modes).
const EXCLUDED_ARGS: &[&str] = &["interactive", "events", "follow"];

const READ_ONLY: &[&str] = &[
    "list",
    "get",
    "stats",
    "search",
    "multi-search",
    "facet-search",
    "similar",
    "health",
    "version",
    "metrics",
    "current",
    "status",
    "diff",
    "fetch",
    "wait",
    "whoami",
];

const INSTRUCTIONS: &str = "Tools for a Meilisearch instance, backed by the `msc` CLI. \
Write operations (adding documents, changing settings, creating or deleting indexes) are \
asynchronous: they return a summarized task with a `taskUid`. Pass `wait: true` to get the \
finished task instead, or call `task_wait`. Use `select` to keep only the fields you need \
from large results. Errors come back as JSON with `kind`, `code`, `message` and often a `hint`.";

struct Tool {
    name: String,
    spec: CommandSpec,
    read_only: bool,
}

impl Tool {
    fn args(&self) -> impl Iterator<Item = &ArgSpec> {
        self.spec
            .args
            .iter()
            .filter(|a| !EXCLUDED_ARGS.contains(&a.name.as_str()))
    }

    fn takes_input(&self) -> bool {
        self.spec.args.iter().any(|a| a.name == "file")
    }

    fn definition(&self) -> Value {
        let mut properties = Map::new();
        let mut required = vec![];
        for arg in self.args() {
            properties.insert(arg.name.clone(), property_schema(arg));
            if arg.required {
                required.push(arg.name.clone());
            }
        }
        if self.takes_input() {
            properties.insert(
                "input".into(),
                json!({
                    "description": "Inline content to send instead of `file` (a JSON value, or a string for NDJSON/CSV)",
                }),
            );
        }
        properties.insert(
            "project".into(),
            json!({"type": "string", "description": "msc project name"}),
        );
        properties.insert(
            "select".into(),
            json!({"type": "array", "items": {"type": "string"}, "description": "Fields to keep (dot paths)"}),
        );
        if !self.read_only {
            properties.insert(
                "wait".into(),
                json!({"type": "boolean", "description": "Return the finished task"}),
            );
        }

        let path = self.spec.path.join(" ");
        let leaf = self.spec.path.last().map(String::as_str).unwrap_or("");
        let destructive = !self.read_only
            && ([
                "delete", "reset", "remove", "swap", "cancel", "stop", "promote", "api",
            ]
            .iter()
            .any(|w| leaf.contains(w)));
        json!({
            "name": self.name,
            "description": format!("{} (`msc {path}`)", self.spec.about),
            "inputSchema": {
                "type": "object",
                "properties": properties,
                "required": required,
                "additionalProperties": false,
            },
            "annotations": {
                "title": format!("msc {path}"),
                "readOnlyHint": self.read_only,
                "destructiveHint": destructive,
                "openWorldHint": false,
            },
        })
    }

    /// Translate tool arguments into a `msc` argv and optional stdin payload.
    fn argv(
        &self,
        arguments: &Map<String, Value>,
    ) -> Result<(Vec<String>, Option<Vec<u8>>), String> {
        let mut argv: Vec<String> = self.spec.path.clone();
        argv.extend(["--json".into(), "--quiet".into()]);
        let mut positionals = vec![];
        let mut input = None;

        for (key, value) in arguments {
            if value.is_null() {
                continue;
            }
            match key.as_str() {
                "input" if self.takes_input() => {
                    input = Some(match value {
                        Value::String(s) => s.clone().into_bytes(),
                        other => serde_json::to_vec(other).unwrap_or_default(),
                    });
                    continue;
                }
                "project" => {
                    argv.extend(["--project".into(), scalar(value)]);
                    continue;
                }
                "select" => {
                    argv.extend(["--select".into(), list(value).join(",")]);
                    continue;
                }
                "wait" if !self.read_only => {
                    if value.as_bool() == Some(true) {
                        argv.push("--wait".into());
                    }
                    continue;
                }
                _ => {}
            }
            let Some(arg) = self.args().find(|a| &a.name == key) else {
                return Err(format!("Unknown argument `{key}` for tool `{}`", self.name));
            };
            if arg.positional {
                positionals.push((arg.name.clone(), value.clone()));
                continue;
            }
            let flag = format!("--{}", arg.long.as_deref().unwrap_or(&arg.name));
            if arg.ty == ArgType::Boolean {
                if value.as_bool() == Some(true) {
                    argv.push(flag);
                }
            } else if arg.multiple {
                let values = list(value);
                match arg.delimiter {
                    Some(d) => argv.extend([flag, values.join(&d.to_string())]),
                    None => {
                        for v in values {
                            argv.extend([flag.clone(), v]);
                        }
                    }
                }
            } else {
                argv.extend([flag, scalar(value)]);
            }
        }

        // Positionals go last, after `--`, in declaration order.
        let order: Vec<&str> = self
            .args()
            .filter(|a| a.positional)
            .map(|a| a.name.as_str())
            .collect();
        positionals.sort_by_key(|(name, _)| order.iter().position(|n| n == name));
        if !positionals.is_empty() {
            argv.push("--".into());
            for (_, v) in positionals {
                argv.extend(list(&v));
            }
        }
        Ok((argv, input))
    }
}

fn property_schema(arg: &ArgSpec) -> Value {
    let item_type = arg.ty_name();
    let mut v = if arg.multiple {
        json!({"type": "array", "items": {"type": item_type}})
    } else {
        json!({"type": item_type})
    };
    if !arg.help.is_empty() {
        v["description"] = json!(arg.help);
    }
    if !arg.possible_values.is_empty() {
        v["enum"] = json!(arg.possible_values);
    }
    if let Some(d) = &arg.default {
        v["default"] = match arg.ty {
            ArgType::Integer => d.parse::<i64>().map(Value::from).unwrap_or(json!(d)),
            _ => json!(d),
        };
    }
    v
}

impl ArgSpec {
    fn ty_name(&self) -> &'static str {
        match self.ty {
            ArgType::Boolean => "boolean",
            ArgType::Integer => "integer",
            ArgType::String => "string",
        }
    }
}

fn scalar(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn list(v: &Value) -> Vec<String> {
    match v {
        Value::Array(items) => items.iter().map(scalar).collect(),
        other => vec![scalar(other)],
    }
}

fn matches(name: &str, patterns: &[String]) -> bool {
    patterns.iter().any(|p| match p.strip_suffix('*') {
        Some(prefix) => name.starts_with(prefix),
        None => name == p,
    })
}

fn tools() -> Vec<Tool> {
    let root = schema::root();
    root.leaves()
        .into_iter()
        .filter(|c| {
            !EXCLUDED
                .iter()
                .any(|ex| c.path.iter().map(String::as_str).eq(ex.iter().copied()))
        })
        .map(|c| Tool {
            name: c.path.join("_").replace('-', "_"),
            read_only: c
                .path
                .last()
                .is_some_and(|l| READ_ONLY.contains(&l.as_str())),
            spec: c.clone(),
        })
        .collect()
}

pub async fn run(cli: &Cli, args: &McpArgs) -> Result<()> {
    let tools: Vec<Tool> = tools()
        .into_iter()
        .filter(|t| !args.read_only || t.read_only)
        .filter(|t| args.tools.as_ref().is_none_or(|p| matches(&t.name, p)))
        .collect();
    let exe = std::env::current_exe().context("Cannot locate the msc executable")?;
    // Forward connection settings given to `msc mcp` to every tool call.
    let mut env = vec![];
    if let Some(p) = &cli.project {
        env.push(("MSC_PROJECT", p.clone()));
    }
    if let Some(u) = &cli.url {
        env.push(("MSC_URL", u.clone()));
    }
    if let Some(k) = &cli.api_key {
        env.push(("MSC_API_KEY", k.clone()));
    }

    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Value>(&line) {
            Ok(msg) => handle(&msg, &tools, &exe, &env).await,
            Err(e) => Some(rpc_error(Value::Null, -32700, &format!("Parse error: {e}"))),
        };
        if let Some(resp) = response {
            let mut out = serde_json::to_vec(&resp)?;
            out.push(b'\n');
            stdout.write_all(&out).await?;
            stdout.flush().await?;
        }
    }
    Ok(())
}

async fn handle(
    msg: &Value,
    tools: &[Tool],
    exe: &std::path::Path,
    env: &[(&str, String)],
) -> Option<Value> {
    let id = msg.get("id").cloned();
    let method = msg["method"].as_str().unwrap_or("");
    // Notifications get no response.
    let id = id?;

    let result = match method {
        "initialize" => {
            let requested = msg["params"]["protocolVersion"].as_str().unwrap_or("");
            let version = if PROTOCOL_VERSIONS.contains(&requested) {
                requested
            } else {
                PROTOCOL_VERSIONS[0]
            };
            json!({
                "protocolVersion": version,
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": { "name": "msc", "version": env!("CARGO_PKG_VERSION") },
                "instructions": INSTRUCTIONS,
            })
        }
        "ping" => json!({}),
        "tools/list" => json!({ "tools": tools.iter().map(Tool::definition).collect::<Vec<_>>() }),
        "tools/call" => {
            let name = msg["params"]["name"].as_str().unwrap_or("");
            let Some(tool) = tools.iter().find(|t| t.name == name) else {
                return Some(rpc_error(id, -32602, &format!("Unknown tool: {name}")));
            };
            let empty = Map::new();
            let arguments = msg["params"]["arguments"].as_object().unwrap_or(&empty);
            match tool.argv(arguments) {
                Ok((argv, input)) => call(exe, env, &argv, input).await,
                Err(e) => tool_result(&e, true),
            }
        }
        _ => {
            return Some(rpc_error(
                id,
                -32601,
                &format!("Method not found: {method}"),
            ));
        }
    };
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

async fn call(
    exe: &std::path::Path,
    env: &[(&str, String)],
    argv: &[String],
    input: Option<Vec<u8>>,
) -> Value {
    let mut cmd = tokio::process::Command::new(exe);
    cmd.args(argv)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return tool_result(&format!("Failed to run msc: {e}"), true),
    };
    if let (Some(data), Some(mut stdin)) = (input, child.stdin.take()) {
        tokio::spawn(async move {
            let _ = stdin.write_all(&data).await;
        });
    }
    let timeout = std::env::var("MSC_MCP_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(600);
    let output =
        match tokio::time::timeout(Duration::from_secs(timeout), child.wait_with_output()).await {
            Ok(Ok(o)) => o,
            Ok(Err(e)) => return tool_result(&format!("Failed to run msc: {e}"), true),
            Err(_) => return tool_result(&format!("msc timed out after {timeout}s"), true),
        };
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if output.status.success() {
        let text = if stdout.is_empty() {
            "{\"ok\":true}".to_string()
        } else {
            stdout
        };
        tool_result(&text, false)
    } else {
        let text = [stdout, stderr]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        tool_result(&text, true)
    }
}

fn tool_result(text: &str, is_error: bool) -> Value {
    json!({ "content": [{ "type": "text", "text": text }], "isError": is_error })
}

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(name: &str) -> Tool {
        tools().into_iter().find(|t| t.name == name).unwrap()
    }

    #[test]
    fn tools_exclude_interactive_commands() {
        let names: Vec<String> = tools().into_iter().map(|t| t.name).collect();
        assert!(names.contains(&"index_create".to_string()));
        assert!(names.contains(&"multi_search".to_string()));
        assert!(
            !names
                .iter()
                .any(|n| n == "settings_edit" || n == "mcp" || n == "task_watch")
        );
        let search = tool("search").definition();
        assert!(
            search["inputSchema"]["properties"]
                .get("interactive")
                .is_none()
        );
        assert_eq!(search["annotations"]["readOnlyHint"], true);
        assert!(search["inputSchema"]["properties"].get("wait").is_none());
    }

    #[test]
    fn argv_maps_flags_positionals_and_lists() {
        let t = tool("search");
        let args = json!({"uid": "movies", "query": "-dash", "limit": 5, "facets": ["a", "b"], "select": ["id"]});
        let (argv, input) = t.argv(args.as_object().unwrap()).unwrap();
        assert!(input.is_none());
        assert_eq!(&argv[..3], &["search", "--json", "--quiet"]);
        let tail = argv.iter().position(|a| a == "--").unwrap();
        assert_eq!(&argv[tail + 1..], &["movies", "-dash"]);
        assert!(argv.windows(2).any(|w| w == ["--limit", "5"]));
        assert!(argv.windows(2).any(|w| w == ["--facets", "a,b"]));
        assert!(argv.windows(2).any(|w| w == ["--select", "id"]));
    }

    #[test]
    fn argv_passes_input_on_stdin() {
        let t = tool("document_add");
        let args = json!({"uid": "movies", "input": [{"id": 1}], "wait": true});
        let (argv, input) = t.argv(args.as_object().unwrap()).unwrap();
        assert_eq!(input.unwrap(), br#"[{"id":1}]"#);
        assert!(argv.contains(&"--wait".to_string()));
    }

    #[test]
    fn tool_name_patterns() {
        let p = vec!["index_*".to_string(), "search".to_string()];
        assert!(matches("index_create", &p));
        assert!(matches("search", &p));
        assert!(!matches("search_x", &p));
        assert!(!matches("document_add", &p));
    }

    #[test]
    fn argv_rejects_unknown_arguments() {
        let t = tool("index_get");
        assert!(t.argv(json!({"nope": 1}).as_object().unwrap()).is_err());
    }
}
