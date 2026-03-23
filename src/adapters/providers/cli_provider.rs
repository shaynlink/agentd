use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::config::AppConfig;
use crate::domain::plan::Plan;
use crate::ports::provider::{Provider, ProviderRunRequest, ProviderRunResult};

pub struct CliProvider;

impl CliProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CliProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy)]
enum PromptMode {
    Stdin,
    Arg,
}

impl PromptMode {
    fn from_value(value: &str) -> Self {
        match value {
            v if v.eq_ignore_ascii_case("arg") => Self::Arg,
            _ => Self::Stdin,
        }
    }
}

#[derive(Debug, Clone)]
struct CliProviderConfig {
    command: String,
    args: Vec<String>,
    prompt_mode: PromptMode,
    prompt_flag: String,
    runtime_dir: PathBuf,
}

impl CliProviderConfig {
    fn load() -> Result<Self> {
        let cfg = AppConfig::load()?;
        let cli_cfg = cfg.cli;

        Ok(Self {
            command: cli_cfg.command,
            args: cli_cfg.args,
            prompt_mode: PromptMode::from_value(&cli_cfg.prompt_mode),
            prompt_flag: cli_cfg.prompt_flag,
            runtime_dir: cli_cfg.runtime_dir,
        })
    }

    fn pid_path(&self, agent_id: &str) -> PathBuf {
        self.runtime_dir.join(format!("{agent_id}.pid"))
    }
}

fn write_pid(path: &Path, pid: u32) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create runtime directory: {}", parent.display()))?;
    }
    fs::write(path, pid.to_string())
        .with_context(|| format!("failed to write pid file: {}", path.display()))
}

fn cleanup_pid(path: &Path) {
    let _ = fs::remove_file(path);
}

fn read_pid(path: &Path) -> Result<i32> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read pid file: {}", path.display()))?;
    let pid = raw
        .trim()
        .parse::<i32>()
        .with_context(|| format!("invalid pid in file: {}", path.display()))?;
    Ok(pid)
}

async fn send_kill_signal(pid: i32, signal: &str) -> Result<bool> {
    let status = Command::new("kill")
        .arg(signal)
        .arg(pid.to_string())
        .status()
        .await
        .with_context(|| format!("failed to execute kill {signal} {pid}"))?;
    Ok(status.success())
}

#[async_trait]
impl Provider for CliProvider {
    fn name(&self) -> &'static str {
        "cli"
    }

    async fn generate_plan(&self, _goal: &str) -> Result<Plan> {
        bail!("cli provider does not implement plan generation yet")
    }

    async fn run_agent(&self, request: ProviderRunRequest) -> Result<ProviderRunResult> {
        let cfg = CliProviderConfig::load()?;

        let mut command = Command::new(&cfg.command);
        command.args(&cfg.args);
        command.stdout(Stdio::piped()).stderr(Stdio::piped());

        match cfg.prompt_mode {
            PromptMode::Stdin => {
                command.stdin(Stdio::piped());
            }
            PromptMode::Arg => {
                command.stdin(Stdio::null());
                command.arg(&cfg.prompt_flag).arg(&request.prompt);
            }
        }

        let mut child = command
            .spawn()
            .with_context(|| format!("failed to spawn CLI command: {}", cfg.command))?;

        let pid = child.id().with_context(|| {
            format!("spawned process has no pid for agent {}", request.agent_id)
        })?;
        let pid_path = cfg.pid_path(&request.agent_id);
        write_pid(&pid_path, pid)?;

        if matches!(cfg.prompt_mode, PromptMode::Stdin)
            && let Some(mut stdin) = child.stdin.take()
        {
            stdin
                .write_all(request.prompt.as_bytes())
                .await
                .context("failed to send prompt to CLI process stdin")?;
            stdin
                .write_all(b"\n")
                .await
                .context("failed to terminate prompt input on stdin")?;
            stdin
                .shutdown()
                .await
                .context("failed to close CLI process stdin")?;
        }

        let output = child
            .wait_with_output()
            .await
            .with_context(|| format!("failed waiting for CLI process: {}", cfg.command));
        cleanup_pid(&pid_path);
        let output = output?;

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        if output.status.success() {
            let merged = if !stdout.is_empty() {
                stdout
            } else if !stderr.is_empty() {
                stderr
            } else {
                "(cli provider completed with empty output)".to_string()
            };
            return Ok(ProviderRunResult { output: merged });
        }

        let detail = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            "no process output".to_string()
        };
        bail!(
            "cli command failed with status {}: {}",
            output.status,
            detail
        )
    }

    async fn cancel(&self, agent_id: &str) -> Result<()> {
        let cfg = CliProviderConfig::load()?;
        let pid_path = cfg.pid_path(agent_id);
        if !pid_path.exists() {
            return Ok(());
        }

        let pid = read_pid(&pid_path)?;
        let terminated = send_kill_signal(pid, "-TERM").await.unwrap_or(false);
        if !terminated {
            let _ = send_kill_signal(pid, "-KILL").await;
        }
        cleanup_pid(&pid_path);
        Ok(())
    }
}
