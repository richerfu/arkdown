//! Bundle plan construction and bundle stage execution.
//!
//! Converts a `LoaderJson` (written by `arkdown-profile`) into a typed
//! [`BundlePlan`] and runs the Rolldown bundle stage.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rolldown_plugin::__inner::SharedPluginable;
use rustc_hash::FxHashSet;
use thiserror::Error;

use arkdown::plugins::ets_transform::{EtsTransformPlugin, RolldownEtsTransformPlan};
use arkdown::{
    BundleCachePolicy, BundleInput, BundleMockConfig, BundleModuleMetaInfoFile, BundleOutput,
    BundleOutputOptions, BundlePlan, BundlePluginSet, BundleRuntimePackageContext,
    BundleRuntimePackageInfo, BundleSourceMapPackageInfo, Bundler, HvigorHarImportees,
    LoaderResourceJoin, PreloadSystemSoTask, SourceMapPolicy,
};
use arkdown_config::{build_profile::parse_api_version, ArkdownCustomConfig};
use arkdown_core::{ArkdownConfig, ArkdownError, ArktsLanguageVersion};
use arkdown_path::{ArtifactPathSerializer, NormalizedOhmUrl};
use arkdown_profile::{LoaderJson, PkgContextInfoOpt};
use arkdown_session::{ModuleCompileContext, PackageSnapshot};

use crate::layout::{ModuleBuildLayout, ModuleSourceCacheLayout, ModuleSourceLayout};

// ---------------------------------------------------------------------------
// BundlePlan construction
// ---------------------------------------------------------------------------

pub struct BundlePlanPackageInfo {
    pub source_map: BundleSourceMapPackageInfo,
    pub runtime: BundleRuntimePackageInfo,
    package_snapshots: rustc_hash::FxHashMap<PathBuf, Arc<PackageSnapshot>>,
}

impl BundlePlanPackageInfo {
    /// Project the immutable package snapshots needed by loader fallback
    /// contexts. This replaces the old bundle-stage `oh-package.json5` rereads.
    pub fn from_module_context(
        source_map: BundleSourceMapPackageInfo,
        runtime: BundleRuntimePackageInfo,
        context: &ModuleCompileContext,
    ) -> Self {
        let mut package_snapshots = rustc_hash::FxHashMap::default();
        if let Some(package) = context.module.package.as_ref() {
            package_snapshots.insert(context.module.root.clone(), Arc::clone(package));
        }
        for node in context.deps.nodes.values() {
            if let (Some(root), Some(package)) = (node.package_root.as_ref(), node.package.as_ref())
            {
                package_snapshots
                    .entry(root.clone())
                    .or_insert_with(|| Arc::clone(package));
            }
        }
        Self {
            source_map,
            runtime,
            package_snapshots,
        }
    }
}

/// Effective Ark bytecode target selected by latest hvigor's ModuleMode path.
///
/// `module_mode.js::generateEs2AbcCmd` uses
/// `projectConfig.compatibleSdkVersion` (and its stage), rather than the API
/// level of the SDK installation that provides `es2abc`. Keeping this in one
/// value object prevents the ETS transform and SDK compiler from targeting
/// different language/runtime versions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HvigorCompileTarget {
    api_version: u32,
    api_sub_version: Option<String>,
}

impl HvigorCompileTarget {
    pub fn from_loader_json(loader_json: &LoaderJson, config: &ArkdownConfig) -> Self {
        Self::from_compatible_sdk_version(
            &config.product.compatible_sdk_version,
            config.product.compatible_sdk_version_stage.as_deref(),
            loader_json.api_version,
        )
    }

    pub fn from_compatible_sdk_version(
        compatible_sdk_version: &str,
        compatible_sdk_version_stage: Option<&str>,
        fallback_api_version: u32,
    ) -> Self {
        Self {
            api_version: parse_api_version(compatible_sdk_version).unwrap_or(fallback_api_version),
            api_sub_version: compatible_sdk_version_stage
                .filter(|stage| !stage.is_empty())
                .map(str::to_string),
        }
    }

    pub fn api_version(&self) -> u32 {
        self.api_version
    }

    pub fn api_sub_version(&self) -> Option<String> {
        self.api_sub_version.clone()
    }
}

