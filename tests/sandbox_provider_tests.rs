use std::fs;
use std::sync::{Mutex, MutexGuard};

use agentd::adapters::store::sqlite::SqliteStore;
use agentd::app::{App, OutputMode, OutputOptions};
use agentd::domain::agent::AgentState;
use agentd::ports::store::StateStore;
use rusqlite::Connection;
use uuid::Uuid;

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn test_output_options() -> OutputOptions {
    OutputOptions {
        mode: OutputMode::Text,
        quiet: true,
    }
}

fn temp_db_path() -> String {
    let mut path = std::env::temp_dir();
    path.push(format!("agentd-sandbox-test-{}.db", Uuid::new_v4()));
    path.to_string_lossy().to_string()
}

fn temp_sandbox_dir() -> String {
    let mut path = std::env::temp_dir();
    path.push(format!("agentd-sandbox-{}", Uuid::new_v4()));
    path.to_string_lossy().to_string()
}

fn temp_audit_path() -> String {
    let mut path = std::env::temp_dir();
    path.push(format!("agentd-audit-{}.log", Uuid::new_v4()));
    path.to_string_lossy().to_string()
}

struct EnvGuard {
    _guard: MutexGuard<'static, ()>,
    entries: Vec<(String, Option<String>)>,
}

impl EnvGuard {
    fn set(entries: &[(&str, String)]) -> Self {
        let guard = ENV_LOCK.lock().unwrap();
        let mut saved = Vec::new();
        for (key, value) in entries {
            let old = std::env::var(key).ok();
            // SAFETY: set_var is unsafe because it involves global mutable state.
            // We use it in tests with ENV_LOCK to prevent concurrent access.
            unsafe {
                std::env::set_var(key, value);
            }
            saved.push((key.to_string(), old));
        }
        Self {
            _guard: guard,
            entries: saved,
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, old_value) in self.entries.drain(..) {
            match old_value {
                Some(v) => {
                    // SAFETY: Same as above - protected by ENV_LOCK.
                    unsafe {
                        std::env::set_var(&key, v);
                    }
                }
                None => {
                    // SAFETY: Same as above - protected by ENV_LOCK.
                    unsafe {
                        std::env::remove_var(&key);
                    }
                }
            }
        }
    }
}

#[tokio::test]
async fn sandbox_provider_rejects_disallowed_command() {
    let sandbox_dir = temp_sandbox_dir();
    let db_path = temp_db_path();

    // Create sandbox config with restrictive ACL
    let _env = EnvGuard::set(&[
        ("AGENTD_SANDBOX_RUNTIME", "process".to_string()),
        ("AGENTD_SANDBOX_WORKDIR", sandbox_dir.clone()),
        (
            "AGENTD_SANDBOX_ALLOWED_COMMANDS_JSON",
            r#"["echo", "cat"]"#.to_string(),
        ),
        ("AGENTD_SANDBOX_TRACE_COMMANDS", "true".to_string()),
        ("AGENTD_SANDBOX_TRACE_DIFF", "true".to_string()),
    ]);

    let app = App::new(db_path.clone(), test_output_options()).expect("create app");

    // Try to spawn with disallowed command "ls" (not in ["echo", "cat"])
    app.spawn("denied_cmd", "sandbox", "ls /tmp", 5, 0, None)
        .await
        .expect("spawn agent with disallowed command");

    let store = SqliteStore::new(db_path);
    let agents = store.list_agents().expect("list agents");
    assert_eq!(agents.len(), 1);
    let agent_id = agents[0].id.clone();

    // Attempt attach - should fail due to ACL
    let err = app
        .attach(&agent_id, 5, 0, false, false, None)
        .await
        .expect_err("attach should fail due to disallowed command");

    let err_msg = err.to_string();
    assert!(
        err_msg.contains("command not allowed") || err_msg.contains("not in allowed_commands"),
        "expected ACL rejection, got: {}",
        err_msg
    );

    // Verify agent failed
    let agent = store.get_agent(&agent_id).expect("get agent").unwrap();
    assert_eq!(agent.state, AgentState::Failed);
}

