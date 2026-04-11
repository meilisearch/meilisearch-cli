use std::path::PathBuf;

use anyhow::Result;
use clap::Args;

use super::{Cli, build_client, print_json, read_json_input};

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

    /// Interactive TUI mode
    #[arg(short, long)]
    pub interactive: bool,
}

pub async fn run(cli: &Cli, args: &SearchArgs) -> Result<()> {
    if args.interactive {
        let client = build_client(cli)?;
        return crate::tui::search::run_interactive_search(client, &args.uid).await;
    }

    let client = build_client(cli)?;
    let query = args.query.as_deref().unwrap_or("");
    let result = client
        .search(
            &args.uid,
            query,
            args.filter.as_deref(),
            args.facets.as_deref(),
            args.limit,
            args.offset,
            args.sort.as_deref(),
            args.attributes_to_retrieve.as_deref(),
            args.attributes_to_highlight.as_deref(),
        )
        .await?;
    print_json(&result, cli.raw);
    Ok(())
}

// ── Multi-Search ──────────────────────────────────────────────

#[derive(Args)]
pub struct MultiSearchArgs {
    /// JSON file containing the queries array (reads from stdin if omitted)
    #[arg(long)]
    pub file: Option<PathBuf>,
}

pub async fn run_multi_search(cli: &Cli, args: &MultiSearchArgs) -> Result<()> {
    let client = build_client(cli)?;
    let queries = read_json_input(args.file.as_deref())?;
    let result = client.multi_search(&queries).await?;
    print_json(&result, cli.raw);
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
    print_json(&result, cli.raw);
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

    /// Filter expression
    #[arg(long)]
    pub filter: Option<String>,
}

pub async fn run_similar(cli: &Cli, args: &SimilarArgs) -> Result<()> {
    let client = build_client(cli)?;
    let result = client
        .similar(&args.uid, &args.id, args.limit, args.filter.as_deref())
        .await?;
    print_json(&result, cli.raw);
    Ok(())
}
