use anyhow::Result;
use clap::Subcommand;
use dialoguer::{Input, Password};

use crate::config::{Config, Project};

#[derive(Subcommand)]
pub enum ProjectCommand {
    /// Add a new project
    Add {
        name: String,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        api_key: Option<String>,
    },
    /// Remove a project
    Remove { name: String },
    /// List all projects
    List,
    /// Set the default project
    Use { name: String },
    /// Show the current default project
    Current,
}

pub async fn run(cmd: &ProjectCommand) -> Result<()> {
    match cmd {
        ProjectCommand::Add { name, url, api_key } => {
            let resolved_url = match url {
                Some(u) => u.clone(),
                None => Input::<String>::new()
                    .with_prompt("URL")
                    .interact_text()?,
            };
            let resolved_key = match api_key {
                Some(k) => Some(k.clone()),
                None => {
                    let key: String = Password::new()
                        .with_prompt("API key (leave empty for none)")
                        .allow_empty_password(true)
                        .interact()?;
                    if key.is_empty() { None } else { Some(key) }
                }
            };
            let mut config = Config::load()?;
            let project = Project {
                r#type: None,
                url: resolved_url,
                api_key: resolved_key,
                data_dir: None,
                binary: None,
            };
            config.add_project(name.clone(), project)?;
            println!("Project '{}' added.", name);
        }
        ProjectCommand::Remove { name } => {
            let mut config = Config::load()?;
            config.remove_project(name)?;
            println!("Project '{}' removed.", name);
        }
        ProjectCommand::List => {
            let config = Config::load()?;
            for (name, project) in &config.projects {
                let marker = if name == &config.default {
                    " (default)"
                } else {
                    ""
                };
                let key_info = if project.api_key.is_some() { " [api key set]" } else { "" };
                println!("  {}{} — {}{}", name, marker, project.url, key_info);
            }
        }
        ProjectCommand::Use { name } => {
            let mut config = Config::load()?;
            config.set_default(name)?;
            println!("Default project set to '{}'.", name);
        }
        ProjectCommand::Current => {
            let config = Config::load()?;
            let (name, project) = config.get_project(None)?;
            println!("{} — {}", name, project.url);
        }
    }
    Ok(())
}
