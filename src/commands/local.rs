use anyhow::{Context, Result, bail};
use clap::Subcommand;

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
    Reset,
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

async fn start_docker() -> Result<()> {
    let data = data_dir()?;
    std::fs::create_dir_all(&data)?;

    if container_running() {
        println!("Local Meilisearch is already running.");
        return Ok(());
    }

    if container_exists() {
        let status = std::process::Command::new("docker")
            .args(["start", "meilisearch-local"])
            .status()?;
        if !status.success() {
            bail!("Failed to start existing container");
        }
    } else {
        let status = std::process::Command::new("docker")
            .args([
                "run",
                "-d",
                "--name",
                "meilisearch-local",
                "-p",
                "7700:7700",
                "-v",
                &format!("{}:/meili_data", data.display()),
                "getmeili/meilisearch:latest",
            ])
            .status()?;
        if !status.success() {
            bail!("Failed to start Meilisearch container");
        }
    }

    // Wait for health
    let client = reqwest::Client::new();
    for _ in 0..30 {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        if let Ok(resp) = client.get("http://127.0.0.1:7700/health").send().await
            && resp.status().is_success()
        {
            println!("✓ Local Meilisearch started (http://127.0.0.1:7700)");
            return Ok(());
        }
    }
    bail!("Meilisearch started but health check timed out");
}

async fn start_binary() -> Result<()> {
    let data = data_dir()?;
    std::fs::create_dir_all(&data)?;

    let bin_dir = dirs::home_dir()
        .context("Could not determine home directory")?
        .join(".local/share/msc/bin");
    let binary = bin_dir.join("meilisearch-server");

    if !binary.exists() {
        println!("Downloading Meilisearch binary...");
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
            println!("✓ Local Meilisearch started via binary (http://127.0.0.1:7700)");
            return Ok(());
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

    println!("Downloading from {url}...");
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

    println!("Downloaded to {}", dest.display());
    Ok(())
}

async fn stop() -> Result<()> {
    if container_running() {
        let status = std::process::Command::new("docker")
            .args(["stop", "meilisearch-local"])
            .status()?;
        if status.success() {
            println!("Local Meilisearch stopped.");
        } else {
            bail!("Failed to stop container");
        }
    } else {
        let pid_file = data_dir()?.join("meili.pid");
        if pid_file.exists() {
            let pid = std::fs::read_to_string(&pid_file)?;
            let _ = std::process::Command::new("kill").arg(pid.trim()).status();
            let _ = std::fs::remove_file(&pid_file);
            println!("Local Meilisearch stopped.");
        } else {
            println!("Local Meilisearch is not running.");
        }
    }
    Ok(())
}

async fn start() -> Result<()> {
    if docker_available() {
        println!("Starting local Meilisearch with Docker...");
        start_docker().await
    } else {
        println!("Docker not available. Falling back to binary...");
        start_binary().await
    }
}

pub async fn run(cmd: &LocalCommand) -> Result<()> {
    match cmd {
        LocalCommand::Start => start().await,
        LocalCommand::Stop => stop().await,
        LocalCommand::Restart => {
            stop().await?;
            start().await
        }
        LocalCommand::Status => {
            if container_running() {
                let output = std::process::Command::new("docker")
                    .args([
                        "inspect",
                        "--format",
                        "{{.State.Status}} since {{.State.StartedAt}}",
                        "meilisearch-local",
                    ])
                    .output()?;
                let info = String::from_utf8_lossy(&output.stdout);
                println!("Mode: Docker");
                println!("Status: {}", info.trim());

                if let Ok(resp) = reqwest::get("http://127.0.0.1:7700/version").await
                    && let Ok(v) = resp.json::<serde_json::Value>().await
                {
                    println!("Version: {}", v["pkgVersion"].as_str().unwrap_or("unknown"));
                }
            } else {
                let pid_file = data_dir()?.join("meili.pid");
                if pid_file.exists() {
                    println!("Mode: Binary");
                    println!("Status: running (PID file exists)");
                } else {
                    println!("Status: stopped");
                }
            }
            Ok(())
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
                    println!("No logs found.");
                }
            }
            Ok(())
        }
        LocalCommand::Reset => {
            stop().await?;
            if container_exists() {
                let _ = std::process::Command::new("docker")
                    .args(["rm", "meilisearch-local"])
                    .status();
            }
            let data = data_dir()?;
            if data.exists() {
                std::fs::remove_dir_all(&data)?;
            }
            println!("Local data wiped.");
            start().await
        }
        LocalCommand::Upgrade => {
            if docker_available() {
                println!("Pulling latest Meilisearch image...");
                let status = std::process::Command::new("docker")
                    .args(["pull", "getmeili/meilisearch:latest"])
                    .status()?;
                if !status.success() {
                    bail!("Failed to pull latest image");
                }
                stop().await?;
                if container_exists() {
                    let _ = std::process::Command::new("docker")
                        .args(["rm", "meilisearch-local"])
                        .status();
                }
                start().await?;
                println!("✓ Upgraded to latest version.");
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
                start().await?;
                println!("✓ Upgraded to latest version.");
            }
            Ok(())
        }
    }
}
