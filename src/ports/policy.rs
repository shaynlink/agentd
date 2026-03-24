use std::path::PathBuf;

use async_trait::async_trait;

use crate::domain::capability::{Capability, PolicyDecision};

#[derive(Debug, Clone)]
pub struct RuntimeAction {
    pub capability: Capability,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub target_path: Option<PathBuf>,
}

impl RuntimeAction {
    pub fn for_capability(capability: Capability, cwd: PathBuf) -> Self {
        Self {
            capability,
            command: None,
            args: Vec::new(),
            cwd,
            target_path: None,
        }
    }
}

#[async_trait]
pub trait PolicyPort: Send + Sync {
    fn name(&self) -> &'static str;

    async fn evaluate(
        &self,
        session_id: &str,
        action: &RuntimeAction,
    ) -> anyhow::Result<PolicyDecision>;
}
