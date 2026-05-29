//! Core bundler — delegates the full pipeline to rolldown 1.0.3.
//!
//! ## Pipeline (new)
//!
//! 1. Convert [`LoaderJson`] entry points → rolldown [`InputItem`] list.
//! 2. Pass [`SharedPluginable`] list to rolldown directly (no adapter).
//! 3. Construct a rolldown [`Bundler`] with ESM output format.
//! 4. Call [`Bundler::generate()`] — in-memory bundle, no disk writes.
//!    During this step rolldown invokes the standard plugin hook sequence:
//!    `build_start` → `resolve_id` → `load` → `transform` → … → `render_chunk`.
//! 5. Convert rolldown's [`BundleOutput`] → our [`ChunkGraph`].
//! 6. Write loader_out `.js`, `sourceMaps.map`, and ABC handoff paths via
//!    [`output::write_output`].
//!
//! The old custom stages (`link`, `tree_shake`, `chunk_split`, `codegen`) in
//! `crates/arkdown/src/stages/` are superseded by this rolldown-backed pipeline
//! and are no longer called from here.  They are retained in the codebase as
//! reference implementations.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use index_vec::IndexVec;
use rustc_hash::FxHashMap;

use arkdown_core::{ArkChunk, ArkdownConfig, ArkdownError, BuildMode, ChunkGraph};
use arkdown_profile::LoaderJson;
use rolldown::{BundlerOptions, InputItem, OutputFormat};
use rolldown_common::{
    ChunkingContext, ManualCodeSplittingOptions, MatchGroup, MatchGroupName, MatchGroupTest,
    ModuleType as RdModuleType, Output, SourceMapType, TreeshakeOptions,
};
use rolldown_plugin::__inner::SharedPluginable;
use rolldown_utils::js_regex::HybridRegex;

use crate::output;

/// The top-level bundler for one OHOS module build.
///
/// Orchestrates the full pipeline via rolldown with OHOS-specific plugins.
pub struct Bundler {
    pub config: Arc<ArkdownConfig>,
    pub plugins: Vec<SharedPluginable>,
}

/// The output produced by a successful [`Bundler::bundle`] call.
pub struct BundleOutput {
    pub chunk_graph: ChunkGraph,
    pub output_dir: PathBuf,
    /// `(chunk_name, absolute_path)` for each written `.js` file.
    pub chunk_files: Vec<(String, PathBuf)>,
    /// Native ABC backend output path. Packagers must use this path as the
    /// source for the final archive entry `ets/modules.abc`.
    pub modules_abc_path: PathBuf,
    /// `sourceMaps.map` if a real source map was produced from bundler inputs.
    pub source_maps_path: Option<PathBuf>,
}

impl Bundler {
    /// Create a new `Bundler` with the given configuration and plugin list.
    pub fn new(config: ArkdownConfig, plugins: Vec<SharedPluginable>) -> Self {
        Bundler {
            config: Arc::new(config),
            plugins,
        }
    }

