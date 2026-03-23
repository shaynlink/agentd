use anyhow::Result;

use crate::domain::agent::{AgentLog, AgentRecord, AgentState};
use crate::domain::schedule::{ScheduleRecord, ScheduleRun, ScheduleState};

pub trait StateStore {
    fn init(&self) -> Result<()>;
    fn create_agent(&self, agent: &AgentRecord) -> Result<()>;
    fn update_state(&self, agent_id: &str, state: AgentState) -> Result<()>;
    fn bump_attempts(&self, agent_id: &str) -> Result<()>;
    fn list_agents(&self) -> Result<Vec<AgentRecord>>;
    fn get_agent(&self, agent_id: &str) -> Result<Option<AgentRecord>>;
    fn append_log(&self, agent_id: &str, level: &str, message: &str) -> Result<()>;
    fn get_logs(&self, agent_id: &str, limit: usize) -> Result<Vec<AgentLog>>;

    fn create_schedule(&self, schedule: &ScheduleRecord) -> Result<()>;
    fn list_schedules(&self, limit: usize) -> Result<Vec<ScheduleRecord>>;
    fn list_due_schedules(&self, now_rfc3339: &str, limit: usize) -> Result<Vec<ScheduleRecord>>;
    fn update_schedule_state(&self, schedule_id: &str, state: ScheduleState) -> Result<()>;
    fn append_schedule_run(
        &self,
        schedule_id: &str,
        agent_id: Option<&str>,
        status: &str,
        error: Option<&str>,
    ) -> Result<()>;
    fn get_schedule_runs(&self, schedule_id: &str, limit: usize) -> Result<Vec<ScheduleRun>>;
}
