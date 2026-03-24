use std::path::PathBuf;

use anyhow::{Result, bail};
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};

use crate::app::{App, OutputMode, OutputOptions};
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
    }
}
