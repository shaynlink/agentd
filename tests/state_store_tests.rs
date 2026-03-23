use std::path::PathBuf;

use agentd::adapters::store::sqlite::SqliteStore;
use agentd::domain::agent::{AgentRecord, AgentState};
use agentd::ports::store::StateStore;
use chrono::Utc;
use uuid::Uuid;

fn temp_db_path() -> String {
    let mut path = PathBuf::from(std::env::temp_dir());
    path.push(format!("agentd-test-{}.db", Uuid::new_v4()));
    path.to_string_lossy().to_string()
}

fn make_agent(id: &str, state: AgentState) -> AgentRecord {
    let now = Utc::now();
    AgentRecord {
        id: id.to_string(),
        name: "test-agent".to_string(),
        provider: "mock".to_string(),
        prompt: "hello".to_string(),
        state,
        created_at: now,
        updated_at: now,
        attempts: 0,
    }
}

#[test]
fn enforces_terminal_state_transitions() {
    let store = SqliteStore::new(temp_db_path());
    store.init().expect("init sqlite schema");

    let agent = make_agent("a1", AgentState::Pending);
    store.create_agent(&agent).expect("insert agent");

    store
        .update_state(&agent.id, AgentState::Running)
        .expect("pending -> running must pass");
    store
        .update_state(&agent.id, AgentState::Succeeded)
        .expect("running -> succeeded must pass");

    let err = store
        .update_state(&agent.id, AgentState::Running)
        .expect_err("succeeded -> running must fail");
    assert!(
        err.to_string().contains("invalid state transition"),
        "unexpected error: {err}"
    );
}

#[test]
fn recovers_running_agents_and_clears_locks() {
    let store = SqliteStore::new(temp_db_path());
    store.init().expect("init sqlite schema");

    let agent = make_agent("a2", AgentState::Pending);
    store.create_agent(&agent).expect("insert agent");
    store
        .update_state(&agent.id, AgentState::Running)
        .expect("pending -> running");

    let got_lock = store
        .try_acquire_execution_lock(&agent.id, "test-owner")
        .expect("acquire initial lock");
    assert!(got_lock, "first lock acquisition should succeed");

    let recovered = store
        .recover_stuck_executions()
        .expect("recover stuck executions");
    assert_eq!(recovered.len(), 1, "expected one recovered agent");
    assert_eq!(recovered[0], agent.id);

    let fetched = store
        .get_agent(&agent.id)
        .expect("get recovered agent")
        .expect("agent should exist");
    assert_eq!(fetched.state, AgentState::Pending);

    let relock = store
        .try_acquire_execution_lock(&agent.id, "new-owner")
        .expect("acquire lock after recovery");
    assert!(relock, "lock should be free after recovery");
}
