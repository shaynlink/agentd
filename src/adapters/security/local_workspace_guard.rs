use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::ports::workspace_guard::WorkspaceGuardPort;

pub struct LocalWorkspaceGuard {
    workspace_root: PathBuf,
    blocked_paths: Vec<PathBuf>,
    allowed_read_paths: Vec<PathBuf>,
    allowed_write_paths: Vec<PathBuf>,
}

impl LocalWorkspaceGuard {
    pub fn new(
        workspace_root: PathBuf,
        blocked_paths: Vec<PathBuf>,
        allowed_read_paths: Vec<PathBuf>,
        allowed_write_paths: Vec<PathBuf>,
    ) -> Result<Self> {
        let workspace_root = fs::canonicalize(workspace_root)
            .context("failed to canonicalize workspace root")?;

        Ok(Self {
            workspace_root,
            blocked_paths,
            allowed_read_paths,
            allowed_write_paths,
        })
    }

    fn resolve_path(&self, cwd: &Path, path: &Path) -> Result<PathBuf> {
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            cwd.join(path)
        };

        if absolute.exists() {
            return fs::canonicalize(&absolute)
                .with_context(|| format!("failed to canonicalize path: {}", absolute.display()));
        }

        let parent = absolute
            .parent()
            .context("target path has no parent")?
            .to_path_buf();

        let canonical_parent = fs::canonicalize(&parent)
            .with_context(|| format!("failed to canonicalize parent path: {}", parent.display()))?;

        let file_name = absolute
            .file_name()
            .context("target path has no file name")?;

        Ok(canonical_parent.join(file_name))
    }

    fn ensure_in_workspace(&self, resolved: &Path) -> Result<()> {
        if !resolved.starts_with(&self.workspace_root) {
            bail!(
                "path '{}' escapes workspace root '{}'",
                resolved.display(),
                self.workspace_root.display()
            );
        }
        Ok(())
    }

    fn ensure_not_blocked(&self, resolved: &Path) -> Result<()> {
        for blocked in &self.blocked_paths {
            let blocked_abs = if blocked.is_absolute() {
                blocked.clone()
            } else {
                self.workspace_root.join(blocked)
            };

            let blocked_norm = blocked_abs
                .components()
                .collect::<PathBuf>();

            if resolved.starts_with(&blocked_norm) {
                bail!("path '{}' is blocked by workspace policy", resolved.display());
            }
        }
        Ok(())
    }

    fn ensure_allowed_by_list(&self, resolved: &Path, allowed_paths: &[PathBuf]) -> Result<()> {
        if allowed_paths.is_empty() {
            return Ok(());
        }

        for allowed in allowed_paths {
            let allowed_abs = if allowed.is_absolute() {
                allowed.clone()
            } else {
                self.workspace_root.join(allowed)
            };

            if resolved.starts_with(&allowed_abs) {
                return Ok(());
            }
        }

        bail!(
            "path '{}' is not in allowed paths",
            resolved.display()
        )
    }
}

impl WorkspaceGuardPort for LocalWorkspaceGuard {
    fn name(&self) -> &'static str {
        "local-workspace-guard"
    }

    fn check_read(&self, cwd: &Path, path: &Path) -> Result<PathBuf> {
        let resolved = self.resolve_path(cwd, path)?;
        self.ensure_in_workspace(&resolved)?;
        self.ensure_not_blocked(&resolved)?;
        self.ensure_allowed_by_list(&resolved, &self.allowed_read_paths)?;
        Ok(resolved)
    }

    fn check_write(&self, cwd: &Path, path: &Path) -> Result<PathBuf> {
        let resolved = self.resolve_path(cwd, path)?;
        self.ensure_in_workspace(&resolved)?;
        self.ensure_not_blocked(&resolved)?;
        self.ensure_allowed_by_list(&resolved, &self.allowed_write_paths)?;
        Ok(resolved)
    }

    fn check_exec_cwd(&self, cwd: &Path) -> Result<PathBuf> {
        let resolved = self.resolve_path(cwd, Path::new("."))?;
        self.ensure_in_workspace(&resolved)?;
        self.ensure_not_blocked(&resolved)?;
        Ok(resolved)
    }
}
