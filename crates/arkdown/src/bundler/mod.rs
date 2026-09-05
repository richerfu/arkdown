//! Core bundler for hvigor-style ESMODULE builds.
//!
//! hvigor disables Rollup generateBundle for Stage-model ESMODULE builds and
//! hands transformed module sources to CompileArkTS/es2abc.  This module mirrors
//! that shape: Rolldown owns resolve/load/transform/parse/semantic scan, while
//! arkdown consumes the collected module graph and writes the CompileArkTS module
//! source cache.  It does not generate or write bundled JavaScript chunks on the
//! production path.

mod bytecode_har_imports;
mod crossplatform_collection;
mod import_rewriter;
mod input_plan;
mod module_path_map;
mod module_source_id;
mod module_source_state;
mod module_source_writer;
mod transform_collector;
mod types;

#[cfg(test)]
mod import_rewriter_tests;
#[cfg(test)]
mod module_source_state_tests;
#[cfg(test)]
mod module_source_writer_tests;
#[cfg(test)]
mod transform_collector_tests;

pub use crossplatform_collection::CrossplatformCollectionWriter;
pub use types::{
    BundleOutput, HvigorHarImportees, HvigorModuleRequestRewritePlan, InsightIntentModuleSource,
    PreloadSystemSoTask, SourceMapInputFingerprint,
};

pub(crate) use bytecode_har_imports::HvigorBytecodeHarImports;
pub(crate) use import_rewriter::{HvigorModuleRequestSpecifiers, HvigorModuleSourceImportRewriter};
pub(crate) use input_plan::RolldownInputPlan;
pub(crate) use module_path_map::{HvigorChunkSource, HvigorChunkSourceKind, HvigorModulePathMap};
pub(crate) use module_source_id::{
    HvigorModuleSourceFileAttribution, HvigorModuleSourceId, HvigorModuleSourceOutputPath,
};
pub(crate) use module_source_state::{HvigorModuleSourceState, HvigorSourceSpan, ModuleGraphFacts};
pub(crate) use module_source_writer::{HvigorModuleSourceWriter, HvigorModuleSourceWriterInput};
#[cfg(feature = "memory-diagnostics")]
pub(crate) use transform_collector::MemoryTransformDiagnosticPlugin;
pub(crate) use transform_collector::{
    HvigorModuleTransformCollector, HvigorModuleTransformSnapshot, HvigorModuleTransformStore,
};

use rustc_hash::{FxHashMap, FxHashSet};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;

use arkdown_cache::CacheFingerprint;
use arkdown_core::{ArkdownError, HvigorModuleSourceFile};
use arkdown_ets_transform::RolldownEtsTransformPlan;
use arkdown_path::{lexical_normalize, AbsPath, ArtifactPathSerializer};
use rolldown::{
    BundlerOptions, ModuleSourceScanImport, ModuleSourceScanModule, ModuleSourceScanOutput,
    ModuleSourceScanSpan, OutputFormat,
};
use rolldown_common::{
    BundleMode, BundlerTransformOptions, ChunkingContext, CodeSplittingMode, ExperimentalOptions,
    ManualCodeSplittingOptions, MatchGroup, MatchGroupName, MatchGroupTest, ModuleTable,
    ModuleType as RdModuleType, SanitizeFilename, ScanMode, TreeshakeOptions, TypeScriptOptions,
};
use rolldown_plugin::__inner::SharedPluginable;

use crate::chunk_module_request::HvigorChunkModuleRequestProcessor;
use crate::diagnostics::RolldownBuildDiagnostics;
use crate::manual_chunks::{ChunkName, ManualChunkPolicy};
use crate::output;
use crate::plan::{BundlePlan, SourceMapPolicy};
use crate::plugins::ets_transform::EtsTransformPlugin;

type DynamicChunkNameFuture =
    Pin<Box<dyn Future<Output = anyhow::Result<Option<String>>> + Send + 'static>>;
