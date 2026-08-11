//! Core bundler for hvigor-style ESMODULE builds.
//!
//! hvigor disables Rollup generateBundle for Stage-model ESMODULE builds and
//! hands transformed module sources to CompileArkTS/es2abc.  This module mirrors
//! that shape: Rolldown owns resolve/load/transform/parse/semantic scan, while
//! arkdown consumes the collected module graph and writes the CompileArkTS module
//! source cache.  It does not generate or write bundled JavaScript chunks on the
//! production path.

use rustc_hash::{FxHashMap, FxHashSet};
use std::borrow::Cow;
use std::collections::BTreeSet;
use std::future::Future;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::pin::Pin;
#[cfg(feature = "memory-diagnostics")]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use arcstr::ArcStr;
use arkdown_cache::CacheKeyMaterial;
use indexmap::IndexMap;
use oxc_allocator::Allocator;
use oxc_ast::ast::{Expression, ImportExpression, StringLiteral};
use oxc_ast_visit::{walk, Visit};
use oxc_index::IndexVec;
use oxc_parser::Parser;
use rayon::prelude::*;

use arkdown_ets_transform::{CompileArktsSourceType, RolldownEtsTransformPlan, SourceClassifier};

use arkdown_core::{
    ArkdownConfig, ArkdownError, ChunkGraph, ChunkSourceFile, HvigorModuleSourceFile,
    HvigorNpmEntry,
};
use arkdown_path::{
    lexical_normalize, specifier::SpecifierKind, strip_relative_prefix, AbsPath,
    ArtifactPathSerializer, ModuleSpecifier, NormalizedOhmUrl,
};
use arkdown_resolver::OhPackage;
use rolldown::{BundlerOptions, InputItem, ModuleSourceScanOutput, OutputFormat};
use rolldown_common::{
    BundlerTransformOptions, ChunkingContext, CodeSplittingMode, ManualCodeSplittingOptions,
    MatchGroup, MatchGroupName, MatchGroupTest, ModuleType as RdModuleType, SanitizeFilename,
    TreeshakeOptions, TypeScriptOptions,
};
use rolldown_plugin::{
    __inner::SharedPluginable, HookTransformArgs, HookTransformReturn, HookUsage, Plugin,
    SharedTransformPluginContext,
};
use rolldown_utils::dashmap::FxDashMap;

use crate::chunk_module_request::{HvigorChunkImportMap, HvigorChunkModuleRequestProcessor};
use crate::compile_entry::CompileEntryNamer;
use crate::diagnostics::RolldownBuildDiagnostics;
use crate::manual_chunks::{ChunkName, ManualChunkPolicy};
use crate::output;
use crate::plan::{
    BundleMockConfig, BundlePlan, BundleRuntimePackageContext, BundleRuntimePackageInfo,
    SourceMapPolicy,
};
use crate::plugins::ets_transform::EtsTransformModuleMetadata;
use crate::plugins::module_meta::ModuleMetaPlugin;

/// The top-level bundler for one OHOS module build.
///
/// Orchestrates the full pipeline via rolldown with OHOS-specific plugins.
pub struct Bundler {
    pub config: Arc<ArkdownConfig>,
    pub plugins: Vec<SharedPluginable>,
    ets_transform_plan: Option<Arc<RolldownEtsTransformPlan>>,
}

/// The output produced by a successful [`Bundler::bundle`] call.
pub struct BundleOutput {
    pub chunk_graph: ChunkGraph,
    pub output_dir: PathBuf,
    /// `(chunk_name, absolute_path)` for each written `.js` file.
    pub chunk_files: Vec<(String, PathBuf)>,
    /// `(chunk_name, source path relative to src/main)` for compile-entry chunks.
    ///
    /// The bytecode stage uses this to derive hvigor-compatible ABC record names
    /// for `filesInfo.txt`.
    pub chunk_source_files: Vec<ChunkSourceFile>,
    /// Transformed module sources compiled by SDK es2abc in hvigor ESMODULE mode.
    pub module_source_files: Vec<HvigorModuleSourceFile>,
    /// Entry module IDs passed to Rolldown scan.
    pub entry_module_ids: Vec<String>,
    /// Module graph import edges collected from Rolldown/OXC module records.
    pub module_imports: FxHashMap<String, Vec<String>>,
    /// External module requests collected from Rolldown module import records.
    ///
    /// The bytecode stage lowers these to hvigor-compatible `npmEntries.txt`
    /// entries before invoking SDK es2abc.
    pub external_module_requests: Vec<String>,
    /// Importee inventories written by hvigor's ArkTS `processImportees`
    /// build step and consumed by ByteCode HAR artifact processing.
    pub har_importees: HvigorHarImportees,
    /// System/native imports reachable from hvigor preload entry roots.
    /// Written by the CLI as `intermediates/preload/<target>/preload.json`.
    pub preload_system_so_tasks: Vec<PreloadSystemSoTask>,
    /// Package and local dependency mappings written to `npmEntries.txt`.
    pub npm_entries: Vec<HvigorNpmEntry>,
    /// Native ABC backend output path. Packagers must use this path as the
    /// source for the final archive entry `ets/modules.abc`.
    pub modules_abc_path: PathBuf,
    /// `sourceMaps.map` if a real source map was produced from bundler inputs.
    pub source_maps_path: Option<PathBuf>,
    /// Content fingerprints for source-map entries whose original source was
    /// already inventoried by the build session. The CLI cache handoff uses
    /// these to retain latest ets-loader's per-module incremental map order
    /// without re-reading source files after Rolldown finishes.
    pub source_map_input_fingerprints: Vec<SourceMapInputFingerprint>,
}

/// One source-map entry's pre-transform source fingerprint.
///
/// This is internal incremental metadata, not an artifact serialized into the
/// HAP/HSP/HAR. It mirrors the source identity Rollup uses to decide which
/// `SourceMapGenerator.sourceMaps` entries belong to the current pass.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SourceMapInputFingerprint {
    pub key: String,
    pub content_hash: String,
}

/// Deterministic equivalent of the three importee arrays produced by latest
/// `@ohos/hvigor-arkts-base/process-importee`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HvigorHarImportees {
    /// Package subpath requests made by modules owned by the current HAR.
    pub non_entry: Vec<String>,
    /// All bare package requests observed across the compiled module graph.
    pub compiled: Vec<String>,
    /// All bare package requests made by modules owned by the current HAR.
    pub direct: Vec<String>,
}

/// One `systemPreloadHintStartupTasks` item generated by the SDK Ark compiler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreloadSystemSoTask {
    pub name: String,
    pub src_entry: String,
    pub ohm_url: String,
}

impl Bundler {
    /// Create a new `Bundler` with the given configuration and plugin list.
    pub fn new(config: ArkdownConfig, plugins: Vec<SharedPluginable>) -> Self {
        Bundler {
            config: Arc::new(config),
            plugins,
            ets_transform_plan: None,
        }
    }

    pub fn with_ets_transform_plan(mut self, plan: Arc<RolldownEtsTransformPlan>) -> Self {
        self.ets_transform_plan = Some(plan);
        self
    }

    /// Run the full bundle pipeline and return the chunk outputs.
    ///
    /// The output remains ESM so the Rust ABC backend compiles the same
    /// CompileArkTS-stage module shape that hvigor sends to es2abc.
    pub async fn bundle(&self, plan: &BundlePlan) -> Result<BundleOutput, ArkdownError> {
        let bundle_start = Instant::now();
        plan.validate()?;
        let input_plan = RolldownInputPlan::from_bundle_plan(plan);

        // ── 2. Pass plugins directly to rolldown ─────────────────────────────
        let transform_store = HvigorModuleTransformStore::default();
        let mut rd_plugins: Vec<SharedPluginable> = self.plugins.clone();
        let source_collector =
            Arc::new(HvigorModuleTransformCollector::new(transform_store.clone()))
                as SharedPluginable;
        let source_collector_index = rd_plugins
            .iter()
            .position(|plugin| plugin.call_name().as_ref() == "apiTransform")
            .or_else(|| {
                rd_plugins
                    .iter()
                    .position(|plugin| plugin.call_name().as_ref() == "ets-transform")
            })
            .map(|index| index + 1)
            .unwrap_or(rd_plugins.len());
        rd_plugins.insert(source_collector_index, source_collector);
        #[cfg(feature = "memory-diagnostics")]
        rd_plugins.push(Arc::new(MemoryTransformDiagnosticPlugin::new()) as SharedPluginable);

        // ── 3. Build rolldown BundlerOptions ─────────────────────────────────
        //
        // Output format is ESM.  hvigor's CompileArkTS phase feeds es2abc the
        // transformed module source directly; generating CJS here injects
        // rolldown helpers (`__toESM`, `__copyProps`, etc.) that do not exist
        // in hvigor's bytecode.
        //
        // module_types maps ".ets" → Ts so rolldown parses the post-transform
        // source as standard TypeScript. EtsTransformPlugin also implements the
        // load hook, so ArkTS-only syntax such as @Builder functions and ArkUI
        // DSL blocks is lowered before rolldown's parser sees the module.
        let mut module_types = rustc_hash::FxHashMap::default();
        module_types.insert("ets".to_string(), RdModuleType::Ts);

        // cwd: canonicalize the project root so rolldown's resolver operates on
        // an absolute path.  The entry paths built above are already absolute.
        let cwd = Self::canonical_project_root(&plan.input.project_root);

        // hvigor still sets Rollup `treeshake: buildMode === "release"` in
        // arkts-pack.ts, but ESMODULE builds also set `disableGenerateBundle`.
        // The module-source handoff comes from Rollup's module graph/cache, not
        // generated chunks, so Rollup has not rewritten the transformed source
        // before collecting `resolvedIds`.  Rolldown runs OXC DCE before its AST
        // scanner when treeshake is enabled; that can fold a local string into
        // `import("pkg")` and create a module request Rollup never reports.
        let treeshake_enabled = Self::module_source_scan_treeshake_enabled();
        let treeshake = TreeshakeOptions::Boolean(treeshake_enabled);

        // manualChunks for supportChunks=true: mirrors hvigor's arkts-pack.js logic.
        // oh_modules/node_modules → "commons"; non-entry local modules → "vendors".
        let code_splitting = if plan.input.support_chunks {
            Some(CodeSplittingMode::Advanced(
                Self::build_manual_code_splitting_options()?,
            ))
        } else {
            None
        };

        let options = BundlerOptions {
            input: Some(input_plan.input_items()?),
            cwd: Some(cwd),
            format: Some(OutputFormat::Esm),
            module_types: Some(module_types),
            sourcemap: None,
            treeshake,
            code_splitting,
            transform: Some(BundlerTransformOptions {
                typescript: Some(TypeScriptOptions::default()),
                ..Default::default()
            }),
            // Match hvigor's rollup options:
            // shimMissingExports: true — suppress "missing export" errors for
            // modules that are present at runtime but not statically exported.
            shim_missing_exports: Some(plan.shim_missing_exports_override.unwrap_or(true)),
            // context: "globalThis" — hvigor sets this so top-level `this`
            // resolves to globalThis rather than undefined in strict ESM.
            context: Some(
                plan.context_override
                    .clone()
                    .unwrap_or_else(|| "globalThis".to_string()),
            ),
            // sanitizeFileName: false — hvigor disables file-name sanitization
            // so OHM URL path segments are preserved verbatim.
            sanitize_filename: Some(SanitizeFilename::Boolean(false)),
            ..Default::default()
        };
        let entry_module_ids = input_plan.entry_module_ids()?;
        #[cfg(feature = "memory-diagnostics")]
        crate::memory_diagnostics::dump_with(
            "bundle:before-scan",
            format_args!(
                "inputs={} plugins={} treeshake={} support_chunks={}",
                options.input.as_ref().map_or(0, Vec::len),
                rd_plugins.len(),
                treeshake_enabled,
                plan.input.support_chunks
            ),
        );

        // ── 4. Scan the hvigor module-source graph ──────────────────────────
        let scan_start = Instant::now();
        let mut rd_bundler = rolldown::Bundler::with_plugins(options, rd_plugins)
            .map_err(|e| ArkdownError::Bundler(RolldownBuildDiagnostics::new(e).into_message()))?;
        let module_source_scan = rd_bundler
            .scan_module_sources()
            .await
            .map_err(|e| ArkdownError::Bundler(RolldownBuildDiagnostics::new(e).into_message()))?;
        rd_bundler
            .close()
            .await
            .map_err(|e| ArkdownError::Bundler(RolldownBuildDiagnostics::new(e).into_message()))?;
        #[cfg(feature = "memory-diagnostics")]
        crate::memory_diagnostics::dump("bundle:after-scan");
        tracing::info!(
            elapsed_ms = scan_start.elapsed().as_millis(),
            "rolldown scan completed"
        );

        // ── 4b. Surface rolldown scan warnings ────────────────────────────────
        // hvigor `arkts-pack.js::generateConfig()` routes every non-fatal
        // bundler warning through `onRollupWarn(message, code, isFaMode)`:
        // `SHIMMED_EXPORT` is always suppressed, `SOURCEMAP_BROKEN` only in FA
        // mode (out of scope), and everything else is `console.warn` to the
        // build log. The scan warnings must not be silently dropped.
        for warning in &module_source_scan.warnings {
            if warning.kind().to_string() == "SHIMMED_EXPORT" {
                continue;
            }
            let message = format!("{}: {}", warning.kind(), warning);
            match warning.id() {
                Some(module_id) => tracing::warn!("{module_id}: {message}"),
                None => tracing::warn!("{message}"),
            }
        }

        // ── 5. Materialize the hvigor CompileArkTS module-source handoff ─────
        let module_source_start = Instant::now();
        let mut module_source_state = HvigorModuleSourceState::from_rolldown_scan(
            module_source_scan,
            transform_store.finish(),
        );
        #[cfg(feature = "memory-diagnostics")]
        crate::memory_diagnostics::dump_with(
            "bundle:module-source-state",
            format_args!("{}", module_source_state.memory_summary()),
        );
        tracing::info!(
            elapsed_ms = module_source_start.elapsed().as_millis(),
            "hvigor module source snapshot materialized"
        );
        let module_graph_start = Instant::now();
        let module_graph_facts = module_source_state.module_graph_facts();
        if module_graph_facts.full_module_ids.is_empty() {
            return Err(ArkdownError::Bundler(
                "rolldown did not publish module graph facts during scan".to_string(),
            ));
        }
        let chunk_graph = module_graph_facts.chunk_graph();
        let chunk_source_files = input_plan.chunk_source_files()?;
        let module_imports = module_source_state.module_imports();
        let module_source_ids =
            Self::module_source_ids(&entry_module_ids, &module_graph_facts, &module_source_state);
        let external_module_requests = module_source_state.external_module_requests();
        let har_importees = module_source_state
            .har_importees(&input_plan.module_path_map, plan.input.module_name.as_str());
        let preload_system_so_tasks = module_source_state.preload_system_so_tasks(
            &plan.input.preload_system_so_entries,
            &HvigorChunkModuleRequestProcessor::new(
                plan.bundle_name.as_str(),
                plan.input.module_name.as_str(),
            ),
        );
        #[cfg(feature = "memory-diagnostics")]
        crate::memory_diagnostics::dump_with(
            "bundle:module-graph",
            format_args!(
                "modules={} entry_modules={} importers={} module_source_ids={} external_module_requests={}",
                module_graph_facts.full_module_ids.len(),
                entry_module_ids.len(),
                module_imports.len(),
                module_source_ids.len(),
                external_module_requests.len()
            ),
        );
        tracing::info!(
            elapsed_ms = module_graph_start.elapsed().as_millis(),
            "rolldown module graph collected"
        );

        // ── 6. Write CompileArkTS handoff artifacts to disk ─────────────────
        let output_dir = plan.output.output_dir.clone();
        fs_err::create_dir_all(&output_dir).map_err(ArkdownError::Io)?;

        let source_map_packages = &plan.input.source_map_packages;
        if source_map_packages.entry_package_name.is_empty()
            || source_map_packages.entry_package_version.is_empty()
        {
            return Err(ArkdownError::Bundler(
                "source map entry package metadata is missing from BundlePlan".to_string(),
            ));
        }
        let package_versions = Self::source_map_package_versions(plan, &module_graph_facts);
        let source_map_module_root = plan
            .input
            .module_path_map
            .iter()
            .find_map(|(name, root)| (name == &plan.input.module_name).then_some(root))
            .unwrap_or(&plan.input.module_root);
        // `SourceMapGenerator::getIntermediateModuleId` determines an ETS/TS
        // source-map extension from the materialized ModuleMode cache path.
        // Move raw maps out without cloning so the source writer can publish
        // that authoritative path before we serialize the map manifest.
        let raw_source_maps = if plan.source_map == SourceMapPolicy::External {
            std::mem::take(&mut module_source_state.source_maps)
        } else {
            module_source_state.source_maps.clear();
            FxHashMap::default()
        };
        let module_source_write_start = Instant::now();
        #[cfg(feature = "memory-diagnostics")]
        crate::memory_diagnostics::dump("bundle:before-module-source-write");
        let module_source_files = HvigorModuleSourceWriter::new(
            plan.bundle_name.as_str(),
            plan.input.module_name.as_str(),
            &plan.input.project_root,
            &input_plan.module_path_map,
            &plan.input.runtime_packages,
            &plan.input.byte_code_har_packages,
            plan.use_normalized_ohm_url,
            &module_graph_facts,
            module_source_ids,
            module_source_state,
            plan.output
                .module_source_output_dir
                .clone()
                .ok_or_else(|| {
                    ArkdownError::Bundler(
                        "hvigor module source output dir is required for ModuleMode compilation"
                            .to_string(),
                    )
                })?,
        )
        .with_mock_config(plan.input.mock_config.as_ref())
        .write()?;
        #[cfg(feature = "memory-diagnostics")]
        crate::memory_diagnostics::dump_with(
            "bundle:after-module-source-write",
            format_args!("module_source_files={}", module_source_files.len()),
        );
        tracing::info!(
            elapsed_ms = module_source_write_start.elapsed().as_millis(),
            "hvigor module sources written"
        );
        let source_map_start = Instant::now();
        let source_map_opts = output::SourceMapOptions {
            project_root: Self::canonical_project_root(&plan.input.project_root),
            module_root: Self::canonical_project_root(source_map_module_root),
            module_name: source_map_packages.entry_package_name.clone(),
            package_name: source_map_packages.entry_package_name.clone(),
            version: source_map_packages.entry_package_version.clone(),
            chunk_source_files: input_plan.chunk_source_file_map(&chunk_source_files),
            package_versions,
            local_dependency_roots: input_plan
                .module_path_map
                .source_map_local_dependency_roots(),
            javascript_source_paths: Self::source_map_javascript_paths(&module_source_files),
        };
        let source_map_output = if plan.source_map == SourceMapPolicy::External {
            let mut source_maps = raw_source_maps
                .iter()
                .map(|(module_id, source_map)| (module_id.as_str(), source_map.as_ref(), false))
                .collect::<Vec<_>>();
            source_maps.sort_by(|left, right| left.0.cmp(right.0));
            output::WrittenOutput::write_hvigor_source_maps_with_metadata_from_raw(
                &output_dir,
                source_maps,
                &source_map_opts,
            )?
        } else {
            None
        };
        let source_map_input_fingerprints = source_map_output
            .as_ref()
            .map(|output| {
                self.source_map_input_fingerprints(
                    &plan.input.project_root,
                    output.entries.as_slice(),
                )
            })
            .unwrap_or_default();
        let source_maps_path = source_map_output.map(|output| output.path);
        tracing::info!(
            elapsed_ms = source_map_start.elapsed().as_millis(),
            source_map_policy = ?plan.source_map,
            "source map output written"
        );
        let npm_entries_start = Instant::now();
        // `module_mode.js::collectModuleFileList` only records package entry
        // mappings for the legacy OHM URL mode. Normalized mode derives package
        // identity from pkgContextInfo and keeps npmEntries limited to the
        // native entries appended by the Ark compiler driver.
        let npm_entries = if plan.use_normalized_ohm_url {
            Vec::new()
        } else {
            input_plan.module_path_map.npm_entries(
                &plan.input.project_root,
                plan.bundle_name.as_str(),
                &entry_module_ids,
                &module_source_files,
            )?
        };
        tracing::info!(
            elapsed_ms = npm_entries_start.elapsed().as_millis(),
            "npm entries collected"
        );
        let mut chunk_source_files = chunk_source_files.into_values().collect::<Vec<_>>();
        chunk_source_files.sort_by(|left, right| left.chunk_name.cmp(&right.chunk_name));
        let modules_abc_path = output::WrittenOutput::abc_path_in(&output_dir);

        tracing::info!(
            elapsed_ms = bundle_start.elapsed().as_millis(),
            "bundler completed"
        );

        Ok(BundleOutput {
            chunk_graph,
            output_dir,
            chunk_files: Vec::new(),
            chunk_source_files,
            module_source_files,
            entry_module_ids,
            module_imports,
            external_module_requests,
            har_importees,
            preload_system_so_tasks,
            npm_entries,
            modules_abc_path,
            source_maps_path,
            source_map_input_fingerprints,
        })
    }

    fn canonical_project_root(path: &std::path::Path) -> PathBuf {
        AbsPath::canonicalize(path)
            .map(AbsPath::into_path_buf)
            .unwrap_or_else(|_| {
                if path.has_root() {
                    lexical_normalize(path)
                } else {
                    lexical_normalize(
                        &std::env::current_dir()
                            .unwrap_or_else(|_| PathBuf::from("."))
                            .join(path),
                    )
                }
            })
    }

    fn source_map_input_fingerprints(
        &self,
        project_root: &Path,
        entries: &[output::HvigorSourceMapOutputEntry],
    ) -> Vec<SourceMapInputFingerprint> {
        let Some(ets_transform_plan) = self.ets_transform_plan.as_ref() else {
            return Vec::new();
        };
        let project_root = Self::canonical_project_root(project_root);
        let mut fingerprints = entries
            .iter()
            .filter_map(|entry| {
                let source_path = entry.source_path.as_ref()?;
                let source_path = PathBuf::from(source_path);
                let source_path = if source_path.is_absolute() {
                    source_path
                } else {
                    project_root.join(source_path)
                };
                let source = ets_transform_plan.sources.source_text(&source_path)?;
                Some(SourceMapInputFingerprint {
                    key: entry.key.clone(),
                    content_hash: CacheKeyMaterial::compute_content_hash(source.as_ref()),
                })
            })
            .collect::<Vec<_>>();
        fingerprints.sort_by(|left, right| left.key.cmp(&right.key));
        fingerprints.dedup_by(|left, right| left.key == right.key);
        fingerprints
    }

    fn module_source_scan_treeshake_enabled() -> bool {
        false
    }

    fn module_source_ids(
        entry_module_ids: &[String],
        module_graph_facts: &ModuleGraphFacts,
        module_source_state: &HvigorModuleSourceState,
    ) -> Vec<String> {
        let mut seen = FxHashSet::default();
        let mut ids = Vec::new();
        // Latest ets-loader forwards Rollup `getModuleIds()` directly to
        // ModuleMode. Rollup inserts into `modulesById` after an async native
        // resolver completes, so the remaining Map iteration order is not a
        // reproducible output contract. Keep Rolldown's stable input/graph
        // order for declared modules and sort only collector fallbacks. This
        // gives the same module set and ModuleMode metadata without making an
        // Arkdown build depend on resolver timing.
        for id in entry_module_ids {
            if seen.insert(id.clone()) {
                ids.push(id.clone());
            }
        }
        for id in &module_graph_facts.full_module_ids {
            if seen.insert(id.clone()) {
                ids.push(id.clone());
            }
        }
        let mut source_ids = module_source_state
            .sources
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        source_ids.sort();
        for id in source_ids {
            if seen.insert(id.clone()) {
                ids.push(id);
            }
        }
        ids
    }

    fn source_map_package_versions(
        plan: &BundlePlan,
        module_graph_facts: &ModuleGraphFacts,
    ) -> FxHashMap<String, String> {
        let mut package_versions = plan
            .input
            .source_map_packages
            .dependency_package_versions
            .clone();
        for module_id in &module_graph_facts.full_module_ids {
            let meta = ModuleMetaPlugin::compute_meta(
                module_id,
                &plan.input.project_root,
                &plan.input.module_name,
            );
            let Some(package_name) = meta.pkg_name.filter(|name| !name.is_empty()) else {
                continue;
            };
            let Some(package_version) = meta.pkg_version.filter(|version| !version.is_empty())
            else {
                continue;
            };
            package_versions
                .entry(package_name)
                .or_insert(package_version);
        }
        package_versions
    }

    fn source_map_javascript_paths(
        module_source_files: &[HvigorModuleSourceFile],
    ) -> FxHashSet<PathBuf> {
        module_source_files
            .iter()
            .filter(|source| {
                matches!(
                    source
                        .path
                        .extension()
                        .and_then(|extension| extension.to_str()),
                    Some("js" | "mjs" | "cjs")
                )
            })
            .map(|source| Self::canonical_project_root(Path::new(&source.module_id)))
            .collect()
    }

    /// Build `ManualCodeSplittingOptions` that mirrors hvigor's `manualChunks` callback.
    ///
    /// Group 1 (`commons`): modules under dependency module directories.
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
        let common_chunk_policy =
            Arc::new(ManualChunkPolicy::from_plan(Vec::new(), Default::default()));
        let commons_test_policy = Arc::clone(&common_chunk_policy);
        let commons_test = Arc::new(move |module_id: &str| {
            let is_common = commons_test_policy.is_common_dependency(module_id);
            Box::pin(async move { Ok(Some(is_common)) })
                as Pin<Box<dyn Future<Output = anyhow::Result<Option<bool>>> + Send + 'static>>
        });

        let commons_group = MatchGroup {
            name: MatchGroupName::Static(ChunkName::COMMONS.to_string()),
            test: Some(MatchGroupTest::Function(commons_test)),
            ..Default::default()
        };

        // Dynamic name function: returns "vendors" for non-entry modules, None for entries.
        // The function captures no state; it queries `is_entry` via ChunkingContext.
        let vendors_name_fn: Arc<
            dyn Fn(
                    &str,
                    &ChunkingContext,
                )
                    -> Pin<Box<dyn Future<Output = anyhow::Result<Option<String>>> + Send + 'static>>
                + Send
                + Sync,
        > = Arc::new(|module_id: &str, ctx: &ChunkingContext| {
            let result = ctx.get_module_info(module_id).and_then(|info| {
                if !info.is_entry {
                    Some(ChunkName::VENDORS.to_string())
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
}

struct HvigorExternalModuleRequest<'a> {
    request: &'a str,
}

impl<'a> HvigorExternalModuleRequest<'a> {
    fn new(request: &'a str) -> Self {
        Self { request }
    }

    fn is_match(&self) -> bool {
        self.request.starts_with("@ohos:")
            || self.request.starts_with("@ohos.")
            || self.request.starts_with("@hms:")
            || self.request.starts_with("@hms.")
            || self.request.starts_with("@native:")
            || self.request.starts_with("@native.")
            || self.request.starts_with("@app:")
            || self.request.starts_with("@system.")
    }
}

struct HvigorNormalizedOhmUrl<'a> {
    context: &'a BundleRuntimePackageContext,
    project_file_path: &'a str,
}

impl<'a> HvigorNormalizedOhmUrl<'a> {
    fn new(context: &'a BundleRuntimePackageContext, project_file_path: &'a str) -> Self {
        Self {
            context,
            project_file_path,
        }
    }

    fn to_request(&self) -> String {
        let is_so = if self.context.is_so { "Y" } else { "N" };
        let module_name = self.context.module_name.as_str();
        format!("@normalized:{}&{}&{}", is_so, module_name, self.to_record())
    }

    fn to_record(&self) -> String {
        let source_path = Path::new(self.project_file_path).with_extension("");
        let source_path = ArtifactPathSerializer::rel_to_slash(&source_path);
        format!(
            "{}&{}/{}&{}",
            self.context.bundle_name, self.context.package_name, source_path, self.context.version
        )
    }
}

#[derive(Debug, Clone, Default)]
struct HvigorModuleTransformStore {
    module_source_overrides: Arc<FxDashMap<String, Arc<str>>>,
    source_maps: Arc<FxDashMap<String, Arc<str>>>,
    javascript_module_ids: Arc<FxDashMap<String, ()>>,
    shared_module_ids: Arc<FxDashMap<String, ()>>,
}

impl HvigorModuleTransformStore {
    fn record(
        &self,
        module_id: String,
        module_source_code: Option<Arc<str>>,
        source_map: Option<Arc<str>>,
        emits_javascript: bool,
        is_shared_module: bool,
    ) {
        if let Some(module_source_code) = module_source_code {
            self.module_source_overrides
                .insert(module_id.clone(), module_source_code);
        }
        if let Some(source_map) = source_map {
            self.source_maps.insert(module_id.clone(), source_map);
        }
        if emits_javascript {
            self.javascript_module_ids.insert(module_id.clone(), ());
        }
        if is_shared_module {
            self.shared_module_ids.insert(module_id, ());
        }
    }

    fn finish(&self) -> HvigorModuleTransformSnapshot {
        let mut module_source_overrides = FxHashMap::default();
        for item in self.module_source_overrides.iter() {
            module_source_overrides.insert(item.key().clone(), Arc::clone(item.value()));
        }
        self.module_source_overrides.clear();
        let mut source_maps = FxHashMap::default();
        for item in self.source_maps.iter() {
            source_maps.insert(item.key().clone(), Arc::clone(item.value()));
        }
        self.source_maps.clear();
        let javascript_module_ids = self
            .javascript_module_ids
            .iter()
            .map(|item| item.key().clone())
            .collect();
        self.javascript_module_ids.clear();
        let shared_module_ids = self
            .shared_module_ids
            .iter()
            .map(|item| item.key().clone())
            .collect();
        self.shared_module_ids.clear();
        HvigorModuleTransformSnapshot {
            module_source_overrides,
            source_maps,
            javascript_module_ids,
            shared_module_ids,
        }
    }
}

struct HvigorModuleTransformSnapshot {
    module_source_overrides: FxHashMap<String, Arc<str>>,
    source_maps: FxHashMap<String, Arc<str>>,
    javascript_module_ids: FxHashSet<String>,
    shared_module_ids: FxHashSet<String>,
}

#[derive(Debug, Clone, Default)]
struct HvigorModuleSourceState {
    sources: FxHashMap<String, HvigorCollectedModuleSource>,
    source_maps: FxHashMap<String, Arc<str>>,
    resolved_imports: FxHashMap<String, FxHashMap<String, String>>,
    resolved_import_records: FxHashMap<String, Vec<HvigorResolvedImportRecord>>,
    module_ids: Vec<String>,
    seen_module_ids: FxHashSet<String>,
    full_module_is_commonjs: FxHashMap<String, bool>,
    javascript_module_ids: FxHashSet<String>,
    shared_module_ids: FxHashSet<String>,
}

impl HvigorModuleSourceState {
    fn from_rolldown_scan(
        scan: ModuleSourceScanOutput,
        mut transform: HvigorModuleTransformSnapshot,
    ) -> Self {
        let mut state = Self {
            source_maps: transform.source_maps,
            javascript_module_ids: transform.javascript_module_ids,
            shared_module_ids: transform.shared_module_ids,
            ..Self::default()
        };
        for module in scan.modules {
            let module_id = module.id.to_string();
            state.record_module_id(module_id.clone());
            let source = transform
                .module_source_overrides
                .remove(&module_id)
                .map(|source| ArcStr::from(source.as_ref()))
                .unwrap_or(module.source);
            state.sources.insert(
                module_id.clone(),
                HvigorCollectedModuleSource { code: source },
            );
            state
                .full_module_is_commonjs
                .insert(module_id.clone(), module.exports_kind.is_commonjs());

            for import in module.imports {
                let resolved_id = import
                    .resolved_id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| import.module_request.clone());
                state.record_module_id(resolved_id.clone());
                state
                    .resolved_imports
                    .entry(module_id.clone())
                    .or_default()
                    .insert(import.module_request.clone(), resolved_id);
                state
                    .resolved_import_records
                    .entry(module_id.clone())
                    .or_default()
                    .push(HvigorResolvedImportRecord {
                        specifier: import.module_request,
                        source_span: HvigorSourceSpan {
                            start: import.importer_span.start,
                            end: import.importer_span.end,
                        },
                    });
            }
        }
        state
    }

    fn record_module_id(&mut self, id: String) {
        if self.seen_module_ids.insert(id.clone()) {
            self.module_ids.push(id);
        }
    }

    fn module_graph_facts(&self) -> ModuleGraphFacts {
        ModuleGraphFacts {
            // `record_module_id` already de-duplicates while retaining the
            // module scan's insertion order. Sorting here changes filesInfo,
            // proto cache and merged ABC input order relative to hvigor.
            full_module_ids: self.module_ids.clone(),
            full_module_is_commonjs: self.full_module_is_commonjs.clone(),
        }
    }

    fn module_imports(&self) -> FxHashMap<String, Vec<String>> {
        let mut imports = FxHashMap::default();
        for (importer, resolved_imports) in &self.resolved_imports {
            let mut imported_ids = resolved_imports.values().cloned().collect::<Vec<_>>();
            imported_ids.sort();
            imported_ids.dedup();
            imports.insert(importer.clone(), imported_ids);
        }
        imports
    }

    fn external_module_requests(&self) -> Vec<String> {
        let mut requests = Vec::new();
        for resolved_imports in self.resolved_imports.values() {
            for (specifier, resolved_id) in resolved_imports {
                if HvigorExternalModuleRequest::new(specifier).is_match() {
                    requests.push(specifier.clone());
                } else if HvigorExternalModuleRequest::new(resolved_id).is_match() {
                    requests.push(resolved_id.clone());
                }
            }
        }
        requests.sort();
        requests.dedup();
        requests
    }

    fn har_importees(
        &self,
        module_path_map: &HvigorModulePathMap,
        current_module_name: &str,
    ) -> HvigorHarImportees {
        let mut importees = HvigorHarImportees::default();
        let mut seen_compiled = FxHashSet::default();
        let mut seen_direct = FxHashSet::default();
        let mut seen_non_entry = FxHashSet::default();

        // `process-importee.js::collectImportees` walks Rolldown modules and
        // each module's resolvedIds in encounter order. `module_ids` retains
        // that scan order while `resolved_import_records` retains request
        // order without depending on FxHashMap iteration.
        for module_id in &self.module_ids {
            let Some(records) = self.resolved_import_records.get(module_id) else {
                continue;
            };
            let belongs_to_current_module = module_path_map
                .source_file_for_entry(module_id)
                .is_ok_and(|source| {
                    source.module_name == current_module_name
                        && matches!(source.kind, HvigorChunkSourceKind::SourceModule)
                });
            for record in records {
                let request = HvigorImporteeRequest::new(&record.specifier);
                if request.is_valid(false) && seen_compiled.insert(record.specifier.clone()) {
                    importees.compiled.push(record.specifier.clone());
                }
                if !belongs_to_current_module {
                    continue;
                }
                if request.is_valid(true) && seen_non_entry.insert(record.specifier.clone()) {
                    importees.non_entry.push(record.specifier.clone());
                }
                if request.is_valid(false) && seen_direct.insert(record.specifier.clone()) {
                    importees.direct.push(record.specifier.clone());
                }
            }
        }
        importees
    }

    fn preload_system_so_tasks(
        &self,
        entry_files: &[PathBuf],
        processor: &HvigorChunkModuleRequestProcessor,
    ) -> Vec<PreloadSystemSoTask> {
        if entry_files.is_empty() {
            return Vec::new();
        }

        let mut module_ids_by_path = FxHashMap::<PathBuf, String>::default();
        for module_id in &self.module_ids {
            let module_id = module_id.strip_prefix('\0').unwrap_or(module_id);
            let path = Path::new(module_id);
            if path.has_root() {
                module_ids_by_path.insert(
                    arkdown_path::canonicalize_or_owned(path),
                    module_id.to_string(),
                );
            }
        }

        let mut pending = entry_files
            .iter()
            .filter_map(|entry| {
                module_ids_by_path
                    .get(&arkdown_path::canonicalize_or_owned(entry))
                    .cloned()
            })
            .collect::<Vec<_>>();
        let mut reachable = FxHashSet::default();
        while let Some(module_id) = pending.pop() {
            if !reachable.insert(module_id.clone()) {
                continue;
            }
            if let Some(imports) = self.resolved_imports.get(&module_id) {
                for resolved_id in imports.values() {
                    if self.sources.contains_key(resolved_id) {
                        pending.push(resolved_id.clone());
                    }
                }
            }
        }

        let mut reachable = reachable.into_iter().collect::<Vec<_>>();
        reachable.sort();
        let mut seen_names = FxHashSet::default();
        let mut tasks = Vec::new();
        for module_id in reachable {
            let Some(records) = self.resolved_import_records.get(&module_id) else {
                continue;
            };
            for record in records {
                let Some(ohm_url) =
                    PreloadSystemSoRequest::new(&record.specifier, processor).to_ohm_url()
                else {
                    continue;
                };
                if !seen_names.insert(record.specifier.clone()) {
                    continue;
                }
                tasks.push(PreloadSystemSoTask {
                    name: record.specifier.clone(),
                    src_entry: record.specifier.clone(),
                    ohm_url,
                });
            }
        }
        tasks
    }
}

struct HvigorImporteeRequest<'a> {
    value: &'a str,
}

impl<'a> HvigorImporteeRequest<'a> {
    fn new(value: &'a str) -> Self {
        Self { value }
    }

