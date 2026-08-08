//! Build coordinator.
//!
//! `BuildCoordinator` owns the explicit coarse sequencing of the build
//! pipeline:
//!
//! ```text
//! for each module wave:
//!   1. resource_stage   — compile resources via arkdown-resource
//!   2. profile_stage    — process module.json5 / loader.json via arkdown-profile
//!   3. bundle_stage     — bundle with Rolldown via arkdown
//!   4. bytecode_stage   — compile ABC via SDK es2abc
//!   5. package_stage    — assemble HAP/HSP/HAR via arkdown-pack
//! then:
//!   6. sign_stage       — sign archives via arkdown-sign
//! optionally:
//!   7. watch_stage      — watch for changes and drive incremental rebuilds
//! ```
//!
//! The coordinator does NOT own the logic inside each stage; it only sequences
//! the typed inputs and outputs.  All business logic lives in the domain crates
//! and the thin stage wrappers.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use indexmap::IndexMap;
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};

use arkdown_core::{
  ArkdownConfig, BuildMode, HapManifest, HarArtifactType, ModuleConfig, ObfuscationConfig,
  OhosModuleType, OutputTarget,
};
use arkdown_path::{
  canonicalize_or_owned, lexical_normalize, ArtifactPathSerializer, HvigorSourceFileFilter,
  NormalizedOhmUrl,
};
use arkdown_session::dependency_graph::snapshot::{
  DependencyEdgeKind, DependencyNodeSnapshot, DependencyNodeType,
};
use arkdown_session::module::snapshot::OhosModuleType as SessionOhosModuleType;
use arkdown_session::{BuildSession, ModuleCompileContext, PackageSnapshot, SdkToolKind};
use arkdown_sign::SignBackendRunner;

use super::args::{AbcBackendChoice, BuildArgs, NativeTaskRoot};
use super::packing_check::PackingCheck;
use super::pre_build::PreBuildValidator;
use super::bundle_plan::{
  make_bundle_plan, BundlePackageOutput, BundlePlanPackageInfo, BundleRuntimePackageInfoBuilder,
  BundleStage, BundleStageInput, BundleStageOutput, HvigorCompileTarget,
};
use super::bytecode::{BytecodeAbcInput, RolldownBytecodeBackendChoice, RolldownBytecodeCompiler};
use super::bytecode_har_declaration::ByteCodeHarDeclarationCollector;
use super::bytecode_stage::{BytecodeStage, BytecodeStageInput};
use super::code_resource::CodeResourceCopyPlan;
use super::coverage_instrument::CoverageInstrumenter;
use super::custom_config::ArkdownCustomConfigLoader;
use super::generate_har_declarations::HarDeclarationGenerator;
use super::har_dedup::HarDedupProjectPlan;
use super::har_importee_cache::HarImporteeCache;
use super::intermediates::{IntermediateArtifactWriter, SourceMapPackageHandoff};
use super::layout::{
  ModuleBuildLayout, ModuleSourceLayout, ProjectBuildLayout, ResourceOutputLayout,
};
use super::module_plan::ModuleSelectionPlanner;
use super::preload_system_so::{PreloadSystemSoArtifactWriter, PreloadSystemSoPlan};
use super::syscap::SyscapTransform;
use super::watch_stage::{WatchStage, WatchStageInput};
use arkdown::plugins::factory::{
  HarDedupFactoryMode, PackagePlugin, PluginFactory, PluginFactoryInput,
};
use arkdown::{
  BundleCacheBridge, BundleSourceMapPackageInfo, PluginCacheModuleInventory,
  ShouldInvalidCacheRequest, SourceMapPolicy,
};

use crate::cmd::ets_checker::{
  SdkEtsCheckerBatch, SdkEtsCheckerEntryCollector, SdkEtsCheckerInput, SdkEtsCheckerPaths,
  SdkEtsCheckerPlan, SdkEtsCheckerPlanBuilder,
};
use crate::cmd::native_build::{
  DebugSymbolCollector, HvigorModuleCleanStage, NativeLibCollectionPolicy, NativeLibCollector,
  NativeLibPackageInfo, NativeSdkToolchain,
};
use crate::progress::Progress;
use crate::task_cache::{TaskInputSnapshot, TaskInputs};

use arkdown::plugins::ets_transform::{
  CheckerMode, ComponentMetadata, ModuleAnalyzerKind, NavigationRouteMap, ResourceReference,
  SystemResourceReferenceLoader, TransformResolvedStaticModule, TransformSourceSnapshot,
};
use arkdown::plugins::ohos_resolve::{ProcessImporteeConfig, ProcessImporteeRunner};
use arkdown_cache::{BuildCache, BundleCacheKey, CacheGroupKey, CacheGroupSnapshot, CacheKey};
use arkdown_config::build_profile::{
  parse_api_version, BuildOptionProfileInventory, EffectiveModuleBuildOptionResolver,
  EffectiveTargetConfig,
};
use arkdown_ets_transform::{
  ArkuiComponentNameCollector, RolldownEtsTransformPlan, SourceClassifier,
};
use arkdown_obfuscate::{
  ArkguardAdapterOptions, BytecodeObfuscationInput, BytecodeObfuscationModuleRecord,
  ConsumerObfuscationFileWriter, ObfuscationMode, ObfuscationModuleFile, ObfuscationModuleInput,
  ObfuscationOptions, ObfuscationOutputPaths, ObfuscationPlan, Obfuscator,
};
use arkdown_pack::{
  minify_package_intermediate_module_json, BundledHarResourceInput, HarBundledDependencyInput,
  HarPackInput, HarPackageParameterInput, HarVariantInput, HspInterfaceHarInput, HspTgzInput,
  IntegratedHspCandidate, IntegratedHspCollector, PackAbility, PackInput, Packager,
};
use arkdown_profile::{
  BuildProfileInput, BuildProfileKind, BuildProfileWriter, ByteCodeHarDependencyRoot,
  ByteCodeHarMetadataWriter, ByteCodeHarScanEntry, CustomTypeResolver, FormProfileProcessor,
  LoaderByteCodeHarFields, LoaderJson, ModuleFormProfileInput, OhExportsProcessor,
  PkgContextCurrentInput, PkgContextDependencyInput, PkgContextDependencyKind,
  PkgContextInfoGenerator, PkgContextInfoInput, PkgContextInfoOpt, PkgContextModuleKind,
  ProfileProcessor,
};
use arkdown_resolver::{OhPackage, PkgBriefInfo};
use arkdown_resource::{ResourceBackendRunner, ResourceCompileConfig, ResourceCompileInput};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Typed output of a complete build run.
pub struct BuildOutput {
  /// Absolute paths to all produced artifacts (signed HAP/HSP/HAR files).
  pub artifact_paths: Vec<PathBuf>,
}

/// Selected ArkTS diagnostic policy.
///
/// `sdk` is the parity default: it runs the installed ets-loader semantic
/// checker before Rolldown while retaining Rust/OXC structural diagnostics in
/// the transform adapter. `oxc` is an explicit fast structural-only mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArkTsCheckerSelection {
  Sdk,
  Oxc,
  Off,
}

impl ArkTsCheckerSelection {
  fn parse(value: Option<&str>) -> Result<Self> {
    match value.unwrap_or("sdk") {
      "sdk" => Ok(Self::Sdk),
      "oxc" => Ok(Self::Oxc),
      "off" => Ok(Self::Off),
      checker => anyhow::bail!("unsupported checker '{checker}'; expected 'sdk', 'oxc', or 'off'"),
    }
  }

  fn oxc_mode(self) -> CheckerMode {
    match self {
      Self::Sdk | Self::Oxc => CheckerMode::Oxc,
      Self::Off => CheckerMode::Off,
    }
  }

  fn runs_sdk_semantic_checker(self) -> bool {
    matches!(self, Self::Sdk)
  }
}

/// Runs the installed SDK checker from fully prepared Rust plans before any
/// Rolldown transform hook executes. Keeping this outside a Rolldown hook
/// mirrors `etsChecker.buildStart` without introducing SDK I/O into the hot
/// transform path.
struct SdkSemanticCheckStage {
  plans: Vec<SdkEtsCheckerPlan>,
}

impl SdkSemanticCheckStage {
  fn new(plans: Vec<SdkEtsCheckerPlan>) -> Self {
    Self { plans }
  }

  fn run(self) -> Result<()> {
    let outputs = SdkEtsCheckerBatch::new(self.plans).run()?;
    let mut errors = 0usize;
    for output in outputs {
      errors += output.error_count();
      for diagnostic in output.diagnostics {
        match diagnostic.level.to_ascii_lowercase().as_str() {
          "error" => tracing::error!(
            module = %output.module_name,
            "ArkTS semantic check: {}",
            diagnostic.message
          ),
          "warn" | "warning" => tracing::warn!(
            module = %output.module_name,
            "ArkTS semantic check: {}",
            diagnostic.message
          ),
          _ => tracing::debug!(
            module = %output.module_name,
            level = %diagnostic.level,
            "ArkTS semantic check: {}",
            diagnostic.message
          ),
        }
      }
    }
    if errors > 0 {
      anyhow::bail!("ARKDOWN_ETS_SEMANTIC_CHECK_FAILED: SDK etsChecker reported {errors} error(s)");
    }
    Ok(())
  }
}

/// One observed hvigor task boundary in Arkdown's native build pipeline.
///
/// The Node runtime uses these boundaries to execute TypeScript task plugins
/// on the same side of the owning native action as hvigor 6.24.3. Paths stay
/// out of the transport: module and target names are resolved by the runtime's
/// already-initialized node/task containers.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildTaskEvent {
  pub phase: BuildTaskEventPhase,
  pub module_name: String,
  pub target_name: Option<String>,
  pub task_name: String,
}

impl BuildTaskEvent {
  fn new(
    phase: BuildTaskEventPhase,
    module_name: &str,
    target_name: Option<&str>,
    task_name: &str,
  ) -> Self {
    Self {
      phase,
      module_name: module_name.to_string(),
      target_name: target_name.map(str::to_string),
      task_name: task_name.to_string(),
    }
  }
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BuildTaskEventPhase {
  Before,
  After,
  UpToDate,
}

/// Marks one complete native pipeline execution inside a normal, multi-target,
/// or watch build. The Node runtime uses these markers to reset custom-task
/// execution state and persist successful incremental snapshots per cycle.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildCycleEvent {
  phase: BuildCycleEventPhase,
}

impl BuildCycleEvent {
  fn new(phase: BuildCycleEventPhase) -> Self {
    Self { phase }
  }
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
enum BuildCycleEventPhase {
  BuildCycleStart,
  BuildCycleEnd,
}

/// Latest-hvigor `OhosModuleContext.transformAbc` callback input.
///
/// `ArkCompile.doBeforeObfuscationAction` invokes this after es2abc produces
/// the target's main `modules.abc` and before ArkGuard bytecode obfuscation.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildAbcTransformEvent {
  phase: BuildAbcTransformEventPhase,
  pub module_name: String,
  pub target_name: String,
  pub abc_path: String,
  pub config: BuildAbcTransformConfig,
}

impl BuildAbcTransformEvent {
  fn new(
    module_name: &str,
    target_name: &str,
    abc_path: &Path,
    is_ark_guard_enabled: bool,
  ) -> Result<Self> {
    Ok(Self {
      phase: BuildAbcTransformEventPhase::TransformAbc,
      module_name: module_name.to_string(),
      target_name: target_name.to_string(),
      abc_path: abc_path
        .to_str()
        .ok_or_else(|| {
          anyhow::anyhow!(
            "transformAbc requires a UTF-8 ABC path: {}",
            abc_path.display()
          )
        })?
        .to_string(),
      config: BuildAbcTransformConfig {
        is_ark_guard_enabled,
      },
    })
  }
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
enum BuildAbcTransformEventPhase {
  TransformAbc,
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildAbcTransformConfig {
  pub is_ark_guard_enabled: bool,
}

/// Same-process bridge for TypeScript task lifecycle behavior.
///
/// `before_task` returns whether the native action remains enabled. Latest
/// hvigor checks `CoreTaskImpl.getEnabled()` before beforeRun/action/afterRun,
/// so the coordinator must make the decision before touching stage outputs.
#[async_trait::async_trait]
pub trait BuildTaskObserver: Send + Sync {
  async fn build_cycle_started(&self, event: BuildCycleEvent) -> Result<()>;
  async fn build_cycle_finished(&self, event: BuildCycleEvent) -> Result<()>;
  async fn before_task(&self, event: BuildTaskEvent) -> Result<bool>;
  async fn after_task(&self, event: BuildTaskEvent) -> Result<()>;
  async fn task_up_to_date(&self, event: BuildTaskEvent) -> Result<()>;
  async fn transform_abc(&self, event: BuildAbcTransformEvent) -> Result<()>;
}

pub(super) struct BuildTaskLifecycle {
  observer: Option<Arc<dyn BuildTaskObserver>>,
  remaining_roots: Vec<NativeTaskRoot>,
  cutoff_enabled: bool,
}

impl BuildTaskLifecycle {
  pub(super) fn new(
    observer: Option<Arc<dyn BuildTaskObserver>>,
    remaining_roots: Vec<NativeTaskRoot>,
  ) -> Self {
    let cutoff_enabled = !remaining_roots.is_empty();
    Self {
      observer,
      remaining_roots,
      cutoff_enabled,
    }
  }

  pub(super) async fn begin_cycle(&self) -> Result<()> {
    let Some(observer) = self.observer.as_ref() else {
      return Ok(());
    };
    observer
      .build_cycle_started(BuildCycleEvent::new(BuildCycleEventPhase::BuildCycleStart))
      .await
  }

  pub(super) async fn finish_cycle(&self) -> Result<()> {
    let Some(observer) = self.observer.as_ref() else {
      return Ok(());
    };
    observer
      .build_cycle_finished(BuildCycleEvent::new(BuildCycleEventPhase::BuildCycleEnd))
      .await
  }

  pub(super) async fn before(
    &mut self,
    module_name: &str,
    target_name: Option<&str>,
    task_name: &str,
  ) -> Result<bool> {
    let enabled = if let Some(observer) = self.observer.as_ref() {
      observer
        .before_task(BuildTaskEvent::new(
          BuildTaskEventPhase::Before,
          module_name,
          target_name,
          task_name,
        ))
        .await?
    } else {
      true
    };
    if !enabled {
      self.complete_root(module_name, target_name, task_name)?;
    }
    Ok(enabled)
  }

  pub(super) async fn after(
    &mut self,
    module_name: &str,
    target_name: Option<&str>,
    task_name: &str,
  ) -> Result<()> {
    if let Some(observer) = self.observer.as_ref() {
      observer
        .after_task(BuildTaskEvent::new(
          BuildTaskEventPhase::After,
          module_name,
          target_name,
          task_name,
        ))
        .await?;
    }
    self.complete_root(module_name, target_name, task_name)
  }

  async fn up_to_date(
    &mut self,
    module_name: &str,
    target_name: Option<&str>,
    task_name: &str,
  ) -> Result<()> {
    if let Some(observer) = self.observer.as_ref() {
      observer
        .task_up_to_date(BuildTaskEvent::new(
          BuildTaskEventPhase::UpToDate,
          module_name,
          target_name,
          task_name,
        ))
        .await?;
    }
    self.complete_root(module_name, target_name, task_name)
  }

  pub(super) async fn marker(
    &mut self,
    module_name: &str,
    target_name: Option<&str>,
    task_name: &str,
  ) -> Result<()> {
    if self.before(module_name, target_name, task_name).await? {
      self.after(module_name, target_name, task_name).await?;
    }
    Ok(())
  }

  fn has_root(&self, module_name: &str, target_name: Option<&str>, task_name: &str) -> bool {
    self.remaining_roots.iter().any(|root| {
      root.module_name == module_name
        && root.target_name.as_deref() == target_name
        && root.task_name == task_name
    })
  }

  fn is_observed(&self) -> bool {
    self.observer.is_some()
  }

  fn complete_root(
    &mut self,
    module_name: &str,
    target_name: Option<&str>,
    task_name: &str,
  ) -> Result<()> {
    if !self.cutoff_enabled {
      return Ok(());
    }
    self.remaining_roots.retain(|root| {
      root.module_name != module_name
        || root.target_name.as_deref() != target_name
        || root.task_name != task_name
    });
    if self.remaining_roots.is_empty() {
      return Err(NativeTaskCutoffReached.into());
    }
    Ok(())
  }

  async fn transform_abc(
    &self,
    module_name: &str,
    target_name: &str,
    abc_path: &Path,
    is_ark_guard_enabled: bool,
  ) -> Result<()> {
    let Some(observer) = self.observer.as_ref() else {
      return Ok(());
    };
    observer
      .transform_abc(BuildAbcTransformEvent::new(
        module_name,
        target_name,
        abc_path,
        is_ark_guard_enabled,
      )?)
      .await
  }
}

#[derive(Debug, thiserror::Error)]
#[error("selected native hvigor task roots completed")]
pub(super) struct NativeTaskCutoffReached;

struct WidgetCompileArktsPlan {
  entries: Vec<String>,
  cache_dir: PathBuf,
  output_dir: PathBuf,
  modules_abc_path: PathBuf,
  cache_file: PathBuf,
}

struct WidgetCompileArktsOutput {
  bundle_out: BundleStageOutput,
  plan: WidgetCompileArktsPlan,
}

struct WidgetCompileArktsPass<'a> {
  config: &'a ArkdownConfig,
  module: &'a ModuleConfig,
  manifest: &'a HapManifest,
  loader_json: &'a LoaderJson,
}

impl<'a> WidgetCompileArktsPass<'a> {
  fn new(
    config: &'a ArkdownConfig,
    module: &'a ModuleConfig,
    manifest: &'a HapManifest,
    loader_json: &'a LoaderJson,
  ) -> Self {
    Self {
      config,
      module,
      manifest,
      loader_json,
    }
  }

  fn plan(&self) -> Result<Option<WidgetCompileArktsPlan>> {
    // hvigor's widget compile pass (`needSubmitArkTsWidget`) runs on the
    // form widget module itself — an HSP whose `module.json5` declares
    // `formExtensionModule` (`isFormWidgetModule`), in addition to the HAP
    // form extension module's own form-ability sources.
    let module_layout = ModuleBuildLayout::new(&self.module.src_path);
    let form_sources = FormProfileProcessor::new()
      .collect_module_form_sources(ModuleFormProfileInput {
        module_name: &self.module.name,
        module_root: &self.module.src_path,
        module_type: &self.module.module_type,
        module_json_path: &module_layout.source().module_json5(),
        extension_abilities: &self.manifest.extension_abilities,
        project_modules: &self.config.modules,
        compatible_api_version: Some(self.loader_json.api_version),
      })
      .map_err(|err| anyhow::anyhow!("collect widget form sources failed: {}", err))?;
    let mut entries = form_sources
      .into_iter()
      .filter(|source| source.is_arkts())
      .map(|source| {
        let path = canonicalize_or_owned(&source.source_path);
        ArtifactPathSerializer::path_to_slash_string(&path)
          .with_context(|| format!("serialize widget compile entry {}", path.display()))
      })
      .collect::<Result<Vec<_>>>()?;
    entries.sort();
    entries.dedup();
    if entries.is_empty() {
      return Ok(None);
    }

    let cache_dir = self.widget_cache_dir();
    let output_dir = PathBuf::from(&self.loader_json.ace_module_build);
    Ok(Some(WidgetCompileArktsPlan {
      entries,
      cache_file: cache_dir.join("modules.cache"),
      cache_dir,
      modules_abc_path: output_dir.join("widgets.abc"),
      output_dir,
    }))
  }

  fn bundle_plan(
    &self,
    base_plan: &arkdown::BundlePlan,
    plan: &WidgetCompileArktsPlan,
  ) -> arkdown::BundlePlan {
    let mut bundle_plan = base_plan.clone();
    bundle_plan.input.entries = plan.entries.clone();
    bundle_plan.input.form_entry_sources = plan.entries.clone();
    bundle_plan.input.runtime_entry_sources.clear();
    bundle_plan.input.preload_system_so_entries.clear();
    bundle_plan.output.module_source_output_dir = Some(plan.cache_dir.clone());
    bundle_plan.output.output_dir = plan.output_dir.clone();
    bundle_plan.source_map = SourceMapPolicy::None;
    bundle_plan
  }

  fn compile_context_loader_json(&self, plan: &WidgetCompileArktsPlan) -> Result<LoaderJson> {
    let mut loader_json = self.loader_json.clone();
    // 6.24.3: the widget loader JSON is the same loader object written to a
    // second path (`needSubmitArkTsWidget && outputJSONSync(aceWidgetLoaderJson,
    // c)`); its `compileEntry` matches the main loader (ResourceTable.ts etc.)
    // and never carries the widget form sources — those go to `widgets.abc`.
    loader_json.form_entry_sources = plan.entries.clone();
    loader_json.runtime_entry_sources.clear();
    loader_json.cache_path = ArtifactPathSerializer::path_to_slash_string(&plan.cache_dir)
      .with_context(|| {
        format!(
          "serialize widget compile cache directory {}",
          plan.cache_dir.display()
        )
      })?;
    // 6.24.3 `GenerateLoaderJson`: the widget loader JSON always carries
    // `pkgName2OtherCompileFiles`; the main loader.json never does. The
    // per-ByteCode-HAR grouping is populated when
    // `arkOptions.widget.transitiveDeps` is enabled
    // (`isWidgetTransitiveDeps`), otherwise it stays an empty object.
    let transitive_deps = self
      .config
      .product
      .build_option
      .ark_options
      .as_ref()
      .and_then(|options| options.widget.as_ref())
      .is_some_and(|widget| widget.transitive_deps);
    loader_json.pkg_name2_other_compile_files = Some(
      if transitive_deps {
        LoaderByteCodeHarFields::collect_pkg_name2_other_compile_files(
          &self.module.src_path,
          &self.config.project_root,
        )
        .into_iter()
        .map(|(package_name, files)| {
          (
            package_name,
            files
              .into_iter()
              .map(|path| {
                ArtifactPathSerializer::path_to_slash_string(&path).unwrap_or_default()
              })
              .collect(),
          )
        })
        .collect()
      } else {
        IndexMap::new()
      },
    );
    Ok(loader_json)
  }

  fn write_component_collection(
    &self,
    output: &WidgetCompileArktsOutput,
    ets_transform_plan: &RolldownEtsTransformPlan,
  ) -> Result<()> {
    let mut collection = BTreeMap::<String, Vec<String>>::new();
    for source in &output.bundle_out.bundle_output.module_source_files {
      let module_id = Path::new(&source.module_id);
      if module_id.extension().and_then(|ext| ext.to_str()) != Some("ets") {
        continue;
      }
      if !module_id.is_file() {
        continue;
      }
      let content = fs_err::read_to_string(module_id)?;
      let options = ets_transform_plan.transform_options_for(&source.module_id);
      let names = ArkuiComponentNameCollector::with_options(&content, &source.module_id, &options)
        .collect()
        .map_err(|err| anyhow::anyhow!("collect widget component names failed: {}", err))?;
      if names.is_empty() {
        continue;
      }
      collection.insert(self.collection_key(module_id), names);
    }

    let cache_path = output.plan.cache_dir.join("component_collection.json");
    let loader_path = output
      .plan
      .output_dir
      .join("widget")
      .join("component_collection.json");
    self.write_component_collection_file(&cache_path, &collection)?;
    self.write_component_collection_file(&loader_path, &collection)?;
    Ok(())
  }

  fn widget_cache_dir(&self) -> PathBuf {
    let cache_dir = PathBuf::from(&self.loader_json.cache_path);
    let widget_name = format!("{}_widget", self.loader_json.build_mode);
    cache_dir
      .parent()
      .map(|parent| parent.join(&widget_name))
      .unwrap_or_else(|| cache_dir.with_file_name(widget_name))
  }

  fn collection_key(&self, module_id: &Path) -> String {
    let project_root = canonicalize_or_owned(&self.config.project_root);
    let source_path = canonicalize_or_owned(module_id);
    let rel = source_path
      .strip_prefix(&project_root)
      .unwrap_or(&source_path);
    ArtifactPathSerializer::rel_to_slash(rel)
  }

  fn write_component_collection_file(
    &self,
    path: &Path,
    collection: &BTreeMap<String, Vec<String>>,
  ) -> Result<()> {
    if let Some(parent) = path.parent() {
      fs_err::create_dir_all(parent)?;
    }
    fs_err::write(path, serde_json::to_vec_pretty(collection)?)?;
    Ok(())
  }
}

/// `FormUtil.getTargetModuleOptObj`: the form widget module (HSP declaring
/// `formExtensionModule`) merges the form extension module's `type: "form"`
/// abilities — reduced to `name`/`type`/`metadata` — into its ark module.
fn merge_form_widget_abilities(
  config: &arkdown_core::ArkdownConfig,
  module_json5: &arkdown_core::ModuleJson5,
) -> anyhow::Result<Vec<serde_json::Value>> {
  let Some(extension_module_name) = module_json5.module.form_extension_module.as_deref() else {
    return Ok(Vec::new());
  };
  let Some(extension_module) = config
    .modules
    .iter()
    .find(|module| module.name == extension_module_name)
  else {
    return Ok(Vec::new());
  };
  let path = extension_module.src_path.join("src/main/module.json5");
  let content = std::fs::read_to_string(&path)
    .map_err(|error| anyhow::anyhow!("read {}: {error}", path.display()))?;
  let extension_json: arkdown_core::ModuleJson5 =
    json5::from_str(&content).map_err(|error| anyhow::anyhow!("parse {}: {error}", path.display()))?;
  Ok(extension_json
    .module
    .extension_abilities
    .iter()
    .filter(|ability| ability.extension_type == "form")
    .map(|ability| {
      serde_json::json!({
        "name": ability.name,
        "type": "form",
        "metadata": ability.metadata.iter().map(|entry| {
          let mut value = serde_json::Map::new();
          value.insert("name".into(), entry.name.clone().into());
          if let Some(resource) = entry.resource.as_ref() {
            value.insert("resource".into(), resource.clone().into());
          }
          serde_json::Value::Object(value)
        }).collect::<Vec<_>>(),
      })
    })
    .collect())
}

struct TransformSourceSnapshotBuilder<'a> {
  module_roots: &'a [(String, PathBuf)],
  dependency_source_roots: &'a [PathBuf],
  native_declaration_roots: &'a [PathBuf],
  sdk_declaration_roots: &'a [PathBuf],
  custom_type_declaration_inputs: &'a [PathBuf],
  source_set_overlays: &'a [(PathBuf, PathBuf)],
  package_roots: &'a [(String, PathBuf)],
  /// Local unit-test source roots (`<module>/.test/testability`,
  /// `<module>/src/test`) — compiler inputs of the `hvigorw test` chain.
  test_source_roots: &'a [PathBuf],
}

impl<'a> TransformSourceSnapshotBuilder<'a> {
  fn new(
    module_roots: &'a [(String, PathBuf)],
    dependency_source_roots: &'a [PathBuf],
    native_declaration_roots: &'a [PathBuf],
    source_set_overlays: &'a [(PathBuf, PathBuf)],
  ) -> Self {
    Self {
      module_roots,
      dependency_source_roots,
      native_declaration_roots,
      sdk_declaration_roots: &[],
      custom_type_declaration_inputs: &[],
      source_set_overlays,
      package_roots: &[],
      test_source_roots: &[],
    }
  }

  fn with_test_source_roots(mut self, roots: &'a [PathBuf]) -> Self {
    self.test_source_roots = roots;
    self
  }

  fn with_package_roots(mut self, roots: &'a [(String, PathBuf)]) -> Self {
    self.package_roots = roots;
    self
  }

  fn with_custom_type_declaration_inputs(mut self, inputs: &'a [PathBuf]) -> Self {
    self.custom_type_declaration_inputs = inputs;
    self
  }

  fn with_sdk_declaration_roots(mut self, roots: &'a [PathBuf]) -> Self {
    self.sdk_declaration_roots = roots;
    self
  }

  fn build(&self) -> Result<TransformSourceSnapshot> {
    let mut files = FxHashMap::default();
    let mut runtime_source_paths = FxHashSet::default();
    let mut collected_roots = FxHashSet::default();
    for (_, module_root) in self.module_roots {
      self.collect_source_root(
        &ModuleSourceLayout::new(module_root).ets(),
        &mut files,
        &mut runtime_source_paths,
        &mut collected_roots,
      )?;
    }
    for root in self.dependency_source_roots {
      self.collect_source_root(
        root,
        &mut files,
        &mut runtime_source_paths,
        &mut collected_roots,
      )?;
    }
    for root in self.test_source_roots {
      self.collect_source_root(
        root,
        &mut files,
        &mut runtime_source_paths,
        &mut collected_roots,
      )?;
    }
    for root in self.native_declaration_roots {
      self.collect_declaration_root(
        root,
        &mut files,
        &mut runtime_source_paths,
        &mut collected_roots,
      )?;
    }
    for root in self.sdk_declaration_roots {
      self.collect_declaration_root(
        root,
        &mut files,
        &mut runtime_source_paths,
        &mut collected_roots,
      )?;
    }
    for input in self.custom_type_declaration_inputs {
      self.collect_declaration_input(
        input,
        &mut files,
        &mut runtime_source_paths,
        &mut collected_roots,
      )?;
    }
    for (base_root, target_root) in self.source_set_overlays {
      self.collect_source_root(
        base_root,
        &mut files,
        &mut runtime_source_paths,
        &mut collected_roots,
      )?;
      self.collect_source_root(
        target_root,
        &mut files,
        &mut runtime_source_paths,
        &mut collected_roots,
      )?;
    }
    let resolved_static_modules = self.collect_resolved_static_modules();
    Ok(
      TransformSourceSnapshot::with_resolved_static_modules_and_runtime_sources(
        files,
        resolved_static_modules,
        runtime_source_paths,
      )
      .with_sdk_declaration_roots(self.sdk_declaration_roots.to_vec())
      .with_package_roots(
        self
          .package_roots
          .iter()
          .map(|(name, root)| (name.clone(), root.clone()))
          .collect(),
      ),
    )
  }

  fn collect_source_root(
    &self,
    root: &Path,
    files: &mut FxHashMap<PathBuf, Arc<str>>,
    runtime_source_paths: &mut FxHashSet<PathBuf>,
    collected_roots: &mut FxHashSet<PathBuf>,
  ) -> Result<()> {
    self.collect_root(
      root,
      files,
      runtime_source_paths,
      collected_roots,
      SourceRootMode::TransformAndDeclarations,
    )
  }

  fn collect_declaration_root(
    &self,
    root: &Path,
    files: &mut FxHashMap<PathBuf, Arc<str>>,
    runtime_source_paths: &mut FxHashSet<PathBuf>,
    collected_roots: &mut FxHashSet<PathBuf>,
  ) -> Result<()> {
    self.collect_root(
      root,
      files,
      runtime_source_paths,
      collected_roots,
      SourceRootMode::DeclarationsOnly,
    )
  }

  fn collect_declaration_input(
    &self,
    input: &Path,
    files: &mut FxHashMap<PathBuf, Arc<str>>,
    runtime_source_paths: &mut FxHashSet<PathBuf>,
    collected_roots: &mut FxHashSet<PathBuf>,
  ) -> Result<()> {
    if input.is_dir() {
      return self.collect_declaration_root(input, files, runtime_source_paths, collected_roots);
    }
    if input.is_file() && SourceClassifier::classify_path(input).is_declaration() {
      let path = canonicalize_or_owned(input);
      let source = Arc::<str>::from(fs_err::read_to_string(&path)?);
      files.insert(path, source);
    }
    Ok(())
  }

  fn collect_root(
    &self,
    root: &Path,
    files: &mut FxHashMap<PathBuf, Arc<str>>,
    runtime_source_paths: &mut FxHashSet<PathBuf>,
    collected_roots: &mut FxHashSet<PathBuf>,
    mode: SourceRootMode,
  ) -> Result<()> {
    if !root.is_dir() {
      return Ok(());
    }
    let root = canonicalize_or_owned(root);
    if !collected_roots.insert(lexical_normalize(&root)) {
      return Ok(());
    }

    let mut paths = Vec::new();
    for entry in walkdir::WalkDir::new(&root)
      .into_iter()
      .filter_map(|entry| entry.ok())
      .filter(|entry| entry.file_type().is_file())
    {
      if HvigorSourceFileFilter::new(entry.path()).is_ignored_macos_metadata() {
        continue;
      }
      let kind = SourceClassifier::classify_path(entry.path());
      if matches!(mode, SourceRootMode::TransformAndDeclarations) && kind.is_javascript_runtime() {
        runtime_source_paths.insert(canonicalize_or_owned(entry.path()));
      }
      let accepts = match mode {
        SourceRootMode::TransformAndDeclarations => {
          kind.is_ets_transform_input() || kind.is_declaration()
        }
        SourceRootMode::DeclarationsOnly => kind.is_declaration(),
      };
      if accepts {
        paths.push(entry.into_path());
      }
    }
    paths.sort();

    let results = paths
      .par_iter()
      .map(|path| {
        fs_err::read_to_string(path)
          .map(|source| (canonicalize_or_owned(path), Arc::<str>::from(source)))
      })
      .collect::<Vec<_>>();
    for result in results {
      let (path, source) = result?;
      files.insert(path, source);
    }
    Ok(())
  }

  fn collect_resolved_static_modules(&self) -> Vec<TransformResolvedStaticModule> {
    let mut package_roots = BTreeSet::new();
    for (_, module_root) in self.module_roots {
      package_roots.insert(canonicalize_or_owned(module_root));
    }
    for root in self.dependency_source_roots {
      package_roots.insert(canonicalize_or_owned(root));
    }

    let mut modules = Vec::new();
    for package_root in package_roots {
      self.collect_package_resolved_static_modules(&package_root, &mut modules);
    }
    modules
  }

  fn collect_package_resolved_static_modules(
    &self,
    package_root: &Path,
    modules: &mut Vec<TransformResolvedStaticModule>,
  ) {
    let Some(package) = OhPackage::load_from_root(package_root) else {
      return;
    };
    for (specifier, dependency_root) in package.file_dependencies() {
      let dependency_root = canonicalize_or_owned(&dependency_root);
      let Some(dependency_package) = OhPackage::load_from_root(&dependency_root) else {
        continue;
      };
      let Some(declaration_entry) = dependency_package.existing_declaration_entry() else {
        continue;
      };
      modules.push(TransformResolvedStaticModule::new(
        package_root,
        specifier,
        declaration_entry,
      ));
    }
  }
}

#[derive(Debug, Clone, Copy)]
enum SourceRootMode {
  TransformAndDeclarations,
  DeclarationsOnly,
}

struct ModuleObfuscationContext {
  config: Option<ObfuscationConfig>,
  dependency_rule_files: Vec<PathBuf>,
  output_target: OutputTarget,
  target_name: Option<String>,
}

struct ModuleArkguardPlanBuilder<'a> {
  config: &'a ArkdownConfig,
  session: &'a BuildSession,
  module: &'a ModuleConfig,
  loader_json: &'a LoaderJson,
  bundle_out: &'a BundleStageOutput,
  node_executable: PathBuf,
}

