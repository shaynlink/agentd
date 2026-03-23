use async_trait::async_trait;
use tokio::time::{Duration, sleep};

use crate::domain::plan::{Plan, PlanStep};
use crate::ports::provider::{Provider, ProviderRunRequest, ProviderRunResult};

pub struct MockProvider;

impl MockProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MockProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Provider for MockProvider {
    fn name(&self) -> &'static str {
        "mock"
    }

    async fn generate_plan(&self, goal: &str) -> anyhow::Result<Plan> {
        Ok(Plan {
            name: format!("plan-for-{goal}"),
            steps: vec![
                PlanStep {
                    id: "step-1".to_string(),
                    name: "analyze".to_string(),
                    prompt: format!("Analyze objective: {goal}"),
                    provider: Some("mock".to_string()),
                    depends_on: Vec::new(),
                    timeout_secs: Some(5),
                    retries: Some(1),
                },
                PlanStep {
                    id: "step-2".to_string(),
                    name: "execute".to_string(),
                    prompt: format!("Execute objective: {goal}"),
                    provider: Some("mock".to_string()),
                    depends_on: vec!["step-1".to_string()],
                    timeout_secs: Some(10),
                    retries: Some(1),
                },
            ],
        })
    }

    async fn run_agent(&self, request: ProviderRunRequest) -> anyhow::Result<ProviderRunResult> {
        let wait_ms = (request.timeout_secs.min(2) * 1000) / 2;
        sleep(Duration::from_millis(wait_ms)).await;
        Ok(ProviderRunResult {
            output: format!(
                "[mock:{}] completed prompt='{}'",
                request.agent_id, request.prompt
            ),
        })
    }
}
