mod api;
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
mod mcp;
mod network;
mod project;
mod schema;
mod search;
pub mod self_update;
mod settings;
mod skill;
mod task;
mod whoami;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};

use crate::client::MeiliClient;
use crate::config::Config;
use crate::error::CliError;
pub use crate::output::print_json;

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
{tab}  whoami        Show the target server, credentials and auth status
{tab}  key           Manage API keys
{tab}  log           Manage logs
{tab}  network       Manage network configuration
{tab}  experimental  Manage experimental features

{tab}Configuration:
{tab}  project       Manage project credentials
{tab}  local         Manage local Meilisearch instance
{tab}  self-update   Update the CLI to the latest version
{tab}  completions   Generate shell completions

{tab}Agents:
{tab}  schema        Print the command tree as JSON
{tab}  api           Call any API route (escape hatch)
{tab}  mcp           Run as an MCP server over stdio
{tab}  skill         Install the agent skill for Claude Code & co

Output is JSON when stdout is not a terminal (or with --json).
Exit codes: 0 ok, 1 error, 2 usage, 3 auth, 4 not found, 5 API, 6 network,
7 task failed, 8 timeout. Run `msc schema` for the machine-readable spec.

Options:
{options}"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Target project from the config file (overrides default)
    #[arg(long, global = true, env = "MSC_PROJECT")]
    pub project: Option<String>,

    /// Meilisearch URL; bypasses the config file entirely
    #[arg(long, global = true, env = "MSC_URL")]
    pub url: Option<String>,

    /// API key (overrides the project's key)
    #[arg(long, global = true, env = "MSC_API_KEY", hide_env_values = true)]
    pub api_key: Option<String>,

    /// Output compact JSON (default when stdout is not a terminal)
    #[arg(
        long,
        short = 'j',
        global = true,
        visible_alias = "raw",
        short_alias = 'r',
        conflicts_with = "pretty"
    )]
    pub json: bool,

    /// Force human-readable output even when piped
    #[arg(long, global = true)]
    pub pretty: bool,

    /// Render results as a text table (lists become rows)
    #[arg(long, short = 't', global = true, conflicts_with = "json")]
    pub table: bool,

    /// Quiet mode — no status messages on stderr
    #[arg(long, short, global = true)]
    pub quiet: bool,

    /// Keep only these fields in the output (comma-separated dot paths, e.g. uid,error.code).
    /// Applied to each element of `results`/`hits` lists.
    #[arg(long, global = true, value_delimiter = ',', value_name = "PATHS")]
    pub select: Option<Vec<String>>,

    /// Wait for enqueued tasks to finish and print the final task.
    /// Exits with code 7 if the task fails.
    #[arg(long, global = true, env = "MSC_WAIT")]
    pub wait: bool,

    /// Timeout for --wait, in milliseconds
    #[arg(long, global = true, default_value = "300000", value_name = "MS")]
    pub wait_timeout: u64,
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
    /// Show which server and credentials are used, and whether they work
    Whoami,
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

    // ── Agents ───────────────────────────────────────────────
    /// Print the command tree (arguments, types, exit codes) as JSON
    Schema(schema::SchemaArgs),
    /// Call any Meilisearch API route with msc's auth, errors and --wait
    Api(api::ApiArgs),
    /// Run as a Model Context Protocol server over stdio
    Mcp(mcp::McpArgs),
    /// Install or show the agent skills bundled with msc
    Skill {
        #[command(subcommand)]
        cmd: skill::SkillCommand,
    },
}

mod chat;
mod clone;
mod promote;

/// Read raw bytes from a file path, or stdin if no path is given.
/// Refuses to block on an interactive terminal.
pub fn read_input(file: Option<&std::path::Path>) -> Result<Vec<u8>> {
    if let Some(path) = file {
        return std::fs::read(path).map_err(|e| {
            CliError::usage(
                "file_not_readable",
                format!("Failed to read file {}: {e}", path.display()),
            )
            .into()
        });
    }
    use std::io::{IsTerminal, Read};
    if std::io::stdin().is_terminal() {
        return Err(CliError::usage("missing_input", "No input given")
            .with_hint("Pass --file <PATH> or pipe data on stdin")
            .into());
    }
    let mut buf = Vec::new();
    std::io::stdin()
        .read_to_end(&mut buf)
        .context("Failed to read from stdin")?;
    Ok(buf)
}

