use anyhow::Result;
use clap::Subcommand;

use super::{Cli, build_client, print_json};

#[derive(Subcommand)]
pub enum LogCommand {
    /// Update the target of stderr logs
    Stderr {
        /// Log target (e.g. "info", "meilisearch=debug")
        target: String,
    },
    /// Start streaming logs
    Stream {
        /// Log target (e.g. "info", "meilisearch=debug")
        target: String,
        /// Output mode (human or json)
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
                println!("Log target updated.");
            } else {
                print_json(&result, cli.raw);
            }
        }
        LogCommand::Stream { target, mode } => {
            let result = client.stream_logs(target, mode.as_deref()).await?;
            print_json(&result, cli.raw);
        }
        LogCommand::Stop => {
            let result = client.stop_log_stream().await?;
            if result.is_null() {
                println!("Log stream stopped.");
            } else {
                print_json(&result, cli.raw);
            }
        }
    }
    Ok(())
}
