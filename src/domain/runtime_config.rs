use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub runtime: String,
}

impl RuntimeConfig {
    pub fn resolve(
        plan_runtime: Option<&str>,
        cli_runtime: Option<&str>,
        config_runtime: &str,
    ) -> Self {
        let runtime = plan_runtime
            .filter(|v| !v.trim().is_empty())
            .or(cli_runtime.filter(|v| !v.trim().is_empty()))
            .unwrap_or(config_runtime)
            .to_string();

        Self { runtime }
    }
}
