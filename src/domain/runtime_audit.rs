use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeSessionRecord {
    pub session_id: String,
    pub mode: String,
    pub workspace_dir: String,
    pub repo_root: Option<String>,
    pub base_commit: Option<String>,
    pub branch_name: Option<String>,
    pub permissions_profile: String,
    pub env_profile: String,
    pub log_path: String,
    pub created_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeEventInsert {
    pub ts: DateTime<Utc>,
    pub session_id: String,
    pub event_type: String,
    pub command: Option<String>,
    pub cwd: Option<String>,
    pub exit_code: Option<i64>,
    pub payload: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeEventRecord {
    pub id: i64,
    pub ts: DateTime<Utc>,
    pub session_id: String,
    pub event_type: String,
    pub command: Option<String>,
    pub cwd: Option<String>,
    pub exit_code: Option<i64>,
    pub payload: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeArtifactInsert {
    pub ts: DateTime<Utc>,
    pub session_id: String,
    pub artifact_type: String,
    pub path: String,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeArtifactRecord {
    pub id: i64,
    pub ts: DateTime<Utc>,
    pub session_id: String,
    pub artifact_type: String,
    pub path: String,
    pub metadata: Option<String>,
}
