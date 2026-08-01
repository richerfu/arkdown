use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer};

use crate::error::ConfigError;

struct AppScopeLayout<'a> {
    project_root: &'a Path,
}

impl<'a> AppScopeLayout<'a> {
    fn new(project_root: &'a Path) -> Self {
        Self { project_root }
    }

    fn app_json5(&self) -> PathBuf {
        self.project_root.join("AppScope").join("app.json5")
    }
}

// ---------------------------------------------------------------------------
// Raw deserialization structs for AppScope/app.json5
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct RawAppJson5 {
    pub app: RawAppJson5App,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawAppJson5App {
    #[serde(default)]
    pub bundle_name: String,
    #[serde(default)]
    pub vendor: String,
    #[serde(default)]
    pub version_code: u64,
    #[serde(default)]
    pub version_name: String,
    #[serde(default)]
    pub build_version: Option<String>,
}

// ---------------------------------------------------------------------------
// Raw deserialization structs for build-profile.json5
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct RawBuildProfile {
    pub app: RawApp,
    pub modules: Vec<RawModule>,
}

impl RawBuildProfile {
    /// Parse `build-profile.json5` content from a string.
    pub fn parse(content: &str, file: &std::path::Path) -> Result<RawBuildProfile, ConfigError> {
        json5::from_str::<RawBuildProfile>(content).map_err(|e| ConfigError::ParseError {
            file: file.to_path_buf(),
            message: e.to_string(),
        })
    }

    /// Load and parse `build-profile.json5` from the project root.
    pub fn load(root: &std::path::Path) -> Result<RawBuildProfile, ConfigError> {
        let path: PathBuf = root.join("build-profile.json5");
        if !path.exists() {
            return Err(ConfigError::FileNotFound(path));
        }
        let content = fs_err::read_to_string(&path)?;
        Self::parse(&content, &path)
    }
}

impl RawAppJson5 {
    /// Load and parse `AppScope/app.json5` from the project root.
    pub fn load(root: &std::path::Path) -> Result<RawAppJson5, ConfigError> {
        let path = AppScopeLayout::new(root).app_json5();
        if !path.exists() {
            return Ok(RawAppJson5 {
                app: RawAppJson5App::default(),
            });
        }
        let content = fs_err::read_to_string(&path)?;
        let app: RawAppJson5 = json5::from_str(&content).map_err(|e| ConfigError::ParseError {
            file: path.clone(),
            message: e.to_string(),
        })?;
        Self::validate(&app, &path)?;
        Ok(app)
    }

    fn validate(app: &Self, path: &std::path::Path) -> Result<(), ConfigError> {
        if app.app.bundle_name.is_empty() || HvigorBundleName::new(&app.app.bundle_name).is_valid()
        {
            return Ok(());
        }

        Err(ConfigError::ValidationError {
            file: path.to_path_buf(),
            message: format!(
                "app.bundleName must match pattern \"^(?:[a-zA-Z](?:\\\\w*[0-9a-zA-Z])?)(?:\\\\.[0-9a-zA-Z](?:\\\\w*[0-9a-zA-Z])?){{2,}}$\", got {:?}",
                app.app.bundle_name
            ),
        })
    }
}

struct HvigorBundleName<'a> {
    value: &'a str,
}

impl<'a> HvigorBundleName<'a> {
    fn new(value: &'a str) -> Self {
        Self { value }
    }

    fn is_valid(&self) -> bool {
        let mut segments = self.value.split('.');
        let Some(first) = segments.next() else {
            return false;
        };
        if !HvigorBundleNameSegment::new(first).is_valid_first() {
            return false;
        }

        let mut suffix_count = 0;
        for segment in segments {
            suffix_count += 1;
            if !HvigorBundleNameSegment::new(segment).is_valid_suffix() {
                return false;
            }
        }
        suffix_count >= 2
    }
}

struct HvigorBundleNameSegment<'a> {
    value: &'a str,
}

impl<'a> HvigorBundleNameSegment<'a> {
    fn new(value: &'a str) -> Self {
        Self { value }
    }

    fn is_valid_first(&self) -> bool {
        self.is_valid(true)
    }

    fn is_valid_suffix(&self) -> bool {
        self.is_valid(false)
    }

    fn is_valid(&self, first_segment: bool) -> bool {
        let bytes = self.value.as_bytes();
        let Some((&first, rest)) = bytes.split_first() else {
            return false;
        };
        let first_ok = if first_segment {
            first.is_ascii_alphabetic()
        } else {
            first.is_ascii_alphanumeric()
        };
        if !first_ok {
            return false;
        }
        if !rest
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
        {
            return false;
        }
        bytes
            .last()
            .is_some_and(|byte| byte.is_ascii_alphanumeric())
    }
}

