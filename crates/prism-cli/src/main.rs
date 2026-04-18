use clap::{Parser, Subcommand};

mod cmd_enrich;
mod cmd_fix;
mod cmd_hook;
mod cmd_lint;
mod cmd_start;
mod cmd_status;
mod cmd_stop;

#[derive(Parser)]
#[command(name = "prism", version, about = "Claude Code doc-sync plugin")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Install hooks and scaffold .prism/ in the current project
    Start,
    /// Remove hooks from .claude/settings.json
    Stop,
    /// Show drift and enrichment state
    Status,
    /// Drain pending enrichment directives via Haiku
    Enrich,
    /// Audit the project for ICM spec violations (read-only)
    Lint,
    /// Drain pending ICM fix directives via Haiku
    Fix,
    /// Dispatch a Claude Code hook event (internal)
    Hook {
        /// Event name: session-start | post-tool-use
        event: String,
    },
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Start => cmd_start::run(),
        Commands::Stop => cmd_stop::run(),
        Commands::Status => cmd_status::run(),
        Commands::Enrich => cmd_enrich::run(),
        Commands::Lint => cmd_lint::run(),
        Commands::Fix => cmd_fix::run(),
        Commands::Hook { event } => cmd_hook::run(&event),
    }
}
