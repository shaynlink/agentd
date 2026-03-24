use std::fs;
use std::path::{Path, PathBuf};

use agentd::adapters::security::local_workspace_guard::LocalWorkspaceGuard;
use agentd::ports::workspace_guard::WorkspaceGuardPort;
use uuid::Uuid;

fn temp_workspace() -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("agentd-workspace-{}", Uuid::new_v4()));
    fs::create_dir_all(&path).expect("create temp workspace");
    path
}

#[test]
fn allows_read_inside_workspace() {
    let root = temp_workspace();
    let file = root.join("src").join("main.rs");
    fs::create_dir_all(file.parent().expect("has parent")).expect("create src dir");
    fs::write(&file, "fn main() {}\n").expect("write test file");

    let guard = LocalWorkspaceGuard::new(root.clone(), Vec::new(), Vec::new(), Vec::new())
        .expect("create guard");

    let resolved = guard
        .check_read(&root, Path::new("src/main.rs"))
        .expect("read should be allowed");

    let canonical_root = fs::canonicalize(&root).expect("canonicalize root");
    assert!(resolved.starts_with(&canonical_root));
}

#[test]
fn denies_traversal_outside_workspace() {
    let root = temp_workspace();
    let outside = root
        .parent()
        .expect("workspace has parent")
        .join(format!("outside-{}", Uuid::new_v4()));
    fs::create_dir_all(&outside).expect("create outside dir");
    fs::write(outside.join("secret.txt"), "secret\n").expect("write outside file");

    let guard = LocalWorkspaceGuard::new(root.clone(), Vec::new(), Vec::new(), Vec::new())
        .expect("create guard");

    let err = guard
        .check_read(&root, Path::new("../secret.txt"))
        .expect_err("read should be denied outside workspace");

    assert!(
        err.to_string().contains("escapes workspace root"),
        "unexpected error: {}",
        err
    );
}

#[test]
fn denies_blocked_path_and_write_outside_allowlist() {
    let root = temp_workspace();
    let git_dir = root.join(".git");
    fs::create_dir_all(&git_dir).expect("create .git");
    fs::write(git_dir.join("config"), "[core]\n").expect("write .git config");

    let src_dir = root.join("src");
    fs::create_dir_all(&src_dir).expect("create src dir");

    let guard = LocalWorkspaceGuard::new(
        root.clone(),
        vec![PathBuf::from(".git")],
        vec![PathBuf::from("src")],
        vec![PathBuf::from("src")],
    )
    .expect("create guard");

    let blocked = guard
        .check_read(&root, Path::new(".git/config"))
        .expect_err(".git read should be denied");
    assert!(blocked.to_string().contains("blocked"));

    let write_denied = guard
        .check_write(&root, Path::new("README.md"))
        .expect_err("write outside src should be denied");
    assert!(write_denied.to_string().contains("not in allowed paths"));

    guard
        .check_write(&root, Path::new("src/new.rs"))
        .expect("write in src should be allowed");
}
