use std::path::{Path, PathBuf};
use std::time::Instant;
use std::{fs, io::Write};

use anyhow::{Result, bail};
use chrono::Utc;
use serde_json::json;

use crate::domain::capability::Capability;
use crate::domain::process_handle::ProcessExecutionResult;
use crate::ports::policy::{PolicyPort, RuntimeAction};
use crate::ports::runtime::RuntimePort;
use crate::ports::workspace_guard::WorkspaceGuardPort;

pub struct RuntimeExecutor {
    policy: Box<dyn PolicyPort>,
    workspace_guard: Box<dyn WorkspaceGuardPort>,
    runtime: Box<dyn RuntimePort>,
    event_log_path: Option<PathBuf>,
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
        }
    }

    pub fn with_event_log_path(mut self, event_log_path: PathBuf) -> Self {
        self.event_log_path = Some(event_log_path);
        self
    }

    fn append_runtime_event(&self, event: serde_json::Value) -> Result<()> {
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
