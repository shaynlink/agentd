use std::path::PathBuf;

use agentd::adapters::security::local_policy::{LocalPolicyConfig, LocalPolicyEngine};
use agentd::domain::capability::Capability;
use agentd::ports::policy::{PolicyPort, RuntimeAction};

#[tokio::test]
async fn dev_safe_allows_git_read_and_denies_merge() {
    let engine = LocalPolicyEngine::new("dev-safe");

    let mut git_status = RuntimeAction::for_capability(Capability::ExecGitRead, PathBuf::from("."));
    git_status.command = Some("git".to_string());
    git_status.args = vec!["status".to_string()];

    let allowed = engine
        .evaluate("sess_1", &git_status)
        .await
        .expect("evaluate git status");
    assert!(allowed.effect.is_allowed(), "git read should be allowed");

    let merge_action = RuntimeAction::for_capability(Capability::MergeBranch, PathBuf::from("."));
    let denied = engine
        .evaluate("sess_1", &merge_action)
        .await
        .expect("evaluate merge");
    assert!(!denied.effect.is_allowed(), "merge should be denied in dev-safe");
}

#[tokio::test]
async fn read_only_denies_write_capability() {
    let engine = LocalPolicyEngine::new("read-only");
    let action = RuntimeAction::for_capability(Capability::WriteFile, PathBuf::from("."));

    let decision = engine
        .evaluate("sess_2", &action)
        .await
        .expect("evaluate write");

    assert!(!decision.effect.is_allowed(), "write should be denied");
}

#[tokio::test]
async fn blocked_command_is_denied_even_if_capability_allows() {
    let mut cfg = LocalPolicyConfig::full_trusted();
    cfg.blocked_commands.insert("rm".to_string());

    let engine = LocalPolicyEngine::with_config("full-trusted", cfg);

    let mut action = RuntimeAction::for_capability(Capability::ExecShell, PathBuf::from("."));
    action.command = Some("rm".to_string());
    action.args = vec!["-rf".to_string(), "tmp".to_string()];

    let decision = engine
        .evaluate("sess_3", &action)
        .await
        .expect("evaluate blocked command");

    assert!(!decision.effect.is_allowed(), "rm should be denied");
}
