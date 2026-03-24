use chrono::Utc;

use agentd::domain::runtime_session::{RuntimeMode, RuntimeSession, RuntimeSessionCreateRequest};

#[test]
fn runtime_session_create_request_has_safe_defaults() {
    let req = RuntimeSessionCreateRequest::default();

    assert_eq!(req.mode, RuntimeMode::Worktree);
    assert_eq!(req.permissions_profile, "dev-safe");
    assert_eq!(req.env_profile, "default");
    assert!(req.workspace_dir.to_string_lossy().contains(".agentd/runtime"));
}

#[test]
fn runtime_session_roundtrips_json() {
    let req = RuntimeSessionCreateRequest::default();
    let now = Utc::now();
    let session = RuntimeSession::from_request(
        "sess_123".to_string(),
        now,
        req,
        Some("abc123".to_string()),
        Some("agentd/sess_123".to_string()),
    );

    let raw = serde_json::to_string(&session).expect("serialize runtime session");
    let decoded: RuntimeSession = serde_json::from_str(&raw).expect("deserialize runtime session");

    assert_eq!(decoded.session_id, "sess_123");
    assert_eq!(decoded.base_commit.as_deref(), Some("abc123"));
    assert_eq!(decoded.branch_name.as_deref(), Some("agentd/sess_123"));
    assert_eq!(decoded.mode, RuntimeMode::Worktree);
}
