use agentd::adapters::store::sqlite::SqliteStore;
use agentd::domain::runtime_audit::{
    RuntimeArtifactInsert, RuntimeEventInsert, RuntimeSessionRecord,
};
use agentd::ports::store::StateStore;
use chrono::Utc;
use uuid::Uuid;

fn temp_db_path() -> String {
    let mut path = std::env::temp_dir();
    path.push(format!("agentd-runtime-store-{}.db", Uuid::new_v4()));
    path.to_string_lossy().to_string()
}

#[test]
fn runtime_store_persists_session_events_and_artifacts() {
    let store = SqliteStore::new(temp_db_path());
    store.init().expect("init sqlite schema");

    let session = RuntimeSessionRecord {
        session_id: "sess_store_1".to_string(),
        mode: "worktree".to_string(),
        workspace_dir: "./workspace".to_string(),
        repo_root: Some("./repo".to_string()),
        base_commit: Some("abc123".to_string()),
        branch_name: Some("agentd/sess_store_1".to_string()),
        permissions_profile: "dev-safe".to_string(),
        env_profile: "default".to_string(),
        log_path: "./.agentd/runtime/events.jsonl".to_string(),
        created_at: Utc::now(),
        closed_at: None,
    };

    store
        .create_runtime_session(&session)
        .expect("create runtime session");

    store
        .append_runtime_event(&RuntimeEventInsert {
            ts: Utc::now(),
            session_id: session.session_id.clone(),
            event_type: "command.executed".to_string(),
            command: Some("echo".to_string()),
            cwd: Some("./workspace".to_string()),
            exit_code: Some(0),
            payload: "{\"summary\":\"ok\"}".to_string(),
        })
        .expect("append runtime event");

    store
        .append_runtime_artifact(&RuntimeArtifactInsert {
            ts: Utc::now(),
            session_id: session.session_id.clone(),
            artifact_type: "raw_log".to_string(),
            path: "./.agentd/runtime/logs/sess_store_1.log".to_string(),
            metadata: Some("{\"size\":128}".to_string()),
        })
        .expect("append runtime artifact");

    let loaded = store
        .get_runtime_session(&session.session_id)
        .expect("get runtime session")
        .expect("runtime session must exist");
    assert_eq!(loaded.session_id, session.session_id);
    assert!(loaded.closed_at.is_none(), "session should initially be open");

    let events = store
        .list_runtime_events(&session.session_id, 10)
        .expect("list runtime events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, "command.executed");
    assert_eq!(events[0].command.as_deref(), Some("echo"));

    let artifacts = store
        .list_runtime_artifacts(&session.session_id, 10)
        .expect("list runtime artifacts");
    assert_eq!(artifacts.len(), 1);
    assert_eq!(artifacts[0].artifact_type, "raw_log");

    store
        .close_runtime_session(&session.session_id)
        .expect("close runtime session");
    let closed = store
        .get_runtime_session(&session.session_id)
        .expect("reload runtime session")
        .expect("runtime session must still exist");
    assert!(closed.closed_at.is_some(), "session should be closed");
}
