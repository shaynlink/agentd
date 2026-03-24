use std::fs;
use std::path::{Path, PathBuf};

use agentd::adapters::store::sqlite::SqliteStore;
use agentd::app::{App, OutputMode, OutputOptions};
use agentd::domain::agent::AgentState;
use agentd::ports::store::StateStore;
use uuid::Uuid;

fn test_output_options() -> OutputOptions {
    OutputOptions {
        mode: OutputMode::Text,
        quiet: true,
    }
}

fn temp_db_path() -> String {
    let mut path = PathBuf::from(std::env::temp_dir());
    path.push(format!("agentd-test-{}.db", Uuid::new_v4()));
    path.to_string_lossy().to_string()
}

fn temp_plan_path(ext: &str) -> PathBuf {
    let mut path = PathBuf::from(std::env::temp_dir());
    path.push(format!("agentd-plan-{}.{}", Uuid::new_v4(), ext));
    path
}

fn write_plan_file(path: &Path, content: &str) {
    fs::write(path, content).expect("write plan file");
}

#[tokio::test]
async fn run_plan_parses_yaml_plan_file() {
    let db_path = temp_db_path();
    let app = App::new(db_path.clone(), test_output_options()).expect("create app");

    let plan_path = temp_plan_path("yaml");
    write_plan_file(
        &plan_path,
        r#"name: yaml-plan
steps:
  - id: s1
    name: step-one
    prompt: do one
    provider: mock
  - id: s2
    name: step-two
    prompt: do two
    provider: mock
    depends_on: [s1]
    timeout_secs: 5
    retries: 0
"#,
    );

    app.run_plan(&plan_path, "mock")
        .await
        .expect("run yaml plan");

    let store = SqliteStore::new(db_path);
    let agents = store.list_agents().expect("list agents");
    assert_eq!(agents.len(), 2, "expected two agents from yaml plan");
    assert!(
        agents.iter().all(|a| a.state == AgentState::Succeeded),
        "all agents should succeed"
    );

    let _ = fs::remove_file(&plan_path);
}

#[tokio::test]
async fn run_plan_parses_json_plan_file() {
    let db_path = temp_db_path();
    let app = App::new(db_path.clone(), test_output_options()).expect("create app");

    let plan_path = temp_plan_path("json");
    write_plan_file(
        &plan_path,
        r#"{
  "name": "json-plan",
  "steps": [
    {
      "id": "s1",
      "name": "step-one",
      "prompt": "do one"
    },
    {
      "id": "s2",
      "name": "step-two",
      "prompt": "do two",
      "depends_on": ["s1"],
      "timeout_secs": 5,
      "retries": 0
    }
  ]
}"#,
    );

    app.run_plan(&plan_path, "mock")
        .await
        .expect("run json plan");

    let store = SqliteStore::new(db_path);
    let agents = store.list_agents().expect("list agents");
    assert_eq!(agents.len(), 2, "expected two agents from json plan");
    assert!(
        agents.iter().all(|a| a.state == AgentState::Succeeded),
        "all agents should succeed"
    );

    let _ = fs::remove_file(&plan_path);
}
