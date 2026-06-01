mod cmd;
mod progress;
mod task_cache;

use clap::{Parser, Subcommand};
use tracing::Level;

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
}

#[tokio::main]
async fn main() {
    init_tracing();

    let cli = Cli::parse();
    let result = match cli.command {
        Commands::Build(args) => cmd::build::run(args).await,
        Commands::Clean(args) => cmd::clean::run(args),
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
    let Some(level) = std::env::var("ARKDOWN_LOG")
        .ok()
        .and_then(|value| parse_log_level(&value))
    else {
        return;
    };

    let _ = tracing_subscriber::fmt()
        .with_max_level(level)
        .with_target(false)
        .without_time()
        .with_writer(std::io::stderr)
        .try_init();
}

fn parse_log_level(value: &str) -> Option<Level> {
    match value.trim().to_ascii_lowercase().as_str() {
        "trace" => Some(Level::TRACE),
        "debug" => Some(Level::DEBUG),
        "info" => Some(Level::INFO),
        "warn" | "warning" => Some(Level::WARN),
        "error" => Some(Level::ERROR),
        _ => None,
    }
}
