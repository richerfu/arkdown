//! Watch mode coordinator.
//!
//! Wraps `arkdown-watcher`'s `SessionWatcher` and drives incremental rebuilds
//! when source files change.  This is a thin coordinator that:
//!
//! 1. Determines which directories to watch based on the build mode
//!    (full watch vs preview resources plus SuperVisual inputs).
//! 2. Computes an importer map snapshot from the previous build's
//!    `importer_map.json` files for M2 hot-reload delta gating.
//! 3. Classifies each change batch as **hot** (ETS/TS source only) or **cold**
//!    (config/resource change) using `classify_watch_change`.
//!    - Hot → skips resource + profile stages (bundle-only rebuild).
//!    - Cold → full pipeline rebuild.
//! 4. Calls `BuildService::execute` with the appropriate `skip_*` flags set.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use rustc_hash::{FxHashMap, FxHashSet};

use arkdown_core::ArkdownConfig;
use arkdown_path::{lexical_normalize, AbsPath, ArtifactPathSerializer};
use arkdown_session::BuildSession;
use arkdown_watcher::{SessionWatcher, WatchChangeKind, WatchInput, WatchInvalidationBatch};

use arkdown::plugins::factory::PackagePlugin;

use crate::layout::ModuleBuildLayout;
use crate::request::{
    AbcBackendChoice, AnalyzeModeChoice, BuildAction, BuildRequest, OhpmModeChoice,
};

use super::BuildService;
use crate::bundle_plan::BundleRuntime;

/// Input for the watch stage.
///
/// Individual fields keep the long-lived watcher independent from transient
/// N-API request values and avoid retaining unrelated build state.
pub(super) struct WatchStageInput {
    pub(super) bundle_runtime: BundleRuntime,
    pub(super) config: Arc<ArkdownConfig>,
    pub(super) session: Arc<BuildSession>,
    /// Build target string (e.g. "hap", "hsp", "har").
    pub(super) target: String,
    /// Build mode string (e.g. "debug", "release").
    pub(super) mode: String,
    pub(super) analyze: AnalyzeModeChoice,
    pub(super) ohpm_mode: OhpmModeChoice,
    /// Optional module filter.
    pub(super) module_filter: Option<String>,
    /// Optional product filter.
    pub(super) product_filter: Option<String>,
    /// Project root path.
    pub(super) project_root: PathBuf,
    /// ABC backend choice for incremental rebuilds.
    pub(super) abc_backend: AbcBackendChoice,
    pub(super) build_target_name: Option<String>,
    pub(super) build_version: Option<String>,
    pub(super) coverage: bool,
    pub(super) ohos_debug_asan: bool,
    pub(super) ohos_enable_hwasan: bool,
    pub(super) ohos_enable_tsan: bool,
    pub(super) ohos_enable_ubsan: bool,
    pub(super) ohos_enable_binxo: bool,
    pub(super) enable_zig: bool,
    pub(super) disable_abc_cache: bool,
    pub(super) cache_file: Option<PathBuf>,
    pub(super) checker: Option<String>,
    pub(super) is_crossplatform: bool,
    pub(super) ignore_crossplatform_check: bool,
    pub(super) cache: bool,
    pub(super) package_plugins: Vec<PackagePlugin>,
    pub(super) node_executable: Option<PathBuf>,
    pub(super) custom_task_input_paths: Vec<PathBuf>,
    pub(super) pack_compress_level: Option<u32>,
    pub(super) align_device_types: Vec<String>,
    pub(super) enable_sign_task: bool,
    pub(super) collect_debug_symbol: Option<bool>,
    pub(super) ui_transform_optimization: bool,
    pub(super) integrated_hsp_use_get_current_bundle_name: bool,
    pub(super) byte_code_har_integrated_optimization: bool,
    pub(super) keep_dependency: bool,
    pub(super) hvigor_properties: crate::request::HvigorRuntimeProperties,
    pub(super) parameter_file_override: Option<PathBuf>,
    pub(super) hvigor_cache_dir: Option<PathBuf>,
    pub(super) action: BuildAction,
    /// When `true`, only `src/main/resources/` directories are watched.
    pub(super) preview_mode: bool,
    pub(super) check_entry: bool,
    /// Persistent Hvigor HotReloadArkTS worker semantics.
    pub(super) hot_reload_mode: bool,
}

