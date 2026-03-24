use std::path::PathBuf;

use agentd::adapters::security::local_securable::LocalSecurable;
use agentd::config::SandboxProviderConfig;
use agentd::ports::securable::{AuditEventFilters, SecurablePort};
use serde_json::json;
use uuid::Uuid;

fn temp_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("agentd-{name}-{}", Uuid::new_v4()));
    path
}

fn test_sandbox_config(audit_backend: &str, audit_log_path: PathBuf) -> SandboxProviderConfig {
    SandboxProviderConfig {
        runtime: "process".to_string(),
        role: "operator".to_string(),
        workdir: temp_path("workdir"),
        audit_log_path,
        audit_backend: audit_backend.to_string(),
        allowed_commands: Vec::new(),
        allowed_read_paths: Vec::new(),
        allowed_write_paths: Vec::new(),
        trace_commands: false,
        trace_diff: false,
        vibe_path: "vibe".to_string(),
    }
}

#[tokio::test]
async fn sqlite_audit_filters_apply_at_query_time() {
    let audit_db_path = temp_path("audit-sqlite.db");
    let config = test_sandbox_config("sqlite", audit_db_path.clone());
    let securable = LocalSecurable::new(&config);

    let event_1 = json!({
        "ts": "2026-01-01T10:00:00Z",
        "agent_id": "agent-a",
        "role": "operator",
        "runtime": "process",
        "allowed": true,
        "command_input": "echo one"
    });
    let event_2 = json!({
        "ts": "2026-01-01T11:00:00Z",
        "agent_id": "agent-b",
        "role": "viewer",
        "runtime": "docker",
        "allowed": false,
        "command_input": "echo two"
    });
    let event_3 = json!({
        "ts": "2026-01-01T12:00:00Z",
        "agent_id": "agent-a",
        "role": "admin",
        "runtime": "process",
        "allowed": true,
        "command_input": "echo three"
    });

    securable
        .log_audit_event(&event_1.to_string())
        .await
        .expect("write event 1");
    securable
        .log_audit_event(&event_2.to_string())
        .await
        .expect("write event 2");
    securable
        .log_audit_event(&event_3.to_string())
        .await
        .expect("write event 3");

    let filtered = securable
        .list_audit_events(
            50,
            AuditEventFilters {
                role: None,
                allowed: Some(true),
                runtime: Some("process"),
                agent_id: Some("agent-a"),
                since: Some("2026-01-01T10:30:00Z"),
                until: Some("2026-01-01T12:00:00Z"),
            },
        )
        .await
        .expect("list filtered sqlite events");

    assert_eq!(filtered.len(), 1, "expected only one filtered sqlite event");
    assert!(
        filtered[0].contains("echo three"),
        "expected latest matching command in filtered event: {}",
        filtered[0]
    );

    let ordered = securable
        .list_audit_events(
            50,
            AuditEventFilters {
                runtime: Some("process"),
                ..Default::default()
            },
        )
        .await
        .expect("list ordered sqlite events");

    assert_eq!(ordered.len(), 2, "expected two process runtime events");
    assert!(
        ordered[0].contains("echo three") && ordered[1].contains("echo one"),
        "expected newest-first order for sqlite events: {:?}",
        ordered
    );
}

#[tokio::test]
async fn file_audit_filters_apply_for_json_lines() {
    let audit_log_path = temp_path("audit-file.log");
    let config = test_sandbox_config("file", audit_log_path.clone());
    let securable = LocalSecurable::new(&config);

    let event_1 = json!({
        "ts": "2026-01-02T08:00:00Z",
        "agent_id": "agent-x",
        "role": "operator",
        "runtime": "process",
        "allowed": true,
        "command_input": "echo alpha"
    });
    let event_2 = json!({
        "ts": "2026-01-02T09:00:00Z",
        "agent_id": "agent-y",
        "role": "viewer",
        "runtime": "docker",
        "allowed": false,
        "command_input": "echo beta"
    });

    securable
        .log_audit_event(&event_1.to_string())
        .await
        .expect("write file event 1");
    securable
        .log_audit_event(&event_2.to_string())
        .await
        .expect("write file event 2");

    let filtered = securable
        .list_audit_events(
            10,
            AuditEventFilters {
                role: Some("viewer"),
                allowed: Some(false),
                runtime: Some("docker"),
                agent_id: Some("agent-y"),
                since: Some("2026-01-02T08:30:00Z"),
                until: Some("2026-01-02T09:30:00Z"),
            },
        )
        .await
        .expect("list filtered file events");

    assert_eq!(filtered.len(), 1, "expected one filtered file event");
    assert!(
        filtered[0].contains("echo beta"),
        "expected matching file event payload: {}",
        filtered[0]
    );
}
