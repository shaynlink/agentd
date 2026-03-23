use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};

use crate::domain::agent::{AgentLog, AgentRecord, AgentState};
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
                state: AgentState::from_str(&state).unwrap_or(AgentState::Failed),
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
                state: AgentState::from_str(&state).unwrap_or(AgentState::Failed),
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
}

fn parse_ts(input: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(input)?.with_timezone(&Utc))
}
