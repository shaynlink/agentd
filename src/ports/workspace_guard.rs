use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy)]
pub enum FileAccessKind {
    Read,
    Write,
}

pub trait WorkspaceGuardPort: Send + Sync {
    fn name(&self) -> &'static str;

    fn check_read(&self, cwd: &Path, path: &Path) -> anyhow::Result<PathBuf>;

    fn check_write(&self, cwd: &Path, path: &Path) -> anyhow::Result<PathBuf>;

    fn check_exec_cwd(&self, cwd: &Path) -> anyhow::Result<PathBuf>;
}
