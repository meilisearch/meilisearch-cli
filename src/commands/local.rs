use anyhow::{Context, Result, bail};
use clap::Subcommand;
use serde_json::{Value, json};

use super::Cli;
use crate::output::emit;
use crate::status;

const LOCAL_URL: &str = "http://127.0.0.1:7700";

#[derive(Subcommand)]
pub enum LocalCommand {
    /// Start the local Meilisearch instance
    Start,
    /// Stop the local Meilisearch instance
    Stop,
    /// Restart the local Meilisearch instance
    Restart,
    /// Show status of the local instance
    Status,
    /// Show logs of the local instance
    Logs {
        /// Follow log output
        #[arg(short, long)]
        follow: bool,
    },
    /// Reset local data (wipe and restart fresh)
    Reset {
        /// Show what would be wiped without doing it
        #[arg(long)]
        dry_run: bool,
    },
    /// Upgrade local Meilisearch to latest version
    Upgrade,
}

fn data_dir() -> Result<std::path::PathBuf> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    Ok(home.join(".local/share/msc/local"))
}

fn docker_available() -> bool {
    std::process::Command::new("docker")
        .arg("info")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn container_running() -> bool {
    std::process::Command::new("docker")
        .args([
            "inspect",
            "--format",
            "{{.State.Running}}",
            "meilisearch-local",
        ])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "true")
        .unwrap_or(false)
}