/// Immutable inputs used to construct one Rolldown [`BundlePlan`].
pub struct BundlePlanBuilderInput<'a> {
    pub loader_json: &'a LoaderJson,
    pub loader_json_path: &'a Path,
    pub output_dir: PathBuf,
    pub config: &'a ArkdownConfig,
    /// Owning OHOS module name from `ModuleModel`, matching
    /// `AbstractCompileNode.moduleName`.
    pub module_name: &'a str,
    pub module_target_name: &'a str,
    pub custom_config: Option<&'a ArkdownCustomConfig>,
    pub arkts_language_version: ArktsLanguageVersion,
    pub should_emit_js: bool,
    pub package_info: BundlePlanPackageInfo,
}

/// Constructs the complete Rolldown bundle plan from one loader snapshot.
pub struct BundlePlanBuilder<'a> {
    input: BundlePlanBuilderInput<'a>,
}

impl<'a> BundlePlanBuilder<'a> {
    pub fn new(input: BundlePlanBuilderInput<'a>) -> Self {
        Self { input }
    }

    pub fn build(self) -> BundlePlan {
        let BundlePlanBuilderInput {
            loader_json,
            loader_json_path,
            output_dir,
            config,
            module_name,
            module_target_name,
            custom_config,
            arkts_language_version,
            should_emit_js,
            package_info,
        } = self.input;
        let BundlePlanPackageInfo {
            mut source_map,
            mut runtime,
            package_snapshots,
        } = package_info;
        Self::add_runtime_package_versions(&runtime, &mut source_map);
        Self::add_loader_ohm_package_versions(loader_json, &mut source_map);
        LoaderRuntimePackageContextCollector::new(loader_json, &package_snapshots)
            .extend(&mut runtime);
        let entries = Self::ordered_compile_entries(loader_json);
        let module_meta_info_files = BundleModuleMetaInfoFileCollector::new(
            loader_json,
            module_name,
            config.product.bundle_name.as_str(),
            source_map.entry_package_name.as_str(),
        )
        .collect(&entries);
        let compile_target = HvigorCompileTarget::from_loader_json(loader_json, config);
        let module_root = loader_json
            .module_path_map
            .get(&loader_json.module_name)
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(&loader_json.module_root_path)
                    .parent()
                    .and_then(Path::parent)
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| PathBuf::from(&loader_json.module_root_path))
            });
        let source_layout = ModuleSourceLayout::new(&module_root);
        let build_layout = ModuleBuildLayout::for_output_target_product(
            &config.output_target,
            &module_root,
            &config.product.name,
            module_target_name,
        );
        let cache_root = PathBuf::from(&loader_json.cache_path);
        let cache_layout = ModuleSourceCacheLayout::new(&cache_root);
        let user_config_path = source_layout.mock_config_json5();
        let mock_config = BundleMockConfig {
            process: matches!(
                config.output_target,
                arkdown_core::OutputTarget::Preview | arkdown_core::OutputTarget::Test
            ) || module_target_name == "ohosTest",
            user_config_path: user_config_path.is_file().then_some(user_config_path),
            ets_source_root: source_layout.ets(),
            transformed_output_path: build_layout.intermediates_mock_config_json(),
            transformed_cache_path: cache_layout.transformed_mock_config_json(),
            user_cache_path: cache_layout.user_mock_config_json5(),
            dependencies: Vec::new(),
        };
        BundlePlan {
            input: BundleInput {
                loader_json_path: loader_json_path.to_path_buf(),
                project_root: PathBuf::from(&loader_json.project_root_path),
                project_path: PathBuf::from(&loader_json.module_root_path).join("ets"),
                module_root: PathBuf::from(&loader_json.module_root_path),
                module_name: loader_json.module_name.clone(),
                module_path_map: loader_json
                    .module_path_map
                    .iter()
                    .map(|(name, path)| (name.clone(), PathBuf::from(path)))
                    .collect(),
                module_record_roots: Vec::new(),
                module_meta_info_files,
                entries,
                form_entry_sources: loader_json.form_entry_sources.clone(),
                insight_intent_form_bindings: loader_json.insight_intent_form_bindings.clone(),
                runtime_entry_sources: loader_json.runtime_entry_sources.clone(),
                page_entry_sources: loader_json.page_entry_sources.clone(),
                support_chunks: loader_json.support_chunks,
                source_map_packages: source_map,
                runtime_packages: runtime,
                byte_code_har_packages: loader_json.byte_code_har_info.keys().cloned().collect(),
                byte_code_har: loader_json.byte_code_har.unwrap_or(false),
                preload_system_so_entries: Vec::new(),
                mock_config: Some(mock_config),
            },
            output: BundleOutputOptions {
                output_dir,
                module_source_output_dir: Some(PathBuf::from(&loader_json.cache_path)),
                source_map_dir: None,
                entry_file_names: None,
                chunk_file_names: None,
                preserve_symlinks: false,
                shim_missing_exports: false,
            },
            source_map: SourceMapPolicy::External,
            cache: BundleCachePolicy::default(),
            loader_resource: LoaderResourceJoin {
                loader_json_path: loader_json_path.to_path_buf(),
                resources_index_path: None,
                resource_table_path: None,
            },
            plugins: BundlePluginSet::default(),
            build_mode: config.build_mode.clone(),
            arkts_language_version,
            target_api_version: compile_target.api_version(),
            compatible_sdk_version: config.product.compatible_sdk_version.clone(),
            runtime_os: config.product.runtime_os.clone(),
            bundle_name: config.product.bundle_name.clone(),
            use_normalized_ohm_url: loader_json.use_normalized_ohm_url,
            // Latest hvigor keeps transformed TypeScript for normal Stage/ESMODULE
            // builds. `rollup-plugin-ets-typescript.ts::getShouldEmitJs` selects
            // JavaScript for coverage builds (and still honors per-file suppression
            // directives when this project-level switch is false).
            should_emit_js,
            output_target: config.output_target.clone(),
            preview_mode: matches!(config.output_target, arkdown_core::OutputTarget::Preview),
            check_entry: false,
            shim_missing_exports_override: custom_config.and_then(|c| c.shim_missing_exports),
            treeshake_override: custom_config.and_then(|c| c.treeshake),
            context_override: custom_config.and_then(|c| c.context.clone()),
        }
    }
}