impl<'a> ModuleArkguardPlanBuilder<'a> {
  fn new(
    config: &'a ArkdownConfig,
    session: &'a BuildSession,
    module: &'a ModuleConfig,
    loader_json: &'a LoaderJson,
    bundle_out: &'a BundleStageOutput,
    node_executable: PathBuf,
  ) -> Self {
    Self {
      config,
      session,
      module,
      loader_json,
      bundle_out,
      node_executable,
    }
  }

  fn build(&self) -> Result<(ArkguardAdapterOptions, ObfuscationModuleInput)> {
    let source_maps_path = self
      .bundle_out
      .bundle_output
      .source_maps_path
      .clone()
      .ok_or_else(|| {
        anyhow::anyhow!(
          "ArkGuard requires the pre-obfuscation sourceMaps.map for module '{}'",
          self.module.name
        )
      })?;
    let source_maps: serde_json::Map<String, serde_json::Value> =
      serde_json::from_slice(&std::fs::read(&source_maps_path)?)?;
    let files = self
      .bundle_out
      .bundle_output
      .module_source_files
      .iter()
      .map(|source| {
        let previous_source_map =
          source_maps
            .get(&source.source_file)
            .cloned()
            .ok_or_else(|| {
              anyhow::anyhow!(
                "sourceMaps.map has no entry '{}' for ArkGuard input {}",
                source.source_file,
                source.path.display()
              )
            })?;
        let relative_path = previous_source_map
          .get("sources")
          .and_then(serde_json::Value::as_array)
          .and_then(|sources| sources.first())
          .and_then(serde_json::Value::as_str)
          .filter(|source| !source.is_empty())
          .ok_or_else(|| {
            anyhow::anyhow!(
              "source map '{}' has no original source path",
              source.source_file
            )
          })?
          .to_string();
        let original_path = PathBuf::from(&source.module_id);
        if !original_path.is_file() {
          anyhow::bail!(
            "ArkGuard original source does not exist for '{}': {}",
            source.source_file,
            original_path.display()
          );
        }
        Ok(ObfuscationModuleFile {
          build_path: source.path.clone(),
          original_path,
          relative_path,
          source_map_key: source.source_file.clone(),
          previous_source_map: Some(previous_source_map),
        })
      })
      .collect::<Result<Vec<_>>>()?;
    if files.is_empty() {
      anyhow::bail!(
        "ArkGuard source obfuscation has no CompileArkTS module sources for '{}'",
        self.module.name
      );
    }
    let module_context = self
      .session
      .module_context(&self.module.name)
      .ok_or_else(|| {
        anyhow::anyhow!(
          "missing build session module context for {}",
          self.module.name
        )
      })?;
    let package = module_context.module.package.as_ref().ok_or_else(|| {
      anyhow::anyhow!(
        "ArkGuard requires oh-package.json5 metadata for module '{}'",
        self.module.name
      )
    })?;
    let module_path_map = self
      .loader_json
      .module_path_map
      .iter()
      .map(|(name, path)| (name.clone(), PathBuf::from(path)))
      .collect();
    let adapter = ArkguardAdapterOptions {
      node_executable: self.node_executable.clone(),
      ets_loader_path: self.config.sdk_config.ets_loader_path.clone(),
      arkguard_package: self
        .config
        .sdk_config
        .ets_loader_path
        .join("node_modules")
        .join("arkguard"),
      sdk_api_paths: self
        .config
        .sdk_config
        .arkguard_api_dirs(&self.config.product.runtime_os),
      module_root: self.module.src_path.clone(),
      project_root: self.config.project_root.clone(),
      project_path: ModuleBuildLayout::new(&self.module.src_path).source().ets(),
      cache_path: PathBuf::from(&self.loader_json.cache_path),
      ace_module_build: PathBuf::from(&self.loader_json.ace_module_build),
      ace_module_json_path: ModuleBuildLayout::new(&self.module.src_path).res_ark_module_json(),
      compile_sdk_version: self.config.sdk_config.compile_sdk_version.clone(),
      compatible_sdk_version: parse_api_version(&self.config.product.compatible_sdk_version)
        .unwrap_or(self.config.sdk_config.compile_api_version),
      compatible_sdk_version_stage: self.config.product.compatible_sdk_version_stage.clone(),
      entry_package_info: format!("{}|{}", package.name, package.version),
      package_dir: if self.loader_json.package_manager_type == "ohpm" {
        "oh_modules".to_string()
      } else {
        "node_modules".to_string()
      },
      module_path_map,
      entry_array: self.arkguard_entry_array(),
      resource_table_path: self.generated_resource_table_path(),
      local_packages: Vec::new(),
      compile_har: matches!(self.module.module_type, OhosModuleType::Har),
      compile_shared: matches!(self.module.module_type, OhosModuleType::Shared),
      use_normalized: self.loader_json.use_normalized_ohm_url,
      use_ts_har: false,
      byte_code_har_info: serde_json::to_value(&self.loader_json.byte_code_har_info)?,
    };
    Ok((
      adapter,
      ObfuscationModuleInput {
        files,
        source_maps_path: Some(source_maps_path),
      },
    ))
  }

  fn bytecode_input(&self, module: ObfuscationModuleInput) -> BytecodeObfuscationInput {
    let module_records = self
      .bundle_out
      .bundle_output
      .module_source_files
      .iter()
      .map(|source| BytecodeObfuscationModuleRecord {
        source_path: PathBuf::from(&source.module_id),
        record_name: source.record_name.clone(),
      })
      .collect();
    // Latest SDK `module_mode.js::generateCompileContextInfo` registers only
    // explicit module-meta `ohmurl` values. Arkdown's matching meta collector
    // assigns that shape to generated ResourceTable entries (`&...&`).
    let resource_table_path = self.generated_resource_table_path();
    let mut record_name_white_list = self
      .bundle_out
      .bundle_output
      .module_source_files
      .iter()
      .filter(|source| {
        resource_table_path
          .as_ref()
          .is_some_and(|path| Path::new(&source.module_id) == path)
      })
      .map(|source| source.record_name.clone())
      .collect::<Vec<_>>();
    record_name_white_list.sort();
    record_name_white_list.dedup();
    BytecodeObfuscationInput {
      module,
      modules_abc_path: self.bundle_out.bundle_output.modules_abc_path.clone(),
      module_records,
      record_name_white_list,
    }
  }

  fn arkguard_entry_array(&self) -> Vec<String> {
    let mut entries = self.loader_json.runtime_entry_sources.clone();
    entries.extend(self.loader_json.hvigor_compile_entry.iter().cloned());
    let mut seen = FxHashSet::default();
    entries.retain(|entry| seen.insert(entry.clone()));
    entries
  }

  fn generated_resource_table_path(&self) -> Option<PathBuf> {
    self
      .loader_json
      .hvigor_compile_entry
      .iter()
      .map(PathBuf::from)
      .find(|path| {
        path.file_name().and_then(|name| name.to_str()) == Some("ResourceTable.ts")
          && path
            .strip_prefix(&self.module.src_path)
            .is_ok_and(|relative| {
              relative.starts_with("build")
                && relative
                  .components()
                  .any(|part| part.as_os_str() == "generated")
            })
      })
  }
}

struct PendingBytecodeObfuscation {
  obfuscator: Obfuscator,
  input: BytecodeObfuscationInput,
}

impl PendingBytecodeObfuscation {
  fn new(obfuscator: Obfuscator, input: BytecodeObfuscationInput) -> Self {
    Self { obfuscator, input }
  }

  fn run(self, module_name: &str) -> Result<()> {
    self
      .obfuscator
      .obfuscate_bytecode(self.input)
      .map_err(|error| {
        anyhow::anyhow!(
          "Bytecode obfuscation failed for module '{}': {}",
          module_name,
          error
        )
      })?;
    Ok(())
  }
}

impl ModuleObfuscationContext {
  fn resolve(
    config: &ArkdownConfig,
    session: &BuildSession,
    module: &ModuleConfig,
    build_option: &arkdown_core::BuildOption,
  ) -> Result<Self> {
    let effective = build_option
      .ark_options
      .as_ref()
      .and_then(|options| options.obfuscation.clone());
    let dependency_rule_files = if effective
      .as_ref()
      .is_some_and(|obfuscation| obfuscation.is_enabled_for_mode(&config.build_mode))
    {
      let module_context = session
        .module_context(&module.name)
        .ok_or_else(|| anyhow::anyhow!("missing module compile context for {}", module.name))?;
      ObfuscationDependencyRuleCollector::new(module_context.deps.as_ref()).collect()
    } else {
      Vec::new()
    };
    Ok(Self {
      config: effective,
      dependency_rule_files,
      output_target: config.output_target.clone(),
      target_name: None,
    })
  }

  fn is_enabled(&self, build_mode: &BuildMode) -> bool {
    // hvigor UnitTestArkCompile/OhosTestArkCompile delete obfuscationOptions;
    // local unit-test and ohosTest builds never obfuscate.
    if matches!(self.output_target, OutputTarget::Test)
      || self.target_name.as_deref() == Some("ohosTest")
    {
      return false;
    }
    self
      .config
      .as_ref()
      .is_some_and(|config| config.is_enabled_for_mode(build_mode))
  }
}

#[cfg(test)]
mod transform_source_snapshot_builder_tests {
  use super::*;
  use arkdown_ets_transform::{RolldownEtsTransformPlan, RolldownEtsTransformer};

  #[test]
  fn custom_type_file_outside_source_roots_is_loaded_into_transform_snapshot() {
    let temp = tempfile::tempdir().expect("tempdir");
    let module_root = temp.path().join("entry");
    let declaration = temp.path().join("shared-types/runtime.d.ts");
    fs_err::create_dir_all(declaration.parent().expect("declaration parent"))
      .expect("declaration directory");
    fs_err::write(
      &declaration,
      "declare interface RuntimeEnvironment { name: string }\n",
    )
    .expect("custom type declaration");

    let module_roots = vec![("entry".to_string(), module_root)];
    let custom_type_inputs = vec![declaration.clone()];
    let sources = TransformSourceSnapshotBuilder::new(&module_roots, &[], &[], &[])
      .with_custom_type_declaration_inputs(&custom_type_inputs)
      .build()
      .expect("source snapshot");

    assert_eq!(
      sources.source_text(&declaration).as_deref(),
      Some("declare interface RuntimeEnvironment { name: string }\n")
    );
  }

  #[test]
  fn native_so_dependency_roots_provide_declaration_imports_to_ets_transform() {
    let temp = tempfile::tempdir().expect("tempdir");
    let package_root = temp.path().join("oh_modules/@meituan/crypto");
    let module_root = temp.path().join("entry");
    let source_path = package_root.join("src/main/ets/core/AES.ets");
    let native_root = package_root.join("src/main/cpp/types/libmtcrypto");
    let declaration_path = native_root.join("index.d.ts");

    fs_err::create_dir_all(source_path.parent().expect("source parent")).expect("source dir");
    fs_err::create_dir_all(&native_root).expect("native declaration dir");

    let source = r#"import libmtcrypto from "libmtcrypto.so";
import { EncryptionResult } from "../../cpp/types/libmtcrypto";

export class AESUtil {
    static encrypt(): EncryptionResult {
        return libmtcrypto.AESEncrypt("", "", "");
    }
}

export { EncryptionResult };
"#;
    fs_err::write(&source_path, source).expect("source file");
    fs_err::write(
      &declaration_path,
      r#"export interface EncryptionResult {
    result: string;
    errorCode: number;
}
export const AESEncrypt: (input: string, key: string, iv: string) => EncryptionResult;
"#,
    )
    .expect("declaration file");

    let project_path = module_root.join("src/main/ets");
    let module_roots = vec![("entry".to_string(), module_root)];
    let dependency_source_roots = vec![package_root];
    let native_declaration_roots = vec![native_root];
    let sources = TransformSourceSnapshotBuilder::new(
      &module_roots,
      &dependency_source_roots,
      &native_declaration_roots,
      &[],
    )
    .build()
    .expect("source snapshot");
    let plan = RolldownEtsTransformPlan::new(
      "com.example",
      "entry",
      temp.path().to_path_buf(),
      project_path,
      module_roots,
      12,
      FxHashMap::default(),
      ComponentMetadata::default(),
      sources,
    );
    let transformer = RolldownEtsTransformer::new(Arc::new(plan));
    let output = transformer
      .compile_transform(source, source_path.to_str().expect("utf-8 source path"))
      .expect("transform")
      .expect("ets output");

    assert!(
      output.code.contains("libmtcrypto.so"),
      "native runtime import must remain: {}",
      output.code
    );
    assert!(
      output
        .code
        .contains(r#"import type { EncryptionResult } from "../../cpp/types/libmtcrypto";"#),
      "native declaration import must become type-only for Rolldown TS parsing: {}",
      output.code
    );
    assert!(
      output.code.contains("export type { EncryptionResult };"),
      "type-only local export must remain type-only for Rolldown TS parsing: {}",
      output.code
    );
    assert!(
      !output.code.contains("import { EncryptionResult }"),
      "native declaration import must not remain as a runtime import: {}",
      output.code
    );
    assert!(
      !output.code.contains("export { EncryptionResult }"),
      "type-only local export must not remain as a runtime export: {}",
      output.code
    );
  }

  #[test]
  fn native_declaration_roots_follow_ohpm_symlinks() {
    let temp = tempfile::tempdir().expect("tempdir");
    let store_package_root = temp
      .path()
      .join("oh_modules/.ohpm/@meituan+crypto@hash=/oh_modules/@meituan/crypto");
    let symlink_package_root = temp.path().join("oh_modules/@meituan/crypto");
    let module_root = temp.path().join("entry");
    let source_path = store_package_root.join("src/main/ets/core/AES.ets");
    let store_native_root = store_package_root.join("src/main/cpp/types/libmtcrypto");
    let symlink_native_root = symlink_package_root.join("src/main/cpp/types/libmtcrypto");
    let declaration_path = store_native_root.join("index.d.ts");

    fs_err::create_dir_all(source_path.parent().expect("source parent")).expect("source dir");
    fs_err::create_dir_all(&store_native_root).expect("native declaration dir");
    fs_err::create_dir_all(symlink_package_root.parent().expect("symlink parent"))
      .expect("symlink parent dir");
    symlink_dir(&store_package_root, &symlink_package_root).expect("package symlink");

    let source = r#"import libmtcrypto from "libmtcrypto.so";
import { EncryptionResult } from "../../cpp/types/libmtcrypto";

export class AESUtil {
    static encrypt(): EncryptionResult {
        return libmtcrypto.AESEncrypt("", "", "");
    }
}

export { EncryptionResult };
"#;
    fs_err::write(&source_path, source).expect("source file");
    fs_err::write(
      &declaration_path,
      r#"export interface EncryptionResult {
    result: string;
    errorCode: number;
}
export const AESEncrypt: (input: string, key: string, iv: string) => EncryptionResult;
"#,
    )
    .expect("declaration file");

    let project_path = module_root.join("src/main/ets");
    let module_roots = vec![("entry".to_string(), module_root)];
    let dependency_source_roots = vec![symlink_package_root];
    let native_declaration_roots = vec![symlink_native_root];
    let sources = TransformSourceSnapshotBuilder::new(
      &module_roots,
      &dependency_source_roots,
      &native_declaration_roots,
      &[],
    )
    .build()
    .expect("source snapshot");
    let plan = RolldownEtsTransformPlan::new(
      "com.example",
      "entry",
      temp.path().to_path_buf(),
      project_path,
      module_roots,
      12,
      FxHashMap::default(),
      ComponentMetadata::default(),
      sources,
    );
    let transformer = RolldownEtsTransformer::new(Arc::new(plan));
    let output = transformer
      .compile_transform(source, source_path.to_str().expect("utf-8 source path"))
      .expect("transform")
      .expect("ets output");

    assert!(
      output.code.contains("libmtcrypto.so"),
      "native runtime import must remain: {}",
      output.code
    );
    assert!(
      output
        .code
        .contains(r#"import type { EncryptionResult } from "../../cpp/types/libmtcrypto";"#),
      "declaration import from canonical ohpm path must become type-only: {}",
      output.code
    );
    assert!(
      !output.code.contains("import { EncryptionResult }"),
      "declaration import from canonical ohpm path must not remain as runtime import: {}",
      output.code
    );
  }

  #[test]
  fn native_so_declaration_resolution_uses_package_dependency_types_entry() {
    let temp = tempfile::tempdir().expect("tempdir");
    let package_root = temp.path().join("oh_modules/@scope/widget");
    let module_root = temp.path().join("entry");
    let source_path = package_root.join("Index.ets");
    let native_root = package_root.join("native-declarations/arbitrary-location");
    let native_entry = native_root.join("Entry.d.ts");
    let native_types = native_root.join("Types.d.ts");

    fs_err::create_dir_all(&package_root).expect("package dir");
    fs_err::create_dir_all(&native_root).expect("native declaration dir");
    fs_err::write(
      package_root.join("oh-package.json5"),
      r#"{
  "name": "@scope/widget",
  "version": "1.0.0",
  "dependencies": {
    "libwidget.so": "file:./native-declarations/arbitrary-location"
  }
}"#,
    )
    .expect("package manifest");
    fs_err::write(
      native_root.join("oh-package.json5"),
      r#"{
  "name": "libwidget.so",
  "version": "1.0.0",
  "types": "./Entry.d.ts"
}"#,
    )
    .expect("native manifest");
    fs_err::write(
      &source_path,
      r#"export { EdgeType, nativeCall } from "libwidget.so";"#,
    )
    .expect("source file");
    fs_err::write(
      &native_entry,
      r#"export * from "./Types";
export const nativeCall: () => void;"#,
    )
    .expect("native declaration entry");
    fs_err::write(
      &native_types,
      r#"export const enum EdgeType {
    TOP = 1
}"#,
    )
    .expect("native declaration types");

    let project_path = module_root.join("src/main/ets");
    let module_roots = vec![("entry".to_string(), module_root)];
    let dependency_source_roots = vec![package_root];
    let native_declaration_roots = vec![native_root];
    let sources = TransformSourceSnapshotBuilder::new(
      &module_roots,
      &dependency_source_roots,
      &native_declaration_roots,
      &[],
    )
    .build()
    .expect("source snapshot");
    let plan = RolldownEtsTransformPlan::new(
      "com.example",
      "entry",
      temp.path().to_path_buf(),
      project_path,
      module_roots,
      12,
      FxHashMap::default(),
      ComponentMetadata::default(),
      sources,
    );
    let transformer = RolldownEtsTransformer::new(Arc::new(plan));
    let output = transformer
      .compile_transform(
        r#"export { EdgeType, nativeCall } from "libwidget.so";"#,
        source_path.to_str().expect("utf-8 source path"),
      )
      .expect("transform")
      .expect("ets output");

    assert!(
      output.code.contains("nativeCall") && output.code.contains("libwidget.so"),
      "native runtime reexport must remain: {}",
      output.code
    );
    assert!(
      output
        .code
        .contains(r#"export type { EdgeType } from "libwidget.so";"#),
      "native declaration const enum must become a type-only reexport: {}",
      output.code
    );
    assert!(
      !output.code.contains("export { EdgeType"),
      "native declaration const enum must not remain in runtime reexport: {}",
      output.code
    );
  }

  #[cfg(unix)]
  fn symlink_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(src, dst)
  }

  #[cfg(windows)]
  fn symlink_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    junction::create(src, dst)
  }
}

struct CompileEnvKeyBuilder<'a> {
  config: &'a ArkdownConfig,
  module: &'a ModuleConfig,
  loader_json: &'a LoaderJson,
  obfuscation_enabled: bool,
  target_name: &'a str,
}

impl<'a> CompileEnvKeyBuilder<'a> {
  fn new(
    config: &'a ArkdownConfig,
    module: &'a ModuleConfig,
    loader_json: &'a LoaderJson,
    obfuscation_enabled: bool,
    target_name: &'a str,
  ) -> Self {
    Self {
      config,
      module,
      loader_json,
      obfuscation_enabled,
      target_name,
    }
  }

  fn build(&self) -> Result<String> {
    let ets_loader_path =
      ArtifactPathSerializer::path_to_slash_string(&self.config.sdk_config.ets_loader_path)
        .map_err(|error| {
          anyhow::anyhow!("serialize ETS loader path for compile env failed: {error}")
        })?;
    let compiler_cache_path = arkdown_cache::compiler_cache_path(&self.loader_json.cache_path);

    Ok(format!(
      "{}#target={}",
      arkdown_cache::CompileEnv::from_plan(
        self.config.sdk_config.compile_sdk_version.clone(),
        self.config.product.compatible_sdk_version.clone(),
        self.config.product.runtime_os.clone(),
        ets_loader_path,
        self.obfuscation_enabled,
        format!("{:?}", self.module.module_type),
        compiler_cache_path,
      )
      .to_key_string(),
      self.target_name
    ))
  }
}

struct PkgContextInfoInputBuilder<'a> {
  config: &'a ArkdownConfig,
  session: &'a BuildSession,
  module: &'a ModuleConfig,
  module_context: &'a ModuleCompileContext,
  excluded_package_roots: FxHashSet<PathBuf>,
}

impl<'a> PkgContextInfoInputBuilder<'a> {
  fn new(
    config: &'a ArkdownConfig,
    session: &'a BuildSession,
    module: &'a ModuleConfig,
    module_context: &'a ModuleCompileContext,
  ) -> Self {
    Self {
      config,
      session,
      module,
      module_context,
      excluded_package_roots: FxHashSet::default(),
    }
  }

  fn excluding_package_roots(mut self, roots: &[PathBuf]) -> Self {
    self.excluded_package_roots = roots
      .iter()
      .map(|root| canonicalize_or_owned(root))
      .collect();
    self
  }

  fn is_excluded(&self, node: &DependencyNodeSnapshot) -> bool {
    node.package_root.as_ref().is_some_and(|root| {
      self
        .excluded_package_roots
        .contains(&canonicalize_or_owned(root))
    })
  }

  fn build(&self) -> Result<PkgContextInfoInput> {
    let package = self.module_context.module.package.as_ref().ok_or_else(|| {
      anyhow::anyhow!(
        "GeneratePkgContextInfo requires oh-package.json5 snapshot for module '{}'",
        self.module.name
      )
    })?;
    let source_root = match self.module_context.module.module_type {
      SessionOhosModuleType::Entry | SessionOhosModuleType::Feature => {
        self.module_context.module.src_main.clone()
      }
      SessionOhosModuleType::Shared | SessionOhosModuleType::Har => self
        .module_context
        .module
        .source_roots
        .first()
        .cloned()
        .unwrap_or_else(|| self.module_context.module.src_main.clone()),
      SessionOhosModuleType::Unknown(ref module_type) => {
        anyhow::bail!(
          "GeneratePkgContextInfo does not support unknown module type '{}'",
          module_type
        )
      }
    };

    let mut dependencies = Vec::with_capacity(self.module_context.deps.nodes.len());
    for node in self.module_context.deps.nodes.values() {
      if self.is_excluded(node) {
        continue;
      }
      dependencies.push(PkgContextDependencyInput {
        alias: node.alias.clone(),
        package_name: node.package_name.clone(),
        version_spec: self.dependency_version(node)?,
        package_root: node.package_root.clone(),
        node_entry: node.package.as_deref().map(Self::dependency_node_entry),
        main: node.package.as_ref().and_then(|package| {
          package
            .package_json
            .main
            .clone()
            .or_else(|| package.package_json.types.clone())
        }),
        kind: self.dependency_kind(node.node_type),
        module_name: node
          .module_profile
          .as_ref()
          .and_then(|profile| profile.module_name.clone()),
        bundle_name: node
          .module_profile
          .as_ref()
          .and_then(|profile| profile.bundle_name.clone()),
        bundle_type: node
          .module_profile
          .as_ref()
          .and_then(|profile| profile.bundle_type.clone()),
        oh_export_files: node
          .package
          .as_ref()
          .and_then(|package| package.oh_export_files.clone()),
      });
    }
    let native_build_option = self.effective_native_build_option()?;
    let native_libraries = NativeLibCollector::new(
      self.module,
      &native_build_option.abi_filters,
      self.module_context.deps.as_ref(),
    )
    .excluding_dependency_roots(
      &self
        .excluded_package_roots
        .iter()
        .cloned()
        .collect::<Vec<_>>(),
    )
    .with_policy(
      NativeLibCollectionPolicy::default()
        .with_native_lib_option(native_build_option.native_lib)
        .with_self_package(self.self_package_info()),
    )
    .collect_context_names();

    Ok(PkgContextInfoInput {
      current: PkgContextCurrentInput {
        package_name: package.name.clone(),
        bundle_name: self.session.project.bundle_name.clone(),
        bundle_type: self.session.project.bundle_type.clone(),
        module_kind: self.current_module_kind()?,
        module_root: self.module_context.module.root.clone(),
        source_root,
        main: package.package_json.main.clone(),
        version: package.version.clone(),
        oh_export_files: package.oh_export_files.clone(),
      },
      dependencies,
      native_libraries,
    })
  }

  /// Match hvigor-arkts-compose `findNodeEntryFiles()` entry-key priority.
  fn dependency_node_entry(package: &PackageSnapshot) -> String {
    for key in ["exports", "module", "jsnext:main"] {
      if let Some(entry) = package
        .package_json
        .extra
        .get(key)
        .and_then(Self::node_entry_from_value)
      {
        return entry;
      }
    }
    if let Some(main) = package
      .package_json
      .main
      .as_deref()
      .filter(|entry| !entry.is_empty())
    {
      return main.to_string();
    }
    "index.js".to_string()
  }

  /// Match `module-meta-info-plugin.ts#parseEntryFiles` and return the first
  /// candidate consumed by `dependency-manager.ts#getEntryMainFilePath`.
  fn node_entry_from_value(value: &serde_json::Value) -> Option<String> {
    match value {
      serde_json::Value::String(entry) => (!entry.is_empty()).then(|| entry.clone()),
      serde_json::Value::Array(entries) => entries.iter().find_map(Self::node_entry_from_value),
      serde_json::Value::Object(entries) => ["import", ".", "default", "require", "node"]
        .into_iter()
        .find_map(|key| entries.get(key).and_then(Self::node_entry_from_value)),
      _ => None,
    }
  }

  fn source_map_package_info(&self) -> Result<BundleSourceMapPackageInfo> {
    let package = self.module_context.module.package.as_ref().ok_or_else(|| {
      anyhow::anyhow!(
        "SourceMapGenerator requires oh-package.json5 snapshot for module '{}'",
        self.module.name
      )
    })?;
    if package.name.is_empty() || package.version.is_empty() {
      anyhow::bail!(
        "SourceMapGenerator requires package name and version for module '{}'",
        self.module.name
      );
    }

    let mut dependency_package_versions = FxHashMap::default();
    for (name, version) in package.package_json.dev_dependencies.iter() {
      if !version.is_empty() {
        dependency_package_versions
          .entry(name.clone())
          .or_insert_with(|| version.clone());
      }
    }
    if let Some(root_package) = self.session.project.root_package.as_ref() {
      for (name, version) in root_package.package_json.dev_dependencies.iter() {
        if !version.is_empty() {
          dependency_package_versions
            .entry(name.clone())
            .or_insert_with(|| version.clone());
        }
      }
    }
    for node in self.module_context.deps.nodes.values() {
      if self.is_excluded(node) {
        continue;
      }
      let Some(version) = node
        .package
        .as_ref()
        .map(|package| package.version.as_str())
        .filter(|version| !version.is_empty())
      else {
        continue;
      };
      dependency_package_versions
        .entry(node.package_name.clone())
        .or_insert_with(|| version.to_string());
      if node.alias != node.package_name {
        dependency_package_versions
          .entry(node.alias.clone())
          .or_insert_with(|| version.to_string());
      }
    }

    Ok(BundleSourceMapPackageInfo {
      entry_package_name: package.name.clone(),
      entry_package_version: package.version.clone(),
      dependency_package_versions,
    })
  }

  fn effective_native_build_option(
    &self,
  ) -> Result<arkdown_config::build_profile::EffectiveNativeBuildOption> {
    Ok(
      arkdown_config::build_profile::EffectiveNativeBuildOption::from_build_option(
        &self.config.product.build_option,
      ),
    )
  }

  fn self_package_info(&self) -> NativeLibPackageInfo {
    self
      .module_context
      .module
      .package
      .as_ref()
      .map(|package| NativeLibPackageInfo::local(package.name.clone(), package.version.clone()))
      .unwrap_or_else(|| NativeLibPackageInfo::local(self.module.name.clone(), String::new()))
  }

  fn current_module_kind(&self) -> Result<PkgContextModuleKind> {
    Ok(match self.module_context.module.module_type {
      SessionOhosModuleType::Entry | SessionOhosModuleType::Feature => PkgContextModuleKind::Hap,
      SessionOhosModuleType::Shared => PkgContextModuleKind::Hsp,
      SessionOhosModuleType::Har => PkgContextModuleKind::Har,
      SessionOhosModuleType::Unknown(ref module_type) => {
        anyhow::bail!(
          "GeneratePkgContextInfo does not support unknown module type '{}'",
          module_type
        )
      }
    })
  }

  fn dependency_kind(&self, node_type: DependencyNodeType) -> PkgContextDependencyKind {
    match node_type {
      DependencyNodeType::SharedHsp | DependencyNodeType::IntegratedHsp => {
        PkgContextDependencyKind::Hsp
      }
      DependencyNodeType::NativeSo => PkgContextDependencyKind::So,
      DependencyNodeType::SourceHar | DependencyNodeType::BytecodeHar => {
        PkgContextDependencyKind::Har
      }
      DependencyNodeType::OhpmSource | DependencyNodeType::Unknown => {
        PkgContextDependencyKind::Other
      }
    }
  }

  fn dependency_version(&self, node: &DependencyNodeSnapshot) -> Result<Option<String>> {
    match self.dependency_kind(node.node_type) {
      PkgContextDependencyKind::Hsp | PkgContextDependencyKind::So => Ok(None),
      PkgContextDependencyKind::Har | PkgContextDependencyKind::Other => {
        let package = node.package.as_ref().ok_or_else(|| {
          anyhow::anyhow!(
            "GeneratePkgContextInfo requires package metadata for dependency '{}'",
            node.package_name
          )
        })?;
        if package.version.is_empty() {
          anyhow::bail!(
            "GeneratePkgContextInfo requires package version for dependency '{}'",
            node.package_name
          );
        }
        Ok(Some(package.version.clone()))
      }
    }
  }
}

struct SanitizerProfileFlags {
  asan: bool,
  hwasan: bool,
}

impl SanitizerProfileFlags {
  fn new(asan: bool, hwasan: bool) -> Self {
    Self { asan, hwasan }
  }

  fn apply_to_module_json(&self, path: &Path) -> Result<()> {
    if !self.asan && !self.hwasan {
      return Ok(());
    }
    let content = fs_err::read_to_string(path)?;
    let mut value: serde_json::Value = serde_json::from_str(&content)?;
    let root = value
      .as_object_mut()
      .ok_or_else(|| anyhow::anyhow!("module.json root must be an object"))?;
    let app = root
      .entry("app")
      .or_insert_with(|| serde_json::json!({}))
      .as_object_mut()
      .ok_or_else(|| anyhow::anyhow!("module.json app must be an object"))?;
    if self.asan {
      app.insert("asanEnabled".to_string(), serde_json::Value::Bool(true));
    }
    if self.hwasan {
      app.insert("hwasanEnabled".to_string(), serde_json::Value::Bool(true));
    }
    fs_err::write(path, serde_json::to_vec(&value)?)?;
    Ok(())
  }
}

struct PackageModuleJsonOptions<'a> {
  path: &'a Path,
}

impl<'a> PackageModuleJsonOptions<'a> {
  fn new(path: &'a Path) -> Self {
    Self { path }
  }

  fn compress_native_libs(&self) -> Result<bool> {
    let content = fs_err::read_to_string(self.path)?;
    let value: serde_json::Value = serde_json::from_str(&content)?;
    Ok(
      value
        .get("module")
        .and_then(|module| module.get("compressNativeLibs"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false),
    )
  }

  fn write_for_pack(
    &self,
    output: &Path,
    deduplicate_har: Option<bool>,
    preload_system_so_effective: bool,
  ) -> Result<PathBuf> {
    let content = fs_err::read_to_string(self.path)?;
    let mut value: serde_json::Value = serde_json::from_str(&content)?;
    let module = value
      .get_mut("module")
      .and_then(serde_json::Value::as_object_mut)
      .ok_or_else(|| anyhow::anyhow!("package module.json must contain a module object"))?;
    if preload_system_so_effective {
      module
        .entry("appStartup")
        .or_insert_with(|| serde_json::Value::String("$profile:startup_config".to_string()));
    }
    if let Some(deduplicate_har) = deduplicate_har {
      module.insert(
        "deduplicateHar".to_string(),
        serde_json::Value::Bool(deduplicate_har),
      );
    }
    if let Some(parent) = output.parent() {
      fs_err::create_dir_all(parent)?;
    }
    // GeneratePkgModuleJson is the PackageHap input in hvigor. It writes the
    // schema-ordered compact form before app_packing_tool parses it into its
    // HashMap-backed archive entry. Passing the resource-stage insertion order
    // through here changes collision order in that final serializer.
    let generated = serde_json::to_string(&value)?;
    let generated = minify_package_intermediate_module_json(&generated)?;
    fs_err::write(output, generated.as_bytes())?;
    Ok(output.to_path_buf())
  }
}

struct CompileContextInfoWriter<'a> {
  loader_json: &'a LoaderJson,
  module_source_files: &'a [arkdown_core::HvigorModuleSourceFile],
  bundle_type: &'a str,
  bundle_name: &'a str,
  /// `isFormWidgetModule`: the widget pass joins form card sources into the
  /// main es2abc input only on the form widget module itself.
  form_widget_module: bool,
}

impl<'a> CompileContextInfoWriter<'a> {
  fn new(
    loader_json: &'a LoaderJson,
    module_source_files: &'a [arkdown_core::HvigorModuleSourceFile],
    _pkg_context_info: &'a BTreeMap<String, PkgContextInfoOpt>,
    bundle_type: &'a str,
    bundle_name: &'a str,
    form_widget_module: bool,
  ) -> Self {
    Self {
      loader_json,
      module_source_files,
      bundle_type,
      bundle_name,
      form_widget_module,
    }
  }

  fn write(&self, output_path: &Path) -> Result<()> {
    if let Some(parent) = output_path.parent() {
      fs_err::create_dir_all(parent)?;
    }
    let hsp_pkg_names = self.hsp_pkg_names();
    let compile_entries = self.compile_entries(&hsp_pkg_names);
    let update_version_info = self
      .loader_json
      .update_version_info
      .iter()
      .map(|(package_name, value)| (package_name.clone(), value.clone()))
      .collect::<BTreeMap<_, _>>();
    let mut value = serde_json::json!({
        "hspPkgNames": hsp_pkg_names,
        "compileEntries": compile_entries,
        "updateVersionInfo": update_version_info,
    });
    if matches!(self.bundle_type, "shared" | "appPlugin") {
      value["needModifyRecord"] = serde_json::Value::Bool(true);
      value["bundleName"] = serde_json::Value::String(self.bundle_name.to_string());
    }
    fs_err::write(output_path, serde_json::to_vec(&value)?)?;
    Ok(())
  }

