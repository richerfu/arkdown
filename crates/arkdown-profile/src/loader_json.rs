use rustc_hash::{FxHashMap, FxHashSet};
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};
use std::time::Instant;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use arkdown_config::build_profile::parse_api_version;
use arkdown_core::{
    ArkdownConfig, ArkdownError, HapManifest, ModuleConfig, ModuleJson5, OhosModuleType,
    OutputTarget, PkgBriefInfo,
};
use arkdown_path::{
    canonicalize_or_owned, AbsPath, ArtifactPath, ArtifactPathSerializer, ModuleSpecifier,
    ModuleSpecifierText, OhmUrl, SourceFileName, SpecifierKind,
};

use crate::bytecode_har::{
    ByteCodeHarMetadataWriter, ByteCodeHarScanEntry, CompatibleSdkVersionValue,
    LoaderByteCodeHarFields,
};
use crate::form_profile::{FormProfileProcessor, ModuleFormProfileInput};

/// Exact `GenerateLoaderJson.isFullCompilationEnabled()` policy from Hvigor
/// 6.24.2/DevEco 6.1.1.280.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct LoaderCompilationPolicy {
    compile_lib_entry_file: Option<bool>,
}

impl LoaderCompilationPolicy {
    pub(crate) fn from_hvigor_property(compile_lib_entry_file: Option<bool>) -> Self {
        Self {
            compile_lib_entry_file,
        }
    }

    fn is_full_compilation_enabled(self, is_form_widget_module: bool) -> bool {
        (!is_form_widget_module || self.compile_lib_entry_file.is_some())
            && self.compile_lib_entry_file != Some(true)
    }
}
use crate::layout::ProfileModuleLayout;
use crate::pkg_context_info::{
    PkgContextCurrentInput, PkgContextDependencyInput, PkgContextDependencyKind,
    PkgContextInfoGenerator, PkgContextInfoInput, PkgContextInfoOpt, PkgContextModuleKind,
};
use crate::resource_ref::ProfileResourceRef;

mod runtime_only;
use runtime_only::RuntimeOnlyCollector;

// ---------------------------------------------------------------------------
// loader.json schema types
// ---------------------------------------------------------------------------

/// Top-level `loader.json` structure.
///
/// The serialized fields match the hvigor `GenerateLoaderJson` artifact. The
/// skipped fields are retained for arkdown's in-process bundler, which still
/// needs an explicit entry list and output paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoaderJson {
    /// Map of module name → absolute module root path.
    pub module_path_map: IndexMap<String, String>,
    /// hvigor compile mode string, e.g. `"esmodule"`.
    pub compile_mode: String,
    /// Absolute path to the project root directory.
    pub project_root_path: String,
    /// hvigor node_modules sandbox path under `intermediates/loader_out`.
    pub node_modules_path: String,
    /// ByteCode HAR dependency metadata.
    pub byte_code_har_info: IndexMap<String, ByteCodeHarEntry>,
    /// ByteCode HAR declaration entries. Empty for the minimal HAP fixture.
    pub declaration_entry: Vec<String>,
    /// The OHOS module name (e.g. `"entry"`).
    pub module_name: String,
    /// HSP package name → OHM URL map.
    pub hsp_name_ohm_map: IndexMap<String, String>,
    /// HAR package name → OHM URL map.
    pub har_name_ohm_map: IndexMap<String, String>,
    /// Package manager type — `"ohpm"` for OHOS projects.
    pub package_manager_type: String,
    /// hvigor's serialized compile entries. Empty for the minimal HAP fixture.
    #[serde(rename = "compileEntry")]
    pub hvigor_compile_entry: Vec<String>,
    /// Additional compile files emitted by hvigor. Empty for the minimal fixture.
    pub other_compile_files: Vec<String>,
    /// Dynamic import metadata. Empty until dependency collection is implemented.
    pub dynamic_import_lib_info: IndexMap<String, serde_json::Value>,
    /// Router map entries. Empty for the minimal fixture.
    pub router_map: Vec<serde_json::Value>,
    /// HSP resource table map. Empty for the minimal fixture.
    pub hsp_resources_map: FxHashMap<String, String>,
    /// Incremental update metadata. Empty for the minimal fixture.
    pub update_version_info: IndexMap<String, serde_json::Value>,
    /// Whether HAR compilation uses `packingOptions.customizedOptions.basePackage`.
    pub customized_har: bool,
    /// Present only for the HAR currently being emitted as ByteCode HAR.
    ///
    /// Latest `GenerateLoaderJson` appends this field after constructing the
    /// common loader object; source HAR/HAP/HSP loader files omit it.
    #[serde(rename = "byteCodeHar", skip_serializing_if = "Option::is_none")]
    pub byte_code_har: Option<bool>,
    /// 6.24.3 widget-loader field: transitively blocked dependency compile
    /// files grouped by the topmost blocking ByteCode HAR package.
    ///
    /// `GenerateLoaderJson.doTaskAction()` writes `pkgName2OtherCompileFiles`
    /// only on the widget loader JSON (`{...common, pkgName2OtherCompileFiles}`);
    /// the main `loader.json` never carries the key. The field is therefore
    /// `None` for the common loader and `Some(_)` (possibly empty) for the
    /// widget loader when `needSubmitArkTsWidget` is set.
    #[serde(
        rename = "pkgName2OtherCompileFiles",
        skip_serializing_if = "Option::is_none"
    )]
    pub pkg_name2_other_compile_files: Option<IndexMap<String, Vec<String>>>,
    /// Ark Native output path emitted by hvigor when AOT is enabled for this module.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub an_build_out_put: Option<String>,
    /// Ark Native build mode emitted by hvigor when AOT is enabled for this module.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub an_build_mode: Option<String>,
    /// AOT profile path emitted by hvigor when a valid `.ap` file participates.
    #[serde(rename = "apPath", skip_serializing_if = "Option::is_none")]
    pub ap_path: Option<String>,
    /// Preview/local-test build config path emitted by hvigor when requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_config_path: Option<String>,
    /// Quick-fix patch config copied from build config when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch_config: Option<serde_json::Value>,
    /// Non-entry importees in OHM URL format, populated after bundling by
    /// `process_importees`.  Empty until the post-bundle pass runs.
    ///
    /// Not serialized to `loader.json`: hvigor keeps the corresponding
    /// incremental sets in conditional CompileArkTS cache files.
    #[serde(skip)]
    pub non_entry_importees: Vec<String>,
    /// Compiled importee source paths (relative to `src/main`), populated
    /// after bundling by `process_importees`.  Empty until that pass runs.
    ///
    /// Not serialized to `loader.json`: hvigor keeps the corresponding
    /// incremental sets in conditional CompileArkTS cache files.
    #[serde(skip)]
    pub compiled_importees: Vec<String>,

    /// Ordered list of entry-point source paths used by arkdown's bundler.
    #[serde(skip)]
    pub compile_entry: Vec<String>,
    /// Form/card entry sources collected from the profiles referenced by
    /// `module.json5`. SDK `main.js::readCardForm` retains this set as
    /// `projectConfig.cardObj`; transforms must not infer it from directories.
    #[serde(skip)]
    pub form_entry_sources: Vec<String>,
    /// Source entries that the OHOS runtime loads directly from `module.json`.
    #[serde(skip)]
    pub runtime_entry_sources: Vec<String>,
    /// Absolute path to the module source root directory (e.g. `<project>/entry/src/main`).
    #[serde(skip)]
    pub module_root_path: String,
    /// Absolute path to the incremental compilation cache directory.
    #[serde(skip)]
    pub cache_path: String,
    /// Absolute path where the ArkTS loader writes its output `.js` files.
    #[serde(skip)]
    pub ace_module_build: String,
    /// `"debug"` or `"release"`.
    #[serde(skip, default = "default_build_mode")]
    pub build_mode: String,
    /// Whether the bundler may split output into multiple chunks.
    #[serde(skip)]
    pub support_chunks: bool,
    /// Whether OHM URL module identifiers are used in the bundle graph.
    #[serde(skip)]
    pub use_normalized_ohm_url: bool,
    /// Numeric SDK API version (e.g. `12`).
    #[serde(skip)]
    pub api_version: u32,
    /// Whether to fall back to AN (Ark Native) build.
    #[serde(skip)]
    pub fallback_an_build: bool,
    /// Map of package name → brief package info (source roots, etc.).
    #[serde(skip)]
    pub pkg_name_to_pkg_brief_info: FxHashMap<String, PkgBriefInfo>,
    /// WebWorker source file paths. hvigor reads this field when present and
    /// expects filesystem paths to `.ets`, `.ts`, or `.js` worker sources.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub workers: Vec<String>,

    /// Source root directories relative to the module root (for HAR consumers).
    /// Populated from src/main/ets/ relative paths.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_roots: Vec<String>,

    /// Direct HAR/HSP bundle dependency names kept for in-process planning.
    ///
    /// Hvigor writes `metadata.bundleDependencies` to ByteCode HAR package
    /// metadata, not to `GenerateLoaderJson`'s HAP `loader.json`.
    #[serde(skip)]
    pub bundle_dependencies: Vec<String>,

    /// Global dependencies available to all modules (from oh-package.json5 dependencies).
    /// Mirrors hvigor's globalDependencies field in loader.json.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub global_dependencies: Vec<String>,

    /// Raw values forwarded to hvigor's `projectConfig.compilerTypes`.
    #[serde(skip)]
    pub compiler_types: Vec<String>,

    /// Resolved declaration roots loaded into the OXC transform snapshot.
    #[serde(skip)]
    pub compiler_type_declaration_inputs: Vec<PathBuf>,

    /// Version info from ByteCode HAR deps' oh-package.json5 metadata.
    /// Mirrors hvigor's dependencyPkgVersion in loader.json.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependency_pkg_version: Vec<String>,

    /// Extra importee OHM URLs from ByteCode HAR otherCompileFiles.
    /// Mirrors hvigor's extraImportees in loader.json.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_importees: Vec<String>,

    /// Package names of all compiled dependencies.
    /// Mirrors hvigor's compiledPackageNames in loader.json.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub compiled_package_names: Vec<String>,

    /// Absolute source paths collected from the current HAR/HSP package's
    /// `oh-exports` declaration.
    #[serde(rename = "oh-exports", default, skip_serializing_if = "Vec::is_empty")]
    pub oh_exports: Vec<String>,
}

struct AotLoaderFields {
    an_build_out_put: Option<String>,
    an_build_mode: Option<String>,
    ap_path: Option<String>,
    fallback_an_build: bool,
}

