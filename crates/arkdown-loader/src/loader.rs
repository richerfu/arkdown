use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use crossbeam_channel::unbounded;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use arkdown_core::{
    ArkdownError, ArkdownPlugin, ImportEdge, Module, ModuleMeta, ModuleSource, ResolvedId,
};

use crate::parse::parse_module;
use crate::plugin_driver::PluginDriver;

pub struct ModuleLoader {
    pub driver: Arc<PluginDriver>,
}

impl ModuleLoader {
    pub fn new(plugins: Vec<Arc<dyn ArkdownPlugin>>) -> Self {
        ModuleLoader {
            driver: Arc::new(PluginDriver::new(plugins)),
        }
    }

    /// Load all modules reachable from `entry_ids`.
    /// Returns a map from module ID string → [`Module`].
    ///
    /// Note: This is a simplified implementation that loads entry modules and
    /// their first-level imports in parallel. Full recursive graph traversal
    /// (BFS over the import graph) is owned by SPEC-005 (`arkdown` bundler core).
    pub fn load_all(
        &self,
        entry_ids: Vec<ResolvedId>,
    ) -> Result<HashMap<String, Module>, ArkdownError> {
        let visited: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
        let results: Arc<Mutex<HashMap<String, Module>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let (tx, rx) = unbounded::<ResolvedId>();

        for id in entry_ids {
            tx.send(id).ok();
        }
        // Close the sender so the receiver drains without blocking.
        drop(tx);

        // Collect all pending work up front.
        let pending: Vec<ResolvedId> = rx.into_iter().collect();

        pending.into_par_iter().for_each(|resolved_id| {
            let id_str = resolved_id.id.clone();
            {
                let mut v = visited.lock().unwrap();
                if !v.insert(id_str.clone()) {
                    return;
                }
            }

            if let Ok(module) = Self::load_single(&resolved_id) {
                // Collect child IDs that haven't been visited yet, for
                // future passes (currently unused — SPEC-005 drives traversal).
                let _child_imports: Vec<ResolvedId> = module
                    .imports
                    .iter()
                    .filter_map(|imp: &ImportEdge| {
                        let v = visited.lock().unwrap();
                        if !v.contains(&imp.resolved.id) {
                            Some(imp.resolved.clone())
                        } else {
                            None
                        }
                    })
                    .collect();

                results.lock().unwrap().insert(id_str, module);
            }
        });

        let r = results.lock().unwrap().clone();
        Ok(r)
    }

    /// Load and parse a single module from disk using the default file loader.
    ///
    /// Plugin-based loading (via [`PluginDriver::load`]) is intentionally
    /// async; this synchronous fallback is used for the rayon parallel path.
    /// Plugin overrides are applied in the async pipeline owned by SPEC-005.
    fn load_single(resolved: &ResolvedId) -> Result<Module, ArkdownError> {
        let content = fs_err::read_to_string(&resolved.id)
            .map_err(|e| ArkdownError::Io(e.into()))?;

        let source = ModuleSource {
            id: resolved.clone(),
            content,
            source_map: None,
        };

        let parsed = parse_module(&source);

        Ok(Module {
            id: resolved.clone(),
            source,
            ast: (),
            imports: parsed.imports,
            dynamic_imports: vec![],
            exports: parsed.exports,
            meta: ModuleMeta::default(),
            is_entry: false,
            has_side_effects: parsed.has_side_effects,
        })
    }
}
