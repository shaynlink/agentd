use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use async_trait::async_trait;

use crate::config::SandboxProviderConfig;
use crate::domain::permission::RuntimeRole;
use crate::ports::securable::SecurablePort;

pub struct LocalSecurable {
    allowed_read_paths: Vec<String>,
    allowed_write_paths: Vec<String>,
    audit_log_path: PathBuf,
}

impl LocalSecurable {
    pub fn new(config: &SandboxProviderConfig) -> Self {
        Self {
            allowed_read_paths: config.allowed_read_paths.clone(),
            allowed_write_paths: config.allowed_write_paths.clone(),
            audit_log_path: config.audit_log_path.clone(),
        }
    }
}

fn is_path_allowed(path: &Path, allowed_paths: &[String]) -> Result<bool> {
    if allowed_paths.is_empty() {
        return Ok(true);
    }

    let canonical = fs::canonicalize(path)
        .or_else(|_| {
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

        if canonical == allowed_canonical || canonical.starts_with(&allowed_canonical) {
            return Ok(true);
        }
    }

    Ok(false)
}

#[async_trait]
impl SecurablePort for LocalSecurable {
    async fn check_command_access(&self, _command: &str, role: &str) -> Result<bool> {
        let role = RuntimeRole::from_value(role);
        Ok(role != RuntimeRole::Viewer)
    }

    async fn check_file_access(&self, path: &Path, role: &str) -> Result<bool> {
        let role = RuntimeRole::from_value(role);
        if role == RuntimeRole::Admin {
            return Ok(true);
        }

        let mut allowed_paths = Vec::new();
        allowed_paths.extend(self.allowed_read_paths.clone());
        allowed_paths.extend(self.allowed_write_paths.clone());

        is_path_allowed(path, &allowed_paths)
    }

    async fn log_audit_event(&self, payload: &str) -> Result<()> {
        if let Some(parent) = self.audit_log_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create audit log dir: {}", parent.display()))?;
        }

        let mut existing = if self.audit_log_path.exists() {
            fs::read_to_string(&self.audit_log_path).with_context(|| {
                format!(
                    "failed to read audit log file: {}",
                    self.audit_log_path.display()
                )
            })?
        } else {
            String::new()
        };

        if !existing.is_empty() && !existing.ends_with('\n') {
            existing.push('\n');
        }
        existing.push_str(payload);
        existing.push('\n');

        fs::write(&self.audit_log_path, existing).with_context(|| {
            format!(
                "failed to write audit log file: {}",
                self.audit_log_path.display()
            )
        })?;

        Ok(())
    }
}
