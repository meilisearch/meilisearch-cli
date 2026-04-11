use anyhow::{Result, bail};
use clap::Args;

use crate::client::MeiliClient;
use crate::config::Config;

use super::print_json;

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
}

pub async fn run(cli: &super::Cli, args: &CloneArgs) -> Result<()> {
    let config = Config::load()?;

    let from_name = args
        .from
        .as_deref()
        .or(cli.project.as_deref())
        .unwrap_or(&config.default);
    let to_name = args.to.as_deref().unwrap_or(from_name);

    let (_, from_proj) = config.get_project(Some(from_name))?;
    let (_, to_proj) = config.get_project(Some(to_name))?;

    let src_client = MeiliClient::new(&from_proj.url, from_proj.api_key.as_deref())?;
    let dst_client = MeiliClient::new(&to_proj.url, to_proj.api_key.as_deref())?;

    println!(
        "Cloning '{}' ({}) → '{}' ({})",
        args.source_uid, from_name, args.dest_uid, to_name
    );

    // 1. Get settings from source
    let settings = src_client.get_settings(&args.source_uid).await?;

    // 2. Create destination index
    let _ = dst_client.create_index(&args.dest_uid, None).await;

    // 3. Apply settings
    let task = dst_client
        .update_settings(&args.dest_uid, &settings)
        .await?;
    if let Some(task_uid) = task["taskUid"].as_u64() {
        let result = dst_client.wait_for_task(task_uid, 60_000).await?;
        if result["status"].as_str() == Some("failed") {
            bail!("Settings update failed: {:?}", result["error"]);
        }
    }
    println!("  ✓ Settings synced");

    // 4. Export documents from source using pagination
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

        let docs_array = serde_json::json!(results.unwrap());
        let task = dst_client
            .add_documents(&args.dest_uid, &docs_array, None)
            .await?;

        if let Some(task_uid) = task["taskUid"].as_u64() {
            let result = dst_client.wait_for_task(task_uid, 300_000).await?;
            if result["status"].as_str() == Some("failed") {
                bail!("Document import failed: {:?}", result["error"]);
            }
        }

        total_docs += count as u64;
        offset += batch_limit;

        if count < batch_limit as usize {
            break;
        }
    }

    println!("  ✓ {} documents cloned", total_docs);

    if !cli.raw {
        let stats = dst_client.index_stats(&args.dest_uid).await?;
        print_json(&stats, cli.raw);
    }

    println!(
        "\nDone. Cloned '{}' → '{}'.",
        args.source_uid, args.dest_uid
    );
    Ok(())
}
