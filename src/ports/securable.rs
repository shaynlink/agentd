use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default)]
pub struct AuditEventFilters<'a> {
    pub role: Option<&'a str>,
    pub allowed: Option<bool>,
    pub runtime: Option<&'a str>,
    pub agent_id: Option<&'a str>,
    pub since: Option<&'a str>,
    pub until: Option<&'a str>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RbacPolicySpec {
    pub name: String,
    pub resource_type: String,
    pub action: String,
    pub resource_pattern: String,
    pub effect: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RbacRoleRecord {
    pub name: String,
    pub description: Option<String>,
    pub is_builtin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RbacPolicyRecord {
    pub name: String,
    pub resource_type: String,
    pub action: String,
    pub resource_pattern: String,
    pub effect: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RbacBindingRecord {
    pub subject_type: String,
    pub subject: String,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RbacRolePolicyRecord {
    pub role: String,
    pub policy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RbacSnapshot {
    pub roles: Vec<RbacRoleRecord>,
    pub policies: Vec<RbacPolicyRecord>,
    pub bindings: Vec<RbacBindingRecord>,
    pub role_policies: Vec<RbacRolePolicyRecord>,
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
    async fn create_role(&self, name: &str, description: Option<&str>) -> anyhow::Result<()>;
    async fn create_policy(&self, spec: &RbacPolicySpec) -> anyhow::Result<()>;
    async fn bind_role(
        &self,
        subject_type: &str,
        subject: &str,
        role_name: &str,
    ) -> anyhow::Result<()>;
    async fn list_rbac(&self) -> anyhow::Result<RbacSnapshot>;
}
