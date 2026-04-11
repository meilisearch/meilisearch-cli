use std::io::Write;

use anyhow::Result;
use clap::Subcommand;

use crate::client::TaskFilters;

use super::{Cli, build_client, print_json};

#[derive(Subcommand)]
pub enum TaskCommand {
    /// List tasks
    List {
        #[arg(long)]
        uids: Option<String>,
        #[arg(long)]
        statuses: Option<String>,
        #[arg(long)]
        types: Option<String>,
        #[arg(long)]
        index_uids: Option<String>,
        #[arg(long)]
        canceled_by: Option<String>,
        #[arg(long)]
        before_enqueued_at: Option<String>,
        #[arg(long)]
        after_enqueued_at: Option<String>,
        #[arg(long)]
        before_started_at: Option<String>,
        #[arg(long)]
        after_started_at: Option<String>,
        #[arg(long)]
        before_finished_at: Option<String>,
        #[arg(long)]
        after_finished_at: Option<String>,
        #[arg(long)]
        from: Option<u64>,
        #[arg(long)]
        limit: Option<u64>,
    },
    /// Get a task by ID
    Get { id: u64 },
    /// Cancel tasks matching filters
    Cancel {
        #[arg(long)]
        uids: Option<String>,
        #[arg(long)]
        statuses: Option<String>,
        #[arg(long)]
        types: Option<String>,
        #[arg(long)]
        index_uids: Option<String>,
        #[arg(long)]
        canceled_by: Option<String>,
        #[arg(long)]
        before_enqueued_at: Option<String>,
        #[arg(long)]
        after_enqueued_at: Option<String>,
        #[arg(long)]
        before_started_at: Option<String>,
        #[arg(long)]
        after_started_at: Option<String>,
        #[arg(long)]
        before_finished_at: Option<String>,
        #[arg(long)]
        after_finished_at: Option<String>,
    },
    /// Delete tasks matching filters
    Delete {
        #[arg(long)]
        uids: Option<String>,
        #[arg(long)]
        statuses: Option<String>,
        #[arg(long)]
        types: Option<String>,
        #[arg(long)]
        index_uids: Option<String>,
        #[arg(long)]
        canceled_by: Option<String>,
        #[arg(long)]
        before_enqueued_at: Option<String>,
        #[arg(long)]
        after_enqueued_at: Option<String>,
        #[arg(long)]
        before_started_at: Option<String>,
        #[arg(long)]
        after_started_at: Option<String>,
        #[arg(long)]
        before_finished_at: Option<String>,
        #[arg(long)]
        after_finished_at: Option<String>,
    },
    /// Wait for a task to complete
    Wait {
        id: u64,
        #[arg(long, default_value = "60000")]
        timeout: u64,
    },
    /// Watch a task until completion
    Watch { id: u64 },
}

pub async fn run(cli: &Cli, cmd: &TaskCommand) -> Result<()> {
    let client = build_client(cli)?;
    match cmd {
        TaskCommand::List {
            uids,
            statuses,
            types,
            index_uids,
            canceled_by,
            before_enqueued_at,
            after_enqueued_at,
            before_started_at,
            after_started_at,
            before_finished_at,
            after_finished_at,
            from,
            limit,
        } => {
            let filters = TaskFilters {
                uids: uids.clone(),
                statuses: statuses.clone(),
                types: types.clone(),
                index_uids: index_uids.clone(),
                canceled_by: canceled_by.clone(),
                before_enqueued_at: before_enqueued_at.clone(),
                after_enqueued_at: after_enqueued_at.clone(),
                before_started_at: before_started_at.clone(),
                after_started_at: after_started_at.clone(),
                before_finished_at: before_finished_at.clone(),
                after_finished_at: after_finished_at.clone(),
                from: *from,
                limit: *limit,
            };
            let result = client.list_tasks(&filters).await?;
            print_json(&result, cli.raw);
        }
        TaskCommand::Get { id } => {
            let result = client.get_task(*id).await?;
            print_json(&result, cli.raw);
        }
        TaskCommand::Cancel {
            uids,
            statuses,
            types,
            index_uids,
            canceled_by,
            before_enqueued_at,
            after_enqueued_at,
            before_started_at,
            after_started_at,
            before_finished_at,
            after_finished_at,
        } => {
            let filters = TaskFilters {
                uids: uids.clone(),
                statuses: statuses.clone(),
                types: types.clone(),
                index_uids: index_uids.clone(),
                canceled_by: canceled_by.clone(),
                before_enqueued_at: before_enqueued_at.clone(),
                after_enqueued_at: after_enqueued_at.clone(),
                before_started_at: before_started_at.clone(),
                after_started_at: after_started_at.clone(),
                before_finished_at: before_finished_at.clone(),
                after_finished_at: after_finished_at.clone(),
                ..Default::default()
            };
            let result = client.cancel_tasks(&filters).await?;
            print_json(&result, cli.raw);
        }
        TaskCommand::Delete {
            uids,
            statuses,
            types,
            index_uids,
            canceled_by,
            before_enqueued_at,
            after_enqueued_at,
            before_started_at,
            after_started_at,
            before_finished_at,
            after_finished_at,
        } => {
            let filters = TaskFilters {
                uids: uids.clone(),
                statuses: statuses.clone(),
                types: types.clone(),
                index_uids: index_uids.clone(),
                canceled_by: canceled_by.clone(),
                before_enqueued_at: before_enqueued_at.clone(),
                after_enqueued_at: after_enqueued_at.clone(),
                before_started_at: before_started_at.clone(),
                after_started_at: after_started_at.clone(),
                before_finished_at: before_finished_at.clone(),
                after_finished_at: after_finished_at.clone(),
                ..Default::default()
            };
            let result = client.delete_tasks(&filters).await?;
            print_json(&result, cli.raw);
        }
        TaskCommand::Wait { id, timeout } => {
            let result = client.wait_for_task(*id, *timeout).await?;
            print_json(&result, cli.raw);
        }
        TaskCommand::Watch { id } => loop {
            let task = client.get_task(*id).await?;
            let status = task["status"].as_str().unwrap_or("unknown");
            let task_type = task["type"].as_str().unwrap_or("unknown");
            print!("\r{} — {} [{}]   ", id, task_type, status);
            std::io::stdout().flush()?;
            match status {
                "succeeded" | "failed" | "canceled" => {
                    println!();
                    print_json(&task, cli.raw);
                    break;
                }
                _ => {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
        },
    }
    Ok(())
}
