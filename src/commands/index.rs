use anyhow::Result;
use clap::Subcommand;

use super::{Cli, build_client, print_json};

#[derive(Subcommand)]
pub enum IndexCommand {
    /// List all indexes
    List {
        #[arg(long)]
        offset: Option<u64>,
        #[arg(long)]
        limit: Option<u64>,
    },
    /// Create an index
    Create {
        uid: String,
        #[arg(long)]
        primary_key: Option<String>,
    },
    /// Get index info
    Get { uid: String },
    /// Delete an index
    Delete { uid: String },
    /// Show index stats
    Stats { uid: String },
    /// Update an index (change primary key)
    Update {
        uid: String,
        #[arg(long)]
        primary_key: Option<String>,
    },
    /// Swap two indexes
    Swap { index_a: String, index_b: String },
}

pub async fn run(cli: &Cli, cmd: &IndexCommand) -> Result<()> {
    let client = build_client(cli)?;
    match cmd {
        IndexCommand::List { offset, limit } => {
            let result = client.list_indexes(*offset, *limit).await?;
            print_json(&result, cli.raw);
        }
        IndexCommand::Create { uid, primary_key } => {
            let result = client.create_index(uid, primary_key.as_deref()).await?;
            print_json(&result, cli.raw);
        }
        IndexCommand::Get { uid } => {
            let result = client.get_index(uid).await?;
            print_json(&result, cli.raw);
        }
        IndexCommand::Delete { uid } => {
            let result = client.delete_index(uid).await?;
            print_json(&result, cli.raw);
        }
        IndexCommand::Stats { uid } => {
            let result = client.index_stats(uid).await?;
            print_json(&result, cli.raw);
        }
        IndexCommand::Update { uid, primary_key } => {
            let result = client.update_index(uid, primary_key.as_deref()).await?;
            print_json(&result, cli.raw);
        }
        IndexCommand::Swap { index_a, index_b } => {
            let result = client.swap_indexes(&[(index_a, index_b)]).await?;
            print_json(&result, cli.raw);
        }
    }
    Ok(())
}