fn container_exists() -> bool {
    std::process::Command::new("docker")
        .args(["inspect", "meilisearch-local"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Run a docker command without letting its output reach our stdout.
fn docker(args: &[&str]) -> Result<()> {
    let output = std::process::Command::new("docker")
        .args(args)
        .output()
        .context("Failed to run docker")?;
    if !output.status.success() {
        bail!(
            "`docker {}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

async fn start_docker() -> Result<Value> {
    let data = data_dir()?;
    std::fs::create_dir_all(&data)?;

    if container_running() {
        return Ok(
            json!({ "status": "running", "mode": "docker", "url": LOCAL_URL, "alreadyRunning": true }),
        );
    }

    if container_exists() {
        docker(&["start", "meilisearch-local"])?;
    } else {
        docker(&[
            "run",
            "-d",
            "--name",
            "meilisearch-local",
            "-p",
            "7700:7700",
            "-v",
            &format!("{}:/meili_data", data.display()),
            "getmeili/meilisearch:latest",
        ])?;
    }

    // Wait for health
    let client = reqwest::Client::new();
    for _ in 0..30 {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        if let Ok(resp) = client.get("http://127.0.0.1:7700/health").send().await
            && resp.status().is_success()
        {
            return Ok(json!({ "status": "running", "mode": "docker", "url": LOCAL_URL }));
        }
    }
    bail!("Meilisearch started but health check timed out");
}

async fn start_binary() -> Result<Value> {
    let data = data_dir()?;
    std::fs::create_dir_all(&data)?;

    let bin_dir = dirs::home_dir()
        .context("Could not determine home directory")?
        .join(".local/share/msc/bin");
    let binary = bin_dir.join("meilisearch-server");

    if !binary.exists() {
        status!("Downloading Meilisearch binary...");
        download_binary(&binary).await?;
    }

    let pid_file = data.join("meili.pid");
    let log_file = data.join("meilisearch.log");

    let log = std::fs::File::create(&log_file)?;
    let child = std::process::Command::new(&binary)
        .args([
            "--db-path",
            &data.join("data.ms").to_string_lossy(),
            "--http-addr",
            "127.0.0.1:7700",
        ])
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()
        .context("Failed to start Meilisearch binary")?;

    std::fs::write(&pid_file, child.id().to_string())?;

    // Wait for health
    let client = reqwest::Client::new();
    for _ in 0..30 {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        if let Ok(resp) = client.get("http://127.0.0.1:7700/health").send().await
            && resp.status().is_success()
        {
            return Ok(
                json!({ "status": "running", "mode": "binary", "url": LOCAL_URL, "pid": child.id() }),
            );
        }
    }
    bail!("Meilisearch binary started but health check timed out");
}

async fn download_binary(dest: &std::path::Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let os = if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        bail!("Unsupported platform for binary download");
    };

    let arch = if cfg!(target_arch = "x86_64") {
        "amd64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        bail!("Unsupported architecture for binary download");
    };

    let url = format!(
        "https://github.com/meilisearch/meilisearch/releases/latest/download/meilisearch-{}-{}",
        os, arch
    );

    status!("Downloading from {url}...");
    let resp = reqwest::get(&url).await?;
    if !resp.status().is_success() {
        bail!("Download failed: HTTP {}", resp.status());
    }

    let bytes = resp.bytes().await?;
    std::fs::write(dest, &bytes)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dest, std::fs::Permissions::from_mode(0o755))?;
    }

    status!("Downloaded to {}", dest.display());
    Ok(())
}

async fn stop() -> Result<Value> {
    if container_running() {
        docker(&["stop", "meilisearch-local"])?;
        Ok(json!({ "status": "stopped", "mode": "docker" }))
    } else {
        let pid_file = data_dir()?.join("meili.pid");
        if pid_file.exists() {
            let pid = std::fs::read_to_string(&pid_file)?;
            let _ = std::process::Command::new("kill").arg(pid.trim()).status();
            let _ = std::fs::remove_file(&pid_file);
            Ok(json!({ "status": "stopped", "mode": "binary" }))
        } else {
            Ok(json!({ "status": "stopped", "wasRunning": false }))
        }
    }
}

async fn start() -> Result<Value> {
    if docker_available() {
        status!("Starting local Meilisearch with Docker...");
        start_docker().await
    } else {
        status!("Docker not available. Falling back to binary...");
        start_binary().await
    }
}

fn describe(v: &Value) -> String {
    match v["status"].as_str() {
        Some("running") if v["alreadyRunning"] == true => {
            "Local Meilisearch is already running.".to_string()
        }
        Some("running") => format!(
            "✓ Local Meilisearch started via {} ({})",
            v["mode"].as_str().unwrap_or("?"),
            LOCAL_URL
        ),
        Some("stopped") if v["wasRunning"] == false => {
            "Local Meilisearch is not running.".to_string()
        }
        Some("stopped") => "Local Meilisearch stopped.".to_string(),
        _ => v.to_string(),
    }
}

fn dir_size(path: &std::path::Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| match e.metadata() {
            Ok(m) if m.is_dir() => dir_size(&e.path()),
            Ok(m) => m.len(),
            Err(_) => 0,
        })
        .sum()
}

pub async fn run(_cli: &Cli, cmd: &LocalCommand) -> Result<()> {
    match cmd {
        LocalCommand::Start => {
            let v = start().await?;
            emit(&v, || describe(&v));
        }
        LocalCommand::Stop => {
            let v = stop().await?;
            emit(&v, || describe(&v));
        }
        LocalCommand::Restart => {
            stop().await?;
            let v = start().await?;
            emit(&v, || describe(&v));
        }
        LocalCommand::Status => {
            let mut v = if container_running() {
                let output = std::process::Command::new("docker")
                    .args([
                        "inspect",
                        "--format",
                        "{{.State.StartedAt}}",
                        "meilisearch-local",
                    ])
                    .output()?;
                json!({
                    "status": "running",
                    "mode": "docker",
                    "startedAt": String::from_utf8_lossy(&output.stdout).trim(),
                })
            } else if data_dir()?.join("meili.pid").exists() {
                json!({ "status": "running", "mode": "binary" })
            } else {
                json!({ "status": "stopped" })
            };
            if v["status"] == "running" {
                v["url"] = json!(LOCAL_URL);
                if let Ok(resp) = reqwest::get(format!("{LOCAL_URL}/version")).await
                    && let Ok(ver) = resp.json::<Value>().await
                {
                    v["version"] = ver["pkgVersion"].clone();
                }
            }
            emit(&v, || {
                let mut out = String::new();
                if let Some(mode) = v["mode"].as_str() {
                    out.push_str(&format!("Mode: {mode}\n"));
                }
                out.push_str(&format!("Status: {}", v["status"].as_str().unwrap_or("?")));
                if let Some(since) = v["startedAt"].as_str() {
                    out.push_str(&format!(" since {since}"));
                }
                if let Some(ver) = v["version"].as_str() {
                    out.push_str(&format!("\nVersion: {ver}"));
                }
                out
            });
        }
        LocalCommand::Logs { follow } => {
            if container_exists() {
                let mut args = vec!["logs"];
                if *follow {
                    args.push("-f");
                }
                args.push("meilisearch-local");
                let status = std::process::Command::new("docker").args(&args).status()?;
                if !status.success() {
                    bail!("Failed to get logs");
                }
            } else {
                let log_file = data_dir()?.join("meilisearch.log");
                if log_file.exists() {
                    let content = std::fs::read_to_string(&log_file)?;
                    print!("{content}");
                } else {
                    status!("No logs found.");
                }
            }
        }
        LocalCommand::Reset { dry_run } => {
            let data = data_dir()?;
            if *dry_run {
                let v = json!({
                    "dryRun": true,
                    "action": "local.reset",
                    "dataDir": data.display().to_string(),
                    "bytes": dir_size(&data),
                    "removesContainer": container_exists(),
                });
                emit(&v, || {
                    format!(
                        "Would wipe {} ({} bytes) and restart.",
                        data.display(),
                        v["bytes"]
                    )
                });
                return Ok(());
            }
            stop().await?;
            if container_exists() {
                let _ = docker(&["rm", "meilisearch-local"]);
            }
            if data.exists() {
                std::fs::remove_dir_all(&data)?;
            }
            status!("Local data wiped.");
            let mut v = start().await?;
            v["reset"] = json!(true);
            emit(&v, || describe(&v));
        }
        LocalCommand::Upgrade => {
            if docker_available() {
                status!("Pulling latest Meilisearch image...");
                docker(&["pull", "getmeili/meilisearch:latest"])?;
                stop().await?;
                if container_exists() {
                    let _ = docker(&["rm", "meilisearch-local"]);
                }
            } else {
                let bin_dir = dirs::home_dir()
                    .context("Could not determine home directory")?
                    .join(".local/share/msc/bin");
                let binary = bin_dir.join("meilisearch-server");
                stop().await?;
                if binary.exists() {
                    std::fs::remove_file(&binary)?;
                }
                download_binary(&binary).await?;
            }
            let mut v = start().await?;
            v["upgraded"] = json!(true);
            emit(&v, || "✓ Upgraded to latest version.".to_string());
        }
    }
    Ok(())
}