pub(super) struct WatchStageBuildInput<'a> {
    pub(super) bundle_runtime: BundleRuntime,
    pub(super) config: Arc<ArkdownConfig>,
    pub(super) session: Arc<BuildSession>,
    pub(super) args: &'a BuildRequest,
    pub(super) project_root: PathBuf,
    pub(super) abc_backend: AbcBackendChoice,
    pub(super) coverage: bool,
    pub(super) disable_abc_cache: bool,
    pub(super) cache_file: Option<PathBuf>,
    pub(super) package_plugins: Vec<PackagePlugin>,
    pub(super) preview_mode: bool,
    pub(super) hot_reload_mode: bool,
}

impl WatchStageInput {
    pub(super) fn from_build(input: WatchStageBuildInput<'_>) -> Self {
        let WatchStageBuildInput {
            bundle_runtime,
            config,
            session,
            args,
            project_root,
            abc_backend,
            coverage,
            disable_abc_cache,
            cache_file,
            package_plugins,
            preview_mode,
            hot_reload_mode,
        } = input;
        Self {
            bundle_runtime,
            config,
            session,
            target: args.target.clone(),
            mode: args.mode.clone(),
            analyze: args.analyze,
            ohpm_mode: args.ohpm_mode,
            module_filter: args.module.clone(),
            product_filter: args.product.clone(),
            project_root,
            abc_backend,
            build_target_name: args.build_target_name.clone(),
            build_version: args.build_version.clone(),
            coverage,
            ohos_debug_asan: args.ohos_debug_asan,
            ohos_enable_hwasan: args.ohos_enable_hwasan,
            ohos_enable_tsan: args.ohos_enable_tsan,
            ohos_enable_ubsan: args.ohos_enable_ubsan,
            ohos_enable_binxo: args.ohos_enable_binxo,
            enable_zig: args.enable_zig,
            disable_abc_cache,
            cache_file,
            checker: args.checker.clone(),
            is_crossplatform: args.is_crossplatform,
            ignore_crossplatform_check: args.ignore_crossplatform_check,
            cache: args.cache,
            package_plugins,
            node_executable: args.node_executable.clone(),
            custom_task_input_paths: args.custom_task_input_paths.clone(),
            pack_compress_level: args.pack_compress_level,
            align_device_types: args.align_device_types.clone(),
            enable_sign_task: args.enable_sign_task,
            collect_debug_symbol: args.collect_debug_symbol,
            ui_transform_optimization: args.ui_transform_optimization,
            integrated_hsp_use_get_current_bundle_name: args
                .integrated_hsp_use_get_current_bundle_name,
            byte_code_har_integrated_optimization: args.byte_code_har_integrated_optimization,
            keep_dependency: args.keep_dependency,
            hvigor_properties: args.hvigor_properties.clone(),
            parameter_file_override: args.parameter_file_override.clone(),
            hvigor_cache_dir: args.hvigor_cache_dir.clone(),
            action: args.action,
            preview_mode,
            check_entry: args.check_entry,
            hot_reload_mode,
        }
    }
}

/// Watch stage coordinator.
pub(super) struct WatchStage;

