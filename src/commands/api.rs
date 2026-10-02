//! `msc api`: call any Meilisearch route with the CLI's auth, errors and
//! `--wait`, for endpoints and parameters that have no dedicated command.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, ValueEnum};

use super::{Cli, build_client, print_json, read_input};
use crate::error::CliError;

#[derive(Clone, Copy, Debug, ValueEnum)]
#[value(rename_all = "UPPER")]
pub enum Method {
    #[value(alias = "get")]
    Get,
    #[value(alias = "post")]
    Post,
    #[value(alias = "put")]
    Put,
    #[value(alias = "patch")]
    Patch,
    #[value(alias = "delete")]
    Delete,
}

impl From<Method> for reqwest::Method {
    fn from(m: Method) -> Self {
        match m {
            Method::Get => reqwest::Method::GET,
            Method::Post => reqwest::Method::POST,
            Method::Put => reqwest::Method::PUT,
            Method::Patch => reqwest::Method::PATCH,
            Method::Delete => reqwest::Method::DELETE,
        }
    }
}

#[derive(Args)]
pub struct ApiArgs {
    /// HTTP method
    #[arg(value_enum, ignore_case = true)]
    pub method: Method,

    /// Route path, e.g. /indexes/movies/settings
    pub path: String,

    /// Inline JSON request body
    #[arg(long, short = 'd', conflicts_with = "file")]
    pub data: Option<String>,

    /// Request body file (reads stdin when piped and no --data is given)
    #[arg(long)]
    pub file: Option<PathBuf>,

    /// Query parameter as KEY=VALUE (repeatable)
    #[arg(long = "param", short = 'p', value_name = "KEY=VALUE")]
    pub params: Vec<String>,

    /// Content-Type of the body (e.g. application/x-ndjson, text/csv)
    #[arg(long, default_value = "application/json")]
    pub content_type: String,
}

fn parse_params(params: &[String]) -> Result<Vec<(String, String)>> {
    params
        .iter()
        .map(|p| {
            p.split_once('=')
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .ok_or_else(|| {
                    CliError::usage(
                        "invalid_param",
                        format!("Invalid --param '{p}': expected KEY=VALUE"),
                    )
                    .into()
                })
        })
        .collect()
}

pub async fn run(cli: &Cli, args: &ApiArgs) -> Result<()> {
    if !args.path.starts_with('/') {
        return Err(CliError::usage(
            "invalid_path",
            format!("Path must start with '/': {}", args.path),
        )
        .with_hint("For example: msc api GET /indexes/movies/settings")
        .into());
    }
    let query = parse_params(&args.params)?;

    let body = if let Some(data) = &args.data {
        Some(data.clone().into_bytes())
    } else if args.file.is_some() {
        Some(read_input(args.file.as_deref())?)
    } else if !matches!(args.method, Method::Get) && stdin_has_data() {
        Some(read_input(None)?).filter(|b| !b.is_empty())
    } else {
        None
    };
    if let Some(bytes) = &body
        && args.content_type == "application/json"
        && let Err(e) = serde_json::from_slice::<serde_json::Value>(bytes)
    {
        return Err(CliError::usage(
            "invalid_json",
            format!("Request body is not valid JSON: {e}"),
        )
        .into());
    }

    let client = build_client(cli)?;
    let result = client
        .request(
            args.method.into(),
            &args.path,
            &query,
            body.map(|b| (b, args.content_type.clone())),
        )
        .await?;
    match &result {
        // Plain-text routes like /metrics print as-is.
        serde_json::Value::String(text) if !crate::output::is_json() => println!("{text}"),
        serde_json::Value::Null => {}
        other => print_json(other),
    }
    Ok(())
}

/// Stdin is a pipe or file (not a terminal), so a body may be waiting there.
fn stdin_has_data() -> bool {
    use std::io::IsTerminal;
    !std::io::stdin().is_terminal()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn params_parse_key_value() {
        let p = parse_params(&["limit=5".into(), "fields=a,b".into()]).unwrap();
        assert_eq!(p[1], ("fields".to_string(), "a,b".to_string()));
        assert!(parse_params(&["nope".into()]).is_err());
    }
}