struct BundleModuleMetaInfoFileCollector<'a> {
    module_root: Option<&'a Path>,
    use_normalized_ohm_url: bool,
    module_name: &'a str,
    bundle_name: &'a str,
    package_name: &'a str,
}

impl<'a> BundleModuleMetaInfoFileCollector<'a> {
    fn new(
        loader_json: &'a LoaderJson,
        module_name: &'a str,
        bundle_name: &'a str,
        package_name: &'a str,
    ) -> Self {
        Self {
            module_root: loader_json
                .module_path_map
                .get(&loader_json.module_name)
                .map(Path::new),
            use_normalized_ohm_url: loader_json.use_normalized_ohm_url,
            module_name,
            bundle_name,
            package_name,
        }
    }

    fn collect(&self, entries: &[String]) -> Vec<BundleModuleMetaInfoFile> {
        entries
            .iter()
            .map(PathBuf::from)
            .filter(|path| self.is_generated_resource_table(path))
            .map(|file_path| BundleModuleMetaInfoFile {
                file_path,
                ohm_url: self.resource_table_ohm_url(),
                package_name: self.package_name.to_string(),
            })
            .collect()
    }

    fn is_generated_resource_table(&self, path: &Path) -> bool {
        if path.file_name().and_then(|name| name.to_str()) != Some("ResourceTable.ts") {
            return false;
        }
        let Some(module_root) = self.module_root else {
            return false;
        };
        // Local unit-test builds root at `<module>/.test`; the generated
        // ResourceTable still carries the fixed `build/generated/r/ResourceTable`
        // Ohm URL (hvigor `getOhmUrlByFilepath` + ModuleInfo attribution keep
        // the standard record name regardless of `buildRoot`).
        path.strip_prefix(module_root).is_ok_and(|relative| {
            (relative.starts_with("build") || relative.starts_with(".test"))
                && relative
                    .components()
                    .any(|part| part.as_os_str() == "generated")
        })
    }