#[derive(Deserialize)]
struct DependencyPackageJson {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    dependencies: IndexMap<String, String>,
    #[serde(default)]
    main: Option<String>,
    #[serde(default)]
    types: Option<String>,
    #[serde(default)]
    version: String,
    #[serde(default)]
    metadata: Option<DependencyPackageMetadata>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DependencyPackageMetadata {
    #[serde(default)]
    resource: Option<DependencyResourceMetadata>,
}

#[derive(Deserialize)]
struct DependencyResourceMetadata {
    #[serde(default)]
    directories: Vec<String>,
}

#[derive(Deserialize)]
struct DependencyModuleJson {
    module: DependencyModuleSection,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DependencyModuleSection {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    abilities: Vec<DependencyAbilityEntry>,
    #[serde(default)]
    extension_abilities: Vec<DependencyAbilityEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DependencyAbilityEntry {
    #[serde(default)]
    src_entry: Option<String>,
    #[serde(rename = "type", alias = "extensionType", default)]
    extension_type: Option<String>,
}

impl AotLoaderFields {
    fn from_config(
        config: &ArkdownConfig,
        module_cfg: &ModuleConfig,
        manifest: &HapManifest,
        target_name: &str,
    ) -> Self {
        let fallback_an_build = matches!(manifest.module_type, OhosModuleType::Har);
        if fallback_an_build {
            return Self::empty(fallback_an_build);
        }

        // Preview builds do not run AOT compilation. hvigor skips
        // `anBuildMode`/`anBuildOutPut` for preview targets. Local unit-test
        // builds keep them (hvigor `handleAotFields` is target-agnostic).
        if matches!(config.output_target, arkdown_core::OutputTarget::Preview) {
            return Self::empty(false);
        }

        let api_version = config.sdk_config.compile_api_version;
        let an_build_mode = Self::resolve_an_build_mode(config);
        let ap_path = Self::resolve_ap_path(config, module_cfg);
        let ap_exists = ap_path.as_ref().is_some_and(|path| path.exists());
        let has_ap_path = ap_path.is_some();
        let ap_suffix_valid = ap_path.as_ref().map_or(true, |path| {
            path.extension().map_or(false, |ext| ext == "ap")
        });

        if api_version == 9 && an_build_mode != "partial" && has_ap_path {
            panic!(
                "MISMATCH_AOT_COMPILE_MODE: apPath is only valid with aotCompileMode=partial on API 9"
            );
        }
        if api_version == 9 && an_build_mode == "partial" && !has_ap_path {
            panic!("PARTIAL_EMPTY_PATH: aotCompileMode=partial requires apPath on API 9");
        }
        if !ap_suffix_valid {
            panic!("PARTIAL_INVALID_SUFFIX: apPath must end with .ap");
        }
        if an_build_mode == "partial" && !ap_exists {
            let display_path = ap_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_default();
            panic!("PARTIAL_INVALID_PATH: apPath does not exist: {display_path}");
        }
        if ((api_version == 9 && !has_ap_path) || api_version >= 10) && an_build_mode.is_empty() {
            return Self::empty(fallback_an_build);
        }

        let module_root = PathBuf::from(to_absolute_path_string(&module_cfg.src_path));
        let aot_layout = if matches!(config.output_target, OutputTarget::Test) {
            ProfileModuleLayout::for_test_product(&module_root, &config.product.name)
        } else if matches!(config.output_target, OutputTarget::Preview) {
            ProfileModuleLayout::for_preview_product(
                &module_root,
                &config.product.name,
                target_name,
            )
        } else if target_name != "default" {
            ProfileModuleLayout::for_target_product(&module_root, &config.product.name, target_name)
        } else {
            ProfileModuleLayout::for_product(&module_root, &config.product.name)
        };
        let an_build_out_put = normalize_path_string(&aot_layout.aot_loader_output_dir());

        let should_write_ap_path =
            ((api_version == 9 && an_build_mode == "partial") || api_version >= 10) && ap_exists;
        if should_write_ap_path {
            let ap_path = ap_path.expect("ap_exists implies ap_path");
            Self::copy_ap_file(module_cfg, &aot_layout, &ap_path);
            return Self {
                an_build_out_put: Some(an_build_out_put),
                an_build_mode: Some(an_build_mode),
                ap_path: Some(to_absolute_path_string(&ap_path)),
                fallback_an_build,
            };
        }

        let without_ap_path = (api_version == 9 && an_build_mode == "type" && !has_ap_path)
            || (api_version >= 10 && an_build_mode == "type" && !ap_exists);
        if without_ap_path {
            return Self {
                an_build_out_put: Some(an_build_out_put),
                an_build_mode: Some(an_build_mode),
                ap_path: None,
                fallback_an_build,
            };
        }

        Self::empty(fallback_an_build)
    }

    fn empty(fallback_an_build: bool) -> Self {
        Self {
            an_build_out_put: None,
            an_build_mode: None,
            ap_path: None,
            fallback_an_build,
        }
    }

    fn resolve_an_build_mode(config: &ArkdownConfig) -> String {
        let api_version = config.sdk_config.compile_api_version;
        if api_version == 9 {
            return config
                .product
                .build_option
                .aot_compile_mode
                .clone()
                .unwrap_or_default();
        }

        let host_pgo = config.product.build_option.host_pgo == Some(true);
        let is_openharmony = !arkdown_core::is_harmony_os(&config.product.runtime_os);
        if is_openharmony
            && !aot_fixed_sdk_version(&config.sdk_config.compile_sdk_version)
            && !host_pgo
        {
            return String::new();
        }

        if host_pgo {
            "partial".to_string()
        } else {
            "type".to_string()
        }
    }

    fn resolve_ap_path(config: &ArkdownConfig, module_cfg: &ModuleConfig) -> Option<PathBuf> {
        if let Some(path) = config.product.build_option.ap_path.as_ref() {
            return Some(if path.has_root() {
                path.clone()
            } else {
                module_cfg.src_path.join(path)
            });
        }

        (config.sdk_config.compile_api_version >= 10)
            .then(|| ProfileModuleLayout::new(&module_cfg.src_path).modules_ap())
    }

    fn copy_ap_file(module_cfg: &ModuleConfig, layout: &ProfileModuleLayout<'_>, source: &Path) {
        let dest_dir = layout.ap_intermediate_dir();
        std::fs::create_dir_all(&dest_dir).unwrap_or_else(|error| {
            panic!(
                "failed to create hvigor AOT intermediates directory {}: {error}",
                dest_dir.display()
            )
        });
        let dest = dest_dir.join(Path::new(&module_cfg.name).with_extension("ap"));
        std::fs::copy(source, &dest).unwrap_or_else(|error| {
            panic!(
                "failed to copy AOT profile {} to {}: {error}",
                source.display(),
                dest.display()
            )
        });
    }
}

fn aot_fixed_sdk_version(version: &str) -> bool {
    AotSdkVersionText::new(version).is_after_fixed_threshold()
}

struct AotSdkVersionText<'a> {
    raw: &'a str,
}

impl<'a> AotSdkVersionText<'a> {
    const FIXED_THRESHOLD: (u32, u32, u32, u32) = (4, 0, 8, 1);

    fn new(raw: &'a str) -> Self {
        Self { raw }
    }

    fn is_after_fixed_threshold(&self) -> bool {
        let raw = self.raw.trim();
        if raw.is_empty() {
            return true;
        }

        match self.version_tuple() {
            Some(current) => current > Self::FIXED_THRESHOLD,
            None => true,
        }
    }

    fn version_tuple(&self) -> Option<(u32, u32, u32, u32)> {
        let mut parts = self.raw.trim().split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let revision = parts.next()?.parse().ok()?;
        let build = parts.next()?.parse().ok()?;
        parts
            .next()
            .is_none()
            .then_some((major, minor, revision, build))
    }
}

struct LoaderRouterModuleName<'a> {
    host_module_name: &'a str,
    module_name: &'a str,
}

struct RouterMapModule {
    module_name: String,
    package_name: String,
    src_path: PathBuf,
}

#[derive(Deserialize)]
struct MainPagesProfile {
    src: Vec<String>,
}

struct LoaderTimingProbe<'a> {
    module: &'a str,
    stage: &'static str,
    start: Instant,
}

impl<'a> LoaderTimingProbe<'a> {
    fn new(module: &'a str, stage: &'static str) -> Self {
        Self {
            module,
            stage,
            start: Instant::now(),
        }
    }
}

impl Drop for LoaderTimingProbe<'_> {
    fn drop(&mut self) {
        tracing::debug!(
            module = %self.module,
            stage = self.stage,
            elapsed_ms = self.start.elapsed().as_millis(),
            "loader json substage completed"
        );
    }
}

impl<'a> LoaderRouterModuleName<'a> {
    fn new(host_module_name: &'a str, module_name: &'a str) -> Self {
        Self {
            host_module_name,
            module_name,
        }
    }

    fn string(&self) -> String {
        if self.module_name == self.host_module_name {
            return self.host_module_name.to_string();
        }
        let mut name =
            String::with_capacity(self.host_module_name.len() + 1 + self.module_name.len());
        name.push_str(self.host_module_name);
        name.push('@');
        name.push_str(self.module_name);
        name
    }
}

impl LoaderJson {
    /// Apply the validated current-package `oh-exports` file snapshot.
    ///
    /// Mirrors `GenerateLoaderJson.addOhExports2CompileEntry`,
    /// `processCodeFiles`, and `writeOhExportsToLoaderJson`: declaration files
    /// remain visible in `oh-exports`, but are not compile entries.
    pub(crate) fn apply_oh_exports_files(&mut self, files: &[PathBuf]) {
        self.oh_exports = files
            .iter()
            .map(|path| {
                ArtifactPathSerializer::path_to_slash_string(path)
                    .unwrap_or_else(|_| path.display().to_string())
            })
            .collect();

        let source_root = Path::new(&self.module_root_path);
        for path in files {
            let lower_path = path.to_string_lossy().to_ascii_lowercase();
            if lower_path.ends_with(".d.ets") || lower_path.ends_with(".d.ts") || !path.exists() {
                continue;
            }
            let absolute = to_absolute_path_string(path);
            if !self.hvigor_compile_entry.contains(&absolute) {
                self.hvigor_compile_entry.push(absolute.clone());
            }
            let canonical_path = canonicalize_or_owned(path);
            let internal_entry = canonical_path
                .strip_prefix(source_root)
                .map(Self::compile_entry_from_src_main_rel)
                .unwrap_or(absolute);
            if !self.compile_entry.contains(&internal_entry) {
                self.compile_entry.push(internal_entry);
            }
        }
    }

    /// Add the generated `ModuleInfo.ts` required by a ByteCode HAR.
    ///
    /// Mirrors `GenerateLoaderJson.processByteCodeHar()` from the installed
    /// hvigor 6.24.3. The path participates in both the serialized loader
    /// contract and Arkdown's in-process compiler input inventory.
    pub fn include_byte_code_har_module_info(&mut self, path: &Path) {
        if !path.is_file() {
            return;
        }
        let absolute = to_absolute_path_string(path);
        if !self.hvigor_compile_entry.contains(&absolute) {
            self.hvigor_compile_entry.push(absolute.clone());
        }
        if !self.compile_entry.contains(&absolute) {
            self.compile_entry.push(absolute);
        }
    }

    /// Apply the final ByteCode HAR scan result to this loader.
    ///
    /// Profile generation can run before the session-backed dependency graph is
    /// available. The coordinator later supplies the authoritative scan result,
    /// and all ByteCode HAR-derived loader surfaces must move together:
    /// `byteCodeHarInfo`, propagated `declarationEntry`, serialized
    /// `otherCompileFiles`, `updateVersionInfo`, and arkdown's internal
    /// compile entry list.
    pub fn apply_bytecode_har_scan_entries(
        &mut self,
        deps: &[ByteCodeHarScanEntry],
        module_root: &Path,
        project_root: &Path,
        output_target: OutputTarget,
    ) {
        let previous_other_compile_files = std::mem::take(&mut self.other_compile_files);
        self.compile_entry
            .retain(|entry| !previous_other_compile_files.contains(entry));

        // Installed hvigor 6.24.3
        // `GenerateLoaderJson.setByteCodeHarInfoAndOtherCompileFiles()` gates
        // this entire surface on normalized OHM URLs and `isHapOrHsp()`.
        // A normal HAR target therefore keeps these fields empty even when it
        // is itself emitted as a ByteCode HAR.
        if !self.use_normalized_ohm_url || matches!(output_target, OutputTarget::Har) {
            self.byte_code_har_info.clear();
            self.declaration_entry.clear();
            self.update_version_info.clear();
            return;
        }

        self.byte_code_har_info = deps
            .iter()
            .cloned()
            .map(ByteCodeHarScanEntry::into_loader_entry)
            .collect();
        self.update_version_info = ByteCodeHarScanEntry::collect_bytecode_har_update_version_info(
            deps,
            module_root,
            project_root,
        );

        if matches!(
            output_target,
            OutputTarget::Hap | OutputTarget::Hsp | OutputTarget::Preview | OutputTarget::Test
        ) {
            let mut entries = deps
                .iter()
                .flat_map(|dep| dep.declaration_entry.iter())
                .filter(|url| !url.trim().is_empty())
                .cloned()
                .collect::<Vec<_>>();
            entries.sort();
            entries.dedup();
            self.declaration_entry = entries;
        }

        let other_compile_paths =
            LoaderByteCodeHarFields::collect_other_compile_files(deps, module_root, project_root);
        self.other_compile_files = other_compile_paths
            .iter()
            .map(|path| to_absolute_path_string(path))
            .collect();
        self.compile_entry
            .extend(self.other_compile_files.iter().cloned());
        let mut seen = FxHashSet::default();
        self.compile_entry
            .retain(|entry| seen.insert(entry.clone()));
    }

    /// Replace `harNameOhmMap` from the exact dependency view selected by
    /// hvigor's ByteCode HAR task service.
    pub fn apply_bytecode_har_ohm_packages(
        &mut self,
        package_names: &[String],
        pkg_context_info: &std::collections::BTreeMap<String, PkgContextInfoOpt>,
    ) {
        if self.byte_code_har != Some(true) || !self.use_normalized_ohm_url {
            return;
        }
        self.har_name_ohm_map.clear();
        for package_name in package_names {
            let Some(context) = pkg_context_info.get(package_name) else {
                continue;
            };
            self.har_name_ohm_map.insert(
                package_name.clone(),
                Self::normalized_ohm_from_pkg_context(context),
            );
        }
    }