impl WatchStage {
    /// Start watching for file changes and drive incremental rebuilds.
    ///
    /// This call blocks until the user sends `Ctrl+C` or the watcher
    /// encounters a fatal error.
    pub(super) async fn run(input: WatchStageInput) -> Result<()> {
        let mut bundle_runtime = input.bundle_runtime;
        let config_for_watch = input.config.clone();
        let arkts_version = input.session.arkts_language_version;
        let session = input.session;
        let preview_mode = input.preview_mode;
        let check_entry = input.check_entry;
        let target = input.target;
        let mode = input.mode;
        let analyze = input.analyze;
        let ohpm_mode = input.ohpm_mode;
        let module_filter = input.module_filter;
        let product_filter = input.product_filter;
        let project_root = input.project_root;
        let abc_backend = input.abc_backend;
        let build_target_name = input.build_target_name;
        let build_version = input.build_version;
        let coverage = input.coverage;
        let ohos_debug_asan = input.ohos_debug_asan;
        let ohos_enable_hwasan = input.ohos_enable_hwasan;
        let ohos_enable_tsan = input.ohos_enable_tsan;
        let ohos_enable_ubsan = input.ohos_enable_ubsan;
        let ohos_enable_binxo = input.ohos_enable_binxo;
        let enable_zig = input.enable_zig;
        let disable_abc_cache = input.disable_abc_cache;
        let cache_file = input.cache_file;
        let checker = input.checker;
        let is_crossplatform = input.is_crossplatform;
        let ignore_crossplatform_check = input.ignore_crossplatform_check;
        let cache = input.cache;
        let package_plugins = input.package_plugins;
        let node_executable = input.node_executable;
        let custom_task_input_paths = input.custom_task_input_paths;
        let pack_compress_level = input.pack_compress_level;
        let align_device_types = input.align_device_types;
        let enable_sign_task = input.enable_sign_task;
        let collect_debug_symbol = input.collect_debug_symbol;
        let ui_transform_optimization = input.ui_transform_optimization;
        let integrated_hsp_use_get_current_bundle_name =
            input.integrated_hsp_use_get_current_bundle_name;
        let byte_code_har_integrated_optimization = input.byte_code_har_integrated_optimization;
        let keep_dependency = input.keep_dependency;
        let hvigor_properties = input.hvigor_properties;
        let parameter_file_override = input.parameter_file_override;
        let hvigor_cache_dir = input.hvigor_cache_dir;
        let action = input.action;
        let hot_reload_mode = input.hot_reload_mode;

        let watch_plan = WatchSessionPlan::from_session(
            &session,
            preview_mode,
            hot_reload_mode,
            &custom_task_input_paths,
        );
        let fingerprint_baseline = watch_plan.input.fingerprint_baseline.clone();
        let custom_input_files = watch_plan.custom_input_files;
        let custom_input_dirs = watch_plan.custom_input_dirs;
        let watcher = SessionWatcher::new(watch_plan.input);

        tracing::info!("watching for changes");
        if preview_mode {
            tracing::info!("preview watch enabled for resources and SuperVisual inputs");
        } else if hot_reload_mode {
            tracing::info!("Hvigor HotReloadArkTS persistent worker enabled");
        }

        let rt = tokio::runtime::Handle::current();

        tokio::task::spawn_blocking(move || {
            let watch_result = watcher.watch(|events| {
                let invalidation = WatchInvalidationBatch::from_watch_events_with_custom_inputs(
                    events,
                    &fingerprint_baseline,
                    &custom_input_files,
                    &custom_input_dirs,
                );
                if invalidation.is_empty() {
                    return Ok(());
                }
                let changed = &invalidation.changed_files;
                let change_kind = if preview_mode {
                    WatchChangeKind::Cold
                } else {
                    invalidation.change_kind()
                };

                // M2: Hot-reload delta computation.
                let changed_paths: Vec<WatchChangedPath> = changed
                    .iter()
                    .map(|path| WatchChangedPath::new(path, &project_root))
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|error| format!("serialize changed path failed: {error}"))?;

                let importer_maps = WatchImporterMapInventory::load(&config_for_watch);
                let mut affected_chunks: FxHashSet<String> = FxHashSet::default();
                for changed_path in &changed_paths {
                    for key in importer_maps.entries.keys() {
                        if changed_path.matches_importer_key(key) {
                            affected_chunks.insert(key.clone());
                        }
                    }
                }

                if !hot_reload_mode
                    && matches!(change_kind, WatchChangeKind::Hot)
                    && !importer_maps.entries.is_empty()
                    && affected_chunks.is_empty()
                {
                    tracing::info!(
                        changed_files = changed.len(),
                        "changed files are not in module graph; skipping rebuild"
                    );
                    return Ok(());
                }

                // M3: Preview mode — write changed resource list for IDE polling.
                if preview_mode {
                    let preview_json = serde_json::json!({
                        "type": "resource_changed",
                        "files": changed
                            .iter()
                            .map(|path| ArtifactPathSerializer::path_to_slash_string(path))
                            .collect::<std::result::Result<Vec<_>, _>>()
                            .map_err(|error| {
                                format!("serialize preview changed path failed: {error}")
                            })?,
                    });
                    for module in &config_for_watch.modules {
                        let out_dir = ModuleBuildLayout::for_product(
                            &module.src_path,
                            &config_for_watch.product.name,
                        )
                        .outputs_root();
                        let _ = std::fs::create_dir_all(&out_dir);
                        let _ = std::fs::write(
                            out_dir.join("preview_changes.json"),
                            serde_json::to_string_pretty(&preview_json).unwrap_or_default(),
                        );
                    }
                }

                let mode_label = if hot_reload_mode {
                    "hot reload patch"
                } else {
                    match change_kind {
                        WatchChangeKind::Hot => "hot rebuild (bundle-only)",
                        WatchChangeKind::Cold => {
                            if preview_mode {
                                "resource reload"
                            } else {
                                "full rebuild"
                            }
                        }
                    }
                };
                tracing::info!(
                    changed_files = changed.len(),
                    mode = mode_label,
                    affected_chunks = if affected_chunks.is_empty() {
                        "all".to_string()
                    } else {
                        affected_chunks.len().to_string()
                    },
                    "watch rebuild started"
                );

                // For hot changes, skip resource and profile stages to only
                // rerun the bundle → bytecode → package pipeline.
                let is_hot = hot_reload_mode || matches!(change_kind, WatchChangeKind::Hot);
                let rebuild_args = BuildRequest {
                    action,
                    ohpm_mode,
                    target: target.clone(),
                    mode: mode.clone(),
                    analyze,
                    arkts_version,
                    module: module_filter.clone(),
                    product: product_filter.clone(),
                    project: Some(project_root.clone()),
                    build_target_name: build_target_name.clone(),
                    build_version: build_version.clone(),
                    watch: false,
                    coverage,
                    scope: None,
                    ohos_test_coverage: None,
                    coverage_path_filter: None,
                    coverage_mode: None,
                    unit_test_replace_page: None,
                    fast_build_app: false,
                    enable_sign_task,
                    ohos_debug_asan,
                    ohos_enable_hwasan,
                    ohos_enable_tsan,
                    ohos_enable_ubsan,
                    ohos_enable_binxo,
                    enable_zig,
                    preview: false,
                    check_entry,
                    preview_transform_mode: preview_mode,
                    disable_abc_cache,
                    cache_file: cache_file.clone(),
                    abc_backend,
                    checker: checker.clone(),
                    is_crossplatform,
                    ignore_crossplatform_check,
                    cache,
                    skip_resource: is_hot,
                    skip_profile: is_hot,
                    watch_invalidation: Some(invalidation),
                    custom_task_input_paths: custom_task_input_paths.clone(),
                    pack_compress_level,
                    align_device_types: align_device_types.clone(),
                    collect_debug_symbol,
                    ui_transform_optimization,
                    integrated_hsp_use_get_current_bundle_name,
                    byte_code_har_integrated_optimization,
                    keep_dependency,
                    hvigor_properties: hvigor_properties.clone(),
                    parameter_file_override: parameter_file_override.clone(),
                    node_executable: node_executable.clone(),
                    hvigor_cache_dir: hvigor_cache_dir.clone(),
                };
                let command =
                    BuildService::new(rebuild_args).with_package_plugins(package_plugins.clone());
                if let Err(e) =
                    rt.block_on(command.execute_with_bundle_runtime(&mut bundle_runtime))
                {
                    tracing::error!(error = %format!("{e:#}"), "watch rebuild failed");
                }
                Ok(())
            });
            let close_result = rt
                .block_on(bundle_runtime.close())
                .map_err(|error| arkdown_watcher::WatchError::Build(error.to_string()));
            watch_result?;
            close_result
        })
        .await
        .map_err(|error| anyhow::anyhow!("watch stage task failed: {error}"))?
        .map_err(|error| anyhow::anyhow!("watch stage error: {error}"))
    }
}