    fn resource_table_ohm_url(&self) -> String {
        // `AbstractCompileNode.getModuleMetaInfoFiles` always uses the owning
        // `this.moduleName`. The loader's active name and `modulePathMap` can
        // contain target aliases or sibling modules; neither changes the
        // ResourceTable record owner.
        if self.use_normalized_ohm_url {
            return format!("&{}/build/generated/r/ResourceTable&", self.module_name);
        }
        format!(
            "{}/{}/build/generated/r/ResourceTable",
            self.bundle_name, self.module_name
        )
    }
}

pub struct BundleRuntimePackageInfoBuilder<'a> {
    contexts: &'a BTreeMap<String, PkgContextInfoOpt>,
}

impl<'a> BundleRuntimePackageInfoBuilder<'a> {
    pub fn new(contexts: &'a BTreeMap<String, PkgContextInfoOpt>) -> Self {
        Self { contexts }
    }

    pub fn build(&self) -> BundleRuntimePackageInfo {
        let mut packages = rustc_hash::FxHashMap::default();
        for (package_name, context) in self.contexts {
            let runtime_context = BundleRuntimePackageContext {
                package_name: context.package_name.clone(),
                bundle_name: context.bundle_name.clone(),
                module_name: context.module_name.clone(),
                version: context.version.clone(),
                entry_path: context.entry_path.clone(),
                is_so: context.is_so,
            };
            packages.insert(package_name.clone(), runtime_context.clone());
            if !context.dependency_alias.is_empty() {
                packages
                    .entry(context.dependency_alias.clone())
                    .or_insert(runtime_context);
            }
        }
        BundleRuntimePackageInfo { packages }
    }
}

struct LoaderRuntimePackageContextCollector<'a> {
    loader_json: &'a LoaderJson,
    package_snapshots: &'a rustc_hash::FxHashMap<PathBuf, Arc<PackageSnapshot>>,
}

impl<'a> LoaderRuntimePackageContextCollector<'a> {
    fn new(
        loader_json: &'a LoaderJson,
        package_snapshots: &'a rustc_hash::FxHashMap<PathBuf, Arc<PackageSnapshot>>,
    ) -> Self {
        Self {
            loader_json,
            package_snapshots,
        }
    }

    fn extend(&self, runtime_packages: &mut BundleRuntimePackageInfo) {
        for (package_name, ohm_url) in self.loader_ohm_maps() {
            let Some(context) = self.context_from_normalized_ohm(package_name, ohm_url) else {
                continue;
            };
            runtime_packages
                .packages
                .entry(package_name.clone())
                .or_insert(context);
        }
        for package_name in self.loader_so_package_names() {
            runtime_packages
                .packages
                .entry(package_name.to_string())
                .or_insert_with(|| BundleRuntimePackageContext {
                    package_name: package_name.to_string(),
                    bundle_name: String::new(),
                    module_name: String::new(),
                    version: String::new(),
                    entry_path: String::new(),
                    is_so: true,
                });
        }
        for (package_name, package_root) in &self.loader_json.module_path_map {
            if runtime_packages.packages.contains_key(package_name) {
                continue;
            }
            let Some(context) = self.context_from_loader_package_root(package_name, package_root)
            else {
                continue;
            };
            runtime_packages
                .packages
                .insert(package_name.clone(), context);
        }
    }

    fn loader_ohm_maps(&self) -> impl Iterator<Item = (&String, &String)> {
        self.loader_json
            .har_name_ohm_map
            .iter()
            .chain(self.loader_json.hsp_name_ohm_map.iter())
    }

    fn context_from_normalized_ohm(
        &self,
        package_name: &str,
        ohm_url: &str,
    ) -> Option<BundleRuntimePackageContext> {
        let normalized = NormalizedOhmUrl::parse_non_so(ohm_url)?;
        let normalized_import = normalized.normalized_import()?;
        if normalized.package_name()? != package_name {
            return None;
        }
        let entry_path = normalized_import
            .strip_prefix(package_name)?
            .strip_prefix('/')
            .unwrap_or("");
        Some(BundleRuntimePackageContext {
            package_name: package_name.to_string(),
            bundle_name: normalized.bundle_name()?.to_string(),
            module_name: normalized.module_name()?.to_string(),
            version: normalized.version()?.to_string(),
            entry_path: entry_path.to_string(),
            is_so: false,
        })
    }

