use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use rayon::iter::{IntoParallelIterator, ParallelIterator};

use arkdown_core::{
    ArkdownError, ArkdownPlugin, Module, ModuleIdx, ModuleMeta, ModuleSource, NormalModule,
    ResolvedId,
};

use crate::parse::parse_module;
use crate::plugin_driver::PluginDriver;

pub struct ModuleLoader {
    pub driver: Arc<PluginDriver>,
}

struct LoadState {
    visited: Arc<Mutex<HashSet<String>>>,
    modules: Arc<Mutex<HashMap<String, Module>>>,
}

impl LoadState {
    fn new() -> Self {
        Self {
            visited: Arc::new(Mutex::new(HashSet::new())),
            modules: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn take_batch(&self, queue: &mut Vec<ResolvedId>) -> Vec<ResolvedId> {
        let mut visited = self.visited.lock().unwrap();
        queue
            .drain(..)
            .filter(|id| visited.insert(id.id.clone()))
            .collect()
    }

    fn load_batch(batch: Vec<ResolvedId>) -> Vec<(String, Module)> {
        batch
            .into_par_iter()
            .filter_map(|resolved_id| {
                if resolved_id.external {
                    return None;
                }
                match ModuleLoader::load_single(&resolved_id) {
                    Ok(module) => Some((resolved_id.id.clone(), module)),
                    Err(error) => {
                        tracing::warn!(
                            module = %resolved_id.id,
                            error = %error,
                            "failed to load module"
                        );
                        None
                    }
                }
            })
            .collect()
    }

    fn commit_batch(&self, new_modules: Vec<(String, Module)>) -> Vec<ResolvedId> {
        let visited = self.visited.lock().unwrap();
        let mut modules = self.modules.lock().unwrap();
        let mut next_wave = Vec::new();

        for (id, module) in new_modules {
            for import in module.imports() {
                if !visited.contains(&import.resolved.id) {
                    next_wave.push(import.resolved.clone());
                }
            }
            if let Module::Normal(normal) = &module {
                for dynamic_import in &normal.dynamic_imports {
                    if !visited.contains(&dynamic_import.resolved.id) {
                        next_wave.push(dynamic_import.resolved.clone());
                    }
                }
            }
            modules.insert(id, module);
        }

        next_wave
    }

    fn into_modules(self) -> HashMap<String, Module> {
        self.modules.lock().unwrap().clone()
    }
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
        let state = LoadState::new();
        let mut queue: Vec<ResolvedId> = entry_ids;

        while !queue.is_empty() {
            let batch = state.take_batch(&mut queue);
            if batch.is_empty() {
                break;
            }

            queue = state.commit_batch(LoadState::load_batch(batch));
        }

        Ok(state.into_modules())
    }

    /// Load and parse a single module from disk using the default file loader.
    ///
    /// Plugin-based loading (via [`PluginDriver::load`]) is intentionally
    /// async; this synchronous fallback is used for the rayon parallel path.
    /// Plugin overrides are applied in the async pipeline owned by SPEC-005.
    fn load_single(resolved: &ResolvedId) -> Result<Module, ArkdownError> {
        let content =
            fs_err::read_to_string(&resolved.id).map_err(|e| ArkdownError::Io(e.into()))?;

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
