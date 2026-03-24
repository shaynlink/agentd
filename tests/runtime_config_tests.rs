use agentd::domain::runtime_config::RuntimeConfig;

#[test]
fn runtime_priority_plan_over_cli_over_config() {
    let resolved = RuntimeConfig::resolve(Some("docker"), Some("builtin"), "process");
    assert_eq!(resolved.runtime, "docker");
}

#[test]
fn runtime_priority_cli_over_config_when_plan_missing() {
    let resolved = RuntimeConfig::resolve(None, Some("builtin"), "process");
    assert_eq!(resolved.runtime, "builtin");
}

#[test]
fn runtime_falls_back_to_config() {
    let resolved = RuntimeConfig::resolve(None, None, "process");
    assert_eq!(resolved.runtime, "process");
}
