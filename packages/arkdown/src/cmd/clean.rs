use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

#[derive(Args)]
pub struct CleanArgs {
  #[arg(long)]
  pub project: Option<PathBuf>,
}

pub fn run(args: CleanArgs) -> Result<()> {
  let project = args
    .project
    .unwrap_or_else(|| std::env::current_dir().expect("failed to get current directory"));
  let build_dir = project.join("build");
  if build_dir.exists() {
    std::fs::remove_dir_all(&build_dir)?;
    tracing::info!(path = %build_dir.display(), "removed build directory");
  } else {
    tracing::info!("nothing to clean");
  }
  Ok(())
}
