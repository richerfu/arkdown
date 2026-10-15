//! Native TypeScript project semantic checking for the CompileArkTS root set.
//!
//! Upstream evidence: `ets_checker.ts::serviceChecker` creates one program
//! from `rootFileNames`, resolves package modules, then obtains syntactic and
//! semantic diagnostics before the source transform runs.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::Instant,
};

use anyhow::Result;
use arkdown_core::{ArkTscTarget, ArkdownConfig, HarArtifactType, ModuleConfig, OhosModuleType};
use arkdown_ets_checker::{
    NativeAnnotationConstant, NativeAnnotationInfo, NativeAnnotationTransformInfo,
    NativeArktsTransformTypeFacts, NativeArktsTransformTypeQuery, NativeCompilerOptions,
    NativeDiagnostic, NativeOhSdkCheckPlugin, NativeOhSdkClassCheckPlugin, NativeOhSdkConfig,
    NativeOhSdkPluginHost, NativeProcessedSource, NativeProgram, NativeProgramFileChanges,
    NativeProgramSourceGraph, NativeResolvedTypeIdentity, NativeSourcePreprocessor,
    NativeTypeQueryRange, NativeTypeSession,
};
use arkdown_ets_transform::{
    transform::component_meta::ComponentMetadata, AnnotationConstant, AnnotationImportDisposition,
    AnnotationInfo, AnnotationProperty, AnnotationUse, ArktsExtendSource, ArktsFileTypeFacts,
    ArktsSourceSemanticCollector, ArktsSourceSemanticInventory, ArktsTypeFactsSnapshot,
    CrossplatformSourceCollection, PropertyTypeFacts, ResolvedTypeIdentity,
    SpannedAnnotationImport, SpannedAnnotationInfo, SpannedAnnotationUse, SpannedPropertyTypeFacts,
    SpannedTypeIdentity,
};
use arkdown_path::lexical_normalize;
use arkdown_resolver::PackageResolutionPlan;
use arkdown_session::{ModuleCompileContext, SdkSnapshot, SourceContent, SourceStore};
use arkdown_watcher::{WatchEventKind, WatchInvalidationBatch};
use oxc_span::Span;
use rayon::prelude::*;
use rustc_hash::FxHashMap;
use serde::Serialize;

const ARKTS_LINTER_FAQ: &str = "For details about ArkTS syntax errors, see FAQs";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ArktsLinterOutput<'a> {
    category_info: &'static str,
    file_name: &'a Path,
    line: u32,
    character: u32,
    message_text: &'a str,
}

