use std::path::PathBuf;
use std::sync::Mutex;

use agentd::adapters::store::sqlite::SqliteStore;
use agentd::app::App;
use agentd::domain::agent::AgentState;
use agentd::ports::store::StateStore;
use serde_json::Value;
use uuid::Uuid;

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn temp_db_path() -> String {
    let mut path = PathBuf::from(std::env::temp_dir());
    path.push(format!("agentd-test-{}.db", Uuid::new_v4()));
    path.to_string_lossy().to_string()
}

fn temp_runtime_dir() -> String {
    let mut path = PathBuf::from(std::env::temp_dir());
    path.push(format!("agentd-runtime-{}", Uuid::new_v4()));
    path.to_string_lossy().to_string()
}

struct EnvGuard {
    entries: Vec<(String, Option<String>)>,
}

impl EnvGuard {
    fn set(entries: &[(&str, String)]) -> Self {
        let mut saved = Vec::with_capacity(entries.len());
        for (key, value) in entries {
            let key_string = (*key).to_string();
            let previous = std::env::var(key).ok();
            // Tests run under a global lock to avoid concurrent env access.
            unsafe {
                std::env::set_var(&key_string, value);
            }
            saved.push((key_string, previous));
        }

        Self { entries: saved }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, previous) in &self.entries {
            match previous {
                Some(value) => {
                    // Restores prior process env after each test.
                    unsafe {
                        std::env::set_var(key, value);
                    }
                }
                None => {
                    // Removes keys that were not originally present.
                    unsafe {
                        std::env::remove_var(key);
                    }
                }
            }
        }
    }
}

#[tokio::test]
async fn attach_retries_and_fails_on_provider_error() {
    let _lock = ENV_LOCK.lock().expect("lock test env");
    let _env = EnvGuard::set(&[
        ("AGENTD_CLI_COMMAND", "/usr/bin/false".to_string()),
        ("AGENTD_CLI_ARGS_JSON", "[]".to_string()),
        ("AGENTD_CLI_RUNTIME_DIR", temp_runtime_dir()),
    ]);

    let db_path = temp_db_path();
    let app = App::new(db_path.clone()).expect("create app");

    app.spawn("failing", "cli", "ignored", 2, 2)
        .await
        .expect("spawn failing cli agent");

    let store = SqliteStore::new(db_path.clone());
    let agents = store.list_agents().expect("list agents");
    assert_eq!(agents.len(), 1, "expected one agent");
    let agent_id = agents[0].id.clone();

    let err = app
        .attach(&agent_id, 2, 2, false, false)
        .await
        .expect_err("attach should fail after retries");
    assert!(
        err.to_string().contains("failed after 3 attempt(s)"),
        "unexpected error: {err}"
    );

    let agent = store
        .get_agent(&agent_id)
        .expect("get agent")
        .expect("agent exists");
    assert_eq!(agent.state, AgentState::Failed);
    assert_eq!(agent.attempts, 3);

    let logs = store.get_logs(&agent_id, 20).expect("get logs");
    let provider_error_count = logs
        .iter()
        .filter(|l| l.message.contains("\"category\":\"provider_error\""))
        .count();
    assert_eq!(provider_error_count, 3);

    let structured = logs
        .iter()
        .find(|l| l.message.contains("\"category\":\"provider_error\""))
        .expect("provider error structured log exists");
    let parsed: Value =
        serde_json::from_str(&structured.message).expect("provider error log should be json");
    assert_eq!(
        parsed.get("context").and_then(Value::as_str),
        Some("attach")
    );
    assert_eq!(parsed.get("provider").and_then(Value::as_str), Some("cli"));
    assert_eq!(
        parsed.get("category").and_then(Value::as_str),
        Some("provider_error")
    );
}

#[tokio::test]
async fn attach_retries_and_times_out() {
    let _lock = ENV_LOCK.lock().expect("lock test env");
    let _env = EnvGuard::set(&[
        ("AGENTD_CLI_COMMAND", "/bin/sh".to_string()),
        ("AGENTD_CLI_ARGS_JSON", "[\"-c\",\"sleep 2\"]".to_string()),
        ("AGENTD_CLI_RUNTIME_DIR", temp_runtime_dir()),
    ]);

    let db_path = temp_db_path();
    let app = App::new(db_path.clone()).expect("create app");

    app.spawn("timeout", "cli", "ignored", 1, 1)
        .await
        .expect("spawn timeout cli agent");

    let store = SqliteStore::new(db_path.clone());
    let agents = store.list_agents().expect("list agents");
    assert_eq!(agents.len(), 1, "expected one agent");
    let agent_id = agents[0].id.clone();

    let err = app
        .attach(&agent_id, 1, 1, false, false)
        .await
        .expect_err("attach should time out after retries");
    assert!(
        err.to_string().contains("timed out after 2 attempt(s)"),
        "unexpected error: {err}"
    );

    let agent = store
        .get_agent(&agent_id)
        .expect("get agent")
        .expect("agent exists");
    assert_eq!(agent.state, AgentState::TimedOut);
    assert_eq!(agent.attempts, 2);

    let logs = store.get_logs(&agent_id, 20).expect("get logs");
    let timeout_count = logs
        .iter()
        .filter(|l| l.message.contains("\"category\":\"timeout\""))
        .count();
    assert_eq!(timeout_count, 2);

    let structured = logs
        .iter()
        .find(|l| l.message.contains("\"category\":\"timeout\""))
        .expect("timeout structured log exists");
    let parsed: Value =
        serde_json::from_str(&structured.message).expect("timeout log should be json");
    assert_eq!(
        parsed.get("context").and_then(Value::as_str),
        Some("attach")
    );
    assert_eq!(parsed.get("provider").and_then(Value::as_str), Some("cli"));
    assert_eq!(
        parsed.get("category").and_then(Value::as_str),
        Some("timeout")
    );
    assert_eq!(
        parsed.get("message").and_then(Value::as_str),
        Some("execution timed out")
    );
}
