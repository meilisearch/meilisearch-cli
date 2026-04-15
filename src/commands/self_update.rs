use anyhow::{Context, Result};

const REPO: &str = "meilisearch/meilisearch-cli";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(serde::Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[derive(serde::Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

fn current_target() -> Result<&'static str> {
    // Use compile-time target
    Ok(env!("TARGET"))
}

fn parse_version(v: &str) -> &str {
    v.strip_prefix('v').unwrap_or(v)
}

pub async fn check_for_updates() -> Result<Option<String>> {
    let client = reqwest::Client::builder()
        .user_agent("meilisearch-cli")
        .build()?;

    let release: GithubRelease = client
        .get(format!(
            "https://api.github.com/repos/{REPO}/releases/latest"
        ))
        .send()
        .await?
        .json()
        .await?;

    let latest = parse_version(&release.tag_name);
    if latest != CURRENT_VERSION {
        Ok(Some(release.tag_name))
    } else {
        Ok(None)
    }
}

pub async fn run(force: bool) -> Result<()> {
    let client = reqwest::Client::builder()
        .user_agent("meilisearch-cli")
        .build()?;

    eprintln!("Current version: v{CURRENT_VERSION}");
    eprintln!("Checking for updates...");

    let release: GithubRelease = client
        .get(format!(
            "https://api.github.com/repos/{REPO}/releases/latest"
        ))
        .send()
        .await?
        .json()
        .await?;

    let latest = parse_version(&release.tag_name);
    if latest == CURRENT_VERSION && !force {
        eprintln!("Already up to date.");
        return Ok(());
    }

    eprintln!("New version available: {}", release.tag_name);

    let target = current_target()?;
    let archive_name = format!("msc-{}-{target}.tar.gz", release.tag_name);

    let asset = release
        .assets
        .iter()
        .find(|a| a.name == archive_name)
        .with_context(|| format!("No release artifact for {target}"))?;

    eprintln!("Downloading {archive_name}...");

    let bytes = client
        .get(&asset.browser_download_url)
        .send()
        .await?
        .bytes()
        .await?;

    // Extract binary from tar.gz
    let decoder = flate2::read::GzDecoder::new(&bytes[..]);
    let mut archive = tar::Archive::new(decoder);

    let current_exe = std::env::current_exe().context("Failed to determine current executable")?;
    let tmp_path = current_exe.with_extension("tmp");

    let mut found = false;
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;
        if path.file_name().and_then(|n| n.to_str()) == Some("msc") {
            entry.unpack(&tmp_path)?;
            found = true;
            break;
        }
    }

    if !found {
        anyhow::bail!("Binary not found in archive");
    }

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o755))?;
    }

    // Replace current binary
    let backup_path = current_exe.with_extension("bak");
    if backup_path.exists() {
        std::fs::remove_file(&backup_path).ok();
    }
    std::fs::rename(&current_exe, &backup_path)
        .context("Failed to backup current binary — try running with sudo")?;
    if let Err(e) = std::fs::rename(&tmp_path, &current_exe) {
        // Restore backup on failure
        std::fs::rename(&backup_path, &current_exe).ok();
        return Err(e).context("Failed to install new binary");
    }
    std::fs::remove_file(&backup_path).ok();

    eprintln!("Updated to {}.", release.tag_name);
    Ok(())
}