#[derive(Debug, Default)]
pub struct ProjectSemanticCheckReport {
    pub errors: usize,
    pub diagnostics: Vec<String>,
    pub warnings: Vec<String>,
    /// Implementation files reachable from the source-owned compiler roots.
    /// Standalone OXC checks consume this graph instead of scanning every file
    /// below the module directory.
    pub checked_sources: Vec<PathBuf>,
    pub type_facts: ArktsTypeFactsSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProjectSemanticMode {
    Full,
    TransformFactsOnly,
}

impl ProjectSemanticMode {
    fn checks_diagnostics(self) -> bool {
        matches!(self, Self::Full)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SemanticProgramLifetime {
    OneShot,
    Runtime,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct ProjectSemanticRuntimeKey {
    project_root: PathBuf,
    module_root: PathBuf,
    cache_path: PathBuf,
}

struct ProjectSemanticRuntimeEntry {
    sdk_plugin_callbacks: bool,
    session: NativeTypeSession,
    program: NativeProgram,
}

/// Long-lived owner of native TypeScript programs for one JS-CLI watch cycle.
///
/// Source owner: `rollup-plugin-ets-checker.ts::etsChecker`. The first
/// `buildStart` creates the language service; later watch builds reuse it and
/// request a new builder program before collecting linter and TypeScript
/// diagnostics again.
#[derive(Default)]
pub(crate) struct ProjectSemanticRuntime {
    sessions: FxHashMap<ProjectSemanticRuntimeKey, ProjectSemanticRuntimeEntry>,
}

impl ProjectSemanticRuntime {
    fn file_changes(invalidation: Option<&WatchInvalidationBatch>) -> NativeProgramFileChanges {
        let mut changes = NativeProgramFileChanges::default();
        let Some(invalidation) = invalidation else {
            return changes;
        };
        let mut final_events = BTreeMap::new();
        for event in &invalidation.raw_events {
            final_events.insert(event.path.clone(), event.kind);
        }
        for (path, kind) in final_events {
            match kind {
                WatchEventKind::Modified => changes.changed.push(path),
                WatchEventKind::Created => changes.created.push(path),
                WatchEventKind::Deleted => changes.deleted.push(path),
            }
        }
        changes
    }

    fn program<'a>(
        &'a mut self,
        key: ProjectSemanticRuntimeKey,
        project_root: &Path,
        prepared: &PreparedSemanticCheck,
        changes: &NativeProgramFileChanges,
    ) -> Result<(&'a mut NativeTypeSession, NativeProgram)> {
        let sdk_plugin_callbacks = prepared.sdk_plugin_host.is_some();
        let entry = self.sessions.remove(&key);
        let entry = match entry {
            Some(mut entry) if entry.sdk_plugin_callbacks == sdk_plugin_callbacks => {
                let program = entry.session.update_program_snapshot(
                    entry.program,
                    &prepared.roots,
                    &prepared.options,
                    prepared.sources.clone(),
                    changes,
                )?;
                ProjectSemanticRuntimeEntry { program, ..entry }
            }
            _ => {
                let mut session = NativeTypeSession::start(
                    project_root,
                    prepared.sources.clone(),
                    prepared.sdk_plugin_host.clone(),
                )?
                .with_source_preprocessor(Arc::new(ArktsCheckerSourcePreprocessor));
                let program = session.create_program(&prepared.roots, &prepared.options)?;
                ProjectSemanticRuntimeEntry {
                    sdk_plugin_callbacks,
                    session,
                    program,
                }
            }
        };
        self.sessions.insert(key.clone(), entry);
        let entry = self
            .sessions
            .get_mut(&key)
            .expect("semantic runtime entry was inserted above");
        Ok((&mut entry.session, entry.program.clone()))
    }

    pub(crate) fn close(&mut self) {
        self.sessions.clear();
    }
}

struct PreparedSemanticCheck {
    roots: Vec<PathBuf>,
    options: NativeCompilerOptions,
    sdk_plugin_host: Option<Arc<dyn NativeOhSdkPluginHost>>,
    sources: BTreeMap<PathBuf, Arc<str>>,
}

struct SemanticSourceInputs {
    ui_props: BTreeSet<String>,
    new_extend_names: BTreeSet<String>,
    sources: BTreeMap<PathBuf, Arc<str>>,
    semantic_inventories: BTreeMap<PathBuf, ArktsSourceSemanticInventory>,
}

// ets_checker.ts::getScriptSnapshot calls processContent only when the
// Program reads an implementation file. SourceSnapshot owns version caching.
struct ArktsCheckerSourcePreprocessor;
impl NativeSourcePreprocessor for ArktsCheckerSourcePreprocessor {
    fn preprocess(&self, _path: &Path, source: Arc<str>) -> NativeProcessedSource {
        let processed = ArktsExtendSource::new(&source, arkdown_core::ArktsLanguageVersion::V1_1);
        NativeProcessedSource {
            text: if processed.as_str() == source.as_ref() {
                Arc::clone(&source)
            } else {
                Arc::from(processed.as_str())
            },
            legacy_extend_names: processed.legacy_names().clone(),
            new_extend_names: processed.modern_names().clone(),
        }
    }
}

/// One compiler-owned file identity paired with the canonical startup
/// snapshot identity consumed by OXC/Rolldown.
///
/// TypeScript intentionally preserves the module path by which a source was
/// resolved, while `SourceStore` canonicalizes materialized OHPM packages.
/// The pair keeps requests addressed to the Program's identity without
/// duplicating source text or widening the source-owned compiler roots.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SemanticSourceIdentity {
    compiler_path: PathBuf,
    snapshot_path: PathBuf,
}

pub struct ProjectSemanticChecker<'a> {
    project_root: &'a Path,
    root_files: &'a [PathBuf],
    package_resolution: &'a PackageResolutionPlan,
    component_metadata: &'a ComponentMetadata,
    sdk_declarations: Vec<PathBuf>,
    source_store: Option<&'a SourceStore>,
    builtin_declarations: Vec<PathBuf>,
    compiler_options: NativeCompilerOptions,
    sdk_root: Option<PathBuf>,
    cache_path: Option<PathBuf>,
    check_declarations: bool,
    sdk_plugin_host: Option<Arc<dyn NativeOhSdkPluginHost>>,
    heap_reclaimer: Option<Arc<dyn crate::request::BuildHeapReclaimer>>,
    evict_source_texts: bool,
    crossplatform_dependencies: FxHashMap<(String, String), CrossplatformSourceCollection>,
}

impl<'a> ProjectSemanticChecker<'a> {
    pub fn with_sdk_options(mut self, sdk: &SdkSnapshot) -> Result<Self> {
        self.compiler_options.ets = Arc::clone(sdk.ets_options.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "ARKDOWN_ETS_CONFIG_MISSING: selected SDK has no ets-loader/tsconfig.json"
            )
        })?);
        self.compiler_options.compile_sdk_version = sdk.api_version;
        self.compiler_options.oh_sdk_configs = sdk
            .resolver_configs
            .iter()
            .map(|config| NativeOhSdkConfig {
                api_paths: config.api_paths.clone(),
            })
            .collect();
        self.compiler_options.oh_system_modules = sdk.system_modules.to_vec();
        self.compiler_options.oh_sdk_config_prefixes = sdk.module_prefixes.to_vec();
        self.compiler_options.oh_fallback_module_roots = sdk.fallback_module_roots.to_vec();
        self.compiler_options.oh_external_api_paths = sdk.external_api_paths.to_vec();
        self.compiler_options.oh_all_module_paths = sdk.all_module_paths.to_vec();
        self.compiler_options.oh_global_module_paths = sdk.global_module_paths.to_vec();
        self.compiler_options.oh_ark_ui_declaration_dirs = sdk.arkui_declaration_dirs.to_vec();
        self.compiler_options.oh_sdk_check_plugins = sdk
            .api_check_plugins
            .iter()
            .map(|plugin| NativeOhSdkCheckPlugin {
                os_name: plugin.os_name.clone(),
                tag: plugin.tag.clone(),
                plugin_type: plugin.plugin_type.clone(),
                path: plugin.path.clone(),
                function_name: plugin.function_name.clone(),
            })
            .collect();
        self.compiler_options.oh_sdk_class_check_plugins = sdk
            .api_class_check_plugins
            .iter()
            .map(|plugin| NativeOhSdkClassCheckPlugin {
                tag_name: plugin.tag_name.clone(),
                path: plugin.path.clone(),
                class_name: plugin.class_name.clone(),
            })
            .collect();
        self.sdk_root = Some(sdk.sdk_root.clone());
        self.crossplatform_dependencies = sdk
            .crossplatform_dependencies
            .iter()
            .map(|dependency| {
                (
                    (dependency.api_module.clone(), dependency.function.clone()),
                    CrossplatformSourceCollection {
                        modules: dependency.modules.to_vec(),
                        components: dependency.components.to_vec(),
                    },
                )
            })
            .collect();
        Ok(self)
    }

    pub fn new(
        project_root: &'a Path,
        root_files: &'a [PathBuf],
        package_resolution: &'a PackageResolutionPlan,
        component_metadata: &'a ComponentMetadata,
    ) -> Self {
        Self {
            project_root,
            root_files,
            package_resolution,
            component_metadata,
            sdk_declarations: Vec::new(),
            source_store: None,
            builtin_declarations: Vec::new(),
            compiler_options: NativeCompilerOptions::arkts11(project_root),
            sdk_root: None,
            cache_path: None,
            check_declarations: false,
            sdk_plugin_host: None,
            heap_reclaimer: None,
            evict_source_texts: false,
            crossplatform_dependencies: FxHashMap::default(),
        }
    }

    pub(crate) fn with_source_text_eviction(mut self, enabled: bool) -> Self {
        self.evict_source_texts = enabled;
        self
    }

    pub fn with_heap_reclaimer(
        mut self,
        reclaimer: Option<Arc<dyn crate::request::BuildHeapReclaimer>>,
    ) -> Self {
        self.heap_reclaimer = reclaimer;
        self
    }

    pub fn with_sdk_plugin_host(mut self, host: Option<Arc<dyn NativeOhSdkPluginHost>>) -> Self {
        self.sdk_plugin_host = host;
        self
    }

    /// `ets_checker.ts::setCompilerOptions` starts from CommonJS and switches
    /// to ES2020 only for the ESMODULE code-generation mode. This value is not
    /// the same as `process.env.compileMode`: `main.js` changes that separate
    /// source-model value to `moduleJson` when `aceModuleJsonPath` exists.
    pub fn with_compile_mode(mut self, compile_mode: &str) -> Self {
        // Keep a non-empty FA-model value until `with_module_config` observes a
        // module.json5 and applies the source-owned Stage-model sentinel.
        self.compiler_options.oh_compile_mode = compile_mode.to_string();
        self.compiler_options.module = if compile_mode == "esmodule" { 6 } else { 1 };
        self
    }

    /// `collectExternalApiChecker` passes the complete build `projectConfig`
    /// object to SDK class plugins. Reuse the same immutable snapshot exposed
    /// to compile plugins instead of reconstructing a narrowed checker view.
    pub fn with_sdk_plugin_project_config(
        mut self,
        project_config: &serde_json::Map<String, serde_json::Value>,
    ) -> Self {
        self.compiler_options.oh_sdk_plugin_project_config = Some(project_config.clone());
        self
    }

    /// Consume the already-resolved module configuration. Source owner:
    /// ets_checker.ts::setCompilerOptions/getTargetESVersionLib/isCompileJsHar.
    #[allow(
        clippy::manual_range_contains,
        reason = "preserve upstream numeric comparison semantics, including NaN"
    )]
    pub fn with_module_config(
        mut self,
        config: &ArkdownConfig,
        module: &ModuleConfig,
        module_context: &ModuleCompileContext,
        bundle_type: &str,
    ) -> Self {
        let build_option = &config.product.build_option;
        let ark_options = build_option.ark_options.as_ref();
        let target = ark_options
            .map(|options| options.tsc_config.target_es_version)
            .unwrap_or_default();
        let (target, lib) = match target {
            // Upstream deliberately pairs ES2017 output with the ES2020 lib.
            ArkTscTarget::Es2017 => (4, "lib.es2020.d.ts"),
            ArkTscTarget::Es2021 => (8, "lib.es2021.d.ts"),
        };
        self.compiler_options.target = target;
        // ets_checker.ts::getMaxFlowDepth checks range, not integer-ness.
        self.compiler_options.max_flow_depth = match ark_options
            .and_then(|options| options.tsc_config.max_flow_depth)
        {
            Some(value) if value < 2000.0 || value > 65535.0 => {
                tracing::warn!(
                    "ArkTS: Invalid maxFlowDepth for control flow analysis.The value of maxFlowDepth ranges from 2000 to 65535.\nIf the modification does not take effect, set maxFlowDepth to the default value."
                );
                2000.0
            }
            Some(value) => value,
            None => 2000.0,
        };
        self.compiler_options.lib = vec![lib.to_owned()];
        self.compiler_options.types = ark_options
            .map(|options| options.types.clone())
            .unwrap_or_default();
        self.compiler_options.module_root_path = module_context.module.root.clone();
        self.compiler_options.base_url = module_context.module.src_main.join("ets");
        self.compiler_options.paths_base_path = self.compiler_options.base_url.clone();
        self.compiler_options.oh_project_path = self.compiler_options.base_url.clone();
        self.compiler_options.oh_project_root_path = config.project_root.clone();
        self.compiler_options.oh_module_path = module_context.module.root.clone();
        if module_context.module.module_json.is_some() {
            // compiler/main.js::initProjectConfig and
            // ets_checker.ts::initEtsStandaloneCheckerConfig both set the
            // environment compile mode to this exact value when the Stage
            // module profile exists. system_api/api_check_utils.ts uses it to
            // choose FA-only rather than Stage-only API diagnostics.
            self.compiler_options.oh_compile_mode = "moduleJson".to_string();
        }
        self.compiler_options.oh_runtime_os = config.product.runtime_os.clone();
        self.compiler_options.oh_origin_compatible_sdk_version =
            config.product.compatible_sdk_version.clone();
        self.compiler_options.compatible_sdk_version =
            arkdown_config::build_profile::parse_api_version(
                &config.product.compatible_sdk_version,
            )
            .map(f64::from);
        self.compiler_options.compatible_sdk_version_stage = config
            .product
            .compatible_sdk_version_stage
            .clone()
            .unwrap_or_default();
        self.compiler_options.skip_oh_modules_lint = ark_options
            .and_then(|options| options.skip_oh_modules_lint)
            .unwrap_or(false);
        self.compiler_options.check_js = bundle_type == "atomicService";
        self.compiler_options.oh_bundle_type = bundle_type.to_string();
        self.compiler_options.is_compatible_version = self
            .compiler_options
            .compatible_sdk_version
            .is_none_or(|version| version < 10.0);
        if let Some(manifest) = module_context.module.module_json.as_ref() {
            self.compiler_options.oh_device_types = manifest.module.device_types.clone();
            self.compiler_options.oh_request_permissions = manifest
                .module
                .request_permissions
                .iter()
                .filter_map(|permission| permission.get("name").and_then(serde_json::Value::as_str))
                .map(str::to_string)
                .collect();
            let metadata = &manifest.module.metadata;
            self.compiler_options.need_do_ark_ts_linter = !metadata.iter().any(|entry| {
                entry.name == "Api11ArkTSCheck"
                    && entry.value.as_deref() == Some("SkipArkTSCheckInApi11")
            });
            let force_compatible = metadata.iter().any(|entry| {
                entry.name == "Api11ArkTSCheckMode"
                    && entry.value.as_deref() == Some("DoArkTSCheckInCompatibleModeInApi11")
            });
            self.compiler_options.is_compatible_version |= force_compatible;
            self.compiler_options.skip_tsc_oh_module_check = metadata.iter().any(|entry| {
                entry.name == "SkipTscOhModuleCheck" && entry.value.as_deref() == Some("true")
            });
            self.compiler_options.skip_ark_ts_static_blocks_check = metadata.iter().any(|entry| {
                entry.name == "SkipArkTSStaticBlocksCheck" && entry.value.as_deref() == Some("true")
            });
        }
        self.compiler_options.allow_js = self.compiler_options.need_do_ark_ts_linter;
        self.compiler_options.oh_api_compatibility_check = if matches!(
            build_option.strict_mode.api_compatibility_check,
            Some(arkdown_core::ApiCompatibilityCheckLevel::Error)
        ) {
            "error".to_string()
        } else {
            String::new()
        };
        self.compiler_options.is_compile_js_har = module.module_type == OhosModuleType::Har
            && build_option.har_artifact_type != HarArtifactType::Bytecode
            && config.build_mode.is_release()
            && ark_options
                .and_then(|options| options.obfuscation.as_ref())
                .is_some_and(|options| options.is_enabled_for_mode(&config.build_mode))
            && !module_context
                .module
                .module_json
                .as_ref()
                .is_some_and(|manifest| {
                    manifest.module.metadata.iter().any(|entry| {
                        entry.name == "UseTsHar" && entry.value.as_deref() == Some("true")
                    })
                });
        self.check_declarations = matches!(
            module.module_type,
            OhosModuleType::Har | OhosModuleType::Shared
        );
        self.compiler_options.oh_package_exports = self
            .package_resolution
            .sorted_entries()
            .into_iter()
            .filter_map(|(_, info)| {
                info.manifest.oh_export_files.as_ref().map(|files| {
                    (
                        info.pkg_name.clone(),
                        files.iter().cloned().collect::<Vec<_>>(),
                    )
                })
            })
            .collect();
        self
    }

    pub fn with_compile_inputs(
        mut self,
        project_path: &Path,
        cache_path: Option<&Path>,
        package_manager_type: &str,
        compiler_types: Option<&[String]>,
        resolve_module_paths: impl IntoIterator<Item = PathBuf>,
        form_entry_sources: impl IntoIterator<Item = PathBuf>,
    ) -> Self {
        self.compiler_options.base_url = project_path.to_path_buf();
        self.compiler_options.paths_base_path = project_path.to_path_buf();
        self.compiler_options.oh_project_path = project_path.to_path_buf();
        self.compiler_options.package_manager_type = package_manager_type.to_string();
        self.cache_path = cache_path.map(Path::to_path_buf);
        if let Some(types) = compiler_types {
            self.compiler_options.types = types.to_vec();
        }
        self.compiler_options.ts_build_info_file = cache_path
            .and_then(Path::parent)
            .map(|parent| parent.join(".tsbuildinfo"))
            .unwrap_or_default();
        self.compiler_options.oh_card_entry_files = form_entry_sources.into_iter().collect();
        let all_paths = self
            .compiler_options
            .paths
            .entry("*".to_string())
            .or_default();
        all_paths.push(PathBuf::from("*"));
        for path in resolve_module_paths {
            let name = path.file_name().and_then(|name| name.to_str());
            if matches!(name, Some("oh_modules" | "node_modules")) {
                continue;
            }
            let relative = Self::relative_path(project_path, &path);
            all_paths.push(relative.join("*"));
        }
        all_paths.dedup();
        self
    }

    pub fn with_sdk_validation_context(
        mut self,
        crossplatform: bool,
        ignore_crossplatform_check: bool,
        syscap_intersection: impl IntoIterator<Item = String>,
        syscap_union: impl IntoIterator<Item = String>,
    ) -> Self {
        self.compiler_options.oh_crossplatform = crossplatform;
        self.compiler_options.oh_ignore_crossplatform_check = ignore_crossplatform_check;
        self.compiler_options.oh_syscap_intersection = syscap_intersection.into_iter().collect();
        self.compiler_options.oh_syscap_union = syscap_union.into_iter().collect();
        self
    }

    /// Attach the SDK declaration inventory captured by [`SourceStore`] at
    /// session startup. `ets_checker.ts::serviceChecker` receives these roots
    /// through TypeScript's configured module resolution; the native checker
    /// maps the same declaration filenames to exact module specifiers without
    /// invoking the SDK checker process.
    pub fn with_sdk_declaration_roots(
        mut self,
        source_store: &'a SourceStore,
        roots: &[PathBuf],
    ) -> Self {
        self.source_store = Some(source_store);
        self.sdk_declarations = roots
            .iter()
            .flat_map(|root| source_store.source_files_under(root))
            .map(|file| file.path.clone())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.ends_with(".d.ts") || name.ends_with(".d.ets"))
            })
            .collect();
        self.sdk_declarations.sort();
        self.sdk_declarations.dedup();
        self
    }

    pub fn run(&self) -> Result<ProjectSemanticCheckReport> {
        self.run_inner(None, None, ProjectSemanticMode::Full)
    }

    pub(crate) fn run_for_build(
        &self,
        mode: ProjectSemanticMode,
    ) -> Result<ProjectSemanticCheckReport> {
        self.run_inner(None, None, mode)
    }

    pub(crate) fn run_for_build_with_runtime(
        &self,
        runtime: &mut ProjectSemanticRuntime,
        invalidation: Option<&WatchInvalidationBatch>,
        mode: ProjectSemanticMode,
    ) -> Result<ProjectSemanticCheckReport> {
        self.run_inner(Some(runtime), invalidation, mode)
    }

    fn run_inner(
        &self,
        runtime: Option<&mut ProjectSemanticRuntime>,
        invalidation: Option<&WatchInvalidationBatch>,
        mode: ProjectSemanticMode,
    ) -> Result<ProjectSemanticCheckReport> {
        let mut roots = self
            .root_files
            .iter()
            .filter(|path| Self::is_checkable_root(path))
            .cloned()
            .collect::<Vec<_>>();
        roots.sort();
        roots.dedup();
        if roots.is_empty() {
            return Ok(ProjectSemanticCheckReport::default());
        }

        let options = self.compiler_options.clone();
        tracing::info!(
            sdk_function_plugins = options.oh_sdk_check_plugins.len(),
            sdk_class_plugins = options.oh_sdk_class_check_plugins.len(),
            sdk_function_plugin_paths = ?options
                .oh_sdk_check_plugins
                .iter()
                .map(|plugin| plugin.path.as_path())
                .collect::<Vec<_>>(),
            sdk_class_plugin_paths = ?options
                .oh_sdk_class_check_plugins
                .iter()
                .map(|plugin| plugin.path.as_path())
                .collect::<Vec<_>>(),
            "ArkTS SDK checker plugin inventory"
        );
        let sdk_plugin_host = if options.uses_sdk_plugin_callbacks() {
            Some(self.sdk_plugin_host.clone().ok_or_else(|| {
                anyhow::anyhow!(
                    "ARKDOWN_SDK_PLUGIN_HOST_MISSING: the SDK config declares JavaScript checker plugins but the JavaScript service entry did not provide a plugin host"
                )
            })?)
        } else {
            None
        };
        // ets_checker.ts::getScriptFileNames includes only loader declarations
        // as global roots. SDK API modules are resolved on demand, not all
        // force-loaded (which would also parse unrelated annotation dialects).
        roots.extend(self.builtin_declarations.iter().cloned());
        roots.sort();
        roots.dedup();
        // Runtime/watch snapshots preserve their eager source equality and
        // previous-snapshot reuse. Only the coordinator's one-shot owner opts in.
        let evict_source_texts = self.evict_source_texts && runtime.is_none();
        let mut sources = BTreeMap::new();
        if !evict_source_texts {
            if let Some(store) = self.source_store {
                for file in store.source_files() {
                    if let SourceContent::Text(text) = file.cached_content()? {
                        sources.insert(file.path.clone(), text);
                    }
                }
            }
        }
        let prepared = PreparedSemanticCheck {
            roots,
            options,
            sdk_plugin_host,
            sources,
        };
        if let Some(runtime) = runtime {
            let key = ProjectSemanticRuntimeKey {
                project_root: self.project_root.to_path_buf(),
                module_root: self.compiler_options.oh_module_path.clone(),
                cache_path: self.cache_path.clone().unwrap_or_default(),
            };
            let changes = ProjectSemanticRuntime::file_changes(invalidation);
            let (session, program) =
                runtime.program(key, self.project_root, &prepared, &changes)?;
            return self.check_program(
                session,
                &program,
                &prepared,
                mode,
                SemanticProgramLifetime::Runtime,
            );
        }
        let mut session = NativeTypeSession::start(
            self.project_root,
            prepared.sources.clone(),
            prepared.sdk_plugin_host.clone(),
        )?
        .with_source_preprocessor(Arc::new(ArktsCheckerSourcePreprocessor));
        if evict_source_texts {
            if let Some(store) = self.source_store {
                session = session.with_source_handles(
                    store
                        .source_files()
                        .into_iter()
                        .map(|file| {
                            (
                                file.path.clone(),
                                Arc::new(file.clone()) as Arc<dyn arkdown_core::SourceTextHandle>,
                            )
                        })
                        .collect(),
                );
            }
        }
        let program = session.create_program(&prepared.roots, &prepared.options)?;
        if evict_source_texts {
            if let Some(store) = self.source_store {
                store.evict_unleased_texts();
            }
        }
        let report = self.check_program(
            &mut session,
            &program,
            &prepared,
            mode,
            SemanticProgramLifetime::OneShot,
        );
        drop(program);
        drop(session);
        drop(prepared);
        if let Some(reclaimer) = &self.heap_reclaimer {
            reclaimer.reclaim(crate::request::BuildHeapBoundary::CheckerReleased);
        }
        if evict_source_texts {
            if let Some(store) = self.source_store {
                store.restore_evicted_texts()?;
            }
        }
        report
    }

    fn semantic_source_inputs(
        &self,
        files: &[SemanticSourceIdentity],
        read_sources: &BTreeMap<PathBuf, NativeProcessedSource>,
    ) -> SemanticSourceInputs {
        let selected = files
            .iter()
            .map(|file| file.snapshot_path.clone())
            .collect::<BTreeSet<_>>();
        let selected = selected.into_iter().collect::<Vec<_>>();
        let inventories = selected
            .par_iter()
            .filter(|path| Self::is_type_script_preprocess_source(path))
            .map(|path| {
                let source = &read_sources[path];
                let inventory = ArktsSourceSemanticCollector::new(self.component_metadata)
                    .collect(&source.text, self.is_ui_diagnostic_source(path));
                (path.clone(), inventory)
            })
            .collect::<Vec<_>>();
        let mut ui_props = BTreeSet::new();
        let mut new_extend_names = BTreeSet::new();
        for (path, inventory) in &inventories {
            ui_props.extend(inventory.diagnostic_names().iter().cloned());
            ui_props.extend(read_sources[path].legacy_extend_names.iter().cloned());
            new_extend_names.extend(read_sources[path].new_extend_names.iter().cloned());
        }
        SemanticSourceInputs {
            ui_props,
            new_extend_names,
            sources: selected
                .into_iter()
                .map(|path| {
                    let text = Arc::clone(&read_sources[&path].text);
                    (path, text)
                })
                .collect(),
            semantic_inventories: inventories.into_iter().collect(),
        }
    }

    fn check_program(
        &self,
        session: &mut NativeTypeSession,
        program: &NativeProgram,
        prepared: &PreparedSemanticCheck,
        mode: ProjectSemanticMode,
        lifetime: SemanticProgramLifetime,
    ) -> Result<ProjectSemanticCheckReport> {
        let source_graph = session.source_graph(program)?;
        let read_sources = session.read_sources();
        let sources = read_sources
            .iter()
            .map(|(path, source)| (path.clone(), Arc::clone(&source.text)))
            .collect();
        let mut source_identities = source_graph
            .files
            .iter()
            .filter_map(|file| self.semantic_source_identity(&file.file_name, &sources))
            .collect::<Vec<_>>();
        source_identities.sort_unstable_by(|left, right| {
            left.compiler_path
                .cmp(&right.compiler_path)
                .then_with(|| left.snapshot_path.cmp(&right.snapshot_path))
        });
        source_identities.dedup_by(|left, right| left.compiler_path == right.compiler_path);
        let inputs = self.semantic_source_inputs(&source_identities, &read_sources);
        let checked_sources = source_identities
            .iter()
            .filter(|source| Self::is_checkable_root(&source.snapshot_path))
            .map(|source| source.snapshot_path.clone())
            .collect::<Vec<_>>();
        let diagnostics_started = Instant::now();
        let normal = if mode.checks_diagnostics() {
            Some(session.arkts_normal_diagnostics(program)?)
        } else {
            tracing::info!(
                files = source_graph.files.len(),
                "ArkTS diagnostics skipped; retaining transform type inference"
            );
            None
        };
        let mut declaration_diagnostics = Vec::new();
        if mode.checks_diagnostics() && self.check_declarations {
            let declaration_sources = source_graph
                .files
                .iter()
                .map(|file| file.file_name.clone())
                .filter(|file| Self::is_declaration_diagnostic_source(file))
                .collect::<Vec<_>>();
            declaration_diagnostics =
                session.declaration_diagnostics(program, &declaration_sources)?;
        }
        let normal_elapsed = diagnostics_started.elapsed();
        let annotation_sources = source_identities
            .iter()
            .filter(|source| Self::is_type_script_preprocess_source(&source.snapshot_path))
            .cloned()
            .collect::<Vec<_>>();
        let annotation_files = annotation_sources
            .iter()
            .map(|source| source.compiler_path.clone())
            .collect::<Vec<_>>();
        let source_identities = annotation_sources
            .iter()
            .map(|source| (source.compiler_path.clone(), source.snapshot_path.clone()))
            .collect::<FxHashMap<_, _>>();
        let facts_started = Instant::now();
        let native_type_facts = session.annotation_transform_infos(program, &annotation_files)?;
        let transform_queries = self.native_transform_type_queries(
            &annotation_sources,
            &inputs.sources,
            &inputs.semantic_inventories,
        )?;
        let sdk_api_use_files = if prepared.options.oh_crossplatform {
            annotation_files.as_slice()
        } else {
            &[]
        };
        let native_transform_type_facts =
            session.arkts_transform_type_facts(program, &transform_queries, sdk_api_use_files)?;
        tracing::info!(
            elapsed_ms = facts_started.elapsed().as_millis(),
            files = transform_queries.len(),
            property_queries = transform_queries
                .iter()
                .map(|query| query.properties.len())
                .sum::<usize>(),
            builder_queries = transform_queries
                .iter()
                .map(|query| query.builder_accesses.len())
                .sum::<usize>(),
            member_queries = transform_queries
                .iter()
                .map(|query| query.member_accesses.len())
                .sum::<usize>(),
            "ArkTS transform type facts completed"
        );
        // All normal-pool outputs are now owned. Retire its regions before the
        // strict linter is initialized; preserve linter/syntactic/semantic order.
        let (mut linter, mut result) = if let Some(normal) = normal {
            let strict_started = Instant::now();
            session.release_arkts_build_checkers(program)?;
            if lifetime == SemanticProgramLifetime::OneShot {
                session.release_sdk_sources_before_strict(&prepared.options);
            }
            if let Some(reclaimer) = &self.heap_reclaimer {
                reclaimer.reclaim(crate::request::BuildHeapBoundary::NormalCheckerReleased);
            }
            let diagnostics = session.arkts_strict_diagnostics(program, normal)?;
            let mut result = diagnostics.syntactic_diagnostics;
            result.extend(diagnostics.semantic_diagnostics);
            result.extend(declaration_diagnostics);
            tracing::info!(
                elapsed_ms = (normal_elapsed + strict_started.elapsed()).as_millis(),
                files = source_graph.files.len(),
                "ArkTS build diagnostics completed"
            );
            (diagnostics.linter_diagnostics, result)
        } else {
            session.release_arkts_build_checkers(program)?;
            (Vec::new(), Vec::new())
        };
        let type_facts = NativeAnnotationFactsConverter::new(
            &inputs.sources,
            &source_identities,
            &self.crossplatform_dependencies,
        )
        .convert(native_type_facts, native_transform_type_facts)?;
        if mode.checks_diagnostics() {
            linter = self.prepare_linter_diagnostics(linter)?;
            self.apply_ui_diagnostic_rules(&mut linter, &inputs.ui_props, &inputs.new_extend_names);
            self.apply_ui_diagnostic_rules(&mut result, &inputs.ui_props, &inputs.new_extend_names);
            let ignored = self.ignored_diagnostic_files(&source_graph);
            linter.retain(|diagnostic| self.keep_diagnostic(diagnostic, &ignored));
            result.retain(|diagnostic| self.keep_diagnostic(diagnostic, &ignored));
        }
        let mut report = ProjectSemanticCheckReport {
            checked_sources,
            type_facts,
            ..ProjectSemanticCheckReport::default()
        };
        for diagnostic in linter.into_iter().chain(result) {
            if diagnostic.is_error() {
                report.errors += 1;
                report.diagnostics.push(diagnostic.render());
            } else {
                report.warnings.push(diagnostic.render());
            }
        }
        Ok(report)
    }

    pub fn with_builtin_declarations(mut self, loader: &Path) -> Self {
        self.compiler_options.ets_loader_path = loader.to_path_buf();
        self.compiler_options.oh_loader_module_root = loader.join("node_modules");
        let directory = loader.join("declarations");
        if let Some(store) = self.source_store {
            self.builtin_declarations = store
                .source_files_under(&directory)
                .into_iter()
                .map(|file| file.path.as_path())
                .filter(|path| {
                    path.parent() == Some(directory.as_path())
                        && path
                            .file_name()
                            .and_then(|name| name.to_str())
                            .is_some_and(|name| name.ends_with(".d.ts"))
                })
                .map(Path::to_path_buf)
                .collect();
        }
        self
    }

    fn filter_linter_diagnostics(&self, diagnostics: &mut [NativeDiagnostic]) {
        let compatible = self.compiler_options.is_compatible_version;
        let sdk_root = self.sdk_root.as_deref();
        for diagnostic in diagnostics {
            let in_oh_modules = diagnostic.file_name.as_ref().is_some_and(|path| {
                path.components()
                    .any(|component| component.as_os_str() == "oh_modules")
            });
            let in_sdk_ets = diagnostic.file_name.as_ref().is_some_and(|path| {
                sdk_root.is_some_and(|root| path.starts_with(root))
                    && path.extension().and_then(|extension| extension.to_str()) == Some("ets")
            });
            if compatible || diagnostic.is_error() && (in_oh_modules || in_sdk_ets) {
                diagnostic.category = 0;
            }
        }
    }

    /// Source owner: `do_arkTS_linter.ts::doArkTSLinter`, including removal
    /// and compatible-mode output of `ArkTS/ArkTSLinter_output.json`.
    fn prepare_linter_diagnostics(
        &self,
        mut diagnostics: Vec<NativeDiagnostic>,
    ) -> Result<Vec<NativeDiagnostic>> {
        let output_path = self
            .cache_path
            .as_ref()
            .map(|cache| cache.join("ArkTS").join("ArkTSLinter_output.json"));
        if let Some(path) = output_path.as_ref().filter(|path| path.is_file()) {
            fs_err::remove_file(path)?;
        }
        if diagnostics.is_empty() {
            return Ok(diagnostics);
        }

        if !self.compiler_options.is_compatible_version {
            self.filter_linter_diagnostics(&mut diagnostics);
            diagnostics.push(NativeDiagnostic::warning(ARKTS_LINTER_FAQ));
            return Ok(diagnostics);
        }

        let written_path = if let (Some(cache), Some(path)) =
            (self.cache_path.as_ref(), output_path.as_ref())
        {
            if cache.exists() {
                let directory = path.parent().expect("linter output has parent");
                if !directory.exists() {
                    fs_err::create_dir(directory)?;
                }
                let output = diagnostics
                    .iter()
                    .map(|diagnostic| {
                        let file_name = diagnostic.file_name.as_deref().ok_or_else(|| {
                            anyhow::anyhow!(
                                "ARKDOWN_ETS_LINTER_OUTPUT: diagnostic {} has no source file",
                                diagnostic.code
                            )
                        })?;
                        let position = diagnostic.start_position.as_ref().ok_or_else(|| {
                            anyhow::anyhow!(
                                "ARKDOWN_ETS_LINTER_OUTPUT: diagnostic {} has no source position",
                                diagnostic.code
                            )
                        })?;
                        Ok(ArktsLinterOutput {
                            category_info: if diagnostic.is_error() {
                                "Error"
                            } else {
                                "Warning"
                            },
                            file_name,
                            line: position.line + 1,
                            character: position.character + 1,
                            message_text: &diagnostic.text,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                match serde_json::to_vec_pretty(&output)
                    .map_err(anyhow::Error::from)
                    .and_then(|bytes| fs_err::write(path, bytes).map_err(anyhow::Error::from))
                {
                    Ok(()) => Some(path),
                    Err(error) => {
                        tracing::debug!(%error, "ArkTS compatible linter output could not be written");
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        };

        if let Some(path) = written_path {
            diagnostics = vec![NativeDiagnostic::warning(format!(
                "Has {} ArkTS Linter Error. You can get the output in {}",
                diagnostics.len(),
                path.display()
            ))];
        } else {
            for diagnostic in &mut diagnostics {
                diagnostic.category = 0;
            }
        }
        diagnostics.push(NativeDiagnostic::warning(ARKTS_LINTER_FAQ));
        Ok(diagnostics)
    }

    fn ignored_diagnostic_files(&self, graph: &NativeProgramSourceGraph) -> BTreeSet<PathBuf> {
        if !self.compiler_options.need_do_ark_ts_linter {
            return BTreeSet::new();
        }
        let type_references = graph
            .type_reference_files
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let sdk_root = self.sdk_root.as_deref();
        let mut ignored = graph
            .files
            .iter()
            .filter(|file| {
                let in_sdk = sdk_root.is_some_and(|root| file.file_name.starts_with(root));
                let js = Self::is_javascript(&file.file_name);
                (!in_sdk || js) && !type_references.contains(&file.file_name)
            })
            .map(|file| file.file_name.clone())
            .collect::<BTreeSet<_>>();
        let dependencies = graph
            .files
            .iter()
            .map(|file| (file.file_name.as_path(), file.dependencies.as_slice()))
            .collect::<BTreeMap<_, _>>();
        let mut pending = self
            .root_files
            .iter()
            .filter(|path| !Self::is_javascript(path))
            .cloned()
            .collect::<Vec<_>>();
        let mut reachable = BTreeSet::new();
        while let Some(file) = pending.pop() {
            if !reachable.insert(file.clone()) || Self::is_javascript(&file) {
                continue;
            }
            if let Some(children) = dependencies.get(file.as_path()) {
                pending.extend(children.iter().cloned());
            }
        }
        for file in reachable {
            ignored.remove(&file);
        }
        if self.compiler_options.oh_bundle_type == "atomicService" {
            ignored.retain(|file| {
                !Self::is_javascript(file)
                    || sdk_root.is_some_and(|root| file.starts_with(root))
                    || !file.starts_with(&self.compiler_options.oh_project_path)
            });
        }
        ignored
    }

    /// Source owner: `ets_checker.ts::validateError/validateNewExtend`.
    fn apply_ui_diagnostic_rules(
        &self,
        diagnostics: &mut Vec<NativeDiagnostic>,
        props: &BTreeSet<String>,
        new_extend_names: &BTreeSet<String>,
    ) {
        diagnostics.retain(|diagnostic| {
            !Self::diagnostic_property_name(&diagnostic.text)
                .is_some_and(|name| props.contains(name))
        });
        for diagnostic in diagnostics {
            if Self::missing_property_name(&diagnostic.text)
                .is_some_and(|name| new_extend_names.contains(name))
            {
                diagnostic.category = 0;
            }
        }
    }

    fn diagnostic_property_name(message: &str) -> Option<&str> {
        Self::cannot_find_name(message).or_else(|| Self::missing_property_name(message))
    }

    fn cannot_find_name(message: &str) -> Option<&str> {
        message
            .match_indices("Cannot find name")
            .find_map(|(offset, _)| {
                let tail = message[offset + "Cannot find name".len()..].trim_start();
                let name = tail.strip_prefix('\'')?.split('\'').next()?;
                (!name.is_empty()
                    && name.chars().all(|character| {
                        character == '$' || character == '_' || character.is_ascii_alphanumeric()
                    })
                    && name
                        .chars()
                        .take_while(|character| *character == '$')
                        .count()
                        <= 2)
                    .then_some(name)
            })
    }

    fn missing_property_name(message: &str) -> Option<&str> {
        message.match_indices("Property").find_map(|(offset, _)| {
            let tail = message[offset + "Property".len()..].trim_start();
            let tail = tail.strip_prefix('\'')?;
            let (name, tail) = tail.split_once('\'')?;
            (!name.is_empty()
                && tail.trim_start().starts_with("does not exist on type")
                && name.chars().all(|character| {
                    character == '$' || character == '_' || character.is_ascii_alphanumeric()
                })
                && name
                    .chars()
                    .take_while(|character| *character == '$')
                    .count()
                    <= 1)
                .then_some(name)
        })
    }

    fn keep_diagnostic(&self, diagnostic: &NativeDiagnostic, ignored: &BTreeSet<PathBuf>) -> bool {
        let Some(file) = diagnostic.file_name.as_ref() else {
            return true;
        };
        if ignored.contains(file) {
            return false;
        }
        if self.compiler_options.oh_bundle_type == "atomicService"
            && Self::is_javascript(file)
            && file.starts_with(&self.compiler_options.oh_project_path)
        {
            return diagnostic
                .text
                .contains("can't support atomicservice application.");
        }
        if diagnostic.text.contains("can't support form application.") {
            return self
                .compiler_options
                .oh_card_entry_files
                .iter()
                .any(|card| card == file);
        }
        true
    }

    fn is_javascript(path: &Path) -> bool {
        matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("js" | "mjs" | "cjs")
        )
    }

    fn is_checkable_root(path: &Path) -> bool {
        // serviceChecker consumes the supplied root inventory. Do not re-stat
        // each path here: SourceStore owns the build's immutable source view,
        // and the compiler host diagnoses unavailable roots itself.
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        !name.ends_with(".d.ts")
            && !name.ends_with(".d.ets")
            && matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("ets" | "ts" | "tsx" | "mts" | "cts" | "js" | "mjs" | "cjs")
            )
    }

    fn is_type_script_preprocess_source(path: &Path) -> bool {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        !name.ends_with(".d.ts")
            && !name.ends_with(".d.ets")
            && matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("ets" | "ts")
            )
    }

    /// Build the exact checker query set selected by the OXC syntax pass.
    ///
    /// Source owners are documented on `ArktsSourceSemanticCollector`. Keeping
    /// this selection out of tsrs avoids a second full-project AST traversal;
    /// tsrs remains responsible for every returned type identity.
    fn native_transform_type_queries(
        &self,
        files: &[SemanticSourceIdentity],
        sources: &BTreeMap<PathBuf, Arc<str>>,
        inventories: &BTreeMap<PathBuf, ArktsSourceSemanticInventory>,
    ) -> Result<Vec<NativeArktsTransformTypeQuery>> {
        files
            .iter()
            .filter_map(|file| {
                inventories
                    .get(&file.snapshot_path)
                    .filter(|inventory| inventory.has_type_queries())
                    .map(|inventory| (file, inventory))
            })
            .map(|(file, inventory)| {
                let source = sources.get(&file.snapshot_path).ok_or_else(|| {
                    anyhow::anyhow!(
                        "ARKDOWN_TYPE_QUERY_SOURCE: unavailable OXC source {}",
                        file.snapshot_path.display()
                    )
                })?;
                let offsets = Utf16OffsetMap::new(source);
                Ok(NativeArktsTransformTypeQuery {
                    file: file.compiler_path.clone(),
                    properties: offsets
                        .query_ranges(inventory.property_queries(), &file.snapshot_path)?,
                    builder_accesses: offsets
                        .query_ranges(inventory.builder_access_queries(), &file.snapshot_path)?,
                    member_accesses: offsets
                        .query_ranges(inventory.member_access_queries(), &file.snapshot_path)?,
                })
            })
            .collect()
    }

    /// Resolve TypeScript's source-graph spelling through the immutable
    /// startup inventory. `processImportModule` asks the checker about the
    /// resolved Program file, while every OXC consumer uses `SourceStore`'s
    /// canonical source identity.
    fn semantic_source_identity(
        &self,
        compiler_path: &Path,
        sources: &BTreeMap<PathBuf, Arc<str>>,
    ) -> Option<SemanticSourceIdentity> {
        if sources.contains_key(compiler_path) {
            return Some(SemanticSourceIdentity {
                compiler_path: compiler_path.to_path_buf(),
                snapshot_path: compiler_path.to_path_buf(),
            });
        }
        let snapshot_path = self
            .source_store?
            .source_by_path(compiler_path)?
            .path
            .clone();
        sources
            .contains_key(&snapshot_path)
            .then(|| SemanticSourceIdentity {
                compiler_path: compiler_path.to_path_buf(),
                snapshot_path,
            })
    }

    fn is_ui_diagnostic_source(&self, path: &Path) -> bool {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if name.ends_with(".d.ets")
            || path.extension().and_then(|extension| extension.to_str()) != Some("ets")
        {
            return false;
        }
        self.compiler_options.oh_compile_mode == "moduleJson"
            || lexical_normalize(path)
                != lexical_normalize(&self.compiler_options.oh_project_path.join("app.ets"))
    }

    fn is_declaration_diagnostic_source(path: &Path) -> bool {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        !name.ends_with(".d.ts")
            && !name.ends_with(".d.ets")
            && path.extension().and_then(|extension| extension.to_str()) != Some("js")
            && !path
                .components()
                .any(|component| component.as_os_str() == "oh_modules")
    }

    /// Platform-native lexical equivalent of Node's `path.relative`, used by
    /// `ets_checker.ts::setCompilerOptions` for the wildcard path list.
    fn relative_path(from: &Path, to: &Path) -> PathBuf {
        let from = lexical_normalize(from);
        let to = lexical_normalize(to);
        let from_components = from.components().collect::<Vec<_>>();
        let to_components = to.components().collect::<Vec<_>>();
        if from_components
            .first()
            .zip(to_components.first())
            .is_some_and(|(left, right)| {
                matches!(left, Component::Prefix(_) | Component::RootDir)
                    && matches!(right, Component::Prefix(_) | Component::RootDir)
                    && left != right
            })
        {
            return to;
        }
        let common = from_components
            .iter()
            .zip(&to_components)
            .take_while(|(left, right)| left == right)
            .count();
        let mut relative = PathBuf::new();
        for component in &from_components[common..] {
            if matches!(component, Component::Normal(_)) {
                relative.push("..");
            }
        }
        for component in &to_components[common..] {
            if matches!(component, Component::Normal(_)) {
                relative.push(component.as_os_str());
            }
        }
        if relative.as_os_str().is_empty() {
            relative.push(".");
        }
        relative
    }
}

struct NativeAnnotationFactsConverter<'a> {
    sources: &'a BTreeMap<PathBuf, Arc<str>>,
    source_identities: &'a FxHashMap<PathBuf, PathBuf>,
    crossplatform_dependencies: &'a FxHashMap<(String, String), CrossplatformSourceCollection>,
}

impl<'a> NativeAnnotationFactsConverter<'a> {
    fn new(
        sources: &'a BTreeMap<PathBuf, Arc<str>>,
        source_identities: &'a FxHashMap<PathBuf, PathBuf>,
        crossplatform_dependencies: &'a FxHashMap<(String, String), CrossplatformSourceCollection>,
    ) -> Self {
        Self {
            sources,
            source_identities,
            crossplatform_dependencies,
        }
    }

    fn convert(
        &self,
        infos: Vec<NativeAnnotationTransformInfo>,
        transform_infos: Vec<NativeArktsTransformTypeFacts>,
    ) -> Result<ArktsTypeFactsSnapshot> {
        let mut files = FxHashMap::default();
        for info in infos {
            let (source_path, source) = self.source(&info.file_name)?;
            let offsets = Utf16OffsetMap::new(source);
            let declarations = info
                .declarations
                .into_iter()
                .map(|fact| {
                    Ok(SpannedAnnotationInfo {
                        span: offsets.span(fact.pos, fact.end, &info.file_name)?,
                        info: self.annotation_info(fact.info)?,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            let uses = info
                .uses
                .into_iter()
                .map(|fact| {
                    Ok(SpannedAnnotationUse {
                        span: offsets.span(fact.pos, fact.end, &info.file_name)?,
                        annotation: AnnotationUse {
                            runtime_retained: fact.runtime_retained,
                            info: self.annotation_info(fact.info)?,
                        },
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            let imports = info
                .imports
                .into_iter()
                .map(|fact| {
                    let disposition = match fact.disposition.as_str() {
                        "rename" => AnnotationImportDisposition::Rename,
                        "remove" => AnnotationImportDisposition::Remove,
                        other => anyhow::bail!(
                            "ARKDOWN_TYPE_FACTS_IMPORT: unsupported annotation import disposition '{other}'"
                        ),
                    };
                    Ok(SpannedAnnotationImport {
                        span: offsets.span(fact.pos, fact.end, &info.file_name)?,
                        disposition,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            files.insert(
                source_path.to_path_buf(),
                ArktsFileTypeFacts {
                    declarations,
                    uses,
                    imports,
                    ..ArktsFileTypeFacts::default()
                },
            );
        }
        for info in transform_infos {
            let (source_path, source) = self.source(&info.file_name)?;
            let offsets = Utf16OffsetMap::new(source);
            let facts = files
                .entry(source_path.to_path_buf())
                .or_insert_with(ArktsFileTypeFacts::default);
            facts.properties = info
                .properties
                .into_iter()
                .map(|property| {
                    Ok(SpannedPropertyTypeFacts {
                        span: offsets.span(
                            property.name_pos,
                            property.name_end,
                            &info.file_name,
                        )?,
                        facts: PropertyTypeFacts {
                            declared_type: property.r#type.map(Self::type_identity),
                        },
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            facts.builder_accesses = info
                .builder_accesses
                .into_iter()
                .map(|access| {
                    Ok(SpannedTypeIdentity {
                        span: offsets.span(access.pos, access.end, &info.file_name)?,
                        identity: Self::type_identity(access.receiver_type),
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            facts.member_accesses = info
                .member_accesses
                .into_iter()
                .map(|access| {
                    Ok(SpannedTypeIdentity {
                        span: offsets.span(access.pos, access.end, &info.file_name)?,
                        identity: Self::type_identity(access.r#type),
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            for usage in info.sdk_api_uses {
                if let Some(dependency) = self
                    .crossplatform_dependencies
                    .get(&(usage.api_module, usage.function))
                {
                    facts.crossplatform_dependencies.merge(dependency);
                }
            }
        }
        Ok(ArktsTypeFactsSnapshot::new(files))
    }

    fn source(&self, compiler_path: &Path) -> Result<(&Path, &Arc<str>)> {
        let requested_path = self
            .source_identities
            .get(compiler_path)
            .map(PathBuf::as_path)
            .unwrap_or(compiler_path);
        let (source_path, source) =
            self.sources.get_key_value(requested_path).ok_or_else(|| {
                anyhow::anyhow!(
                    "ARKDOWN_TYPE_FACTS_SOURCE: compiler returned facts for unavailable source {}",
                    compiler_path.display()
                )
            })?;
        Ok((source_path, source))
    }

    fn type_identity(identity: NativeResolvedTypeIdentity) -> ResolvedTypeIdentity {
        ResolvedTypeIdentity {
            is_nullable: identity.is_nullable,
            is_union: identity.is_union,
            is_enum: identity.is_enum,
            is_basic: identity.is_basic,
            is_observed_v2: identity.is_observed_v2,
            is_function: identity.is_function,
            symbol_name: identity.symbol_name,
            types: identity
                .types
                .into_iter()
                .map(Self::type_identity)
                .collect(),
        }
    }

    fn annotation_info(&self, info: NativeAnnotationInfo) -> Result<AnnotationInfo> {
        Ok(AnnotationInfo {
            source_retention: info.source_retention,
            properties: info
                .properties
                .into_iter()
                .map(|property| {
                    Ok(AnnotationProperty {
                        name: property.name,
                        name_text: property.name_text,
                        initializer: property
                            .initializer
                            .map(|value| self.constant(value))
                            .transpose()?,
                        argument: property
                            .argument
                            .map(|value| self.constant(value))
                            .transpose()?,
                        array_depth: property.array_depth,
                        enum_first_value: property
                            .enum_first_value
                            .map(|value| self.constant(value))
                            .transpose()?,
                        type_text: property.type_text,
                        element_type_text: property.element_type_text,
                    })
                })
                .collect::<Result<Vec<_>>>()?,
        })
    }

    fn constant(&self, value: NativeAnnotationConstant) -> Result<AnnotationConstant> {
        match value.kind.as_str() {
            "number" => Ok(AnnotationConstant::Number(value.value)),
            "string" => Ok(AnnotationConstant::String(value.value)),
            "boolean" => value
                .value
                .parse()
                .map(AnnotationConstant::Boolean)
                .map_err(anyhow::Error::from),
            "array" => value
                .items
                .into_iter()
                .map(|item| self.constant(item))
                .collect::<Result<Vec<_>>>()
                .map(AnnotationConstant::Array),
            other => anyhow::bail!(
                "ARKDOWN_TYPE_FACTS_CONSTANT: unsupported annotation constant kind '{other}'"
            ),
        }
    }
}

struct Utf16OffsetMap {
    byte_offsets: Vec<Option<usize>>,
    utf16_offsets: Vec<Option<usize>>,
}

impl Utf16OffsetMap {
    fn new(source: &str) -> Self {
        let mut byte_offsets = Vec::with_capacity(source.len() + 1);
        let mut utf16_offsets = vec![None; source.len() + 1];
        byte_offsets.push(Some(0));
        utf16_offsets[0] = Some(0);
        let mut utf16_position = 0;
        for (byte, character) in source.char_indices() {
            utf16_offsets[byte] = Some(utf16_position);
            if character.len_utf16() == 2 {
                byte_offsets.push(None);
            }
            utf16_position += character.len_utf16();
            byte_offsets.push(Some(byte + character.len_utf8()));
            utf16_offsets[byte + character.len_utf8()] = Some(utf16_position);
        }
        Self {
            byte_offsets,
            utf16_offsets,
        }
    }

    fn query_ranges(&self, spans: &[Span], file: &Path) -> Result<Vec<NativeTypeQueryRange>> {
        spans
            .iter()
            .map(|span| {
                Ok(NativeTypeQueryRange {
                    pos: self.utf16_position(span.start, file)?,
                    end: self.utf16_position(span.end, file)?,
                })
            })
            .collect()
    }

    fn span(&self, pos: usize, end: usize, file: &Path) -> Result<Span> {
        let byte_pos = self.offset(pos, file)?;
        let byte_end = self.offset(end, file)?;
        Ok(Span::new(
            u32::try_from(byte_pos).map_err(|_| {
                anyhow::anyhow!(
                    "ARKDOWN_TYPE_FACTS_SPAN: source exceeds OXC span range: {}",
                    file.display()
                )
            })?,
            u32::try_from(byte_end).map_err(|_| {
                anyhow::anyhow!(
                    "ARKDOWN_TYPE_FACTS_SPAN: source exceeds OXC span range: {}",
                    file.display()
                )
            })?,
        ))
    }

    fn offset(&self, position: usize, file: &Path) -> Result<usize> {
        self.byte_offsets
            .get(position)
            .copied()
            .flatten()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "ARKDOWN_TYPE_FACTS_POSITION: invalid UTF-16 offset {position} in {}",
                    file.display()
                )
            })
    }

    fn utf16_position(&self, byte: u32, file: &Path) -> Result<usize> {
        self.utf16_offsets
            .get(byte as usize)
            .copied()
            .flatten()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "ARKDOWN_TYPE_QUERY_POSITION: invalid OXC byte offset {byte} in {}",
                    file.display()
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, sync::Arc};

    use super::{NativeAnnotationFactsConverter, ProjectSemanticChecker};
    use arkdown_ets_transform::transform::component_meta::ComponentMetadata;
    #[cfg(unix)]
    use arkdown_ets_transform::ArktsSourceSemanticCollector;
    use arkdown_resolver::PackageResolutionPlan;
    #[cfg(unix)]
    use arkdown_session::SourceContent;
    use arkdown_session::SourceStore;

    #[test]
    fn semantic_runtime_keeps_sdk_sources_across_rebuilds_and_reclaims_normal_pools() {
        use super::{ProjectSemanticMode, ProjectSemanticRuntime};
        use crate::request::{BuildHeapBoundary, BuildHeapReclaimer};
        use arkdown_ets_checker::NativeOhSdkClassCheckPlugin;
        use arkdown_ets_checker::{
            NativeCheckError, NativeOhSdkPluginHost, NativeOhSdkPluginRequest,
            NativeOhSdkPluginResponse,
        };

        #[derive(Debug, Default)]
        struct RuntimeSdkHost {
            operations: std::sync::Mutex<Vec<(u64, String)>>,
            normal_reclaims: std::sync::atomic::AtomicUsize,
        }
        impl NativeOhSdkPluginHost for RuntimeSdkHost {
            fn invoke_batch(
                &self,
                requests: Vec<NativeOhSdkPluginRequest>,
            ) -> Result<Vec<NativeOhSdkPluginResponse>, NativeCheckError> {
                Ok(requests
                    .into_iter()
                    .map(|request| {
                        self.operations
                            .lock()
                            .unwrap()
                            .push((request.session_id, request.operation.clone()));
                        NativeOhSdkPluginResponse {
                            found: true,
                            check_result: true,
                            ..Default::default()
                        }
                    })
                    .collect())
            }
        }
        impl BuildHeapReclaimer for RuntimeSdkHost {
            fn reclaim(&self, boundary: BuildHeapBoundary) {
                if boundary == BuildHeapBoundary::NormalCheckerReleased {
                    self.normal_reclaims
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    assert!(!self
                        .operations
                        .lock()
                        .unwrap()
                        .iter()
                        .any(|(_, op)| op == "releaseSources"));
                }
            }
        }

        // The public OpenHarmony SDK has no vendor class-check plugins. Supply
        // the callback contract directly so this lifetime regression is independent
        // of whichever SDK happens to be installed on the test host.
        let temp = tempfile::tempdir().unwrap();
        let sdk = temp.path().join("api.d.ets");
        let app = temp.path().join("entry.ts");
        std::fs::write(
            &sdk,
            "/** @syscap base */ declare function sdkCall(): void;",
        )
        .unwrap();
        std::fs::write(&app, "export {}; sdkCall();").unwrap();
        let roots = [sdk.clone(), app];
        let packages = PackageResolutionPlan::default();
        let metadata = ComponentMetadata::default();
        let host = Arc::new(RuntimeSdkHost::default());
        let mut checker = ProjectSemanticChecker::new(temp.path(), &roots, &packages, &metadata)
            .with_sdk_plugin_host(Some(host.clone()))
            .with_heap_reclaimer(Some(host.clone()));
        checker.builtin_declarations = vec![sdk.clone()];
        checker.compiler_options.ets_loader_path = temp.path().join("ets-loader");
        checker.compiler_options.oh_compile_mode = "moduleJson".into();
        checker.compiler_options.oh_device_types = vec!["phone".into()];
        checker.compiler_options.oh_syscap_intersection = vec!["base".into()];
        checker.compiler_options.oh_all_module_paths = vec![sdk];
        checker.compiler_options.oh_sdk_class_check_plugins = vec![NativeOhSdkClassCheckPlugin {
            tag_name: "syscap".into(),
            path: temp.path().join("checker.cjs"),
            class_name: "Checker".into(),
        }];
        let mut semantic = ProjectSemanticRuntime::default();
        for _ in 0..2 {
            let report = checker
                .run_for_build_with_runtime(&mut semantic, None, ProjectSemanticMode::Full)
                .unwrap();
            assert!(report.diagnostics.is_empty(), "{report:?}");
        }
        semantic.close();
        let operations = host.operations.lock().unwrap();
        assert_eq!(
            host.normal_reclaims
                .load(std::sync::atomic::Ordering::Relaxed),
            2
        );
        assert_eq!(
            operations
                .iter()
                .map(|(id, _)| id)
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            1,
            "both programs share one SDK session"
        );
        assert!(operations.iter().any(|(_, op)| op == "syscap"));
        assert_eq!(
            operations
                .iter()
                .filter(|(_, op)| op == "compactSession")
                .count(),
            2
        );
        assert!(!operations.iter().any(|(_, op)| op == "releaseSources"));
        assert_eq!(&operations.last().unwrap().1, "releaseSession");
    }

    #[derive(Debug)]
    struct SourceLeaseReclaimer {
        used: std::sync::Weak<str>,
        unused: std::sync::Weak<str>,
        changed_path: Option<std::path::PathBuf>,
        calls: std::sync::atomic::AtomicUsize,
    }

    impl crate::request::BuildHeapReclaimer for SourceLeaseReclaimer {
        fn reclaim(&self, boundary: crate::request::BuildHeapBoundary) {
            if boundary == crate::request::BuildHeapBoundary::NormalCheckerReleased {
                return;
            }
            assert_eq!(boundary, crate::request::BuildHeapBoundary::CheckerReleased);
            assert!(
                self.used.upgrade().is_some(),
                "program-read text stays captured"
            );
            assert!(
                self.unused.upgrade().is_none(),
                "unused text must be evicted before restoration"
            );
            if let Some(path) = &self.changed_path {
                std::fs::write(path, "changed after startup").unwrap();
            }
            self.calls
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    #[test]
    fn one_shot_checker_evicts_unread_texts_restores_after_reclaim_and_preserves_facts() {
        use super::{ProjectSemanticMode, ProjectSemanticRuntime};
        use std::sync::atomic::{AtomicUsize, Ordering};
        let project = tempfile::tempdir().unwrap();
        let app = project.path().join("entry.ets");
        let unused = project.path().join("unused.ets");
        std::fs::write(&app, "class Model { value: number = 1; }\n@Component\nstruct Page {\n @ObjectLink model: Model = new Model();\n build() {}\n}\n").unwrap();
        std::fs::write(&unused, "invalid unused source").unwrap();
        let store = SourceStore::discover(&[project.path().into()]).unwrap();
        let reclaimer = Arc::new(SourceLeaseReclaimer {
            used: Arc::downgrade(&store.read_text(&app).unwrap()),
            unused: Arc::downgrade(&store.read_text(&unused).unwrap()),
            changed_path: None,
            calls: AtomicUsize::new(0),
        });
        let packages = PackageResolutionPlan::default();
        let metadata = ComponentMetadata::default();
        let roots = [app];
        let checker = ProjectSemanticChecker::new(project.path(), &roots, &packages, &metadata)
            .with_sdk_declaration_roots(&store, &[]);
        let baseline = checker.run().unwrap();
        assert!(!baseline.checked_sources.is_empty());
        assert!(format!("{:?}", baseline.type_facts).contains("Model"));
        let checker = checker
            .with_source_text_eviction(true)
            .with_heap_reclaimer(Some(reclaimer.clone()));
        let optimized = checker.run().unwrap();
        assert_eq!(format!("{optimized:?}"), format!("{baseline:?}"));
        assert_eq!(reclaimer.calls.load(Ordering::Relaxed), 1);
        assert_eq!(
            store.read_text(&unused).unwrap().as_ref(),
            "invalid unused source"
        );

        let unused_before = store.read_text(&unused).unwrap();
        let mut runtime = ProjectSemanticRuntime::default();
        for _ in 0..2 {
            let watched = checker
                .run_for_build_with_runtime(&mut runtime, None, ProjectSemanticMode::Full)
                .unwrap();
            assert_eq!(format!("{watched:?}"), format!("{baseline:?}"));
        }
        assert_eq!(
            reclaimer.calls.load(Ordering::Relaxed),
            1,
            "watch never releases the retained checker"
        );
        assert!(Arc::ptr_eq(
            &unused_before,
            &store.read_text(&unused).unwrap()
        ));
        assert_eq!(
            store.evict_unleased_texts().files,
            0,
            "runtime retains its eager source leases"
        );
    }

    #[test]
    fn one_shot_checker_propagates_a_typed_restore_mismatch() {
        let project = tempfile::tempdir().unwrap();
        let app = project.path().join("entry.ets");
        let unused = project.path().join("unused.ets");
        std::fs::write(&app, "export {}; const n: number = 1;").unwrap();
        std::fs::write(&unused, "unused startup text").unwrap();
        let store = SourceStore::discover(&[project.path().into()]).unwrap();
        let reclaimer = Arc::new(SourceLeaseReclaimer {
            used: Arc::downgrade(&store.read_text(&app).unwrap()),
            unused: Arc::downgrade(&store.read_text(&unused).unwrap()),
            changed_path: Some(unused),
            calls: Default::default(),
        });
        let packages = PackageResolutionPlan::default();
        let metadata = ComponentMetadata::default();
        let roots = [app];
        let error = ProjectSemanticChecker::new(project.path(), &roots, &packages, &metadata)
            .with_sdk_declaration_roots(&store, &[])
            .with_source_text_eviction(true)
            .with_heap_reclaimer(Some(reclaimer))
            .run()
            .unwrap_err();
        assert!(
            matches!(error.downcast_ref::<arkdown_core::SourceReadError>(), Some(arkdown_core::SourceReadError::SnapshotMismatch { path, .. }) if path.ends_with("unused.ets"))
        );
    }

    #[test]
    fn compiler_module_kind_follows_ets_checker_compile_mode_branch() {
        let project = tempfile::tempdir().unwrap();
        let roots = Vec::new();
        let packages = PackageResolutionPlan::default();
        let metadata = ComponentMetadata::default();
        let checker = ProjectSemanticChecker::new(project.path(), &roots, &packages, &metadata)
            .with_compile_mode("esmodule");
        assert_eq!(checker.compiler_options.module, 6);
        assert_eq!(checker.compiler_options.oh_compile_mode, "esmodule");

        let checker = checker.with_compile_mode("jsbundle");
        assert_eq!(checker.compiler_options.module, 1);
        assert_eq!(checker.compiler_options.oh_compile_mode, "jsbundle");
    }

    #[cfg(unix)]
    #[test]
    fn transform_type_queries_preserve_compiler_alias_and_use_snapshot_source() {
        use std::os::unix::fs::symlink;

        let project = tempfile::tempdir().unwrap();
        let physical_root = project.path().join("store/package");
        std::fs::create_dir_all(&physical_root).unwrap();
        let source = Arc::<str>::from(
            "@Component\nstruct Page {\n  @ObjectLink model: Model\n  build() {}\n}\n",
        );
        let physical_file = physical_root.join("Index.ets");
        std::fs::write(&physical_file, source.as_bytes()).unwrap();
        let compiler_root = project.path().join("oh_modules/package");
        std::fs::create_dir_all(compiler_root.parent().unwrap()).unwrap();
        symlink(&physical_root, &compiler_root).unwrap();
        let compiler_file = compiler_root.join("Index.ets");
        let store = SourceStore::discover(std::slice::from_ref(&compiler_root)).unwrap();
        let snapshot_file = store
            .source_by_path(&compiler_file)
            .expect("source alias must resolve through the startup inventory")
            .path
            .clone();
        let snapshot_source = match store
            .source_by_path(&snapshot_file)
            .map(|file| file.cached_content().unwrap())
        {
            Some(SourceContent::Text(source)) => source,
            _ => panic!("fixture source must be captured as text"),
        };
        let sources = BTreeMap::from([(snapshot_file.clone(), snapshot_source)]);
        let inventory =
            ArktsSourceSemanticCollector::new(&ComponentMetadata::default()).collect(&source, true);
        let inventories = BTreeMap::from([(snapshot_file.clone(), inventory)]);
        let roots = vec![compiler_file.clone()];
        let packages = PackageResolutionPlan::default();
        let metadata = ComponentMetadata::default();
        let mut checker = ProjectSemanticChecker::new(project.path(), &roots, &packages, &metadata);
        checker.source_store = Some(&store);

        let identity = checker
            .semantic_source_identity(&compiler_file, &sources)
            .expect("compiler alias must map to the canonical snapshot source");
        assert_eq!(identity.compiler_path, compiler_file);
        assert_eq!(identity.snapshot_path, snapshot_file);
        let queries = checker
            .native_transform_type_queries(&[identity], &sources, &inventories)
            .unwrap();

        assert_eq!(queries.len(), 1);
        assert_eq!(queries[0].file, roots[0]);
        assert_eq!(queries[0].properties.len(), 1);

        let identities =
            rustc_hash::FxHashMap::from_iter([(roots[0].clone(), snapshot_file.clone())]);
        let dependencies = rustc_hash::FxHashMap::default();
        let converter = NativeAnnotationFactsConverter::new(&sources, &identities, &dependencies);
        let (fact_path, fact_source) = converter
            .source(&roots[0])
            .expect("compiler facts must be stored under the Rolldown source identity");
        assert_eq!(fact_path, snapshot_file);
        assert_eq!(fact_source.as_ref(), source.as_ref());
    }
    #[test]
    fn semantic_inputs_use_program_read_overlays_and_exclude_stale_unused_names() {
        use super::{ArktsCheckerSourcePreprocessor, SemanticSourceIdentity, Utf16OffsetMap};
        use arkdown_ets_checker::NativeSourcePreprocessor;
        let project = tempfile::tempdir().unwrap();
        let file = project.path().join("Page.ets");
        let unused = project.path().join("Unused.ets");
        let raw = Arc::<str>::from(
            "// 😀\n@Extend Text.used(color: string) {}\n@Component\nstruct Page {\n  @ObjectLink model: Model\n  build() {}\n}\n",
        );
        let processor = ArktsCheckerSourcePreprocessor;
        let processed = processor.preprocess(&file, Arc::clone(&raw));
        let stale = processor.preprocess(
            &unused,
            Arc::from("@Extend Text.unused() {}\n@Extend(Text) function ghost() {}"),
        );
        let read = BTreeMap::from([(file.clone(), processed.clone()), (unused, stale)]);
        let roots = vec![file.clone()];
        let packages = PackageResolutionPlan::default();
        let metadata = ComponentMetadata::default();
        let checker = ProjectSemanticChecker::new(project.path(), &roots, &packages, &metadata);
        let identities = vec![SemanticSourceIdentity {
            compiler_path: file.clone(),
            snapshot_path: file.clone(),
        }];
        let inputs = checker.semantic_source_inputs(&identities, &read);
        assert_eq!(inputs.sources.len(), 1);
        assert!(inputs.ui_props.contains("used"));
        assert!(inputs.ui_props.contains("$model"));
        assert!(!inputs.ui_props.contains("unused"));
        assert!(!inputs.new_extend_names.contains("ghost"));
        let query = checker
            .native_transform_type_queries(
                &identities,
                &inputs.sources,
                &inputs.semantic_inventories,
            )
            .unwrap();
        assert_eq!(query.len(), 1);
        let property = &query[0].properties[0];
        let offsets = Utf16OffsetMap::new(&processed.text);
        let span = offsets.span(property.pos, property.end, &file).unwrap();
        assert_eq!(
            &processed.text[span.start as usize..span.end as usize],
            "model"
        );
        assert_ne!(processed.text, raw, "Extend preprocessing changes offsets");
        let identity_map = rustc_hash::FxHashMap::from_iter([(file.clone(), file.clone())]);
        let dependencies = rustc_hash::FxHashMap::default();
        let converter =
            NativeAnnotationFactsConverter::new(&inputs.sources, &identity_map, &dependencies);
        assert_eq!(converter.source(&file).unwrap().1, &processed.text);
    }
}
