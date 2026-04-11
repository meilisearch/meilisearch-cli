use std::path::PathBuf;

use anyhow::Result;
use clap::Subcommand;

use super::{Cli, build_client, print_json, read_json_input};

#[derive(Subcommand)]
pub enum NetworkCommand {
    /// Get current network configuration
    Get,
    /// Update network configuration (reads JSON from file or stdin)
    Update {
        #[arg(long)]
        file: Option<PathBuf>,
    },
}

pub async fn run(cli: &Cli, cmd: &NetworkCommand) -> Result<()> {
    let client = build_client(cli)?;
    match cmd {
        NetworkCommand::Get => {
            let result = client.get_network().await?;
            print_json(&result, cli.raw);
        }
        NetworkCommand::Update { file } => {
            let body = read_json_input(file.as_deref())?;
            let result = client.update_network(&body).await?;
            print_json(&result, cli.raw);
        }
    }
    Ok(())
}
