use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Subcommand;
use serde_json::{Map, Value, json};

use super::{Cli, build_client, print_json, read_json_input};
use crate::error::CliError;
use crate::status;

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
        return Err(CliError::usage(
            "invalid_settings_sub_resource",
            format!("Unknown settings sub-resource: '{sub}'"),
        )
        .with_hint(format!(
            "Valid sub-resources: {}",
            VALID_SUB_RESOURCES.join(", ")
        ))
        .into());
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
        /// Show the changes that would be applied without applying them
        #[arg(long)]
        dry_run: bool,
    },
    /// Reset settings to defaults (all or a specific sub-resource)
    Reset {
        /// Index UID
        #[arg(value_name = "INDEX_UID")]
        uid: String,
        /// Optional sub-resource (e.g. synonyms, displayed-attributes, typo-tolerance)
        sub_resource: Option<String>,
        /// Show the current values that would be reset without resetting them
        #[arg(long)]
        dry_run: bool,
    },
    /// Edit settings in $EDITOR or interactive TUI
    Edit {
        /// Index UID
        #[arg(value_name = "INDEX_UID")]
        uid: String,
        /// Optional sub-resource (e.g. synonyms, displayed-attributes, typo-tolerance)
        sub_resource: Option<String>,
        /// Interactive TUI mode
        #[arg(short, long)]
        interactive: bool,
    },
    /// Show what changes between current settings and a settings file
    Diff {
        /// Index UID
        #[arg(value_name = "INDEX_UID")]
        uid: String,
        /// Settings JSON to compare against (reads stdin if omitted)
        #[arg(long)]
        against: Option<PathBuf>,
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
            print_json(&result);
        }
        SettingsCommand::Update {
            uid,
            sub_resource,
            file,
            dry_run,
        } => {
            let settings = read_json_input(file.as_deref())?;
            if let Some(sub) = sub_resource {
                validate_sub_resource(sub)?;
            }
            if *dry_run {
                let changes = match sub_resource {
                    Some(sub) => {
                        let current = client.get_setting(uid, sub).await?;
                        diff_value(&camel_case(sub), &current, &settings)
                    }
                    None => diff_settings(&client.get_settings(uid).await?, &settings),
                };
                print_json(&json!({
                    "dryRun": true,
                    "action": "settings.update",
                    "indexUid": uid,
                    "changes": changes,
                }));
                return Ok(());
            }
            let result = if let Some(sub) = sub_resource {
                client.update_setting(uid, sub, &settings).await?
            } else {
                client.update_settings(uid, &settings).await?
            };
            print_json(&result);
        }
        SettingsCommand::Reset {
            uid,
            sub_resource,
            dry_run,
        } => {
            if let Some(sub) = sub_resource {
                validate_sub_resource(sub)?;
            }
            if *dry_run {
                let current = match sub_resource {
                    Some(sub) => json!({ camel_case(sub): client.get_setting(uid, sub).await? }),
                    None => client.get_settings(uid).await?,
                };
                print_json(&json!({
                    "dryRun": true,
                    "action": "settings.reset",
                    "indexUid": uid,
                    "current": current,
                }));
                return Ok(());
            }
            let result = if let Some(sub) = sub_resource {
                client.reset_setting(uid, sub).await?
            } else {
                client.reset_settings(uid).await?
            };
            print_json(&result);
        }
        SettingsCommand::Edit {
            uid,
            sub_resource,
            interactive,
        } => {
            let alternative = format!(
                "Use `msc settings get {uid}` then `msc settings update {uid} --file <FILE>` (add --dry-run to preview)"
            );
            if *interactive {
                crate::output::require_interactive("Interactive settings (-i)", &alternative)?;
                let client = build_client(cli)?;
                return crate::tui::settings::run_interactive_settings(client, uid).await;
            }
            crate::output::require_interactive("Editing settings in $EDITOR", &alternative)?;
            run_editor(cli, uid, sub_resource.as_deref()).await?;
        }
        SettingsCommand::Diff { uid, against } => {
            let proposed = read_json_input(against.as_deref())?;
            let current = client.get_settings(uid).await?;
            print_json(&json!({ "indexUid": uid, "changes": diff_settings(&current, &proposed) }));
        }
    }
    Ok(())
}

async fn run_editor(cli: &Cli, uid: &str, sub_resource: Option<&str>) -> Result<()> {
    if let Some(sub) = sub_resource {
        validate_sub_resource(sub)?;
    }

    let client = build_client(cli)?;
    let settings = if let Some(sub) = sub_resource {
        client.get_setting(uid, sub).await?
    } else {
        client.get_settings(uid).await?
    };
    let pretty = serde_json::to_string_pretty(&settings)?;

    let file_label = sub_resource.unwrap_or("settings");
    let tmp_path = std::env::temp_dir().join(format!("msc-{file_label}-{uid}.json"));
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
        status!("No changes.");
        return Ok(());
    }

    let new_settings: serde_json::Value =
        serde_json::from_str(&new_content).context("Failed to parse edited settings JSON")?;

    status!("Changes detected. Applying...");

    let client = client.without_wait();
    let result = if let Some(sub) = sub_resource {
        client.update_setting(uid, sub, &new_settings).await?
    } else {
        client.update_settings(uid, &new_settings).await?
    };

    let task = match result["taskUid"].as_u64() {
        Some(task_uid) => {
            status!("Waiting for task {task_uid}...");
            client.wait_for_success(task_uid, cli.wait_timeout).await?
        }
        None => result,
    };
    print_json(&task);

    Ok(())
}

/// `typo-tolerance` → `typoTolerance`
fn camel_case(kebab: &str) -> String {
    let mut out = String::new();
    let mut upper = false;
    for c in kebab.chars() {
        if c == '-' {
            upper = true;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

fn diff_value(key: &str, from: &Value, to: &Value) -> Value {
    let mut changes = Map::new();
    if from != to {
        changes.insert(key.to_string(), json!({ "from": from, "to": to }));
    }
    Value::Object(changes)
}

/// Settings updates are partial: only keys present in `proposed` change.
fn diff_settings(current: &Value, proposed: &Value) -> Value {
    let mut changes = Map::new();
    if let Some(obj) = proposed.as_object() {
        for (key, to) in obj {
            let from = current.get(key).cloned().unwrap_or(Value::Null);
            if &from != to {
                changes.insert(key.clone(), json!({ "from": from, "to": to }));
            }
        }
    }
    Value::Object(changes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_only_reports_changed_keys() {
        let current =
            json!({"searchableAttributes": ["*"], "stopWords": [], "distinctAttribute": null});
        let proposed = json!({"searchableAttributes": ["title"], "stopWords": []});
        let d = diff_settings(&current, &proposed);
        assert_eq!(
            d,
            json!({"searchableAttributes": {"from": ["*"], "to": ["title"]}})
        );
    }

    #[test]
    fn camel_cases_sub_resources() {
        assert_eq!(camel_case("typo-tolerance"), "typoTolerance");
        assert_eq!(camel_case("synonyms"), "synonyms");
    }
}
