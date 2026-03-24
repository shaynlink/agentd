use std::env;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub default_provider: String,
    pub cli: CliProviderConfig,
    pub http: HttpProviderConfig,
    pub sandbox: SandboxProviderConfig,
}

#[derive(Debug, Clone)]
pub struct CliProviderConfig {
    pub command: String,
    pub args: Vec<String>,
    pub prompt_mode: String,
    pub prompt_flag: String,
    pub runtime_dir: PathBuf,
    pub plan_command: String,
    pub plan_args: Vec<String>,
    pub plan_goal_mode: String,
    pub plan_goal_flag: String,
    pub plan_output_format: String,
}

#[derive(Debug, Clone)]
pub struct HttpProviderConfig {
    pub endpoint: String,
    pub auth_mode: String,
    pub bearer_token: Option<String>,
    pub api_key: Option<String>,
    pub api_key_header: String,
}

#[derive(Debug, Clone)]
pub struct SandboxProviderConfig {
    pub runtime: String,
    pub role: String,
    pub workdir: PathBuf,
    pub audit_log_path: PathBuf,
    pub audit_backend: String,
    pub allowed_commands: Vec<String>,
    pub allowed_read_paths: Vec<String>,
    pub allowed_write_paths: Vec<String>,
    pub trace_commands: bool,
    pub trace_diff: bool,
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
    sandbox: Option<FileSandboxProviderConfig>,
}

