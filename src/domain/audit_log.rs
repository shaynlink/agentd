use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandAuditEntry {
    pub ts: DateTime<Utc>,
    pub agent_id: String,
    pub role: String,
    pub runtime: String,
    pub command_input: String,
    pub command_output_preview: String,
    pub allowed: bool,
    pub exit_code: Option<i32>,
}

pub fn output_preview(output: &str, max_len: usize) -> String {
    let trimmed = output.trim();
    if trimmed.len() <= max_len {
        return trimmed.to_string();
    }
    format!("{}...", &trimmed[..max_len])
}
