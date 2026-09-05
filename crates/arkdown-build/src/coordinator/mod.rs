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
use rustc_hash::{FxHashMap, FxHashSet};

use crate::pre_build::PreBuildValidator;
use arkdown_abc::aot::AotArtifactWriter;
use arkdown_abc::bytecode::{
    BytecodeAbcInput, RolldownBytecodeBackendChoice, RolldownBytecodeCompileInput,
    RolldownBytecodeCompiler,
};
use arkdown_core::{
    ArkdownConfig, BuildMode, HapManifest, HarArtifactType, ModuleConfig, OhosModuleType,
    OutputTarget,
};
use arkdown_pack::PackingCheck;
use arkdown_path::{canonicalize_or_owned, ArtifactPathSerializer};
use arkdown_profile::{
    FormWidgetAbilityMerger, InsightIntentEntityInput, InsightIntentInput,
    InsightIntentModuleInput, InsightIntentProfileInput, InsightIntentProfileProcessor,
};
use arkdown_session::dependency_graph::snapshot::{
    DependencyEdgeKind, DependencyNodeSnapshot, DependencyNodeType,
};
use arkdown_session::{BuildSession, ProjectSnapshot, SdkToolKind};
use arkdown_sign::{HarSignBackendRunner, SignBackendRunner};

use super::native_stage::NativeDependencyStageExecutor;
use super::watch_stage::{WatchStage, WatchStageBuildInput, WatchStageInput};
use crate::bundle_plan::{
    BundlePackageOutput, BundlePlanBuilder, BundlePlanBuilderInput, BundlePlanPackageInfo,
    BundleRuntimePackageInfoBuilder, BundleStage, BundleStageInput, BundleStageOutput,
    HvigorCompileTarget,
};
use crate::bytecode_har_declaration::ByteCodeHarDeclarationCollector;
use crate::code_resource::CodeResourceCopyPlan;
use crate::compile_context::{
    CompileContextInfoWriter, CompileEnvKeyBuilder, PackageModuleJsonOptions,
    PkgContextInfoInputBuilder, SanitizerProfileFlags,
};
use crate::har_dedup::HarDedupProjectPlan;
use crate::hot_reload::{
    HotReloadCompileInput, HotReloadCompilePhase, HotReloadProcessor, HotReloadTaskPlan,
    ReloadResourceTaskPlan,
};
use crate::intermediates::{
    IntermediateArtifactWriter, PackageMetadataInput, RelatedEntryMetadata, ResourceConfigInput,
    SourceMapPackageHandoff,
};
use crate::layout::{
    ModuleBuildLayout, ModuleSourceLayout, ProjectBuildLayout, ResourceOutputLayout,
};
use crate::module_plan::ModuleSelectionPlanner;
use crate::quickfix::{
    ColdReloadProcessor, QuickFixArtifactInput, QuickFixArtifactProcessor, QuickFixTaskBoundary,
    QuickFixTaskPlan, QuickFixVersionStore,
};
use crate::remote_hsp_sign::{RemoteHspSignScope, RemoteHspSigner};
use crate::request::{AbcBackendChoice, BuildRequest};
use crate::source_snapshot::TransformSourceSnapshotBuilder;
use crate::static_arkts::{StaticArktsCompileInput, StaticArktsCompiler};
use arkdown::plugins::factory::{
    HarDedupFactoryMode, PackagePlugin, PluginFactory, PluginFactoryInput,
};
use arkdown::{
    BundleCacheBridge, BundleMockDependency, CrossplatformCollectionWriter,
    PluginCacheModuleInventory, ShouldInvalidCacheRequest, SourceMapPolicy,
};

use crate::package_resolution::{LocalPackageSourceRootInventory, ModulePackageResolutionPlanner};
use crate::progress::Progress;
use crate::sdk_checker::{
    ArkTsCheckerSelection, SdkEtsCheckerBuildPaths, SdkEtsCheckerEntryCollector,
    SdkEtsCheckerInput, SdkEtsCheckerPaths, SdkEtsCheckerPlanBuilder, SdkSemanticCheckStage,
};
use crate::task_cache::{TaskInputSnapshot, TaskInputs};
use arkdown_native::{
    DebugSymbolCollector, NativeCmakeLocalDependency, NativeLibCollectionPolicy,
    NativeLibCollector, NativeLibPackageInfo, NativeSdkToolchain,
};