/// Read JSON from a file path or stdin if no path is given.
pub fn read_json_input(file: Option<&std::path::Path>) -> Result<serde_json::Value> {
    let content = read_input(file)?;
    serde_json::from_slice(&content).map_err(|e| {
        CliError::usage("invalid_json", format!("Failed to parse JSON input: {e}")).into()
    })
}

/// Connection target resolved from flags, environment and config.
pub struct Target {
    pub name: String,
    pub url: String,
    pub api_key: Option<String>,
}

/// Resolve a target. `--url` wins and never touches the config file;
/// otherwise the named (or default) project is used. `--api-key` overrides
/// the project's key in both cases.
pub fn resolve_target(cli: &Cli, project: Option<&str>) -> Result<Target> {
    if project.is_none()
        && let Some(url) = &cli.url
    {
        return Ok(Target {
            name: url.clone(),
            url: url.clone(),
            api_key: cli.api_key.clone(),
        });
    }
    let config = Config::load()?;
    let (name, proj) = config.get_project(project.or(cli.project.as_deref()))?;
    Ok(Target {
        name: name.to_string(),
        url: proj.url.clone(),
        api_key: cli.api_key.clone().or_else(|| proj.api_key.clone()),
    })
}

pub fn client_for(cli: &Cli, target: &Target) -> Result<MeiliClient> {
    Ok(MeiliClient::new(&target.url, target.api_key.as_deref())?
        .with_wait(cli.wait.then_some(cli.wait_timeout)))
}

pub fn build_client(cli: &Cli) -> Result<MeiliClient> {
    client_for(cli, &resolve_target(cli, None)?)
}

/// Standard output for a skipped idempotent operation.
pub fn print_skipped(reason: &str, extra: serde_json::Value) {
    let mut v = serde_json::json!({ "skipped": true, "reason": reason });
    if let (Some(obj), Some(extra)) = (v.as_object_mut(), extra.as_object()) {
        obj.extend(extra.clone());
    }
    print_json(&v);
}

pub async fn run(cli: Cli) -> Result<()> {
    match &cli.command {
        Command::Health => health::run(&cli).await,
        Command::Version => {
            let mut v = serde_json::json!({ "cli": env!("CARGO_PKG_VERSION") });
            if let Ok(client) = build_client(&cli)
                && let Ok(server) = client.version().await
            {
                v["server"] = server;
            }
            // The update check hits GitHub; only do it for humans.
            if !crate::output::is_json()
                && let Ok(Some(latest)) = self_update::check_for_updates().await
            {
                v["latestCli"] = serde_json::json!(latest);
            }
            crate::output::emit(&v, || {
                let mut out = format!("msc (meilisearch-cli) v{}", env!("CARGO_PKG_VERSION"));
                if let Some(server) = v["server"]["pkgVersion"].as_str() {
                    out.push_str(&format!("\nmeilisearch server {server}"));
                }
                if let Some(latest) = v["latestCli"].as_str() {
                    out.push_str(&format!(
                        "\n\nUpdate available: {latest}\nRun `msc self-update` to upgrade."
                    ));
                }
                out
            });
            Ok(())
        }
        Command::Stats => {
            let client = build_client(&cli)?;
            let s = client.stats().await?;
            print_json(&s);
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
        Command::Project { cmd } => project::run(&cli, cmd).await,
        Command::Local { cmd } => local::run(&cli, cmd).await,
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
        Command::Schema(args) => schema::run(args),
        Command::Mcp(args) => mcp::run(&cli, args).await,
        Command::Api(args) => api::run(&cli, args).await,
        Command::Whoami => whoami::run(&cli).await,
        Command::Skill { cmd } => skill::run(cmd),
    }
}
