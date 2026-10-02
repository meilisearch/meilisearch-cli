use anyhow::Result;
use clap::Subcommand;

use serde_json::json;

use super::{Cli, build_client, print_json, print_skipped};
use crate::error::is_not_found;

#[derive(Subcommand)]
pub enum KeyCommand {
    /// List all API keys
    List {
        /// Number of keys to skip
        #[arg(long)]
        offset: Option<u64>,
        /// Maximum number of keys to return
        #[arg(long)]
        limit: Option<u64>,
    },
    /// Get an API key
    Get {
        /// API key or UID
        #[arg(value_name = "API_KEY")]
        key: String,
    },
    /// Create an API key
    Create {
        /// Comma-separated actions (e.g. search,documents.add or *)
        #[arg(long, value_delimiter = ',')]
        actions: Vec<String>,
        /// Comma-separated index UIDs or patterns (* for all)
        #[arg(long, value_delimiter = ',')]
        indexes: Vec<String>,
        /// Key description
        #[arg(long)]
        description: Option<String>,
        /// Expiration date (RFC 3339), omit for no expiry
        #[arg(long)]
        expires_at: Option<String>,
    },
    /// Update an API key
    Update {
        /// API key or UID
        #[arg(value_name = "API_KEY")]
        key: String,
        /// Key description
        #[arg(long)]
        description: Option<String>,
        /// Key name
        #[arg(long)]
        name: Option<String>,
    },
    /// Delete an API key
    Delete {
        /// API key or UID
        #[arg(value_name = "API_KEY")]
        key: String,
        /// Succeed without changes if the key does not exist
        #[arg(long)]
        if_exists: bool,
        /// Show the key that would be deleted without deleting it
        #[arg(long)]
        dry_run: bool,
    },
}

pub async fn run(cli: &Cli, cmd: &KeyCommand) -> Result<()> {
    let client = build_client(cli)?;
    match cmd {
        KeyCommand::List { offset, limit } => {
            let result = client.list_keys(*offset, *limit).await?;
            print_json(&result);
        }
        KeyCommand::Get { key } => {
            let result = client.get_key(key).await?;
            print_json(&result);
        }
        KeyCommand::Create {
            actions,
            indexes,
            description,
            expires_at,
        } => {
            let result = client
                .create_key(
                    description.as_deref(),
                    actions,
                    indexes,
                    expires_at.as_deref(),
                )
                .await?;
            print_json(&result);
        }
        KeyCommand::Update {
            key,
            description,
            name,
        } => {
            let result = client
                .update_key(key, description.as_deref(), name.as_deref())
                .await?;
            print_json(&result);
        }
        KeyCommand::Delete {
            key,
            if_exists,
            dry_run,
        } => {
            if *if_exists || *dry_run {
                let existing = match client.get_key(key).await {
                    Ok(k) => k,
                    Err(e) if *if_exists && is_not_found(&e) => {
                        print_skipped("not_found", json!({ "key": key }));
                        return Ok(());
                    }
                    Err(e) => return Err(e),
                };
                if *dry_run {
                    print_json(&json!({ "dryRun": true, "action": "key.delete", "key": existing }));
                    return Ok(());
                }
            }
            client.delete_key(key).await?;
            crate::output::emit(&json!({ "deleted": true, "key": key }), || {
                "Key deleted successfully.".to_string()
            });
        }
    }
    Ok(())
}
