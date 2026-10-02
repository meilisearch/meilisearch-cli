use std::path::PathBuf;

use anyhow::Result;
use clap::Args;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use serde_json::{Value, json};

use super::document::DocFormat;
use super::{Cli, build_client, read_input};
use crate::client::ensure_succeeded;
use crate::error::CliError;
use crate::output::{event, is_json};
use crate::status;

#[derive(Args)]
pub struct ImportArgs {
    /// Index UID
    #[arg(value_name = "INDEX_UID")]
    pub uid: String,

    /// File to import (reads stdin if omitted)
    #[arg(long)]
    pub file: Option<PathBuf>,

    /// Payload format (default: from file extension, else json)
    #[arg(long, value_enum)]
    pub format: Option<DocFormat>,

    /// Primary key field
    #[arg(long)]
    pub primary_key: Option<String>,

    /// Batch size in bytes (default: 20 MiB)
    #[arg(long, default_value = "20971520")]
    pub batch_size: usize,

    /// Return as soon as all batches are enqueued, without waiting for indexing
    #[arg(long)]
    pub no_wait: bool,

    /// Emit NDJSON progress events on stdout instead of a single summary
    #[arg(long)]
    pub events: bool,
}

/// Split the payload into batches of at most `batch_size` bytes.
/// NDJSON splits on lines, JSON arrays on elements; CSV is sent whole.
fn split_batches(data: Vec<u8>, format: DocFormat, batch_size: usize) -> Result<Vec<Vec<u8>>> {
    match format {
        DocFormat::Csv => Ok(vec![data]),
        DocFormat::Ndjson => {
            let text = String::from_utf8_lossy(&data);
            let mut batches = vec![];
            let mut batch = String::new();
            for line in text.lines().filter(|l| !l.trim().is_empty()) {
                if !batch.is_empty() && batch.len() + line.len() + 1 > batch_size {
                    batches.push(std::mem::take(&mut batch).into_bytes());
                }
                batch.push_str(line);
                batch.push('\n');
            }
            if !batch.is_empty() {
                batches.push(batch.into_bytes());
            }
            Ok(batches)
        }
        DocFormat::Json => {
            let value: Value = serde_json::from_slice(&data).map_err(|e| {
                CliError::usage(
                    "invalid_json",
                    format!("Failed to parse JSON documents: {e}"),
                )
            })?;
            let Value::Array(docs) = value else {
                // A single object: send as-is.
                return Ok(vec![data]);
            };
            let mut batches = vec![];
            let mut batch: Vec<Value> = vec![];
            let mut size = 2usize;
            for doc in docs {
                let doc_size = serde_json::to_vec(&doc)?.len() + 1;
                if !batch.is_empty() && size + doc_size > batch_size {
                    batches.push(serde_json::to_vec(&batch)?);
                    batch.clear();
                    size = 2;
                }
                size += doc_size;
                batch.push(doc);
            }
            if !batch.is_empty() {
                batches.push(serde_json::to_vec(&batch)?);
            }
            Ok(batches)
        }
    }
}

pub async fn run(cli: &Cli, args: &ImportArgs) -> Result<()> {
    // Import manages waiting itself so batches are enqueued back to back.
    let client = build_client(cli)?.without_wait();
    let format = DocFormat::resolve(args.format, args.file.as_deref());
    let data = read_input(args.file.as_deref())?;
    let total_bytes = data.len() as u64;
    let batches = split_batches(data, format, args.batch_size)?;

    let pb = ProgressBar::new(total_bytes);
    if is_json() || args.events || crate::output::ctx().quiet {
        pb.set_draw_target(ProgressDrawTarget::hidden());
    }
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .progress_chars("█▉▊▋▌▍▎▏  "),
    );

    let mut task_uids = vec![];
    let batch_count = batches.len();
    for (i, batch) in batches.into_iter().enumerate() {
        let bytes = batch.len();
        let result = client
            .add_documents_raw(
                &args.uid,
                batch,
                format.content_type(),
                args.primary_key.as_deref(),
            )
            .await?;
        let task_uid = result["taskUid"].as_u64();
        if let Some(uid) = task_uid {
            task_uids.push(uid);
        }
        if args.events {
            event(&json!({
                "event": "batch_enqueued",
                "batch": i + 1,
                "batches": batch_count,
                "bytes": bytes,
                "taskUid": task_uid,
            }));
        }
        pb.inc(bytes as u64);
    }
    pb.finish_and_clear();

    let mut summary = json!({
        "indexUid": args.uid,
        "batches": batch_count,
        "bytes": total_bytes,
        "taskUids": task_uids,
    });

    if args.no_wait {
        summary["status"] = json!("enqueued");
    } else {
        status!("Waiting for {} task(s)...", task_uids.len());
        let mut indexed = 0u64;
        for task_uid in &task_uids {
            let task = client.wait_for_task(*task_uid, cli.wait_timeout).await?;
            if args.events {
                event(&json!({
                    "event": "task_finished",
                    "taskUid": task_uid,
                    "status": task["status"],
                    "indexedDocuments": task["details"]["indexedDocuments"],
                }));
            }
            let task = ensure_succeeded(task)?;
            indexed += task["details"]["indexedDocuments"].as_u64().unwrap_or(0);
        }
        summary["status"] = json!("succeeded");
        summary["indexedDocuments"] = json!(indexed);
    }

    if args.events {
        event(&json!({ "event": "done", "summary": summary }));
    } else {
        crate::output::emit(&summary, || match summary["status"].as_str() {
            Some("enqueued") => format!(
                "Enqueued {} batch(es) into '{}' (tasks: {:?})",
                batch_count, args.uid, task_uids
            ),
            _ => format!(
                "✓ Imported {} document(s) in {} batch(es) into '{}'",
                summary["indexedDocuments"], batch_count, args.uid
            ),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ndjson_batches_respect_size() {
        let data = b"{\"id\":1}\n{\"id\":2}\n\n{\"id\":3}\n".to_vec();
        let batches = split_batches(data, DocFormat::Ndjson, 18).unwrap();
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0], b"{\"id\":1}\n{\"id\":2}\n");
    }

    #[test]
    fn json_arrays_are_split_into_valid_arrays() {
        let data = br#"[{"id":1},{"id":2},{"id":3}]"#.to_vec();
        let batches = split_batches(data, DocFormat::Json, 20).unwrap();
        assert!(batches.len() > 1);
        let total: usize = batches
            .iter()
            .map(|b| serde_json::from_slice::<Vec<Value>>(b).unwrap().len())
            .sum();
        assert_eq!(total, 3);
    }

    #[test]
    fn invalid_json_is_a_usage_error() {
        let err = split_batches(b"[{".to_vec(), DocFormat::Json, 100).unwrap_err();
        assert_eq!(crate::error::report(&err).exit_code(), 2);
    }
}
