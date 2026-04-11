use anyhow::Result;
use clap::Subcommand;

use super::{Cli, build_client, print_json};

#[derive(Subcommand)]
pub enum DumpCommand {
    /// Create a dump
    Create,
    /// Create a snapshot
    Snapshot,
}

pub async fn run(cli: &Cli, cmd: &DumpCommand) -> Result<()> {
    let client = build_client(cli)?;
    match cmd {
        DumpCommand::Create => {
            let result = client.create_dump().await?;
            print_json(&result, cli.raw);
        }
        DumpCommand::Snapshot => {
            let result = client.create_snapshot().await?;
            print_json(&result, cli.raw);
        }
    }
    Ok(())
}