    fn compile_entry_from_src_main_rel(path: &Path) -> String {
        ModuleSpecifier::from_relative_path(path).into_string()
    }

    fn compile_entry_from_ets_rel(path: &Path) -> String {
        let path = Path::new("ets").join(path);
        Self::compile_entry_from_src_main_rel(&path)
    }

    fn compile_entry_from_page(page: &str) -> String {
        let mut page_path = PathBuf::from(arkdown_path::strip_relative_prefix(page));
        if !Self::page_path_is_src_main_relative(&page_path) {
            page_path = Path::new("ets").join(page_path);
        }
        let mut path = Self::source_path_without_extension(&page_path);
        path.set_extension("ets");
        Self::compile_entry_from_src_main_rel(&path)
    }

    fn page_path_is_src_main_relative(path: &Path) -> bool {
        path.components().next().is_some_and(
            |component| matches!(component, Component::Normal(name) if name == OsStr::new("ets")),
        )
    }

    fn absolute_dir_path_string(path: &Path) -> String {
        let mut serialized = to_absolute_path_string(path);
        if !serialized.ends_with('/') {
            serialized.push('/');
        }
        serialized
    }

    fn source_entry_stem_path(source_entry: &str) -> Option<PathBuf> {
        let source_entry = arkdown_path::strip_relative_prefix(source_entry);
        let mut path = PathBuf::new();
        for component in Path::new(source_entry).components() {
            if matches!(component, Component::CurDir) {
                continue;
            }
            path.push(component.as_os_str());
        }
        (!path.as_os_str().is_empty()).then(|| Self::source_path_without_extension(&path))
    }

