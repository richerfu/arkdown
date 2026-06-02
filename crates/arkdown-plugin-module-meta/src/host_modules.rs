use rustc_hash::FxHashMap;

use arkdown_core::{HostModuleInfo, Module};

/// Build a reverse-import map: `module_id` → list of [`HostModuleInfo`].
///
/// For each module in `modules`, every import edge is recorded so that we
/// know which modules (and which packages/OHOS-modules they belong to) depend
/// on a given module id.
pub fn build_reverse_map(modules: &[&Module]) -> FxHashMap<String, Vec<HostModuleInfo>> {
    let mut map: FxHashMap<String, Vec<HostModuleInfo>> = FxHashMap::default();

    for module in modules {
        // Only normal (non-external) modules contribute to the reverse map.
        let nm = match module.as_normal() {
            Some(m) => m,
            None => continue,
        };

        let host_dep = nm.meta.pkg_name.clone().unwrap_or_default();
        let host_mod = nm.meta.module_name.clone().unwrap_or_default();

        for import in &nm.imports {
            map.entry(import.resolved.id.clone())
                .or_default()
                .push(HostModuleInfo {
                    host_dependency_name: host_dep.clone(),
                    host_module_name: host_mod.clone(),
                });
        }

        // Dynamic imports are also reverse-mapped so hot-reload can track
        // every consumer, not just static ones.
        for import in &nm.dynamic_imports {
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
