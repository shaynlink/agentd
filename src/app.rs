use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use cron::Schedule;
use serde_json::{Value, json};
use tokio::time::timeout;
use uuid::Uuid;

use crate::adapters::providers;
use crate::adapters::security;
use crate::adapters::store::sqlite::SqliteStore;
use crate::adapters::versioning;
use crate::domain::agent::{AgentRecord, AgentState};
use crate::domain::plan::Plan;
use crate::domain::schedule::{ScheduleRecord, ScheduleState};
use crate::ports::provider::ProviderRunRequest;
use crate::ports::securable::{AuditEventFilters, RbacPolicySpec};
use crate::ports::store::StateStore;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Text,
    Json,
    Jsonl,
    Tsv,
}

#[derive(Debug, Clone, Copy)]
pub struct OutputOptions {
    pub mode: OutputMode,
    pub quiet: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AuditListFilters<'a> {
    pub role: Option<&'a str>,
    pub allowed: Option<bool>,
    pub runtime: Option<&'a str>,
    pub agent_id: Option<&'a str>,
    pub since: Option<&'a str>,
    pub until: Option<&'a str>,
}

pub struct App {
    store: SqliteStore,
    output: OutputOptions,
}

fn structured_log_message(context: &str, provider: &str, category: &str, message: &str) -> String {
    serde_json::json!({
        "context": context,
        "provider": provider,
        "category": category,
        "message": message,
    })
    .to_string()
}

