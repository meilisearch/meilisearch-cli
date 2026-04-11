use anyhow::Result;
use clap::Subcommand;

use super::{Cli, build_client, print_json};

#[derive(Subcommand)]
pub enum BatchCommand {
    /// List batches
    List {
        #[arg(long)]
        limit: Option<u64>,
        #[arg(long)]
        from: Option<u64>,
        #[arg(long)]
        uids: Option<String>,
        #[arg(long)]
        index_uids: Option<String>,
        #[arg(long)]
        statuses: Option<String>,
        #[arg(long)]
        types: Option<String>,
    },
    /// Get a batch by UID
    Get { uid: u64 },
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
            print_json(&result, cli.raw);
        }
        BatchCommand::Get { uid } => {
            let result = client.get_batch(*uid).await?;
            print_json(&result, cli.raw);
        }
    }
    Ok(())
}