use arkdown::plugins::ets_transform::{
    CheckerMode, ComponentMetadata, InsightIntentSchemaCatalog, ModuleAnalyzerKind,
    NavigationRouteMap, ResourceModuleReference, ResourceReference, ResourceTransformContext,
    SystemApiCheckOptions, SystemResourceReferenceLoader,
};
use arkdown::plugins::ohos_resolve::{ProcessImporteeConfig, ProcessImporteeRunner};
use arkdown_cache::{
    BuildCache, BundleCacheKey, CacheGroupKey, CacheGroupSnapshot, CacheKey, HarImporteeCache,
};
use arkdown_config::build_profile::{parse_api_version, EffectiveTargetConfig};
use arkdown_ets_transform::{
    ArkuiComponentNameCollector, CoverageInstrumenter, HarDeclarationGenerator,
    RolldownEtsTransformPlan,
};
use arkdown_obfuscate::{
    ObfuscationMode, ObfuscationOptions, ObfuscationOutputPaths, ObfuscationPlan, Obfuscator,
};
use arkdown_pack::{
    AppPackageRunner, BundledHarResourceInput, HarBundledDependencyInput, HarPackInput,
    HarPackageParameterInput, HarVariantInput, HspInterfaceHarInput, HspTgzInput,
    IntegratedHspCandidate, IntegratedHspCollector, PackInfoInput, PackInfoModuleInput, PackInput,
    Packager,
};
use arkdown_profile::{
    BuildProfileInput, BuildProfileKind, BuildProfileWriter, ByteCodeHarMetadataWriter,
    ByteCodeHarScanEntry, CustomTypeResolver, FormProfileProcessor, LoaderByteCodeHarFields,
    LoaderJson, ModuleFormProfileInput, OhExportsProcessor, PkgContextInfoGenerator,
    PreloadSystemSoArtifactWriter, PreloadSystemSoPlan, ProfileProcessor,
};
use arkdown_resolver::PackageResolutionPlan;
use arkdown_resource::{
    IdDefinedFileProcessor, ResourceBackendRunner, ResourceCompileConfig, ResourceCompileInput,
    ResourceReferenceInventory, SyscapTransform, SyscapTransformInput,
};

mod bundle_stage;
mod bytecode_stage;
use crate::obfuscation::{
    ModuleArkguardPlanBuilder, ModuleObfuscationContext, PendingBytecodeObfuscation,
};
use crate::planning::{
    BuildTimingProbe, DependencyCustomTypeInputBuilder, DirectoryTreeCopier,
    HarDeclarationDependencyPackageInventory, ModuleBuildConfiguration, ModuleTargetSelection,
    NativeCmakeLocalDependencyInventoryBuilder, NativeDependencyStagePlanBuilder,
    TargetBuildExecutionPlan,
};
use crate::plugin_share::{
    HvigorCompilePluginShareBuilder, HvigorCompilePluginShareInput, PluginCacheInvalidationRunner,
    PluginContextCacheRunner,
};

mod entry;
mod executor;
mod package_sign_stage;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Typed output of a complete build run.
pub struct BuildOutput {
    /// Absolute paths to all produced artifacts (signed HAP/HSP/HAR files).
    pub artifact_paths: Vec<PathBuf>,
}

type ModuleBuildRecord = (
    ModuleConfig,
    HapManifest,
    LoaderJson,
    FxHashMap<String, ResourceReference>,
    String,
    Arc<ArkdownConfig>,
);

#[derive(Debug, Clone)]
struct ModulePackInfoPlan {
    input: PackInfoInput,
    output_path: PathBuf,
    /// Related entry suffix used by `getModuleTargetOutputFileName`.
    related_entry: Option<String>,
}

