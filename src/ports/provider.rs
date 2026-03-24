use async_trait::async_trait;

use crate::domain::plan::Plan;

#[derive(Debug, Clone)]
pub struct ProviderRunRequest {
    pub agent_id: String,
    pub prompt: String,
    pub timeout_secs: u64,
    pub stream_output: bool,
    pub json_lines: bool,
    pub runtime_override: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProviderRunResult {
    pub output: String,
}

#[async_trait]
pub trait Provider: Send + Sync {
    fn name(&self) -> &'static str;

    async fn generate_plan(&self, goal: &str) -> anyhow::Result<Plan>;

    async fn run_agent(&self, request: ProviderRunRequest) -> anyhow::Result<ProviderRunResult>;

    async fn cancel(&self, _agent_id: &str) -> anyhow::Result<()> {
        Ok(())
    }
}
