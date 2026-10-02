use anyhow::Result;
use clap::Subcommand;
use dialoguer::{Input, Password};
use serde_json::json;

use super::{Cli, print_json, print_skipped};
use crate::config::{Config, Project};
use crate::output::{emit, require_interactive};

#[derive(Subcommand)]
pub enum ProjectCommand {
    /// Add a new project (uses the global --url and --api-key; prompts if missing)
    Add {
        /// Project name
        name: String,
        /// Succeed without changes if the project already exists
        #[arg(long, conflicts_with = "force")]
        if_not_exists: bool,
        /// Overwrite the project if it already exists
        #[arg(long)]
        force: bool,
    },
    /// Remove a project
    Remove {
        /// Project name
        name: String,
        /// Succeed without changes if the project does not exist
        #[arg(long)]
        if_exists: bool,
    },
    /// List all projects
    List,
    /// Set the default project
    Use {
        /// Project name
        name: String,
    },
    /// Update an existing project (with --url / --api-key, or interactively)
    Update {
        /// Project name
        name: String,
        /// Remove the stored API key
        #[arg(long, conflicts_with = "api_key")]
        remove_api_key: bool,
    },
    /// Show the current default project
    Current,
}

fn project_json(name: &str, project: &Project, default: &str) -> serde_json::Value {
    json!({
        "name": name,
        "url": project.url,
        "default": name == default,
        "apiKeySet": project.api_key.is_some(),
        "type": project.r#type,
    })
}

pub async fn run(cli: &Cli, cmd: &ProjectCommand) -> Result<()> {
    match cmd {
        ProjectCommand::Add {
            name,
            if_not_exists,
            force,
        } => {
            let mut config = Config::load()?;
            if config.projects.contains_key(name) && *if_not_exists {
                print_skipped("already_exists", json!({ "name": name }));
                return Ok(());
            }
            let resolved_url = match &cli.url {
                Some(u) => u.clone(),
                None => {
                    require_interactive(
                        "Prompting for the project URL",
                        "Pass --url <URL> (and --api-key <KEY> if needed)",
                    )?;
                    Input::<String>::new().with_prompt("URL").interact_text()?
                }
            };
            let resolved_key = match &cli.api_key {
                Some(k) => Some(k.clone()),
                // Non-interactive callers that gave a URL but no key get no key.
                None if cli.url.is_some() && !crate::output::is_interactive() => None,
                None => {
                    let key: String = Password::new()
                        .with_prompt("API key (leave empty for none)")
                        .allow_empty_password(true)
                        .interact()?;
                    if key.is_empty() { None } else { Some(key) }
                }
            };
            let project = Project {
                r#type: None,
                url: resolved_url,
                api_key: resolved_key,
                data_dir: None,
                binary: None,
            };
            let out = project_json(name, &project, &config.default);
            if *force && config.projects.contains_key(name) {
                config.update_project(name, project)?;
            } else {
                config.add_project(name.clone(), project)?;
            }
            emit(&out, || format!("Project '{name}' added."));
        }
        ProjectCommand::Remove { name, if_exists } => {
            let mut config = Config::load()?;
            if *if_exists && !config.projects.contains_key(name) {
                print_skipped("not_found", json!({ "name": name }));
                return Ok(());
            }
            config.remove_project(name)?;
            emit(&json!({ "removed": true, "name": name }), || {
                format!("Project '{name}' removed.")
            });
        }
        ProjectCommand::List => {
            let config = Config::load()?;
            if crate::output::is_json() {
                let projects: Vec<_> = config
                    .projects
                    .iter()
                    .map(|(n, p)| project_json(n, p, &config.default))
                    .collect();
                print_json(&json!({ "default": config.default, "results": projects }));
                return Ok(());
            }
            for (name, project) in &config.projects {
                let marker = if name == &config.default {
                    " (default)"
                } else {
                    ""
                };
                let key_info = if project.api_key.is_some() {
                    " [api key set]"
                } else {
                    ""
                };
                println!("  {}{} — {}{}", name, marker, project.url, key_info);
            }
        }
        ProjectCommand::Use { name } => {
            let mut config = Config::load()?;
            config.set_default(name)?;
            emit(&json!({ "default": name }), || {
                format!("Default project set to '{name}'.")
            });
        }
        ProjectCommand::Update {
            name,
            remove_api_key,
        } => {
            let config = Config::load()?;
            let (_, existing) = config.get_project(Some(name))?;
            let non_interactive = cli.url.is_some() || cli.api_key.is_some() || *remove_api_key;
            let (url, api_key) = if non_interactive {
                let api_key = if *remove_api_key {
                    None
                } else {
                    cli.api_key.clone().or_else(|| existing.api_key.clone())
                };
                (
                    cli.url.clone().unwrap_or_else(|| existing.url.clone()),
                    api_key,
                )
            } else {
                require_interactive(
                    "Prompting for project settings",
                    "Pass --url, --api-key or --remove-api-key",
                )?;
                let url = Input::<String>::new()
                    .with_prompt("URL")
                    .default(existing.url.clone())
                    .interact_text()?;
                let key: String = Password::new()
                    .with_prompt("API key (leave empty to remove)")
                    .allow_empty_password(true)
                    .interact()?;
                (url, if key.is_empty() { None } else { Some(key) })
            };
            let project = Project {
                r#type: existing.r#type.clone(),
                url,
                api_key,
                data_dir: existing.data_dir.clone(),
                binary: existing.binary.clone(),
            };
            let out = project_json(name, &project, &config.default);
            let mut config = Config::load()?;
            config.update_project(name, project)?;
            emit(&out, || format!("Project '{name}' updated."));
        }
        ProjectCommand::Current => {
            let config = Config::load()?;
            let (name, project) = config.get_project(cli.project.as_deref())?;
            emit(&project_json(name, project, &config.default), || {
                format!("{} — {}", name, project.url)
            });
        }
    }
    Ok(())
}
