use rustc_hash::FxHashMap;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use arkdown_core::{HostModuleInfo, ModuleMeta};
use rolldown_common::{ModuleInfo, NormalModule};
use rolldown_plugin::{HookBuildStartArgs, HookNoopReturn, HookUsage, Plugin, PluginContext};

use super::meta_compute::ModuleMetaComputer;
use crate::facts::{BundleFacts, ModuleFact};

// ---------------------------------------------------------------------------
// Shared inner state
// ---------------------------------------------------------------------------

/// Mutable state accumulated across `module_parsed` calls.
///
/// Stored behind `Arc<Mutex<…>>` so it can be shared between the plugin
/// instance (which the caller holds) and the async futures spawned by
/// rolldown's hook runner.
#[derive(Default)]
struct PluginState {
    /// Reverse importer map: `importee_id → [HostModuleInfo, …]`.
    reverse_map: FxHashMap<String, Vec<HostModuleInfo>>,
    /// Per-module facts accumulated as each module is parsed.
    module_facts: BTreeMap<String, ModuleFact>,
}

// ---------------------------------------------------------------------------
// ModuleMetaPlugin
// ---------------------------------------------------------------------------

/// Rolldown plugin that annotates every resolved module with OHOS metadata.
///
/// For each module parsed by rolldown the plugin:
/// 1. Walks up the file tree to locate the nearest `oh-package.json5`.
/// 2. Derives [`ModuleFact`] (package name, version, module name, entry flag).
/// 3. Populates the reverse importer map (importee → host modules).
///
/// After the bundle run, call [`collect_bundle_facts`] to harvest the
/// accumulated [`BundleFacts`].
///
/// # Upstream correspondence
///
/// Replaces `module-meta-info-plugin.js` from the SDK JS plugin set.
pub struct ModuleMetaPlugin {
    project_root: PathBuf,
    module_name: String,
    state: Arc<Mutex<PluginState>>,
}

impl std::fmt::Debug for ModuleMetaPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModuleMetaPlugin")
            .field("module_name", &self.module_name)
            .finish_non_exhaustive()
    }
}

impl ModuleMetaPlugin {
    /// Create a new plugin for the given project root and OHOS module name.
    ///
    /// - `project_root`: absolute path to the project root (stored in
    ///   `ModuleMeta::belong_project_path`).
    /// - `module_name`: OHOS module name from `module.json5` (e.g. `"entry"`).
    pub fn new(project_root: PathBuf, module_name: impl Into<String>) -> Self {
        ModuleMetaPlugin {
            project_root,
            module_name: module_name.into(),
            state: Arc::new(Mutex::new(PluginState::default())),
        }
    }

    /// Snapshot the reverse importer map accumulated so far.
    ///
    /// Returns a `FxHashMap<importee_id, Vec<HostModuleInfo>>` where each
    /// entry records which host modules import the given importee.
    pub fn reverse_map_snapshot(&self) -> FxHashMap<String, Vec<HostModuleInfo>> {
        self.state
            .lock()
            .map(|g| g.reverse_map.clone())
            .unwrap_or_default()
    }

    /// Compute [`ModuleMeta`] for `module_id` using this plugin's context.
    pub fn meta_for(&self, module_id: &str) -> ModuleMeta {
        Self::compute_meta(module_id, &self.project_root, &self.module_name)
    }

    /// Compute [`ModuleMeta`] from module path and project context.
    ///
    /// This is the underlying computation also used by
    /// [`meta_for`](Self::meta_for).
    pub fn compute_meta(module_id: &str, project_root: &Path, module_name: &str) -> ModuleMeta {
        ModuleMetaComputer::compute(module_id, project_root, module_name)
    }

    /// Drain the accumulated [`BundleFacts`] from this plugin.
    ///
    /// Clears the internal `module_facts` map after returning so the plugin
    /// can be reused across incremental builds without double-counting.
    ///
    /// Call this after rolldown's `generate()` / `write()` completes, before
    /// passing the facts to downstream stages (ABC compilation, packaging).
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to the global `moduleMetaInfoMap` that `module-meta-info-plugin.js`
    /// accumulates and exposes to downstream hvigor tasks.
    pub fn collect_bundle_facts(&self) -> BundleFacts {
        let mut state = self.state.lock().expect("module-meta: state lock poisoned");

        let module_facts = std::mem::take(&mut state.module_facts);
        BundleFacts {
            module_facts,
            ..BundleFacts::default()
        }
    }
}

// ---------------------------------------------------------------------------
// Plugin trait impl
// ---------------------------------------------------------------------------

impl Plugin for ModuleMetaPlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("module-meta")
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::ModuleParsed | HookUsage::BuildStart
    }

    fn module_parsed(
        &self,
        _ctx: &PluginContext,
        module_info: Arc<ModuleInfo>,
        _normal_module: &NormalModule,
    ) -> impl std::future::Future<Output = HookNoopReturn> + Send {
        let id_str = module_info.id.as_str().to_owned();
        let is_entry = module_info.is_entry;
        let imported_ids: Vec<String> = module_info
            .imported_ids
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect();
        let dynamically_imported_ids: Vec<String> = module_info
            .dynamically_imported_ids
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect();
        let project_root = self.project_root.clone();
        let module_name = self.module_name.clone();
        let state = Arc::clone(&self.state);

        async move {
            let meta = ModuleMetaPlugin::compute_meta(&id_str, &project_root, &module_name);

            // ── reverse importer map ──────────────────────────────────────
            let host_dep = meta.pkg_name.clone().unwrap_or_default();
            let host_mod = meta.module_name.clone().unwrap_or_default();

            // ── ModuleFact for BundleFacts ────────────────────────────────
            let fact = ModuleFact {
                module_id: id_str.clone(),
                package_name: meta.pkg_name,
                package_version: meta.pkg_version,
                is_entry,
                module_name: meta.module_name,
            };

            let mut locked = state
                .lock()
                .map_err(|e| anyhow::anyhow!("module-meta: failed to lock state: {e}"))?;

            // Insert the fact (last write wins for duplicate IDs).
            locked.module_facts.insert(id_str.clone(), fact);

            // Update reverse importer map for every import edge.
            for id in imported_ids.iter().chain(dynamically_imported_ids.iter()) {
                locked
                    .reverse_map
                    .entry(id.clone())
                    .or_default()
                    .push(HostModuleInfo {
                        host_dependency_name: host_dep.clone(),
                        host_module_name: host_mod.clone(),
                    });
            }

            Ok(())
        }
    }

    fn build_start(
        &self,
        _ctx: &PluginContext,
        _args: &HookBuildStartArgs<'_>,
    ) -> impl std::future::Future<Output = HookNoopReturn> + Send {
        let state = Arc::clone(&self.state);
        async move {
            let mut locked = state
                .lock()
                .map_err(|e| anyhow::anyhow!("module-meta: failed to lock state: {e}"))?;
            locked.reverse_map.clear();
            locked.module_facts.clear();
            Ok(())
        }
    }
}
