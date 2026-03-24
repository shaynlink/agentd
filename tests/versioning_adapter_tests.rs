use std::fs;
use std::path::PathBuf;
use std::process::Command;

use agentd::adapters::versioning;
use uuid::Uuid;

fn temp_repo_path() -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("agentd-versioning-{}", Uuid::new_v4()));
    path
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
fn git_like_branch_diff_merge_rollback_flow() {
    let repo = setup_repo();
    let base_branch = current_branch(&repo);
    let adapter = versioning::build_versioning("git").expect("build versioning adapter");

    adapter
        .create_branch(&repo, "feature/rbac", None)
        .expect("create feature branch");

    let branches = adapter.list_branches(&repo).expect("list branches");
    assert!(
        branches.iter().any(|b| b.name == "feature/rbac"),
        "feature branch should exist"
    );

    run_git(&repo, &["checkout", "feature/rbac"]);
    let file = repo.join("README.md");
    fs::write(&file, "hello\nrbac\n").expect("update feature file");
    run_git(&repo, &["add", "."]);
    run_git(&repo, &["commit", "-m", "feature commit"]);

    let diff = adapter
        .diff(&repo, &base_branch, "feature/rbac")
        .expect("diff refs");
    assert!(
        diff.contains("+rbac"),
        "diff should include feature line, got: {diff}"
    );

    let result = adapter
        .merge(&repo, "feature/rbac", &base_branch, true)
        .expect("merge feature into master");
    assert_eq!(result.source, "feature/rbac");
    assert_eq!(result.target, base_branch);

    let content_after_merge = fs::read_to_string(&file).expect("read merged file");
    assert!(content_after_merge.contains("rbac"));

    let parent = git_stdout(&repo, &["rev-parse", "HEAD~1"]);
    let head_after_reset = adapter
        .rollback_hard(&repo, &parent)
        .expect("hard rollback to parent");
    assert_eq!(head_after_reset, parent, "HEAD should match rollback target");

    let content_after_rollback = fs::read_to_string(&file).expect("read rolled-back file");
    assert!(
        !content_after_rollback.contains("rbac"),
        "rollback should remove merged line"
    );
}

#[test]
fn git_like_merge_conflict_reports_files_and_aborts_merge_state() {
    let repo = setup_repo();
    let base_branch = current_branch(&repo);
    let adapter = versioning::build_versioning("git").expect("build versioning adapter");

    adapter
        .create_branch(&repo, "feature/left", Some(&base_branch))
        .expect("create left branch");
    adapter
        .create_branch(&repo, "feature/right", Some(&base_branch))
        .expect("create right branch");

    run_git(&repo, &["checkout", "feature/left"]);
    let file = repo.join("README.md");
    fs::write(&file, "left-side\n").expect("write left side change");
    run_git(&repo, &["add", "."]);
    run_git(&repo, &["commit", "-m", "left change"]);

    run_git(&repo, &["checkout", "feature/right"]);
    fs::write(&file, "right-side\n").expect("write right side change");
    run_git(&repo, &["add", "."]);
    run_git(&repo, &["commit", "-m", "right change"]);

    let err = adapter
        .merge(&repo, "feature/left", "feature/right", true)
        .expect_err("merge should fail with conflict");
    let message = err.to_string();
    assert!(
        message.contains("merge conflict") && message.contains("README.md"),
        "expected conflict details with file list, got: {message}"
    );

    let status = git_stdout(&repo, &["status", "--porcelain"]);
    assert!(
        status.is_empty(),
        "merge conflict should be auto-aborted and leave clean status, got: {status}"
    );
}
