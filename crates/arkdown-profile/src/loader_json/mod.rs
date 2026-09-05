use rustc_hash::{FxHashMap, FxHashSet};
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use arkdown_config::build_profile::parse_api_version;
use arkdown_core::{
    ArkdownConfig, ArkdownError, HapManifest, InsightIntentFormBinding, ModuleConfig, ModuleJson5,
    OhosModuleType, OutputTarget, PkgBriefInfo,
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

mod aot;
pub(crate) use aot::{AotLoaderFields, DependencyPackageJson};
mod dependencies;
mod generate;

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
    ///
    /// `compiler/main.js::loadBuildJson` coerces a missing value with `!!`.
    /// The current GenerateLoaderJson output therefore omits the key for the
    /// ordinary `false` case and serializes it only for a customized HAR.
    #[serde(default, skip_serializing_if = "is_false")]
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
    /// only for widget transitive-dependency compilation; the common loader
    /// and an ordinary widget loader omit it.
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
    /// Form-extension ability ownership and registered widget names used by
    /// the Rust InsightIntent transform. This never enters hvigor loader.json.
    #[serde(skip)]
    pub insight_intent_form_bindings: Vec<InsightIntentFormBinding>,
    /// Source entries that the OHOS runtime loads directly from `module.json`.
    #[serde(skip)]
    pub runtime_entry_sources: Vec<String>,
    /// UI page entries originating from the module `pages` profile.
    ///
    /// The upstream compiler appends `?entry` to both page and ability Rollup
    /// inputs, but excludes `abilityPagesFullPath` before applying page-only
    /// `@Entry`/`@Preview` checks. Keep the page set separately so this
    /// distinction survives the Rust profile-to-bundler handoff.
    #[serde(skip)]
    pub page_entry_sources: Vec<String>,
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

fn is_false(value: &bool) -> bool {
    !*value
}

struct LoaderRouterModuleName<'a> {
    host_module_name: &'a str,
    module_name: &'a str,
}

struct RouterMapModule {
    module_name: String,
    package_name: String,
    src_path: PathBuf,
    router_map_ref: Option<String>,
    profile_source: RouterMapProfileSource,
}

enum RouterMapProfileSource {
    Disk,
    Session(Option<Arc<serde_json::Value>>),
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

fn default_build_mode() -> String {
    "debug".to_string()
}

#[cfg(test)]
mod tests;
