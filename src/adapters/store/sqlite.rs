use std::path::Path;

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};

use crate::domain::agent::{AgentLog, AgentRecord, AgentState};
use crate::domain::schedule::{ScheduleRecord, ScheduleRun, ScheduleState};
use crate::ports::store::StateStore;

pub struct SqliteStore {
    db_path: String,
}

impl SqliteStore {
    pub fn new(db_path: impl Into<String>) -> Self {
        Self {
            db_path: db_path.into(),
        }
    }

    fn open(&self) -> Result<Connection> {
        if let Some(parent) = Path::new(&self.db_path).parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create DB directory: {}", parent.display()))?;
        }
        Connection::open(&self.db_path)
            .with_context(|| format!("failed to open sqlite DB: {}", self.db_path))
    }
}

impl StateStore for SqliteStore {
    fn init(&self) -> Result<()> {
        let conn = self.open()?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS agents (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                provider TEXT NOT NULL,
                prompt TEXT NOT NULL,
                state TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                attempts INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS agent_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                agent_id TEXT NOT NULL,
                ts TEXT NOT NULL,
                level TEXT NOT NULL,
                message TEXT NOT NULL,
                FOREIGN KEY(agent_id) REFERENCES agents(id)
            );

            CREATE INDEX IF NOT EXISTS idx_agent_logs_agent_id_ts
                ON agent_logs(agent_id, ts);

            CREATE TABLE IF NOT EXISTS execution_locks (
                agent_id TEXT PRIMARY KEY,
                owner TEXT NOT NULL,
                locked_at TEXT NOT NULL,
                FOREIGN KEY(agent_id) REFERENCES agents(id)
            );

            CREATE TABLE IF NOT EXISTS schedules (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                provider TEXT NOT NULL,
                prompt TEXT NOT NULL,
                cron_expr TEXT,
                run_at TEXT NOT NULL,
                timeout_secs INTEGER NOT NULL,
                retries INTEGER NOT NULL,
                state TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_schedules_run_at_state
                ON schedules(run_at, state);

            CREATE TABLE IF NOT EXISTS schedule_runs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                schedule_id TEXT NOT NULL,
                agent_id TEXT,
                started_at TEXT NOT NULL,
                finished_at TEXT,
                status TEXT NOT NULL,
                error TEXT,
                FOREIGN KEY(schedule_id) REFERENCES schedules(id)
            );

            CREATE INDEX IF NOT EXISTS idx_schedule_runs_schedule_id
                ON schedule_runs(schedule_id, id DESC);
            "#,
        )
        .context("failed to initialize sqlite schema")?;

