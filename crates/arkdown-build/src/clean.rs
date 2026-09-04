//! Rust-owned clean application invoked by the typed N-API service.

use std::path::{Path, PathBuf};

use arkdown_config::build_profile::RawBuildProfile;
use arkdown_native::{NativeCleanStage, NativeSdkToolchain};
use arkdown_session::{SdkSnapshot, SdkToolKind};
use thiserror::Error;

#[derive(Debug)]
pub struct CleanRequest {
    /// Project root. The process working directory is used when omitted by the
    /// JavaScript entrypoint.
    pub project: Option<PathBuf>,
}

#[derive(Debug, Error)]
pub enum CleanError {
    #[error("ARKDOWN_CLEAN_CURRENT_DIR: failed to get current directory: {0}")]
    CurrentDirectory(#[source] std::io::Error),
    #[error("ARKDOWN_CLEAN_PROJECT_NOT_FOUND: project root '{path}' is unavailable: {source}")]
    ProjectRoot {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("clean project config error: {0}")]
    ProjectConfig(#[source] arkdown_config::ConfigError),
    #[error("project clean failed: {0}")]
    ProjectClean(#[source] arkdown_native::NativeBuildError),
    #[error("module clean failed for '{path}': {source}")]
    ModuleClean {
        path: PathBuf,
        #[source]
        source: arkdown_native::NativeBuildError,
    },
}

/// Owns project discovery, SDK lookup, output planning, and clean execution.
pub struct CleanApplication {
    request: CleanRequest,
}

impl CleanApplication {
    pub fn new(request: CleanRequest) -> Self {
        Self { request }
    }

    pub fn execute(self) -> Result<(), CleanError> {
        let project_root = self.project_root()?;
        let module_roots = self.module_roots(&project_root)?;
        let native_toolchain = CleanNativeToolchainResolver::new().resolve();

        NativeCleanStage::for_project(&project_root)
            .execute()
            .map_err(CleanError::ProjectClean)?;
        for module_root in module_roots {
            NativeCleanStage::new(&project_root, &module_root, &native_toolchain)
                .execute()
                .map_err(|source| CleanError::ModuleClean {
                    path: module_root,
                    source,
                })?;
        }
        tracing::info!(project = %project_root.display(), "clean completed");
        Ok(())
    }

    fn project_root(&self) -> Result<PathBuf, CleanError> {
        let configured = match self.request.project.as_ref() {
            Some(project) => project.clone(),
            None => std::env::current_dir().map_err(CleanError::CurrentDirectory)?,
        };
        dunce::canonicalize(&configured).map_err(|source| CleanError::ProjectRoot {
            path: configured,
            source,
        })
    }

    fn module_roots(&self, project_root: &Path) -> Result<Vec<PathBuf>, CleanError> {
        let profile = RawBuildProfile::load(project_root).map_err(CleanError::ProjectConfig)?;
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
/// leaves deterministic Rust-owned output removal enabled and skips only the
/// optional Ninja clean pass over retained `.cxx` trees.
struct CleanNativeToolchainResolver;

impl CleanNativeToolchainResolver {
    fn new() -> Self {
        Self
    }

    fn resolve(&self) -> NativeSdkToolchain {
        let ninja = arkdown_config::sdk::discover_sdk(None)
            .map_err(
                |error| tracing::debug!(%error, "clean could not discover an SDK native toolchain"),
            )
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
