pub mod cli_provider;
pub mod http_provider;
pub mod mock;
pub mod sandbox_provider;

use anyhow::{Result, bail};

use crate::ports::provider::Provider;

pub fn build_provider(name: &str) -> Result<Box<dyn Provider>> {
    match name {
        "mock" => Ok(Box::new(mock::MockProvider::new())),
        "http" => Ok(Box::new(http_provider::HttpProvider::new())),
        "cli" => Ok(Box::new(cli_provider::CliProvider::new())),
        "vibe" => Ok(Box::new(cli_provider::CliProvider::new_vibe())),
        "sandbox" => Ok(Box::new(sandbox_provider::SandboxProvider::new())),
        other => bail!("unknown provider: {other}. expected one of: mock|http|cli|vibe|sandbox"),
    }
}
