use anyhow::Result;

use crate::domain::agent::{AgentLog, AgentRecord, AgentState};
use crate::domain::runtime_audit::{
    RuntimeArtifactInsert, RuntimeArtifactRecord, RuntimeEventInsert, RuntimeEventRecord,
    RuntimeSessionRecord,
};
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
    fn try_acquire_execution_lock(&self, agent_id: &str, owner: &str) -> Result<bool>;
    fn release_execution_lock(&self, agent_id: &str) -> Result<()>;
    fn recover_stuck_executions(&self) -> Result<Vec<String>>;

    fn create_schedule(&self, schedule: &ScheduleRecord) -> Result<()>;
    fn list_schedules(&self, limit: usize) -> Result<Vec<ScheduleRecord>>;
    fn list_due_schedules(&self, now_rfc3339: &str, limit: usize) -> Result<Vec<ScheduleRecord>>;
    fn update_schedule_state(&self, schedule_id: &str, state: ScheduleState) -> Result<()>;
    fn update_schedule_run_at(&self, schedule_id: &str, run_at_rfc3339: &str) -> Result<()>;
    fn append_schedule_run(
        &self,
        schedule_id: &str,
        agent_id: Option<&str>,
        status: &str,
        error: Option<&str>,
    ) -> Result<()>;
    fn get_schedule_runs(&self, schedule_id: &str, limit: usize) -> Result<Vec<ScheduleRun>>;

    fn create_runtime_session(&self, session: &RuntimeSessionRecord) -> Result<()>;
    fn close_runtime_session(&self, session_id: &str) -> Result<()>;
    fn get_runtime_session(&self, session_id: &str) -> Result<Option<RuntimeSessionRecord>>;
    fn append_runtime_event(&self, event: &RuntimeEventInsert) -> Result<()>;
    fn list_runtime_events(&self, session_id: &str, limit: usize)
        -> Result<Vec<RuntimeEventRecord>>;
    fn append_runtime_artifact(&self, artifact: &RuntimeArtifactInsert) -> Result<()>;
    fn list_runtime_artifacts(
        &self,
        session_id: &str,
        limit: usize,
    ) -> Result<Vec<RuntimeArtifactRecord>>;
}
