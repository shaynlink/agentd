use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use cron::Schedule;
use tokio::time::timeout;
use uuid::Uuid;

use crate::adapters::providers;
use crate::adapters::store::sqlite::SqliteStore;
use crate::domain::agent::{AgentRecord, AgentState};
use crate::domain::plan::Plan;
use crate::domain::schedule::{ScheduleRecord, ScheduleState};
use crate::ports::provider::ProviderRunRequest;
use crate::ports::store::StateStore;

pub struct App {
    store: SqliteStore,
}

impl App {
    pub fn new(db_path: String) -> Result<Self> {
        let store = SqliteStore::new(db_path);
        store.init()?;
        Ok(Self { store })
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
            println!("generated plan written to {}", path.display());
        } else {
            println!("{serialized}");
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

        println!(
            "running plan '{}' with {} step(s)",
            plan.name,
            plan.steps.len()
        );
        for step in plan.steps {
            let provider_name = step.provider.as_deref().unwrap_or(default_provider);
            self.spawn_and_run(
                &step.name,
                provider_name,
                &step.prompt,
                step.timeout_secs.unwrap_or(60),
                step.retries.unwrap_or(0),
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
    ) -> Result<()> {
        self.spawn_agent_record(name, provider, prompt)?;
        println!("spawned agent '{}' (provider={provider})", name);
        println!("use 'attach' to run now or inspect logs/status");
        println!("default policy timeout={timeout_secs}s retries={retries}");
        Ok(())
    }

    pub async fn attach(&self, agent_id: &str, timeout_secs: u64, retries: u32) -> Result<()> {
        let Some(agent) = self.store.get_agent(agent_id)? else {
            bail!("agent not found: {agent_id}");
        };

        self.store.update_state(agent_id, AgentState::Running)?;
        self.store
            .append_log(agent_id, "info", "attach requested")?;

        let mut attempt = 0;
        loop {
            attempt += 1;
            self.store.bump_attempts(agent_id)?;

            let provider = providers::build_provider(&agent.provider)?;
            let req = ProviderRunRequest {
                agent_id: agent.id.clone(),
                prompt: agent.prompt.clone(),
                timeout_secs,
            };

            let result = timeout(Duration::from_secs(timeout_secs), provider.run_agent(req)).await;
            match result {
                Ok(Ok(done)) => {
                    self.store.update_state(agent_id, AgentState::Succeeded)?;
                    self.store.append_log(agent_id, "info", &done.output)?;
                    println!("agent {agent_id} succeeded");
                    println!("{}", done.output);
                    return Ok(());
                }
                Ok(Err(err)) => {
                    self.store
                        .append_log(agent_id, "error", &format!("provider error: {err}"))?;
                    if attempt > retries {
                        self.store.update_state(agent_id, AgentState::Failed)?;
                        bail!("agent {agent_id} failed after {attempt} attempt(s): {err}");
                    }
                }
                Err(_) => {
                    self.store
                        .append_log(agent_id, "error", "execution timed out")?;
                    if attempt > retries {
                        self.store.update_state(agent_id, AgentState::TimedOut)?;
                        bail!("agent {agent_id} timed out after {attempt} attempt(s)");
                    }
                }
            }
        }
    }

    pub fn list(&self) -> Result<()> {
        let agents = self.store.list_agents()?;
        if agents.is_empty() {
            println!("no agents found");
            return Ok(());
        }

        for a in agents {
            println!(
                "{} | {} | {} | {} | attempts={}",
                a.id,
                a.name,
                a.provider,
                a.state.as_str(),
                a.attempts
            );
        }
        Ok(())
    }

    pub fn pause(&self, agent_id: &str) -> Result<()> {
        self.store.update_state(agent_id, AgentState::Paused)?;
        self.store.append_log(agent_id, "info", "paused")?;
        println!("paused {agent_id}");
        Ok(())
    }

    pub fn resume(&self, agent_id: &str) -> Result<()> {
        self.store.update_state(agent_id, AgentState::Running)?;
        self.store.append_log(agent_id, "info", "resumed")?;
        println!("resumed {agent_id}");
        Ok(())
    }

    pub async fn stop(&self, agent_id: &str) -> Result<()> {
        let Some(agent) = self.store.get_agent(agent_id)? else {
            bail!("agent not found: {agent_id}");
        };
        let provider = providers::build_provider(&agent.provider)?;
        let _ = provider.cancel(agent_id).await;
        self.store.update_state(agent_id, AgentState::Cancelled)?;
        self.store
            .append_log(agent_id, "info", "stopped/cancelled")?;
        println!("stopped {agent_id}");
        Ok(())
    }

    pub fn status(&self, agent_id: &str) -> Result<()> {
        if let Some(agent) = self.store.get_agent(agent_id)? {
            println!(
                "{} | {} | {} | {} | created={} | updated={} | attempts={}",
                agent.id,
                agent.name,
                agent.provider,
                agent.state.as_str(),
                agent.created_at,
                agent.updated_at,
                agent.attempts
            );
            return Ok(());
        }
        bail!("agent not found: {agent_id}")
    }

    pub fn logs(&self, agent_id: &str, limit: usize) -> Result<()> {
        let logs = self.store.get_logs(agent_id, limit)?;
        for log in logs.into_iter().rev() {
            println!("{} [{}] {}", log.ts, log.level, log.message);
        }
        Ok(())
    }

    pub fn schedule_run_at(
        &self,
        name: &str,
        provider: &str,
        prompt: &str,
        run_at: DateTime<Utc>,
        timeout_secs: u64,
        retries: u32,
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
        println!(
            "scheduled '{}' at {} (provider={})",
            schedule.name, schedule.run_at, schedule.provider
        );
        println!("schedule_id={}", schedule.id);
        Ok(())
    }

    pub fn schedule_cron(
        &self,
        name: &str,
        provider: &str,
        prompt: &str,
        cron_expr: &str,
        timeout_secs: u64,
        retries: u32,
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
        println!(
            "scheduled cron '{}' next run at {} (provider={})",
            record.name, record.run_at, record.provider
        );
        println!("schedule_id={}", record.id);
        Ok(())
    }

    pub fn list_schedules(&self, limit: usize) -> Result<()> {
        let schedules = self.store.list_schedules(limit)?;
        if schedules.is_empty() {
            println!("no schedules found");
            return Ok(());
        }

        for s in schedules {
            let mode = s
                .cron_expr
                .as_ref()
                .map(|expr| format!("cron={expr}"))
                .unwrap_or_else(|| "run-at".to_string());
            println!(
                "{} | {} | {} | {} | {} | run_at={} | timeout={}s | retries={}",
                s.id,
                s.name,
                s.provider,
                s.state.as_str(),
                mode,
                s.run_at,
                s.timeout_secs,
                s.retries
            );
        }
        Ok(())
    }

    pub async fn dispatch_due_schedules(&self, limit: usize) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let due = self.store.list_due_schedules(&now, limit)?;
        if due.is_empty() {
            println!("no due schedules");
            return Ok(());
        }

        println!("dispatching {} due schedule(s)", due.len());
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
                    println!("schedule {} succeeded (agent_id={})", schedule.id, agent_id);
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
                    println!("schedule {} failed: {}", schedule.id, error_text);
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
    ) -> Result<String> {
        let id = self.spawn_agent_record(name, provider, prompt)?;
        self.attach(&id, timeout_secs, retries).await?;
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
        self.store.append_log(&id, "info", "agent spawned")?;
        Ok(id)
    }
}
