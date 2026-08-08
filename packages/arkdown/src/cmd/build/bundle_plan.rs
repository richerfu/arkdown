//! Bundle plan construction and bundle stage execution.
//!
//! Converts a `LoaderJson` (written by `arkdown-profile`) into a typed
//! [`BundlePlan`] and runs the Rolldown bundle stage.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use rolldown_plugin::__inner::SharedPluginable;
use rustc_hash::FxHashSet;

use arkdown::plugins::ets_transform::{ModuleAnalyzerKind, RolldownEtsTransformPlan};
use arkdown::{
  BundleCachePolicy, BundleInput, BundleModuleMetaInfoFile, BundleOutput, BundleOutputOptions,
  BundlePlan, BundlePluginSet, BundleRuntimePackageContext, BundleRuntimePackageInfo,
  BundleSourceMapPackageInfo, Bundler, HvigorHarImportees, LoaderResourceJoin, PreloadSystemSoTask,
  SourceMapPolicy,
};
use arkdown_config::build_profile::parse_api_version;
use arkdown_core::ArkdownConfig;
use arkdown_path::{ArtifactPathSerializer, NormalizedOhmUrl};
use arkdown_profile::{LoaderJson, PkgContextInfoOpt};
use arkdown_session::PackageSnapshot;

use super::custom_config::ArkdownCustomConfig;

// ---------------------------------------------------------------------------
// BundlePlan construction
// ---------------------------------------------------------------------------

pub(super) struct BundlePlanPackageInfo {
  pub(super) source_map: BundleSourceMapPackageInfo,
  pub(super) runtime: BundleRuntimePackageInfo,
}

/// Effective Ark bytecode target selected by latest hvigor's ModuleMode path.
///
/// `module_mode.js::generateEs2AbcCmd` uses
/// `projectConfig.compatibleSdkVersion` (and its stage), rather than the API
/// level of the SDK installation that provides `es2abc`. Keeping this in one
/// value object prevents the ETS transform and SDK compiler from targeting
/// different language/runtime versions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct HvigorCompileTarget {
  api_version: u32,
  api_sub_version: Option<String>,
}

impl HvigorCompileTarget {
  pub(super) fn from_loader_json(loader_json: &LoaderJson, config: &ArkdownConfig) -> Self {
    Self::from_compatible_sdk_version(
      &config.product.compatible_sdk_version,
      config.product.compatible_sdk_version_stage.as_deref(),
      loader_json.api_version,
    )
  }

  pub(super) fn from_compatible_sdk_version(
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

  pub(super) fn api_version(&self) -> u32 {
    self.api_version
  }

  pub(super) fn api_sub_version(&self) -> Option<String> {
    self.api_sub_version.clone()
  }
}

/// Build a [`BundlePlan`] from a `loader.json` and the active config.
///
/// Called by [`BundleStage::run`] and directly from coordinator cache-miss
/// paths.
pub(super) fn make_bundle_plan(
  loader_json: &LoaderJson,
  loader_json_path: impl AsRef<Path>,
  output_dir: impl Into<PathBuf>,
  config: &ArkdownConfig,
  custom_cfg: Option<&ArkdownCustomConfig>,
  module_analyzer: ModuleAnalyzerKind,
  package_info: BundlePlanPackageInfo,
) -> BundlePlan {
  let BundlePlanPackageInfo {
    mut source_map,
    mut runtime,
  } = package_info;
  add_runtime_package_versions(&runtime, &mut source_map);
  add_loader_ohm_package_versions(loader_json, &mut source_map);
  LoaderRuntimePackageContextCollector::new(loader_json).extend(&mut runtime);
  let entries = ordered_compile_entries(loader_json);
  let module_meta_info_files =
    BundleModuleMetaInfoFileCollector::new(loader_json, config.product.bundle_name.as_str())
      .collect(&entries);
  let compile_target = HvigorCompileTarget::from_loader_json(loader_json, config);
  BundlePlan {
    input: BundleInput {
      loader_json_path: loader_json_path.as_ref().to_path_buf(),
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
      runtime_entry_sources: loader_json.runtime_entry_sources.clone(),
      support_chunks: loader_json.support_chunks,
      source_map_packages: source_map,
      runtime_packages: runtime,
      byte_code_har_packages: loader_json.byte_code_har_info.keys().cloned().collect(),
      preload_system_so_entries: Vec::new(),
    },
    output: BundleOutputOptions {
      output_dir: output_dir.into(),
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
      loader_json_path: loader_json_path.as_ref().to_path_buf(),
      resources_index_path: None,
      resource_table_path: None,
    },
    plugins: BundlePluginSet::default(),
    build_mode: config.build_mode.clone(),
    target_api_version: compile_target.api_version(),
    bundle_name: config.product.bundle_name.clone(),
    use_normalized_ohm_url: loader_json.use_normalized_ohm_url,
    should_emit_js: true,
    module_analyzer,
    output_target: config.output_target.clone(),
    shim_missing_exports_override: custom_cfg.and_then(|c| c.shim_missing_exports),
    treeshake_override: custom_cfg.and_then(|c| c.treeshake),
    context_override: custom_cfg.and_then(|c| c.context.clone()),
  }
}

struct BundleModuleMetaInfoFileCollector<'a> {
  loader_json: &'a LoaderJson,
  bundle_name: &'a str,
}