/// Parse the numeric API version from a `compatibleSdkVersion` string.
pub fn parse_api_version(compatible_sdk_version: &str) -> Option<u32> {
    // Find the last '(' and ')' pair and parse the contained digits.
    let open = compatible_sdk_version.rfind('(')?;
    let close = compatible_sdk_version.rfind(')')?;
    if close <= open {
        return None;
    }
    let inner = &compatible_sdk_version[open + 1..close];
    inner.parse::<u32>().ok()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawApp {
    pub products: Vec<RawProduct>,
    /// Top-level signing configurations (real OHOS format: under `app`, not per-product).
    #[serde(default)]
    pub signing_configs: Vec<RawSigningConfig>,
    #[serde(default)]
    pub build_mode_set: Vec<RawBuildMode>,
    /// Real OHOS projects configure `multiProjects` under build-profile `app`.
    /// Hvigor ignores the same field when it is written in AppScope/app.json5.
    #[serde(default)]
    pub multi_projects: bool,
}

/// Top-level signing config entry (under `app.signingConfigs` in real OHOS projects).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawSigningConfig {
    pub name: String,
    #[serde(rename = "type", default)]
    pub config_type: String,
    pub material: Option<RawSigningMaterial>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawProduct {
    pub name: String,
    /// Product-level app profile override.
    ///
    /// Hvigor resolves `hvigorfile appOpt` → product → `AppScope/app.json5`.
    #[serde(default)]
    pub bundle_name: String,
    #[serde(default)]
    pub vendor: Option<String>,
    #[serde(default)]
    pub bundle_type: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub compatible_sdk_version: String,
    #[serde(default)]
    pub compatible_sdk_version_stage: Option<String>,
    /// Real OHOS projects use targetSdkVersion; takes priority over compatibleSdkVersion.
    #[serde(default)]
    pub target_sdk_version: Option<String>,
    #[serde(rename = "runtimeOS", alias = "runtimeOs", default)]
    pub runtime_os: String,
    /// Reference to a signing config by name (looks up `app.signingConfigs`).
    #[serde(default)]
    pub signing_config: Option<String>,
    /// Kept for backward compatibility (old format embedded signingConfigs in products).
    #[serde(default)]
    pub signing_configs: Vec<RawSigningConfigEntry>,
    #[serde(default)]
    pub build_option: Option<RawBuildOption>,
    /// Optional product-level `versionCode` override.
    ///
    /// When set, overrides the `versionCode` in `AppScope/app.json5` for this
    /// product.  Matches hvigor's product-level app override semantics.
    /// Real OHOS projects usually leave this absent and use `app.json5`.
    #[serde(default)]
    pub version_code: Option<u64>,
    /// Optional product-level `versionName` override.
    ///
    /// When set, overrides the `versionName` in `AppScope/app.json5` for this
    /// product.  Matches hvigor's product-level app override semantics.
    #[serde(default)]
    pub version_name: Option<String>,
    /// Optional product-level `buildVersion`.
    ///
    /// Hvigor gives this precedence over `AppScope/app.json5`, while the
    /// build invocation's extra config may override both.
    #[serde(default)]
    pub build_version: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RawBuildMode {
    pub name: String,
}

/// Per-product signing config entry (old / backward-compat format).
#[derive(Debug, Deserialize)]
pub struct RawSigningConfigEntry {
    pub name: String,
    pub material: Option<RawSigningMaterial>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawSigningMaterial {
    /// Real OHOS projects write `certpath` (lowercase p); old format uses `certPath` (camelCase).
    #[serde(alias = "certpath", default)]
    pub cert_path: String,
    #[serde(default)]
    pub key_alias: String,
    #[serde(default)]
    pub key_password: String,
    #[serde(default)]
    pub profile: String,
    #[serde(default)]
    pub sign_alg: String,
    #[serde(default)]
    pub store_file: String,
    #[serde(default)]
    pub store_password: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawBuildOption {
    /// Optional name when the same shape is used inside module `buildOptionSet`.
    #[serde(default)]
    pub name: Option<String>,
    /// Optional copyFrom when the same shape is used inside module `buildOptionSet`.
    #[serde(default)]
    pub copy_from: Option<String>,
    /// HAR representation: original, obfuscation, or bytecode.
    #[serde(default)]
    pub artifact_type: Option<String>,
    #[serde(default)]
    pub strict_mode: Option<RawStrictMode>,
    #[serde(default)]
    pub ark_options: Option<RawArkOptions>,
    #[serde(default)]
    pub source_option: Option<RawSourceOption>,
    /// Whether `buildOption.preloadSystemSo` enables system SO preload handling.
    #[serde(default)]
    pub preload_system_so: bool,
    /// SO file paths listed under `buildOption.preloadSo` in `build-profile.json5`.
    #[serde(default)]
    pub preload_so: Vec<std::path::PathBuf>,
    /// HSP/source-HAR deduplication policy.
    #[serde(rename = "packOptions", default)]
    pub pack_options: Option<RawPackOptions>,
    /// HAR compiler/packing options from `buildOption.packingOptions`.
    #[serde(default)]
    pub packing_options: Option<RawPackingOptions>,
    /// Resource options from `buildOption.resOptions` in `build-profile.json5`.
    #[serde(rename = "resOptions", default)]
    pub res_options: Option<RawResOptions>,
    /// Permissions to remove from HAR/HSP dependency `requestPermissions` lists.
    /// Matches hvigor's `MergeProfile.getRemovePermissionSet()` behavior.
    #[serde(
        rename = "removePermissions",
        default,
        deserialize_with = "deserialize_remove_permissions"
    )]
    pub remove_permissions: Vec<String>,
    /// AOT compile mode: `""` | `"type"` | `"partial"`. API 10+ ignores
    /// this field and derives AOT mode from `arkOptions.hostPGO`.
    #[serde(rename = "aotCompileMode", default)]
    pub aot_compile_mode: Option<String>,
    /// Path to AOT profile data (`.ap` file) for PGO-based AOT compilation.
    #[serde(rename = "apPath", default)]
    pub ap_path: Option<std::path::PathBuf>,
    /// Native build options including ABI filters.
    /// `buildOption.externalNativeOptions.abiFilters` controls which CPU
    /// architectures' SO libraries are collected and included in the package.
    #[serde(rename = "externalNativeOptions", default)]
    pub external_native_options: Option<RawExternalNativeOptions>,
    /// Legacy native filter entry merged before `nativeLib.filter` by hvigor.
    #[serde(rename = "napiLibFilterOption", default)]
    pub napi_lib_filter_option: Option<RawNativeLibFilterOption>,
    /// Native library packaging options.
    #[serde(rename = "nativeLib", default)]
    pub native_lib: Option<RawNativeLibOption>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawPackOptions {
    #[serde(default)]
    pub deduplicate_har: bool,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawPackingOptions {
    #[serde(default)]
    pub customized_options: Option<RawCustomizedPackingOptions>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawCustomizedPackingOptions {
    #[serde(default)]
    pub base_package: Option<std::path::PathBuf>,
}

/// `buildOption.externalNativeOptions` — controls native/CMake build settings.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawExternalNativeOptions {
    /// CMake script path. Hvigor requires the basename to be `CMakeLists.txt`.
    #[serde(default)]
    pub path: Option<std::path::PathBuf>,
    #[serde(default)]
    pub arguments: Option<StringList>,
    /// ABI filter list (e.g. `["arm64-v8a", "armeabi-v7a", "x86_64"]`).
    /// When empty, defaults to `["arm64-v8a"]` (hvigor default).
    #[serde(default)]
    pub abi_filters: Vec<String>,
    #[serde(default)]
    pub cpp_flags: Option<String>,
    #[serde(default)]
    pub c_flags: Option<String>,
    #[serde(default)]
    pub targets: Option<Vec<String>>,
}

/// `buildOption.nativeLib` — controls hvigor ProcessLibs collection/filtering.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawNativeLibOption {
    /// When true hvigor scans `**/*` instead of `**/*.so*(.*[0-9])`.
    #[serde(default)]
    pub collect_all_libs: Option<bool>,
    /// `nativeLib.filter`, merged over `napiLibFilterOption`.
    #[serde(default)]
    pub filter: Option<RawNativeLibFilterOption>,
    #[serde(default)]
    pub header_path: Option<StringList>,
    #[serde(default)]
    pub exclude_from_har: Option<bool>,
    #[serde(default)]
    pub exclude_so_from_interface_har: Option<bool>,
    #[serde(rename = "excludeSoFromBinXO", alias = "excludeSoFromBinxo", default)]
    pub exclude_so_from_binxo: Option<StringList>,
    #[serde(default)]
    pub debug_symbol: Option<RawNativeDebugSymbolOption>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawNativeDebugSymbolOption {
    #[serde(default)]
    pub strip: Option<bool>,
    #[serde(default)]
    pub exclude: Option<StringList>,
}

/// `NapiLibFilterOpt` from hvigor `options/build/build-opt.d.ts`.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawNativeLibFilterOption {
    #[serde(default)]
    pub excludes: Option<Vec<String>>,
    #[serde(default)]
    pub pick_firsts: Option<Vec<String>>,
    #[serde(default)]
    pub pick_lasts: Option<Vec<String>>,
    #[serde(default)]
    pub enable_override: Option<bool>,
    #[serde(default)]
    pub select: Option<Vec<RawNativeLibPackageSelection>>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawNativeLibPackageSelection {
    #[serde(default)]
    pub package: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub include: Option<Vec<String>>,
    #[serde(default)]
    pub exclude: Option<Vec<String>>,
    #[serde(default)]
    pub include_pattern: Option<Vec<String>>,
    #[serde(default)]
    pub exclude_pattern: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RawModuleBuildProfile {
    #[serde(default)]
    build_option: Option<RawBuildOption>,
    #[serde(default)]
    build_option_set: Vec<RawBuildOption>,
    #[serde(default)]
    build_mode_binder: Vec<RawBuildModeBinder>,
    #[serde(default)]
    targets: Vec<RawTarget>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RawBuildModeBinder {
    #[serde(default)]
    build_mode_name: String,
    #[serde(default)]
    mappings: Vec<RawBuildModeBinderMapping>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RawBuildModeBinderMapping {
    #[serde(default)]
    target_name: String,
    #[serde(default)]
    build_option_name: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawResOptions {
    /// Whether to include AppScope/resources in resource compilation.
    #[serde(rename = "includeAppScopeRes", default = "default_true")]
    pub include_app_scope_res: bool,
    /// Additional resource directories to include.
    #[serde(default)]
    pub directories: Vec<std::path::PathBuf>,
    /// Resource compression configuration forwarded to opt-compression.json.
    #[serde(default)]
    pub compression: Option<serde_json::Value>,
    /// Static files copied from the ArkTS source roots into loader_out.
    #[serde(default)]
    pub copy_code_resource: RawCopyCodeResource,
    /// Maximum resource compiler worker count.
    #[serde(default)]
    pub res_compile_threads: Option<usize>,
    /// Resource globs ignored by restool.
    #[serde(default)]
    pub ignore_resource_pattern: Vec<String>,
    /// Resource qualifier restrictions; not supported for HAR by hvigor.
    #[serde(default)]
    pub qualifiers_config: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawCopyCodeResource {
    #[serde(default = "default_true")]
    pub enable: bool,
    #[serde(default)]
    pub excludes: Vec<String>,
    #[serde(default)]
    pub includes: Option<Vec<String>>,
}

impl Default for RawCopyCodeResource {
    fn default() -> Self {
        Self {
            enable: true,
            excludes: Vec::new(),
            includes: None,
        }
    }
}

fn default_true() -> bool {
    true
}

fn deserialize_remove_permissions<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let values = Vec::<serde_json::Value>::deserialize(deserializer)?;
    Ok(values
        .into_iter()
        .filter_map(|value| match value {
            serde_json::Value::String(name) => (!name.is_empty()).then_some(name),
            serde_json::Value::Object(object) => object
                .get("name")
                .and_then(|value| value.as_str())
                .filter(|name| !name.is_empty())
                .map(str::to_string),
            _ => None,
        })
        .collect())
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawSourceOption {
    #[serde(default)]
    pub workers: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct RawStrictMode {
    #[serde(rename = "caseSensitiveCheck", default)]
    pub case_sensitive_check: bool,
    /// JSON key is `useNormalizedOHMUrl` (all-caps OHM — not camelCase).
    #[serde(rename = "useNormalizedOHMUrl", default)]
    pub use_normalized_ohm_url: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawArkOptions {
    #[serde(default)]
    pub obfuscation: Option<RawObfuscation>,
    #[serde(default)]
    pub types: Vec<String>,
    #[serde(default)]
    pub runtime_only: Option<RawRuntimeOnly>,
    #[serde(default)]
    pub build_profile_fields: Option<serde_json::Map<String, serde_json::Value>>,
    #[serde(default)]
    pub support_chunks: Option<bool>,
    #[serde(default)]
    pub byte_code_har: Option<bool>,
    #[serde(default)]
    pub bundled_dependencies: Option<bool>,
    #[serde(default)]
    pub integrated_hsp: Option<bool>,
    #[serde(rename = "aotCompileMode", default)]
    pub aot_compile_mode: Option<String>,
    #[serde(rename = "apPath", default)]
    pub ap_path: Option<std::path::PathBuf>,
    #[serde(rename = "hostPGO", default)]
    pub host_pgo: Option<bool>,
}

pub(crate) struct BuildOptionDecoder<'a> {
    raw: Option<&'a RawBuildOption>,
    path_root: &'a Path,
    profile_path: &'a Path,
}

impl<'a> BuildOptionDecoder<'a> {
    pub(crate) fn new(
        raw: Option<&'a RawBuildOption>,
        path_root: &'a Path,
        profile_path: &'a Path,
    ) -> Self {
        Self {
            raw,
            path_root,
            profile_path,
        }
    }

    pub(crate) fn decode(&self) -> Result<arkdown_core::BuildOption, ConfigError> {
        let strict_mode = self
            .raw
            .and_then(|option| option.strict_mode.as_ref())
            .map(|strict| arkdown_core::StrictModeConfig {
                case_sensitive_check: strict.case_sensitive_check,
                use_normalized_ohm_url: strict.use_normalized_ohm_url,
            })
            .unwrap_or(arkdown_core::StrictModeConfig {
                case_sensitive_check: false,
                use_normalized_ohm_url: false,
            });
        let raw_ark_options = self.raw.and_then(|option| option.ark_options.as_ref());
        let ark_options = raw_ark_options.map(|options| arkdown_core::ArkOptions {
            obfuscation: options
                .obfuscation
                .as_ref()
                .map(|obfuscation| self.decode_obfuscation(obfuscation)),
            types: options.types.clone(),
            runtime_only: options.runtime_only.as_ref().map(|runtime_only| {
                arkdown_core::RuntimeOnlyConfig {
                    packages: runtime_only.packages.clone(),
                    sources: runtime_only.sources.clone(),
                    exclude_packages: runtime_only.exclude_packages.clone(),
                }
            }),
            build_profile_fields: options.build_profile_fields.clone().unwrap_or_default(),
            support_chunks: options.support_chunks.unwrap_or(false),
            byte_code_har: options.byte_code_har.unwrap_or(false),
            bundled_dependencies: options.bundled_dependencies.unwrap_or(false),
            integrated_hsp: options.integrated_hsp,
        });
        let source_option = self
            .raw
            .and_then(|option| option.source_option.as_ref())
            .map(|source| arkdown_core::SourceOption {
                source_language: None,
                workers: source
                    .workers
                    .iter()
                    .map(|worker| self.resolve_path(Path::new(worker)))
                    .collect(),
            });
        let har_artifact_type =
            if raw_ark_options.and_then(|options| options.byte_code_har) == Some(true) {
                arkdown_core::HarArtifactType::Bytecode
            } else {
                HarArtifactTypeParser::new(
                    self.raw
                        .and_then(|option| option.artifact_type.as_deref())
                        .unwrap_or("original"),
                    self.profile_path,
                )
                .parse()?
            };
        let mut native_build_option = resolve_product_native_build_option(self.raw);
        native_build_option.normalize_module_paths(self.path_root);
        let res_options =
            self.raw
                .and_then(|option| option.res_options.as_ref())
                .map(|resources| arkdown_core::ResOptions {
                    res_path: None,
                    include_app_scope_res: resources.include_app_scope_res,
                    resource_directories: resources
                        .directories
                        .iter()
                        .map(|path| self.resolve_path(path))
                        .collect(),
                    compression: resources.compression.clone(),
                    copy_code_resource: arkdown_core::CopyCodeResource {
                        enable: resources.copy_code_resource.enable,
                        excludes: resources.copy_code_resource.excludes.clone(),
                        includes: resources.copy_code_resource.includes.clone(),
                    },
                    res_compile_threads: resources.res_compile_threads,
                    ignore_resource_pattern: resources.ignore_resource_pattern.clone(),
                    qualifiers_config: resources.qualifiers_config.clone(),
                });
        let preload_so = self
            .raw
            .map(|option| {
                option
                    .preload_so
                    .iter()
                    .map(|path| self.resolve_path(path))
                    .collect()
            })
            .unwrap_or_default();
        let ap_path = raw_ark_options
            .and_then(|options| options.ap_path.as_ref())
            .or_else(|| self.raw.and_then(|option| option.ap_path.as_ref()))
            .map(|path| self.resolve_path(path));
        let customized_har_base_package = self
            .raw
            .and_then(|option| option.packing_options.as_ref())
            .and_then(|options| options.customized_options.as_ref())
            .and_then(|options| options.base_package.as_ref())
            .filter(|path| !path.as_os_str().is_empty())
            .map(|path| self.resolve_path(path));

        Ok(arkdown_core::BuildOption {
            strict_mode,
            ark_options,
            res_options,
            source_option,
            preload_system_so: self.raw.is_some_and(|option| option.preload_system_so),
            preload_so,
            deduplicate_har: self
                .raw
                .and_then(|option| option.pack_options.as_ref())
                .is_some_and(|options| options.deduplicate_har),
            customized_har_base_package,
            remove_permissions: self
                .raw
                .map(|option| option.remove_permissions.clone())
                .unwrap_or_default(),
            aot_compile_mode: raw_ark_options
                .and_then(|options| options.aot_compile_mode.clone())
                .or_else(|| self.raw.and_then(|option| option.aot_compile_mode.clone())),
            host_pgo: raw_ark_options.and_then(|options| options.host_pgo),
            ap_path,
            abi_filters: native_build_option.abi_filters,
            external_native_options: native_build_option.external_native_options,
            har_artifact_type,
            native_lib: native_build_option.native_lib,
        })
    }

    fn decode_obfuscation(&self, raw: &RawObfuscation) -> arkdown_core::ObfuscationConfig {
        let resolve_paths = |paths: &[String]| {
            paths
                .iter()
                .map(|path| self.resolve_path(Path::new(path)))
                .collect()
        };
        let rule_files = raw
            .rule_options
            .as_ref()
            .map(|options| {
                if options.rules.is_empty() {
                    options
                        .files
                        .as_ref()
                        .map(|files| resolve_paths(files.as_slice()))
                        .unwrap_or_default()
                } else {
                    resolve_paths(&options.rules)
                }
            })
            .unwrap_or_default();
        let consumer_rule_files = if raw.consumer_rules.is_empty() {
            raw.consumer_files
                .as_ref()
                .map(|files| resolve_paths(files.as_slice()))
                .unwrap_or_default()
        } else {
            resolve_paths(&raw.consumer_rules)
        };
        arkdown_core::ObfuscationConfig {
            enable_obfuscation: raw
                .rule_options
                .as_ref()
                .is_some_and(|options| options.enable),
            rule_files,
            consumer_rule_files,
        }
    }

    fn resolve_path(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.path_root.join(path)
        }
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawRuntimeOnly {
    #[serde(default)]
    pub packages: Vec<String>,
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub exclude_packages: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawObfuscation {
    pub rule_options: Option<RawRuleOptions>,
    #[serde(default)]
    pub consumer_files: Option<StringList>,
    #[serde(default)]
    pub consumer_rules: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct RawRuleOptions {
    #[serde(default)]
    pub enable: bool,
    #[serde(default)]
    pub files: Option<StringList>,
    /// Paths to rule files (relative to the project root), e.g.
    /// `["./obfuscation-rules.txt"]`.
    #[serde(default)]
    pub rules: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum StringList {
    One(String),
    Many(Vec<String>),
}

impl StringList {
    pub fn as_slice(&self) -> &[String] {
        match self {
            Self::One(value) => std::slice::from_ref(value),
            Self::Many(values) => values.as_slice(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawModule {
    pub name: String,
    pub src_path: String,
    #[serde(default)]
    pub targets: Vec<RawTarget>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawTarget {
    pub name: String,
    /// Products this target applies to.  Empty means the target applies only to
    /// the product named `"default"` (hvigor's default-product fallback).
    #[serde(default)]
    pub apply_to_products: Vec<String>,
    #[serde(default)]
    pub config: Option<RawTargetConfig>,
    /// Target-specific resource directories, relative to the module root.
    #[serde(default)]
    pub resource: Option<RawTargetResource>,
    /// Source-set overlay: abilities with per-target icon/label/launchType overrides.
    #[serde(default)]
    pub source: Option<RawTargetSource>,
    /// Optional build-mode filter.  When present this target is only selected
    /// for builds whose mode appears in this list.  Empty list means the target
    /// is active for all build modes (hvigor's default).
    ///
    /// Example `build-profile.json5` fragment:
    /// ```json5
    /// targets: [{
    ///   name: "release-target",
    ///   applyToProducts: ["default"],
    ///   buildModeSet: [{ name: "release" }],
    ///   resource: { directories: ["src/release/resources"] }
    /// }]
    /// ```
    #[serde(default)]
    pub build_mode_set: Vec<RawBuildMode>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawTargetResource {
    #[serde(default)]
    pub directories: Vec<PathBuf>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawTargetConfig {
    #[serde(default)]
    pub distro_filter: Option<serde_json::Value>,
    #[serde(default)]
    pub distribution_filter: Option<serde_json::Value>,
    /// Target-local build option; hvigor merges this over module/product
    /// build options for the target.
    #[serde(rename = "buildOption", default)]
    pub build_option: Option<RawBuildOption>,
}

/// Source overlay for a target, allowing per-target ability configuration.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawTargetSource {
    /// Additional source roots copied into interface-HAR metadata before the
    /// module's standard and generated roots.
    #[serde(default)]
    pub source_roots: Vec<String>,
    /// Per-target ability overrides (icon, label, launchType).
    #[serde(default)]
    pub abilities: Vec<RawTargetAbility>,
}

/// Per-target ability configuration override.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawTargetAbility {
    pub name: String,
    pub icon: Option<String>,
    pub label: Option<String>,
    #[serde(default)]
    pub launch_type: Option<String>,
}

// ---------------------------------------------------------------------------
// Effective target resolution
// ---------------------------------------------------------------------------

/// The resolved effective target for a specific module/product/build-mode
/// combination.
///
/// hvigor resolves one "effective target" per module by selecting the first
/// target in declaration order that matches both `applyToProducts` and
/// `buildModeSet` constraints. This struct carries the resolved configuration
/// that downstream pipeline stages need.
#[derive(Debug, Clone, Default)]
pub struct EffectiveTargetConfig {
    /// Target name, e.g. `"default"` or `"phone"`.
    pub name: String,
    /// Target-specific resource directories (from `target.resource.directories`).
    /// Empty when the target has no resource override.
    pub resource_directories: Vec<PathBuf>,
    /// Source ability overrides for this target.
    /// Empty when the target has no `source.abilities`.
    pub source_abilities: Vec<RawTargetAbility>,
    /// Target-specific source roots, preserved as manifest-relative strings.
    pub source_roots: Vec<String>,
    /// Distribution/distro filter configuration from `target.config`.
    /// `None` when the target has no filter configured.
    pub distribution_filter: Option<serde_json::Value>,
    /// Build modes this target applies to; empty means all modes.
    pub build_mode_set: Vec<String>,
}

impl EffectiveTargetConfig {
    fn from_raw(target: &RawTarget) -> Self {
        Self {
            name: target.name.clone(),
            resource_directories: target
                .resource
                .as_ref()
                .map(|resource| resource.directories.clone())
                .unwrap_or_default(),
            source_abilities: target
                .source
                .as_ref()
                .map(|source| source.abilities.clone())
                .unwrap_or_default(),
            source_roots: target
                .source
                .as_ref()
                .map(|source| source.source_roots.clone())
                .unwrap_or_default(),
            distribution_filter: target.config.as_ref().and_then(|config| {
                config
                    .distribution_filter
                    .clone()
                    .or_else(|| config.distro_filter.clone())
            }),
            build_mode_set: target
                .build_mode_set
                .iter()
                .map(|mode| mode.name.clone())
                .collect(),
        }
    }
}

/// Immutable project/module build-profile inventory loaded during build-session
/// startup.
///
/// hvigor parses the project and module models once, then every stage consumes
/// the same resolved model. Keeping both the typed project target model and the
/// lossless JSON build-option model here prevents stage-local file reads and
/// target-selection drift.
#[derive(Debug)]
pub struct BuildOptionProfileInventory {
    project_root: PathBuf,
    project_path: PathBuf,
    project: BuildProfileValue,
    raw_project: RawBuildProfile,
    modules: BTreeMap<String, ModuleBuildProfileSnapshot>,
}

#[derive(Debug)]
struct ModuleBuildProfileSnapshot {
    root: PathBuf,
    path: PathBuf,
    profile: Option<BuildProfileValue>,
}

impl BuildOptionProfileInventory {
    /// Load project and module build profiles exactly once.
    pub fn load(
        project_root: &Path,
        modules: &[arkdown_core::ModuleConfig],
    ) -> Result<Self, ConfigError> {
        let project_path = project_root.join("build-profile.json5");
        if !project_path.is_file() {
            return Err(ConfigError::FileNotFound(project_path));
        }
        let project_content = fs_err::read_to_string(&project_path)?;
        let project = BuildProfileValue::parse(&project_content, &project_path)?;
        let raw_project = RawBuildProfile::parse(&project_content, &project_path)?;
        let mut module_profiles = BTreeMap::new();
        for module in modules {
            let path = module.src_path.join("build-profile.json5");
            let profile = if path.is_file() {
                let content = fs_err::read_to_string(&path)?;
                Some(BuildProfileValue::parse(&content, &path)?)
            } else {
                None
            };
            module_profiles.insert(
                module.name.clone(),
                ModuleBuildProfileSnapshot {
                    root: module.src_path.clone(),
                    path,
                    profile,
                },
            );
        }
        Ok(Self {
            project_root: project_root.to_path_buf(),
            project_path,
            project,
            raw_project,
            modules: module_profiles,
        })
    }

    /// Return all applicable module targets in declaration order.
    pub fn applicable_targets(
        &self,
        module_name: &str,
        product_name: &str,
        build_mode_name: &str,
    ) -> Vec<EffectiveTargetConfig> {
        let Some(module) = self
            .raw_project
            .modules
            .iter()
            .find(|module| module.name == module_name)
        else {
            return Vec::new();
        };
        enumerate_applicable_raw_targets(&module.targets, product_name, build_mode_name)
            .into_iter()
            .map(EffectiveTargetConfig::from_raw)
            .collect()
    }

    /// Select the effective target using hvigor's target precedence.
    pub fn effective_target(
        &self,
        module_name: &str,
        product_name: &str,
        build_mode_name: &str,
    ) -> Option<EffectiveTargetConfig> {
        let module = self
            .raw_project
            .modules
            .iter()
            .find(|module| module.name == module_name)?;
        pick_effective_raw_target(&module.targets, product_name, build_mode_name)
            .map(EffectiveTargetConfig::from_raw)
    }

    fn module(&self, module_name: &str) -> Result<&ModuleBuildProfileSnapshot, ConfigError> {
        self.modules
            .get(module_name)
            .ok_or_else(|| ConfigError::ValidationError {
                file: self.project_path.clone(),
                message: format!(
                    "module '{module_name}' is missing from the build-profile inventory"
                ),
            })
    }
}

/// Resolves the complete hvigor build option for one module target.
///
/// Upstream correspondence:
/// - `BuildModeManager.initProjectBuildOptions`
/// - `BuildModeManager.initModuleBuildOptionMap`
/// - `BuildOptionCombiner.mergeBuildOption`
pub struct EffectiveModuleBuildOptionResolver<'a> {
    inventory: &'a BuildOptionProfileInventory,
    module_name: &'a str,
    product_name: &'a str,
    build_mode_name: &'a str,
    target_name: &'a str,
    build_script_override: Option<serde_json::Value>,
}

impl<'a> EffectiveModuleBuildOptionResolver<'a> {
    pub fn new(
        inventory: &'a BuildOptionProfileInventory,
        module_name: &'a str,
        product_name: &'a str,
        build_mode_name: &'a str,
        target_name: &'a str,
    ) -> Self {
        Self {
            inventory,
            module_name,
            product_name,
            build_mode_name,
            target_name,
            build_script_override: None,
        }
    }

    /// Apply statically extracted `hvigorfile.ts` overrides at hvigor's
    /// highest-precedence layer.
    pub fn with_build_script_override(mut self, override_value: serde_json::Value) -> Self {
        self.build_script_override = Some(override_value);
        self
    }

    pub fn resolve(&self) -> Result<arkdown_core::BuildOption, ConfigError> {
        let module_snapshot = self.inventory.module(self.module_name)?;
        let project = &self.inventory.project;
        let module = module_snapshot.profile.as_ref();
        let mut effective = BuildOptionValue::default();

        effective.merge_layer(
            project.project_build_mode_option(self.build_mode_name),
            &self.inventory.project_root,
        );
        effective.merge_layer(
            project.product_build_option(self.product_name),
            &self.inventory.project_root,
        );
        if let Some(module) = module {
            effective.merge_layer(module.module_build_option(), &module_snapshot.root);
            if let Some(option_set) = module.selected_build_option(
                self.build_mode_name,
                self.target_name,
                &module_snapshot.path,
            )? {
                effective.merge_layer(Some(&option_set), &module_snapshot.root);
            }
            effective.merge_layer(
                module.target_build_option(self.target_name),
                &module_snapshot.root,
            );
        }
        effective.merge_layer(self.build_script_override.as_ref(), &module_snapshot.root);

        let raw = effective.deserialize(&module_snapshot.path)?;
        BuildOptionDecoder::new(Some(&raw), &module_snapshot.root, &module_snapshot.path).decode()
    }
}

#[derive(Debug)]
struct BuildProfileValue {
    value: serde_json::Value,
}

impl BuildProfileValue {
    fn parse(content: &str, path: &Path) -> Result<Self, ConfigError> {
        let value = json5::from_str::<serde_json::Value>(content).map_err(|error| {
            ConfigError::ParseError {
                file: path.to_path_buf(),
                message: error.to_string(),
            }
        })?;
        Ok(Self { value })
    }

    fn project_build_mode_option(&self, build_mode_name: &str) -> Option<&serde_json::Value> {
        self.value
            .pointer("/app/buildModeSet")
            .and_then(serde_json::Value::as_array)?
            .iter()
            .rev()
            .find(|entry| {
                entry.get("name").and_then(serde_json::Value::as_str) == Some(build_mode_name)
            })
            .and_then(|entry| entry.get("buildOption"))
    }

    fn product_build_option(&self, product_name: &str) -> Option<&serde_json::Value> {
        self.value
            .pointer("/app/products")
            .and_then(serde_json::Value::as_array)?
            .iter()
            .rev()
            .find(|entry| {
                entry.get("name").and_then(serde_json::Value::as_str) == Some(product_name)
            })
            .and_then(|entry| entry.get("buildOption"))
    }

    fn module_build_option(&self) -> Option<&serde_json::Value> {
        self.value.get("buildOption")
    }

    fn target_build_option(&self, target_name: &str) -> Option<&serde_json::Value> {
        let target = self
            .value
            .get("targets")
            .and_then(serde_json::Value::as_array)?
            .iter()
            .rev()
            .find(|entry| {
                entry.get("name").and_then(serde_json::Value::as_str) == Some(target_name)
            })?;
        target
            .pointer("/config/buildOption")
            .or_else(|| target.get("buildOption"))
    }

    fn selected_build_option(
        &self,
        build_mode_name: &str,
        target_name: &str,
        file: &Path,
    ) -> Result<Option<serde_json::Value>, ConfigError> {
        let name = self.build_option_name(build_mode_name, target_name);
        self.resolve_build_option_set(&name, file, &mut Vec::new())
    }

    fn build_option_name(&self, build_mode_name: &str, target_name: &str) -> String {
        if let Some(binders) = self
            .value
            .get("buildModeBinder")
            .and_then(serde_json::Value::as_array)
        {
            for binder in binders.iter().rev() {
                if binder
                    .get("buildModeName")
                    .and_then(serde_json::Value::as_str)
                    != Some(build_mode_name)
                {
                    continue;
                }
                if let Some(mapping) = binder
                    .get("mappings")
                    .and_then(serde_json::Value::as_array)
                    .and_then(|mappings| {
                        mappings.iter().rev().find(|mapping| {
                            mapping
                                .get("targetName")
                                .and_then(serde_json::Value::as_str)
                                == Some(target_name)
                        })
                    })
                {
                    if let Some(name) = mapping
                        .get("buildOptionName")
                        .and_then(serde_json::Value::as_str)
                        .filter(|name| !name.is_empty())
                    {
                        return name.to_string();
                    }
                }
            }
        }
        match build_mode_name {
            "debug" | "test" => "debug",
            "release" => "release",
            _ => "default",
        }
        .to_string()
    }

    fn resolve_build_option_set(
        &self,
        name: &str,
        file: &Path,
        stack: &mut Vec<String>,
    ) -> Result<Option<serde_json::Value>, ConfigError> {
        let Some(option) = self
            .value
            .get("buildOptionSet")
            .and_then(serde_json::Value::as_array)
            .and_then(|options| {
                options.iter().rev().find(|option| {
                    option.get("name").and_then(serde_json::Value::as_str) == Some(name)
                })
            })
        else {
            return Ok(None);
        };
        if stack.iter().any(|visited| visited == name) {
            return Err(ConfigError::ValidationError {
                file: file.to_path_buf(),
                message: format!(
                    "circular buildOptionSet copyFrom chain: {} -> {name}",
                    stack.join(" -> ")
                ),
            });
        }
        stack.push(name.to_string());
        let parent =
            if let Some(copy_from) = option.get("copyFrom").and_then(serde_json::Value::as_str) {
                self.resolve_build_option_set(copy_from, file, stack)?
            } else {
                None
            };
        stack.pop();
        let empty = serde_json::Value::Object(serde_json::Map::new());
        Ok(Some(BuildOptionValue::merge_values(
            parent.as_ref().unwrap_or(&empty),
            option,
            "",
        )))
    }
}

#[derive(Default)]
struct BuildOptionValue {
    value: serde_json::Value,
}

impl BuildOptionValue {
    const MERGED_ARRAY_PATH: &'static str = "resOptions.compression.filters";

    fn merge_layer(&mut self, layer: Option<&serde_json::Value>, path_root: &Path) {
        let Some(layer) = layer else {
            return;
        };
        let mut normalized = layer.clone();
        Self::normalize_paths(&mut normalized, path_root);
        self.value = Self::merge_values(&self.value, &normalized, "");
    }

    fn deserialize(&self, file: &Path) -> Result<RawBuildOption, ConfigError> {
        serde_json::from_value(self.value.clone()).map_err(|error| ConfigError::ParseError {
            file: file.to_path_buf(),
            message: error.to_string(),
        })
    }

    fn merge_values(
        lower: &serde_json::Value,
        higher: &serde_json::Value,
        path: &str,
    ) -> serde_json::Value {
        match (lower, higher) {
            (serde_json::Value::Object(lower), serde_json::Value::Object(higher)) => {
                let mut merged = lower.clone();
                for (key, higher_value) in higher {
                    let child_path = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{path}.{key}")
                    };
                    let value = merged.get(key).map_or_else(
                        || higher_value.clone(),
                        |lower_value| Self::merge_values(lower_value, higher_value, &child_path),
                    );
                    merged.insert(key.clone(), value);
                }
                serde_json::Value::Object(merged)
            }
            (serde_json::Value::Array(lower), serde_json::Value::Array(higher))
                if path == Self::MERGED_ARRAY_PATH =>
            {
                serde_json::Value::Array(
                    higher
                        .iter()
                        .cloned()
                        .chain(lower.iter().cloned())
                        .collect(),
                )
            }
            _ => higher.clone(),
        }
    }

    fn normalize_paths(value: &mut serde_json::Value, root: &Path) {
        for pointer in [
            "/apPath",
            "/arkOptions/apPath",
            "/externalNativeOptions/path",
            "/preloadSo",
            "/sourceOption/workers",
            "/resOptions/directories",
            "/nativeLib/headerPath",
            "/arkOptions/obfuscation/ruleOptions/files",
            "/arkOptions/obfuscation/ruleOptions/rules",
            "/arkOptions/obfuscation/consumerFiles",
            "/arkOptions/obfuscation/consumerRules",
        ] {
            if let Some(path_value) = value.pointer_mut(pointer) {
                Self::normalize_path_value(path_value, root);
            }
        }
        if let Some(filters) = value
            .pointer_mut("/resOptions/compression/filters")
            .and_then(serde_json::Value::as_array_mut)
        {
            for filter in filters {
                for pointer in ["/files/path", "/exclude/path"] {
                    if let Some(path_value) = filter.pointer_mut(pointer) {
                        Self::normalize_path_value(path_value, root);
                    }
                }
            }
        }
    }

    fn normalize_path_value(value: &mut serde_json::Value, root: &Path) {
        match value {
            serde_json::Value::String(path) => {
                let candidate = Path::new(path);
                if candidate.is_relative() {
                    *path = root.join(candidate).to_string_lossy().into_owned();
                }
            }
            serde_json::Value::Array(paths) => {
                for path in paths {
                    Self::normalize_path_value(path, root);
                }
            }
            _ => {}
        }
    }
}

/// Select the effective target for `module_name` given the current
/// `product_name` and `build_mode_name`.
///
/// Reads `build-profile.json5` from `project_root` and returns the resolved
/// [`EffectiveTargetConfig`].  Returns `None` when:
/// - `build-profile.json5` cannot be read.
/// - The module is not declared in the profile.
/// - No target at all is declared for the module.
///
/// Priority order (matches hvigor):
/// 1. Named `"default"` + matches product + matches build mode.
/// 2. Any target matching product + build mode.
/// 3. Product match without build-mode constraint.
/// 4. Bare `"default"` target.
pub fn select_effective_target(
    project_root: &Path,
    module_name: &str,
    product_name: &str,
    build_mode_name: &str,
) -> Option<EffectiveTargetConfig> {
    let profile = RawBuildProfile::load(project_root).ok()?;
    let raw_module = profile.modules.iter().find(|m| m.name == module_name)?;
    let target = pick_effective_raw_target(&raw_module.targets, product_name, build_mode_name)?;

    Some(EffectiveTargetConfig::from_raw(target))
}

/// Pick the first [`RawTarget`] matching the given product and build mode.
///
/// Priority (matches hvigor's effective-target selection):
/// 1. Target with a SPECIFIC `buildModeSet` (non-empty) that includes the current
///    mode AND matches the product.  This allows build-mode overrides to take
///    precedence over the catch-all "default" target.
/// 2. Target named `"default"` that matches the product (catch-all).
/// 3. Any target matching the product (broader fallback).
/// 4. Bare `"default"` target (last resort).
///
/// Use this to ensure all pipeline stages apply the same priority rules.
pub fn pick_effective_raw_target<'a>(
    targets: &'a [RawTarget],
    product_name: &str,
    build_mode_name: &str,
) -> Option<&'a RawTarget> {
    fn applies_to_product(t: &RawTarget, product: &str) -> bool {
        if t.apply_to_products.is_empty() {
            product == "default"
        } else {
            t.apply_to_products.iter().any(|p| p == product)
        }
    }

    fn has_specific_mode_match(t: &RawTarget, mode: &str) -> bool {
        // "specific" means the target explicitly restricts to certain modes.
        !t.build_mode_set.is_empty() && t.build_mode_set.iter().any(|m| m.name == mode)
    }

    // 1. Target with EXPLICIT buildModeSet matching this mode + product.
    //    These override catch-all targets for the specific build mode.
    if let Some(t) = targets.iter().find(|t| {
        has_specific_mode_match(t, build_mode_name) && applies_to_product(t, product_name)
    }) {
        return Some(t);
    }
    // 2. Named "default" + product match (catch-all for all modes).
    if let Some(t) = targets
        .iter()
        .find(|t| t.name == "default" && applies_to_product(t, product_name))
    {
        return Some(t);
    }
    // 3. Any target matching the product with empty buildModeSet (catch-all for all modes).
    // Targets with a specific buildModeSet that does NOT include the current mode
    // are intentionally excluded — they only apply for their declared modes.
    if let Some(t) = targets
        .iter()
        .find(|t| applies_to_product(t, product_name) && t.build_mode_set.is_empty())
    {
        return Some(t);
    }
    // 4. Bare "default" (last resort).
    targets.iter().find(|t| t.name == "default")
}

/// Count how many targets in `targets` are applicable to the given product and
/// build mode.
///
/// Used by the CLI to detect when a project declares multiple targets that
/// could apply to the current build, but only one is being used.  In hvigor's
/// multi-target execution model, each applicable target produces a separate
/// artifact; arkdown currently builds only the highest-priority target.
///
/// When this returns > 1, the CLI should warn that only one target is built
/// per invocation.
///
/// Section 3.1: "多 target 产物执行模型：需 CLI 架构重构支持同 product 多 artifact"
pub fn count_applicable_targets(
    targets: &[RawTarget],
    product_name: &str,
    build_mode_name: &str,
) -> usize {
    fn applies_to_product(t: &RawTarget, product: &str) -> bool {
        if t.apply_to_products.is_empty() {
            product == "default"
        } else {
            t.apply_to_products.iter().any(|p| p == product)
        }
    }
    targets
        .iter()
        .filter(|t| {
            if !applies_to_product(t, product_name) {
                return false;
            }
            // A target applies when it has no buildModeSet (catch-all) or when
            // its buildModeSet explicitly includes the current mode.
            t.build_mode_set.is_empty()
                || t.build_mode_set.iter().any(|m| m.name == build_mode_name)
        })
        .count()
}

/// Count applicable targets for a specific module, loading `build-profile.json5`
/// from `project_root`.  Returns 0 when the file is absent or the module is
/// not found.  This is a convenience wrapper over `count_applicable_targets`
/// for use in the CLI build loop.
pub fn count_applicable_targets_for_module(
    project_root: &Path,
    module_name: &str,
    product_name: &str,
    build_mode_name: &str,
) -> usize {
    let Ok(profile) = RawBuildProfile::load(project_root) else {
        return 0;
    };
    let Some(raw_module) = profile.modules.iter().find(|m| m.name == module_name) else {
        return 0;
    };
    count_applicable_targets(&raw_module.targets, product_name, build_mode_name)
}

/// Return ALL [`EffectiveTargetConfig`] entries applicable to the given module,
/// product, and build mode.
///
/// Unlike [`select_effective_target`] which returns only the highest-priority
/// target, this function returns every target that matches the product and
/// build-mode constraints, in declaration order.  The first entry is the
/// highest-priority target (same as `select_effective_target`).
///
/// Returns an empty `Vec` when:
/// - `build-profile.json5` cannot be read.
/// - The module is not declared in the profile.
/// - No target at all is declared for the module.
///
/// ## Multi-artifact execution model (section 3.1)
///
/// hvigor builds a separate artifact for each applicable target.  The CLI
/// uses this function to iterate over all targets and produce one output
/// artifact per target.  Each artifact path is disambiguated by appending
/// the target name (e.g. `entry-default.hap`, `entry-phone.hap`).
pub fn enumerate_applicable_targets_for_module(
    project_root: &Path,
    module_name: &str,
    product_name: &str,
    build_mode_name: &str,
) -> Vec<EffectiveTargetConfig> {
    let Ok(profile) = RawBuildProfile::load(project_root) else {
        return vec![];
    };
    let Some(raw_module) = profile.modules.iter().find(|m| m.name == module_name) else {
        return vec![];
    };
    enumerate_applicable_raw_targets(&raw_module.targets, product_name, build_mode_name)
        .into_iter()
        .map(EffectiveTargetConfig::from_raw)
        .collect()
}

/// Return all [`RawTarget`] entries applicable to the given product and build
/// mode, in declaration order.
///
/// A target is applicable when:
/// - Its `applyToProducts` list includes `product_name` (or is empty and
///   `product_name == "default"`).
/// - Its `buildModeSet` is empty (applies to all modes) OR explicitly includes
///   `build_mode_name`.
fn enumerate_applicable_raw_targets<'a>(
    targets: &'a [RawTarget],
    product_name: &str,
    build_mode_name: &str,
) -> Vec<&'a RawTarget> {
    fn applies_to_product(t: &RawTarget, product: &str) -> bool {
        if t.apply_to_products.is_empty() {
            product == "default"
        } else {
            t.apply_to_products.iter().any(|p| p == product)
        }
    }

    targets
        .iter()
        .filter(|t| {
            if !applies_to_product(t, product_name) {
                return false;
            }
            t.build_mode_set.is_empty()
                || t.build_mode_set.iter().any(|m| m.name == build_mode_name)
        })
        .collect()
}

#[derive(Debug, Clone)]
pub struct EffectiveNativeBuildOption {
    pub abi_filters: Vec<String>,
    pub external_native_options: Option<arkdown_core::ExternalNativeOptions>,
    pub native_lib: arkdown_core::NativeLibOption,
}

impl EffectiveNativeBuildOption {
    pub fn from_build_option(build_option: &arkdown_core::BuildOption) -> Self {
        Self {
            abi_filters: build_option.abi_filters.clone(),
            external_native_options: build_option.external_native_options.clone(),
            native_lib: build_option.native_lib.clone(),
        }
    }

    fn normalize_module_paths(&mut self, module_root: &Path) {
        if let Some(path) = self
            .external_native_options
            .as_mut()
            .and_then(|options| options.path.as_mut())
        {
            if path.is_relative() {
                *path = module_root.join(&*path);
            }
        }
        for path in &mut self.native_lib.header_paths {
            if path.is_relative() {
                *path = module_root.join(&*path);
            }
        }
    }
}

pub fn resolve_effective_har_artifact_type(
    module_root: &Path,
    product_artifact_type: arkdown_core::HarArtifactType,
    build_mode_name: &str,
    target_name: &str,
) -> Result<arkdown_core::HarArtifactType, ConfigError> {
    let module_profile_path = module_root.join("build-profile.json5");
    if !module_profile_path.is_file() {
        return Ok(product_artifact_type);
    }
    let content = std::fs::read_to_string(&module_profile_path)?;
    let profile: RawModuleBuildProfile =
        json5::from_str(&content).map_err(|error| ConfigError::ParseError {
            file: module_profile_path.clone(),
            message: error.to_string(),
        })?;
    let mut artifact_type = product_artifact_type;
    let mut apply = |build_option: &RawBuildOption| -> Result<(), ConfigError> {
        if let Some(value) = build_option.artifact_type.as_deref() {
            artifact_type = HarArtifactTypeParser::new(value, &module_profile_path).parse()?;
        }
        if let Some(byte_code_har) = build_option
            .ark_options
            .as_ref()
            .and_then(|options| options.byte_code_har)
        {
            artifact_type = if byte_code_har {
                arkdown_core::HarArtifactType::Bytecode
            } else if matches!(artifact_type, arkdown_core::HarArtifactType::Bytecode) {
                arkdown_core::HarArtifactType::Original
            } else {
                artifact_type
            };
        }
        Ok(())
    };
    if let Some(build_option) = profile.build_option.as_ref() {
        apply(build_option)?;
    }
    let build_option_name =
        ModuleBuildOptionSelector::new(&profile).build_option_name(build_mode_name, target_name);
    let mut names = Vec::new();
    let mut current = Some(build_option_name.as_str());
    while let Some(name) = current {
        if names.iter().any(|visited| visited == name) {
            return Err(ConfigError::ValidationError {
                file: module_profile_path.clone(),
                message: format!(
                    "circular buildOptionSet copyFrom chain: {}",
                    names.join(" -> ")
                ),
            });
        }
        let Some(build_option) = profile
            .build_option_set
            .iter()
            .find(|build_option| build_option.name.as_deref() == Some(name))
        else {
            break;
        };
        names.push(name.to_string());
        current = build_option.copy_from.as_deref();
    }
    for name in names.iter().rev() {
        if let Some(build_option) = profile
            .build_option_set
            .iter()
            .find(|build_option| build_option.name.as_deref() == Some(name.as_str()))
        {
            apply(build_option)?;
        }
    }
    if let Some(build_option) = profile
        .targets
        .iter()
        .find(|target| target.name == target_name)
        .and_then(|target| target.config.as_ref())
        .and_then(|config| config.build_option.as_ref())
    {
        apply(build_option)?;
    }
    Ok(artifact_type)
}

struct HarArtifactTypeParser<'a> {
    value: &'a str,
    file: &'a Path,
}

impl<'a> HarArtifactTypeParser<'a> {
    fn new(value: &'a str, file: &'a Path) -> Self {
        Self { value, file }
    }

    fn parse(&self) -> Result<arkdown_core::HarArtifactType, ConfigError> {
        match self.value {
            "original" => Ok(arkdown_core::HarArtifactType::Original),
            "obfuscation" => Ok(arkdown_core::HarArtifactType::Obfuscation),
            "bytecode" => Ok(arkdown_core::HarArtifactType::Bytecode),
            value => Err(ConfigError::ValidationError {
                file: self.file.to_path_buf(),
                message: format!(
                    "unsupported buildOption.artifactType '{value}'; expected original, obfuscation, or bytecode"
                ),
            }),
        }
    }
}

pub fn resolve_product_native_build_option(
    build_option: Option<&RawBuildOption>,
) -> EffectiveNativeBuildOption {
    let mut effective = EffectiveNativeBuildOption {
        abi_filters: vec!["arm64-v8a".to_string()],
        external_native_options: None,
        native_lib: arkdown_core::NativeLibOption::default(),
    };
    if let Some(build_option) = build_option {
        NativeBuildOptionMerger::new(&mut effective).apply_build_option(build_option);
    }
    if effective.abi_filters.is_empty() {
        effective.abi_filters.push("arm64-v8a".to_string());
    }
    effective
}

/// Resolve the native subset of hvigor target build options for one module
/// target.
///
/// Upstream:
/// - `BuildModeManager.initModuleTargetBuildOptionMap`
/// - `TargetTaskService.getNativeLibOption`
/// - `BuildOptionUtil.getBuildOptNameFromMapping`
pub fn resolve_effective_native_build_option(
    module_root: &Path,
    product_abi_filters: &[String],
    product_external_native_options: Option<&arkdown_core::ExternalNativeOptions>,
    product_native_lib: &arkdown_core::NativeLibOption,
    build_mode_name: &str,
    target_name: &str,
) -> Result<EffectiveNativeBuildOption, ConfigError> {
    let mut effective = EffectiveNativeBuildOption {
        abi_filters: product_abi_filters.to_vec(),
        external_native_options: product_external_native_options.cloned(),
        native_lib: product_native_lib.clone(),
    };
    let module_profile_path = module_root.join("build-profile.json5");
    if !module_profile_path.is_file() {
        effective.normalize_module_paths(module_root);
        return Ok(effective);
    }
    let content = std::fs::read_to_string(&module_profile_path)?;
    let profile: RawModuleBuildProfile =
        json5::from_str(&content).map_err(|error| ConfigError::ParseError {
            file: module_profile_path.clone(),
            message: error.to_string(),
        })?;

    if let Some(build_option) = profile.build_option.as_ref() {
        NativeBuildOptionMerger::new(&mut effective).apply_build_option(build_option);
    }

    let build_option_name =
        ModuleBuildOptionSelector::new(&profile).build_option_name(build_mode_name, target_name);
    NativeBuildOptionSetResolver::new(&profile, &module_profile_path).apply_named(
        &build_option_name,
        &mut effective,
        &mut Vec::new(),
    )?;

    if let Some(target) = profile
        .targets
        .iter()
        .find(|target| target.name == target_name)
    {
        if let Some(build_option) = target
            .config
            .as_ref()
            .and_then(|config| config.build_option.as_ref())
        {
            NativeBuildOptionMerger::new(&mut effective).apply_build_option(build_option);
        }
    }

    if effective.abi_filters.is_empty() {
        effective.abi_filters.push("arm64-v8a".to_string());
    }
    effective.normalize_module_paths(module_root);
    Ok(effective)
}

struct ModuleBuildOptionSelector<'a> {
    profile: &'a RawModuleBuildProfile,
}

impl<'a> ModuleBuildOptionSelector<'a> {
    fn new(profile: &'a RawModuleBuildProfile) -> Self {
        Self { profile }
    }

    fn build_option_name(&self, build_mode_name: &str, target_name: &str) -> String {
        for binder in &self.profile.build_mode_binder {
            if binder.build_mode_name != build_mode_name {
                continue;
            }
            for mapping in &binder.mappings {
                if mapping.target_name == target_name && !mapping.build_option_name.is_empty() {
                    return mapping.build_option_name.clone();
                }
            }
        }
        match build_mode_name {
            "debug" | "test" => "debug".to_string(),
            "release" => "release".to_string(),
            _ => "default".to_string(),
        }
    }
}

/// Resolves the effective ArkGuard configuration for one module target.
///
/// Hvigor merges module `buildOption`, the selected `buildOptionSet`
/// `copyFrom` chain, and target-local `config.buildOption` over the product
/// option. Rule paths declared by a module are relative to that module rather
/// than the project root.
pub struct EffectiveObfuscationConfigResolver<'a> {
    module_root: &'a Path,
    product_config: Option<&'a arkdown_core::ObfuscationConfig>,
    build_mode_name: &'a str,
    target_name: &'a str,
}

impl<'a> EffectiveObfuscationConfigResolver<'a> {
    pub fn new(
        module_root: &'a Path,
        product_config: Option<&'a arkdown_core::ObfuscationConfig>,
        build_mode_name: &'a str,
        target_name: &'a str,
    ) -> Self {
        Self {
            module_root,
            product_config,
            build_mode_name,
            target_name,
        }
    }

    pub fn resolve(&self) -> Result<Option<arkdown_core::ObfuscationConfig>, ConfigError> {
        let module_profile_path = self.module_root.join("build-profile.json5");
        if !module_profile_path.is_file() {
            return Ok(self.product_config.cloned());
        }
        let content = std::fs::read_to_string(&module_profile_path)?;
        let profile: RawModuleBuildProfile =
            json5::from_str(&content).map_err(|error| ConfigError::ParseError {
                file: module_profile_path.clone(),
                message: error.to_string(),
            })?;
        let mut effective = self.product_config.cloned();
        let mut merger = ObfuscationConfigMerger::new(self.module_root, &mut effective);

        if let Some(build_option) = profile.build_option.as_ref() {
            merger.apply_build_option(build_option);
        }
        let build_option_name = ModuleBuildOptionSelector::new(&profile)
            .build_option_name(self.build_mode_name, self.target_name);
        ObfuscationBuildOptionSetResolver::new(&profile, &module_profile_path).apply_named(
            &build_option_name,
            &mut merger,
            &mut Vec::new(),
        )?;
        if let Some(build_option) = profile
            .targets
            .iter()
            .find(|target| target.name == self.target_name)
            .and_then(|target| target.config.as_ref())
            .and_then(|config| config.build_option.as_ref())
        {
            merger.apply_build_option(build_option);
        }
        Ok(effective)
    }
}

struct ObfuscationBuildOptionSetResolver<'a> {
    profile: &'a RawModuleBuildProfile,
    file: &'a Path,
}

impl<'a> ObfuscationBuildOptionSetResolver<'a> {
    fn new(profile: &'a RawModuleBuildProfile, file: &'a Path) -> Self {
        Self { profile, file }
    }

    fn apply_named(
        &self,
        name: &str,
        merger: &mut ObfuscationConfigMerger<'_>,
        stack: &mut Vec<String>,
    ) -> Result<(), ConfigError> {
        let Some(build_option) = self
            .profile
            .build_option_set
            .iter()
            .find(|build_option| build_option.name.as_deref() == Some(name))
        else {
            return Ok(());
        };
        if stack.iter().any(|visited| visited == name) {
            return Err(ConfigError::ValidationError {
                file: self.file.to_path_buf(),
                message: format!(
                    "circular buildOptionSet copyFrom chain: {} -> {name}",
                    stack.join(" -> ")
                ),
            });
        }
        stack.push(name.to_string());
        if let Some(copy_from) = build_option.copy_from.as_deref() {
            self.apply_named(copy_from, merger, stack)?;
        }
        merger.apply_build_option(build_option);
        stack.pop();
        Ok(())
    }
}

struct ObfuscationConfigMerger<'a> {
    module_root: &'a Path,
    effective: &'a mut Option<arkdown_core::ObfuscationConfig>,
}

impl<'a> ObfuscationConfigMerger<'a> {
    fn new(
        module_root: &'a Path,
        effective: &'a mut Option<arkdown_core::ObfuscationConfig>,
    ) -> Self {
        Self {
            module_root,
            effective,
        }
    }

    fn apply_build_option(&mut self, build_option: &RawBuildOption) {
        let Some(raw) = build_option
            .ark_options
            .as_ref()
            .and_then(|options| options.obfuscation.as_ref())
        else {
            return;
        };
        let resolve_paths = |paths: &[String]| {
            paths
                .iter()
                .map(|path| {
                    let path = Path::new(path);
                    if path.is_absolute() {
                        path.to_path_buf()
                    } else {
                        self.module_root.join(path)
                    }
                })
                .collect()
        };
        let rule_files = raw
            .rule_options
            .as_ref()
            .map(|options| {
                if options.rules.is_empty() {
                    options
                        .files
                        .as_ref()
                        .map(|files| resolve_paths(files.as_slice()))
                        .unwrap_or_default()
                } else {
                    resolve_paths(&options.rules)
                }
            })
            .unwrap_or_default();
        let consumer_rule_files = if raw.consumer_rules.is_empty() {
            raw.consumer_files
                .as_ref()
                .map(|files| resolve_paths(files.as_slice()))
                .unwrap_or_default()
        } else {
            resolve_paths(&raw.consumer_rules)
        };
        *self.effective = Some(arkdown_core::ObfuscationConfig {
            enable_obfuscation: raw
                .rule_options
                .as_ref()
                .is_some_and(|options| options.enable),
            rule_files,
            consumer_rule_files,
        });
    }
}

struct NativeBuildOptionSetResolver<'a> {
    profile: &'a RawModuleBuildProfile,
    file: &'a Path,
}

impl<'a> NativeBuildOptionSetResolver<'a> {
    fn new(profile: &'a RawModuleBuildProfile, file: &'a Path) -> Self {
        Self { profile, file }
    }

    fn apply_named(
        &self,
        name: &str,
        effective: &mut EffectiveNativeBuildOption,
        stack: &mut Vec<String>,
    ) -> Result<(), ConfigError> {
        let Some(build_option) = self
            .profile
            .build_option_set
            .iter()
            .find(|build_option| build_option.name.as_deref() == Some(name))
        else {
            return Ok(());
        };
        if stack.iter().any(|visited| visited == name) {
            return Err(ConfigError::ValidationError {
                file: self.file.to_path_buf(),
                message: format!(
                    "circular buildOptionSet copyFrom chain: {}",
                    stack.join(" -> ")
                ),
            });
        }
        stack.push(name.to_string());
        if let Some(copy_from) = build_option.copy_from.as_deref() {
            self.apply_named(copy_from, effective, stack)?;
        }
        NativeBuildOptionMerger::new(effective).apply_build_option(build_option);
        stack.pop();
        Ok(())
    }
}

struct NativeBuildOptionMerger<'a> {
    effective: &'a mut EffectiveNativeBuildOption,
}

impl<'a> NativeBuildOptionMerger<'a> {
    fn new(effective: &'a mut EffectiveNativeBuildOption) -> Self {
        Self { effective }
    }

    fn apply_build_option(&mut self, build_option: &RawBuildOption) {
        if let Some(external_native_options) = build_option.external_native_options.as_ref() {
            let effective_external = self
                .effective
                .external_native_options
                .get_or_insert_with(arkdown_core::ExternalNativeOptions::default);
            if external_native_options.path.is_some() {
                effective_external.path = external_native_options.path.clone();
            }
            if let Some(arguments) = external_native_options.arguments.as_ref() {
                effective_external.arguments = arguments.as_slice().to_vec();
            }
            if !external_native_options.abi_filters.is_empty() {
                self.effective.abi_filters = external_native_options.abi_filters.clone();
                effective_external.abi_filters = external_native_options.abi_filters.clone();
            }
            if external_native_options.cpp_flags.is_some() {
                effective_external.cpp_flags = external_native_options.cpp_flags.clone();
            }
            if external_native_options.c_flags.is_some() {
                effective_external.c_flags = external_native_options.c_flags.clone();
            }
            if let Some(targets) = external_native_options.targets.as_ref() {
                effective_external.targets = targets.clone();
            }
        }
        if let Some(filter) = build_option.napi_lib_filter_option.as_ref() {
            self.apply_filter(filter);
        }
        if let Some(native_lib) = build_option.native_lib.as_ref() {
            if let Some(collect_all_libs) = native_lib.collect_all_libs {
                self.effective.native_lib.collect_all_libs = collect_all_libs;
            }
            if let Some(filter) = native_lib.filter.as_ref() {
                self.apply_filter(filter);
            }
            if let Some(header_paths) = native_lib.header_path.as_ref() {
                self.effective.native_lib.header_paths =
                    header_paths.as_slice().iter().map(PathBuf::from).collect();
            }
            if let Some(exclude_from_har) = native_lib.exclude_from_har {
                self.effective.native_lib.exclude_from_har = exclude_from_har;
            }
            if let Some(exclude_so_from_interface_har) = native_lib.exclude_so_from_interface_har {
                self.effective.native_lib.exclude_so_from_interface_har =
                    exclude_so_from_interface_har;
            }
            if let Some(exclude_so_from_binxo) = native_lib.exclude_so_from_binxo.as_ref() {
                self.effective.native_lib.exclude_so_from_binxo =
                    exclude_so_from_binxo.as_slice().to_vec();
            }
            if let Some(debug_symbol) = native_lib.debug_symbol.as_ref() {
                if let Some(strip) = debug_symbol.strip {
                    self.effective.native_lib.debug_symbol.strip = strip;
                }
                if let Some(exclude) = debug_symbol.exclude.as_ref() {
                    self.effective.native_lib.debug_symbol.exclude = exclude.as_slice().to_vec();
                }
            }
        }
    }

    fn apply_filter(&mut self, filter: &RawNativeLibFilterOption) {
        if let Some(excludes) = filter.excludes.as_ref() {
            self.effective.native_lib.filter.excludes = excludes.clone();
        }
        if let Some(pick_firsts) = filter.pick_firsts.as_ref() {
            self.effective.native_lib.filter.pick_firsts = pick_firsts.clone();
        }
        if let Some(pick_lasts) = filter.pick_lasts.as_ref() {
            self.effective.native_lib.filter.pick_lasts = pick_lasts.clone();
        }
        if let Some(enable_override) = filter.enable_override {
            self.effective.native_lib.filter.enable_override = enable_override;
        }
        if let Some(select) = filter.select.as_ref() {
            self.effective.native_lib.filter.select = select
                .iter()
                .map(|selection| arkdown_core::NativeLibPackageSelection {
                    package: selection.package.clone(),
                    version: selection.version.clone(),
                    include: selection.include.clone(),
                    exclude: selection.exclude.clone(),
                    include_pattern: selection.include_pattern.clone(),
                    exclude_pattern: selection.exclude_pattern.clone(),
                })
                .collect();
        }
    }
}

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

/// Parse the contents of `build-profile.json5`.

/// Extract the numeric API version from a string like `"5.0.0(12)"` → `12`.
/// Returns `None` if no parenthesised number is found.

/// Load and parse a `build-profile.json5` file from the given `root` directory.

/// Load and parse `AppScope/app.json5` from the project root.
///
/// Returns a default (all-empty) value if the file does not exist, so callers
/// can use it without failing on older-layout projects.

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_compatible_sdk_version() {
        assert_eq!(parse_api_version("5.0.0(12)"), Some(12));
        assert_eq!(parse_api_version("4.1.0(9)"), Some(9));
        assert_eq!(parse_api_version("5.0.0"), None);
        assert_eq!(parse_api_version(""), None);
        assert_eq!(parse_api_version("(abc)"), None);
    }

    #[test]
    fn test_parse_build_profile_minimal() {
        let json = r#"{
            app: {
                products: [{
                    name: "default",
                    bundleName: "com.example.app",
                    compatibleSdkVersion: "5.0.0(12)",
                    compatibleSdkVersionStage: "beta3",
                    runtimeOS: "HarmonyOS",
                }],
            },
            modules: [{
                name: "entry",
                srcPath: "./entry",
                targets: [{ name: "default", applyToProducts: ["default"] }]
            }]
        }"#;
        let profile =
            RawBuildProfile::parse(json, std::path::Path::new("build-profile.json5")).unwrap();
        assert_eq!(profile.app.products.len(), 1);
        assert_eq!(profile.app.products[0].name, "default");
        assert_eq!(profile.app.products[0].bundle_name, "com.example.app");
        assert_eq!(profile.app.products[0].runtime_os, "HarmonyOS");
        assert_eq!(
            parse_api_version(&profile.app.products[0].compatible_sdk_version),
            Some(12)
        );
        assert_eq!(
            profile.app.products[0]
                .compatible_sdk_version_stage
                .as_deref(),
            Some("beta3")
        );
        assert_eq!(profile.modules.len(), 1);
        assert_eq!(profile.modules[0].name, "entry");
    }

    #[test]
    fn test_parse_build_profile_module_src_path() {
        let json = r#"{
            app: {
                products: [{ name: "default", bundleName: "com.test", compatibleSdkVersion: "5.0.0(12)", runtimeOS: "HarmonyOS" }],
            },
            modules: [{
                name: "entry",
                srcPath: "./entry",
            }]
        }"#;
        let profile =
            RawBuildProfile::parse(json, std::path::Path::new("build-profile.json5")).unwrap();
        assert_eq!(profile.modules[0].name, "entry");
        assert_eq!(profile.modules[0].src_path, "./entry");
    }

    #[test]
    fn test_parse_target_resource_directories() {
        let json = r#"{
            app: {
                products: [{ name: "default", bundleName: "com.test", compatibleSdkVersion: "5.0.0(12)", runtimeOS: "HarmonyOS" }],
            },
            modules: [{
                name: "entry",
                srcPath: "./entry",
                targets: [{
                    name: "phone",
                    applyToProducts: ["default"],
                    resource: {
                        directories: ["src/phone/resources", "custom/resources"]
                    }
                }]
            }]
        }"#;
        let profile =
            RawBuildProfile::parse(json, std::path::Path::new("build-profile.json5")).unwrap();
        let directories = &profile.modules[0].targets[0]
            .resource
            .as_ref()
            .expect("target resource")
            .directories;
        assert_eq!(
            directories,
            &vec![
                PathBuf::from("src/phone/resources"),
                PathBuf::from("custom/resources")
            ]
        );
    }

    #[test]
    fn test_parse_build_profile_bundle_name_access() {
        let json = r#"{
            app: {
                products: [{
                    name: "release",
                    bundleName: "com.example.myapp",
                    compatibleSdkVersion: "5.0.0(12)",
                    runtimeOS: "HarmonyOS",
                }],
            },
            modules: [{ name: "entry", srcPath: "./entry" }]
        }"#;
        let profile =
            RawBuildProfile::parse(json, std::path::Path::new("build-profile.json5")).unwrap();
        assert_eq!(profile.app.products[0].bundle_name, "com.example.myapp");
    }

    #[test]
    fn test_parse_build_profile_app_multi_projects() {
        let json = r#"{
            app: {
                multiProjects: true,
                products: [{
                    name: "default",
                    bundleName: "com.example.app",
                    compatibleSdkVersion: "5.0.0(12)",
                    runtimeOS: "HarmonyOS",
                }],
            },
            modules: [{ name: "entry", srcPath: "./entry" }]
        }"#;
        let profile =
            RawBuildProfile::parse(json, std::path::Path::new("build-profile.json5")).unwrap();
        assert!(profile.app.multi_projects);
    }

    #[test]
    fn test_parse_remove_permissions_accepts_hvigor_named_objects() {
        let json = r#"{
            app: {
                products: [{
                    name: "default",
                    bundleName: "com.example.app",
                    compatibleSdkVersion: "5.0.0(12)",
                    runtimeOS: "HarmonyOS",
                    buildOption: {
                        removePermissions: [
                            { name: "ohos.permission.CAMERA" },
                            { name: "ohos.permission.MICROPHONE" },
                            "ohos.permission.LOCATION"
                        ]
                    }
                }],
            },
            modules: [{ name: "entry", srcPath: "./entry" }]
        }"#;
        let profile =
            RawBuildProfile::parse(json, std::path::Path::new("build-profile.json5")).unwrap();
        let build_option = profile.app.products[0].build_option.as_ref().unwrap();
        assert_eq!(
            build_option.remove_permissions,
            vec![
                "ohos.permission.CAMERA",
                "ohos.permission.MICROPHONE",
                "ohos.permission.LOCATION"
            ]
        );
    }

    #[test]
    fn test_parse_preload_system_so() {
        let json = r#"{
            app: {
                products: [{
                    name: "default",
                    bundleName: "com.example.app",
                    compatibleSdkVersion: "5.0.0(12)",
                    runtimeOS: "HarmonyOS",
                    buildOption: {
                        preloadSystemSo: true
                    }
                }],
            },
            modules: [{ name: "entry", srcPath: "./entry" }]
        }"#;
        let profile =
            RawBuildProfile::parse(json, std::path::Path::new("build-profile.json5")).unwrap();
        let build_option = profile.app.products[0].build_option.as_ref().unwrap();
        assert!(build_option.preload_system_so);
    }

    #[test]
    fn test_parse_aot_fields_under_ark_options() {
        let json = r#"{
            app: {
                products: [{
                    name: "default",
                    bundleName: "com.example.app",
                    compatibleSdkVersion: "5.0.0(12)",
                    runtimeOS: "HarmonyOS",
                    buildOption: {
                        aotCompileMode: "type",
                        apPath: "./top.ap",
                        arkOptions: {
                            aotCompileMode: "partial",
                            apPath: "./modules.ap",
                            hostPGO: true
                        }
                    }
                }],
            },
            modules: [{ name: "entry", srcPath: "./entry" }]
        }"#;
        let profile =
            RawBuildProfile::parse(json, std::path::Path::new("build-profile.json5")).unwrap();
        let build_option = profile.app.products[0].build_option.as_ref().unwrap();
        let ark_options = build_option.ark_options.as_ref().unwrap();
        assert_eq!(build_option.aot_compile_mode.as_deref(), Some("type"));
        assert_eq!(
            build_option.ap_path.as_deref(),
            Some(std::path::Path::new("./top.ap"))
        );
        assert_eq!(ark_options.aot_compile_mode.as_deref(), Some("partial"));
        assert_eq!(
            ark_options.ap_path.as_deref(),
            Some(std::path::Path::new("./modules.ap"))
        );
        assert_eq!(ark_options.host_pgo, Some(true));
    }

    #[test]
    fn test_parse_build_profile_missing_required_field_returns_err() {
        // `app` key is missing entirely — required by `RawBuildProfile`
        let json = r#"{ modules: [] }"#;
        let result = RawBuildProfile::parse(json, std::path::Path::new("build-profile.json5"));
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_build_profile_multiple_modules() {
        let json = r#"{
            app: {
                products: [{ name: "default", bundleName: "com.multi", compatibleSdkVersion: "5.0.0(12)", runtimeOS: "HarmonyOS" }],
            },
            modules: [
                { name: "entry", srcPath: "./entry" },
                { name: "feature", srcPath: "./feature" }
            ]
        }"#;
        let profile =
            RawBuildProfile::parse(json, std::path::Path::new("build-profile.json5")).unwrap();
        assert_eq!(profile.modules.len(), 2);
        assert_eq!(profile.modules[1].name, "feature");
        assert_eq!(profile.modules[1].src_path, "./feature");
    }

    #[test]
    fn test_parse_build_profile_with_signing() {
        let json = r#"{
            app: {
                products: [{
                    name: "default",
                    bundleName: "com.example.app",
                    compatibleSdkVersion: "5.0.0(12)",
                    runtimeOS: "HarmonyOS",
                    signingConfigs: [{
                        name: "default",
                        material: {
                            certPath: "/path/to/cert",
                            keyAlias: "mykey",
                            keyPassword: "secret",
                            profile: "/path/to/profile",
                            signAlg: "SHA256withECDSA",
                            storeFile: "/path/to/store",
                            storePassword: "storepass"
                        }
                    }],
                    buildOption: {
                        strictMode: {
                            caseSensitiveCheck: true,
                            useNormalizedOHMUrl: true,
                        },
                        arkOptions: {
                            types: ["./types/runtime"],
                            buildProfileFields: {
                                MTDEV: true,
                                CHANNEL: "beta"
                            },
                            obfuscation: {
                                ruleOptions: { enable: false }
                            },
                            runtimeOnly: {
                                packages: ["@ohos/dynamic-feature"],
                                sources: ["src/main/ets/runtime"],
                                excludePackages: ["@ohos/remote-har"]
                            }
                        }
                    }
                }],
            },
            modules: []
        }"#;
        let profile =
            RawBuildProfile::parse(json, std::path::Path::new("build-profile.json5")).unwrap();
        let product = &profile.app.products[0];
        assert_eq!(product.signing_configs.len(), 1);
        let material = product.signing_configs[0].material.as_ref().unwrap();
        assert_eq!(material.cert_path, "/path/to/cert");
        assert_eq!(material.sign_alg, "SHA256withECDSA");
        let build_option = product.build_option.as_ref().unwrap();
        let strict = build_option.strict_mode.as_ref().unwrap();
        assert!(strict.case_sensitive_check);
        assert!(strict.use_normalized_ohm_url);
        let runtime_only = build_option
            .ark_options
            .as_ref()
            .and_then(|ark_options| ark_options.runtime_only.as_ref())
            .expect("runtimeOnly parsed");
        assert_eq!(runtime_only.packages, vec!["@ohos/dynamic-feature"]);
        assert_eq!(runtime_only.sources, vec!["src/main/ets/runtime"]);
        assert_eq!(runtime_only.exclude_packages, vec!["@ohos/remote-har"]);
        let ark_options = build_option.ark_options.as_ref().unwrap();
        assert_eq!(ark_options.types, vec!["./types/runtime"]);
        assert_eq!(
            ark_options
                .build_profile_fields
                .as_ref()
                .unwrap()
                .get("MTDEV"),
            Some(&serde_json::Value::Bool(true))
        );
    }

    #[test]
    fn test_parse_obfuscation_rule_files() {
        let json = r#"{
            app: {
                products: [{
                    name: "default",
                    bundleName: "com.example.app",
                    compatibleSdkVersion: "5.0.0(12)",
                    runtimeOS: "HarmonyOS",
                    buildOption: {
                        arkOptions: {
                            obfuscation: {
                                ruleOptions: {
                                    enable: true,
                                    rules: [
                                        "./obfuscation-rules.txt",
                                        "./extra-rules.txt"
                                    ]
                                }
                            }
                        }
                    }
                }],
            },
            modules: []
        }"#;
        let profile =
            RawBuildProfile::parse(json, std::path::Path::new("build-profile.json5")).unwrap();
        let build_option = profile.app.products[0].build_option.as_ref().unwrap();
        let ark_options = build_option.ark_options.as_ref().unwrap();
        let obfuscation = ark_options.obfuscation.as_ref().unwrap();
        let rule_options = obfuscation.rule_options.as_ref().unwrap();
        assert!(rule_options.enable);
        assert_eq!(rule_options.rules.len(), 2);
        assert_eq!(rule_options.rules[0], "./obfuscation-rules.txt");
        assert_eq!(rule_options.rules[1], "./extra-rules.txt");
    }

    #[test]
    fn test_parse_obfuscation_files_and_consumer_files() {
        let json = r#"{
            app: {
                products: [{
                    name: "default",
                    bundleName: "com.example.app",
                    compatibleSdkVersion: "5.0.0(12)",
                    runtimeOS: "HarmonyOS",
                    buildOption: {
                        arkOptions: {
                            obfuscation: {
                                ruleOptions: {
                                    enable: true,
                                    files: "./obfuscation-rules.txt"
                                },
                                consumerFiles: [
                                    "./consumer-rules.txt",
                                    "./extra-consumer-rules.txt"
                                ]
                            }
                        }
                    }
                }],
            },
            modules: []
        }"#;
        let profile =
            RawBuildProfile::parse(json, std::path::Path::new("build-profile.json5")).unwrap();
        let obfuscation = profile.app.products[0]
            .build_option
            .as_ref()
            .and_then(|build_option| build_option.ark_options.as_ref())
            .and_then(|ark_options| ark_options.obfuscation.as_ref())
            .unwrap();
        let rule_options = obfuscation.rule_options.as_ref().unwrap();
        assert_eq!(
            rule_options.files.as_ref().unwrap().as_slice(),
            &["./obfuscation-rules.txt".to_string()]
        );
        assert_eq!(
            obfuscation.consumer_files.as_ref().unwrap().as_slice(),
            &[
                "./consumer-rules.txt".to_string(),
                "./extra-consumer-rules.txt".to_string()
            ]
        );
    }

    // -----------------------------------------------------------------------
    // Tests for the real OHOS project format
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_real_format_app_level_signing_configs() {
        // Real OHOS: signingConfigs live under app, products reference by name.
        let json = r#"{
            app: {
                signingConfigs: [
                    {
                        name: "default",
                        type: "HarmonyOS",
                        material: {
                            certpath: "/certs/debug.cer",
                            keyAlias: "debugKey",
                            keyPassword: "enc_pass",
                            profile: "/certs/debug.p7b",
                            signAlg: "SHA256withECDSA",
                            storeFile: "/certs/debug.p12",
                            storePassword: "enc_store"
                        }
                    }
                ],
                products: [
                    {
                        name: "default",
                        signingConfig: "default",
                        targetSdkVersion: "5.0.5(17)",
                        compatibleSdkVersion: "5.0.5(17)",
                        runtimeOS: "HarmonyOS"
                    }
                ],
                buildModeSet: [{ name: "debug" }, { name: "release" }]
            },
            modules: [{ name: "entry", srcPath: "./entry" }]
        }"#;

        let profile =
            RawBuildProfile::parse(json, std::path::Path::new("build-profile.json5")).unwrap();

        // App-level signing configs
        assert_eq!(profile.app.signing_configs.len(), 1);
        let sc = &profile.app.signing_configs[0];
        assert_eq!(sc.name, "default");
        assert_eq!(sc.config_type, "HarmonyOS");
        let mat = sc.material.as_ref().unwrap();
        // certpath (lowercase p) should deserialise into cert_path
        assert_eq!(mat.cert_path, "/certs/debug.cer");
        assert_eq!(mat.key_alias, "debugKey");

        // Product references signing config by name (not embedded)
        let product = &profile.app.products[0];
        assert_eq!(product.signing_config.as_deref(), Some("default"));
        assert_eq!(product.target_sdk_version.as_deref(), Some("5.0.5(17)"));
        // bundle_name absent → empty string
        assert_eq!(product.bundle_name, "");
    }

    #[test]
    fn test_parse_api_version_target_sdk() {
        // targetSdkVersion uses the same "(N)" format as compatibleSdkVersion.
        assert_eq!(parse_api_version("5.0.5(17)"), Some(17));
        assert_eq!(parse_api_version("5.0.0(12)"), Some(12));
    }

    #[test]
    fn test_load_app_json5_missing_file() {
        // Missing file → default (all empty), no error.
        let tmp = std::env::temp_dir().join("arkdown_no_appscope");
        std::fs::create_dir_all(&tmp).unwrap();
        // Ensure AppScope/app.json5 does not exist.
        let _ = std::fs::remove_file(tmp.join("AppScope/app.json5"));

        let result = RawAppJson5::load(&tmp).unwrap();
        assert_eq!(result.app.bundle_name, "");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_load_app_json5_present() {
        use std::io::Write;

        let tmp = std::env::temp_dir().join("arkdown_appscope_test");
        std::fs::create_dir_all(tmp.join("AppScope")).unwrap();

        let content = r#"{
            "app": {
                "bundleName": "com.example.myapplication",
                "vendor": "example",
                "versionCode": 1000000,
                "versionName": "1.0.0"
            }
        }"#;
        let mut f = std::fs::File::create(tmp.join("AppScope/app.json5")).unwrap();
        f.write_all(content.as_bytes()).unwrap();

        let app = RawAppJson5::load(&tmp).unwrap();
        assert_eq!(app.app.bundle_name, "com.example.myapplication");
        assert_eq!(app.app.vendor, "example");
        assert_eq!(app.app.version_code, 1000000);
        assert_eq!(app.app.version_name, "1.0.0");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_load_app_json5_rejects_hvigor_invalid_bundle_name() {
        use std::io::Write;

        let tmp = std::env::temp_dir().join("arkdown_appscope_invalid_bundle_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("AppScope")).unwrap();

        let content = r#"{
            "app": {
                "bundleName": "Bundle name",
                "vendor": "example",
                "versionCode": 1000000,
                "versionName": "1.0.0"
            }
        }"#;
        let mut f = std::fs::File::create(tmp.join("AppScope/app.json5")).unwrap();
        f.write_all(content.as_bytes()).unwrap();

        let err = RawAppJson5::load(&tmp).expect_err("invalid bundleName rejected");
        assert!(
            matches!(err, ConfigError::ValidationError { .. }),
            "expected validation error, got {err:?}"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_hvigor_bundle_name_pattern() {
        assert!(HvigorBundleName::new("com.example.myapplication").is_valid());
        assert!(HvigorBundleName::new("a.b.c").is_valid());
        assert!(HvigorBundleName::new("com.example_1.app2").is_valid());

        assert!(!HvigorBundleName::new("Bundle name").is_valid());
        assert!(!HvigorBundleName::new("com.example").is_valid());
        assert!(!HvigorBundleName::new("1com.example.app").is_valid());
        assert!(!HvigorBundleName::new("com._example.app").is_valid());
        assert!(!HvigorBundleName::new("com.example_.app").is_valid());
        assert!(!HvigorBundleName::new("com.example.app_").is_valid());
    }

    /// Verify that `enumerate_applicable_targets_for_module` returns all matching
    /// targets in declaration order.
    #[test]
    fn test_enumerate_applicable_raw_targets_returns_all_matches() {
        let targets: Vec<RawTarget> = vec![
            RawTarget {
                name: "default".to_string(),
                apply_to_products: vec!["default".to_string()],
                config: None,
                resource: None,
                source: None,
                build_mode_set: vec![],
            },
            RawTarget {
                name: "phone".to_string(),
                apply_to_products: vec!["default".to_string()],
                config: None,
                resource: Some(RawTargetResource {
                    directories: vec![PathBuf::from("src/phone/resources")],
                }),
                source: None,
                build_mode_set: vec![],
            },
            RawTarget {
                name: "tablet".to_string(),
                apply_to_products: vec!["tablet-product".to_string()],
                config: None,
                resource: None,
                source: None,
                build_mode_set: vec![],
            },
        ];

        let applicable = enumerate_applicable_raw_targets(&targets, "default", "debug");
        // "default" and "phone" both apply to "default" product; "tablet" does not.
        assert_eq!(applicable.len(), 2);
        assert_eq!(applicable[0].name, "default");
        assert_eq!(applicable[1].name, "phone");

        // Only "tablet" applies to "tablet-product".
        let tablet_applicable =
            enumerate_applicable_raw_targets(&targets, "tablet-product", "debug");
        assert_eq!(tablet_applicable.len(), 1);
        assert_eq!(tablet_applicable[0].name, "tablet");
    }

    /// Verify that `enumerate_applicable_raw_targets` respects `buildModeSet` filtering.
    #[test]
    fn test_enumerate_applicable_raw_targets_respects_build_mode() {
        let targets: Vec<RawTarget> = vec![
            RawTarget {
                name: "default".to_string(),
                apply_to_products: vec!["default".to_string()],
                config: None,
                resource: None,
                source: None,
                build_mode_set: vec![],
            },
            RawTarget {
                name: "release-only".to_string(),
                apply_to_products: vec!["default".to_string()],
                config: None,
                resource: None,
                source: None,
                build_mode_set: vec![RawBuildMode {
                    name: "release".to_string(),
                }],
            },
        ];

        // Debug mode: only "default" (release-only is excluded).
        let debug = enumerate_applicable_raw_targets(&targets, "default", "debug");
        assert_eq!(debug.len(), 1);
        assert_eq!(debug[0].name, "default");

        // Release mode: both apply.
        let release = enumerate_applicable_raw_targets(&targets, "default", "release");
        assert_eq!(release.len(), 2);
    }

    /// Verify that `externalNativeOptions.abiFilters` is parsed and resolved.
    #[test]
    fn test_abi_filters_parsed_from_external_native_options() {
        let json = r#"{
            app: {
              products: [{
                name: "default",
                buildOption: {
                  externalNativeOptions: {
                    abiFilters: ["arm64-v8a", "armeabi-v7a", "x86_64"]
                  }
                }
              }]
            },
            modules: [{ name: "entry", srcPath: "./entry" }]
        }"#;

        let profile =
            RawBuildProfile::parse(json, std::path::Path::new("build-profile.json5")).unwrap();
        let product = &profile.app.products[0];
        let build_option = product.build_option.as_ref().unwrap();
        let eno = build_option.external_native_options.as_ref().unwrap();
        assert_eq!(eno.abi_filters, vec!["arm64-v8a", "armeabi-v7a", "x86_64"]);
    }

    #[test]
    fn test_native_lib_collect_all_and_filter_presence_parsed() {
        let json = r#"{
            app: {
              products: [{
                name: "default",
                buildOption: {
                  nativeLib: {
                    collectAllLibs: true,
                    filter: { excludes: ["arm64-v8a/libdebug.so"] }
                  }
                }
              }]
            },
            modules: [{ name: "entry", srcPath: "./entry" }]
        }"#;

        let profile =
            RawBuildProfile::parse(json, std::path::Path::new("build-profile.json5")).unwrap();
        let native_lib = profile.app.products[0]
            .build_option
            .as_ref()
            .unwrap()
            .native_lib
            .as_ref()
            .unwrap();
        assert_eq!(native_lib.collect_all_libs, Some(true));
        assert!(native_lib.filter.is_some());
    }

    #[test]
    fn test_hsp_interface_options_and_target_source_roots_are_parsed() {
        let json = r#"{
            app: {
              products: [{
                name: "default",
                buildOption: {
                  arkOptions: { integratedHsp: true },
                  nativeLib: { excludeSoFromInterfaceHar: true }
                }
              }]
            },
            modules: [{
              name: "entry",
              srcPath: "./entry",
              targets: [{
                name: "default",
                source: { sourceRoots: ["./src/shared", "./generated/contracts"] }
              }]
            }]
        }"#;

        let profile =
            RawBuildProfile::parse(json, std::path::Path::new("build-profile.json5")).unwrap();
        let build_option = profile.app.products[0].build_option.as_ref().unwrap();
        assert_eq!(
            build_option
                .ark_options
                .as_ref()
                .and_then(|options| options.integrated_hsp),
            Some(true)
        );
        assert_eq!(
            build_option
                .native_lib
                .as_ref()
                .and_then(|options| options.exclude_so_from_interface_har),
            Some(true)
        );
        assert_eq!(
            profile.modules[0].targets[0]
                .source
                .as_ref()
                .unwrap()
                .source_roots,
            vec!["./src/shared", "./generated/contracts"]
        );

        let project_root = std::env::temp_dir().join(format!(
            "arkdown-target-source-roots-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&project_root).unwrap();
        std::fs::write(project_root.join("build-profile.json5"), json).unwrap();
        let targets =
            enumerate_applicable_targets_for_module(&project_root, "entry", "default", "debug");
        assert_eq!(
            targets[0].source_roots,
            vec!["./src/shared", "./generated/contracts"]
        );
        std::fs::remove_dir_all(project_root).unwrap();
    }

    #[test]
    fn test_resolve_effective_native_build_option_reads_module_build_option_set() {
        let tmp = std::env::temp_dir().join(format!(
            "arkdown-config-native-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(
            tmp.join("build-profile.json5"),
            r#"{
              buildOption: {
                nativeLib: { collectAllLibs: true }
              },
              buildOptionSet: [{
                name: "release",
                externalNativeOptions: { abiFilters: ["arm64-v8a"] },
                napiLibFilterOption: {
                  excludes: ["**/armeabi-v7a/*.so", "**/x86_64/*.so"]
                },
                nativeLib: {
                  collectAllLibs: false,
                  excludeSoFromInterfaceHar: true,
                  filter: {
                    select: [{
                      package: "native_pkg",
                      version: "1.2.3",
                      include: ["libkeep.so"]
                    }]
                  }
                }
              }]
            }"#,
        )
        .unwrap();

        let product_native = arkdown_core::NativeLibOption::default();
        let effective = resolve_effective_native_build_option(
            &tmp,
            &["armeabi-v7a".to_string()],
            None,
            &product_native,
            "release",
            "default",
        )
        .expect("module build option");
        assert_eq!(effective.abi_filters, vec!["arm64-v8a"]);
        assert!(!effective.native_lib.collect_all_libs);
        assert!(effective.native_lib.exclude_so_from_interface_har);
        assert_eq!(
            effective.native_lib.filter.excludes,
            vec!["**/armeabi-v7a/*.so", "**/x86_64/*.so"]
        );
        assert_eq!(effective.native_lib.filter.select.len(), 1);
        assert_eq!(
            effective.native_lib.filter.select[0].include,
            Some(vec!["libkeep.so".to_string()])
        );
        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn effective_obfuscation_uses_module_release_option_and_module_relative_rules() {
        let tmp = std::env::temp_dir().join(format!(
            "arkdown-config-obfuscation-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(
            tmp.join("build-profile.json5"),
            r#"{
              buildOptionSet: [{
                name: "release-base",
                arkOptions: { obfuscation: {
                  ruleOptions: { enable: true, files: ["./base-rules.txt"] }
                } }
              }, {
                name: "release",
                copyFrom: "release-base",
                arkOptions: { obfuscation: {
                  ruleOptions: { enable: true, files: ["./release-rules.txt"] },
                  consumerFiles: "./consumer-rules.txt"
                } }
              }],
              targets: [{ name: "default" }]
            }"#,
        )
        .unwrap();

        let release = EffectiveObfuscationConfigResolver::new(&tmp, None, "release", "default")
            .resolve()
            .expect("release obfuscation")
            .expect("configured obfuscation");
        assert!(release.enable_obfuscation);
        assert_eq!(release.rule_files, vec![tmp.join("release-rules.txt")]);
        assert_eq!(
            release.consumer_rule_files,
            vec![tmp.join("consumer-rules.txt")]
        );

        let debug = EffectiveObfuscationConfigResolver::new(&tmp, None, "debug", "default")
            .resolve()
            .expect("debug obfuscation");
        assert!(debug.is_none());
        std::fs::remove_dir_all(&tmp).unwrap();
    }

    /// Verify that `buildModeSet` on a target is parsed correctly.
    ///
    /// hvigor supports per-target mode filtering:
    /// ```json5
    /// targets: [{
    ///   name: "release-target",
    ///   applyToProducts: ["default"],
    ///   buildModeSet: [{ name: "release" }],
    ///   resource: { directories: ["src/release/resources"] }
    /// }]
    /// ```
    #[test]
    fn test_target_build_mode_set_parsed() {
        let json = r#"{
            app: { products: [{ name: "default" }] },
            modules: [{
              name: "entry",
              srcPath: "./entry",
              targets: [
                {
                  name: "debug-target",
                  applyToProducts: ["default"],
                  buildModeSet: [{ name: "debug" }],
                  resource: { directories: ["src/debug/resources"] }
                },
                {
                  name: "release-target",
                  applyToProducts: ["default"],
                  buildModeSet: [{ name: "release" }],
                  resource: { directories: ["src/release/resources"] }
                },
                {
                  name: "all-modes-target",
                  applyToProducts: ["default"]
                }
              ]
            }]
        }"#;

        let profile =
            RawBuildProfile::parse(json, std::path::Path::new("build-profile.json5")).unwrap();
        let module = &profile.modules[0];
        assert_eq!(module.targets.len(), 3);

        let debug_target = &module.targets[0];
        assert_eq!(debug_target.name, "debug-target");
        assert_eq!(debug_target.build_mode_set.len(), 1);
        assert_eq!(debug_target.build_mode_set[0].name, "debug");

        let release_target = &module.targets[1];
        assert_eq!(release_target.name, "release-target");
        assert_eq!(release_target.build_mode_set.len(), 1);
        assert_eq!(release_target.build_mode_set[0].name, "release");

        let all_modes_target = &module.targets[2];
        assert_eq!(all_modes_target.name, "all-modes-target");
        assert!(
            all_modes_target.build_mode_set.is_empty(),
            "target without buildModeSet should match all modes"
        );
    }

    /// Verify `pick_effective_raw_target` selects the correct target using
    /// the 4-priority hvigor rules.
    #[test]
    fn test_pick_effective_raw_target_priority() {
        let targets: Vec<RawTarget> = vec![
            // All-modes "default" target.
            RawTarget {
                name: "default".to_string(),
                apply_to_products: vec!["default".to_string()],
                config: None,
                resource: None,
                source: None,
                build_mode_set: vec![],
            },
            // Release-only override target.
            RawTarget {
                name: "release-override".to_string(),
                apply_to_products: vec!["default".to_string()],
                config: None,
                resource: Some(RawTargetResource {
                    directories: vec![PathBuf::from("src/release/resources")],
                }),
                source: None,
                build_mode_set: vec![RawBuildMode {
                    name: "release".to_string(),
                }],
            },
        ];

        // Debug mode → picks "default" (release-override has release-only buildModeSet).
        let debug_target =
            pick_effective_raw_target(&targets, "default", "debug").expect("debug target");
        assert_eq!(debug_target.name, "default");

        // Release mode → picks "release-override" (matches product + mode).
        let release_target =
            pick_effective_raw_target(&targets, "default", "release").expect("release target");
        assert_eq!(release_target.name, "release-override");
    }

    /// Verify `EffectiveTargetConfig` carries the resource directories correctly.
    #[test]
    fn test_effective_target_config_resource_dirs() {
        let targets: Vec<RawTarget> = vec![RawTarget {
            name: "default".to_string(),
            apply_to_products: vec!["default".to_string()],
            config: None,
            resource: Some(RawTargetResource {
                directories: vec![
                    PathBuf::from("src/main/resources"),
                    PathBuf::from("src/extra/resources"),
                ],
            }),
            source: None,
            build_mode_set: vec![],
        }];

        let target = pick_effective_raw_target(&targets, "default", "debug").unwrap();
        let cfg = EffectiveTargetConfig {
            name: target.name.clone(),
            resource_directories: target
                .resource
                .as_ref()
                .map(|r| r.directories.clone())
                .unwrap_or_default(),
            source_abilities: vec![],
            source_roots: vec![],
            distribution_filter: None,
            build_mode_set: vec![],
        };

        assert_eq!(cfg.name, "default");
        assert_eq!(cfg.resource_directories.len(), 2);
        assert_eq!(
            cfg.resource_directories[0],
            PathBuf::from("src/main/resources")
        );
        assert_eq!(
            cfg.resource_directories[1],
            PathBuf::from("src/extra/resources")
        );
    }

    /// Verify that product-level `versionCode` and `versionName` are parsed
    /// correctly from `build-profile.json5`.  These fields are optional; when
    /// absent the product carries `None` so the resolver falls back to
    /// `AppScope/app.json5`.
    #[test]
    fn test_product_version_override_parsed() {
        let raw: RawBuildProfile = json5::from_str(
            r#"{
            app: {
                products: [
                    {
                        name: "default",
                        bundleName: "com.example.app",
                        compatibleSdkVersion: "5.0.0(12)",
                        runtimeOS: "HarmonyOS",
                        versionCode: 2000000,
                        versionName: "2.0.0",
                        buildVersion: "2026.07.23",
                    }
                ],
                signingConfigs: []
            },
            modules: []
        }"#,
        )
        .unwrap();

        let product = &raw.app.products[0];
        assert_eq!(product.version_code, Some(2000000));
        assert_eq!(product.version_name.as_deref(), Some("2.0.0"));
        assert_eq!(product.build_version.as_deref(), Some("2026.07.23"));
    }

    /// Verify that when no product-level version override is present, the
    /// fields are `None` so the resolver falls back to `AppScope/app.json5`.
    #[test]
    fn test_product_version_override_absent_is_none() {
        let raw: RawBuildProfile = json5::from_str(
            r#"{
            app: {
                products: [
                    {
                        name: "default",
                        bundleName: "com.example.app",
                        compatibleSdkVersion: "5.0.0(12)",
                        runtimeOS: "HarmonyOS",
                    }
                ],
                signingConfigs: []
            },
            modules: []
        }"#,
        )
        .unwrap();

        let product = &raw.app.products[0];
        assert_eq!(product.version_code, None);
        assert_eq!(product.version_name, None);
        assert_eq!(product.build_version, None);
    }

    #[test]
    fn effective_module_build_option_matches_hvigor_layer_precedence() {
        let temp = tempfile::tempdir().unwrap();
        let project_root = temp.path();
        let module_root = project_root.join("entry");
        fs_err::create_dir_all(&module_root).unwrap();
        fs_err::write(
            project_root.join("build-profile.json5"),
            r#"{
                app: {
                    buildModeSet: [{
                        name: "debug",
                        buildOption: {
                            preloadSystemSo: true,
                            resOptions: {
                                compression: {
                                    filters: [{ files: { path: ["project-filter"] } }]
                                }
                            }
                        }
                    }],
                    products: [{
                        name: "default",
                        buildOption: {
                            strictMode: {
                                caseSensitiveCheck: true,
                                useNormalizedOHMUrl: false
                            },
                            arkOptions: {
                                runtimeOnly: { packages: ["product-runtime"] },
                                supportChunks: false
                            },
                            resOptions: {
                                compression: {
                                    filters: [{ files: { path: ["product-filter"] } }]
                                }
                            }
                        }
                    }]
                },
                modules: [{
                    name: "entry",
                    srcPath: "./entry",
                    targets: [{ name: "phone", applyToProducts: ["default"] }]
                }]
            }"#,
        )
        .unwrap();
        fs_err::write(
            module_root.join("build-profile.json5"),
            r#"{
                buildOption: {
                    preloadSystemSo: true,
                    packOptions: { deduplicateHar: true },
                    packingOptions: {
                        customizedOptions: { basePackage: "./base.abc" }
                    },
                    nativeLib: {
                        excludeSoFromBinXO: ["**/libskip.so"]
                    },
                    resOptions: {
                        compression: {
                            filters: [{ files: { path: ["module-filter"] } }]
                        }
                    }
                },
                buildOptionSet: [{
                    name: "base",
                    sourceOption: { workers: ["workers/base.ets"] },
                    resOptions: {
                        compression: {
                            filters: [{ files: { path: ["base-filter"] } }]
                        }
                    }
                }, {
                    name: "phone-debug",
                    copyFrom: "base",
                    arkOptions: { supportChunks: true },
                    resOptions: {
                        compression: {
                            filters: [{ files: { path: ["selected-filter"] } }]
                        }
                    }
                }],
                buildModeBinder: [{
                    buildModeName: "debug",
                    mappings: [{
                        targetName: "phone",
                        buildOptionName: "phone-debug"
                    }]
                }],
                targets: [{
                    name: "phone",
                    config: {
                        buildOption: {
                            preloadSystemSo: false,
                            strictMode: { caseSensitiveCheck: false },
                            resOptions: {
                                compression: {
                                    filters: [{ files: { path: ["target-filter"] } }]
                                }
                            }
                        }
                    }
                }]
            }"#,
        )
        .unwrap();

        let modules = vec![arkdown_core::ModuleConfig {
            name: "entry".to_string(),
            src_path: module_root.clone(),
            module_type: arkdown_core::OhosModuleType::Entry,
        }];
        let inventory = BuildOptionProfileInventory::load(project_root, &modules).unwrap();
        let option = EffectiveModuleBuildOptionResolver::new(
            &inventory, "entry", "default", "debug", "phone",
        )
        .resolve()
        .unwrap();

        assert!(!option.preload_system_so);
        assert!(option.deduplicate_har);
        assert!(!option.strict_mode.case_sensitive_check);
        assert!(!option.strict_mode.use_normalized_ohm_url);
        assert_eq!(
            option.customized_har_base_package.as_deref(),
            Some(module_root.join("base.abc").as_path())
        );
        assert_eq!(option.native_lib.exclude_so_from_binxo, ["**/libskip.so"]);
        let ark_options = option.ark_options.unwrap();
        assert!(ark_options.support_chunks);
        assert_eq!(
            ark_options.runtime_only.unwrap().packages,
            ["product-runtime"]
        );
        assert_eq!(
            option.source_option.unwrap().workers,
            [module_root.join("workers/base.ets")]
        );
        let filters = option
            .res_options
            .unwrap()
            .compression
            .unwrap()
            .get("filters")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap();
        let paths = filters
            .iter()
            .map(|filter| filter["files"]["path"][0].as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            [
                module_root.join("target-filter"),
                module_root.join("selected-filter"),
                module_root.join("base-filter"),
                module_root.join("module-filter"),
                project_root.join("product-filter"),
                project_root.join("project-filter"),
            ]
            .map(|path| path.to_string_lossy().into_owned())
        );

        let script_option = EffectiveModuleBuildOptionResolver::new(
            &inventory, "entry", "default", "debug", "phone",
        )
        .with_build_script_override(serde_json::json!({
            "preloadSystemSo": true,
            "strictMode": { "caseSensitiveCheck": true },
            "arkOptions": { "supportChunks": false }
        }))
        .resolve()
        .unwrap();
        assert!(script_option.preload_system_so);
        assert!(script_option.strict_mode.case_sensitive_check);
        assert!(!script_option.ark_options.as_ref().unwrap().support_chunks);
        assert_eq!(
            script_option
                .ark_options
                .as_ref()
                .unwrap()
                .runtime_only
                .as_ref()
                .unwrap()
                .packages,
            ["product-runtime"]
        );
    }
}
