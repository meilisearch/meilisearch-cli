use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use super::{Cli, build_client, print_json, print_skipped};
use crate::error::is_not_found;

#[derive(Subcommand)]
pub enum IndexCommand {
    /// List all indexes
    List {
        /// Number of indexes to skip
        #[arg(long)]
        offset: Option<u64>,
        /// Maximum number of indexes to return
        #[arg(long)]
        limit: Option<u64>,
    },
    /// Create an index
    Create {
        /// Index UID
        #[arg(value_name = "INDEX_UID")]
        uid: String,
        /// Primary key attribute
        #[arg(long)]
        primary_key: Option<String>,
        /// Succeed without changes if the index already exists
        #[arg(long)]
        if_not_exists: bool,
    },
    /// Get index info
    Get {
        /// Index UID
        #[arg(value_name = "INDEX_UID")]
        uid: String,
    },
    /// Delete an index
    Delete {
        /// Index UID
        #[arg(value_name = "INDEX_UID")]
        uid: String,
        /// Succeed without changes if the index does not exist
        #[arg(long)]
        if_exists: bool,
        /// Show what would be deleted without deleting it
        #[arg(long)]
        dry_run: bool,
    },
    /// Show index stats
    Stats {
        /// Index UID
        #[arg(value_name = "INDEX_UID")]
        uid: String,
    },
    /// Update an index (change primary key)
    Update {
        /// Index UID
        #[arg(value_name = "INDEX_UID")]
        uid: String,
        /// Primary key attribute
        #[arg(long)]
        primary_key: Option<String>,
    },
    /// Swap two indexes
    Swap {
        /// First index UID
        index_a: String,
        /// Second index UID
        index_b: String,
    },
}

pub async fn run(cli: &Cli, cmd: &IndexCommand) -> Result<()> {
    let client = build_client(cli)?;
    match cmd {
        IndexCommand::List { offset, limit } => {
            let result = client.list_indexes(*offset, *limit).await?;
            print_json(&result);
        }
        IndexCommand::Create {
            uid,
            primary_key,
            if_not_exists,
        } => {
            if *if_not_exists {
                match client.get_index(uid).await {
                    Ok(index) => {
                        print_skipped("already_exists", json!({ "index": index }));
                        return Ok(());
                    }
                    Err(e) if is_not_found(&e) => {}
                    Err(e) => return Err(e),
                }
            }
            let result = client.create_index(uid, primary_key.as_deref()).await?;
            print_json(&result);
        }
        IndexCommand::Get { uid } => {
            let result = client.get_index(uid).await?;
            print_json(&result);
        }
        IndexCommand::Delete {
            uid,
            if_exists,
            dry_run,
        } => {
            if *if_exists || *dry_run {
                let index = match client.get_index(uid).await {
                    Ok(index) => index,
                    Err(e) if *if_exists && is_not_found(&e) => {
                        print_skipped("not_found", json!({ "indexUid": uid }));
                        return Ok(());
                    }
                    Err(e) => return Err(e),
                };
                if *dry_run {
                    let stats = client.index_stats(uid).await?;
                    print_json(&json!({
                        "dryRun": true,
                        "action": "index.delete",
                        "index": index,
                        "numberOfDocuments": stats["numberOfDocuments"],
                    }));
                    return Ok(());
                }
            }
            let result = client.delete_index(uid).await?;
            print_json(&result);
        }
        IndexCommand::Stats { uid } => {
            let result = client.index_stats(uid).await?;
            print_json(&result);
        }
        IndexCommand::Update { uid, primary_key } => {
            let result = client.update_index(uid, primary_key.as_deref()).await?;
            print_json(&result);
        }
        IndexCommand::Swap { index_a, index_b } => {
            let result = client.swap_indexes(&[(index_a, index_b)]).await?;
            print_json(&result);
        }
    }
    Ok(())
}