    fn context_from_loader_package_root(
        &self,
        package_name: &str,
        package_root: &str,
    ) -> Option<BundleRuntimePackageContext> {
        if self.is_shared_library_name(package_name) {
            return None;
        }
        let package = self.package_snapshots.get(Path::new(package_root))?;
        if package.name != package_name {
            return None;
        }
        let entry_path = package
            .package_json
            .main
            .as_deref()
            .or(package.package_json.types.as_deref())
            .map(Self::trim_declaration_suffix)
            .unwrap_or_default();
        Some(BundleRuntimePackageContext {
            package_name: package.name.clone(),
            bundle_name: String::new(),
            module_name: String::new(),
            version: package.version.clone(),
            entry_path,
            is_so: false,
        })
    }

    fn loader_so_package_names(&self) -> impl Iterator<Item = &str> {
        self.loader_json
            .module_path_map
            .keys()
            .map(String::as_str)
            .filter(|name| self.is_shared_library_name(name))
    }

    fn is_shared_library_name(&self, name: &str) -> bool {
        let Some((_, suffix)) = name.rsplit_once(".so") else {
            return false;
        };
        suffix.is_empty()
            || suffix
                .strip_prefix('.')
                .is_some_and(|version| version.chars().all(|ch| ch == '.' || ch.is_ascii_digit()))
    }

    fn trim_declaration_suffix(path: &str) -> String {
        let path = path
            .strip_suffix(".d.ets")
            .or_else(|| path.strip_suffix(".d.ts"))
            .unwrap_or(path);
        ArtifactPathSerializer::rel_to_slash(Path::new(path))
    }
}

impl BundlePlanBuilder<'_> {
    fn add_loader_ohm_package_versions(
        loader_json: &LoaderJson,
        source_map_packages: &mut BundleSourceMapPackageInfo,
    ) {
        for (package_name, ohm_url) in loader_json
            .har_name_ohm_map
            .iter()
            .chain(loader_json.hsp_name_ohm_map.iter())
        {
            let Some(normalized) = NormalizedOhmUrl::parse_non_so(ohm_url) else {
                continue;
            };
            let Some(version) = normalized.version().filter(|version| !version.is_empty()) else {
                continue;
            };
            source_map_packages
                .dependency_package_versions
                .entry(package_name.clone())
                .or_insert_with(|| version.to_string());
        }
    }

    fn add_runtime_package_versions(
        runtime_packages: &BundleRuntimePackageInfo,
        source_map_packages: &mut BundleSourceMapPackageInfo,
    ) {
        for (package_key, context) in &runtime_packages.packages {
            if context.version.is_empty() {
                continue;
            }
            source_map_packages
                .dependency_package_versions
                .entry(context.package_name.clone())
                .or_insert_with(|| context.version.clone());
            source_map_packages
                .dependency_package_versions
                .entry(package_key.clone())
                .or_insert_with(|| context.version.clone());
        }
    }

    fn ordered_compile_entries(loader_json: &LoaderJson) -> Vec<String> {
        let mut entries = Vec::new();
        let mut seen = FxHashSet::default();
        for entry in loader_json
            .hvigor_compile_entry
            .iter()
            .chain(loader_json.compile_entry.iter())
        {
            if seen.insert(entry.clone()) {
                entries.push(entry.clone());
            }
        }
        entries
    }
}

// ---------------------------------------------------------------------------
// Bundle stage
// ---------------------------------------------------------------------------

