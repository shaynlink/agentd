use std::path::PathBuf;
use std::sync::Mutex;

use agentd::adapters::providers::cli_provider::CliProvider;
use agentd::ports::provider::{Provider, ProviderRunRequest};
use uuid::Uuid;

static ENV_LOCK: Mutex<()> = Mutex::new(());

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

async fn run_stream_case(json_lines: bool) -> String {
    let _lock = ENV_LOCK.lock().expect("lock test env");
    let _env = EnvGuard::set(&[
        ("AGENTD_CLI_COMMAND", "/bin/sh".to_string()),
        (
            "AGENTD_CLI_ARGS_JSON",
            "[\"-c\",\"printf 'line-out\\n'; printf 'line-err\\n' >&2\"]".to_string(),
        ),
        ("AGENTD_CLI_RUNTIME_DIR", temp_runtime_dir()),
    ]);

    let provider = CliProvider::new();
    let req = ProviderRunRequest {
        agent_id: Uuid::new_v4().to_string(),
        prompt: "ignored".to_string(),
        timeout_secs: 5,
        stream_output: true,
        json_lines,
    };

    let result = provider.run_agent(req).await.expect("run stream case");
    result.output
}

async fn generate_plan_case(plan_output_format: &str, goal: &str) -> String {
    let _lock = ENV_LOCK.lock().expect("lock test env");
    let _env = EnvGuard::set(&[
        ("AGENTD_CLI_PLAN_COMMAND", "/bin/sh".to_string()),
        ("AGENTD_CLI_PLAN_ARGS_JSON", "[\"-c\",\"cat\"]".to_string()),
        ("AGENTD_CLI_PLAN_GOAL_MODE", "stdin".to_string()),
        (
            "AGENTD_CLI_PLAN_OUTPUT_FORMAT",
            plan_output_format.to_string(),
        ),
        ("AGENTD_CLI_RUNTIME_DIR", temp_runtime_dir()),
    ]);

    let provider = CliProvider::new();
    let plan = provider
        .generate_plan(goal)
        .await
        .expect("generate plan from cli provider");
    serde_yaml::to_string(&plan).expect("serialize generated plan")
}

async fn run_vibe_case() -> String {
    let _lock = ENV_LOCK.lock().expect("lock test env");
    let _env = EnvGuard::set(&[
        ("AGENTD_VIBE_COMMAND", "/bin/sh".to_string()),
        (
            "AGENTD_VIBE_ARGS_JSON",
            "[\"-c\",\"printf 'vibe-out\\n'\"]".to_string(),
        ),
        ("AGENTD_VIBE_PROMPT_MODE", "stdin".to_string()),
        ("AGENTD_VIBE_RUNTIME_DIR", temp_runtime_dir()),
    ]);

    let provider = CliProvider::new_vibe();
    let req = ProviderRunRequest {
        agent_id: Uuid::new_v4().to_string(),
        prompt: "ignored".to_string(),
        timeout_secs: 5,
        stream_output: false,
        json_lines: false,
    };

    let result = provider.run_agent(req).await.expect("run vibe case");
    result.output
}

#[tokio::test]
async fn cli_provider_stream_text_mode_returns_combined_output() {
    let output = run_stream_case(false).await;
    assert!(
        output.contains("line-out"),
        "stdout line should be present in output: {output}"
    );
    assert!(
        output.contains("line-err"),
        "stderr line should be present in output: {output}"
    );
}

#[tokio::test]
async fn cli_provider_stream_json_lines_mode_returns_combined_output() {
    let output = run_stream_case(true).await;
    assert!(
        output.contains("line-out"),
        "stdout line should be present in output: {output}"
    );
    assert!(
        output.contains("line-err"),
        "stderr line should be present in output: {output}"
    );
}

#[tokio::test]
async fn cli_provider_generate_plan_from_yaml_output() {
    let plan_yaml = r#"name: plan-yaml
steps:
  - id: step-1
    name: Analyze
    prompt: Do it
"#;
    let output = generate_plan_case("yaml", plan_yaml).await;
    assert!(
        output.contains("name: plan-yaml"),
        "unexpected output: {output}"
    );
    assert!(output.contains("id: step-1"), "unexpected output: {output}");
}

#[tokio::test]
async fn cli_provider_generate_plan_from_json_output() {
    let plan_json =
        r#"{"name":"plan-json","steps":[{"id":"step-1","name":"Analyze","prompt":"Do it"}]}"#;
    let output = generate_plan_case("json", plan_json).await;
    assert!(
        output.contains("name: plan-json"),
        "unexpected output: {output}"
    );
    assert!(output.contains("id: step-1"), "unexpected output: {output}");
}

#[tokio::test]
async fn vibe_provider_uses_vibe_env_configuration() {
    let output = run_vibe_case().await;
    assert!(
        output.contains("vibe-out"),
        "vibe provider output should be present: {output}"
    );
}
