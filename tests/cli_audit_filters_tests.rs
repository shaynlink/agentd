use std::path::PathBuf;
use std::process::Command;

use agentd::adapters::security::local_securable::LocalSecurable;
use agentd::config::SandboxProviderConfig;
use agentd::ports::securable::SecurablePort;
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

fn test_sandbox_config(audit_db_path: PathBuf) -> SandboxProviderConfig {
    SandboxProviderConfig {
        runtime: "process".to_string(),
        role: "operator".to_string(),
        workdir: temp_path("workdir"),
        audit_log_path: audit_db_path,
        audit_backend: "sqlite".to_string(),
        allowed_commands: Vec::new(),
        allowed_read_paths: Vec::new(),
        allowed_write_paths: Vec::new(),
        trace_commands: false,
        trace_diff: false,
        vibe_path: "vibe".to_string(),
    }
}

#[tokio::test]
async fn cli_audit_list_applies_filters_end_to_end() {
    let db_path = temp_db_path();
    let audit_db_path = temp_path("audit-e2e.db");
    let cfg = test_sandbox_config(audit_db_path.clone());
    let securable = LocalSecurable::new(&cfg);

    securable
        .log_audit_event(
            &serde_json::json!({
                "ts": "2026-02-01T10:00:00Z",
                "agent_id": "agent-x",
                "role": "operator",
                "runtime": "process",
                "allowed": true,
                "command_input": "echo alpha"
            })
            .to_string(),
        )
        .await
        .expect("write audit event 1");

    securable
        .log_audit_event(
            &serde_json::json!({
                "ts": "2026-02-01T11:00:00Z",
                "agent_id": "agent-y",
                "role": "viewer",
                "runtime": "docker",
                "allowed": false,
                "command_input": "echo beta"
            })
            .to_string(),
        )
        .await
        .expect("write audit event 2");

    securable
        .log_audit_event(
            &serde_json::json!({
                "ts": "2026-02-01T12:00:00Z",
                "agent_id": "agent-x",
                "role": "operator",
                "runtime": "process",
                "allowed": true,
                "command_input": "echo gamma"
            })
            .to_string(),
        )
        .await
        .expect("write audit event 3");

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
            "--output",
            "json",
            "audit-list",
            "--limit",
            "50",
            "--runtime",
            "process",
            "--agent-id",
            "agent-x",
            "--allowed",
            "true",
            "--since",
            "2026-02-01T11:30:00Z",
            "--until",
            "2026-02-01T12:30:00Z",
        ],
        &envs,
    );
    assert!(
        out.status.success(),
        "audit-list should succeed, stderr: {}",
        stderr_text(&out)
    );

    let payload: Value = serde_json::from_str(&stdout_text(&out)).expect("stdout should be JSON");
    let data = payload
        .get("data")
        .expect("json payload should contain data");
    let count = data
        .get("count")
        .and_then(Value::as_u64)
        .expect("data.count should be present") as usize;
    let events = data
        .get("events")
        .and_then(Value::as_array)
        .expect("data.events should be an array");

    assert_eq!(count, 1, "expected exactly one matching audit event");
    assert_eq!(events.len(), 1, "events length should match count");

    let first = &events[0];
    assert_eq!(
        first.get("command_input").and_then(Value::as_str),
        Some("echo gamma")
    );
    assert_eq!(
        first.get("agent_id").and_then(Value::as_str),
        Some("agent-x")
    );
    assert_eq!(
        first.get("runtime").and_then(Value::as_str),
        Some("process")
    );
    assert_eq!(first.get("allowed").and_then(Value::as_bool), Some(true));
}

#[test]
fn cli_audit_list_invalid_since_returns_validation_error() {
    let db_path = temp_db_path();
    let audit_path = temp_path("audit-invalid-since.db");
    let audit_path_string = audit_path.to_string_lossy().to_string();
    let out = run_cli_with_env(
        &[
            "--db-path",
            &db_path,
            "audit-list",
            "--since",
            "not-a-timestamp",
            "--limit",
            "1",
        ],
        &[
            ("AGENTD_SANDBOX_AUDIT_BACKEND", "sqlite"),
            ("AGENTD_SANDBOX_AUDIT_LOG_PATH", audit_path_string.as_str()),
        ],
    );
    assert!(
        !out.status.success(),
        "audit-list should fail on invalid --since"
    );

    let err: Value = serde_json::from_str(&stderr_text(&out)).expect("stderr should be JSON");
    assert_eq!(
        err.get("category").and_then(Value::as_str),
        Some("validation")
    );
    assert!(
        err.get("message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("invalid --since timestamp"),
        "unexpected error payload: {}",
        stderr_text(&out)
    );
}

#[tokio::test]
async fn cli_audit_list_role_and_runtime_match_case_insensitively() {
    let db_path = temp_db_path();
    let audit_db_path = temp_path("audit-caseinsensitive.db");
    let cfg = test_sandbox_config(audit_db_path.clone());
    let securable = LocalSecurable::new(&cfg);

    securable
        .log_audit_event(
            &serde_json::json!({
                "ts": "2026-03-01T08:00:00Z",
                "agent_id": "agent-p",
                "role": "operator",
                "runtime": "process",
                "allowed": true,
                "command_input": "echo proc"
            })
            .to_string(),
        )
        .await
        .expect("write process event");

    securable
        .log_audit_event(
            &serde_json::json!({
                "ts": "2026-03-01T09:00:00Z",
                "agent_id": "agent-d",
                "role": "viewer",
                "runtime": "docker",
                "allowed": false,
                "command_input": "echo docker"
            })
            .to_string(),
        )
        .await
        .expect("write docker event");

    securable
        .log_audit_event(
            &serde_json::json!({
                "ts": "2026-03-01T10:00:00Z",
                "agent_id": "agent-a",
                "role": "admin",
                "runtime": "process",
                "allowed": true,
                "command_input": "echo admin"
            })
            .to_string(),
        )
        .await
        .expect("write admin event");

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
            "--output",
            "json",
            "audit-list",
            "--limit",
            "50",
            "--role",
            "OPERATOR",
            "--runtime",
            "PROCESS",
        ],
        &envs,
    );
    assert!(
        out.status.success(),
        "audit-list with uppercase role/runtime should succeed, stderr: {}",
        stderr_text(&out)
    );

    let payload: Value = serde_json::from_str(&stdout_text(&out)).expect("stdout should be JSON");
    let data = payload
        .get("data")
        .expect("json payload should contain data");
    let events = data
        .get("events")
        .and_then(Value::as_array)
        .expect("data.events should be an array");

    assert_eq!(
        events.len(),
        1,
        "case-insensitive role/runtime filter should match exactly one event"
    );
    let first = &events[0];
    assert_eq!(
        first.get("command_input").and_then(Value::as_str),
        Some("echo proc"),
        "expected filtered event with lowercase role=operator and runtime=process"
    );
}