fn extract_conflicted_files(message: &str) -> Vec<String> {
    let marker = "conflicted files:";
    let Some(start) = message.find(marker) else {
        return Vec::new();
    };

    let rest = &message[start + marker.len()..];
    let files_part = match rest.find(". details:") {
        Some(idx) => &rest[..idx],
        None => rest,
    };

    files_part
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

impl App {
    pub fn new(db_path: String, output: OutputOptions) -> Result<Self> {
        let store = SqliteStore::new(db_path);
        store.init()?;
        let recovered = store.recover_stuck_executions()?;
        for agent_id in &recovered {
            let _ = store.append_log(
                agent_id,
                "warn",
                &structured_log_message(
                    "startup",
                    "system",
                    "recovery",
                    "recovered after restart: running -> pending",
                ),
            );
        }

        let app = Self { store, output };
        if !recovered.is_empty() {
            app.emit(
                "startup_recovery",
                json!({ "recovered_count": recovered.len() }),
                Some(format!(
                    "recovery: {} in-progress agent(s) moved to pending",
                    recovered.len()
                )),
            );
        }

        Ok(app)
    }

    fn emit(&self, event: &str, data: Value, text: Option<String>) {
        if self.output.quiet {
            return;
        }

        match self.output.mode {
            OutputMode::Text => {
                if let Some(line) = text {
                    println!("{line}");
                }
            }
            OutputMode::Json => {
                println!("{}", json!({ "event": event, "data": data }));
            }
            OutputMode::Jsonl => {
                if let Some(items) = data.as_array() {
                    for item in items {
                        println!("{}", json!({ "event": event, "data": item }));
                    }
                } else {
                    println!("{}", json!({ "event": event, "data": data }));
                }
            }
            OutputMode::Tsv => {
                // Special handling for agent_list with TSV format
                if event == "agent_list" {
                    if let Some(agents) = data.get("agents").and_then(|a| a.as_array()) {
                        // Print header
                        println!("id\tname\tprovider\tstate\tattempts\tcreated_at\tupdated_at");
                        // Print each agent as TSV row
                        for agent in agents {
                            let id = agent.get("id").and_then(|v| v.as_str()).unwrap_or("");
                            let name = agent.get("name").and_then(|v| v.as_str()).unwrap_or("");
                            let provider =
                                agent.get("provider").and_then(|v| v.as_str()).unwrap_or("");
                            let state = agent.get("state").and_then(|v| v.as_str()).unwrap_or("");
                            let attempts =
                                agent.get("attempts").and_then(|v| v.as_u64()).unwrap_or(0);
                            let created_at = agent
                                .get("created_at")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let updated_at = agent
                                .get("updated_at")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            println!(
                                "{}\t{}\t{}\t{}\t{}\t{}\t{}",
                                id, name, provider, state, attempts, created_at, updated_at
                            );
                        }
                    }
                } else if event == "agent_ids" {
                    if let Some(ids) = data.get("ids").and_then(|i| i.as_array()) {
                        for id in ids {
                            if let Some(id_str) = id.as_str() {
                                println!("{}", id_str);
                            }
                        }
                    }
                } else {
                    // Fallback to JSON for other events
                    println!("{}", json!({ "event": event, "data": data }));
                }
            }
        }
    }

    pub async fn plan_generate(
        &self,
        provider_name: &str,
        goal: &str,
        output: Option<&Path>,
    ) -> Result<()> {
        let provider = providers::build_provider(provider_name)?;
        let plan = provider.generate_plan(goal).await?;
        let serialized =
            serde_yaml::to_string(&plan).context("failed to serialize plan as YAML")?;
        if let Some(path) = output {
            std::fs::write(path, serialized)
                .with_context(|| format!("failed to write plan file: {}", path.display()))?;
            self.emit(
                "plan_generated",
                json!({
                    "provider": provider_name,
                    "goal": goal,
                    "output_path": path.display().to_string(),
                    "plan": plan,
                }),
                Some(format!("generated plan written to {}", path.display())),
            );
        } else {
            self.emit(
                "plan_generated",
                json!({
                    "provider": provider_name,
                    "goal": goal,
                    "plan": plan,
                    "plan_yaml": serialized,
                }),
                Some(serialized.trim().to_string()),
            );
        }
        Ok(())
    }

    pub async fn run_plan(&self, plan_file: &Path, default_provider: &str) -> Result<()> {
        let content = std::fs::read_to_string(plan_file)
            .with_context(|| format!("failed to read plan file: {}", plan_file.display()))?;
        let plan: Plan = if plan_file.extension().and_then(|e| e.to_str()) == Some("json") {
            serde_json::from_str(&content).context("failed to parse JSON plan")?
        } else {
            serde_yaml::from_str(&content).context("failed to parse YAML plan")?
        };

        self.emit(
            "plan_run_started",
            json!({
                "plan_name": plan.name,
                "step_count": plan.steps.len(),
                "plan_file": plan_file.display().to_string(),
            }),
            Some(format!(
                "running plan '{}' with {} step(s)",
                plan.name,
                plan.steps.len()
            )),
        );

        for step in plan.steps {
            let provider_name = step.provider.as_deref().unwrap_or(default_provider);
            self.spawn_and_run(
                &step.name,
                provider_name,
                &step.prompt,
                step.timeout_secs.unwrap_or(60),
                step.retries.unwrap_or(0),
                step.runtime,
            )
            .await?;
        }
        Ok(())
    }

    pub async fn spawn(
        &self,
        name: &str,
        provider: &str,
        prompt: &str,
        timeout_secs: u64,
        retries: u32,
        sandbox_runtime: Option<String>,
    ) -> Result<()> {
        let _sandbox_runtime = sandbox_runtime; // For future: persist to agent record
        let agent_id = self.spawn_agent_record(name, provider, prompt)?;

        self.emit(
            "agent_spawned",
            json!({
                "agent_id": agent_id,
                "name": name,
                "provider": provider,
                "timeout_secs": timeout_secs,
                "retries": retries,
            }),
            Some(format!("spawned agent '{}' (provider={provider})", name)),
        );
        self.emit(
            "agent_spawn_hint",
            json!({ "agent_id": agent_id, "hint": "use attach to run now or inspect logs/status" }),
            Some("use 'attach' to run now or inspect logs/status".to_string()),
        );
        self.emit(
            "agent_policy",
            json!({
                "agent_id": agent_id,
                "timeout_secs": timeout_secs,
                "retries": retries,
            }),
            Some(format!(
                "default policy timeout={timeout_secs}s retries={retries}"
            )),
        );

        Ok(())
    }

    pub async fn attach(
        &self,
        agent_id: &str,
        timeout_secs: u64,
        retries: u32,
        stream_output: bool,
        json_lines: bool,
        sandbox_runtime: Option<String>,
    ) -> Result<()> {
        let Some(agent) = self.store.get_agent(agent_id)? else {
            bail!("agent not found: {agent_id}");
        };

        let owner = format!("pid:{}", std::process::id());
        let lock_acquired = self.store.try_acquire_execution_lock(agent_id, &owner)?;
        if !lock_acquired {
            bail!("agent {agent_id} is already running (execution lock held)");
        }

        if let Err(err) = self.store.update_state(agent_id, AgentState::Running) {
            let _ = self.store.release_execution_lock(agent_id);
            return Err(err);
        }
        let attach_requested =
            structured_log_message("attach", &agent.provider, "lifecycle", "attach requested");
        if let Err(err) = self.store.append_log(agent_id, "info", &attach_requested) {
            let _ = self.store.release_execution_lock(agent_id);
            return Err(err);
        }

        let mut attempt = 0;
        loop {
            attempt += 1;
            self.store.bump_attempts(agent_id)?;

            let provider = match providers::build_provider(&agent.provider) {
                Ok(provider) => provider,
                Err(err) => {
                    let _ = self.store.release_execution_lock(agent_id);
                    return Err(err);
                }
            };
            let req = ProviderRunRequest {
                agent_id: agent.id.clone(),
                prompt: agent.prompt.clone(),
                timeout_secs,
                stream_output,
                json_lines,
                runtime_override: sandbox_runtime.clone(),
            };

            let result = timeout(Duration::from_secs(timeout_secs), provider.run_agent(req)).await;
            match result {
                Ok(Ok(done)) => {
                    self.store.update_state(agent_id, AgentState::Succeeded)?;
                    let output_log = structured_log_message(
                        "attach",
                        &agent.provider,
                        "provider_output",
                        &done.output,
                    );
                    self.store.append_log(agent_id, "info", &output_log)?;
                    let _ = self.store.release_execution_lock(agent_id);
                    self.emit(
                        "agent_attach_succeeded",
                        json!({
                            "agent_id": agent_id,
                            "provider": agent.provider,
                            "attempt": attempt,
                            "output": done.output,
                        }),
                        Some(format!("agent {agent_id} succeeded\n{}", done.output)),
                    );
                    return Ok(());
                }
                Ok(Err(err)) => {
                    let message = format!("provider error: {err}");
                    let structured = structured_log_message(
                        "attach",
                        &agent.provider,
                        "provider_error",
                        &message,
                    );
                    self.store.append_log(agent_id, "error", &structured)?;
                    if attempt > retries {
                        self.store.update_state(agent_id, AgentState::Failed)?;
                        let _ = self.store.release_execution_lock(agent_id);
                        bail!("agent {agent_id} failed after {attempt} attempt(s): {err}");
                    }
                }
                Err(_) => {
                    let structured = structured_log_message(
                        "attach",
                        &agent.provider,
                        "timeout",
                        "execution timed out",
                    );
                    self.store.append_log(agent_id, "error", &structured)?;
                    if attempt > retries {
                        self.store.update_state(agent_id, AgentState::TimedOut)?;
                        let _ = self.store.release_execution_lock(agent_id);
                        bail!("agent {agent_id} timed out after {attempt} attempt(s)");
                    }
                }
            }
        }
    }

    pub fn list(
        &self,
        state: Option<&str>,
        provider: Option<&str>,
        limit: Option<usize>,
        ids_only: bool,
        sort_by: Option<&str>,
    ) -> Result<()> {
        let mut agents = self.store.list_agents()?;

        if let Some(state_filter) = state {
            agents.retain(|a| a.state.as_str() == state_filter);
        }
        if let Some(provider_filter) = provider {
            agents.retain(|a| a.provider == provider_filter);
        }

        // Apply sorting
        if let Some(sort_field) = sort_by {
            match sort_field {
                "created_at" => {
                    agents.sort_by(|a, b| b.created_at.cmp(&a.created_at)); // newest first
                }
                "state" => {
                    agents.sort_by(|a, b| a.state.as_str().cmp(b.state.as_str()));
                }
                "provider" => {
                    agents.sort_by(|a, b| a.provider.cmp(&b.provider));
                }
                _ => {
                    // Invalid sort field, silently ignore
                }
            }
        }

        if let Some(limit_value) = limit {
            agents.truncate(limit_value);
        }

        if agents.is_empty() {
            self.emit(
                "agent_list",
                json!({ "count": 0, "agents": [] }),
                Some("no agents found".to_string()),
            );
            return Ok(());
        }

        if ids_only {
            let ids: Vec<String> = agents.iter().map(|a| a.id.clone()).collect();
            self.emit(
                "agent_ids",
                json!({ "count": ids.len(), "ids": ids }),
                Some(ids.join("\n")),
            );
            return Ok(());
        }

        let agents_json =
            serde_json::to_value(&agents).context("failed to serialize agent list")?;
        self.emit(
            "agent_list",
            json!({
                "count": agents.len(),
                "agents": agents_json,
            }),
            Some(
                agents
                    .iter()
                    .map(|a| {
                        format!(
                            "{} | {} | {} | {} | attempts={}",
                            a.id,
                            a.name,
                            a.provider,
                            a.state.as_str(),
                            a.attempts
                        )
                    })
                    .collect::<Vec<String>>()
                    .join("\n"),
            ),
        );
        Ok(())
    }

    pub fn pause(&self, agent_id: &str) -> Result<()> {
        self.store.update_state(agent_id, AgentState::Paused)?;
        self.store.append_log(
            agent_id,
            "info",
            &structured_log_message("lifecycle", "unknown", "state_change", "paused"),
        )?;
        self.emit(
            "agent_paused",
            json!({ "agent_id": agent_id, "state": "paused" }),
            Some(format!("paused {agent_id}")),
        );
        Ok(())
    }

    pub fn resume(&self, agent_id: &str) -> Result<()> {
        self.store.update_state(agent_id, AgentState::Running)?;
        self.store.append_log(
            agent_id,
            "info",
            &structured_log_message("lifecycle", "unknown", "state_change", "resumed"),
        )?;
        self.emit(
            "agent_resumed",
            json!({ "agent_id": agent_id, "state": "running" }),
            Some(format!("resumed {agent_id}")),
        );
        Ok(())
    }

    pub async fn stop(&self, agent_id: &str) -> Result<()> {
        let Some(agent) = self.store.get_agent(agent_id)? else {
            bail!("agent not found: {agent_id}");
        };
        let provider = providers::build_provider(&agent.provider)?;
        let _ = provider.cancel(agent_id).await;
        self.store.update_state(agent_id, AgentState::Cancelled)?;
        let _ = self.store.release_execution_lock(agent_id);
        self.store.append_log(
            agent_id,
            "info",
            &structured_log_message(
                "lifecycle",
                &agent.provider,
                "state_change",
                "stopped/cancelled",
            ),
        )?;
        self.emit(
            "agent_stopped",
            json!({ "agent_id": agent_id, "state": "cancelled", "provider": agent.provider }),
            Some(format!("stopped {agent_id}")),
        );
        Ok(())
    }

    pub fn status(&self, agent_id: &str) -> Result<()> {
        if let Some(agent) = self.store.get_agent(agent_id)? {
            let agent_json =
                serde_json::to_value(&agent).context("failed to serialize agent status")?;
            self.emit(
                "agent_status",
                json!({ "agent": agent_json }),
                Some(format!(
                    "{} | {} | {} | {} | created={} | updated={} | attempts={}",
                    agent.id,
                    agent.name,
                    agent.provider,
                    agent.state.as_str(),
                    agent.created_at,
                    agent.updated_at,
                    agent.attempts
                )),
            );
            return Ok(());
        }
        bail!("agent not found: {agent_id}")
    }

    pub fn logs(
        &self,
        agent_id: &str,
        limit: usize,
        level: Option<&str>,
        contains: Option<&str>,
    ) -> Result<()> {
        let mut logs = self.store.get_logs(agent_id, limit)?;
        logs.reverse();

        if let Some(level_filter) = level {
            logs.retain(|log| log.level.eq_ignore_ascii_case(level_filter));
        }
        if let Some(text_filter) = contains {
            logs.retain(|log| log.message.contains(text_filter));
        }

        let logs_json = serde_json::to_value(&logs).context("failed to serialize logs")?;
        self.emit(
            "agent_logs",
            json!({ "agent_id": agent_id, "count": logs.len(), "logs": logs_json }),
            Some(
                logs.iter()
                    .map(|log| format!("{} [{}] {}", log.ts, log.level, log.message))
                    .collect::<Vec<String>>()
                    .join("\n"),
            ),
        );

        Ok(())
    }

    pub async fn audit_list(&self, limit: usize, filters: AuditListFilters<'_>) -> Result<()> {
        let cfg = crate::config::AppConfig::load()?;
        let securable = security::build_securable(&cfg.sandbox);

        if let Some(s) = filters.since {
            DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.with_timezone(&Utc))
                .with_context(|| format!("invalid --since timestamp (RFC3339 expected): {s}"))?;
        }

        if let Some(s) = filters.until {
            DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.with_timezone(&Utc))
                .with_context(|| format!("invalid --until timestamp (RFC3339 expected): {s}"))?;
        }

        let raw_events = securable
            .list_audit_events(
                limit,
                AuditEventFilters {
                    role: filters.role,
                    allowed: filters.allowed,
                    runtime: filters.runtime,
                    agent_id: filters.agent_id,
                    since: filters.since,
                    until: filters.until,
                },
            )
            .await?;

        let mut events = Vec::new();
        for payload in raw_events {
            if let Ok(v) = serde_json::from_str::<Value>(&payload) {
                events.push(v);
            }
        }

        self.emit(
            "audit_list",
            json!({ "count": events.len(), "events": events }),
            Some(
                events
                    .iter()
                    .map(|e| {
                        format!(
                            "{} | role={} | allowed={} | runtime={} | input={}",
                            e.get("ts").and_then(Value::as_str).unwrap_or(""),
                            e.get("role").and_then(Value::as_str).unwrap_or(""),
                            e.get("allowed").and_then(Value::as_bool).unwrap_or(false),
                            e.get("runtime").and_then(Value::as_str).unwrap_or(""),
                            e.get("command_input").and_then(Value::as_str).unwrap_or(""),
                        )
                    })
                    .collect::<Vec<String>>()
                    .join("\n"),
            ),
        );

        Ok(())
    }

    pub async fn rbac_create_role(&self, name: &str, description: Option<&str>) -> Result<()> {
        let cfg = crate::config::AppConfig::load()?;
        let securable = security::build_securable(&cfg.sandbox);
        securable.create_role(name, description).await?;

        self.emit(
            "rbac_role_created",
            json!({ "name": name, "description": description }),
            Some(format!("rbac role created: {name}")),
        );
        Ok(())
    }

    pub async fn rbac_create_policy(
        &self,
        name: &str,
        resource_type: &str,
        action: &str,
        resource_pattern: &str,
        effect: &str,
    ) -> Result<()> {
        let cfg = crate::config::AppConfig::load()?;
        let securable = security::build_securable(&cfg.sandbox);
        let spec = RbacPolicySpec {
            name: name.to_string(),
            resource_type: resource_type.to_string(),
            action: action.to_string(),
            resource_pattern: resource_pattern.to_string(),
            effect: effect.to_string(),
        };
        securable.create_policy(&spec).await?;

        self.emit(
            "rbac_policy_created",
            json!({
                "name": name,
                "resource_type": resource_type,
                "action": action,
                "resource_pattern": resource_pattern,
                "effect": effect,
            }),
            Some(format!(
                "rbac policy created: {name} ({resource_type}:{action} {resource_pattern} => {effect})"
            )),
        );
        Ok(())
    }

    pub async fn rbac_bind_role(
        &self,
        subject_type: &str,
        subject: &str,
        role_name: &str,
    ) -> Result<()> {
        let cfg = crate::config::AppConfig::load()?;
        let securable = security::build_securable(&cfg.sandbox);
        securable.bind_role(subject_type, subject, role_name).await?;

        self.emit(
            "rbac_binding_created",
            json!({
                "subject_type": subject_type,
                "subject": subject,
                "role": role_name,
            }),
            Some(format!(
                "rbac binding created: {subject_type}:{subject} -> {role_name}"
            )),
        );
        Ok(())
    }

    pub async fn rbac_attach_policy(&self, role_name: &str, policy_name: &str) -> Result<()> {
        let cfg = crate::config::AppConfig::load()?;
        let securable = security::build_securable(&cfg.sandbox);
        securable
            .attach_policy_to_role(role_name, policy_name)
            .await?;

        self.emit(
            "rbac_role_policy_attached",
            json!({
                "role": role_name,
                "policy": policy_name,
            }),
            Some(format!(
                "rbac policy attached: role={role_name} policy={policy_name}"
            )),
        );
        Ok(())
    }

    pub async fn rbac_list(&self) -> Result<()> {
        let cfg = crate::config::AppConfig::load()?;
        let securable = security::build_securable(&cfg.sandbox);
        let snapshot = securable.list_rbac().await?;
        let role_count = snapshot.roles.len();
        let policy_count = snapshot.policies.len();
        let binding_count = snapshot.bindings.len();
        let role_policy_count = snapshot.role_policies.len();

        self.emit(
            "rbac_list",
            json!({
                "roles": snapshot.roles,
                "policies": snapshot.policies,
                "bindings": snapshot.bindings,
                "role_policies": snapshot.role_policies,
            }),
            Some(format!(
                "roles={} policies={} bindings={} role_policies={}",
                role_count,
                policy_count,
                binding_count,
                role_policy_count
            )),
        );
        Ok(())
    }

    pub fn version_branch_create(
        &self,
        repo_path: &Path,
        branch: &str,
        from_ref: Option<&str>,
    ) -> Result<()> {
        let adapter = versioning::build_versioning("git")?;
        adapter.create_branch(repo_path, branch, from_ref)?;

        self.emit(
            "version_branch_created",
            json!({
                "repo_path": repo_path.display().to_string(),
                "branch": branch,
                "from_ref": from_ref,
            }),
            Some(format!(
                "created branch '{branch}'{}",
                from_ref
                    .map(|base| format!(" from '{base}'"))
                    .unwrap_or_default()
            )),
        );
        Ok(())
    }

    pub fn version_branch_list(&self, repo_path: &Path) -> Result<()> {
        let adapter = versioning::build_versioning("git")?;
        let branches = adapter.list_branches(repo_path)?;

        self.emit(
            "version_branch_list",
            json!({
                "repo_path": repo_path.display().to_string(),
                "count": branches.len(),
                "branches": branches,
            }),
            Some(
                branches
                    .iter()
                    .map(|b| {
                        if b.current {
                            format!("* {}", b.name)
                        } else {
                            format!("  {}", b.name)
                        }
                    })
                    .collect::<Vec<String>>()
                    .join("\n"),
            ),
        );
        Ok(())
    }

    pub fn version_diff(&self, repo_path: &Path, from_ref: &str, to_ref: &str) -> Result<()> {
        let adapter = versioning::build_versioning("git")?;
        let diff = adapter.diff(repo_path, from_ref, to_ref)?;

        self.emit(
            "version_diff",
            json!({
                "repo_path": repo_path.display().to_string(),
                "from_ref": from_ref,
                "to_ref": to_ref,
                "diff": diff,
            }),
            Some(diff),
        );
        Ok(())
    }

    pub fn version_merge(
        &self,
        repo_path: &Path,
        source_branch: &str,
        target_branch: &str,
        no_ff: bool,
        dry_run: bool,
    ) -> Result<()> {
        let adapter = versioning::build_versioning("git")?;
        let result = match adapter.merge(repo_path, source_branch, target_branch, no_ff, dry_run) {
            Ok(result) => result,
            Err(err) => {
                let message = err.to_string();
                if message.contains("merge conflict while merging") {
                    let conflicted_files = extract_conflicted_files(&message);
                    self.emit(
                        "version_merge_conflict",
                        json!({
                            "repo_path": repo_path.display().to_string(),
                            "source": source_branch,
                            "target": target_branch,
                            "dry_run": dry_run,
                            "conflicted_files": conflicted_files,
                            "message": message,
                        }),
                        Some(format!(
                            "merge conflict '{}' -> '{}': {}",
                            source_branch,
                            target_branch,
                            conflicted_files.join(", ")
                        )),
                    );
                }
                return Err(err);
            }
        };

        self.emit(
            "version_merge",
            json!({
                "repo_path": repo_path.display().to_string(),
                "source": result.source,
                "target": result.target,
                "commit": result.commit,
                "no_ff": no_ff,
                "dry_run": dry_run,
            }),
            Some(format!(
                "{} '{}' into '{}' at {}",
                if dry_run { "dry-run merged" } else { "merged" },
                source_branch,
                target_branch,
                result.commit
            )),
        );
        Ok(())
    }

    pub fn version_rollback_hard(&self, repo_path: &Path, to_ref: &str, confirm: bool) -> Result<()> {
        if !confirm {
            bail!(
                "destructive rollback requires --confirm-hard-reset=true (this performs git reset --hard)"
            );
        }

        let adapter = versioning::build_versioning("git")?;
        let commit = adapter.rollback_hard(repo_path, to_ref)?;

        self.emit(
            "version_rollback",
            json!({
                "repo_path": repo_path.display().to_string(),
                "to_ref": to_ref,
                "head": commit,
                "mode": "hard",
            }),
            Some(format!("hard rollback to '{}' (HEAD={commit})", to_ref)),
        );
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn schedule_run_at(
        &self,
        name: &str,
        provider: &str,
        prompt: &str,
        run_at: DateTime<Utc>,
        timeout_secs: u64,
        retries: u32,
        _sandbox_runtime: Option<String>,
    ) -> Result<()> {
        let now = Utc::now();
        let schedule = ScheduleRecord {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            provider: provider.to_string(),
            prompt: prompt.to_string(),
            cron_expr: None,
            run_at,
            timeout_secs,
            retries,
            state: ScheduleState::Scheduled,
            created_at: now,
            updated_at: now,
        };

        self.store.create_schedule(&schedule)?;
        let schedule_json =
            serde_json::to_value(&schedule).context("failed to serialize schedule")?;
        self.emit(
            "schedule_created",
            json!({ "schedule": schedule_json, "mode": "run_at" }),
            Some(format!(
                "scheduled '{}' at {} (provider={})\nschedule_id={}",
                schedule.name, schedule.run_at, schedule.provider, schedule.id
            )),
        );
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn schedule_cron(
        &self,
        name: &str,
        provider: &str,
        prompt: &str,
        cron_expr: &str,
        timeout_secs: u64,
        retries: u32,
        _sandbox_runtime: Option<String>,
    ) -> Result<()> {
        let schedule = Schedule::from_str(cron_expr)
            .with_context(|| format!("invalid cron expression: {cron_expr}"))?;
        let next_run = schedule
            .upcoming(Utc)
            .next()
            .context("cron expression does not produce a next run")?;

        let now = Utc::now();
        let record = ScheduleRecord {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            provider: provider.to_string(),
            prompt: prompt.to_string(),
            cron_expr: Some(cron_expr.to_string()),
            run_at: next_run,
            timeout_secs,
            retries,
            state: ScheduleState::Scheduled,
            created_at: now,
            updated_at: now,
        };

        self.store.create_schedule(&record)?;
        let schedule_json =
            serde_json::to_value(&record).context("failed to serialize schedule")?;
        self.emit(
            "schedule_created",
            json!({ "schedule": schedule_json, "mode": "cron" }),
            Some(format!(
                "scheduled cron '{}' next run at {} (provider={})\nschedule_id={}",
                record.name, record.run_at, record.provider, record.id
            )),
        );
        Ok(())
    }

    pub fn list_schedules(&self, limit: usize) -> Result<()> {
        let schedules = self.store.list_schedules(limit)?;
        if schedules.is_empty() {
            self.emit(
                "schedule_list",
                json!({ "count": 0, "schedules": [] }),
                Some("no schedules found".to_string()),
            );
            return Ok(());
        }

        let schedules_json =
            serde_json::to_value(&schedules).context("failed to serialize schedules")?;
        self.emit(
            "schedule_list",
            json!({ "count": schedules.len(), "schedules": schedules_json }),
            Some(
                schedules
                    .iter()
                    .map(|s| {
                        let mode = s
                            .cron_expr
                            .as_ref()
                            .map(|expr| format!("cron={expr}"))
                            .unwrap_or_else(|| "run-at".to_string());
                        format!(
                            "{} | {} | {} | {} | {} | run_at={} | timeout={}s | retries={}",
                            s.id,
                            s.name,
                            s.provider,
                            s.state.as_str(),
                            mode,
                            s.run_at,
                            s.timeout_secs,
                            s.retries
                        )
                    })
                    .collect::<Vec<String>>()
                    .join("\n"),
            ),
        );
        Ok(())
    }

    pub async fn dispatch_due_schedules(&self, limit: usize) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let due = self.store.list_due_schedules(&now, limit)?;
        if due.is_empty() {
            self.emit(
                "schedule_dispatch",
                json!({ "due_count": 0, "results": [] }),
                Some("no due schedules".to_string()),
            );
            return Ok(());
        }

        self.emit(
            "schedule_dispatch_started",
            json!({ "due_count": due.len() }),
            Some(format!("dispatching {} due schedule(s)", due.len())),
        );
        for schedule in due {
            self.store
                .update_schedule_state(&schedule.id, ScheduleState::Running)?;

            let outcome = self
                .spawn_and_run(
                    &schedule.name,
                    &schedule.provider,
                    &schedule.prompt,
                    schedule.timeout_secs,
                    schedule.retries,
                    None,
                )
                .await;

            match outcome {
                Ok(agent_id) => {
                    self.store.append_schedule_run(
                        &schedule.id,
                        Some(&agent_id),
                        "succeeded",
                        None,
                    )?;
                    if let Some(expr) = schedule.cron_expr.as_deref() {
                        let cron = Schedule::from_str(expr).with_context(|| {
                            format!("invalid cron expression in schedule {}", schedule.id)
                        })?;
                        let next_run = cron
                            .after(&schedule.run_at)
                            .next()
                            .context("cron expression does not produce a subsequent run")?;
                        self.store
                            .update_schedule_run_at(&schedule.id, &next_run.to_rfc3339())?;
                        self.store
                            .update_schedule_state(&schedule.id, ScheduleState::Scheduled)?;
                    } else {
                        self.store
                            .update_schedule_state(&schedule.id, ScheduleState::Succeeded)?;
                    }
                    self.emit(
                        "schedule_dispatch_result",
                        json!({
                            "schedule_id": schedule.id,
                            "status": "succeeded",
                            "agent_id": agent_id,
                        }),
                        Some(format!(
                            "schedule {} succeeded (agent_id={})",
                            schedule.id, agent_id
                        )),
                    );
                }
                Err(err) => {
                    let error_text = err.to_string();
                    self.store.append_schedule_run(
                        &schedule.id,
                        None,
                        "failed",
                        Some(&error_text),
                    )?;
                    if let Some(expr) = schedule.cron_expr.as_deref() {
                        let cron = Schedule::from_str(expr).with_context(|| {
                            format!("invalid cron expression in schedule {}", schedule.id)
                        })?;
                        let next_run = cron
                            .after(&schedule.run_at)
                            .next()
                            .context("cron expression does not produce a subsequent run")?;
                        self.store
                            .update_schedule_run_at(&schedule.id, &next_run.to_rfc3339())?;
                        self.store
                            .update_schedule_state(&schedule.id, ScheduleState::Scheduled)?;
                    } else {
                        self.store
                            .update_schedule_state(&schedule.id, ScheduleState::Failed)?;
                    }
                    self.emit(
                        "schedule_dispatch_result",
                        json!({
                            "schedule_id": schedule.id,
                            "status": "failed",
                            "error": error_text,
                        }),
                        Some(format!("schedule {} failed: {}", schedule.id, error_text)),
                    );
                }
            }
        }
        Ok(())
    }

    async fn spawn_and_run(
        &self,
        name: &str,
        provider: &str,
        prompt: &str,
        timeout_secs: u64,
        retries: u32,
        sandbox_runtime: Option<String>,
    ) -> Result<String> {
        let id = self.spawn_agent_record(name, provider, prompt)?;
        self.attach(&id, timeout_secs, retries, false, false, sandbox_runtime)
            .await?;
        Ok(id)
    }

    fn spawn_agent_record(&self, name: &str, provider: &str, prompt: &str) -> Result<String> {
        let now = Utc::now();
        let id = Uuid::new_v4().to_string();
        let agent = AgentRecord {
            id: id.clone(),
            name: name.to_string(),
            provider: provider.to_string(),
            prompt: prompt.to_string(),
            state: AgentState::Pending,
            created_at: now,
            updated_at: now,
            attempts: 0,
        };
        self.store.create_agent(&agent)?;
        self.store.append_log(
            &id,
            "info",
            &structured_log_message("spawn", provider, "lifecycle", "agent spawned"),
        )?;
        Ok(id)
    }
}
