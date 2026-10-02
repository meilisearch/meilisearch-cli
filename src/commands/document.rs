use std::path::PathBuf;

use anyhow::Result;
use clap::{Subcommand, ValueEnum};
use serde_json::json;

use super::{Cli, build_client, print_json, read_input};

/// Document payload format.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum DocFormat {
    Json,
    Ndjson,
    Csv,
}

impl DocFormat {
    pub fn content_type(self) -> &'static str {
        match self {
            DocFormat::Json => "application/json",
            DocFormat::Ndjson => "application/x-ndjson",
            DocFormat::Csv => "text/csv",
        }
    }

    /// Explicit format, else guessed from the file extension, else JSON.
    pub fn resolve(explicit: Option<DocFormat>, file: Option<&std::path::Path>) -> DocFormat {
        explicit.unwrap_or_else(|| {
            match file.and_then(|p| p.extension()).and_then(|e| e.to_str()) {
                Some("csv") => DocFormat::Csv,
                Some("ndjson") | Some("jsonl") => DocFormat::Ndjson,
                _ => DocFormat::Json,
            }
        })
    }
}

#[derive(Subcommand)]
pub enum DocumentCommand {
    /// Add or replace documents from a file or stdin
    Add {
        /// Index UID
        #[arg(value_name = "INDEX_UID")]
        uid: String,
        /// Documents file (reads stdin if omitted)
        #[arg(long)]
        file: Option<PathBuf>,
        /// Payload format (default: from file extension, else json)
        #[arg(long, value_enum)]
        format: Option<DocFormat>,
        /// Primary key attribute (only used if the index has none yet)
        #[arg(long)]
        primary_key: Option<String>,
    },
    /// Add or update documents (partial update) from a file or stdin
    Update {
        /// Index UID
        #[arg(value_name = "INDEX_UID")]
        uid: String,
        /// Documents file (reads stdin if omitted)
        #[arg(long)]
        file: Option<PathBuf>,
        /// Payload format (default: from file extension, else json)
        #[arg(long, value_enum)]
        format: Option<DocFormat>,
        /// Primary key attribute (only used if the index has none yet)
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
        /// Number of documents to skip
        #[arg(long)]
        offset: Option<u64>,
        /// Maximum number of documents to return
        #[arg(long)]
        limit: Option<u64>,
        /// Comma-separated attributes to return
        #[arg(long)]
        fields: Option<String>,
    },
    /// Fetch documents with POST (supports filter)
    Fetch {
        /// Index UID
        #[arg(value_name = "INDEX_UID")]
        uid: String,
        /// Filter expression (e.g. "genre = horror")
        #[arg(long)]
        filter: Option<String>,
        /// Number of documents to skip
        #[arg(long)]
        offset: Option<u64>,
        /// Maximum number of documents to return
        #[arg(long)]
        limit: Option<u64>,
        /// Comma-separated attributes to return
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
        /// Show how many documents would be deleted without deleting them
        #[arg(long)]
        dry_run: bool,
    },
    /// Delete documents matching a filter
    DeleteByFilter {
        /// Index UID
        #[arg(value_name = "INDEX_UID")]
        uid: String,
        /// Filter expression (e.g. "genre = horror")
        filter: String,
        /// Show how many documents match without deleting them
        #[arg(long)]
        dry_run: bool,
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
            format,
            primary_key,
        } => {
            let format = DocFormat::resolve(*format, file.as_deref());
            let data = read_input(file.as_deref())?;
            let result = client
                .add_documents_raw(uid, data, format.content_type(), primary_key.as_deref())
                .await?;
            print_json(&result);
        }
        DocumentCommand::Get { uid, id } => {
            let result = client.get_document(uid, id).await?;
            print_json(&result);
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
            print_json(&result);
        }
        DocumentCommand::Update {
            uid,
            file,
            format,
            primary_key,
        } => {
            let format = DocFormat::resolve(*format, file.as_deref());
            let data = read_input(file.as_deref())?;
            let result = client
                .add_or_update_documents_raw(
                    uid,
                    data,
                    format.content_type(),
                    primary_key.as_deref(),
                )
                .await?;
            print_json(&result);
        }
        DocumentCommand::Fetch {
            uid,
            filter,
            offset,
            limit,
            fields,
        } => {
            let mut body = json!({});
            if let Some(f) = filter {
                body["filter"] = json!(f);
            }
            if let Some(o) = offset {
                body["offset"] = json!(o);
            }
            if let Some(l) = limit {
                body["limit"] = json!(l);
            }
            if let Some(f) = fields {
                body["fields"] = json!(f);
            }
            let result = client.fetch_documents(uid, &body).await?;
            print_json(&result);
        }
        DocumentCommand::Delete { uid, id } => {
            let result = client.delete_document(uid, id).await?;
            print_json(&result);
        }
        DocumentCommand::DeleteAll { uid, dry_run } => {
            if *dry_run {
                let stats = client.index_stats(uid).await?;
                print_json(&json!({
                    "dryRun": true,
                    "action": "documents.deleteAll",
                    "indexUid": uid,
                    "matchedDocuments": stats["numberOfDocuments"],
                }));
                return Ok(());
            }
            let result = client.delete_all_documents(uid).await?;
            print_json(&result);
        }
        DocumentCommand::DeleteByFilter {
            uid,
            filter,
            dry_run,
        } => {
            if *dry_run {
                let matched = client
                    .fetch_documents(uid, &json!({ "filter": filter, "limit": 0 }))
                    .await?;
                print_json(&json!({
                    "dryRun": true,
                    "action": "documents.deleteByFilter",
                    "indexUid": uid,
                    "filter": filter,
                    "matchedDocuments": matched["total"],
                }));
                return Ok(());
            }
            let result = client.delete_documents_by_filter(uid, filter).await?;
            print_json(&result);
        }
        DocumentCommand::DeleteBatch { uid, ids } => {
            let id_values: Vec<serde_json::Value> = ids.iter().map(|id| json!(id)).collect();
            let result = client.delete_documents_batch(uid, &id_values).await?;
            print_json(&result);
        }
        DocumentCommand::Edit {
            uid,
            function,
            filter,
        } => {
            let result = client
                .edit_documents(uid, function, filter.as_deref())
                .await?;
            print_json(&result);
        }
    }
    Ok(())
}