  fn hsp_pkg_names(&self) -> Vec<String> {
    let mut names = self
      .loader_json
      .hsp_name_ohm_map
      .keys()
      .cloned()
      .collect::<Vec<_>>();
    names.sort();
    names
  }

  fn compile_entries(&self, hsp_pkg_names: &[String]) -> Vec<String> {
    let hsp_names = hsp_pkg_names
      .iter()
      .map(String::as_str)
      .collect::<BTreeSet<_>>();
    // Latest SDK `ModuleMode.generateCompileContextInfo()` iterates
    // `projectConfig.entryObj` into a JavaScript Set. Preserve that first-seen
    // order: sorting roots can change initialization order for cyclic ESM
    // graphs even though the same record names remain present.
    let mut entries = Vec::new();
    let mut seen_entries = FxHashSet::default();
    let source_records = self.source_record_index();
    for source_entry in self.loader_json.hvigor_compile_entry.iter() {
      if let Some(record_name) = self.record_name_for_source_entry(source_entry, &source_records) {
        if seen_entries.insert(record_name.clone()) {
          entries.push(record_name);
        }
      }
    }
    let other_compile_files = self
      .loader_json
      .other_compile_files
      .iter()
      .map(String::as_str)
      .collect::<BTreeSet<_>>();
    for source_entry in self.loader_json.compile_entry.iter() {
      if other_compile_files.contains(source_entry.as_str()) {
        continue;
      }
      if let Some(record_name) = self.record_name_for_source_entry(source_entry, &source_records) {
        if seen_entries.insert(record_name.clone()) {
          entries.push(record_name);
        }
      }
    }
    for runtime_entry in &self.loader_json.runtime_entry_sources {
      if let Some(record_name) = self.record_name_for_source_entry(runtime_entry, &source_records) {
        if seen_entries.insert(record_name.clone()) {
          entries.push(record_name);
        }
      }
    }
    // hvigor's widget pass (`needSubmitArkTsWidget`) compiles the form card
    // sources into the main es2abc input (`filesInfo.txt`) in addition to the
    // dedicated `widgets.abc` — but only on the form widget module itself.
    // The form extension module (HAP) keeps them out (`getFormJsonArr`
    // returns [] for `isFormExtensionModule`).
    if self.form_widget_module {
      for form_entry in &self.loader_json.form_entry_sources {
        if let Some(record_name) = self.record_name_for_source_entry(form_entry, &source_records) {
          if seen_entries.insert(record_name.clone()) {
            entries.push(record_name);
          }
        }
      }
    }
    for ohm_url in &self.loader_json.declaration_entry {
      if let Some(record_name) = self.record_name_from_ohm_url(ohm_url, &hsp_names) {
        if seen_entries.insert(record_name.clone()) {
          entries.push(record_name);
        }
      }
    }
    for router_entry in &self.loader_json.router_map {
      if let Some(ohm_url) = router_entry
        .get("ohmurl")
        .and_then(serde_json::Value::as_str)
      {
        if let Some(record_name) = self.record_name_from_ohm_url(ohm_url, &hsp_names) {
          if seen_entries.insert(record_name.clone()) {
            entries.push(record_name);
          }
        }
      }
    }
    // Hvigor writes compile entries as dependency roots. The ArkTS compiler
    // resolves static and dynamic imports from those roots; expanding the full
    // import closure here turns dependencies into roots and can perturb module
    // initialization order for cyclic ESM graphs.
    entries
  }

  fn source_record_index(&self) -> FxHashMap<SourcePathKey, &str> {
    let mut records = FxHashMap::default();
    for source in self.module_source_files {
      if let Some(key) = SourcePathKey::from_module_source(source) {
        records.entry(key).or_insert(source.record_name.as_str());
      }
    }
    records
  }

  fn record_name_for_source_entry(
    &self,
    source_entry: &str,
    source_records: &FxHashMap<SourcePathKey, &str>,
  ) -> Option<String> {
    let expected =
      SourcePathKey::from_source_entry(source_entry, Path::new(&self.loader_json.module_root_path));
    source_records
      .get(&expected)
      .map(|record_name| (*record_name).to_string())
  }

  fn record_name_from_ohm_url(
    &self,
    ohm_url: &str,
    hsp_pkg_names: &BTreeSet<&str>,
  ) -> Option<String> {
    let normalized = NormalizedOhmUrl::parse(ohm_url)?;
    let package_name = normalized.package_name()?;
    if hsp_pkg_names.contains(package_name) {
      return None;
    }
    normalized.record_name()
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SourcePathKey {
  value: PathBuf,
}

impl SourcePathKey {
  fn from_source_entry(source_entry: &str, module_root: &Path) -> Self {
    let path = Path::new(source_entry);
    let value = if path.has_root() {
      path.to_path_buf()
    } else {
      module_root.join(Self::trim_relative_prefix(path))
    };
    Self::new(value)
  }

  fn from_module_source(source: &arkdown_core::HvigorModuleSourceFile) -> Option<Self> {
    let module_id_path = Path::new(&source.module_id);
    if module_id_path.has_root() {
      return Some(Self::new(module_id_path));
    }
    source
      .source_file
      .rsplit_once('|')
      .map(|(_, source_file)| Self::new(Path::new(source_file)))
  }

  fn new(path: impl AsRef<Path>) -> Self {
    let path = canonicalize_or_owned(path.as_ref());
    Self {
      value: lexical_normalize(&path).with_extension(""),
    }
  }

  fn trim_relative_prefix(path: &Path) -> PathBuf {
    path
      .components()
      .filter(|component| !matches!(component, std::path::Component::CurDir))
      .collect()
  }
}

#[cfg(test)]
mod compile_context_info_tests {
  use super::*;
  use arkdown_core::HvigorModuleSourceFile;
  use indexmap::IndexMap;

  #[test]
  fn compile_target_uses_hvigor_compatible_sdk_version_over_installed_sdk_api() {
    let target = HvigorCompileTarget::from_compatible_sdk_version("5.0.0(12)", Some("beta3"), 24);

    assert_eq!(target.api_version(), 12);
    assert_eq!(target.api_sub_version().as_deref(), Some("beta3"));
  }

  #[test]
  fn compile_entries_preserve_latest_hvigor_entry_obj_insertion_order() {
    let temp = tempfile::tempdir().expect("tempdir");
    let module_root = temp.path().join("entry");
    let index = module_root.join("src/main/ets/Index.ets");
    let module_info = module_root.join("build/default/generated/profile/default/ModuleInfo.ts");
    let mut loader_json = test_loader_json(&module_root);
    loader_json.compile_entry = vec![
      index.to_string_lossy().into_owned(),
      module_info.to_string_lossy().into_owned(),
      index.to_string_lossy().into_owned(),
    ];
    let module_source_files = vec![
      HvigorModuleSourceFile {
        module_id: module_info.to_string_lossy().into_owned(),
        path: temp.path().join("cache/ModuleInfo.js"),
        record_name: "&entry/build/default/generated/profile/default/ModuleInfo&1.0.0".to_string(),
        source_file: "entry|entry|1.0.0|build/default/generated/profile/default/ModuleInfo.js"
          .to_string(),
        package_name: Some("entry".to_string()),
        is_commonjs: false,
        is_shared_module: false,
        origin_source_lang: "ts".to_string(),
        imported_record_names: vec![],
      },
      HvigorModuleSourceFile {
        module_id: index.to_string_lossy().into_owned(),
        path: temp.path().join("cache/Index.ts"),
        record_name: "&entry/src/main/ets/Index&1.0.0".to_string(),
        source_file: "entry|entry|1.0.0|src/main/ets/Index.ts".to_string(),
        package_name: Some("entry".to_string()),
        is_commonjs: false,
        is_shared_module: false,
        origin_source_lang: "ets".to_string(),
        imported_record_names: vec![],
      },
    ];
    let pkg_context_info = BTreeMap::new();
    let writer = CompileContextInfoWriter::new(
      &loader_json,
      &module_source_files,
      &pkg_context_info,
      "app",
      "com.example.entry",
      false,
    );

    assert_eq!(
      writer.compile_entries(&[]),
      vec![
        "&entry/src/main/ets/Index&1.0.0".to_string(),
        "&entry/build/default/generated/profile/default/ModuleInfo&1.0.0".to_string(),
      ]
    );
  }

  #[test]
  fn compile_entries_keep_runtime_roots_without_import_closure() {
    let temp = tempfile::tempdir().expect("tempdir");
    let module_root = temp.path().join("meituan/src/main");
    let runtime_entry = temp
      .path()
      .join("meituan/src/main/ets/abilitystage/EntryAbilityStage.ets");
    let package_entry = temp.path().join("meituan/oh_modules/mtlauncher/Index.ets");
    let package_build_profile = temp
      .path()
      .join("meituan/oh_modules/mtlauncher/BuildProfile.ets");
    let log_entry = temp
      .path()
      .join("meituan/oh_modules/@meituan/log/Index.ets");
    let mut loader_json = test_loader_json(&module_root);
    loader_json.runtime_entry_sources = vec![runtime_entry.to_string_lossy().into_owned()];

    let module_source_files = vec![
      HvigorModuleSourceFile {
        module_id: runtime_entry.to_string_lossy().into_owned(),
        path: temp
          .path()
          .join("cache/meituan/src/main/ets/abilitystage/EntryAbilityStage.ts"),
        record_name: "&meituan/src/main/ets/abilitystage/EntryAbilityStage&".to_string(),
        source_file: "meituan|meituan|1.0.0|src/main/ets/abilitystage/EntryAbilityStage.ts"
          .to_string(),
        package_name: Some("meituan".to_string()),
        is_commonjs: false,
        is_shared_module: false,
        origin_source_lang: "ets".to_string(),
        imported_record_names: vec!["&mtlauncher/Index&12.59.403".to_string()],
      },
      HvigorModuleSourceFile {
        module_id: package_entry.to_string_lossy().into_owned(),
        path: temp.path().join("cache/oh_modules/mtlauncher/Index.ts"),
        record_name: "&mtlauncher/Index&12.59.403".to_string(),
        source_file: "meituan|mtlauncher|12.59.403|Index.ts".to_string(),
        package_name: Some("mtlauncher".to_string()),
        is_commonjs: false,
        is_shared_module: false,
        origin_source_lang: "ets".to_string(),
        imported_record_names: vec![
          "&mtlauncher/BuildProfile&12.59.403".to_string(),
          "&@meituan/log/Index&0.0.18".to_string(),
        ],
      },
      HvigorModuleSourceFile {
        module_id: package_build_profile.to_string_lossy().into_owned(),
        path: temp
          .path()
          .join("cache/oh_modules/mtlauncher/BuildProfile.ts"),
        record_name: "&mtlauncher/BuildProfile&12.59.403".to_string(),
        source_file: "meituan|mtlauncher|12.59.403|BuildProfile.ts".to_string(),
        package_name: Some("mtlauncher".to_string()),
        is_commonjs: false,
        is_shared_module: false,
        origin_source_lang: "ets".to_string(),
        imported_record_names: vec![],
      },
      HvigorModuleSourceFile {
        module_id: log_entry.to_string_lossy().into_owned(),
        path: temp.path().join("cache/oh_modules/@meituan/log/Index.ts"),
        record_name: "&@meituan/log/Index&0.0.18".to_string(),
        source_file: "meituan|@meituan/log|0.0.18|Index.ts".to_string(),
        package_name: Some("@meituan/log".to_string()),
        is_commonjs: false,
        is_shared_module: false,
        origin_source_lang: "ets".to_string(),
        imported_record_names: vec![],
      },
    ];
    let mut pkg_context_info = BTreeMap::new();
    pkg_context_info.insert(
      "mtlauncher".to_string(),
      test_pkg_context_info("mtlauncher", "12.59.403", "Index.ets"),
    );
    let writer = CompileContextInfoWriter::new(
      &loader_json,
      &module_source_files,
      &pkg_context_info,
      "app",
      "com.sankuai.hmeituan",
      false,
    );

    assert_eq!(
      writer.compile_entries(&[]),
      vec!["&meituan/src/main/ets/abilitystage/EntryAbilityStage&".to_string()]
    );
  }

  #[test]
  fn compile_entries_do_not_add_dependency_package_entry_for_represented_package() {
    let temp = tempfile::tempdir().expect("tempdir");
    let module_root = temp.path().join("meituan/src/main");
    let api_entry = temp
      .path()
      .join("meituan/oh_modules/@mrn/mrn/src/main/ets/mapi/apis/getfeedcommentlist_bin.ets");
    let index_entry = temp.path().join("meituan/oh_modules/@mrn/mrn/Index.ets");
    let build_profile_entry = temp
      .path()
      .join("meituan/oh_modules/@mrn/mrn/BuildProfile.ets");
    let config_entry = temp
      .path()
      .join("meituan/oh_modules/@mrn/mrn/src/main/ets/config/MRNConfigCenter.ets");
    let mut loader_json = test_loader_json(&module_root);
    loader_json.compile_entry = vec![api_entry.to_string_lossy().into_owned()];

    let module_source_files = vec![
      HvigorModuleSourceFile {
        module_id: api_entry.to_string_lossy().into_owned(),
        path: temp
          .path()
          .join("cache/oh_modules/@mrn/mrn/src/main/ets/mapi/apis/getfeedcommentlist_bin.ts"),
        record_name: "&@mrn/mrn/src/main/ets/mapi/apis/getfeedcommentlist_bin&12.59.403"
          .to_string(),
        source_file: "meituan|@mrn/mrn|12.59.403|src/main/ets/mapi/apis/getfeedcommentlist_bin.ts"
          .to_string(),
        package_name: Some("@mrn/mrn".to_string()),
        is_commonjs: false,
        is_shared_module: false,
        origin_source_lang: "ets".to_string(),
        imported_record_names: vec![],
      },
      HvigorModuleSourceFile {
        module_id: index_entry.to_string_lossy().into_owned(),
        path: temp.path().join("cache/oh_modules/@mrn/mrn/Index.ts"),
        record_name: "&@mrn/mrn/Index&12.59.403".to_string(),
        source_file: "meituan|@mrn/mrn|12.59.403|Index.ts".to_string(),
        package_name: Some("@mrn/mrn".to_string()),
        is_commonjs: false,
        is_shared_module: false,
        origin_source_lang: "ets".to_string(),
        imported_record_names: vec![
          "&@mrn/mrn/src/main/ets/config/MRNConfigCenter&12.59.403".to_string()
        ],
      },
      HvigorModuleSourceFile {
        module_id: build_profile_entry.to_string_lossy().into_owned(),
        path: temp
          .path()
          .join("cache/oh_modules/@mrn/mrn/BuildProfile.ts"),
        record_name: "&@mrn/mrn/BuildProfile&12.59.403".to_string(),
        source_file: "meituan|@mrn/mrn|12.59.403|BuildProfile.ts".to_string(),
        package_name: Some("@mrn/mrn".to_string()),
        is_commonjs: false,
        is_shared_module: false,
        origin_source_lang: "ets".to_string(),
        imported_record_names: vec![],
      },
      HvigorModuleSourceFile {
        module_id: config_entry.to_string_lossy().into_owned(),
        path: temp
          .path()
          .join("cache/oh_modules/@mrn/mrn/src/main/ets/config/MRNConfigCenter.ts"),
        record_name: "&@mrn/mrn/src/main/ets/config/MRNConfigCenter&12.59.403".to_string(),
        source_file: "meituan|@mrn/mrn|12.59.403|src/main/ets/config/MRNConfigCenter.ts"
          .to_string(),
        package_name: Some("@mrn/mrn".to_string()),
        is_commonjs: false,
        is_shared_module: false,
        origin_source_lang: "ets".to_string(),
        imported_record_names: vec![],
      },
    ];
    let mut pkg_context_info = BTreeMap::new();
    pkg_context_info.insert(
      "@mrn/mrn".to_string(),
      test_pkg_context_info("@mrn/mrn", "12.59.403", "Index.ets"),
    );
    let writer = CompileContextInfoWriter::new(
      &loader_json,
      &module_source_files,
      &pkg_context_info,
      "app",
      "com.sankuai.hmeituan",
      false,
    );

    assert_eq!(
      writer.compile_entries(&[]),
      vec!["&@mrn/mrn/src/main/ets/mapi/apis/getfeedcommentlist_bin&12.59.403".to_string()]
    );
  }

  #[test]
  fn compile_entries_keep_compile_roots_without_import_closure() {
    let temp = tempfile::tempdir().expect("tempdir");
    let module_root = temp.path().join("meituan/src/main");
    let form_entry = temp
      .path()
      .join("meituan/src/main/ets/form/FormAbility.ets");
    let subscribe_entry = temp
      .path()
      .join("oh_modules/@meituan/card_subscribe/index.ets");
    let subscribe_form = temp
      .path()
      .join("oh_modules/@meituan/card_subscribe/src/main/ets/subscribeForm/PTSubscribeForm.ets");
    let mut loader_json = test_loader_json(&module_root);
    loader_json.compile_entry = vec![form_entry.to_string_lossy().into_owned()];

    let module_source_files = vec![
      HvigorModuleSourceFile {
        module_id: form_entry.to_string_lossy().into_owned(),
        path: temp
          .path()
          .join("cache/meituan/src/main/ets/form/FormAbility.ts"),
        record_name: "&meituan/src/main/ets/form/FormAbility&".to_string(),
        source_file: "meituan|meituan|1.0.0|src/main/ets/form/FormAbility.ts".to_string(),
        package_name: Some("meituan".to_string()),
        is_commonjs: false,
        is_shared_module: false,
        origin_source_lang: "ets".to_string(),
        imported_record_names: vec!["&@meituan/card_subscribe/&1.0.32".to_string()],
      },
      HvigorModuleSourceFile {
        module_id: subscribe_entry.to_string_lossy().into_owned(),
        path: temp
          .path()
          .join("cache/oh_modules/@meituan/card_subscribe/index.ts"),
        record_name: "&@meituan/card_subscribe/&1.0.32".to_string(),
        source_file: "meituan|@meituan/card_subscribe|1.0.32|index.ts".to_string(),
        package_name: Some("@meituan/card_subscribe".to_string()),
        is_commonjs: false,
        is_shared_module: false,
        origin_source_lang: "ets".to_string(),
        imported_record_names: vec![
          "&@meituan/card_subscribe/src/main/ets/subscribeForm/PTSubscribeForm&1.0.32".to_string(),
        ],
      },
      HvigorModuleSourceFile {
        module_id: subscribe_form.to_string_lossy().into_owned(),
        path: temp.path().join(
          "cache/oh_modules/@meituan/card_subscribe/src/main/ets/subscribeForm/PTSubscribeForm.ts",
        ),
        record_name: "&@meituan/card_subscribe/src/main/ets/subscribeForm/PTSubscribeForm&1.0.32"
          .to_string(),
        source_file:
          "meituan|@meituan/card_subscribe|1.0.32|src/main/ets/subscribeForm/PTSubscribeForm.ts"
            .to_string(),
        package_name: Some("@meituan/card_subscribe".to_string()),
        is_commonjs: false,
        is_shared_module: false,
        origin_source_lang: "ets".to_string(),
        imported_record_names: vec![],
      },
    ];
    let pkg_context_info = BTreeMap::new();
    let writer = CompileContextInfoWriter::new(
      &loader_json,
      &module_source_files,
      &pkg_context_info,
      "app",
      "com.sankuai.hmeituan",
      false,
    );

    assert_eq!(
      writer.compile_entries(&[]),
      vec!["&meituan/src/main/ets/form/FormAbility&".to_string()]
    );
  }

  #[test]
  fn compile_entries_do_not_include_other_compile_files() {
    let temp = tempfile::tempdir().expect("tempdir");
    let module_root = temp.path().join("meituan/src/main");
    let other_entry = temp
      .path()
      .join("oh_modules/dljson/src/main/ets/internal/DlJson.ts");
    let mut loader_json = test_loader_json(&module_root);
    loader_json.other_compile_files = vec![other_entry.to_string_lossy().into_owned()];
    loader_json.compile_entry = loader_json.other_compile_files.clone();
    let module_source_files = vec![HvigorModuleSourceFile {
      module_id: other_entry.to_string_lossy().into_owned(),
      path: temp
        .path()
        .join("cache/oh_modules/dljson/src/main/ets/internal/DlJson.ts"),
      record_name: "&dljson/src/main/ets/internal/DlJson&0.0.5".to_string(),
      source_file: "meituan|dljson|0.0.5|src/main/ets/internal/DlJson.ts".to_string(),
      package_name: Some("dljson".to_string()),
      is_commonjs: false,
      is_shared_module: false,
      origin_source_lang: "ts".to_string(),
      imported_record_names: vec![],
    }];
    let pkg_context_info = BTreeMap::new();
    let writer = CompileContextInfoWriter::new(
      &loader_json,
      &module_source_files,
      &pkg_context_info,
      "app",
      "com.sankuai.hmeituan",
      false,
    );

    assert_eq!(writer.compile_entries(&[]), Vec::<String>::new());
  }

  #[test]
  fn compile_entries_do_not_include_bytecode_har_entry_records() {
    let temp = tempfile::tempdir().expect("tempdir");
    let module_root = temp.path().join("meituan/src/main");
    let mut loader_json = test_loader_json(&module_root);
    loader_json.byte_code_har_info.insert(
      "@tencent/qq-open-sdk".to_string(),
      arkdown_profile::ByteCodeHarEntry {
        abc_path: temp
          .path()
          .join("oh_modules/@tencent/qq-open-sdk/ets/modules.abc")
          .to_string_lossy()
          .into_owned(),
        compatible_sdk_version: Some(arkdown_profile::CompatibleSdkVersionValue::Int(12)),
        compatible_sdk_version_stage: None,
        source_maps_path: None,
        insight_intent: None,
        declaration_entry: vec![],
      },
    );
    let mut pkg_context_info = BTreeMap::new();
    pkg_context_info.insert(
      "@tencent/qq-open-sdk".to_string(),
      test_pkg_context_info("@tencent/qq-open-sdk", "1.0.4", "Index"),
    );
    let writer = CompileContextInfoWriter::new(
      &loader_json,
      &[],
      &pkg_context_info,
      "app",
      "com.sankuai.hmeituan",
      false,
    );

    assert_eq!(writer.compile_entries(&[]), Vec::<String>::new());
  }

  #[test]
  fn compile_context_info_serializes_hvigor_update_version_shape() {
    let temp = tempfile::tempdir().expect("tempdir");
    let module_root = temp.path().join("meituan/src/main");
    let output_path = temp.path().join("compileContextInfo.json");
    let mut loader_json = test_loader_json(&module_root);
    loader_json
      .update_version_info
      .insert("@tencent/qq-open-sdk".to_string(), serde_json::json!({}));
    let mut pkg_context_info = BTreeMap::new();
    pkg_context_info.insert(
      "@tencent/qq-open-sdk".to_string(),
      test_pkg_context_info("@tencent/qq-open-sdk", "1.0.4", "Index"),
    );

    CompileContextInfoWriter::new(
      &loader_json,
      &[],
      &pkg_context_info,
      "app",
      "com.sankuai.hmeituan",
      false,
    )
    .write(&output_path)
    .expect("write compileContextInfo");

    let value: serde_json::Value =
      serde_json::from_slice(&fs_err::read(&output_path).expect("read compileContextInfo"))
        .expect("parse compileContextInfo");
    assert!(
      value.get("pkgContextInfo").is_none(),
      "hvigor compileContextInfo does not embed pkgContextInfo"
    );
    assert_eq!(
      value["updateVersionInfo"]["@tencent/qq-open-sdk"],
      serde_json::json!({})
    );
  }

  fn test_loader_json(module_root: &Path) -> LoaderJson {
    LoaderJson {
      module_path_map: IndexMap::new(),
      compile_mode: "esmodule".to_string(),
      project_root_path: "/project".to_string(),
      node_modules_path: "/project/build/default/intermediates/loader_out/default/node_modules"
        .to_string(),
      byte_code_har_info: IndexMap::new(),
      declaration_entry: vec![],
      module_name: "meituan".to_string(),
      hsp_name_ohm_map: IndexMap::new(),
      har_name_ohm_map: IndexMap::new(),
      package_manager_type: "ohpm".to_string(),
      hvigor_compile_entry: vec![],
      other_compile_files: vec![],
      dynamic_import_lib_info: IndexMap::new(),
      router_map: vec![],
      hsp_resources_map: FxHashMap::default(),
      update_version_info: IndexMap::new(),
      customized_har: false,
      byte_code_har: None,
      pkg_name2_other_compile_files: None,
      an_build_out_put: None,
      an_build_mode: None,
      ap_path: None,
      build_config_path: None,
      patch_config: None,
      non_entry_importees: vec![],
      compiled_importees: vec![],
      compile_entry: vec![],
      form_entry_sources: vec![],
      runtime_entry_sources: vec![],
      module_root_path: module_root.to_string_lossy().into_owned(),
      cache_path: String::new(),
      ace_module_build: String::new(),
      build_mode: "debug".to_string(),
      support_chunks: false,
      use_normalized_ohm_url: true,
      api_version: 12,
      fallback_an_build: false,
      pkg_name_to_pkg_brief_info: FxHashMap::default(),
      workers: vec![],
      source_roots: vec![],
      bundle_dependencies: vec![],
      global_dependencies: vec![],
      compiler_types: vec![],
      compiler_type_declaration_inputs: vec![],
      dependency_pkg_version: vec![],
      extra_importees: vec![],
      compiled_package_names: vec![],
      oh_exports: vec![],
    }
  }

  fn test_pkg_context_info(
    package_name: &str,
    version: &str,
    entry_path: &str,
  ) -> PkgContextInfoOpt {
    PkgContextInfoOpt {
      package_name: package_name.to_string(),
      bundle_name: String::new(),
      module_name: String::new(),
      version: version.to_string(),
      entry_path: entry_path.to_string(),
      is_so: false,
      dependency_alias: String::new(),
      oh_exports: None,
    }
  }
}

// ---------------------------------------------------------------------------
// Cache group snapshot builder
// ---------------------------------------------------------------------------

/// Compute deterministic cache group snapshots for the current module+config.
///
/// Each group corresponds to one `this.cache.set(key, hash)` call in hvigor's
/// `invalid-cache-plugin.ts`.  When any group's hash changes between builds,
/// affected modules are marked for recompilation.
///
/// # Upstream correspondence
///
/// Mirrors `invalid-cache-plugin.ts::buildEnd` + `beforeBuildEnd`.
fn build_cache_group_snapshots(
  compile_env: &str,
  config: &ArkdownConfig,
  module: &ModuleConfig,
  loader_json: &LoaderJson,
  package_plugins: &[PackagePlugin],
  project_root: &Path,
  byte_code_har_integrated_optimization: bool,
) -> Vec<CacheGroupSnapshot> {
  let module_layout = ModuleBuildLayout::for_output_target(&config.output_target, &module.src_path, "default");

  // Helper: FNV-1a hash over file content; returns CacheGroupSnapshot.
  let file_hash = |path: &PathBuf, key: CacheGroupKey| -> CacheGroupSnapshot {
    match std::fs::read(path) {
      Ok(bytes) => {
        let h = arkdown_cache::CacheKeyMaterial::hash_u64(&String::from_utf8_lossy(&bytes));
        CacheGroupSnapshot::from_u64(key, h)
      }
      Err(_) => CacheGroupSnapshot::from_value(key, ""),
    }
  };

  // 1. CompileEnv — the 7-field key string already computed by caller.
  let compile_env_snap = CacheGroupSnapshot::from_value(CacheGroupKey::CompileEnv, compile_env);

  // 2. ProjectConfig — hash of module.json5 + build-profile.json5 bytes.
  let project_config_hash = {
    let mut h: u64 = 0xcbf29ce484222325;
    let module_json5_path = module_layout.source().module_json5();
    for path in &[module_json5_path, project_root.join("build-profile.json5")] {
      if let Ok(bytes) = std::fs::read(path) {
        for b in &bytes {
          h ^= u64::from(*b);
          h = h.wrapping_mul(0x100000001b3);
        }
      }
    }
    h
  };
  let project_config_snap =
    CacheGroupSnapshot::from_u64(CacheGroupKey::ProjectConfig, project_config_hash);

  // 3. PackageHash — oh-package.json5 content hash.
  let package_hash_snap = file_hash(
    &module_layout.oh_package_json5(),
    CacheGroupKey::PackageHash,
  );

  // 4. HarNameOhmMap — sorted serialization of HAR name → OHM URL map.
  let har_name_ohm_map_str = {
    let mut pairs: Vec<_> = loader_json
      .har_name_ohm_map
      .iter()
      .map(|(k, v)| format!("{}={}", k, v))
      .collect();
    pairs.sort();
    pairs.join("|")
  };
  let har_name_ohm_map_snap =
    CacheGroupSnapshot::from_value(CacheGroupKey::HarNameOhmMap, &har_name_ohm_map_str);

  // 5. RouterMap — hash of the compiled router map loader file.
  let router_map_snap = file_hash(&module_layout.router_map_loader(), CacheGroupKey::RouterMap);

  // 6. PreloadSo — explicit preload paths plus the system-SO collection switch.
  let preload_so_str = {
    let mut paths: Vec<String> = config
      .product
      .build_option
      .preload_so
      .iter()
      .map(|path| {
        ArtifactPathSerializer::path_to_slash_string(path)
          .unwrap_or_else(|_| path.display().to_string())
      })
      .collect();
    paths.sort();
    format!(
      "preloadSystemSo={}|{}",
      config.product.build_option.preload_system_so,
      paths.join("|")
    )
  };
  let preload_so_snap = CacheGroupSnapshot::from_value(CacheGroupKey::PreloadSo, &preload_so_str);

  // 7. PluginIdentity — concatenated cache identities of all package plugins.
  let plugin_identity = package_plugins
    .iter()
    .map(|p| p.cache_identity())
    .collect::<Vec<_>>()
    .join("|");
  let plugin_identity_snap =
    CacheGroupSnapshot::from_value(CacheGroupKey::PluginIdentity, &plugin_identity);

  // 8. BundleName — from app.json5 (already resolved in config).
  let bundle_name_snap =
    CacheGroupSnapshot::from_value(CacheGroupKey::BundleName, &config.product.bundle_name);

  // 9. ByteCodeHarOptimize — exact boolean property consumed by latest
  // InjectUtil.isByteCodeHarOptimize().
  let byte_code_har_optimize_snap = CacheGroupSnapshot::from_bool(
    CacheGroupKey::ByteCodeHarOptimize,
    byte_code_har_integrated_optimization,
  );

  // 10. MockConfig — hash of src/mock/mock-config.json5 if it exists.
  let mock_config_path = module
    .src_path
    .join("src")
    .join("mock")
    .join("mock-config.json5");
  let mock_config_snap = file_hash(&mock_config_path, CacheGroupKey::MockConfig);

  vec![
    compile_env_snap,
    project_config_snap,
    package_hash_snap,
    har_name_ohm_map_snap,
    router_map_snap,
    preload_so_snap,
    plugin_identity_snap,
    bundle_name_snap,
    byte_code_har_optimize_snap,
    mock_config_snap,
  ]
}

// ---------------------------------------------------------------------------
// Share context serialization
// ---------------------------------------------------------------------------

/// Build the JSON share context passed to JS plugin lifecycle hooks.
///
/// Returns `{ share: { bundleName, buildMode, target, moduleName? } }` —
/// the subset of hvigor's `projectConfig` that is available at the
/// coordinator level.
///
/// Plugins can access this as `this.share` inside `beforeBuild`,
/// `afterBuildStart`, `buildStart`, etc., matching hvigor's
/// `getCompilePluginContext(this).share` shape.
fn build_share_context_json(
  config: &ArkdownConfig,
  args: &BuildArgs,
  module: Option<&ModuleConfig>,
) -> String {
  let build_mode = match config.build_mode {
    BuildMode::Debug => "debug",
    BuildMode::Release => "release",
  };
  let target = args.target.as_str();
  serde_json::json!({
      "share": {
          "projectConfig": {
              "bundleName": config.product.bundle_name,
              "buildMode": build_mode,
              "target": target,
              "useNormalizedOHMUrl":
                  config.product.build_option.strict_mode.use_normalized_ohm_url,
              "compileSdkVersion": config.sdk_config.compile_sdk_version,
              "compatibleSdkVersion": config.product.compatible_sdk_version,
              "packageManagerType": "ohpm",
              "isByteCodeHarOptimize": args.byte_code_har_integrated_optimization,
              "moduleName": module.map(|module| module.name.as_str()),
              "modulePath": module.map(|module| module.src_path.as_path()),
              "projectTopDir": args.project.as_deref(),
          }
      }
  })
  .to_string()
}

struct PluginCacheInvalidationRunner<'a> {
  plugins: &'a [PackagePlugin],
  cache_dir: &'a Path,
}

impl<'a> PluginCacheInvalidationRunner<'a> {
  fn new(plugins: &'a [PackagePlugin], cache_dir: &'a Path) -> Self {
    Self { plugins, cache_dir }
  }

