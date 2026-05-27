use std::path::PathBuf;
use clap::Args;
use anyhow::Result;

#[derive(Args)]
pub struct CleanArgs {
    #[arg(long)]
    pub project: Option<PathBuf>,
}

pub fn run(args: CleanArgs) -> Result<()> {
    let project = args.project.unwrap_or_else(|| {
        std::env::current_dir().expect("failed to get current directory")
    });
    let build_dir = project.join("build");
    if build_dir.exists() {
        std::fs::remove_dir_all(&build_dir)?;
        println!("Removed {}", build_dir.display());
    } else {
        println!("Nothing to clean.");
    }
    Ok(())
}
