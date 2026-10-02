use std::collections::HashMap;
use std::io::Write;

use anyhow::Result;
use clap::Args;
use futures_util::StreamExt;
use serde_json::{Value, json};

use super::Cli;
use crate::error::{CliError, ErrorKind};
use crate::output::{event, is_json};
use crate::status;

#[derive(Args)]
pub struct ChatArgs {
    /// Chat message
    pub message: Option<String>,

    /// Workspace UID (defaults to "cloud")
    #[arg(long, default_value = "cloud")]
    pub workspace: String,

    /// Model to use (passed through to LLM provider)
    #[arg(long)]
    pub model: Option<String>,

    /// Interactive TUI mode
    #[arg(short, long)]
    pub interactive: bool,

    /// Emit NDJSON events (delta, sources, done) as the answer streams
    #[arg(long)]
    pub events: bool,
}

async fn ensure_chat_enabled(client: &crate::client::MeiliClient) -> Result<()> {
    let features = client.get_experimental_features().await?;
    if features["chatCompletions"].as_bool() != Some(true) {
        status!("Enabling chatCompletions experimental feature...");
        client
            .update_experimental_features(&json!({ "chatCompletions": true }))
            .await?;
    }
    Ok(())
}

/// Declaring this tool makes Meilisearch stream the documents it used.
fn sources_tool() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "_meiliSearchSources",
            "description": "Provides sources of the search",
            "parameters": {
                "type": "object",
                "properties": {
                    "call_id": { "type": "string", "description": "The call ID to track the original search associated to those sources." },
                    "documents": { "type": "object", "description": "The documents associated with the search (call_id). Only the displayed attributes of the documents are returned." }
                },
                "required": ["call_id", "documents"],
                "additionalProperties": false
            },
            "strict": true
        }
    })
}

/// Accumulates the streamed answer and tool-call arguments.
#[derive(Default)]
struct Collector {
    answer: String,
    sources: Vec<Value>,
    tool_names: HashMap<u64, String>,
    tool_args: HashMap<u64, String>,
}

impl Collector {
    /// Parse completed tool calls; returns newly found source documents.
    fn flush_tools(&mut self) -> Vec<Value> {
        let mut found = vec![];
        for (idx, args) in self.tool_args.drain() {
            if self.tool_names.get(&idx).map(String::as_str) != Some("_meiliSearchSources") {
                continue;
            }
            if let Ok(parsed) = serde_json::from_str::<Value>(&args) {
                match &parsed["documents"] {
                    Value::Array(docs) => found.extend(docs.iter().cloned()),
                    Value::Null => {}
                    other => found.push(other.clone()),
                }
            }
        }
        self.tool_names.clear();
        self.sources.extend(found.iter().cloned());
        found
    }
}

pub async fn run(cli: &Cli, args: &ChatArgs) -> Result<()> {
    if args.interactive {
        crate::output::require_interactive(
            "Interactive chat (-i)",
            "Run `msc chat \"<message>\"` without -i",
        )?;
    }
    let client = super::build_client(cli)?;
    ensure_chat_enabled(&client).await?;

    if args.interactive {
        return crate::tui::chat::run_interactive_chat(
            client,
            &args.workspace,
            args.model.as_deref(),
        )
        .await;
    }

    let Some(message) = args.message.as_deref() else {
        return Err(
            CliError::usage("missing_argument", "A chat message is required")
                .with_hint("Run `msc chat \"<message>\"`, or `msc chat -i` for the interactive TUI")
                .into(),
        );
    };
    let model = args.model.as_deref().unwrap_or("gpt-4o-mini");

    let body = json!({
        "model": model,
        "messages": [{ "role": "user", "content": message }],
        "stream": true,
        "tools": [sources_tool()],
    });

    let resp = client.chat_completions(&args.workspace, &body).await?;
    // Plain-text streaming only for humans; JSON callers get one result object.
    let stream_text = !is_json() && !args.events;

    let mut stream = resp.bytes_stream();
    let mut buffer = String::new();
    let mut c = Collector::default();

    'outer: while let Some(chunk) = stream.next().await {
        buffer.push_str(&String::from_utf8_lossy(&chunk?));

        while let Some(line_end) = buffer.find('\n') {
            let line = buffer[..line_end].trim().to_string();
            buffer.drain(..=line_end);

            if line == "data: [DONE]" {
                break 'outer;
            }
            let Some(data) = line.strip_prefix("data: ") else {
                continue;
            };
            let Ok(json) = serde_json::from_str::<Value>(data) else {
                continue;
            };
            if let Some(err) = json.get("error") {
                let msg = err["message"].as_str().unwrap_or("chat stream error");
                return Err(CliError::new(
                    ErrorKind::Api,
                    err["code"].as_str().unwrap_or("chat_error"),
                    msg,
                )
                .into());
            }
            let choice = &json["choices"][0];
            if let Some(content) = choice["delta"]["content"].as_str() {
                c.answer.push_str(content);
                if args.events {
                    event(&json!({ "event": "delta", "content": content }));
                } else if stream_text {
                    print!("{content}");
                    std::io::stdout().flush()?;
                }
            }
            if let Some(calls) = choice["delta"]["tool_calls"].as_array() {
                for call in calls {
                    let idx = call["index"].as_u64().unwrap_or(0);
                    if let Some(name) = call["function"]["name"].as_str() {
                        c.tool_names.insert(idx, name.to_string());
                    }
                    if let Some(a) = call["function"]["arguments"].as_str() {
                        c.tool_args.entry(idx).or_default().push_str(a);
                    }
                }
            }
            if !choice["finish_reason"].is_null() {
                let found = c.flush_tools();
                if args.events && !found.is_empty() {
                    event(&json!({ "event": "sources", "documents": found }));
                }
            }
        }
    }
    let found = c.flush_tools();
    if args.events && !found.is_empty() {
        event(&json!({ "event": "sources", "documents": found }));
    }

    let result = json!({
        "answer": c.answer,
        "sources": c.sources,
        "model": model,
        "workspace": args.workspace,
    });
    if args.events {
        event(&json!({ "event": "done", "result": result }));
    } else if stream_text {
        println!();
        if !c.sources.is_empty() {
            status!(
                "({} source document(s); use --json to see them)",
                c.sources.len()
            );
        }
    } else {
        crate::output::print_json(&result);
    }
    Ok(())
}