#[tokio::test]
async fn sandbox_provider_allows_whitelisted_command() {
    let sandbox_dir = temp_sandbox_dir();
    let db_path = temp_db_path();

    let _env = EnvGuard::set(&[
        ("AGENTD_SANDBOX_RUNTIME", "process".to_string()),
        ("AGENTD_SANDBOX_WORKDIR", sandbox_dir.clone()),
        (
            "AGENTD_SANDBOX_ALLOWED_COMMANDS_JSON",
            r#"["echo"]"#.to_string(),
        ),
        ("AGENTD_SANDBOX_TRACE_COMMANDS", "true".to_string()),
        ("AGENTD_SANDBOX_TRACE_DIFF", "false".to_string()),
    ]);

    let app = App::new(db_path.clone(), test_output_options()).expect("create app");

    app.spawn("allowed_cmd", "sandbox", "echo hello", 5, 0, None)
        .await
        .expect("spawn agent");

    let store = SqliteStore::new(db_path);
    let agents = store.list_agents().expect("list agents");
    let agent_id = agents[0].id.clone();

    // Attach should succeed
    app.attach(&agent_id, 5, 0, false, false, None)
        .await
        .expect("attach allowed command");

    // Verify agent succeeded
    let agent = store.get_agent(&agent_id).expect("get agent").unwrap();
    assert_eq!(agent.state, AgentState::Succeeded);

    // Verify logs contain command execution
    let logs = store.get_logs(&agent_id, 100).expect("get logs");
    assert!(!logs.is_empty(), "expected logs for executed command");
}

