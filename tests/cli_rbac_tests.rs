use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;
use uuid::Uuid;

fn bin_path() -> &'static str {
    env!("CARGO_BIN_EXE_agentd")
}

fn run_cli_with_env(args: &[&str], envs: &[(&str, &str)]) -> std::process::Output {
    let mut cmd = Command::new(bin_path());
    cmd.args(args);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    cmd.output().expect("run agentd cli command")
}

fn stdout_text(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr_text(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).trim().to_string()
}

fn temp_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("agentd-{name}-{}", Uuid::new_v4()));
    path
}

fn temp_db_path() -> String {
    temp_path("state.db").to_string_lossy().to_string()
}

#[test]
fn cli_rbac_create_and_list_end_to_end() {
    let db_path = temp_db_path();
    let audit_db_path = temp_path("rbac-audit.db");
    let audit_db_path_string = audit_db_path.to_string_lossy().to_string();
    let envs = [
        ("AGENTD_SANDBOX_AUDIT_BACKEND", "sqlite"),
        (
            "AGENTD_SANDBOX_AUDIT_LOG_PATH",
            audit_db_path_string.as_str(),
        ),
    ];

    let create_role = run_cli_with_env(
        &[
            "--db-path",
            &db_path,
            "--output",
            "json",
            "rbac-create-role",
            "--name",
            "deployer",
            "--description",
            "Deploys production workloads",
        ],
        &envs,
    );
    assert!(
        create_role.status.success(),
        "rbac-create-role should succeed, stderr: {}",
        stderr_text(&create_role)
    );

    let create_policy = run_cli_with_env(
        &[
            "--db-path",
            &db_path,
            "--output",
            "json",
            "rbac-create-policy",
            "--name",
            "deployer.command.execute.allow.deploy",
            "--resource-type",
            "command",
            "--action",
            "execute",
            "--resource-pattern",
            "deploy*",
            "--effect",
            "allow",
        ],
        &envs,
    );
    assert!(
        create_policy.status.success(),
        "rbac-create-policy should succeed, stderr: {}",
        stderr_text(&create_policy)
    );

    let bind_role = run_cli_with_env(
        &[
            "--db-path",
            &db_path,
            "--output",
            "json",
            "rbac-bind-role",
            "--subject-type",
            "runtime_role",
            "--subject",
            "release-engineer",
            "--role",
            "deployer",
        ],
        &envs,
    );
    assert!(
        bind_role.status.success(),
        "rbac-bind-role should succeed, stderr: {}",
        stderr_text(&bind_role)
    );

    let attach_policy = run_cli_with_env(
        &[
            "--db-path",
            &db_path,
            "--output",
            "json",
            "rbac-attach-policy",
            "--role",
            "deployer",
            "--policy",
            "deployer.command.execute.allow.deploy",
        ],
        &envs,
    );
    assert!(
        attach_policy.status.success(),
        "rbac-attach-policy should succeed, stderr: {}",
        stderr_text(&attach_policy)
    );

    let list = run_cli_with_env(
        &["--db-path", &db_path, "--output", "json", "rbac-list"],
        &envs,
    );
    assert!(
        list.status.success(),
        "rbac-list should succeed, stderr: {}",
        stderr_text(&list)
    );

    let payload: Value =
        serde_json::from_str(&stdout_text(&list)).expect("rbac-list stdout should be JSON");
    let data = payload
        .get("data")
        .expect("rbac-list json payload should contain data");

    let roles = data
        .get("roles")
        .and_then(Value::as_array)
        .expect("data.roles should be an array");
    let policies = data
        .get("policies")
        .and_then(Value::as_array)
        .expect("data.policies should be an array");
    let bindings = data
        .get("bindings")
        .and_then(Value::as_array)
        .expect("data.bindings should be an array");
    let role_policies = data
        .get("role_policies")
        .and_then(Value::as_array)
        .expect("data.role_policies should be an array");

    assert!(
        roles.iter().any(|r| {
            r.get("name").and_then(Value::as_str) == Some("deployer")
                && r.get("description").and_then(Value::as_str)
                    == Some("Deploys production workloads")
        }),
        "expected custom role in RBAC list"
    );

    assert!(
        policies.iter().any(|p| {
            p.get("name").and_then(Value::as_str)
                == Some("deployer.command.execute.allow.deploy")
                && p.get("resource_type").and_then(Value::as_str) == Some("command")
                && p.get("action").and_then(Value::as_str) == Some("execute")
                && p.get("resource_pattern").and_then(Value::as_str) == Some("deploy*")
                && p.get("effect").and_then(Value::as_str) == Some("allow")
        }),
        "expected custom policy in RBAC list"
    );

    assert!(
        bindings.iter().any(|b| {
            b.get("subject_type").and_then(Value::as_str) == Some("runtime_role")
                && b.get("subject").and_then(Value::as_str) == Some("release-engineer")
                && b.get("role").and_then(Value::as_str) == Some("deployer")
        }),
        "expected custom binding in RBAC list"
    );

    assert!(
        role_policies.iter().any(|rp| {
            rp.get("role").and_then(Value::as_str) == Some("deployer")
                && rp.get("policy").and_then(Value::as_str)
                    == Some("deployer.command.execute.allow.deploy")
        }),
        "expected custom role-policy attachment in RBAC list"
    );
}