  async fn should_invalidate(&self) -> Result<bool> {
    let cache_plugins = self
      .plugins
      .iter()
      .filter(|plugin| plugin.has_should_invalid_cache())
      .cloned()
      .collect::<Vec<_>>();
    if cache_plugins.is_empty() {
      return Ok(false);
    }

    let inventory = match PluginCacheModuleInventory::load(self.cache_dir) {
      Ok(Some(inventory)) if !inventory.is_empty() => inventory,
      Ok(_) => {
        tracing::info!(
          cache_dir = %self.cache_dir.display(),
          "plugin module cache is absent; rebuilding before shouldInvalidCache can run"
        );
        return Ok(true);
      }
      Err(error) => {
        tracing::warn!(
          cache_dir = %self.cache_dir.display(),
          error = %error,
          "plugin module cache is unreadable; rebuilding"
        );
        return Ok(true);
      }
    };

    for record in inventory.modules() {
      let module = match record.read(self.cache_dir) {
        Ok(module) => Arc::new(module),
        Err(error) => {
          tracing::warn!(
            cache_dir = %self.cache_dir.display(),
            error = %error,
            "cached transformed module is unreadable; rebuilding"
          );
          return Ok(true);
        }
      };
      // Latest invalid-cache-plugin.js evaluates every custom plugin for the
      // current cached module with Promise.all. JoinSet preserves that parallel
      // boundary while keeping modules themselves in deterministic ID order.
      let mut tasks = tokio::task::JoinSet::new();
      for plugin in cache_plugins.iter().cloned() {
        let module = Arc::clone(&module);
        tasks.spawn(async move { plugin.should_invalid_cache(module.as_ref()).await });
      }
      let mut should_invalidate = false;
      while let Some(result) = tasks.join_next().await {
        should_invalidate |=
          result.map_err(|error| anyhow::anyhow!("shouldInvalidCache task failed: {error}"))??;
      }
      if should_invalidate {
        return Ok(true);
      }
    }
    Ok(false)
  }
}

// ---------------------------------------------------------------------------

/// The build coordinator.
///
/// Owns the explicit coarse sequencing of all build stages for a single
/// `arkdown build` invocation.  Constructed from a parsed [`BuildSession`]
/// and the CLI [`BuildArgs`]; call [`BuildCoordinator::execute`] to run.
pub struct BuildCoordinator {
  session: Arc<BuildSession>,
  config: Arc<ArkdownConfig>,
  args: BuildArgs,
  package_plugins: Vec<PackagePlugin>,
  task_observer: Option<Arc<dyn BuildTaskObserver>>,
  owns_task_cycle: bool,
}

struct TargetBuildExecutionPlan {
  target_names: Vec<String>,
}

impl TargetBuildExecutionPlan {
  fn new(
    config: &ArkdownConfig,
    build_options: &BuildOptionProfileInventory,
    args: &BuildArgs,
  ) -> Self {
    if let Some(target_name) = args.build_target_name.as_ref() {
      return Self {
        target_names: vec![target_name.clone()],
      };
    }
    let build_mode_name = match config.build_mode {
      BuildMode::Debug => "debug",
      BuildMode::Release => "release",
    };
    let mut target_names = Vec::new();
    for module in config
      .modules
      .iter()
      .filter(|module| args.module.as_ref().is_none_or(|name| name == &module.name))
    {
      for target in
        build_options.applicable_targets(&module.name, &config.product.name, build_mode_name)
      {
        if !target_names.contains(&target.name) {
          target_names.push(target.name);
        }
      }
    }
    Self { target_names }
  }
}

struct ModuleTargetSelection {
  target: EffectiveTargetConfig,
}

impl ModuleTargetSelection {
  fn resolve(
    config: &ArkdownConfig,
    build_options: &BuildOptionProfileInventory,
    module: &ModuleConfig,
    requested_target: Option<&str>,
  ) -> Self {
    let build_mode_name = match config.build_mode {
      BuildMode::Debug => "debug",
      BuildMode::Release => "release",
    };
    if let Some(requested_target) = requested_target {
      let requested = build_options
        .applicable_targets(&module.name, &config.product.name, build_mode_name)
        .into_iter()
        .find(|target| target.name == requested_target);
      if let Some(target) = requested {
        return Self { target };
      }
      // The `ohosTest` target is created dynamically (hvigor accepts
      // `-p module=entry@ohosTest` without a build-profile entry as long as
      // `src/ohosTest/module.json5` exists).
      if requested_target == "ohosTest"
        && module.src_path.join("src/ohosTest/module.json5").is_file()
      {
        return Self {
          target: EffectiveTargetConfig {
            name: "ohosTest".to_string(),
            ..EffectiveTargetConfig::default()
          },
        };
      }
      tracing::debug!(
        module = %module.name,
        requested_target,
        "requested target does not apply to this module; using its effective target"
      );
    }
    let target = build_options
      .effective_target(&module.name, &config.product.name, build_mode_name)
      .unwrap_or_else(|| EffectiveTargetConfig {
        name: "default".to_string(),
        ..EffectiveTargetConfig::default()
      });
    Self { target }
  }
}

struct ModuleBuildConfiguration {
  target_name: String,
  target: EffectiveTargetConfig,
  config: Arc<ArkdownConfig>,
}

impl ModuleBuildConfiguration {
  fn resolve(
    config: &ArkdownConfig,
    build_options: &BuildOptionProfileInventory,
    module: &ModuleConfig,
    requested_target: Option<&str>,
    build_script_override: Option<&serde_json::Value>,
  ) -> Result<Self> {
    let target =
      ModuleTargetSelection::resolve(config, build_options, module, requested_target).target;
    let target_name = target.name.clone();
    let mut resolver = EffectiveModuleBuildOptionResolver::new(
      build_options,
      &module.name,
      &config.product.name,
      match config.build_mode {
        BuildMode::Debug => "debug",
        BuildMode::Release => "release",
      },
      &target_name,
    );
    if let Some(build_script_override) = build_script_override {
      resolver = resolver.with_build_script_override(build_script_override.clone());
    }
    let build_option = resolver.resolve().with_context(|| {
      format!(
        "resolve effective build option for module '{}' target '{}'",
        module.name, target_name
      )
    })?;
    let mut resolved = config.clone();
    resolved.product.build_option = build_option;
    Ok(Self {
      target_name,
      target,
      config: Arc::new(resolved),
    })
  }
}

struct BuildTimingProbe<'a> {
  stage: &'static str,
  module: Option<&'a str>,
  start: Instant,
}

impl<'a> BuildTimingProbe<'a> {
  fn global(stage: &'static str) -> Self {
    Self {
      stage,
      module: None,
      start: Instant::now(),
    }
  }

  fn module(module: &'a str, stage: &'static str) -> Self {
    Self {
      stage,
      module: Some(module),
      start: Instant::now(),
    }
  }
}

impl Drop for BuildTimingProbe<'_> {
  fn drop(&mut self) {
    let elapsed_ms = self.start.elapsed().as_millis();
    if let Some(module) = self.module {
      tracing::debug!(
          module = %module,
          stage = self.stage,
          elapsed_ms,
          "profile substage completed"
      );
    } else {
      tracing::debug!(stage = self.stage, elapsed_ms, "profile substage completed");
    }
  }
}

impl BuildCoordinator {
  /// `isFormWidgetModule`: an HSP whose `module.json5` declares
  /// `formExtensionModule` (hvigor `module-model-impl.ts`).
  fn module_json5_has_form_extension_module(module_root: &Path) -> bool {
    let module_json_path = module_root.join("src/main/module.json5");
    std::fs::read_to_string(&module_json_path)
      .ok()
      .and_then(|content| json5::from_str::<arkdown_core::ModuleJson5>(&content).ok())
      .is_some_and(|json| json.module.form_extension_module.is_some())
  }

  /// Construct a new coordinator from a completed build session and CLI args.
  pub fn new(session: Arc<BuildSession>, config: Arc<ArkdownConfig>, args: BuildArgs) -> Self {
    Self {
      session,
      config,
      args,
      package_plugins: vec![],
      task_observer: None,
      owns_task_cycle: true,
    }
  }

  /// Attach package-supplied Rolldown plugins collected by the Node.js API.
  pub fn with_package_plugins(mut self, package_plugins: Vec<PackagePlugin>) -> Self {
    self.package_plugins = package_plugins;
    self
  }

  /// Attach the same-process task observer used by the Node hvigor runtime.
  pub fn with_task_observer(mut self, task_observer: Arc<dyn BuildTaskObserver>) -> Self {
    self.task_observer = Some(task_observer);
    self
  }

  /// Execute inside a cycle owned by a project-level coordinator.
  ///
  /// `assembleApp` builds HSP/HAP children and then runs project tasks as one
  /// hvigor graph. Suppressing child cycle markers keeps TypeScript custom
  /// task completion and incremental snapshots alive across those stages.
  pub(super) fn within_task_cycle(mut self) -> Self {
    self.owns_task_cycle = false;
    self
  }

  /// Execute the full build pipeline and return the produced artifacts.
  ///
  pub async fn execute(self) -> Result<BuildOutput> {
    let observer = self.task_observer.clone();
    let owns_task_cycle = self.owns_task_cycle;
    match self.execute_targets().await {
      Ok(output) => Ok(output),
      Err(error) if owns_task_cycle && error.is::<NativeTaskCutoffReached>() => {
        BuildTaskLifecycle::new(observer, Vec::new())
          .finish_cycle()
          .await?;
        Ok(BuildOutput {
          artifact_paths: Vec::new(),
        })
      }
      Err(error) => Err(error),
    }
  }

  async fn execute_targets(self) -> Result<BuildOutput> {
    let target_plan = TargetBuildExecutionPlan::new(
      self.config.as_ref(),
      self.session.build_options.as_ref(),
      &self.args,
    );
    if target_plan.target_names.len() <= 1 || self.args.build_target_name.is_some() {
      return self.execute_impl().await;
    }

    tracing::info!(
      targets = ?target_plan.target_names,
      "multi-target: executing the complete build pipeline once per target"
    );
    let mut artifact_paths = Vec::new();
    for target_name in target_plan.target_names {
      let mut args = self.args.clone();
      args.build_target_name = Some(target_name.clone());
      tracing::info!(target = %target_name, "multi-target pipeline started");
      let output = Self {
        session: self.session.clone(),
        config: self.config.clone(),
        args,
        package_plugins: self.package_plugins.clone(),
        task_observer: self.task_observer.clone(),
        owns_task_cycle: self.owns_task_cycle,
      }
      .execute_impl()
      .await?;
      artifact_paths.extend(output.artifact_paths);
    }
    Ok(BuildOutput { artifact_paths })
  }

