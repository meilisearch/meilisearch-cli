use anyhow::Result;
use clap::Args;
use futures_util::StreamExt;

use super::Cli;

#[derive(Args)]
pub struct ChatArgs {
    /// Chat message
    pub message: Option<String>,

    /// Workspace UID (defaults to "cloud")
    #[arg(long, default_value = "cloud")]
    pub workspace: String,

    /// Model to use (passed through to LLM provider)
    #[arg(long)]
    pub model: Option<String>,

    /// Interactive TUI mode
    #[arg(short, long)]
    pub interactive: bool,
}

async fn ensure_chat_enabled(client: &crate::client::MeiliClient) -> Result<()> {
    let features = client.get_experimental_features().await?;
    if features["chatCompletions"].as_bool() != Some(true) {
        eprintln!("Enabling chatCompletions experimental feature...");
        client
            .update_experimental_features(&serde_json::json!({ "chatCompletions": true }))
            .await?;
    }
    Ok(())
}

pub async fn run(cli: &Cli, args: &ChatArgs) -> Result<()> {
    let client = super::build_client(cli)?;
    ensure_chat_enabled(&client).await?;

    if args.interactive {
        return crate::tui::chat::run_interactive_chat(
            client,
            &args.workspace,
            args.model.as_deref(),
        )
        .await;
    }

    let message = args.message.as_deref().unwrap_or("Hello");
    let model = args.model.as_deref().unwrap_or("gpt-4o-mini");

    let body = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "user", "content": message }
        ],
        "stream": true
    });

    let resp = client.chat_completions(&args.workspace, &body).await?;
    let status = resp.status();

    if !status.is_success() {
        let text = resp.text().await?;
        anyhow::bail!("Chat API error (HTTP {}): {}", status, text);
    }

    // Stream SSE response
    let mut stream = resp.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(line_end) = buffer.find('\n') {
            let line = buffer[..line_end].trim().to_string();
            buffer = buffer[line_end + 1..].to_string();

            if line.is_empty() || line == "data: [DONE]" {
                continue;
            }

            if let Some(data) = line.strip_prefix("data: ")
                && let Ok(json) = serde_json::from_str::<serde_json::Value>(data)
                && let Some(content) = json["choices"][0]["delta"]["content"].as_str()
            {
                print!("{content}");
            }
        }
    }
    println!();

    Ok(())
}
