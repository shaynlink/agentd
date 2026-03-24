use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;
use tokio::process::Command;

use crate::adapters::runtimes;
use crate::adapters::security;
use crate::domain::audit_log::{CommandAuditEntry, output_preview};
use crate::domain::permission::{PermissionSet, RuntimeRole};
use crate::domain::plan::Plan;
use crate::domain::runtime_config::RuntimeConfig;
use crate::ports::provider::{Provider, ProviderRunRequest, ProviderRunResult};

pub struct SandboxProvider;

impl SandboxProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SandboxProvider {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Command & Path Validation
// ============================================================================

fn is_command_allowed(cmd: &str, allowed_commands: &[String]) -> bool {
    if allowed_commands.is_empty() {
        return true; // No restrictions if empty
    }

    for allowed in allowed_commands {
        // Exact match
        if cmd == allowed {
            return true;
        }

        // Wildcard match (e.g., "vibe *")
        if allowed.ends_with('*') {
            let prefix = allowed.trim_end_matches('*').trim_end();
            if cmd.starts_with(prefix) {
                return true;
            }
        }
    }

    false
}

fn is_path_allowed(path: &Path, allowed_paths: &[String]) -> Result<bool> {
    if allowed_paths.is_empty() {
        return Ok(true); // No restrictions if empty
    }

    let canonical = fs::canonicalize(path)
        .or_else(|_| {
            // If path doesn't exist yet, try to canonicalize parent
            if let Some(parent) = path.parent() {
                fs::canonicalize(parent).map(|p| p.join(path.file_name().unwrap_or_default()))
            } else {
                fs::canonicalize(path)
            }
        })
        .context("failed to canonicalize path")?;

    for allowed in allowed_paths {
        let allowed_path = PathBuf::from(allowed);
        let allowed_canonical = fs::canonicalize(&allowed_path).or_else(|_| {
            if let Some(parent) = allowed_path.parent() {
                fs::canonicalize(parent)
                    .map(|p| p.join(allowed_path.file_name().unwrap_or_default()))
            } else {
                fs::canonicalize(&allowed_path)
            }
        })?;

        // Check if canonical is under allowed_canonical or is identical
        if canonical == allowed_canonical || canonical.starts_with(&allowed_canonical) {
            return Ok(true);
        }
    }

    Ok(false)
}

// ============================================================================
// Filesystem Snapshot & Diff Tracing
// ============================================================================

#[derive(Debug, Clone)]
struct FileSnapshot {
    size: u64,
    modified: i64,
}

fn hash_file(path: &Path) -> Result<FileSnapshot> {
    let metadata = fs::metadata(path).context("failed to read file metadata")?;
    Ok(FileSnapshot {
        size: metadata.len(),
        modified: metadata
            .modified()
            .ok()
            .and_then(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .ok()
                    .map(|d| d.as_secs() as i64)
            })
            .unwrap_or(0),
    })
}

fn snapshot_directory(path: &Path) -> Result<HashMap<PathBuf, FileSnapshot>> {
    let mut snapshot = HashMap::new();

    if !path.exists() {
        return Ok(snapshot);
    }

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            let hash = hash_file(&path)?;
            snapshot.insert(path, hash);
        } else if path.is_dir() {
            let subdir = snapshot_directory(&path)?;
            snapshot.extend(subdir);
        }
    }

    Ok(snapshot)
}

fn compute_diff(
    before: &HashMap<PathBuf, FileSnapshot>,
    after: &HashMap<PathBuf, FileSnapshot>,
) -> serde_json::Value {
    let mut created = Vec::new();
    let mut modified = Vec::new();
    let mut deleted = Vec::new();

    // Find created and modified files
    for (path, after_snap) in after {
        match before.get(path) {
            None => created.push(path.display().to_string()),
            Some(before_snap) => {
                if before_snap.size != after_snap.size
                    || before_snap.modified != after_snap.modified
                {
                    modified.push(path.display().to_string());
                }
            }
        }
    }

    // Find deleted files
    for path in before.keys() {
        if !after.contains_key(path) {
            deleted.push(path.display().to_string());
        }
    }

    json!({
        "created": created,
        "modified": modified,
        "deleted": deleted,
    })
}

// ============================================================================
// Runtime Execution
// ============================================================================

