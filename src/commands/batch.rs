use anyhow::Result;
use clap::Subcommand;

use super::{Cli, build_client, print_json};

#[derive(Subcommand)]
pub enum BatchCommand {
    /// List batches
    List {
        /// Maximum number of batches to return
        #[arg(long)]
        limit: Option<u64>,
        /// Start listing from this batch UID (pagination cursor)
        #[arg(long)]
        from: Option<u64>,
        /// Comma-separated batch UIDs
        #[arg(long)]
        uids: Option<String>,
        /// Comma-separated index UIDs
        #[arg(long)]
        index_uids: Option<String>,
        /// Comma-separated statuses
        #[arg(long)]
        statuses: Option<String>,
        /// Comma-separated task types
        #[arg(long)]
        types: Option<String>,
    },
    /// Get a batch by UID
    Get {
        /// Batch UID
        #[arg(value_name = "BATCH_UID")]
        uid: u64,
    },
}

pub async fn run(cli: &Cli, cmd: &BatchCommand) -> Result<()> {
    let client = build_client(cli)?;
    match cmd {
        BatchCommand::List {
            limit,
            from,
            uids,
            index_uids,
            statuses,
            types,
        } => {
            let result = client
                .list_batches(
                    *limit,
                    *from,
                    uids.as_deref(),
                    index_uids.as_deref(),
                    statuses.as_deref(),
                    types.as_deref(),
                )
                .await?;
            print_json(&result);
        }
        BatchCommand::Get { uid } => {
            let result = client.get_batch(*uid).await?;
            print_json(&result);
        }
    }
    Ok(())
}
