//! Core bundler — delegates the full pipeline to rolldown 1.0.3.
//!
//! ## Pipeline (new)
//!
//! 1. Convert [`LoaderJson`] entry points → rolldown [`InputItem`] list.
//! 2. Wrap each `Arc<dyn ArkdownPlugin>` in [`ArkdownPluginAdapter`].
//! 3. Construct a rolldown [`Bundler`] with CJS output format.
//! 4. Call [`Bundler::generate()`] — in-memory bundle, no disk writes.
//!    During this step rolldown invokes the standard plugin hook sequence:
//!    `build_start` → `resolve_id` → `load` → `transform` → … → `render_chunk`.
//!    The `render_chunk` hook is where [`arkdown-plugin-chunk-wrapper`] converts
//!    CJS output to IIFE and installs the `globalThis` registry entry.
//! 5. Convert rolldown's [`BundleOutput`] → our [`ChunkGraph`].
//! 6. Write `.js` (and `.js.map`) files via [`output::write_chunks`].
//!
//! The old custom stages (`link`, `tree_shake`, `chunk_split`, `codegen`) in
//! `crates/arkdown/src/stages/` are superseded by this rolldown-backed pipeline
//! and are no longer called from here.  They are retained in the codebase as
//! reference implementations.

use std::path::PathBuf;
use std::sync::Arc;

use index_vec::IndexVec;
use rustc_hash::FxHashMap;

use arkdown_core::{ArkChunk, ArkdownConfig, ArkdownError, ArkdownPlugin, ChunkGraph};
use arkdown_profile::LoaderJson;
use rolldown::{BundlerOptions, InputItem, OutputFormat};
use rolldown_common::{ModuleType as RdModuleType, Output};
use rolldown_plugin::__inner::SharedPluginable;

use crate::output;
use crate::plugin_adapter::ArkdownPluginAdapter;

/// The top-level bundler for one OHOS module build.
///
/// Orchestrates the full pipeline via rolldown with OHOS-specific plugins.
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
    ///
    /// The IIFE-wrapping and `globalThis` registry installation are performed
    /// by the `arkdown-plugin-chunk-wrapper` plugin via rolldown's `render_chunk`
    /// hook — no post-processing step is required here.
    pub async fn bundle(&self, loader_json: &LoaderJson) -> Result<BundleOutput, ArkdownError> {
        // ── 1. Build rolldown input entries ──────────────────────────────────
        let module_root = PathBuf::from(&loader_json.module_root_path);
        let input: Vec<InputItem> = loader_json
            .compile_entry
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let import_path = if entry.starts_with('/') || entry.starts_with('@') {
                    // Absolute path or OHM URL — pass through as-is.
                    entry.clone()
                } else {
                    module_root
                        .join(entry.trim_start_matches("./"))
                        .to_string_lossy()
                        .to_string()
                };
                InputItem {
                    // Stable names drive the chunk file names rolldown produces.
                    name: Some(format!("entry_{i}")),
                    import: import_path,
                }
            })
            .collect();

        // ── 2. Adapt OHOS plugins to rolldown's Plugin trait ─────────────────
        let rd_plugins: Vec<SharedPluginable> = self
            .plugins
            .iter()
            .map(|p| {
                Arc::new(ArkdownPluginAdapter(Arc::clone(p))) as SharedPluginable
            })
            .collect();

        // ── 3. Build rolldown BundlerOptions ─────────────────────────────────
        //
        // Output format is CJS: rolldown generates CommonJS `require()` calls
        // for cross-chunk references.  The chunk-wrapper plugin's `render_chunk`
        // hook then converts the CJS bundle to an OHOS-style IIFE with
        // `globalThis[key]` lookups.
        //
        // module_types maps ".ets" → Ts so rolldown uses the TypeScript parser
        // for ArkTS source files.  Rolldown's transform hook runs AFTER the
        // parse step, so the module must parse successfully before EtsTransform
        // can process ArkTS-specific constructs.  Standard-TypeScript .ets files
        // (the common case) parse cleanly under the Ts parser.
        let mut module_types = rustc_hash::FxHashMap::default();
        module_types.insert("ets".to_string(), RdModuleType::Ts);

        let options = BundlerOptions {
            input: Some(input),
            // Use the project root as cwd so that relative entry paths resolve.
            cwd: Some(PathBuf::from(&loader_json.project_root_path)),
            format: Some(OutputFormat::Cjs),
            module_types: Some(module_types),
            ..Default::default()
        };

        // ── 4. Create rolldown Bundler and generate in-memory output ──────────
        let mut rd_bundler = rolldown::Bundler::with_plugins(options, rd_plugins)
            .map_err(|e| ArkdownError::Bundler(format!("{e:?}")))?;

        let rd_output = rd_bundler
            .generate()
            .await
            .map_err(|e| ArkdownError::Bundler(format!("{e:?}")))?;

        // ── 5. Convert rolldown output → ChunkGraph ───────────────────────────
        let chunk_graph = build_chunk_graph(rd_output)?;

        // ── 6. Write .js (and .js.map) files to disk ─────────────────────────
        let output_dir = PathBuf::from(&loader_json.ace_module_build);
        let chunk_files = output::write_chunks(&chunk_graph, &output_dir)?;

        Ok(BundleOutput {
            chunk_graph,
            output_dir,
            chunk_files,
        })
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Convert rolldown's [`rolldown::BundleOutput`] into our [`ChunkGraph`].
///
/// Asset outputs (CSS, images, etc.) are ignored — only `Output::Chunk`
/// entries are included in the chunk graph.
fn build_chunk_graph(
    rd_output: rolldown::BundleOutput,
) -> Result<ChunkGraph, ArkdownError> {
    use arkdown_core::ChunkIdx;

    let mut chunks: IndexVec<ChunkIdx, ArkChunk> = IndexVec::new();

    for asset in rd_output.assets {
        if let Output::Chunk(chunk) = asset {
            let ark_chunk = ArkChunk {
                name: chunk.name.to_string(),
                code: chunk.code.clone(),
                // Source map conversion (oxc_sourcemap::SourceMap → String JSON)
                // is deferred to a follow-up.  Set to None for now.
                source_map: None,
                is_entry: chunk.is_entry,
                is_dynamic_entry: chunk.is_dynamic_entry,
                facade_module_id: chunk
                    .facade_module_id
                    .as_ref()
                    .map(|id| id.as_str().to_owned()),
                exports: chunk.exports.iter().map(|e| e.to_string()).collect(),
                imports: chunk.imports.iter().map(|i| i.to_string()).collect(),
                dynamic_imports: chunk
                    .dynamic_imports
                    .iter()
                    .map(|d| d.to_string())
                    .collect(),
                ..ArkChunk::default()
            };
            chunks.push(ark_chunk);
        }
    }

    Ok(ChunkGraph {
        chunks,
        // module_to_chunk and entry_module_to_entry_chunk are not populated
        // from rolldown output because rolldown uses its own module index space.
        // Callers that need module→chunk mapping (e.g. arkdown-abc) must use
        // ArkChunk.facade_module_id / ArkChunk.module_ids instead.
        module_to_chunk: IndexVec::new(),
        entry_module_to_entry_chunk: FxHashMap::default(),
    })
}