type DynamicChunkName = Arc<dyn Fn(&str, &ChunkingContext) -> DynamicChunkNameFuture + Send + Sync>;

/// The top-level bundler for one OHOS module build.
///
/// Orchestrates the full pipeline via rolldown with OHOS-specific plugins.
pub struct Bundler {
    pub plugins: Vec<SharedPluginable>,
    ets_transform_plan: Option<Arc<RolldownEtsTransformPlan>>,
    ets_transform_plugin: Option<Arc<EtsTransformPlugin>>,
    graph_session: Option<RolldownGraphSession>,
}

struct RolldownGraphSession {
    bundler: rolldown::Bundler,
    transform_store: HvigorModuleTransformStore,
    transform_snapshot: HvigorModuleTransformSnapshot,
}

impl Bundler {
    /// Create a new `Bundler` with the given configuration and plugin list.
    pub fn new(plugins: Vec<SharedPluginable>) -> Self {
        Bundler {
            plugins,
            ets_transform_plan: None,
            ets_transform_plugin: None,
            graph_session: None,
        }
    }

    pub fn with_ets_transform_plan(mut self, plan: Arc<RolldownEtsTransformPlan>) -> Self {
        self.ets_transform_plan = Some(plan);
        self
    }

    pub fn with_ets_transform_plugin(mut self, plugin: Arc<EtsTransformPlugin>) -> Self {
        self.ets_transform_plugin = Some(plugin);
        self
    }

    pub fn update_ets_transform_plan(&mut self, plan: Arc<RolldownEtsTransformPlan>) {
        self.ets_transform_plan = Some(plan);
    }

    /// Close a retained Rolldown graph and run its `closeBundle` hooks.
    pub async fn close(&mut self) -> Result<(), ArkdownError> {
        let Some(mut session) = self.graph_session.take() else {
            return Ok(());
        };
        session.bundler.close().await.map_err(|error| {
            ArkdownError::Bundler(RolldownBuildDiagnostics::new(error).into_message())
        })
    }

    /// Run the full bundle pipeline and return the chunk outputs.
    ///
    /// The output remains ESM so the Rust ABC backend compiles the same
    /// CompileArkTS-stage module shape that hvigor sends to es2abc.
    pub async fn bundle(&mut self, plan: &BundlePlan) -> Result<BundleOutput, ArkdownError> {
        self.bundle_with_graph_cache(plan, None).await
    }

    /// Incrementally rescan the changed module ids while retaining Rolldown's
    /// parsed graph/AST snapshot in this typed build-session owner.
    pub async fn bundle_incremental(
        &mut self,
        plan: &BundlePlan,
        changed_files: &[PathBuf],
    ) -> Result<BundleOutput, ArkdownError> {
        self.bundle_with_graph_cache(plan, Some(changed_files))
            .await
    }

