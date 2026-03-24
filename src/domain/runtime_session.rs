use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeMode {
    Local,
    #[default]
    Worktree,
    Sandbox,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeSession {
    pub session_id: String,
    pub mode: RuntimeMode,
    pub workspace_dir: PathBuf,
    pub repo_root: Option<PathBuf>,
    pub base_commit: Option<String>,
    pub branch_name: Option<String>,
    pub permissions_profile: String,
    pub env_profile: String,
    pub log_path: PathBuf,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeSessionCreateRequest {
    pub mode: RuntimeMode,
    pub workspace_dir: PathBuf,
    pub repo_root: Option<PathBuf>,
    pub permissions_profile: String,
    pub env_profile: String,
    pub log_path: PathBuf,
}

impl Default for RuntimeSessionCreateRequest {
    fn default() -> Self {
        Self {
            mode: RuntimeMode::Worktree,
            workspace_dir: PathBuf::from("./.agentd/runtime/workspaces/default"),
            repo_root: None,
            permissions_profile: "dev-safe".to_string(),
            env_profile: "default".to_string(),
            log_path: PathBuf::from("./.agentd/runtime/logs/default.jsonl"),
        }
    }
}

impl RuntimeSession {
    pub fn from_request(
        session_id: String,
        created_at: DateTime<Utc>,
        request: RuntimeSessionCreateRequest,
        base_commit: Option<String>,
        branch_name: Option<String>,
    ) -> Self {
        Self {
            session_id,
            mode: request.mode,
            workspace_dir: request.workspace_dir,
            repo_root: request.repo_root,
            base_commit,
            branch_name,
            permissions_profile: request.permissions_profile,
            env_profile: request.env_profile,
            log_path: request.log_path,
            created_at,
        }
    }
}
