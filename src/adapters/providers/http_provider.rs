use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};
use serde::Serialize;
use serde_json::Value;

use crate::config::AppConfig;
use crate::domain::plan::Plan;
use crate::ports::provider::{Provider, ProviderRunRequest, ProviderRunResult};

pub struct HttpProvider;

impl HttpProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HttpProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy)]
enum AuthMode {
    None,
    Bearer,
    ApiKey,
}

impl AuthMode {
    fn from_value(value: &str) -> Self {
        match value {
            v if v.eq_ignore_ascii_case("bearer") => Self::Bearer,
            v if v.eq_ignore_ascii_case("api-key") => Self::ApiKey,
            _ => Self::None,
        }
    }
}

#[derive(Debug, Clone)]
struct HttpProviderConfig {
    endpoint: String,
    auth_mode: AuthMode,
    bearer_token: Option<String>,
    api_key: Option<String>,
    api_key_header: String,
}

impl HttpProviderConfig {
    fn load() -> Result<Self> {
        let cfg = AppConfig::load()?;
        let http_cfg = cfg.http;

        Ok(Self {
            endpoint: http_cfg.endpoint,
            auth_mode: AuthMode::from_value(&http_cfg.auth_mode),
            bearer_token: http_cfg.bearer_token,
            api_key: http_cfg.api_key,
            api_key_header: http_cfg.api_key_header,
        })
    }

    fn build_headers(&self) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        match self.auth_mode {
            AuthMode::None => {}
            AuthMode::Bearer => {
                let token = self.bearer_token.as_ref().context(
                    "AGENTD_HTTP_BEARER_TOKEN is required when AGENTD_HTTP_AUTH_MODE=bearer",
                )?;
                let value = HeaderValue::from_str(&format!("Bearer {token}"))
                    .context("invalid bearer token for HTTP header")?;
                headers.insert(AUTHORIZATION, value);
            }
            AuthMode::ApiKey => {
                let key = self.api_key.as_ref().context(
                    "AGENTD_HTTP_API_KEY is required when AGENTD_HTTP_AUTH_MODE=api-key",
                )?;
                let header_name = HeaderName::from_bytes(self.api_key_header.as_bytes())
                    .context("invalid AGENTD_HTTP_API_KEY_HEADER name")?;
                let value =
                    HeaderValue::from_str(key).context("invalid AGENTD_HTTP_API_KEY value")?;
                headers.insert(header_name, value);
            }
        }

        Ok(headers)
    }
}

#[derive(Debug, Serialize)]
struct RunAgentPayload<'a> {
    agent_id: &'a str,
    prompt: &'a str,
    timeout_secs: u64,
}

fn extract_output_from_json(body: &Value) -> Option<String> {
    body.get("output")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| {
            body.get("result")
                .and_then(|v| v.get("output"))
                .and_then(Value::as_str)
                .map(ToString::to_string)
        })
        .or_else(|| {
            body.get("data")
                .and_then(|v| v.get("output"))
                .and_then(Value::as_str)
                .map(ToString::to_string)
        })
}

fn compact_body_preview(input: &str) -> String {
    const MAX_LEN: usize = 600;
    let trimmed = input.trim();
    if trimmed.len() <= MAX_LEN {
        return trimmed.to_string();
    }
    format!("{}...", &trimmed[..MAX_LEN])
}

#[async_trait]
impl Provider for HttpProvider {
    fn name(&self) -> &'static str {
        "http"
    }

    async fn generate_plan(&self, _goal: &str) -> Result<Plan> {
        bail!("http provider does not implement plan generation yet")
    }

    async fn run_agent(&self, request: ProviderRunRequest) -> Result<ProviderRunResult> {
        let cfg = HttpProviderConfig::load()?;
        let headers = cfg.build_headers()?;

        let payload = RunAgentPayload {
            agent_id: &request.agent_id,
            prompt: &request.prompt,
            timeout_secs: request.timeout_secs,
        };

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(request.timeout_secs))
            .build()
            .context("failed to build reqwest HTTP client")?;

        let response = client
            .post(&cfg.endpoint)
            .headers(headers)
            .json(&payload)
            .send()
            .await
            .with_context(|| format!("failed to call HTTP provider endpoint: {}", cfg.endpoint))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .context("failed to read HTTP provider response body")?;

        if !status.is_success() {
            bail!(
                "http provider request failed with status {}: {}",
                status,
                compact_body_preview(&body)
            );
        }

        if let Ok(json_body) = serde_json::from_str::<Value>(&body)
            && let Some(output) = extract_output_from_json(&json_body)
        {
            return Ok(ProviderRunResult { output });
        }

        let trimmed = body.trim();
        if !trimmed.is_empty() {
            return Ok(ProviderRunResult {
                output: trimmed.to_string(),
            });
        }

        bail!("http provider returned an empty response body")
    }
}