    fn source_path_without_extension(path: &Path) -> PathBuf {
        let mut path = path.to_path_buf();
        if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("ets" | "ts" | "js")
        ) {
            path.set_extension("");
        }
        path
    }

    fn collect_runtime_entry_sources(
        config: &ArkdownConfig,
        manifest: &HapManifest,
        module_layout: &ProfileModuleLayout<'_>,
        target_name: &str,
    ) -> Result<Vec<String>, ArkdownError> {
        let mut runtime_entry_sources: Vec<String> = manifest
            .abilities
            .iter()
            .map(|a| a.src_entry.clone())
            .chain(
                manifest
                    .extension_abilities
                    .iter()
                    .map(|ext| ext.src_entry.clone()),
            )
            .chain(manifest.src_entry.iter().cloned())
            .collect();

        // Local unit-test builds replace the module pages with the generated
        // `.test/testability/pages/Index` entry; the real main pages do not
        // participate in the `test` compilation (hvigor `UnitTestArkTS`
        // inputs). The `ohosTest` target similarly compiles the generated
        // `src/ohosTest` pages (`$profile:test_pages` lives in the template
        // tree, not `src/main/resources`). Preview keeps the normal pages.
        if let Some(pages_ref) = &manifest.pages {
            if !matches!(config.output_target, OutputTarget::Test) && target_name != "ohosTest" {
                runtime_entry_sources
                    .extend(Self::read_main_page_entries(module_layout, pages_ref)?);
            }
        }

        Ok(runtime_entry_sources)
    }

    fn read_main_page_entries(
        module_layout: &ProfileModuleLayout<'_>,
        pages_ref: &str,
    ) -> Result<Vec<String>, ArkdownError> {
        let profile_name =
            ProfileResourceRef::profile_name_or_bare(pages_ref).ok_or_else(|| {
                ArkdownError::ParseError {
                    file: module_layout.module_json5().display().to_string(),
                    message: "module.pages must reference a profile resource".to_string(),
                }
            })?;
        let profile_path = module_layout.profile_json(profile_name);
        let content = std::fs::read_to_string(&profile_path)?;
        let pages = serde_json::from_str::<MainPagesProfile>(&content).map_err(|error| {
            ArkdownError::ParseError {
                file: profile_path.display().to_string(),
                message: error.to_string(),
            }
        })?;

        Ok(pages
            .src
            .iter()
            .map(|page| Self::compile_entry_from_page(page))
            .collect())
    }

    fn bundle_ohm_url(bundle_name: &str, module_name: &str, source_path: &Path) -> String {
        OhmUrl::bundle(
            bundle_name,
            module_name,
            &ArtifactPathSerializer::rel_to_slash(source_path),
        )
        .into_string()
    }

    fn router_ohm_module_name<'a>(
        host_module_name: &'a str,
        module_name: &'a str,
    ) -> LoaderRouterModuleName<'a> {
        LoaderRouterModuleName::new(host_module_name, module_name)
    }

    /// Build a [`LoaderJson`] from the processed profile data.
    ///
    /// Mirrors hvigor's `GenerateLoaderJson` task, which produces the manifest
    /// the bundler reads for entry points, module paths, and dependency metadata.
    pub fn generate(
        config: &ArkdownConfig,
        module_cfg: &ModuleConfig,
        manifest: &HapManifest,
    ) -> Self {
        Self::try_generate(config, module_cfg, manifest, "default")
            .unwrap_or_else(|err| panic!("failed to generate loader.json: {err}"))
    }

    /// Fallible form of [`LoaderJson::generate`] with an explicit module
    /// target name (e.g. `ohosTest` keys `generated/r/<target>/`).
    pub fn try_generate(
        config: &ArkdownConfig,
        module_cfg: &ModuleConfig,
        manifest: &HapManifest,
        target_name: &str,
    ) -> Result<Self, ArkdownError> {
        // Build the dependency graph ONCE per try_generate() invocation.
        // Per SPEC-048: the same snapshot feeds all downstream helpers.
        let dep_graph = {
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "local_dependency_graph_build");
            crate::dependency_graph::LocalDependencyGraph::build(
                &module_cfg.src_path,
                &config.project_root,
            )
        };
        Self::try_generate_with_dependency_graph(
            config,
            module_cfg,
            manifest,
            &dep_graph,
            target_name,
            LoaderCompilationPolicy::default(),
        )
    }

    pub(crate) fn try_generate_with_dependency_graph(
        config: &ArkdownConfig,
        module_cfg: &ModuleConfig,
        manifest: &HapManifest,
        dep_graph: &crate::dependency_graph::LocalDependencyGraph,
        target_name: &str,
        compilation_policy: LoaderCompilationPolicy,
    ) -> Result<Self, ArkdownError> {
        // Local unit-test builds root at `<module>/.test` (hvigor
        // `buildRoot=.test`), which moves cache/assets/loader paths below it.
        let module_layout = if matches!(config.output_target, OutputTarget::Test) {
            ProfileModuleLayout::for_test_product(&module_cfg.src_path, &config.product.name)
        } else if matches!(config.output_target, OutputTarget::Preview) {
            ProfileModuleLayout::for_preview_product(
                &module_cfg.src_path,
                &config.product.name,
                target_name,
            )
        } else if target_name != "default" {
            ProfileModuleLayout::for_target_product(
                &module_cfg.src_path,
                &config.product.name,
                target_name,
            )
        } else {
            ProfileModuleLayout::for_product(&module_cfg.src_path, &config.product.name)
        };
        let runtime_entry_sources = {
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "runtime_entry_sources");
            Self::collect_runtime_entry_sources(config, manifest, &module_layout, target_name)?
        };

        // Collect compile_entry from runtime srcEntry values.
        let mut compile_entry = Vec::new();
        if matches!(config.output_target, OutputTarget::Test) {
            // hvigor UnitTestArkTS input order puts the generated testability
            // templates before the module abilities (filesInfo.txt:
            // ResourceTable, TestAbility, Index, EntryAbility, List.test).
            // es2abc compiles in input order, so the entry order must match.
            let testability_dir = module_layout.testability_dir();
            for entry in [
                testability_dir.join("TestAbility.ets"),
                testability_dir.join("pages").join("Index.ets"),
            ] {
                if entry.is_file() {
                    compile_entry.push(to_absolute_path_string(&entry));
                }
            }
            // `UnitTestArkCompile.initDefaultArkCompileConfig()` supplies
            // `mainModulePath` for the first selected entry module. The SDK
            // compiler consequently adds that module's ability and extension
            // srcEntry values even though local-test loader.json serializes
            // only mock sources and ResourceTable.ts. Arkdown's in-process
            // bundler needs the same entries explicitly.
            compile_entry.extend(runtime_entry_sources.iter().cloned());
        } else if target_name == "ohosTest" {
            // The `ohosTest` target compiles the generated template tree
            // (`.test/<product>/intermediates/src/ohosTest/ets/`) — the
            // TestAbility srcEntry points there and the test pages live in
            // the template `resources`. Relative `./ets/...` srcEntry values
            // resolve against the module ets root, which does not exist for
            // the ohosTest module; the absolute template paths below are the
            // compiler inputs.
            let template_ets = module_layout.ohos_test_template_ets_root();
            for entry in [
                template_ets.join("testability").join("TestAbility.ets"),
                template_ets
                    .join("testability")
                    .join("pages")
                    .join("Index.ets"),
                template_ets
                    .join("testrunner")
                    .join("OpenHarmonyTestRunner.ets"),
            ] {
                if entry.is_file() {
                    compile_entry.push(to_absolute_path_string(&entry));
                }
            }
            for source in runtime_entry_sources
                .iter()
                .filter(|source| !Path::new(source).has_root() && !source.starts_with("./ets/"))
            {
                compile_entry.push(source.clone());
            }
        } else {
            compile_entry.extend(runtime_entry_sources.iter().cloned());
        }
        let runtime_only_collector = RuntimeOnlyCollector::new(config, module_cfg);
        let module_json_path = module_layout.module_json5();
        let form_sources = {
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "form_sources");
            FormProfileProcessor::new().collect_module_form_sources(ModuleFormProfileInput {
                module_name: &module_cfg.name,
                module_root: &module_cfg.src_path,
                module_type: &module_cfg.module_type,
                module_json_path: &module_json_path,
                extension_abilities: &manifest.extension_abilities,
                project_modules: &config.modules,
                compatible_api_version: parse_api_version(&config.product.compatible_sdk_version)
                    .or(Some(config.sdk_config.compile_api_version)),
            })?
        };
        let src_main_root = module_layout.src_main();
        // Widget (form card) sources never enter the module compile entries:
        // hvigor compiles them in the dedicated widget pass (`widgets.abc`)
        // under `needSubmitArkTsWidget`, while the non-full-compilation
        // `compileEntry` only carries ResourceTable.ts and the configured
        // entries. The form collection above still runs for its profile
        // validations (`FORM_WIDGET_MODULE_INVALID_*`).
        let form_entry_sources = form_sources
            .iter()
            .map(|form_source| {
                if let Ok(rel) = form_source.source_path.strip_prefix(&src_main_root) {
                    Self::compile_entry_from_src_main_rel(rel)
                } else {
                    to_absolute_path_string(&form_source.source_path)
                }
            })
            .collect::<Vec<_>>();

        // Module-level srcEntry (used by HSP shared packages that export a single file
        // without declaring abilities).
        if let Some(src_entry) = &manifest.src_entry {
            compile_entry.push(src_entry.clone());
        }
        if compile_entry.is_empty()
            && matches!(
                manifest.module_type,
                OhosModuleType::Har | OhosModuleType::Shared
            )
        {
            if let Some(main) = Self::read_package_main(&module_cfg.src_path) {
                compile_entry.push(to_absolute_path_string(&module_cfg.src_path.join(main)));
            }
        }

        // Discover WebWorker files from src/main/ets/workers. Upstream
        // `main.js::readWorkerFile()` accepts explicit loader.json `workers` entries
        // with .ets/.ts/.js suffixes; `loadWorker()` otherwise scans this directory
        // recursively. Keep both surfaces aligned: serialize absolute worker paths
        // for loader consumers and add relative `./ets/...` entries for arkdown's
        // in-process bundler.
        let worker_files = {
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "worker_files");
            runtime_only_collector.discover_worker_files()
        };
        let mut discovered_workers: Vec<String> = vec![];
        let ets_root = module_layout.ets_source_root();
        for path in &worker_files {
            if let Ok(rel) = path.strip_prefix(&ets_root) {
                compile_entry.push(Self::compile_entry_from_ets_rel(rel));
            }
            discovered_workers.push(to_absolute_path_string(path));
        }
        let dependency_worker_files = {
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "dependency_worker_files");
            Self::collect_dependency_worker_files(dep_graph)
        };
        for path in &dependency_worker_files {
            let abs = to_absolute_path_string(path);
            if !discovered_workers.contains(&abs) {
                discovered_workers.push(abs.clone());
            }
            // The SDK loader consumes `workers` as a separate compiler entry
            // surface. Arkdown's in-process bundler has one input list, so keep
            // every dependency worker reachable internally without leaking it
            // into serialized HAP/HSP `compileEntry` below.
            compile_entry.push(abs);
        }

        // Merge in explicit sourceOption.workers entries from build-profile.json5.
        // Hvigor's `targetService.getWorkerConfig()` resolves these to absolute paths;
        // we do the same in resolve.rs, so they arrive here already absolute.
        if let Some(source_opt) = config.product.build_option.source_option.as_ref() {
            for worker_path in &source_opt.workers {
                let abs = to_absolute_path_string(worker_path);
                if !discovered_workers.contains(&abs) {
                    // Also add a compile entry if the file is under src/main/ets.
                    if let Ok(rel) = worker_path.strip_prefix(&ets_root) {
                        let entry = Self::compile_entry_from_ets_rel(rel);
                        if !compile_entry.contains(&entry) {
                            compile_entry.push(entry);
                        }
                    }
                    discovered_workers.push(abs);
                }
            }
        }

        let use_normalized_ohm_url = config
            .product
            .build_option
            .strict_mode
            .use_normalized_ohm_url;
        let module_is_bytecode_har = matches!(config.output_target, OutputTarget::Har)
            && matches!(module_cfg.module_type, OhosModuleType::Har)
            && matches!(
                config.product.build_option.har_artifact_type,
                arkdown_core::HarArtifactType::Bytecode
            );
        let collects_bytecode_har_inputs =
            use_normalized_ohm_url && !matches!(config.output_target, OutputTarget::Har);
        let bundles_bytecode_har_dependencies = matches!(config.output_target, OutputTarget::Har)
            && matches!(
                config.product.build_option.har_artifact_type,
                arkdown_core::HarArtifactType::Bytecode
            )
            && config
                .product
                .build_option
                .ark_options
                .as_ref()
                .is_some_and(|options| options.bundled_dependencies);
        let pkg_contexts = {
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "dependency_pkg_contexts");
            use_normalized_ohm_url
                .then(|| Self::dependency_pkg_contexts(config, module_cfg, dep_graph))
                .flatten()
        };
        let router_map = {
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "router_map");
            Self::read_loader_router_maps(
                config,
                module_cfg,
                manifest.router_map.as_deref(),
                dep_graph,
                pkg_contexts.as_ref(),
            )
        };

        // Full HSP compilation starts from every code file below
        // `src/main/ets`. This mirrors GenerateLoaderJson's
        // allowToCollectEtsFolder()/collectCompileFiles contract and ensures
        // exported shared-library files participate even without abilities.
        // Form widget modules compile in non-full-compilation mode
        // (`isFullCompilationEnabled` is false without `OHOS_COMPILE_LIB_ENTRY`):
        // hvigor's compileEntry only carries ResourceTable.ts and the
        // configured entries, never the ets tree (widget pages go to
        // `widgets.abc` via `needSubmitArkTsWidget`).
        let is_form_widget_module = std::fs::read_to_string(&module_json_path)
            .ok()
            .and_then(|content| json5::from_str::<ModuleJson5>(&content).ok())
            .is_some_and(|json| json.module.form_extension_module.is_some());
        let full_compilation =
            compilation_policy.is_full_compilation_enabled(is_form_widget_module);
        let collect_own_ets = target_name != "ohosTest"
            && full_compilation
            && matches!(
                module_cfg.module_type,
                OhosModuleType::Shared | OhosModuleType::Har
            );
        let mut serialized_compile_entry = if collect_own_ets {
            Self::collect_code_files(&ets_root)
        } else {
            Vec::new()
        };
        // Dependency ETS folders are not unconditional full-compilation
        // inputs. GenerateLoaderJson.collectDepEtsFolderFilesEntry() is called
        // only from processHarDynamicImport() for a selected runtimeOnly HAR;
        // RuntimeOnlyCollector::collect_package_source_files() below owns that
        // filtered path, including remote-HAR exclusion.
        serialized_compile_entry.extend({
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "insight_intent_compile_entries");
            Self::collect_insight_intent_src_entries(module_cfg)
        });
        serialized_compile_entry.extend({
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "mock_compile_entries");
            Self::collect_mock_compile_entries(config, module_cfg, target_name)
        });
        // Device-test target: add the module's src/ohosTest/ets sources.
        serialized_compile_entry.extend({
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "ohos_test_compile_entries");
            Self::collect_ohos_test_compile_entries(module_cfg, target_name)
        });
        serialized_compile_entry.extend({
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "app_startup_compile_entries");
            Self::collect_app_startup_compile_entries(module_cfg)
        });
        serialized_compile_entry.extend({
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "runtime_only_source_files");
            runtime_only_collector.collect_source_files()
        });
        serialized_compile_entry.extend({
            let _timer =
                LoaderTimingProbe::new(&module_cfg.name, "runtime_only_package_source_files");
            runtime_only_collector.collect_package_source_files()
        });
        // `GenerateLoaderJson.processBundledDependencies` adds dependency
        // workers only while producing a bundled ByteCode HAR. HAP/HSP builds
        // still expose them through `workers`, but compile dependency abilities
        // through `collectDepAbilitySrcEntryArr` below.
        if bundles_bytecode_har_dependencies {
            serialized_compile_entry.extend(dependency_worker_files);
        }
        serialized_compile_entry.extend({
            let _timer =
                LoaderTimingProbe::new(&module_cfg.name, "dependency_router_map_source_files");
            Self::collect_dependency_router_map_source_files(
                config,
                module_cfg,
                dep_graph,
                pkg_contexts.as_ref(),
            )
        });
        serialized_compile_entry.extend({
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "dependency_ability_sources");
            Self::collect_dependency_ability_source_files(dep_graph)
        });
        serialized_compile_entry.extend({
            let _timer =
                LoaderTimingProbe::new(&module_cfg.name, "dependency_insight_intent_sources");
            Self::collect_dependency_insight_intent_src_entries(dep_graph)
        });
        {
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "compile_entry_dedupe");
            runtime_only_collector.dedupe_paths(&mut serialized_compile_entry);
        }
        // Latest GenerateLoaderJson.addResourceTable2CompileEntry appends the
        // restool-generated TS table after normal source filtering. It gates
        // on the selected SDK component API, not the project's target API.
        if !matches!(module_cfg.module_type, OhosModuleType::Har)
            && !matches!(config.output_target, OutputTarget::Preview)
            && config.sdk_config.sdk_component_api_version >= 23
        {
            serialized_compile_entry.push(module_layout.generated_resource_table_typescript());
        }

        let mut hvigor_compile_entry: Vec<String> = serialized_compile_entry
            .iter()
            .map(|path| to_absolute_path_string(path))
            .collect();
        if matches!(config.output_target, OutputTarget::Test) {
            // Local unit-test builds compile in non-full-compilation mode:
            // hvigor's serialized compileEntry carries only ResourceTable.ts
            // (plus mock sources — `collectMockFileEntry` runs before
            // `addResourceTable2CompileEntry` for local-test mode). The
            // testability templates and src/test sources reach the compiler
            // through moduleJson abilities / the replaced main_pages entry.
            let mock_sources_dir = canonicalize_or_owned(&module_layout.mock_sources_dir());
            hvigor_compile_entry.retain(|entry| {
                let path = Path::new(entry);
                path.file_name() == Some(OsStr::new("ResourceTable.ts"))
                    || path.strip_prefix(&mock_sources_dir).is_ok()
            });
        }
        for path in &serialized_compile_entry {
            if let Ok(rel) = path.strip_prefix(&src_main_root) {
                compile_entry.push(Self::compile_entry_from_src_main_rel(rel));
            } else {
                compile_entry.push(to_absolute_path_string(path));
            }
        }
        if matches!(config.output_target, OutputTarget::Test) {
            // The `src/test` suite participates in the local-test compiler
            // input surface (testability templates were added up front).
            let test_sources = module_layout.test_sources_dir();
            if test_sources.is_dir() {
                for file in Self::collect_code_files(&test_sources) {
                    compile_entry.push(to_absolute_path_string(&file));
                }
            }
        }

        let bytecode_har_deps = if collects_bytecode_har_inputs {
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "bytecode_har_graph_scan");
            ByteCodeHarMetadataWriter::scan_bytecode_har_deps_from_graph(
                &module_cfg.src_path,
                &config.project_root,
            )
        } else {
            Vec::new()
        };
        let other_compile_paths = {
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "bytecode_other_compile_files");
            LoaderByteCodeHarFields::collect_other_compile_files(
                &bytecode_har_deps,
                &module_cfg.src_path,
                &config.project_root,
            )
        };
        let other_compile_files: Vec<String> = other_compile_paths
            .iter()
            .map(|path| to_absolute_path_string(path))
            .collect();
        compile_entry.extend(other_compile_files.iter().cloned());

        // Deduplicate while preserving order.
        {
            let mut seen = FxHashSet::default();
            compile_entry.retain(|e| seen.insert(e.clone()));
        }
        if matches!(config.output_target, OutputTarget::Har) {
            for entry in &compile_entry {
                let path = Path::new(entry);
                let absolute = if path.has_root() {
                    to_absolute_path_string(path)
                } else {
                    to_absolute_path_string(&src_main_root.join(entry.trim_start_matches("./")))
                };
                if !hvigor_compile_entry.contains(&absolute) {
                    hvigor_compile_entry.push(absolute);
                }
            }
        }

        let module_root = to_absolute_path_string(&module_cfg.src_path);
        let project_root = to_absolute_path_string(&config.project_root);
        let src_main = to_absolute_path_string(&module_layout.src_main());

        let build_mode_str = config.build_mode.as_str().to_string();

        let compile_mode = match manifest.compile_mode {
            arkdown_core::CompileMode::EsModule => "esmodule",
            arkdown_core::CompileMode::JsBundle => "jsbundle",
        }
        .to_string();

        let mut module_path_map = IndexMap::new();
        for project_module in &config.modules {
            module_path_map.insert(
                project_module.name.clone(),
                to_absolute_path_string(&project_module.src_path),
            );
        }
        if module_path_map.is_empty() {
            module_path_map.insert(module_cfg.name.clone(), module_root.clone());
        }

        let byte_code_har_info: IndexMap<String, ByteCodeHarEntry> = bytecode_har_deps
            .iter()
            .cloned()
            .map(|e| e.into_loader_entry())
            .collect();

        let declaration_entry = if use_normalized_ohm_url {
            let mut entries: Vec<String> = bytecode_har_deps
                .iter()
                .flat_map(|dep| dep.declaration_entry.iter().cloned())
                .collect();

            // D2: When this module is itself a ByteCode HAR (metadata.byteCodeHar=true),
            // generate its own declarationEntry from abilities + extension abilities.
            // This provides the OHM URLs that downstream consumers of THIS ByteCode HAR
            // will put into their own loader.json declarationEntry.
            let bundle_name = &config.product.bundle_name;
            let mod_name = &module_cfg.name;
            if module_is_bytecode_har {
                for ability_entry in &manifest.abilities {
                    let Some(path) = Self::source_entry_stem_path(&ability_entry.src_entry) else {
                        continue;
                    };
                    let ohm = Self::bundle_ohm_url(&bundle_name, mod_name, &path);
                    entries.push(ohm);
                }
                for ext_ability in &manifest.extension_abilities {
                    let Some(path) = Self::source_entry_stem_path(&ext_ability.src_entry) else {
                        continue;
                    };
                    let ohm = Self::bundle_ohm_url(&bundle_name, mod_name, &path);
                    entries.push(ohm);
                }
            }

            let mut seen = FxHashSet::default();
            entries.retain(|entry| seen.insert(entry.clone()));
            entries
        } else {
            Vec::new()
        };
        let update_version_info = if matches!(config.output_target, OutputTarget::Har) {
            IndexMap::new()
        } else {
            ByteCodeHarScanEntry::collect_bytecode_har_update_version_info(
                &bytecode_har_deps,
                &module_cfg.src_path,
                &config.project_root,
            )
        };

        // --- TASK-LOADER-02: populate dynamicImportLibInfo from oh-package.json5 ---
        let dynamic_import_lib_info: IndexMap<String, serde_json::Value> = if module_is_bytecode_har
        {
            IndexMap::new()
        } else {
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "dynamic_import_lib_info");
            let dynamic_dep_names = runtime_only_collector.collect_dynamic_import_dep_names();
            let entries = LoaderByteCodeHarFields::collect_dynamic_import_lib_info(
                &module_cfg.name,
                &dynamic_dep_names,
                &module_cfg.src_path,
                &config.project_root,
            );
            entries
                .into_iter()
                .filter_map(|(k, v)| serde_json::to_value(v).ok().map(|json| (k, json)))
                .collect()
        };

        // --- D5: hspNameOhmMap/harNameOhmMap from direct oh_modules deps ---
        let (hsp_name_ohm_map, har_name_ohm_map) = {
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "hsp_har_ohm_maps");
            Self::collect_hsp_har_ohm_maps(config, module_cfg, dep_graph, module_is_bytecode_har)
        };

        let aot_fields = {
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "aot_loader_fields");
            AotLoaderFields::from_config(config, module_cfg, manifest, target_name)
        };
        let build_config_path = {
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "loader_build_config_path");
            Self::loader_build_config_path(config, module_cfg, target_name)
        };
        let build_config = {
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "loader_build_config_read");
            Self::read_loader_build_config(&build_config_path)
        };
        let patch_config = (!matches!(config.output_target, OutputTarget::Test))
            .then(|| {
                build_config
                    .as_ref()
                    .and_then(|config| config.get("patchConfig").cloned())
            })
            .flatten();
        let serialized_build_config_path = match config.output_target {
            // hvigor GenerateLoaderJson:
            // `(isPreview || isLocalTest()) && (c.buildConfigPath =
            // relative(modulePath, buildConfigPath))`.
            // The active ModulePathInfo build root is `build`, `.preview`, or
            // `.test`; `getBuildConfigPath()` always appends
            // `config/buildConfig.json` directly to that root.
            OutputTarget::Test => Some(".test/config/buildConfig.json".to_string()),
            OutputTarget::Preview => build_config_path
                .strip_prefix(&module_cfg.src_path)
                .map(ArtifactPathSerializer::rel_to_slash)
                .ok()
                .or_else(|| Some(to_absolute_path_string(&build_config_path))),
            _ => None,
        };

        let loader_out_root = module_layout.loader_out_default_dir();
        let node_modules_path = to_absolute_path_string(&loader_out_root.join("node_modules"));
        // Local unit-test builds run the `UnitTestArkTS` task; its cache
        // directory (`default@UnitTestArkTS`) participates in es2abc module
        // identities, so the task name must match hvigor exactly.
        let compile_task_name = if matches!(config.output_target, OutputTarget::Test) {
            "UnitTestArkTS"
        } else if matches!(config.output_target, OutputTarget::Preview) {
            "PreviewArkTS"
        } else if matches!(config.output_target, OutputTarget::Har) {
            "HarCompileArkTS"
        } else {
            "CompileArkTS"
        };
        let compile_arkts_cache = module_layout
            .build_default()
            .join("cache")
            .join("default")
            .join(format!("default@{compile_task_name}"))
            .join(&compile_mode)
            .join(&build_mode_str);
        let cache_path = Self::absolute_dir_path_string(&compile_arkts_cache);
        let ets_loader_out = loader_out_root.join("ets");
        let ace_module_build = Self::absolute_dir_path_string(&ets_loader_out);
        let hsp_resources_map = {
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "hsp_resources_map");
            Self::collect_hsp_resources_map(dep_graph)
        };
        let pkg_name_to_pkg_brief_info = {
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "pkg_brief_info");
            Self::build_pkg_brief_info(config, module_cfg, dep_graph)
        };

        Ok(LoaderJson {
            module_path_map,
            compile_mode,
            project_root_path: project_root.clone(),
            node_modules_path,
            byte_code_har_info,
            declaration_entry,
            module_name: module_cfg.name.clone(),
            hsp_name_ohm_map,
            har_name_ohm_map,
            package_manager_type: "ohpm".to_string(),
            hvigor_compile_entry,
            other_compile_files,
            dynamic_import_lib_info,
            router_map,
            hsp_resources_map,
            update_version_info,
            customized_har: config
                .product
                .build_option
                .customized_har_base_package
                .is_some(),
            byte_code_har: module_is_bytecode_har.then_some(true),
            // The common loader.json never carries the widget-only field.
            pkg_name2_other_compile_files: None,
            an_build_out_put: aot_fields.an_build_out_put,
            an_build_mode: aot_fields.an_build_mode,
            ap_path: aot_fields.ap_path,
            build_config_path: serialized_build_config_path,
            patch_config,
            non_entry_importees: vec![],
            compiled_importees: vec![],

            compile_entry,
            form_entry_sources,
            runtime_entry_sources,
            module_root_path: src_main.clone(),
            cache_path,
            ace_module_build,
            build_mode: build_mode_str,
            support_chunks: config
                .product
                .build_option
                .ark_options
                .as_ref()
                .and_then(|ao| ao.support_chunks.then_some(true))
                .unwrap_or(false),
            use_normalized_ohm_url,
            api_version: config.sdk_config.compile_api_version,
            fallback_an_build: aot_fields.fallback_an_build,
            pkg_name_to_pkg_brief_info,
            workers: discovered_workers,
            // GenerateLoaderJson does not serialize sourceRoots. HAR package
            // source roots belong to oh-package.json5 metadata and are added
            // later by ProcessHarArtifacts.processPackageJson.
            source_roots: Vec::new(),
            bundle_dependencies: {
                use crate::dependency_graph::DependencyNodeType;
                dep_graph
                    .nodes
                    .values()
                    .filter(|node| {
                        matches!(
                            node.node_type,
                            DependencyNodeType::SourceHar | DependencyNodeType::SharedHsp
                        )
                    })
                    .map(|node| node.package_name.clone())
                    .collect()
            },
            global_dependencies: Vec::new(),
            compiler_types: Vec::new(),
            compiler_type_declaration_inputs: Vec::new(),
            dependency_pkg_version: Vec::new(),
            extra_importees: Vec::new(),
            compiled_package_names: Vec::new(),
            oh_exports: Vec::new(),
        })
    }
}

