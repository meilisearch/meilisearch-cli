//! `msc whoami`: which server and credentials a command would use, where
//! each came from, and whether they work.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Cli, client_for, resolve_target};
use crate::config::Config;
use crate::error::{ErrorKind, report};

/// Where a setting came from: the command line, the environment, or config.
fn source(flag: &str, env: &str, set: bool) -> Option<String> {
    if !set {
        return None;
    }
    let on_cli = std::env::args().any(|a| a == flag || a.starts_with(&format!("{flag}=")));
    Some(if on_cli {
        format!("flag {flag}")
    } else if std::env::var_os(env).is_some() {
        format!("env {env}")
    } else {
        "config".to_string()
    })
}

pub async fn run(cli: &Cli) -> Result<()> {
    let target = resolve_target(cli, None)?;

    let target_source = source("--url", "MSC_URL", cli.url.is_some())
        .or_else(|| source("--project", "MSC_PROJECT", cli.project.is_some()))
        .unwrap_or_else(|| "config default project".to_string());
    let key_source = source("--api-key", "MSC_API_KEY", cli.api_key.is_some()).or_else(|| {
        target
            .api_key
            .as_ref()
            .map(|_| format!("project '{}'", target.name))
    });

    let mut out = json!({
        "project": if cli.url.is_some() { Value::Null } else { json!(target.name) },
        "url": target.url,
        "source": target_source,
        "apiKey": { "set": target.api_key.is_some(), "source": key_source },
    });
    if cli.url.is_none() {
        out["configFile"] = json!(Config::config_path()?.display().to_string());
    }

    // /health never needs a key; /version needs one when the server has a master key.
    let client = client_for(cli, &target)?.without_wait();
    let mut failure = None;
    match client.health().await {
        Err(e) => {
            out["reachable"] = json!(false);
            failure = Some(e);
        }
        Ok(_) => {
            out["reachable"] = json!(true);
            match client.version().await {
                Ok(v) => {
                    out["auth"] = json!("ok");
                    out["version"] = v["pkgVersion"].clone();
                }
                Err(e) => {
                    let r = report(&e);
                    out["auth"] = json!(if r.error.kind == ErrorKind::Auth {
                        if target.api_key.is_some() {
                            "invalid_key"
                        } else {
                            "key_required"
                        }
                    } else {
                        "unknown"
                    });
                    failure = Some(e);
                }
            }
        }
    }

    crate::output::emit(&out, || {
        let mut s = format!(
            "URL:     {}  ({})",
            target.url,
            out["source"].as_str().unwrap_or("")
        );
        if let Some(p) = out["project"].as_str() {
            s.push_str(&format!("\nProject: {p}"));
        }
        s.push_str(&format!(
            "\nAPI key: {}",
            match out["apiKey"]["source"].as_str() {
                Some(src) => format!("set ({src})"),
                None => "none".to_string(),
            }
        ));
        s.push_str(&format!(
            "\nServer:  {}",
            match (out["reachable"].as_bool(), out["auth"].as_str()) {
                (Some(false), _) => "unreachable".to_string(),
                (_, Some("ok")) =>
                    format!("ok, Meilisearch {}", out["version"].as_str().unwrap_or("?")),
                (_, Some(a)) => format!("reachable, auth: {a}"),
                _ => "?".to_string(),
            }
        ));
        s
    });
    // Exit with the failure's code (6 network, 3 auth, …) after the report.
    match failure {
        Some(e) => Err(e),
        None => Ok(()),
    }
}
