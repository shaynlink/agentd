use std::path::PathBuf;

use agentd::adapters::runtimes;
use agentd::adapters::security::local_policy::LocalPolicyEngine;
use agentd::adapters::security::local_workspace_guard::LocalWorkspaceGuard;
use agentd::app::runtime_executor::RuntimeExecutor;

#[tokio::test]
async fn runtime_executor_runs_allowed_command() {
    let workspace = std::env::current_dir().expect("get current dir");
    let policy = Box::new(LocalPolicyEngine::new("full-trusted"));
    let guard = Box::new(
        LocalWorkspaceGuard::new(workspace.clone(), Vec::new(), Vec::new(), Vec::new())
            .expect("create workspace guard"),
    );
    let runtime = runtimes::build_runtime("builtin").expect("build runtime");

    let executor = RuntimeExecutor::new(policy, guard, runtime);
    let args = vec!["hello-runtime".to_string()];

    let result = executor
        .execute_command("sess_exec_1", "echo", &args, 5, &workspace)
        .await
        .expect("execute echo command");

    assert_eq!(result.exit_code, 0);
    assert!(result.output.contains("hello-runtime"));
}

#[tokio::test]
async fn runtime_executor_denies_by_policy() {
    let workspace = std::env::current_dir().expect("get current dir");
    let policy = Box::new(LocalPolicyEngine::new("read-only"));
    let guard = Box::new(
        LocalWorkspaceGuard::new(workspace.clone(), Vec::new(), Vec::new(), Vec::new())
            .expect("create workspace guard"),
    );
    let runtime = runtimes::build_runtime("builtin").expect("build runtime");

    let executor = RuntimeExecutor::new(policy, guard, runtime);
    let args = vec!["blocked".to_string()];

    let err = executor
        .execute_command("sess_exec_2", "echo", &args, 5, &workspace)
        .await
        .expect_err("read-only profile should deny ExecShell");

    assert!(
        err.to_string().contains("runtime policy denied"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn runtime_executor_denies_cwd_outside_workspace() {
    let workspace = std::env::current_dir().expect("get current dir");
    let policy = Box::new(LocalPolicyEngine::new("full-trusted"));
    let guard = Box::new(
        LocalWorkspaceGuard::new(workspace.clone(), Vec::new(), Vec::new(), Vec::new())
            .expect("create workspace guard"),
    );
    let runtime = runtimes::build_runtime("builtin").expect("build runtime");

    let executor = RuntimeExecutor::new(policy, guard, runtime);

    let outside = workspace
        .parent()
        .map(PathBuf::from)
        .expect("workspace should have parent");

    let err = executor
        .execute_command("sess_exec_3", "echo", &["x".to_string()], 5, &outside)
        .await
        .expect_err("outside cwd must be denied");

    assert!(
        err.to_string().contains("escapes workspace root"),
        "unexpected error: {err}"
    );
}
