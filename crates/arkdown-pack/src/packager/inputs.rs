//! Typed inputs for HAP/HSP/HAR packaging.

use std::collections::BTreeMap;
use std::path::PathBuf;

use arkdown_core::{HarArtifactType, RuntimeOnlyConfig};
use arkdown_path::ArtifactPath;
use indexmap::IndexMap;

use crate::integrated_hsp::IntegratedHspEntry;

/// Metadata owned by hvigor's `MakePackInfo` task and shared by normal and
/// fast module packaging.
#[derive(Debug, Clone)]
pub struct PackInfoModuleInput {
    pub module_name: String,
    /// Complete source-set `module.json5` model used by Hvigor's
    /// `ModuleModelImpl.getJsonObjByTargetName`.
    pub module_json: serde_json::Value,
    pub resources_src_dirs: Vec<PathBuf>,
    /// Intersection of the feature and related-entry target device types.
    pub device_types: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PackInfoInput {
    pub module_name: String,
    /// Unsuffixed archive identity returned by Hvigor's
    /// `getModuleTargetOutputFileName(..., null)` after removing `.hap`.
    pub package_name: String,
    /// Complete current source-set model. This is carried from the immutable
    /// build session so JS context replacements and unmodelled schema fields
    /// are not lost through a later filesystem read. It deliberately is not
    /// the `MergeProfile` output: `MakePackInfo.getModuleObj` reads the raw
    /// module model.
    pub module_json: serde_json::Value,
    pub resources_src_dirs: Vec<PathBuf>,
    pub bundle_version_code: u64,
    pub bundle_version_name: String,
    pub bundle_build_version: Option<String>,
    /// `AppScope/app.json5.app.minCompatibleVersionCode`. Hvigor reads this
    /// from the app resource model rather than from the merged module profile.
    pub min_compatible_version_code: Option<u64>,
    pub bundle_type: String,
    /// Remaining module summaries, in Hvigor emission order. This contains a
    /// related entry for feature packages and the default-source-set mirrors
    /// appended by an `ohosTest` build.
    pub additional_modules: Vec<PackInfoModuleInput>,
    pub device_types: Vec<String>,
    /// Device types written to `packages[0]`. For a related-entry package this
    /// is the complete related target list, while each summary module receives
    /// the feature/entry intersection.
    pub package_device_types: Vec<String>,
    pub installation_free: bool,
    pub delivery_with_install: bool,
}

#[derive(serde::Serialize)]
pub struct PackInput {
    pub module_name: String,
    /// Effective Hvigor target name used by MakePackInfo archive identity.
    pub target_name: String,
    /// Path to the SDK frontend's merged ABC when CompileArkTS ran. Dynamic
    /// ArkTS produces `modules.abc`; static ArkTS produces
    /// `modules_static.abc`. Hvigor omits `--ets-path` entirely when the
    /// selected source set has no ETS code model, so source-less HAP/HSP
    /// modules carry no ABC.
    pub abc_path: Option<PathBuf>,
    /// Other merged ABCs produced for the same module. ArkTS 1.2 hybrid
    /// modules package dynamic `modules.abc` and static
    /// `modules_static.abc` together, matching Hvigor's `--ets-path` input.
    pub additional_abc_paths: Vec<PathBuf>,
    /// Path to `resources.index` produced by arkdown-resource.
    pub resources_index: PathBuf,
    /// Path to the processed `module.json` produced by arkdown-profile.
    pub module_json: PathBuf,
    /// Current-module profile used by Hvigor's MakePackInfo task.
    ///
    /// This is deliberately separate from `module_json`: ProcessProfile may
    /// append dependency HAR abilities before packaging, while MakePackInfo
    /// reads the current module model and must not report dependency entries.
    pub pack_info_module_json: Option<PathBuf>,
    /// Output of the independent Hvigor `MakePackInfo` task. When supplied,
    /// packaging embeds these exact bytes instead of regenerating metadata.
    pub pack_info_path: Option<PathBuf>,
    /// Optional native `.so` libraries to bundle.
    pub native_libs: Vec<PathBuf>,
    /// Whether native `.so` libraries should be deflated in the package.
    pub compress_native_libs: bool,
    /// Destination archive path, e.g. `build/.../entry.hap`.
    pub output_path: PathBuf,
    /// Directory whose contents are copied into HAP under `resources/`.
    ///
    /// The build-service passes the resource compiler's processed `.../res/default/resources`
    /// directory so the archive contains restool-style resource files rather than
    /// raw source `element/*.json` inputs.
    pub resources_src_dir: Option<PathBuf>,
    /// Bundle version code (from `AppScope/app.json5` `versionCode`).
    pub bundle_version_code: u64,
    /// Bundle version name (from `AppScope/app.json5` `versionName`).
    pub bundle_version_name: String,
    /// Optional effective build version.
    ///
    /// Hvigor emits this as `summary.app.version.build` after `code` and
    /// `name`, and omits it entirely when no buildVersion is configured.
    pub bundle_build_version: Option<String>,
    /// Bundle type: `"app"` for HAP, `"shared"` for HSP.
    pub bundle_type: String,
    /// Path to `sourceMaps.map` produced by the bundler.
    pub source_maps_path: Option<PathBuf>,
    /// Effective `anBuildMode` emitted into loader.json. A non-empty value is
    /// the exact Hvigor gate for passing AOT/AP directories to the packing
    /// tool; this prevents stale intermediate directories from leaking into a
    /// non-AOT package.
    pub aot_mode: Option<String>,
    /// Path to `pkgContextInfo.json` produced by GeneratePkgContextInfo.
    ///
    /// Hvigor packages this at the HAP root when normalized OHM URLs are enabled.
    /// ArkRuntime uses it to map esmodule entry paths onto records inside
    /// `ets/modules.abc`.
    pub pkg_context_info_path: Option<PathBuf>,
    /// Path to legacy `pkgSdkInfo.json` emitted when normalized OHM URLs are
    /// disabled. The SDK packing tool copies these bytes to the archive root.
    pub pkg_sdk_info_path: Option<PathBuf>,
    /// Remote HSPs to embed inside this HAP (integrated HSP feature, API >= 11).
    ///
    /// Each entry is stored at `libs/<hsp_dir_name>/<hsp_file_name>` inside the archive.
    /// Pass an empty vec for HAPs without integrated HSPs (the common case).
    pub integrated_hsps: Vec<IntegratedHspEntry>,
    pub privacy_profile_path: Option<std::path::PathBuf>,
    /// `rpcid.sc` emitted by the SDK `syscap_tool` for OpenHarmony targets.
    /// HarmonyOS targets and HAR modules do not provide this input.
    pub syscap_path: Option<PathBuf>,
    /// Device types from the module manifest (e.g. `["phone", "tablet"]`).
    /// Used in pack.info `deviceType` field.  Defaults to `["phone"]` when empty.
    pub device_types: Vec<String>,
    /// Whether this module is installation-free (atomicService bundles).
    /// Used in pack.info `distro.installationFree` field.
    pub installation_free: bool,
    /// Whether this module is delivered with install.
    /// Used in pack.info `distro.deliveryWithInstall` and `packages[].deliveryWithInstall`.
    pub delivery_with_install: bool,
}

pub struct HarPackInput {
    pub module_name: String,
    /// Source ETS files directory (`src/main/ets/`).
    pub source_dir: PathBuf,
    /// Resources directory (`src/main/resources/`).
    pub resources_dir: PathBuf,
    /// Permission reason resources generated by MergeProfile as `resource_str`.
    /// Packaged under `src/main/resource/` for source HAR/interface HAR layout.
    pub resource_str_dir: Option<PathBuf>,
    /// Path to `oh-package.json5`.
    pub oh_package_json: PathBuf,
    /// Destination archive path.
    pub output_path: PathBuf,
    /// Directory containing declaration stubs (`index.d.ts`, `index.d.ets`).
    ///
    /// Hvigor copies these intermediates for compiled HAR variants. Original
    /// source HARs retain their ArkTS sources and deliberately omit generated
    /// declarations. When `None` (or the directory does not exist), declaration
    /// stubs are omitted.
    pub declarations_dir: Option<PathBuf>,
    /// Path to the consumer obfuscation rules file (`obfuscation.txt`).
    ///
    /// hvigor calls `generateConsumerObConfigFile()` and includes the result so
    /// consumers know which symbols the HAR exposes and how to configure their
    /// own obfuscation for packages that depend on this HAR.
    /// When `None` (or the file does not exist), consumer rules are omitted.
    pub consumer_rules_path: Option<PathBuf>,
}

impl HarPackInput {
    /// Project-wide package-manager decision made by hvigor's
    /// `CoreProjectModelImpl.isOhpmProject()`. The selected manifest path is
    /// carried as a platform-native `PathBuf`; archive names remain portable.
    pub(super) fn is_ohpm_project(&self) -> bool {
        self.oh_package_json
            .file_name()
            .is_some_and(|name| name == std::ffi::OsStr::new("oh-package.json5"))
    }