/// Resolves the source resource model consumed by Hvigor's `MakePackInfo`.
///
/// `ModuleModelImpl.getFormJsonObjByTargetName` keeps extension-ability
/// metadata on the HAP module, but redirects each referenced form profile to
/// the HSP named by `formWidgetModule`. The redirected module independently
/// selects the requested source set and falls back to `default` when that
/// source set is absent.
pub(super) struct PackInfoFormResourceResolver<'a> {
    project: &'a ProjectSnapshot,
}

impl<'a> PackInfoFormResourceResolver<'a> {
    pub(super) fn new(project: &'a ProjectSnapshot) -> Self {
        Self { project }
    }

    pub(super) fn resolve(
        &self,
        module_name: &str,
        target_name: &str,
        module_json: &serde_json::Value,
    ) -> Result<Vec<PathBuf>> {
        let current_module = self.project.modules.get(module_name).ok_or_else(|| {
            anyhow::anyhow!("missing build session module snapshot for '{module_name}'")
        })?;
        let resource_module = module_json
            .pointer("/module/formWidgetModule")
            .and_then(serde_json::Value::as_str)
            .and_then(|widget_module| self.project.modules.get(widget_module))
            .unwrap_or(current_module);
        let source = ModuleSourceLayout::new(&resource_module.root);
        let resources =
            if target_name == "ohosTest" && resource_module.ohos_test_module_json_value.is_some() {
                source.target_resources("ohosTest")
            } else {
                source.resources()
            };
        Ok(vec![resources])
    }
}

#[derive(Debug, Clone)]
struct ModulePackageArtifact {
    pack_info_path: PathBuf,
    package_path: PathBuf,
    signed_path: PathBuf,
}

impl ModulePackageArtifact {
    fn for_module_target(
        module: &ModuleConfig,
        module_config: &ArkdownConfig,
        target_name: &str,
        extension: &str,
        output_dir: &Path,
        pack_info_plans: &[ModulePackInfoPlan],
    ) -> Vec<Self> {
        let plans = if matches!(module_config.output_target, OutputTarget::Hap) {
            pack_info_plans
        } else {
            pack_info_plans.get(..1).unwrap_or_default()
        };
        plans
            .iter()
            .map(|plan| {
                let archive_module_name = plan
                    .related_entry
                    .as_deref()
                    .map(|entry| format!("{}-{entry}", module.name))
                    .unwrap_or_else(|| module.name.clone());
                let name = ArtifactOutputName::new(
                    module_config.product.artifact_name.as_deref(),
                    &archive_module_name,
                    target_name,
                    extension,
                );
                let package_path = if matches!(module_config.output_target, OutputTarget::Har) {
                    output_dir.join(name.har())
                } else {
                    output_dir.join(name.unsigned())
                };
                let signed_path = if matches!(module_config.output_target, OutputTarget::Har) {
                    output_dir.join(name.signed_har())
                } else {
                    output_dir.join(name.signed())
                };
                Self {
                    pack_info_path: plan.output_path.clone(),
                    package_path,
                    signed_path,
                }
            })
            .collect()
    }
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
    dependencies: &'a arkdown_session::DependencyGraphSnapshot,
}