async fn run_via_vibe(
    vibe_path: &str,
    agent_prompt: &str,
    workdir: &Path,
    timeout_secs: u64,
) -> Result<(String, i32)> {
    let mut command = Command::new(vibe_path);
    command
        .arg("run")
        .arg("--prompt")
        .arg(agent_prompt)
        .current_dir(workdir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let start = Instant::now();
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs),
        command.output(),
    )
    .await
    .context("vibe execution timeout")?
    .context("failed to execute vibe")?;

    let _elapsed = start.elapsed().as_secs();
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

    let code = output.status.code().unwrap_or(1);

    Ok((combined, code))
}

async fn run_via_process(
    command_name: &str,
    args: &[String],
    agent_prompt: &str,
    workdir: &Path,
    timeout_secs: u64,
) -> Result<(String, i32)> {
    let runtime = runtimes::build_runtime("builtin")?;
    let mut full_args = args.to_vec();
    if !agent_prompt.is_empty() {
        full_args.push(agent_prompt.to_string());
    }

    let result = runtime
        .execute(command_name, &full_args, timeout_secs, workdir, None)
        .await?;

    Ok((result.output, result.exit_code))
}

// ============================================================================
// Provider Trait Implementation
// ============================================================================

#[async_trait]
impl Provider for SandboxProvider {
    fn name(&self) -> &'static str {
        "sandbox"
    }

    async fn generate_plan(&self, _goal: &str) -> Result<Plan> {
        // Sandbox provider delegates plan generation to underlying runtime
        // For now, we return a simple plan structure; in production, this would
        // invoke the configured runtime with plan-generation mode
        bail!("plan generation not yet supported for sandbox provider (use cli or http provider)")
    }

    async fn run_agent(&self, request: ProviderRunRequest) -> Result<ProviderRunResult> {
        let cfg = crate::config::AppConfig::load()?;
        let sandbox_cfg = &cfg.sandbox;
        let securable = security::build_securable(sandbox_cfg);
        let permissions = PermissionSet {
            role: RuntimeRole::from_value(&sandbox_cfg.role),
            allowed_commands: sandbox_cfg.allowed_commands.clone(),
            allowed_read_paths: sandbox_cfg.allowed_read_paths.clone(),
            allowed_write_paths: sandbox_cfg.allowed_write_paths.clone(),
        };

        // Trim whitespace from prompt for validation
        let prompt_trimmed = request.prompt.trim();
        let cmd_parts: Vec<&str> = prompt_trimmed.split_whitespace().collect();
        let main_cmd = cmd_parts.first().copied().unwrap_or("");

        let role_allowed = securable
            .check_command_access(main_cmd, permissions.role.as_str())
            .await?;

        if !permissions.can_execute_any_command() || !role_allowed {
            let denied = CommandAuditEntry {
                ts: Utc::now(),
                agent_id: request.agent_id.clone(),
                role: permissions.role.as_str().to_string(),
                runtime: sandbox_cfg.runtime.clone(),
                command_input: prompt_trimmed.to_string(),
                command_output_preview: String::new(),
                allowed: false,
                exit_code: None,
            };
            let denied_payload =
                serde_json::to_string(&denied).context("failed to serialize denied audit event")?;
            securable.log_audit_event(&denied_payload).await?;
            bail!(
                "command execution denied for role '{}'",
                permissions.role.as_str()
            );
        }

        // Validate command against ACL
        if !permissions.bypass_acl()
            && !sandbox_cfg.allowed_commands.is_empty()
            && !cmd_parts.is_empty()
            && !is_command_allowed(main_cmd, &sandbox_cfg.allowed_commands)
        {
            let denied = CommandAuditEntry {
                ts: Utc::now(),
                agent_id: request.agent_id.clone(),
                role: permissions.role.as_str().to_string(),
                runtime: sandbox_cfg.runtime.clone(),
                command_input: prompt_trimmed.to_string(),
                command_output_preview: String::new(),
                allowed: false,
                exit_code: None,
            };
            let denied_payload =
                serde_json::to_string(&denied).context("failed to serialize denied audit event")?;
            securable.log_audit_event(&denied_payload).await?;
            bail!(
                "command not allowed: '{}' (not in allowed_commands)",
                main_cmd
            );
        }

        // Create agent-specific workdir
        let agent_workdir = sandbox_cfg.workdir.join(&request.agent_id);
        fs::create_dir_all(&agent_workdir).context("failed to create agent workdir")?;

        let has_workdir_access = securable
            .check_file_access(&agent_workdir, permissions.role.as_str())
            .await?;
        if !has_workdir_access {
            let denied = CommandAuditEntry {
                ts: Utc::now(),
                agent_id: request.agent_id.clone(),
                role: permissions.role.as_str().to_string(),
                runtime: sandbox_cfg.runtime.clone(),
                command_input: format!("workdir:{}", agent_workdir.display()),
                command_output_preview: String::new(),
                allowed: false,
                exit_code: None,
            };
            let denied_payload =
                serde_json::to_string(&denied).context("failed to serialize denied audit event")?;
            securable.log_audit_event(&denied_payload).await?;
            bail!(
                "file access denied for role '{}' on {}",
                permissions.role.as_str(),
                agent_workdir.display()
            );
        }

        // Validate read paths
        for read_path in &sandbox_cfg.allowed_read_paths {
            let path = PathBuf::from(read_path);
            if !is_path_allowed(&path, &sandbox_cfg.allowed_read_paths)? {
                bail!("read path not allowed: {}", read_path);
            }
        }

        // Validate write paths
        for write_path in &sandbox_cfg.allowed_write_paths {
            let path = PathBuf::from(write_path);
            if !is_path_allowed(&path, &sandbox_cfg.allowed_write_paths)? {
                bail!("write path not allowed: {}", write_path);
            }
        }

        // Capture pre-execution snapshot if tracing diff
        let before_snapshot = if sandbox_cfg.trace_diff {
            snapshot_directory(&agent_workdir)?
        } else {
            HashMap::new()
        };

        let resolved_runtime = RuntimeConfig::resolve(
            None,
            request.runtime_override.as_deref(),
            sandbox_cfg.runtime.as_str(),
        );

        // Execute via selected runtime
        let (output, exit_code) = match resolved_runtime.runtime.as_str() {
            "vibe" => {
                run_via_vibe(
                    &sandbox_cfg.vibe_path,
                    prompt_trimmed,
                    &agent_workdir,
                    request.timeout_secs,
                )
                .await?
            }
            "process" => {
                // For process runtime, treat the prompt as command + args
                let parts: Vec<&str> = prompt_trimmed.splitn(2, ' ').collect();
                let (cmd, args) = if parts.len() == 2 {
                    let args_vec: Vec<String> =
                        parts[1].split_whitespace().map(|s| s.to_string()).collect();
                    (parts[0], args_vec)
                } else {
                    (parts[0], Vec::new())
                };

                run_via_process(cmd, &args, "", &agent_workdir, request.timeout_secs).await?
            }
            "docker" => {
                bail!("docker runtime not yet implemented")
            }
            other => {
                bail!("unknown sandbox runtime: {}", other)
            }
        };

        // Capture post-execution snapshot if tracing diff
        let diff = if sandbox_cfg.trace_diff {
            let after_snapshot = snapshot_directory(&agent_workdir)?;
            compute_diff(&before_snapshot, &after_snapshot)
        } else {
            json!(null)
        };

        // Compose tracing data
        let audit = CommandAuditEntry {
            ts: Utc::now(),
            agent_id: request.agent_id.clone(),
            role: permissions.role.as_str().to_string(),
            runtime: resolved_runtime.runtime.clone(),
            command_input: prompt_trimmed.to_string(),
            command_output_preview: output_preview(&output, 300),
            allowed: true,
            exit_code: Some(exit_code),
        };
        let audit_payload =
            serde_json::to_string(&audit).context("failed to serialize audit event")?;
        securable.log_audit_event(&audit_payload).await?;

        let tracing = if sandbox_cfg.trace_commands || sandbox_cfg.trace_diff {
            json!({
                "timestamp": Utc::now().to_rfc3339(),
                "agent_id": request.agent_id,
                "command": prompt_trimmed,
                "exit_code": exit_code,
                "runtime": resolved_runtime.runtime,
                "role": permissions.role.as_str(),
                "audit": audit,
                "workdir": agent_workdir.display().to_string(),
                "diff": if sandbox_cfg.trace_diff { diff } else { json!(null) },
            })
            .to_string()
        } else {
            String::new()
        };

        // Combine output
        let final_output = if !tracing.is_empty() {
            if !output.is_empty() {
                format!("{}\n[TRACING]\n{}", output, tracing)
            } else {
                format!("[TRACING]\n{}", tracing)
            }
        } else {
            output
        };

        Ok(ProviderRunResult {
            output: final_output,
        })
    }

    async fn cancel(&self, _agent_id: &str) -> Result<()> {
        // Sandbox provider cancellation could kill processes in the workdir
        // For now, just return OK as process cleanup is OS-level
        Ok(())
    }
}
