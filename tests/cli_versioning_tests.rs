use std::fs;
use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;
use uuid::Uuid;

fn bin_path() -> &'static str {
    env!("CARGO_BIN_EXE_agentd")
}

fn run_cli(args: &[&str]) -> std::process::Output {
    Command::new(bin_path())
        .args(args)
        .output()
        .expect("run agentd cli command")
}

fn stdout_text(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr_text(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).trim().to_string()
}

fn temp_repo_path() -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("agentd-cli-versioning-{}", Uuid::new_v4()));
    path
}

fn temp_db_path() -> String {
    let mut path = std::env::temp_dir();
    path.push(format!("agentd-cli-versioning-state-{}.db", Uuid::new_v4()));
    path.to_string_lossy().to_string()
}

fn run_git(repo: &PathBuf, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .expect("run git command");
    assert!(
        out.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
}

fn git_stdout(repo: &PathBuf, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .expect("run git command");
    assert!(
        out.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn setup_repo() -> PathBuf {
    let repo = temp_repo_path();
    fs::create_dir_all(&repo).expect("create temp repo dir");

    run_git(&repo, &["init"]);
    run_git(&repo, &["config", "user.email", "agentd@example.com"]);
    run_git(&repo, &["config", "user.name", "agentd-tests"]);

    let file = repo.join("README.md");
    fs::write(&file, "hello\n").expect("write initial file");
    run_git(&repo, &["add", "."]);
    run_git(&repo, &["commit", "-m", "initial"]);

    repo
}

fn current_branch(repo: &PathBuf) -> String {
    git_stdout(repo, &["rev-parse", "--abbrev-ref", "HEAD"])
}

#[test]
fn cli_version_branch_diff_and_merge_dry_run_flow() {
    let repo = setup_repo();
    let db_path = temp_db_path();
    let base_branch = current_branch(&repo);
    let repo_str = repo.to_string_lossy().to_string();

    let create = run_cli(&[
        "--output",
        "json",
        "--db-path",
        &db_path,
        "version-branch-create",
        "--repo",
        &repo_str,
        "--branch",
        "feature/cli",
        "--from-ref",
        &base_branch,
    ]);
    assert!(
        create.status.success(),
        "version-branch-create should succeed, stderr: {}",
        stderr_text(&create)
    );

    let list = run_cli(&[
        "--output",
        "json",
        "--db-path",
        &db_path,
        "version-branch-list",
        "--repo",
        &repo_str,
    ]);
    assert!(
        list.status.success(),
        "version-branch-list should succeed, stderr: {}",
        stderr_text(&list)
    );
    let list_payload: Value =
        serde_json::from_str(&stdout_text(&list)).expect("version-branch-list should return JSON");
    let branches = list_payload
        .get("data")
        .and_then(|d| d.get("branches"))
        .and_then(Value::as_array)
        .expect("branches should exist");
    assert!(
        branches
            .iter()
            .any(|b| b.get("name").and_then(Value::as_str) == Some("feature/cli")),
        "feature branch should be present in list"
    );

    run_git(&repo, &["checkout", "feature/cli"]);
    fs::write(repo.join("README.md"), "hello\nfrom-cli\n").expect("update feature file");
    run_git(&repo, &["add", "."]);
    run_git(&repo, &["commit", "-m", "feature cli commit"]);
    run_git(&repo, &["checkout", &base_branch]);

    let head_before = git_stdout(&repo, &["rev-parse", "HEAD"]);

    let diff = run_cli(&[
        "--output",
        "json",
        "--db-path",
        &db_path,
        "version-diff",
        "--repo",
        &repo_str,
        "--from-ref",
        &base_branch,
        "--to-ref",
        "feature/cli",
    ]);
    assert!(
        diff.status.success(),
        "version-diff should succeed, stderr: {}",
        stderr_text(&diff)
    );
    assert!(
        stdout_text(&diff).contains("from-cli"),
        "diff output should include feature line"
    );

    let merge = run_cli(&[
        "--output",
        "json",
        "--db-path",
        &db_path,
        "version-merge",
        "--repo",
        &repo_str,
        "--source",
        "feature/cli",
        "--target",
        &base_branch,
        "--no-ff",
        "--dry-run",
    ]);
    assert!(
        merge.status.success(),
        "version-merge --dry-run should succeed, stderr: {}",
        stderr_text(&merge)
    );

    let head_after = git_stdout(&repo, &["rev-parse", "HEAD"]);
    assert_eq!(
        head_before, head_after,
        "dry-run merge should not change HEAD"
    );

    let status = git_stdout(&repo, &["status", "--porcelain"]);
    assert!(status.is_empty(), "dry-run merge should keep clean status");
}

#[test]
fn cli_version_rollback_requires_explicit_confirmation() {
    let repo = setup_repo();
    let db_path = temp_db_path();
    let repo_str = repo.to_string_lossy().to_string();

    let rollback = run_cli(&[
        "--db-path",
        &db_path,
        "version-rollback",
        "--repo",
        &repo_str,
        "--to-ref",
        "HEAD~1",
    ]);
    assert!(
        !rollback.status.success(),
        "version-rollback should fail without confirmation"
    );

    let err: Value = serde_json::from_str(&stderr_text(&rollback)).expect("stderr should be JSON");
    assert!(
        err.get("message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("--confirm-hard-reset=true"),
        "unexpected rollback error payload: {}",
        stderr_text(&rollback)
    );
}
