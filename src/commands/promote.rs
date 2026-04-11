use anyhow::{Context, Result, bail};
use clap::Args;

use crate::client::MeiliClient;
use crate::config::Config;

#[derive(Args)]
pub struct PromoteArgs {
    /// Source project
    #[arg(long, default_value = "local")]
    pub from: String,

    /// Destination project
    #[arg(long)]
    pub to: String,

    /// Only promote specific indexes (comma-separated)
    #[arg(long, value_delimiter = ',')]
    pub indexes: Option<Vec<String>>,

    /// Dry run — show what would be promoted
    #[arg(long)]
    pub dry_run: bool,

    /// Create destination indexes if they don't exist
    #[arg(long)]
    pub create: bool,
}

pub async fn run(_cli: &super::Cli, args: &PromoteArgs) -> Result<()> {
    let config = Config::load()?;

    let (_, from_proj) = config.get_project(Some(&args.from))?;
    let (_, to_proj) = config.get_project(Some(&args.to))?;

    let src = MeiliClient::new(&from_proj.url, from_proj.api_key.as_deref())?;
    let dst = MeiliClient::new(&to_proj.url, to_proj.api_key.as_deref())?;

    println!("Promoting {} → {}", args.from, args.to);

    // Get list of indexes to promote
    let src_indexes = src.list_indexes(None, Some(1000)).await?;
    let index_list = src_indexes["results"]
        .as_array()
        .context("Failed to list source indexes")?;

    let uids: Vec<String> = if let Some(filter) = &args.indexes {
        filter.clone()
    } else {
        index_list
            .iter()
            .filter_map(|i| i["uid"].as_str().map(String::from))
            .collect()
    };

    if uids.is_empty() {
        println!("No indexes to promote.");
        return Ok(());
    }

    if args.dry_run {
        println!("Dry run — would promote:");
        for uid in &uids {
            let stats = src.index_stats(uid).await?;
            let doc_count = stats["numberOfDocuments"].as_u64().unwrap_or(0);
            println!("  {} ({} docs)", uid, doc_count);
        }
        return Ok(());
    }

    let start = std::time::Instant::now();
    let mut promoted = 0;

    for uid in &uids {
        let idx_start = std::time::Instant::now();

        // 1. Create destination index if needed
        if args.create {
            let _ = dst.create_index(uid, None).await;
        }

        // 2. Sync settings
        let settings = src.get_settings(uid).await?;
        let task = dst.update_settings(uid, &settings).await?;
        if let Some(task_uid) = task["taskUid"].as_u64() {
            let result = dst.wait_for_task(task_uid, 60_000).await?;
            if result["status"].as_str() == Some("failed") {
                bail!("Settings sync failed for '{}': {:?}", uid, result["error"]);
            }
        }

        // 3. Try export route first
        let export_result = src
            .export(&serde_json::json!({
                "url": dst.base_url,
                "apiKey": config.get_project(Some(&args.to))?.1.api_key,
                "indexes": [uid]
            }))
            .await;

        if let Ok(task) = export_result {
            // Export route available — wait for task
            if let Some(task_uid) = task["taskUid"].as_u64() {
                let result = src.wait_for_task(task_uid, 300_000).await?;
                if result["status"].as_str() == Some("failed") {
                    bail!("Export failed for '{}': {:?}", uid, result["error"]);
                }
            }
        } else {
            // Fallback to manual copy via pagination
            let mut offset = 0u64;
            let batch_limit = 1000u64;
            loop {
                let docs = src
                    .get_documents(uid, Some(offset), Some(batch_limit), None)
                    .await?;
                let results = docs["results"].as_array();
                let count = results.map(|r| r.len()).unwrap_or(0);
                if count == 0 {
                    break;
                }
                let docs_array = serde_json::json!(results.unwrap());
                let task = dst.add_documents(uid, &docs_array, None).await?;
                if let Some(task_uid) = task["taskUid"].as_u64() {
                    dst.wait_for_task(task_uid, 300_000).await?;
                }
                offset += batch_limit;
                if count < batch_limit as usize {
                    break;
                }
            }
        }

        // 4. Verify
        let src_stats = src.index_stats(uid).await?;
        let doc_count = src_stats["numberOfDocuments"].as_u64().unwrap_or(0);
        let elapsed = idx_start.elapsed().as_secs_f64();

        println!(
            "  ✓ {:20} {:>8} docs  settings synced  {:.1}s",
            uid, doc_count, elapsed
        );
        promoted += 1;
    }

    let total_elapsed = start.elapsed().as_secs_f64();
    println!(
        "\nDone. {} indexes promoted in {:.1}s.",
        promoted, total_elapsed
    );
    Ok(())
}
