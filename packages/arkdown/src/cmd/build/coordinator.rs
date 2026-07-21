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

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use rustc_hash::{FxHashMap, FxHashSet};

use arkdown_core::{
  ArkdownConfig, BuildMode, HapManifest, ModuleConfig, OhosModuleType, OutputTarget,
};
use arkdown_path::{
  canonicalize_or_owned, lexical_normalize, ArtifactPathSerializer, HvigorSourceFileFilter,
  NormalizedOhmUrl,
};
use arkdown_session::dependency_graph::snapshot::{DependencyNodeSnapshot, DependencyNodeType};
use arkdown_session::module::snapshot::OhosModuleType as SessionOhosModuleType;
use arkdown_session::{BuildSession, ModuleCompileContext, SdkToolKind};
use arkdown_sign::SignBackendRunner;

use super::args::{AbcBackendChoice, BuildArgs};
use super::bundle_plan::{
  make_bundle_plan, BundlePackageOutput, BundleRuntimePackageInfoBuilder, BundleStage,
  BundleStageInput, BundleStageOutput,
};
use super::bytecode::{BytecodeAbcInput, RolldownBytecodeBackendChoice, RolldownBytecodeCompiler};
use super::bytecode_har_declaration::ByteCodeHarDeclarationCollector;
use super::bytecode_stage::{BytecodeStage, BytecodeStageInput};
use super::coverage_instrument::CoverageInstrumenter;
use super::custom_config::ArkdownCustomConfigLoader;
use super::generate_har_declarations::HarDeclarationGenerator;
use super::intermediates::IntermediateArtifactWriter;
use super::layout::{
  ModuleBuildLayout, ModuleSourceLayout, ProjectBuildLayout, ResourceOutputLayout,
};
use super::module_plan::ModuleSelectionPlanner;
use super::watch_stage::{WatchStage, WatchStageInput};
use arkdown::plugins::factory::{PackagePlugin, PluginFactory, PluginFactoryInput};
use arkdown::{
  BundleCacheBridge, BundleSourceMapPackageInfo, ShouldInvalidCacheRequest, SourceMapPolicy,
};

use crate::cmd::native_build::{
  NativeLibCollectionPolicy, NativeLibCollector, NativeLibPackageInfo, NativeSdkToolchain,
};
use crate::progress::Progress;
use crate::task_cache::TaskInputs;

use arkdown::plugins::ets_transform::{
  CheckerMode, ComponentMetadata, ModuleAnalyzerKind, ResourceReference,
  SystemResourceReferenceLoader, TransformResolvedStaticModule, TransformSourceSnapshot,
};
use arkdown::plugins::ohos_resolve::{ProcessImporteeConfig, ProcessImporteeRunner};
use arkdown_cache::{BuildCache, BundleCacheKey, CacheGroupKey, CacheGroupSnapshot, CacheKey};
use arkdown_config::build_profile::{parse_api_version, resolve_effective_native_build_option};
use arkdown_ets_transform::{
  ArkuiComponentNameCollector, RolldownEtsTransformPlan, SourceClassifier,
};
use arkdown_obfuscate::{
  ConsumerObfuscationFileWriter, ObfuscationOptions, ObfuscationOutputPaths, ObfuscationPlan,
  Obfuscator,
};
use arkdown_pack::{
  HarPackInput, IntegratedHspCandidate, IntegratedHspCollector, PackAbility, PackInput, Packager,
};
use arkdown_profile::{
  ByteCodeHarMetadataWriter, ByteCodeHarScanEntry, FormProfileProcessor, LoaderJson,
  ModuleFormProfileInput, PkgContextCurrentInput, PkgContextDependencyInput,
  PkgContextDependencyKind, PkgContextInfoGenerator, PkgContextInfoInput, PkgContextInfoOpt,
  PkgContextModuleKind, ProfileProcessor,
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
    if self.module.module_type == OhosModuleType::Shared {
      return Ok(None);
    }
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
        canonicalize_or_owned(&source.source_path)
          .to_string_lossy()
          .into_owned()
      })
      .collect::<Vec<_>>();
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
    bundle_plan.input.runtime_entry_sources.clear();
    bundle_plan.output.module_source_output_dir = Some(plan.cache_dir.clone());
    bundle_plan.output.output_dir = plan.output_dir.clone();
    bundle_plan.source_map = SourceMapPolicy::None;
    bundle_plan
  }

  fn compile_context_loader_json(&self, plan: &WidgetCompileArktsPlan) -> LoaderJson {
    let mut loader_json = self.loader_json.clone();
    loader_json.hvigor_compile_entry.clear();
    loader_json.compile_entry = plan.entries.clone();
    loader_json.runtime_entry_sources.clear();
    loader_json.cache_path = plan.cache_dir.to_string_lossy().into_owned();
    loader_json
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

struct TransformSourceSnapshotBuilder<'a> {
  module_roots: &'a [(String, PathBuf)],
  dependency_source_roots: &'a [PathBuf],
  native_declaration_roots: &'a [PathBuf],
  source_set_overlays: &'a [(PathBuf, PathBuf)],
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
      source_set_overlays,
    }
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
    for root in self.native_declaration_roots {
      self.collect_declaration_root(
        root,
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

    for path in paths {
      let source = Arc::<str>::from(fs_err::read_to_string(&path)?);
      files.insert(canonicalize_or_owned(&path), source);
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

struct ObfuscationTargetFileCollector<'a> {
  bundle_outs: &'a [(BundleStageOutput,)],
}

impl<'a> ObfuscationTargetFileCollector<'a> {
  fn new(bundle_outs: &'a [(BundleStageOutput,)]) -> Self {
    Self { bundle_outs }
  }

  fn collect(&self) -> BTreeSet<PathBuf> {
    let mut paths = BTreeSet::new();
    for (bundle_out,) in self.bundle_outs {
      for (_chunk_name, path) in &bundle_out.bundle_output.chunk_files {
        self.insert_if_obfuscatable(&mut paths, path);
      }
      for source in &bundle_out.bundle_output.module_source_files {
        self.insert_if_obfuscatable(&mut paths, &source.path);
      }
    }
    paths
  }

  fn insert_if_obfuscatable(&self, paths: &mut BTreeSet<PathBuf>, path: &Path) {
    if path.exists() && Self::is_obfuscatable_source(path) {
      paths.insert(path.to_path_buf());
    }
  }

  fn is_obfuscatable_source(path: &Path) -> bool {
    matches!(
      path.extension().and_then(|extension| extension.to_str()),
      Some("js" | "ts")
    )
  }
}

#[cfg(test)]
mod transform_source_snapshot_builder_tests {
  use super::*;
  use arkdown_ets_transform::{RolldownEtsTransformPlan, RolldownEtsTransformer};

  #[test]
  fn native_so_dependency_roots_provide_declaration_imports_to_ets_transform() {
    let temp = tempfile::tempdir().expect("tempdir");
    let package_root = temp.path().join("oh_modules/@example/crypto");
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
      .join("oh_modules/.ohpm/@example+crypto@hash=/oh_modules/@example/crypto");
    let symlink_package_root = temp.path().join("oh_modules/@example/crypto");
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
    std::os::windows::fs::symlink_dir(src, dst)
  }
}

#[cfg(test)]
mod obfuscation_target_file_collector_tests {
  use super::*;
  use arkdown_core::{ChunkGraph, HvigorModuleSourceFile};

  #[test]
  fn collects_chunk_and_module_source_files_for_abc_handoff_obfuscation() {
    let temp = tempfile::tempdir().expect("tempdir");
    let chunk_js = temp.path().join("loader_out/default/ets/Index.js");
    let module_ts = temp
      .path()
      .join("cache/default@CompileArkTS/entry/pages/Index.ts");
    let ignored_map = temp.path().join("loader_out/default/ets/sourceMaps.map");
    fs_err::create_dir_all(chunk_js.parent().expect("chunk parent")).expect("chunk dir");
    fs_err::create_dir_all(module_ts.parent().expect("module parent")).expect("module dir");
    fs_err::write(&chunk_js, "export const longName = 1;").expect("chunk file");
    fs_err::write(&module_ts, "export const typedName: number = 1;").expect("module file");
    fs_err::write(&ignored_map, "{}").expect("map file");

    let bundle_out = BundleStageOutput {
      bundle_output: arkdown::BundleOutput {
        chunk_graph: ChunkGraph::default(),
        output_dir: temp.path().join("loader_out/default/ets"),
        chunk_files: vec![
          ("Index".to_string(), chunk_js.clone()),
          ("Duplicate".to_string(), chunk_js.clone()),
          ("sourceMaps".to_string(), ignored_map),
        ],
        chunk_source_files: vec![],
        module_source_files: vec![HvigorModuleSourceFile {
          module_id: "entry/src/main/ets/pages/Index.ets".to_string(),
          path: module_ts.clone(),
          record_name: "com.example/entry/pages/Index".to_string(),
          source_file: "src/main/ets/pages/Index.ets".to_string(),
          package_name: Some("entry".to_string()),
          is_commonjs: false,
          origin_source_lang: "ets".to_string(),
          imported_record_names: vec![],
        }],
        entry_module_ids: vec![],
        module_imports: FxHashMap::default(),
        external_module_requests: vec![],
        npm_entries: vec![],
        modules_abc_path: temp.path().join("modules.abc"),
        source_maps_path: None,
      },
      from_cache: false,
    };

    let paths = ObfuscationTargetFileCollector::new(&[(bundle_out,)]).collect();
    assert_eq!(paths.len(), 2, "got: {paths:?}");
    assert!(paths.contains(&chunk_js), "chunk js must be obfuscated");
    assert!(
      paths.contains(&module_ts),
      "CompileArkTS module source must be obfuscated"
    );
  }
}

struct CompileEnvKeyBuilder<'a> {
  config: &'a ArkdownConfig,
  module: &'a ModuleConfig,
  loader_json: &'a LoaderJson,
}

impl<'a> CompileEnvKeyBuilder<'a> {
  fn new(config: &'a ArkdownConfig, module: &'a ModuleConfig, loader_json: &'a LoaderJson) -> Self {
    Self {
      config,
      module,
      loader_json,
    }
  }

  fn build(&self) -> Result<String> {
    let obfuscation_enabled = self
      .config
      .product
      .build_option
      .ark_options
      .as_ref()
      .and_then(|ark_options| ark_options.obfuscation.as_ref())
      .map(|obfuscation| obfuscation.enable_obfuscation)
      .unwrap_or(false);
    let ets_loader_path =
      ArtifactPathSerializer::path_to_slash_string(&self.config.sdk_config.ets_loader_path)
        .map_err(|error| {
          anyhow::anyhow!("serialize ETS loader path for compile env failed: {error}")
        })?;
    let compiler_cache_path = arkdown_cache::compiler_cache_path(&self.loader_json.cache_path);

    Ok(
      arkdown_cache::CompileEnv::from_plan(
        self.config.sdk_config.compile_sdk_version.clone(),
        self.config.product.compatible_sdk_version.clone(),
        self.config.product.runtime_os.clone(),
        ets_loader_path,
        obfuscation_enabled,
        format!("{:?}", self.module.module_type),
        compiler_cache_path,
      )
      .to_key_string(),
    )
  }
}

struct PkgContextInfoInputBuilder<'a> {
  config: &'a ArkdownConfig,
  session: &'a BuildSession,
  module: &'a ModuleConfig,
  module_context: &'a ModuleCompileContext,
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
    }
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
      dependencies.push(PkgContextDependencyInput {
        alias: node.alias.clone(),
        package_name: node.package_name.clone(),
        version_spec: self.dependency_version(node)?,
        main: node.package.as_ref().and_then(|package| {
          package
            .package_json
            .main
            .clone()
            .or_else(|| package.package_json.types.clone())
        }),
        kind: self.dependency_kind(node.node_type),
        module_name: None,
        bundle_name: None,
        bundle_type: None,
      });
    }
    let native_build_option = self.effective_native_build_option()?;
    let native_libraries = NativeLibCollector::new(
      self.module,
      &native_build_option.abi_filters,
      self.module_context.deps.as_ref(),
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
      },
      dependencies,
      native_libraries,
    })
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
    let build_mode_name = match self.config.build_mode {
      BuildMode::Debug => "debug",
      BuildMode::Release => "release",
    };
    let target_name = arkdown_config::build_profile::select_effective_target(
      &self.config.project_root,
      &self.module.name,
      &self.config.product.name,
      build_mode_name,
    )
    .map(|target| target.name)
    .unwrap_or_else(|| "default".to_string());
    resolve_effective_native_build_option(
      &self.module.src_path,
      &self.config.product.build_option.abi_filters,
      &self.config.product.build_option.native_lib,
      build_mode_name,
      &target_name,
    )
    .map_err(|error| anyhow::anyhow!("resolve native build option failed: {}", error))
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
}

