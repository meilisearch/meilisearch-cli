use anyhow::Result;
use clap::Args;
use serde_json::json;

use super::{client_for, print_json, resolve_target};
use crate::client::ensure_succeeded;
use crate::error::is_not_found;
use crate::status;

#[derive(Args)]
pub struct CloneArgs {
    /// Source index UID
    pub source_uid: String,

    /// Destination index UID
    pub dest_uid: String,

    /// Source project
    #[arg(long)]
    pub from: Option<String>,

    /// Destination project
    #[arg(long)]
    pub to: Option<String>,

    /// Show what would be cloned without writing anything
    #[arg(long)]
    pub dry_run: bool,
}

pub async fn run(cli: &super::Cli, args: &CloneArgs) -> Result<()> {
    let from = resolve_target(cli, args.from.as_deref())?;
    let to = resolve_target(cli, args.to.as_deref().or(args.from.as_deref()))?;

    // This command manages task waiting itself.
    let src_client = client_for(cli, &from)?.without_wait();
    let dst_client = client_for(cli, &to)?.without_wait();
    let timeout = cli.wait_timeout;

    let settings = src_client.get_settings(&args.source_uid).await?;

    if args.dry_run {
        let stats = src_client.index_stats(&args.source_uid).await?;
        let dest_exists = match dst_client.get_index(&args.dest_uid).await {
            Ok(_) => true,
            Err(e) if is_not_found(&e) => false,
            Err(e) => return Err(e),
        };
        print_json(&json!({
            "dryRun": true,
            "action": "clone",
            "source": { "project": from.name, "indexUid": args.source_uid },
            "destination": { "project": to.name, "indexUid": args.dest_uid, "exists": dest_exists },
            "numberOfDocuments": stats["numberOfDocuments"],
            "settings": settings,
        }));
        return Ok(());
    }

    status!(
        "Cloning '{}' ({}) → '{}' ({})",
        args.source_uid,
        from.name,
        args.dest_uid,
        to.name
    );

    // Creating may fail if the index already exists; settings are applied either way.
    let _ = dst_client.create_index(&args.dest_uid, None).await;

    let task = dst_client
        .update_settings(&args.dest_uid, &settings)
        .await?;
    if let Some(task_uid) = task["taskUid"].as_u64() {
        dst_client.wait_for_success(task_uid, timeout).await?;
    }
    status!("  ✓ Settings synced");

    let mut offset = 0u64;
    let batch_limit = 1000u64;
    let mut total_docs = 0u64;

    loop {
        let docs = src_client
            .get_documents(&args.source_uid, Some(offset), Some(batch_limit), None)
            .await?;

        let results = docs["results"].as_array();
        let count = results.map(|r| r.len()).unwrap_or(0);

        if count == 0 {
            break;
        }

        let docs_array = json!(results.unwrap());
        let task = dst_client
            .add_documents(&args.dest_uid, &docs_array, None)
            .await?;

        if let Some(task_uid) = task["taskUid"].as_u64() {
            let finished = dst_client.wait_for_task(task_uid, timeout).await?;
            ensure_succeeded(finished)?;
        }

        total_docs += count as u64;
        offset += batch_limit;

        if count < batch_limit as usize {
            break;
        }
    }

    status!("  ✓ {} documents cloned", total_docs);

    let stats = dst_client.index_stats(&args.dest_uid).await?;
    let result = json!({
        "source": { "project": from.name, "indexUid": args.source_uid },
        "destination": { "project": to.name, "indexUid": args.dest_uid },
        "documentsCopied": total_docs,
        "stats": stats,
    });
    crate::output::emit(&result, || {
        format!(
            "Done. Cloned '{}' → '{}' ({} documents).",
            args.source_uid, args.dest_uid, total_docs
        )
    });
    Ok(())
}