  /// Internal build pipeline implementation.
  ///
  /// Called by [`BuildCoordinator::execute`], which wraps it to guarantee
  /// unconditional `cleanUp` hook invocation.
  async fn execute_impl(self) -> Result<BuildOutput> {
    let start = Instant::now();

    let args = &self.args;
    let config = &self.config;
    let session = &self.session;
    let package_plugins = &self.package_plugins;
    let mut task_lifecycle =
      BuildTaskLifecycle::new(self.task_observer.clone(), args.native_task_roots.clone());
    if self.owns_task_cycle {
      task_lifecycle.begin_cycle().await?;
    }

    let watch_mode = args.watch || args.preview;
    let is_incremental_watch = watch_mode || args.watch_invalidation.is_some();
    let watch_changed_files = args
      .watch_invalidation
      .as_ref()
      .map(|batch| batch.changed_files.clone())
      .unwrap_or_default();
    let preview_mode = args.preview;
    // Local unit-test builds default to code-coverage instrumentation:
    // hvigor `setTestDefaultParameters` injects `ohos-test-coverage=true`
    // for the `test` task; the CLI flag can still override it.
    let coverage_mode = args.coverage
      || (matches!(config.output_target, OutputTarget::Test)
        && args.ohos_test_coverage.unwrap_or(true));
    let disable_abc_cache = args.disable_abc_cache || !args.cache;
    let cli_cache_file = args.cache_file.clone();
    let abc_backend = args.abc_backend;
    let named_target = args.build_target_name.clone();
    let module_analyzer = ModuleAnalyzerKind::from(args.module_analyzer);

    let checker_selection = ArkTsCheckerSelection::parse(args.checker.as_deref())?;
    let checker_mode = checker_selection.oxc_mode();

    let project_root = args
      .project
      .clone()
      .unwrap_or_else(|| std::env::current_dir().expect("failed to get current directory"));
    let project_layout = ProjectBuildLayout::new(&project_root);

    if let Some(ref named) = named_target {
      tracing::info!(
          target = %named,
          "multi-target: building with named target configuration '{}'",
          named
      );
    }

    let progress = Progress::new();

    let custom_cfg = ArkdownCustomConfigLoader::new(&project_root).load();
    let intermediate_writer = IntermediateArtifactWriter::new(&config.sdk_config.restool_path);

    let build_modules =
      ModuleSelectionPlanner::build_modules_for_target(config.as_ref(), args.module.as_deref())?;
    let build_module_configs = build_modules
      .iter()
      .map(|module| {
        ModuleBuildConfiguration::resolve(
          config.as_ref(),
          session.build_options.as_ref(),
          module,
          named_target.as_deref(),
          args.module_hvigor_overrides.get(&module.name),
        )
      })
      .collect::<Result<Vec<_>>>()?;
    let mut har_dedup_config = config.as_ref().clone();
    har_dedup_config.product.build_option.deduplicate_har = build_module_configs
      .iter()
      .any(|resolved| resolved.config.product.build_option.deduplicate_har);
    let har_dedup_project_plan =
      HarDedupProjectPlan::collect(&har_dedup_config, session.project.as_ref())?;
    #[cfg(feature = "memory-diagnostics")]
    arkdown::memory_diagnostics::dump_with(
      "build:start",
      format_args!(
        "modules={} target={:?} mode={}",
        build_modules.len(),
        config.output_target,
        args.mode
      ),
    );

    if coverage_mode {
      tracing::info!(
        "coverage mode enabled — instrumenting executable statements before ABC compilation"
      );
    }
    if args.ohos_debug_asan && args.ohos_enable_hwasan {
      anyhow::bail!(
        "ohos-debug-asan and ohos-enable-hwasan are mutually exclusive sanitizer modes"
      );
    }
    if args.ohos_debug_asan {
      tracing::info!("ohos-debug-asan enabled — module.json app.asanEnabled=true");
    }
    if args.ohos_enable_hwasan {
      tracing::info!("ohos-enable-hwasan enabled — module.json app.hwasanEnabled=true");
    }
    let native_sdk_toolchain = NativeSdkToolchain::new(
      session.sdk.sdk_root.clone(),
      session
        .sdk
        .tool_path(SdkToolKind::Cmake)
        .map(|tool| tool.path),
      session
        .sdk
        .tool_path(SdkToolKind::NativeNinja)
        .map(|tool| tool.path),
      session
        .sdk
        .tool_path(SdkToolKind::LlvmStrip)
        .map(|tool| tool.path),
    );
    let sanitizer_native_policy = match (args.ohos_debug_asan, args.ohos_enable_hwasan) {
      (true, false) => Some(
        NativeLibCollectionPolicy::asan()
          .with_build_mode(args.mode.clone())
          .with_sdk_toolchain(native_sdk_toolchain.clone()),
      ),
      (false, true) => Some(
        NativeLibCollectionPolicy::hwasan(
          session
            .sdk
            .tool_path(SdkToolKind::LlvmReadElf)
            .map(|tool| tool.path),
        )
        .with_build_mode(args.mode.clone())
        .with_sdk_toolchain(native_sdk_toolchain.clone()),
      ),
      _ => None,
    };

    // ── Step 2: Profile + Resource (per module) ───────────────────────
    progress.step(2, 6, "Processing profiles");

    let sanitizer_profile_flags =
      SanitizerProfileFlags::new(args.ohos_debug_asan, args.ohos_enable_hwasan);
    let permission_validator = arkdown_profile::PermissionValidator::new();
    let privacy_profile_processor = arkdown_profile::PrivacyProfileProcessor::new();
    let process_profile_processor = arkdown_profile::ProcessProfileProcessor::new();
    let permission_reason_processor = arkdown_profile::PermissionReasonProcessor::new();
    let share_config_processor = arkdown_profile::ShareConfigProcessor::new();
    let startup_config_processor = arkdown_profile::StartupConfigProcessor::new();
    let distro_filter_processor = arkdown_profile::DistroFilterProcessor::new();

    let mut loader_jsons = Vec::new();
    let mut all_bc_har_deps: Vec<Vec<ByteCodeHarScanEntry>> = Vec::new();
    let mut privacy_profile_paths: Vec<Option<PathBuf>> = Vec::new();

    let sdk_permissions = {
      let _timer = BuildTimingProbe::global("sdk_permission_data");
      if BuildPolicy::new(config.as_ref()).uses_sdk_permission_validation() {
        if let Some(module_json_path) = config.sdk_config.permission_module_json_path() {
          permission_validator
            .collect_sdk_permission_data(&module_json_path)
            .map_err(|e| {
              anyhow::anyhow!(
                "collect SDK permission data failed from {}: {}",
                module_json_path.display(),
                e
              )
            })?
        } else {
          None
        }
      } else {
        None
      }
    };

    let app_scope_bundle_name = session.project.bundle_name.clone();
    let pac_schema_path = config
      .sdk_config
      .pac_schema_path_for_runtime(&config.product.runtime_os);
    // Match ets-loader's `globalModulePaths`: HarmonyOS component SDKs add
    // declaration roots through `ets/sdkConfig.json`. The same snapshot must
    // feed both semantic metadata and the transform source store so SDK
    // `@Builder` functions are classified before ArkUI lowering.
    let sdk_declaration_roots = config
      .sdk_config
      .arkguard_api_dirs(&config.product.runtime_os);
    let component_metadata = {
      let _timer = BuildTimingProbe::global("component_metadata");
      ComponentMetadata::from_ets_loader_path_with_external_declaration_roots(
        &config.sdk_config.ets_loader_path,
        &sdk_declaration_roots,
      )
      .map_err(|e| {
        anyhow::anyhow!(
          "SDK component metadata load failed from {}: {}",
          config.sdk_config.ets_loader_path.display(),
          e
        )
      })?
    };

    for (module, resolved_module) in build_modules.into_iter().zip(build_module_configs) {
      let _module_timer = BuildTimingProbe::module(&module.name, "module_total");
      let module_target_name = resolved_module.target_name;
      let module_target = resolved_module.target;
      let module_config = resolved_module.config;
      let module_build_option = &module_config.product.build_option;
      let processor = ProfileProcessor::new(module_config.clone());
      // Local unit-test builds (`--target test`) use the `.test` build root
      // (hvigor `setTestDefaultParameters` injects `buildRoot=.test`);
      // named targets (e.g. `ohosTest`) key intermediate paths by target.
      let module_layout =
        ModuleBuildLayout::for_output_target(&config.output_target, &module.src_path, &module_target_name);
      let module_context = session.module_context(&module.name).ok_or_else(|| {
        anyhow::anyhow!("missing build session module context for {}", module.name)
      })?;
      let module_har_dedup_plan = har_dedup_project_plan.module(&module.name);
      let skip_profile_writes = args.skip_profile;

      if task_lifecycle.is_observed() && task_lifecycle.before(&module.name, None, "clean").await? {
        HvigorModuleCleanStage::new(&project_root, &module.src_path, &native_sdk_toolchain)
          .with_native_variants(
            vec![config.product.name.clone()],
            vec![module_target_name.clone()],
            match config.build_mode {
              BuildMode::Debug => "debug",
              BuildMode::Release => "release",
            },
          )
          .execute()
          .map_err(|error| anyhow::anyhow!("clean failed: {error}"))?;
        task_lifecycle.after(&module.name, None, "clean").await?;
      }
      task_lifecycle
        .marker(&module.name, Some(&module_target_name), "PreBuild")
        .await?;
      // hvigor `GenerateUnitTestTemplate` writes the three local unit-test
      // templates into `<module>/.test/testability/` (declared output). It
      // runs for the `test` command entry (`hvigorw test`).
      if matches!(config.output_target, OutputTarget::Test) {
        let _timer = BuildTimingProbe::module(&module.name, "generate_unit_test_template");
        let testability = module_layout.testability_dir();
        if !testability.join("TestAbility.ets").is_file() {
          let coverage = args.ohos_test_coverage.unwrap_or(true);
          let scope = args.scope.as_deref().unwrap_or("");
          // hvigor embeds the absolute module path (`pathInfo.getModulePath()`)
          // into the template; canonicalize so `--project .` does not leak
          // `./` segments into the generated file.
          let module_path = std::fs::canonicalize(&module.src_path)
            .unwrap_or_else(|_| config.project_root.join(&module.src_path));
          super::unit_test::generate_unit_test_template(
            &module_path,
            &super::unit_test::UnitTestTemplateParams {
              bundle_name: &config.product.bundle_name,
              module_name: &module.name,
              scope,
              coverage,
              module_path: &module_path,
            },
          )
          .map_err(|error| {
            anyhow::anyhow!("GenerateUnitTestTemplate failed for module '{}': {error}", module.name)
          })?;
        }
        task_lifecycle
          .after(&module.name, Some(&module_target_name), "GenerateUnitTestTemplate")
          .await?;
      }
      // hvigor `GenerateOhosTestTemplate`: the `ohosTest` target build
      // (`-p module=entry@ohosTest`) copies the SDK test templates into
      // `.test/<product>/intermediates/src/ohosTest/`.
      if module_target_name == "ohosTest"
        && module.src_path.join("src/ohosTest/module.json5").is_file()
      {
        let _timer = BuildTimingProbe::module(&module.name, "generate_ohos_test_template");
        if !super::unit_test::has_ohos_test_template(&module.src_path) {
          super::unit_test::generate_ohos_test_template(
            &module.src_path,
            config.sdk_config.compile_api_version,
          )
          .map_err(|error| {
            anyhow::anyhow!(
              "GenerateOhosTestTemplate failed for module '{}': {error}",
              module.name
            )
          })?;
        }
        task_lifecycle
          .after(&module.name, Some(&module_target_name), "GenerateOhosTestTemplate")
          .await?;
      }
      if !skip_profile_writes {
        // hvigor `pre-build.ts` validation: schema checks against the SDK
        // modulecheck schemas plus structural/path/API-version constraints.
        let pre_build_validator = PreBuildValidator::new(
          config,
          session,
          &module,
          &module_context,
          module_build_option,
        );
        pre_build_validator
          .validate()
          .map_err(|error| anyhow::anyhow!("pre-build validation failed: {error}"))?;
      }
      let module_info_task_name = if module.module_type == OhosModuleType::Har {
        "CreateHarModuleInfo"
      } else {
        "CreateModuleInfo"
      };
      if skip_profile_writes {
        task_lifecycle
          .up_to_date(
            &module.name,
            Some(&module_target_name),
            module_info_task_name,
          )
          .await?;
      } else if task_lifecycle
        .before(
          &module.name,
          Some(&module_target_name),
          module_info_task_name,
        )
        .await?
      {
        let is_bytecode_har = module.module_type == OhosModuleType::Har
          && module_build_option.har_artifact_type == arkdown_core::HarArtifactType::Bytecode;
        let is_integrated_hsp = module.module_type == OhosModuleType::Shared
          && module_build_option
            .ark_options
            .as_ref()
            .and_then(|options| options.integrated_hsp)
            .unwrap_or(false);
        let should_write_module_info = is_bytecode_har
          || args.ui_transform_optimization
          || (is_integrated_hsp && args.integrated_hsp_use_get_current_bundle_name);
        intermediate_writer.write_module_info(
          &module.src_path,
          &module_target_name,
          should_write_module_info,
        )?;
        task_lifecycle
          .after(
            &module.name,
            Some(&module_target_name),
            module_info_task_name,
          )
          .await?;
      }
      task_lifecycle
        .marker(&module.name, Some(&module_target_name), "GenerateMetadata")
        .await?;
      task_lifecycle
        .marker(&module.name, Some(&module_target_name), "ConfigureCmake")
        .await?;
      if task_lifecycle.has_root(&module.name, None, "compileNative") {
        let native_build_option =
          arkdown_config::build_profile::EffectiveNativeBuildOption::from_build_option(
            module_build_option,
          );
        let self_package = module_context
          .module
          .package
          .as_ref()
          .map(|package| NativeLibPackageInfo::local(package.name.clone(), package.version.clone()))
          .unwrap_or_else(|| NativeLibPackageInfo::local(module.name.clone(), String::new()));
        let native_policy = sanitizer_native_policy
          .clone()
          .unwrap_or_else(|| {
            NativeLibCollectionPolicy::default()
              .with_build_mode(args.mode.clone())
              .with_sdk_toolchain(native_sdk_toolchain.clone())
          })
          .with_external_native_options(native_build_option.external_native_options.clone())
          .with_native_lib_option(native_build_option.native_lib.clone())
          .with_self_package(self_package);
        let native_collector = NativeLibCollector::new(
          module,
          &native_build_option.abi_filters,
          module_context.deps.as_ref(),
        )
        .with_policy(native_policy);
        let native_collector = if matches!(config.output_target, OutputTarget::Har) {
          native_collector
        } else {
          native_collector
            .excluding_dependency_roots(module_har_dedup_plan.duplicate_package_roots())
        };
        if task_lifecycle
          .before(
            &module.name,
            Some(&module_target_name),
            "BuildNativeWithCmake",
          )
          .await?
        {
          native_collector
            .configure_native()
            .map_err(|error| anyhow::anyhow!("BuildNativeWithCmake failed: {error}"))?;
          task_lifecycle
            .after(
              &module.name,
              Some(&module_target_name),
              "BuildNativeWithCmake",
            )
            .await?;
        }
        task_lifecycle
          .marker(&module.name, None, "compileNative")
          .await?;
      }
      task_lifecycle
        .marker(&module.name, Some(&module_target_name), "MergeProfile")
        .await?;
      let build_profile_task_name = if module.module_type == OhosModuleType::Har {
        "CreateHarBuildProfile"
      } else {
        "CreateBuildProfile"
      };
      if skip_profile_writes {
        task_lifecycle
          .up_to_date(
            &module.name,
            Some(&module_target_name),
            build_profile_task_name,
          )
          .await?;
      } else if task_lifecycle
        .before(
          &module.name,
          Some(&module_target_name),
          build_profile_task_name,
        )
        .await?
      {
        let _timer = BuildTimingProbe::module(&module.name, "build_profile");
        let build_profile_path =
          module_layout.build_profile_for_module(&module.module_type, &module_target_name);
        let module_profile_path = module.src_path.join("build-profile.json5");
        let empty_fields = serde_json::Map::new();
        let fields = module_build_option
          .ark_options
          .as_ref()
          .map(|options| &options.build_profile_fields)
          .unwrap_or(&empty_fields);
        let kind = if module.module_type == OhosModuleType::Har {
          let version = module_context
            .module
            .package
            .as_deref()
            .map(|package| package.version.as_str())
            .ok_or_else(|| {
              anyhow::anyhow!(
                "HAR module '{}' requires oh-package.json5 for BuildProfile.ets generation",
                module.name
              )
            })?;
          BuildProfileKind::Har { version }
        } else {
          BuildProfileKind::Application {
            bundle_name: &session.project.bundle_name,
            bundle_type: &session.project.bundle_type,
            version_code: session.project.version_code,
            version_name: &session.project.version_name,
            product_name: &config.product.name,
            build_version: session.project.build_version.as_deref(),
          }
        };
        BuildProfileWriter::new()
          .write(BuildProfileInput {
            output_path: &build_profile_path,
            profile_path: &module_profile_path,
            kind,
            target_name: &module_target_name,
            build_mode: &config.build_mode,
            fields,
          })
          .map_err(|error| anyhow::anyhow!("CreateBuildProfile failed: {error}"))?;
        task_lifecycle
          .after(
            &module.name,
            Some(&module_target_name),
            build_profile_task_name,
          )
          .await?;
      }
      task_lifecycle
        .marker(&module.name, None, "GenerateBuildProfile")
        .await?;
      task_lifecycle
        .marker(&module.name, Some(&module_target_name), "PreCheckSyscap")
        .await?;

      let module_json5_path = module_context
        .module
        .module_json_path
        .clone()
        .unwrap_or_else(|| module_layout.source().module_json5());
      // The `ohosTest` target builds the `src/ohosTest` module (`module.json5`
      // lives there with its own module name, e.g. `entry_test`). hvigor
      // `MergeProfile` spreads `getOhosTestAbilityConfig()` underneath the
      // module document when the source module.json5 lacks `srcEntry`
      // abilities (templates-after-source-code-migration).
      let module_json5 = if module_target_name == "ohosTest" {
        let ohos_test_json5 = module.src_path.join("src/ohosTest/module.json5");
        let content = std::fs::read_to_string(&ohos_test_json5).map_err(|error| {
          anyhow::anyhow!(
            "cannot read ohosTest module.json5 '{}': {error}",
            ohos_test_json5.display()
          )
        })?;
        let mut parsed: arkdown_core::ModuleJson5 = json5::from_str(&content).map_err(
          |error| {
            anyhow::anyhow!(
              "cannot parse ohosTest module.json5 '{}': {error}",
              ohos_test_json5.display()
            )
          },
        )?;
        let template_after_migration = parsed
          .module
          .abilities
          .iter()
          .all(|ability| ability.src_entry.is_empty())
          && !module
            .src_path
            .join("src/ohosTest/ets/testrunner/OpenHarmonyTestRunner.ets")
            .is_file();
        if template_after_migration {
          // `getOhosTestAbilityConfig()`: description/mainElement/pages +
          // the synthetic TestAbility, overlaid by the source module.json5
          // (hvigor `MergeProfile`: `{...getOhosTestAbilityConfig(),
          // ...o.module}` — the source module document wins on conflicts).
          let base = serde_json::json!({
            "module": {
              "description": "$string:module_test_desc",
              "mainElement": "TestAbility",
              "pages": "$profile:test_pages",
              "abilities": [{
                "name": "TestAbility",
                "srcEntry": "./ets/testability/TestAbility.ets",
                "description": "$string:TestAbility_desc",
                "label": "$string:TestAbility_label",
                "exported": true,
              }],
            }
          });
          // Parse the raw source document as a JSON value so missing fields
          // stay missing (serde `ModuleJson5Module` would serialize empty
          // arrays for absent abilities and overwrite the template).
          let raw_module: serde_json::Value = json5::from_str(&content).map_err(|error| {
            anyhow::anyhow!(
              "cannot parse ohosTest module.json5 '{}': {error}",
              ohos_test_json5.display()
            )
          })?;
          // hvigor `o.module = {...getOhosTestAbilityConfig(), ...o.module}`
          // merges at the module-document level: the source document wins on
          // conflicting keys, the template fills the rest.
          let merged = match (base, raw_module) {
            (
              serde_json::Value::Object(mut base),
              serde_json::Value::Object(src),
            ) => {
              if let (
                Some(serde_json::Value::Object(base_module)),
                Some(serde_json::Value::Object(src_module)),
              ) = (base.get_mut("module"), src.get("module"))
              {
                base_module.extend(src_module.clone());
              } else {
                base.extend(src);
              }
              serde_json::Value::Object(base)
            }
            (_, src) => src,
          };
          parsed = serde_json::from_value(merged)
            .map_err(|error| anyhow::anyhow!("merge ohosTest module config: {error}"))?;
        }
        parsed
      } else {
        module_context
          .module
          .module_json
          .as_ref()
          .cloned()
          .ok_or_else(|| anyhow::anyhow!("missing module.json5 snapshot for {}", module.name))?
      };
      let profile_dependency_context = {
        let _timer = BuildTimingProbe::module(&module.name, "profile_dependency_context");
        arkdown_profile::ProfileDependencyContext::from_session_graph(
          module_config.as_ref(),
          module,
          module_context.deps.as_ref(),
          config
            .product
            .version_code
            .unwrap_or(session.project.version_code),
        )
        .excluding_package_roots(module_har_dedup_plan.duplicate_package_roots())
      };

      {
        let _timer = BuildTimingProbe::module(&module.name, "validate_compile_mode");
        process_profile_processor
          .validate_compile_mode(&module_json5, &module_json5_path.display().to_string())
          .map_err(|e| anyhow::anyhow!("{}", e))?;
      }

      // For hot rebuilds (skip_profile=true): run profile processing in
      // memory but skip all disk writes.  module.json5 and oh-package.json5
      // have not changed, so the previous build's disk outputs are still
      // valid.  We still need the in-memory `manifest` + `loader_json` for
      // the bundle and pack stages.
      //
      // # Upstream correspondence
      //
      // Mirrors hvigor's GenerateLoaderJson task being marked as cached when
      // no declared input has changed: the task body is not re-executed (disk
      // writes skipped) but the in-memory data is available to downstream
      // stages.
      if skip_profile_writes {
        tracing::debug!(
            module = %module.name,
            "hot rebuild: skip_profile=true — running profile in-memory, skipping writes"
        );
      }

      let manifest_device_types: Vec<String> = if module_json5.module.device_types.is_empty() {
        vec!["phone".to_string()]
      } else {
        module_json5.module.device_types.clone()
      };

      // ── Resource compilation ──────────────────────────────────────
      let res_out = module_layout.res_default_dir();
      let res_layout = ResourceOutputLayout::new(&res_out);
      let skip_res = args.skip_resource;
      if skip_res {
        tracing::debug!(
            module = %module.name,
            "hot rebuild: skipping resource compilation (skip_resource=true)"
        );
      }

      let mut resource_refs = FxHashMap::default();

      // Preload SO list
      let preload_so_paths = &module_build_option.preload_so;
      if !preload_so_paths.is_empty() {
        let _timer = BuildTimingProbe::module(&module.name, "preload_so_list");
        let preload_so_out = module_layout.preload_so_dir();
        std::fs::create_dir_all(&preload_so_out).ok();
        let so_list: Vec<String> = preload_so_paths
          .iter()
          .map(|path| ArtifactPathSerializer::path_to_slash_string(path))
          .collect::<Result<Vec<_>, _>>()
          .map_err(|error| anyhow::anyhow!("serialize preloadSo path failed: {error}"))?;
        let json = serde_json::json!({ "soFileList": so_list });
        if !skip_profile_writes {
          intermediate_writer
            .write_json_pretty(&module_layout.preload_so_list_json(), &json)
            .unwrap_or_else(|e| {
              tracing::warn!(error = %e, "failed to write preload_so_list.json");
            });
        }
      }

      // ── Profile processing ────────────────────────────────────────
      let ability_names: Vec<String> = module_json5
        .module
        .abilities
        .iter()
        .map(|a| a.name.clone())
        .collect();
      {
        let _timer = BuildTimingProbe::module(&module.name, "validate_request_permissions");
        permission_validator
          .validate_request_permissions(arkdown_profile::RequestPermissionValidationInput {
            module_type: &module_json5.module.type_,
            module_json5_path: Some(&module_json5_path),
            request_permissions: &module_json5.module.request_permissions,
            define_permissions: &module_json5.module.define_permissions,
            sdk_permissions: sdk_permissions.as_ref(),
            ability_names: if ability_names.is_empty() {
              None
            } else {
              Some(&ability_names)
            },
          })
          .map_err(|e| anyhow::anyhow!("validate request permissions failed: {}", e))?;
      }

      if matches!(module_json5.module.type_.as_str(), "shared" | "har")
        || matches!(&config.output_target, OutputTarget::Hsp | OutputTarget::Har)
      {
        let _timer = BuildTimingProbe::module(&module.name, "permission_reason_resources");
        permission_reason_processor
          .process_permission_reason_resources(arkdown_profile::PermissionReasonResourceInput {
            module_root: &module.src_path,
            target_name: &module_target_name,
            request_permissions: &module_json5.module.request_permissions,
          })
          .map_err(|e| anyhow::anyhow!("process permission reason resources failed: {}", e))?;
      } else if matches!(&config.output_target, OutputTarget::Hap) {
        let _timer = BuildTimingProbe::module(&module.name, "dependency_permission_reason_merge");
        permission_reason_processor
          .merge_dependency_permission_reason_resources_from_dependency_graph(
            arkdown_profile::DependencyPermissionReasonGraphResourceInput {
              module_root: &module.src_path,
              project_root: &project_root,
              target_name: &module_target_name,
              dependency_graph: module_context.deps.as_ref(),
            },
          )
          .map_err(|e| {
            anyhow::anyhow!("merge dependency permission reason resources failed: {}", e)
          })?;
      }

      {
        let _timer = BuildTimingProbe::module(&module.name, "shortcut_profiles");
        arkdown_profile::ShortcutProfileProcessor::new(&module.name)
          .process_shortcut_profiles(arkdown_profile::ShortcutProfileInput {
            module_root: &module.src_path,
            metadata: &module_json5.module.metadata,
            app_bundle_name: &app_scope_bundle_name,
            product_bundle_name: &config.product.bundle_name,
            output_profile_dir: &res_layout.profile_dir(),
          })
          .map_err(|e| anyhow::anyhow!("process shortcut profiles failed: {}", e))?;
      }

      let pkg_context_info = if module_build_option.strict_mode.use_normalized_ohm_url {
        let _timer = BuildTimingProbe::module(&module.name, "pkg_context_info_generate");
        let input =
          PkgContextInfoInputBuilder::new(module_config.as_ref(), session, module, &module_context)
            .excluding_package_roots(module_har_dedup_plan.duplicate_package_roots())
            .build()?;
        Some(
          PkgContextInfoGenerator::new()
            .generate(&input)
            .map_err(|e| anyhow::anyhow!("GeneratePkgContextInfo failed: {}", e))?,
        )
      } else {
        None
      };

      if skip_profile_writes {
        task_lifecycle
          .up_to_date(
            &module.name,
            Some(&module_target_name),
            "GeneratePkgContextInfo",
          )
          .await?;
      } else if task_lifecycle
        .before(
          &module.name,
          Some(&module_target_name),
          "GeneratePkgContextInfo",
        )
        .await?
      {
        if let Some(pkg_context_info) = pkg_context_info.as_ref() {
          let _timer = BuildTimingProbe::module(&module.name, "pkg_context_info_write");
          PkgContextInfoGenerator::new()
            .write_generated(pkg_context_info, &module_layout.pkg_context_info_json())
            .map_err(|e| anyhow::anyhow!("GeneratePkgContextInfo failed: {}", e))?;
        }
        task_lifecycle
          .after(
            &module.name,
            Some(&module_target_name),
            "GeneratePkgContextInfo",
          )
          .await?;
      }

      // `GeneratePkgSdkInfo` is registered globally, but latest hvigor only
      // schedules it through the HAP/HSP pack task graph. A normal HAR build
      // therefore has no `intermediates/loader/.../pkgSdkInfo.json`; retain a
      // directly selected native task root for plugin/task-runner parity.
      let runs_pkg_sdk_info = module.module_type != OhosModuleType::Har
        || task_lifecycle.has_root(
          &module.name,
          Some(&module_target_name),
          "GeneratePkgSdkInfo",
        );
      if runs_pkg_sdk_info {
        if skip_profile_writes {
          task_lifecycle
            .up_to_date(
              &module.name,
              Some(&module_target_name),
              "GeneratePkgSdkInfo",
            )
            .await?;
        } else if task_lifecycle
          .before(
            &module.name,
            Some(&module_target_name),
            "GeneratePkgSdkInfo",
          )
          .await?
        {
          if !module_build_option.strict_mode.use_normalized_ohm_url {
            arkdown_profile::PkgSdkInfoGenerator::new(
              module_context.module.as_ref(),
              module_context.deps.as_ref(),
            )
            .write(&module_layout.pkg_sdk_info_json())
            .map_err(|error| anyhow::anyhow!("GeneratePkgSdkInfo failed: {error}"))?;
          }
          task_lifecycle
            .after(
              &module.name,
              Some(&module_target_name),
              "GeneratePkgSdkInfo",
            )
            .await?;
        }
      }

      let privacy_profile_path = {
        let _timer = BuildTimingProbe::module(&module.name, "module_privacy_profile");
        if task_lifecycle
          .before(
            &module.name,
            Some(&module_target_name),
            "ProcessModulePrivacyProfile",
          )
          .await?
        {
          let output = if BuildPolicy::module_processes_privacy_profile(&module.module_type) {
            privacy_profile_processor
              .process_module_privacy_profile(arkdown_profile::PrivacyProfileInput {
                module_root: &module.src_path,
                target_name: &module_target_name,
                pac_schema_path: pac_schema_path.as_deref(),
              })
              .map_err(|e| anyhow::anyhow!("ProcessModulePrivacyProfile failed: {}", e))?
              .map(|output| output.processed_path)
          } else {
            None
          };
          task_lifecycle
            .after(
              &module.name,
              Some(&module_target_name),
              "ProcessModulePrivacyProfile",
            )
            .await?;
          output
        } else {
          None
        }
      };

      let custom_type_inventory = {
        let configured_types = module_build_option
          .ark_options
          .as_ref()
          .map(|options| options.types.as_slice())
          .unwrap_or_default();
        let base_ets_root = module_layout.source().ets();
        let target_ets_root = module_layout.source().target_ets(&module_target_name);
        let has_arkts_sources = module_context
          .module
          .source_roots
          .iter()
          .any(|root| root.file_name().and_then(|name| name.to_str()) == Some("ets"))
          || base_ets_root.is_dir()
          || target_ets_root.is_dir();
        let module_profile_path = module.src_path.join("build-profile.json5");
        let profile_path = if module_profile_path.is_file() {
          module_profile_path
        } else {
          project_root.join("build-profile.json5")
        };
        CustomTypeResolver::new(
          &module.name,
          &module.src_path,
          &profile_path,
          module_config.sdk_config.compile_api_version,
          has_arkts_sources,
          module_context.deps.as_ref(),
        )
        .resolve(configured_types)
        .map_err(|error| anyhow::anyhow!("custom types validation failed: {error}"))?
      };

      let (manifest, mut loader_json) = {
        let _timer = BuildTimingProbe::module(&module.name, "process_module");
        if !task_lifecycle
          .before(&module.name, Some(&module_target_name), "ProcessProfile")
          .await?
        {
          anyhow::bail!(
            "ARKDOWN_DISABLED_REQUIRED_HVIGOR_TASK: {}:{}@ProcessProfile is required to construct the in-memory manifest",
            module.name,
            module_target_name
          );
        }
        let output = processor
          .process_module_with_dependency_context_and_target(
            module,
            &module_json5,
            &profile_dependency_context,
            &module_target_name,
          )
          .map_err(|e| anyhow::anyhow!("{}", e))?;
        task_lifecycle
          .after(&module.name, Some(&module_target_name), "ProcessProfile")
          .await?;
        output
      };
      if matches!(
        module_build_option.har_artifact_type,
        arkdown_core::HarArtifactType::Bytecode
      ) {
        loader_json.include_byte_code_har_module_info(
          &module_layout.generated_module_info(&module_target_name),
        );
      }
      if let Some(pkg_context_info) = pkg_context_info.as_ref() {
        let bundled_dependencies = module_build_option
          .ark_options
          .as_ref()
          .is_some_and(|options| options.bundled_dependencies);
        let package_names = module_context
          .deps
          .bytecode_har_ohm_nodes_in_traversal_order(
            bundled_dependencies,
            args.byte_code_har_integrated_optimization,
          )
          .into_iter()
          .map(|node| node.package_name.clone())
          .collect::<Vec<_>>();
        loader_json.apply_bytecode_har_ohm_packages(&package_names, pkg_context_info);
      }
      loader_json.compiler_types = custom_type_inventory.compiler_types;
      loader_json.compiler_type_declaration_inputs = custom_type_inventory.declaration_inputs;
      if let Some(package) = module_context.module.package.as_deref() {
        let _timer = BuildTimingProbe::module(&module.name, "oh_exports");
        OhExportsProcessor::new()
          .process(
            package,
            &module_context.module.module_type,
            &mut loader_json,
          )
          .map_err(|error| anyhow::anyhow!("process oh-exports failed: {error}"))?;
      }

      // Profile disk writes.  Skipped for hot rebuilds (skip_profile=true)
      // since module.json5 / oh-package.json5 have not changed and the
      // previous build's outputs are still valid.
      let module_json_out = res_out.join("module.json");
      if !skip_profile_writes {
        let _timer = BuildTimingProbe::module(&module.name, "write_module_json_initial");
        processor
          .write_module_json_with_dependency_context(
            &manifest,
            &module_json_out,
            None,
            &profile_dependency_context,
          )
          .map_err(|e| anyhow::anyhow!("write module.json failed: {}", e))?;
        sanitizer_profile_flags
          .apply_to_module_json(&module_json_out)
          .map_err(|e| anyhow::anyhow!("apply sanitizer profile flags failed: {}", e))?;
      }

      // `ProcessRouterMap` feeds BasePackHapTask. Source/bytecode HARs carry
      // their router profile through `ProcessHarArtifacts` instead, so normal
      // HAR assembly must not create the two loader-router intermediates.
      // Preserve explicit native task invocation for the plugin task graph.
      let runs_router_map_task = module.module_type != OhosModuleType::Har
        || task_lifecycle.has_root(&module.name, Some(&module_target_name), "ProcessRouterMap");
      if runs_router_map_task {
        if skip_profile_writes {
          task_lifecycle
            .up_to_date(&module.name, Some(&module_target_name), "ProcessRouterMap")
            .await?;
        } else if task_lifecycle
          .before(&module.name, Some(&module_target_name), "ProcessRouterMap")
          .await?
        {
          let _timer = BuildTimingProbe::module(&module.name, "router_map_intermediates");
          intermediate_writer.write_router_map_intermediates(
            module,
            &loader_json,
            module_config.as_ref(),
            &res_out,
          )?;
          intermediate_writer.apply_compile_resource_module_metadata(
            &module_json_out,
            !loader_json.router_map.is_empty(),
            &res_layout.profile_dir().join("insight_intent.json"),
          )?;
          task_lifecycle
            .after(&module.name, Some(&module_target_name), "ProcessRouterMap")
            .await?;
        }
      }

      let process_share_config = if skip_profile_writes {
        task_lifecycle
          .up_to_date(
            &module.name,
            Some(&module_target_name),
            "ProcessShareConfig",
          )
          .await?;
        false
      } else {
        task_lifecycle
          .before(
            &module.name,
            Some(&module_target_name),
            "ProcessShareConfig",
          )
          .await?
      };
      let share_out = if process_share_config || skip_profile_writes {
        let _timer = BuildTimingProbe::module(&module.name, "share_config");
        let collect_dependency_share_config = module.module_type != OhosModuleType::Har
          || module_build_option
            .ark_options
            .as_ref()
            .is_some_and(|options| options.bundled_dependencies);
        let workspace_module_roots = module_config
          .modules
          .iter()
          .map(|project_module| canonicalize_or_owned(&project_module.src_path))
          .collect::<FxHashSet<_>>();
        let dependency_packages = if collect_dependency_share_config {
          module_context
            .deps
            .nodes
            .values()
            .filter(|node| {
              matches!(
                node.node_type,
                DependencyNodeType::SourceHar | DependencyNodeType::BytecodeHar
              )
            })
            .filter_map(|node| {
              node.package_root.as_ref().map(|package_root| {
                arkdown_profile::ShareConfigDependencyInput {
                  package_name: node.package_name.clone(),
                  package_root: package_root.clone(),
                  is_local: workspace_module_roots.contains(&canonicalize_or_owned(package_root)),
                }
              })
            })
            .collect()
        } else {
          Vec::new()
        };
        let share_out = share_config_processor
          .merge_share_config(arkdown_profile::ShareConfigInput {
            module_root: &module.src_path,
            dependency_packages,
            self_entries: module_json5
              .module
              .cross_app_shared_config
              .inline_entries()
              .to_vec(),
            self_profile_name: module_json5
              .module
              .cross_app_shared_config
              .profile_name()
              .map(str::to_string),
          })
          .map_err(|e| anyhow::anyhow!("ProcessShareConfig failed: {}", e))?;
        if process_share_config {
          let share_config_out = module_layout.share_config_temp_json();
          // Latest `ProcessShareConfig.processHarShareConfig` only writes its
          // declared output when a HAR actually has target share-config
          // entries. HAP/HSP use `processHapOrHspShareConfig`, which writes an
          // empty handoff as well. Remove a stale HAR handoff when a config is
          // deleted so CompileResource cannot consume obsolete state.
          let should_write_temp_share_config =
            !matches!(module.module_type, OhosModuleType::Har) || !share_out.entries.is_empty();
          if should_write_temp_share_config {
            share_config_processor
              .write_share_config(&share_out, &share_config_out)
              .map_err(|e| anyhow::anyhow!("write share config failed: {}", e))?;
          } else if share_config_out.is_file() {
            std::fs::remove_file(&share_config_out).map_err(|error| {
              anyhow::anyhow!(
                "remove stale HAR share-config handoff {} failed: {error}",
                share_config_out.display()
              )
            })?;
          }
          task_lifecycle
            .after(
              &module.name,
              Some(&module_target_name),
              "ProcessShareConfig",
            )
            .await?;
        }
        share_out
      } else {
        arkdown_profile::ShareConfigOutput { entries: vec![] }
      };

      let process_startup_config = if skip_profile_writes {
        task_lifecycle
          .up_to_date(
            &module.name,
            Some(&module_target_name),
            "ProcessStartupConfig",
          )
          .await?;
        false
      } else {
        task_lifecycle
          .before(
            &module.name,
            Some(&module_target_name),
            "ProcessStartupConfig",
          )
          .await?
      };
      let startup_out = if process_startup_config || skip_profile_writes {
        let _timer = BuildTimingProbe::module(&module.name, "startup_config");
        let startup_out = startup_config_processor
          .merge_startup_config(arkdown_profile::StartupConfigInput {
            project_root: &config.project_root,
            module_root: &module.src_path,
            module_name: &module.name,
            bundle_name: Some(config.product.bundle_name.as_str()),
            app_startup: module_json5.module.app_startup.clone(),
            collect_preload_system_so: module_build_option.preload_system_so
              && matches!(
                module.module_type,
                arkdown_core::OhosModuleType::Entry | arkdown_core::OhosModuleType::Feature
              )
              && !matches!(
                config.output_target,
                OutputTarget::Preview | OutputTarget::Test
              ),
          })
          .map_err(|e| anyhow::anyhow!("ProcessStartupConfig failed: {}", e))?;
        if process_startup_config {
          startup_config_processor
            .write_startup_config(&startup_out, &module_layout.startup_config_temp_json())
            .map_err(|e| anyhow::anyhow!("write startup config failed: {}", e))?;
          task_lifecycle
            .after(
              &module.name,
              Some(&module_target_name),
              "ProcessStartupConfig",
            )
            .await?;
        }
        startup_out
      } else {
        arkdown_profile::StartupConfigOutput { config: None }
      };

      if !skip_profile_writes {
        {
          let _timer = BuildTimingProbe::module(&module.name, "startup_profile_ref_update");
          ModuleJsonReader::new(module_json_out.parent().unwrap_or(&module_json_out))
            .ensure_startup_profile_ref(&module_json_out, &startup_out, &intermediate_writer)
            .map_err(|e| anyhow::anyhow!("update startup module.json failed: {}", e))?;
        }
        {
          let _timer = BuildTimingProbe::module(&module.name, "distro_filter_profile");
          distro_filter_processor
            .process_resolved_distro_filter_profile(
              arkdown_profile::DistroFilterProfileInput {
                project_root: &project_root,
                module_name: &module.name,
                module_root: &module.src_path,
                product_name: &config.product.name,
                target_name: Some(&module_target_name),
                build_mode_name: Some(match config.build_mode {
                  BuildMode::Debug => "debug",
                  BuildMode::Release => "release",
                }),
                compile_api_version: config.sdk_config.compile_api_version,
                metadata: &module_json5.module.metadata,
                output_profile_dir: &res_layout.profile_dir(),
                module_json_path: Some(&module_json_out),
              },
              module_target.distribution_filter.as_ref(),
            )
            .map_err(|e| anyhow::anyhow!("process distro filter profile failed: {}", e))?;
        }
        {
          let _timer = BuildTimingProbe::module(&module.name, "profile_intermediates_initial");
          // hvigor `getTargetModuleOptObj` merges the form extension module's
          // form abilities (name/type/metadata) into the form widget module's
          // ark_module.json consumed by restool.
          let form_widget_abilities = merge_form_widget_abilities(config, &module_json5)?;
          intermediate_writer.write_profile_intermediates(
            &module.src_path,
            &config.output_target,
            &module_target_name,
            &module_json_out,
            module_json5.module.extension_abilities.len(),
            &form_widget_abilities,
          )?;
        }

        // hvigor `UnitTestProcessProfile` (runs before CompileResource so
        // restool annotates the injected TestAbility): when the generated
        // testability template exists (`<module>/.test/testability/TestAbility.ets`),
        // append the synthetic TestAbility to the processed profile and
        // overwrite the ark module json / module.json with the same document.
        if matches!(config.output_target, OutputTarget::Test)
          && super::unit_test::has_test_ability_template(&module.src_path.join(".test"))
        {
          let process_profile_json = module_layout.process_profile_module_json();
          let process_value = std::fs::read_to_string(&process_profile_json).map_err(|error| {
            anyhow::anyhow!(
              "UnitTestProcessProfile cannot read processed profile '{}': {error}",
              process_profile_json.display()
            )
          })?;
          let mut document: serde_json::Value = serde_json::from_str(&process_value).map_err(
            |error| {
              anyhow::anyhow!(
                "UnitTestProcessProfile cannot parse processed profile '{}': {error}",
                process_profile_json.display()
              )
            },
          )?;
          super::unit_test::inject_unit_test_ability(
            &mut document,
            module_json5.module.type_.as_str(),
          )
          .map_err(|error| {
            anyhow::anyhow!(
              "UnitTestProcessProfile failed for module '{}': {error}",
              module.name
            )
          })?;
          // hvigor `outputJSONSync(..., { spaces: "\t" })` for both files.
          let serialized = {
            let mut out = Vec::new();
            let mut serializer = serde_json::Serializer::with_formatter(
              &mut out,
              serde_json::ser::PrettyFormatter::with_indent(b"\t"),
            );
            serde::Serialize::serialize(&document, &mut serializer)
              .map_err(|error| anyhow::anyhow!("UnitTestProcessProfile serialize: {error}"))?;
            out
          };
          std::fs::write(&process_profile_json, &serialized).map_err(|error| {
            anyhow::anyhow!(
              "UnitTestProcessProfile write '{}': {error}",
              process_profile_json.display()
            )
          })?;
          std::fs::write(&module_layout.res_ark_module_json(), &serialized).map_err(|error| {
            anyhow::anyhow!(
              "UnitTestProcessProfile write ark_module.json '{}': {error}",
              module_layout.res_ark_module_json().display()
            )
          })?;
          std::fs::write(&module_json_out, &serialized).map_err(|error| {
            anyhow::anyhow!(
              "UnitTestProcessProfile write module.json '{}': {error}",
              module_json_out.display()
            )
          })?;
          task_lifecycle
            .after(
              &module.name,
              Some(&module_target_name),
              "UnitTestProcessProfile",
            )
            .await?;
        }

        let _timer = BuildTimingProbe::module(&module.name, "resource_config_write_initial");
        let mut module_res_dirs: Vec<PathBuf> =
          ResourceDirSelector::new(module_config.as_ref(), module, &module_target)
            .select()
            .into_iter()
            .filter(|d| d.exists())
            .collect();
        // The `ohosTest` target compiles the generated template resources
        // (`.test/<product>/intermediates/src/ohosTest/resources`) that define
        // `$string:module_test_desc` / `$string:TestAbility_*` / test_pages.
        if module_target_name == "ohosTest" {
          let template_resources = module
            .src_path
            .join(".test/default/intermediates/src/ohosTest/resources");
          if template_resources.is_dir() && !module_res_dirs.contains(&template_resources) {
            module_res_dirs.push(template_resources);
          }
        }
        if module_res_dirs.is_empty() {
          intermediate_writer.write_resource_config_files(
            &project_root,
            module,
            module_config.as_ref(),
            &res_out,
          )?;
        } else {
          intermediate_writer.write_resource_config_files_with_dirs_and_names(
            &project_root,
            module,
            module_config.as_ref(),
            &res_out,
            &module_res_dirs,
            &[],
            if module_target_name == "ohosTest" {
              std::slice::from_ref(&module_json5.module.name)
            } else {
              &[]
            },
            &module_target_name,
          )?;
        }
      }

      let process_resource_enabled = if skip_res {
        task_lifecycle
          .up_to_date(&module.name, Some(&module_target_name), "ProcessResource")
          .await?;
        false
      } else {
        task_lifecycle
          .before(&module.name, Some(&module_target_name), "ProcessResource")
          .await?
      };
      let (app_scope_res, res_dirs) = {
        let _timer = BuildTimingProbe::module(&module.name, "resource_dir_select");
        let include_app_scope =
          ResourceDirSelector::new(module_config.as_ref(), module, &module_target)
            .include_app_scope();
        let mut res_dirs: Vec<PathBuf> = vec![];
        let app_scope_res = project_layout.app_scope_resources();
        if include_app_scope && app_scope_res.exists() {
          res_dirs.push(app_scope_res.clone());
        }
        if let Some(res_opts) = module_build_option.res_options.as_ref() {
          for dir in &res_opts.resource_directories {
            if dir.exists() && !res_dirs.contains(dir) {
              res_dirs.push(dir.clone());
            }
          }
        }
        for resource_module in
          ModuleSelectionPlanner::resource_modules_for_build(config.as_ref(), module)
        {
          let resource_module_config = ModuleBuildConfiguration::resolve(
            config.as_ref(),
            session.build_options.as_ref(),
            resource_module,
            named_target.as_deref(),
            args.module_hvigor_overrides.get(&resource_module.name),
          )?;
          for module_res_dir in ResourceDirSelector::new(
            resource_module_config.config.as_ref(),
            resource_module,
            &resource_module_config.target,
          )
          .select()
          {
            if module_res_dir.exists() && !res_dirs.contains(&module_res_dir) {
              res_dirs.push(module_res_dir);
            }
          }
        }
        for dependency_res_dir in
          HarResourceDependencySelector::new(&module.src_path, module_context.deps.as_ref())
            .select()
        {
          if module_har_dedup_plan
            .duplicate_package_roots()
            .iter()
            .any(|root| dependency_res_dir.starts_with(root))
          {
            continue;
          }
          if !res_dirs.contains(&dependency_res_dir) {
            res_dirs.push(dependency_res_dir);
          }
        }
        // `AbstractResource.getRestoolConfigBuilder` registers the permission
        // reason `resource_str` intermediate after all HAR resource roots.
        let resource_str = module_layout.res_resource_str_dir();
        if resource_str.is_dir() && !res_dirs.contains(&resource_str) {
          res_dirs.push(resource_str);
        }
        // The `ohosTest` target compiles the generated template resources
        // (`.test/<product>/intermediates/src/ohosTest/resources`) that define
        // `$string:module_test_desc` / `$string:TestAbility_*` / test_pages.
        if module_target_name == "ohosTest" {
          let template_resources = module
            .src_path
            .join(".test/default/intermediates/src/ohosTest/resources");
          if template_resources.is_dir() && !res_dirs.contains(&template_resources) {
            res_dirs.push(template_resources);
          }
        }
        (app_scope_res, res_dirs)
      };
      if !skip_profile_writes {
        let _timer = BuildTimingProbe::module(&module.name, "resource_config_write_final");
        let current_module_resource_dirs: Vec<PathBuf> =
          ResourceDirSelector::new(module_config.as_ref(), module, &module_target)
            .select()
            .into_iter()
            .filter(|dir| dir.exists())
            .collect();
        let dependency_resource_dirs: Vec<PathBuf> = res_dirs
          .iter()
          .filter(|dir| **dir != app_scope_res)
          .filter(|dir| !current_module_resource_dirs.contains(dir))
          .cloned()
          .collect();
        if current_module_resource_dirs.is_empty() {
          intermediate_writer.write_resource_config_files(
            &project_root,
            module,
            module_config.as_ref(),
            &res_out,
          )?;
        } else {
          intermediate_writer.write_resource_config_files_with_dirs_and_names(
            &project_root,
            module,
            module_config.as_ref(),
            &res_out,
            &current_module_resource_dirs,
            &dependency_resource_dirs,
            if module_target_name == "ohosTest" {
              std::slice::from_ref(&module_json5.module.name)
            } else {
              &[]
            },
            &module_target_name,
          )?;
        }
      }
      if process_resource_enabled {
        task_lifecycle
          .after(&module.name, Some(&module_target_name), "ProcessResource")
          .await?;
      }

      let resource_backend =
        ResourceBackendRunner::default_sdk(config.sdk_config.restool_path.clone());
      let resource_compile_config = ResourceCompileConfig {
        package_name: config.product.bundle_name.clone(),
        module_name: module.name.clone(),
        // The `ohosTest` target packs the `src/ohosTest` module under its own
        // module name (e.g. `entry_test`); restool `--modules` must match the
        // processed profile name.
        module_names: if module_target_name == "ohosTest" {
          vec![module_json5.module.name.clone()]
        } else {
          config.modules.iter().map(|m| m.name.clone()).collect()
        },
        device_types: manifest_device_types,
        is_debug: matches!(config.build_mode, BuildMode::Debug),
      };

      let compile_resource_enabled = if skip_res || res_dirs.is_empty() {
        task_lifecycle
          .up_to_date(&module.name, Some(&module_target_name), "CompileResource")
          .await?;
        false
      } else {
        task_lifecycle
          .before(&module.name, Some(&module_target_name), "CompileResource")
          .await?
      };
      if compile_resource_enabled {
        let _timer = BuildTimingProbe::module(&module.name, "resource_compile_restool");
        match resource_backend.compile(
          &resource_compile_config,
          ResourceCompileInput {
            resources_dirs: res_dirs,
            output_dir: res_out.clone(),
          },
        ) {
          Ok(out) => {
            resource_refs = out
              .resource_infos
              .into_iter()
              .map(|(name, info)| {
                (
                  CompiledResourceKey::new(&name).into_string(),
                  ResourceReference {
                    id: info.id,
                    runtime_type: info.runtime_type,
                    bundle_name: config.product.bundle_name.clone(),
                    module_name: module.name.clone(),
                  },
                )
              })
              .collect();
            let resource_ids = out.resource_ids;
            if !skip_profile_writes {
              let _timer = BuildTimingProbe::module(&module.name, "resource_ids_profile_rewrite");
              processor
                .annotate_module_json_resource_ids(&module_json_out, &resource_ids)
                .map_err(|e| anyhow::anyhow!("annotate module.json resource IDs failed: {}", e))?;
            }
          }
          Err(e) => {
            return Err(anyhow::anyhow!(
              "resource compilation failed for module '{}': {}",
              module.name,
              e
            ));
          }
        }
      }
      // Restool materializes source resources into its output directory and
      // can overwrite processed profiles. Hvigor's CompileResource task then
      // emits the runtime-only startup/share profiles and updates module.json,
      // so perform the same handoff after restool returns.
      if compile_resource_enabled && !skip_profile_writes {
        if let Some(compiled_startup_config) = startup_out.compiled_resource_profile() {
          let profile_path = module_layout.res_profile_dir().join("startup_config.json");
          intermediate_writer
            .write_json_minified(&profile_path, &compiled_startup_config)
            .map_err(|e| anyhow::anyhow!("write startup resource profile failed: {}", e))?;
        }
        if !share_out.entries.is_empty() {
          let profile_name = module_json5
            .module
            .cross_app_shared_config
            .profile_name()
            .unwrap_or("default_share_config");
          let profile_path = module_layout
            .res_profile_dir()
            .join(format!("{profile_name}.json"));
          share_config_processor
            .write_compiled_share_config(&share_out, profile_name, &profile_path, &module_json_out)
            .map_err(|e| anyhow::anyhow!("write compiled share config failed: {}", e))?;
        }
        sanitizer_profile_flags
          .apply_to_module_json(&module_json_out)
          .map_err(|e| anyhow::anyhow!("apply sanitizer profile flags failed: {}", e))?;
      }
      {
        let _timer = BuildTimingProbe::module(&module.name, "system_resource_references");
        resource_refs.extend(
          SystemResourceReferenceLoader::new(
            &config.sdk_config.ets_loader_path,
            &config.product.bundle_name,
            &module.name,
          )
          .load()?,
        );
      }
      if compile_resource_enabled {
        task_lifecycle
          .after(&module.name, Some(&module_target_name), "CompileResource")
          .await?;
      }

      let loader_json_out = module_layout.loader_json();

      // Scan ByteCode HAR dependencies.
      //
      // Primary path: consume `DependencyGraphSnapshot::bytecode_har_nodes()`
      // from the session dependency graph.  When the session graph has
      // `BytecodeHar` nodes with `package_root` set, we read their
      // `oh-package.json5` directly rather than doing an independent
      // `oh_modules/` filesystem walk.
      //
      // # Upstream correspondence
      //
      // Mirrors hvigor's `DependencyManager` providing typed node lists to
      // `generate-loader-json.ts` instead of re-scanning `oh_modules/`.
      let module_context = session
        .module_context(&module.name)
        .ok_or_else(|| anyhow::anyhow!("missing module compile context for {}", module.name))?;
      let mut bc_har_deps = {
        let _timer = BuildTimingProbe::module(&module.name, "bytecode_har_scan");
        let typed_roots: Vec<ByteCodeHarDependencyRoot> =
          if module_build_option.strict_mode.use_normalized_ohm_url
            && !matches!(config.output_target, OutputTarget::Har)
          {
            module_context
              .deps
              .bytecode_har_nodes_in_traversal_order()
              .into_iter()
              .filter_map(|node| {
                node
                  .package_root
                  .as_ref()
                  .map(|root| ByteCodeHarDependencyRoot {
                    package_name: node.package_name.clone(),
                    package_root: root.clone(),
                    is_har_dependency: node
                      .module_profile
                      .as_ref()
                      .and_then(|profile| profile.module_type.as_ref())
                      .is_some_and(|module_type| matches!(module_type, OhosModuleType::Har)),
                  })
              })
              .collect()
          } else {
            Vec::new()
          };

        let bc_har_deps = ByteCodeHarMetadataWriter::scan_from_typed_roots(&typed_roots);

        if !typed_roots.is_empty() {
          tracing::debug!(
              module = %module.name,
              count = bc_har_deps.len(),
              "ByteCode HAR scan: used {} typed session-graph roots",
              typed_roots.len()
          );
        }

        bc_har_deps
      };

      bc_har_deps
        .retain(|entry| !module_har_dedup_plan.contains_bytecode_package(&entry.package_name));

      loader_json.apply_bytecode_har_scan_entries(
        &bc_har_deps,
        &module.src_path,
        &config.project_root,
        config.output_target.clone(),
      );

      if skip_profile_writes {
        task_lifecycle
          .up_to_date(
            &module.name,
            Some(&module_target_name),
            "GenerateLoaderJson",
          )
          .await?;
      } else {
        if task_lifecycle
          .before(
            &module.name,
            Some(&module_target_name),
            "GenerateLoaderJson",
          )
          .await?
        {
          let _timer = BuildTimingProbe::module(&module.name, "loader_json_write");
          processor
            .write_loader_json(&loader_json, &loader_json_out)
            .map_err(|e| anyhow::anyhow!("write loader.json with byteCodeHarInfo failed: {}", e))?;
          task_lifecycle
            .after(
              &module.name,
              Some(&module_target_name),
              "GenerateLoaderJson",
            )
            .await?;
        }
      }

      // hvigor `ReplaceUnitTestIndexFile` (after CompileResource): rewrite
      // the res module.json pages and emit `main_pages.json` from the
      // `unit.test.replace.page` default.
      if matches!(config.output_target, OutputTarget::Test) {
        let process_profile_json = module_layout.process_profile_module_json();
        let process_value = std::fs::read_to_string(&process_profile_json)?;
        let document: serde_json::Value = serde_json::from_str(&process_value)?;
        super::unit_test::replace_unit_test_index_file(
          &module_layout.res_module_json(),
          &document,
          &module_layout.res_profile_dir(),
          module_json5.module.type_.as_str(),
        )
        .map_err(|error| {
          anyhow::anyhow!(
            "ReplaceUnitTestIndexFile failed for module '{}': {error}",
            module.name
          )
        })?;
        task_lifecycle
          .after(&module.name, Some(&module_target_name), "ReplaceUnitTestIndexFile")
          .await?;
      }

      if module_target_name == "ohosTest" {
        // The ohosTest loader names the test module (`entry_test`) and maps
        // it to the same source path (hvigor `modulePathMap`).
        loader_json.module_name = module_json5.module.name.clone();
        let test_module_path = std::fs::canonicalize(&module.src_path)
          .unwrap_or_else(|_| module.src_path.clone());
        loader_json.module_path_map.insert(
          module_json5.module.name.clone(),
          arkdown_path::ArtifactPathSerializer::path_to_slash_string(&test_module_path)
            .unwrap_or_else(|_| test_module_path.display().to_string()),
        );
      }
      loader_jsons.push((
        module.clone(),
        manifest,
        loader_json,
        resource_refs,
        module_target_name,
        module_config,
      ));
      all_bc_har_deps.push(bc_har_deps);
      privacy_profile_paths.push(privacy_profile_path);
    }
    progress.done();

    // Latest ets-loader registers `etsChecker` before `etsTransform` and runs
    // it from `buildStart`.  Profiles/loader entries are now complete, while
    // no Rolldown transform has run yet, which is the equivalent native
    // boundary. The SDK checker remains an explicit stage so transform hooks
    // never read SDK/configuration state.
    if checker_selection.runs_sdk_semantic_checker() {
      let semantic_checker_start = Instant::now();
      let mut checker_plans = Vec::with_capacity(loader_jsons.len());
      for (module, _manifest, loader_json, _resource_refs, module_target_name, module_config) in
        &loader_jsons
      {
        if !BuildPolicy::new(module_config.as_ref()).module_requires_arkts_compilation(module)? {
          continue;
        }
        let module_context = session.module_context(&module.name).ok_or_else(|| {
          anyhow::anyhow!("missing build session module context for {}", module.name)
        })?;
        let module_layout = ModuleBuildLayout::for_output_target(
          &config.output_target,
          &module.src_path,
          module_target_name,
        );
        let generated_profile_path = module_layout
          .build_profile_for_module(&module.module_type, module_target_name)
          .parent()
          .map(Path::to_path_buf)
          .ok_or_else(|| {
            anyhow::anyhow!(
              "SDK etsChecker cannot derive generated profile directory for module '{}'",
              module.name
            )
          })?;
        let paths = SdkEtsCheckerPaths::build(
          module_layout.res_profile_dir(),
          module_layout.res_ark_module_json(),
          module_layout.resource_table_txt(),
          module_layout.res_resources_dir().join("rawfile"),
          module_layout.source().ets(),
          module_layout.source().src_main().join("supervisual"),
          module_layout.loader_json(),
          PathBuf::from(&loader_json.cache_path),
          PathBuf::from(&loader_json.ace_module_build),
          generated_profile_path,
          module_layout.generated_resource_table_typescript(),
        );
        let entries = SdkEtsCheckerEntryCollector::new(loader_json).collect();
        checker_plans.push(
          SdkEtsCheckerPlanBuilder::new(SdkEtsCheckerInput {
            config: module_config.as_ref(),
            session: session.as_ref(),
            module,
            module_context: &module_context,
            module_build_option: &module_config.product.build_option,
            loader_json: Some(loader_json),
            target_name: module_target_name,
            node_executable: args.resolved_node_executable(),
            paths,
            entries,
            coverage_enabled: coverage_mode,
            debug_line: false,
            byte_code_har_integrated_optimization: args.byte_code_har_integrated_optimization,
          })
          .build()?,
        );
      }
      if !checker_plans.is_empty() {
        SdkSemanticCheckStage::new(checker_plans).run()?;
        tracing::info!(
          elapsed_ms = semantic_checker_start.elapsed().as_millis(),
          "SDK etsChecker semantic stage completed"
        );
      }
    }

    // ── Step 3: Bundling ──────────────────────────────────────────────
    progress.step(3, 6, "Bundling");
    let mut bundle_outs: Vec<(BundleStageOutput,)> = Vec::new();
    let mut widget_bundle_outs: Vec<Option<WidgetCompileArktsOutput>> = Vec::new();
    let mut compile_task_active = Vec::new();
    let module_obfuscation_contexts = loader_jsons
      .iter()
      .map(|(module, _, _, _, module_target_name, module_config)| {
        ModuleObfuscationContext::resolve(
          config.as_ref(),
          session,
          module,
          &module_config.product.build_option,
        )
        .map(|mut context| {
          context.target_name = Some(module_target_name.clone());
          context
        })
      })
      .collect::<Result<Vec<_>>>()?;
    for (
      (module, manifest, loader_json, resource_refs, module_target_name, module_config),
      module_obfuscation,
    ) in loader_jsons.iter().zip(module_obfuscation_contexts.iter())
    {
      let module_build_option = &module_config.product.build_option;
      let module_layout = ModuleBuildLayout::for_output_target(
        &config.output_target,
        &module.src_path,
        module_target_name,
      );
      let module_package_plugins = package_plugins
        .iter()
        .filter(|plugin| plugin.applies_to_module_target(&module.src_path, module_target_name))
        .cloned()
        .collect::<Vec<_>>();
      // `this.share.projectConfig` is module-specific in latest hvigor. Seed
      // only the plugins participating in this module before cache hooks or
      // Rolldown hooks execute.
      let share_ctx_json = build_share_context_json(module_config.as_ref(), args, Some(module));
      for plugin in &module_package_plugins {
        plugin.seed_hvigor_context(&share_ctx_json).await?;
      }
      let module_har_dedup_plan = har_dedup_project_plan.module(&module.name);
      let cache_dir = PathBuf::from(&loader_json.cache_path);
      let output_dir = PathBuf::from(&loader_json.ace_module_build);
      let cached_abc = output_dir.join("modules.abc");
      if !BuildPolicy::new(module_config.as_ref()).module_requires_arkts_compilation(module)? {
        task_lifecycle
          .up_to_date(
            &module.name,
            Some(module_target_name),
            if matches!(module.module_type, OhosModuleType::Har) {
              "HarCompileArkTS"
            } else {
              "CompileArkTS"
            },
          )
          .await?;
        task_lifecycle
          .up_to_date(&module.name, Some(module_target_name), "BuildJS")
          .await?;
        bundle_outs.push((BundleStageOutput::skipped_original_har(
          cached_abc, output_dir,
        ),));
        widget_bundle_outs.push(None);
        compile_task_active.push(false);
        continue;
      }
      let widget_pass =
        WidgetCompileArktsPass::new(module_config.as_ref(), module, manifest, loader_json);
      let widget_compile_plan = widget_pass.plan()?;
      let task_plugin_hash =
        module_package_plugins
          .iter()
          .fold(0xcbf29ce484222325u64, |mut hash, plugin| {
            for byte in plugin.cache_identity().bytes() {
              hash ^= u64::from(byte);
              hash = hash.wrapping_mul(0x100000001b3);
            }
            hash
          });
      let task_input_snapshot = TaskInputSnapshot::for_module(session.as_ref(), &module.name)
        .ok_or_else(|| {
          anyhow::anyhow!(
            "missing session input fingerprints for module '{}'",
            module.name
          )
        })?;
      let bundle_compile_env = CompileEnvKeyBuilder::new(
        module_config.as_ref(),
        module,
        loader_json,
        module_obfuscation.is_enabled(&config.build_mode),
        module_target_name,
      )
      .build()?;
      let task_inputs =
        TaskInputs::compute(module_config.as_ref(), loader_json, &task_input_snapshot)
          .with_module_analyzer(module_analyzer)
          .with_effective_obfuscation(module_obfuscation.config.as_ref(), &config.build_mode)
          .with_compile_env(bundle_compile_env.clone())
          .with_plugin_hash(task_plugin_hash)
          .with_additional_obfuscation_rule_files(&module_obfuscation.dependency_rule_files)
          .with_custom_type_inputs(
            &loader_json.compiler_types,
            &loader_json.compiler_type_declaration_inputs,
          )?;

      // Compute deterministic cache-group snapshots for this module.
      // Mirrors hvigor's invalid-cache-plugin.ts group tracking.
      let group_snapshots = build_cache_group_snapshots(
        &bundle_compile_env,
        module_config.as_ref(),
        module,
        loader_json,
        &module_package_plugins,
        &project_root,
        args.byte_code_har_integrated_optimization,
      );

      // Build the cache bridge — loads group_keys.json and diffs against current snapshots.
      let cache_bridge = BundleCacheBridge::from_plan(
        bundle_compile_env.clone(),
        Some(cache_dir.clone()),
        group_snapshots.clone(),
      );

      let widget_cache_ready = widget_compile_plan
        .as_ref()
        .is_none_or(|plan| plan.modules_abc_path.exists());
      let declaration_cache_ready = !matches!(module.module_type, OhosModuleType::Shared)
        || module_layout.declarations_dir().is_dir();
      let mut cache_hit = !disable_abc_cache
        && task_inputs.matches_cached(&cache_dir)
        && cached_abc.exists()
        && widget_cache_ready
        && declaration_cache_ready;
      if cache_hit {
        // System-level check: compile-env + file changes + cache group changes.
        let system_request = ShouldInvalidCacheRequest {
          compile_env: bundle_compile_env.clone(),
          changed_files: watch_changed_files.clone(),
          module_name: Some(module.name.clone()),
          cache_dir: Some(cache_dir.clone()),
          is_watch: is_incremental_watch,
          cache_hit,
          group_snapshots: group_snapshots.clone(),
          module_deps: vec![],
        };
        let system_result = cache_bridge.ask_plugins(&system_request);
        if system_result.should_invalidate {
          tracing::debug!(
              module = %module.name,
              reason = ?system_result.reason,
              global = system_result.global_invalidation,
              changed_groups = %system_result.changed_groups.explain(),
              "cache invalidated by system check"
          );
          cache_hit = false;
        }
      }
      if cache_hit {
        cache_hit =
          !PluginCacheInvalidationRunner::new(&module_package_plugins, cache_dir.as_path())
            .should_invalidate()
            .await?;
      }

      if cache_hit {
        task_lifecycle
          .up_to_date(
            &module.name,
            Some(module_target_name),
            if matches!(module.module_type, OhosModuleType::Har) {
              "HarCompileArkTS"
            } else {
              "CompileArkTS"
            },
          )
          .await?;
        task_lifecycle
          .up_to_date(&module.name, Some(module_target_name), "BuildJS")
          .await?;
        bundle_outs.push((BundleStageOutput::from_cache(cached_abc, output_dir),));
        widget_bundle_outs.push(None);
        compile_task_active.push(false);
        continue;
      }

      let compile_task_name = if matches!(module.module_type, OhosModuleType::Har) {
        "HarCompileArkTS"
      } else {
        "CompileArkTS"
      };
      if !task_lifecycle
        .before(&module.name, Some(module_target_name), compile_task_name)
        .await?
      {
        task_lifecycle
          .up_to_date(&module.name, Some(module_target_name), "BuildJS")
          .await?;
        bundle_outs.push((BundleStageOutput::skipped_original_har(
          cached_abc, output_dir,
        ),));
        widget_bundle_outs.push(None);
        compile_task_active.push(false);
        continue;
      }
      compile_task_active.push(true);

      // Clean stale bundled .js files from previous builds.
      if output_dir.exists() {
        for entry in walkdir::WalkDir::new(&output_dir)
          .max_depth(1)
          .into_iter()
          .filter_map(|e| e.ok())
        {
          if entry.path().extension().and_then(|e| e.to_str()) == Some("js") {
            let _ = std::fs::remove_file(entry.path());
          }
        }
      }

      let module_context = session.module_context(&module.name).ok_or_else(|| {
        anyhow::anyhow!("missing build session module context for {}", module.name)
      })?;
      let pkg_context_builder =
        PkgContextInfoInputBuilder::new(module_config.as_ref(), session, module, &module_context)
          .excluding_package_roots(module_har_dedup_plan.duplicate_package_roots());
      let source_map_packages = pkg_context_builder.source_map_package_info()?;
      let runtime_pkg_contexts = PkgContextInfoGenerator::new()
        .generate(&pkg_context_builder.build()?)
        .map_err(|e| anyhow::anyhow!("GeneratePkgContextInfo failed: {}", e))?;
      let runtime_packages = BundleRuntimePackageInfoBuilder::new(&runtime_pkg_contexts).build();
      let loader_json_out = module_layout.loader_json();
      let mut module_record_roots = Vec::new();
      for node in module_context.deps.nodes.values() {
        if !matches!(
          node.node_type,
          DependencyNodeType::SourceHar
            | DependencyNodeType::OhpmSource
            | DependencyNodeType::IntegratedHsp
        ) {
          continue;
        }
        let Some(package_root) = node.package_root.clone() else {
          continue;
        };
        module_record_roots.push((node.package_name.clone(), package_root.clone()));
        if node.alias != node.package_name {
          module_record_roots.push((node.alias.clone(), package_root));
        }
      }
      module_record_roots
        .sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
      module_record_roots.dedup();

      let mut bundle_plan = make_bundle_plan(
        loader_json,
        &loader_json_out,
        PathBuf::from(&loader_json.ace_module_build),
        module_config.as_ref(),
        Some(&custom_cfg),
        module_analyzer,
        BundlePlanPackageInfo {
          source_map: source_map_packages,
          runtime: runtime_packages,
        },
      );
      if module_target_name == "ohosTest" {
        // The ohosTest module's ets root is the generated template tree
        // (`.test/<product>/intermediates/src/ohosTest/ets`); entry page
        // paths resolve against it (`testability/pages/Index`).
        let template_ets = module
          .src_path
          .join(".test/default/intermediates/src/ohosTest/ets");
        if template_ets.is_dir() {
          bundle_plan.input.project_path = template_ets;
        }
      }
      bundle_plan.input.module_record_roots = module_record_roots;
      // hvigor's widget pass compiles the form card sources into the main
      // es2abc input (`filesInfo.txt`) in addition to `widgets.abc`, but only
      // on the form widget module itself (`needSubmitArkTsWidget`).
      if matches!(module.module_type, OhosModuleType::Shared)
        && Self::module_json5_has_form_extension_module(&module.src_path)
      {
        for form_entry in &loader_json.form_entry_sources {
          if !bundle_plan.input.entries.contains(form_entry) {
            bundle_plan.input.entries.push(form_entry.clone());
          }
        }
      }
      let preload_system_so_enabled = module_build_option.preload_system_so
        && module.module_type != OhosModuleType::Shared
        && match config.output_target {
          OutputTarget::Hap => true,
          OutputTarget::Har => {
            module_build_option.har_artifact_type == arkdown_core::HarArtifactType::Bytecode
          }
          _ => false,
        };
      if preload_system_so_enabled {
        let module_json = module_context.module.module_json.as_ref().ok_or_else(|| {
          anyhow::anyhow!(
            "module '{}' is missing module.json5 for preloadSystemSo planning",
            module.name
          )
        })?;
        bundle_plan.input.preload_system_so_entries =
          PreloadSystemSoPlan::new(&module.src_path, module_json, module_context.deps.as_ref())
            .entries()?;
      }

      let allowed_package_roots: Vec<PathBuf> = module_context
        .deps
        .nodes
        .values()
        .filter_map(|node| node.package_root.clone())
        .collect();
      let mut pkg_name_to_pkg_info = FxHashMap::default();
      for node in module_context.deps.nodes.values() {
        let Some(pkg_root) = node.package_root.clone() else {
          continue;
        };
        let info = PkgBriefInfo {
          local_source: !arkdown_path::classify::is_inside_oh_modules(&pkg_root),
          pkg_root,
          pkg_name: node.package_name.clone(),
          source_roots: vec![],
        };
        pkg_name_to_pkg_info
          .entry(node.package_name.clone())
          .or_insert_with(|| info.clone());
        if node.alias != node.package_name {
          pkg_name_to_pkg_info
            .entry(node.alias.clone())
            .or_insert_with(|| info.clone());
        }
      }

      // Compute source-set overlays for build-target overrides.
      let source_set_overlays: Vec<(PathBuf, PathBuf)> = {
        let context_layout = ModuleBuildLayout::new(&module_context.module.root);
        let base_ets = context_layout.source().ets();
        if module_target_name != "default" {
          let target_ets = context_layout.source().target_ets(module_target_name);
          if target_ets.is_dir() && target_ets != base_ets {
            vec![(base_ets, target_ets)]
          } else {
            vec![]
          }
        } else {
          vec![]
        }
      };

      let all_module_roots: Vec<(String, PathBuf)> = config
        .modules
        .iter()
        .map(|m| (m.name.clone(), m.src_path.clone()))
        .collect();
      let dependency_source_roots: Vec<PathBuf> = module_context
        .deps
        .nodes
        .values()
        .filter(|node| {
          matches!(
            node.node_type,
            DependencyNodeType::SourceHar
              | DependencyNodeType::OhpmSource
              | DependencyNodeType::IntegratedHsp
          )
        })
        .filter_map(|node| node.package_root.clone())
        .collect();
      let native_declaration_roots: Vec<PathBuf> = module_context
        .deps
        .nodes
        .values()
        .filter(|node| matches!(node.node_type, DependencyNodeType::NativeSo))
        .filter_map(|node| node.package_root.clone())
        .collect();
      let transform_snapshot_start = Instant::now();
      let dependency_package_roots: Vec<(String, PathBuf)> = module_context
        .deps
        .nodes
        .values()
        .filter_map(|node| {
          node
            .package_root
            .clone()
            .map(|root| (node.package_name.clone(), root))
        })
        .collect();
      // Local unit-test builds additionally compile the generated
      // testability templates and the `src/test` suite (hvigor
      // `UnitTestArkTS` inputs; `GenerateUnitTestTemplate` output).
      let test_source_roots: Vec<PathBuf> = if matches!(config.output_target, OutputTarget::Test)
      {
        vec![module_layout.testability_dir(), module_layout.test_sources_dir()]
      } else if module_target_name == "ohosTest" {
        vec![
          super::unit_test::ohos_test_template_root(&module.src_path),
          module_layout.source().target_ets(&module_target_name),
        ]
      } else {
        Vec::new()
      };
      let transform_sources = TransformSourceSnapshotBuilder::new(
        &all_module_roots,
        &dependency_source_roots,
        &native_declaration_roots,
        &source_set_overlays,
      )
      .with_test_source_roots(&test_source_roots)
      .with_package_roots(&dependency_package_roots)
      .with_sdk_declaration_roots(&sdk_declaration_roots)
      .with_custom_type_declaration_inputs(&loader_json.compiler_type_declaration_inputs)
      .build()?;
      tracing::info!(
        module = %module.name,
        elapsed_ms = transform_snapshot_start.elapsed().as_millis(),
        "transform source snapshot built"
      );

      let plugin_factory_start = Instant::now();
      let mut factory_input = PluginFactoryInput {
        target_name: module_target_name.clone(),
        sdk_api_dirs: config.sdk_config.ets_api_dirs(),
        sdk_kits_dirs: config.sdk_config.ets_kits_dirs(),
        sdk_arkts_dirs: config.sdk_config.ets_arkts_dirs(),
        all_module_roots,
        resource_refs: resource_refs.clone(),
        component_metadata: component_metadata.clone(),
        transform_sources,
        navigation_routes: NavigationRouteMap::from_loader_entries(&loader_json.router_map),
        allowed_package_roots,
        pkg_name_to_pkg_info,
        source_set_overlays: source_set_overlays.clone(),
        checker_mode,
        tsc_target: module_build_option
          .ark_options
          .as_ref()
          .map(|options| options.tsc_config.target_es_version)
          .unwrap_or_default(),
        case_sensitive_check: module_build_option.strict_mode.case_sensitive_check,
        har_dedup: HarDedupFactoryMode::Disabled,
        disabled_plugins: custom_cfg.plugins.disabled.clone(),
        package_plugins: module_package_plugins.clone(),
        auto_lazy_import: module_build_option
          .ark_options
          .as_ref()
          .and_then(|options| options.auto_lazy_import)
          .unwrap_or(false),
        auto_lazy_filter: module_build_option
          .ark_options
          .as_ref()
          .and_then(|options| options.auto_lazy_filter.clone()),
        expand_import_path: module_build_option
          .ark_options
          .as_ref()
          .and_then(|options| options.expand_import_path.clone()),
        hsp_har_package_names: loader_json
          .hsp_name_ohm_map
          .keys()
          .chain(loader_json.har_name_ohm_map.keys())
          .cloned()
          .collect(),
        // `meta.pkgName` for `isNotAutoLazyImport`: the owning module's
        // `oh-package.json5#name`.
        module_pkg_name: std::fs::read_to_string(module.src_path.join("oh-package.json5"))
          .ok()
          .and_then(|content| json5::from_str::<serde_json::Value>(&content).ok())
          .and_then(|raw| {
            raw.get("name")
              .and_then(|name| name.as_str())
              .map(str::to_string)
          }),
      };
      let collect_factory_output = if module_har_dedup_plan.is_empty() {
        None
      } else {
        let mut collect_input = factory_input.clone();
        collect_input.har_dedup = HarDedupFactoryMode::Collect {
          collect_file: module_layout.source_har_duplicate_files_info(),
          source_har_roots: module_har_dedup_plan.source_har_roots().to_vec(),
        };
        // The hvigor collection task runs the SDK compiler plugins but not
        // package-supplied compile plugins or their lifecycle hooks.
        collect_input.package_plugins.clear();
        Some(PluginFactory::new(&bundle_plan, collect_input).build_output()?)
      };
      if !module_har_dedup_plan.is_empty() {
        factory_input.har_dedup = HarDedupFactoryMode::RealBuild {
          collect_file: module_layout.source_har_duplicate_files_info(),
        };
      }
      let widget_bundle_plan = widget_compile_plan
        .as_ref()
        .map(|plan| widget_pass.bundle_plan(&bundle_plan, plan));

      let bundle_stage_start = Instant::now();
      #[cfg(feature = "memory-diagnostics")]
      arkdown::memory_diagnostics::dump_with(
        "build:before-bundle-stage",
        format_args!("module={}", module.name),
      );
      if let Some(collect_factory_output) = collect_factory_output {
        let collect_out = BundleStage::run(BundleStageInput {
          config: config.clone(),
          plugins: collect_factory_output.plugins,
          ets_transform_plan: Some(collect_factory_output.ets_transform_plan),
          plan: bundle_plan.clone(),
        })
        .await?;
        let bytecode_module_ids = module_har_dedup_plan
          .collect_bytecode_module_ids(collect_out.bundle_output.external_module_requests.iter());
        module_har_dedup_plan.write_bytecode_module_ids(
          &module_layout.bytecode_har_duplicate_files_info(),
          &bytecode_module_ids,
        )?;
      }
      let plugin_factory_output = PluginFactory::new(&bundle_plan, factory_input).build_output()?;
      let plugins = plugin_factory_output.plugins;
      let widget_plugins = plugins.clone();
      let ets_transform_plan = plugin_factory_output.ets_transform_plan;
      tracing::info!(
        module = %module.name,
        elapsed_ms = plugin_factory_start.elapsed().as_millis(),
        "bundle plugin factory completed"
      );

      let mut bundle_out = BundleStage::run(BundleStageInput {
        config: config.clone(),
        plugins,
        ets_transform_plan: Some(Arc::clone(&ets_transform_plan)),
        plan: bundle_plan,
      })
      .await?;
      // ets-loader's SourceMapGenerator runs during the Rollup pass, before
      // CompileArkTS. Stage the map at the same boundary so its previous cache
      // is still available for the current-pass-first merge.
      if let Some(source_maps_path) = bundle_out.bundle_output.source_maps_path.clone() {
        // Latest ProcessHarArtifacts does not materialize
        // `intermediates/source_map` for a ByteCode HAR, unlike HAP/HSP and a
        // source HAR. Keep that distinction in the Rust-owned handoff rather
        // than making the packer infer it from an optional file.
        let package_source_map_intermediate = module_layout.source_maps_intermediate();
        let package_source_map_handoff = if module.module_type == OhosModuleType::Har
          && module_build_option.har_artifact_type == HarArtifactType::Bytecode
        {
          SourceMapPackageHandoff::Suppress(&package_source_map_intermediate)
        } else {
          SourceMapPackageHandoff::Materialize(&package_source_map_intermediate)
        };
        let source_maps_path = intermediate_writer.stage_source_maps(
          &source_maps_path,
          &PathBuf::from(&loader_json.cache_path).join("sourceMaps.json"),
          &PathBuf::from(&loader_json.cache_path).join("sourceMaps.map"),
          package_source_map_handoff,
          &config.build_mode,
          &bundle_out.bundle_output.source_map_input_fingerprints,
        )?;
        bundle_out.bundle_output.source_maps_path = Some(source_maps_path);
      }
      let widget_bundle_out = if let (Some(widget_compile_plan), Some(widget_bundle_plan)) =
        (widget_compile_plan, widget_bundle_plan)
      {
        let mut bundle_out = BundleStage::run(BundleStageInput {
          config: config.clone(),
          plugins: widget_plugins,
          ets_transform_plan: Some(Arc::clone(&ets_transform_plan)),
          plan: widget_bundle_plan,
        })
        .await?;
        bundle_out.bundle_output.modules_abc_path = widget_compile_plan.modules_abc_path.clone();
        let output = WidgetCompileArktsOutput {
          bundle_out,
          plan: widget_compile_plan,
        };
        widget_pass.write_component_collection(&output, &ets_transform_plan)?;
        Some(output)
      } else {
        None
      };
      #[cfg(feature = "memory-diagnostics")]
      arkdown::memory_diagnostics::dump_with(
        "build:after-bundle-stage",
        format_args!(
          "module={} chunks={} module_source_files={} npm_entries={} source_maps={}",
          module.name,
          bundle_out.bundle_output.chunk_files.len(),
          bundle_out.bundle_output.module_source_files.len(),
          bundle_out.bundle_output.npm_entries.len(),
          bundle_out.bundle_output.source_maps_path.is_some()
        ),
      );
      tracing::info!(
        module = %module.name,
        elapsed_ms = bundle_stage_start.elapsed().as_millis(),
        "bundle stage run completed"
      );
      if preload_system_so_enabled {
        PreloadSystemSoArtifactWriter::new(
          &module_layout.preload_system_so_json(),
          &module_layout.res_profile_dir(),
          &module_layout.res_module_json(),
          &module.module_type,
          &bundle_out.bundle_output.preload_system_so_tasks,
        )
        .write_preload_artifact()?;
      }
      let mut code_resource_roots = vec![module_layout.source().ets()];
      code_resource_roots.extend(
        source_set_overlays
          .iter()
          .map(|(_, target_root)| target_root.clone()),
      );
      let code_resource_options = module_config
        .product
        .build_option
        .res_options
        .as_ref()
        .map(|options| options.copy_code_resource.clone())
        .unwrap_or_default();
      CodeResourceCopyPlan::new(
        code_resource_roots,
        module_layout.source().src_main().join("share"),
        bundle_out.bundle_output.output_dir.clone(),
        module_layout
          .default_cache_dir("code_resources")
          .join("manifest.json"),
        code_resource_options,
      )
      .execute()?;
      if matches!(module.module_type, OhosModuleType::Shared) {
        HarDeclarationGenerator::new_hsp(
          &module.src_path,
          &module_layout.source().ets(),
          &module.name,
        )
        .generate()
        .map_err(|error| anyhow::anyhow!("generate HSP declarations failed: {error}"))?;
      }
      // beforeBuildEnd is supplied by the internal Rolldown gap adapter and
      // executes immediately before Rolldown's native buildEnd hooks.

      if module_package_plugins
        .iter()
        .any(PackagePlugin::has_should_invalid_cache)
      {
        PluginCacheModuleInventory::from_module_sources(
          cache_dir.as_path(),
          &bundle_out.bundle_output.module_source_files,
        )?
        .save(cache_dir.as_path())?;
      } else {
        PluginCacheModuleInventory::clear(cache_dir.as_path())?;
      }

      // Write bundle-level cache manifest.
      {
        let bundle_cache_dir = module_layout.default_cache_dir("bundle");
        let bundle_cache = BuildCache::new(bundle_cache_dir);
        let bundle_config_hash = {
          let key_str = format!(
            "{}#{}#{:?}#{}#preloadSystemSo={}",
            config.sdk_config.compile_sdk_version,
            match config.build_mode {
              BuildMode::Debug => "debug",
              BuildMode::Release => "release",
            },
            config.output_target,
            module_analyzer.as_str(),
            module_build_option.preload_system_so
          );
          arkdown_cache::CacheKeyMaterial::hash_u64(&key_str)
        };
        let bundle_dep_hash = {
          let mut h: u64 = 0xcbf29ce484222325;
          let mut entries: Vec<_> = loader_json.har_name_ohm_map.iter().collect();
          entries.sort_by_key(|(k, _)| k.as_str());
          for (k, v) in &entries {
            for b in k.bytes().chain(b"|".iter().copied()).chain(v.bytes()) {
              h ^= b as u64;
              h = h.wrapping_mul(0x100000001b3);
            }
          }
          h
        };
        let bundle_key = BundleCacheKey {
          inner: CacheKey {
            content_hash: task_input_snapshot.source_hash.clone(),
            config_hash: bundle_config_hash,
            dependency_hash: bundle_dep_hash,
          },
          compile_env: bundle_compile_env,
        };
        if let Err(e) = bundle_cache.write_bundle_manifest(&bundle_key) {
          tracing::debug!(
              error = %e,
              "failed to write bundle cache manifest (non-fatal)"
          );
        }
      }

      // Persist cache-group hashes so the next build can diff against them.
      // This matches hvigor's `buildEnd` + `beforeBuildEnd` cache writes in
      // `invalid-cache-plugin.ts`.
      if let Err(e) = cache_bridge.commit_rollup_cache("", &group_snapshots) {
        tracing::debug!(error = %e, "failed to persist cache group hashes (non-fatal)");
      }

      bundle_outs.push((bundle_out,));
      widget_bundle_outs.push(widget_bundle_out);
    }
    progress.done();

    // Post-bundle: process importees and update loader.json.
    for (
      (
        (module, _manifest, loader_json, _resource_refs, module_target_name, module_config),
        (bundle_out,),
      ),
      bc_har_deps,
    ) in loader_jsons
      .iter_mut()
      .zip(bundle_outs.iter_mut())
      .zip(all_bc_har_deps.iter())
    {
      let processor = ProfileProcessor::new(module_config.clone());
      if bundle_out.bypasses_compilation() {
        continue;
      }
      let module_layout = ModuleBuildLayout::for_output_target(
        &config.output_target,
        &module.src_path,
        module_target_name,
      );
      let module_src_main = PathBuf::from(&loader_json.module_root_path);
      let importee_cfg = ProcessImporteeConfig {
        bundle_name: config.product.bundle_name.clone(),
        module_name: loader_json.module_name.clone(),
        module_src_main,
      };
      let result = ProcessImporteeRunner::new(
        &bundle_out.bundle_output.module_source_files,
        &bundle_out.bundle_output.entry_module_ids,
        &bundle_out.bundle_output.module_imports,
        &importee_cfg,
      )
      .process();
      loader_json.non_entry_importees = result.non_entry_importees;
      loader_json.compiled_importees = result.compiled_importees;

      let module_build_option = &module_config.product.build_option;
      let is_bytecode_har = matches!(module.module_type, OhosModuleType::Har)
        && matches!(
          module_build_option.har_artifact_type,
          HarArtifactType::Bytecode
        );
      let bundled_dependencies = module_build_option
        .ark_options
        .as_ref()
        .is_some_and(|options| options.bundled_dependencies);
      HarImporteeCache::new(&loader_json.cache_path).write_compile_output(
        &bundle_out.bundle_output.har_importees,
        is_bytecode_har,
        bundled_dependencies,
        args.byte_code_har_integrated_optimization,
      )?;

      // Hvigor enables collectImportersPlugin only for hot-reload builds.
      // Normal CompileArkTS builds must not emit importer_map intermediates.
      if watch_mode {
        let mut importer_map: FxHashMap<String, Vec<String>> = FxHashMap::default();
        let compiled_module_ids = bundle_out
          .bundle_output
          .module_source_files
          .iter()
          .map(|source| source.module_id.as_str())
          .collect::<FxHashSet<_>>();
        for (importer, imports) in &bundle_out.bundle_output.module_imports {
          if !compiled_module_ids.contains(importer.as_str()) {
            continue;
          }
          for imported in imports {
            if compiled_module_ids.contains(imported.as_str()) {
              importer_map
                .entry(imported.clone())
                .or_default()
                .push(importer.clone());
            }
          }
        }
        for importers in importer_map.values_mut() {
          importers.sort();
          importers.dedup();
        }
        let out_dir = module_layout.importer_map_dir();
        std::fs::create_dir_all(&out_dir).ok();
        let json = serde_json::to_value(&importer_map).unwrap_or_default();
        intermediate_writer
          .write_json_pretty(&module_layout.importer_map_json(), &json)
          .unwrap_or_else(|e| tracing::warn!(error = %e, "failed to write importer_map.json"));
      }

      let bundle_name = &config.product.bundle_name;
      for dep in bc_har_deps {
        if let Some(ohm_url) = dep.ohm_url(bundle_name) {
          if !loader_json.compiled_importees.contains(&ohm_url) {
            loader_json.compiled_importees.push(ohm_url);
          }
        }
      }

      // Remove stale .d.ts/.d.ets files no longer in the current bundle output.
      // Mirrors hvigor's removeRedundantDeclaredFiles (rm-declared-files.js).
      if !bundle_out.bundle_output.chunk_files.is_empty() {
        let current_stems: std::collections::HashSet<String> = bundle_out
          .bundle_output
          .chunk_files
          .iter()
          .filter_map(|(name, _)| {
            std::path::Path::new(name)
              .file_stem()
              .and_then(|s| s.to_str())
              .map(|s| s.to_string())
          })
          .collect();
        let out_dir = &bundle_out.bundle_output.output_dir;
        if out_dir.exists() {
          if let Ok(entries) = std::fs::read_dir(out_dir) {
            for entry in entries.flatten() {
              let path = entry.path();
              let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
              if ext == "d.ts" || ext == "d.ets" {
                let stem = path
                  .file_stem()
                  .and_then(|s| s.to_str())
                  .unwrap_or("")
                  .to_string();
                if !current_stems.contains(&stem) {
                  tracing::debug!(
                      path = %path.display(),
                      "removing stale declaration file (removeRedundantDeclaredFiles)"
                  );
                  let _ = std::fs::remove_file(&path);
                }
              }
            }
          }
        }
      }

      let loader_json_out = module_layout.loader_json();
      processor
        .write_loader_json(loader_json, &loader_json_out)
        .map_err(|e| anyhow::anyhow!("update loader.json with importees failed: {}", e))?;

      bundle_out.release_chunk_graph();
    }

    // ── Obfuscation (release builds only) ────────────────────────────
    let mut pending_bytecode_obfuscations = (0..bundle_outs.len())
      .map(|_| None)
      .collect::<Vec<Option<PendingBytecodeObfuscation>>>();
    for (
      bundle_index,
      (
        (
          (module, _manifest, loader_json, _resource_refs, _module_target_name, _module_config),
          (bundle_out,),
        ),
        module_obfuscation,
      ),
    ) in loader_jsons
      .iter()
      .zip(bundle_outs.iter())
      .zip(module_obfuscation_contexts.iter())
      .enumerate()
    {
      if bundle_out.bypasses_compilation() {
        continue;
      }
      let Some(obf_config) = module_obfuscation.config.as_ref() else {
        continue;
      };
      let obf_cache_dir = PathBuf::from(&loader_json.cache_path).join("obfuscation");
      let dependency_rule_files = module_obfuscation.dependency_rule_files.clone();
      let obfuscation_plan = ObfuscationPlan::from_build_options(
        &config.build_mode,
        Some(obf_config),
        ObfuscationOutputPaths::new(obf_cache_dir.clone()),
        None,
      );
      if obfuscation_plan.should_obfuscate() {
        obfuscation_plan
          .validate()
          .map_err(|e| anyhow::anyhow!("Obfuscation config invalid: {}", e))?;
        let rule_files = obfuscation_plan.all_rule_files();
        let arkguard_plan = ModuleArkguardPlanBuilder::new(
          config.as_ref(),
          session,
          module,
          loader_json,
          bundle_out,
          args.resolved_node_executable(),
        );
        let (arkguard, module_input) = arkguard_plan.build()?;
        let obfuscator = Obfuscator::new(ObfuscationOptions {
          enable_obfuscation: obfuscation_plan.should_obfuscate(),
          rule_files,
          dependency_rule_files: dependency_rule_files.clone(),
          cache_dir: obf_cache_dir,
          export_rule_path: None,
          arkguard: Some(arkguard),
        })
        .map_err(|e| anyhow::anyhow!("Obfuscation config invalid: {}", e))?;
        match obfuscator.mode() {
          ObfuscationMode::Bytecode => {
            pending_bytecode_obfuscations[bundle_index] = Some(PendingBytecodeObfuscation::new(
              obfuscator,
              arkguard_plan.bytecode_input(module_input),
            ));
          }
          ObfuscationMode::Source => {
            obfuscator
              .obfuscate_module(module_input)
              .map_err(|e| anyhow::anyhow!("Obfuscation failed: {}", e))?;
          }
          ObfuscationMode::Disabled => {}
        }
      }

      if matches!(config.build_mode, BuildMode::Release)
        && (args.target == "har" || matches!(&config.output_target, OutputTarget::Har))
        && !obf_config.consumer_rule_files.is_empty()
      {
        ConsumerObfuscationFileWriter::new(
          obf_config.consumer_rule_files.clone(),
          project_layout.obfuscation_txt(),
        )
        .with_dependency_rule_files(dependency_rule_files)
        .write()
        .map_err(|e| anyhow::anyhow!("Generate obfuscation.txt failed: {}", e))?;
      }
      tracing::debug!(module = %module.name, "module obfuscation stage completed");
    }

    // ── Coverage instrumentation ──────────────────────────────────────
    if coverage_mode {
      for (bundle_out,) in &bundle_outs {
        for (_chunk_name, path) in &bundle_out.bundle_output.chunk_files {
          if path.extension().and_then(|e| e.to_str()) == Some("js") && path.exists() {
            if let Ok(source) = fs_err::read_to_string(path) {
              let file_id = ArtifactPathSerializer::path_to_slash_string(path)?;
              let instrumented = CoverageInstrumenter::new(&source, &file_id).instrument();
              let _ = fs_err::write(path, instrumented);
            }
          }
        }
      }
    }

    // ── Step 5: ABC compilation ───────────────────────────────────────
    progress.step(5, 6, "Compiling ABC");
    for (
      bundle_index,
      (
        (
          (module, manifest, loader_json, _resource_refs, module_target_name, module_config),
          (bundle_out,),
        ),
        bc_har_deps,
      ),
    ) in loader_jsons
      .iter()
      .zip(bundle_outs.iter())
      .zip(all_bc_har_deps.iter())
      .enumerate()
    {
      let module_package_plugins = package_plugins
        .iter()
        .filter(|plugin| plugin.applies_to_module_target(&module.src_path, module_target_name))
        .collect::<Vec<_>>();
      let module_har_dedup_plan = har_dedup_project_plan.module(&module.name);
      let module_obfuscation = &module_obfuscation_contexts[bundle_index];
      if bundle_out.bypasses_compilation() {
        continue;
      }
      let compile_target = HvigorCompileTarget::from_loader_json(loader_json, module_config);
      let effective_cache_file = if disable_abc_cache {
        None
      } else {
        cli_cache_file
          .clone()
          .or_else(|| Some(PathBuf::from(&loader_json.cache_path).join("modules.cache")))
      };

      let abc_ts2abc_backend_inner = match abc_backend {
        AbcBackendChoice::Auto => RolldownBytecodeBackendChoice::Auto,
        AbcBackendChoice::Sdk => RolldownBytecodeBackendChoice::Sdk,
      };
      let bytecode_cache_dir = PathBuf::from(&loader_json.cache_path);
      let bytecode_project_root = PathBuf::from(&loader_json.project_root_path);
      let abc_input_files = bc_har_deps
        .iter()
        .map(|dep| {
          BytecodeAbcInput::from_project_cache(
            dep.abc_path.clone(),
            dep.package_name.clone(),
            &bytecode_project_root,
            &bytecode_cache_dir,
          )
          .map_err(|error| anyhow::anyhow!(error))
        })
        .collect::<Result<Vec<_>>>()?;
      let compile_context_info_path = if loader_json.use_normalized_ohm_url {
        let module_context = session
          .module_context(&module.name)
          .ok_or_else(|| anyhow::anyhow!("missing module compile context for {}", module.name))?;
        let pkg_context_input =
          PkgContextInfoInputBuilder::new(module_config.as_ref(), session, module, &module_context)
            .excluding_package_roots(module_har_dedup_plan.duplicate_package_roots())
            .build()?;
        let pkg_context_info = PkgContextInfoGenerator::new()
          .generate(&pkg_context_input)
          .map_err(|e| anyhow::anyhow!("GeneratePkgContextInfo failed: {}", e))?;
        let path = PathBuf::from(&loader_json.cache_path).join("compileContextInfo.json");
        CompileContextInfoWriter::new(
          loader_json,
          &bundle_out.bundle_output.module_source_files,
          &pkg_context_info,
          &session.project.bundle_type,
          &config.product.bundle_name,
          matches!(module.module_type, OhosModuleType::Shared)
            && Self::module_json5_has_form_extension_module(&module.src_path),
        )
        .write(&path)
        .map_err(|e| anyhow::anyhow!("GenerateCompileContextInfo failed: {}", e))?;
        Some(path)
      } else {
        None
      };
      #[cfg(feature = "memory-diagnostics")]
      arkdown::memory_diagnostics::dump_with(
        "build:before-bytecode-stage",
        format_args!(
          "module={} chunks={} module_sources={} external_requests={} npm_entries={}",
          module.name,
          bundle_out.bundle_output.chunk_files.len(),
          bundle_out.bundle_output.module_source_files.len(),
          bundle_out.bundle_output.external_module_requests.len(),
          bundle_out.bundle_output.npm_entries.len()
        ),
      );
      // `module_mode.js` es2abc command construction: `--branch-elimination`
      // only for non-debuggable builds without bytecode obfuscation, and
      // `--transform-lib` when `arkOptions.transformLib` is configured.
      let ark_options = &module_config.product.build_option.ark_options;
      let branch_elimination = !matches!(config.build_mode, BuildMode::Debug)
        && pending_bytecode_obfuscations[bundle_index].is_none()
        && ark_options
          .as_ref()
          .and_then(|options| options.branch_elimination)
          .unwrap_or(false);
      let transform_lib = ark_options
        .as_ref()
        .and_then(|options| options.transform_lib.clone());
      let bytecode_result = BytecodeStage::run(BytecodeStageInput {
        compiler: RolldownBytecodeCompiler::new(
          abc_ts2abc_backend_inner,
          config.sdk_config.es2abc_path.clone(),
          compile_target.api_version(),
          matches!(config.build_mode, BuildMode::Debug),
        )
        .with_target_api_sub_version(compile_target.api_sub_version())
        .with_branch_elimination(branch_elimination)
        .with_transform_lib(transform_lib.clone()),
        module_source_files: bundle_out.bundle_output.module_source_files.clone(),
        external_module_requests: bundle_out.bundle_output.external_module_requests.clone(),
        npm_entries: bundle_out.bundle_output.npm_entries.clone(),
        modules_abc_path: bundle_out.bundle_output.modules_abc_path.clone(),
        cache_dir: bytecode_cache_dir,
        cache_file: effective_cache_file,
        abc_input_files: abc_input_files.clone(),
        compile_context_info_path,
        byte_code_har: loader_json.byte_code_har.unwrap_or(false),
      });
      // On ABC compile failure, remove the task-inputs cache so the next build
      // is forced to recompile (mirrors hvigor's removeCompileFailedFilesFromCache).
      if bytecode_result.is_err() {
        let cache_dir_path = PathBuf::from(&loader_json.cache_path);
        let task_inputs_file = cache_dir_path.join(".task_inputs");
        if task_inputs_file.exists() {
          tracing::debug!(
            "removing failed-build cache: {} (removeCompileFailedFilesFromCache)",
            task_inputs_file.display()
          );
          let _ = std::fs::remove_file(&task_inputs_file);
        }
      }
      #[cfg(feature = "memory-diagnostics")]
      arkdown::memory_diagnostics::dump_with(
        "build:after-bytecode-stage",
        format_args!("module={} ok={}", module.name, bytecode_result.is_ok()),
      );
      bytecode_result?;

      task_lifecycle
        .transform_abc(
          &module.name,
          module_target_name,
          &bundle_out.bundle_output.modules_abc_path,
          module_obfuscation.is_enabled(&config.build_mode),
        )
        .await?;

      if let Some(pending) = pending_bytecode_obfuscations[bundle_index].take() {
        pending.run(&module.name)?;
      }

      // ── P12: Widget compile pass ──────────────────────────────────
      if let Some(widget_out) = widget_bundle_outs
        .get(bundle_index)
        .and_then(Option::as_ref)
      {
        let widget_abc_input_files = bc_har_deps
          .iter()
          .map(|dep| {
            BytecodeAbcInput::from_project_cache(
              dep.abc_path.clone(),
              dep.package_name.clone(),
              &bytecode_project_root,
              &widget_out.plan.cache_dir,
            )
            .map_err(|error| anyhow::anyhow!(error))
          })
          .collect::<Result<Vec<_>>>()?;
        let widget_loader_json =
          WidgetCompileArktsPass::new(module_config.as_ref(), module, manifest, loader_json)
            .compile_context_loader_json(&widget_out.plan)?;
        // 6.24.3 `GenerateLoaderJson.doTaskAction()` writes the widget loader
        // JSON alongside the common one (`aceWidgetLoaderJson`) whenever the
        // module submits ArkTS widgets.
        let widget_loader_path = ModuleBuildLayout::new(&module.src_path).widget_loader_json();
        let widget_loader_value = serde_json::to_value(&widget_loader_json)
            .map_err(|error| anyhow::anyhow!("serialize widget_loader.json failed: {}", error))?;
        intermediate_writer
          .write_json_pretty(&widget_loader_path, &widget_loader_value)
          .map_err(|error| anyhow::anyhow!("write widget_loader.json failed: {}", error))?;
        let widget_compile_context_info_path = if loader_json.use_normalized_ohm_url {
          let module_context = session
            .module_context(&module.name)
            .ok_or_else(|| anyhow::anyhow!("missing module compile context for {}", module.name))?;
          let pkg_context_input = PkgContextInfoInputBuilder::new(
            module_config.as_ref(),
            session,
            module,
            &module_context,
          )
          .excluding_package_roots(module_har_dedup_plan.duplicate_package_roots())
          .build()?;
          let pkg_context_info = PkgContextInfoGenerator::new()
            .generate(&pkg_context_input)
            .map_err(|e| anyhow::anyhow!("GeneratePkgContextInfo failed: {}", e))?;
          let path = widget_out.plan.cache_dir.join("compileContextInfo.json");
          CompileContextInfoWriter::new(
            &widget_loader_json,
            &widget_out.bundle_out.bundle_output.module_source_files,
            &pkg_context_info,
            &session.project.bundle_type,
            &config.product.bundle_name,
            true,
          )
          .write(&path)
          .map_err(|e| anyhow::anyhow!("GenerateWidgetCompileContextInfo failed: {}", e))?;
          Some(path)
        } else {
          None
        };
        BytecodeStage::run(BytecodeStageInput {
          compiler: RolldownBytecodeCompiler::new(
            abc_ts2abc_backend_inner,
            config.sdk_config.es2abc_path.clone(),
            compile_target.api_version(),
            matches!(config.build_mode, BuildMode::Debug),
          )
          .with_target_api_sub_version(compile_target.api_sub_version())
          .with_branch_elimination(branch_elimination)
          .with_transform_lib(transform_lib.clone()),
          module_source_files: widget_out
            .bundle_out
            .bundle_output
            .module_source_files
            .clone(),
          external_module_requests: widget_out
            .bundle_out
            .bundle_output
            .external_module_requests
            .clone(),
          npm_entries: widget_out.bundle_out.bundle_output.npm_entries.clone(),
          modules_abc_path: widget_out.plan.modules_abc_path.clone(),
          cache_dir: widget_out.plan.cache_dir.clone(),
          cache_file: Some(widget_out.plan.cache_file.clone()),
          abc_input_files: widget_abc_input_files,
          compile_context_info_path: widget_compile_context_info_path,
          byte_code_har: widget_loader_json.byte_code_har.unwrap_or(false),
        })?;
      }

      if bundle_out.bundle_output.source_maps_path.is_some() {
        let compile_source_maps_cache =
          PathBuf::from(&loader_json.cache_path).join("sourceMaps.json");
        intermediate_writer.commit_source_map_input_fingerprints(
          &compile_source_maps_cache,
          &bundle_out.bundle_output.source_map_input_fingerprints,
        )?;
      }

      for (_chunk_name, path) in &bundle_out.bundle_output.chunk_files {
        if path.extension().and_then(|e| e.to_str()) == Some("js") && path.exists() {
          match fs_err::remove_file(path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
              return Err(err).with_context(|| {
                format!("failed to remove Rolldown temp chunk {}", path.display())
              });
            }
          }
        }
      }

      let cache_dir = PathBuf::from(&loader_json.cache_path);
      let task_plugin_hash =
        module_package_plugins
          .iter()
          .fold(0xcbf29ce484222325u64, |mut hash, plugin| {
            for byte in plugin.cache_identity().bytes() {
              hash ^= u64::from(byte);
              hash = hash.wrapping_mul(0x100000001b3);
            }
            hash
          });
      let task_input_snapshot = TaskInputSnapshot::for_module(session.as_ref(), &module.name)
        .ok_or_else(|| {
          anyhow::anyhow!(
            "missing session input fingerprints for module '{}'",
            module.name
          )
        })?;
      let task_compile_env = CompileEnvKeyBuilder::new(
        module_config.as_ref(),
        module,
        loader_json,
        module_obfuscation.is_enabled(&config.build_mode),
        module_target_name,
      )
      .build()?;
      let task_inputs =
        TaskInputs::compute(module_config.as_ref(), loader_json, &task_input_snapshot)
          .with_module_analyzer(module_analyzer)
          .with_effective_obfuscation(module_obfuscation.config.as_ref(), &config.build_mode)
          .with_compile_env(task_compile_env)
          .with_plugin_hash(task_plugin_hash)
          .with_additional_obfuscation_rule_files(&module_obfuscation.dependency_rule_files)
          .with_custom_type_inputs(
            &loader_json.compiler_types,
            &loader_json.compiler_type_declaration_inputs,
          )?;
      if let Err(e) = task_inputs.save(&cache_dir) {
        tracing::warn!(error = %e, "failed to save task inputs cache");
      }

      let build_mode_str = match config.build_mode {
        BuildMode::Debug => "debug",
        BuildMode::Release => "release",
      };
      let compile_info_path = cache_dir.join("compileInfo.json");
      let compile_info = serde_json::json!({ "buildMode": build_mode_str });
      if let Err(e) = fs_err::write(
        &compile_info_path,
        serde_json::to_string(&compile_info).unwrap_or_default(),
      ) {
        tracing::warn!(
            path = %compile_info_path.display(),
            error = %e,
            "failed to write compileInfo.json"
        );
      }

      // ByteCode HAR validation (SPEC-041).
      if !bc_har_deps.is_empty() {
        for dep in bc_har_deps {
          if !dep.abc_path.exists() {
            anyhow::bail!(
              "ByteCode HAR '{}': ABC file not found at '{}'. \
                             Ensure the HAR was built with bytecode output enabled (SPEC-041).",
              dep.package_name,
              dep.abc_path.display()
            );
          }
          tracing::debug!(
              package = %dep.package_name,
              abc_path = %dep.abc_path.display(),
              "ByteCode HAR dep exists; loaded via byteCodeHarInfo for SDK es2abc (SPEC-041)"
          );
        }
      }

      if compile_task_active[bundle_index] {
        task_lifecycle
          .after(
            &module.name,
            Some(module_target_name),
            if matches!(module.module_type, OhosModuleType::Har) {
              "HarCompileArkTS"
            } else {
              "CompileArkTS"
            },
          )
          .await?;
      }
      task_lifecycle
        .up_to_date(&module.name, Some(module_target_name), "BuildJS")
        .await?;
    }
    progress.done();

