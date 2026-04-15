mod batch;
mod document;
mod dump;
mod experimental;
mod health;
mod import;
mod index;
mod key;
mod local;
mod log;
mod network;
mod project;
mod search;
pub mod self_update;
mod settings;
mod task;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};

use crate::client::MeiliClient;
use crate::config::Config;

#[derive(Parser)]
#[command(
    name = "msc",
    about = "The official Meilisearch CLI",
    version,
    after_help = "",
    override_usage = "msc [OPTIONS] <COMMAND>",
    help_template = "\
{about}

{usage-heading} {usage}

{tab}Data:
{tab}  index         Manage indexes
{tab}  document      Manage documents
{tab}  settings      Manage settings
{tab}  import        Import documents from file

{tab}Search:
{tab}  search        Search an index
{tab}  multi-search  Search across multiple indexes
{tab}  facet-search  Perform a facet search
{tab}  similar       Find similar documents
{tab}  chat          Interactive chat with your data

{tab}Operations:
{tab}  clone         Clone an index
{tab}  promote       Promote indexes between projects
{tab}  dump          Create a dump
{tab}  task          Manage tasks
{tab}  batch         Manage batches

{tab}Server:
{tab}  health        Check server health
{tab}  version       Show server version
{tab}  stats         Show server stats
{tab}  metrics       Show Prometheus metrics
{tab}  key           Manage API keys
{tab}  log           Manage logs
{tab}  network       Manage network configuration
{tab}  experimental  Manage experimental features

{tab}Configuration:
{tab}  project       Manage project credentials
{tab}  local         Manage local Meilisearch instance
{tab}  self-update   Update the CLI to the latest version
{tab}  completions   Generate shell completions

Options:
{options}"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Target project (overrides default)
    #[arg(long, global = true)]
    pub project: Option<String>,

    /// Output raw JSON (no formatting)
    #[arg(long, short, global = true)]
    pub raw: bool,

    /// Output as table
    #[arg(long, short = 't', global = true)]
    pub table: bool,

    /// Quiet mode — errors only
    #[arg(long, short, global = true)]
    pub quiet: bool,
}

#[derive(Subcommand)]
pub enum Command {
    // ── Data ─────────────────────────────────────────────────
    /// Manage indexes
    Index {
        #[command(subcommand)]
        cmd: index::IndexCommand,
    },
    /// Manage documents
    Document {
        #[command(subcommand)]
        cmd: document::DocumentCommand,
    },
    /// Manage settings
    Settings {
        #[command(subcommand)]
        cmd: settings::SettingsCommand,
    },
    /// Import documents from file
    Import(import::ImportArgs),

    // ── Search ───────────────────────────────────────────────
    /// Search an index
    Search(search::SearchArgs),
    /// Search across multiple indexes
    MultiSearch(search::MultiSearchArgs),
    /// Perform a facet search
    FacetSearch(search::FacetSearchArgs),
    /// Find similar documents
    Similar(search::SimilarArgs),
    /// Interactive chat with your data
    Chat(chat::ChatArgs),

    // ── Operations ───────────────────────────────────────────
    /// Clone an index
    Clone(clone::CloneArgs),
    /// Promote indexes from one project to another
    Promote(promote::PromoteArgs),
    /// Create a dump
    Dump {
        #[command(subcommand)]
        cmd: dump::DumpCommand,
    },
    /// Manage tasks
    Task {
        #[command(subcommand)]
        cmd: task::TaskCommand,
    },
    /// Manage batches
    Batch {
        #[command(subcommand)]
        cmd: batch::BatchCommand,
    },

    // ── Server ───────────────────────────────────────────────
    /// Check server health
    Health,
    /// Show server version
    Version,
    /// Show server stats
    Stats,
    /// Show Prometheus metrics
    Metrics,
    /// Manage API keys
    Key {
        #[command(subcommand)]
        cmd: key::KeyCommand,
    },
    /// Manage logs
    Log {
        #[command(subcommand)]
        cmd: log::LogCommand,
    },
    /// Manage network configuration
    Network {
        #[command(subcommand)]
        cmd: network::NetworkCommand,
    },
    /// Manage experimental features
    Experimental {
        #[command(subcommand)]
        cmd: experimental::ExperimentalCommand,
    },

