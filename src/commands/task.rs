use std::io::Write;

use anyhow::Result;
use clap::{Args, Subcommand};
use serde_json::json;

use crate::client::{MeiliClient, TaskFilters, ensure_succeeded, is_terminal_status};
use crate::output::{event, is_json};

use super::{Cli, build_client, print_json};

/// Filters shared by `task list`, `task cancel` and `task delete`.
#[derive(Args, Clone)]
pub struct TaskFilterArgs {
    /// Comma-separated task UIDs
    #[arg(long)]
    pub uids: Option<String>,
    /// Comma-separated statuses: enqueued, processing, succeeded, failed, canceled
    #[arg(long)]
    pub statuses: Option<String>,
    /// Comma-separated task types (e.g. documentAdditionOrUpdate, indexCreation, settingsUpdate)
    #[arg(long)]
    pub types: Option<String>,
    /// Comma-separated index UIDs
    #[arg(long)]
    pub index_uids: Option<String>,
    /// Comma-separated UIDs of the canceling tasks
    #[arg(long)]
    pub canceled_by: Option<String>,
    /// Enqueued before this RFC 3339 date
    #[arg(long)]
    pub before_enqueued_at: Option<String>,
    /// Enqueued after this RFC 3339 date
    #[arg(long)]
    pub after_enqueued_at: Option<String>,
    /// Started before this RFC 3339 date
    #[arg(long)]
    pub before_started_at: Option<String>,
    /// Started after this RFC 3339 date
    #[arg(long)]
    pub after_started_at: Option<String>,
    /// Finished before this RFC 3339 date
    #[arg(long)]
    pub before_finished_at: Option<String>,
    /// Finished after this RFC 3339 date
    #[arg(long)]
    pub after_finished_at: Option<String>,
}

impl TaskFilterArgs {
    fn to_filters(&self) -> TaskFilters {
        TaskFilters {
            uids: self.uids.clone(),
            statuses: self.statuses.clone(),
            types: self.types.clone(),
            index_uids: self.index_uids.clone(),
            canceled_by: self.canceled_by.clone(),
            before_enqueued_at: self.before_enqueued_at.clone(),
            after_enqueued_at: self.after_enqueued_at.clone(),
            before_started_at: self.before_started_at.clone(),
            after_started_at: self.after_started_at.clone(),
            before_finished_at: self.before_finished_at.clone(),
            after_finished_at: self.after_finished_at.clone(),
            ..Default::default()
        }
    }
}

#[derive(Subcommand)]
pub enum TaskCommand {
    /// List tasks
    List {
        #[command(flatten)]
        filters: TaskFilterArgs,
        /// Start listing from this task UID (pagination cursor)
        #[arg(long)]
        from: Option<u64>,
        /// Maximum number of tasks to return
        #[arg(long)]
        limit: Option<u64>,
    },
    /// Get a task by UID
    Get {
        /// Task UID
        id: u64,
    },
    /// Cancel tasks matching filters
    Cancel {
        /// Show the matching tasks without canceling them
        #[arg(long)]
        dry_run: bool,
        #[command(flatten)]
        filters: TaskFilterArgs,
    },
    /// Delete tasks matching filters
    Delete {
        /// Show the matching tasks without deleting them
        #[arg(long)]
        dry_run: bool,
        #[command(flatten)]
        filters: TaskFilterArgs,
    },
    /// Wait for a task to finish; exits 7 if it failed or was canceled
    Wait {
        /// Task UID
        id: u64,
        /// Timeout in milliseconds (defaults to --wait-timeout)
        #[arg(long)]
        timeout: Option<u64>,
    },
    /// Watch a task until completion (NDJSON status events in JSON mode)
    Watch {
        /// Task UID
        id: u64,
    },
}

pub async fn run(cli: &Cli, cmd: &TaskCommand) -> Result<()> {
    let client = build_client(cli)?;
    match cmd {
        TaskCommand::List {
            filters,
            from,
            limit,
        } => {
            let filters = TaskFilters {
                from: *from,
                limit: *limit,
                ..filters.to_filters()
            };
            let result = client.list_tasks(&filters).await?;
            print_json(&result);
        }
        TaskCommand::Get { id } => {
            let result = client.get_task(*id).await?;
            print_json(&result);
        }
        TaskCommand::Cancel { dry_run, filters } => {
            let filters = filters.to_filters();
            if *dry_run {
                return preview(&client, filters, "tasks.cancel").await;
            }
            let result = client.cancel_tasks(&filters).await?;
            print_json(&result);
        }
        TaskCommand::Delete { dry_run, filters } => {
            let filters = filters.to_filters();
            if *dry_run {
                return preview(&client, filters, "tasks.delete").await;
            }
            let result = client.delete_tasks(&filters).await?;
            print_json(&result);
        }
        TaskCommand::Wait { id, timeout } => {
            let task = client
                .wait_for_task(*id, timeout.unwrap_or(cli.wait_timeout))
                .await?;
            print_json(&ensure_succeeded(task)?);
        }
        TaskCommand::Watch { id } => {
            let mut last_status = String::new();
            loop {
                let task = client.get_task(*id).await?;
                let status = task["status"].as_str().unwrap_or("unknown").to_string();
                let task_type = task["type"].as_str().unwrap_or("unknown");
                let done = is_terminal_status(&status);
                if is_json() {
                    if done {
                        event(&json!({ "event": "done", "task": task }));
                    } else if status != last_status {
                        event(
                            &json!({ "event": "status", "uid": id, "type": task_type, "status": status }),
                        );
                    }
                } else if !crate::output::ctx().quiet {
                    eprint!("\r{} — {} [{}]   ", id, task_type, status);
                    std::io::stderr().flush()?;
                }
                if done {
                    if !is_json() {
                        eprintln!();
                        print_json(&task);
                    }
                    // Exit code reflects the outcome; the task is already printed.
                    if status != "succeeded" {
                        let mut err = crate::error::CliError::task_failed(task);
                        err.task = None;
                        return Err(err.into());
                    }
                    break;
                }
                last_status = status;
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
    }
    Ok(())
}

/// Dry run for cancel/delete: report what the filters match.
async fn preview(client: &MeiliClient, mut filters: TaskFilters, action: &str) -> Result<()> {
    if action == "tasks.cancel" && filters.statuses.is_none() {
        // Only enqueued and processing tasks can be canceled.
        filters.statuses = Some("enqueued,processing".to_string());
    }
    filters.limit = Some(20);
    let matched = client.list_tasks(&filters).await?;
    let sample: Vec<_> = matched["results"]
        .as_array()
        .map(|r| r.iter().map(|t| t["uid"].clone()).collect())
        .unwrap_or_default();
    print_json(&json!({
        "dryRun": true,
        "action": action,
        "matchedTasks": matched["total"],
        "sampleUids": sample,
    }));
    Ok(())
}
