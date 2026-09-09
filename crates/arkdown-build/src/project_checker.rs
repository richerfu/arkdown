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
    NativeOhSdkPluginHost, NativeProgram, NativeProgramFileChanges, NativeProgramSourceGraph,
    NativeResolvedTypeIdentity, NativeTypeQueryRange, NativeTypeSession,
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct ProjectSemanticRuntimeKey {
    project_root: PathBuf,
    module_root: PathBuf,
    cache_path: PathBuf,
}

struct ProjectSemanticRuntimeEntry {
    executable: PathBuf,
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
        executable: &Path,
        project_root: &Path,
        prepared: &PreparedSemanticCheck,
        changes: &NativeProgramFileChanges,
    ) -> Result<(&'a mut NativeTypeSession, NativeProgram)> {
        let sdk_plugin_callbacks = prepared.sdk_plugin_host.is_some();
        let entry = self.sessions.remove(&key);
        let entry = match entry {
            Some(mut entry)
                if entry.executable == executable
                    && entry.sdk_plugin_callbacks == sdk_plugin_callbacks =>
            {
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
                    executable,
                    project_root,
                    prepared.sources.clone(),
                    prepared.sdk_plugin_host.clone(),
                )?;
                let program = session.create_program(&prepared.roots, &prepared.options)?;
                ProjectSemanticRuntimeEntry {
                    executable: executable.to_path_buf(),
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

    pub(crate) fn close(&mut self) -> Result<()> {
        let mut entries = std::mem::take(&mut self.sessions)
            .into_iter()
            .collect::<Vec<_>>();
        entries.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
        let mut first_error = None;
        for (_, mut entry) in entries {
            if let Err(error) = entry.session.release(entry.program) {
                first_error.get_or_insert(error);
            }
        }
        if let Some(error) = first_error {
            return Err(error.into());
        }
        Ok(())
    }
}

struct PreparedSemanticCheck {
    roots: Vec<PathBuf>,
    options: NativeCompilerOptions,
    sdk_plugin_host: Option<Arc<dyn NativeOhSdkPluginHost>>,
    ui_props: BTreeSet<String>,
    new_extend_names: BTreeSet<String>,
    sources: BTreeMap<PathBuf, Arc<str>>,
    semantic_inventories: BTreeMap<PathBuf, ArktsSourceSemanticInventory>,
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
    typescript_executable: Option<PathBuf>,
    sdk_plugin_host: Option<Arc<dyn NativeOhSdkPluginHost>>,
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
            typescript_executable: None,
            sdk_plugin_host: None,
            crossplatform_dependencies: FxHashMap::default(),
        }
    }

    pub fn with_typescript_executable(mut self, executable: Option<&Path>) -> Self {
        self.typescript_executable = executable.map(Path::to_path_buf);
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
                tracing::warn!("ArkTS: Invalid maxFlowDepth for control flow analysis.The value of maxFlowDepth ranges from 2000 to 65535.\nIf the modification does not take effect, set maxFlowDepth to the default value.");
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
            .flat_map(|root| source_store.text_sources_under(root))
            .map(|(path, _)| path.to_path_buf())
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
        self.run_inner(None, None)
    }

    pub(crate) fn run_with_runtime(
        &self,
        runtime: &mut ProjectSemanticRuntime,
        invalidation: Option<&WatchInvalidationBatch>,
    ) -> Result<ProjectSemanticCheckReport> {
        self.run_inner(Some(runtime), invalidation)
    }

    fn run_inner(
        &self,
        runtime: Option<&mut ProjectSemanticRuntime>,
        invalidation: Option<&WatchInvalidationBatch>,
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
        let mut ui_props = BTreeSet::new();
        let mut new_extend_names = BTreeSet::new();
        let mut sources = BTreeMap::new();
        let mut semantic_inventories = BTreeMap::new();
        if let Some(store) = self.source_store {
            let semantic_collector = ArktsSourceSemanticCollector::new(self.component_metadata);
            let prepared = store
                .source_files()
                .into_par_iter()
                .filter_map(|file| {
                    let Some(SourceContent::Text(text)) = file.cached_content() else {
                        return None;
                    };
                    let processed = if Self::is_type_script_preprocess_source(&file.path) {
                        Some(ArktsExtendSource::new(
                            text.as_ref(),
                            arkdown_core::ArktsLanguageVersion::V1_1,
                        ))
                    } else {
                        None
                    };
                    let legacy_names = processed
                        .as_ref()
                        .map(|source| source.legacy_names().iter().cloned().collect::<Vec<_>>())
                        .unwrap_or_default();
                    let modern_names = processed
                        .as_ref()
                        .map(|source| source.modern_names().iter().cloned().collect::<Vec<_>>())
                        .unwrap_or_default();
                    let content = processed.map_or_else(
                        || Arc::clone(text),
                        |processed| Arc::<str>::from(processed.as_str()),
                    );
                    let semantic_inventory = Self::is_type_script_preprocess_source(&file.path)
                        .then(|| {
                            semantic_collector
                                .collect(content.as_ref(), self.is_ui_diagnostic_source(&file.path))
                        });
                    Some((
                        file.path.clone(),
                        content,
                        semantic_inventory,
                        legacy_names,
                        modern_names,
                    ))
                })
                .collect::<Vec<_>>();
            for (path, content, semantic_inventory, legacy_names, modern_names) in prepared {
                if let Some(inventory) = semantic_inventory {
                    ui_props.extend(inventory.diagnostic_names().iter().cloned());
                    semantic_inventories.insert(path.clone(), inventory);
                }
                ui_props.extend(legacy_names);
                new_extend_names.extend(modern_names);
                sources.insert(path, content);
            }
        }
        let prepared = PreparedSemanticCheck {
            roots,
            options,
            sdk_plugin_host,
            ui_props,
            new_extend_names,
            sources,
            semantic_inventories,
        };
        let executable =
            NativeTypeSession::resolve_executable(self.typescript_executable.as_deref())?;
        if let Some(runtime) = runtime {
            let key = ProjectSemanticRuntimeKey {
                project_root: self.project_root.to_path_buf(),
                module_root: self.compiler_options.oh_module_path.clone(),
                cache_path: self.cache_path.clone().unwrap_or_default(),
            };
            let changes = ProjectSemanticRuntime::file_changes(invalidation);
            let (session, program) =
                runtime.program(key, &executable, self.project_root, &prepared, &changes)?;
            return self.check_program(session, &program, &prepared);
        }
        let mut session = NativeTypeSession::start(
            &executable,
            self.project_root,
            prepared.sources.clone(),
            prepared.sdk_plugin_host.clone(),
        )?;
        let program = session.create_program(&prepared.roots, &prepared.options)?;
        let report = self.check_program(&mut session, &program, &prepared)?;
        session.release(program)?;
        Ok(report)
    }

    fn check_program(
        &self,
        session: &mut NativeTypeSession,
        program: &NativeProgram,
        prepared: &PreparedSemanticCheck,
    ) -> Result<ProjectSemanticCheckReport> {
        let source_graph = session.source_graph(program)?;
        let checked_sources = source_graph
            .files
            .iter()
            .map(|file| file.file_name.clone())
            .filter(|file| prepared.sources.contains_key(file) && Self::is_checkable_root(file))
            .collect::<Vec<_>>();
        let linter_started = Instant::now();
        let linter = session.arkts_linter_diagnostics(program)?;
        tracing::info!(
            elapsed_ms = linter_started.elapsed().as_millis(),
            files = source_graph.files.len(),
            "ArkTS linter diagnostics completed"
        );
        let diagnostics_started = Instant::now();
        let mut result = session.diagnostics(program)?;
        if self.check_declarations {
            let declaration_sources = source_graph
                .files
                .iter()
                .map(|file| file.file_name.clone())
                .filter(|file| Self::is_declaration_diagnostic_source(file))
                .collect::<Vec<_>>();
            result.extend(session.declaration_diagnostics(program, &declaration_sources)?);
        }
        tracing::info!(
            elapsed_ms = diagnostics_started.elapsed().as_millis(),
            files = source_graph.files.len(),
            "ArkTS semantic diagnostics completed"
        );
        let annotation_files = source_graph
            .files
            .iter()
            .map(|file| file.file_name.clone())
            .filter(|file| {
                prepared.sources.contains_key(file) && Self::is_type_script_preprocess_source(file)
            })
            .collect::<Vec<_>>();
        let facts_started = Instant::now();
        let native_type_facts = session.annotation_transform_infos(program, &annotation_files)?;
        let transform_queries = self.native_transform_type_queries(
            &annotation_files,
            &prepared.sources,
            &prepared.semantic_inventories,
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
        let type_facts = NativeAnnotationFactsConverter::new(
            &prepared.sources,
            &self.crossplatform_dependencies,
        )
        .convert(native_type_facts, native_transform_type_facts)?;
        let mut linter = self.prepare_linter_diagnostics(linter)?;
        self.apply_ui_diagnostic_rules(&mut linter, &prepared.ui_props, &prepared.new_extend_names);
        self.apply_ui_diagnostic_rules(&mut result, &prepared.ui_props, &prepared.new_extend_names);
        let ignored = self.ignored_diagnostic_files(&source_graph);
        linter.retain(|diagnostic| self.keep_diagnostic(diagnostic, &ignored));
        result.retain(|diagnostic| self.keep_diagnostic(diagnostic, &ignored));
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
                .text_sources_under(&directory)
                .into_iter()
                .filter(|(path, _)| {
                    path.parent() == Some(directory.as_path())
                        && path
                            .file_name()
                            .and_then(|name| name.to_str())
                            .is_some_and(|name| name.ends_with(".d.ts"))
                })
                .map(|(path, _)| path.to_path_buf())
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
    /// this selection out of TSGO avoids a second full-project AST traversal;
    /// TSGO remains responsible for every returned type identity.
    fn native_transform_type_queries(
        &self,
        files: &[PathBuf],
        sources: &BTreeMap<PathBuf, Arc<str>>,
        inventories: &BTreeMap<PathBuf, ArktsSourceSemanticInventory>,
    ) -> Result<Vec<NativeArktsTransformTypeQuery>> {
        files
            .iter()
            .filter_map(|file| {
                inventories
                    .get(file)
                    .filter(|inventory| inventory.has_type_queries())
                    .map(|inventory| (file, inventory))
            })
            .map(|(file, inventory)| {
                let source = sources.get(file).ok_or_else(|| {
                    anyhow::anyhow!(
                        "ARKDOWN_TYPE_QUERY_SOURCE: unavailable OXC source {}",
                        file.display()
                    )
                })?;
                let offsets = Utf16OffsetMap::new(source);
                Ok(NativeArktsTransformTypeQuery {
                    file: file.clone(),
                    properties: offsets.query_ranges(inventory.property_queries(), file)?,
                    builder_accesses: offsets
                        .query_ranges(inventory.builder_access_queries(), file)?,
                    member_accesses: offsets
                        .query_ranges(inventory.member_access_queries(), file)?,
                })
            })
            .collect()
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
    crossplatform_dependencies: &'a FxHashMap<(String, String), CrossplatformSourceCollection>,
}

impl<'a> NativeAnnotationFactsConverter<'a> {
    fn new(
        sources: &'a BTreeMap<PathBuf, Arc<str>>,
        crossplatform_dependencies: &'a FxHashMap<(String, String), CrossplatformSourceCollection>,
    ) -> Self {
        Self {
            sources,
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
            let source = self.sources.get(&info.file_name).ok_or_else(|| {
                anyhow::anyhow!(
                    "ARKDOWN_TYPE_FACTS_SOURCE: compiler returned facts for unavailable source {}",
                    info.file_name.display()
                )
            })?;
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
                info.file_name,
                ArktsFileTypeFacts {
                    declarations,
                    uses,
                    imports,
                    ..ArktsFileTypeFacts::default()
                },
            );
        }
        for info in transform_infos {
            let source = self.sources.get(&info.file_name).ok_or_else(|| {
                anyhow::anyhow!(
                    "ARKDOWN_TYPE_FACTS_SOURCE: compiler returned facts for unavailable source {}",
                    info.file_name.display()
                )
            })?;
            let offsets = Utf16OffsetMap::new(source);
            let facts = files
                .entry(info.file_name.clone())
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
    use super::ProjectSemanticChecker;
    use arkdown_ets_transform::transform::component_meta::ComponentMetadata;
    use arkdown_resolver::PackageResolutionPlan;

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
}
