use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use arkdown_config::build_profile::RawBuildProfile;
use arkdown_session::{SdkSnapshot, SdkToolKind};

use arkdown_native::{NativeCleanStage, NativeSdkToolchain};

#[derive(Debug)]
pub struct CleanRequest {
  /// Project root (default: current directory).
  pub project: Option<PathBuf>,
}

pub fn run(args: CleanRequest) -> Result<()> {
  CleanCommandRunner::new(args).run()
}

/// Owns the complete direct clean command. Module roots are read from the
/// project model; the JavaScript boundary never inspects the project
/// filesystem or construct output paths.
struct CleanCommandRunner {
  args: CleanRequest,
}

impl CleanCommandRunner {
  fn new(args: CleanRequest) -> Self {
    Self { args }
  }

  fn run(self) -> Result<()> {
    let project_root = self.project_root()?;
    let module_roots = self.module_roots(&project_root)?;
    let native_toolchain = CleanNativeToolchainResolver::new().resolve();

    NativeCleanStage::for_project(&project_root)
      .execute()
      .map_err(|error| anyhow::anyhow!("project clean failed: {error}"))?;
    for module_root in module_roots {
      NativeCleanStage::new(&project_root, &module_root, &native_toolchain)
        .execute()
        .map_err(|error| {
          anyhow::anyhow!(
            "module clean failed for '{}': {error}",
            module_root.display()
          )
        })?;
    }
    tracing::info!(project = %project_root.display(), "clean completed");
    Ok(())
  }

  fn project_root(&self) -> Result<PathBuf> {
    let configured = match self.args.project.as_ref() {
      Some(project) => project.clone(),
      None => std::env::current_dir().context("failed to get current directory")?,
    };
    dunce::canonicalize(&configured).with_context(|| {
      format!(
        "ARKDOWN_CLEAN_PROJECT_NOT_FOUND: project root '{}' is unavailable",
        configured.display()
      )
    })
  }

  fn module_roots(&self, project_root: &Path) -> Result<Vec<PathBuf>> {
    let profile = RawBuildProfile::load(project_root)
      .map_err(|error| anyhow::anyhow!("clean project config error: {error}"))?;
    let mut roots = profile
      .modules
      .into_iter()
      .map(|module| arkdown_path::lexical_normalize(&project_root.join(module.src_path)))
      .collect::<Vec<_>>();
    roots.sort();
    roots.dedup();
    Ok(roots)
  }
}

/// Resolves only the SDK tool needed by CMake cache cleanup. A missing SDK
/// does not prevent deterministic removal of Rust-owned build outputs; it only
/// disables the optional `ninja clean` pass over retained `.cxx` trees.
struct CleanNativeToolchainResolver;

impl CleanNativeToolchainResolver {
  fn new() -> Self {
    Self
  }

  fn resolve(&self) -> NativeSdkToolchain {
    let ninja = arkdown_config::sdk::discover_sdk(None)
      .map_err(|error| tracing::debug!(%error, "clean could not discover an SDK native toolchain"))
      .ok()
      .and_then(|config| {
        SdkSnapshot::from_config(&config)
          .map_err(
            |error| tracing::debug!(%error, "clean could not snapshot the SDK native toolchain"),
          )
          .ok()
      })
      .and_then(|sdk| sdk.tool_path(SdkToolKind::NativeNinja))
      .map(|tool| tool.path);
    NativeSdkToolchain::for_clean(ninja)
  }
}