struct WatchSessionPlan {
    input: WatchInput,
    custom_input_files: Vec<PathBuf>,
    custom_input_dirs: Vec<PathBuf>,
}

impl WatchSessionPlan {
    fn from_session(
        session: &BuildSession,
        preview_mode: bool,
        hot_reload_mode: bool,
        custom_task_input_paths: &[PathBuf],
    ) -> Self {
        let mut watch_dirs = FxHashSet::default();
        let mut watch_files = FxHashSet::default();
        let mut custom_input_files = FxHashSet::default();
        let mut custom_input_dirs = FxHashSet::default();
        for plan in session.watch_inputs.iter() {
            if preview_mode {
                watch_dirs.extend(plan.resource_roots.iter().cloned());
                watch_dirs.extend(plan.visual_roots.iter().cloned());
            } else {
                watch_dirs.extend(plan.source_roots.iter().cloned());
                watch_dirs.extend(plan.visual_roots.iter().cloned());
                if !hot_reload_mode {
                    watch_dirs.extend(plan.resource_roots.iter().cloned());
                    watch_files.extend(plan.config_files.iter().cloned());
                }
            }
        }
        for input in custom_task_input_paths {
            let input = Self::normalize_custom_input(input);
            if input.is_dir() {
                watch_dirs.insert(input.clone());
                custom_input_dirs.insert(input);
            } else {
                watch_files.insert(input.clone());
                custom_input_files.insert(input);
            }
        }

        let mut watch_dirs = watch_dirs.into_iter().collect::<Vec<_>>();
        watch_dirs.sort();
        let mut watch_files = watch_files.into_iter().collect::<Vec<_>>();
        watch_files.sort();
        let mut custom_input_files = custom_input_files.into_iter().collect::<Vec<_>>();
        custom_input_files.sort();
        let mut custom_input_dirs = custom_input_dirs.into_iter().collect::<Vec<_>>();
        custom_input_dirs.sort();
        Self {
            input: WatchInput::new()
                .with_dirs(watch_dirs)
                .with_files(watch_files),
            custom_input_files,
            custom_input_dirs,
        }
    }

