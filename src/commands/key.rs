use anyhow::Result;
use clap::Subcommand;

use super::{Cli, build_client, print_json};

#[derive(Subcommand)]
pub enum KeyCommand {
    /// List all API keys
    List,
    /// Get an API key
    Get {
        /// API key or UID
        #[arg(value_name = "API_KEY")]
        key: String,
    },
    /// Create an API key
    Create {
        #[arg(long, value_delimiter = ',')]
        actions: Vec<String>,
        #[arg(long, value_delimiter = ',')]
        indexes: Vec<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        expires_at: Option<String>,
    },
    /// Update an API key
    Update {
        /// API key or UID
        #[arg(value_name = "API_KEY")]
        key: String,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        name: Option<String>,
    },
    /// Delete an API key
    Delete {
        /// API key or UID
        #[arg(value_name = "API_KEY")]
        key: String,
    },
}

pub async fn run(cli: &Cli, cmd: &KeyCommand) -> Result<()> {
    let client = build_client(cli)?;
    match cmd {
        KeyCommand::List => {
            let result = client.list_keys().await?;
            print_json(&result, cli.raw);
        }
        KeyCommand::Get { key } => {
            let result = client.get_key(key).await?;
            print_json(&result, cli.raw);
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
            print_json(&result, cli.raw);
        }
        KeyCommand::Update {
            key,
            description,
            name,
        } => {
            let result = client
                .update_key(key, description.as_deref(), name.as_deref())
                .await?;
            print_json(&result, cli.raw);
        }
        KeyCommand::Delete { key } => {
            let result = client.delete_key(key).await?;
            if result.is_null() {
                println!("Key deleted successfully.");
            } else {
                print_json(&result, cli.raw);
            }
        }
    }
    Ok(())
}