/// Input for the bundle stage for one module.
pub struct BundleStageInput {
    /// Rolldown plugin set assembled by `PluginFactory`.
    pub plugins: Vec<SharedPluginable>,
    /// Shared ETS transform plan used by both the plugin hook and the hvigor
    /// ModuleSourceFile sidecar materializer.
    pub ets_transform_plan: Option<Arc<RolldownEtsTransformPlan>>,
    /// Concrete handle retained by a long-lived Rolldown graph session so the
    /// next immutable source snapshot can be atomically published.
    pub ets_transform_plugin: Option<Arc<EtsTransformPlugin>>,
    /// Bundle plan derived from `loader.json` by [`BundlePlanBuilder`].
    pub plan: BundlePlan,
    /// Absolute watcher paths for a hot rebuild. `None` selects a disposable
    /// full scan and clears any stale graph session for this module target.
    pub incremental_changes: Option<Vec<PathBuf>>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct BundleRuntimeKey {
    project_path: PathBuf,
    output_dir: PathBuf,
    module_name: String,
}

impl BundleRuntimeKey {
    fn from_plan(plan: &BundlePlan) -> Self {
        Self {
            project_path: plan.input.project_path.clone(),
            output_dir: plan.output.output_dir.clone(),
            module_name: plan.input.module_name.clone(),
        }
    }
}

/// Direct owner of long-lived Rolldown graph sessions used by JS-CLI watch
/// rebuilds. It deliberately contains no global state or mutex.
#[derive(Default)]
pub struct BundleRuntime {
    bundlers: rustc_hash::FxHashMap<BundleRuntimeKey, Bundler>,
}

impl BundleRuntime {
    async fn run(
        &mut self,
        input: BundleStageInput,
    ) -> Result<BundleStageOutput, BundleStageError> {
        let key = BundleRuntimeKey::from_plan(&input.plan);
        let mut bundler = if let Some(mut cached) = self.bundlers.remove(&key) {
            if let Some(plan) = input.ets_transform_plan.as_ref() {
                cached.update_ets_transform_plan(Arc::clone(plan));
            }
            cached
        } else {
            BundleStage::bundler(&input)
        };
        let changed_files = input.incremental_changes.as_deref().unwrap_or_default();
        let result = bundler
            .bundle_incremental(&input.plan, changed_files)
            .await
            .map_err(BundleStageError::Bundle);
        self.bundlers.insert(key, bundler);
        result.map(|bundle_output| BundleStageOutput {
            bundle_output,
            additional_abc_paths: Vec::new(),
            state: BundleStageState::Executed,
        })
    }

    pub async fn invalidate(&mut self, plan: &BundlePlan) -> Result<(), BundleStageError> {
        if let Some(mut bundler) = self.bundlers.remove(&BundleRuntimeKey::from_plan(plan)) {
            bundler.close().await.map_err(BundleStageError::Bundle)?;
        }
        Ok(())
    }

    pub async fn close(&mut self) -> Result<(), BundleStageError> {
        let mut bundlers = std::mem::take(&mut self.bundlers)
            .into_iter()
            .collect::<Vec<_>>();
        bundlers.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
        let mut first_error = None;
        for (_, mut bundler) in bundlers {
            if let Err(error) = bundler.close().await {
                first_error.get_or_insert(BundleStageError::Bundle(error));
            }
        }
        if let Some(error) = first_error {
            return Err(error);
        }
        Ok(())
    }
}

/// Output produced by the bundle stage for one module.
pub struct BundleStageOutput {
    /// Rolldown scan and CompileArkTS handoff output.
    pub bundle_output: BundleOutput,
    /// Additional merged ABCs produced for the same ETS asset directory.
    /// ArkTS 1.2 hybrid modules retain the dynamic `modules.abc` beside the
    /// static driver's primary `modules_static.abc`.
    pub additional_abc_paths: Vec<PathBuf>,
    pub state: BundleStageState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BundleStageState {
    Executed,
    CacheHit,
    /// CompileArkTS is disabled for the selected source set, or the selected
    /// original-HAR task graph does not contain a compile stage.
    Skipped,
}

pub struct BundlePackageOutput {
    pub modules_abc_path: PathBuf,
    pub additional_abc_paths: Vec<PathBuf>,
    pub source_maps_path: Option<PathBuf>,
    pub preload_system_so_tasks: Vec<PreloadSystemSoTask>,
}

impl BundleStageOutput {
    /// Construct a cache-hit output with a pre-existing `modules.abc` path.
    pub fn from_cache(modules_abc_path: PathBuf, output_dir: PathBuf) -> Self {
        use arkdown_core::ChunkGraph;
        Self {
            bundle_output: BundleOutput {
                chunk_graph: ChunkGraph::default(),
                output_dir,
                chunk_files: vec![],
                chunk_source_files: vec![],
                module_source_files: vec![],
                entry_module_ids: vec![],
                module_imports: rustc_hash::FxHashMap::default(),
                external_module_requests: vec![],
                har_importees: HvigorHarImportees::default(),
                preload_system_so_tasks: vec![],
                npm_entries: vec![],
                modules_abc_path,
                source_maps_path: None,
                source_map_input_fingerprints: vec![],
                insight_intent_sources: vec![],
                crossplatform_collections: rustc_hash::FxHashMap::default(),
            },
            additional_abc_paths: Vec::new(),
            state: BundleStageState::CacheHit,
        }
    }

