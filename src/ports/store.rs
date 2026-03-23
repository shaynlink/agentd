use anyhow::Result;

use crate::domain::agent::{AgentLog, AgentRecord, AgentState};

pub trait StateStore {
    fn init(&self) -> Result<()>;
    fn create_agent(&self, agent: &AgentRecord) -> Result<()>;
    fn update_state(&self, agent_id: &str, state: AgentState) -> Result<()>;
    fn bump_attempts(&self, agent_id: &str) -> Result<()>;
    fn list_agents(&self) -> Result<Vec<AgentRecord>>;
    fn get_agent(&self, agent_id: &str) -> Result<Option<AgentRecord>>;
    fn append_log(&self, agent_id: &str, level: &str, message: &str) -> Result<()>;
    fn get_logs(&self, agent_id: &str, limit: usize) -> Result<Vec<AgentLog>>;
}
