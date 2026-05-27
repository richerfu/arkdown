use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use index_vec::IndexVec;
use rustc_hash::FxHashMap;

use arkdown_core::{
    ArkdownConfig, ArkdownError, ArkdownPlugin, BuildMode, ChunkGraph, Module, ModuleGraph,
    ModuleIdx, ModuleType, RenderChunkContext, ResolvedId,
};
use arkdown_loader::ModuleLoader;
use arkdown_profile::LoaderJson;

use crate::output;
use crate::stages::{chunk_split, codegen, link, tree_shake};

/// The top-level bundler for one OHOS module build.
///
/// Orchestrates the full pipeline:
/// 1. Load all modules via [`ModuleLoader`].
/// 2. Build a [`ModuleGraph`].
/// 3. Link cross-module symbol references.
/// 4. Tree-shake unreachable modules (release only).
/// 5. Split modules into output [`ChunkGraph`] entries.
/// 6. Generate CJS code for each chunk.
/// 7. Run `render_chunk` plugin hooks.
/// 8. Write `.js` files to `loader_json.ace_module_build`.
pub struct Bundler {
    pub config: Arc<ArkdownConfig>,
    pub plugins: Vec<Arc<dyn ArkdownPlugin>>,
}

/// The output produced by a successful [`Bundler::bundle`] call.
pub struct BundleOutput {
    pub chunk_graph: ChunkGraph,
    pub output_dir: PathBuf,
    /// `(chunk_name, absolute_path)` for each written `.js` file.
    pub chunk_files: Vec<(String, PathBuf)>,
}

impl Bundler {
    /// Create a new `Bundler` with the given configuration and plugin list.
    pub fn new(config: ArkdownConfig, plugins: Vec<Arc<dyn ArkdownPlugin>>) -> Self {
        Bundler {
            config: Arc::new(config),
            plugins,
        }
    }

    /// Run the full bundle pipeline and return the chunk outputs.
    pub async fn bundle(&self, loader_json: &LoaderJson) -> Result<BundleOutput, ArkdownError> {
        // ── 1. Resolve entry points ──────────────────────────────────────────
        let module_root = PathBuf::from(&loader_json.module_root_path);
        let entries: Vec<ResolvedId> = loader_json
            .compile_entry
            .iter()
            .map(|e| {
                let path = if e.starts_with('/') || e.starts_with('@') {
                    // Absolute path or OHM URL — use as-is.
                    e.clone()
                } else {
                    module_root
                        .join(e.trim_start_matches("./"))
                        .to_string_lossy()
                        .to_string()
                };
                ResolvedId {
                    id: path,
                    external: false,
                    module_type: ModuleType::Ets,
                }
            })
            .collect();

        // ── 2. Load modules ──────────────────────────────────────────────────
        let loader = ModuleLoader::new(self.plugins.clone());
        let module_map: HashMap<String, Module> = loader.load_all(entries.clone())?;

        // ── 3. Build ModuleGraph ─────────────────────────────────────────────
        let mut graph = build_module_graph(module_map, &entries);

        // ── 4. Link ──────────────────────────────────────────────────────────
        link::link(&mut graph);

        // ── 5. Tree-shake (release builds only) ─────────────────────────────
        if matches!(self.config.build_mode, BuildMode::Release) {
            tree_shake::tree_shake(&mut graph);
        }

        // ── 6. Chunk split ───────────────────────────────────────────────────
        let mut chunk_graph = chunk_split::split_chunks(&graph, loader_json.support_chunks);

        // ── 7. Codegen ───────────────────────────────────────────────────────
        codegen::generate_code(&graph, &mut chunk_graph)?;

        // ── 8. render_chunk plugin hooks ─────────────────────────────────────
        for chunk in chunk_graph.chunks.iter_mut() {
            let mut code = chunk.code.clone();
            let ctx = RenderChunkContext {
                chunk_name: chunk.name.clone(),
            };
            for plugin in &self.plugins {
                if let Some(rendered) = plugin.render_chunk(&ctx, code.clone(), chunk).await? {
                    code = rendered.code;
                    if let Some(map) = rendered.map {
                        chunk.source_map = Some(map);
                    }
                }
            }
            chunk.code = code;
        }

        // ── 9. Write output ──────────────────────────────────────────────────
        let output_dir = PathBuf::from(&loader_json.ace_module_build);
        let chunk_files = output::write_chunks(&chunk_graph, &output_dir)?;

        Ok(BundleOutput {
            chunk_graph,
            output_dir,
            chunk_files,
        })
    }
}

/// Construct a [`ModuleGraph`] from a flat module map and the ordered entry list.
fn build_module_graph(
    modules: HashMap<String, Module>,
    entries: &[ResolvedId],
) -> ModuleGraph {
    let mut module_vec: IndexVec<ModuleIdx, Module> = IndexVec::new();
    let mut id_to_idx: FxHashMap<String, ModuleIdx> = FxHashMap::default();

    let entry_ids: std::collections::HashSet<String> =
        entries.iter().map(|e| e.id.clone()).collect();

    for (id, mut module) in modules {
        if entry_ids.contains(&id) {
            module.is_entry = true;
        }
        let idx = module_vec.push(module);
        id_to_idx.insert(id, idx);
    }

    // Preserve entry order from loader_json (important for chunk naming).
    let entry_indices: Vec<ModuleIdx> = entries
        .iter()
        .filter_map(|e| id_to_idx.get(&e.id).copied())
        .collect();

    ModuleGraph {
        modules: module_vec,
        id_to_idx,
        entries: entry_indices,
    }
}
