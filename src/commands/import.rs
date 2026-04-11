use std::io::Read;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::Args;
use indicatif::{ProgressBar, ProgressStyle};

use super::{Cli, build_client, print_json};

#[derive(Args)]
pub struct ImportArgs {
    /// Index UID
    pub uid: String,

    /// File(s) to import
    #[arg(long)]
    pub file: Option<PathBuf>,

    /// Primary key field
    #[arg(long)]
    pub primary_key: Option<String>,

    /// Batch size in bytes (default: 20 MiB)
    #[arg(long, default_value = "20971520")]
    pub batch_size: usize,
}

pub async fn run(cli: &Cli, args: &ImportArgs) -> Result<()> {
    let client = build_client(cli)?;

    let (data, content_type, file_size) = if let Some(path) = &args.file {
        let content = std::fs::read(path)
            .with_context(|| format!("Failed to read file: {}", path.display()))?;
        let ct = match path.extension().and_then(|e| e.to_str()) {
            Some("csv") => "text/csv",
            Some("ndjson") | Some("jsonl") => "application/x-ndjson",
            _ => "application/json",
        };
        let size = content.len();
        (content, ct, size)
    } else {
        let mut buf = Vec::new();
        std::io::stdin()
            .read_to_end(&mut buf)
            .context("Failed to read from stdin")?;
        let size = buf.len();
        (buf, "application/json", size)
    };

    let pb = ProgressBar::new(file_size as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .progress_chars("█▉▊▋▌▍▎▏  "),
    );

    // Split into batches if NDJSON
    if content_type == "application/x-ndjson" {
        let text = String::from_utf8_lossy(&data);
        let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();

        let mut batch = Vec::new();
        let mut batch_bytes = 0usize;
        let mut sent = 0u64;
        let mut task_uids = Vec::new();

        for line in &lines {
            let line_bytes = line.len() + 1;
            if batch_bytes + line_bytes > args.batch_size && !batch.is_empty() {
                let payload = batch.join("\n");
                let result = client
                    .add_documents_raw(
                        &args.uid,
                        payload.into_bytes(),
                        content_type,
                        args.primary_key.as_deref(),
                    )
                    .await?;
                if let Some(task_uid) = result["taskUid"].as_u64() {
                    task_uids.push(task_uid);
                }
                sent += batch_bytes as u64;
                pb.set_position(sent);
                batch.clear();
                batch_bytes = 0;
            }
            batch.push(*line);
            batch_bytes += line_bytes;
        }

        // Send remaining
        if !batch.is_empty() {
            let payload = batch.join("\n");
            let result = client
                .add_documents_raw(
                    &args.uid,
                    payload.into_bytes(),
                    content_type,
                    args.primary_key.as_deref(),
                )
                .await?;
            if let Some(task_uid) = result["taskUid"].as_u64() {
                task_uids.push(task_uid);
            }
        }

        pb.finish_with_message("upload complete");

        // Wait for all tasks
        println!("Waiting for {} task(s)...", task_uids.len());
        for task_uid in &task_uids {
            let task = client.wait_for_task(*task_uid, 300_000).await?;
            let status = task["status"].as_str().unwrap_or("unknown");
            if status == "failed" {
                let error = &task["error"];
                bail!(
                    "Task {} failed: {}",
                    task_uid,
                    error["message"].as_str().unwrap_or("unknown error")
                );
            }
        }
        println!(
            "✓ Imported {} batch(es) into '{}'",
            task_uids.len(),
            args.uid
        );
    } else if content_type == "text/csv" {
        // CSV: send as single batch (CSV doesn't split cleanly)
        let result = client
            .add_documents_raw(&args.uid, data, content_type, args.primary_key.as_deref())
            .await?;
        pb.finish_with_message("upload complete");

        if let Some(task_uid) = result["taskUid"].as_u64() {
            println!("Waiting for task {task_uid}...");
            let task = client.wait_for_task(task_uid, 300_000).await?;
            let status = task["status"].as_str().unwrap_or("unknown");
            if status == "failed" {
                let error = &task["error"];
                bail!(
                    "Task {} failed: {}",
                    task_uid,
                    error["message"].as_str().unwrap_or("unknown error")
                );
            }
            println!("✓ Imported into '{}'", args.uid);
        } else {
            print_json(&result, cli.raw);
        }
    } else {
        // JSON: try to split into batches by array elements
        let result = client
            .add_documents_raw(&args.uid, data, content_type, args.primary_key.as_deref())
            .await?;
        pb.finish_with_message("upload complete");

        if let Some(task_uid) = result["taskUid"].as_u64() {
            println!("Waiting for task {task_uid}...");
            let task = client.wait_for_task(task_uid, 300_000).await?;
            let status = task["status"].as_str().unwrap_or("unknown");
            if status == "failed" {
                let error = &task["error"];
                bail!(
                    "Task {} failed: {}",
                    task_uid,
                    error["message"].as_str().unwrap_or("unknown error")
                );
            }
            println!("✓ Imported into '{}'", args.uid);
        } else {
            print_json(&result, cli.raw);
        }
    }

    Ok(())
}
