use async_trait::async_trait;

use crate::domain::plan::Plan;
use crate::ports::provider::{Provider, ProviderRunRequest, ProviderRunResult};

pub struct CliProvider;

impl CliProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CliProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Provider for CliProvider {
    fn name(&self) -> &'static str {
        "cli"
    }

    async fn generate_plan(&self, _goal: &str) -> anyhow::Result<Plan> {
        anyhow::bail!(
            "cli provider is a stub in this MVP. wire a concrete CLI adapter (vibe, etc.)"
        )
    }

    async fn run_agent(&self, _request: ProviderRunRequest) -> anyhow::Result<ProviderRunResult> {
        anyhow::bail!("cli provider is a stub in this MVP. wire spawn/stdout protocol")
    }
}
