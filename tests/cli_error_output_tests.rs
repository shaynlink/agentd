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

fn stderr_text(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).trim().to_string()
}

fn temp_db_path() -> String {
    let mut path = std::env::temp_dir();
    path.push(format!("agentd-test-{}.db", Uuid::new_v4()));
    path.to_string_lossy().to_string()
}

#[test]
fn status_unknown_agent_returns_not_found_category() {
    let db_path = temp_db_path();
    let out = run_cli(&[
        "--db-path",
        &db_path,
        "status",
        "--id",
        "00000000-0000-0000-0000-000000000000",
    ]);
    assert!(!out.status.success(), "status should fail on unknown agent");

    let err = stderr_text(&out);
    let payload: Value = serde_json::from_str(&err).expect("stderr should be JSON");
    assert_eq!(
        payload.get("category").and_then(Value::as_str),
        Some("not_found")
    );
    assert!(
        payload
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("agent not found"),
        "unexpected error payload: {err}"
    );
}

#[test]
fn schedule_run_at_invalid_timestamp_returns_validation_category() {
    let out = run_cli(&[
        "schedule-run-at",
        "--name",
        "invalid-ts",
        "--prompt",
        "x",
        "--run-at",
        "not-a-date",
    ]);
    assert!(
        !out.status.success(),
        "schedule-run-at should fail on invalid timestamp"
    );

    let err = stderr_text(&out);
    let payload: Value = serde_json::from_str(&err).expect("stderr should be JSON");
    assert_eq!(
        payload.get("category").and_then(Value::as_str),
        Some("validation")
    );
    assert!(
        payload
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("invalid run_at"),
        "unexpected error payload: {err}"
    );
}
