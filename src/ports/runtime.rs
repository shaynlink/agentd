use async_trait::async_trait;

use crate::domain::process_handle::{ProcessExecutionResult, ProcessHandle};
use crate::domain::resource_limit::ResourceLimit;

#[async_trait]
pub trait RuntimePort: Send + Sync {
    fn name(&self) -> &'static str;

    async fn execute(
        &self,
        command: &str,
        args: &[String],
        timeout_secs: u64,
        cwd: &std::path::Path,
        limit: Option<&ResourceLimit>,
    ) -> anyhow::Result<ProcessExecutionResult>;

    async fn spawn_background(
        &self,
        command: &str,
        args: &[String],
        cwd: &std::path::Path,
    ) -> anyhow::Result<ProcessHandle>;
}
