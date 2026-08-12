//! [`BuildSession`] and [`ModuleCompileContext`] — the core session types.

use std::sync::Arc;

use arkdown_config::{
    build_profile::BuildOptionProfileInventory, ArkdownCustomConfig,
};

use crate::artifacts::layout::ArtifactLayout;
use crate::dependency_graph::snapshot::DependencyGraphSnapshot;
use crate::fingerprint::InputFingerprintSet;
use crate::module::snapshot::ModuleSnapshot;
use crate::paths::PathResolver;
use crate::profiles::store::ProfileStore;
use crate::project::snapshot::ProjectSnapshot;
use crate::resources::plan::ResourcePlan;
use crate::sdk::snapshot::SdkSnapshot;
use crate::WatchPlanInput;

/// The immutable build session, constructed once at startup and shared across
/// all compilation stages.
///
/// All fields are `Arc`-wrapped snapshots so they can be cheaply cloned and
/// passed to parallel tasks.
#[derive(Debug, Clone)]
pub struct BuildSession {
    /// Resolved and normalized arkdown configuration.
    pub project: Arc<ProjectSnapshot>,
    /// Selected SDK metadata.
    pub sdk: Arc<SdkSnapshot>,
    /// Path resolver for project/module/artifact paths.
    pub paths: Arc<PathResolver>,
    /// Content fingerprints for all startup inputs.
    pub fingerprints: Arc<InputFingerprintSet>,
    /// Profile file store (lazily indexed by module and profile kind).
    pub profiles: Arc<ProfileStore>,
    /// Project/module target and build-option models parsed once at startup.
    pub build_options: Arc<BuildOptionProfileInventory>,
    /// Arkdown-owned project overrides loaded once from `arkdown.config.toml`
    /// or its legacy JSON fallback.
    pub custom_config: Arc<ArkdownCustomConfig>,
    /// Typed source, resource, dependency, and configuration watch inventory.
    pub watch_inputs: Arc<Vec<WatchPlanInput>>,
}

impl BuildSession {
    /// Return the compile context for a named module.
    ///
    /// # Errors
    ///
    /// Returns `None` when no module with `module_name` exists in the session.
    pub fn module_context(&self, module_name: &str) -> Option<ModuleCompileContext> {
        let module = self.project.modules.get(module_name)?.clone();
        let deps = self.project.dependency_graph(module_name)?;
        let artifacts = ArtifactLayout::for_module(&module, &self.paths);
        let resource_plan = ResourcePlan::for_module(&module, &deps);

        Some(ModuleCompileContext {
            session: Arc::new(self.clone()),
            module,
            deps,
            artifacts: Arc::new(artifacts),
            resource_plan: Arc::new(resource_plan),
        })
    }
}

/// The per-module compile context passed to all plugins and stage executors.
///
/// This type is the single context object passed through the entire compile
/// pipeline for one OHOS module.
#[derive(Debug, Clone)]
pub struct ModuleCompileContext {
    /// The parent build session (project-level data).
    pub session: Arc<BuildSession>,
    /// This module's snapshot.
    pub module: Arc<ModuleSnapshot>,
    /// Dependency graph for this module.
    pub deps: Arc<DependencyGraphSnapshot>,
    /// Output artifact paths for this module.
    pub artifacts: Arc<ArtifactLayout>,
    /// Resource compilation plan for this module.
    pub resource_plan: Arc<ResourcePlan>,
}