    // ── Configuration ────────────────────────────────────────
    /// Manage project credentials
    Project {
        #[command(subcommand)]
        cmd: project::ProjectCommand,
    },
    /// Manage local Meilisearch instance
    Local {
        #[command(subcommand)]
        cmd: local::LocalCommand,
    },
    /// Update the CLI to the latest version
    #[command(name = "self-update")]
    SelfUpdate {
        /// Force re-download even if already up to date
        #[arg(long)]
        force: bool,
    },
    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

mod chat;
mod clone;
mod promote;

/// Read JSON from a file path or stdin if no path is given.
pub fn read_json_input(file: Option<&std::path::Path>) -> Result<serde_json::Value> {
    let content = if let Some(path) = file {
        std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path.display()))?
    } else {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .context("Failed to read from stdin")?;
        buf
    };
    serde_json::from_str(&content).context("Failed to parse JSON input")
}

pub fn build_client(cli: &Cli) -> Result<MeiliClient> {
    let config = Config::load()?;
    let (_, proj) = config.get_project(cli.project.as_deref())?;
    MeiliClient::new(&proj.url, proj.api_key.as_deref())
}

pub fn print_json(value: &serde_json::Value, raw: bool) {
    if raw {
        println!("{}", serde_json::to_string(value).unwrap_or_default());
    } else {
        let formatted = colored_json::to_colored_json_auto(value)
            .unwrap_or_else(|_| serde_json::to_string_pretty(value).unwrap_or_default());
        println!("{formatted}");
    }
}

pub async fn run(cli: Cli) -> Result<()> {
    match &cli.command {
        Command::Health => health::run(&cli).await,
        Command::Version => {
            eprintln!("msc (meilisearch-cli) v{}", env!("CARGO_PKG_VERSION"));
            if let Ok(client) = build_client(&cli)
                && let Ok(v) = client.version().await
            {
                eprintln!(
                    "meilisearch server {}",
                    v["pkgVersion"].as_str().unwrap_or("unknown")
                );
            }
            if let Ok(Some(latest)) = self_update::check_for_updates().await {
                eprintln!("\nUpdate available: {latest}");
                eprintln!("Run `msc self-update` to upgrade.");
            }
            Ok(())
        }
        Command::Stats => {
            let client = build_client(&cli)?;
            let s = client.stats().await?;
            print_json(&s, cli.raw);
            Ok(())
        }
        Command::Metrics => {
            let client = build_client(&cli)?;
            let m = client.get_metrics_raw().await?;
            println!("{m}");
            Ok(())
        }
        Command::Index { cmd } => index::run(&cli, cmd).await,
        Command::Document { cmd } => document::run(&cli, cmd).await,
        Command::Search(args) => search::run(&cli, args).await,
        Command::MultiSearch(args) => search::run_multi_search(&cli, args).await,
        Command::FacetSearch(args) => search::run_facet_search(&cli, args).await,
        Command::Similar(args) => search::run_similar(&cli, args).await,
        Command::Settings { cmd } => settings::run(&cli, cmd).await,
        Command::Task { cmd } => task::run(&cli, cmd).await,
        Command::Batch { cmd } => batch::run(&cli, cmd).await,
        Command::Key { cmd } => key::run(&cli, cmd).await,
        Command::Project { cmd } => project::run(cmd).await,
        Command::Local { cmd } => local::run(cmd).await,
        Command::Import(args) => import::run(&cli, args).await,
        Command::Clone(args) => clone::run(&cli, args).await,
        Command::Promote(args) => promote::run(&cli, args).await,
        Command::Dump { cmd } => dump::run(&cli, cmd).await,
        Command::Log { cmd } => log::run(&cli, cmd).await,
        Command::Network { cmd } => network::run(&cli, cmd).await,
        Command::Experimental { cmd } => experimental::run(&cli, cmd).await,
        Command::Chat(args) => chat::run(&cli, args).await,
        Command::SelfUpdate { force } => self_update::run(*force).await,
        Command::Completions { shell } => {
            clap_complete::generate(*shell, &mut Cli::command(), "msc", &mut std::io::stdout());
            Ok(())
        }
    }
}
