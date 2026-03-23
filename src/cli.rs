use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::app::App;

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
        #[arg(long, default_value = "mock")]
        provider: String,
    },
    /// Ask provider to generate a plan from a goal
    PlanGenerate {
        #[arg(long)]
        goal: String,
        #[arg(long, default_value = "mock")]
        provider: String,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Spawn an agent record (not automatically executed)
    Spawn {
        #[arg(long)]
        name: String,
        #[arg(long)]
        prompt: String,
        #[arg(long, default_value = "mock")]
        provider: String,
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
}

pub async fn run() -> Result<()> {
    let cli = Cli::parse();
    let app = App::new(cli.db_path)?;

    match cli.command {
        Commands::RunPlan { file, provider } => app.run_plan(&file, &provider).await,
        Commands::PlanGenerate {
            goal,
            provider,
            output,
        } => app.plan_generate(&provider, &goal, output.as_deref()).await,
        Commands::Spawn {
            name,
            prompt,
            provider,
            timeout_secs,
            retries,
        } => {
            app.spawn(&name, &provider, &prompt, timeout_secs, retries)
                .await
        }
        Commands::Attach {
            id,
            timeout_secs,
            retries,
        } => app.attach(&id, timeout_secs, retries).await,
        Commands::List => app.list(),
        Commands::Pause { id } => app.pause(&id),
        Commands::Resume { id } => app.resume(&id),
        Commands::Stop { id } => app.stop(&id).await,
        Commands::Status { id } => app.status(&id),
        Commands::Logs { id, limit } => app.logs(&id, limit),
    }
}
