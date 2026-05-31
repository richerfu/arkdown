mod cmd;
mod progress;
mod task_cache;

use clap::{Parser, Subcommand};

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
