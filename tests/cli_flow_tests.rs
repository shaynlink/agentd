use std::process::Command;

use agentd::adapters::store::sqlite::SqliteStore;
use agentd::ports::store::StateStore;
use uuid::Uuid;

fn temp_db_path() -> String {
    let mut path = std::env::temp_dir();
    path.push(format!("agentd-test-{}.db", Uuid::new_v4()));
    path.to_string_lossy().to_string()
}

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

#[test]
fn cli_spawn_status_attach_logs_list_flow() {
    let db_path = temp_db_path();

    let spawn = run_cli(&[
        "--db-path",
        &db_path,
        "spawn",
        "--name",
        "cli-flow",
        "--provider",
        "mock",
        "--prompt",
        "hello from cli flow",
        "--timeout-secs",
        "5",
        "--retries",
        "0",
    ]);
    assert!(spawn.status.success(), "spawn should succeed");
    let spawn_out = stdout_text(&spawn);
    assert!(spawn_out.contains("spawned agent 'cli-flow'"));

    let store = SqliteStore::new(db_path.clone());
    let agents = store.list_agents().expect("list agents after spawn");
    assert_eq!(agents.len(), 1, "expected one agent after spawn");
    let agent_id = agents[0].id.clone();

    let status = run_cli(&["--db-path", &db_path, "status", "--id", &agent_id]);
    assert!(status.status.success(), "status should succeed");
    let status_out = stdout_text(&status);
    assert!(status_out.contains(&agent_id));
    assert!(status_out.contains("pending"));

    let attach = run_cli(&[
        "--db-path",
        &db_path,
        "attach",
        "--id",
        &agent_id,
        "--timeout-secs",
        "5",
        "--retries",
        "0",
    ]);
    assert!(attach.status.success(), "attach should succeed");
    let attach_out = stdout_text(&attach);
    assert!(attach_out.contains("succeeded"));

    let logs = run_cli(&[
        "--db-path",
        &db_path,
        "logs",
        "--id",
        &agent_id,
        "--limit",
        "20",
    ]);
    assert!(logs.status.success(), "logs should succeed");
    let logs_out = stdout_text(&logs);
    assert!(logs_out.contains("agent spawned"));
    assert!(logs_out.contains("attach requested"));

    let list = run_cli(&["--db-path", &db_path, "list"]);
    assert!(list.status.success(), "list should succeed");
    let list_out = stdout_text(&list);
    assert!(list_out.contains(&agent_id));
    assert!(list_out.contains("succeeded"));
}
