use std::path::PathBuf;

use anyhow::{Result, bail};
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};

use crate::app::{App, AuditListFilters, OutputMode, OutputOptions};
use crate::config::AppConfig;

#[derive(Debug, Parser)]
#[command(
    name = "agentd",
    version,
    about = "Provider-agnostic sub-agent orchestrator"
)]
struct Cli {
    #[arg(long, default_value = "./.agentd/state.db")]
    db_path: String,

    /// Output mode: text (human), json (single document), jsonl (one event per line)
    #[arg(long, default_value = "text")]
    output: String,

    /// Suppress successful command output
    #[arg(long, default_value_t = false)]
    quiet: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Execute a plan file (YAML/JSON)
    RunPlan {
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        provider: Option<String>,
    },
    /// Ask provider to generate a plan from a goal
    PlanGenerate {
        #[arg(long)]
        goal: String,
        #[arg(long)]
        provider: Option<String>,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Spawn an agent record (not automatically executed)
    Spawn {
        #[arg(long)]
        name: String,
        #[arg(long)]
        prompt: String,
        #[arg(long)]
        provider: Option<String>,
        #[arg(long)]
        sandbox_runtime: Option<String>,
        #[arg(long, default_value_t = 60)]
        timeout_secs: u64,
        #[arg(long, default_value_t = 0)]
        retries: u32,
    },
    /// Attach to an existing agent and execute it now
    Attach {
        #[arg(long)]
        id: String,
        #[arg(long, default_value_t = 60)]
        timeout_secs: u64,
        #[arg(long, default_value_t = 0)]
        retries: u32,
        #[arg(long, default_value_t = true)]
        stream: bool,
        #[arg(long, default_value_t = false)]
        json_lines: bool,
        #[arg(long)]
        sandbox_runtime: Option<String>,
    },
    /// List all agents
    List {
        #[arg(long)]
        state: Option<String>,
        #[arg(long)]
        provider: Option<String>,
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long, default_value_t = false)]
        ids_only: bool,
        #[arg(long)]
        sort_by: Option<String>,
    },
    /// Pause an agent
    Pause {
        #[arg(long)]
        id: String,
    },
    /// Resume an agent
    Resume {
        #[arg(long)]
        id: String,
    },
    /// Stop/cancel an agent
    Stop {
        #[arg(long)]
        id: String,
    },
    /// Show one agent status
    Status {
        #[arg(long)]
        id: String,
    },
    /// Show logs for one agent
    Logs {
        #[arg(long)]
        id: String,
        #[arg(long, default_value_t = 100)]
        limit: usize,
        #[arg(long)]
        level: Option<String>,
        #[arg(long)]
        contains: Option<String>,
    },
    /// Show security audit events (file/sqlite backend)
    AuditList {
        #[arg(long, default_value_t = 100)]
        limit: usize,
        #[arg(long)]
        role: Option<String>,
        #[arg(long)]
        allowed: Option<bool>,
        #[arg(long)]
        runtime: Option<String>,
        #[arg(long)]
        agent_id: Option<String>,
        #[arg(long)]
        since: Option<String>,
        #[arg(long)]
        until: Option<String>,
    },
    /// Create an RBAC role
    RbacCreateRole {
        #[arg(long)]
        name: String,
        #[arg(long)]
        description: Option<String>,
    },
    /// Create an RBAC policy
    RbacCreatePolicy {
        #[arg(long)]
        name: String,
        #[arg(long)]
        resource_type: String,
        #[arg(long)]
        action: String,
        #[arg(long)]
        resource_pattern: String,
        #[arg(long)]
        effect: String,
    },
    /// Bind an RBAC role to a subject
    RbacBindRole {
        #[arg(long, default_value = "runtime_role")]
        subject_type: String,
        #[arg(long)]
        subject: String,
        #[arg(long)]
        role: String,
    },
    /// Attach an RBAC policy to a role
    RbacAttachPolicy {
        #[arg(long)]
        role: String,
        #[arg(long)]
        policy: String,
    },
    /// List RBAC roles, policies, bindings and assignments
    RbacList,
    /// Create a one-shot schedule at an RFC3339 UTC datetime
    ScheduleRunAt {
        #[arg(long)]
        name: String,
        #[arg(long)]
        prompt: String,
        #[arg(long)]
        run_at: String,
        #[arg(long)]
        provider: Option<String>,
        #[arg(long)]
        sandbox_runtime: Option<String>,
        #[arg(long, default_value_t = 60)]
        timeout_secs: u64,
        #[arg(long, default_value_t = 0)]
        retries: u32,
    },
    /// Create a recurring schedule from a cron expression
    ScheduleCron {
        #[arg(long)]
        name: String,
        #[arg(long)]
        prompt: String,
        #[arg(long)]
        cron: String,
        #[arg(long)]
        provider: Option<String>,
        #[arg(long)]
        sandbox_runtime: Option<String>,
        #[arg(long, default_value_t = 60)]
        timeout_secs: u64,
        #[arg(long, default_value_t = 0)]
        retries: u32,
    },
    /// List schedules
    ScheduleList {
        #[arg(long, default_value_t = 100)]
        limit: usize,
    },
    /// Dispatch due schedules (state=scheduled and run_at <= now)
    ScheduleDispatchDue {
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    /// Create a branch in a git repository
    VersionBranchCreate {
        #[arg(long)]
        repo: PathBuf,
        #[arg(long)]
        branch: String,
        #[arg(long)]
        from_ref: Option<String>,
    },
    /// List branches in a git repository
    VersionBranchList {
        #[arg(long)]
        repo: PathBuf,
        #[arg(long, default_value_t = false)]
        report_json: bool,
    },
    /// Show diff between two refs
    VersionDiff {
        #[arg(long)]
        repo: PathBuf,
        #[arg(long)]
        from_ref: String,
        #[arg(long)]
        to_ref: String,
        #[arg(long, default_value_t = false)]
        report_json: bool,
    },
    /// Merge source branch into target branch
    VersionMerge {
        #[arg(long)]
        repo: PathBuf,
        #[arg(long)]
        source: String,
        #[arg(long)]
        target: String,
        #[arg(long, default_value_t = true)]
        no_ff: bool,
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        #[arg(long, default_value_t = false)]
        report_json: bool,
    },
    /// Hard rollback to a ref (destructive)
    VersionRollback {
        #[arg(long)]
        repo: PathBuf,
        #[arg(long)]
        to_ref: String,
        #[arg(long, default_value_t = false)]
        confirm_hard_reset: bool,
    },
}

