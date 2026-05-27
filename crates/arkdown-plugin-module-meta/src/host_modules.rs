use std::collections::HashMap;

use arkdown_core::{HostModuleInfo, Module};

/// Build a reverse-import map: `module_id` → list of [`HostModuleInfo`].
///
/// For each module in `modules`, every import edge is recorded so that we
/// know which modules (and which packages/OHOS-modules they belong to) depend
/// on a given module id.
pub fn build_reverse_map(modules: &[&Module]) -> HashMap<String, Vec<HostModuleInfo>> {
    let mut map: HashMap<String, Vec<HostModuleInfo>> = HashMap::new();

    for module in modules {
        let host_dep = module
            .meta
            .pkg_name
            .clone()
            .unwrap_or_default();
        let host_mod = module
            .meta
            .module_name
            .clone()
            .unwrap_or_default();

        for import in &module.imports {
            map.entry(import.resolved.id.clone())
                .or_default()
                .push(HostModuleInfo {
                    host_dependency_name: host_dep.clone(),
                    host_module_name: host_mod.clone(),
                });
        }

        // Dynamic imports are also reverse-mapped so hot-reload can track
        // every consumer, not just static ones.
        for import in &module.dynamic_imports {
            map.entry(import.resolved.id.clone())
                .or_default()
                .push(HostModuleInfo {
                    host_dependency_name: host_dep.clone(),
                    host_module_name: host_mod.clone(),
                });
        }
    }

    map
}