    let bundle_package_outs: Vec<(BundlePackageOutput,)> = bundle_outs
      .into_iter()
      .map(|(bundle_out,)| (bundle_out.into_package_output(),))
      .collect();

    // ── Step 6: Packaging + Signing ───────────────────────────────────
    progress.step(6, 6, "Packaging");
    // Local unit-test builds produce no HAP/HSP/HAR archive — the `test`
    // task ends at the compiled assets (`intermediates/assets/<target>/ets/`)
    // plus coverage data. hvigor `Test`/`UnitTestBuild` tasks never run the
    // pack graph.
    if matches!(config.output_target, OutputTarget::Test) {
      return Ok(BuildOutput {
        artifact_paths: Vec::new(),
      });
    }
    let mut output_paths = Vec::new();

    for (
      (
        (
          (module, manifest, loader_json, _resource_refs, module_target_name, module_config),
          (package_out,),
        ),
        privacy_profile_path,
      ),
      module_obfuscation,
    ) in loader_jsons
      .iter()
      .zip(bundle_package_outs.iter())
      .zip(privacy_profile_paths.iter())
      .zip(module_obfuscation_contexts.iter())
    {
      let module_build_option = &module_config.product.build_option;
      let packager = Packager::new(module_config.clone()).with_compress_level(
        args
          .pack_compress_level
          .filter(|_| module_config.sdk_config.sdk_component_api_version >= 12)
          .unwrap_or(1),
      );
      let module_layout = ModuleBuildLayout::for_output_target(
        &config.output_target,
        &module.src_path,
        module_target_name,
      );
      let module_har_dedup_plan = har_dedup_project_plan.module(&module.name);
      if task_lifecycle
        .before(&module.name, Some(module_target_name), "MakePackInfo")
        .await?
      {
        task_lifecycle
          .after(&module.name, Some(module_target_name), "MakePackInfo")
          .await?;
      }
      let resources_index = module_layout.res_resources_index();
      let modules_abc = package_out.modules_abc_path.clone();
      let ext = match &config.output_target {
        OutputTarget::Hsp => "hsp",
        OutputTarget::Har => "har",
        _ => "hap",
      };
      // hvigor places non-default target archives under `outputs/<target>`
      // (e.g. `outputs/ohosTest/entry-ohosTest-unsigned.hap`).
      let output_dir = if module_target_name == "default" {
        module_layout.outputs_default()
      } else {
        module_layout
          .outputs_root()
          .join(&module_target_name)
      };

      let packaging_build_mode_name = match config.build_mode {
        BuildMode::Debug => "debug",
        BuildMode::Release => "release",
      };
      let all_applicable = session.build_options.applicable_targets(
        &module.name,
        &config.product.name,
        packaging_build_mode_name,
      );
      let target_labels = [module_target_name.clone()];

      std::fs::create_dir_all(&output_dir)?;

      for artifact_target_label in &target_labels {
        let interface_source_roots = all_applicable
          .iter()
          .find(|target| target.name == *artifact_target_label)
          .map(|target| target.source_roots.clone())
          .unwrap_or_default();
        let modules_abc = modules_abc.clone();
        let resources_index = resources_index.clone();
        let output_name = ArtifactOutputName::new(&module.name, artifact_target_label, ext);
        let unsigned_output = output_dir.join(output_name.unsigned());
        let signed_output = output_dir.join(output_name.signed());
        let output = if matches!(&config.output_target, OutputTarget::Har) {
          output_dir.join(output_name.har())
        } else {
          unsigned_output.clone()
        };
        let syscap_tool = session
          .sdk
          .tool_path(SdkToolKind::SyscapTool)
          .map(|tool| tool.path)
          .unwrap_or_else(|| session.sdk.sdk_root.join("toolchains/syscap_tool"));
        let syscap_enabled = task_lifecycle
          .before(&module.name, Some(artifact_target_label), "SyscapTransform")
          .await?;
        let syscap_path = if syscap_enabled {
          let syscap_path = SyscapTransform::new(
            &config.product.runtime_os,
            &module.name,
            &module.src_path,
            &manifest.device_types,
            config.sdk_config.compile_api_version,
            &session.sdk.sdk_root,
            &syscap_tool,
            module_layout.syscap_dir(),
            matches!(config.output_target, OutputTarget::Har),
          )
          .run()
          .map_err(|error| anyhow::anyhow!("SyscapTransform failed: {error}"))?;
          task_lifecycle
            .after(&module.name, Some(artifact_target_label), "SyscapTransform")
            .await?;
          syscap_path
        } else {
          None
        };

        let module_context = session.module_context(&module.name).ok_or_else(|| {
          anyhow::anyhow!("missing build session module context for {}", module.name)
        })?;
        let bundle_version_code = session.project.version_code;
        let bundle_version_name = session.project.version_name.clone();
        let bundle_build_version = session.project.build_version.clone();
        let bundle_type = session.project.bundle_type.clone();
        let integrated_hsp_enabled = config
          .product
          .build_option
          .ark_options
          .as_ref()
          .and_then(|options| options.integrated_hsp)
          == Some(true);
        let integrated_hsp_candidates: Vec<IntegratedHspCandidate> = module_context
          .deps
          .integrated_hsp_nodes()
          .filter_map(|node| {
            Some(IntegratedHspCandidate {
              alias: node.alias.clone(),
              package_root: node.package_root.clone()?,
              version: node
                .package
                .as_ref()
                .map(|package| package.version.clone())
                .filter(|version| !version.trim().is_empty())
                .unwrap_or_else(|| "1.0.0".to_string()),
            })
          })
          .collect();
        let integrated_hsp_start = Instant::now();
        let process_integrated_hsp = task_lifecycle
          .before(
            &module.name,
            Some(artifact_target_label),
            "ProcessIntegratedHsp",
          )
          .await?;
        let integrated_hsps = if process_integrated_hsp {
          BuildPolicy::new(config.as_ref()).validate_integrated_hsp_api(
            integrated_hsp_enabled && !integrated_hsp_candidates.is_empty(),
          )?;
          let integrated_hsps = IntegratedHspCollector::new(&integrated_hsp_candidates)
            .with_enabled(integrated_hsp_enabled)
            .collect_normalized(
              &config.product.bundle_name,
              bundle_version_code,
              bundle_build_version.as_deref(),
              &module_layout.integrated_hsp_dir(),
            )
            .map_err(|error| anyhow::anyhow!("K5: failed to normalize integrated HSPs: {error}"))?;
          task_lifecycle
            .after(
              &module.name,
              Some(artifact_target_label),
              "ProcessIntegratedHsp",
            )
            .await?;
          integrated_hsps
        } else {
          Vec::new()
        };
        tracing::info!(
          module = %module.name,
          elapsed_ms = integrated_hsp_start.elapsed().as_millis(),
          "integrated hsp collection completed"
        );

        let native_build_option =
          arkdown_config::build_profile::EffectiveNativeBuildOption::from_build_option(
            module_build_option,
          );
        let native_header_dirs = native_build_option.native_lib.header_paths.clone();
        let exclude_so_from_interface_har =
          native_build_option.native_lib.exclude_so_from_interface_har;
        let self_package = module_context
          .module
          .package
          .as_ref()
          .map(|package| NativeLibPackageInfo::local(package.name.clone(), package.version.clone()))
          .unwrap_or_else(|| NativeLibPackageInfo::local(module.name.clone(), String::new()));
        let native_policy = sanitizer_native_policy
          .clone()
          .unwrap_or_else(|| {
            NativeLibCollectionPolicy::default()
              .with_build_mode(args.mode.clone())
              .with_sdk_toolchain(native_sdk_toolchain.clone())
          })
          .with_external_native_options(native_build_option.external_native_options.clone())
          .with_native_lib_option(native_build_option.native_lib.clone())
          .with_self_package(self_package);
        let native_collector = NativeLibCollector::new(
          module,
          &native_build_option.abi_filters,
          module_context.deps.as_ref(),
        )
        .with_policy(native_policy);
        let native_collector = if matches!(config.output_target, OutputTarget::Har) {
          native_collector
        } else {
          native_collector
            .excluding_dependency_roots(module_har_dedup_plan.duplicate_package_roots())
        };
        if task_lifecycle
          .before(
            &module.name,
            Some(artifact_target_label),
            "BuildNativeWithCmake",
          )
          .await?
        {
          native_collector
            .configure_native()
            .map_err(|error| anyhow::anyhow!("BuildNativeWithCmake failed: {error}"))?;
          task_lifecycle
            .after(
              &module.name,
              Some(artifact_target_label),
              "BuildNativeWithCmake",
            )
            .await?;
        }
        // hvigor 6.24.3 CompileNativeHook is a targetless module hook whose
        // only action is the dependency edge to target@BuildNativeWithCmake.
        task_lifecycle
          .marker(&module.name, None, "compileNative")
          .await?;
        if task_lifecycle
          .before(
            &module.name,
            Some(artifact_target_label),
            "BuildNativeWithNinja",
          )
          .await?
        {
          native_collector
            .build_native()
            .map_err(|error| anyhow::anyhow!("BuildNativeWithNinja failed: {error}"))?;
          task_lifecycle
            .after(
              &module.name,
              Some(artifact_target_label),
              "BuildNativeWithNinja",
            )
            .await?;
        }
        let native_lib_start = Instant::now();
        let process_libs = task_lifecycle
          .before(&module.name, Some(artifact_target_label), "ProcessLibs")
          .await?;
        let processed_native_libs = if process_libs {
          let processed = native_collector
            .process_libs()
            .map_err(|error| anyhow::anyhow!("ProcessLibs failed: {error}"))?;
          task_lifecycle
            .after(&module.name, Some(artifact_target_label), "ProcessLibs")
            .await?;
          processed
        } else {
          Vec::new()
        };
        // DoNativeBinxo: when OHOS_ENABLE_HWASAN + OHOS_ENABLE_BINXO are
        // configured, binxo outputs replace the process-libs inputs for strip.
        let binxo_libs = native_collector
          .binxo_libs(&processed_native_libs)
          .map_err(|error| anyhow::anyhow!("DoNativeBinxo failed: {error}"))?;
        let native_libs = if task_lifecycle
          .before(&module.name, Some(artifact_target_label), "DoNativeStrip")
          .await?
        {
          let stripped = native_collector
            .strip_libs(&binxo_libs)
            .map_err(|error| anyhow::anyhow!("DoNativeStrip failed: {error}"))?;
          task_lifecycle
            .after(&module.name, Some(artifact_target_label), "DoNativeStrip")
            .await?;
          stripped
        } else {
          Vec::new()
        };
        if task_lifecycle
          .before(&module.name, Some(artifact_target_label), "CacheNativeLibs")
          .await?
        {
          native_collector
            .cache_libs()
            .map_err(|error| anyhow::anyhow!("CacheNativeLibs failed: {error}"))?;
          native_collector
            .validate_libs(&native_libs)
            .map_err(|error| anyhow::anyhow!("CacheNativeLibs validation failed: {error}"))?;
          task_lifecycle
            .after(&module.name, Some(artifact_target_label), "CacheNativeLibs")
            .await?;
        }
        tracing::info!(
          module = %module.name,
          elapsed_ms = native_lib_start.elapsed().as_millis(),
          native_lib_count = native_libs.len(),
          "native libs collected"
        );
        let interface_native_libs = native_libs.clone();

        let res_module_json = module_layout.res_module_json();
        let package_module_json = module_layout.package_module_json();
        let preload_system_so_effective =
          PreloadSystemSoArtifactWriter::effective_path(&module_layout.preload_system_so_json())?
            .is_some();
        let module_json = if matches!(config.output_target, OutputTarget::Har) {
          res_module_json.clone()
        } else if task_lifecycle
          .before(
            &module.name,
            Some(artifact_target_label),
            "GeneratePkgModuleJson",
          )
          .await?
        {
          let generated = PackageModuleJsonOptions::new(&res_module_json).write_for_pack(
            &package_module_json,
            (module_build_option.deduplicate_har
              && matches!(module.module_type, OhosModuleType::Shared))
            .then_some(!module_har_dedup_plan.is_empty()),
            preload_system_so_effective,
          )?;
          task_lifecycle
            .after(
              &module.name,
              Some(artifact_target_label),
              "GeneratePkgModuleJson",
            )
            .await?;
          generated
        } else {
          package_module_json
        };

        if matches!(config.output_target, OutputTarget::Hap)
          && task_lifecycle
            .before(
              &module.name,
              Some(artifact_target_label),
              "ProcessCompiledResources",
            )
            .await?
        {
          PreloadSystemSoArtifactWriter::new(
            &module_layout.preload_system_so_json(),
            &module_layout.res_profile_dir(),
            &res_module_json,
            &module.module_type,
            &package_out.preload_system_so_tasks,
          )
          .process_compiled_resources()?;
          task_lifecycle
            .after(
              &module.name,
              Some(artifact_target_label),
              "ProcessCompiledResources",
            )
            .await?;
        }

        let package_task_name = match &config.output_target {
          OutputTarget::Har => "PackageHar",
          OutputTarget::Hsp => "PackageHsp",
          _ => "PackageHap",
        };
        if matches!(config.output_target, OutputTarget::Har) {
          for har_task_name in ["ProcessObfuscationFiles"] {
            task_lifecycle
              .marker(&module.name, Some(artifact_target_label), har_task_name)
              .await?;
          }
        }
        let package_enabled = task_lifecycle
          .before(&module.name, Some(artifact_target_label), package_task_name)
          .await?;
        let out = if package_enabled {
          let out = match &config.output_target {
            OutputTarget::Har => {
              let source_dir = module_layout.source().ets();
              let oh_package_json = module_layout.oh_package_json5();
              if !source_dir.is_dir() {
                anyhow::bail!(
                  "HAR source directory does not exist for module '{}': {}",
                  module.name,
                  source_dir.display()
                );
              }
              let artifact_type = module_build_option.har_artifact_type;
              let declarations_dir = module_layout.declarations_dir();
              if !matches!(artifact_type, HarArtifactType::Original) {
                let generated_module_info =
                  module_layout.generated_module_info(&module_target_name);
                let declaration_generator =
                  HarDeclarationGenerator::new_har(&module.src_path, &source_dir, &module.name);
                let declaration_generator = if matches!(artifact_type, HarArtifactType::Bytecode) {
                  declaration_generator.with_generated_source(&generated_module_info)
                } else {
                  declaration_generator
                };
                declaration_generator
                  .generate()
                  .map_err(|error| anyhow::anyhow!("generate HAR declarations failed: {error}"))?;
              }

              let declaration_entries = if matches!(artifact_type, HarArtifactType::Bytecode) {
                if !loader_json.use_normalized_ohm_url {
                  anyhow::bail!(
                    "ByteCode HAR '{}' requires strictMode.useNormalizedOHMUrl=true",
                    module.name
                  );
                }
                let pkg_context_input = PkgContextInfoInputBuilder::new(
                  config.as_ref(),
                  session,
                  module,
                  &module_context,
                )
                .build()?;
                let pkg_context_info = PkgContextInfoGenerator::new()
                  .generate(&pkg_context_input)
                  .map_err(|error| anyhow::anyhow!("GeneratePkgContextInfo failed: {error}"))?;
                ByteCodeHarDeclarationCollector::new(
                  config.as_ref(),
                  module,
                  manifest,
                  &pkg_context_input,
                  &pkg_context_info,
                )
                .collect()?
                .ohm_urls
              } else {
                Vec::new()
              };
              let bundled_dependencies = module_build_option
                .ark_options
                .as_ref()
                .is_some_and(|options| options.bundled_dependencies);
              let har_importees = HarImporteeCache::new(&loader_json.cache_path).read()?;
              let runtime_only_packages = module_build_option
                .ark_options
                .as_ref()
                .and_then(|options| options.runtime_only.as_ref())
                .map(|runtime_only| runtime_only.packages.as_slice())
                .unwrap_or_default();
              let importee_metadata = module_context.deps.har_importee_metadata(
                &har_importees.non_entry,
                &har_importees.compiled,
                &har_importees.direct,
                bundled_dependencies,
                args.byte_code_har_integrated_optimization,
                runtime_only_packages,
              );
              let compiled_package_names = importee_metadata
                .compiled_package_names
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>();
              let affects_compilation_dependency_keys = importee_metadata
                .affects_compilation_dependency_keys
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>();
              let dependency_pkg_version = module_context.deps.bytecode_har_dependency_versions(
                bundled_dependencies,
                args.byte_code_har_integrated_optimization,
                &compiled_package_names,
                &affects_compilation_dependency_keys,
              )?;
              let package_parameters = module_context
                .module
                .package
                .as_deref()
                .map(|package| {
                  module_context
                    .deps
                    .dependency_parameters
                    .resolve_package_parameters(&package.package_json)
                })
                .transpose()?
                .map(|resolved| HarPackageParameterInput {
                  version: (!resolved.version.is_empty()).then_some(resolved.version),
                  dependencies: resolved.dependencies,
                  dev_dependencies: resolved.dev_dependencies,
                  dynamic_dependencies: resolved.dynamic_dependencies,
                })
                .unwrap_or_default();
              let bundled_dependency_rewrite = if matches!(artifact_type, HarArtifactType::Bytecode)
                && bundled_dependencies
                && !args.keep_dependency
              {
                module_context
                  .module
                  .package
                  .as_deref()
                  .map(|package| {
                    module_context
                      .deps
                      .bundled_har_retained_dependencies(&package.package_json)
                  })
                  .transpose()?
                  .map(|retained| HarBundledDependencyInput {
                    dependencies: retained.dependencies,
                    dynamic_dependencies: retained.dynamic_dependencies,
                  })
              } else {
                None
              };
              // `har-target-util.js::isNeedExclude`: `resOptions.excludeHarRes`
              // drops remote HAR resources (local `file:` dependencies and
              // HAR-host modules are never excluded).
              let exclude_har_res = module_build_option
                .res_options
                .as_ref()
                .map(|options| options.exclude_har_res.as_slice())
                .unwrap_or_default();
              let bundled_resource_snapshots = if bundled_dependencies {
                module_context
                  .deps
                  .bundled_har_resources()
                  .into_iter()
                  .filter(|resource| {
                    let is_local = module_context
                      .deps
                      .nodes
                      .get(&resource.package_name)
                      .and_then(|node| node.version_spec.as_deref())
                      .is_some_and(|spec| spec.starts_with("file:"));
                    !(exclude_har_res.contains(&resource.package_name) && !is_local)
                  })
                  .collect::<Vec<_>>()
              } else {
                Vec::new()
              };
              let bundled_resources = bundled_resource_snapshots
                .iter()
                .map(|resource| BundledHarResourceInput {
                  source_dir: resource.source_dir.clone(),
                  archive_directory: resource.archive_directory.clone(),
                })
                .collect::<Vec<_>>();
              let mut bundled_resource_directories = Vec::new();
              if bundled_dependencies {
                let effective_target = ModuleTargetSelection::resolve(
                  config.as_ref(),
                  session.build_options.as_ref(),
                  module,
                  Some(artifact_target_label),
                )
                .target;
                if effective_target.resource_directories.is_empty() {
                  bundled_resource_directories
                    .push(arkdown_path::ArtifactPath::from_slash("src/main/resources"));
                } else {
                  for directory in &effective_target.resource_directories {
                    bundled_resource_directories.push(
                      ArtifactPathSerializer::path_to_slash(directory).map_err(|error| {
                        anyhow::anyhow!(
                          "invalid HAR target resource directory '{}': {error}",
                          directory.display()
                        )
                      })?,
                    );
                  }
                }
                bundled_resource_directories.extend(
                  bundled_resource_snapshots
                    .iter()
                    .map(|resource| resource.metadata_directory.clone()),
                );
              }
              let resource_table = module_layout.resource_table_txt();
              let processed_module_json = module_layout.process_profile_module_json();
              let source_maps = package_out.source_maps_path.clone().or_else(|| {
                let candidate = module_layout.loader_out_source_maps();
                candidate.is_file().then_some(candidate)
              });
              let consumer_rules_path = project_layout.obfuscation_txt();
              let ignore_resource_patterns = config
                .product
                .build_option
                .res_options
                .as_ref()
                .map(|options| options.ignore_resource_pattern.clone())
                .unwrap_or_default();
              let preload_system_so_json = PreloadSystemSoArtifactWriter::effective_path(
                &module_layout.preload_system_so_json(),
              )?;

              let har_pack_input = HarPackInput {
                module_name: module.name.clone(),
                source_dir: source_dir.clone(),
                resources_dir: module_layout.source().resources(),
                resource_str_dir: {
                  let dir = module_layout.res_resource_str_dir();
                  dir.exists().then_some(dir)
                },
                oh_package_json: oh_package_json.clone(),
                output_path: output.clone(),
                declarations_dir: declarations_dir.exists().then_some(declarations_dir),
                consumer_rules_path: consumer_rules_path.exists().then_some(consumer_rules_path),
              };
              let har_variant = HarVariantInput {
                artifact_type,
                module_root: module.src_path.clone(),
                compiled_output_dir: module_layout.loader_out_dir(),
                processed_module_json: Some(processed_module_json),
                resource_table_path: resource_table.exists().then_some(resource_table),
                native_libs,
                native_header_dirs,
                preload_system_so_json,
                source_maps_path: source_maps,
                build_profile_path: Some(
                  module_layout
                    .build_profile_for_module(&module.module_type, artifact_target_label),
                ),
                declaration_entries,
                package_parameters,
                worker_entries: module_build_option
                  .source_option
                  .as_ref()
                  .map(|options| options.workers.clone())
                  .unwrap_or_default(),
                runtime_only: module_build_option
                  .ark_options
                  .as_ref()
                  .and_then(|options| options.runtime_only.clone()),
                resource_directories: module_build_option
                  .res_options
                  .as_ref()
                  .map(|options| options.resource_directories.clone())
                  .unwrap_or_default(),
                bundled_dependency_rewrite,
                dependency_pkg_version,
                bundled_dependencies,
                bundled_resources,
                bundled_resource_directories,
                compiled_package_names: importee_metadata.compiled_package_names,
                extra_importees: importee_metadata.extra_importees,
                global_dependencies: importee_metadata.global_dependencies,
                ignore_resource_patterns,
                debug: matches!(config.build_mode, BuildMode::Debug),
                target_name: artifact_target_label.clone(),
                package_cache_dir: Some(module_layout.package_har_cache_dir(artifact_target_label)),
              };
              if task_lifecycle
                .before(
                  &module.name,
                  Some(artifact_target_label),
                  "ProcessOHPackageJson",
                )
                .await?
              {
                packager
                  .process_har_package_json(
                    &har_pack_input,
                    &har_variant,
                    &module_layout.generated_pm_oh_package(artifact_target_label),
                  )
                  .map_err(|error| anyhow::anyhow!("ProcessOHPackageJson failed: {error}"))?;
                task_lifecycle
                  .after(
                    &module.name,
                    Some(artifact_target_label),
                    "ProcessOHPackageJson",
                  )
                  .await?;
              }
              task_lifecycle
                .marker(
                  &module.name,
                  Some(artifact_target_label),
                  "ProcessHarArtifacts",
                )
                .await?;
              let packed = packager
                .pack_har_variant(har_pack_input, har_variant)
                .map_err(|e| anyhow::anyhow!("Pack failed: {}", e))?;
              Some(packed)
            }
            _ => {
              if module_json.exists() && modules_abc.exists() {
                if !resources_index.exists() {
                  if let Some(parent) = resources_index.parent() {
                    std::fs::create_dir_all(parent)?;
                  }
                  std::fs::write(&resources_index, [])?;
                }
                {
                  let raw_source_maps_path = package_out
                    .source_maps_path
                    .clone()
                    .filter(|path| path.is_file())
                    .or_else(|| {
                      let candidate = module_layout.loader_out_source_maps();
                      candidate.is_file().then_some(candidate)
                    });
                  let source_maps_path = if matches!(config.build_mode, BuildMode::Release) {
                    None
                  } else {
                    raw_source_maps_path.clone()
                  };
                  let abilities: Vec<PackAbility> = manifest
                    .abilities
                    .iter()
                    .map(|a| PackAbility {
                      name: a.name.clone(),
                      label: a.label.clone(),
                    })
                    .collect();
                  let delivery_with_install = module_context
                    .module
                    .module_json
                    .as_ref()
                    .and_then(|module_json| module_json.module.delivery_with_install)
                    .unwrap_or(true);
                  let compress_native_libs =
                    PackageModuleJsonOptions::new(&module_json).compress_native_libs()?;
                  let pkg_sdk_info_path = (!loader_json.use_normalized_ohm_url)
                    .then(|| module_layout.pkg_sdk_info_json())
                    .filter(|path| path.is_file());
                  let pack_hap_start = Instant::now();
                  #[cfg(feature = "memory-diagnostics")]
                  arkdown::memory_diagnostics::dump_with(
                    "build:before-pack-hap",
                    format_args!(
                      "module={} native_libs={} source_maps={}",
                      module.name,
                      native_libs.len(),
                      source_maps_path.is_some()
                    ),
                  );
                  let pack_input = PackInput {
                    module_name: module.name.clone(),
                    abc_path: modules_abc,
                    resources_index,
                    module_json,
                    pack_info_module_json: Some(module_layout.merge_profile_module_json()),
                    native_libs,
                    compress_native_libs,
                    output_path: output.clone(),
                    resources_src_dir: Some(module_layout.res_resources_dir()),
                    bundle_version_code,
                    bundle_version_name,
                    bundle_build_version,
                    bundle_type: bundle_type.clone(),
                    source_maps_path,
                    pkg_context_info_path: loader_json
                      .use_normalized_ohm_url
                      .then(|| module_layout.pkg_context_info_json()),
                    pkg_sdk_info_path,
                    abilities,
                    integrated_hsps,
                    device_types: manifest.device_types.clone(),
                    installation_free: bundle_type == "atomicService",
                    delivery_with_install,
                    privacy_profile_path: privacy_profile_path.clone(),
                    syscap_path,
                  };
                  let packed = if matches!(config.output_target, OutputTarget::Hsp) {
                    packager.pack_hsp(pack_input)
                  } else {
                    packager.pack_hap(pack_input)
                  }
                  .map_err(|e| anyhow::anyhow!("Pack failed: {}", e))?;

                  if matches!(config.output_target, OutputTarget::Hsp) {
                    let prepare_shared_har = task_lifecycle
                      .before(
                        &module.name,
                        Some(artifact_target_label),
                        "PrepareSharedHarResource",
                      )
                      .await?;
                    let mut interface_dependencies = BTreeMap::new();
                    let mut interface_dynamic_dependencies = BTreeMap::new();
                    for dependency in module_context.deps.nodes.values().filter(|dependency| {
                      !dependency.is_transitive
                        && matches!(
                          dependency.node_type,
                          DependencyNodeType::SharedHsp | DependencyNodeType::IntegratedHsp
                        )
                    }) {
                      let version = dependency.version_spec.clone().or_else(|| {
                        dependency
                          .package
                          .as_ref()
                          .map(|package| package.version.clone())
                      });
                      if let Some(version) = version {
                        let target = if dependency.is_dynamic {
                          &mut interface_dynamic_dependencies
                        } else {
                          &mut interface_dependencies
                        };
                        target.insert(dependency.alias.clone(), version);
                      }
                    }
                    let resource_table_path = module_layout.resource_table_txt();
                    let resource_str_dir = module_layout.res_resource_str_dir();
                    let obfuscation_rules_path = project_layout.obfuscation_txt();
                    let interface_input = HspInterfaceHarInput {
                      module_name: module.name.clone(),
                      module_root: module.src_path.clone(),
                      target_name: artifact_target_label.clone(),
                      oh_package_json: module_layout.oh_package_json5(),
                      processed_module_json: module_layout.process_profile_module_json(),
                      declarations_dir: module_layout.declarations_dir(),
                      resource_table_path: resource_table_path
                        .is_file()
                        .then_some(resource_table_path),
                      resource_str_dir: resource_str_dir.is_dir().then_some(resource_str_dir),
                      privacy_profile_path: privacy_profile_path.clone(),
                      obfuscation_rules_path: obfuscation_rules_path
                        .is_file()
                        .then_some(obfuscation_rules_path),
                      native_libs: interface_native_libs,
                      native_header_dirs,
                      exclude_so_from_interface_har,
                      output_path: output_dir.join(format!("{}.har", module.name)),
                      compatible_sdk_version: parse_api_version(
                        &config.product.compatible_sdk_version,
                      )
                      .unwrap_or(config.sdk_config.compile_api_version),
                      compatible_sdk_type: config.product.runtime_os.clone(),
                      use_normalized_ohm_url: config
                        .product
                        .build_option
                        .strict_mode
                        .use_normalized_ohm_url,
                      integrated_hsp: config
                        .product
                        .build_option
                        .ark_options
                        .as_ref()
                        .and_then(|options| options.integrated_hsp),
                      source_roots: interface_source_roots,
                      dependencies: interface_dependencies,
                      dynamic_dependencies: interface_dynamic_dependencies,
                      debug: matches!(config.build_mode, BuildMode::Debug),
                      obfuscated: module_obfuscation.is_enabled(&config.build_mode),
                    };
                    if prepare_shared_har {
                      task_lifecycle
                        .after(
                          &module.name,
                          Some(artifact_target_label),
                          "PrepareSharedHarResource",
                        )
                        .await?;
                    }
                    if task_lifecycle
                      .before(
                        &module.name,
                        Some(artifact_target_label),
                        "PackageSharedHar",
                      )
                      .await?
                    {
                      packager
                        .pack_hsp_interface_har(interface_input)
                        .map_err(|e| anyhow::anyhow!("PackageSharedHar failed: {}", e))?;
                      task_lifecycle
                        .after(
                          &module.name,
                          Some(artifact_target_label),
                          "PackageSharedHar",
                        )
                        .await?;
                    }
                  }
                  tracing::info!(
                    module = %module.name,
                    elapsed_ms = pack_hap_start.elapsed().as_millis(),
                    output = %packed.display(),
                    "hap packed"
                  );
                  #[cfg(feature = "memory-diagnostics")]
                  arkdown::memory_diagnostics::dump_with(
                    "build:after-pack-hap",
                    format_args!("module={} output={}", module.name, packed.display()),
                  );
                  Some(packed)
                }
              } else {
                None
              }
            }
          };
          task_lifecycle
            .after(&module.name, Some(artifact_target_label), package_task_name)
            .await?;
          out
        } else {
          None
        };

        if let Some(path) = out {
          let packing_check_task = if matches!(config.output_target, OutputTarget::Har) {
            "PackingCheckHar"
          } else {
            "PackingCheck"
          };
          if task_lifecycle
            .before(
              &module.name,
              Some(artifact_target_label),
              packing_check_task,
            )
            .await?
          {
            // hvigor `packing-check.js`: `.so` extension-ability srcEntry
            // existence per ABI + stray-source-file packaging warning.
            let packing_check = PackingCheck::new(
              config,
              module,
              loader_json,
              module_layout.intermediates().join("stripped_native_libs"),
            );
            packing_check
              .validate()
              .map_err(|error| anyhow::anyhow!("packing-check failed: {error}"))?;
            task_lifecycle
              .after(
                &module.name,
                Some(artifact_target_label),
                packing_check_task,
              )
              .await?;
          }
          let is_package_signed = !matches!(config.output_target, OutputTarget::Har)
            && config.product.signing_config.is_some();
          let sign_task_name = match config.output_target {
            OutputTarget::Har => "PackageSignHar",
            OutputTarget::Hsp => "SignHsp",
            _ => "SignHap",
          };
          let sign_enabled = task_lifecycle
            .before(&module.name, Some(artifact_target_label), sign_task_name)
            .await?;
          let final_path = if sign_enabled && is_package_signed {
            let sign_backend = SignBackendRunner::rust();
            let sign_start = Instant::now();
            #[cfg(feature = "memory-diagnostics")]
            arkdown::memory_diagnostics::dump_with(
              "build:before-sign",
              format_args!("module={} input={}", module.name, path.display()),
            );
            sign_backend
              .sign_or_skip(
                &path,
                &signed_output,
                config.product.signing_config.as_ref(),
              )
              .map_err(|e| anyhow::anyhow!("Sign failed: {}", e))?;
            tracing::info!(
              module = %module.name,
              elapsed_ms = sign_start.elapsed().as_millis(),
              output = %signed_output.display(),
              "artifact signed"
            );
            #[cfg(feature = "memory-diagnostics")]
            arkdown::memory_diagnostics::dump_with(
              "build:after-sign",
              format_args!("module={} output={}", module.name, signed_output.display()),
            );
            signed_output.clone()
          } else {
            tracing::info!(
              module = %module.name,
              output = %path.display(),
              "artifact remains unsigned"
            );
            path.clone()
          };
          if sign_enabled {
            task_lifecycle
              .after(&module.name, Some(artifact_target_label), sign_task_name)
              .await?;
          }
          if !matches!(config.output_target, OutputTarget::Har) {
            let package_metadata_start = Instant::now();
            let package_module_json = module_layout.package_module_json();
            intermediate_writer.extract_zip_entry(&path, "module.json", &package_module_json)?;
            intermediate_writer.rewrite_package_intermediate_module_json(&package_module_json)?;
            let artifact_name = final_path
              .file_name()
              .and_then(|name| name.to_str())
              .context("package output artifact name is not valid UTF-8")?;
            intermediate_writer.write_package_metadata(
              &module.src_path,
              &config.output_target,
              artifact_name,
              &manifest.device_types,
              is_package_signed,
            )?;
            intermediate_writer.extract_zip_entry(
              &path,
              "pack.info",
              &module_layout.output_pack_info(),
            )?;
            tracing::info!(
              module = %module.name,
              elapsed_ms = package_metadata_start.elapsed().as_millis(),
              "package intermediate metadata written"
            );
          }
          intermediate_writer.publish_package_source_maps(
            package_out.source_maps_path.as_deref(),
            &module_layout.output_mapping_source_maps(),
          )?;
          if task_lifecycle
            .before(
              &module.name,
              Some(artifact_target_label),
              "CollectDebugSymbol",
            )
            .await?
          {
            let source_map_primary = package_out
              .source_maps_path
              .clone()
              .unwrap_or_else(|| module_layout.loader_out_source_maps());
            DebugSymbolCollector::new(
              module.src_path.clone(),
              module.name.clone(),
              config.build_mode.clone(),
              source_map_primary,
              None,
              PathBuf::from(&loader_json.cache_path)
                .join("obfuscation")
                .join("nameCache.json"),
            )
            .with_collection_override(args.collect_debug_symbol)
            .with_output_variant(&config.product.name, artifact_target_label)
            .collect()
            .map_err(|error| anyhow::anyhow!("CollectDebugSymbol failed: {error}"))?;
            task_lifecycle
              .after(
                &module.name,
                Some(artifact_target_label),
                "CollectDebugSymbol",
              )
              .await?;
          }
          if matches!(config.output_target, OutputTarget::Hsp) {
            let shared_tgz_output = output_dir.join(output_name.shared_tgz());
            if module_build_option.generate_shared_tgz.is_some() && shared_tgz_output.is_file() {
              std::fs::remove_file(&shared_tgz_output)?;
            }
            if task_lifecycle
              .before(
                &module.name,
                Some(artifact_target_label),
                "PackageSharedTgz",
              )
              .await?
            {
              let should_generate_shared_tgz = module_build_option
                .generate_shared_tgz
                .unwrap_or(matches!(config.build_mode, BuildMode::Release));
              if should_generate_shared_tgz {
                packager
                  .pack_hsp_tgz(HspTgzInput {
                    hsp_path: path.clone(),
                    interface_har_path: output_dir.join(format!("{}.har", module.name)),
                    output_path: shared_tgz_output,
                  })
                  .map_err(|error| anyhow::anyhow!("PackageSharedTgz failed: {error}"))?;
              }
              task_lifecycle
                .after(
                  &module.name,
                  Some(artifact_target_label),
                  "PackageSharedTgz",
                )
                .await?;
            }
          }
          output_paths.push(final_path);

          let assemble_task_name = match config.output_target {
            OutputTarget::Har => "assembleHar",
            OutputTarget::Hsp => "assembleHsp",
            _ => "assembleHap",
          };
          if task_lifecycle
            .before(&module.name, None, assemble_task_name)
            .await?
          {
            task_lifecycle
              .after(&module.name, None, assemble_task_name)
              .await?;
          }
        }
      }
    }
    progress.done();

