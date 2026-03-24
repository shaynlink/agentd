use std::path::Path;

use anyhow::{Result, bail};

use crate::domain::capability::Capability;
use crate::domain::process_handle::ProcessExecutionResult;
use crate::ports::policy::{PolicyPort, RuntimeAction};
use crate::ports::runtime::RuntimePort;
use crate::ports::workspace_guard::WorkspaceGuardPort;

pub struct RuntimeExecutor {
    policy: Box<dyn PolicyPort>,
    workspace_guard: Box<dyn WorkspaceGuardPort>,
    runtime: Box<dyn RuntimePort>,
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
        }
    }

    pub async fn execute_command(
        &self,
        session_id: &str,
        command: &str,
        args: &[String],
        timeout_secs: u64,
        cwd: &Path,
    ) -> Result<ProcessExecutionResult> {
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
            bail!("runtime policy denied execution: {}", decision.reason);
        }

        self.runtime
            .execute(command, args, timeout_secs, &allowed_cwd, None)
            .await
    }
}
