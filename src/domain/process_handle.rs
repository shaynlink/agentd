use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProcessState {
    Running,
    Exited,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessHandle {
    pub pid: u32,
    pub command: String,
    pub args: Vec<String>,
    pub started_at: DateTime<Utc>,
    pub state: ProcessState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessExecutionResult {
    pub output: String,
    pub exit_code: i32,
    pub usage: crate::domain::resource_limit::ResourceUsage,
}
