use std::path::{Path, PathBuf};
use std::time::Instant;
use std::{fs, io::Write};

use anyhow::{Result, bail};
use chrono::Utc;
use serde_json::json;

use crate::adapters::store::sqlite::SqliteStore;
use crate::domain::capability::Capability;
use crate::domain::process_handle::ProcessExecutionResult;
use crate::domain::runtime_audit::{RuntimeArtifactInsert, RuntimeEventInsert, RuntimeSessionRecord};
use crate::ports::policy::{PolicyPort, RuntimeAction};
use crate::ports::runtime::RuntimePort;
use crate::ports::store::StateStore;
use crate::ports::workspace_guard::WorkspaceGuardPort;

pub struct RuntimeExecutor {
    policy: Box<dyn PolicyPort>,
    workspace_guard: Box<dyn WorkspaceGuardPort>,
    runtime: Box<dyn RuntimePort>,
    event_log_path: Option<PathBuf>,
    event_db_path: Option<PathBuf>,
}

impl RuntimeExecutor {
    pub fn new(
        policy: Box<dyn PolicyPort>,
        workspace_guard: Box<dyn WorkspaceGuardPort>,
        runtime: Box<dyn RuntimePort>,
    ) -> Self {
        Self {
            policy,
            workspace_guard,
            runtime,
            event_log_path: None,
            event_db_path: None,
        }
    }

    pub fn with_event_log_path(mut self, event_log_path: PathBuf) -> Self {
        self.event_log_path = Some(event_log_path);
        self
    }

    pub fn with_event_db_path(mut self, event_db_path: PathBuf) -> Self {
        self.event_db_path = Some(event_db_path);
        self
    }

    fn append_runtime_event_jsonl(&self, event: &serde_json::Value) -> Result<()> {
        let Some(path) = self.event_log_path.as_ref() else {
            return Ok(());
        };

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut file = fs::OpenOptions::new().create(true).append(true).open(path)?;
        writeln!(file, "{}", event)?;
        Ok(())
    }

    fn ensure_runtime_session(
        &self,
        store: &SqliteStore,
        session_id: &str,
        cwd: &str,
    ) -> Result<bool> {
        if store.get_runtime_session(session_id)?.is_some() {
            return Ok(false);
        }

        let session = RuntimeSessionRecord {
            session_id: session_id.to_string(),
            mode: "sandbox".to_string(),
            workspace_dir: cwd.to_string(),
            repo_root: None,
            base_commit: None,
            branch_name: None,
            permissions_profile: "sandbox".to_string(),
            env_profile: "default".to_string(),
            log_path: self
                .event_log_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
            created_at: Utc::now(),
            closed_at: None,
        };

        store.create_runtime_session(&session)?;
        Ok(true)
    }

    fn append_runtime_event_sqlite(&self, event: &serde_json::Value) -> Result<()> {
        let Some(path) = self.event_db_path.as_ref() else {
            return Ok(());
        };

        let store = SqliteStore::new(path.to_string_lossy().to_string());
        store.init()?;

        let ts = event
            .get("ts")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let session_id = event
            .get("session_id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let event_type = event
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let command = event
            .get("command")
            .and_then(|v| v.as_str())
            .map(|v| v.to_string());
        let cwd = event
            .get("cwd")
            .and_then(|v| v.as_str())
            .map(|v| v.to_string());
        let exit_code = event.get("exit_code").and_then(|v| v.as_i64());
        let payload = event.to_string();

        let parsed_ts = chrono::DateTime::parse_from_rfc3339(&ts)
            .map(|v| v.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        let created = self.ensure_runtime_session(
            &store,
            &session_id,
            cwd.as_deref().unwrap_or_default(),
        )?;

        if created
            && let Some(log_path) = self.event_log_path.as_ref()
        {
            store.append_runtime_artifact(&RuntimeArtifactInsert {
                ts: Utc::now(),
                session_id: session_id.clone(),
                artifact_type: "runtime_event_log".to_string(),
                path: log_path.display().to_string(),
                metadata: Some("{\"format\":\"jsonl\"}".to_string()),
            })?;
        }

        store.append_runtime_event(&RuntimeEventInsert {
            ts: parsed_ts,
            session_id,
            event_type,
            command,
            cwd,
            exit_code,
            payload,
        })?;

        Ok(())
    }

    fn append_runtime_event(&self, event: serde_json::Value) -> Result<()> {
        self.append_runtime_event_jsonl(&event)?;
        self.append_runtime_event_sqlite(&event)?;
        Ok(())
    }

    pub async fn execute_command(
        &self,
        session_id: &str,
        command: &str,
        args: &[String],
        timeout_secs: u64,
        cwd: &Path,
    ) -> Result<ProcessExecutionResult> {
        let started = Instant::now();
        let allowed_cwd = self.workspace_guard.check_exec_cwd(cwd)?;

        let action = RuntimeAction {
            capability: Capability::ExecShell,
            command: Some(command.to_string()),
            args: args.to_vec(),
            cwd: allowed_cwd.clone(),
            target_path: None,
        };

        let decision = self.policy.evaluate(session_id, &action).await?;
        if !decision.effect.is_allowed() {
            self.append_runtime_event(json!({
                "ts": Utc::now().to_rfc3339(),
                "session_id": session_id,
                "type": "command.denied",
                "command": command,
                "args": args,
                "cwd": allowed_cwd.display().to_string(),
                "policy_effect": format!("{:?}", decision.effect).to_ascii_lowercase(),
                "policy_reason": decision.reason,
                "duration_ms": started.elapsed().as_millis() as u64,
            }))?;
            bail!("runtime policy denied execution: {}", decision.reason);
        }

        let result = self
            .runtime
            .execute(command, args, timeout_secs, &allowed_cwd, None)
            .await?;

        self.append_runtime_event(json!({
            "ts": Utc::now().to_rfc3339(),
            "session_id": session_id,
            "type": "command.executed",
            "command": command,
            "args": args,
            "cwd": allowed_cwd.display().to_string(),
            "exit_code": result.exit_code,
            "duration_ms": started.elapsed().as_millis() as u64,
            "summary": result.output.lines().next().unwrap_or_default(),
        }))?;

        Ok(result)
    }
}
