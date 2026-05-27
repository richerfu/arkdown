use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use arkdown_core::{
    ArkdownError, ArkdownPlugin, BuildContext, HostModuleInfo, LoadContext, Module,
    ModuleMeta, ModuleParsedContext, ModuleSource, PluginResult,
};

use crate::meta_compute;

/// Plugin that annotates each module with OHOS metadata derived from the
/// nearest `oh-package.json5` in the file tree.
///
/// Also maintains a reverse-import map (host_modules_info) in memory so
/// consumers can ask "who imports module X?" without a second graph pass.
pub struct ModuleMetaPlugin {
    project_root: PathBuf,
    module_name: String,
    /// module_id → list of modules that import it, accumulated via `module_parsed`.
    reverse_map: Arc<Mutex<HashMap<String, Vec<HostModuleInfo>>>>,
}

impl ModuleMetaPlugin {
    pub fn new(project_root: PathBuf, module_name: impl Into<String>) -> Self {
        ModuleMetaPlugin {
            project_root,
            module_name: module_name.into(),
            reverse_map: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Return a snapshot of the reverse-import map accumulated so far.
    ///
    /// Intended for consumers (e.g., `arkdown-profile`) that call this after
    /// the build phase completes.
    pub fn reverse_map_snapshot(&self) -> HashMap<String, Vec<HostModuleInfo>> {
        self.reverse_map
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    /// Compute [`ModuleMeta`] for the given `module_id` using this plugin's
    /// `project_root` and `module_name`.
    ///
    /// Call this before constructing the [`Module`] so that metadata is
    /// available as soon as the module enters the build graph.
    pub fn meta_for(&self, module_id: &str) -> ModuleMeta {
        meta_compute::compute_meta(module_id, &self.project_root, &self.module_name)
    }
}

#[async_trait]
impl ArkdownPlugin for ModuleMetaPlugin {
    fn name(&self) -> &str {
        "module-meta"
    }

    /// This plugin does not load content — defer to the default file loader.
    async fn load(&self, _ctx: &LoadContext, _id: &str) -> PluginResult<Option<ModuleSource>> {
        Ok(None)
    }

    /// After a module is parsed, record all of its import edges in the
    /// reverse map so that each imported module knows its consumers.
    async fn module_parsed(
        &self,
        _ctx: &ModuleParsedContext,
        module: &Module,
    ) -> PluginResult<()> {
        let host_dep = module.meta.pkg_name.clone().unwrap_or_default();
        let host_mod = module.meta.module_name.clone().unwrap_or_default();

        let mut map = self.reverse_map.lock().map_err(|e| ArkdownError::PluginError {
            plugin: self.name().to_string(),
            source: Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            )),
        })?;

        let push_host = |map: &mut HashMap<String, Vec<HostModuleInfo>>, id: &str| {
            map.entry(id.to_string()).or_default().push(HostModuleInfo {
                host_dependency_name: host_dep.clone(),
                host_module_name: host_mod.clone(),
            });
        };

        for import in &module.imports {
            push_host(&mut map, &import.resolved.id);
        }
        for import in &module.dynamic_imports {
            push_host(&mut map, &import.resolved.id);
        }

        Ok(())
    }

    async fn build_start(&self, _ctx: &mut BuildContext) -> PluginResult<()> {
        // Clear stale state from a previous incremental build.
        self.reverse_map
            .lock()
            .map_err(|e| ArkdownError::PluginError {
                plugin: self.name().to_string(),
                source: Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                )),
            })?
            .clear();
        Ok(())
    }
}

/// Compute [`ModuleMeta`] for a module with the given `module_id`.
///
/// Called by the loader before constructing a [`Module`] so that metadata is
/// available as soon as the module enters the graph.
pub fn compute_module_meta(
    module_id: &str,
    project_root: &Path,
    module_name: &str,
) -> ModuleMeta {
    meta_compute::compute_meta(module_id, project_root, module_name)
}