/// Metadata for a bytecode-compiled HAR dependency.
///
/// Written into `loader.json#byteCodeHarInfo` for each ByteCode HAR
/// that the current HAP/HSP depends on.
///
/// # Upstream correspondence
///
/// Mirrors `generate-loader-json.ts#setByteCodeHarInfoAndOtherCompileFiles`
/// and `process-har-artifacts.ts#processDeclarationEntry`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ByteCodeHarEntry {
    /// Absolute path to the compiled `.abc` file.
    pub abc_path: String,
    /// Compatible SDK version, preserving whether the package declared a JSON
    /// number (for example `12`) or string (for example `"5.0.0(12)"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compatible_sdk_version: Option<CompatibleSdkVersionValue>,
    /// Compatible SDK stage (for example `"beta1"`), if declared by the HAR.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compatible_sdk_version_stage: Option<String>,
    /// Absolute path to the `sourceMaps.map` file, if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_maps_path: Option<String>,
    /// Absolute path to the insight intent metadata, if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insight_intent: Option<String>,
    /// ByteCode HAR package metadata consumed to build the top-level loader
    /// `declarationEntry`. Hvigor does not serialize this field inside each
    /// `byteCodeHarInfo` object.
    #[serde(skip)]
    pub declaration_entry: Vec<String>,
}

// ---------------------------------------------------------------------------
// Generation
// ---------------------------------------------------------------------------

/// Generate a [`LoaderJson`] from the project config, module config, and
/// processed manifest.
///
/// Entry points are collected from:
/// 1. Each ability's `src_entry` path in the manifest
/// 2. The module-level `src_entry` from the module.json5 (if recorded)
///
/// When `useNormalizedOHMUrl` is `true`, callers are expected to convert the
/// `src_entry` paths to OHM URLs (`@bundle:…`) before passing the manifest —
/// this function records whatever values are in `manifest.abilities[].src_entry`.

/// Build the `pkgNameToPkgBriefInfo` map for the loader.json.
///
/// Produces one entry for the current module. HAR/HSP dependencies would be
/// added by a later pass (see `arkdown-plugin-har-dedup`).

/// Return an absolute, normalized path string for `path`.
///
/// Strategy (matches hvigor's behavior of always writing absolute paths):
/// 1. Try `AbsPath::canonicalize` — resolves existing paths without leaking
///    Windows verbatim prefixes into SDK-facing artifacts.
///    on disk (project root, module source root).
/// 2. If canonicalize fails (path not yet on disk, or a hypothetical path used in
///    tests), fall back to:
///    - If `path` is already absolute: normalize `.`/`..` segments in place.
///    - If `path` is relative: join it onto the current working directory, then
///      normalize.
pub(super) fn to_absolute_path_string(path: &Path) -> String {
    // Fast path: canonicalize works for real, existing paths.
    if let Ok(abs) = AbsPath::canonicalize(path) {
        return ArtifactPathSerializer::path_to_slash_string(abs.as_path())
            .unwrap_or_else(|_| abs.to_string());
    }

    // Fallback for non-existent or constructed paths.
    let base = if path.has_root() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };
    normalize_path_string(&base)
}

pub(super) fn normalize_path_string(path: &Path) -> String {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push(component.as_os_str());
                }
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    if normalized.as_os_str().is_empty() {
        ".".to_string()
    } else {
        ArtifactPathSerializer::path_to_slash_string(&normalized)
            .unwrap_or_else(|_| normalized.display().to_string())
    }
}

impl LoaderJson {
    fn read_package_main(module_root: &Path) -> Option<String> {
        let path = module_root.join("oh-package.json5");
        let content = std::fs::read_to_string(path).ok()?;

        #[derive(serde::Deserialize)]
        struct PackageMain {
            main: Option<String>,
        }

        json5::from_str::<PackageMain>(&content).ok()?.main
    }

    fn read_loader_router_maps(
        config: &ArkdownConfig,
        host_module: &ModuleConfig,
        router_map_ref: Option<&str>,
        graph: &crate::dependency_graph::LocalDependencyGraph,
        pkg_contexts: Option<&std::collections::BTreeMap<String, PkgContextInfoOpt>>,
    ) -> Vec<serde_json::Value> {
        let mut router_maps = Vec::new();
        let host_router_module = RouterMapModule {
            module_name: host_module.name.clone(),
            package_name: Self::dependency_package_name(&host_module.src_path),
            src_path: host_module.src_path.clone(),
        };
        router_maps.extend(Self::read_loader_router_map(
            &config.product.bundle_name,
            &host_module.name,
            &host_router_module,
            router_map_ref,
            None,
        ));

        for router_module in Self::dependency_router_map_modules(config, host_module, graph) {
            let Some(router_map_ref) = Self::read_module_router_map_ref(&router_module.src_path)
            else {
                continue;
            };
            router_maps.extend(Self::read_loader_router_map(
                &config.product.bundle_name,
                &host_module.name,
                &router_module,
                Some(&router_map_ref),
                pkg_contexts.and_then(|contexts| contexts.get(&router_module.package_name)),
            ));
        }

        router_maps
    }

    fn dependency_router_map_modules(
        config: &ArkdownConfig,
        host_module: &ModuleConfig,
        graph: &crate::dependency_graph::LocalDependencyGraph,
    ) -> Vec<RouterMapModule> {
        let mut modules = Vec::new();
        let mut seen_roots = FxHashSet::default();
        seen_roots.insert(to_absolute_path_string(&host_module.src_path));

        for module_cfg in Self::router_map_module_order(config, host_module) {
            let root = to_absolute_path_string(&module_cfg.src_path);
            if !seen_roots.insert(root) {
                continue;
            }
            modules.push(RouterMapModule {
                module_name: module_cfg.name.clone(),
                package_name: Self::dependency_package_name(&module_cfg.src_path),
                src_path: module_cfg.src_path.clone(),
            });
        }

        for node in graph.source_hars() {
            if node.pkg_root.as_os_str().is_empty() {
                continue;
            }
            let root = to_absolute_path_string(&node.pkg_root);
            if !seen_roots.insert(root) {
                continue;
            }
            let module_name = Self::read_dependency_module_json(&node.pkg_root)
                .and_then(|module_json| module_json.module.name)
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| {
                    ModuleSpecifierText::new(&node.package_name)
                        .package_module_name_candidate()
                        .unwrap_or(&node.package_name)
                        .to_string()
                });
            modules.push(RouterMapModule {
                module_name,
                package_name: node.package_name.clone(),
                src_path: node.pkg_root.clone(),
            });
        }

