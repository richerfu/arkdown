//! `arkdown` CLI — package entry and diagnostic fixture runner.
//!
//! **Public API**: `packages/arkdown` Node.js package (`build(config, plugins)`).
//!
//! This binary is intended for internal debugging and fixture testing only.
//! Production OHOS builds must go through `packages/arkdown.build()`.
//! The npm package exposes this binary through `packages/arkdown/js/arkdown-cli.js`.
//!
//! # Intended callers
//!
//! | Caller | Supported |
//! |--------|-----------|
//! | `packages/arkdown` NAPI bridge | Production |
//! | CI integration tests (`tests/`) | Fixture/parity |
//! | Manual debugging | Supported |

#![recursion_limit = "256"]

use clap::{Parser, Subcommand};

use arkdown_napi::cmd;

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static DHAT_ALLOC: dhat::Alloc = dhat::Alloc;

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
  /// Dump the resolved effective build configuration as JSON.
  ///
  /// Implements the hvigor effective-config oracle dump mode (gap analysis
  /// section 3.1).  Use this to compare arkdown's resolved config against
  /// hvigor's `--dump-config` output to detect divergences without running
  /// a full build.
  #[command(name = "dump-config")]
  DumpConfig(cmd::dump_config::DumpConfigArgs),
  /// Print build-profile information like hvigor's `build-info` task
  /// (`hvigorw build-info [-p json=] [-p buildOption=] [-p module=<name>]`)
  #[command(name = "build-info")]
  BuildInfo(cmd::build_info::BuildInfoArgs),
}

#[tokio::main]
async fn main() {
  if let Err(error) = arkdown_napi::RuntimeObservability::install_cli() {
    eprintln!("failed to install arkdown tracing: {error}");
  }
  #[cfg(feature = "dhat-heap")]
  let dhat_profiler = start_dhat_heap_profiler();
  #[cfg(feature = "memory-diagnostics")]
  let memory_sampler = arkdown::memory_diagnostics::start_sampler_from_env();

  let cli = Cli::parse();
  let result: anyhow::Result<()> = match cli.command {
    Commands::Build(args) => cmd::build::BuildCommand::new(args).execute().await,
    Commands::Clean(args) => cmd::clean::run(args),
    Commands::Check(args) => cmd::check::run(args),
    Commands::DumpConfig(args) => cmd::dump_config::run(args),
    Commands::BuildInfo(args) => cmd::build_info::run(args),
  };
  if let Err(e) = result {
    tracing::error!("error: {}", e);
    for cause in e.chain().skip(1) {
      tracing::error!("  caused by: {}", cause);
    }
    #[cfg(feature = "memory-diagnostics")]
    drop(memory_sampler);
    #[cfg(feature = "dhat-heap")]
    drop(dhat_profiler);
    std::process::exit(1);
  }
}

#[cfg(feature = "dhat-heap")]
fn start_dhat_heap_profiler() -> Option<dhat::Profiler> {
  if std::env::var_os("ARKDOWN_DHAT_HEAP").is_none() && std::env::var_os("ARKDOWN_DHAT").is_none() {
    return None;
  }
  let file_name = std::env::var_os("ARKDOWN_DHAT_FILE")
    .map(std::path::PathBuf::from)
    .unwrap_or_else(|| std::path::PathBuf::from(format!("dhat-heap-{}.json", std::process::id())));
  let trim_backtraces = std::env::var("ARKDOWN_DHAT_TRIM_BACKTRACES")
    .ok()
    .and_then(|value| match value.as_str() {
      "none" | "false" | "0" => Some(None),
      _ => value.parse::<usize>().ok().map(Some),
    })
    .unwrap_or(Some(10));
  tracing::info!(
    output = %file_name.display(),
    ?trim_backtraces,
    "dhat heap profiler enabled"
  );
  Some(
    dhat::Profiler::builder()
      .file_name(file_name)
      .trim_backtraces(trim_backtraces)
      .build(),
  )
}