    async fn bundle_with_graph_cache(
        &mut self,
        plan: &BundlePlan,
        changed_files: Option<&[PathBuf]>,
    ) -> Result<BundleOutput, ArkdownError> {
        let bundle_start = Instant::now();
        plan.validate()?;
        let input_plan = RolldownInputPlan::from_bundle_plan(plan);
        if let Some(plugin) = self.ets_transform_plugin.as_ref() {
            plugin.begin_build();
        }

        // ── 2. Pass plugins directly to rolldown ─────────────────────────────
        let transform_store = self
            .graph_session
            .as_ref()
            .map(|session| session.transform_store.clone())
            .unwrap_or_default();
        let mut rd_plugins: Vec<SharedPluginable> = self.plugins.clone();
        let source_collector =
            Arc::new(HvigorModuleTransformCollector::new(transform_store.clone()))
                as SharedPluginable;
        // CompileArkTS consumes the result after every Rollup transform hook,
        // including package plugins ordered after the SDK adapters. The
        // collector is the Rust equivalent of that post-transform handoff,
        // so it must remain the final ordinary transform observer.
        rd_plugins.push(source_collector);
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

        // Hvigor 6.24.3 `arkts-pack.ts#generateConfig` uses
        // `treeshake: buildMode === RELEASE`. Keep the decision on BundlePlan
        // so the project override and the build-mode default cannot diverge.
        let treeshake_enabled = plan.treeshake_enabled();
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
            experimental: changed_files.map(|_| ExperimentalOptions {
                incremental_build: Some(true),
                ..ExperimentalOptions::default()
            }),
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
        let (module_source_scan, transform_snapshot) = if let Some(changed_files) = changed_files {
            let had_graph = self.graph_session.is_some();
            if !had_graph {
                let bundler =
                    rolldown::Bundler::with_plugins(options, rd_plugins).map_err(|e| {
                        ArkdownError::Bundler(RolldownBuildDiagnostics::new(e).into_message())
                    })?;
                self.graph_session = Some(RolldownGraphSession {
                    bundler,
                    transform_store: transform_store.clone(),
                    transform_snapshot: HvigorModuleTransformSnapshot::default(),
                });
            }
            if let (Some(plugin), Some(plan)) = (
                self.ets_transform_plugin.as_ref(),
                self.ets_transform_plan.as_ref(),
            ) {
                plugin.update_plan(Arc::clone(plan));
            }
            let session = self
                .graph_session
                .as_mut()
                .expect("graph session initialized above");
            let scan_mode = if had_graph {
                ScanMode::Partial(
                    changed_files
                        .iter()
                        .map(|path| {
                            ArtifactPathSerializer::path_to_slash_string(path)
                                .map(Into::into)
                                .map_err(|error| {
                                    ArkdownError::Bundler(format!(
                                        "incremental module id cannot be serialized: {error}"
                                    ))
                                })
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                )
            } else {
                ScanMode::Full
            };
            let bundle_mode = if had_graph {
                BundleMode::IncrementalBuild
            } else {
                BundleMode::IncrementalFullBuild
            };
            let module_source_scan = session
                .bundler
                .with_cached_bundle_experimental(bundle_mode, async |bundle| {
                    let mut output = bundle.scan_modules(scan_mode).await?;
                    let warnings = std::mem::take(&mut output.warnings);
                    Ok(Self::module_source_scan_output(
                        &output.module_table,
                        warnings,
                    ))
                })
                .await
                .map_err(|e| {
                    ArkdownError::Bundler(RolldownBuildDiagnostics::new(e).into_message())
                })?;
            let update = session.transform_store.finish();
            let active_module_ids = module_source_scan
                .modules
                .iter()
                .map(|module| module.id.to_string())
                .collect();
            session
                .transform_snapshot
                .merge_incremental(update, &active_module_ids);
            (module_source_scan, session.transform_snapshot.clone())
        } else {
            let mut rd_bundler =
                rolldown::Bundler::with_plugins(options, rd_plugins).map_err(|e| {
                    ArkdownError::Bundler(RolldownBuildDiagnostics::new(e).into_message())
                })?;
            let module_source_scan = rd_bundler.scan_module_sources().await.map_err(|e| {
                ArkdownError::Bundler(RolldownBuildDiagnostics::new(e).into_message())
            })?;
            rd_bundler.close().await.map_err(|e| {
                ArkdownError::Bundler(RolldownBuildDiagnostics::new(e).into_message())
            })?;
            (module_source_scan, transform_store.finish())
        };
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
        let mut emitted_warnings = FxHashSet::default();
        for warning in &module_source_scan.warnings {
            let kind = warning.kind().to_string();
            if kind == "SHIMMED_EXPORT" {
                continue;
            }
            let message = format!("{kind}: {warning}");
            let rendered = warning.id().map_or_else(
                || message.clone(),
                |module_id| format!("{module_id}: {message}"),
            );
            if emitted_warnings.insert(rendered.clone()) {
                tracing::warn!("{rendered}");
            }
        }

        // ── 5. Materialize the hvigor CompileArkTS module-source handoff ─────
        let module_source_start = Instant::now();
        let mut module_source_state =
            HvigorModuleSourceState::from_rolldown_scan(module_source_scan, transform_snapshot);
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
        let insight_intent_sources = Self::insight_intent_sources(
            module_source_state.take_insight_intent_sources(),
            &input_plan.module_path_map,
            &plan.input.runtime_packages,
            plan.input.module_name.as_str(),
            plan.use_normalized_ohm_url,
        )?;
        let crossplatform_collections = module_source_state.take_crossplatform_collections();
        let module_source_write_start = Instant::now();
        #[cfg(feature = "memory-diagnostics")]
        crate::memory_diagnostics::dump("bundle:before-module-source-write");
        let module_source_files = HvigorModuleSourceWriter::new(HvigorModuleSourceWriterInput {
            bundle_name: plan.bundle_name.as_str(),
            module_name: plan.input.module_name.as_str(),
            project_root: &plan.input.project_root,
            module_path_map: &input_plan.module_path_map,
            runtime_packages: &plan.input.runtime_packages,
            byte_code_har_packages: &plan.input.byte_code_har_packages,
            use_normalized_ohm_url: plan.use_normalized_ohm_url,
            module_graph_facts: &module_graph_facts,
            module_source_ids,
            state: module_source_state,
            output_dir: plan
                .output
                .module_source_output_dir
                .clone()
                .ok_or_else(|| {
                    ArkdownError::Bundler(
                        "hvigor module source output dir is required for ModuleMode compilation"
                            .to_string(),
                    )
                })?,
        })
        .with_current_byte_code_har(plan.input.byte_code_har)
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

        if let Some(plugin) = self.ets_transform_plugin.as_ref() {
            let mut active_module_ids = module_source_files
                .iter()
                .map(|source| source.module_id.clone())
                .chain(module_imports.keys().cloned())
                .collect::<Vec<_>>();
            active_module_ids.sort();
            active_module_ids.dedup();
            plugin.commit_cache(&module_imports, active_module_ids);
        }

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
            insight_intent_sources,
            crossplatform_collections,
        })
    }

