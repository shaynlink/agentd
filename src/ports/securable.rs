use async_trait::async_trait;

#[async_trait]
pub trait SecurablePort: Send + Sync {
    async fn check_command_access(&self, command: &str, role: &str) -> anyhow::Result<bool>;
    async fn check_file_access(&self, path: &std::path::Path, role: &str) -> anyhow::Result<bool>;
    async fn log_audit_event(&self, payload: &str) -> anyhow::Result<()>;
    async fn list_audit_events(&self, limit: usize) -> anyhow::Result<Vec<String>>;
}
