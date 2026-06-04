mod cmd;
mod progress;
mod task_cache;

use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "arkdown", version, about = "OHOS bundler replacing hvigorw")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build a HAP/HSP/HAR artifact
    Build(cmd::build::BuildArgs),
    /// Remove build outputs
    Clean(cmd::clean::CleanArgs),
    /// Run ETS struct-decoration checks without building (M5 — SPEC-023 lint mode)
    Check(cmd::check::CheckArgs),
}

#[tokio::main]
async fn main() {
    init_tracing();

    let cli = Cli::parse();
    let result: anyhow::Result<()> = match cli.command {
        Commands::Build(args) => cmd::build::run(args).await,
        Commands::Clean(args) => cmd::clean::run(args),
        Commands::Check(args) => cmd::check::run(args),
    };
    if let Err(e) = result {
        eprintln!("error: {}", e);
        for cause in e.chain().skip(1) {
            eprintln!("  caused by: {}", cause);
        }
        std::process::exit(1);
    }
}

fn init_tracing() {
    let filter =
        EnvFilter::try_from_env("ARKDOWN_LOG").or_else(|_| EnvFilter::try_from_default_env());
    let Ok(filter) = filter else { return };

    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .without_time()
        .with_writer(std::io::stderr)
        .try_init();
}
