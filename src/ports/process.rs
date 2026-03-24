use async_trait::async_trait;

use crate::domain::process_handle::ProcessHandle;

#[async_trait]
pub trait ProcessPort: Send + Sync {
    async fn spawn_process(&self, command: &str, args: &[String]) -> anyhow::Result<ProcessHandle>;
    async fn kill_process(&self, pid: u32) -> anyhow::Result<()>;
}