#[tokio::test]
async fn sandbox_provider_captures_filesystem_diff() {
    let sandbox_dir = temp_sandbox_dir();
    let db_path = temp_db_path();

    // Ensure sandbox dir exists
    fs::create_dir_all(&sandbox_dir).expect("create sandbox dir");

    let _env = EnvGuard::set(&[
        ("AGENTD_SANDBOX_RUNTIME", "process".to_string()),
        ("AGENTD_SANDBOX_WORKDIR", sandbox_dir.clone()),
        ("AGENTD_SANDBOX_ALLOWED_COMMANDS_JSON", r#"[]"#.to_string()),
        ("AGENTD_SANDBOX_TRACE_COMMANDS", "true".to_string()),
        ("AGENTD_SANDBOX_TRACE_DIFF", "true".to_string()),
    ]);

    let app = App::new(db_path.clone(), test_output_options()).expect("create app");

    // Command that creates a file
    app.spawn(
        "create_file",
        "sandbox",
        "echo test > output.txt",
        5,
        0,
        None,
    )
    .await
    .expect("spawn agent");

    let store = SqliteStore::new(db_path);
    let agents = store.list_agents().expect("list agents");
    let agent_id = agents[0].id.clone();

    app.attach(&agent_id, 5, 0, false, false, None)
        .await
        .expect("attach agent");

    let agent = store.get_agent(&agent_id).expect("get agent").unwrap();
    assert_eq!(agent.state, AgentState::Succeeded);

    // Check logs for diff tracing - look for escaped JSON with diff field
    let logs = store.get_logs(&agent_id, 100).expect("get logs");
    let output_logs: Vec<_> = logs
        .iter()
        .filter(|log| {
            // Look for tracing output containing escaped diff field
            // In the embedded JSON string, backslashes are escaped, so we look for \\\"diff\\\":
            log.message.contains(",\\\"diff\\\":")
        })
        .collect();

    assert!(
        !output_logs.is_empty(),
        "expected tracing output with diff in logs. Available logs: {:?}",
        logs.iter().map(|l| l.message.clone()).collect::<Vec<_>>()
    );

    // Verify diff field exists in tracing
    if let Some(log) = output_logs.first() {
        let message = &log.message;
        assert!(
            message.contains("\\\"created\\\"") && message.contains("\\\"deleted\\\""),
            "expected diff sub-fields in tracing"
        );
    }
}

#[tokio::test]
async fn sandbox_provider_empty_acl_allows_all_commands() {
    let sandbox_dir = temp_sandbox_dir();
    let db_path = temp_db_path();

    let _env = EnvGuard::set(&[
        ("AGENTD_SANDBOX_RUNTIME", "process".to_string()),
        ("AGENTD_SANDBOX_WORKDIR", sandbox_dir.clone()),
        ("AGENTD_SANDBOX_ALLOWED_COMMANDS_JSON", r#"[]"#.to_string()),
        ("AGENTD_SANDBOX_TRACE_COMMANDS", "true".to_string()),
        ("AGENTD_SANDBOX_TRACE_DIFF", "false".to_string()),
    ]);

    let app = App::new(db_path.clone(), test_output_options()).expect("create app");

    // Simple echo command should be allowed when ACL is empty
    app.spawn("echo_cmd", "sandbox", "echo hello", 5, 0, None)
        .await
        .expect("spawn agent");

    let store = SqliteStore::new(db_path);
    let agents = store.list_agents().expect("list agents");
    assert_eq!(agents.len(), 1, "should have 1 spawned agent");

    let agent = &agents[0];
    app.attach(&agent.id, 5, 0, false, false, None)
        .await
        .expect("attach with empty ACL should always succeed");

    let agent = store.get_agent(&agent.id).expect("get agent").unwrap();
    assert_eq!(
        agent.state,
        AgentState::Succeeded,
        "agent should succeed with empty ACL"
    );
}

#[tokio::test]
async fn sandbox_provider_wildcard_command_acl() {
    let sandbox_dir = temp_sandbox_dir();
    let db_path = temp_db_path();

    let _env = EnvGuard::set(&[
        ("AGENTD_SANDBOX_RUNTIME", "process".to_string()),
        ("AGENTD_SANDBOX_WORKDIR", sandbox_dir.clone()),
        (
            "AGENTD_SANDBOX_ALLOWED_COMMANDS_JSON",
            r#"["echo *"]"#.to_string(),
        ),
        ("AGENTD_SANDBOX_TRACE_COMMANDS", "true".to_string()),
        ("AGENTD_SANDBOX_TRACE_DIFF", "false".to_string()),
    ]);

    let app = App::new(db_path.clone(), test_output_options()).expect("create app");

    // "echo" with any args should be allowed
    app.spawn("echo_test", "sandbox", "echo hello world", 5, 0, None)
        .await
        .expect("spawn echo agent");

    let store = SqliteStore::new(db_path);
    let agents = store.list_agents().expect("list agents");
    let agent_id = agents[0].id.clone();

    app.attach(&agent_id, 5, 0, false, false, None)
        .await
        .expect("attach echo command");

    let agent = store.get_agent(&agent_id).expect("get agent").unwrap();
    assert_eq!(agent.state, AgentState::Succeeded);

    // Now try disallowed wildcard command
    app.spawn("rm_test", "sandbox", "rm file.txt", 5, 0, None)
        .await
        .expect("spawn rm agent");

    let agents = store.list_agents().expect("list agents");
    let agent_id = agents
        .iter()
        .find(|a| a.name == "rm_test")
        .unwrap()
        .id
        .clone();

    let err = app
        .attach(&agent_id, 5, 0, false, false, None)
        .await
        .expect_err("attach rm command should fail");

    assert!(err.to_string().contains("command not allowed"));
}

#[tokio::test]
async fn sandbox_provider_tracing_contains_metadata() {
    let sandbox_dir = temp_sandbox_dir();
    let db_path = temp_db_path();

    let _env = EnvGuard::set(&[
        ("AGENTD_SANDBOX_RUNTIME", "process".to_string()),
        ("AGENTD_SANDBOX_WORKDIR", sandbox_dir.clone()),
        ("AGENTD_SANDBOX_TRACE_COMMANDS", "true".to_string()),
        ("AGENTD_SANDBOX_TRACE_DIFF", "false".to_string()),
    ]);

    let app = App::new(db_path.clone(), test_output_options()).expect("create app");

    app.spawn("trace_test", "sandbox", "echo test", 5, 0, None)
        .await
        .expect("spawn agent");

    let store = SqliteStore::new(db_path);
    let agents = store.list_agents().expect("list agents");
    let agent_id = agents[0].id.clone();

    app.attach(&agent_id, 5, 0, false, false, None)
        .await
        .expect("attach agent");

    let logs = store.get_logs(&agent_id, 100).expect("get logs");

    // Find any log with tracing output
    let tracing_log = logs
        .iter()
        .find(|log| log.message.contains("timestamp"))
        .expect("should have tracing output");

    let msg = &tracing_log.message;

    // Verify tracing fields exist
    assert!(msg.contains("timestamp"), "missing timestamp");
    assert!(msg.contains("agent_id"), "missing agent_id");
    assert!(msg.contains("command"), "missing command");
    assert!(msg.contains("exit_code"), "missing exit_code");
    assert!(msg.contains("runtime"), "missing runtime");
    assert!(msg.contains("role"), "missing role");
    assert!(msg.contains("command_input"), "missing audit command_input");
    assert!(
        msg.contains("command_output_preview"),
        "missing audit command_output_preview"
    );
    assert!(msg.contains("workdir"), "missing workdir");
}

#[tokio::test]
async fn sandbox_provider_persists_audit_log_entries() {
    let sandbox_dir = temp_sandbox_dir();
    let db_path = temp_db_path();
    let audit_path = temp_audit_path();

    let _env = EnvGuard::set(&[
        ("AGENTD_SANDBOX_RUNTIME", "process".to_string()),
        ("AGENTD_SANDBOX_ROLE", "operator".to_string()),
        ("AGENTD_SANDBOX_WORKDIR", sandbox_dir.clone()),
        ("AGENTD_SANDBOX_AUDIT_LOG_PATH", audit_path.clone()),
        ("AGENTD_SANDBOX_TRACE_COMMANDS", "true".to_string()),
        ("AGENTD_SANDBOX_TRACE_DIFF", "false".to_string()),
    ]);

    let app = App::new(db_path.clone(), test_output_options()).expect("create app");
    app.spawn("audit_test", "sandbox", "echo audited", 5, 0, None)
        .await
        .expect("spawn audit test agent");

    let store = SqliteStore::new(db_path);
    let agents = store.list_agents().expect("list agents");
    let agent_id = agents[0].id.clone();

    app.attach(&agent_id, 5, 0, false, false, None)
        .await
        .expect("attach audit test agent");

    let audit_content = fs::read_to_string(&audit_path).expect("read audit log file");
    assert!(
        audit_content.contains("\"agent_id\""),
        "audit log should contain agent id field"
    );
    assert!(
        audit_content.contains("\"command_input\":\"echo audited\""),
        "audit log should contain command input"
    );
    assert!(
        audit_content.contains("\"allowed\":true"),
        "audit log should contain allowed=true"
    );
}

#[tokio::test]
async fn sandbox_provider_persists_audit_log_entries_in_sqlite_backend() {
    let sandbox_dir = temp_sandbox_dir();
    let db_path = temp_db_path();
    let audit_db_path = temp_audit_path();

    let _env = EnvGuard::set(&[
        ("AGENTD_SANDBOX_RUNTIME", "process".to_string()),
        ("AGENTD_SANDBOX_ROLE", "operator".to_string()),
        ("AGENTD_SANDBOX_WORKDIR", sandbox_dir.clone()),
        ("AGENTD_SANDBOX_AUDIT_LOG_PATH", audit_db_path.clone()),
        ("AGENTD_SANDBOX_AUDIT_BACKEND", "sqlite".to_string()),
        ("AGENTD_SANDBOX_TRACE_COMMANDS", "true".to_string()),
        ("AGENTD_SANDBOX_TRACE_DIFF", "false".to_string()),
    ]);

    let app = App::new(db_path.clone(), test_output_options()).expect("create app");
    app.spawn(
        "audit_sqlite_test",
        "sandbox",
        "echo sqlite-audited",
        5,
        0,
        None,
    )
    .await
    .expect("spawn sqlite audit test agent");

    let store = SqliteStore::new(db_path);
    let agents = store.list_agents().expect("list agents");
    let agent_id = agents[0].id.clone();

    app.attach(&agent_id, 5, 0, false, false, None)
        .await
        .expect("attach sqlite audit test agent");

    let conn = Connection::open(&audit_db_path).expect("open sqlite audit db");
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM security_audit_logs", [], |row| {
            row.get(0)
        })
        .expect("count audit logs");
    assert!(count >= 1, "expected at least one sqlite audit row");

    let payload: String = conn
        .query_row(
            "SELECT payload FROM security_audit_logs ORDER BY id DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("fetch latest audit payload");

    assert!(
        payload.contains("\"agent_id\""),
        "missing agent_id in payload"
    );
    assert!(
        payload.contains("\"command_input\":\"echo sqlite-audited\""),
        "missing command_input in payload"
    );
}

#[tokio::test]
async fn sandbox_provider_viewer_role_denies_execution() {
    let sandbox_dir = temp_sandbox_dir();
    let db_path = temp_db_path();

    let _env = EnvGuard::set(&[
        ("AGENTD_SANDBOX_RUNTIME", "process".to_string()),
        ("AGENTD_SANDBOX_ROLE", "viewer".to_string()),
        ("AGENTD_SANDBOX_WORKDIR", sandbox_dir.clone()),
        ("AGENTD_SANDBOX_TRACE_COMMANDS", "true".to_string()),
        ("AGENTD_SANDBOX_TRACE_DIFF", "false".to_string()),
    ]);

    let app = App::new(db_path.clone(), test_output_options()).expect("create app");

    app.spawn("viewer_denied", "sandbox", "echo blocked", 5, 0, None)
        .await
        .expect("spawn agent");

    let store = SqliteStore::new(db_path);
    let agents = store.list_agents().expect("list agents");
    let agent_id = agents[0].id.clone();

    let err = app
        .attach(&agent_id, 5, 0, false, false, None)
        .await
        .expect_err("viewer role should not execute commands");

    assert!(
        err.to_string().contains("denied for role 'viewer'"),
        "unexpected error: {}",
        err
    );
}