        Ok(())
    }

    fn create_agent(&self, agent: &AgentRecord) -> Result<()> {
        let conn = self.open()?;
        conn.execute(
            "INSERT INTO agents (id, name, provider, prompt, state, created_at, updated_at, attempts)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                agent.id,
                agent.name,
                agent.provider,
                agent.prompt,
                agent.state.as_str(),
                agent.created_at.to_rfc3339(),
                agent.updated_at.to_rfc3339(),
                agent.attempts,
            ],
        )
        .context("failed to insert agent")?;
        Ok(())
    }

    fn update_state(&self, agent_id: &str, state: AgentState) -> Result<()> {
        let conn = self.open()?;
        let current: Option<String> = conn
            .query_row(
                "SELECT state FROM agents WHERE id = ?1",
                params![agent_id],
                |row| row.get(0),
            )
            .optional()?;

        let Some(current) = current else {
            bail!("agent not found: {agent_id}");
        };

        let current_state = current.parse().unwrap_or(AgentState::Failed);
        if !current_state.can_transition_to(&state) {
            bail!(
                "invalid state transition for agent {agent_id}: {} -> {}",
                current_state.as_str(),
                state.as_str()
            );
        }

        conn.execute(
            "UPDATE agents SET state = ?1, updated_at = ?2 WHERE id = ?3",
            params![state.as_str(), Utc::now().to_rfc3339(), agent_id],
        )
        .with_context(|| format!("failed to update state for agent: {agent_id}"))?;
        Ok(())
    }

    fn bump_attempts(&self, agent_id: &str) -> Result<()> {
        let conn = self.open()?;
        conn.execute(
            "UPDATE agents SET attempts = attempts + 1, updated_at = ?1 WHERE id = ?2",
            params![Utc::now().to_rfc3339(), agent_id],
        )
        .with_context(|| format!("failed to bump attempts for agent: {agent_id}"))?;
        Ok(())
    }

    fn list_agents(&self) -> Result<Vec<AgentRecord>> {
        let conn = self.open()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, provider, prompt, state, created_at, updated_at, attempts
             FROM agents ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let created_at: String = row.get(5)?;
            let updated_at: String = row.get(6)?;
            let state: String = row.get(4)?;
            Ok(AgentRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                provider: row.get(2)?,
                prompt: row.get(3)?,
                state: state.parse().unwrap_or(AgentState::Failed),
                created_at: parse_ts(&created_at).unwrap_or_else(|_| Utc::now()),
                updated_at: parse_ts(&updated_at).unwrap_or_else(|_| Utc::now()),
                attempts: row.get(7)?,
            })
        })?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    fn get_agent(&self, agent_id: &str) -> Result<Option<AgentRecord>> {
        let conn = self.open()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, provider, prompt, state, created_at, updated_at, attempts
             FROM agents WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![agent_id])?;
        if let Some(row) = rows.next()? {
            let created_at: String = row.get(5)?;
            let updated_at: String = row.get(6)?;
            let state: String = row.get(4)?;
            return Ok(Some(AgentRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                provider: row.get(2)?,
                prompt: row.get(3)?,
                state: state.parse().unwrap_or(AgentState::Failed),
                created_at: parse_ts(&created_at).unwrap_or_else(|_| Utc::now()),
                updated_at: parse_ts(&updated_at).unwrap_or_else(|_| Utc::now()),
                attempts: row.get(7)?,
            }));
        }
        Ok(None)
    }

    fn append_log(&self, agent_id: &str, level: &str, message: &str) -> Result<()> {
        let conn = self.open()?;
        conn.execute(
            "INSERT INTO agent_logs (agent_id, ts, level, message) VALUES (?1, ?2, ?3, ?4)",
            params![agent_id, Utc::now().to_rfc3339(), level, message],
        )
        .with_context(|| format!("failed to append log for agent: {agent_id}"))?;
        Ok(())
    }

    fn get_logs(&self, agent_id: &str, limit: usize) -> Result<Vec<AgentLog>> {
        let conn = self.open()?;
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, ts, level, message
             FROM agent_logs WHERE agent_id = ?1 ORDER BY id DESC LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![agent_id, limit as i64], |row| {
            let ts: String = row.get(2)?;
            Ok(AgentLog {
                id: row.get(0)?,
                agent_id: row.get(1)?,
                ts: parse_ts(&ts).unwrap_or_else(|_| Utc::now()),
                level: row.get(3)?,
                message: row.get(4)?,
            })
        })?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    fn try_acquire_execution_lock(&self, agent_id: &str, owner: &str) -> Result<bool> {
        let conn = self.open()?;
        let changed = conn.execute(
            "INSERT OR IGNORE INTO execution_locks (agent_id, owner, locked_at) VALUES (?1, ?2, ?3)",
            params![agent_id, owner, Utc::now().to_rfc3339()],
        )?;
        Ok(changed > 0)
    }

    fn release_execution_lock(&self, agent_id: &str) -> Result<()> {
        let conn = self.open()?;
        conn.execute(
            "DELETE FROM execution_locks WHERE agent_id = ?1",
            params![agent_id],
        )?;
        Ok(())
    }

    fn recover_stuck_executions(&self) -> Result<Vec<String>> {
        let mut conn = self.open()?;
        let tx = conn.transaction()?;

        let mut recovered_ids = Vec::new();
        {
            let mut stmt = tx.prepare("SELECT id FROM agents WHERE state = 'running'")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            for row in rows {
                recovered_ids.push(row?);
            }
        }

        tx.execute(
            "UPDATE agents SET state = 'pending', updated_at = ?1 WHERE state = 'running'",
            params![Utc::now().to_rfc3339()],
        )?;

        tx.execute("DELETE FROM execution_locks", [])?;
        tx.commit()?;

        Ok(recovered_ids)
    }

    fn create_schedule(&self, schedule: &ScheduleRecord) -> Result<()> {
        let conn = self.open()?;
        conn.execute(
            "INSERT INTO schedules (id, name, provider, prompt, cron_expr, run_at, timeout_secs, retries, state, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                schedule.id,
                schedule.name,
                schedule.provider,
                schedule.prompt,
                schedule.cron_expr,
                schedule.run_at.to_rfc3339(),
                schedule.timeout_secs as i64,
                schedule.retries as i64,
                schedule.state.as_str(),
                schedule.created_at.to_rfc3339(),
                schedule.updated_at.to_rfc3339(),
            ],
        )
        .context("failed to insert schedule")?;
        Ok(())
    }

    fn list_schedules(&self, limit: usize) -> Result<Vec<ScheduleRecord>> {
        let conn = self.open()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, provider, prompt, cron_expr, run_at, timeout_secs, retries, state, created_at, updated_at
             FROM schedules ORDER BY run_at ASC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            let run_at: String = row.get(5)?;
            let state: String = row.get(8)?;
            let created_at: String = row.get(9)?;
            let updated_at: String = row.get(10)?;

            Ok(ScheduleRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                provider: row.get(2)?,
                prompt: row.get(3)?,
                cron_expr: row.get(4)?,
                run_at: parse_ts(&run_at).unwrap_or_else(|_| Utc::now()),
                timeout_secs: row.get::<_, i64>(6)? as u64,
                retries: row.get::<_, i64>(7)? as u32,
                state: state.parse().unwrap_or(ScheduleState::Failed),
                created_at: parse_ts(&created_at).unwrap_or_else(|_| Utc::now()),
                updated_at: parse_ts(&updated_at).unwrap_or_else(|_| Utc::now()),
            })
        })?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    fn list_due_schedules(&self, now_rfc3339: &str, limit: usize) -> Result<Vec<ScheduleRecord>> {
        let conn = self.open()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, provider, prompt, cron_expr, run_at, timeout_secs, retries, state, created_at, updated_at
             FROM schedules
             WHERE state = 'scheduled' AND run_at <= ?1
             ORDER BY run_at ASC
             LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![now_rfc3339, limit as i64], |row| {
            let run_at: String = row.get(5)?;
            let state: String = row.get(8)?;
            let created_at: String = row.get(9)?;
            let updated_at: String = row.get(10)?;

            Ok(ScheduleRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                provider: row.get(2)?,
                prompt: row.get(3)?,
                cron_expr: row.get(4)?,
                run_at: parse_ts(&run_at).unwrap_or_else(|_| Utc::now()),
                timeout_secs: row.get::<_, i64>(6)? as u64,
                retries: row.get::<_, i64>(7)? as u32,
                state: state.parse().unwrap_or(ScheduleState::Failed),
                created_at: parse_ts(&created_at).unwrap_or_else(|_| Utc::now()),
                updated_at: parse_ts(&updated_at).unwrap_or_else(|_| Utc::now()),
            })
        })?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    fn update_schedule_state(&self, schedule_id: &str, state: ScheduleState) -> Result<()> {
        let conn = self.open()?;
        conn.execute(
            "UPDATE schedules SET state = ?1, updated_at = ?2 WHERE id = ?3",
            params![state.as_str(), Utc::now().to_rfc3339(), schedule_id],
        )
        .with_context(|| format!("failed to update schedule state: {schedule_id}"))?;
        Ok(())
    }

    fn update_schedule_run_at(&self, schedule_id: &str, run_at_rfc3339: &str) -> Result<()> {
        let conn = self.open()?;
        conn.execute(
            "UPDATE schedules SET run_at = ?1, updated_at = ?2 WHERE id = ?3",
            params![run_at_rfc3339, Utc::now().to_rfc3339(), schedule_id],
        )
        .with_context(|| format!("failed to update schedule run_at: {schedule_id}"))?;
        Ok(())
    }

    fn append_schedule_run(
        &self,
        schedule_id: &str,
        agent_id: Option<&str>,
        status: &str,
        error: Option<&str>,
    ) -> Result<()> {
        let conn = self.open()?;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO schedule_runs (schedule_id, agent_id, started_at, finished_at, status, error)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![schedule_id, agent_id, now, now, status, error],
        )
        .with_context(|| format!("failed to append schedule run: {schedule_id}"))?;
        Ok(())
    }

    fn get_schedule_runs(&self, schedule_id: &str, limit: usize) -> Result<Vec<ScheduleRun>> {
        let conn = self.open()?;
        let mut stmt = conn.prepare(
            "SELECT id, schedule_id, agent_id, started_at, finished_at, status, error
             FROM schedule_runs
             WHERE schedule_id = ?1
             ORDER BY id DESC
             LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![schedule_id, limit as i64], |row| {
            let started_at: String = row.get(3)?;
            let finished_at: Option<String> = row.get(4)?;
            Ok(ScheduleRun {
                id: row.get(0)?,
                schedule_id: row.get(1)?,
                agent_id: row.get(2)?,
                started_at: parse_ts(&started_at).unwrap_or_else(|_| Utc::now()),
                finished_at: finished_at.as_deref().and_then(|ts| parse_ts(ts).ok()),
                status: row.get(5)?,
                error: row.get(6)?,
            })
        })?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }
}

fn parse_ts(input: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(input)?.with_timezone(&Utc))
}
