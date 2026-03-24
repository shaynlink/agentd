use std::path::Path;
use std::process::Stdio;
use std::time::Instant;

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use tokio::process::Command;

use crate::domain::process_handle::{ProcessExecutionResult, ProcessHandle, ProcessState};
use crate::domain::resource_limit::{ResourceLimit, ResourceUsage};
use crate::ports::process::ProcessPort;
use crate::ports::resource::ResourcePort;
use crate::ports::runtime::RuntimePort;

#[derive(Debug, Default)]
pub struct BuiltinRuntime;

impl BuiltinRuntime {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ProcessPort for BuiltinRuntime {
    async fn spawn_process(&self, command: &str, args: &[String]) -> Result<ProcessHandle> {
        let mut child = Command::new(command)
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("failed to spawn process: {}", command))?;

        let pid = child
            .id()
            .context("spawned process does not expose a pid")?;

        tokio::spawn(async move {
            let _ = child.wait().await;
        });

        Ok(ProcessHandle {
            pid,
            command: command.to_string(),
            args: args.to_vec(),
            started_at: Utc::now(),
            state: ProcessState::Running,
        })
    }

    async fn kill_process(&self, pid: u32) -> Result<()> {
        let status = Command::new("kill")
            .arg("-TERM")
            .arg(pid.to_string())
            .status()
            .await
            .with_context(|| format!("failed to send SIGTERM to pid {}", pid))?;

        if !status.success() {
            anyhow::bail!("failed to kill process with pid {}", pid);
        }

        Ok(())
    }
}

#[async_trait]
impl ResourcePort for BuiltinRuntime {
    async fn get_usage(&self, _pid: u32) -> Result<ResourceUsage> {
        Ok(ResourceUsage::default())
    }

    async fn enforce_limits(&self, _pid: u32, _limit: &ResourceLimit) -> Result<()> {
        Ok(())
    }
}

#[async_trait]
impl RuntimePort for BuiltinRuntime {
    fn name(&self) -> &'static str {
        "builtin"
    }

    async fn execute(
        &self,
        command: &str,
        args: &[String],
        timeout_secs: u64,
        cwd: &Path,
        _limit: Option<&ResourceLimit>,
    ) -> Result<ProcessExecutionResult> {
        let mut command_process = Command::new(command);
        command_process
            .args(args)
            .current_dir(cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let start = Instant::now();
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            command_process.output(),
        )
        .await
        .context("builtin runtime execution timeout")?
        .with_context(|| format!("failed to execute command: {}", command))?;

        let wall_time_millis = start.elapsed().as_millis() as u64;
        let stdout_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr_str = String::from_utf8_lossy(&output.stderr).trim().to_string();

        let combined = if !stdout_str.is_empty() {
            if !stderr_str.is_empty() {
                format!("{}\n{}", stdout_str, stderr_str)
            } else {
                stdout_str
            }
        } else {
            stderr_str
        };

        Ok(ProcessExecutionResult {
            output: combined,
            exit_code: output.status.code().unwrap_or(1),
            usage: ResourceUsage {
                wall_time_millis,
                cpu_millis: None,
                memory_bytes: None,
            },
        })
    }

    async fn spawn_background(
        &self,
        command: &str,
        args: &[String],
        cwd: &Path,
    ) -> Result<ProcessHandle> {
        let mut child = Command::new(command)
            .args(args)
            .current_dir(cwd)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("failed to spawn background command: {}", command))?;

        let pid = child
            .id()
            .context("spawned process does not expose a pid")?;

        tokio::spawn(async move {
            let _ = child.wait().await;
        });

        Ok(ProcessHandle {
            pid,
            command: command.to_string(),
            args: args.to_vec(),
            started_at: Utc::now(),
            state: ProcessState::Running,
        })
    }
}