        modules
    }

    fn router_map_module_order<'a>(
        config: &'a ArkdownConfig,
        host_module: &ModuleConfig,
    ) -> Vec<&'a ModuleConfig> {
        let candidates = config
            .modules
            .iter()
            .filter(|module_cfg| module_cfg.name != host_module.name)
            .map(|module_cfg| {
                (
                    module_cfg,
                    Self::dependency_package_name(&module_cfg.src_path),
                )
            })
            .collect::<Vec<_>>();
        let dependencies = Self::read_dependency_package_json(&host_module.src_path)
            .map(|package| package.dependencies)
            .unwrap_or_default();

        let mut ordered = Vec::new();
        let mut seen = FxHashSet::default();
        for dep_name in dependencies.keys() {
            let Some((module_cfg, _)) = candidates.iter().find(|(module_cfg, package_name)| {
                !seen.contains(module_cfg.name.as_str())
                    && Self::dependency_matches_module(dep_name, module_cfg, package_name)
            }) else {
                continue;
            };
            seen.insert(module_cfg.name.clone());
            ordered.push(*module_cfg);
        }
        for (module_cfg, _) in candidates {
            if seen.insert(module_cfg.name.clone()) {
                ordered.push(module_cfg);
            }
        }
        ordered
    }

    fn dependency_matches_module(
        dep_name: &str,
        module_cfg: &ModuleConfig,
        package_name: &str,
    ) -> bool {
        dep_name == package_name
            || dep_name == module_cfg.name
            || ModuleSpecifierText::new(dep_name)
                .package_module_name_candidate()
                .is_some_and(|name| name == package_name || name == module_cfg.name)
    }

    fn read_loader_router_map(
        bundle_name: &str,
        host_module_name: &str,
        module_cfg: &RouterMapModule,
        router_map_ref: Option<&str>,
        pkg_context: Option<&PkgContextInfoOpt>,
    ) -> Vec<serde_json::Value> {
        let Some(profile_name) = router_map_ref.and_then(|value| value.strip_prefix("$profile:"))
        else {
            return vec![];
        };

        let module_layout = ProfileModuleLayout::new(&module_cfg.src_path);
        let path = module_layout.profile_json(profile_name);
        let Ok(content) = std::fs::read_to_string(path) else {
            return vec![];
        };
        let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&content) else {
            return vec![];
        };

        let Some(entries) = value
            .get_mut("routerMap")
            .and_then(serde_json::Value::as_array_mut)
        else {
            return value.as_array().cloned().unwrap_or_default();
        };

        for entry in entries.iter_mut() {
            let Some(obj) = entry.as_object_mut() else {
                continue;
            };
            let Some(page_source_file) = obj
                .get("pageSourceFile")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
            else {
                continue;
            };
            if Path::new(&page_source_file).has_root() {
                continue;
            }
            let absolute = to_absolute_path_string(
                &module_layout.source_from_src_main_entry(Path::new(&page_source_file)),
            );
            let page_source_path = Self::source_entry_stem_path(&page_source_file)
                .and_then(|path| module_layout.source_entry_relative_to_ets(&path));
            let Some(rel_ets) = page_source_path else {
                continue;
            };
            let rel_ets = ArtifactPathSerializer::rel_to_slash(&Path::new("ets").join(rel_ets));
            let ohmurl = if let Some(context) = pkg_context {
                Self::normalized_ohm_from_pkg_context_source(context, &page_source_file)
            } else {
                let ohm_module_name =
                    Self::router_ohm_module_name(host_module_name, &module_cfg.module_name)
                        .string();
                Self::bundle_ohm_url(bundle_name, &ohm_module_name, Path::new(&rel_ets))
            };
            obj.insert("ohmurl".to_string(), serde_json::Value::String(ohmurl));
            obj.insert(
                "pageSourceFile".to_string(),
                serde_json::Value::String(absolute),
            );
        }

        entries.clone()
    }

    fn read_module_router_map_ref(module_root: &Path) -> Option<String> {
        #[derive(serde::Deserialize)]
        struct ModuleJson {
            module: ModuleRouterMap,
        }
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ModuleRouterMap {
            router_map: Option<String>,
        }
        let layout = ProfileModuleLayout::new(module_root);
        for path in [layout.module_json5(), layout.module_json()] {
            let Ok(content) = std::fs::read_to_string(path) else {
                continue;
            };
            if let Ok(parsed) = json5::from_str::<ModuleJson>(&content) {
                if parsed.module.router_map.is_some() {
                    return parsed.module.router_map;
                }
            }
        }
        None
    }

    fn collect_dependency_router_map_source_files(
        config: &ArkdownConfig,
        host_module: &ModuleConfig,
        graph: &crate::dependency_graph::LocalDependencyGraph,
        pkg_contexts: Option<&std::collections::BTreeMap<String, PkgContextInfoOpt>>,
    ) -> Vec<PathBuf> {
        let mut files = Vec::new();
        for router_module in Self::dependency_router_map_modules(config, host_module, graph) {
            let Some(router_map_ref) = Self::read_module_router_map_ref(&router_module.src_path)
            else {
                continue;
            };
            let router_map = Self::read_loader_router_map(
                &config.product.bundle_name,
                &host_module.name,
                &router_module,
                Some(&router_map_ref),
                pkg_contexts.and_then(|contexts| contexts.get(&router_module.package_name)),
            );
            files.extend(router_map.iter().filter_map(|entry| {
                entry
                    .get("pageSourceFile")
                    .and_then(serde_json::Value::as_str)
                    .map(PathBuf::from)
            }));
        }
        files
    }

    fn collect_dependency_ability_source_files(
        graph: &crate::dependency_graph::LocalDependencyGraph,
    ) -> Vec<PathBuf> {
        Self::collect_dependency_source_har_roots(graph)
            .into_iter()
            .flat_map(|dep_root| Self::collect_dependency_module_ability_source_files(&dep_root))
            .collect()
    }

    fn collect_dependency_worker_files(
        graph: &crate::dependency_graph::LocalDependencyGraph,
    ) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let mut seen = FxHashSet::default();
        for dep_root in Self::collect_dependency_source_har_roots(graph) {
            for worker in Self::collect_dependency_worker_entries(&dep_root) {
                let path = dep_root.join(arkdown_path::strip_relative_prefix(&worker));
                if Self::is_existing_loader_code_file(&path)
                    && seen.insert(to_absolute_path_string(&path))
                {
                    files.push(path);
                }
            }
        }
        files
    }

    fn collect_dependency_worker_entries(dep_root: &Path) -> Vec<String> {
        let mut workers = Vec::new();
        if let Some(module_json) = Self::read_dependency_module_json(dep_root) {
            workers.extend(
                module_json
                    .module
                    .extension_abilities
                    .iter()
                    .filter_map(|ability| {
                        (ability.extension_type.as_deref() == Some("workScheduler"))
                            .then(|| ability.src_entry.clone())
                            .flatten()
                    }),
            );
        }
        workers.extend(Self::read_dependency_package_workers(dep_root));
        workers.extend(Self::read_dependency_build_profile_workers(dep_root));

        let mut seen = FxHashSet::default();
        workers.retain(|worker| seen.insert(worker.clone()));
        workers
    }

    fn read_dependency_package_workers(dep_root: &Path) -> Vec<String> {
        let path = dep_root.join("oh-package.json5");
        let Ok(content) = std::fs::read_to_string(path) else {
            return vec![];
        };
        #[derive(Deserialize)]
        struct PackageWorkers {
            #[serde(default)]
            metadata: PackageWorkerMetadata,
        }
        #[derive(Default, Deserialize)]
        struct PackageWorkerMetadata {
            #[serde(default)]
            workers: Vec<String>,
        }
        json5::from_str::<PackageWorkers>(&content)
            .map(|package| package.metadata.workers)
            .unwrap_or_default()
    }

    fn read_dependency_build_profile_workers(dep_root: &Path) -> Vec<String> {
        let path = dep_root.join("build-profile.json5");
        let Ok(content) = std::fs::read_to_string(path) else {
            return vec![];
        };
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct BuildProfileWorkers {
            #[serde(default)]
            build_option: BuildProfileOption,
        }
        #[derive(Default, Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct BuildProfileOption {
            #[serde(default)]
            source_option: BuildProfileSourceOption,
        }
        #[derive(Default, Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct BuildProfileSourceOption {
            #[serde(default)]
            workers: Vec<String>,
        }
        json5::from_str::<BuildProfileWorkers>(&content)
            .map(|profile| profile.build_option.source_option.workers)
            .unwrap_or_default()
    }

    fn collect_dependency_insight_intent_src_entries(
        graph: &crate::dependency_graph::LocalDependencyGraph,
    ) -> Vec<PathBuf> {
        Self::collect_dependency_source_har_roots(graph)
            .into_iter()
            .flat_map(|dep_root| {
                Self::collect_dependency_module_insight_intent_src_entries(&dep_root)
            })
            .collect()
    }

    fn collect_dependency_source_har_roots(
        graph: &crate::dependency_graph::LocalDependencyGraph,
    ) -> Vec<PathBuf> {
        graph
            .source_hars()
            .into_iter()
            .filter(|node| !node.pkg_root.as_os_str().is_empty())
            .map(|node| node.pkg_root.clone())
            .collect()
    }

    fn collect_dependency_module_ability_source_files(dep_root: &Path) -> Vec<PathBuf> {
        let Some(module_json) = Self::read_dependency_module_json(dep_root) else {
            return vec![];
        };

        let src_main = ProfileModuleLayout::new(dep_root).src_main();
        module_json
            .module
            .abilities
            .iter()
            .chain(module_json.module.extension_abilities.iter())
            .filter_map(|ability| ability.src_entry.as_deref())
            .map(|src_entry| src_main.join(arkdown_path::strip_relative_prefix(src_entry)))
            .filter(|path| Self::is_existing_loader_code_file(path))
            .collect()
    }

    fn collect_dependency_module_insight_intent_src_entries(dep_root: &Path) -> Vec<PathBuf> {
        let mut entries = Vec::new();
        for resource_dir in Self::dependency_resource_dirs(dep_root) {
            let profile_path = resource_dir
                .join("base")
                .join("profile")
                .join("insight_intent.json");
            let Ok(content) = std::fs::read_to_string(profile_path) else {
                continue;
            };

            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct InsightIntentProfile {
                #[serde(default)]
                insight_intents_src_entry: Vec<InsightIntentEntry>,
            }

            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct InsightIntentEntry {
                src_entry: Option<String>,
            }

            let Ok(profile) = json5::from_str::<InsightIntentProfile>(&content) else {
                continue;
            };
            let src_main = ProfileModuleLayout::new(dep_root).src_main();
            entries.extend(
                profile
                    .insight_intents_src_entry
                    .iter()
                    .filter_map(|entry| entry.src_entry.as_deref())
                    .map(|src_entry| src_main.join(arkdown_path::strip_relative_prefix(src_entry)))
                    .filter(|path| Self::is_existing_loader_code_file(path)),
            );
        }
        entries
    }

    fn dependency_resource_dirs(dep_root: &Path) -> Vec<PathBuf> {
        let directories = Self::read_dependency_package_json(dep_root)
            .and_then(|pkg| pkg.metadata)
            .and_then(|metadata| metadata.resource)
            .map(|resource| resource.directories)
            .unwrap_or_default();

        if directories.is_empty() {
            return vec![ProfileModuleLayout::new(dep_root).resources_root()];
        }

        directories
            .into_iter()
            .map(|directory| dep_root.join(arkdown_path::strip_relative_prefix(&directory)))
            .collect()
    }

    fn read_dependency_package_json(pkg_root: &Path) -> Option<DependencyPackageJson> {
        let content = std::fs::read_to_string(pkg_root.join("oh-package.json5")).ok()?;
        json5::from_str::<DependencyPackageJson>(&content).ok()
    }

    fn read_dependency_module_json(pkg_root: &Path) -> Option<DependencyModuleJson> {
        let layout = ProfileModuleLayout::new(pkg_root);
        [layout.module_json5(), layout.module_json()]
            .into_iter()
            .find_map(|path| std::fs::read_to_string(path).ok())
            .and_then(|content| json5::from_str::<DependencyModuleJson>(&content).ok())
    }

    fn loader_build_config_path(
        config: &ArkdownConfig,
        module_cfg: &ModuleConfig,
        target_name: &str,
    ) -> PathBuf {
        let layout = match config.output_target {
            OutputTarget::Preview => ProfileModuleLayout::for_preview_product(
                &module_cfg.src_path,
                &config.product.name,
                target_name,
            ),
            OutputTarget::Test => {
                ProfileModuleLayout::for_test_product(&module_cfg.src_path, &config.product.name)
            }
            _ if target_name != "default" => ProfileModuleLayout::for_target_product(
                &module_cfg.src_path,
                &config.product.name,
                target_name,
            ),
            _ => ProfileModuleLayout::for_product(&module_cfg.src_path, &config.product.name),
        };
        let candidates = layout.build_config_candidates();
        candidates
            .iter()
            .find(|path| path.is_file())
            .cloned()
            .unwrap_or_else(|| candidates[0].clone())
    }

    fn read_loader_build_config(path: &Path) -> Option<serde_json::Value> {
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content)
            .or_else(|_| json5::from_str(&content))
            .ok()
    }

    fn collect_insight_intent_src_entries(module_cfg: &ModuleConfig) -> Vec<PathBuf> {
        let module_layout = ProfileModuleLayout::new(&module_cfg.src_path);
        let profile_path = module_layout.insight_intent_profile_json();
        let Ok(content) = std::fs::read_to_string(&profile_path) else {
            return vec![];
        };

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct InsightIntentProfile {
            #[serde(default)]
            insight_intents: Vec<InsightIntentEntry>,
            #[serde(default)]
            insight_intents_src_entry: Vec<InsightIntentEntry>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct InsightIntentEntry {
            src_entry: Option<String>,
        }

        let Ok(profile) = json5::from_str::<InsightIntentProfile>(&content) else {
            return vec![];
        };
        let src_main = module_layout.src_main();
        let mut entries = Vec::new();
        for entry in profile
            .insight_intents
            .iter()
            .chain(profile.insight_intents_src_entry.iter())
        {
            let Some(src_entry) = entry.src_entry.as_deref() else {
                continue;
            };
            let path = src_main.join(arkdown_path::strip_relative_prefix(src_entry));
            if Self::is_existing_loader_code_file(&path) {
                entries.push(path);
            }
        }
        entries
    }

    fn collect_mock_compile_entries(
        config: &ArkdownConfig,
        module_cfg: &ModuleConfig,
        target_name: &str,
    ) -> Vec<PathBuf> {
        if !matches!(
            config.output_target,
            OutputTarget::Preview | OutputTarget::Test
        ) && target_name != "ohosTest"
        {
            return vec![];
        }

        let mock_config_path = ProfileModuleLayout::new(&module_cfg.src_path).mock_config_json5();
        let Ok(content) = std::fs::read_to_string(mock_config_path) else {
            return vec![];
        };
        let Ok(mock_entries) = json5::from_str::<IndexMap<String, serde_json::Value>>(&content)
        else {
            return vec![];
        };

        mock_entries
            .into_values()
            .filter_map(|entry| {
                let source = entry.get("source").and_then(|source| source.as_str())?;
                if source.is_empty() {
                    return None;
                }
                let path = module_cfg.src_path.join(source);
                Self::is_existing_hvigor_compile_entry_path(&path).then_some(path)
            })
            .collect()
    }

    /// Collect `src/ohosTest/ets/` source files for ohosTest-target builds.
    ///
    /// NOTE: the local unit-test build (`--target test`, `hvigorw test`) does
    /// NOT include `src/ohosTest` sources — its compiler inputs are the
    /// generated `.test/testability` templates plus `src/test` (see the
    /// local-test entry extension above). `src/ohosTest` participates in the
    /// separate `ohosTest` target build (`-p module=entry@ohosTest`), which is
    /// is selected by the module target name rather than `OutputTarget`.
    fn collect_ohos_test_compile_entries(
        module_cfg: &ModuleConfig,
        target_name: &str,
    ) -> Vec<PathBuf> {
        if target_name != "ohosTest" {
            return vec![];
        }

        let test_ets_root = ProfileModuleLayout::new(&module_cfg.src_path).ohos_test_ets_root();
        if !test_ets_root.is_dir() {
            return vec![];
        }

        Self::collect_code_files(&test_ets_root)
    }

    fn collect_code_files(root: &Path) -> Vec<PathBuf> {
        let mut entries = Vec::new();
        let mut directories = std::collections::VecDeque::from([root.to_path_buf()]);
        while let Some(dir) = directories.pop_front() {
            let Ok(read_dir) = std::fs::read_dir(&dir) else {
                continue;
            };
            let mut dir_entries: Vec<_> = read_dir.flatten().collect();
            dir_entries.sort_by_key(|e| e.file_name());
            for entry in dir_entries {
                let path = entry.path();
                if path.is_dir() {
                    directories.push_back(path);
                } else if Self::is_existing_loader_code_file(&path) {
                    entries.push(path);
                }
            }
        }
        entries
    }

    fn collect_app_startup_compile_entries(module_cfg: &ModuleConfig) -> Vec<PathBuf> {
        let mut entries = Vec::new();
        entries.extend(Self::collect_app_startup_config_entry(module_cfg));
        entries.extend(Self::collect_app_startup_task_entries(module_cfg));
        entries
    }

    fn collect_app_startup_config_entry(module_cfg: &ModuleConfig) -> Vec<PathBuf> {
        let Some(app_startup_ref) = Self::read_module_app_startup_ref(module_cfg) else {
            return vec![];
        };
        let Some(profile_name) = app_startup_ref.strip_prefix("$profile:") else {
            return vec![];
        };
        let module_layout = ProfileModuleLayout::new(&module_cfg.src_path);
        let profile_path = module_layout.profile_json(profile_name);
        let Ok(content) = std::fs::read_to_string(profile_path) else {
            return vec![];
        };

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct StartupProfile {
            config_entry: Option<String>,
        }

        let Ok(profile) = json5::from_str::<StartupProfile>(&content) else {
            return vec![];
        };
        let Some(config_entry) = profile.config_entry else {
            return vec![];
        };
        let path = module_layout.source_from_module_src(Path::new(
            arkdown_path::strip_relative_prefix(&config_entry),
        ));
        Self::is_existing_loader_code_file(&path)
            .then_some(path)
            .into_iter()
            .collect()
    }

    fn collect_app_startup_task_entries(module_cfg: &ModuleConfig) -> Vec<PathBuf> {
        let temp_startup_path =
            ProfileModuleLayout::new(&module_cfg.src_path).startup_config_temp_json();
        let Ok(content) = std::fs::read_to_string(temp_startup_path) else {
            return vec![];
        };

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct TempStartupConfig {
            #[serde(default)]
            startup_tasks: Vec<TempStartupTask>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct TempStartupTask {
            src_entry_absolute_path: Option<PathBuf>,
        }

        let Ok(config) = serde_json::from_str::<TempStartupConfig>(&content)
            .or_else(|_| json5::from_str::<TempStartupConfig>(&content))
        else {
            return vec![];
        };
        config
            .startup_tasks
            .into_iter()
            .filter_map(|task| task.src_entry_absolute_path)
            .filter(|path| Self::is_existing_loader_code_file(path))
            .collect()
    }

    fn read_module_app_startup_ref(module_cfg: &ModuleConfig) -> Option<String> {
        let path = ProfileModuleLayout::new(&module_cfg.src_path).module_json5();
        let content = std::fs::read_to_string(path).ok()?;
        #[derive(Deserialize)]
        struct ModuleJson {
            module: ModuleAppStartup,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ModuleAppStartup {
            app_startup: Option<String>,
        }
        json5::from_str::<ModuleJson>(&content)
            .ok()?
            .module
            .app_startup
    }

    fn is_existing_loader_code_file(path: &Path) -> bool {
        if !path.is_file() {
            return false;
        }
        let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
            return false;
        };
        matches!(extension.to_ascii_lowercase().as_str(), "ets" | "ts" | "js")
            && !SourceFileName::new(path).is_declaration()
    }

    fn is_existing_hvigor_compile_entry_path(path: &Path) -> bool {
        path.exists() && !SourceFileName::new(path).is_declaration()
    }

    /// Walk declared `oh_modules` dependencies, find installed HSP packages,
    /// and collect their `ResourceTable.txt` paths.
    ///
    /// Uses `LocalDependencyGraph` for BFS-consistent node ordering and
    /// correct type classification (only SharedHsp nodes contribute).
    /// Matches hvigor's `getHspResourcesMap()` for HSP dependencies.
    fn collect_hsp_resources_map(
        graph: &crate::dependency_graph::LocalDependencyGraph,
    ) -> FxHashMap<String, String> {
        let mut map = FxHashMap::default();

        for node in graph.shared_hsps() {
            if node.pkg_root.as_os_str().is_empty() {
                continue; // RemoteOrUnknown with empty path
            }
            // Check for ResourceTable.txt in priority order:
            // 1. Build output intermediates (hvigor target intermediates layout)
            // 2. Dep root (remote HSP / pre-packaged layout)
            let resource_table = [
                ProfileModuleLayout::new(&node.pkg_root).resource_table_txt(),
                node.pkg_root.join("ResourceTable.txt"),
            ]
            .into_iter()
            .find(|p| p.is_file());
            if let Some(resource_table) = resource_table {
                map.entry(node.package_name.clone())
                    .or_insert_with(|| to_absolute_path_string(&resource_table));
            }
        }
        map
    }

    fn dependency_package_name(pkg_dir: &Path) -> String {
        Self::read_dependency_package_json(pkg_dir)
            .and_then(|pkg| pkg.name)
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| Self::dependency_package_name_from_path(pkg_dir))
    }

    fn dependency_package_name_from_path(pkg_dir: &Path) -> String {
        let name = pkg_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let parent = pkg_dir
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|name| name.to_str());
        if let Some(parent) = parent.filter(|parent| {
            matches!(
                ModuleSpecifierText::new(parent).kind(),
                SpecifierKind::ScopedPackage
            )
        }) {
            ArtifactPath::from_slash(parent).join(name).into_string()
        } else {
            name.to_string()
        }
    }

    /// Scan HSP dependencies, and HAR dependencies only for ByteCode HAR
    /// modules, to build `hspNameOhmMap` / `harNameOhmMap`.
    ///
    /// Matches hvigor's `getHspNameOhmMap()` / `getHarNameOhmMap()`:
    /// `getHspNameOhmMap()` always collects HSP dependencies, while
    /// `getHarNameOhmMap()` returns `{}` unless `targetService.isByteCodeHar()`.
    /// In normalized mode both maps are produced from `pkgContextInfo` via
    /// `generateOhmUrl(...)`, so HAR dependencies keep an empty `moduleName`.
    ///
    /// Key:   dependency package name
    /// Value: OHM URL in the format determined by `useNormalizedOHMUrl`:
    ///   - `false` (default): `@bundle:<bundleName>/<moduleName>/<entryRelPath>`
    ///   - `true`:  `@normalized:N&<pkgContext.moduleName>&<pkgContext.bundleName>&<packageName>/<entryRelPath>&<pkgContext.version>`
    fn collect_hsp_har_ohm_maps(
        config: &ArkdownConfig,
        module_cfg: &ModuleConfig,
        graph: &crate::dependency_graph::LocalDependencyGraph,
        module_is_bytecode_har: bool,
    ) -> (IndexMap<String, String>, IndexMap<String, String>) {
        use crate::dependency_graph::DependencyNodeType;
        let bundle_name = &config.product.bundle_name;
        let use_normalized = config
            .product
            .build_option
            .strict_mode
            .use_normalized_ohm_url;

        let mut hsp_map = IndexMap::new();
        let mut har_map = IndexMap::new();
        let pkg_contexts = use_normalized
            .then(|| Self::dependency_pkg_contexts(config, module_cfg, graph))
            .flatten();

        for node in graph.nodes.values() {
            let is_hsp = node.node_type == DependencyNodeType::SharedHsp;
            let is_har_map_dependency = match node.node_type {
                // A bundled ByteCode HAR compiles source HARs into its ABC and
                // maps the remaining ByteCode HAR / OTHER barriers. The
                // coordinator replaces this coarse local-graph selection with
                // the exact session-tree view before serialization.
                DependencyNodeType::SourceHar => !config
                    .product
                    .build_option
                    .ark_options
                    .as_ref()
                    .is_some_and(|options| options.bundled_dependencies),
                DependencyNodeType::BytecodeHar
                | DependencyNodeType::NativeSo
                | DependencyNodeType::RuntimeOnly
                | DependencyNodeType::RemoteOrUnknown => true,
                DependencyNodeType::SharedHsp => false,
            };
            if !is_hsp && !(module_is_bytecode_har && use_normalized && is_har_map_dependency) {
                continue;
            }
            if node.pkg_root.as_os_str().is_empty() {
                continue;
            }

            let pkg_dir = &node.pkg_root;
            let Some(parsed) = Self::read_dependency_module_json(pkg_dir) else {
                continue;
            };
            let dep_name = &node.package_name;
            let entry_rel = Self::dep_entry_rel_path(pkg_dir);
            let ohm_url = if use_normalized {
                let Some(context) = pkg_contexts
                    .as_ref()
                    .and_then(|contexts| contexts.get(dep_name))
                else {
                    continue;
                };
                Self::normalized_ohm_from_pkg_context(context)
            } else {
                if is_har_map_dependency {
                    continue;
                }
                let module_name = parsed
                    .module
                    .name
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| {
                        ModuleSpecifierText::new(dep_name)
                            .package_module_name_candidate()
                            .unwrap_or(dep_name)
                            .to_string()
                    });
                OhmUrl::bundle(bundle_name, &module_name, &entry_rel).into_string()
            };

            if is_hsp {
                hsp_map.insert(dep_name.clone(), ohm_url);
            } else {
                har_map.insert(dep_name.clone(), ohm_url);
            }
        }

        (hsp_map, har_map)
    }

    fn dependency_pkg_contexts(
        config: &ArkdownConfig,
        module_cfg: &ModuleConfig,
        graph: &crate::dependency_graph::LocalDependencyGraph,
    ) -> Option<std::collections::BTreeMap<String, PkgContextInfoOpt>> {
        use crate::dependency_graph::DependencyNodeType;
        let current_pkg = Self::read_dependency_package_json(&module_cfg.src_path);
        let current = PkgContextCurrentInput {
            package_name: current_pkg
                .as_ref()
                .and_then(|pkg| pkg.name.clone())
                .unwrap_or_else(|| module_cfg.name.clone()),
            bundle_name: config.product.bundle_name.clone(),
            bundle_type: "app".to_string(),
            module_kind: Self::pkg_context_module_kind(&module_cfg.module_type),
            module_root: module_cfg.src_path.clone(),
            source_root: ProfileModuleLayout::new(&module_cfg.src_path).src_main(),
            main: current_pkg.as_ref().and_then(|pkg| pkg.main.clone()),
            version: current_pkg
                .as_ref()
                .map(|pkg| pkg.version.clone())
                .unwrap_or_default(),
            oh_export_files: None,
        };
        let mut dependencies = Vec::new();
        for node in graph.nodes.values() {
            let kind = match node.node_type {
                DependencyNodeType::SourceHar | DependencyNodeType::BytecodeHar => {
                    PkgContextDependencyKind::Har
                }
                DependencyNodeType::SharedHsp => PkgContextDependencyKind::Hsp,
                DependencyNodeType::NativeSo => PkgContextDependencyKind::So,
                DependencyNodeType::RuntimeOnly | DependencyNodeType::RemoteOrUnknown => {
                    PkgContextDependencyKind::Other
                }
            };
            if node.pkg_root.as_os_str().is_empty() {
                continue;
            }
            let package = Self::read_dependency_package_json(&node.pkg_root);
            let module = Self::read_dependency_module_json(&node.pkg_root);
            dependencies.push(PkgContextDependencyInput {
                alias: node.package_name.clone(),
                package_name: node.package_name.clone(),
                version_spec: package.as_ref().map(|pkg| pkg.version.clone()),
                package_root: Some(node.pkg_root.clone()),
                node_entry: package
                    .as_ref()
                    .and_then(|pkg| pkg.main.clone())
                    .or_else(|| Some("index.js".to_string())),
                main: package
                    .as_ref()
                    .and_then(|pkg| pkg.main.clone().or_else(|| pkg.types.clone())),
                kind,
                module_name: module
                    .as_ref()
                    .and_then(|module| module.module.name.clone()),
                bundle_name: None,
                bundle_type: None,
                oh_export_files: None,
            });
        }
        let input = PkgContextInfoInput {
            current,
            dependencies,
            native_libraries: Vec::new(),
        };
        PkgContextInfoGenerator::new().generate(&input).ok()
    }

    fn pkg_context_module_kind(module_type: &OhosModuleType) -> PkgContextModuleKind {
        match module_type {
            OhosModuleType::Shared => PkgContextModuleKind::Hsp,
            OhosModuleType::Har => PkgContextModuleKind::Har,
            OhosModuleType::Entry | OhosModuleType::Feature => PkgContextModuleKind::Hap,
        }
    }

    fn normalized_ohm_from_pkg_context(context: &PkgContextInfoOpt) -> String {
        let normalized_import = if context.entry_path.is_empty() {
            String::new()
        } else {
            let entry_path = Self::source_entry_stem_path(&context.entry_path)
                .map(|path| ArtifactPathSerializer::rel_to_slash(&path))
                .unwrap_or_else(|| context.entry_path.clone());
            ArtifactPath::from_slash(&context.package_name)
                .join(&entry_path)
                .into_string()
        };
        OhmUrl::normalized_non_so(
            &context.module_name,
            &context.bundle_name,
            &normalized_import,
            &context.version,
        )
        .into_string()
    }

    fn normalized_ohm_from_pkg_context_source(
        context: &PkgContextInfoOpt,
        source_entry: &str,
    ) -> String {
        let entry_path = Self::source_entry_stem_path(source_entry)
            .map(|path| ArtifactPathSerializer::rel_to_slash(&path))
            .unwrap_or_else(|| source_entry.to_string());
        let normalized_import = ArtifactPath::from_slash(&context.package_name)
            .join(&entry_path)
            .into_string();
        OhmUrl::normalized_non_so(
            &context.module_name,
            &context.bundle_name,
            &normalized_import,
            &context.version,
        )
        .into_string()
    }

    /// Return the entry file path relative to `pkg_dir` with extension stripped.
    ///
    /// Reads `oh-package.json5#main`; falls back to conventional candidates.
    fn dep_entry_rel_path(pkg_dir: &Path) -> String {
        #[derive(Deserialize)]
        struct PkgEntry {
            #[serde(default)]
            main: Option<String>,
        }
        let main_file: Option<String> = std::fs::read_to_string(pkg_dir.join("oh-package.json5"))
            .ok()
            .and_then(|s| json5::from_str::<PkgEntry>(&s).ok())
            .and_then(|p| p.main)
            .filter(|m| !m.is_empty());

        if let Some(main) = main_file {
            if let Some(rel) = Self::source_entry_stem_path(&main) {
                return ArtifactPathSerializer::rel_to_slash(&rel);
            }
        }

        // Conventional fallbacks: these cover the common package layouts.
        for candidate_rel in &["src/main/ets/index", "src/main/ets/Index", "index", "Index"] {
            let candidate = pkg_dir.join(Path::new(candidate_rel).with_extension("ets"));
            if candidate.exists() {
                return candidate_rel.to_string();
            }
            let candidate = pkg_dir.join(Path::new(candidate_rel).with_extension("ts"));
            if candidate.exists() {
                return candidate_rel.to_string();
            }
        }

        "index".to_string()
    }

    fn build_pkg_brief_info(
        config: &ArkdownConfig,
        module_cfg: &ModuleConfig,
        graph: &crate::dependency_graph::LocalDependencyGraph,
    ) -> FxHashMap<String, PkgBriefInfo> {
        let mut map = FxHashMap::default();
        let module_root = to_absolute_path_string(&module_cfg.src_path);
        let ets_src = to_absolute_path_string(
            &ProfileModuleLayout::new(&module_cfg.src_path).ets_source_root(),
        );

        // Detect effective target to include target-specific source root
        // (`src/<target>/ets/`) when a non-default target is active.
        //
        // hvigor adds `src/<target>` as an additional `sourceRoot` in
        // `pkgNameToPkgBriefInfo` when the effective target is not `default`.
        // This allows the compiler to resolve target-specific source files.
        let build_mode_name = config.build_mode.as_str();
        let target_ets_src = Self::detect_target_specific_ets_src(
            &module_cfg.src_path,
            &config.project_root,
            &module_cfg.name,
            &config.product.name,
            build_mode_name,
        );

        // Add the current module with original (src/main/ets) as originalSourceRoots
        // and target-specific source root (when present) prepended to sourceRoots.
        let source_roots = if let Some(ref target_ets) = target_ets_src {
            vec![target_ets.clone(), ets_src.clone()]
        } else {
            vec![ets_src.clone()]
        };
        map.insert(
            module_cfg.name.clone(),
            PkgBriefInfo {
                pkg_root: module_root.clone(),
                pkg_name: module_cfg.name.clone(),
                source_roots,
                original_source_roots: vec![ets_src],
            },
        );

        // Add all other workspace modules (hvigor includes the full target module graph).
        for project_module in &config.modules {
            if project_module.name == module_cfg.name {
                continue; // already added above
            }
            let pkg_root = to_absolute_path_string(&project_module.src_path);
            let pkg_ets_src = to_absolute_path_string(
                &ProfileModuleLayout::new(&project_module.src_path).ets_source_root(),
            );
            map.insert(
                project_module.name.clone(),
                PkgBriefInfo {
                    pkg_root: pkg_root.clone(),
                    pkg_name: project_module.name.clone(),
                    source_roots: vec![pkg_ets_src.clone()],
                    original_source_roots: vec![pkg_ets_src],
                },
            );
        }

        // Add source-type dependency packages from the shared dependency graph.
        // BytecodeHar and NativeSo are excluded (no ETS sources to compile).
        {
            use crate::dependency_graph::DependencyNodeType;
            for node in graph.nodes.values() {
                if matches!(
                    node.node_type,
                    DependencyNodeType::BytecodeHar | DependencyNodeType::NativeSo
                ) {
                    continue; // pre-compiled or native-only: no ETS sources
                }
                if node.pkg_root.as_os_str().is_empty() {
                    continue; // remote package not installed locally
                }
                let ets_dir = ProfileModuleLayout::new(&node.pkg_root).ets_source_root();
                if !ets_dir.is_dir() {
                    continue; // no source tree → skip
                }
                let pkg_name = &node.package_name;
                if map.contains_key(pkg_name.as_str()) {
                    continue; // prefer workspace module entry
                }
                let pkg_root_str = to_absolute_path_string(&node.pkg_root);
                let pkg_ets_src = to_absolute_path_string(&ets_dir);
                map.insert(
                    pkg_name.clone(),
                    PkgBriefInfo {
                        pkg_root: pkg_root_str,
                        pkg_name: pkg_name.clone(),
                        source_roots: vec![pkg_ets_src.clone()],
                        original_source_roots: vec![pkg_ets_src],
                    },
                );
            }
        }

        map
    }

    /// Detect an active non-default target's `src/<target>/ets/` directory.
    ///
    /// hvigor includes this path in `pkgNameToPkgBriefInfo.sourceRoots` when
    /// a non-default effective target is selected.  The `originalSourceRoots`
    /// keeps `src/main/ets` so the compiler can resolve both roots.
    ///
    /// Returns `None` when:
    /// - There is no non-default effective target for this module.
    /// - The effective target's `src/<name>/ets/` directory does not exist.
    fn detect_target_specific_ets_src(
        module_root: &std::path::Path,
        project_root: &std::path::Path,
        module_name: &str,
        product_name: &str,
        build_mode_name: &str,
    ) -> Option<String> {
        use arkdown_config::build_profile::RawBuildProfile;

        let profile = RawBuildProfile::load(project_root).ok()?;
        let raw_module = profile.modules.iter().find(|m| m.name == module_name)?;

        // Use centralised target selection for consistent priority rules.
        let target = arkdown_config::build_profile::pick_effective_raw_target(
            &raw_module.targets,
            product_name,
            build_mode_name,
        )?;

        // Only emit a target source root for non-default targets.
        if target.name == "default" {
            return None;
        }

        let target_ets = module_root.join("src").join(&target.name).join("ets");
        if target_ets.is_dir() {
            Some(to_absolute_path_string(&target_ets))
        } else {
            None
        }
    }
}

/// Select the effective target for pkgBriefInfo source-root detection.
///
// Target selection is now centralised in
// arkdown_config::build_profile::pick_effective_raw_target.

fn default_build_mode() -> String {
    "debug".to_string()
}

#[cfg(test)]
mod tests;
