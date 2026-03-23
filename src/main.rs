use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    agentd::cli::run().await
}