    fn module_source_scan_output(
        module_table: &ModuleTable,
        warnings: Vec<rolldown_error::BuildDiagnostic>,
    ) -> ModuleSourceScanOutput {
        let module_id_by_idx = module_table
            .modules
            .iter()
            .map(|module| module.id().as_arc_str().clone())
            .collect::<oxc_index::IndexVec<rolldown_common::ModuleIdx, arcstr::ArcStr>>();
        let modules = module_table
            .modules
            .iter()
            .filter_map(|module| {
                let module = module.as_normal()?;
                let imports = module
                    .ecma_view
                    .import_records
                    .iter()
                    .map(|record| ModuleSourceScanImport {
                        module_request: record.module_request.to_string(),
                        resolved_id: record
                            .resolved_module
                            .and_then(|idx| module_id_by_idx.get(idx).cloned()),
                        kind: record.kind,
                        importer_span: ModuleSourceScanSpan {
                            start: record.importer_span.start,
                            end: record.importer_span.end,
                        },
                    })
                    .collect();
                Some(ModuleSourceScanModule {
                    id: module.id.as_arc_str().clone(),
                    source: module.ecma_view.source.clone(),
                    module_type: module.module_type.clone(),
                    exports_kind: module.ecma_view.exports_kind,
                    imports,
                })
            })
            .collect();
        ModuleSourceScanOutput { modules, warnings }
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

    fn insight_intent_sources(
        sources: FxHashMap<String, arkdown_ets_transform::InsightIntentSourceMetadata>,
        module_path_map: &HvigorModulePathMap,
        runtime_packages: &crate::plan::BundleRuntimePackageInfo,
        module_name: &str,
        use_normalized_ohm_url: bool,
    ) -> Result<Vec<InsightIntentModuleSource>, ArkdownError> {
        if sources.is_empty() {
            return Ok(Vec::new());
        }
        if !use_normalized_ohm_url {
            if sources.values().all(|metadata| metadata.is_empty()) {
                let mut sources = sources.into_iter().collect::<Vec<_>>();
                sources.retain(|(_, metadata)| metadata.has_obfuscation_keep());
                sources.sort_by(|left, right| left.0.cmp(&right.0));
                return Ok(sources
                    .into_iter()
                    .map(|(module_id, metadata)| InsightIntentModuleSource {
                        source_file_path: Self::canonical_project_root(Path::new(&module_id)),
                        module_id,
                        decorator_file: String::new(),
                        metadata,
                    })
                    .collect());
            }
            return Err(ArkdownError::Bundler(
                "[10111027] InsightIntent failed to generate a standard OHM URL; enable useNormalizedOHMUrl"
                    .to_string(),
            ));
        }

        let mut sources = sources.into_iter().collect::<Vec<_>>();
        sources.sort_by(|left, right| left.0.cmp(&right.0));
        sources
            .into_iter()
            .map(|(module_id, mut metadata)| {
                let ownership = module_path_map.source_file_for_entry(&module_id)?;
                let decorator_file = ownership.to_normalized_request(runtime_packages, module_name)?;
                for intent in &mut metadata.intents {
                    intent.data.insert(
                        "decoratorFile".to_string(),
                        serde_json::Value::String(decorator_file.clone()),
                    );
                    if intent.data.get("decoratorType").and_then(serde_json::Value::as_str)
                        == Some("@InsightIntentPage")
                    {
                        let page_path = intent
                            .data
                            .get("pagePath")
                            .and_then(serde_json::Value::as_str)
                            .ok_or_else(|| {
                                ArkdownError::Bundler(format!(
                                    "[10110017] InsightIntent pagePath is missing in {module_id}"
                                ))
                            })?;
                        let package_root = module_path_map
                            .package_root_for_entry(&module_id)
                            .ok_or_else(|| {
                                ArkdownError::Bundler(format!(
                                    "[10110017] cannot resolve InsightIntent package root for {module_id}"
                                ))
                            })?;
                        let page_source = package_root
                            .join("src")
                            .join("main")
                            .join(page_path)
                            .with_extension("ets");
                        if !page_source.is_file() {
                            return Err(ArkdownError::Bundler(format!(
                                "[10110017] PagePath in @InsightIntentPage does not match the actual page path: {}",
                                page_source.display()
                            )));
                        }
                        let page_ownership = module_path_map
                            .source_file_for_entry(page_source.to_string_lossy().as_ref())?;
                        let normalized_page = page_ownership
                            .to_normalized_request(runtime_packages, module_name)?;
                        intent.data.insert(
                            "pagePath".to_string(),
                            serde_json::Value::String(normalized_page),
                        );
                    }
                }
                for entity in &mut metadata.entities {
                    entity.data.insert(
                        "decoratorFile".to_string(),
                        serde_json::Value::String(decorator_file.clone()),
                    );
                }
                Ok(InsightIntentModuleSource {
                    source_file_path: Self::canonical_project_root(Path::new(&module_id)),
                    module_id,
                    decorator_file,
                    metadata,
                })
            })
            .collect()
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
                    content_hash: CacheFingerprint::content_hash(source.as_ref()),
                })
            })
            .collect::<Vec<_>>();
        fingerprints.sort_by(|left, right| left.key.cmp(&right.key));
        fingerprints.dedup_by(|left, right| left.key == right.key);
        fingerprints
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
        _module_graph_facts: &ModuleGraphFacts,
    ) -> FxHashMap<String, String> {
        plan.input
            .source_map_packages
            .dependency_package_versions
            .clone()
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
        let vendors_name_fn: DynamicChunkName =
            Arc::new(|module_id: &str, ctx: &ChunkingContext| {
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
