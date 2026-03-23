use std::path::PathBuf;

use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};

use crate::app::App;
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
    },
    /// List all agents
    List,
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
    let config = AppConfig::load()?;
    let default_provider = config.default_provider;
    let app = App::new(cli.db_path)?;

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
            timeout_secs,
            retries,
        } => {
            let provider = provider.unwrap_or_else(|| default_provider.clone());
            app.spawn(&name, &provider, &prompt, timeout_secs, retries)
                .await
        }
        Commands::Attach {
            id,
            timeout_secs,
            retries,
            stream,
            json_lines,
        } => {
            app.attach(&id, timeout_secs, retries, stream, json_lines)
                .await
        }
        Commands::List => app.list(),
        Commands::Pause { id } => app.pause(&id),
        Commands::Resume { id } => app.resume(&id),
        Commands::Stop { id } => app.stop(&id).await,
        Commands::Status { id } => app.status(&id),
        Commands::Logs { id, limit } => app.logs(&id, limit),
        Commands::ScheduleRunAt {
            name,
            prompt,
            run_at,
            provider,
            timeout_secs,
            retries,
        } => {
            let provider = provider.unwrap_or_else(|| default_provider.clone());
            let run_at = DateTime::parse_from_rfc3339(&run_at)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| anyhow::anyhow!("invalid run_at (expected RFC3339): {e}"))?;
            app.schedule_run_at(&name, &provider, &prompt, run_at, timeout_secs, retries)
        }
        Commands::ScheduleList { limit } => app.list_schedules(limit),
        Commands::ScheduleCron {
            name,
            prompt,
            cron,
            provider,
            timeout_secs,
            retries,
        } => {
            let provider = provider.unwrap_or_else(|| default_provider.clone());
            app.schedule_cron(&name, &provider, &prompt, &cron, timeout_secs, retries)
        }
        Commands::ScheduleDispatchDue { limit } => app.dispatch_due_schedules(limit).await,
    }
}