    /// Run the full bundle pipeline and return the chunk outputs.
    ///
    /// The output remains ESM so the Rust ABC backend compiles the same
    /// CompileArkTS-stage module shape that hvigor sends to es2abc.
    pub async fn bundle(&self, loader_json: &LoaderJson) -> Result<BundleOutput, ArkdownError> {
        // ── 1. Build rolldown input entries ──────────────────────────────────
        // module_root_path may be relative and may contain `./` segments
        // (e.g. `tests/fixtures/minimal-hap/./entry/src/main`).  Canonicalize
        // produces an absolute, clean path that rolldown's resolver can use.
        let module_root =
            std::fs::canonicalize(&loader_json.module_root_path).unwrap_or_else(|_| {
                // Fallback: manual `./` removal + join with process cwd
                let normalized = normalize_path(&loader_json.module_root_path);
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(normalized)
            });

        let input: Vec<InputItem> = loader_json
            .compile_entry
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let import_path = if entry.starts_with('/') || entry.starts_with('@') {
                    // Already absolute or an OHM URL — pass through.
                    entry.clone()
                } else {
                    // Build the candidate path, then canonicalize to get an
                    // absolute path without any `.` or `..` components.
                    let candidate = module_root.join(entry.trim_start_matches("./"));
                    std::fs::canonicalize(&candidate)
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|_| candidate.to_string_lossy().to_string())
                };
                // Derive chunk name from the compile_entry file stem so rolldown
                // writes "EntryAbility.js" instead of "entry_0.js".  This keeps
                // the ABC record_name_map lookup (stem → OHM class path) working.
                let chunk_name = std::path::Path::new(entry.as_str())
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("entry_{i}"));
                InputItem {
                    name: Some(chunk_name),
                    import: import_path,
                }
            })
            .collect();

        // ── 2. Pass plugins directly to rolldown ─────────────────────────────
        let rd_plugins: Vec<SharedPluginable> = self.plugins.clone();

        // ── 3. Build rolldown BundlerOptions ─────────────────────────────────
        //
        // Output format is ESM.  hvigor's CompileArkTS phase feeds es2abc the
        // transformed module source directly; generating CJS here injects
        // rolldown helpers (`__toESM`, `__copyProps`, etc.) that do not exist
        // in hvigor's bytecode.
        //
        // module_types maps ".ets" → Ts so rolldown uses the TypeScript parser
        // for ArkTS source files.  Rolldown's transform hook runs AFTER the
        // parse step, so the module must parse successfully before EtsTransform
        // can process ArkTS-specific constructs.  Standard-TypeScript .ets files
        // (the common case) parse cleanly under the Ts parser.
        let mut module_types = rustc_hash::FxHashMap::default();
        module_types.insert("ets".to_string(), RdModuleType::Ts);

        // cwd: canonicalize the project root so rolldown's resolver operates on
        // an absolute path.  The entry paths built above are already absolute.
        let cwd = std::fs::canonicalize(&loader_json.project_root_path).unwrap_or_else(|_| {
            let raw = PathBuf::from(&loader_json.project_root_path);
            if raw.is_absolute() {
                raw
            } else {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(raw)
            }
        });

        // Disable tree-shaking for debug builds.  ArkRuntime entry pages
        // (@Entry @Component struct) have no exports — only a side-effect
        // `registerNamedRoute(...)` call at module level.  With tree-shaking
        // enabled rolldown eliminates the entire module body when nothing
        // imports from it, producing an empty chunk (0 bytes).
        // hvigor also disables tree-shaking in debug mode.
        let treeshake = if matches!(self.config.build_mode, BuildMode::Debug) {
            TreeshakeOptions::Boolean(false)
        } else {
            TreeshakeOptions::Boolean(true)
        };

        // manualChunks for supportChunks=true: mirrors hvigor's arkts-pack.js logic.
        // oh_modules/node_modules → "commons"; non-entry local modules → "vendors".
        let manual_code_splitting = if loader_json.support_chunks {
            Some(build_manual_code_splitting_options()?)
        } else {
            None
        };

        let options = BundlerOptions {
            input: Some(input),
            cwd: Some(cwd),
            format: Some(OutputFormat::Esm),
            module_types: Some(module_types),
            sourcemap: Some(SourceMapType::Hidden),
            treeshake,
            manual_code_splitting,
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

        // ── 6. Write loader_out/default/ets artifacts to disk ────────────────
        let output_dir = PathBuf::from(&loader_json.ace_module_build);
        let written_output = output::write_output(&chunk_graph, &output_dir)?;
        let modules_abc_path = output::modules_abc_path(&output_dir);

        Ok(BundleOutput {
            chunk_graph,
            output_dir,
            chunk_files: written_output.chunk_files,
            modules_abc_path,
            source_maps_path: written_output.source_maps_path,
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
fn build_chunk_graph(rd_output: rolldown::BundleOutput) -> Result<ChunkGraph, ArkdownError> {
    use arkdown_core::ChunkIdx;

    let mut chunks: IndexVec<ChunkIdx, ArkChunk> = IndexVec::new();

    for asset in rd_output.assets {
        if let Output::Chunk(chunk) = asset {
            let ark_chunk = ArkChunk {
                name: chunk.name.to_string(),
                code: chunk.code.clone(),
                source_map: chunk.map.as_ref().map(|map| map.to_json_string()),
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

/// Build `ManualCodeSplittingOptions` that mirrors hvigor's `manualChunks` callback.
///
/// Group 1 (`commons`): modules whose path contains `oh_modules` or `node_modules`.
/// Group 2 (`vendors`): all remaining non-entry modules.
///
/// This matches the hvigor `arkts-pack.js` logic:
/// ```js
/// function manualChunks(id, { getModuleInfo }) {
///   if (id.includes(NODE_MODULES) || id.includes(OH_NODE_MODULES)) return "commons";
///   const info = getModuleInfo(id);
///   return info && !info.isEntry ? "vendors" : undefined;
/// }
/// ```
fn build_manual_code_splitting_options() -> Result<ManualCodeSplittingOptions, ArkdownError> {
    let commons_regex = HybridRegex::new("oh_modules|node_modules")
        .map_err(|e| ArkdownError::Bundler(format!("invalid manualChunks regex: {e}")))?;

    let commons_group = MatchGroup {
        name: MatchGroupName::Static("commons".to_string()),
        test: Some(MatchGroupTest::Regex(commons_regex)),
        ..Default::default()
    };

    // Dynamic name function: returns "vendors" for non-entry modules, None for entries.
    // The function captures no state; it queries `is_entry` via ChunkingContext.
    let vendors_name_fn: Arc<
        dyn Fn(
                &str,
                &ChunkingContext,
            ) -> Pin<
                Box<dyn Future<Output = anyhow::Result<Option<String>>> + Send + 'static>,
            > + Send
            + Sync,
    > = Arc::new(|module_id: &str, ctx: &ChunkingContext| {
        let result = ctx.get_module_info(module_id).and_then(|info| {
            if !info.is_entry {
                Some("vendors".to_string())
            } else {
                None
            }
        });
        Box::pin(async move { Ok(result) })
    });

    let vendors_group = MatchGroup {
        name: MatchGroupName::Dynamic(vendors_name_fn),
        test: None,
        ..Default::default()
    };

    Ok(ManualCodeSplittingOptions {
        groups: Some(vec![commons_group, vendors_group]),
        ..Default::default()
    })
}

/// Normalize a file-system path by resolving `./` (current-dir) components.
///
/// `build-profile.json5` often has `srcPath: "./entry"` which, when joined,
/// produces paths like `project/./entry/src/main/…` that confuse rolldown's
/// file resolver.  This removes all `CurDir` components without touching the
/// rest of the path (no symlink resolution, no allocation if already clean).
fn normalize_path(path: &str) -> String {
    use std::path::Component;
    let mut out = PathBuf::new();
    for comp in PathBuf::from(path).components() {
        match comp {
            Component::CurDir => {} // drop `.` segments
            other => out.push(other),
        }
    }
    out.to_string_lossy().to_string()
}
