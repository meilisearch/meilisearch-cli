use anyhow::Result;
use clap::Subcommand;
use futures_util::StreamExt;
use serde_json::json;

use super::{Cli, build_client, print_json};

#[derive(Subcommand)]
pub enum LogCommand {
    /// Update the target of stderr logs
    Stderr {
        /// Log target (e.g. "info", "meilisearch=debug")
        target: String,
    },
    /// Stream logs to stdout until interrupted (NDJSON with --mode json)
    Stream {
        /// Log target (e.g. "info", "meilisearch=debug")
        target: String,
        /// Output mode: human, json or profile (default: json when output is JSON)
        #[arg(long)]
        mode: Option<String>,
    },
    /// Stop streaming logs
    Stop,
}

pub async fn run(cli: &Cli, cmd: &LogCommand) -> Result<()> {
    let client = build_client(cli)?;
    match cmd {
        LogCommand::Stderr { target } => {
            let result = client.update_log_stderr(target).await?;
            if result.is_null() {
                crate::output::emit(&json!({ "updated": true, "target": target }), || {
                    "Log target updated.".to_string()
                });
            } else {
                print_json(&result);
            }
        }
        LogCommand::Stream { target, mode } => {
            let mode = mode
                .clone()
                .or_else(|| crate::output::is_json().then(|| "json".to_string()));
            let resp = client.stream_logs(target, mode.as_deref()).await?;
            crate::status!("Streaming logs (Ctrl+C to stop)...");
            let mut stream = resp.bytes_stream();
            let mut out = std::io::stdout();
            while let Some(chunk) = stream.next().await {
                use std::io::Write;
                out.write_all(&chunk?)?;
                out.flush()?;
            }
        }
        LogCommand::Stop => {
            let result = client.stop_log_stream().await?;
            if result.is_null() {
                crate::output::emit(&json!({ "stopped": true }), || {
                    "Log stream stopped.".to_string()
                });
            } else {
                print_json(&result);
            }
        }
    }
    Ok(())
}
