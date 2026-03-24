use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionBranch {
    pub name: String,
    pub current: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeResult {
    pub target: String,
    pub source: String,
    pub commit: String,
}

pub trait VersioningPort: Send + Sync {
    fn name(&self) -> &'static str;

    fn create_branch(&self, repo_path: &Path, branch: &str, from_ref: Option<&str>)
        -> anyhow::Result<()>;

    fn list_branches(&self, repo_path: &Path) -> anyhow::Result<Vec<VersionBranch>>;

    fn diff(&self, repo_path: &Path, from_ref: &str, to_ref: &str) -> anyhow::Result<String>;

    fn merge(
        &self,
        repo_path: &Path,
        source_branch: &str,
        target_branch: &str,
        no_ff: bool,
        dry_run: bool,
    ) -> anyhow::Result<MergeResult>;

    fn rollback_hard(&self, repo_path: &Path, to_ref: &str) -> anyhow::Result<String>;
}