struct CompileContextInfoWriter<'a> {
  loader_json: &'a LoaderJson,
  module_source_files: &'a [arkdown_core::HvigorModuleSourceFile],
  bundle_type: &'a str,
  bundle_name: &'a str,
}

impl<'a> CompileContextInfoWriter<'a> {
  fn new(
    loader_json: &'a LoaderJson,
    module_source_files: &'a [arkdown_core::HvigorModuleSourceFile],
    _pkg_context_info: &'a BTreeMap<String, PkgContextInfoOpt>,
    bundle_type: &'a str,
    bundle_name: &'a str,
  ) -> Self {
    Self {
      loader_json,
      module_source_files,
      bundle_type,
      bundle_name,
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
    let mut entries = BTreeSet::new();
    let source_records = self.source_record_index();
    for source_entry in self.loader_json.hvigor_compile_entry.iter() {
      if let Some(record_name) = self.record_name_for_source_entry(source_entry, &source_records) {
        entries.insert(record_name);
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
        entries.insert(record_name);
      }
    }
    for runtime_entry in &self.loader_json.runtime_entry_sources {
      if let Some(record_name) = self.record_name_for_source_entry(runtime_entry, &source_records) {
        entries.insert(record_name);
      }
    }
    for ohm_url in &self.loader_json.declaration_entry {
      if let Some(record_name) = self.record_name_from_ohm_url(ohm_url, &hsp_names) {
        entries.insert(record_name);
      }
    }
    for router_entry in &self.loader_json.router_map {
      if let Some(ohm_url) = router_entry
        .get("ohmurl")
        .and_then(serde_json::Value::as_str)
      {
        if let Some(record_name) = self.record_name_from_ohm_url(ohm_url, &hsp_names) {
          entries.insert(record_name);
        }
      }
    }
    // Hvigor writes compile entries as dependency roots. The ArkTS compiler
    // resolves static and dynamic imports from those roots; expanding the full
    // import closure here turns dependencies into roots and can perturb module
    // initialization order for cyclic ESM graphs.
    entries.into_iter().collect()
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
    let value = if path.is_absolute() {
      path.to_path_buf()
    } else {
      module_root.join(Self::trim_relative_prefix(path))
    };
    Self::new(value)
  }

  fn from_module_source(source: &arkdown_core::HvigorModuleSourceFile) -> Option<Self> {
    let module_id_path = Path::new(&source.module_id);
    if module_id_path.is_absolute() {
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
  fn compile_entries_keep_runtime_roots_without_import_closure() {
    let temp = tempfile::tempdir().expect("tempdir");
    let module_root = temp.path().join("example/src/main");
    let runtime_entry = temp
      .path()
      .join("example/src/main/ets/abilitystage/EntryAbilityStage.ets");
    let package_entry = temp.path().join("example/oh_modules/mtlauncher/Index.ets");
    let package_build_profile = temp
      .path()
      .join("example/oh_modules/mtlauncher/BuildProfile.ets");
    let log_entry = temp
      .path()
      .join("example/oh_modules/@example/log/Index.ets");
    let mut loader_json = test_loader_json(&module_root);
    loader_json.runtime_entry_sources = vec![runtime_entry.to_string_lossy().into_owned()];

    let module_source_files = vec![
      HvigorModuleSourceFile {
        module_id: runtime_entry.to_string_lossy().into_owned(),
        path: temp
          .path()
          .join("cache/example/src/main/ets/abilitystage/EntryAbilityStage.ts"),
        record_name: "&example/src/main/ets/abilitystage/EntryAbilityStage&".to_string(),
        source_file: "example|example|1.0.0|src/main/ets/abilitystage/EntryAbilityStage.ts"
          .to_string(),
        package_name: Some("example".to_string()),
        is_commonjs: false,
        origin_source_lang: "ets".to_string(),
        imported_record_names: vec!["&mtlauncher/Index&12.59.403".to_string()],
      },
      HvigorModuleSourceFile {
        module_id: package_entry.to_string_lossy().into_owned(),
        path: temp.path().join("cache/oh_modules/mtlauncher/Index.ts"),
        record_name: "&mtlauncher/Index&12.59.403".to_string(),
        source_file: "example|mtlauncher|12.59.403|Index.ts".to_string(),
        package_name: Some("mtlauncher".to_string()),
        is_commonjs: false,
        origin_source_lang: "ets".to_string(),
        imported_record_names: vec![
          "&mtlauncher/BuildProfile&12.59.403".to_string(),
          "&@example/log/Index&0.0.18".to_string(),
        ],
      },
      HvigorModuleSourceFile {
        module_id: package_build_profile.to_string_lossy().into_owned(),
        path: temp
          .path()
          .join("cache/oh_modules/mtlauncher/BuildProfile.ts"),
        record_name: "&mtlauncher/BuildProfile&12.59.403".to_string(),
        source_file: "example|mtlauncher|12.59.403|BuildProfile.ts".to_string(),
        package_name: Some("mtlauncher".to_string()),
        is_commonjs: false,
        origin_source_lang: "ets".to_string(),
        imported_record_names: vec![],
      },
      HvigorModuleSourceFile {
        module_id: log_entry.to_string_lossy().into_owned(),
        path: temp.path().join("cache/oh_modules/@example/log/Index.ts"),
        record_name: "&@example/log/Index&0.0.18".to_string(),
        source_file: "example|@example/log|0.0.18|Index.ts".to_string(),
        package_name: Some("@example/log".to_string()),
        is_commonjs: false,
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
      "com.example.hexample",
    );

    assert_eq!(
      writer.compile_entries(&[]),
      vec!["&example/src/main/ets/abilitystage/EntryAbilityStage&".to_string()]
    );
  }

  #[test]
  fn compile_entries_do_not_add_dependency_package_entry_for_represented_package() {
    let temp = tempfile::tempdir().expect("tempdir");
    let module_root = temp.path().join("example/src/main");
    let api_entry = temp
      .path()
      .join("example/oh_modules/@mrn/mrn/src/main/ets/mapi/apis/getfeedcommentlist_bin.ets");
    let index_entry = temp.path().join("example/oh_modules/@mrn/mrn/Index.ets");
    let build_profile_entry = temp
      .path()
      .join("example/oh_modules/@mrn/mrn/BuildProfile.ets");
    let config_entry = temp
      .path()
      .join("example/oh_modules/@mrn/mrn/src/main/ets/config/MRNConfigCenter.ets");
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
        source_file: "example|@mrn/mrn|12.59.403|src/main/ets/mapi/apis/getfeedcommentlist_bin.ts"
          .to_string(),
        package_name: Some("@mrn/mrn".to_string()),
        is_commonjs: false,
        origin_source_lang: "ets".to_string(),
        imported_record_names: vec![],
      },
      HvigorModuleSourceFile {
        module_id: index_entry.to_string_lossy().into_owned(),
        path: temp.path().join("cache/oh_modules/@mrn/mrn/Index.ts"),
        record_name: "&@mrn/mrn/Index&12.59.403".to_string(),
        source_file: "example|@mrn/mrn|12.59.403|Index.ts".to_string(),
        package_name: Some("@mrn/mrn".to_string()),
        is_commonjs: false,
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
        source_file: "example|@mrn/mrn|12.59.403|BuildProfile.ts".to_string(),
        package_name: Some("@mrn/mrn".to_string()),
        is_commonjs: false,
        origin_source_lang: "ets".to_string(),
        imported_record_names: vec![],
      },
      HvigorModuleSourceFile {
        module_id: config_entry.to_string_lossy().into_owned(),
        path: temp
          .path()
          .join("cache/oh_modules/@mrn/mrn/src/main/ets/config/MRNConfigCenter.ts"),
        record_name: "&@mrn/mrn/src/main/ets/config/MRNConfigCenter&12.59.403".to_string(),
        source_file: "example|@mrn/mrn|12.59.403|src/main/ets/config/MRNConfigCenter.ts"
          .to_string(),
        package_name: Some("@mrn/mrn".to_string()),
        is_commonjs: false,
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
      "com.example.hexample",
    );

    assert_eq!(
      writer.compile_entries(&[]),
      vec!["&@mrn/mrn/src/main/ets/mapi/apis/getfeedcommentlist_bin&12.59.403".to_string()]
    );
  }

  #[test]
  fn compile_entries_keep_compile_roots_without_import_closure() {
    let temp = tempfile::tempdir().expect("tempdir");
    let module_root = temp.path().join("example/src/main");
    let form_entry = temp
      .path()
      .join("example/src/main/ets/form/FormAbility.ets");
    let subscribe_entry = temp
      .path()
      .join("oh_modules/@example/card_subscribe/index.ets");
    let subscribe_form = temp
      .path()
      .join("oh_modules/@example/card_subscribe/src/main/ets/subscribeForm/PTSubscribeForm.ets");
    let mut loader_json = test_loader_json(&module_root);
    loader_json.compile_entry = vec![form_entry.to_string_lossy().into_owned()];

    let module_source_files = vec![
      HvigorModuleSourceFile {
        module_id: form_entry.to_string_lossy().into_owned(),
        path: temp
          .path()
          .join("cache/example/src/main/ets/form/FormAbility.ts"),
        record_name: "&example/src/main/ets/form/FormAbility&".to_string(),
        source_file: "example|example|1.0.0|src/main/ets/form/FormAbility.ts".to_string(),
        package_name: Some("example".to_string()),
        is_commonjs: false,
        origin_source_lang: "ets".to_string(),
        imported_record_names: vec!["&@example/card_subscribe/&1.0.32".to_string()],
      },
      HvigorModuleSourceFile {
        module_id: subscribe_entry.to_string_lossy().into_owned(),
        path: temp
          .path()
          .join("cache/oh_modules/@example/card_subscribe/index.ts"),
        record_name: "&@example/card_subscribe/&1.0.32".to_string(),
        source_file: "example|@example/card_subscribe|1.0.32|index.ts".to_string(),
        package_name: Some("@example/card_subscribe".to_string()),
        is_commonjs: false,
        origin_source_lang: "ets".to_string(),
        imported_record_names: vec![
          "&@example/card_subscribe/src/main/ets/subscribeForm/PTSubscribeForm&1.0.32".to_string(),
        ],
      },
      HvigorModuleSourceFile {
        module_id: subscribe_form.to_string_lossy().into_owned(),
        path: temp.path().join(
          "cache/oh_modules/@example/card_subscribe/src/main/ets/subscribeForm/PTSubscribeForm.ts",
        ),
        record_name: "&@example/card_subscribe/src/main/ets/subscribeForm/PTSubscribeForm&1.0.32"
          .to_string(),
        source_file:
          "example|@example/card_subscribe|1.0.32|src/main/ets/subscribeForm/PTSubscribeForm.ts"
            .to_string(),
        package_name: Some("@example/card_subscribe".to_string()),
        is_commonjs: false,
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
      "com.example.hexample",
    );

    assert_eq!(
      writer.compile_entries(&[]),
      vec!["&example/src/main/ets/form/FormAbility&".to_string()]
    );
  }

  #[test]
  fn compile_entries_do_not_include_other_compile_files() {
    let temp = tempfile::tempdir().expect("tempdir");
    let module_root = temp.path().join("example/src/main");
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
      source_file: "example|dljson|0.0.5|src/main/ets/internal/DlJson.ts".to_string(),
      package_name: Some("dljson".to_string()),
      is_commonjs: false,
      origin_source_lang: "ts".to_string(),
      imported_record_names: vec![],
    }];
    let pkg_context_info = BTreeMap::new();
    let writer = CompileContextInfoWriter::new(
      &loader_json,
      &module_source_files,
      &pkg_context_info,
      "app",
      "com.example.hexample",
    );

    assert_eq!(writer.compile_entries(&[]), Vec::<String>::new());
  }

  #[test]
  fn compile_entries_do_not_include_bytecode_har_entry_records() {
    let temp = tempfile::tempdir().expect("tempdir");
    let module_root = temp.path().join("example/src/main");
    let mut loader_json = test_loader_json(&module_root);
    loader_json.byte_code_har_info.insert(
      "@tencent/qq-open-sdk".to_string(),
      arkdown_profile::ByteCodeHarEntry {
        package_name: "@tencent/qq-open-sdk".to_string(),
        abc_path: temp
          .path()
          .join("oh_modules/@tencent/qq-open-sdk/ets/modules.abc")
          .to_string_lossy()
          .into_owned(),
        compatible_sdk_version: "12".to_string(),
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
      "com.example.hexample",
    );

    assert_eq!(writer.compile_entries(&[]), Vec::<String>::new());
  }

  #[test]
  fn compile_context_info_serializes_hvigor_update_version_shape() {
    let temp = tempfile::tempdir().expect("tempdir");
    let module_root = temp.path().join("example/src/main");
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
      "com.example.hexample",
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
      byte_code_har_info: FxHashMap::default(),
      declaration_entry: vec![],
      module_name: "example".to_string(),
      hsp_name_ohm_map: FxHashMap::default(),
      har_name_ohm_map: FxHashMap::default(),
      package_manager_type: "ohpm".to_string(),
      hvigor_compile_entry: vec![],
      other_compile_files: vec![],
      dynamic_import_lib_info: IndexMap::new(),
      router_map: vec![],
      hsp_resources_map: FxHashMap::default(),
      update_version_info: FxHashMap::default(),
      an_build_out_put: None,
      an_build_mode: None,
      ap_path: None,
      build_config_path: None,
      patch_config: None,
      non_entry_importees: vec![],
      compiled_importees: vec![],
      compile_entry: vec![],
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
      custom_types: vec![],
      dependency_pkg_version: vec![],
      extra_importees: vec![],
      compiled_package_names: vec![],
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
      oh_exports: BTreeMap::new(),
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
) -> Vec<CacheGroupSnapshot> {
  let module_layout = ModuleBuildLayout::new(&module.src_path);

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

  // 6. PreloadSo — sorted preload SO path strings.
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
    paths.join("|")
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

  // 9. ByteCodeHarOptimize — not yet modeled in ArkdownConfig; always false.
  let byte_code_har_optimize_snap =
    CacheGroupSnapshot::from_bool(CacheGroupKey::ByteCodeHarOptimize, false);

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
fn build_share_context_json(config: &ArkdownConfig, args: &BuildArgs) -> String {
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
              "isByteCodeHarOptimize": false,
          }
      }
  })
  .to_string()
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
  /// Construct a new coordinator from a completed build session and CLI args.
  pub fn new(session: Arc<BuildSession>, config: Arc<ArkdownConfig>, args: BuildArgs) -> Self {
    Self {
      session,
      config,
      args,
      package_plugins: vec![],
    }
  }

  /// Attach package-supplied Rolldown plugins collected by the Node.js API.
  pub fn with_package_plugins(mut self, package_plugins: Vec<PackagePlugin>) -> Self {
    self.package_plugins = package_plugins;
    self
  }

  /// Execute the full build pipeline and return the produced artifacts.
  ///
  /// Wraps [`BuildCoordinator::execute_impl`] to ensure the `cleanUp`
  /// lifecycle hook is called unconditionally, even when the build fails.
  pub async fn execute(self) -> Result<BuildOutput> {
    // Clone before moving self into execute_impl so we can fire cleanUp
    // after execute_impl returns, regardless of success or failure.
    let package_plugins_for_cleanup = self.package_plugins.clone();
    let result = self.execute_impl().await;
    for plugin in &package_plugins_for_cleanup {
      if let Err(e) = plugin.clean_up().await {
        tracing::warn!(error = %e, "cleanUp hook failed (non-fatal)");
      }
    }
    result
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

    // beforeBuild — hvigor calls this before runArkPack() starts.
    // Called with a JSON share context so plugins can access build
    // metadata as `this.share` (e.g. `this.share.bundleName`).
    // Corresponds to hvigor's `getCompilePluginContext(this).share`.
    let share_ctx_json = build_share_context_json(config.as_ref(), args);
    for plugin in package_plugins.iter() {
      plugin.before_build_with_share(&share_ctx_json).await?;
    }

    let watch_mode = args.watch || args.preview;
    let preview_mode = args.preview;
    let coverage_mode = args.coverage;
    let disable_abc_cache = args.disable_abc_cache || !args.cache;
    let cli_cache_file = args.cache_file.clone();
    let abc_backend = args.abc_backend;
    let named_target = args.build_target_name.clone();
    let module_analyzer = ModuleAnalyzerKind::from(args.module_analyzer);

    let checker_mode = match args.checker.as_deref().unwrap_or("oxc") {
      "oxc" => CheckerMode::Oxc,
      "off" => CheckerMode::Off,
      checker => anyhow::bail!("unsupported checker '{checker}'; expected 'oxc' or 'off'"),
    };

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
      ModuleSelectionPlanner::build_modules_for_target(config.as_ref(), args.module.as_deref());
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
        "coverage mode enabled — build prepared for code coverage (F20); \
                 Istanbul counter injection not yet implemented"
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

    let processor = ProfileProcessor::new(config.clone());
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
    let pac_schema_path = config.sdk_config.pac_schema_path();
    let component_metadata = {
      let _timer = BuildTimingProbe::global("component_metadata");
      ComponentMetadata::from_ets_loader_path(&config.sdk_config.ets_loader_path).map_err(|e| {
        anyhow::anyhow!(
          "SDK component metadata load failed from {}: {}",
          config.sdk_config.ets_loader_path.display(),
          e
        )
      })?
    };

    if matches!(&config.output_target, OutputTarget::Hap) {
      let _timer = BuildTimingProbe::global("project_privacy_profile");
      privacy_profile_processor
        .process_project_privacy_profile(arkdown_profile::ProjectPrivacyProfileInput {
          project_root: &project_root,
          product_name: &config.product.name,
          modules: &config.modules,
          pac_schema_path: pac_schema_path.as_deref(),
        })
        .map_err(|e| anyhow::anyhow!("ProcessProjectPrivacyProfile failed: {}", e))?;
    }

    for module in build_modules {
      let _module_timer = BuildTimingProbe::module(&module.name, "module_total");
      let module_layout = ModuleBuildLayout::new(&module.src_path);
      let module_context = session.module_context(&module.name).ok_or_else(|| {
        anyhow::anyhow!("missing build session module context for {}", module.name)
      })?;

      let module_json5_path = module_context
        .module
        .module_json_path
        .clone()
        .unwrap_or_else(|| module_layout.source().module_json5());
      let module_json5 = module_context
        .module
        .module_json
        .as_ref()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("missing module.json5 snapshot for {}", module.name))?;
      let profile_dependency_context = {
        let _timer = BuildTimingProbe::module(&module.name, "profile_dependency_context");
        arkdown_profile::ProfileDependencyContext::from_session_graph(
          config.as_ref(),
          module,
          module_context.deps.as_ref(),
        )
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
      let skip_profile_writes = args.skip_profile;
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
      let preload_so_paths = &config.product.build_option.preload_so;
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
            target_name: "default",
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
              target_name: "default",
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

      let share_out = {
        let _timer = BuildTimingProbe::module(&module.name, "share_config");
        let share_out = share_config_processor
          .merge_share_config(arkdown_profile::ShareConfigInput {
            project_root: &config.project_root,
            module_root: &module.src_path,
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
        let share_config_out = module_layout.share_config_temp_json();
        share_config_processor
          .write_share_config(&share_out, &share_config_out)
          .map_err(|e| anyhow::anyhow!("write share config failed: {}", e))?;
        share_out
      };

      if !share_out.entries.is_empty() {
        let profile_dir = module_layout.res_profile_dir();
        std::fs::create_dir_all(&profile_dir).ok();
        let profile_path = profile_dir.join("cross_app_shared_config.json");
        let profile_json = serde_json::json!({
            "crossAppSharedConfig": share_out.entries
                .iter()
                .map(|e| {
                    let mut obj = serde_json::json!({
                        "packageName": e.package_name,
                        "uri": e.uri,
                    });
                    if let Some(ref src) = e.src_entry {
                        obj["srcEntry"] = src.clone().into();
                    }
                    obj
                })
                .collect::<Vec<_>>()
        });
        if !skip_profile_writes {
          if let Err(e) = intermediate_writer.write_json_pretty(&profile_path, &profile_json) {
            tracing::warn!(
                error = %e,
                "failed to write cross_app_shared_config profile"
            );
          }
        }
      }

      let startup_out = {
        let _timer = BuildTimingProbe::module(&module.name, "startup_config");
        let startup_out = startup_config_processor
          .merge_startup_config(arkdown_profile::StartupConfigInput {
            project_root: &config.project_root,
            module_root: &module.src_path,
            module_name: &module.name,
            bundle_name: Some(config.product.bundle_name.as_str()),
            app_startup: module_json5.module.app_startup.clone(),
            collect_preload_system_so: config.product.build_option.preload_system_so
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
        let startup_config_out = module_layout.startup_config_temp_json();
        startup_config_processor
          .write_startup_config(&startup_out, &startup_config_out)
          .map_err(|e| anyhow::anyhow!("write startup config failed: {}", e))?;
        startup_out
      };

      if let Some(ref startup_config) = startup_out.config {
        if !startup_config.startup_tasks.is_empty()
          || !startup_config.app_preload_hint_startup_tasks.is_empty()
          || !startup_config.system_preload_hint_startup_tasks.is_empty()
        {
          let profile_dir = module_layout.res_profile_dir();
          std::fs::create_dir_all(&profile_dir).ok();
          let profile_path = profile_dir.join("startup_config.json");
          let clean_tasks: Vec<_> = startup_config
            .startup_tasks
            .iter()
            .map(|t| {
              let mut obj = serde_json::json!({
                  "name": t.name,
                  "srcEntry": t.src_entry,
              });
              if let Some(ref rt) = t.run_on_thread {
                obj["runOnThread"] = rt.clone().into();
              }
              if let Some(wait) = t.wait_on_main_thread {
                obj["waitOnMainThread"] = wait.into();
              }
              if !t.dependencies.is_empty() {
                obj["dependencies"] = t.dependencies.clone().into();
              }
              obj
            })
            .collect();
          let profile_json = serde_json::json!({
              "startupTasks": clean_tasks,
              "appPreloadHintStartupTasks": startup_config.app_preload_hint_startup_tasks
                  .iter()
                  .map(|t| serde_json::json!({"name": t.name, "srcEntry": t.src_entry}))
                  .collect::<Vec<_>>(),
              "systemPreloadHintStartupTasks": startup_config.system_preload_hint_startup_tasks
                  .iter()
                  .map(|t| {
                      let mut obj = serde_json::json!({
                          "name": t.name,
                          "srcEntry": t.src_entry,
                      });
                      if let Some(ref ohmurl) = t.ohmurl {
                          obj["ohmurl"] = ohmurl.clone().into();
                      }
                      obj
                  })
                  .collect::<Vec<_>>(),
          });
          if !skip_profile_writes {
            if let Err(e) = intermediate_writer.write_json_pretty(&profile_path, &profile_json) {
              tracing::warn!(
                  error = %e,
                  "failed to write startup_config resource profile"
              );
            }
          }
        }
      }

      let privacy_profile_path = {
        let _timer = BuildTimingProbe::module(&module.name, "module_privacy_profile");
        if BuildPolicy::module_processes_privacy_profile(&module.module_type) {
          privacy_profile_processor
            .process_module_privacy_profile(arkdown_profile::PrivacyProfileInput {
              module_root: &module.src_path,
              target_name: "default",
              pac_schema_path: pac_schema_path.as_deref(),
            })
            .map_err(|e| anyhow::anyhow!("ProcessModulePrivacyProfile failed: {}", e))?
            .map(|output| output.processed_path)
        } else {
          None
        }
      };

      let (manifest, mut loader_json) = {
        let _timer = BuildTimingProbe::module(&module.name, "process_module");
        processor
          .process_module_with_dependency_context(
            module,
            &module_json5,
            &profile_dependency_context,
          )
          .map_err(|e| anyhow::anyhow!("{}", e))?
      };

      // Profile disk writes.  Skipped for hot rebuilds (skip_profile=true)
      // since module.json5 / oh-package.json5 have not changed and the
      // previous build's outputs are still valid.
      let module_json_out = res_out.join("module.json");
      if !skip_profile_writes {
        {
          let _timer = BuildTimingProbe::module(&module.name, "write_module_json_initial");
          processor
            .write_module_json_with_dependency_context(
              &manifest,
              &module_json_out,
              None,
              &profile_dependency_context,
            )
            .map_err(|e| anyhow::anyhow!("write module.json failed: {}", e))?;
        }
        {
          let _timer = BuildTimingProbe::module(&module.name, "sanitizer_module_json_initial");
          sanitizer_profile_flags
            .apply_to_module_json(&module_json_out)
            .map_err(|e| anyhow::anyhow!("apply sanitizer profile flags failed: {}", e))?;
        }
        {
          let _timer = BuildTimingProbe::module(&module.name, "startup_profile_ref_update");
          ModuleJsonReader::new(&module_json_out.parent().unwrap_or(&module_json_out))
            .ensure_startup_profile_ref(&module_json_out, &startup_out, &intermediate_writer)
            .map_err(|e| anyhow::anyhow!("update startup module.json failed: {}", e))?;
        }
        {
          let _timer = BuildTimingProbe::module(&module.name, "distro_filter_profile");
          distro_filter_processor
            .process_distro_filter_profile(arkdown_profile::DistroFilterProfileInput {
              project_root: &project_root,
              module_name: &module.name,
              module_root: &module.src_path,
              product_name: &config.product.name,
              build_mode_name: Some(match config.build_mode {
                BuildMode::Debug => "debug",
                BuildMode::Release => "release",
              }),
              compile_api_version: config.sdk_config.compile_api_version,
              metadata: &module_json5.module.metadata,
              output_profile_dir: &res_layout.profile_dir(),
              module_json_path: Some(&module_json_out),
            })
            .map_err(|e| anyhow::anyhow!("process distro filter profile failed: {}", e))?;
        }
        {
          let _timer = BuildTimingProbe::module(&module.name, "profile_intermediates_initial");
          intermediate_writer.write_profile_intermediates(&module.src_path, &module_json_out)?;
        }

        let _timer = BuildTimingProbe::module(&module.name, "resource_config_write_initial");
        let module_res_dirs: Vec<PathBuf> = ResourceDirSelector::new(config.as_ref(), module)
          .select()
          .into_iter()
          .filter(|d| d.exists())
          .collect();
        if module_res_dirs.is_empty() {
          intermediate_writer.write_resource_config_files(
            &project_root,
            module,
            config.as_ref(),
            &res_out,
          )?;
        } else {
          intermediate_writer.write_resource_config_files_with_dirs(
            &project_root,
            module,
            config.as_ref(),
            &res_out,
            &module_res_dirs,
            &[],
          )?;
        }
      }

      let (app_scope_res, res_dirs) = {
        let _timer = BuildTimingProbe::module(&module.name, "resource_dir_select");
        let include_app_scope =
          ResourceDirSelector::new(config.as_ref(), module).include_app_scope();
        let mut res_dirs: Vec<PathBuf> = vec![];
        let app_scope_res = project_layout.app_scope_resources();
        if include_app_scope && app_scope_res.exists() {
          res_dirs.push(app_scope_res.clone());
        }
        if let Some(res_opts) = config.product.build_option.res_options.as_ref() {
          for dir in &res_opts.resource_directories {
            if dir.exists() && !res_dirs.contains(dir) {
              res_dirs.push(dir.clone());
            }
          }
        }
        for resource_module in
          ModuleSelectionPlanner::resource_modules_for_build(config.as_ref(), module)
        {
          for module_res_dir in ResourceDirSelector::new(config.as_ref(), resource_module).select()
          {
            if module_res_dir.exists() && !res_dirs.contains(&module_res_dir) {
              res_dirs.push(module_res_dir);
            }
          }
        }
        for dependency_res_dir in
          HarResourceDependencySelector::new(module_context.deps.as_ref()).select()
        {
          if !res_dirs.contains(&dependency_res_dir) {
            res_dirs.push(dependency_res_dir);
          }
        }
        (app_scope_res, res_dirs)
      };
      if !skip_profile_writes {
        let _timer = BuildTimingProbe::module(&module.name, "resource_config_write_final");
        let current_module_resource_dirs: Vec<PathBuf> =
          ResourceDirSelector::new(config.as_ref(), module)
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
            config.as_ref(),
            &res_out,
          )?;
        } else {
          intermediate_writer.write_resource_config_files_with_dirs(
            &project_root,
            module,
            config.as_ref(),
            &res_out,
            &current_module_resource_dirs,
            &dependency_resource_dirs,
          )?;
        }
      }

      let resource_backend =
        ResourceBackendRunner::default_sdk(config.sdk_config.restool_path.clone());
      let resource_compile_config = ResourceCompileConfig {
        package_name: config.product.bundle_name.clone(),
        module_name: module.name.clone(),
        module_names: config.modules.iter().map(|m| m.name.clone()).collect(),
        device_types: manifest_device_types,
        is_debug: matches!(config.build_mode, BuildMode::Debug),
      };

      if !skip_res && !res_dirs.is_empty() {
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
              sanitizer_profile_flags
                .apply_to_module_json(&module_json_out)
                .map_err(|e| anyhow::anyhow!("apply sanitizer profile flags failed: {}", e))?;
              intermediate_writer
                .write_profile_intermediates(&module.src_path, &module_json_out)?;
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

      let loader_json_out = module_layout.loader_json();
      if !skip_profile_writes {
        let _timer = BuildTimingProbe::module(&module.name, "loader_json_write_initial");
        processor
          .write_loader_json(&loader_json, &loader_json_out)
          .map_err(|e| anyhow::anyhow!("write loader.json failed: {}", e))?;
        intermediate_writer.write_static_intermediate_files(&module.src_path)?;
      }

      // Scan ByteCode HAR dependencies.
      //
      // Primary path: consume `DependencyGraphSnapshot::bytecode_har_nodes()`
      // from the session dependency graph.  When the session graph has
      // `BytecodeHar` nodes with `package_root` set, we read their
      // `oh-package.json5` directly rather than doing an independent
      // `oh_modules/` filesystem walk.
      //
      // Fallback: when the session graph provides no ByteCode HAR nodes
      // (e.g. the session was built without a full OHPM lock context),
      // fall back to the independent filesystem scan.
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
        let typed_roots: Vec<(String, std::path::PathBuf)> = module_context
          .deps
          .bytecode_har_nodes()
          .filter_map(|node| {
            node
              .package_root
              .as_ref()
              .map(|root| (node.package_name.clone(), root.clone()))
          })
          .collect();

        let bc_har_deps = ByteCodeHarMetadataWriter::scan_from_typed_roots(
          &typed_roots,
          &module.src_path,
          &project_root,
        );

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

      bc_har_deps.sort_by(|a, b| a.package_name.cmp(&b.package_name));

      loader_json.apply_bytecode_har_scan_entries(
        &bc_har_deps,
        &module.src_path,
        &config.project_root,
        config.output_target.clone(),
      );

      if !skip_profile_writes {
        let _timer = BuildTimingProbe::module(&module.name, "loader_json_write_final");
        processor
          .write_loader_json(&loader_json, &loader_json_out)
          .map_err(|e| anyhow::anyhow!("write loader.json with byteCodeHarInfo failed: {}", e))?;
        if loader_json.use_normalized_ohm_url {
          let _timer = BuildTimingProbe::module(&module.name, "pkg_context_info_write");
          let pkg_context_info =
            PkgContextInfoInputBuilder::new(config.as_ref(), &session, module, &module_context)
              .build()?;
          PkgContextInfoGenerator::new()
            .write(&pkg_context_info, &module_layout.pkg_context_info_json())
            .map_err(|e| anyhow::anyhow!("GeneratePkgContextInfo failed: {}", e))?;
        }
        let _timer = BuildTimingProbe::module(&module.name, "router_map_intermediates");
        intermediate_writer.write_router_map_intermediates(
          module,
          &loader_json,
          config.as_ref(),
          &res_out,
        )?;
      }

      loader_jsons.push((module.clone(), manifest, loader_json, resource_refs));
      all_bc_har_deps.push(bc_har_deps);
      privacy_profile_paths.push(privacy_profile_path);
    }
    progress.done();

    // ── Step 3: Bundling ──────────────────────────────────────────────
    progress.step(3, 6, "Bundling");
    let mut bundle_outs: Vec<(BundleStageOutput,)> = Vec::new();
    let mut widget_bundle_outs: Vec<Option<WidgetCompileArktsOutput>> = Vec::new();
    let obfuscation_enabled_for_release = config
      .product
      .build_option
      .ark_options
      .as_ref()
      .and_then(|options| options.obfuscation.as_ref())
      .is_some_and(|obfuscation| obfuscation.is_enabled_for_mode(&config.build_mode));
    let mut obfuscation_dependency_rule_files = Vec::new();
    let mut seen_obfuscation_dependency_rule_files = FxHashSet::default();
    for (module, manifest, loader_json, resource_refs) in &loader_jsons {
      let module_layout = ModuleBuildLayout::new(&module.src_path);
      let cache_dir = PathBuf::from(&loader_json.cache_path);
      let output_dir = PathBuf::from(&loader_json.ace_module_build);
      let cached_abc = output_dir.join("modules.abc");
      let widget_pass = WidgetCompileArktsPass::new(config.as_ref(), module, manifest, loader_json);
      let widget_compile_plan = widget_pass.plan()?;
      let task_inputs =
        TaskInputs::compute(config.as_ref(), loader_json).with_module_analyzer(module_analyzer);
      let module_obfuscation_dependency_rule_files = if obfuscation_enabled_for_release {
        let module_context = session
          .module_context(&module.name)
          .ok_or_else(|| anyhow::anyhow!("missing module compile context for {}", module.name))?;
        ObfuscationDependencyRuleCollector::new(module_context.deps.as_ref()).collect()
      } else {
        Vec::new()
      };
      for path in &module_obfuscation_dependency_rule_files {
        if seen_obfuscation_dependency_rule_files.insert(path.clone()) {
          obfuscation_dependency_rule_files.push(path.clone());
        }
      }
      let task_plugin_hash =
        package_plugins
          .iter()
          .fold(0xcbf29ce484222325u64, |mut hash, plugin| {
            for byte in plugin.cache_identity().bytes() {
              hash ^= u64::from(byte);
              hash = hash.wrapping_mul(0x100000001b3);
            }
            hash
          });
      // Wire resource_hash: hash of resource output directory state.
      let res_out = module_layout.res_default_dir();
      let task_resource_hash = {
        let mut h: u64 = 0xcbf29ce484222325;
        if let Ok(entries) = std::fs::read_dir(&res_out) {
          let mut names: Vec<_> = entries
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();
          names.sort();
          for name in &names {
            for b in name.bytes() {
              h ^= b as u64;
              h = h.wrapping_mul(0x100000001b3);
            }
          }
        }
        h
      };
      let task_inputs = task_inputs
        .with_plugin_hash(task_plugin_hash)
        .with_resource_hash(task_resource_hash)
        .with_additional_obfuscation_rule_files(&module_obfuscation_dependency_rule_files);
      let bundle_compile_env =
        CompileEnvKeyBuilder::new(config.as_ref(), module, loader_json).build()?;

      // Compute deterministic cache-group snapshots for this module.
      // Mirrors hvigor's invalid-cache-plugin.ts group tracking.
      let group_snapshots = build_cache_group_snapshots(
        &bundle_compile_env,
        config.as_ref(),
        module,
        loader_json,
        package_plugins,
        &project_root,
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
      let mut cache_hit = !disable_abc_cache
        && task_inputs.matches_cached(&cache_dir)
        && cached_abc.exists()
        && widget_cache_ready;
      if cache_hit {
        // System-level check: compile-env + file changes + cache group changes.
        let system_request = ShouldInvalidCacheRequest {
          compile_env: bundle_compile_env.clone(),
          changed_files: vec![],
          module_name: Some(module.name.clone()),
          cache_dir: Some(cache_dir.clone()),
          is_watch: watch_mode,
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
        // Per-plugin shouldInvalidCache check (plugin-specific conditions).
        let invalid_cache_request = ShouldInvalidCacheRequest {
          compile_env: bundle_compile_env.clone(),
          changed_files: vec![],
          module_name: Some(module.name.clone()),
          cache_dir: Some(cache_dir.clone()),
          is_watch: watch_mode,
          cache_hit,
          group_snapshots: group_snapshots.clone(),
          module_deps: vec![],
        };
        for plugin in package_plugins {
          if plugin.should_invalid_cache(&invalid_cache_request).await? {
            cache_hit = false;
            break;
          }
        }
      }

      if cache_hit {
        bundle_outs.push((BundleStageOutput::from_cache(cached_abc, output_dir),));
        widget_bundle_outs.push(None);
        continue;
      }

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

      let router_map_path = module_layout.router_map_loader();
      let router_map_cache = module_layout.router_map_hash_cache();
      if RouterMapCacheInvalidator::new(&router_map_path, &router_map_cache).should_invalidate() {
        let _ = RouterMapCacheInvalidator::new(&router_map_path, &router_map_cache).update();
      }

      let bm_name = match config.build_mode {
        BuildMode::Debug => "debug",
        BuildMode::Release => "release",
      };
      let applicable_targets =
        arkdown_config::build_profile::enumerate_applicable_targets_for_module(
          &project_root,
          &module.name,
          &config.product.name,
          bm_name,
        );

      let effective_target_name: Option<String> = if let Some(ref forced) = named_target {
        Some(forced.clone())
      } else if applicable_targets.len() > 1 {
        tracing::info!(
            module = %module.name,
            product = %config.product.name,
            applicable_targets = applicable_targets.len(),
            targets = ?applicable_targets.iter().map(|t| &t.name).collect::<Vec<_>>(),
            "multi-target: {} targets apply to module '{}' for product '{}' \
             in {} mode; building all applicable targets (section 3.1)",
            applicable_targets.len(),
            module.name,
            config.product.name,
            bm_name
        );
        applicable_targets.first().map(|t| t.name.clone())
      } else {
        applicable_targets.first().map(|t| t.name.clone())
      };

      let module_context = session.module_context(&module.name).ok_or_else(|| {
        anyhow::anyhow!("missing build session module context for {}", module.name)
      })?;
      let pkg_context_builder =
        PkgContextInfoInputBuilder::new(config.as_ref(), &session, module, &module_context);
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
        config.as_ref(),
        Some(&custom_cfg),
        module_analyzer,
        source_map_packages,
        runtime_packages,
      );
      bundle_plan.input.module_record_roots = module_record_roots;

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
        let bm_name = match config.build_mode {
          arkdown_core::BuildMode::Release => "release",
          arkdown_core::BuildMode::Debug => "debug",
        };
        let target_name = effective_target_name.clone().or_else(|| {
          arkdown_config::build_profile::select_effective_target(
            &config.project_root,
            &module.name,
            &config.product.name,
            bm_name,
          )
          .map(|t| t.name)
        });
        if let Some(tname) = target_name {
          if tname != "default" {
            let target_ets = context_layout.source().target_ets(&tname);
            if target_ets.is_dir() && target_ets != base_ets {
              vec![(base_ets, target_ets)]
            } else {
              vec![]
            }
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
      let transform_sources = TransformSourceSnapshotBuilder::new(
        &all_module_roots,
        &dependency_source_roots,
        &native_declaration_roots,
        &source_set_overlays,
      )
      .build()?;
      tracing::info!(
        module = %module.name,
        elapsed_ms = transform_snapshot_start.elapsed().as_millis(),
        "transform source snapshot built"
      );

      let plugin_factory_start = Instant::now();
      let factory_input = PluginFactoryInput {
        sdk_api_dirs: config.sdk_config.ets_api_dirs(),
        sdk_kits_dirs: config.sdk_config.ets_kits_dirs(),
        sdk_arkts_dirs: config.sdk_config.ets_arkts_dirs(),
        all_module_roots,
        resource_refs: resource_refs.clone(),
        component_metadata: component_metadata.clone(),
        transform_sources,
        allowed_package_roots,
        pkg_name_to_pkg_info,
        source_set_overlays,
        checker_mode,
        har_dedup_collect_file: None,
        disabled_plugins: custom_cfg.plugins.disabled.clone(),
        package_plugins: package_plugins.clone(),
      };
      let plugin_factory_output = PluginFactory::new(&bundle_plan, factory_input).build_output()?;
      let plugins = plugin_factory_output.plugins;
      let widget_plugins = plugins.clone();
      let ets_transform_plan = plugin_factory_output.ets_transform_plan;
      let widget_bundle_plan = widget_compile_plan
        .as_ref()
        .map(|plan| widget_pass.bundle_plan(&bundle_plan, plan));
      tracing::info!(
        module = %module.name,
        elapsed_ms = plugin_factory_start.elapsed().as_millis(),
        "bundle plugin factory completed"
      );

      // afterBuildStart — hvigor calls this after Rollup's buildStart.
      // In arkdown, build setup is complete at this point; Rolldown's
      // buildStart hook fires inside BundleStage::run().
      for plugin in package_plugins.iter() {
        plugin.after_build_start_with_share(&share_ctx_json).await?;
      }

      let bundle_stage_start = Instant::now();
      #[cfg(feature = "memory-diagnostics")]
      arkdown::memory_diagnostics::dump_with(
        "build:before-bundle-stage",
        format_args!("module={}", module.name),
      );
      let bundle_out = BundleStage::run(BundleStageInput {
        config: config.clone(),
        plugins,
        ets_transform_plan: Some(Arc::clone(&ets_transform_plan)),
        plan: bundle_plan,
      })
      .await?;
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
      // Collect scanned bundle module IDs to pass as context to
      // beforeBuildEnd.  Corresponds to hvigor's getModuleIds() in the
      // plugin context, most commonly used in beforeBuildEnd hooks.
      let bundle_module_ids = bundle_out.bundle_output.chunk_graph.all_module_ids();
      for plugin in package_plugins {
        plugin
          .before_build_end_with_context(&bundle_module_ids)
          .await?;
      }
      // afterBuildEnd — hvigor calls this after buildEnd.  Called with
      // success=true here since we'd have returned early on error above.
      for plugin in package_plugins.iter() {
        plugin.after_build_end(true).await?;
      }

      // Write bundle-level cache manifest.
      {
        let bundle_cache_dir = module_layout.default_cache_dir("bundle");
        let bundle_cache = BuildCache::new(bundle_cache_dir);
        let source_hash = {
          let mut h: u64 = 0xcbf29ce484222325;
          let mut sorted = loader_json.compile_entry.clone();
          sorted.sort();
          for entry in &sorted {
            for b in entry.bytes() {
              h ^= b as u64;
              h = h.wrapping_mul(0x100000001b3);
            }
          }
          h
        };
        let bundle_config_hash = {
          let key_str = format!(
            "{}#{}#{:?}#{}",
            config.sdk_config.compile_sdk_version,
            match config.build_mode {
              BuildMode::Debug => "debug",
              BuildMode::Release => "release",
            },
            config.output_target,
            module_analyzer.as_str()
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
            content_hash: BundleContentHash::new(source_hash).into_string(),
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
    for (((module, _manifest, loader_json, _resource_refs), (bundle_out,)), bc_har_deps) in
      loader_jsons
        .iter_mut()
        .zip(bundle_outs.iter_mut())
        .zip(all_bc_har_deps.iter())
    {
      if bundle_out.from_cache {
        continue;
      }
      let module_layout = ModuleBuildLayout::new(&module.src_path);
      let intermediates_dir = module_layout.intermediates();
      let module_src_main = PathBuf::from(&loader_json.module_root_path);
      let importee_cfg = ProcessImporteeConfig {
        bundle_name: config.product.bundle_name.clone(),
        module_name: loader_json.module_name.clone(),
        module_version: String::new(),
        project_root: project_root.clone(),
        module_src_main,
        output_dir: intermediates_dir,
      };
      match ProcessImporteeRunner::new(
        &bundle_out.bundle_output.module_source_files,
        &bundle_out.bundle_output.entry_module_ids,
        &bundle_out.bundle_output.module_imports,
        &importee_cfg,
      )
      .process()
      {
        Ok(result) => {
          loader_json.non_entry_importees = result.non_entry_importees;
          loader_json.compiled_importees = result.compiled_importees;
        }
        Err(e) => {
          tracing::warn!(error = %e, "process_importees failed");
        }
      }

      // F13 — collectImportersPlugin: write reverse importer map.
      {
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
    if let Some(obf_config) = config
      .product
      .build_option
      .ark_options
      .as_ref()
      .and_then(|a| a.obfuscation.as_ref())
    {
      let obf_cache_dir = project_layout.obfuscate_cache_dir();
      let dependency_rule_files = obfuscation_dependency_rule_files.clone();
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
        let obfuscator = Obfuscator::new(ObfuscationOptions {
          enable_obfuscation: obfuscation_plan.should_obfuscate(),
          rule_files,
          dependency_rule_files: dependency_rule_files.clone(),
          cache_dir: obf_cache_dir,
          export_rule_path: None,
        })
        .map_err(|e| anyhow::anyhow!("Obfuscation config invalid: {}", e))?;
        for path in ObfuscationTargetFileCollector::new(&bundle_outs).collect() {
          let tmp = path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| path.with_extension(format!("{extension}.obf_tmp")))
            .unwrap_or_else(|| path.with_extension("obf_tmp"));
          obfuscator
            .obfuscate_file(&path, &tmp)
            .map_err(|e| anyhow::anyhow!("Obfuscation failed: {}", e))?;
          std::fs::rename(&tmp, &path)?;
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
      (((module, manifest, loader_json, _resource_refs), (bundle_out,)), bc_har_deps),
    ) in loader_jsons
      .iter()
      .zip(bundle_outs.iter())
      .zip(all_bc_har_deps.iter())
      .enumerate()
    {
      let module_layout = ModuleBuildLayout::new(&module.src_path);
      if bundle_out.from_cache {
        continue;
      }
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
      let abc_input_files = bc_har_deps
        .iter()
        .map(|dep| BytecodeAbcInput {
          path: dep.abc_path.clone(),
          package_name: dep.package_name.clone(),
        })
        .collect::<Vec<_>>();
      let compile_context_info_path = if loader_json.use_normalized_ohm_url {
        let module_context = session
          .module_context(&module.name)
          .ok_or_else(|| anyhow::anyhow!("missing module compile context for {}", module.name))?;
        let pkg_context_input =
          PkgContextInfoInputBuilder::new(config.as_ref(), &session, module, &module_context)
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
      let bytecode_result = BytecodeStage::run(BytecodeStageInput {
        compiler: RolldownBytecodeCompiler::new(
          abc_ts2abc_backend_inner,
          config.sdk_config.es2abc_path.clone(),
          config.sdk_config.compile_api_version,
          matches!(config.build_mode, BuildMode::Debug),
        ),
        module_source_files: bundle_out.bundle_output.module_source_files.clone(),
        external_module_requests: bundle_out.bundle_output.external_module_requests.clone(),
        npm_entries: bundle_out.bundle_output.npm_entries.clone(),
        modules_abc_path: bundle_out.bundle_output.modules_abc_path.clone(),
        cache_dir: PathBuf::from(&loader_json.cache_path),
        cache_file: effective_cache_file,
        abc_input_files: abc_input_files.clone(),
        compile_context_info_path,
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

      // ── P12: Widget compile pass ──────────────────────────────────
      if let Some(widget_out) = widget_bundle_outs
        .get(bundle_index)
        .and_then(Option::as_ref)
      {
        let widget_loader_json =
          WidgetCompileArktsPass::new(config.as_ref(), module, manifest, loader_json)
            .compile_context_loader_json(&widget_out.plan);
        let widget_compile_context_info_path = if loader_json.use_normalized_ohm_url {
          let module_context = session
            .module_context(&module.name)
            .ok_or_else(|| anyhow::anyhow!("missing module compile context for {}", module.name))?;
          let pkg_context_input =
            PkgContextInfoInputBuilder::new(config.as_ref(), &session, module, &module_context)
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
            config.sdk_config.compile_api_version,
            matches!(config.build_mode, BuildMode::Debug),
          ),
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
          abc_input_files: abc_input_files.clone(),
          compile_context_info_path: widget_compile_context_info_path,
        })?;
      }

      if let Some(source_maps_path) = &bundle_out.bundle_output.source_maps_path {
        intermediate_writer
          .copy_if_exists(source_maps_path, &module_layout.source_maps_intermediate())?;
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
      let task_inputs =
        TaskInputs::compute(config.as_ref(), loader_json).with_module_analyzer(module_analyzer);
      let task_plugin_hash =
        package_plugins
          .iter()
          .fold(0xcbf29ce484222325u64, |mut hash, plugin| {
            for byte in plugin.cache_identity().bytes() {
              hash ^= u64::from(byte);
              hash = hash.wrapping_mul(0x100000001b3);
            }
            hash
          });
      let res_out = module_layout.res_default_dir();
      let task_resource_hash = {
        let mut h: u64 = 0xcbf29ce484222325;
        if let Ok(entries) = std::fs::read_dir(&res_out) {
          let mut names: Vec<_> = entries
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();
          names.sort();
          for name in &names {
            for b in name.bytes() {
              h ^= b as u64;
              h = h.wrapping_mul(0x100000001b3);
            }
          }
        }
        h
      };
      let task_inputs = task_inputs
        .with_plugin_hash(task_plugin_hash)
        .with_resource_hash(task_resource_hash);
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
    }
    progress.done();

    let bundle_package_outs: Vec<(BundlePackageOutput,)> = bundle_outs
      .into_iter()
      .map(|(bundle_out,)| (bundle_out.into_package_output(),))
      .collect();

    // ── Step 6: Packaging + Signing ───────────────────────────────────
    progress.step(6, 6, "Packaging");
    let packager = Packager::new(config.clone());
    let mut output_paths = Vec::new();

    for (((module, manifest, loader_json, _resource_refs), (package_out,)), privacy_profile_path) in
      loader_jsons
        .iter()
        .zip(bundle_package_outs.iter())
        .zip(privacy_profile_paths.iter())
    {
      let module_layout = ModuleBuildLayout::new(&module.src_path);
      let module_json = module_layout.res_module_json();
      let resources_index = module_layout.res_resources_index();
      let modules_abc = package_out.modules_abc_path.clone();
      let ext = match &config.output_target {
        OutputTarget::Hsp => "hsp",
        OutputTarget::Har => "har",
        _ => "hap",
      };
      let output_dir = module_layout.outputs_default();

      let packaging_bm_name = match config.build_mode {
        BuildMode::Debug => "debug",
        BuildMode::Release => "release",
      };
      let all_applicable = arkdown_config::build_profile::enumerate_applicable_targets_for_module(
        &project_root,
        &module.name,
        &config.product.name,
        packaging_bm_name,
      );
      let target_labels: Vec<String> = if all_applicable.len() > 1 {
        tracing::info!(
            module = %module.name,
            product = %config.product.name,
            applicable_targets = all_applicable.len(),
            targets = ?all_applicable.iter().map(|t| &t.name).collect::<Vec<_>>(),
            "multi-target: producing {} artifacts for module '{}' (section 3.1)",
            all_applicable.len(),
            module.name,
        );
        all_applicable.iter().map(|t| t.name.clone()).collect()
      } else if let Some(first) = all_applicable.first() {
        vec![first.name.clone()]
      } else {
        vec!["default".to_string()]
      };

      std::fs::create_dir_all(&output_dir)?;

      for artifact_target_label in &target_labels {
        let module_json = module_json.clone();
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

        let out = match &config.output_target {
          OutputTarget::Har => {
            let source_dir = module_layout.source().ets();
            let oh_package_json = module_layout.oh_package_json5();
            if source_dir.exists() {
              let declarations_dir = module_layout.har_dir();
              HarDeclarationGenerator::new(&module.src_path, &source_dir, &module.name)
                .generate()
                .unwrap_or_else(|e| {
                  tracing::warn!(
                      error = %e,
                      "failed to generate HAR declarations"
                  );
                });

              let consumer_rules_path = project_layout.obfuscation_txt();

              // ByteCode HAR declarationEntry — collect OHM URLs for
              // workers + abilities and write to oh-package.json5#metadata.declarationEntry
              // so downstream HAP/HSP consumers can read them from byteCodeHarInfo.
              // Matches hvigor's processDeclarationEntry() in process-har-artifacts.ts.
              {
                let decl_entry = ByteCodeHarDeclarationCollector::collect_from_loader(loader_json);
                if !decl_entry.is_empty() {
                  if let Err(e) = ByteCodeHarDeclarationCollector::write_to_package_metadata(
                    &decl_entry,
                    &oh_package_json,
                  ) {
                    tracing::warn!(
                        error = %e,
                        "failed to write declarationEntry to oh-package.json5 (non-fatal)"
                    );
                  }
                }
              }

              let packed = packager
                .pack_har(HarPackInput {
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
                })
                .map_err(|e| anyhow::anyhow!("Pack failed: {}", e))?;

              let har_info_out = module_layout.har_extend_info_json();
              if let Some(parent) = har_info_out.parent() {
                std::fs::create_dir_all(parent).ok();
              }
              let exported_sources: Vec<String> = walkdir::WalkDir::new(&source_dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| {
                  e.file_type().is_file()
                    && e
                      .path()
                      .extension()
                      .and_then(|x| x.to_str())
                      .map_or(false, |x| x == "ets")
                })
                .filter_map(|e| {
                  e.path()
                    .strip_prefix(&source_dir)
                    .ok()
                    .and_then(|r| ArtifactPathSerializer::path_to_slash_string(r).ok())
                })
                .collect();
              let har_info = serde_json::json!({
                  "moduleName": module.name,
                  "moduleType": "har",
                  "exportedSources": exported_sources,
                  "abilities": manifest.abilities.iter().map(|a| serde_json::json!({
                      "name": a.name,
                      "srcEntry": a.src_entry,
                  })).collect::<Vec<_>>(),
              });
              intermediate_writer
                .write_json_pretty(&har_info_out, &har_info)
                .unwrap_or_else(|e| {
                  tracing::warn!(
                      error = %e,
                      "failed to write har-extend-info.json"
                  );
                });

              Some(packed)
            } else {
              None
            }
          }
          _ => {
            if module_json.exists() && modules_abc.exists() {
              if !resources_index.exists() {
                if let Some(parent) = resources_index.parent() {
                  std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&resources_index, &[])?;
              }
              {
                let raw_source_maps_path = package_out.source_maps_path.clone().or_else(|| {
                  let candidate = module_layout.loader_out_source_maps();
                  candidate.exists().then_some(candidate)
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
                let bundle_version_code = session.project.version_code;
                let bundle_version_name = session.project.version_name.clone();
                let bundle_type = if matches!(config.output_target, OutputTarget::Hsp) {
                  "shared".to_string()
                } else {
                  session.project.bundle_type.clone()
                };
                let delivery_with_install = session
                  .module_context(&module.name)
                  .and_then(|ctx| {
                    ctx
                      .module
                      .module_json
                      .as_ref()
                      .and_then(|module_json| module_json.module.delivery_with_install)
                  })
                  .unwrap_or(true);
                let module_context = session.module_context(&module.name).ok_or_else(|| {
                  anyhow::anyhow!("missing build session module context for {}", module.name)
                })?;
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
                        .map(|p| p.version.clone())
                        .filter(|v| !v.trim().is_empty())
                        .unwrap_or_else(|| "1.0.0".to_string()),
                    })
                  })
                  .collect();
                let integrated_hsp_start = Instant::now();
                let integrated_hsps = {
                  BuildPolicy::new(config.as_ref())
                    .validate_integrated_hsp_api(!integrated_hsp_candidates.is_empty())?;
                  let out_dir = module_layout.integrated_hsp_dir();
                  IntegratedHspCollector::new(&integrated_hsp_candidates)
                    .collect_normalized(&config.product.bundle_name, bundle_version_code, &out_dir)
                    .map_err(|e| {
                      anyhow::anyhow!("K5: failed to normalize integrated HSPs: {}", e)
                    })?
                };
                tracing::info!(
                  module = %module.name,
                  elapsed_ms = integrated_hsp_start.elapsed().as_millis(),
                  "integrated hsp collection completed"
                );
                let native_build_option = resolve_effective_native_build_option(
                  &module.src_path,
                  &config.product.build_option.abi_filters,
                  &config.product.build_option.native_lib,
                  packaging_bm_name,
                  artifact_target_label,
                )
                .map_err(|e| anyhow::anyhow!("resolve native build option failed: {}", e))?;
                let self_package = module_context
                  .module
                  .package
                  .as_ref()
                  .map(|package| {
                    NativeLibPackageInfo::local(package.name.clone(), package.version.clone())
                  })
                  .unwrap_or_else(|| {
                    NativeLibPackageInfo::local(module.name.clone(), String::new())
                  });
                let native_lib_start = Instant::now();
                let native_policy = sanitizer_native_policy
                  .clone()
                  .unwrap_or_default()
                  .with_native_lib_option(native_build_option.native_lib)
                  .with_self_package(self_package);
                let native_libs = NativeLibCollector::new(
                  module,
                  &native_build_option.abi_filters,
                  module_context.deps.as_ref(),
                )
                .with_policy(native_policy)
                .collect()
                .map_err(|e| anyhow::anyhow!("ProcessLibs failed: {}", e))?;
                tracing::info!(
                  module = %module.name,
                  elapsed_ms = native_lib_start.elapsed().as_millis(),
                  native_lib_count = native_libs.len(),
                  "native libs collected"
                );
                let compress_native_libs =
                  PackageModuleJsonOptions::new(&module_json).compress_native_libs()?;
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
                let packed = packager
                  .pack_hap(PackInput {
                    module_name: module.name.clone(),
                    abc_path: modules_abc,
                    resources_index,
                    module_json,
                    native_libs,
                    compress_native_libs,
                    output_path: output.clone(),
                    resources_src_dir: Some(module_layout.res_resources_dir()),
                    bundle_version_code,
                    bundle_version_name,
                    bundle_type: bundle_type.clone(),
                    source_maps_path,
                    pkg_context_info_path: loader_json
                      .use_normalized_ohm_url
                      .then(|| module_layout.pkg_context_info_json()),
                    abilities,
                    integrated_hsps,
                    device_types: manifest.device_types.clone(),
                    installation_free: bundle_type == "atomicService",
                    delivery_with_install,
                    privacy_profile_path: privacy_profile_path.clone(),
                  })
                  .map_err(|e| anyhow::anyhow!("Pack failed: {}", e))?;
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

        if let Some(path) = out {
          let final_path = if matches!(&config.output_target, OutputTarget::Har) {
            if config.product.signing_config.is_some() {
              let sign_backend = SignBackendRunner::rust();
              sign_backend
                .sign_or_skip(
                  &path,
                  &signed_output,
                  config.product.signing_config.as_ref(),
                )
                .map_err(|e| anyhow::anyhow!("HAR sign failed: {}", e))?;
              signed_output.clone()
            } else {
              path
            }
          } else {
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
            let package_metadata_start = Instant::now();
            let package_module_json = module_layout.package_module_json();
            intermediate_writer.extract_zip_entry(&path, "module.json", &package_module_json)?;
            intermediate_writer.rewrite_package_intermediate_module_json(&package_module_json)?;
            intermediate_writer.write_hap_metadata(
              &module.src_path,
              &module.name,
              &manifest.device_types,
            )?;
            intermediate_writer.extract_zip_entry(
              &path,
              "pack.info",
              &module_layout.output_pack_info(),
            )?;
            if let Some(source_maps_path) = &package_out.source_maps_path {
              intermediate_writer.copy_if_exists(
                source_maps_path,
                &module_layout.output_mapping_source_maps(),
              )?;
            }
            tracing::info!(
              module = %module.name,
              elapsed_ms = package_metadata_start.elapsed().as_millis(),
              "package intermediate metadata written"
            );
            signed_output.clone()
          };
          output_paths.push(final_path);
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

    // ── Watch mode ────────────────────────────────────────────────────
    if watch_mode {
      WatchStage::run(WatchStageInput {
        config: config.clone(),
        target: args.target.clone(),
        mode: args.mode.clone(),
        module_filter: args.module.clone(),
        product_filter: args.product.clone(),
        project_root: project_root.clone(),
        abc_backend,
        build_target_name: args.build_target_name.clone(),
        coverage: coverage_mode,
        ohos_debug_asan: args.ohos_debug_asan,
        ohos_enable_hwasan: args.ohos_enable_hwasan,
        disable_abc_cache,
        cache_file: cli_cache_file.clone(),
        checker: args.checker.clone(),
        module_analyzer: args.module_analyzer,
        cache: args.cache,
        package_plugins: package_plugins.clone(),
        preview_mode,
      })?;
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
}

impl<'a> ResourceDirSelector<'a> {
  fn new(config: &'a ArkdownConfig, module: &'a ModuleConfig) -> Self {
    Self { config, module }
  }

  fn select(&self) -> Vec<PathBuf> {
    let build_mode_name = match self.config.build_mode {
      BuildMode::Debug => "debug",
      BuildMode::Release => "release",
    };
    let Some(target) = self.effective_target(build_mode_name) else {
      return vec![ModuleBuildLayout::new(&self.module.src_path)
        .source()
        .resources()];
    };
    let target_dirs = self.resolve_target_dirs(&target.resource_directories);
    if !target_dirs.is_empty() {
      return target_dirs;
    }
    if let Some(source_set_res) = self.source_set_dir(&target.name) {
      return vec![source_set_res];
    }
    vec![ModuleBuildLayout::new(&self.module.src_path)
      .source()
      .resources()]
  }

  fn effective_target(&self, build_mode_name: &str) -> Option<EffectiveResourceTarget> {
    let cfg = arkdown_config::build_profile::select_effective_target(
      &self.config.project_root,
      &self.module.name,
      &self.config.product.name,
      build_mode_name,
    )?;
    Some(EffectiveResourceTarget {
      name: cfg.name,
      resource_directories: cfg.resource_directories,
    })
  }

  fn resolve_target_dirs(&self, directories: &[PathBuf]) -> Vec<PathBuf> {
    directories
      .iter()
      .filter(|dir| !dir.as_os_str().is_empty())
      .map(|dir| {
        if dir.is_absolute() {
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

struct EffectiveResourceTarget {
  name: String,
  resource_directories: Vec<PathBuf>,
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
  deps: &'a arkdown_session::dependency_graph::snapshot::DependencyGraphSnapshot,
}

impl<'a> HarResourceDependencySelector<'a> {
  fn new(deps: &'a arkdown_session::dependency_graph::snapshot::DependencyGraphSnapshot) -> Self {
    Self { deps }
  }

  fn select(&self) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let mut seen = FxHashSet::default();
    for node in self.deps.nodes.values() {
      if !matches!(
        node.node_type,
        DependencyNodeType::SourceHar | DependencyNodeType::OhpmSource
      ) {
        continue;
      }
      let Some(package_root) = node.package_root.as_deref() else {
        continue;
      };
      for resource_dir in self.resource_dirs(node, package_root) {
        if !resource_dir.is_dir() {
          continue;
        }
        let key = canonicalize_or_owned(&resource_dir);
        if seen.insert(key.clone()) {
          roots.push(key);
        }
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
      roots.push(if path.is_absolute() {
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

// ---------------------------------------------------------------------------
// RouterMapCacheInvalidator — same as in runner.rs
// ---------------------------------------------------------------------------

struct RouterMapCacheInvalidator<'a> {
  router_map_path: &'a Path,
  cache_path: &'a Path,
}

impl<'a> RouterMapCacheInvalidator<'a> {
  fn new(router_map_path: &'a Path, cache_path: &'a Path) -> Self {
    Self {
      router_map_path,
      cache_path,
    }
  }

  fn should_invalidate(&self) -> bool {
    let Ok(content) = std::fs::read_to_string(self.router_map_path) else {
      return false;
    };
    let current_hash = arkdown_cache::CacheKeyMaterial::hash_u64(&content);
    let cached: u64 = match std::fs::read_to_string(self.cache_path) {
      Ok(s) => s.trim().parse().unwrap_or(0),
      Err(_) => return true,
    };
    current_hash != cached
  }

  fn update(&self) -> std::io::Result<()> {
    let content = std::fs::read_to_string(self.router_map_path)?;
    let hash = arkdown_cache::CacheKeyMaterial::hash_u64(&content);
    if let Some(parent) = self.cache_path.parent() {
      std::fs::create_dir_all(parent)?;
    }
    std::fs::write(self.cache_path, hash.to_string())
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

struct BundleContentHash {
  hash: u64,
}

impl BundleContentHash {
  fn new(hash: u64) -> Self {
    Self { hash }
  }

  fn into_string(self) -> String {
    HexText::lower_u64(self.hash)
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