    let elapsed = start.elapsed();
    if output_paths.is_empty() {
      tracing::info!(
        elapsed_seconds = elapsed.as_secs_f32(),
        "build succeeded with no output files produced"
      );
    } else {
      for path in &output_paths {
        tracing::info!(
            artifact = %path.display(),
            elapsed_seconds = elapsed.as_secs_f32(),
            "build succeeded"
        );
      }
    }

    if self.owns_task_cycle {
      task_lifecycle.finish_cycle().await?;
    }

    // ── Watch mode ────────────────────────────────────────────────────
    if watch_mode {
      WatchStage::run(WatchStageInput {
        config: config.clone(),
        session: session.clone(),
        target: args.target.clone(),
        mode: args.mode.clone(),
        module_filter: args.module.clone(),
        product_filter: args.product.clone(),
        project_root: project_root.clone(),
        abc_backend,
        build_target_name: args.build_target_name.clone(),
        build_version: args.build_version.clone(),
        coverage: coverage_mode,
        ohos_debug_asan: args.ohos_debug_asan,
        ohos_enable_hwasan: args.ohos_enable_hwasan,
        disable_abc_cache,
        cache_file: cli_cache_file.clone(),
        checker: args.checker.clone(),
        module_analyzer: args.module_analyzer,
        cache: args.cache,
        package_plugins: package_plugins.clone(),
        project_hvigor_overrides: args.project_hvigor_overrides.clone(),
        module_hvigor_overrides: args.module_hvigor_overrides.clone(),
        node_executable: args.node_executable.clone(),
        runtime_context_overrides: args.runtime_context_overrides.clone(),
        custom_task_input_paths: args.custom_task_input_paths.clone(),
        pack_compress_level: args.pack_compress_level,
        align_device_types: args.align_device_types.clone(),
        collect_debug_symbol: args.collect_debug_symbol,
        ui_transform_optimization: args.ui_transform_optimization,
        integrated_hsp_use_get_current_bundle_name: args.integrated_hsp_use_get_current_bundle_name,
        byte_code_har_integrated_optimization: args.byte_code_har_integrated_optimization,
        keep_dependency: args.keep_dependency,
        parameter_file_override: args.parameter_file_override.clone(),
        task_observer: self.task_observer.clone(),
        preview_mode,
      })
      .await?;
    }

