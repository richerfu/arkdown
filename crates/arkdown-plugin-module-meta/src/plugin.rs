use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use arkdown_core::{HostModuleInfo, ModuleMeta};
use rolldown_common::{ModuleInfo, NormalModule};
use rolldown_plugin::{HookBuildStartArgs, HookNoopReturn, HookUsage, Plugin, PluginContext};

use crate::meta_compute;

pub struct ModuleMetaPlugin {
    project_root: PathBuf,
    module_name: String,
    reverse_map: Arc<Mutex<HashMap<String, Vec<HostModuleInfo>>>>,
}

impl std::fmt::Debug for ModuleMetaPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModuleMetaPlugin")
            .field("module_name", &self.module_name)
            .finish_non_exhaustive()
    }
}

impl ModuleMetaPlugin {
    pub fn new(project_root: PathBuf, module_name: impl Into<String>) -> Self {
        ModuleMetaPlugin {
            project_root,
            module_name: module_name.into(),
            reverse_map: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn reverse_map_snapshot(&self) -> HashMap<String, Vec<HostModuleInfo>> {
        self.reverse_map
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    pub fn meta_for(&self, module_id: &str) -> ModuleMeta {
        meta_compute::compute_meta(module_id, &self.project_root, &self.module_name)
    }
}

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
        let reverse_map = Arc::clone(&self.reverse_map);

        async move {
            let meta = meta_compute::compute_meta(&id_str, &project_root, &module_name);
            let host_dep = meta.pkg_name.unwrap_or_default();
            let host_mod = meta.module_name.unwrap_or_default();

            let mut map = reverse_map
                .lock()
                .map_err(|e| anyhow::anyhow!("module-meta: failed to lock reverse_map: {e}"))?;

            for id in imported_ids.iter().chain(dynamically_imported_ids.iter()) {
                map.entry(id.clone()).or_default().push(HostModuleInfo {
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
        let reverse_map = Arc::clone(&self.reverse_map);
        async move {
            reverse_map
                .lock()
                .map_err(|e| anyhow::anyhow!("module-meta: failed to lock reverse_map: {e}"))?
                .clear();
            Ok(())
        }
    }
}

pub fn compute_module_meta(module_id: &str, project_root: &Path, module_name: &str) -> ModuleMeta {
    meta_compute::compute_meta(module_id, project_root, module_name)
}