    fn normalize_custom_input(input: &Path) -> PathBuf {
        AbsPath::canonicalize(input)
            .map(AbsPath::into_path_buf)
            .unwrap_or_else(|_| lexical_normalize(input))
    }
}

struct WatchImporterMapInventory {
    entries: FxHashMap<String, Vec<String>>,
}

impl WatchImporterMapInventory {
    fn load(config: &ArkdownConfig) -> Self {
        let entries = config
            .modules
            .iter()
            .filter_map(|module| {
                let path = ModuleBuildLayout::for_product(&module.src_path, &config.product.name)
                    .importer_map_json();
                std::fs::read_to_string(path).ok().and_then(|content| {
                    serde_json::from_str::<FxHashMap<String, Vec<String>>>(&content).ok()
                })
            })
            .flatten()
            .collect();
        Self { entries }
    }
}

struct WatchChangedPath {
    candidates: FxHashSet<String>,
}

impl WatchChangedPath {
    fn new(path: &Path, project_root: &Path) -> std::result::Result<Self, String> {
        let mut changed_path = Self {
            candidates: FxHashSet::default(),
        };
        changed_path.insert_path(path)?;
        if let Ok(relative) = ArtifactPathSerializer::source_file_key(project_root, path) {
            changed_path.insert_slash_path(relative.into_string());
        }
        Ok(changed_path)
    }

    fn matches_importer_key(&self, key: &str) -> bool {
        let importer_key = WatchImporterKey::new(key);
        self.candidates
            .iter()
            .any(|candidate| importer_key.matches_candidate(candidate))
    }

    fn insert_path(&mut self, path: &Path) -> std::result::Result<(), String> {
        let slash_path = ArtifactPathSerializer::path_to_slash_string(path)
            .map_err(|error| error.to_string())?;
        self.insert_slash_path(slash_path);
        Ok(())
    }

    fn insert_slash_path(&mut self, slash_path: String) {
        if slash_path.is_empty() {
            return;
        }
        if let Some(extensionless) = WatchImporterKey::new(&slash_path).extensionless_path() {
            self.candidates.insert(extensionless);
        }
        self.candidates.insert(slash_path);
    }
}

struct WatchImporterKey<'a> {
    key: &'a str,
}

impl<'a> WatchImporterKey<'a> {
    fn new(key: &'a str) -> Self {
        Self { key }
    }

    fn matches_candidate(&self, candidate: &str) -> bool {
        if self.key.is_empty() || candidate.is_empty() {
            return false;
        }
        if self.key == candidate {
            return true;
        }
        let key_path = Path::new(self.key);
        let candidate_path = Path::new(candidate);
        key_path.ends_with(candidate_path) || candidate_path.ends_with(key_path)
    }

    fn extensionless_path(&self) -> Option<String> {
        let path = Path::new(self.key);
        path.extension()?;
        let file_stem = path.file_stem()?;
        let mut extensionless = path.to_path_buf();
        extensionless.set_file_name(file_stem);
        Some(ArtifactPathSerializer::rel_to_slash(&extensionless))
    }
}
