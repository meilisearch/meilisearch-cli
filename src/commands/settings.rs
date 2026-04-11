use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Subcommand;

use super::{Cli, build_client, print_json, read_json_input};

/// Valid settings sub-resources.
const VALID_SUB_RESOURCES: &[&str] = &[
    "chat",
    "dictionary",
    "displayed-attributes",
    "distinct-attribute",
    "embedders",
    "facet-search",
    "faceting",
    "filterable-attributes",
    "localized-attributes",
    "non-separator-tokens",
    "pagination",
    "prefix-search",
    "proximity-precision",
    "ranking-rules",
    "search-cutoff-ms",
    "searchable-attributes",
    "separator-tokens",
    "sortable-attributes",
    "stop-words",
    "synonyms",
    "typo-tolerance",
];

fn validate_sub_resource(sub: &str) -> Result<()> {
    if !VALID_SUB_RESOURCES.contains(&sub) {
        anyhow::bail!(
            "Unknown settings sub-resource: '{}'\nValid sub-resources: {}",
            sub,
            VALID_SUB_RESOURCES.join(", ")
        );
    }
    Ok(())
}

#[derive(Subcommand)]
pub enum SettingsCommand {
    /// Get current settings (all or a specific sub-resource)
    Get {
        /// Index UID
        #[arg(value_name = "INDEX_UID")]
        uid: String,
        /// Optional sub-resource (e.g. synonyms, displayed-attributes, typo-tolerance)
        sub_resource: Option<String>,
    },
    /// Update settings from JSON file or stdin (all or a specific sub-resource)
    Update {
        /// Index UID
        #[arg(value_name = "INDEX_UID")]
        uid: String,
        /// Optional sub-resource (e.g. synonyms, displayed-attributes, typo-tolerance)
        sub_resource: Option<String>,
        /// JSON file (reads from stdin if omitted)
        #[arg(long)]
        file: Option<PathBuf>,
    },
    /// Reset settings to defaults (all or a specific sub-resource)
    Reset {
        /// Index UID
        #[arg(value_name = "INDEX_UID")]
        uid: String,
        /// Optional sub-resource (e.g. synonyms, displayed-attributes, typo-tolerance)
        sub_resource: Option<String>,
    },
    /// Edit settings in $EDITOR
    Edit {
        /// Index UID
        #[arg(value_name = "INDEX_UID")]
        uid: String,
    },
    /// Show diff of current vs default settings
    Diff {
        /// Index UID
        #[arg(value_name = "INDEX_UID")]
        uid: String,
    },
}

pub async fn run(cli: &Cli, cmd: &SettingsCommand) -> Result<()> {
    let client = build_client(cli)?;
    match cmd {
        SettingsCommand::Get { uid, sub_resource } => {
            let result = if let Some(sub) = sub_resource {
                validate_sub_resource(sub)?;
                client.get_setting(uid, sub).await?
            } else {
                client.get_settings(uid).await?
            };
            print_json(&result, cli.raw);
        }
        SettingsCommand::Update {
            uid,
            sub_resource,
            file,
        } => {
            let settings = read_json_input(file.as_deref())?;
            let result = if let Some(sub) = sub_resource {
                validate_sub_resource(sub)?;
                client.update_setting(uid, sub, &settings).await?
            } else {
                client.update_settings(uid, &settings).await?
            };
            print_json(&result, cli.raw);
        }
        SettingsCommand::Reset { uid, sub_resource } => {
            let result = if let Some(sub) = sub_resource {
                validate_sub_resource(sub)?;
                client.reset_setting(uid, sub).await?
            } else {
                client.reset_settings(uid).await?
            };
            print_json(&result, cli.raw);
        }
        SettingsCommand::Edit { uid } => {
            run_editor(cli, uid).await?;
        }
        SettingsCommand::Diff { uid } => {
            let settings = client.get_settings(uid).await?;
            print_json(&settings, cli.raw);
            println!("\n(Diff shows current settings. Reset to return to defaults.)");
        }
    }
    Ok(())
}

async fn run_editor(cli: &Cli, uid: &str) -> Result<()> {
    let client = build_client(cli)?;
    let settings = client.get_settings(uid).await?;
    let pretty = serde_json::to_string_pretty(&settings)?;

    let tmp_path = std::env::temp_dir().join(format!("meilisearch-settings-{uid}.json"));
    std::fs::write(&tmp_path, &pretty)?;

    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| {
            if which::which("vim").is_ok() {
                "vim".to_string()
            } else if which::which("nano").is_ok() {
                "nano".to_string()
            } else {
                "vi".to_string()
            }
        });

    let status = std::process::Command::new(&editor)
        .arg(&tmp_path)
        .status()
        .with_context(|| format!("Failed to open editor: {editor}"))?;

    if !status.success() {
        anyhow::bail!("Editor exited with non-zero status");
    }

    let new_content = std::fs::read_to_string(&tmp_path)?;
    let _ = std::fs::remove_file(&tmp_path);

    if new_content == pretty {
        println!("No changes.");
        return Ok(());
    }

    let new_settings: serde_json::Value =
        serde_json::from_str(&new_content).context("Failed to parse edited settings JSON")?;

    // Show diff
    println!("Changes detected. Applying...");

    let result = client.update_settings(uid, &new_settings).await?;
    print_json(&result, cli.raw);

    // Wait for task
    if let Some(task_uid) = result["taskUid"].as_u64() {
        print!("Waiting for task {task_uid}...");
        std::io::stdout().flush()?;
        let task = client.wait_for_task(task_uid, 30000).await?;
        println!(" {}", task["status"].as_str().unwrap_or("done"));
    }

    Ok(())
}