impl<'a> BundleModuleMetaInfoFileCollector<'a> {
  fn new(loader_json: &'a LoaderJson, bundle_name: &'a str) -> Self {
    Self {
      loader_json,
      bundle_name,
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
      })
      .collect()
  }

  fn is_generated_resource_table(&self, path: &Path) -> bool {
    if path.file_name().and_then(|name| name.to_str()) != Some("ResourceTable.ts") {
      return false;
    }
    let Some(module_root) = self
      .loader_json
      .module_path_map
      .get(&self.loader_json.module_name)
      .map(Path::new)
    else {
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
    if self.loader_json.use_normalized_ohm_url {
      return format!(
        "&{}/build/generated/r/ResourceTable&",
        self.loader_json.module_name
      );
    }
    format!(
      "{}/{}/build/generated/r/ResourceTable",
      self.bundle_name, self.loader_json.module_name
    )
  }
}

pub(super) struct BundleRuntimePackageInfoBuilder<'a> {
  contexts: &'a BTreeMap<String, PkgContextInfoOpt>,
}

impl<'a> BundleRuntimePackageInfoBuilder<'a> {
  pub(super) fn new(contexts: &'a BTreeMap<String, PkgContextInfoOpt>) -> Self {
    Self { contexts }
  }

  pub(super) fn build(&self) -> BundleRuntimePackageInfo {
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
}

impl<'a> LoaderRuntimePackageContextCollector<'a> {
  fn new(loader_json: &'a LoaderJson) -> Self {
    Self { loader_json }
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
      let Some(context) = self.context_from_loader_package_root(package_name, package_root) else {
        continue;
      };
      runtime_packages
        .packages
        .insert(package_name.clone(), context);
    }
  }

  fn loader_ohm_maps(&self) -> impl Iterator<Item = (&String, &String)> {
    self
      .loader_json
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
    let package = PackageSnapshot::from_root(Path::new(package_root))
      .ok()
      .flatten()?;
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
      package_name: package.name,
      bundle_name: String::new(),
      module_name: String::new(),
      version: package.version,
      entry_path,
      is_so: false,
    })
  }

  fn loader_so_package_names(&self) -> impl Iterator<Item = &str> {
    self
      .loader_json
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

// ---------------------------------------------------------------------------
// Bundle stage
// ---------------------------------------------------------------------------

/// Input for the bundle stage for one module.
pub(super) struct BundleStageInput {
  pub(super) config: Arc<ArkdownConfig>,
  /// Rolldown plugin set assembled by `PluginFactory`.
  pub(super) plugins: Vec<SharedPluginable>,
  /// Shared ETS transform plan used by both the plugin hook and the hvigor
  /// ModuleSourceFile sidecar materializer.
  pub(super) ets_transform_plan: Option<Arc<RolldownEtsTransformPlan>>,
  /// Bundle plan derived from `loader.json` by `make_bundle_plan`.
  pub(super) plan: BundlePlan,
}

/// Output produced by the bundle stage for one module.
pub(super) struct BundleStageOutput {
  /// Rolldown scan and CompileArkTS handoff output.
  pub(super) bundle_output: BundleOutput,
  pub(super) state: BundleStageState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BundleStageState {
  Executed,
  CacheHit,
  /// Hvigor's original source HAR task graph runs ArkTS linting but does not
  /// run HarCompileArkTS/CompileArkTS or es2abc.
  SkippedOriginalHar,
}

pub(super) struct BundlePackageOutput {
  pub(super) modules_abc_path: PathBuf,
  pub(super) source_maps_path: Option<PathBuf>,
  pub(super) preload_system_so_tasks: Vec<PreloadSystemSoTask>,
}

impl BundleStageOutput {
  /// Construct a cache-hit output with a pre-existing `modules.abc` path.
  pub(super) fn from_cache(modules_abc_path: PathBuf, output_dir: PathBuf) -> Self {
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
      },
      state: BundleStageState::CacheHit,
    }
  }

  pub(super) fn skipped_original_har(modules_abc_path: PathBuf, output_dir: PathBuf) -> Self {
    let mut output = Self::from_cache(modules_abc_path, output_dir);
    output.state = BundleStageState::SkippedOriginalHar;
    output
  }

  pub(super) fn bypasses_compilation(&self) -> bool {
    self.state != BundleStageState::Executed
  }

  pub(super) fn release_chunk_graph(&mut self) {
    self.bundle_output.chunk_graph = arkdown_core::ChunkGraph::default();
  }

  pub(super) fn into_package_output(self) -> BundlePackageOutput {
    BundlePackageOutput {
      modules_abc_path: self.bundle_output.modules_abc_path,
      source_maps_path: self.bundle_output.source_maps_path,
      preload_system_so_tasks: self.bundle_output.preload_system_so_tasks,
    }
  }
}

/// Bundle stage runner.
pub(super) struct BundleStage;

impl BundleStage {
  /// Run the bundle stage for one module.
  pub(super) async fn run(input: BundleStageInput) -> Result<BundleStageOutput> {
    let mut bundler = Bundler::new((*input.config).clone(), input.plugins);
    if let Some(ets_transform_plan) = input.ets_transform_plan {
      bundler = bundler.with_ets_transform_plan(ets_transform_plan);
    }
    let bundle_output = bundler
      .bundle(&input.plan)
      .await
      .map_err(|e| anyhow::anyhow!("bundle stage failed: {}", e))?;
    Ok(BundleStageOutput {
      bundle_output,
      state: BundleStageState::Executed,
    })
  }
}