    /// Exact shape of latest `process-importee.js::isValidImportee`.
    fn is_valid(&self, require_subpath: bool) -> bool {
        if self.value.is_empty()
            || self.value.contains('\0')
            || Path::new(self.value).is_absolute()
            || self.value.starts_with('.')
        {
            return false;
        }
        if !require_subpath {
            return true;
        }
        let segment_count = self
            .value
            .split(['/', '\\'])
            .filter(|segment| !segment.is_empty())
            .count();
        segment_count > usize::from(self.value.starts_with('@')) + 1
    }
}

struct PreloadSystemSoRequest<'a> {
    request: &'a str,
    processor: &'a HvigorChunkModuleRequestProcessor,
}

impl<'a> PreloadSystemSoRequest<'a> {
    fn new(request: &'a str, processor: &'a HvigorChunkModuleRequestProcessor) -> Self {
        Self { request, processor }
    }

    fn to_ohm_url(&self) -> Option<String> {
        let ohm_url = self.processor.process_request(self.request);
        if ohm_url != self.request || Self::is_pre_normalized_system_request(self.request) {
            Some(ohm_url)
        } else {
            None
        }
    }

    fn is_pre_normalized_system_request(request: &str) -> bool {
        request.starts_with("@ohos:")
            || request.starts_with("@hms:")
            || request.starts_with("@native:")
    }
}

#[cfg(feature = "memory-diagnostics")]
impl HvigorModuleSourceState {
    fn memory_summary(&self) -> String {
        let source_bytes = self
            .sources
            .values()
            .map(|source| source.code.len())
            .sum::<usize>();
        let source_map_bytes = self
            .source_maps
            .values()
            .map(|source_map| source_map.len())
            .sum::<usize>();
        let resolved_imports = self
            .resolved_imports
            .values()
            .map(|imports| imports.len())
            .sum::<usize>();
        let resolved_import_records = self
            .resolved_import_records
            .values()
            .map(Vec::len)
            .sum::<usize>();
        format!(
            "sources={} source_bytes={} source_maps={} source_map_bytes={} module_ids={} resolved_importers={} resolved_imports={} resolved_import_records={} pending_resolved_imports={} full_commonjs={}",
            self.sources.len(),
            source_bytes,
            self.source_maps.len(),
            source_map_bytes,
            self.module_ids.len(),
            self.resolved_imports.len(),
            resolved_imports,
            resolved_import_records,
            0,
            self.full_module_is_commonjs.len()
        )
    }
}

#[derive(Debug, Clone)]
struct HvigorCollectedModuleSource {
    code: ArcStr,
}

#[derive(Debug, Clone)]
struct HvigorResolvedImportRecord {
    specifier: String,
    source_span: HvigorSourceSpan,
}

#[derive(Debug, Clone, Copy)]
struct HvigorSourceSpan {
    start: u32,
    end: u32,
}

impl HvigorSourceSpan {
    #[cfg(test)]
    fn for_module_request_literal(source: &str, module_id: &str, specifier: &str) -> Option<Self> {
        HvigorModuleRequestSpecifiers::new(source, module_id)
            .collect_literals()
            .into_iter()
            .find(|literal| literal.specifier == specifier)
            .map(|literal| literal.source_span)
    }
}

#[derive(Debug)]
struct HvigorModuleTransformCollector {
    store: HvigorModuleTransformStore,
}

impl HvigorModuleTransformCollector {
    fn new(store: HvigorModuleTransformStore) -> Self {
        Self { store }
    }

    /// `transform.js::preserveSourceMap` ignores Rollup/CommonJS virtual
    /// modules. Only a materializable runtime JavaScript module can own a
    /// ModuleMode source-map entry.
    fn is_runtime_javascript_source(module_id: &str) -> bool {
        HvigorModuleSourceId::new(module_id).should_compile()
            && arkdown_ets_transform::SourceClassifier::classify(module_id).is_javascript_runtime()
    }

    fn restore_hvigor_javascript_mapping_lines(
        raw_map: String,
        is_comment_only_javascript: bool,
    ) -> String {
        let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&raw_map) else {
            return raw_map;
        };
        let Some(mappings) = value.get("mappings").and_then(|value| value.as_str()) else {
            return raw_map;
        };
        let mappings = if is_comment_only_javascript {
            mappings.is_empty().then_some(";".to_string())
        } else {
            (!mappings.ends_with(';')).then(|| format!("{mappings};"))
        };
        if let Some(mappings) = mappings {
            value["mappings"] = serde_json::Value::String(mappings);
            return serde_json::to_string(&value).unwrap_or(raw_map);
        }
        raw_map
    }
}

impl Plugin for HvigorModuleTransformCollector {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("arkdown-module-source-transform-collector")
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::Transform
    }

    fn transform(
        &self,
        ctx: SharedTransformPluginContext,
        args: &HookTransformArgs<'_>,
    ) -> impl Future<Output = HookTransformReturn> + Send {
        let store = self.store.clone();
        let module_id = args.id.to_string();
        let metadata = ctx
            .meta()
            .get::<EtsTransformModuleMetadata>()
            .and_then(|metadata| metadata.take(&module_id));
        let is_runtime_javascript = Self::is_runtime_javascript_source(&module_id);
        let source_map = if let Some(metadata) = metadata.as_ref() {
            // Latest IDE ets-loader `transform.js::preserveSourceMap` retains
            // Rollup's combined mappings verbatim, then changes only source
            // attribution/package metadata in its SourceMapGenerator. Rolldown
            // drops the final unmapped line that TypeScript's source-map URL
            // comment contributes to an emitted JavaScript module. Restore
            // that boundary for SDK `Program.emit` output; comment-only input
            // is its one-line `"mappings":";"` sentinel form.
            let raw_map = ctx.get_combined_sourcemap().to_json_string();
            let raw_map = if metadata.emits_javascript {
                Self::restore_hvigor_javascript_mapping_lines(
                    raw_map,
                    metadata.is_comment_only_javascript,
                )
            } else {
                raw_map
            };
            Some(Arc::<str>::from(raw_map))
        } else if is_runtime_javascript {
            // ets-loader's SourceMapGenerator records runtime JavaScript as
            // well as ETS/TS transform inputs. JS bypasses EtsTransformPlugin,
            // so retain Rolldown's identity/combined map here instead of
            // silently losing every package JS source from sourceMaps.json.
            Some(Arc::<str>::from(
                ctx.get_combined_sourcemap().to_json_string(),
            ))
        } else {
            None
        };
        async move {
            if let Some(metadata) = metadata {
                store.record(
                    module_id,
                    metadata.module_source_code,
                    source_map,
                    metadata.emits_javascript,
                    metadata.is_shared_module,
                );
            } else if let Some(source_map) = source_map {
                store.record(module_id, None, Some(source_map), true, false);
            }
            Ok(None)
        }
    }
}

#[cfg(feature = "memory-diagnostics")]
#[derive(Debug, Default)]
struct MemoryTransformDiagnosticPlugin {
    transformed_modules: AtomicUsize,
    transformed_code_bytes: AtomicUsize,
}

#[cfg(feature = "memory-diagnostics")]
impl MemoryTransformDiagnosticPlugin {
    fn new() -> Self {
        Self::default()
    }
}

#[cfg(feature = "memory-diagnostics")]
impl Plugin for MemoryTransformDiagnosticPlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("arkdown-memory-transform-diagnostics")
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::Transform
    }

    fn transform(
        &self,
        _ctx: SharedTransformPluginContext,
        args: &HookTransformArgs<'_>,
    ) -> impl Future<Output = HookTransformReturn> + Send {
        let count = self.transformed_modules.fetch_add(1, Ordering::Relaxed) + 1;
        let bytes = self
            .transformed_code_bytes
            .fetch_add(args.code.len(), Ordering::Relaxed)
            + args.code.len();
        if count == 1 || count % 500 == 0 {
            crate::memory_diagnostics::dump_with(
                "bundle:transform-progress",
                format_args!(
                    "modules={} observed_code_bytes={} last_module={} last_code_bytes={}",
                    count,
                    bytes,
                    args.id,
                    args.code.len()
                ),
            );
        }
        async { Ok(None) }
    }
}

struct HvigorModuleSourceWriter<'a> {
    bundle_name: &'a str,
    module_name: &'a str,
    project_root: &'a Path,
    module_path_map: &'a HvigorModulePathMap,
    runtime_packages: &'a BundleRuntimePackageInfo,
    bytecode_har_imports: HvigorBytecodeHarImports<'a>,
    use_normalized_ohm_url: bool,
    module_graph_facts: &'a ModuleGraphFacts,
    module_source_ids: Vec<String>,
    state: HvigorModuleSourceState,
    output_dir: PathBuf,
    processor: HvigorChunkModuleRequestProcessor,
    mock_config: Option<&'a BundleMockConfig>,
}

#[derive(Default)]
struct HvigorModuleSourceRecordIndex {
    by_module_id: FxHashMap<String, HvigorModuleSourceRecord>,
    by_canonical_module_id: FxHashMap<String, String>,
}

struct HvigorModuleSourceRecord {
    ownership: HvigorChunkSource,
    record_name: String,
}

struct HvigorModuleSourceWriteJob {
    module_id: String,
    path: PathBuf,
}

#[derive(Default)]
struct HvigorMockConfigRewrites {
    imports_by_module: FxHashMap<String, HvigorChunkImportMap>,
}

struct HvigorMockConfigSource {
    source: String,
    module_id: PathBuf,
}

impl<'a> HvigorModuleSourceWriter<'a> {
    fn new(
        bundle_name: &'a str,
        module_name: &'a str,
        project_root: &'a Path,
        module_path_map: &'a HvigorModulePathMap,
        runtime_packages: &'a BundleRuntimePackageInfo,
        byte_code_har_packages: &'a [String],
        use_normalized_ohm_url: bool,
        module_graph_facts: &'a ModuleGraphFacts,
        module_source_ids: Vec<String>,
        state: HvigorModuleSourceState,
        output_dir: PathBuf,
    ) -> Self {
        Self {
            bundle_name,
            module_name,
            project_root,
            module_path_map,
            runtime_packages,
            bytecode_har_imports: HvigorBytecodeHarImports::new(
                runtime_packages,
                byte_code_har_packages,
                use_normalized_ohm_url,
            ),
            use_normalized_ohm_url,
            module_graph_facts,
            module_source_ids,
            state,
            output_dir,
            processor: HvigorChunkModuleRequestProcessor::new(bundle_name, module_name),
            mock_config: None,
        }
    }

    fn with_mock_config(mut self, mock_config: Option<&'a BundleMockConfig>) -> Self {
        self.mock_config = mock_config;
        self
    }

    fn write(self) -> Result<Vec<HvigorModuleSourceFile>, ArkdownError> {
        let had_output_dir = self.output_dir.exists();
        if had_output_dir && !self.output_dir.is_dir() {
            fs_err::remove_file(&self.output_dir)
                .map_err(|e| ArkdownError::Bundler(e.to_string()))?;
        }
        if !self.output_dir.exists() {
            fs_err::create_dir_all(&self.output_dir)
                .map_err(|e| ArkdownError::Bundler(e.to_string()))?;
        }

        let mut module_ids = self
            .module_source_ids
            .iter()
            .filter(|module_id| self.state.sources.contains_key(module_id.as_str()))
            .filter(|module_id| HvigorModuleSourceId::new(module_id).should_compile())
            .cloned()
            .collect::<Vec<_>>();
        module_ids.dedup();

        let module_records = self.module_source_record_index(&module_ids)?;
        let mock_rewrites = self.prepare_mock_config(&module_records)?;
        let jobs = module_ids
            .iter()
            .filter_map(|module_id| {
                self.module_source_write_job(module_id, &module_records)
                    .transpose()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut destinations = FxHashMap::<PathBuf, String>::default();
        for job in &jobs {
            if let Some(existing) = destinations.insert(job.path.clone(), job.module_id.clone()) {
                return Err(ArkdownError::Bundler(format!(
                    "CompileArkTS module sources '{existing}' and '{}' resolve to the same output path {}",
                    job.module_id,
                    job.path.display()
                )));
            }
        }
        let files = jobs
            .par_iter()
            .map(|job| self.write_module_source(job, &module_records, &mock_rewrites))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let active_paths = files
            .iter()
            .map(|file| file.path.clone())
            .collect::<FxHashSet<_>>();
        if had_output_dir {
            self.remove_stale_output_files(&active_paths)?;
        }
        Ok(files)
    }

    /// Mirror ets-loader `ModuleSourceFile` mock processing at its ModuleMode
    /// boundary. Runtime imports continue to reference the real OHM record;
    /// `mock-config.json` maps that record to the configured mock. Static
    /// imports made by a mock source receive the SDK `.origin` suffix and a
    /// reverse entry so hamock can reach the unmocked implementation.
    fn prepare_mock_config(
        &self,
        module_records: &HvigorModuleSourceRecordIndex,
    ) -> Result<HvigorMockConfigRewrites, ArkdownError> {
        let Some(options) = self.mock_config else {
            return Ok(HvigorMockConfigRewrites::default());
        };
        let Some(config_path) = options
            .process
            .then_some(options.user_config_path.as_ref())
            .flatten()
        else {
            self.remove_mock_config_cache(options)?;
            return Ok(HvigorMockConfigRewrites::default());
        };

        let user_content = fs_err::read_to_string(config_path)
            .map_err(|error| ArkdownError::Bundler(error.to_string()))?;
        let user_document = json5::from_str::<serde_json::Value>(&user_content).map_err(|error| {
            ArkdownError::Bundler(format!(
                "invalid mock config '{}': {error}",
                config_path.display()
            ))
        })?;
        let user_entries = user_document.as_object().ok_or_else(|| {
            ArkdownError::Bundler(format!(
                "mock config '{}' must contain a JSON object",
                config_path.display()
            ))
        })?;
        let module_root = self.module_path_map.current_module_root();
        let mut configured_sources = IndexMap::<String, HvigorMockConfigSource>::new();
        let mut mock_module_ids = FxHashSet::default();
        for (key, entry) in user_entries {
            let Some(source) = entry.get("source").and_then(serde_json::Value::as_str) else {
                continue;
            };
            if source.is_empty() {
                continue;
            }
            let configured_path = PathBuf::from(source);
            let module_id = if configured_path.has_root() {
                configured_path
            } else {
                module_root.join(configured_path)
            };
            let module_id = arkdown_path::canonicalize_or_owned(&module_id);
            if !module_id.is_file() {
                return Err(ArkdownError::Bundler(format!(
                    "mock source '{}' configured by '{}' does not exist",
                    module_id.display(),
                    config_path.display()
                )));
            }
            mock_module_ids.insert(module_id.clone());
            configured_sources.insert(
                key.clone(),
                HvigorMockConfigSource {
                    source: source.to_string(),
                    module_id,
                },
            );
        }

        let mut rewrites = HvigorMockConfigRewrites::default();
        let mut transformed_entries = serde_json::Map::new();
        // `collectMockConfigInfo()` eagerly converts dependency keys through
        // `getMockConfigKey2ModuleInfoMap`, even when no source currently
        // imports that dependency. Preserve that observable config output.
        for (key, configured) in &configured_sources {
            let Some(real_module_id) = self.mock_dependency_module_id(key, options) else {
                continue;
            };
            let real_module_id = real_module_id.to_string_lossy();
            let Some(real_ohm_url) =
                self.ohm_url_for_resolved_id(&real_module_id, module_records)?
            else {
                continue;
            };
            let mock_module_id = configured.module_id.to_string_lossy();
            let Some(mock_ohm_url) =
                self.ohm_url_for_resolved_id(&mock_module_id, module_records)?
            else {
                continue;
            };
            transformed_entries
                .entry(real_ohm_url)
                .or_insert_with(|| serde_json::json!({ "source": mock_ohm_url }));
        }
        let mut module_ids = self.module_source_ids.clone();
        module_ids.sort();
        module_ids.dedup();
        for module_id in module_ids {
            let Some(source) = self.state.sources.get(&module_id) else {
                continue;
            };
            let (imports, _) = self.imports_for_module(&module_id, module_records)?;
            let canonical_module_id = arkdown_path::canonicalize_or_owned(Path::new(&module_id));
            let is_mock_source = mock_module_ids.contains(&canonical_module_id);
            let requests =
                HvigorModuleRequestSpecifiers::new(source.code.as_ref(), &module_id)
                    .collect_literals();
            for request in requests {
                let Some(ohm_url) = imports.get(&request.specifier) else {
                    continue;
                };
                let configured = configured_sources
                    .get(&request.specifier)
                    .or_else(|| {
                        self.local_mock_config_key(&module_id, &request.specifier, options)
                            .and_then(|key| configured_sources.get(&key))
                    });
                if let Some(configured) = configured {
                    let mock_module_id = configured.module_id.to_string_lossy();
                    let mock_ohm_url = self
                        .ohm_url_for_resolved_id(&mock_module_id, module_records)?
                        .ok_or_else(|| {
                            ArkdownError::Bundler(format!(
                                "cannot derive OHM URL for mock source '{}'",
                                configured.source
                            ))
                        })?;
                    transformed_entries.entry(ohm_url.clone()).or_insert_with(|| {
                        serde_json::json!({ "source": mock_ohm_url })
                    });
                }
                if is_mock_source
                    && !request.is_dynamic
                    && !SourceClassifier::classify(&module_id).is_javascript_runtime()
                {
                    let origin_url = format!("{ohm_url}.origin");
                    rewrites
                        .imports_by_module
                        .entry(module_id.clone())
                        .or_default()
                        .insert(request.specifier.clone(), origin_url.clone());
                    transformed_entries
                        .entry(origin_url)
                        .or_insert_with(|| serde_json::json!({ "source": ohm_url }));
                }
            }
        }
        self.write_mock_config_artifacts(
            options,
            &user_document,
            serde_json::Value::Object(transformed_entries),
        )?;
        Ok(rewrites)
    }

    fn mock_dependency_module_id(
        &self,
        key: &str,
        options: &BundleMockConfig,
    ) -> Option<PathBuf> {
        let mut components = key.split(['/', '\\']);
        let first = components.next()?;
        if first.starts_with('.') {
            return None;
        }
        let dependency_name = if first.starts_with('@') {
            let package = components.next()?;
            format!("{first}/{package}")
        } else {
            first.to_string()
        };
        let dependency = options
            .dependencies
            .iter()
            .rev()
            .find(|dependency| dependency.dependency_name == dependency_name)?;
        let suffix = components.fold(PathBuf::new(), |path, component| path.join(component));
        Some(if suffix.as_os_str().is_empty() {
            dependency.main_file_path.clone()
        } else {
            dependency.package_root.join(suffix)
        })
    }

    fn local_mock_config_key(
        &self,
        module_id: &str,
        specifier: &str,
        options: &BundleMockConfig,
    ) -> Option<String> {
        let resolved_id = self.state.resolved_imports.get(module_id)?.get(specifier)?;
        let resolved_path = arkdown_path::canonicalize_or_owned(Path::new(resolved_id));
        let source_root = arkdown_path::canonicalize_or_owned(&options.ets_source_root);
        let relative = resolved_path.strip_prefix(source_root).ok()?;
        Some(ArtifactPathSerializer::rel_to_slash(relative))
    }

    fn remove_mock_config_cache(
        &self,
        options: &BundleMockConfig,
    ) -> Result<(), ArkdownError> {
        for path in [
            &options.transformed_cache_path,
            &options.user_cache_path,
        ] {
            match fs_err::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => return Err(ArkdownError::Bundler(error.to_string())),
            }
        }
        Ok(())
    }

    fn write_mock_config_artifacts(
        &self,
        options: &BundleMockConfig,
        user_document: &serde_json::Value,
        transformed_document: serde_json::Value,
    ) -> Result<(), ArkdownError> {
        for path in [
            &options.transformed_output_path,
            &options.transformed_cache_path,
            &options.user_cache_path,
        ] {
            if let Some(parent) = path.parent() {
                fs_err::create_dir_all(parent)
                    .map_err(|error| ArkdownError::Bundler(error.to_string()))?;
            }
        }

        let serialized_user = serde_json::to_vec(user_document)
            .map_err(|error| ArkdownError::Bundler(error.to_string()))?;
        let cached_user = fs_err::read_to_string(&options.user_cache_path)
            .ok()
            .and_then(|content| json5::from_str::<serde_json::Value>(&content).ok())
            .and_then(|document| serde_json::to_vec(&document).ok());
        let cache_is_usable = options.transformed_cache_path.is_file()
            && cached_user.as_deref() == Some(serialized_user.as_slice());

        if !cache_is_usable {
            let transformed = serde_json::to_vec(&transformed_document)
                .map_err(|error| ArkdownError::Bundler(error.to_string()))?;
            fs_err::write(&options.transformed_output_path, &transformed)
                .map_err(|error| ArkdownError::Bundler(error.to_string()))?;
            fs_err::write(&options.transformed_cache_path, transformed)
                .map_err(|error| ArkdownError::Bundler(error.to_string()))?;
            let user_config_path = options.user_config_path.as_ref().ok_or_else(|| {
                ArkdownError::Bundler("active mock processing has no user config path".to_string())
            })?;
            fs_err::copy(user_config_path, &options.user_cache_path)
                .map_err(|error| ArkdownError::Bundler(error.to_string()))?;
            return Ok(());
        }

        let cached_content = fs_err::read_to_string(&options.transformed_cache_path)
            .map_err(|error| ArkdownError::Bundler(error.to_string()))?;
        let mut cached_document = json5::from_str::<serde_json::Value>(&cached_content)
            .map_err(|error| ArkdownError::Bundler(error.to_string()))?;
        let cached_entries = cached_document.as_object_mut().ok_or_else(|| {
            ArkdownError::Bundler(format!(
                "cached mock config '{}' must contain a JSON object",
                options.transformed_cache_path.display()
            ))
        })?;
        let transformed_entries = transformed_document.as_object().ok_or_else(|| {
            ArkdownError::Bundler("transformed mock config must contain a JSON object".to_string())
        })?;
        let mut changed = false;
        for (key, value) in transformed_entries {
            if !cached_entries.contains_key(key) {
                cached_entries.insert(key.clone(), value.clone());
                changed = true;
            }
        }
        if changed {
            let merged = serde_json::to_vec(&cached_document)
                .map_err(|error| ArkdownError::Bundler(error.to_string()))?;
            fs_err::write(&options.transformed_output_path, &merged)
                .map_err(|error| ArkdownError::Bundler(error.to_string()))?;
            fs_err::write(&options.transformed_cache_path, merged)
                .map_err(|error| ArkdownError::Bundler(error.to_string()))?;
        } else {
            fs_err::copy(
                &options.transformed_cache_path,
                &options.transformed_output_path,
            )
            .map_err(|error| ArkdownError::Bundler(error.to_string()))?;
        }
        Ok(())
    }

    fn module_source_write_job(
        &self,
        module_id: &str,
        module_records: &HvigorModuleSourceRecordIndex,
    ) -> Result<Option<HvigorModuleSourceWriteJob>, ArkdownError> {
        if !self.state.sources.contains_key(module_id) {
            return Ok(None);
        }
        let Some(module_record) = module_records.by_module_id.get(module_id) else {
            return Ok(None);
        };
        let ownership = &module_record.ownership;
        let emits_javascript = self.state.javascript_module_ids.contains(module_id);
        let path = HvigorModuleSourceOutputPath::new(
            &self.output_dir,
            self.project_root,
            ownership,
            &ownership.module_name,
            &ownership.source_file,
            module_id,
            emits_javascript,
        )
        .path()?;
        Ok(Some(HvigorModuleSourceWriteJob {
            module_id: module_id.to_string(),
            path,
        }))
    }

    fn write_module_source(
        &self,
        job: &HvigorModuleSourceWriteJob,
        module_records: &HvigorModuleSourceRecordIndex,
        mock_rewrites: &HvigorMockConfigRewrites,
    ) -> Result<Option<HvigorModuleSourceFile>, ArkdownError> {
        let module_id = job.module_id.as_str();
        let Some(source) = self.state.sources.get(module_id) else {
            return Ok(None);
        };
        let Some(module_record) = module_records.by_module_id.get(module_id) else {
            return Ok(None);
        };
        let ownership = &module_record.ownership;
        let (mut imports, imported_record_names) =
            self.imports_for_module(module_id, module_records)?;
        if let Some(overrides) = mock_rewrites.imports_by_module.get(module_id) {
            imports.extend(
                overrides
                    .iter()
                    .map(|(specifier, request)| (specifier.clone(), request.clone())),
            );
        }
        let code = HvigorModuleSourceImportRewriter::new(source.code.as_ref(), module_id, &imports)
            .rewrite();
        let emits_javascript = self.state.javascript_module_ids.contains(module_id);
        if let Some(parent) = job.path.parent() {
            fs_err::create_dir_all(parent).map_err(|e| ArkdownError::Bundler(e.to_string()))?;
        }
        let code = Self::finalize_source_text(code.into_owned(), emits_javascript);
        self.write_file(&job.path, code.as_bytes())?;
        let origin_source_lang = HvigorModuleSourceId::new(module_id)
            .origin_source_lang()
            .to_string();
        let entry_package = self.runtime_packages.packages.get(self.module_name);
        let current_package_identity = self.module_path_map.current_package_identity();
        let entry_package_name = entry_package
            .map(|context| context.package_name.as_str())
            .filter(|name| !name.is_empty())
            .or_else(|| current_package_identity.map(|(name, _)| name))
            .unwrap_or(self.module_name);
        let entry_package_version = entry_package
            .map(|context| context.version.as_str())
            .filter(|version| !version.is_empty())
            .or_else(|| current_package_identity.map(|(_, version)| version))
            .unwrap_or_default();
        Ok(Some(HvigorModuleSourceFile {
            module_id: module_id.to_string(),
            path: job.path.clone(),
            record_name: module_record.record_name.clone(),
            source_file: ownership.file_info_source_file(
                entry_package_name,
                entry_package_version,
                self.module_path_map.has_dependency_package_info(module_id),
                emits_javascript,
            ),
            package_name: Some(self.file_info_package_name(module_id, ownership)?),
            is_commonjs: self
                .module_graph_facts
                .full_module_is_commonjs
                .get(module_id)
                .copied()
                .unwrap_or(false),
            is_shared_module: self.state.shared_module_ids.contains(module_id),
            origin_source_lang,
            imported_record_names,
        }))
    }

    fn finalize_source_text(mut code: String, emits_javascript: bool) -> String {
        if !emits_javascript && !code.ends_with('\n') {
            code.push('\n');
        }
        code
    }

    fn write_file(&self, path: &Path, contents: &[u8]) -> Result<(), ArkdownError> {
        match fs_err::metadata(path) {
            Ok(metadata) if metadata.is_dir() => {
                fs_err::remove_dir_all(path).map_err(|e| ArkdownError::Bundler(e.to_string()))?;
            }
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(ArkdownError::Bundler(error.to_string())),
        }

        fs_err::write(path, contents).map_err(|e| ArkdownError::Bundler(e.to_string()))
    }

    fn remove_stale_output_files(
        &self,
        active_paths: &FxHashSet<PathBuf>,
    ) -> Result<(), ArkdownError> {
        if !self.output_dir.is_dir() {
            return Ok(());
        }

        let mut stack = vec![self.output_dir.clone()];
        let mut dirs = Vec::new();
        while let Some(dir) = stack.pop() {
            dirs.push(dir.clone());
            for entry in fs_err::read_dir(&dir).map_err(|e| ArkdownError::Bundler(e.to_string()))? {
                let entry = entry.map_err(|e| ArkdownError::Bundler(e.to_string()))?;
                let file_type = entry
                    .file_type()
                    .map_err(|e| ArkdownError::Bundler(e.to_string()))?;
                let path = entry.path();
                if file_type.is_dir() {
                    stack.push(path);
                    continue;
                }
                let is_stale_module_source =
                    file_type.is_file() && !active_paths.contains(&path) && {
                        let source_kind = SourceClassifier::classify_path(&path);
                        source_kind.is_ets_transform_input() || source_kind.is_javascript_runtime()
                    };
                if is_stale_module_source {
                    fs_err::remove_file(&path).map_err(|e| ArkdownError::Bundler(e.to_string()))?;
                }
            }
        }

        for dir in dirs.into_iter().rev() {
            if dir == self.output_dir {
                continue;
            }
            match fs_err::remove_dir(&dir) {
                Ok(()) => {}
                Err(error)
                    if matches!(
                        error.kind(),
                        ErrorKind::DirectoryNotEmpty | ErrorKind::NotFound
                    ) => {}
                Err(error) => return Err(ArkdownError::Bundler(error.to_string())),
            }
        }
        Ok(())
    }

    fn module_source_record_index(
        &self,
        module_ids: &[String],
    ) -> Result<HvigorModuleSourceRecordIndex, ArkdownError> {
        let mut index = HvigorModuleSourceRecordIndex::default();
        for module_id in module_ids {
            let ownership = self.module_path_map.source_file_for_entry(module_id)?;
            if ownership.module_name.is_empty() {
                continue;
            }
            let record_name = if let Some(record_name) =
                self.module_path_map.module_meta_info_ohm_url(module_id)
            {
                record_name.to_string()
            } else if self.use_normalized_ohm_url {
                ownership.normalized_record_name(self.runtime_packages)?
            } else {
                self.module_path_map.non_normalized_record_name(
                    self.project_root,
                    module_id,
                    self.bundle_name,
                    self.module_name,
                    &ownership,
                )?
            };
            if let Some(canonical_module_id) = Self::canonical_module_id(module_id) {
                index
                    .by_canonical_module_id
                    .insert(canonical_module_id, module_id.clone());
            }
            index.by_module_id.insert(
                module_id.clone(),
                HvigorModuleSourceRecord {
                    ownership,
                    record_name,
                },
            );
        }
        Ok(index)
    }

    fn canonical_module_id(module_id: &str) -> Option<String> {
        let path = Path::new(module_id);
        path.has_root().then(|| {
            HvigorModulePathMap::canonical_root(path)
                .to_string_lossy()
                .into_owned()
        })
    }

    fn record_for_resolved_id<'b>(
        module_records: &'b HvigorModuleSourceRecordIndex,
        resolved_id: &str,
    ) -> Option<&'b HvigorModuleSourceRecord> {
        module_records.by_module_id.get(resolved_id).or_else(|| {
            Self::canonical_module_id(resolved_id)
                .and_then(|canonical| module_records.by_canonical_module_id.get(&canonical))
                .and_then(|module_id| module_records.by_module_id.get(module_id))
        })
    }

    fn file_info_package_name(
        &self,
        module_id: &str,
        ownership: &HvigorChunkSource,
    ) -> Result<String, ArkdownError> {
        // The generated ResourceTable carries the owning module name in its
        // fixed Ohm URL (e.g. `com.example.arkdown.test/entry/build/
        // generated/r/ResourceTable`) even for the ohosTest module.
        if let Some(ohm_url) = self.module_path_map.module_meta_info_ohm_url(module_id) {
            if let Some((_, module)) = ohm_url.split_once('/') {
                if let Some(module) = module.split('/').next() {
                    return Ok(module.to_string());
                }
            }
        }
        match &ownership.kind {
            // `ModuleMode.processModuleInfos` uses `getPkgModulesFilePkgName`
            // for ordinary OHPM packages. The value is the physical OHPM
            // record root (for example `pkg_modules/.ohpm/foo@1/pkg_modules/foo`),
            // not the logical package name exposed in pkgContextInfo.
            //
            // Evidence: DevEco Studio 6.24.3 ets-loader
            // `module/module_mode.js::processModuleInfos`.
            HvigorChunkSourceKind::PackageModule(package) => self
                .module_path_map
                .package_record_for_path(self.project_root, &package.package_root)
                .ok_or_else(|| {
                    ArkdownError::Bundler(format!(
                        "cannot derive hvigor packageName for module '{}' rooted at {}",
                        module_id,
                        package.package_root.display()
                    ))
                }),
            HvigorChunkSourceKind::SourceModule
            | HvigorChunkSourceKind::LocalDependencyModule(_) => {
                // Files under the current module root carry the entry module
                // name (hvigor `pkgName@<module>` for the `ohosTest` template
                // tree uses the test module name `entry_test`).
                if arkdown_path::AbsPath::canonicalize(Path::new(module_id))
                    .map(|path| path.starts_with(self.module_path_map.current_module_root()))
                    .unwrap_or(false)
                {
                    Ok(self.module_name.to_string())
                } else {
                    Ok(ownership.module_name.clone())
                }
            }
        }
    }

    fn imports_for_module(
        &self,
        module_id: &str,
        module_records: &HvigorModuleSourceRecordIndex,
    ) -> Result<(HvigorChunkImportMap, Vec<String>), ArkdownError> {
        let mut imports = HvigorChunkImportMap::default();
        let mut imported_record_names = BTreeSet::new();
        let mut seen_specifiers = FxHashSet::default();

        if let Some(resolved_imports) = self.state.resolved_imports.get(module_id) {
            for (specifier, resolved_id) in resolved_imports {
                seen_specifiers.insert(specifier.clone());
                self.record_import(
                    module_id,
                    specifier,
                    resolved_id,
                    &mut imports,
                    &mut imported_record_names,
                    module_records,
                )?;
            }
        }
        if let Some(source) = self.state.sources.get(module_id) {
            for specifier in
                HvigorModuleRequestSpecifiers::new(source.code.as_ref(), module_id).collect()
            {
                if !seen_specifiers.insert(specifier.clone()) {
                    continue;
                }
                self.record_import(
                    module_id,
                    &specifier,
                    &specifier,
                    &mut imports,
                    &mut imported_record_names,
                    module_records,
                )?;
            }
        }

        Ok((imports, imported_record_names.into_iter().collect()))
    }

    fn record_import(
        &self,
        _importer: &str,
        specifier: &str,
        resolved_id: &str,
        imports: &mut HvigorChunkImportMap,
        imported_record_names: &mut BTreeSet<String>,
        module_records: &HvigorModuleSourceRecordIndex,
    ) -> Result<(), ArkdownError> {
        if let Some(ohm_url) = self.bytecode_har_imports.ohm_url_for_request(specifier)? {
            let record_name = NormalizedOhmUrl::parse(&ohm_url)
                .and_then(|url| url.record_name())
                .ok_or_else(|| {
                    ArkdownError::Bundler(format!(
                        "bytecode HAR import '{}' produced invalid normalized OHM URL '{}'",
                        specifier, ohm_url
                    ))
                })?;
            imported_record_names.insert(record_name);
            imports.insert(specifier.to_string(), ohm_url);
            return Ok(());
        }
        let sdk_request = self.processor.process_request(specifier);
        if sdk_request != specifier {
            imports.insert(specifier.to_string(), sdk_request);
            return Ok(());
        }
        let resolved_ohm_url = self.ohm_url_for_resolved_id(resolved_id, module_records)?;
        let specifier_ohm_url = if resolved_ohm_url.is_some() {
            None
        } else {
            self.ohm_url_for_resolved_id(specifier, module_records)?
        };
        if let Some(ohm_url) = resolved_ohm_url.or(specifier_ohm_url) {
            imports.insert(specifier.to_string(), ohm_url);
        }
        if let Some(record) = Self::record_for_resolved_id(module_records, resolved_id) {
            imported_record_names.insert(record.record_name.clone());
        }
        Ok(())
    }

    fn ohm_url_for_resolved_id(
        &self,
        resolved_id: &str,
        module_records: &HvigorModuleSourceRecordIndex,
    ) -> Result<Option<String>, ArkdownError> {
        if resolved_id.starts_with("@bundle:")
            || resolved_id.starts_with("@package:")
            || resolved_id.starts_with("@shared:")
            || resolved_id.starts_with("@app:")
            || resolved_id.starts_with("@normalized:")
        {
            return Ok(Some(resolved_id.to_string()));
        }
        if resolved_id.starts_with("@ohos:")
            || resolved_id.starts_with("@hms:")
            || resolved_id.starts_with("@native:")
        {
            return Ok(Some(resolved_id.to_string()));
        }

        let path = Path::new(resolved_id);
        if !path.has_root() {
            return Ok(None);
        }
        if let Some(record) = Self::record_for_resolved_id(module_records, resolved_id) {
            return self.ohm_url_for_module_record(record).map(Some);
        }
        let ownership = self.module_path_map.source_file_for_entry(resolved_id)?;
        if ownership.module_name.is_empty() {
            return Ok(None);
        }
        if self.use_normalized_ohm_url {
            return ownership
                .to_normalized_request(self.runtime_packages, self.module_name)
                .map(Some);
        }
        let record_name = self.module_path_map.non_normalized_record_name(
            self.project_root,
            resolved_id,
            self.bundle_name,
            self.module_name,
            &ownership,
        )?;
        Ok(Some(self.legacy_ohm_url(&ownership, &record_name)))
    }

    fn ohm_url_for_module_record(
        &self,
        record: &HvigorModuleSourceRecord,
    ) -> Result<String, ArkdownError> {
        if self.use_normalized_ohm_url {
            return record
                .ownership
                .to_normalized_request(self.runtime_packages, self.module_name);
        }
        Ok(self.legacy_ohm_url(&record.ownership, &record.record_name))
    }

    fn legacy_ohm_url(&self, ownership: &HvigorChunkSource, record_name: &str) -> String {
        if record_name.starts_with('@') {
            return record_name.to_string();
        }
        match &ownership.kind {
            HvigorChunkSourceKind::PackageModule(_) => format!("@package:{record_name}"),
            HvigorChunkSourceKind::SourceModule
            | HvigorChunkSourceKind::LocalDependencyModule(_) => {
                format!("@bundle:{record_name}")
            }
        }
    }
}

