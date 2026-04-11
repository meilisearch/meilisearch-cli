use std::path::PathBuf;

use anyhow::Result;
use clap::Subcommand;

use super::{Cli, build_client, print_json, read_json_input};

#[derive(Subcommand)]
pub enum ExperimentalCommand {
    /// Get all experimental features
    Get,
    /// Configure experimental features (reads JSON from file or stdin)
    Update {
        #[arg(long)]
        file: Option<PathBuf>,
    },
}

pub async fn run(cli: &Cli, cmd: &ExperimentalCommand) -> Result<()> {
    let client = build_client(cli)?;
    match cmd {
        ExperimentalCommand::Get => {
            let result = client.get_experimental_features().await?;
            print_json(&result, cli.raw);
        }
        ExperimentalCommand::Update { file } => {
            let features = read_json_input(file.as_deref())?;
            let result = client.update_experimental_features(&features).await?;
            print_json(&result, cli.raw);
        }
    }
    Ok(())
}
