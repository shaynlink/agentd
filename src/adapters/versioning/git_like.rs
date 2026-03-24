use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::ports::versioning::{MergeResult, VersionBranch, VersioningPort};

pub struct GitLikeVersioningAdapter;

impl GitLikeVersioningAdapter {
    pub fn new() -> Self {
        Self
    }

    fn ensure_repo(&self, repo_path: &Path) -> Result<()> {
        let output = Command::new("git")
            .arg("-C")
            .arg(repo_path)
            .arg("rev-parse")
            .arg("--is-inside-work-tree")
            .output()
            .with_context(|| format!("failed to execute git in {}", repo_path.display()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            bail!("not a git repository at {}: {stderr}", repo_path.display());
        }

        Ok(())
    }

    fn run_git(&self, repo_path: &Path, args: &[&str]) -> Result<String> {
        let output = Command::new("git")
            .arg("-C")
            .arg(repo_path)
            .args(args)
            .output()
            .with_context(|| format!("failed to execute git {:?} in {}", args, repo_path.display()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            bail!("git command failed (git {:?}): {stderr}", args);
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn run_git_output(&self, repo_path: &Path, args: &[&str]) -> Result<std::process::Output> {
        Command::new("git")
            .arg("-C")
            .arg(repo_path)
            .args(args)
            .output()
            .with_context(|| format!("failed to execute git {:?} in {}", args, repo_path.display()))
    }
}

impl Default for GitLikeVersioningAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl VersioningPort for GitLikeVersioningAdapter {
    fn name(&self) -> &'static str {
        "git-like"
    }

    fn create_branch(&self, repo_path: &Path, branch: &str, from_ref: Option<&str>) -> Result<()> {
        self.ensure_repo(repo_path)?;
        if branch.trim().is_empty() {
            bail!("branch name cannot be empty");
        }

        match from_ref {
            Some(base) => {
                self.run_git(repo_path, &["branch", branch, base])?;
            }
            None => {
                self.run_git(repo_path, &["branch", branch])?;
            }
        }

        Ok(())
    }

    fn list_branches(&self, repo_path: &Path) -> Result<Vec<VersionBranch>> {
        self.ensure_repo(repo_path)?;
        let output = self.run_git(repo_path, &["branch", "--format=%(refname:short)|%(HEAD)"])?;
        let mut branches = Vec::new();

        for line in output.lines() {
            let mut parts = line.split('|');
            let name = parts.next().unwrap_or_default().trim().to_string();
            let head_marker = parts.next().unwrap_or_default().trim();
            if !name.is_empty() {
                branches.push(VersionBranch {
                    name,
                    current: head_marker == "*",
                });
            }
        }

        Ok(branches)
    }

    fn diff(&self, repo_path: &Path, from_ref: &str, to_ref: &str) -> Result<String> {
        self.ensure_repo(repo_path)?;
        if from_ref.trim().is_empty() || to_ref.trim().is_empty() {
            bail!("from_ref and to_ref are required");
        }

        self.run_git(repo_path, &["--no-pager", "diff", &format!("{from_ref}..{to_ref}")])
    }

    fn merge(
        &self,
        repo_path: &Path,
        source_branch: &str,
        target_branch: &str,
        no_ff: bool,
        dry_run: bool,
    ) -> Result<MergeResult> {
        self.ensure_repo(repo_path)?;
        if source_branch.trim().is_empty() || target_branch.trim().is_empty() {
            bail!("source_branch and target_branch are required");
        }

        self.run_git(repo_path, &["checkout", target_branch])?;
        let merge_output = if dry_run {
            if no_ff {
                self.run_git_output(
                    repo_path,
                    &["merge", "--no-ff", "--no-commit", source_branch],
                )?
            } else {
                self.run_git_output(repo_path, &["merge", "--no-commit", source_branch])?
            }
        } else if no_ff {
            self.run_git_output(
                repo_path,
                &[
                    "merge",
                    "--no-ff",
                    source_branch,
                    "-m",
                    &format!("merge {source_branch} into {target_branch}"),
                ],
            )?
        } else {
            self.run_git_output(repo_path, &["merge", source_branch])?
        };

        if !merge_output.status.success() {
            let conflicted_files = self
                .run_git(repo_path, &["diff", "--name-only", "--diff-filter=U"])
                .unwrap_or_default();
            let _ = self.run_git(repo_path, &["merge", "--abort"]);

            let stderr = String::from_utf8_lossy(&merge_output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&merge_output.stdout).trim().to_string();
            let details = if !stderr.is_empty() {
                stderr
            } else {
                stdout
            };

            if !conflicted_files.is_empty() {
                bail!(
                    "merge conflict while merging '{source_branch}' into '{target_branch}'; conflicted files: {}. details: {}",
                    conflicted_files.replace('\n', ", "),
                    details
                );
            }

            bail!(
                "merge failed while merging '{source_branch}' into '{target_branch}': {}",
                details
            );
        }

        if dry_run {
            let _ = self.run_git(repo_path, &["merge", "--abort"]);
        }

        let commit = self.run_git(repo_path, &["rev-parse", "HEAD"])?;

        Ok(MergeResult {
            target: target_branch.to_string(),
            source: source_branch.to_string(),
            commit,
        })
    }

    fn rollback_hard(&self, repo_path: &Path, to_ref: &str) -> Result<String> {
        self.ensure_repo(repo_path)?;
        if to_ref.trim().is_empty() {
            bail!("to_ref is required");
        }

        self.run_git(repo_path, &["reset", "--hard", to_ref])?;
        self.run_git(repo_path, &["rev-parse", "HEAD"])
    }
}
