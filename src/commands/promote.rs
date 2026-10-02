use anyhow::{Context, Result};
use clap::Args;
use serde_json::json;

use super::{client_for, print_json, resolve_target};
use crate::client::ensure_succeeded;
use crate::error::is_not_found;
use crate::status;

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

pub async fn run(cli: &super::Cli, args: &PromoteArgs) -> Result<()> {
    let from = resolve_target(cli, Some(&args.from))?;
    let to = resolve_target(cli, Some(&args.to))?;

    // This command manages task waiting itself.
    let src = client_for(cli, &from)?.without_wait();
    let dst = client_for(cli, &to)?.without_wait();
    let timeout = cli.wait_timeout;

    let uids: Vec<String> = if let Some(filter) = &args.indexes {
        filter.clone()
    } else {
        let src_indexes = src.list_indexes(None, Some(1000)).await?;
        src_indexes["results"]
            .as_array()
            .context("Failed to list source indexes")?
            .iter()
            .filter_map(|i| i["uid"].as_str().map(String::from))
            .collect()
    };

    if args.dry_run {
        let mut plan = vec![];
        for uid in &uids {
            let stats = src.index_stats(uid).await?;
            let dest_exists = match dst.get_index(uid).await {
                Ok(_) => true,
                Err(e) if is_not_found(&e) => false,
                Err(e) => return Err(e),
            };
            plan.push(json!({
                "indexUid": uid,
                "numberOfDocuments": stats["numberOfDocuments"],
                "destinationExists": dest_exists,
            }));
        }
        let result = json!({
            "dryRun": true,
            "action": "promote",
            "from": from.name,
            "to": to.name,
            "indexes": plan,
        });
        crate::output::emit(&result, || {
            let mut out = format!("Dry run — would promote {} → {}:", from.name, to.name);
            for p in &plan {
                out.push_str(&format!(
                    "\n  {} ({} docs){}",
                    p["indexUid"].as_str().unwrap_or(""),
                    p["numberOfDocuments"],
                    if p["destinationExists"] == true {
                        ""
                    } else {
                        " [new]"
                    }
                ));
            }
            out
        });
        return Ok(());
    }

    status!("Promoting {} → {}", from.name, to.name);

    let start = std::time::Instant::now();
    let mut promoted = vec![];

    for uid in &uids {
        let idx_start = std::time::Instant::now();

        if args.create {
            let _ = dst.create_index(uid, None).await;
        }

        let settings = src.get_settings(uid).await?;
        let task = dst.update_settings(uid, &settings).await?;
        if let Some(task_uid) = task["taskUid"].as_u64() {
            dst.wait_for_success(task_uid, timeout)
                .await
                .with_context(|| format!("Settings sync failed for '{uid}'"))?;
        }

        // Prefer the server-side export route; fall back to paginated copy.
        let export_result = src
            .export(&json!({
                "url": dst.base_url,
                "apiKey": to.api_key,
                "indexes": [uid]
            }))
            .await;

        let method = if let Ok(task) = export_result {
            if let Some(task_uid) = task["taskUid"].as_u64() {
                src.wait_for_success(task_uid, timeout)
                    .await
                    .with_context(|| format!("Export failed for '{uid}'"))?;
            }
            "export"
        } else {
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
                let docs_array = json!(results.unwrap());
                let task = dst.add_documents(uid, &docs_array, None).await?;
                if let Some(task_uid) = task["taskUid"].as_u64() {
                    ensure_succeeded(dst.wait_for_task(task_uid, timeout).await?)?;
                }
                offset += batch_limit;
                if count < batch_limit as usize {
                    break;
                }
            }
            "copy"
        };

        let src_stats = src.index_stats(uid).await?;
        let doc_count = src_stats["numberOfDocuments"].as_u64().unwrap_or(0);
        let elapsed = idx_start.elapsed().as_secs_f64();

        status!(
            "  ✓ {:20} {:>8} docs  settings synced  {:.1}s",
            uid,
            doc_count,
            elapsed
        );
        promoted.push(json!({
            "indexUid": uid,
            "numberOfDocuments": doc_count,
            "method": method,
            "seconds": elapsed,
        }));
    }

    let total_elapsed = start.elapsed().as_secs_f64();
    let result = json!({
        "from": from.name,
        "to": to.name,
        "indexes": promoted,
        "seconds": total_elapsed,
    });
    if crate::output::is_json() {
        print_json(&result);
    } else {
        println!(
            "Done. {} indexes promoted in {:.1}s.",
            promoted.len(),
            total_elapsed
        );
    }
    Ok(())
}