#[derive(Debug, Deserialize, Default)]
struct FileCliProviderConfig {
    command: Option<String>,
    args: Option<Vec<String>>,
    prompt_mode: Option<String>,
    prompt_flag: Option<String>,
    runtime_dir: Option<PathBuf>,
    plan_command: Option<String>,
    plan_args: Option<Vec<String>>,
    plan_goal_mode: Option<String>,
    plan_goal_flag: Option<String>,
    plan_output_format: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct FileHttpProviderConfig {
    endpoint: Option<String>,
    auth_mode: Option<String>,
    bearer_token: Option<String>,
    api_key: Option<String>,
    api_key_header: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct FileSandboxProviderConfig {
    runtime: Option<String>,
    role: Option<String>,
    workdir: Option<PathBuf>,
    audit_log_path: Option<PathBuf>,
    audit_backend: Option<String>,
    allowed_commands: Option<Vec<String>>,
    allowed_read_paths: Option<Vec<String>>,
    allowed_write_paths: Option<Vec<String>>,
    trace_commands: Option<bool>,
    trace_diff: Option<bool>,
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let cfg_path = env::var("AGENTD_CONFIG").unwrap_or_else(|_| "agentd.toml".to_string());
        let file_cfg = load_file_config(&cfg_path)?;

        let file_cli = file_cfg.providers.as_ref().and_then(|p| p.cli.as_ref());
        let file_http = file_cfg.providers.as_ref().and_then(|p| p.http.as_ref());
        let file_sandbox = file_cfg.providers.as_ref().and_then(|p| p.sandbox.as_ref());

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

        let plan_command = env::var("AGENTD_CLI_PLAN_COMMAND")
            .ok()
            .or_else(|| file_cli.and_then(|c| c.plan_command.clone()))
            .unwrap_or_else(|| command.clone());

        let plan_args = if let Ok(raw) = env::var("AGENTD_CLI_PLAN_ARGS_JSON") {
            serde_json::from_str::<Vec<String>>(&raw)
                .context("AGENTD_CLI_PLAN_ARGS_JSON must be a JSON string array")?
        } else {
            file_cli
                .and_then(|c| c.plan_args.clone())
                .unwrap_or_else(|| args.clone())
        };

        let plan_goal_mode = env::var("AGENTD_CLI_PLAN_GOAL_MODE")
            .ok()
            .or_else(|| file_cli.and_then(|c| c.plan_goal_mode.clone()))
            .unwrap_or_else(|| prompt_mode.clone());

        let plan_goal_flag = env::var("AGENTD_CLI_PLAN_GOAL_FLAG")
            .ok()
            .or_else(|| file_cli.and_then(|c| c.plan_goal_flag.clone()))
            .unwrap_or_else(|| prompt_flag.clone());

        let plan_output_format = env::var("AGENTD_CLI_PLAN_OUTPUT_FORMAT")
            .ok()
            .or_else(|| file_cli.and_then(|c| c.plan_output_format.clone()))
            .unwrap_or_else(|| "yaml".to_string());

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

        let runtime = env::var("AGENTD_SANDBOX_RUNTIME")
            .ok()
            .or_else(|| file_sandbox.and_then(|s| s.runtime.clone()))
            .unwrap_or_else(|| "process".to_string());

        let role = env::var("AGENTD_SANDBOX_ROLE")
            .ok()
            .or_else(|| file_sandbox.and_then(|s| s.role.clone()))
            .unwrap_or_else(|| "operator".to_string());

        let workdir = env::var("AGENTD_SANDBOX_WORKDIR")
            .ok()
            .map(PathBuf::from)
            .or_else(|| file_sandbox.and_then(|s| s.workdir.clone()))
            .unwrap_or_else(|| PathBuf::from("./.agentd/sandbox"));

        let audit_log_path = env::var("AGENTD_SANDBOX_AUDIT_LOG_PATH")
            .ok()
            .map(PathBuf::from)
            .or_else(|| file_sandbox.and_then(|s| s.audit_log_path.clone()))
            .unwrap_or_else(|| PathBuf::from("./.agentd/audit.log"));

        let audit_backend = env::var("AGENTD_SANDBOX_AUDIT_BACKEND")
            .ok()
            .or_else(|| file_sandbox.and_then(|s| s.audit_backend.clone()))
            .unwrap_or_else(|| "file".to_string());

        let allowed_commands = if let Ok(raw) = env::var("AGENTD_SANDBOX_ALLOWED_COMMANDS_JSON") {
            serde_json::from_str::<Vec<String>>(&raw)
                .context("AGENTD_SANDBOX_ALLOWED_COMMANDS_JSON must be a JSON string array")?
        } else {
            file_sandbox
                .and_then(|s| s.allowed_commands.clone())
                .unwrap_or_default()
        };

        let allowed_read_paths = if let Ok(raw) = env::var("AGENTD_SANDBOX_ALLOWED_READ_PATHS_JSON")
        {
            serde_json::from_str::<Vec<String>>(&raw)
                .context("AGENTD_SANDBOX_ALLOWED_READ_PATHS_JSON must be a JSON string array")?
        } else {
            file_sandbox
                .and_then(|s| s.allowed_read_paths.clone())
                .unwrap_or_default()
        };

        let allowed_write_paths = if let Ok(raw) =
            env::var("AGENTD_SANDBOX_ALLOWED_WRITE_PATHS_JSON")
        {
            serde_json::from_str::<Vec<String>>(&raw)
                .context("AGENTD_SANDBOX_ALLOWED_WRITE_PATHS_JSON must be a JSON string array")?
        } else {
            file_sandbox
                .and_then(|s| s.allowed_write_paths.clone())
                .unwrap_or_default()
        };

        let trace_commands = env::var("AGENTD_SANDBOX_TRACE_COMMANDS")
            .ok()
            .and_then(|v| v.parse::<bool>().ok())
            .or_else(|| file_sandbox.and_then(|s| s.trace_commands))
            .unwrap_or(true);

        let trace_diff = env::var("AGENTD_SANDBOX_TRACE_DIFF")
            .ok()
            .and_then(|v| v.parse::<bool>().ok())
            .or_else(|| file_sandbox.and_then(|s| s.trace_diff))
            .unwrap_or(true);

        Ok(Self {
            default_provider,
            cli: CliProviderConfig {
                command,
                args,
                prompt_mode,
                prompt_flag,
                runtime_dir,
                plan_command,
                plan_args,
                plan_goal_mode,
                plan_goal_flag,
                plan_output_format,
            },
            http: HttpProviderConfig {
                endpoint,
                auth_mode,
                bearer_token,
                api_key,
                api_key_header,
            },
            sandbox: SandboxProviderConfig {
                runtime,
                role,
                workdir,
                audit_log_path,
                audit_backend,
                allowed_commands,
                allowed_read_paths,
                allowed_write_paths,
                trace_commands,
                trace_diff,
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
