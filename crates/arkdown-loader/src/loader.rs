use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use rayon::iter::{IntoParallelIterator, ParallelIterator};

use arkdown_core::{
    ArkdownError, ArkdownPlugin, Module, ModuleMeta, ModuleSource, NormalModule,
    ModuleIdx, ResolvedId,
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

    /// Load all modules transitively reachable from `entry_ids`.
    ///
    /// Uses a BFS loop over import edges so that deep dependency chains
    /// (e.g. EntryAbility → Helper → Types) are fully included in the result.
    /// Each BFS wave is loaded in parallel via rayon.
    ///
    /// # Cycle safety
    /// Modules are added to the `visited` set before loading begins, so
    /// circular imports (A → B → A) terminate after one round-trip.
    ///
    /// # External modules
    /// Any module whose `ResolvedId.external` flag is set (OHM URLs such as
    /// `@bundle:`, `@shared:`, or SDK specifiers like `@ohos/hilog`) is
    /// skipped — it is resolved at runtime by ArkRuntime, not bundled.
    ///
    /// # Error handling
    /// A failure to load an individual module emits a warning on stderr and
    /// continues the build.  This is intentional: some import specifiers
    /// point to ambient declarations that exist only in `.d.ts` files and
    /// are never on disk.
    pub fn load_all(
        &self,
        entry_ids: Vec<ResolvedId>,
    ) -> Result<HashMap<String, Module>, ArkdownError> {
        let visited: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
        let results: Arc<Mutex<HashMap<String, Module>>> = Arc::new(Mutex::new(HashMap::new()));

        // BFS queue — start with entries.
        let mut queue: Vec<ResolvedId> = entry_ids;

        while !queue.is_empty() {
            // Mark all pending IDs as visited BEFORE loading to prevent
            // double-enqueue when two modules in the same wave import the
            // same dependency.
            let batch: Vec<ResolvedId> = {
                let mut v = visited.lock().unwrap();
                queue
                    .drain(..)
                    .filter(|id| v.insert(id.id.clone()))
                    .collect()
            };

            if batch.is_empty() {
                break;
            }

            // Load this wave in parallel.
            let new_modules: Vec<(String, Module)> = batch
                .into_par_iter()
                .filter_map(|resolved_id| {
                    // External modules (OHM URLs, SDK APIs) are never read from disk.
                    if resolved_id.external {
                        return None;
                    }
                    match Self::load_single(&resolved_id) {
                        Ok(module) => Some((resolved_id.id.clone(), module)),
                        Err(e) => {
                            eprintln!("Warning: failed to load {}: {}", resolved_id.id, e);
                            None
                        }
                    }
                })
                .collect();

            // Enqueue imports discovered in this wave for the next BFS round.
            let mut next_wave: Vec<ResolvedId> = Vec::new();
            {
                let v = visited.lock().unwrap();
                let mut r = results.lock().unwrap();
                for (id, module) in new_modules {
                    // Enqueue static imports not yet visited.
                    for imp in module.imports() {
                        if !v.contains(&imp.resolved.id) {
                            next_wave.push(imp.resolved.clone());
                        }
                    }
                    // Enqueue dynamic imports not yet visited.
                    if let Module::Normal(nm) = &module {
                        for dyn_imp in &nm.dynamic_imports {
                            if !v.contains(&dyn_imp.resolved.id) {
                                next_wave.push(dyn_imp.resolved.clone());
                            }
                        }
                    }
                    r.insert(id, module);
                }
            }
            queue = next_wave;
        }

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

        Ok(Module::Normal(Box::new(NormalModule {
            idx: ModuleIdx(0), // placeholder; updated by bundler after insertion
            exec_order: 0,
            id: resolved.clone(),
            source,
            ast: (),
            imports: parsed.imports,
            dynamic_imports: vec![],
            exports: parsed.exports,
            meta: ModuleMeta::default(),
            is_entry: false,
            has_side_effects: parsed.has_side_effects,
        })))
    }
}