    pub fn with_additional_abc_paths(mut self, paths: Vec<PathBuf>) -> Self {
        self.additional_abc_paths = paths;
        self
    }

    pub fn skipped(modules_abc_path: PathBuf, output_dir: PathBuf) -> Self {
        let mut output = Self::from_cache(modules_abc_path, output_dir);
        output.state = BundleStageState::Skipped;
        output
    }

    pub fn bypasses_compilation(&self) -> bool {
        self.state != BundleStageState::Executed
    }

    pub fn release_chunk_graph(&mut self) {
        self.bundle_output.chunk_graph = arkdown_core::ChunkGraph::default();
    }

    pub fn into_package_output(self) -> BundlePackageOutput {
        BundlePackageOutput {
            modules_abc_path: self.bundle_output.modules_abc_path,
            additional_abc_paths: self.additional_abc_paths,
            source_maps_path: self.bundle_output.source_maps_path,
            preload_system_so_tasks: self.bundle_output.preload_system_so_tasks,
        }
    }
}

/// Bundle stage runner.
pub struct BundleStage;

#[derive(Debug, Error)]
pub enum BundleStageError {
    #[error("bundle stage failed: {0}")]
    Bundle(#[source] ArkdownError),
}

impl BundleStage {
    fn bundler(input: &BundleStageInput) -> Bundler {
        let mut bundler = Bundler::new(input.plugins.clone());
        if let Some(ets_transform_plan) = input.ets_transform_plan.as_ref() {
            bundler = bundler.with_ets_transform_plan(Arc::clone(ets_transform_plan));
        }
        if let Some(ets_transform_plugin) = input.ets_transform_plugin.as_ref() {
            bundler = bundler.with_ets_transform_plugin(Arc::clone(ets_transform_plugin));
        }
        bundler
    }

    /// Run the bundle stage for one module.
    pub async fn run(
        input: BundleStageInput,
        runtime: Option<&mut BundleRuntime>,
    ) -> Result<BundleStageOutput, BundleStageError> {
        if input.incremental_changes.is_some() {
            if let Some(runtime) = runtime {
                return runtime.run(input).await;
            }
        }
        if let Some(runtime) = runtime {
            runtime.invalidate(&input.plan).await?;
        }
        let mut bundler = Self::bundler(&input);
        let bundle_output = bundler
            .bundle(&input.plan)
            .await
            .map_err(BundleStageError::Bundle)?;
        Ok(BundleStageOutput {
            bundle_output,
            additional_abc_paths: Vec::new(),
            state: BundleStageState::Executed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{BundleModuleMetaInfoFileCollector, HvigorCompileTarget};

    #[test]
    fn compile_target_uses_hvigor_compatible_sdk_version_over_installed_sdk_api() {
        let target =
            HvigorCompileTarget::from_compatible_sdk_version("5.0.0(12)", Some("beta3"), 24);

        assert_eq!(target.api_version(), 12);
        assert_eq!(target.api_sub_version().as_deref(), Some("beta3"));
    }

    #[test]
    fn resource_table_record_uses_owning_module_not_another_loader_module() {
        // `AbstractCompileNode.getModuleMetaInfoFiles` uses `this.moduleName`.
        // A multi-module `modulePathMap` must never change that owner.
        let collector = BundleModuleMetaInfoFileCollector {
            module_root: None,
            use_normalized_ohm_url: false,
            module_name: "entry",
            bundle_name: "com.example.app",
            package_name: "@example/entry",
        };

        assert_eq!(
            collector.resource_table_ohm_url(),
            "com.example.app/entry/build/generated/r/ResourceTable"
        );
    }
}
