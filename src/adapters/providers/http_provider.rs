use async_trait::async_trait;

use crate::domain::plan::Plan;
use crate::ports::provider::{Provider, ProviderRunRequest, ProviderRunResult};

pub struct HttpProvider;

impl HttpProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HttpProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Provider for HttpProvider {
    fn name(&self) -> &'static str {
        "http"
    }

    async fn generate_plan(&self, _goal: &str) -> anyhow::Result<Plan> {
        anyhow::bail!("http provider is a stub in this MVP. configure adapter implementation")
    }

    async fn run_agent(&self, _request: ProviderRunRequest) -> anyhow::Result<ProviderRunResult> {
        anyhow::bail!("http provider is a stub in this MVP. configure adapter implementation")
    }
}
