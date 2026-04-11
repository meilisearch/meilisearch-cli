use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Subcommand;

use super::{Cli, build_client, print_json};

#[derive(Subcommand)]
pub enum DocumentCommand {
    /// Add or replace documents
    Add {
        /// Index UID
        #[arg(value_name = "INDEX_UID")]
        uid: String,
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long)]
        primary_key: Option<String>,
    },
    /// Add or update documents (partial update)
    Update {
        /// Index UID
        #[arg(value_name = "INDEX_UID")]
        uid: String,
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long)]
        primary_key: Option<String>,
    },
    /// Get a single document
    Get {
        /// Index UID
        #[arg(value_name = "INDEX_UID")]
        uid: String,
        /// Document ID
        #[arg(value_name = "DOCUMENT_ID")]
        id: String,
    },
    /// List documents
    List {
        /// Index UID
        #[arg(value_name = "INDEX_UID")]
        uid: String,
        #[arg(long)]
        offset: Option<u64>,
        #[arg(long)]
        limit: Option<u64>,
        #[arg(long)]
        fields: Option<String>,
    },
    /// Fetch documents with POST (supports filter)
    Fetch {
        /// Index UID
        #[arg(value_name = "INDEX_UID")]
        uid: String,
        #[arg(long)]
        filter: Option<String>,
        #[arg(long)]
        offset: Option<u64>,
        #[arg(long)]
        limit: Option<u64>,
        #[arg(long, value_delimiter = ',')]
        fields: Option<Vec<String>>,
    },
    /// Delete a single document
    Delete {
        /// Index UID
        #[arg(value_name = "INDEX_UID")]
        uid: String,
        /// Document ID
        #[arg(value_name = "DOCUMENT_ID")]
        id: String,
    },
    /// Delete all documents
    DeleteAll {
        /// Index UID
        #[arg(value_name = "INDEX_UID")]
        uid: String,
    },
    /// Delete documents matching a filter
    DeleteByFilter {
        /// Index UID
        #[arg(value_name = "INDEX_UID")]
        uid: String,
        /// Filter expression (e.g. "genre = horror")
        filter: String,
    },
    /// Delete documents by batch of IDs
    DeleteBatch {
        /// Index UID
        #[arg(value_name = "INDEX_UID")]
        uid: String,
        /// Comma-separated document IDs
        #[arg(value_delimiter = ',')]
        ids: Vec<String>,
    },
    /// Edit documents by function
    Edit {
        /// Index UID
        #[arg(value_name = "INDEX_UID")]
        uid: String,
        /// JavaScript function body
        function: String,
        /// Optional filter expression
        #[arg(long)]
        filter: Option<String>,
    },
}

pub async fn run(cli: &Cli, cmd: &DocumentCommand) -> Result<()> {
    let client = build_client(cli)?;
    match cmd {
        DocumentCommand::Add {
            uid,
            file,
            primary_key,
        } => {
            let data = if let Some(path) = file {
                std::fs::read(path)
                    .with_context(|| format!("Failed to read file: {}", path.display()))?
            } else {
                use std::io::Read;
                let mut buf = Vec::new();
                std::io::stdin()
                    .read_to_end(&mut buf)
                    .context("Failed to read from stdin")?;
                buf
            };

            let content_type = if let Some(path) = file {
                match path.extension().and_then(|e| e.to_str()) {
                    Some("csv") => "text/csv",
                    Some("ndjson") | Some("jsonl") => "application/x-ndjson",
                    _ => "application/json",
                }
            } else {
                "application/json"
            };

            let result = client
                .add_documents_raw(uid, data, content_type, primary_key.as_deref())
                .await?;
            print_json(&result, cli.raw);
        }
        DocumentCommand::Get { uid, id } => {
            let result = client.get_document(uid, id).await?;
            print_json(&result, cli.raw);
        }
        DocumentCommand::List {
            uid,
            offset,
            limit,
            fields,
        } => {
            let result = client
                .get_documents(uid, *offset, *limit, fields.as_deref())
                .await?;
            print_json(&result, cli.raw);
        }
        DocumentCommand::Update {
            uid,
            file,
            primary_key,
        } => {
            let data = if let Some(path) = file {
                std::fs::read(path)
                    .with_context(|| format!("Failed to read file: {}", path.display()))?
            } else {
                use std::io::Read;
                let mut buf = Vec::new();
                std::io::stdin()
                    .read_to_end(&mut buf)
                    .context("Failed to read from stdin")?;
                buf
            };

            let content_type = if let Some(path) = file {
                match path.extension().and_then(|e| e.to_str()) {
                    Some("csv") => "text/csv",
                    Some("ndjson") | Some("jsonl") => "application/x-ndjson",
                    _ => "application/json",
                }
            } else {
                "application/json"
            };

            let result = client
                .add_or_update_documents_raw(uid, data, content_type, primary_key.as_deref())
                .await?;
            print_json(&result, cli.raw);
        }
        DocumentCommand::Fetch {
            uid,
            filter,
            offset,
            limit,
            fields,
        } => {
            let mut body = serde_json::json!({});
            if let Some(f) = filter {
                body["filter"] = serde_json::json!(f);
            }
            if let Some(o) = offset {
                body["offset"] = serde_json::json!(o);
            }
            if let Some(l) = limit {
                body["limit"] = serde_json::json!(l);
            }
            if let Some(f) = fields {
                body["fields"] = serde_json::json!(f);
            }
            let result = client.fetch_documents(uid, &body).await?;
            print_json(&result, cli.raw);
        }
        DocumentCommand::Delete { uid, id } => {
            let result = client.delete_document(uid, id).await?;
            print_json(&result, cli.raw);
        }
        DocumentCommand::DeleteAll { uid } => {
            let result = client.delete_all_documents(uid).await?;
            print_json(&result, cli.raw);
        }
        DocumentCommand::DeleteByFilter { uid, filter } => {
            let result = client.delete_documents_by_filter(uid, filter).await?;
            print_json(&result, cli.raw);
        }
        DocumentCommand::DeleteBatch { uid, ids } => {
            let id_values: Vec<serde_json::Value> =
                ids.iter().map(|id| serde_json::json!(id)).collect();
            let result = client.delete_documents_batch(uid, &id_values).await?;
            print_json(&result, cli.raw);
        }
        DocumentCommand::Edit {
            uid,
            function,
            filter,
        } => {
            let result = client
                .edit_documents(uid, function, filter.as_deref())
                .await?;
            print_json(&result, cli.raw);
        }
    }
    Ok(())
}