struct HvigorModuleSourceImportRewriter<'a> {
    source: &'a str,
    module_id: &'a str,
    imports: &'a HvigorChunkImportMap,
    request_rewrite_kind: HvigorModuleRequestRewriteKind,
}

/// Reusable module-request rewrite inventory for a transformed source file.
///
/// The unit-test coverage plugin runs after `etsTransform` but before
/// `genAbc` in hvigor. Arkdown invokes the SDK-owned coverage transformer at
/// the equivalent boundary and then replays the exact request rewrites that
/// the initial Rolldown graph resolved for the module. Keeping this plan next
/// to [`HvigorModuleSourceImportRewriter`] ensures both paths share the same
/// ArkTS parser and literal-emission rules.
pub struct HvigorModuleRequestRewritePlan {
    module_id: String,
    imports: HvigorChunkImportMap,
}

impl HvigorModuleRequestRewritePlan {
    pub fn from_baseline(
        module_id: impl Into<String>,
        transformed_source: &str,
        rewritten_source: &str,
    ) -> Result<Self, ArkdownError> {
        let module_id = module_id.into();
        let transformed_requests =
            HvigorModuleRequestSpecifiers::new(transformed_source, &module_id).collect_literals();
        let rewritten_requests =
            HvigorModuleRequestSpecifiers::new(rewritten_source, &module_id).collect_literals();
        if transformed_requests.len() != rewritten_requests.len() {
            return Err(ArkdownError::Bundler(format!(
                "cannot capture module-request rewrites for '{module_id}': transformed source has {} requests but materialized source has {}",
                transformed_requests.len(),
                rewritten_requests.len()
            )));
        }

        let mut imports = HvigorChunkImportMap::default();
        for (transformed, rewritten) in transformed_requests.iter().zip(&rewritten_requests) {
            if transformed.specifier == rewritten.specifier {
                continue;
            }
            if let Some(previous) = imports.insert(
                transformed.specifier.clone(),
                rewritten.specifier.clone(),
            ) {
                if previous != rewritten.specifier {
                    return Err(ArkdownError::Bundler(format!(
                        "module request '{}' in '{module_id}' resolved to both '{}' and '{}'",
                        transformed.specifier, previous, rewritten.specifier
                    )));
                }
            }
        }
        Ok(Self { module_id, imports })
    }

    pub fn rewrite<'a>(&'a self, source: &'a str) -> Cow<'a, str> {
        HvigorModuleSourceImportRewriter::new(source, &self.module_id, &self.imports).rewrite()
    }
}

impl<'a> HvigorModuleSourceImportRewriter<'a> {
    fn new(source: &'a str, module_id: &'a str, imports: &'a HvigorChunkImportMap) -> Self {
        Self {
            source,
            module_id,
            imports,
            request_rewrite_kind: HvigorModuleRequestRewriteKind::from_module_id(module_id),
        }
    }

    fn rewrite(&self) -> Cow<'a, str> {
        if self.imports.is_empty() {
            return Cow::Borrowed(self.source);
        }

        let mut patches = self.collect_module_request_literal_patches();
        if patches.is_empty() {
            return Cow::Borrowed(self.source);
        }

        patches.sort_by_key(|patch| (patch.start, patch.end));
        patches.dedup_by_key(|patch| (patch.start, patch.end));

        let mut rewritten = self.source.to_string();
        let mut previous_start = self.source.len();
        for patch in patches.into_iter().rev() {
            if patch.end > previous_start || patch.start >= patch.end || patch.end > rewritten.len()
            {
                continue;
            }
            previous_start = patch.start;
            rewritten.replace_range(patch.start..patch.end, &patch.quoted_replacement());
        }
        Cow::Owned(rewritten)
    }

    fn collect_module_request_literal_patches(&self) -> Vec<HvigorModuleSourceImportPatch> {
        HvigorModuleRequestSpecifiers::new(self.source, self.module_id)
            .collect_literals()
            .into_iter()
            .filter_map(|literal| {
                let replacement = self.imports.get(literal.specifier.as_str())?;
                if replacement == literal.specifier.as_str() {
                    return None;
                }
                let start = literal.source_span.start as usize;
                let end = literal.source_span.end as usize;
                let source_literal = self.source.get(start..end)?;
                Some(HvigorModuleSourceImportPatch {
                    start,
                    end,
                    replacement: replacement.clone(),
                    quote: self.request_rewrite_kind.source_quote(source_literal),
                })
            })
            .collect()
    }
}

/// Latest ets-loader uses two different module-request paths. Runtime
/// JavaScript takes the regular-expression path and retains its delimiter,
/// while transformed `.ets`/`.ts` is rewritten through
/// `ts.factory.createStringLiteral`, which emits a generated double-quoted
/// literal. The two paths must not be collapsed into one source-preservation
/// rule: es2abc includes source text in anonymous function identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HvigorModuleRequestRewriteKind {
    JavaScript,
    TransformedTypeScript,
}

impl HvigorModuleRequestRewriteKind {
    fn from_module_id(module_id: &str) -> Self {
        if SourceClassifier::classify(module_id).is_javascript_runtime() {
            Self::JavaScript
        } else {
            Self::TransformedTypeScript
        }
    }

    fn source_quote(self, source_literal: &str) -> Option<char> {
        if self != Self::JavaScript {
            return None;
        }
        match source_literal.as_bytes() {
            [b'\'', .., b'\''] => Some('\''),
            [b'"', .., b'"'] => Some('"'),
            _ => None,
        }
    }
}

/// AST inventory of requests which hvigor's `ModuleSourceFile` may rewrite.
///
/// `processTransformedTsModuleRequest` visits ESM static imports/exports and
/// `import()` expressions. It deliberately does not rewrite CommonJS
/// `require()` calls, whose relative runtime resolution must survive in the
/// materialized module source.
struct HvigorModuleRequestSpecifiers<'a> {
    source: &'a str,
    module_id: &'a str,
}

impl<'a> HvigorModuleRequestSpecifiers<'a> {
    fn new(source: &'a str, module_id: &'a str) -> Self {
        Self { source, module_id }
    }

    fn collect(&self) -> Vec<String> {
        self.collect_literals()
            .into_iter()
            .map(|literal| literal.specifier)
            .collect()
    }

    fn collect_literals(&self) -> Vec<HvigorModuleRequestLiteral> {
        let allocator = Allocator::default();
        let source_type = CompileArktsSourceType::from_id(self.module_id);
        let parsed = Parser::new(&allocator, self.source, source_type).parse();
        if !parsed.diagnostics.is_empty() {
            return Vec::new();
        }
        let mut inventory = HvigorModuleRequestLiteralInventory::default();
        inventory.visit_program(&parsed.program);
        inventory.literals
    }
}

#[derive(Default)]
struct HvigorModuleRequestLiteralInventory {
    literals: Vec<HvigorModuleRequestLiteral>,
}

impl HvigorModuleRequestLiteralInventory {
    fn push(&mut self, literal: &StringLiteral<'_>, is_dynamic: bool) {
        self.literals.push(HvigorModuleRequestLiteral {
            specifier: literal.value.to_string(),
            source_span: HvigorSourceSpan {
                start: literal.span.start,
                end: literal.span.end,
            },
            is_dynamic,
        });
    }
}

impl<'a> Visit<'a> for HvigorModuleRequestLiteralInventory {
    fn visit_import_declaration(&mut self, declaration: &oxc_ast::ast::ImportDeclaration<'a>) {
        self.push(&declaration.source, false);
        walk::walk_import_declaration(self, declaration);
    }

    fn visit_lazy_import_declaration(
        &mut self,
        declaration: &oxc_ast::ast::LazyImportDeclaration<'a>,
    ) {
        self.push(&declaration.source, false);
        walk::walk_lazy_import_declaration(self, declaration);
    }

    fn visit_export_all_declaration(
        &mut self,
        declaration: &oxc_ast::ast::ExportAllDeclaration<'a>,
    ) {
        self.push(&declaration.source, false);
        walk::walk_export_all_declaration(self, declaration);
    }

    fn visit_export_named_declaration(
        &mut self,
        declaration: &oxc_ast::ast::ExportNamedDeclaration<'a>,
    ) {
        if let Some(source) = &declaration.source {
            self.push(source, false);
        }
        walk::walk_export_named_declaration(self, declaration);
    }

    fn visit_import_expression(&mut self, expression: &ImportExpression<'a>) {
        if let Expression::StringLiteral(source) = &expression.source {
            self.push(source, true);
        }
        walk::walk_import_expression(self, expression);
    }
}

struct HvigorModuleRequestLiteral {
    specifier: String,
    source_span: HvigorSourceSpan,
    is_dynamic: bool,
}

struct HvigorModuleSourceImportPatch {
    start: usize,
    end: usize,
    replacement: String,
    quote: Option<char>,
}

impl HvigorModuleSourceImportPatch {
    fn quoted_replacement(&self) -> String {
        let Some(quote) = self.quote else {
            return serde_json::to_string(&self.replacement)
                .expect("module request replacement must serialize as a JSON string");
        };

        // `module_source_file.js::processJsModuleRequest` retains the capture
        // from `(['"])` when it replaces a JavaScript request. Transformed
        // TypeScript deliberately leaves `quote` empty above and follows
        // `ts.factory.createStringLiteral` through the JSON double-quote path.
        // Apart from matching the source-cache text, this matters to es2abc:
        // anonymous-function names are derived from source text and a
        // different quote character changes those names even when the parsed
        // program is otherwise equal.
        let mut replacement = String::with_capacity(self.replacement.len() + 2);
        replacement.push(quote);
        for character in self.replacement.chars() {
            match character {
                '\\' => replacement.push_str("\\\\"),
                '\n' => replacement.push_str("\\n"),
                '\r' => replacement.push_str("\\r"),
                '\t' => replacement.push_str("\\t"),
                '\u{0008}' => replacement.push_str("\\b"),
                '\u{000c}' => replacement.push_str("\\f"),
                '\u{2028}' => replacement.push_str("\\u2028"),
                '\u{2029}' => replacement.push_str("\\u2029"),
                character if character == quote => {
                    replacement.push('\\');
                    replacement.push(character);
                }
                character if character.is_control() => {
                    use std::fmt::Write;

                    write!(&mut replacement, "\\u{:04x}", character as u32)
                        .expect("writing into a String cannot fail");
                }
                character => replacement.push(character),
            }
        }
        replacement.push(quote);
        replacement
    }
}

struct HvigorBytecodeHarImports<'a> {
    runtime_packages: &'a BundleRuntimePackageInfo,
    package_names: FxHashSet<&'a str>,
    use_normalized_ohm_url: bool,
}

impl<'a> HvigorBytecodeHarImports<'a> {
    fn new(
        runtime_packages: &'a BundleRuntimePackageInfo,
        package_names: &'a [String],
        use_normalized_ohm_url: bool,
    ) -> Self {
        Self {
            runtime_packages,
            package_names: package_names.iter().map(String::as_str).collect(),
            use_normalized_ohm_url,
        }
    }

    fn ohm_url_for_request(&self, request: &str) -> Result<Option<String>, ArkdownError> {
        if !self.use_normalized_ohm_url {
            return Ok(None);
        }
        if let Some(ohm_url) = self.normalized_so_ohm_url(request) {
            return Ok(Some(ohm_url));
        }
        if self.package_names.is_empty() {
            return Ok(None);
        }
        let Some(package_name) = self.alias_for_request(request) else {
            return Ok(None);
        };
        Ok(Some(self.normalized_ohm_url(package_name, request)?))
    }

    fn normalized_so_ohm_url(&self, request: &str) -> Option<String> {
        let context = self.runtime_packages.packages.get(request)?;
        if !context.is_so {
            return None;
        }
        Some(format!(
            "@normalized:Y&{}&{}&{}&{}",
            context.module_name,
            context.bundle_name,
            Path::new(request)
                .file_name()
                .and_then(|file_name| file_name.to_str())
                .unwrap_or(request),
            context.version
        ))
    }

    fn alias_for_request(&self, request: &str) -> Option<&'a str> {
        if let Some(package_name) = self.package_names.get(request).copied() {
            return Some(package_name);
        }
        self.package_names
            .iter()
            .copied()
            .filter(|package_name| {
                request
                    .strip_prefix(*package_name)
                    .is_some_and(|remaining| remaining.as_bytes().first() == Some(&b'/'))
            })
            .max_by_key(|package_name| package_name.len())
    }

    fn normalized_ohm_url(
        &self,
        package_name: &str,
        request: &str,
    ) -> Result<String, ArkdownError> {
        let context = self
            .runtime_packages
            .packages
            .get(package_name)
            .ok_or_else(|| {
                ArkdownError::Bundler(format!(
                    "pkgContextInfo is missing runtime package context for bytecode HAR '{}'",
                    package_name
                ))
            })?;
        let record = if request == package_name {
            if context.entry_path.is_empty() {
                return Err(ArkdownError::Bundler(format!(
                    "pkgContextInfo is missing bytecode HAR entryPath for '{}'",
                    package_name
                )));
            }
            self.remove_module_suffix(&format!("{package_name}/{}", context.entry_path))
        } else {
            request.replacen(package_name, &context.package_name, 1)
        };
        Ok(format!(
            "@normalized:{}&{}&{}&{}&{}",
            if context.is_so { "Y" } else { "N" },
            context.module_name,
            context.bundle_name,
            ArtifactPathSerializer::rel_to_slash(Path::new(&record)),
            context.version
        ))
    }

    fn remove_module_suffix(&self, value: &str) -> String {
        let value = ArtifactPathSerializer::rel_to_slash(Path::new(value));
        for suffix in [".d.ets", ".d.ts", ".ets", ".ts", ".mjs", ".cjs", ".js"] {
            if let Some(stripped) = value.strip_suffix(suffix) {
                return stripped.to_string();
            }
        }
        value
    }
}

struct HvigorModuleSourceId<'a> {
    module_id: &'a str,
}

impl<'a> HvigorModuleSourceId<'a> {
    fn new(module_id: &'a str) -> Self {
        Self { module_id }
    }

    fn should_compile(&self) -> bool {
        let path = Path::new(self.module_id);
        if !path.has_root() {
            return false;
        }
        if arkdown_path::SourceFileName::new(path).is_declaration() {
            return false;
        }
        matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("ets" | "ts" | "js" | "mjs" | "cjs")
        )
    }

    fn origin_source_lang(&self) -> &'static str {
        match Path::new(self.module_id)
            .extension()
            .and_then(|extension| extension.to_str())
        {
            Some("ets") => "ets",
            Some("ts") => "ts",
            _ => "js",
        }
    }
}

struct HvigorModuleSourceOutputPath<'a> {
    output_dir: &'a Path,
    project_root: &'a Path,
    ownership: &'a HvigorChunkSource,
    module_name: &'a str,
    source_file: &'a str,
    module_id: &'a str,
    emits_javascript: bool,
}

impl<'a> HvigorModuleSourceOutputPath<'a> {
    fn new(
        output_dir: &'a Path,
        project_root: &'a Path,
        ownership: &'a HvigorChunkSource,
        module_name: &'a str,
        source_file: &'a str,
        module_id: &'a str,
        emits_javascript: bool,
    ) -> Self {
        Self {
            output_dir,
            project_root,
            ownership,
            module_name,
            source_file,
            module_id,
            emits_javascript,
        }
    }

    fn path(&self) -> Result<PathBuf, ArkdownError> {
        Ok(self.output_dir.join(match &self.ownership.kind {
            HvigorChunkSourceKind::SourceModule => Path::new(self.module_name).join(
                HvigorModuleSourceFileAttribution::project_source_file_for(self.source_file)
                    .with_extension(self.output_extension()),
            ),
            HvigorChunkSourceKind::LocalDependencyModule(_) => {
                self.local_dependency_cache_source_file()
            }
            HvigorChunkSourceKind::PackageModule(_) => self.package_cache_source_file()?,
        }))
    }

    fn local_dependency_cache_source_file(&self) -> PathBuf {
        Path::new(&self.ownership.cache_module_name)
            .join(self.source_file)
            .with_extension(self.output_extension())
    }

    fn package_cache_source_file(&self) -> Result<PathBuf, ArkdownError> {
        let module_path = Path::new(self.module_id);
        let relative = module_path.strip_prefix(self.project_root).map_err(|_| {
            ArkdownError::Bundler(format!(
                "hvigor package module source is outside project root: {} not under {}",
                module_path.display(),
                self.project_root.display()
            ))
        })?;
        Ok(relative.with_extension(self.output_extension()))
    }

    fn output_extension(&self) -> &'static str {
        if self.emits_javascript {
            return "js";
        }
        match Path::new(self.module_id)
            .extension()
            .and_then(|extension| extension.to_str())
        {
            Some("ets" | "ts") => "ts",
            Some("js" | "mjs" | "cjs") => "js",
            _ => "js",
        }
    }
}

struct HvigorModuleSourceFileAttribution;

impl HvigorModuleSourceFileAttribution {
    fn project_source_file_for(source_file: &str) -> PathBuf {
        let source_file_path = Path::new(source_file).with_extension("ts");
        if source_file.starts_with("ets/")
            || source_file.starts_with("js/")
            || source_file.starts_with("mock/")
        {
            Path::new("src").join("main").join(source_file_path)
        } else {
            source_file_path
        }
    }

    fn package_source_file_for(source_file: &str) -> PathBuf {
        let path = Path::new(source_file);
        match path.extension().and_then(|extension| extension.to_str()) {
            Some("ets") => path.with_extension("ts"),
            _ => path.to_path_buf(),
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

struct RolldownInputPlan {
    module_root: PathBuf,
    module_path_map: HvigorModulePathMap,
    entries: Vec<String>,
    namer: CompileEntryNamer,
}

impl RolldownInputPlan {
    fn from_bundle_plan(plan: &BundlePlan) -> Self {
        let module_root = Self::canonical_module_root(&plan.input.module_root);
        let mut module_path_map = HvigorModulePathMap::new(
            &plan.input.module_name,
            &module_root,
            &plan.input.module_path_map,
        );
        module_path_map.add_module_record_roots(&plan.input.module_record_roots);
        module_path_map.add_module_meta_info_files(&plan.input.module_meta_info_files);
        let entries = plan.input.entries.clone();
        module_path_map.add_compile_entry_package_roots(&entries);
        Self {
            module_root,
            module_path_map,
            namer: CompileEntryNamer::new(&entries),
            entries,
        }
    }

    fn input_items(&self) -> Result<Vec<InputItem>, ArkdownError> {
        self.entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                Ok(InputItem {
                    name: Some(self.namer.chunk_name(entry, index)),
                    import: self.import_path(entry)?,
                })
            })
            .collect()
    }

    fn chunk_source_files(&self) -> Result<FxHashMap<String, ChunkSourceFile>, ArkdownError> {
        self.entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let chunk_name = self.namer.chunk_name(entry, index);
                let source = self.module_path_map.source_file_for_entry(entry)?;
                Ok((chunk_name.clone(), source.with_chunk_name(chunk_name)))
            })
            .collect()
    }

    fn entry_module_ids(&self) -> Result<Vec<String>, ArkdownError> {
        let mut entry_module_ids = Vec::with_capacity(self.entries.len());
        let mut seen = FxHashSet::default();
        for entry in &self.entries {
            let module_id = self.import_path(entry)?;
            if seen.insert(module_id.clone()) {
                entry_module_ids.push(module_id);
            }
        }
        Ok(entry_module_ids)
    }

    fn chunk_source_file_map(
        &self,
        sources: &FxHashMap<String, ChunkSourceFile>,
    ) -> FxHashMap<String, String> {
        sources
            .iter()
            .map(|(chunk_name, source)| (chunk_name.clone(), source.source_file.clone()))
            .collect()
    }

    fn import_path(&self, entry: &str) -> Result<String, ArkdownError> {
        let specifier = ModuleSpecifier::new(entry);
        if matches!(
            specifier.kind(),
            SpecifierKind::AbsolutePath | SpecifierKind::OhmUrl | SpecifierKind::ScopedPackage
        ) {
            return Ok(entry.to_string());
        }

        let candidate = self.module_root.join(strip_relative_prefix(entry));
        AbsPath::canonicalize(&candidate)
            .map(AbsPath::into_path_buf)
            .unwrap_or_else(|_| lexical_normalize(&candidate))
            .into_os_string()
            .into_string()
            .map_err(|path| {
                ArkdownError::Bundler(format!(
                    "bundle entry path is not valid UTF-8: {}",
                    path.display()
                ))
            })
    }

    fn canonical_module_root(path: &std::path::Path) -> PathBuf {
        AbsPath::canonicalize(path)
            .map(AbsPath::into_path_buf)
            .unwrap_or_else(|_| {
                if path.has_root() {
                    lexical_normalize(path)
                } else {
                    lexical_normalize(
                        &std::env::current_dir()
                            .unwrap_or_else(|_| PathBuf::from("."))
                            .join(path),
                    )
                }
            })
    }
}

#[derive(Debug, Clone)]
struct HvigorModulePathMap {
    current_module_name: String,
    current_module_root: PathBuf,
    roots: Vec<HvigorModuleRoot>,
    module_meta_info_ohm_urls: FxHashMap<PathBuf, String>,
}

impl HvigorModulePathMap {
    fn current_module_root(&self) -> &Path {
        &self.current_module_root
    }

    fn new(
        current_module_name: &str,
        module_root: &Path,
        module_path_map: &[(String, PathBuf)],
    ) -> Self {
        let mut roots = module_path_map
            .iter()
            .map(|(name, root)| {
                HvigorModuleRoot::from_module_path_map(
                    current_module_name,
                    name.clone(),
                    module_root,
                    Self::canonical_root(root),
                )
            })
            .collect::<Vec<_>>();
        // The loader module root points at `<module>/src/main`, while
        // modulePathMap owns the complete module root. Do not add a more
        // specific fallback root in that case: it cannot load oh-package.json5
        // and would shadow the owning root with the synthetic `1.0.0` version.
        let has_current_module_owner = roots.iter().any(|root| {
            root.module_name == current_module_name
                && root.kind == HvigorModuleRootKind::SourceModule
                && module_root.starts_with(&root.path)
        });
        if !has_current_module_owner {
            roots.push(HvigorModuleRoot::source_root(
                current_module_name.to_string(),
                module_root.to_path_buf(),
            ));
        }
        let current_module_root = roots
            .iter()
            .find(|root| {
                root.module_name == current_module_name
                    && root.kind == HvigorModuleRootKind::SourceModule
                    && (root.path.join("oh-package.json5").is_file()
                        || root.path.join("package.json").is_file())
            })
            .map(|root| root.path.clone())
            .or_else(|| Self::package_root_for_compile_entry(module_root))
            .unwrap_or_else(|| Self::canonical_root(module_root));
        Self::from_roots(current_module_name.to_string(), current_module_root, roots)
    }

