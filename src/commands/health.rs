use anyhow::Result;

use super::{Cli, build_client, print_json};

pub async fn run(cli: &Cli) -> Result<()> {
    let client = build_client(cli)?;
    let result = client.health().await?;
    print_json(&result, cli.raw);
    Ok(())
}