#[test]
fn cli_rbac_bind_unknown_role_returns_not_found() {
    let db_path = temp_db_path();
    let audit_db_path = temp_path("rbac-bind-unknown.db");
    let audit_db_path_string = audit_db_path.to_string_lossy().to_string();
    let envs = [
        ("AGENTD_SANDBOX_AUDIT_BACKEND", "sqlite"),
        (
            "AGENTD_SANDBOX_AUDIT_LOG_PATH",
            audit_db_path_string.as_str(),
        ),
    ];

    let out = run_cli_with_env(
        &[
            "--db-path",
            &db_path,
            "rbac-bind-role",
            "--subject-type",
            "runtime_role",
            "--subject",
            "qa-engineer",
            "--role",
            "role-that-does-not-exist",
        ],
        &envs,
    );
    assert!(!out.status.success(), "rbac-bind-role should fail on unknown role");

    let err: Value = serde_json::from_str(&stderr_text(&out)).expect("stderr should be JSON");
    assert_eq!(
        err.get("category").and_then(Value::as_str),
        Some("not_found")
    );
    assert!(
        err.get("message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("RBAC role not found"),
        "unexpected error payload: {}",
        stderr_text(&out)
    );
}

#[test]
fn cli_rbac_attach_unknown_policy_returns_not_found() {
    let db_path = temp_db_path();
    let audit_db_path = temp_path("rbac-attach-unknown-policy.db");
    let audit_db_path_string = audit_db_path.to_string_lossy().to_string();
    let envs = [
        ("AGENTD_SANDBOX_AUDIT_BACKEND", "sqlite"),
        (
            "AGENTD_SANDBOX_AUDIT_LOG_PATH",
            audit_db_path_string.as_str(),
        ),
    ];

    let create_role = run_cli_with_env(
        &[
            "--db-path",
            &db_path,
            "rbac-create-role",
            "--name",
            "deployer",
        ],
        &envs,
    );
    assert!(create_role.status.success(), "rbac-create-role should succeed");

    let out = run_cli_with_env(
        &[
            "--db-path",
            &db_path,
            "rbac-attach-policy",
            "--role",
            "deployer",
            "--policy",
            "missing.policy",
        ],
        &envs,
    );
    assert!(
        !out.status.success(),
        "rbac-attach-policy should fail on unknown policy"
    );

    let err: Value = serde_json::from_str(&stderr_text(&out)).expect("stderr should be JSON");
    assert_eq!(
        err.get("category").and_then(Value::as_str),
        Some("not_found")
    );
    assert!(
        err.get("message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("RBAC policy not found"),
        "unexpected error payload: {}",
        stderr_text(&out)
    );
}
