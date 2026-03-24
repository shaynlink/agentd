use async_trait::async_trait;

#[derive(Debug, Clone, Copy, Default)]
pub struct AuditEventFilters<'a> {
    pub role: Option<&'a str>,
    pub allowed: Option<bool>,
    pub runtime: Option<&'a str>,
    pub agent_id: Option<&'a str>,
    pub since: Option<&'a str>,
    pub until: Option<&'a str>,
}

#[async_trait]
pub trait SecurablePort: Send + Sync {
    async fn check_command_access(&self, command: &str, role: &str) -> anyhow::Result<bool>;
    async fn check_file_access(&self, path: &std::path::Path, role: &str) -> anyhow::Result<bool>;
    async fn log_audit_event(&self, payload: &str) -> anyhow::Result<()>;
    async fn list_audit_events(
        &self,
        limit: usize,
        filters: AuditEventFilters<'_>,
    ) -> anyhow::Result<Vec<String>>;
}
