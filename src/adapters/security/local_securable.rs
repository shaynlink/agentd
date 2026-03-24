use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use async_trait::async_trait;
use rusqlite::params;

use crate::config::SandboxProviderConfig;
use crate::domain::permission::RuntimeRole;
use crate::ports::securable::SecurablePort;

pub struct LocalSecurable {
    allowed_read_paths: Vec<String>,
    allowed_write_paths: Vec<String>,
    audit_log_path: PathBuf,
    audit_backend: String,
}

impl LocalSecurable {
    pub fn new(config: &SandboxProviderConfig) -> Self {
        Self {
            allowed_read_paths: config.allowed_read_paths.clone(),
            allowed_write_paths: config.allowed_write_paths.clone(),
            audit_log_path: config.audit_log_path.clone(),
            audit_backend: config.audit_backend.clone(),
        }
    }

    fn write_file_audit(&self, payload: &str) -> Result<()> {
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

    fn write_sqlite_audit(&self, payload: &str) -> Result<()> {
        if let Some(parent) = self.audit_log_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create sqlite audit dir: {}", parent.display())
            })?;
        }

        let conn = rusqlite::Connection::open(&self.audit_log_path).with_context(|| {
            format!(
                "failed to open sqlite audit DB: {}",
                self.audit_log_path.display()
            )
        })?;

        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS security_audit_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ts TEXT NOT NULL,
                payload TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_security_audit_logs_ts
                ON security_audit_logs(ts);
            "#,
        )
        .context("failed to initialize sqlite audit schema")?;

        conn.execute(
            "INSERT INTO security_audit_logs (ts, payload) VALUES (?1, ?2)",
            params![chrono::Utc::now().to_rfc3339(), payload],
        )
        .context("failed to append sqlite audit log")?;

        Ok(())
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
        if self.audit_backend.eq_ignore_ascii_case("sqlite") {
            return self.write_sqlite_audit(payload);
        }

        self.write_file_audit(payload)
    }
}