pub async fn run() -> Result<()> {
    let cli = Cli::parse();
    let output_mode = match cli.output.to_ascii_lowercase().as_str() {
        "text" => OutputMode::Text,
        "json" => OutputMode::Json,
        "jsonl" => OutputMode::Jsonl,
        "tsv" => OutputMode::Tsv,
        other => {
            bail!("invalid --output value: {other}. expected one of: text|json|jsonl|tsv");
        }
    };

    let config = AppConfig::load()?;
    let default_provider = config.default_provider;
    let app = App::new(
        cli.db_path,
        OutputOptions {
            mode: output_mode,
            quiet: cli.quiet,
        },
    )?;

    match cli.command {
        Commands::RunPlan { file, provider } => {
            let provider = provider.unwrap_or_else(|| default_provider.clone());
            app.run_plan(&file, &provider).await
        }
        Commands::PlanGenerate {
            goal,
            provider,
            output,
        } => {
            let provider = provider.unwrap_or_else(|| default_provider.clone());
            app.plan_generate(&provider, &goal, output.as_deref()).await
        }
        Commands::Spawn {
            name,
            prompt,
            provider,
            sandbox_runtime,
            timeout_secs,
            retries,
        } => {
            let provider = provider.unwrap_or_else(|| default_provider.clone());
            app.spawn(
                &name,
                &provider,
                &prompt,
                timeout_secs,
                retries,
                sandbox_runtime,
            )
            .await
        }
        Commands::Attach {
            id,
            timeout_secs,
            retries,
            stream,
            json_lines,
            sandbox_runtime,
        } => {
            app.attach(
                &id,
                timeout_secs,
                retries,
                stream,
                json_lines,
                sandbox_runtime,
            )
            .await
        }
        Commands::List {
            state,
            provider,
            limit,
            ids_only,
            sort_by,
        } => app.list(
            state.as_deref(),
            provider.as_deref(),
            limit,
            ids_only,
            sort_by.as_deref(),
        ),
        Commands::Pause { id } => app.pause(&id),
        Commands::Resume { id } => app.resume(&id),
        Commands::Stop { id } => app.stop(&id).await,
        Commands::Status { id } => app.status(&id),
        Commands::Logs {
            id,
            limit,
            level,
            contains,
        } => app.logs(&id, limit, level.as_deref(), contains.as_deref()),
        Commands::AuditList {
            limit,
            role,
            allowed,
            runtime,
            agent_id,
            since,
            until,
        } => {
            app.audit_list(
                limit,
                AuditListFilters {
                    role: role.as_deref(),
                    allowed,
                    runtime: runtime.as_deref(),
                    agent_id: agent_id.as_deref(),
                    since: since.as_deref(),
                    until: until.as_deref(),
                },
            )
            .await
        }
        Commands::RbacCreateRole { name, description } => {
            app.rbac_create_role(&name, description.as_deref()).await
        }
        Commands::RbacCreatePolicy {
            name,
            resource_type,
            action,
            resource_pattern,
            effect,
        } => {
            app.rbac_create_policy(
                &name,
                &resource_type,
                &action,
                &resource_pattern,
                &effect,
            )
            .await
        }
        Commands::RbacBindRole {
            subject_type,
            subject,
            role,
        } => app.rbac_bind_role(&subject_type, &subject, &role).await,
        Commands::RbacAttachPolicy { role, policy } => {
            app.rbac_attach_policy(&role, &policy).await
        }
        Commands::RbacList => app.rbac_list().await,
        Commands::ScheduleRunAt {
            name,
            prompt,
            run_at,
            provider,
            sandbox_runtime,
            timeout_secs,
            retries,
        } => {
            let provider = provider.unwrap_or_else(|| default_provider.clone());
            let run_at = DateTime::parse_from_rfc3339(&run_at)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| anyhow::anyhow!("invalid run_at (expected RFC3339): {e}"))?;
            app.schedule_run_at(
                &name,
                &provider,
                &prompt,
                run_at,
                timeout_secs,
                retries,
                sandbox_runtime,
            )
        }
        Commands::ScheduleList { limit } => app.list_schedules(limit),
        Commands::ScheduleCron {
            name,
            prompt,
            cron,
            provider,
            sandbox_runtime,
            timeout_secs,
            retries,
        } => {
            let provider = provider.unwrap_or_else(|| default_provider.clone());
            app.schedule_cron(
                &name,
                &provider,
                &prompt,
                &cron,
                timeout_secs,
                retries,
                sandbox_runtime,
            )
        }
        Commands::ScheduleDispatchDue { limit } => app.dispatch_due_schedules(limit).await,
        Commands::VersionBranchCreate {
            repo,
            branch,
            from_ref,
        } => app.version_branch_create(&repo, &branch, from_ref.as_deref()),
        Commands::VersionBranchList { repo, report_json } => {
            app.version_branch_list(&repo, report_json)
        }
        Commands::VersionDiff {
            repo,
            from_ref,
            to_ref,
            report_json,
        } => app.version_diff(&repo, &from_ref, &to_ref, report_json),
        Commands::VersionMerge {
            repo,
            source,
            target,
            no_ff,
            dry_run,
            report_json,
        } => app.version_merge(&repo, &source, &target, no_ff, dry_run, report_json),
        Commands::VersionRollback {
            repo,
            to_ref,
            confirm_hard_reset,
        } => app.version_rollback_hard(&repo, &to_ref, confirm_hard_reset),
    }
}
