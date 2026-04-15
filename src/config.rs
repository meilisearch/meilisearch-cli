use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub default: String,
    #[serde(default)]
    pub projects: BTreeMap<String, Project>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    #[serde(default)]
    pub r#type: Option<String>,
    pub url: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub data_dir: Option<String>,
    #[serde(default)]
    pub binary: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        let mut projects = BTreeMap::new();
        projects.insert(
            "local".to_string(),
            Project {
                r#type: Some("local".to_string()),
                url: "http://127.0.0.1:7700".to_string(),
                api_key: None,
                data_dir: Some("~/.local/share/msc/local".to_string()),
                binary: Some("~/.local/share/msc/bin/meilisearch-server".to_string()),
            },
        );
        Config {
            default: "local".to_string(),
            projects,
        }
    }
}

impl Config {
    pub fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir().context("Could not determine config directory")?;
        Ok(config_dir.join("msc").join("config.toml"))
    }

    /// Legacy config path from when the CLI binary was named "meilisearch".
    /// Used for one-time migration on first run.
    fn legacy_config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir().context("Could not determine config directory")?;
        Ok(config_dir.join("meilisearch").join("config.toml"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            // Migrate from old ~/.config/meilisearch/ location if present.
            let legacy = Self::legacy_config_path()?;
            if legacy.exists() {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::rename(&legacy, &path).with_context(|| {
                    format!(
                        "Failed to migrate config from {} to {}",
                        legacy.display(),
                        path.display()
                    )
                })?;
                // Remove the now-empty legacy directory (best-effort).
                if let Some(legacy_dir) = legacy.parent() {
                    std::fs::remove_dir(legacy_dir).ok();
                }
                eprintln!(
                    "Migrated config from {} to {}",
                    legacy.display(),
                    path.display()
                );
            } else {
                let config = Config::default();
                config.save()?;
                return Ok(config);
            }
        }
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;
        let config: Config =
            toml::from_str(&content).with_context(|| "Failed to parse config file")?;
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    pub fn get_project<'a>(&'a self, name: Option<&'a str>) -> Result<(&'a str, &'a Project)> {
        let name = name.unwrap_or(&self.default);
        let project = self.projects.get(name).with_context(|| {
            format!(
                "Project '{}' not found. Run `msc project list` to see available projects.",
                name
            )
        })?;
        Ok((name, project))
    }

    pub fn add_project(&mut self, name: String, project: Project) -> Result<()> {
        if self.projects.contains_key(&name) {
            bail!("Project '{}' already exists. Remove it first.", name);
        }
        self.projects.insert(name, project);
        self.save()
    }

    pub fn remove_project(&mut self, name: &str) -> Result<()> {
        if name == self.default {
            bail!(
                "Cannot remove the default project '{}'. Set a different default first.",
                name
            );
        }
        if self.projects.remove(name).is_none() {
            bail!("Project '{}' not found.", name);
        }
        self.save()
    }

    pub fn set_default(&mut self, name: &str) -> Result<()> {
        if !self.projects.contains_key(name) {
            bail!("Project '{}' not found.", name);
        }
        self.default = name.to_string();
        self.save()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.default, "local");
        assert!(config.projects.contains_key("local"));
        let local = &config.projects["local"];
        assert_eq!(local.url, "http://127.0.0.1:7700");
        assert_eq!(local.r#type.as_deref(), Some("local"));
    }

    #[test]
    fn test_serialize_roundtrip() {
        let config = Config::default();
        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: Config = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.default, config.default);
        assert_eq!(deserialized.projects.len(), config.projects.len());
    }

    #[test]
    fn test_add_project() {
        let mut config = Config::default();
        let project = Project {
            r#type: None,
            url: "https://prod.example.com".to_string(),
            api_key: Some("key123".to_string()),
            data_dir: None,
            binary: None,
        };
        // We can't actually save to disk in tests, so test the logic
        config
            .projects
            .insert("production".to_string(), project.clone());
        assert!(config.projects.contains_key("production"));
        assert_eq!(
            config.projects["production"].url,
            "https://prod.example.com"
        );
    }

    #[test]
    fn test_get_project_default() {
        let config = Config::default();
        let (name, project) = config.get_project(None).unwrap();
        assert_eq!(name, "local");
        assert_eq!(project.url, "http://127.0.0.1:7700");
    }

    #[test]
    fn test_get_project_not_found() {
        let config = Config::default();
        assert!(config.get_project(Some("nonexistent")).is_err());
    }

    #[test]
    fn test_set_default_nonexistent() {
        let mut config = Config::default();
        assert!(config.set_default("nonexistent").is_err());
    }

    #[test]
    fn test_parse_full_config() {
        let toml_str = r#"
default = "local"

[projects.local]
type = "local"
url = "http://127.0.0.1:7700"
api_key = "meilisearch_local_dev"
data_dir = "~/.local/share/msc/local"
binary = "~/.local/share/msc/bin/meilisearch-server"

[projects.production]
url = "https://my-instance.meilisearch.io"
api_key = "masterKey123"

[projects.staging]
url = "http://staging.meilisearch.io"
api_key = "stagingKey"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.default, "local");
        assert_eq!(config.projects.len(), 3);
        assert_eq!(
            config.projects["production"].api_key.as_deref(),
            Some("masterKey123")
        );
        assert_eq!(config.projects["local"].r#type.as_deref(), Some("local"));
    }
}
