use async_trait::async_trait;

use crate::domain::resource_limit::{ResourceLimit, ResourceUsage};

#[async_trait]
pub trait ResourcePort: Send + Sync {
    async fn get_usage(&self, pid: u32) -> anyhow::Result<ResourceUsage>;
    async fn enforce_limits(&self, pid: u32, limit: &ResourceLimit) -> anyhow::Result<()>;
}