impl<'a> WidgetCompileArktsPass<'a> {
    fn new(
        config: &'a ArkdownConfig,
        module: &'a ModuleConfig,
        manifest: &'a HapManifest,
        loader_json: &'a LoaderJson,
        dependencies: &'a arkdown_session::DependencyGraphSnapshot,
    ) -> Self {
        Self {
            config,
            module,
            manifest,
            loader_json,
            dependencies,
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
        loader_json.pkg_name2_other_compile_files = Some(if transitive_deps {
            LoaderByteCodeHarFields::collect_pkg_name2_other_compile_files_from_graph(
                self.dependencies,
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
        });
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
            let names =
                ArkuiComponentNameCollector::with_options(&content, &source.module_id, &options)
                    .collect()
                    .map_err(|err| {
                        anyhow::anyhow!("collect widget component names failed: {}", err)
                    })?;
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
#[cfg(test)]
mod transform_source_snapshot_builder_tests {
    use super::*;
    use arkdown_ets_transform::{RolldownEtsTransformPlan, RolldownEtsTransformer};

    #[test]
    fn json_modules_are_preloaded_before_rolldown_hooks_run() {
        let temp = tempfile::tempdir().expect("tempdir");
        let module_root = temp.path().join("entry");
        let source_root = module_root.join("src/main/ets");
        let json = source_root.join("configuration.json5");
        fs_err::create_dir_all(&source_root).expect("source root");
        fs_err::write(&json, "{ featureEnabled: true }\n").expect("json source");

        let module_roots = vec![("entry".to_string(), module_root)];
        let sources = TransformSourceSnapshotBuilder::new(&module_roots, &[], &[], &[])
            .build()
            .expect("source snapshot");

        assert_eq!(
            sources.source_text(&json).as_deref(),
            Some("{ featureEnabled: true }\n")
        );
    }

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
            output.code.contains(
                r#"import type { EncryptionResult } from "../../cpp/types/libmtcrypto";"#
            ),
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
            output.code.contains(
                r#"import type { EncryptionResult } from "../../cpp/types/libmtcrypto";"#
            ),
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

#[cfg(test)]
mod compile_context_info_tests {
    use super::*;
    use arkdown_core::HvigorModuleSourceFile;
    use arkdown_profile::PkgContextInfoOpt;
    use indexmap::IndexMap;

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
                record_name: "&entry/build/default/generated/profile/default/ModuleInfo&1.0.0"
                    .to_string(),
                source_file:
                    "entry|entry|1.0.0|build/default/generated/profile/default/ModuleInfo.js"
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
                path: temp.path().join(
                    "cache/oh_modules/@mrn/mrn/src/main/ets/mapi/apis/getfeedcommentlist_bin.ts",
                ),
                record_name: "&@mrn/mrn/src/main/ets/mapi/apis/getfeedcommentlist_bin&12.59.403"
                    .to_string(),
                source_file:
                    "meituan|@mrn/mrn|12.59.403|src/main/ets/mapi/apis/getfeedcommentlist_bin.ts"
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
                    "&@mrn/mrn/src/main/ets/config/MRNConfigCenter&12.59.403".to_string(),
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
        let subscribe_form = temp.path().join(
            "oh_modules/@meituan/card_subscribe/src/main/ets/subscribeForm/PTSubscribeForm.ets",
        );
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
            node_modules_path:
                "/project/build/default/intermediates/loader_out/default/node_modules".to_string(),
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
            insight_intent_form_bindings: vec![],
            runtime_entry_sources: vec![],
            page_entry_sources: vec![],
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
    let module_layout = ModuleBuildLayout::for_output_target_product(
        &config.output_target,
        &module.src_path,
        &config.product.name,
        "default",
    );

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
    let mock_config_path = ModuleSourceLayout::new(&module.src_path).mock_config_json5();
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

/// The build coordinator.
///
/// Owns the explicit coarse sequencing of all build stages for a single
/// `arkdown build` invocation.  Constructed from a parsed [`BuildSession`]
/// and a typed [`BuildRequest`]; call [`BuildCoordinator::execute`] to run.
pub struct BuildCoordinator {
    session: Arc<BuildSession>,
    config: Arc<ArkdownConfig>,
    args: BuildRequest,
    package_plugins: Vec<PackagePlugin>,
    owns_task_cycle: bool,
}

#[cfg(test)]
mod target_build_execution_plan_tests {
    use super::{
        BuildRequest, EffectiveTargetConfig, ModuleTargetSelection, OutputTarget,
        TargetBuildExecutionPlan,
    };

    #[test]
    fn hvigor_implicit_all_builds_regular_targets_but_not_ohos_test() {
        let plan = TargetBuildExecutionPlan::from_implicit_targets([
            "default".to_string(),
            "ohosTest".to_string(),
            "phone".to_string(),
            "default".to_string(),
        ]);

        assert_eq!(plan.target_names, ["default", "phone"]);
    }

    #[test]
    fn hvigor_align_target_overrides_normal_target_but_not_test_actions() {
        let mut args = BuildRequest {
            build_target_name: Some("phone".to_string()),
            hvigor_properties: crate::request::HvigorRuntimeProperties {
                align_target: Some("aligned".to_string()),
                ..crate::request::HvigorRuntimeProperties::default()
            },
            ..BuildRequest::default()
        };

        assert_eq!(
            TargetBuildExecutionPlan::aligned_target_for_action(&OutputTarget::Hap, &args)
                .as_deref(),
            Some("aligned")
        );
        assert_eq!(
            TargetBuildExecutionPlan::aligned_target_for_action(&OutputTarget::Test, &args),
            None
        );
        args.build_target_name = Some("ohosTest".to_string());
        assert_eq!(
            TargetBuildExecutionPlan::aligned_target_for_action(&OutputTarget::Hap, &args),
            None
        );
    }

    #[test]
    fn hvigor_target_candidates_use_first_existing_fallback_then_default() {
        let targets = ["default", "tablet", "wearable"]
            .into_iter()
            .map(|name| EffectiveTargetConfig {
                name: name.to_string(),
                ..EffectiveTargetConfig::default()
            })
            .collect::<Vec<_>>();

        let selected = ModuleTargetSelection::first_named_target(
            &targets,
            ["missing-align", "missing-fallback", "wearable", "default"],
        )
        .expect("fallback target");
        assert_eq!(selected.name, "wearable");
    }

    #[test]
    fn hvigor_device_test_builds_ohos_test_before_default_and_ignores_explicit_target() {
        let temp = tempfile::tempdir().unwrap();
        let module_root = temp.path().join("entry");
        std::fs::create_dir_all(module_root.join("src/ohosTest")).unwrap();
        std::fs::write(
            module_root.join("src/ohosTest/module.json5"),
            "{ module: { name: 'entry_test', type: 'entry' } }",
        )
        .unwrap();
        let config = arkdown_core::ArkdownConfig {
            project_root: temp.path().to_path_buf(),
            build_mode: arkdown_core::BuildMode::Debug,
            output_target: OutputTarget::Hap,
            product: arkdown_core::ProductConfig {
                name: "default".to_string(),
                artifact_name: None,
                bundle_name: "com.example.device.test".to_string(),
                compatible_sdk_version: "5.0.0(12)".to_string(),
                compatible_sdk_version_stage: None,
                target_sdk_version: None,
                runtime_os: "OpenHarmony".to_string(),
                signing_config: None,
                build_option: arkdown_core::BuildOption {
                    strict_mode: arkdown_core::StrictModeConfig {
                        case_sensitive_check: false,
                        use_normalized_ohm_url: true,
                        no_external_import_by_path: None,
                        duplicate_dependency_check: false,
                        api_compatibility_check: None,
                    },
                    ark_options: None,
                    res_options: None,
                    source_option: None,
                    preload_system_so: false,
                    preload_so: Vec::new(),
                    deduplicate_har: false,
                    app_with_signed_pkg: false,
                    enable_source_code_check: true,
                    generate_shared_tgz: None,
                    customized_har_base_package: None,
                    remove_permissions: Vec::new(),
                    aot_compile_mode: None,
                    host_pgo: None,
                    ap_path: None,
                    abi_filters: Vec::new(),
                    external_native_options: None,
                    har_artifact_type: arkdown_core::HarArtifactType::Original,
                    native_lib: arkdown_core::NativeLibOption::default(),
                },
                version_code: None,
                version_name: None,
                build_version: None,
                app_options: arkdown_core::AppOptionOverride::default(),
            },
            modules: vec![arkdown_core::ModuleConfig {
                name: "entry".to_string(),
                src_path: module_root,
                module_type: arkdown_core::OhosModuleType::Entry,
            }],
            sdk_config: arkdown_core::SdkConfig {
                ets_loader_path: temp.path().join("sdk").join("ets-loader"),
                es2abc_path: temp.path().join("sdk").join("es2abc"),
                restool_path: temp.path().join("sdk").join("restool"),
                compile_api_version: 12,
                sdk_component_api_version: 12,
                has_rollup_plugin: true,
                compile_sdk_version: "6.0.0.47".to_string(),
            },
        };
        let build_options =
            arkdown_config::build_profile::BuildOptionProfileInventory::from_values(
                temp.path(),
                serde_json::json!({
                  "modules": [{
                    "name": "entry",
                    "srcPath": "./entry",
                    "targets": [{
                      "name": "ohosTest",
                      "config": { "deviceType": ["phone", "tablet"] }
                    }]
                  }]
                }),
                &[],
            )
            .expect("empty startup build-profile inventory");
        let args = BuildRequest {
            action: crate::request::BuildAction::DeviceTest,
            build_target_name: Some("phone".to_string()),
            ..BuildRequest::default()
        };
        let dependency_target = ModuleTargetSelection::resolve_dependency(
            &config,
            &build_options,
            &config.modules[0],
            Some("ohosTest"),
            &args.hvigor_properties,
        );
        assert_eq!(
            dependency_target.target.device_types,
            ["phone", "tablet"],
            "declared ohosTest target data takes precedence over the synthetic source-set target"
        );
        let plan = TargetBuildExecutionPlan::new(&config, &build_options, &args);
        assert_eq!(plan.target_names, ["ohosTest", "default"]);
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
            || arkdown_core::is_harmony_os(&self.config.product.runtime_os)
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

/// Ordered resource roots in the physical-path identity domain used by
/// Hvigor's `BasePackageResolver.resolvePackagePath()`.
///
/// Hvigor resolves installed packages with `realpathSync.native()` before its
/// dependency manager and `RestoolConfigBuilder` see their resource roots. An
/// arkdown build can discover the same local HAR through a project module path
/// containing `..` and through one or more `oh_modules` symlinks. Canonicalize
/// at this merge boundary so those spellings cannot become duplicate restool
/// inputs after artifact serialization.
#[derive(Default)]
struct ResourceInputRootCollector {
    roots: Vec<PathBuf>,
    seen: FxHashSet<PathBuf>,
}

impl ResourceInputRootCollector {
    fn insert_existing(&mut self, root: PathBuf) {
        if !root.is_dir() {
            return;
        }
        self.insert_resolved(canonicalize_or_owned(&root));
    }

    fn insert_resolved(&mut self, root: PathBuf) {
        if self.seen.insert(root.clone()) {
            self.roots.push(root);
        }
    }

    fn into_paths(self) -> Vec<PathBuf> {
        self.roots
    }
}

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
        let mut selected = self.resolve_target_dirs(&self.target.resource_directories);
        if selected.is_empty() {
            if let Some(source_set_res) = self.source_set_dir(&self.target.name) {
                selected.push(source_set_res);
            }
        }
        let main_resources = ModuleBuildLayout::new(&self.module.src_path)
            .source()
            .resources();
        if selected.is_empty() {
            selected.push(main_resources.clone());
        }
        // `AbstractProcessResource.addOhosTestExtraCompilationResources` adds
        // `src/main/resources` after the selected ohosTest resource roots even
        // when that target declares custom or source-set resources.
        if self.target.name == "ohosTest"
            && main_resources.is_dir()
            && !selected.contains(&main_resources)
        {
            selected.push(main_resources);
        }
        selected
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

#[cfg(test)]
mod resource_input_root_collector_tests {
    use super::*;

    #[test]
    fn hvigor_realpath_identity_deduplicates_local_and_oh_modules_resource_roots() {
        let temp = tempfile::tempdir().expect("create resource roots fixture");
        let package_root = temp.path().join("store/@msi/jsi_interface");
        let resources = package_root.join("src/main/resources");
        let project_link = temp.path().join("oh_modules/@msi/jsi_interface");
        let transitive_link = temp
            .path()
            .join("meituan/oh_modules/@msi/jsi/oh_modules/@msi/jsi_interface");
        fs_err::create_dir_all(&resources).expect("create physical resources");
        fs_err::create_dir_all(project_link.parent().expect("project link parent"))
            .expect("create project link parent");
        fs_err::create_dir_all(transitive_link.parent().expect("transitive link parent"))
            .expect("create transitive link parent");
        link_directory(&package_root, &project_link).expect("create project oh_modules link");
        link_directory(&package_root, &transitive_link).expect("create transitive oh_modules link");

        let mut roots = ResourceInputRootCollector::default();
        roots.insert_existing(
            package_root
                .join("src/main")
                .join("..")
                .join("main/resources"),
        );
        roots.insert_resolved(canonicalize_or_owned(
            &project_link.join("src/main/resources"),
        ));
        roots.insert_resolved(canonicalize_or_owned(
            &transitive_link.join("src/main/resources"),
        ));

        assert_eq!(roots.into_paths(), vec![canonicalize_or_owned(&resources)]);
    }

    #[cfg(unix)]
    fn link_directory(source: &Path, destination: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(source, destination)
    }

    #[cfg(windows)]
    fn link_directory(source: &Path, destination: &Path) -> std::io::Result<()> {
        junction::create(source, destination)
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
                node.package_root
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
            compile_inputs: Default::default(),
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
            installed_alias_path: None,
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

/// Exact `PackageSignHar.taskShouldDo()` gate from hvigor 6.1.1.280.
///
/// Unlike HAP/HSP signing, this task deliberately ignores
/// `enableSignTask`; it is controlled by `ohos.sign.har` and only exists for
/// daemon OHPM HarmonyOS builds on API 12 or newer.
struct PackageSignHarPlan {
    sdk_sign_tool: PathBuf,
}

impl PackageSignHarPlan {
    fn resolve(
        args: &BuildRequest,
        config: &ArkdownConfig,
        loader_json: &LoaderJson,
        session: &BuildSession,
    ) -> Option<Self> {
        if !args.hvigor_properties.daemon
            || !args.hvigor_properties.sign_har
            || loader_json.package_manager_type != "ohpm"
            || config.product.signing_config.is_none()
            || config.sdk_config.compile_api_version < 12
            || parse_api_version(&config.product.compatible_sdk_version).unwrap_or_default() < 12
            || !arkdown_core::is_harmony_os(&config.product.runtime_os)
        {
            return None;
        }
        let sdk_sign_tool = session.sdk.tool_path(SdkToolKind::SdkSignToolFull)?.path;
        sdk_sign_tool.is_file().then_some(Self { sdk_sign_tool })
    }
}

struct ArtifactOutputName<'a> {
    artifact_name: Option<&'a str>,
    module_name: &'a str,
    target_label: &'a str,
    extension: &'a str,
}

impl<'a> ArtifactOutputName<'a> {
    fn new(
        artifact_name: Option<&'a str>,
        module_name: &'a str,
        target_label: &'a str,
        extension: &'a str,
    ) -> Self {
        Self {
            artifact_name,
            module_name,
            target_label,
            extension,
        }
    }

    fn unsigned(&self) -> String {
        if let Some(artifact_name) = self.artifact_name {
            return CoordinatorText::from_parts([artifact_name, "-unsigned.", self.extension]);
        }
        CoordinatorText::from_parts([
            self.module_name,
            "-",
            self.target_label,
            "-unsigned.",
            self.extension,
        ])
    }

    fn signed(&self) -> String {
        if let Some(artifact_name) = self.artifact_name {
            return CoordinatorText::from_parts([artifact_name, ".", self.extension]);
        }
        CoordinatorText::from_parts([
            self.module_name,
            "-",
            self.target_label,
            "-signed.",
            self.extension,
        ])
    }

    fn signed_har(&self) -> String {
        CoordinatorText::from_parts([
            self.artifact_name.unwrap_or(self.module_name),
            "-signed.",
            self.extension,
        ])
    }

    fn har(&self) -> String {
        CoordinatorText::from_parts([
            self.artifact_name.unwrap_or(self.module_name),
            ".",
            self.extension,
        ])
    }

    fn shared_tgz(&self) -> String {
        if let Some(artifact_name) = self.artifact_name {
            return CoordinatorText::from_parts([artifact_name, ".tgz"]);
        }
        CoordinatorText::from_parts([self.module_name, "-", self.target_label, ".tgz"])
    }

    fn pack_info(&self) -> String {
        self.artifact_name.map_or_else(
            || CoordinatorText::from_parts([self.module_name, "-", self.target_label]),
            str::to_string,
        )
    }
}

struct CoordinatorText;

impl CoordinatorText {
    fn from_parts<const N: usize>(parts: [&str; N]) -> String {
        parts.into_iter().collect()
    }
}
