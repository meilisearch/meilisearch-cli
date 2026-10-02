use std::path::PathBuf;

use anyhow::Result;
use clap::Args;

use serde_json::json;

use super::{Cli, build_client, print_json, read_json_input};
use crate::error::CliError;

#[derive(Args)]
pub struct SearchArgs {
    /// Index UID
    #[arg(value_name = "INDEX_UID")]
    pub uid: String,

    /// Search query
    pub query: Option<String>,

    /// Filter expression
    #[arg(long)]
    pub filter: Option<String>,

    /// Facets to return
    #[arg(long, value_delimiter = ',')]
    pub facets: Option<Vec<String>>,

    /// Maximum number of results
    #[arg(long)]
    pub limit: Option<u64>,

    /// Result offset
    #[arg(long)]
    pub offset: Option<u64>,

    /// Sort rules
    #[arg(long, value_delimiter = ',')]
    pub sort: Option<Vec<String>>,

    /// Attributes to retrieve
    #[arg(long, value_delimiter = ',')]
    pub attributes_to_retrieve: Option<Vec<String>>,

    /// Attributes to highlight
    #[arg(long, value_delimiter = ',')]
    pub attributes_to_highlight: Option<Vec<String>>,

    /// Extra search parameters as a JSON object, merged over the flags
    /// (e.g. '{"hybrid":{"embedder":"default"},"showRankingScore":true}')
    #[arg(long, value_name = "JSON")]
    pub body: Option<String>,

    /// Interactive TUI mode
    #[arg(short, long)]
    pub interactive: bool,
}

/// Build the search request from flags, then overlay `--body`.
fn request_body(args: &SearchArgs) -> Result<serde_json::Value> {
    let mut body = json!({ "q": args.query.as_deref().unwrap_or("") });
    let mut set = |k: &str, v: serde_json::Value| {
        body[k] = v;
    };
    if let Some(f) = &args.filter {
        set("filter", json!(f));
    }
    if let Some(f) = &args.facets {
        set("facets", json!(f));
    }
    if let Some(l) = args.limit {
        set("limit", json!(l));
    }
    if let Some(o) = args.offset {
        set("offset", json!(o));
    }
    if let Some(s) = &args.sort {
        set("sort", json!(s));
    }
    if let Some(a) = &args.attributes_to_retrieve {
        set("attributesToRetrieve", json!(a));
    }
    if let Some(a) = &args.attributes_to_highlight {
        set("attributesToHighlight", json!(a));
    }
    if let Some(raw) = &args.body {
        let extra: serde_json::Value = serde_json::from_str(raw).map_err(|e| {
            CliError::usage("invalid_json", format!("--body is not valid JSON: {e}"))
        })?;
        let Some(extra) = extra.as_object() else {
            return Err(CliError::usage("invalid_json", "--body must be a JSON object").into());
        };
        for (k, v) in extra {
            body[k] = v.clone();
        }
    }
    Ok(body)
}

pub async fn run(cli: &Cli, args: &SearchArgs) -> Result<()> {
    if args.interactive {
        crate::output::require_interactive(
            "Interactive search (-i)",
            "Run `msc search <INDEX_UID> <QUERY>` without -i",
        )?;
        let client = build_client(cli)?;
        return crate::tui::search::run_interactive_search(client, &args.uid).await;
    }

    let client = build_client(cli)?;
    let result = client
        .search_with_body(&args.uid, &request_body(args)?)
        .await?;
    print_json(&result);
    Ok(())
}

// ── Multi-Search ──────────────────────────────────────────────

#[derive(Args)]
pub struct MultiSearchArgs {
    /// JSON file with a queries array or a full body ({"queries": [...], "federation": {...}}); reads stdin if omitted
    #[arg(long)]
    pub file: Option<PathBuf>,
}

pub async fn run_multi_search(cli: &Cli, args: &MultiSearchArgs) -> Result<()> {
    let client = build_client(cli)?;
    let queries = read_json_input(args.file.as_deref())?;
    let result = client.multi_search(&queries).await?;
    print_json(&result);
    Ok(())
}

// ── Facet Search ──────────────────────────────────────────────

#[derive(Args)]
pub struct FacetSearchArgs {
    /// Index UID
    #[arg(value_name = "INDEX_UID")]
    pub uid: String,

    /// Facet name to search
    pub facet_name: String,

    /// Facet query string
    #[arg(long)]
    pub facet_query: Option<String>,

    /// Filter expression
    #[arg(long)]
    pub filter: Option<String>,
}

pub async fn run_facet_search(cli: &Cli, args: &FacetSearchArgs) -> Result<()> {
    let client = build_client(cli)?;
    let result = client
        .facet_search(
            &args.uid,
            &args.facet_name,
            args.facet_query.as_deref(),
            args.filter.as_deref(),
        )
        .await?;
    print_json(&result);
    Ok(())
}

// ── Similar Documents ─────────────────────────────────────────

#[derive(Args)]
pub struct SimilarArgs {
    /// Index UID
    #[arg(value_name = "INDEX_UID")]
    pub uid: String,

    /// Document ID to find similar documents for
    #[arg(value_name = "DOCUMENT_ID")]
    pub id: String,

    /// Maximum number of results
    #[arg(long)]
    pub limit: Option<u64>,

    /// Result offset
    #[arg(long)]
    pub offset: Option<u64>,

    /// Filter expression
    #[arg(long)]
    pub filter: Option<String>,

    /// Embedder to use
    #[arg(long)]
    pub embedder: Option<String>,
}

pub async fn run_similar(cli: &Cli, args: &SimilarArgs) -> Result<()> {
    let client = build_client(cli)?;
    let result = client
        .similar(
            &args.uid,
            &args.id,
            args.limit,
            args.offset,
            args.filter.as_deref(),
            args.embedder.as_deref(),
        )
        .await?;
    print_json(&result);
    Ok(())
}
