use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "prism", version, about = "Claude Code doc-sync plugin")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Start,
    Stop,
    Status,
    Enrich,
    Hook {
        event: String,
    },
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Start => todo!("port from prism-old cmd_start"),
        Commands::Stop => todo!("port from prism-old cmd_stop"),
        Commands::Status => todo!("port from prism-old cmd_status"),
        Commands::Enrich => todo!("port from prism-old cmd_enrich"),
        Commands::Hook { event: _ } => todo!("port from prism-old cmd_hook (session-start + post-tool-use only)"),
    }
}