    fn add_compile_entry_package_roots(&mut self, entries: &[String]) {
        let mut roots = std::mem::take(&mut self.roots);
        for entry in entries {
            let entry_path = Path::new(entry);
            if !entry_path.has_root() || !arkdown_path::classify::is_inside_oh_modules(entry_path) {
                continue;
            }
            let Some(root_path) = Self::package_root_for_compile_entry(entry_path) else {
                continue;
            };
            if roots.iter().any(|root| root.path == root_path) {
                continue;
            }
            let Some(root) = HvigorModuleRoot::from_package_root(root_path) else {
                continue;
            };
            roots.push(root);
        }
        self.rebuild_roots(roots);
    }

    fn add_module_record_roots(&mut self, module_record_roots: &[(String, PathBuf)]) {
        let mut roots = std::mem::take(&mut self.roots);
        for (name, root) in module_record_roots {
            roots.push(HvigorModuleRoot::from_record_root(
                name.clone(),
                Self::canonical_root(root),
            ));
        }
        self.rebuild_roots(roots);
    }

    fn add_module_meta_info_files(&mut self, files: &[crate::plan::BundleModuleMetaInfoFile]) {
        self.module_meta_info_ohm_urls.reserve(files.len());
        for file in files {
            self.module_meta_info_ohm_urls
                .insert(Self::canonical_root(&file.file_path), file.ohm_url.clone());
        }
    }

    fn module_meta_info_ohm_url(&self, module_id: &str) -> Option<&str> {
        self.module_meta_info_ohm_urls
            .get(&Self::canonical_root(Path::new(module_id)))
            .map(String::as_str)
    }

    fn rebuild_roots(&mut self, roots: Vec<HvigorModuleRoot>) {
        let current_module_name = self.current_module_name.clone();
        let current_module_root = self.current_module_root.clone();
        let module_meta_info_ohm_urls = std::mem::take(&mut self.module_meta_info_ohm_urls);
        let mut rebuilt = Self::from_roots(current_module_name, current_module_root, roots);
        rebuilt.module_meta_info_ohm_urls = module_meta_info_ohm_urls;
        *self = rebuilt;
    }

    fn from_roots(
        current_module_name: String,
        current_module_root: PathBuf,
        mut roots: Vec<HvigorModuleRoot>,
    ) -> Self {
        roots.sort_by(|left, right| {
            right
                .path
                .components()
                .count()
                .cmp(&left.path.components().count())
                .then_with(|| left.path.cmp(&right.path))
                .then_with(|| {
                    left.module_name
                        .starts_with('@')
                        .cmp(&right.module_name.starts_with('@'))
                })
                .then_with(|| left.module_name.cmp(&right.module_name))
        });
        let mut merged_roots: Vec<HvigorModuleRoot> = Vec::with_capacity(roots.len());
        for root in roots {
            if let Some(existing) = merged_roots
                .iter_mut()
                .find(|existing| existing.path == root.path)
            {
                existing.dependency_aliases.extend(root.dependency_aliases);
                existing.dependency_aliases.sort();
                existing.dependency_aliases.dedup();
                continue;
            }
            merged_roots.push(root);
        }
        Self {
            current_module_name,
            current_module_root,
            roots: merged_roots,
            module_meta_info_ohm_urls: FxHashMap::default(),
        }
    }

    fn package_root_for_compile_entry(entry_path: &Path) -> Option<PathBuf> {
        let mut current = if entry_path.is_dir() {
            Some(entry_path)
        } else {
            entry_path.parent()
        };
        while let Some(path) = current {
            if path.join("oh-package.json5").is_file() || path.join("package.json").is_file() {
                return Some(Self::canonical_root(path));
            }
            current = path.parent();
        }
        None
    }

    fn source_file_for_entry(&self, entry: &str) -> Result<HvigorChunkSource, ArkdownError> {
        let entry_path = Path::new(entry);
        if !entry_path.has_root() {
            return Ok(HvigorChunkSource::source_module(
                self.current_module_name.clone(),
                self.current_module_name.clone(),
                "1.0.0".to_string(),
                strip_relative_prefix(entry).to_string(),
            ));
        }

        let entry_path = Self::canonical_root(entry_path);
        for root in &self.roots {
            if let Ok(relative) = entry_path.strip_prefix(&root.path) {
                return root.source_for_relative(&entry_path, relative);
            }
        }
        Ok(HvigorChunkSource::source_module(
            String::new(),
            String::new(),
            "1.0.0".to_string(),
            HvigorModuleRelativeSourceFile::new(&entry_path).source_file(),
        ))
    }

    fn has_dependency_package_info(&self, module_id: &str) -> bool {
        let module_path = Self::canonical_root(Path::new(module_id));
        // `module-meta-info-plugin.js::addModuleInfoData` only attaches
        // dependencyPkgInfo when the module is outside projectConfig.modulePath.
        // A remote Source HAR installed below `<module>/oh_modules` therefore
        // keeps entry-package attribution in source-map keys even though its
        // ModuleInfo packageName is the HAR package name.
        !module_path.starts_with(&self.current_module_root)
    }

    fn current_package_identity(&self) -> Option<(&str, &str)> {
        self.roots
            .iter()
            .find(|root| {
                root.kind == HvigorModuleRootKind::SourceModule
                    && root.path == self.current_module_root
            })
            .or_else(|| {
                self.roots.iter().find(|root| {
                    root.kind == HvigorModuleRootKind::SourceModule
                        && root.module_name == self.current_module_name
                })
            })
            .map(|root| (root.package_name.as_str(), root.package_version.as_str()))
    }

    /// Source-map ownership for project-model source HARs is derived from
    /// `module-meta-info-plugin.js::addModuleInfoData`, not from their module
    /// record URL. Keep this list in the same most-specific-first root order
    /// used by ModuleMode source attribution.
    fn source_map_local_dependency_roots(&self) -> Vec<output::SourceMapLocalDependencyRoot> {
        self.roots
            .iter()
            .filter(|root| root.kind == HvigorModuleRootKind::LocalDependencyModule)
            .map(|root| output::SourceMapLocalDependencyRoot {
                root: root.path.clone(),
                package_name: root.package_name.clone(),
                package_version: root.package_version.clone(),
            })
            .collect()
    }

    fn canonical_root(path: &Path) -> PathBuf {
        AbsPath::canonicalize(path)
            .map(AbsPath::into_path_buf)
            .unwrap_or_else(|_| lexical_normalize(path))
    }

    /// Mirrors `ark_utils.getOhmUrlByFilepath` for ModuleMode builds where
    /// `useNormalizedOHMUrl` is disabled.
    fn non_normalized_record_name(
        &self,
        project_root: &Path,
        module_id: &str,
        bundle_name: &str,
        entry_module_name: &str,
        source: &HvigorChunkSource,
    ) -> Result<String, ArkdownError> {
        match &source.kind {
            HvigorChunkSourceKind::PackageModule(_) => {
                let module_path = Path::new(module_id);
                let record = self
                    .package_record_for_path(project_root, module_path)
                    .ok_or_else(|| {
                        ArkdownError::Bundler(format!(
                            "cannot derive hvigor package record for '{}'",
                            module_path.display()
                        ))
                    })?;
                Ok(ArtifactPathSerializer::rel_to_slash(
                    &Path::new(&record).with_extension(""),
                ))
            }
            HvigorChunkSourceKind::SourceModule
            | HvigorChunkSourceKind::LocalDependencyModule(_) => {
                let source_file =
                    HvigorModuleRelativeSourceFile::new(Path::new(source.source_file.as_str()))
                        .source_file();
                let source_file = ArtifactPathSerializer::rel_to_slash(
                    &Path::new(&source_file).with_extension(""),
                );
                // Files under the current module root belong to the entry
                // module even when the source ownership resolved them to a
                // configured module name (the `ohosTest` template tree lives
                // under `<module>/.test/...` but carries the test module name
                // `entry_test`; hvigor `getBelongModuleInfo` keys by the
                // modulePathMap entry).
                let belongs_to_current = source.module_name == entry_module_name
                    || Self::canonical_root(Path::new(module_id)).starts_with(
                        &self.current_module_root,
                    );
                let module_segment = if belongs_to_current {
                    entry_module_name.to_string()
                } else {
                    format!("{entry_module_name}@{}", source.module_name)
                };
                Ok(format!("{bundle_name}/{module_segment}/{source_file}"))
            }
        }
    }

    fn npm_entries(
        &self,
        project_root: &Path,
        bundle_name: &str,
        entry_module_ids: &[String],
        module_source_files: &[HvigorModuleSourceFile],
    ) -> Result<Vec<HvigorNpmEntry>, ArkdownError> {
        let active_records = module_source_files
            .iter()
            .map(|source| source.record_name.as_str())
            .collect::<FxHashSet<_>>();
        let mut entries = Vec::new();
        let mut active_package_roots = FxHashSet::default();
        let compile_entry_paths = entry_module_ids
            .iter()
            .map(|entry| Self::canonical_root(Path::new(entry)))
            .collect::<FxHashSet<_>>();

        for root in self
            .roots
            .iter()
            .filter(|root| root.kind == HvigorModuleRootKind::PackageModule)
        {
            let Some((entry_path, build_path)) = self.package_root_entry(project_root, root)?
            else {
                continue;
            };
            if !active_records.contains(build_path.as_str()) {
                continue;
            }
            active_package_roots.insert(entry_path.clone());
            entries.push(HvigorNpmEntry {
                entry_path,
                build_path,
            });
        }

        self.add_package_symlink_entries(project_root, &active_package_roots, &mut entries)?;
        self.add_local_dependency_entries(
            bundle_name,
            &compile_entry_paths,
            &active_records,
            &mut entries,
        )?;

        // `ModuleMode.generateNpmEntriesInfo` preserves Map insertion order.
        // Keep the graph/root traversal order and only collapse exact duplicate
        // mappings, instead of sorting an order-sensitive npm entry manifest.
        let mut seen_entries = FxHashSet::default();
        entries.retain(|entry| {
            seen_entries.insert((entry.entry_path.clone(), entry.build_path.clone()))
        });
        Ok(entries)
    }

    fn package_root_entry(
        &self,
        project_root: &Path,
        root: &HvigorModuleRoot,
    ) -> Result<Option<(String, String)>, ArkdownError> {
        let Some(entry_path) = self.package_record_for_path(project_root, &root.path) else {
            return Ok(None);
        };
        let entry_rel = OhPackage::load_from_root(&root.path)
            .and_then(|package| {
                package
                    .exports_import_field()
                    .or_else(|| package.main.clone())
            })
            .unwrap_or_else(|| "index".to_string());
        let entry_rel = strip_relative_prefix(&entry_rel);
        // In legacy mode `ModuleMode.getPackageEntryInfo` writes the physical
        // `pkg_modules` path for both sides of a package entry mapping. It does
        // not use the normalized pkgContextInfo record that is valid only when
        // `useNormalizedOHMUrl` is enabled.
        let build_path = ArtifactPathSerializer::rel_to_slash(
            &Path::new(&entry_path).join(entry_rel).with_extension(""),
        );
        Ok(Some((entry_path, build_path)))
    }

    fn add_package_symlink_entries(
        &self,
        project_root: &Path,
        active_package_roots: &FxHashSet<String>,
        entries: &mut Vec<HvigorNpmEntry>,
    ) -> Result<(), ArkdownError> {
        let mut canonical_package_roots = FxHashMap::default();
        for root in self
            .roots
            .iter()
            .filter(|root| root.kind == HvigorModuleRootKind::PackageModule)
        {
            let Some(record) = self.package_record_for_path(project_root, &root.path) else {
                continue;
            };
            if !active_package_roots.contains(record.as_str()) {
                continue;
            }
            canonical_package_roots.insert(Self::canonical_root(&root.path), record);
        }

        for oh_modules_root in self.oh_modules_scan_roots(project_root) {
            self.scan_hvigor_oh_modules_dir(
                project_root,
                &oh_modules_root,
                &canonical_package_roots,
                entries,
            )?;
            let store_root = oh_modules_root.join(".ohpm");
            let Ok(store_entries) = fs_err::read_dir(&store_root) else {
                continue;
            };
            for store_entry in store_entries {
                let store_entry = store_entry?;
                let package_oh_modules = store_entry.path().join("oh_modules");
                self.scan_hvigor_oh_modules_dir(
                    project_root,
                    &package_oh_modules,
                    &canonical_package_roots,
                    entries,
                )?;
            }
        }
        Ok(())
    }

    /// Mirrors `resolve-ohmodules.js::resolveProjectOhModules`. Hvigor records
    /// only direct OHPM links from each project/module `oh_modules` directory,
    /// followed by the direct links in every immediate `.ohpm/*/oh_modules`
    /// directory. It deliberately does not recursively walk package trees.
    fn scan_hvigor_oh_modules_dir(
        &self,
        project_root: &Path,
        root: &Path,
        canonical_package_roots: &FxHashMap<PathBuf, String>,
        entries: &mut Vec<HvigorNpmEntry>,
    ) -> Result<(), ArkdownError> {
        let Ok(read_dir) = fs_err::read_dir(root) else {
            return Ok(());
        };
        for entry in read_dir {
            let entry = entry?;
            let entry_path = entry.path();
            if !fs_err::metadata(&entry_path)
                .map(|metadata| metadata.is_dir())
                .unwrap_or(false)
            {
                continue;
            }
            let entry_name = entry.file_name();
            let entry_name = entry_name.to_string_lossy();
            if entry_name == ".ohpm" {
                continue;
            }
            if entry_name.starts_with('@') {
                let Ok(scoped_entries) = fs_err::read_dir(&entry_path) else {
                    continue;
                };
                for scoped_entry in scoped_entries {
                    let scoped_path = scoped_entry?.path();
                    if !fs_err::metadata(&scoped_path)
                        .map(|metadata| metadata.is_dir())
                        .unwrap_or(false)
                    {
                        continue;
                    }
                    self.record_hvigor_symlink_entry(
                        project_root,
                        &scoped_path,
                        canonical_package_roots,
                        entries,
                    );
                }
                continue;
            }
            self.record_hvigor_symlink_entry(
                project_root,
                &entry_path,
                canonical_package_roots,
                entries,
            );
        }
        Ok(())
    }

    fn record_hvigor_symlink_entry(
        &self,
        project_root: &Path,
        link_path: &Path,
        canonical_package_roots: &FxHashMap<PathBuf, String>,
        entries: &mut Vec<HvigorNpmEntry>,
    ) {
        let canonical_link = Self::canonical_root(link_path);
        if canonical_link == lexical_normalize(link_path) {
            return;
        }
        let Some(build_path) = canonical_package_roots.get(&canonical_link) else {
            return;
        };
        let Some(entry_path) = self.package_record_for_path(project_root, link_path) else {
            return;
        };
        entries.push(HvigorNpmEntry {
            entry_path,
            build_path: build_path.clone(),
        });
    }

    fn add_local_dependency_entries(
        &self,
        bundle_name: &str,
        compile_entry_paths: &FxHashSet<PathBuf>,
        active_records: &FxHashSet<&str>,
        entries: &mut Vec<HvigorNpmEntry>,
    ) -> Result<(), ArkdownError> {
        let current_root = self
            .roots
            .iter()
            .find(|root| {
                root.kind == HvigorModuleRootKind::SourceModule
                    && root.package_name == self.current_module_name
            })
            .or_else(|| {
                self.roots
                    .iter()
                    .find(|root| root.kind == HvigorModuleRootKind::SourceModule)
            });
        let Some(current_root) = current_root else {
            return Ok(());
        };
        let current_package = OhPackage::load_from_root(&current_root.path);
        let dependency_aliases = current_package
            .as_ref()
            .map(|package| package.file_dependencies())
            .unwrap_or_default();

        for root in self
            .roots
            .iter()
            .filter(|root| root.kind == HvigorModuleRootKind::LocalDependencyModule)
            .filter(|root| root.path != current_root.path)
        {
            let Some(alias) = dependency_aliases
                .iter()
                .find_map(|(alias, path)| {
                    let canonical = Self::canonical_root(path);
                    if canonical == root.path {
                        return Some(alias.as_str());
                    }
                    OhPackage::load_from_root(&canonical).and_then(|package| {
                        (package.name() == Some(root.package_name.as_str()))
                            .then_some(alias.as_str())
                    })
                })
                .or_else(|| root.dependency_aliases.first().map(String::as_str))
            else {
                continue;
            };
            let entry_source = OhPackage::load_from_root(&root.path)
                .and_then(|package| package.main_field().map(str::to_string))
                .filter(|main| !main.is_empty())
                .unwrap_or_else(|| "Index.ets".to_string());
            // `ModuleMode.getPackageEntryInfo` only receives a local package
            // entry when the original module metadata marks it as a node
            // entry. In the compose pipeline that corresponds to the
            // compile-entry inventory, not every active local HAR module.
            // HmosWorld, for example, compiles `conference/Index.ets` through
            // graph traversal but does not add a bare `@ohos/conference`
            // npm-entry mapping.
            let source_entry = Self::canonical_root(&root.path.join(&entry_source));
            if !compile_entry_paths.contains(&source_entry) {
                continue;
            }
            let build_record = self.local_dependency_record_name(bundle_name, root, &entry_source);
            if !active_records.contains(build_record.as_str()) {
                continue;
            }
            entries.push(HvigorNpmEntry {
                entry_path: format!("pkg_modules@{}/{}", self.current_module_name, alias),
                build_path: build_record.clone(),
            });
            entries.push(HvigorNpmEntry {
                entry_path: alias.to_string(),
                build_path: format!("@bundle.{build_record}"),
            });
        }
        Ok(())
    }

    fn local_dependency_record_name(
        &self,
        bundle_name: &str,
        root: &HvigorModuleRoot,
        entry_source: &str,
    ) -> String {
        let source_file =
            HvigorModuleRelativeSourceFile::new(Path::new(entry_source)).source_file();
        let source_file =
            ArtifactPathSerializer::rel_to_slash(&Path::new(&source_file).with_extension(""));
        let module_segment = if root.module_name == self.current_module_name {
            self.current_module_name.clone()
        } else {
            format!("{}@{}", self.current_module_name, root.module_name)
        };
        format!("{bundle_name}/{module_segment}/{source_file}")
    }

    fn oh_modules_scan_roots(&self, project_root: &Path) -> Vec<PathBuf> {
        let mut roots = vec![Self::canonical_root(project_root).join("oh_modules")];
        roots.extend(
            self.roots
                .iter()
                .filter(|root| root.kind.is_project_oh_modules_owner())
                .map(|root| root.path.join("oh_modules")),
        );
        roots.sort();
        roots.dedup();
        roots
    }

    fn package_record_for_path(&self, project_root: &Path, path: &Path) -> Option<String> {
        let project_oh_modules = Self::canonical_root(project_root).join("oh_modules");
        if let Some(relative) = Self::relative_to_root_preserving_links(path, &project_oh_modules) {
            if relative.starts_with(Path::new(".ohpm").join("oh_modules")) {
                return None;
            }
            return Some(Self::pkg_modules_record("pkg_modules", &relative));
        }

        for root in self
            .roots
            .iter()
            .filter(|root| root.kind.is_project_oh_modules_owner())
        {
            let oh_modules = root.path.join("oh_modules");
            if let Some(relative) = Self::relative_to_root_preserving_links(path, &oh_modules) {
                return Some(Self::pkg_modules_record(
                    &format!("pkg_modules@{}", root.module_name),
                    &relative,
                ));
            }
        }
        None
    }

    fn relative_to_root_preserving_links(path: &Path, root: &Path) -> Option<PathBuf> {
        let lexical_path = lexical_normalize(path);
        let lexical_root = lexical_normalize(root);
        if let Ok(relative) = lexical_path.strip_prefix(&lexical_root) {
            return Some(relative.to_path_buf());
        }

        // `/var` -> `/private/var` on macOS is a common project-root alias.
        // Resolve that alias only for non-link terminal paths; resolving a
        // package link here would erase the OHPM alias that ModuleMode stores
        // in npmEntries.
        let is_terminal_link = fs_err::symlink_metadata(path)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false);
        if is_terminal_link {
            return None;
        }
        let canonical_path = Self::canonical_root(path);
        let canonical_root = Self::canonical_root(root);
        canonical_path
            .strip_prefix(canonical_root)
            .ok()
            .map(Path::to_path_buf)
    }

    fn pkg_modules_record(prefix: &str, relative: &Path) -> String {
        let mut path = PathBuf::from(prefix);
        for component in relative.components() {
            let value = component.as_os_str();
            if value == "oh_modules" {
                path.push("pkg_modules");
            } else {
                path.push(value);
            }
        }
        ArtifactPathSerializer::rel_to_slash(&path)
    }
}

#[derive(Debug, Clone)]
struct HvigorModuleRoot {
    module_name: String,
    package_name: String,
    package_version: String,
    dependency_aliases: Vec<String>,
    path: PathBuf,
    kind: HvigorModuleRootKind,
}

impl HvigorModuleRoot {
    fn source_root(module_name: String, path: PathBuf) -> Self {
        let package = OhPackage::load_from_root(&path);
        Self::with_package(
            module_name,
            path,
            HvigorModuleRootKind::SourceModule,
            package,
        )
    }

    fn from_module_path_map(
        current_module_name: &str,
        module_name: String,
        current_module_root: &Path,
        path: PathBuf,
    ) -> Self {
        let kind = HvigorModuleRootKind::from_root_path(&path);
        let package = OhPackage::load_from_root(&path);
        let kind = if kind == HvigorModuleRootKind::SourceModule
            && module_name != current_module_name
            && path != current_module_root
            && package.is_some()
        {
            HvigorModuleRootKind::LocalDependencyModule
        } else {
            kind
        };
        Self::with_package(module_name, path, kind, package)
    }

    fn from_package_root(path: PathBuf) -> Option<Self> {
        let package = OhPackage::load_from_root(&path)?;
        let module_name = package.name().map(str::to_string)?;
        let kind = HvigorModuleRootKind::from_root_path(&path);
        Some(Self::with_package(module_name, path, kind, Some(package)))
    }

    fn from_record_root(module_name: String, path: PathBuf) -> Self {
        let package = OhPackage::load_from_root(&path);
        Self::with_package(
            module_name,
            path,
            HvigorModuleRootKind::PackageModule,
            package,
        )
    }

    fn with_package(
        module_name: String,
        path: PathBuf,
        kind: HvigorModuleRootKind,
        package: Option<OhPackage>,
    ) -> Self {
        let package_name = package
            .as_ref()
            .and_then(|package| package.name())
            .unwrap_or(module_name.as_str())
            .to_string();
        let package_version = package
            .as_ref()
            .map(|package| package.version())
            .filter(|version| !version.is_empty())
            .unwrap_or("1.0.0")
            .to_string();
        Self {
            dependency_aliases: module_name
                .starts_with('@')
                .then(|| module_name.clone())
                .into_iter()
                .collect(),
            module_name,
            package_name,
            package_version,
            path,
            kind,
        }
    }

    fn source_for_relative(
        &self,
        absolute_path: &Path,
        relative: &Path,
    ) -> Result<HvigorChunkSource, ArkdownError> {
        match self.kind {
            HvigorModuleRootKind::SourceModule => Ok(HvigorChunkSource::source_module(
                self.package_name.clone(),
                self.module_name.clone(),
                self.package_version.clone(),
                HvigorModuleRelativeSourceFile::new(relative).source_file(),
            )),
            HvigorModuleRootKind::PackageModule => {
                let package =
                    HvigorPackageModuleSource::new(absolute_path, &self.path, &self.package_name)?;
                Ok(HvigorChunkSource::package_module(
                    self.package_name.clone(),
                    self.module_name.clone(),
                    self.package_version.clone(),
                    ArtifactPathSerializer::rel_to_slash(relative),
                    package,
                ))
            }
            HvigorModuleRootKind::LocalDependencyModule => {
                let package =
                    HvigorPackageModuleSource::new(absolute_path, &self.path, &self.package_name)?;
                Ok(HvigorChunkSource::local_dependency_module(
                    self.package_name.clone(),
                    self.module_name.clone(),
                    self.package_version.clone(),
                    ArtifactPathSerializer::rel_to_slash(relative),
                    package,
                ))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HvigorModuleRootKind {
    SourceModule,
    LocalDependencyModule,
    PackageModule,
}

impl HvigorModuleRootKind {
    fn from_root_path(path: &Path) -> Self {
        if arkdown_path::classify::is_inside_oh_modules(path) {
            Self::PackageModule
        } else {
            Self::SourceModule
        }
    }

    fn is_project_oh_modules_owner(self) -> bool {
        matches!(self, Self::SourceModule | Self::LocalDependencyModule)
    }
}

struct HvigorChunkSource {
    module_name: String,
    cache_module_name: String,
    package_version: String,
    source_file: String,
    kind: HvigorChunkSourceKind,
}

enum HvigorChunkSourceKind {
    SourceModule,
    LocalDependencyModule(HvigorPackageModuleSource),
    PackageModule(HvigorPackageModuleSource),
}

impl HvigorChunkSource {
    fn source_module(
        module_name: String,
        cache_module_name: String,
        package_version: String,
        source_file: String,
    ) -> Self {
        Self {
            cache_module_name,
            module_name,
            package_version,
            source_file,
            kind: HvigorChunkSourceKind::SourceModule,
        }
    }

    fn package_module(
        module_name: String,
        cache_module_name: String,
        package_version: String,
        source_file: String,
        package: HvigorPackageModuleSource,
    ) -> Self {
        Self {
            cache_module_name,
            module_name,
            package_version,
            source_file,
            kind: HvigorChunkSourceKind::PackageModule(package),
        }
    }

    fn local_dependency_module(
        module_name: String,
        cache_module_name: String,
        package_version: String,
        source_file: String,
        package: HvigorPackageModuleSource,
    ) -> Self {
        Self {
            cache_module_name,
            module_name,
            package_version,
            source_file,
            kind: HvigorChunkSourceKind::LocalDependencyModule(package),
        }
    }

    fn with_chunk_name(self, chunk_name: String) -> ChunkSourceFile {
        ChunkSourceFile {
            chunk_name,
            module_name: self.module_name,
            source_file: self.source_file,
        }
    }

    fn normalized_record_name(
        &self,
        runtime_packages: &BundleRuntimePackageInfo,
    ) -> Result<String, ArkdownError> {
        match &self.kind {
            HvigorChunkSourceKind::SourceModule => {
                let context = self.runtime_context(runtime_packages)?;
                let project_file_path = self.source_project_file_path();
                Ok(HvigorNormalizedOhmUrl::new(context, &project_file_path).to_record())
            }
            HvigorChunkSourceKind::PackageModule(package) => {
                package.to_normalized_record(runtime_packages)
            }
            HvigorChunkSourceKind::LocalDependencyModule(package) => {
                package.to_normalized_record(runtime_packages)
            }
        }
    }

    fn to_normalized_request(
        &self,
        runtime_packages: &BundleRuntimePackageInfo,
        _entry_module_name: &str,
    ) -> Result<String, ArkdownError> {
        match &self.kind {
            HvigorChunkSourceKind::SourceModule => {
                let context = self.runtime_context(runtime_packages)?;
                let project_file_path = self.source_project_file_path();
                Ok(HvigorNormalizedOhmUrl::new(context, &project_file_path).to_request())
            }
            HvigorChunkSourceKind::PackageModule(package)
            | HvigorChunkSourceKind::LocalDependencyModule(package) => {
                package.to_normalized_request(runtime_packages)
            }
        }
    }

    fn runtime_context<'a>(
        &self,
        runtime_packages: &'a BundleRuntimePackageInfo,
    ) -> Result<&'a BundleRuntimePackageContext, ArkdownError> {
        runtime_packages
            .packages
            .get(self.module_name.as_str())
            .ok_or_else(|| {
                ArkdownError::Bundler(format!(
                    "pkgContextInfo is missing runtime package context for '{}'",
                    self.module_name
                ))
            })
    }

    fn source_project_file_path(&self) -> String {
        ArtifactPathSerializer::rel_to_slash(
            &HvigorModuleSourceFileAttribution::project_source_file_for(&self.source_file),
        )
    }

    fn file_info_source_file(
        &self,
        entry_package_name: &str,
        entry_package_version: &str,
        has_dependency_package_info: bool,
        emits_javascript: bool,
    ) -> String {
        let mut source_file = match &self.kind {
            HvigorChunkSourceKind::SourceModule => {
                HvigorModuleSourceFileAttribution::project_source_file_for(&self.source_file)
            }
            HvigorChunkSourceKind::PackageModule(_) => {
                HvigorModuleSourceFileAttribution::package_source_file_for(&self.source_file)
            }
            HvigorChunkSourceKind::LocalDependencyModule(_) => {
                HvigorModuleSourceFileAttribution::package_source_file_for(&self.source_file)
            }
        };
        // SDK ModuleMode.processModuleInfos always writes `.mjs` and `.cjs`
        // modules to a `.js` cache path. addModuleInfoItem then derives the
        // filesInfo source attribution from that rewritten cache path.
        let module_mode_rewrites_extension = matches!(
            Path::new(&self.source_file)
                .extension()
                .and_then(|extension| extension.to_str()),
            Some("mjs" | "cjs")
        );
        if emits_javascript || module_mode_rewrites_extension {
            source_file.set_extension("js");
        }
        let source_file = ArtifactPathSerializer::rel_to_slash(&source_file);
        let (package_name, package_version) = if has_dependency_package_info {
            (self.module_name.as_str(), self.package_version.as_str())
        } else {
            (entry_package_name, entry_package_version)
        };
        format!(
            "{}|{}|{}|{}",
            entry_package_name, package_name, package_version, source_file
        )
    }
}

struct HvigorPackageModuleSource {
    package_name: String,
    package_root: PathBuf,
    project_file_path: String,
}

impl HvigorPackageModuleSource {
    fn new(
        absolute_path: &Path,
        package_root: &Path,
        package_name: &str,
    ) -> Result<Self, ArkdownError> {
        let project_file_path = absolute_path.strip_prefix(package_root).map_err(|error| {
            ArkdownError::Bundler(format!(
                "hvigor package module source is outside package root: {} not under {}: {}",
                absolute_path.display(),
                package_root.display(),
                error
            ))
        })?;
        let project_file_path = ArtifactPathSerializer::rel_to_slash(project_file_path);
        let package = OhPackage::load_from_root(package_root).ok_or_else(|| {
            ArkdownError::Bundler(format!(
                "hvigor package root for {package_name} does not contain package metadata: {}",
                package_root.display()
            ))
        })?;
        if package.version().is_empty() {
            return Err(ArkdownError::Bundler(format!(
                "hvigor package root for {package_name} does not contain package version metadata: {}",
                package_root.display()
            )));
        }
        Ok(Self {
            package_name: package_name.to_string(),
            package_root: package_root.to_path_buf(),
            project_file_path,
        })
    }

    fn normalized_project_file_path<'a>(
        &'a self,
        _context: &BundleRuntimePackageContext,
    ) -> Cow<'a, str> {
        Cow::Borrowed(&self.project_file_path)
    }

    fn to_normalized_record(
        &self,
        runtime_packages: &BundleRuntimePackageInfo,
    ) -> Result<String, ArkdownError> {
        let context = self.runtime_context(runtime_packages)?;
        let project_file_path = self.normalized_project_file_path(context);
        Ok(HvigorNormalizedOhmUrl::new(context, project_file_path.as_ref()).to_record())
    }

    fn to_normalized_request(
        &self,
        runtime_packages: &BundleRuntimePackageInfo,
    ) -> Result<String, ArkdownError> {
        let context = self.runtime_context(runtime_packages)?;
        let project_file_path = self.normalized_project_file_path(context);
        Ok(HvigorNormalizedOhmUrl::new(context, project_file_path.as_ref()).to_request())
    }

    fn runtime_context<'a>(
        &self,
        runtime_packages: &'a BundleRuntimePackageInfo,
    ) -> Result<&'a BundleRuntimePackageContext, ArkdownError> {
        runtime_packages
            .packages
            .get(self.package_name.as_str())
            .ok_or_else(|| {
                ArkdownError::Bundler(format!(
                    "pkgContextInfo is missing runtime package context for '{}'",
                    self.package_name
                ))
            })
    }
}

