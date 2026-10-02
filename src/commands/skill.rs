//! `msc skill`: install the agent skills bundled in this binary, so the
//! installed skill always matches the installed CLI version.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Subcommand;
use serde_json::{Value, json};

use super::print_json;
use crate::error::CliError;
use crate::output::emit;

/// Skills shipped with the CLI: (name, SKILL.md contents).
const SKILLS: &[(&str, &str)] = &[("msc", include_str!("../../skills/msc/SKILL.md"))];

#[derive(Subcommand)]
pub enum SkillCommand {
    /// List the bundled skills and where they are installed
    List,
    /// Print a bundled skill's SKILL.md
    Show {
        /// Skill name (default: msc)
        #[arg(default_value = "msc")]
        name: String,
    },
    /// Install the bundled skills (default: ~/.claude/skills)
    Install {
        #[command(flatten)]
        target: Target,
    },
    /// Remove installed skills
    Uninstall {
        #[command(flatten)]
        target: Target,
    },
}

#[derive(clap::Args)]
pub struct Target {
    /// Use ./.claude/skills (this repository only) instead of ~/.claude/skills
    #[arg(long, conflicts_with = "dir")]
    local: bool,
    /// Install into this skills directory (e.g. for other agents)
    #[arg(long, value_name = "DIR")]
    dir: Option<PathBuf>,
}

impl Target {
    fn root(&self) -> Result<PathBuf> {
        if let Some(dir) = &self.dir {
            return Ok(dir.clone());
        }
        if self.local {
            return Ok(std::env::current_dir()?.join(".claude/skills"));
        }
        user_skills_dir()
    }
}

pub fn user_skills_dir() -> Result<PathBuf> {
    Ok(dirs::home_dir()
        .context("Could not determine home directory")?
        .join(".claude/skills"))
}

fn skill_file(root: &Path, name: &str) -> PathBuf {
    root.join(name).join("SKILL.md")
}

/// Write every bundled skill under `root`; reports installed/updated/unchanged.
pub fn install_into(root: &Path) -> Result<Vec<Value>> {
    let mut results = vec![];
    for (name, content) in SKILLS {
        let path = skill_file(root, name);
        let status = match std::fs::read_to_string(&path) {
            Ok(existing) if existing == *content => "unchanged",
            Ok(_) => "updated",
            Err(_) => "installed",
        };
        if status != "unchanged" {
            std::fs::create_dir_all(path.parent().unwrap())
                .with_context(|| format!("Failed to create {}", root.display()))?;
            std::fs::write(&path, content)
                .with_context(|| format!("Failed to write {}", path.display()))?;
        }
        results.push(json!({ "name": name, "path": path.display().to_string(), "status": status }));
    }
    Ok(results)
}

fn description(content: &str) -> &str {
    content
        .lines()
        .find_map(|l| l.strip_prefix("description: "))
        .unwrap_or("")
}

pub fn run(cmd: &SkillCommand) -> Result<()> {
    match cmd {
        SkillCommand::List => {
            let user = user_skills_dir()?;
            let project = std::env::current_dir()?.join(".claude/skills");
            let skills: Vec<Value> = SKILLS
                .iter()
                .map(|(name, content)| {
                    let installed: Vec<String> = [&user, &project]
                        .iter()
                        .map(|root| skill_file(root, name))
                        .filter(|p| p.exists())
                        .map(|p| p.display().to_string())
                        .collect();
                    json!({ "name": name, "description": description(content), "installed": installed })
                })
                .collect();
            emit(&json!({ "results": skills }), || {
                skills
                    .iter()
                    .map(|s| {
                        let where_ = match s["installed"].as_array() {
                            Some(a) if !a.is_empty() => format!(
                                "installed: {}",
                                a.iter()
                                    .filter_map(|p| p.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ),
                            _ => "not installed (run `msc skill install`)".to_string(),
                        };
                        format!(
                            "  {} — {}\n    {}",
                            s["name"].as_str().unwrap_or(""),
                            s["description"].as_str().unwrap_or(""),
                            where_
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            });
        }
        SkillCommand::Show { name } => {
            let (_, content) = SKILLS.iter().find(|(n, _)| n == name).ok_or_else(|| {
                CliError::usage("unknown_skill", format!("Unknown skill: {name}"))
                    .with_hint("Run `msc skill list` to see bundled skills")
            })?;
            print!("{content}");
        }
        SkillCommand::Install { target } => {
            let root = target.root()?;
            let results = install_into(&root)?;
            emit(&json!({ "results": results }), || {
                results
                    .iter()
                    .map(|r| {
                        format!(
                            "✓ {} skill {} ({})",
                            r["name"].as_str().unwrap_or(""),
                            r["status"].as_str().unwrap_or(""),
                            r["path"].as_str().unwrap_or("")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            });
        }
        SkillCommand::Uninstall { target } => {
            let root = target.root()?;
            let mut results = vec![];
            for (name, _) in SKILLS {
                let dir = root.join(name);
                let removed = dir.join("SKILL.md").exists();
                if removed {
                    std::fs::remove_dir_all(&dir)
                        .with_context(|| format!("Failed to remove {}", dir.display()))?;
                }
                results.push(
                    json!({ "name": name, "path": dir.display().to_string(), "removed": removed }),
                );
            }
            print_json(&json!({ "results": results }));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_skills_have_frontmatter() {
        for (name, content) in SKILLS {
            assert!(content.starts_with("---\n"), "{name}");
            assert!(content.contains(&format!("name: {name}\n")), "{name}");
            assert!(!description(content).is_empty(), "{name}");
        }
    }

    #[test]
    fn install_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let first = install_into(dir.path()).unwrap();
        assert_eq!(first[0]["status"], "installed");
        let again = install_into(dir.path()).unwrap();
        assert_eq!(again[0]["status"], "unchanged");
        std::fs::write(skill_file(dir.path(), "msc"), "old").unwrap();
        assert_eq!(install_into(dir.path()).unwrap()[0]["status"], "updated");
    }
}
