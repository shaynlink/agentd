use std::env;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub default_provider: String,
    pub cli: CliProviderConfig,
    pub http: HttpProviderConfig,
}

#[derive(Debug, Clone)]
pub struct CliProviderConfig {
    pub command: String,
    pub args: Vec<String>,
    pub prompt_mode: String,
    pub prompt_flag: String,
    pub runtime_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct HttpProviderConfig {
    pub endpoint: String,
    pub auth_mode: String,
    pub bearer_token: Option<String>,
    pub api_key: Option<String>,
    pub api_key_header: String,
}

#[derive(Debug, Deserialize, Default)]
struct FileConfig {
    default_provider: Option<String>,
    providers: Option<FileProvidersConfig>,
}

#[derive(Debug, Deserialize, Default)]
struct FileProvidersConfig {
    cli: Option<FileCliProviderConfig>,
    http: Option<FileHttpProviderConfig>,
}

#[derive(Debug, Deserialize, Default)]
struct FileCliProviderConfig {
    command: Option<String>,
    args: Option<Vec<String>>,
    prompt_mode: Option<String>,
    prompt_flag: Option<String>,
    runtime_dir: Option<PathBuf>,
}

#[derive(Debug, Deserialize, Default)]
struct FileHttpProviderConfig {
    endpoint: Option<String>,
    auth_mode: Option<String>,
    bearer_token: Option<String>,
    api_key: Option<String>,
    api_key_header: Option<String>,
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let cfg_path = env::var("AGENTD_CONFIG").unwrap_or_else(|_| "agentd.toml".to_string());
        let file_cfg = load_file_config(&cfg_path)?;

        let file_cli = file_cfg.providers.as_ref().and_then(|p| p.cli.as_ref());
        let file_http = file_cfg.providers.as_ref().and_then(|p| p.http.as_ref());

        let default_provider = env::var("AGENTD_DEFAULT_PROVIDER")
            .ok()
            .or_else(|| file_cfg.default_provider.clone())
            .unwrap_or_else(|| "mock".to_string());

        let command = env::var("AGENTD_CLI_COMMAND")
            .ok()
            .or_else(|| file_cli.and_then(|c| c.command.clone()))
            .unwrap_or_else(|| "cat".to_string());

        let args = if let Ok(raw) = env::var("AGENTD_CLI_ARGS_JSON") {
            serde_json::from_str::<Vec<String>>(&raw)
                .context("AGENTD_CLI_ARGS_JSON must be a JSON string array")?
        } else {
            file_cli.and_then(|c| c.args.clone()).unwrap_or_default()
        };

        let prompt_mode = env::var("AGENTD_CLI_PROMPT_MODE")
            .ok()
            .or_else(|| file_cli.and_then(|c| c.prompt_mode.clone()))
            .unwrap_or_else(|| "stdin".to_string());

        let prompt_flag = env::var("AGENTD_CLI_PROMPT_FLAG")
            .ok()
            .or_else(|| file_cli.and_then(|c| c.prompt_flag.clone()))
            .unwrap_or_else(|| "--prompt".to_string());

        let runtime_dir = env::var("AGENTD_CLI_RUNTIME_DIR")
            .ok()
            .map(PathBuf::from)
            .or_else(|| file_cli.and_then(|c| c.runtime_dir.clone()))
            .unwrap_or_else(|| PathBuf::from("./.agentd/runtime"));

        let endpoint = env::var("AGENTD_HTTP_ENDPOINT")
            .ok()
            .or_else(|| file_http.and_then(|h| h.endpoint.clone()))
            .unwrap_or_else(|| "http://localhost:8080/run-agent".to_string());

        let auth_mode = env::var("AGENTD_HTTP_AUTH_MODE")
            .ok()
            .or_else(|| file_http.and_then(|h| h.auth_mode.clone()))
            .unwrap_or_else(|| "none".to_string());

        let bearer_token = env::var("AGENTD_HTTP_BEARER_TOKEN")
            .ok()
            .or_else(|| file_http.and_then(|h| h.bearer_token.clone()));

        let api_key = env::var("AGENTD_HTTP_API_KEY")
            .ok()
            .or_else(|| file_http.and_then(|h| h.api_key.clone()));

        let api_key_header = env::var("AGENTD_HTTP_API_KEY_HEADER")
            .ok()
            .or_else(|| file_http.and_then(|h| h.api_key_header.clone()))
            .unwrap_or_else(|| "x-api-key".to_string());

        Ok(Self {
            default_provider,
            cli: CliProviderConfig {
                command,
                args,
                prompt_mode,
                prompt_flag,
                runtime_dir,
            },
            http: HttpProviderConfig {
                endpoint,
                auth_mode,
                bearer_token,
                api_key,
                api_key_header,
            },
        })
    }
}

fn load_file_config(path: &str) -> Result<FileConfig> {
    let file_path = PathBuf::from(path);
    if !file_path.exists() {
        return Ok(FileConfig::default());
    }

    let raw = std::fs::read_to_string(&file_path)
        .with_context(|| format!("failed to read config file: {}", file_path.display()))?;
    let cfg: FileConfig = toml::from_str(&raw).with_context(|| {
        format!(
            "failed to parse config file as TOML: {}",
            file_path.display()
        )
    })?;
    Ok(cfg)
}