struct HvigorModuleRelativeSourceFile<'a> {
    relative: &'a Path,
}

impl<'a> HvigorModuleRelativeSourceFile<'a> {
    fn new(relative: &'a Path) -> Self {
        Self { relative }
    }

    fn source_file(&self) -> String {
        let source = self
            .after_src_main()
            .unwrap_or_else(|| self.relative.to_path_buf());
        ArtifactPathSerializer::rel_to_slash(&source)
    }

    fn after_src_main(&self) -> Option<PathBuf> {
        // hvigor `ark_utils.getOhmUrlByFilepath` matches
        // `/src/(?:main|ohosTest)/(ets|js|mock)/` and maps the remainder to
        // `ets/...` — the `ohosTest` template tree
        // (`.test/<product>/intermediates/src/ohosTest/ets/...`) hits the
        // `src/ohosTest` arm.
        let components = self.relative.components().collect::<Vec<_>>();
        let src_main_index = components.windows(2).position(|window| {
            matches!(window[0], std::path::Component::Normal(component) if component == "src")
                && matches!(
                    window[1],
                    std::path::Component::Normal(component)
                        if component == "main" || component == "ohosTest"
                )
        })?;
        let mut path = PathBuf::new();
        for component in components.into_iter().skip(src_main_index + 2) {
            path.push(component.as_os_str());
        }
        (!path.as_os_str().is_empty()).then_some(path)
    }
}

#[derive(Debug, Clone, Default)]
struct ModuleGraphFacts {
    full_module_ids: Vec<String>,
    full_module_is_commonjs: FxHashMap<String, bool>,
}