    pub(super) fn manifest_archive_name(&self) -> &'static str {
        if self.is_ohpm_project() {
            "oh-package.json5"
        } else {
            "package.json"
        }
    }
}

/// Variant-specific HAR intermediates produced by the compile, profile,
/// resource, and native stages. `AbstractProcessHarArtifacts` assembles these
/// into one temporary directory before calling ohpm; arkdown models the same
/// inputs explicitly so no source manifest has to be mutated.
#[derive(Debug, Clone, Default)]
pub struct HarPackageParameterInput {
    pub version: Option<String>,
    pub dependencies: IndexMap<String, String>,
    pub dev_dependencies: IndexMap<String, String>,
    pub dynamic_dependencies: IndexMap<String, String>,
}

#[derive(Debug, Clone, Default)]
pub struct HarBundledDependencyInput {
    pub dependencies: IndexMap<String, String>,
    pub dynamic_dependencies: IndexMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct HarVariantInput {
    pub artifact_type: HarArtifactType,
    pub module_root: PathBuf,
    /// `intermediates/loader_out/<target>`; obfuscated HAR copies the complete
    /// tree while ByteCode HAR copies only its `ets/` child.
    pub compiled_output_dir: PathBuf,
    pub processed_module_json: Option<PathBuf>,
    pub resource_table_path: Option<PathBuf>,
    pub native_libs: Vec<PathBuf>,
    pub native_header_dirs: Vec<PathBuf>,
    pub preload_system_so_json: Option<PathBuf>,
    pub source_maps_path: Option<PathBuf>,
    /// `HarBuildProfileTask` output. When supplied, this is the authoritative
    /// generated profile and may contain target `buildProfileFields`.
    pub build_profile_path: Option<PathBuf>,
    pub declaration_entries: Vec<String>,
    /// Values resolved from the project `parameterFile` for hvigor's
    /// `version`/dependency whitelist. The manifest writer replaces these in
    /// place so source key order remains unchanged.
    pub package_parameters: HarPackageParameterInput,
    /// Effective `buildOption.sourceOption.workers` entries for
    /// `ProcessOhPackageJson.addWorkerData`.
    pub worker_entries: Vec<PathBuf>,
    /// Effective `buildOption.arkOptions.runtimeOnly` value for
    /// `ProcessOhPackageJson.addDynamicImport`.
    pub runtime_only: Option<RuntimeOnlyConfig>,
    /// Effective target resource directories serialized by
    /// `ProcessOhPackageJson.processResourceConfig`.
    pub resource_directories: Vec<PathBuf>,
    /// `ProcessHarArtifacts.deleteOhPkgDep` result. `Some` means the runtime
    /// observed a falsy `hvigor.keepDependency` for a bundled ByteCode HAR.
    pub bundled_dependency_rewrite: Option<HarBundledDependencyInput>,
    /// Direct/all dependency versions selected by
    /// `ProcessHarArtifacts.setDependencyPkgVersion`.
    pub dependency_pkg_version: IndexMap<String, String>,
    pub bundled_dependencies: bool,
    pub bundled_resources: Vec<BundledHarResourceInput>,
    pub bundled_resource_directories: Vec<ArtifactPath>,
    pub compiled_package_names: Vec<String>,
    pub extra_importees: Vec<String>,
    pub global_dependencies: Vec<String>,
    pub ignore_resource_patterns: Vec<String>,
    pub debug: bool,
    /// Effective module target selected by hvigor's target service.
    /// Used by `HarBuildProfileTask` for `TARGET_NAME`.
    pub target_name: String,
    /// Latest hvigor materializes the complete `PackageHar` input tree below
    /// `build/<product>/cache/<target>/<target>@PackageHar` before invoking
    /// ohpm. Keeping this optional lets library callers omit build-cache state.
    pub package_cache_dir: Option<PathBuf>,
}

/// One dependency resource directory staged by latest hvigor's
/// `ProcessHarArtifacts.copyBundledDepRes`.
#[derive(Debug, Clone)]
pub struct BundledHarResourceInput {
    pub source_dir: PathBuf,
    pub archive_directory: ArtifactPath,
}

/// Inputs consumed by hvigor's `PrepareSharedHarRes` and `PackageSharedHar`
/// tasks. The resulting interface HAR is emitted beside the HSP and exposes
/// declarations, resources, and native link inputs to consumers without
/// shipping the HSP implementation sources.
#[derive(Debug, Clone)]
pub struct HspInterfaceHarInput {
    pub module_name: String,
    pub module_root: PathBuf,
    pub target_name: String,
    pub oh_package_json: PathBuf,
    pub processed_module_json: PathBuf,
    pub declarations_dir: PathBuf,
    pub resource_table_path: Option<PathBuf>,
    pub resource_str_dir: Option<PathBuf>,
    pub privacy_profile_path: Option<PathBuf>,
    pub obfuscation_rules_path: Option<PathBuf>,
    pub native_libs: Vec<PathBuf>,
    pub native_header_dirs: Vec<PathBuf>,
    pub exclude_so_from_interface_har: bool,
    pub output_path: PathBuf,
    pub compatible_sdk_version: u32,
    pub compatible_sdk_type: String,
    pub use_normalized_ohm_url: bool,
    pub integrated_hsp: Option<bool>,
    pub source_roots: Vec<String>,
    pub dependencies: BTreeMap<String, String>,
    pub dynamic_dependencies: BTreeMap<String, String>,
    pub debug: bool,
    pub obfuscated: bool,
}

/// Inputs consumed by hvigor's `PackageSharedTgz` task.
///
/// The archive contains exactly the distributable HSP (renamed without the
/// `-unsigned` marker) and the interface HAR at its root.
#[derive(Debug, Clone)]
pub struct HspTgzInput {
    pub hsp_path: PathBuf,
    pub interface_har_path: PathBuf,
    pub output_path: PathBuf,
}