    Ok(BuildOutput {
      artifact_paths: output_paths,
    })
  }
}

// ---------------------------------------------------------------------------
// BuildPolicy — owns build-mode validation helpers (same as in runner.rs)
// ---------------------------------------------------------------------------

struct BuildPolicy<'a> {
  config: &'a ArkdownConfig,
}

impl<'a> BuildPolicy<'a> {
  fn new(config: &'a ArkdownConfig) -> Self {
    Self { config }
  }

  fn uses_sdk_permission_validation(&self) -> bool {
    self.config.sdk_config.compile_api_version >= 12
      || self
        .config
        .product
        .runtime_os
        .eq_ignore_ascii_case("HarmonyOS")
  }

  fn module_processes_privacy_profile(module_type: &arkdown_core::OhosModuleType) -> bool {
    matches!(
      module_type,
      arkdown_core::OhosModuleType::Shared | arkdown_core::OhosModuleType::Har
    )
  }

  /// Match hvigor's HAR task graph: an original source HAR is linted and
  /// packaged directly, while obfuscated and bytecode HAR variants depend on
  /// HarCompileArkTS/CompileArkTS and es2abc.
  fn module_requires_arkts_compilation(&self, module: &ModuleConfig) -> Result<bool> {
    if !matches!(self.config.output_target, OutputTarget::Har)
      || !matches!(module.module_type, OhosModuleType::Har)
    {
      return Ok(true);
    }

    Ok(!matches!(
      self.config.product.build_option.har_artifact_type,
      HarArtifactType::Original
    ))
  }

  fn validate_integrated_hsp_api(&self, has_integrated_hsp: bool) -> Result<()> {
    if !has_integrated_hsp {
      return Ok(());
    }
    let compatible_api = parse_api_version(&self.config.product.compatible_sdk_version)
      .unwrap_or(self.config.sdk_config.compile_api_version);
    if self.config.sdk_config.compile_api_version <= 11 || compatible_api <= 11 {
      return Err(anyhow::anyhow!(
        "INTEGRATED_HSP_UNSUPPORTED_API: integrated HSP requires compile \
                 and compatible API greater than 11"
      ));
    }
    Ok(())
  }
}

// ---------------------------------------------------------------------------
// ResourceDirSelector — same logic as in runner.rs
// ---------------------------------------------------------------------------

struct ResourceDirSelector<'a> {
  config: &'a ArkdownConfig,
  module: &'a ModuleConfig,
  target: &'a EffectiveTargetConfig,
}

impl<'a> ResourceDirSelector<'a> {
  fn new(
    config: &'a ArkdownConfig,
    module: &'a ModuleConfig,
    target: &'a EffectiveTargetConfig,
  ) -> Self {
    Self {
      config,
      module,
      target,
    }
  }

  fn select(&self) -> Vec<PathBuf> {
    let target_dirs = self.resolve_target_dirs(&self.target.resource_directories);
    if !target_dirs.is_empty() {
      return target_dirs;
    }
    if let Some(source_set_res) = self.source_set_dir(&self.target.name) {
      return vec![source_set_res];
    }
    vec![ModuleBuildLayout::new(&self.module.src_path)
      .source()
      .resources()]
  }

  fn resolve_target_dirs(&self, directories: &[PathBuf]) -> Vec<PathBuf> {
    directories
      .iter()
      .filter(|dir| !dir.as_os_str().is_empty())
      .map(|dir| {
        if dir.has_root() {
          dir.clone()
        } else {
          self.module.src_path.join(dir)
        }
      })
      .collect()
  }

  fn source_set_dir(&self, target_name: &str) -> Option<PathBuf> {
    if target_name == "default" {
      return None;
    }
    let dir = ModuleBuildLayout::new(&self.module.src_path)
      .source()
      .target_resources(target_name);
    dir.exists().then_some(dir)
  }

  fn include_app_scope(&self) -> bool {
    let Some(res_options) = self.config.product.build_option.res_options.as_ref() else {
      return true;
    };
    if res_options.include_app_scope_res {
      return true;
    }
    if matches!(
      self.module.module_type,
      arkdown_core::OhosModuleType::Shared
    ) {
      return false;
    }
    tracing::warn!(
        module = %self.module.name,
        module_type = %self.module.module_type,
        "'IncludeAppScopeRes' configured at 'build-profile.json5' only works for HSP module, \
         and will not take effect in HAP/HAR module '{}'.",
        self.module.name
    );
    true
  }
}

struct ObfuscationDependencyRuleCollector<'a> {
  deps: &'a arkdown_session::dependency_graph::snapshot::DependencyGraphSnapshot,
}

impl<'a> ObfuscationDependencyRuleCollector<'a> {
  fn new(deps: &'a arkdown_session::dependency_graph::snapshot::DependencyGraphSnapshot) -> Self {
    Self { deps }
  }

  fn collect(&self) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut seen = FxHashSet::default();
    for node in self.deps.nodes.values() {
      if !matches!(
        node.node_type,
        DependencyNodeType::SourceHar
          | DependencyNodeType::BytecodeHar
          | DependencyNodeType::IntegratedHsp
          | DependencyNodeType::SharedHsp
      ) {
        continue;
      }
      let Some(package_root) = node.package_root.as_deref() else {
        continue;
      };
      let path = package_root.join("obfuscation.txt");
      if !path.is_file() {
        continue;
      }
      let key = canonicalize_or_owned(&path);
      if seen.insert(key.clone()) {
        paths.push(key);
      }
    }
    paths
  }
}

#[cfg(test)]
mod obfuscation_dependency_rule_collector_tests {
  use super::*;

  fn node(alias: &str, root: PathBuf, node_type: DependencyNodeType) -> DependencyNodeSnapshot {
    DependencyNodeSnapshot {
      alias: alias.to_string(),
      package_name: alias.to_string(),
      version_spec: None,
      package_root: Some(root),
      package: None,
      module_profile: None,
      node_type,
      is_runtime_only: false,
      is_dynamic: false,
      is_transitive: false,
      direct_deps: vec![],
    }
  }

  #[test]
  fn collects_hvigor_dependency_obfuscation_txt_only() {
    let temp = tempfile::tempdir().expect("tempdir");
    let har_root = temp.path().join("har");
    let hsp_root = temp.path().join("hsp");
    let source_root = temp.path().join("source");
    fs_err::create_dir_all(&har_root).expect("har dir");
    fs_err::create_dir_all(&hsp_root).expect("hsp dir");
    fs_err::create_dir_all(&source_root).expect("source dir");
    fs_err::write(
      har_root.join("obfuscation.txt"),
      "-keep-global-name\nHarApi\n",
    )
    .expect("har obfuscation");
    fs_err::write(
      hsp_root.join("obfuscation.txt"),
      "-keep-property-name\nhspProp\n",
    )
    .expect("hsp obfuscation");
    fs_err::write(
      source_root.join("obfuscation.txt"),
      "-keep-global-name\nIgnored\n",
    )
    .expect("source obfuscation");

    let mut deps = arkdown_session::dependency_graph::snapshot::DependencyGraphSnapshot::default();
    deps.nodes.insert(
      "har".to_string(),
      node("har", har_root.clone(), DependencyNodeType::SourceHar),
    );
    deps.nodes.insert(
      "hsp".to_string(),
      node("hsp", hsp_root.clone(), DependencyNodeType::SharedHsp),
    );
    deps.nodes.insert(
      "source".to_string(),
      node("source", source_root, DependencyNodeType::OhpmSource),
    );
    deps.nodes.insert(
      "same-har".to_string(),
      node(
        "same-har",
        har_root.clone(),
        DependencyNodeType::BytecodeHar,
      ),
    );

    let paths = ObfuscationDependencyRuleCollector::new(&deps).collect();

    assert_eq!(paths.len(), 2, "paths: {paths:?}");
    assert!(paths.contains(&canonicalize_or_owned(&har_root.join("obfuscation.txt"))));
    assert!(paths.contains(&canonicalize_or_owned(&hsp_root.join("obfuscation.txt"))));
  }
}

struct HarResourceDependencySelector<'a> {
  host_root: &'a Path,
  deps: &'a arkdown_session::dependency_graph::snapshot::DependencyGraphSnapshot,
}

impl<'a> HarResourceDependencySelector<'a> {
  fn new(
    host_root: &'a Path,
    deps: &'a arkdown_session::dependency_graph::snapshot::DependencyGraphSnapshot,
  ) -> Self {
    Self { host_root, deps }
  }

  fn select(&self) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let nodes_by_root: FxHashMap<PathBuf, &DependencyNodeSnapshot> = self
      .deps
      .nodes
      .values()
      .filter_map(|node| {
        node
          .package_root
          .as_deref()
          .map(|root| (canonicalize_or_owned(root), node))
      })
      .collect();
    let mut edges_by_issuer: FxHashMap<PathBuf, Vec<_>> = FxHashMap::default();
    for edge in &self.deps.edges {
      if edge.kind == DependencyEdgeKind::Dependency {
        edges_by_issuer
          .entry(canonicalize_or_owned(&edge.issuer_root))
          .or_default()
          .push(edge);
      }
    }

    // Hvigor's DependencyManager preserves declaration order: direct HARs
    // precede the dependency closure. Traverse the session edges breadth-first
    // instead of iterating the package-name-indexed node map.
    let mut queue = VecDeque::from([canonicalize_or_owned(self.host_root)]);
    let mut seen_packages = FxHashSet::default();
    let mut seen_resources = FxHashSet::default();
    while let Some(issuer_root) = queue.pop_front() {
      let Some(edges) = edges_by_issuer.get(&issuer_root) else {
        continue;
      };
      for edge in edges {
        let package_root = canonicalize_or_owned(&edge.package_root);
        if !seen_packages.insert(package_root.clone()) {
          continue;
        }
        let Some(node) = nodes_by_root.get(&package_root).copied() else {
          continue;
        };
        if matches!(
          node.node_type,
          DependencyNodeType::SourceHar
            | DependencyNodeType::BytecodeHar
            | DependencyNodeType::OhpmSource
        ) {
          for resource_dir in self.resource_dirs(node, &package_root) {
            if !resource_dir.is_dir() {
              continue;
            }
            let resource_dir = canonicalize_or_owned(&resource_dir);
            if seen_resources.insert(resource_dir.clone()) {
              roots.push(resource_dir);
            }
          }
        }
        queue.push_back(package_root);
      }
    }
    roots
  }

  fn resource_dirs(&self, node: &DependencyNodeSnapshot, package_root: &Path) -> Vec<PathBuf> {
    let Some(package) = node.package.as_ref() else {
      return vec![ModuleBuildLayout::new(package_root).source().resources()];
    };
    let Some(directories) = package
      .package_json
      .extra
      .get("metadata")
      .and_then(|metadata| metadata.get("resource"))
      .and_then(|resource| resource.get("directories"))
      .and_then(|directories| directories.as_array())
    else {
      return vec![ModuleBuildLayout::new(package_root).source().resources()];
    };
    let mut roots = Vec::new();
    for directory in directories {
      let Some(directory) = directory.as_str() else {
        continue;
      };
      let path = PathBuf::from(directory);
      roots.push(if path.has_root() {
        path
      } else {
        package_root.join(path)
      });
    }
    if roots.is_empty() {
      vec![ModuleBuildLayout::new(package_root).source().resources()]
    } else {
      roots
    }
  }
}

#[cfg(test)]
mod har_resource_dependency_selector_tests {
  use super::*;
  use arkdown_session::dependency_graph::snapshot::{
    DependencyEdgeSnapshot, DependencyGraphSnapshot,
  };

  fn node(name: &str, root: &Path, node_type: DependencyNodeType) -> DependencyNodeSnapshot {
    DependencyNodeSnapshot {
      alias: name.to_string(),
      package_name: name.to_string(),
      version_spec: None,
      package_root: Some(root.to_path_buf()),
      package: None,
      module_profile: None,
      node_type,
      is_runtime_only: false,
      is_dynamic: false,
      is_transitive: false,
      direct_deps: vec![],
    }
  }

  fn edge(
    issuer_root: &Path,
    name: &str,
    package_root: &Path,
    kind: DependencyEdgeKind,
  ) -> DependencyEdgeSnapshot {
    DependencyEdgeSnapshot {
      issuer_root: issuer_root.to_path_buf(),
      alias: name.to_string(),
      version_spec: None,
      package_name: name.to_string(),
      package_root: package_root.to_path_buf(),
      kind,
      is_transitive: false,
    }
  }

  #[test]
  fn follows_declared_har_order_and_includes_bytecode_hars() {
    let temp = tempfile::tempdir().expect("create resource graph fixture");
    let host = temp.path().join("entry");
    let bytecode = temp.path().join("bytecode");
    let source = temp.path().join("source");
    let nested = temp.path().join("nested");
    let dev = temp.path().join("dev");
    for root in [&bytecode, &source, &nested, &dev] {
      fs_err::create_dir_all(ModuleBuildLayout::new(root).source().resources())
        .expect("create dependency resource root");
    }

    let mut graph = DependencyGraphSnapshot::default();
    for (name, root, node_type) in [
      ("z-bytecode", &bytecode, DependencyNodeType::BytecodeHar),
      ("a-source", &source, DependencyNodeType::SourceHar),
      ("nested", &nested, DependencyNodeType::BytecodeHar),
      ("dev", &dev, DependencyNodeType::SourceHar),
    ] {
      graph
        .nodes
        .insert(name.to_string(), node(name, root, node_type));
    }
    graph.edges = vec![
      edge(
        &host,
        "z-bytecode",
        &bytecode,
        DependencyEdgeKind::Dependency,
      ),
      edge(&host, "a-source", &source, DependencyEdgeKind::Dependency),
      edge(&bytecode, "nested", &nested, DependencyEdgeKind::Dependency),
      edge(&host, "dev", &dev, DependencyEdgeKind::DevDependency),
    ];

    let selected = HarResourceDependencySelector::new(&host, &graph).select();
    assert_eq!(
      selected,
      vec![
        canonicalize_or_owned(&ModuleBuildLayout::new(&bytecode).source().resources()),
        canonicalize_or_owned(&ModuleBuildLayout::new(&source).source().resources()),
        canonicalize_or_owned(&ModuleBuildLayout::new(&nested).source().resources()),
      ]
    );
  }
}

// ---------------------------------------------------------------------------
// ModuleJsonReader — same as in runner.rs
// ---------------------------------------------------------------------------

use anyhow::Context as _;

struct ModuleJsonReader;

impl ModuleJsonReader {
  fn new(_module_root: &Path) -> Self {
    Self
  }

  fn ensure_startup_profile_ref(
    &self,
    module_json_path: &Path,
    startup_out: &arkdown_profile::StartupConfigOutput,
    artifact_writer: &IntermediateArtifactWriter,
  ) -> Result<()> {
    let Some(startup_config) = startup_out.config.as_ref() else {
      return Ok(());
    };
    if startup_config.startup_tasks.is_empty()
      && startup_config.app_preload_hint_startup_tasks.is_empty()
      && startup_config.system_preload_hint_startup_tasks.is_empty()
    {
      return Ok(());
    }
    let content = fs_err::read_to_string(module_json_path)?;
    let mut value: serde_json::Value = serde_json::from_str(&content)?;
    let module = value
      .get_mut("module")
      .and_then(|v| v.as_object_mut())
      .context("module.json missing module object")?;
    if module.contains_key("appStartup") {
      return Ok(());
    }
    module.insert(
      "appStartup".to_string(),
      serde_json::Value::String("$profile:startup_config".to_string()),
    );
    artifact_writer.write_json_pretty(module_json_path, &value)
  }
}

struct CompiledResourceKey<'a> {
  name: &'a str,
}

impl<'a> CompiledResourceKey<'a> {
  fn new(name: &'a str) -> Self {
    Self { name }
  }

  fn into_string(self) -> String {
    CoordinatorText::from_parts(["app.", self.name])
  }
}

struct ArtifactOutputName<'a> {
  module_name: &'a str,
  target_label: &'a str,
  extension: &'a str,
}

impl<'a> ArtifactOutputName<'a> {
  fn new(module_name: &'a str, target_label: &'a str, extension: &'a str) -> Self {
    Self {
      module_name,
      target_label,
      extension,
    }
  }

  fn unsigned(&self) -> String {
    CoordinatorText::from_parts([
      self.module_name,
      "-",
      self.target_label,
      "-unsigned.",
      self.extension,
    ])
  }

  fn signed(&self) -> String {
    CoordinatorText::from_parts([
      self.module_name,
      "-",
      self.target_label,
      "-signed.",
      self.extension,
    ])
  }

  fn har(&self) -> String {
    CoordinatorText::from_parts([self.module_name, ".", self.extension])
  }

  fn shared_tgz(&self) -> String {
    CoordinatorText::from_parts([self.module_name, "-", self.target_label, ".tgz"])
  }
}

struct HexText;

impl HexText {
  fn lower_u64(hash: u64) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let mut bytes = [0_u8; 16];
    let mut len = 0;
    let mut started = false;
    for shift in (0..16).rev() {
      let nibble = ((hash >> (shift * 4)) & 0x0f) as usize;
      if nibble != 0 || started || shift == 0 {
        started = true;
        bytes[len] = HEX[nibble];
        len += 1;
      }
    }

    bytes[..len].iter().map(|byte| char::from(*byte)).collect()
  }
}

struct CoordinatorText;

impl CoordinatorText {
  fn from_parts<const N: usize>(parts: [&str; N]) -> String {
    parts.into_iter().collect()
  }
}