impl ModuleGraphFacts {
    fn chunk_graph(&self) -> ChunkGraph {
        ChunkGraph {
            chunks: IndexVec::new(),
            full_module_ids: self.full_module_ids.clone(),
            full_module_is_commonjs: self.full_module_is_commonjs.clone(),
            module_to_chunk: IndexVec::new(),
            entry_module_to_entry_chunk: FxHashMap::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BundleCachePolicy, BundleInput, BundleOutputOptions, BundlePluginSet, LoaderResourceJoin,
        SourceMapPolicy,
    };
    use arkdown_core::BuildMode;

    #[test]
    fn transform_collector_excludes_rollup_virtual_runtime_source_maps() {
        assert!(
            HvigorModuleTransformCollector::is_runtime_javascript_source(
                "/project/oh_modules/runtime/index.js"
            )
        );
        assert!(
            !HvigorModuleTransformCollector::is_runtime_javascript_source("\0rolldown/runtime.js")
        );
    }

    #[test]
    fn transform_collector_restores_sdk_javascript_mapping_line_boundaries() {
        let restored = HvigorModuleTransformCollector::restore_hvigor_javascript_mapping_lines(
            r#"{"version":3,"sources":["ResourceTable.ts"],"names":[],"mappings":""}"#.to_string(),
            true,
        );
        let value: serde_json::Value =
            serde_json::from_str(&restored).expect("restored source map JSON");
        assert_eq!(value["mappings"], ";");

        let populated = HvigorModuleTransformCollector::restore_hvigor_javascript_mapping_lines(
            r#"{"version":3,"sources":["Index.ts"],"names":[],"mappings":"AAAA"}"#.to_string(),
            true,
        );
        let value: serde_json::Value =
            serde_json::from_str(&populated).expect("populated source map JSON");
        assert_eq!(value["mappings"], "AAAA");

        let emitted = HvigorModuleTransformCollector::restore_hvigor_javascript_mapping_lines(
            r#"{"version":3,"sources":["ModuleInfo.ts"],"names":[],"mappings":"AAAA"}"#.to_string(),
            false,
        );
        let value: serde_json::Value =
            serde_json::from_str(&emitted).expect("emitted source map JSON");
        assert_eq!(value["mappings"], "AAAA;");

        let emitted_with_boundary =
            HvigorModuleTransformCollector::restore_hvigor_javascript_mapping_lines(
                r#"{"version":3,"sources":["ModuleInfo.ts"],"names":[],"mappings":"AAAA;"}"#
                    .to_string(),
                false,
            );
        let value: serde_json::Value =
            serde_json::from_str(&emitted_with_boundary).expect("source map JSON with boundary");
        assert_eq!(value["mappings"], "AAAA;");
    }

    fn runtime_packages(
        contexts: &[(&str, &str, &str, &str, &str, bool)],
    ) -> BundleRuntimePackageInfo {
        let mut packages = FxHashMap::default();
        for (key, package_name, bundle_name, module_name, version, is_so) in contexts {
            packages.insert(
                (*key).to_string(),
                BundleRuntimePackageContext {
                    package_name: (*package_name).to_string(),
                    bundle_name: (*bundle_name).to_string(),
                    module_name: (*module_name).to_string(),
                    version: (*version).to_string(),
                    entry_path: if *key == "phone" || *key == "audioplayer" {
                        "src/main/".to_string()
                    } else {
                        "index".to_string()
                    },
                    is_so: *is_so,
                },
            );
        }
        BundleRuntimePackageInfo { packages }
    }

    fn record_resolved_import(
        state: &mut HvigorModuleSourceState,
        importer: &str,
        specifier: &str,
        resolved_id: &str,
    ) {
        state
            .resolved_imports
            .entry(importer.to_string())
            .or_default()
            .insert(specifier.to_string(), resolved_id.to_string());
        if let Some(source_span) = state.sources.get(importer).and_then(|source| {
            HvigorSourceSpan::for_module_request_literal(source.code.as_ref(), importer, specifier)
        }) {
            state
                .resolved_import_records
                .entry(importer.to_string())
                .or_default()
                .push(HvigorResolvedImportRecord {
                    specifier: specifier.to_string(),
                    source_span,
                });
        }
    }

    #[test]
    fn non_normalized_source_record_matches_hvigor_get_ohm_url_by_filepath() {
        let project = tempfile::tempdir().unwrap();
        let module_root = project.path().join("entry");
        let source_path = module_root.join("src/main/ets/pages/Index.ets");
        std::fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        std::fs::write(&source_path, "").unwrap();

        let module_root = arkdown_path::canonicalize_or_owned(&module_root);
        let source_path = arkdown_path::canonicalize_or_owned(&source_path);
        let module_path_map = HvigorModulePathMap::new("entry", &module_root, &[]);
        let source = module_path_map
            .source_file_for_entry(source_path.to_str().unwrap())
            .unwrap();
        let record = module_path_map
            .non_normalized_record_name(
                project.path(),
                source_path.to_str().unwrap(),
                "com.example.arkdown.test",
                "entry",
                &source,
            )
            .unwrap();

        assert_eq!(record, "com.example.arkdown.test/entry/ets/pages/Index");
    }

    #[test]
    fn module_meta_info_ohm_url_survives_compile_entry_root_enrichment() {
        let project = tempfile::tempdir().unwrap();
        let module_root = project.path().join("entry");
        let resource_table = module_root.join("build/default/generated/r/default/ResourceTable.ts");
        std::fs::create_dir_all(resource_table.parent().unwrap()).unwrap();
        std::fs::write(&resource_table, "//@ts-noCheck\n").unwrap();

        let mut module_path_map = HvigorModulePathMap::new(
            "entry",
            &module_root,
            &[("entry".to_string(), module_root.clone())],
        );
        module_path_map.add_module_meta_info_files(&[crate::plan::BundleModuleMetaInfoFile {
            file_path: resource_table.clone(),
            ohm_url: "com.example/entry/build/generated/r/ResourceTable".to_string(),
        }]);
        module_path_map
            .add_compile_entry_package_roots(&[resource_table.to_string_lossy().into_owned()]);

        assert_eq!(
            module_path_map.module_meta_info_ohm_url(resource_table.to_str().unwrap()),
            Some("com.example/entry/build/generated/r/ResourceTable")
        );
    }

    #[test]
    fn module_path_map_keeps_package_version_for_src_main_sources() {
        let project = tempfile::tempdir().unwrap();
        let module_root = project.path().join("entry");
        let src_main = module_root.join("src/main");
        let source_path = src_main.join("ets/pages/Index.ets");
        std::fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        std::fs::write(
            module_root.join("oh-package.json5"),
            r#"{"name":"@example/entry","version":"12.61.200"}"#,
        )
        .unwrap();
        std::fs::write(&source_path, "").unwrap();

        let module_root = arkdown_path::canonicalize_or_owned(&module_root);
        let src_main = arkdown_path::canonicalize_or_owned(&src_main);
        let source_path = arkdown_path::canonicalize_or_owned(&source_path);
        let module_path_map =
            HvigorModulePathMap::new("entry", &src_main, &[("entry".to_string(), module_root)]);
        let source = module_path_map
            .source_file_for_entry(source_path.to_str().unwrap())
            .unwrap();

        assert_eq!(source.module_name, "@example/entry");
        assert_eq!(source.package_version, "12.61.200");
        assert_eq!(source.source_file, "ets/pages/Index.ets");
    }

    #[test]
    fn normalized_source_record_and_dependency_attribution_match_latest_module_meta_plugin() {
        let project = tempfile::tempdir().unwrap();
        let module_root = project.path().join("entry");
        let entry_source = module_root.join("src/main/ets/Index.ets");
        let bundled_har_root = module_root.join("oh_modules/share-config-har");
        let bundled_har_source = bundled_har_root.join("Index.ets");
        let external_root = project.path().join("oh_modules/external-source-har");
        let external_source = external_root.join("Index.ets");
        std::fs::create_dir_all(entry_source.parent().unwrap()).unwrap();
        std::fs::create_dir_all(&bundled_har_root).unwrap();
        std::fs::create_dir_all(&external_root).unwrap();
        std::fs::write(
            module_root.join("oh-package.json5"),
            r#"{"name":"entry","version":"2.0.0"}"#,
        )
        .unwrap();
        std::fs::write(
            bundled_har_root.join("oh-package.json5"),
            r#"{"name":"share-config-har","version":"9.0.0","main":"Index.ets"}"#,
        )
        .unwrap();
        std::fs::write(
            external_root.join("oh-package.json5"),
            r#"{"name":"external-source-har","version":"8.0.0","main":"Index.ets"}"#,
        )
        .unwrap();
        std::fs::write(&entry_source, "export const entry = true;").unwrap();
        std::fs::write(&bundled_har_source, "export const bundled = true;").unwrap();
        std::fs::write(&external_source, "export const external = true;").unwrap();

        let module_path_map = HvigorModulePathMap::new(
            "entry",
            &module_root,
            &[
                ("entry".to_string(), module_root.clone()),
                ("share-config-har".to_string(), bundled_har_root),
                ("external-source-har".to_string(), external_root),
            ],
        );
        let entry = module_path_map
            .source_file_for_entry(entry_source.to_str().unwrap())
            .unwrap();
        let mut packages = FxHashMap::default();
        packages.insert(
            "entry".to_string(),
            BundleRuntimePackageContext {
                package_name: "entry".to_string(),
                bundle_name: String::new(),
                module_name: "entry".to_string(),
                version: "2.0.0".to_string(),
                entry_path: "src/main/ets/Index.ets".to_string(),
                is_so: false,
            },
        );
        assert_eq!(
            entry
                .normalized_record_name(&BundleRuntimePackageInfo { packages })
                .unwrap(),
            "&entry/src/main/ets/Index&2.0.0"
        );

        let bundled = module_path_map
            .source_file_for_entry(bundled_har_source.to_str().unwrap())
            .unwrap();
        assert!(!module_path_map.has_dependency_package_info(bundled_har_source.to_str().unwrap()));
        assert_eq!(
            bundled.file_info_source_file("entry", "2.0.0", false, false),
            "entry|entry|2.0.0|Index.ts"
        );
        assert_eq!(bundled.module_name, "share-config-har");

        let external = module_path_map
            .source_file_for_entry(external_source.to_str().unwrap())
            .unwrap();
        assert!(module_path_map.has_dependency_package_info(external_source.to_str().unwrap()));
        assert_eq!(
            external.file_info_source_file("entry", "2.0.0", true, false),
            "entry|external-source-har|8.0.0|Index.ts"
        );
    }

    #[test]
    fn preload_system_so_tasks_follow_reachable_module_graph_like_sdk() {
        let temp = tempfile::tempdir().unwrap();
        let entry = temp.path().join("EntryAbility.ets");
        let dependency = temp.path().join("Dependency.ets");
        std::fs::write(&entry, "").unwrap();
        std::fs::write(&dependency, "").unwrap();
        let entry_id = arkdown_path::canonicalize_or_owned(&entry)
            .to_string_lossy()
            .into_owned();
        let dependency_id = arkdown_path::canonicalize_or_owned(&dependency)
            .to_string_lossy()
            .into_owned();
        let mut state = HvigorModuleSourceState::default();
        state.module_ids = vec![entry_id.clone(), dependency_id.clone()];
        state.sources.insert(
            entry_id.clone(),
            HvigorCollectedModuleSource {
                code: format!(
                    "import './Dependency'; import hilog from '@ohos.hilog'; export {{ hilog }};"
                )
                .into(),
            },
        );
        state.sources.insert(
            dependency_id.clone(),
            HvigorCollectedModuleSource {
                code: "import crypto from 'libcrypto.so'; export { crypto };".into(),
            },
        );
        record_resolved_import(&mut state, &entry_id, "./Dependency", &dependency_id);
        record_resolved_import(&mut state, &entry_id, "@ohos.hilog", "@ohos.hilog");
        record_resolved_import(&mut state, &dependency_id, "libcrypto.so", "libcrypto.so");

        let tasks = state.preload_system_so_tasks(
            &[entry],
            &HvigorChunkModuleRequestProcessor::new("com.example.app", "entry"),
        );
        assert_eq!(tasks.len(), 2, "{tasks:?}");
        assert!(tasks
            .iter()
            .any(|task| { task.name == "@ohos.hilog" && task.ohm_url == "@ohos:hilog" }));
        assert!(tasks.iter().any(|task| {
            task.name == "libcrypto.so" && task.ohm_url == "@app:com.example.app/entry/crypto"
        }));
    }

    #[test]
    fn bytecode_har_imports_follow_hvigor_normalized_alias_rules() {
        let mut packages = FxHashMap::default();
        packages.insert(
            "@tencent/qq-open-sdk".to_string(),
            BundleRuntimePackageContext {
                package_name: "@tencent/qq-open-sdk".to_string(),
                bundle_name: String::new(),
                module_name: String::new(),
                version: "1.0.4".to_string(),
                entry_path: "Index.d.ets".to_string(),
                is_so: false,
            },
        );
        let runtime_packages = BundleRuntimePackageInfo { packages };
        let package_names = vec!["@tencent/qq-open-sdk".to_string()];
        let imports = HvigorBytecodeHarImports::new(&runtime_packages, &package_names, true);

        assert_eq!(
            imports
                .ohm_url_for_request("@tencent/qq-open-sdk")
                .unwrap()
                .as_deref(),
            Some("@normalized:N&&&@tencent/qq-open-sdk/Index&1.0.4")
        );
        assert_eq!(
            imports
                .ohm_url_for_request("@tencent/qq-open-sdk/src/main/ets/QQOpenApi")
                .unwrap()
                .as_deref(),
            Some("@normalized:N&&&@tencent/qq-open-sdk/src/main/ets/QQOpenApi&1.0.4")
        );
    }

    #[test]
    fn normalized_native_so_imports_follow_hvigor_pkg_context_info() {
        let mut packages = FxHashMap::default();
        packages.insert(
            "liblx.so".to_string(),
            BundleRuntimePackageContext {
                package_name: "liblx.so".to_string(),
                is_so: true,
                ..BundleRuntimePackageContext::default()
            },
        );
        let runtime_packages = BundleRuntimePackageInfo { packages };
        let imports = HvigorBytecodeHarImports::new(&runtime_packages, &[], true);

        assert_eq!(
            imports.ohm_url_for_request("liblx.so").unwrap().as_deref(),
            Some("@normalized:Y&&&liblx.so&")
        );
    }

    #[test]
    fn module_source_writer_rewrites_unresolved_native_so_static_import() {
        let app_root = tempfile::tempdir().unwrap();
        let project_root = app_root.path();
        let phone_root = project_root.join("phone");
        let entry_id = phone_root
            .join("src/main/ets/entryability/EntryAbility.ets")
            .to_string_lossy()
            .into_owned();
        let module_path_map = HvigorModulePathMap::new("phone", &phone_root, &[]);
        let mut state = HvigorModuleSourceState::default();
        state.sources.insert(
            entry_id.clone(),
            HvigorCollectedModuleSource {
                code: "import { setHornLxDict } from \"liblx.so\";"
                    .to_string()
                    .into(),
            },
        );
        record_resolved_import(&mut state, &entry_id, "liblx.so", "liblx.so");
        let facts = ModuleGraphFacts {
            full_module_ids: vec![entry_id.clone()],
            full_module_is_commonjs: FxHashMap::default(),
        };
        let out_dir = app_root
            .path()
            .join("cache")
            .join("default@CompileArkTS")
            .join("esmodule")
            .join("debug");
        let mut packages = FxHashMap::default();
        packages.insert(
            "phone".to_string(),
            BundleRuntimePackageContext {
                package_name: "phone".to_string(),
                ..BundleRuntimePackageContext::default()
            },
        );
        packages.insert(
            "liblx.so".to_string(),
            BundleRuntimePackageContext {
                package_name: "liblx.so".to_string(),
                is_so: true,
                ..BundleRuntimePackageContext::default()
            },
        );
        let runtime_packages = BundleRuntimePackageInfo { packages };

        let files = HvigorModuleSourceWriter::new(
            "com.example.hmosworld",
            "phone",
            project_root,
            &module_path_map,
            &runtime_packages,
            &[],
            true,
            &facts,
            facts.full_module_ids.clone(),
            state,
            out_dir.clone(),
        )
        .write()
        .unwrap();

        let entry_file = files
            .iter()
            .find(|file| file.module_id == entry_id)
            .expect("entry module source");
        assert_eq!(entry_file.imported_record_names, vec!["&liblx.so&"]);
        let output = std::fs::read_to_string(&entry_file.path).unwrap();
        assert!(output.contains("\"@normalized:Y&&&liblx.so&\""), "{output}");
        assert!(!output.contains("@app:"), "{output}");

        let stale_file = out_dir.join("stale").join("Old.ts");
        fs_err::create_dir_all(stale_file.parent().unwrap()).unwrap();
        fs_err::write(&stale_file, "old").unwrap();
        let source_maps_cache = out_dir.join("sourceMaps.json");
        let program_cache = out_dir.join("modules.cache");
        fs_err::write(&source_maps_cache, r#"{ "Index.ts": {} }"#).unwrap();
        fs_err::write(&program_cache, "module-cache").unwrap();

        let mut next_state = HvigorModuleSourceState::default();
        next_state.sources.insert(
            entry_id.clone(),
            HvigorCollectedModuleSource {
                code: "import { setHornLxDict } from \"liblx.so\";"
                    .to_string()
                    .into(),
            },
        );
        record_resolved_import(&mut next_state, &entry_id, "liblx.so", "liblx.so");

        let next_files = HvigorModuleSourceWriter::new(
            "com.example.hmosworld",
            "phone",
            project_root,
            &module_path_map,
            &runtime_packages,
            &[],
            true,
            &facts,
            facts.full_module_ids.clone(),
            next_state,
            out_dir,
        )
        .write()
        .unwrap();
        let next_entry_file = next_files
            .iter()
            .find(|file| file.module_id == entry_id)
            .expect("entry module source after second write");
        assert!(!stale_file.exists(), "stale module source must be removed");
        assert_eq!(
            fs_err::read_to_string(&source_maps_cache).unwrap(),
            r#"{ "Index.ts": {} }"#,
            "CompileArkTS source-map cache is not a stale module source"
        );
        assert_eq!(
            fs_err::read_to_string(&program_cache).unwrap(),
            "module-cache",
            "CompileArkTS ProgramCache is not a stale module source"
        );
        assert_eq!(
            std::fs::read_to_string(&next_entry_file.path).unwrap(),
            output
        );
    }

    #[test]
    fn module_source_writer_rewrites_type_only_static_imports_with_hvigor_requests() {
        let app_root = tempfile::tempdir().unwrap();
        let project_root = app_root.path();
        let meituan_root = project_root.join("meituan");
        let entry_id = meituan_root
            .join("src/main/ets/pages/TypeOnly.ets")
            .to_string_lossy()
            .into_owned();
        let module_path_map = HvigorModulePathMap::new("meituan", &meituan_root, &[]);
        let mut state = HvigorModuleSourceState::default();
        state.sources.insert(
            entry_id.clone(),
            HvigorCollectedModuleSource {
                code: r#"
import type Want from "@ohos.app.ability.Want";
import type { MapBridge } from "libmsimap.so";
import type { IMtBluetoothAccess } from "@meituan/privacy";

export type Use = Want | MapBridge | IMtBluetoothAccess;
"#
                .to_string()
                .into(),
            },
        );
        let facts = ModuleGraphFacts {
            full_module_ids: vec![entry_id.clone()],
            full_module_is_commonjs: FxHashMap::default(),
        };
        let out_dir = app_root
            .path()
            .join("cache")
            .join("default@CompileArkTS")
            .join("esmodule")
            .join("debug");
        let mut packages = FxHashMap::default();
        packages.insert(
            "meituan".to_string(),
            BundleRuntimePackageContext {
                package_name: "meituan".to_string(),
                ..BundleRuntimePackageContext::default()
            },
        );
        packages.insert(
            "libmsimap.so".to_string(),
            BundleRuntimePackageContext {
                package_name: "libmsimap.so".to_string(),
                is_so: true,
                ..BundleRuntimePackageContext::default()
            },
        );
        packages.insert(
            "@meituan/privacy".to_string(),
            BundleRuntimePackageContext {
                package_name: "@meituan/privacy".to_string(),
                version: "0.3.20".to_string(),
                entry_path: "Index.d.ets".to_string(),
                ..BundleRuntimePackageContext::default()
            },
        );
        let runtime_packages = BundleRuntimePackageInfo { packages };
        let byte_code_har_packages = vec!["@meituan/privacy".to_string()];

        let files = HvigorModuleSourceWriter::new(
            "com.sankuai.hmeituan",
            "meituan",
            project_root,
            &module_path_map,
            &runtime_packages,
            &byte_code_har_packages,
            true,
            &facts,
            facts.full_module_ids.clone(),
            state,
            out_dir,
        )
        .write()
        .unwrap();

        let entry_file = files
            .iter()
            .find(|file| file.module_id == entry_id)
            .expect("entry module source");
        assert_eq!(
            entry_file.imported_record_names,
            vec![
                "&@meituan/privacy/Index&0.3.20".to_string(),
                "&libmsimap.so&".to_string()
            ]
        );
        let output = std::fs::read_to_string(&entry_file.path).unwrap();
        assert!(output.contains("\"@ohos:app.ability.Want\""), "{output}");
        assert!(
            output.contains("\"@normalized:Y&&&libmsimap.so&\""),
            "{output}"
        );
        assert!(
            output.contains("\"@normalized:N&&&@meituan/privacy/Index&0.3.20\""),
            "{output}"
        );
    }

    #[test]
    fn module_source_ids_use_stable_graph_order_and_sorted_collector_fallbacks() {
        let entry_id = "/project/entry/src/main/ets/EntryAbility.ets".to_string();
        let profile_id = "/project/entry/build/generated/profile/ModuleInfo.ets".to_string();
        let dependency_id = "/project/oh_modules/@scope/pkg/Index.ets".to_string();
        let collected_id = "/project/oh_modules/@scope/z-extra/Index.ets".to_string();
        let earlier_collected_id = "/project/oh_modules/@scope/a-extra/Index.ets".to_string();
        let facts = ModuleGraphFacts {
            full_module_ids: vec![dependency_id.clone(), profile_id.clone(), entry_id.clone()],
            full_module_is_commonjs: FxHashMap::default(),
        };
        let mut state = HvigorModuleSourceState::default();
        state.sources.insert(
            collected_id.clone(),
            HvigorCollectedModuleSource {
                code: "export const value = 1;".to_string().into(),
            },
        );
        state.sources.insert(
            earlier_collected_id.clone(),
            HvigorCollectedModuleSource {
                code: "export const value = 2;".to_string().into(),
            },
        );

        assert_eq!(
            Bundler::module_source_ids(&[entry_id.clone(), profile_id.clone()], &facts, &state),
            vec![
                entry_id,
                profile_id,
                dependency_id,
                earlier_collected_id,
                collected_id
            ]
        );
    }

    #[test]
    fn module_source_writer_records_bytecode_har_import_records_for_compile_context() {
        let app_root = tempfile::tempdir().unwrap();
        let project_root = app_root.path();
        let phone_root = project_root.join("phone");
        let entry_id = phone_root
            .join("src/main/ets/entryability/EntryAbility.ets")
            .to_string_lossy()
            .into_owned();
        let module_path_map = HvigorModulePathMap::new("phone", &phone_root, &[]);
        let mut state = HvigorModuleSourceState::default();
        state.sources.insert(
            entry_id.clone(),
            HvigorCollectedModuleSource {
                code: "import { IQQOpenApi } from \"@tencent/qq-open-sdk\";"
                    .to_string()
                    .into(),
            },
        );
        state
            .resolved_imports
            .entry(entry_id.clone())
            .or_default()
            .insert(
                "@tencent/qq-open-sdk".to_string(),
                "@tencent/qq-open-sdk".to_string(),
            );
        let facts = ModuleGraphFacts {
            full_module_ids: vec![entry_id.clone()],
            full_module_is_commonjs: FxHashMap::default(),
        };
        let out_dir = app_root
            .path()
            .join("cache")
            .join("default@CompileArkTS")
            .join("esmodule")
            .join("release");
        let mut packages = FxHashMap::default();
        packages.insert(
            "phone".to_string(),
            BundleRuntimePackageContext {
                package_name: "phone".to_string(),
                ..BundleRuntimePackageContext::default()
            },
        );
        packages.insert(
            "@tencent/qq-open-sdk".to_string(),
            BundleRuntimePackageContext {
                package_name: "@tencent/qq-open-sdk".to_string(),
                version: "1.0.4".to_string(),
                entry_path: "Index".to_string(),
                ..BundleRuntimePackageContext::default()
            },
        );
        let runtime_packages = BundleRuntimePackageInfo { packages };
        let byte_code_har_packages = vec!["@tencent/qq-open-sdk".to_string()];

        let files = HvigorModuleSourceWriter::new(
            "com.example.hmosworld",
            "phone",
            project_root,
            &module_path_map,
            &runtime_packages,
            &byte_code_har_packages,
            true,
            &facts,
            facts.full_module_ids.clone(),
            state,
            out_dir,
        )
        .write()
        .unwrap();

        let entry_file = files
            .iter()
            .find(|file| file.module_id == entry_id)
            .expect("entry module source");
        assert_eq!(
            entry_file.imported_record_names,
            vec!["&@tencent/qq-open-sdk/Index&1.0.4".to_string()]
        );
        let output = std::fs::read_to_string(&entry_file.path).unwrap();
        assert!(
            output.contains("\"@normalized:N&&&@tencent/qq-open-sdk/Index&1.0.4\""),
            "{output}"
        );
    }

    #[test]
    fn module_source_writer_rewrites_case_mismatched_bare_package_entry_request() {
        let app_root = tempfile::tempdir().unwrap();
        let project_root = app_root.path();
        let phone_root = project_root.join("phone");
        let abtest_root =
            project_root.join("oh_modules/.ohpm/@meituan+abtest@0.0.23/oh_modules/@meituan/abtest");
        std::fs::create_dir_all(&abtest_root).unwrap();
        std::fs::write(
            abtest_root.join("oh-package.json5"),
            r#"{ name: "@meituan/abtest", version: "0.0.23", main: "Index.ets" }"#,
        )
        .unwrap();
        std::fs::write(
            abtest_root.join("Index.ets"),
            "export class ABTestService {}",
        )
        .unwrap();
        let entry_id = phone_root
            .join("src/main/ets/entryability/EntryAbility.ets")
            .to_string_lossy()
            .into_owned();
        let abtest_id = abtest_root.join("Index.ets").to_string_lossy().into_owned();
        let module_path_map = HvigorModulePathMap::new(
            "phone",
            &phone_root,
            &[("@meituan/abtest".to_string(), abtest_root.clone())],
        );
        let mut state = HvigorModuleSourceState::default();
        state.sources.insert(
            entry_id.clone(),
            HvigorCollectedModuleSource {
                code: "import { ABTestService } from \"@meituan/abTest\";"
                    .to_string()
                    .into(),
            },
        );
        state.sources.insert(
            abtest_id.clone(),
            HvigorCollectedModuleSource {
                code: "export class ABTestService {}".to_string().into(),
            },
        );
        state
            .resolved_imports
            .entry(entry_id.clone())
            .or_default()
            .insert("@meituan/abTest".to_string(), abtest_id.clone());
        let facts = ModuleGraphFacts {
            full_module_ids: vec![entry_id.clone(), abtest_id.clone()],
            full_module_is_commonjs: FxHashMap::default(),
        };
        let out_dir = app_root
            .path()
            .join("cache")
            .join("default@CompileArkTS")
            .join("esmodule")
            .join("release");
        let mut packages = FxHashMap::default();
        packages.insert(
            "phone".to_string(),
            BundleRuntimePackageContext {
                package_name: "phone".to_string(),
                ..BundleRuntimePackageContext::default()
            },
        );
        packages.insert(
            "@meituan/abtest".to_string(),
            BundleRuntimePackageContext {
                package_name: "@meituan/abtest".to_string(),
                version: "0.0.23".to_string(),
                entry_path: "Index.ets".to_string(),
                ..BundleRuntimePackageContext::default()
            },
        );
        let runtime_packages = BundleRuntimePackageInfo { packages };

        let files = HvigorModuleSourceWriter::new(
            "com.example.hmosworld",
            "phone",
            project_root,
            &module_path_map,
            &runtime_packages,
            &[],
            true,
            &facts,
            facts.full_module_ids.clone(),
            state,
            out_dir,
        )
        .write()
        .unwrap();

        let entry_file = files
            .iter()
            .find(|file| file.module_id == entry_id)
            .expect("entry module source");
        assert_eq!(
            entry_file.imported_record_names,
            vec!["&@meituan/abtest/Index&0.0.23".to_string()]
        );
        let output = std::fs::read_to_string(&entry_file.path).unwrap();
        assert!(
            output.contains("\"@normalized:N&&&@meituan/abtest/Index&0.0.23\""),
            "{output}"
        );
    }

    #[test]
    fn chunk_source_files_follow_hvigor_module_path_map() {
        let app_root = PathBuf::from("/workspace/app");
        let module_root = app_root.join("products/phone/src/main");
        let plan = BundlePlan {
            input: BundleInput {
                loader_json_path: app_root.join("loader.json"),
                project_root: app_root.clone(),
                project_path: module_root.join("ets"),
                module_root: module_root.clone(),
                module_name: "phone".to_string(),
                module_path_map: vec![
                    ("phone".to_string(), app_root.join("products/phone")),
                    (
                        "audioplayer".to_string(),
                        app_root.join("commons/audioplayer"),
                    ),
                ],
                module_record_roots: Vec::new(),
                module_meta_info_files: Vec::new(),
                entries: vec![
                    "./ets/entryability/EntryAbility.ets".to_string(),
                    app_root
                        .join("commons/audioplayer/src/main/ets/service/SpeechPlayerService.ets")
                        .to_string_lossy()
                        .into_owned(),
                    app_root
                        .join("commons/audioplayer/Index.ets")
                        .to_string_lossy()
                        .into_owned(),
                ],
                form_entry_sources: Vec::new(),
                runtime_entry_sources: vec![],
                support_chunks: false,
                source_map_packages: crate::plan::BundleSourceMapPackageInfo {
                    entry_package_name: "phone".to_string(),
                    entry_package_version: "1.0.0".to_string(),
                    dependency_package_versions: FxHashMap::default(),
                },
                runtime_packages: runtime_packages(&[("phone", "phone", "", "", "", false)]),
                byte_code_har_packages: Vec::new(),
                preload_system_so_entries: Vec::new(),
                mock_config: None,
            },
            output: BundleOutputOptions::default(),
            source_map: SourceMapPolicy::External,
            cache: BundleCachePolicy::default(),
            loader_resource: LoaderResourceJoin {
                loader_json_path: app_root.join("loader.json"),
                resources_index_path: None,
                resource_table_path: None,
            },
            plugins: BundlePluginSet::default(),
            build_mode: BuildMode::Debug,
            target_api_version: 12,
            bundle_name: "com.example.hmosworld".to_string(),
            use_normalized_ohm_url: true,
            should_emit_js: true,
            module_analyzer: arkdown_ets_transform::ModuleAnalyzerKind::default(),
            output_target: arkdown_core::OutputTarget::Hap,
            shim_missing_exports_override: None,
            treeshake_override: None,
            context_override: None,
        };

        let input_plan = RolldownInputPlan::from_bundle_plan(&plan);
        let source_files = input_plan.chunk_source_files().unwrap();

        assert_eq!(
            source_files
                .get("entryability/EntryAbility")
                .unwrap()
                .source_file,
            "ets/entryability/EntryAbility.ets"
        );
        assert_eq!(
            source_files
                .get("entryability/EntryAbility")
                .unwrap()
                .module_name,
            "phone"
        );
        assert_eq!(
            source_files.get("SpeechPlayerService").unwrap().source_file,
            "ets/service/SpeechPlayerService.ets"
        );
        assert_eq!(
            source_files.get("SpeechPlayerService").unwrap().module_name,
            "audioplayer"
        );
        assert_eq!(source_files.get("Index").unwrap().source_file, "Index.ets");
        assert_eq!(
            source_files.get("Index").unwrap().module_name,
            "audioplayer"
        );
    }

    #[test]
    fn compile_entry_package_roots_do_not_require_loader_module_path_map_entries() {
        let app_root = tempfile::tempdir().unwrap();
        let project_root = app_root.path();
        let module_root = project_root.join("phone/src/main");
        let phone_root = project_root.join("phone");
        let package_root =
            project_root.join("oh_modules/.ohpm/mtlauncher@hash=/oh_modules/mtlauncher");
        fs_err::create_dir_all(&package_root).unwrap();
        fs_err::write(
            package_root.join("oh-package.json5"),
            r#"{
  "name": "mtlauncher",
  "version": "12.59.403",
  "main": "Index.ets"
}"#,
        )
        .unwrap();
        let package_entry = package_root.join("Index.ets");
        fs_err::write(&package_entry, "export const launcher = 1;").unwrap();
        let package_entry_id = package_entry.to_string_lossy().into_owned();
        let mut runtime_package_info = runtime_packages(&[("phone", "phone", "", "", "", false)]);
        runtime_package_info.packages.insert(
            "mtlauncher".to_string(),
            BundleRuntimePackageContext {
                package_name: "mtlauncher".to_string(),
                version: "12.59.403".to_string(),
                entry_path: "Index.ets".to_string(),
                ..BundleRuntimePackageContext::default()
            },
        );
        let plan = BundlePlan {
            input: BundleInput {
                loader_json_path: project_root.join("loader.json"),
                project_root: project_root.to_path_buf(),
                project_path: module_root.join("ets"),
                module_root: module_root.clone(),
                module_name: "phone".to_string(),
                module_path_map: vec![("phone".to_string(), phone_root)],
                module_record_roots: Vec::new(),
                module_meta_info_files: Vec::new(),
                entries: vec![package_entry_id.clone()],
                form_entry_sources: Vec::new(),
                runtime_entry_sources: vec![],
                support_chunks: false,
                source_map_packages: crate::plan::BundleSourceMapPackageInfo {
                    entry_package_name: "phone".to_string(),
                    entry_package_version: "1.0.0".to_string(),
                    dependency_package_versions: FxHashMap::default(),
                },
                runtime_packages: runtime_package_info,
                byte_code_har_packages: Vec::new(),
                preload_system_so_entries: Vec::new(),
                mock_config: None,
            },
            output: BundleOutputOptions::default(),
            source_map: SourceMapPolicy::External,
            cache: BundleCachePolicy::default(),
            loader_resource: LoaderResourceJoin {
                loader_json_path: project_root.join("loader.json"),
                resources_index_path: None,
                resource_table_path: None,
            },
            plugins: BundlePluginSet::default(),
            build_mode: BuildMode::Debug,
            target_api_version: 12,
            bundle_name: "com.example.hmosworld".to_string(),
            use_normalized_ohm_url: true,
            should_emit_js: true,
            module_analyzer: arkdown_ets_transform::ModuleAnalyzerKind::default(),
            output_target: arkdown_core::OutputTarget::Hap,
            shim_missing_exports_override: None,
            treeshake_override: None,
            context_override: None,
        };

        let input_plan = RolldownInputPlan::from_bundle_plan(&plan);
        let source_files = input_plan.chunk_source_files().unwrap();
        assert_eq!(source_files.get("Index").unwrap().module_name, "mtlauncher");
        assert_eq!(source_files.get("Index").unwrap().source_file, "Index.ets");

        let mut state = HvigorModuleSourceState::default();
        state.sources.insert(
            package_entry_id.clone(),
            HvigorCollectedModuleSource {
                code: "export const launcher = 1;".to_string().into(),
            },
        );
        let facts = ModuleGraphFacts {
            full_module_ids: vec![package_entry_id.clone()],
            full_module_is_commonjs: FxHashMap::default(),
        };
        let files = HvigorModuleSourceWriter::new(
            "com.example.hmosworld",
            "phone",
            project_root,
            &input_plan.module_path_map,
            &plan.input.runtime_packages,
            &[],
            true,
            &facts,
            facts.full_module_ids.clone(),
            state,
            project_root.join("cache"),
        )
        .write()
        .unwrap();

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].record_name, "&mtlauncher/Index&12.59.403");
        assert_eq!(files[0].source_file, "phone|mtlauncher|12.59.403|Index.ts");
    }

    #[test]
    fn module_record_roots_attribute_dependency_sources_without_loader_module_path_map_entries() {
        let app_root = tempfile::tempdir().unwrap();
        let project_root = app_root.path();
        let phone_root = project_root.join("phone");
        let log_root =
            project_root.join("oh_modules/.ohpm/@meituan+log@hash=/oh_modules/@meituan/log");
        fs_err::create_dir_all(&log_root).unwrap();
        fs_err::write(
            log_root.join("oh-package.json5"),
            r#"{
  "name": "@meituan/log",
  "version": "0.0.18",
  "main": "Index.ets"
}"#,
        )
        .unwrap();
        let log_entry = log_root.join("Index.ets");
        fs_err::write(&log_entry, "export const log = 1;").unwrap();
        let log_entry_id = log_entry.to_string_lossy().into_owned();
        let mut module_path_map = HvigorModulePathMap::new(
            "phone",
            &phone_root,
            &[("phone".to_string(), phone_root.clone())],
        );
        module_path_map.add_module_record_roots(&[("@meituan/log".to_string(), log_root.clone())]);

        let ownership = module_path_map
            .source_file_for_entry(&log_entry_id)
            .expect("dependency source ownership");
        assert_eq!(ownership.module_name, "@meituan/log");
        assert_eq!(ownership.source_file, "Index.ets");

        let mut state = HvigorModuleSourceState::default();
        state.sources.insert(
            log_entry_id.clone(),
            HvigorCollectedModuleSource {
                code: "export const log = 1;".to_string().into(),
            },
        );
        let facts = ModuleGraphFacts {
            full_module_ids: vec![log_entry_id.clone()],
            full_module_is_commonjs: FxHashMap::default(),
        };
        let mut runtime_packages = runtime_packages(&[("phone", "phone", "", "", "", false)]);
        runtime_packages.packages.insert(
            "@meituan/log".to_string(),
            BundleRuntimePackageContext {
                package_name: "@meituan/log".to_string(),
                version: "0.0.18".to_string(),
                entry_path: "Index.ets".to_string(),
                ..BundleRuntimePackageContext::default()
            },
        );

        let files = HvigorModuleSourceWriter::new(
            "com.example.hmosworld",
            "phone",
            project_root,
            &module_path_map,
            &runtime_packages,
            &[],
            true,
            &facts,
            facts.full_module_ids.clone(),
            state,
            project_root.join("cache"),
        )
        .write()
        .unwrap();

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].record_name, "&@meituan/log/Index&0.0.18");
        assert_eq!(files[0].source_file, "phone|@meituan/log|0.0.18|Index.ts");
    }

    #[test]
    fn module_source_writer_rewrites_source_har_imports_with_pkg_context_module_name() {
        let app_root = tempfile::tempdir().unwrap();
        let project_root = app_root.path();
        let phone_root = project_root.join("phone");
        let audioplayer_root = project_root.join("commons/audioplayer");
        let entry_id = phone_root
            .join("src/main/ets/entryability/EntryAbility.ets")
            .to_string_lossy()
            .into_owned();
        let speech_id = audioplayer_root
            .join("src/main/ets/service/SpeechPlayerService.ets")
            .to_string_lossy()
            .into_owned();
        let module_path_map = HvigorModulePathMap::new(
            "phone",
            &phone_root,
            &[("audioplayer".to_string(), audioplayer_root.clone())],
        );
        let mut state = HvigorModuleSourceState::default();
        state.sources.insert(
            entry_id.clone(),
            HvigorCollectedModuleSource {
                code: "import { SpeechPlayerService } from \"../SpeechPlayerService\";"
                    .to_string()
                    .into(),
            },
        );
        state.sources.insert(
            speech_id.clone(),
            HvigorCollectedModuleSource {
                code: "export class SpeechPlayerService {}".to_string().into(),
            },
        );
        state
            .resolved_imports
            .entry(entry_id.clone())
            .or_default()
            .insert("../SpeechPlayerService".to_string(), speech_id.clone());
        let facts = ModuleGraphFacts {
            full_module_ids: vec![entry_id.clone(), speech_id.clone()],
            full_module_is_commonjs: FxHashMap::default(),
        };
        let out_dir = app_root
            .path()
            .join("cache")
            .join("default@CompileArkTS")
            .join("esmodule")
            .join("release");
        let runtime_packages = runtime_packages(&[
            ("phone", "phone", "", "", "", false),
            ("audioplayer", "audioplayer", "", "", "1.0.0", false),
        ]);

        let files = HvigorModuleSourceWriter::new(
            "com.example.hmosworld",
            "phone",
            project_root,
            &module_path_map,
            &runtime_packages,
            &[],
            true,
            &facts,
            facts.full_module_ids.clone(),
            state,
            out_dir,
        )
        .write()
        .unwrap();

        assert_eq!(files.len(), 2);
        let entry_file = files
            .iter()
            .find(|file| file.module_id == entry_id)
            .expect("entry module source");
        let speech_file = files
            .iter()
            .find(|file| file.module_id == speech_id)
            .expect("dependency module source");
        assert_eq!(
            entry_file.record_name,
            "&phone/src/main/ets/entryability/EntryAbility&"
        );
        assert_eq!(
            entry_file.source_file,
            "phone|phone|1.0.0|src/main/ets/entryability/EntryAbility.ts"
        );
        assert_eq!(
            entry_file.path,
            app_root
                .path()
                .join("cache")
                .join("default@CompileArkTS")
                .join("esmodule")
                .join("release")
                .join("phone")
                .join("src")
                .join("main")
                .join("ets")
                .join("entryability")
                .join("EntryAbility.ts")
        );
        assert_eq!(entry_file.package_name.as_deref(), Some("phone"));
        assert_eq!(
            entry_file.imported_record_names,
            vec!["&audioplayer/src/main/ets/service/SpeechPlayerService&1.0.0".to_string()]
        );
        assert_eq!(
            speech_file.record_name,
            "&audioplayer/src/main/ets/service/SpeechPlayerService&1.0.0"
        );
        assert_eq!(
            speech_file.source_file,
            "phone|audioplayer|1.0.0|src/main/ets/service/SpeechPlayerService.ts"
        );
        assert_eq!(speech_file.package_name.as_deref(), Some("audioplayer"));
        let output = std::fs::read_to_string(&entry_file.path).unwrap();
        assert!(
            output.contains(
                "\"@normalized:N&&&audioplayer/src/main/ets/service/SpeechPlayerService&1.0.0\""
            ),
            "{output}"
        );
    }

    #[test]
    fn module_source_writer_rewrites_static_re_exports_from_rolldown_graph() {
        let app_root = tempfile::tempdir().unwrap();
        let project_root = app_root.path();
        let phone_root = project_root.join("products/phone");
        let mrn_root = project_root.join("oh_modules/.ohpm/@mrn+mrn@12.59.403/oh_modules/@mrn/mrn");
        let index_id = mrn_root.join("Index.ets").to_string_lossy().into_owned();
        let config_id = mrn_root
            .join("src/main/ets/config/MRNConfigCenter.ets")
            .to_string_lossy()
            .into_owned();
        std::fs::create_dir_all(&mrn_root).unwrap();
        std::fs::write(
            mrn_root.join("oh-package.json5"),
            r#"{
                "name": "@mrn/mrn",
                "version": "12.59.403",
                "main": "Index.ets"
            }"#,
        )
        .unwrap();
        std::fs::write(mrn_root.join("Index.ets"), "export {};").unwrap();
        std::fs::create_dir_all(mrn_root.join("src/main/ets/config")).unwrap();
        std::fs::write(
            mrn_root.join("src/main/ets/config/MRNConfigCenter.ets"),
            "export class MRNConfigCenter {}",
        )
        .unwrap();

        let module_path_map = HvigorModulePathMap::new(
            "phone",
            &phone_root,
            &[("@mrn/mrn".to_string(), mrn_root.clone())],
        );
        let mut state = HvigorModuleSourceState::default();
        state.sources.insert(
            index_id.clone(),
            HvigorCollectedModuleSource {
                code: "export { MRNConfigCenter } from \"./src/main/ets/config/MRNConfigCenter\";"
                    .to_string()
                    .into(),
            },
        );
        state.sources.insert(
            config_id.clone(),
            HvigorCollectedModuleSource {
                code: "export class MRNConfigCenter {}".to_string().into(),
            },
        );
        record_resolved_import(
            &mut state,
            &index_id,
            "./src/main/ets/config/MRNConfigCenter",
            &config_id,
        );
        let facts = ModuleGraphFacts {
            full_module_ids: vec![index_id.clone(), config_id.clone()],
            full_module_is_commonjs: FxHashMap::default(),
        };
        let out_dir = app_root
            .path()
            .join("cache")
            .join("default@CompileArkTS")
            .join("esmodule")
            .join("release");
        let mut runtime_packages =
            runtime_packages(&[("@mrn/mrn", "@mrn/mrn", "", "", "12.59.403", false)]);
        runtime_packages
            .packages
            .get_mut("@mrn/mrn")
            .expect("mrn package context")
            .entry_path = "Index.ets".to_string();

        let files = HvigorModuleSourceWriter::new(
            "com.example.hmosworld",
            "phone",
            project_root,
            &module_path_map,
            &runtime_packages,
            &[],
            true,
            &facts,
            facts.full_module_ids.clone(),
            state,
            out_dir,
        )
        .write()
        .unwrap();

        let index_file = files
            .iter()
            .find(|file| file.module_id == index_id)
            .expect("package index source");
        assert_eq!(index_file.record_name, "&@mrn/mrn/Index&12.59.403");
        assert_eq!(
            index_file.imported_record_names,
            vec!["&@mrn/mrn/src/main/ets/config/MRNConfigCenter&12.59.403".to_string()]
        );
        let output = std::fs::read_to_string(&index_file.path).unwrap();
        assert!(
            output.contains(
                "\"@normalized:N&&&@mrn/mrn/src/main/ets/config/MRNConfigCenter&12.59.403\""
            ),
            "{output}"
        );
    }

    #[test]
    fn module_source_writer_rewrites_relative_package_imports_from_resolved_module_ids() {
        let app_root = tempfile::tempdir().unwrap();
        let project_root = app_root.path();
        let phone_root = project_root.join("products/phone");
        let launcher_root =
            project_root.join("oh_modules/.ohpm/mtlauncher@12.59.403/oh_modules/mtlauncher");
        let index_id = launcher_root
            .join("Index.ets")
            .to_string_lossy()
            .into_owned();
        let build_profile_id = launcher_root
            .join("BuildProfile.ets")
            .to_string_lossy()
            .into_owned();
        let resolved_build_profile_id = launcher_root
            .join(".")
            .join("BuildProfile.ets")
            .to_string_lossy()
            .into_owned();
        std::fs::create_dir_all(&launcher_root).unwrap();
        std::fs::write(
            launcher_root.join("oh-package.json5"),
            r#"{
                "name": "mtlauncher",
                "version": "12.59.403",
                "main": "Index.ets"
            }"#,
        )
        .unwrap();
        std::fs::write(launcher_root.join("Index.ets"), "export {};").unwrap();
        std::fs::write(
            launcher_root.join("BuildProfile.ets"),
            "export default class BuildProfile {}",
        )
        .unwrap();

        let module_path_map = HvigorModulePathMap::new(
            "phone",
            &phone_root,
            &[("mtlauncher".to_string(), launcher_root.clone())],
        );
        let mut state = HvigorModuleSourceState::default();
        state.sources.insert(
            index_id.clone(),
            HvigorCollectedModuleSource {
                code: "import BuildProfile from \"./BuildProfile\"; export { BuildProfile };"
                    .to_string()
                    .into(),
            },
        );
        state.sources.insert(
            build_profile_id.clone(),
            HvigorCollectedModuleSource {
                code: "export default class BuildProfile {}".to_string().into(),
            },
        );
        record_resolved_import(
            &mut state,
            &index_id,
            "./BuildProfile",
            &resolved_build_profile_id,
        );
        let facts = ModuleGraphFacts {
            full_module_ids: vec![index_id.clone(), build_profile_id.clone()],
            full_module_is_commonjs: FxHashMap::default(),
        };
        let out_dir = app_root
            .path()
            .join("cache")
            .join("default@CompileArkTS")
            .join("esmodule")
            .join("release");
        let mut runtime_packages =
            runtime_packages(&[("mtlauncher", "mtlauncher", "", "", "12.59.403", false)]);
        runtime_packages
            .packages
            .get_mut("mtlauncher")
            .expect("mtlauncher package context")
            .entry_path = "Index.ets".to_string();

        let files = HvigorModuleSourceWriter::new(
            "com.example.hmosworld",
            "phone",
            project_root,
            &module_path_map,
            &runtime_packages,
            &[],
            true,
            &facts,
            facts.full_module_ids.clone(),
            state,
            out_dir,
        )
        .write()
        .unwrap();

        let index_file = files
            .iter()
            .find(|file| file.module_id == index_id)
            .expect("launcher index source");
        assert_eq!(index_file.record_name, "&mtlauncher/Index&12.59.403");
        assert_eq!(
            index_file.imported_record_names,
            vec!["&mtlauncher/BuildProfile&12.59.403".to_string()]
        );
        let output = std::fs::read_to_string(&index_file.path).unwrap();
        assert!(
            output.contains("\"@normalized:N&&&mtlauncher/BuildProfile&12.59.403\""),
            "{output}"
        );
        assert!(!output.contains("\"./BuildProfile\""), "{output}");
    }

    #[test]
    fn module_source_writer_preserves_duplicate_specifiers_in_resolved_id_map() {
        let app_root = tempfile::tempdir().unwrap();
        let project_root = app_root.path();
        let phone_root = project_root.join("products/phone");
        let package_root =
            project_root.join("oh_modules/.ohpm/mtlauncher@12.59.403/oh_modules/mtlauncher");
        let index_id = package_root
            .join("Index.ets")
            .to_string_lossy()
            .into_owned();
        let constant_id = package_root
            .join("constant/index.ets")
            .to_string_lossy()
            .into_owned();
        let reporter_id = package_root
            .join("reporter/ApiCallReporter.ets")
            .to_string_lossy()
            .into_owned();
        std::fs::create_dir_all(package_root.join("constant")).unwrap();
        std::fs::create_dir_all(package_root.join("reporter")).unwrap();
        std::fs::write(
            package_root.join("oh-package.json5"),
            r#"{
                "name": "mtlauncher",
                "version": "12.59.403",
                "main": "Index.ets"
            }"#,
        )
        .unwrap();
        std::fs::write(package_root.join("Index.ets"), "export {};").unwrap();
        std::fs::write(
            package_root.join("constant/index.ets"),
            "export const ASYNC_OR_SYNC_MODE = 1; export const INNER_SCOPE = 2;",
        )
        .unwrap();
        std::fs::write(
            package_root.join("reporter/ApiCallReporter.ets"),
            "export const KEY_SCOPE = 'scope';",
        )
        .unwrap();

        let module_path_map = HvigorModulePathMap::new(
            "phone",
            &phone_root,
            &[("mtlauncher".to_string(), package_root.clone())],
        );
        let mut state = HvigorModuleSourceState::default();
        state.sources.insert(
            index_id.clone(),
            HvigorCollectedModuleSource {
                code: r#"import { ASYNC_OR_SYNC_MODE } from "./constant";
import { INNER_SCOPE } from "./constant/index";
import { KEY_SCOPE } from "./reporter/ApiCallReporter";
export { ASYNC_OR_SYNC_MODE, INNER_SCOPE, KEY_SCOPE };"#
                    .to_string()
                    .into(),
            },
        );
        state.sources.insert(
            constant_id.clone(),
            HvigorCollectedModuleSource {
                code: "export const ASYNC_OR_SYNC_MODE = 1; export const INNER_SCOPE = 2;"
                    .to_string()
                    .into(),
            },
        );
        state.sources.insert(
            reporter_id.clone(),
            HvigorCollectedModuleSource {
                code: "export const KEY_SCOPE = 'scope';".to_string().into(),
            },
        );
        record_resolved_import(&mut state, &index_id, "./constant", &constant_id);
        record_resolved_import(&mut state, &index_id, "./constant/index", &constant_id);
        record_resolved_import(
            &mut state,
            &index_id,
            "./reporter/ApiCallReporter",
            &reporter_id,
        );
        let facts = ModuleGraphFacts {
            full_module_ids: vec![index_id.clone(), constant_id.clone(), reporter_id.clone()],
            full_module_is_commonjs: FxHashMap::default(),
        };
        let out_dir = app_root
            .path()
            .join("cache")
            .join("default@CompileArkTS")
            .join("esmodule")
            .join("release");
        let mut runtime_packages =
            runtime_packages(&[("mtlauncher", "mtlauncher", "", "", "12.59.403", false)]);
        runtime_packages
            .packages
            .get_mut("mtlauncher")
            .expect("mtlauncher package context")
            .entry_path = "Index.ets".to_string();

        let files = HvigorModuleSourceWriter::new(
            "com.example.hmosworld",
            "phone",
            project_root,
            &module_path_map,
            &runtime_packages,
            &[],
            true,
            &facts,
            facts.full_module_ids.clone(),
            state,
            out_dir,
        )
        .write()
        .unwrap();

        let index_file = files
            .iter()
            .find(|file| file.module_id == index_id)
            .expect("launcher index source");
        assert_eq!(
            index_file.imported_record_names,
            vec![
                "&mtlauncher/constant/index&12.59.403".to_string(),
                "&mtlauncher/reporter/ApiCallReporter&12.59.403".to_string(),
            ]
        );
        let output = std::fs::read_to_string(&index_file.path).unwrap();
        assert!(
            output.contains("\"@normalized:N&&&mtlauncher/constant/index&12.59.403\""),
            "{output}"
        );
        assert!(
            output.contains("\"@normalized:N&&&mtlauncher/reporter/ApiCallReporter&12.59.403\""),
            "{output}"
        );
        assert!(!output.contains("\"./constant\""), "{output}");
        assert!(
            !output.contains("\"./reporter/ApiCallReporter\""),
            "{output}"
        );
    }

    #[test]
    fn module_source_writer_preserves_arkts_syntax_when_rewriting_requests() {
        let app_root = tempfile::tempdir().unwrap();
        let project_root = app_root.path();
        let phone_root = project_root.join("products/phone");
        let lx_root =
            project_root.join("oh_modules/.ohpm/@meituan+lx@4.119.1/oh_modules/@meituan/lx");
        let manager_id = lx_root
            .join("src/main/ets/channel/ChannelManager.ets")
            .to_string_lossy()
            .into_owned();
        let utils_id = lx_root
            .join("src/main/ets/utils/Utils.ets")
            .to_string_lossy()
            .into_owned();
        std::fs::create_dir_all(lx_root.join("src/main/ets/channel")).unwrap();
        std::fs::create_dir_all(lx_root.join("src/main/ets/utils")).unwrap();
        std::fs::write(
            lx_root.join("oh-package.json5"),
            r#"{
                "name": "@meituan/lx",
                "version": "4.119.1",
                "main": "Index.ets"
            }"#,
        )
        .unwrap();
        let manager_code = [
            "import type { PageInfo } from '../pageinfo/PageInfo';",
            "import { isEmptyString } from '../utils/Utils';",
            "export default class ChannelManager {",
            "  private defaultChannelName: string;",
            "  private static instance: ChannelManager;",
            "  constructor(defaultChannelName: string) {",
            "    this.defaultChannelName = isEmptyString(defaultChannelName) ? '' : defaultChannelName;",
            "  }",
            "}",
        ]
        .join("\n");
        let utils_code =
            "export function isEmptyString(value?: string): boolean { return !value; }".to_string();
        std::fs::write(&manager_id, &manager_code).unwrap();
        std::fs::write(&utils_id, &utils_code).unwrap();
        let module_path_map = HvigorModulePathMap::new(
            "phone",
            &phone_root,
            &[("@meituan/lx".to_string(), lx_root.clone())],
        );
        let mut state = HvigorModuleSourceState::default();
        state.sources.insert(
            manager_id.clone(),
            HvigorCollectedModuleSource {
                code: manager_code.into(),
            },
        );
        state.sources.insert(
            utils_id.clone(),
            HvigorCollectedModuleSource {
                code: utils_code.into(),
            },
        );
        state
            .resolved_imports
            .entry(manager_id.clone())
            .or_default()
            .insert("../utils/Utils".to_string(), utils_id.clone());
        let facts = ModuleGraphFacts {
            full_module_ids: vec![manager_id.clone(), utils_id.clone()],
            full_module_is_commonjs: FxHashMap::default(),
        };
        let out_dir = app_root
            .path()
            .join("cache")
            .join("default@CompileArkTS")
            .join("esmodule")
            .join("release");
        let runtime_packages =
            runtime_packages(&[("@meituan/lx", "@meituan/lx", "", "", "4.119.1", false)]);

        let files = HvigorModuleSourceWriter::new(
            "com.example.hmosworld",
            "phone",
            project_root,
            &module_path_map,
            &runtime_packages,
            &[],
            true,
            &facts,
            facts.full_module_ids.clone(),
            state,
            out_dir,
        )
        .write()
        .unwrap();

        let manager_file = files
            .iter()
            .find(|file| file.module_id == manager_id)
            .expect("manager module source");
        let output = std::fs::read_to_string(&manager_file.path).unwrap();
        assert!(
            output.contains("import type { PageInfo } from '../pageinfo/PageInfo';"),
            "{output}"
        );
        assert!(
            output.contains("\"@normalized:N&&&@meituan/lx/src/main/ets/utils/Utils&4.119.1\""),
            "{output}"
        );
        assert!(
            output.contains("private defaultChannelName: string;"),
            "{output}"
        );
        assert!(
            output.contains("private static instance: ChannelManager;"),
            "{output}"
        );
        assert!(
            output.contains("constructor(defaultChannelName: string)"),
            "{output}"
        );
    }

    #[test]
    fn module_source_writer_rewrites_lowercase_index_request_to_pascal_index_record() {
        let app_root = tempfile::tempdir().unwrap();
        let project_root = app_root.path();
        let phone_root = project_root.join("products/phone");
        let mrn_root = project_root.join("oh_modules/.ohpm/@mrn+mrn@12.59.403/oh_modules/@mrn/mrn");
        let manager_id = mrn_root
            .join("src/main/ets/engine/MRNInstanceManager.ets")
            .to_string_lossy()
            .into_owned();
        let route_index_id = mrn_root
            .join("src/main/ets/route/Index.ets")
            .to_string_lossy()
            .into_owned();
        std::fs::create_dir_all(mrn_root.join("src/main/ets/engine")).unwrap();
        std::fs::create_dir_all(mrn_root.join("src/main/ets/route")).unwrap();
        std::fs::write(
            mrn_root.join("oh-package.json5"),
            r#"{
                "name": "@mrn/mrn",
                "version": "12.59.403",
                "main": "Index.ets"
            }"#,
        )
        .unwrap();
        std::fs::write(
            mrn_root.join("src/main/ets/engine/MRNInstanceManager.ets"),
            "import { getMRNRouteURL } from \"../route/index\";",
        )
        .unwrap();
        std::fs::write(
            mrn_root.join("src/main/ets/route/Index.ets"),
            "export function getMRNRouteURL() { return \"imeituan://www.meituan.com/mrn\"; }",
        )
        .unwrap();

        let module_path_map = HvigorModulePathMap::new(
            "phone",
            &phone_root,
            &[("@mrn/mrn".to_string(), mrn_root.clone())],
        );
        let mut state = HvigorModuleSourceState::default();
        state.sources.insert(
            manager_id.clone(),
            HvigorCollectedModuleSource {
                code: "import { getMRNRouteURL } from \"../route/index\";"
                    .to_string()
                    .into(),
            },
        );
        state.sources.insert(
            route_index_id.clone(),
            HvigorCollectedModuleSource {
                code: "export function getMRNRouteURL() { return \"imeituan://www.meituan.com/mrn\"; }"
                    .to_string().into(),
            },
        );
        record_resolved_import(&mut state, &manager_id, "../route/index", &route_index_id);
        let facts = ModuleGraphFacts {
            full_module_ids: vec![manager_id.clone(), route_index_id.clone()],
            full_module_is_commonjs: FxHashMap::default(),
        };
        let out_dir = app_root
            .path()
            .join("cache")
            .join("default@CompileArkTS")
            .join("esmodule")
            .join("release");
        let runtime_packages =
            runtime_packages(&[("@mrn/mrn", "@mrn/mrn", "", "", "12.59.403", false)]);

        let files = HvigorModuleSourceWriter::new(
            "com.example.hmosworld",
            "phone",
            project_root,
            &module_path_map,
            &runtime_packages,
            &[],
            true,
            &facts,
            facts.full_module_ids.clone(),
            state,
            out_dir,
        )
        .write()
        .unwrap();

        let manager_file = files
            .iter()
            .find(|file| file.module_id == manager_id)
            .expect("manager module source");
        assert_eq!(
            manager_file.imported_record_names,
            vec!["&@mrn/mrn/src/main/ets/route/Index&12.59.403".to_string()]
        );
        let output = std::fs::read_to_string(&manager_file.path).unwrap();
        assert!(
            output.contains("\"@normalized:N&&&@mrn/mrn/src/main/ets/route/Index&12.59.403\""),
            "{output}"
        );
    }

    #[test]
    fn module_source_writer_rewrites_mixed_case_request_to_uppercase_record() {
        let app_root = tempfile::tempdir().unwrap();
        let project_root = app_root.path();
        let phone_root = project_root.join("products/phone");
        let lx_root =
            project_root.join("oh_modules/.ohpm/@meituan+lx@4.119.1/oh_modules/@meituan/lx");
        let inner_id = lx_root
            .join("src/main/ets/event/inner/InnerDataManager.ets")
            .to_string_lossy()
            .into_owned();
        let lx_id = lx_root
            .join("src/main/ets/LX.ets")
            .to_string_lossy()
            .into_owned();
        std::fs::create_dir_all(lx_root.join("src/main/ets/event/inner")).unwrap();
        std::fs::create_dir_all(lx_root.join("src/main/ets")).unwrap();
        std::fs::write(
            lx_root.join("oh-package.json5"),
            r#"{
                "name": "@meituan/lx",
                "version": "4.119.1",
                "main": "Index.ets"
            }"#,
        )
        .unwrap();
        std::fs::write(
            lx_root.join("src/main/ets/event/inner/InnerDataManager.ets"),
            "import { isInitialized } from \"../../Lx\";",
        )
        .unwrap();
        std::fs::write(
            lx_root.join("src/main/ets/LX.ets"),
            "export const isInitialized = true;",
        )
        .unwrap();

        let module_path_map = HvigorModulePathMap::new(
            "phone",
            &phone_root,
            &[("@meituan/lx".to_string(), lx_root.clone())],
        );
        let mut state = HvigorModuleSourceState::default();
        state.sources.insert(
            inner_id.clone(),
            HvigorCollectedModuleSource {
                code: "import { isInitialized } from \"../../Lx\";"
                    .to_string()
                    .into(),
            },
        );
        state.sources.insert(
            lx_id.clone(),
            HvigorCollectedModuleSource {
                code: "export const isInitialized = true;".to_string().into(),
            },
        );
        record_resolved_import(&mut state, &inner_id, "../../Lx", &lx_id);
        let facts = ModuleGraphFacts {
            full_module_ids: vec![inner_id.clone(), lx_id.clone()],
            full_module_is_commonjs: FxHashMap::default(),
        };
        let out_dir = app_root
            .path()
            .join("cache")
            .join("default@CompileArkTS")
            .join("esmodule")
            .join("release");
        let runtime_packages =
            runtime_packages(&[("@meituan/lx", "@meituan/lx", "", "", "4.119.1", false)]);

        let files = HvigorModuleSourceWriter::new(
            "com.example.hmosworld",
            "phone",
            project_root,
            &module_path_map,
            &runtime_packages,
            &[],
            true,
            &facts,
            facts.full_module_ids.clone(),
            state,
            out_dir,
        )
        .write()
        .unwrap();

        let inner_file = files
            .iter()
            .find(|file| file.module_id == inner_id)
            .expect("inner module source");
        assert_eq!(
            inner_file.imported_record_names,
            vec!["&@meituan/lx/src/main/ets/LX&4.119.1".to_string()]
        );
        let output = std::fs::read_to_string(&inner_file.path).unwrap();
        assert!(
            output.contains("\"@normalized:N&&&@meituan/lx/src/main/ets/LX&4.119.1\""),
            "{output}"
        );
    }

    #[test]
    fn module_source_writer_rewrites_case_mismatched_relative_file_request() {
        let app_root = tempfile::tempdir().unwrap();
        let project_root = app_root.path();
        let phone_root = project_root.join("products/phone");
        let network_root =
            project_root.join("oh_modules/.ohpm/@waimai+network@1.0.32/oh_modules/@waimai/network");
        let index_id = network_root
            .join("src/main/ets/network/index.ets")
            .to_string_lossy()
            .into_owned();
        let init_id = network_root
            .join("src/main/ets/network/NetworkInit.ets")
            .to_string_lossy()
            .into_owned();
        std::fs::create_dir_all(network_root.join("src/main/ets/network")).unwrap();
        std::fs::write(
            network_root.join("oh-package.json5"),
            r#"{
                "name": "@waimai/network",
                "version": "1.0.32",
                "main": "Index.ets"
            }"#,
        )
        .unwrap();
        std::fs::write(
            network_root.join("src/main/ets/network/index.ets"),
            "export { NetWorkInit } from \"./NetWorkInit\";",
        )
        .unwrap();
        std::fs::write(
            network_root.join("src/main/ets/network/NetworkInit.ets"),
            "export class NetWorkInit {}",
        )
        .unwrap();

        let module_path_map = HvigorModulePathMap::new(
            "phone",
            &phone_root,
            &[("@waimai/network".to_string(), network_root.clone())],
        );
        let mut state = HvigorModuleSourceState::default();
        state.sources.insert(
            index_id.clone(),
            HvigorCollectedModuleSource {
                code: "export { NetWorkInit } from \"./NetWorkInit\";"
                    .to_string()
                    .into(),
            },
        );
        state.sources.insert(
            init_id.clone(),
            HvigorCollectedModuleSource {
                code: "export class NetWorkInit {}".to_string().into(),
            },
        );
        record_resolved_import(&mut state, &index_id, "./NetWorkInit", &init_id);
        let facts = ModuleGraphFacts {
            full_module_ids: vec![index_id.clone(), init_id.clone()],
            full_module_is_commonjs: FxHashMap::default(),
        };
        let out_dir = app_root
            .path()
            .join("cache")
            .join("default@CompileArkTS")
            .join("esmodule")
            .join("release");
        let runtime_packages = runtime_packages(&[(
            "@waimai/network",
            "@waimai/network",
            "",
            "",
            "1.0.32",
            false,
        )]);

        let files = HvigorModuleSourceWriter::new(
            "com.example.hmosworld",
            "phone",
            project_root,
            &module_path_map,
            &runtime_packages,
            &[],
            true,
            &facts,
            facts.full_module_ids.clone(),
            state,
            out_dir,
        )
        .write()
        .unwrap();

        let index_file = files
            .iter()
            .find(|file| file.module_id == index_id)
            .expect("network index module source");
        assert_eq!(
            index_file.imported_record_names,
            vec!["&@waimai/network/src/main/ets/network/NetworkInit&1.0.32".to_string()]
        );
        let output = std::fs::read_to_string(&index_file.path).unwrap();
        assert!(
            output.contains(
                "\"@normalized:N&&&@waimai/network/src/main/ets/network/NetworkInit&1.0.32\""
            ),
            "{output}"
        );
    }

    #[test]
    fn module_source_writer_preserves_standard_source_root_index_record() {
        let app_root = tempfile::tempdir().unwrap();
        let project_root = app_root.path();
        let phone_root = project_root.join("products/phone");
        let msi_root = project_root.join("oh_modules/.ohpm/@msi+msi@12.59.400/oh_modules/@msi/msi");
        let package_index_id = msi_root.join("Index.ets").to_string_lossy().into_owned();
        let source_root_index_id = msi_root
            .join("src/main/ets/index.ets")
            .to_string_lossy()
            .into_owned();
        std::fs::create_dir_all(msi_root.join("src/main/ets")).unwrap();
        std::fs::write(
            msi_root.join("oh-package.json5"),
            r#"{
                "name": "@msi/msi",
                "version": "12.59.400",
                "main": "Index.ets"
            }"#,
        )
        .unwrap();
        std::fs::write(
            msi_root.join("Index.ets"),
            "export * from \"./src/main/ets/index\";",
        )
        .unwrap();
        std::fs::write(
            msi_root.join("src/main/ets/index.ets"),
            "export class MSI {}",
        )
        .unwrap();

        let module_path_map = HvigorModulePathMap::new(
            "phone",
            &phone_root,
            &[("@msi/msi".to_string(), msi_root.clone())],
        );
        let mut state = HvigorModuleSourceState::default();
        state.sources.insert(
            package_index_id.clone(),
            HvigorCollectedModuleSource {
                code: "export * from \"./src/main/ets/index\";".to_string().into(),
            },
        );
        state.sources.insert(
            source_root_index_id.clone(),
            HvigorCollectedModuleSource {
                code: "export class MSI {}".to_string().into(),
            },
        );
        record_resolved_import(
            &mut state,
            &package_index_id,
            "./src/main/ets/index",
            &source_root_index_id,
        );
        let facts = ModuleGraphFacts {
            full_module_ids: vec![package_index_id.clone(), source_root_index_id.clone()],
            full_module_is_commonjs: FxHashMap::default(),
        };
        let out_dir = app_root
            .path()
            .join("cache")
            .join("default@CompileArkTS")
            .join("esmodule")
            .join("release");
        let runtime_packages =
            runtime_packages(&[("@msi/msi", "@msi/msi", "", "", "12.59.400", false)]);

        let files = HvigorModuleSourceWriter::new(
            "com.example.hmosworld",
            "phone",
            project_root,
            &module_path_map,
            &runtime_packages,
            &[],
            true,
            &facts,
            facts.full_module_ids.clone(),
            state,
            out_dir,
        )
        .write()
        .unwrap();

        let source_root_index_file = files
            .iter()
            .find(|file| file.module_id == source_root_index_id)
            .expect("source root index module source");
        assert_eq!(
            source_root_index_file.record_name,
            "&@msi/msi/src/main/ets/index&12.59.400"
        );
        let package_index_file = files
            .iter()
            .find(|file| file.module_id == package_index_id)
            .expect("package index module source");
        assert_eq!(
            package_index_file.imported_record_names,
            vec!["&@msi/msi/src/main/ets/index&12.59.400".to_string()]
        );
        let output = std::fs::read_to_string(&package_index_file.path).unwrap();
        assert!(
            output.contains("\"@normalized:N&&&@msi/msi/src/main/ets/index&12.59.400\""),
            "{output}"
        );
    }

    #[test]
    fn module_source_writer_preserves_package_source_index_subpath_record() {
        let app_root = tempfile::tempdir().unwrap();
        let project_root = app_root.path();
        let phone_root = project_root.join("products/phone");
        let privacy_root = project_root
            .join("oh_modules/.ohpm/@meituan+privacy@0.3.20/oh_modules/@meituan/privacy");
        let privacy_impl_root = project_root
            .join("oh_modules/.ohpm/@meituan+privacy_impl@1.0.0/oh_modules/@meituan/privacy_impl");
        let importer_id = privacy_impl_root
            .join("src/main/ets/monitor/BabelMonitor.ets")
            .to_string_lossy()
            .into_owned();
        let def_index_id = privacy_root
            .join("src/main/ets/def/index.ets")
            .to_string_lossy()
            .into_owned();
        std::fs::create_dir_all(privacy_root.join("src/main/ets/def")).unwrap();
        std::fs::create_dir_all(privacy_impl_root.join("src/main/ets/monitor")).unwrap();
        std::fs::write(
            privacy_root.join("oh-package.json5"),
            r#"{
                "name": "@meituan/privacy",
                "version": "0.3.20",
                "main": "Index.ets"
            }"#,
        )
        .unwrap();
        std::fs::write(
            privacy_impl_root.join("oh-package.json5"),
            r#"{
                "name": "@meituan/privacy_impl",
                "version": "1.0.0",
                "main": "Index.ets"
            }"#,
        )
        .unwrap();
        std::fs::write(
            privacy_impl_root.join("src/main/ets/monitor/BabelMonitor.ets"),
            "import { DefFilterResult } from \"@meituan/privacy/src/main/ets/def\";",
        )
        .unwrap();
        std::fs::write(
            privacy_root.join("src/main/ets/def/index.ets"),
            "export class DefFilterResult {}",
        )
        .unwrap();

        let module_path_map = HvigorModulePathMap::new(
            "phone",
            &phone_root,
            &[
                ("@meituan/privacy".to_string(), privacy_root.clone()),
                (
                    "@meituan/privacy_impl".to_string(),
                    privacy_impl_root.clone(),
                ),
            ],
        );
        let mut state = HvigorModuleSourceState::default();
        state.sources.insert(
            importer_id.clone(),
            HvigorCollectedModuleSource {
                code: "import { DefFilterResult } from \"@meituan/privacy/src/main/ets/def\";"
                    .to_string()
                    .into(),
            },
        );
        state.sources.insert(
            def_index_id.clone(),
            HvigorCollectedModuleSource {
                code: "export class DefFilterResult {}".to_string().into(),
            },
        );
        record_resolved_import(
            &mut state,
            &importer_id,
            "@meituan/privacy/src/main/ets/def",
            &def_index_id,
        );
        let facts = ModuleGraphFacts {
            full_module_ids: vec![importer_id.clone(), def_index_id.clone()],
            full_module_is_commonjs: FxHashMap::default(),
        };
        let out_dir = app_root
            .path()
            .join("cache")
            .join("default@CompileArkTS")
            .join("esmodule")
            .join("release");
        let runtime_packages = runtime_packages(&[
            (
                "@meituan/privacy",
                "@meituan/privacy",
                "",
                "",
                "0.3.20",
                false,
            ),
            (
                "@meituan/privacy_impl",
                "@meituan/privacy_impl",
                "",
                "",
                "1.0.0",
                false,
            ),
        ]);

        let files = HvigorModuleSourceWriter::new(
            "com.example.hmosworld",
            "phone",
            project_root,
            &module_path_map,
            &runtime_packages,
            &[],
            true,
            &facts,
            facts.full_module_ids.clone(),
            state,
            out_dir,
        )
        .write()
        .unwrap();

        let def_index_file = files
            .iter()
            .find(|file| file.module_id == def_index_id)
            .expect("privacy def index module source");
        assert_eq!(
            def_index_file.record_name,
            "&@meituan/privacy/src/main/ets/def/index&0.3.20"
        );
        let importer_file = files
            .iter()
            .find(|file| file.module_id == importer_id)
            .expect("privacy impl importer module source");
        assert_eq!(
            importer_file.imported_record_names,
            vec!["&@meituan/privacy/src/main/ets/def/index&0.3.20".to_string()]
        );
        let output = std::fs::read_to_string(&importer_file.path).unwrap();
        assert!(
            output.contains("\"@normalized:N&&&@meituan/privacy/src/main/ets/def/index&0.3.20\""),
            "{output}"
        );
    }

    #[test]
    fn module_source_writer_uses_local_dependency_module_name_for_cache_path() {
        let app_root = tempfile::tempdir().unwrap();
        let project_root = app_root.path();
        let phone_root = project_root.join("products/phone");
        let runtime_root = project_root.join("repo/repos/_msi_runtime/msi_runtime");
        let runtime_api_id = runtime_root
            .join("src/main/ets/ApiPortal.ts")
            .to_string_lossy()
            .into_owned();
        std::fs::create_dir_all(runtime_root.join("src/main/ets")).unwrap();
        std::fs::write(
            runtime_root.join("oh-package.json5"),
            r#"{
                "name": "@msi/runtime",
                "version": "12.59.400",
                "main": "Index.ets"
            }"#,
        )
        .unwrap();
        std::fs::write(
            runtime_root.join("src/main/ets/ApiPortal.ts"),
            "export class ApiPortal {}",
        )
        .unwrap();

        let mut module_path_map = HvigorModulePathMap::new(
            "phone",
            &phone_root,
            &[
                ("phone".to_string(), phone_root.clone()),
                ("msi_runtime".to_string(), runtime_root.clone()),
            ],
        );
        module_path_map
            .add_module_record_roots(&[("@msi/runtime".to_string(), runtime_root.clone())]);

        let mut state = HvigorModuleSourceState::default();
        state.sources.insert(
            runtime_api_id.clone(),
            HvigorCollectedModuleSource {
                code: "export class ApiPortal {}".to_string().into(),
            },
        );
        let facts = ModuleGraphFacts {
            full_module_ids: vec![runtime_api_id.clone()],
            full_module_is_commonjs: FxHashMap::default(),
        };
        let out_dir = app_root
            .path()
            .join("cache")
            .join("default@CompileArkTS")
            .join("esmodule")
            .join("debug");
        let mut runtime_packages = runtime_packages(&[("phone", "phone", "", "", "", false)]);
        runtime_packages.packages.insert(
            "@msi/runtime".to_string(),
            BundleRuntimePackageContext {
                package_name: "@msi/runtime".to_string(),
                version: "12.59.400".to_string(),
                entry_path: "Index.ets".to_string(),
                ..BundleRuntimePackageContext::default()
            },
        );

        let files = HvigorModuleSourceWriter::new(
            "com.example.hmosworld",
            "phone",
            project_root,
            &module_path_map,
            &runtime_packages,
            &[],
            true,
            &facts,
            facts.full_module_ids.clone(),
            state,
            out_dir.clone(),
        )
        .write()
        .unwrap();

        let api_file = files
            .iter()
            .find(|file| file.module_id == runtime_api_id)
            .expect("local dependency source");
        assert_eq!(
            api_file.record_name,
            "&@msi/runtime/src/main/ets/ApiPortal&12.59.400"
        );
        assert_eq!(
            api_file.path,
            out_dir.join("msi_runtime/src/main/ets/ApiPortal.ts")
        );
        assert_eq!(
            api_file.source_file,
            "phone|@msi/runtime|12.59.400|src/main/ets/ApiPortal.ts"
        );
        assert_eq!(api_file.package_name.as_deref(), Some("@msi/runtime"));
    }

    #[test]
    fn module_source_writer_uses_actual_package_file_path_for_package_root_record() {
        let app_root = tempfile::tempdir().unwrap();
        let project_root = app_root.path();
        let phone_root = project_root.join("products/phone");
        let imageknife_root = project_root
            .join("oh_modules/.ohpm/@meituan+imageknife@5.0.8/oh_modules/@meituan/imageknife");
        let entry_id = phone_root
            .join("src/main/ets/abilitystage/EntryAbilityStage.ets")
            .to_string_lossy()
            .into_owned();
        let imageknife_id = imageknife_root
            .join("index.ets")
            .to_string_lossy()
            .into_owned();
        std::fs::create_dir_all(&imageknife_root).unwrap();
        std::fs::write(
            imageknife_root.join("oh-package.json5"),
            r#"{
                "name": "@meituan/imageknife",
                "version": "5.0.8",
                "main": "Index.ets"
            }"#,
        )
        .unwrap();
        std::fs::write(
            imageknife_root.join("index.ets"),
            "export class ImageKnife {}",
        )
        .unwrap();

        let module_path_map = HvigorModulePathMap::new(
            "phone",
            &phone_root,
            &[("@meituan/imageknife".to_string(), imageknife_root.clone())],
        );
        let mut state = HvigorModuleSourceState::default();
        state.sources.insert(
            entry_id.clone(),
            HvigorCollectedModuleSource {
                code: "import { ImageKnife } from \"@meituan/imageknife\";"
                    .to_string()
                    .into(),
            },
        );
        state.sources.insert(
            imageknife_id.clone(),
            HvigorCollectedModuleSource {
                code: "export class ImageKnife {}".to_string().into(),
            },
        );
        record_resolved_import(&mut state, &entry_id, "@meituan/imageknife", &imageknife_id);
        let facts = ModuleGraphFacts {
            full_module_ids: vec![entry_id.clone(), imageknife_id.clone()],
            full_module_is_commonjs: FxHashMap::default(),
        };
        let out_dir = app_root
            .path()
            .join("cache")
            .join("default@CompileArkTS")
            .join("esmodule")
            .join("release");
        let mut packages = FxHashMap::default();
        packages.insert(
            "phone".to_string(),
            BundleRuntimePackageContext {
                package_name: "phone".to_string(),
                entry_path: "src/main/".to_string(),
                ..BundleRuntimePackageContext::default()
            },
        );
        packages.insert(
            "@meituan/imageknife".to_string(),
            BundleRuntimePackageContext {
                package_name: "@meituan/imageknife".to_string(),
                version: "5.0.8".to_string(),
                entry_path: "Index.ets".to_string(),
                ..BundleRuntimePackageContext::default()
            },
        );
        let runtime_packages = BundleRuntimePackageInfo { packages };

        let files = HvigorModuleSourceWriter::new(
            "com.example.hmosworld",
            "phone",
            project_root,
            &module_path_map,
            &runtime_packages,
            &[],
            true,
            &facts,
            facts.full_module_ids.clone(),
            state,
            out_dir,
        )
        .write()
        .unwrap();

        let imageknife_file = files
            .iter()
            .find(|file| file.module_id == imageknife_id)
            .expect("imageknife package entry module source");
        assert_eq!(
            imageknife_file.record_name,
            "&@meituan/imageknife/index&5.0.8"
        );
        let entry_file = files
            .iter()
            .find(|file| file.module_id == entry_id)
            .expect("entry module source");
        assert_eq!(
            entry_file.imported_record_names,
            vec!["&@meituan/imageknife/index&5.0.8".to_string()]
        );
        let output = std::fs::read_to_string(&entry_file.path).unwrap();
        assert!(
            output.contains("\"@normalized:N&&&@meituan/imageknife/index&5.0.8\""),
            "{output}"
        );
    }

    #[test]
    fn module_source_writer_rewrites_bare_package_imports_from_rolldown_graph() {
        let app_root = tempfile::tempdir().unwrap();
        let project_root = app_root.path();
        let phone_root = project_root.join("products/phone");
        let log_root =
            project_root.join("oh_modules/.ohpm/@meituan+log@0.0.18/oh_modules/@meituan/log");
        let entry_id = phone_root
            .join("src/main/ets/abilitystage/EntryAbilityStage.ets")
            .to_string_lossy()
            .into_owned();
        let log_id = log_root.join("Index.ets").to_string_lossy().into_owned();
        std::fs::create_dir_all(&log_root).unwrap();
        std::fs::write(
            log_root.join("oh-package.json5"),
            r#"{
                "name": "@meituan/log",
                "version": "0.0.18",
                "main": "Index.ets"
            }"#,
        )
        .unwrap();
        std::fs::write(log_root.join("Index.ets"), "export class MLog {}").unwrap();

        let module_path_map = HvigorModulePathMap::new(
            "phone",
            &phone_root,
            &[("@meituan/log".to_string(), log_root.clone())],
        );
        let mut state = HvigorModuleSourceState::default();
        state.sources.insert(
            entry_id.clone(),
            HvigorCollectedModuleSource {
                code: "import { MLog } from \"@meituan/log\";".to_string().into(),
            },
        );
        state.sources.insert(
            log_id.clone(),
            HvigorCollectedModuleSource {
                code: "export class MLog {}".to_string().into(),
            },
        );
        state
            .resolved_imports
            .entry(entry_id.clone())
            .or_default()
            .insert("@meituan/log".to_string(), log_id.clone());
        let facts = ModuleGraphFacts {
            full_module_ids: vec![entry_id.clone(), log_id.clone()],
            full_module_is_commonjs: FxHashMap::default(),
        };
        let out_dir = app_root
            .path()
            .join("cache")
            .join("default@CompileArkTS")
            .join("esmodule")
            .join("release");
        let mut packages = FxHashMap::default();
        packages.insert(
            "phone".to_string(),
            BundleRuntimePackageContext {
                package_name: "phone".to_string(),
                entry_path: "src/main/".to_string(),
                ..BundleRuntimePackageContext::default()
            },
        );
        packages.insert(
            "@meituan/log".to_string(),
            BundleRuntimePackageContext {
                package_name: "@meituan/log".to_string(),
                version: "0.0.18".to_string(),
                entry_path: "Index.ets".to_string(),
                ..BundleRuntimePackageContext::default()
            },
        );
        let runtime_packages = BundleRuntimePackageInfo { packages };

        let files = HvigorModuleSourceWriter::new(
            "com.example.hmosworld",
            "phone",
            project_root,
            &module_path_map,
            &runtime_packages,
            &[],
            true,
            &facts,
            facts.full_module_ids.clone(),
            state,
            out_dir,
        )
        .write()
        .unwrap();

        let entry_file = files
            .iter()
            .find(|file| file.module_id == entry_id)
            .expect("entry module source");
        assert_eq!(
            entry_file.imported_record_names,
            vec!["&@meituan/log/Index&0.0.18".to_string()]
        );
        let output = std::fs::read_to_string(&entry_file.path).unwrap();
        assert!(
            output.contains("\"@normalized:N&&&@meituan/log/Index&0.0.18\""),
            "{output}"
        );
    }

    #[test]
    fn module_source_writer_uses_actual_entry_file_when_pkg_context_entry_path_is_empty() {
        let app_root = tempfile::tempdir().unwrap();
        let project_root = app_root.path();
        let phone_root = project_root.join("products/phone");
        let pako_root = project_root.join("oh_modules/.ohpm/pako@2.1.0/oh_modules/pako");
        let entry_id = phone_root
            .join("src/main/ets/entryability/EntryAbility.ets")
            .to_string_lossy()
            .into_owned();
        let pako_id = pako_root
            .join("dist/pako.esm.mjs")
            .to_string_lossy()
            .into_owned();
        std::fs::create_dir_all(pako_root.join("dist")).unwrap();
        std::fs::write(
            pako_root.join("oh-package.json5"),
            r#"{
                "name": "pako",
                "version": "2.1.0",
                "module": "./dist/pako.esm.mjs",
                "exports": {
                    ".": {
                        "import": "./dist/pako.esm.mjs"
                    }
                }
            }"#,
        )
        .unwrap();
        std::fs::write(pako_root.join("dist/pako.esm.mjs"), "export const zip = 1;").unwrap();

        let module_path_map = HvigorModulePathMap::new(
            "phone",
            &phone_root,
            &[("pako".to_string(), pako_root.clone())],
        );
        let mut state = HvigorModuleSourceState::default();
        state.sources.insert(
            entry_id.clone(),
            HvigorCollectedModuleSource {
                code: "import { zip } from \"pako\";".to_string().into(),
            },
        );
        state.sources.insert(
            pako_id.clone(),
            HvigorCollectedModuleSource {
                code: "export const zip = 1;".to_string().into(),
            },
        );
        state
            .resolved_imports
            .entry(entry_id.clone())
            .or_default()
            .insert("pako".to_string(), pako_id.clone());
        let facts = ModuleGraphFacts {
            full_module_ids: vec![entry_id.clone(), pako_id.clone()],
            full_module_is_commonjs: FxHashMap::default(),
        };
        let out_dir = app_root
            .path()
            .join("cache")
            .join("default@CompileArkTS")
            .join("esmodule")
            .join("release");
        let mut packages = FxHashMap::default();
        packages.insert(
            "phone".to_string(),
            BundleRuntimePackageContext {
                package_name: "phone".to_string(),
                entry_path: "src/main/".to_string(),
                ..BundleRuntimePackageContext::default()
            },
        );
        packages.insert(
            "pako".to_string(),
            BundleRuntimePackageContext {
                package_name: "pako".to_string(),
                version: "2.1.0".to_string(),
                entry_path: String::new(),
                ..BundleRuntimePackageContext::default()
            },
        );
        let runtime_packages = BundleRuntimePackageInfo { packages };

        let files = HvigorModuleSourceWriter::new(
            "com.example.hmosworld",
            "phone",
            project_root,
            &module_path_map,
            &runtime_packages,
            &[],
            true,
            &facts,
            facts.full_module_ids.clone(),
            state,
            out_dir,
        )
        .write()
        .unwrap();

        let entry_file = files
            .iter()
            .find(|file| file.module_id == entry_id)
            .expect("entry module source");
        let pako_file = files
            .iter()
            .find(|file| file.module_id == pako_id)
            .expect("pako module source");
        assert_eq!(pako_file.record_name, "&pako/dist/pako.esm&2.1.0");
        assert_eq!(
            entry_file.imported_record_names,
            vec!["&pako/dist/pako.esm&2.1.0".to_string()]
        );
        let output = std::fs::read_to_string(&entry_file.path).unwrap();
        assert!(
            output.contains("\"@normalized:N&&&pako/dist/pako.esm&2.1.0\""),
            "{output}"
        );
    }

    #[test]
    fn module_source_writer_matches_index_source_for_empty_main_package_root_record() {
        let app_root = tempfile::tempdir().unwrap();
        let project_root = app_root.path();
        let phone_root = project_root.join("products/phone");
        let subscribe_root = project_root.join(
            "oh_modules/.ohpm/@meituan+card_subscribe@1.0.32/oh_modules/@meituan/card_subscribe",
        );
        let entry_id = phone_root
            .join("src/main/ets/form/FormAbility.ets")
            .to_string_lossy()
            .into_owned();
        let subscribe_id = subscribe_root
            .join("index.ets")
            .to_string_lossy()
            .into_owned();
        std::fs::create_dir_all(&subscribe_root).unwrap();
        std::fs::write(
            subscribe_root.join("oh-package.json5"),
            r#"{
                "name": "@meituan/card_subscribe",
                "version": "1.0.32",
                "main": "",
                "metadata": {
                    "sourceRoots": ["./src/main"],
                    "useNormalizedOHMUrl": true
                }
            }"#,
        )
        .unwrap();
        std::fs::write(subscribe_root.join("index.ets"), "export const status = 1;").unwrap();

        let module_path_map = HvigorModulePathMap::new(
            "phone",
            &phone_root,
            &[(
                "@meituan/card_subscribe".to_string(),
                subscribe_root.clone(),
            )],
        );
        let mut state = HvigorModuleSourceState::default();
        state.sources.insert(
            entry_id.clone(),
            HvigorCollectedModuleSource {
                code: "import { status } from \"@meituan/card_subscribe\";"
                    .to_string()
                    .into(),
            },
        );
        state.sources.insert(
            subscribe_id.clone(),
            HvigorCollectedModuleSource {
                code: "export const status = 1;".to_string().into(),
            },
        );
        state
            .resolved_imports
            .entry(entry_id.clone())
            .or_default()
            .insert("@meituan/card_subscribe".to_string(), subscribe_id.clone());
        let facts = ModuleGraphFacts {
            full_module_ids: vec![entry_id.clone(), subscribe_id.clone()],
            full_module_is_commonjs: FxHashMap::default(),
        };
        let out_dir = app_root
            .path()
            .join("cache")
            .join("default@CompileArkTS")
            .join("esmodule")
            .join("release");
        let mut packages = FxHashMap::default();
        packages.insert(
            "phone".to_string(),
            BundleRuntimePackageContext {
                package_name: "phone".to_string(),
                entry_path: "src/main/".to_string(),
                ..BundleRuntimePackageContext::default()
            },
        );
        packages.insert(
            "@meituan/card_subscribe".to_string(),
            BundleRuntimePackageContext {
                package_name: "@meituan/card_subscribe".to_string(),
                version: "1.0.32".to_string(),
                entry_path: String::new(),
                ..BundleRuntimePackageContext::default()
            },
        );
        let runtime_packages = BundleRuntimePackageInfo { packages };

        let files = HvigorModuleSourceWriter::new(
            "com.example.hmosworld",
            "phone",
            project_root,
            &module_path_map,
            &runtime_packages,
            &[],
            true,
            &facts,
            facts.full_module_ids.clone(),
            state,
            out_dir,
        )
        .write()
        .unwrap();

        let entry_file = files
            .iter()
            .find(|file| file.module_id == entry_id)
            .expect("entry module source");
        let subscribe_file = files
            .iter()
            .find(|file| file.module_id == subscribe_id)
            .expect("subscribe package source");
        assert_eq!(
            subscribe_file.record_name,
            "&@meituan/card_subscribe/index&1.0.32"
        );
        assert_eq!(
            entry_file.imported_record_names,
            vec!["&@meituan/card_subscribe/index&1.0.32".to_string()]
        );
        let output = std::fs::read_to_string(&entry_file.path).unwrap();
        assert!(
            output.contains("\"@normalized:N&&&@meituan/card_subscribe/index&1.0.32\""),
            "{output}"
        );
    }

    #[test]
    fn module_source_output_path_uses_transform_language_metadata() {
        let ownership = HvigorChunkSource::source_module(
            "entry".to_string(),
            "entry".to_string(),
            String::new(),
            "ets/Zip.ts".to_string(),
        );
        let output = HvigorModuleSourceOutputPath::new(
            Path::new("/workspace/cache"),
            Path::new("/workspace"),
            &ownership,
            "entry",
            "ets/Zip.ts",
            "/workspace/entry/src/main/ets/Zip.ts",
            true,
        )
        .path()
        .expect("output path");

        assert_eq!(
            output,
            Path::new("/workspace/cache/entry/src/main/ets/Zip.js")
        );
        assert_eq!(
            ownership.file_info_source_file("entry", "1.0.0", false, true),
            "entry|entry|1.0.0|src/main/ets/Zip.js"
        );

        let mjs_ownership = HvigorChunkSource::source_module(
            "entry".to_string(),
            "entry".to_string(),
            String::new(),
            "ets/vendor/pako.esm.mjs".to_string(),
        );
        assert_eq!(
            mjs_ownership.file_info_source_file("entry", "1.0.0", false, false),
            "entry|entry|1.0.0|src/main/ets/vendor/pako.esm.js"
        );

        let cjs_ownership = HvigorChunkSource::source_module(
            "entry".to_string(),
            "entry".to_string(),
            String::new(),
            "ets/vendor/index.cjs".to_string(),
        );
        assert_eq!(
            cjs_ownership.file_info_source_file("entry", "1.0.0", false, false),
            "entry|entry|1.0.0|src/main/ets/vendor/index.js"
        );
    }

    #[test]
    fn module_source_writer_matches_latest_module_mode_final_newline_contract() {
        assert_eq!(
            HvigorModuleSourceWriter::finalize_source_text(
                "export const value = 1;".to_string(),
                false,
            ),
            "export const value = 1;\n"
        );
        assert_eq!(
            HvigorModuleSourceWriter::finalize_source_text(
                concat!(
                    "//@ts-noCheck\r\n",
                    "export const value = runtime();\r\n",
                    "//# sourceMappingURL=ModuleInfo.js.map"
                )
                .to_string(),
                true,
            ),
            concat!(
                "//@ts-noCheck\r\n",
                "export const value = runtime();\r\n",
                "//# sourceMappingURL=ModuleInfo.js.map"
            )
        );
    }

    #[test]
    fn module_source_state_keeps_lazy_compile_arkts_override_after_rolldown_scan() {
        let module_id = "/workspace/oh_modules/@scope/speech/FaceSpeech.ets";
        let standard_source = r#"import textToSpeech from "@hms.ai.textToSpeech";
textToSpeech.createEngine({});"#;
        let lazy_source = r#"import lazy textToSpeech from "@hms.ai.textToSpeech";
textToSpeech.createEngine({});"#;
        let state = HvigorModuleSourceState::from_rolldown_scan(
            rolldown::ModuleSourceScanOutput {
                warnings: Vec::new(),
                modules: vec![rolldown::ModuleSourceScanModule {
                    id: module_id.into(),
                    source: standard_source.into(),
                    module_type: rolldown_common::ModuleType::Ts,
                    exports_kind: rolldown_common::ExportsKind::Esm,
                    imports: vec![rolldown::ModuleSourceScanImport {
                        module_request: "@hms.ai.textToSpeech".to_string(),
                        resolved_id: None,
                        kind: rolldown_common::ImportKind::Import,
                        importer_span: rolldown::ModuleSourceScanSpan { start: 0, end: 52 },
                    }],
                }],
            },
            HvigorModuleTransformSnapshot {
                module_source_overrides: FxHashMap::from_iter([(
                    module_id.to_string(),
                    Arc::<str>::from(lazy_source),
                )]),
                source_maps: FxHashMap::default(),
                javascript_module_ids: FxHashSet::default(),
                shared_module_ids: FxHashSet::default(),
            },
        );

        let stored_source = state.sources.get(module_id).expect("module source");
        assert_eq!(stored_source.code.as_str(), lazy_source);
        assert_eq!(
            HvigorModuleRequestSpecifiers::new(stored_source.code.as_str(), module_id).collect(),
            vec!["@hms.ai.textToSpeech".to_string()]
        );

        let imports = HvigorChunkImportMap::from_iter([(
            "@hms.ai.textToSpeech".to_string(),
            "@hms:ai.textToSpeech".to_string(),
        )]);
        let rewritten =
            HvigorModuleSourceImportRewriter::new(stored_source.code.as_str(), module_id, &imports)
                .rewrite();
        assert!(
            rewritten.contains("import lazy textToSpeech from \"@hms:ai.textToSpeech\";"),
            "the final CompileArkTS writer must normalize lazy SDK requests: {rewritten}"
        );
    }

    #[test]
    fn module_source_state_prefers_ets_transform_snapshot_over_rolldown_printer_output() {
        let module_id = "/workspace/entry/src/main/ets/model/Local.ets";
        let ets_transform_source = "import type { Local } from './LocalTypes';\nexport const value: Local = {} as Local;\n";
        let rolldown_scanned_source = "import type { Local } from \"./LocalTypes\";\nexport const value: Local = {} as Local;\n";
        let state = HvigorModuleSourceState::from_rolldown_scan(
            rolldown::ModuleSourceScanOutput {
                warnings: Vec::new(),
                modules: vec![rolldown::ModuleSourceScanModule {
                    id: module_id.into(),
                    source: rolldown_scanned_source.into(),
                    module_type: rolldown_common::ModuleType::Ts,
                    exports_kind: rolldown_common::ExportsKind::Esm,
                    imports: Vec::new(),
                }],
            },
            HvigorModuleTransformSnapshot {
                module_source_overrides: FxHashMap::from_iter([(
                    module_id.to_string(),
                    Arc::<str>::from(ets_transform_source),
                )]),
                source_maps: FxHashMap::default(),
                javascript_module_ids: FxHashSet::default(),
                shared_module_ids: FxHashSet::default(),
            },
        );

        assert_eq!(
            state.sources.get(module_id).map(|source| source.code.as_str()),
            Some(ets_transform_source),
            "ModuleMode must materialize etsTransform output instead of Rolldown's parser-normalized source"
        );
    }

    #[test]
    fn coverage_retransform_replays_resolved_module_requests() {
        let module_id = PathBuf::from("workspace")
            .join("entry")
            .join("src")
            .join("main")
            .join("ets")
            .join("EntryAbility.ets");
        let transformed = r#"import UIAbility from "@ohos.app.ability.UIAbility";
import { helper } from "./helper";
export default class EntryAbility extends UIAbility {}"#;
        let materialized = r#"import UIAbility from "@ohos:app.ability.UIAbility";
import { helper } from "@bundle:com.example/entry/ets/helper";
export default class EntryAbility extends UIAbility {}"#;
        let instrumented = r#"import UIAbility from "@ohos.app.ability.UIAbility";
import { helper } from "./helper";
class BjcCov {}
export default class EntryAbility extends UIAbility {}"#;

        let plan = HvigorModuleRequestRewritePlan::from_baseline(
            module_id.to_string_lossy().as_ref(),
            transformed,
            materialized,
        )
        .expect("capture module request rewrite plan");
        let rewritten = plan.rewrite(instrumented);

        assert!(rewritten.contains("from \"@ohos:app.ability.UIAbility\""));
        assert!(rewritten.contains("from \"@bundle:com.example/entry/ets/helper\""));
        assert!(rewritten.contains("class BjcCov {}"));
    }

    #[test]
    fn module_source_writer_generates_hvigor_mock_config_and_origin_import() {
        let app_root = tempfile::tempdir().unwrap();
        let project_root = app_root.path();
        let module_root = project_root.join("entry");
        let dependency_root = project_root.join("oh_modules").join("service-lib");
        let src_root = module_root.join("src");
        let ets_root = src_root.join("main").join("ets");
        let main_source = ets_root.join("pages").join("Index.ets");
        let service_source = ets_root.join("service").join("Service.ets");
        let mock_root = src_root.join("mock");
        let mock_source = mock_root.join("ServiceMock.ets");
        let mock_config_path = mock_root.join("mock-config.json5");
        let dependency_main = dependency_root.join("index.ets");
        for path in [
            &main_source,
            &service_source,
            &mock_source,
            &mock_config_path,
            &dependency_main,
        ] {
            fs_err::create_dir_all(path.parent().unwrap()).unwrap();
        }
        fs_err::write(
            &main_source,
            "import { load } from \"../service/Service\"; export const value = load();",
        )
        .unwrap();
        fs_err::write(&service_source, "export const load = () => 1;").unwrap();
        fs_err::write(
            &mock_source,
            "import { load } from \"../main/ets/service/Service\"; export const mock = load;",
        )
        .unwrap();
        fs_err::write(
            &mock_config_path,
            r#"{
              "service/Service.ets": { "source": "src/mock/ServiceMock.ets" },
              "service-lib": { "source": "src/mock/ServiceMock.ets" }
            }"#,
        )
        .unwrap();
        fs_err::write(&dependency_main, "export const dependency = 1;").unwrap();
        fs_err::write(
            dependency_root.join("oh-package.json5"),
            r#"{ "name": "service-lib", "version": "1.0.0", "main": "index.ets" }"#,
        )
        .unwrap();

        let main_id = main_source.to_string_lossy().into_owned();
        let service_id = service_source.to_string_lossy().into_owned();
        let mock_id = mock_source.to_string_lossy().into_owned();
        let mut state = HvigorModuleSourceState::default();
        state.sources.insert(
            main_id.clone(),
            HvigorCollectedModuleSource {
                code: fs_err::read_to_string(&main_source).unwrap().into(),
            },
        );
        state.sources.insert(
            service_id.clone(),
            HvigorCollectedModuleSource {
                code: fs_err::read_to_string(&service_source).unwrap().into(),
            },
        );
        state.sources.insert(
            mock_id.clone(),
            HvigorCollectedModuleSource {
                code: fs_err::read_to_string(&mock_source).unwrap().into(),
            },
        );
        record_resolved_import(
            &mut state,
            &main_id,
            "../service/Service",
            &service_id,
        );
        record_resolved_import(
            &mut state,
            &mock_id,
            "../main/ets/service/Service",
            &service_id,
        );
        let facts = ModuleGraphFacts {
            full_module_ids: vec![main_id.clone(), service_id, mock_id.clone()],
            full_module_is_commonjs: FxHashMap::default(),
        };
        let module_path_map = HvigorModulePathMap::new(
            "entry",
            &module_root,
            &[
                ("entry".to_string(), module_root.clone()),
                ("service-lib".to_string(), dependency_root.clone()),
            ],
        );
        let runtime_packages = runtime_packages(&[(
            "entry",
            "entry",
            "com.example",
            "entry",
            "1.0.0",
            false,
        )]);
        let cache_root = project_root.join("cache");
        let transformed_output_path = project_root.join("intermediates").join("mock-config.json");
        let mock_options = BundleMockConfig {
            process: true,
            user_config_path: Some(mock_config_path.clone()),
            ets_source_root: ets_root,
            transformed_output_path: transformed_output_path.clone(),
            transformed_cache_path: cache_root.join("mock-config.json"),
            user_cache_path: cache_root.join("mock-config.json5"),
            dependencies: vec![crate::plan::BundleMockDependency {
                dependency_name: "service-lib".to_string(),
                package_name: "service-lib".to_string(),
                package_root: dependency_root,
                main_file_path: dependency_main,
            }],
        };
        let files = HvigorModuleSourceWriter::new(
            "com.example",
            "entry",
            project_root,
            &module_path_map,
            &runtime_packages,
            &[],
            false,
            &facts,
            facts.full_module_ids.clone(),
            state,
            cache_root.clone(),
        )
        .with_mock_config(Some(&mock_options))
        .write()
        .unwrap();

        let main_output = fs_err::read_to_string(
            &files.iter().find(|file| file.module_id == main_id).unwrap().path,
        )
        .unwrap();
        let mock_output = fs_err::read_to_string(
            &files.iter().find(|file| file.module_id == mock_id).unwrap().path,
        )
        .unwrap();
        assert!(!main_output.contains(".origin"), "{main_output}");
        assert!(mock_output.contains(".origin\""), "{mock_output}");

        let transformed = serde_json::from_slice::<serde_json::Value>(
            &fs_err::read(&transformed_output_path).unwrap(),
        )
        .unwrap();
        let entries = transformed.as_object().unwrap();
        let (origin_url, origin_entry) = entries
            .iter()
            .find(|(key, _)| key.ends_with(".origin"))
            .expect("origin mock entry");
        let actual_url = origin_url.strip_suffix(".origin").unwrap();
        assert_eq!(origin_entry["source"], actual_url);
        assert!(entries[actual_url]["source"]
            .as_str()
            .is_some_and(|source| source.contains("src/mock/ServiceMock")));
        assert!(
            entries
                .keys()
                .any(|url| url.contains("service-lib") && !url.ends_with(".origin")),
            "dependency mock keys must be eagerly converted like collectMockConfigInfo: {entries:?}"
        );
        assert_eq!(
            fs_err::read(&mock_options.transformed_cache_path).unwrap(),
            fs_err::read(&transformed_output_path).unwrap()
        );
        assert_eq!(
            fs_err::read(&mock_options.user_cache_path).unwrap(),
            fs_err::read(&mock_config_path).unwrap()
        );
    }

    #[test]
    fn compile_arkts_rewrites_lazy_native_import_in_ts_dependency_like_sdk() {
        let module_id = "/workspace/oh_modules/mtmediaplayer/src/main/ts/MTMediaPlayerCache.ts";
        let source = r#"import lazy nativePlayer from "libaemonplayer.so";
nativePlayer.cacheInit();"#;

        assert_eq!(
            HvigorModuleRequestSpecifiers::new(source, module_id).collect(),
            vec!["libaemonplayer.so".to_string()],
            "SDK ModuleSourceFile.processTransformedTsModuleRequest recognizes lazy imports in .ts"
        );

        let mut packages = FxHashMap::default();
        packages.insert(
            "libaemonplayer.so".to_string(),
            BundleRuntimePackageContext {
                package_name: "libaemonplayer.so".to_string(),
                is_so: true,
                ..BundleRuntimePackageContext::default()
            },
        );
        let runtime_packages = BundleRuntimePackageInfo { packages };
        let normalized_request = HvigorBytecodeHarImports::new(&runtime_packages, &[], true)
            .ohm_url_for_request("libaemonplayer.so")
            .expect("native package context lookup")
            .expect("native package context must produce an OHM URL");
        let imports = HvigorChunkImportMap::from_iter([(
            "libaemonplayer.so".to_string(),
            normalized_request,
        )]);
        let rewritten =
            HvigorModuleSourceImportRewriter::new(source, module_id, &imports).rewrite();

        assert!(
            rewritten
                .contains("import lazy nativePlayer from \"@normalized:Y&&&libaemonplayer.so&\";"),
            "CompileArkTS must retain lazy semantics and rewrite native requests: {rewritten}"
        );
    }

    #[test]
    fn module_source_rewriter_preserves_commonjs_relative_require_like_hvigor() {
        let module_id = "/workspace/oh_modules/protobufjs/src/converter.js";
        let source = r#"var Enum = require("./enum");
import staticValue from "./static";
const lazyValue = import("./lazy");"#;
        let imports = HvigorChunkImportMap::from_iter([
            (
                "./enum".to_string(),
                "@package:pkg_modules/protobufjs/src/enum".to_string(),
            ),
            (
                "./static".to_string(),
                "@package:pkg_modules/protobufjs/src/static".to_string(),
            ),
            (
                "./lazy".to_string(),
                "@package:pkg_modules/protobufjs/src/lazy".to_string(),
            ),
        ]);

        assert_eq!(
            HvigorModuleRequestSpecifiers::new(source, module_id).collect(),
            vec!["./static".to_string(), "./lazy".to_string()],
            "latest ets-loader ModuleSourceFile only visits static ESM and import() requests"
        );
        let rewritten =
            HvigorModuleSourceImportRewriter::new(source, module_id, &imports).rewrite();

        assert!(
            rewritten.contains("require(\"./enum\")"),
            "CommonJS relative require must retain runtime resolution: {rewritten}"
        );
        assert!(
            !rewritten.contains("@package:pkg_modules/protobufjs/src/enum"),
            "CommonJS require must not become an es2abc package request: {rewritten}"
        );
        assert!(
            rewritten.contains("@package:pkg_modules/protobufjs/src/static")
                && rewritten.contains("@package:pkg_modules/protobufjs/src/lazy"),
            "hvigor still rewrites static ESM and dynamic import requests: {rewritten}"
        );
    }

    #[test]
    fn module_source_rewriter_preserves_js_request_quote_style_like_latest_hvigor() {
        let module_id = "/workspace/oh_modules/@ohos/lottie/src/main/js/modules/full.js";
        let source = r#"import lottie from './main';
export { registerRenderer } from "../renderers/main";
const lazyRenderer = import('./renderers/lazy');"#;
        let imports = HvigorChunkImportMap::from_iter([
            (
                "./main".to_string(),
                "@package:pkg_modules/@ohos/lottie/src/main/js/modules/main".to_string(),
            ),
            (
                "../renderers/main".to_string(),
                "@package:pkg_modules/@ohos/lottie/src/main/js/renderers/main".to_string(),
            ),
            (
                "./renderers/lazy".to_string(),
                "@package:pkg_modules/@ohos/lottie/src/main/js/modules/renderers/lazy".to_string(),
            ),
        ]);

        let rewritten =
            HvigorModuleSourceImportRewriter::new(source, module_id, &imports).rewrite();

        assert_eq!(
            rewritten,
            r#"import lottie from '@package:pkg_modules/@ohos/lottie/src/main/js/modules/main';
export { registerRenderer } from "@package:pkg_modules/@ohos/lottie/src/main/js/renderers/main";
const lazyRenderer = import('@package:pkg_modules/@ohos/lottie/src/main/js/modules/renderers/lazy');"#,
            "latest ets-loader module_source_file.js::processJsModuleRequest reuses its captured quote delimiter"
        );
    }

    #[test]
    fn module_source_rewriter_uses_generated_double_quotes_for_transformed_ts_like_hvigor() {
        let module_id = "/workspace/entry/src/main/ets/pages/MainPage.ets";
        let source = r#"import page from './Page';
export { ViewModel } from '../model/ViewModel';
const lazyPage = import('./lazy/Page');"#;
        let imports = HvigorChunkImportMap::from_iter([
            (
                "./Page".to_string(),
                "@bundle:com.example/entry/ets/pages/Page".to_string(),
            ),
            (
                "../model/ViewModel".to_string(),
                "@bundle:com.example/entry/ets/model/ViewModel".to_string(),
            ),
            (
                "./lazy/Page".to_string(),
                "@bundle:com.example/entry/ets/pages/lazy/Page".to_string(),
            ),
        ]);

        let rewritten =
            HvigorModuleSourceImportRewriter::new(source, module_id, &imports).rewrite();

        assert_eq!(
            rewritten,
            r#"import page from "@bundle:com.example/entry/ets/pages/Page";
export { ViewModel } from "@bundle:com.example/entry/ets/model/ViewModel";
const lazyPage = import("@bundle:com.example/entry/ets/pages/lazy/Page");"#,
            "latest ets-loader ModuleSourceFile.processTransformedTsModuleRequest uses ts.factory.createStringLiteral for import, export, and import() requests"
        );
    }

    #[test]
    fn module_source_state_consumes_rolldown_compact_scan_facts() {
        let importer = "/workspace/oh_modules/@meituan/passport/RSAEncryptionUtils.ets";
        let resolved = "/workspace/oh_modules/@ohos/hypium/index.js";
        let cjs_module = "/workspace/oh_modules/@sgfe/flashbuy_price_harmony/build/index.cjs.js";
        let mut source_maps = FxHashMap::default();
        source_maps.insert(importer.to_string(), Arc::from("{\"version\":3}"));
        let state = HvigorModuleSourceState::from_rolldown_scan(
            rolldown::ModuleSourceScanOutput {
                warnings: Vec::new(),
                modules: vec![
                    rolldown::ModuleSourceScanModule {
                        id: importer.into(),
                        source: r#"import { HostType } from "@sgfe/flashbuy_price_harmony";
import hypium from "@ohos/hypium";
const dynamicHar = import("bytecode-har-dep");"#
                            .into(),
                        module_type: rolldown_common::ModuleType::Ts,
                        exports_kind: rolldown_common::ExportsKind::Esm,
                        imports: vec![
                            rolldown::ModuleSourceScanImport {
                                module_request: "@sgfe/flashbuy_price_harmony".to_string(),
                                resolved_id: Some(cjs_module.into()),
                                kind: rolldown_common::ImportKind::Import,
                                importer_span: rolldown::ModuleSourceScanSpan { start: 0, end: 62 },
                            },
                            rolldown::ModuleSourceScanImport {
                                module_request: "@ohos/hypium".to_string(),
                                resolved_id: Some(resolved.into()),
                                kind: rolldown_common::ImportKind::Import,
                                importer_span: rolldown::ModuleSourceScanSpan {
                                    start: 63,
                                    end: 91,
                                },
                            },
                            rolldown::ModuleSourceScanImport {
                                module_request: "bytecode-har-dep".to_string(),
                                resolved_id: None,
                                kind: rolldown_common::ImportKind::DynamicImport,
                                importer_span: rolldown::ModuleSourceScanSpan {
                                    start: 92,
                                    end: 142,
                                },
                            },
                        ],
                    },
                    rolldown::ModuleSourceScanModule {
                        id: cjs_module.into(),
                        source: "exports.HostType = void 0;".into(),
                        module_type: rolldown_common::ModuleType::Js,
                        exports_kind: rolldown_common::ExportsKind::CommonJs,
                        imports: Vec::new(),
                    },
                ],
            },
            HvigorModuleTransformSnapshot {
                module_source_overrides: FxHashMap::default(),
                source_maps,
                javascript_module_ids: FxHashSet::from_iter([importer.to_string()]),
                shared_module_ids: FxHashSet::default(),
            },
        );
        assert_eq!(
            state
                .resolved_imports
                .get("/workspace/oh_modules/@meituan/passport/RSAEncryptionUtils.ets")
                .and_then(|imports| imports.get("@sgfe/flashbuy_price_harmony"))
                .map(String::as_str),
            Some(cjs_module)
        );
        assert_eq!(
            state
                .resolved_imports
                .get("/workspace/oh_modules/@meituan/passport/RSAEncryptionUtils.ets")
                .and_then(|imports| imports.get("@ohos/hypium"))
                .map(String::as_str),
            Some(resolved)
        );
        assert_eq!(
            state
                .resolved_imports
                .get("/workspace/oh_modules/@meituan/passport/RSAEncryptionUtils.ets")
                .and_then(|imports| imports.get("bytecode-har-dep"))
                .map(String::as_str),
            Some("bytecode-har-dep")
        );
        assert_eq!(
            state
                .full_module_is_commonjs
                .get("/workspace/oh_modules/@meituan/passport/RSAEncryptionUtils.ets"),
            Some(&false)
        );
        assert_eq!(state.full_module_is_commonjs.get(cjs_module), Some(&true));
        assert!(state
            .source_maps
            .contains_key("/workspace/oh_modules/@meituan/passport/RSAEncryptionUtils.ets"));
        assert!(state.javascript_module_ids.contains(importer));
        assert_eq!(
            state
                .resolved_import_records
                .get("/workspace/oh_modules/@meituan/passport/RSAEncryptionUtils.ets")
                .map(Vec::len),
            Some(3)
        );
    }

    #[tokio::test]
    async fn rolldown_scan_preserves_variable_dynamic_import_without_request() {
        let temp = tempfile::tempdir().expect("tempdir");
        let build_profile = temp.path().join("BuildProfile.js");
        std::fs::write(
            &build_profile,
            r#"export default class BuildProfile {
}
BuildProfile.USE_ANT_NETWORK_LIB = false;
"#,
        )
        .expect("write build profile");
        let entry = temp.path().join("entry.js");
        std::fs::write(
            &entry,
            r#"import BuildProfile from "./BuildProfile.js";
const USE_ANT_NETWORK_LIB = BuildProfile.USE_ANT_NETWORK_LIB;
export class APDIDNetworkUtil {
    static async executeRPC(value) {
        if (USE_ANT_NETWORK_LIB) {
            let request = "@alipay/network";
            return import(request);
        }
        return value;
    }
}
"#,
        )
        .expect("write entry");

        let mut bundler = rolldown::Bundler::with_plugins(
            BundlerOptions {
                input: Some(vec![InputItem {
                    name: None,
                    import: entry.to_string_lossy().into_owned(),
                }]),
                cwd: Some(temp.path().to_path_buf()),
                format: Some(OutputFormat::Esm),
                transform: Some(BundlerTransformOptions {
                    typescript: Some(TypeScriptOptions::default()),
                    ..Default::default()
                }),
                treeshake: TreeshakeOptions::Boolean(
                    Bundler::module_source_scan_treeshake_enabled(),
                ),
                ..Default::default()
            },
            Vec::new(),
        )
        .expect("bundler");

        let scan = bundler.scan_module_sources().await.expect("scan");
        bundler.close().await.expect("close");

        let imports = scan
            .modules
            .iter()
            .flat_map(|module| module.imports.iter())
            .map(|import| import.module_request.as_str())
            .collect::<Vec<_>>();
        assert!(
            !imports.contains(&"@alipay/network"),
            "non-static dynamic import must not become a module request: {imports:?}"
        );
    }

    #[test]
    fn module_source_writer_rewrites_ohpm_imports_to_package_records() {
        let app_root = tempfile::tempdir().unwrap();
        let project_root = app_root.path();
        let phone_root = project_root.join("products/phone");
        let hmcore_root = project_root
            .join("oh_modules/.ohpm/@hw-agconnect+hmcore@1.0.6/oh_modules/@hw-agconnect/hmcore");
        let hypium_root =
            project_root.join("oh_modules/.ohpm/@ohos+hypium@1.0.11/oh_modules/@ohos/hypium");
        let entry_id = phone_root
            .join("src/main/ets/entryability/EntryAbility.ets")
            .to_string_lossy()
            .into_owned();
        let hmcore_id = hmcore_root.join("index.ts").to_string_lossy().into_owned();
        let hypium_id = hypium_root.join("index.js").to_string_lossy().into_owned();
        let logger_id = hmcore_root
            .join("src/main/ets/base/log/Logger.ts")
            .to_string_lossy()
            .into_owned();
        let router_id = hmcore_root
            .join("src/main/ets/components/RouterMap.ets")
            .to_string_lossy()
            .into_owned();
        std::fs::create_dir_all(&hmcore_root).unwrap();
        std::fs::write(
            hmcore_root.join("oh-package.json5"),
            r#"{
                "name": "@hw-agconnect/hmcore",
                "version": "1.0.6",
                "main": "index"
            }"#,
        )
        .unwrap();
        std::fs::write(hmcore_root.join("index.ts"), "export {};").unwrap();
        std::fs::create_dir_all(&hypium_root).unwrap();
        std::fs::write(
            hypium_root.join("oh-package.json5"),
            r#"{
                "name": "@ohos/hypium",
                "version": "1.0.11",
                "main": "index.js"
            }"#,
        )
        .unwrap();
        std::fs::write(
            hypium_root.join("index.js"),
            "export const DEFAULT = 'default';",
        )
        .unwrap();
        std::fs::create_dir_all(hmcore_root.join("src/main/ets/base/log")).unwrap();
        std::fs::write(
            hmcore_root.join("src/main/ets/base/log/Logger.ts"),
            "export class Logger {}",
        )
        .unwrap();
        std::fs::create_dir_all(hmcore_root.join("src/main/ets/components")).unwrap();
        std::fs::write(
            hmcore_root.join("src/main/ets/components/RouterMap.ets"),
            "@Component\nstruct RouterMap { build() {} }",
        )
        .unwrap();
        let module_path_map = HvigorModulePathMap::new(
            "phone",
            &phone_root,
            &[
                ("@hw-agconnect/hmcore".to_string(), hmcore_root.clone()),
                ("@ohos/hypium".to_string(), hypium_root.clone()),
            ],
        );
        let mut state = HvigorModuleSourceState::default();
        state.sources.insert(
            entry_id.clone(),
            HvigorCollectedModuleSource {
                code: [
                    "import { initialize } from \"@hw-agconnect/hmcore\";",
                    "import { DEFAULT } from \"@ohos/hypium\";",
                ]
                .join("\n")
                .into(),
            },
        );
        state.sources.insert(
            hmcore_id.clone(),
            HvigorCollectedModuleSource {
                code: [
                    "export { Logger } from \"./src/main/ets/base/log/Logger\";",
                    "export { RouterMap } from \"./src/main/ets/components/RouterMap\";",
                ]
                .join("\n")
                .into(),
            },
        );
        state.sources.insert(
            hypium_id.clone(),
            HvigorCollectedModuleSource {
                code: "export const DEFAULT = 'default';".to_string().into(),
            },
        );
        state.sources.insert(
            logger_id.clone(),
            HvigorCollectedModuleSource {
                code: "export class Logger {}".to_string().into(),
            },
        );
        state.sources.insert(
            router_id.clone(),
            HvigorCollectedModuleSource {
                code: "export class RouterMap {}".to_string().into(),
            },
        );
        state
            .resolved_imports
            .entry(entry_id.clone())
            .or_default()
            .insert("@hw-agconnect/hmcore".to_string(), hmcore_id.clone());
        state
            .resolved_imports
            .entry(entry_id.clone())
            .or_default()
            .insert("@ohos/hypium".to_string(), hypium_id.clone());
        state
            .resolved_imports
            .entry(hmcore_id.clone())
            .or_default()
            .insert(
                "./src/main/ets/base/log/Logger".to_string(),
                logger_id.clone(),
            );
        state
            .resolved_imports
            .entry(hmcore_id.clone())
            .or_default()
            .insert(
                "./src/main/ets/components/RouterMap".to_string(),
                router_id.clone(),
            );
        let facts = ModuleGraphFacts {
            full_module_ids: vec![
                entry_id.clone(),
                hmcore_id.clone(),
                hypium_id.clone(),
                logger_id.clone(),
                router_id.clone(),
            ],
            full_module_is_commonjs: FxHashMap::default(),
        };
        let out_dir = app_root
            .path()
            .join("cache")
            .join("default@CompileArkTS")
            .join("esmodule")
            .join("release");
        let runtime_packages = runtime_packages(&[
            ("phone", "phone", "", "", "", false),
            (
                "@hw-agconnect/hmcore",
                "@hw-agconnect/hmcore",
                "",
                "",
                "1.0.6",
                false,
            ),
            ("@ohos/hypium", "@ohos/hypium", "", "", "1.0.11", false),
        ]);

        let files = HvigorModuleSourceWriter::new(
            "com.example.hmosworld",
            "phone",
            project_root,
            &module_path_map,
            &runtime_packages,
            &[],
            true,
            &facts,
            facts.full_module_ids.clone(),
            state,
            out_dir,
        )
        .write()
        .unwrap();

        let entry_file = files
            .iter()
            .find(|file| file.module_id == entry_id)
            .expect("entry module source");
        let hmcore_file = files
            .iter()
            .find(|file| file.module_id == hmcore_id)
            .expect("hmcore module source");
        let hypium_file = files
            .iter()
            .find(|file| file.module_id == hypium_id)
            .expect("hypium module source");
        let logger_file = files
            .iter()
            .find(|file| file.module_id == logger_id)
            .expect("logger module source");
        let router_file = files
            .iter()
            .find(|file| file.module_id == router_id)
            .expect("router module source");
        assert_eq!(hmcore_file.record_name, "&@hw-agconnect/hmcore/index&1.0.6");
        assert_eq!(hypium_file.record_name, "&@ohos/hypium/index&1.0.11");
        assert_eq!(
            hmcore_file.source_file,
            "phone|@hw-agconnect/hmcore|1.0.6|index.ts"
        );
        assert_eq!(
            hypium_file.source_file,
            "phone|@ohos/hypium|1.0.11|index.js"
        );
        assert_eq!(
            entry_file.path,
            app_root
                .path()
                .join("cache")
                .join("default@CompileArkTS")
                .join("esmodule")
                .join("release")
                .join("phone")
                .join("src")
                .join("main")
                .join("ets")
                .join("entryability")
                .join("EntryAbility.ts")
        );
        assert_eq!(
            hmcore_file.path,
            app_root
                .path()
                .join("cache")
                .join("default@CompileArkTS")
                .join("esmodule")
                .join("release")
                .join("oh_modules")
                .join(".ohpm")
                .join("@hw-agconnect+hmcore@1.0.6")
                .join("oh_modules")
                .join("@hw-agconnect")
                .join("hmcore")
                .join("index.ts")
        );
        // `ModuleMode.processModuleInfos` computes packageName via
        // `getPkgModulesFilePkgName()`: the physical OHPM record root
        // (path.join("pkg_modules", pkgPath - projectRoot/packageDir)
        // with packageDir replaced by the constant "pkg_modules"),
        // not the logical package name. The logical name only appears in
        // the `phone|<pkg>|<version>|<entry>` source_file shape.
        // Evidence: SDK ets-loader `module/module_mode.js::getPkgModulesFilePkgName`.
        assert_eq!(
            hmcore_file.package_name.as_deref(),
            Some("pkg_modules/.ohpm/@hw-agconnect+hmcore@1.0.6/pkg_modules/@hw-agconnect/hmcore")
        );
        assert_eq!(
            logger_file.record_name,
            "&@hw-agconnect/hmcore/src/main/ets/base/log/Logger&1.0.6"
        );
        assert_eq!(
            router_file.record_name,
            "&@hw-agconnect/hmcore/src/main/ets/components/RouterMap&1.0.6"
        );
        assert_eq!(
            router_file.source_file,
            "phone|@hw-agconnect/hmcore|1.0.6|src/main/ets/components/RouterMap.ts"
        );
        assert_eq!(
            router_file.path,
            app_root
                .path()
                .join("cache")
                .join("default@CompileArkTS")
                .join("esmodule")
                .join("release")
                .join("oh_modules")
                .join(".ohpm")
                .join("@hw-agconnect+hmcore@1.0.6")
                .join("oh_modules")
                .join("@hw-agconnect")
                .join("hmcore")
                .join("src")
                .join("main")
                .join("ets")
                .join("components")
                .join("RouterMap.ts")
        );
        let entry_output = std::fs::read_to_string(&entry_file.path).unwrap();
        assert!(
            entry_output.contains("\"@normalized:N&&&@hw-agconnect/hmcore/index&1.0.6\""),
            "{entry_output}"
        );
        assert!(
            entry_output.contains("\"@normalized:N&&&@ohos/hypium/index&1.0.11\""),
            "{entry_output}"
        );
        let hmcore_output = std::fs::read_to_string(&hmcore_file.path).unwrap();
        assert!(
            hmcore_output.contains(
                "\"@normalized:N&&&@hw-agconnect/hmcore/src/main/ets/base/log/Logger&1.0.6\""
            ),
            "{hmcore_output}"
        );
        assert!(
            hmcore_output.contains(
                "\"@normalized:N&&&@hw-agconnect/hmcore/src/main/ets/components/RouterMap&1.0.6\""
            ),
            "{hmcore_output}"
        );
    }
}
