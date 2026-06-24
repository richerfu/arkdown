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
    /// Kept for backward compatibility (old format had bundleName in product).
    /// Real OHOS projects put bundleName in AppScope/app.json5 instead.
    #[serde(default)]
    pub bundle_name: String,
    #[serde(default)]
    pub compatible_sdk_version: String,
    #[serde(default)]
    pub compatible_sdk_version_stage: Option<String>,
    /// Real OHOS projects use targetSdkVersion; takes priority over compatibleSdkVersion.
    #[serde(default)]
    pub target_sdk_version: Option<String>,
    #[serde(default)]
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawBuildOption {
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
}

/// `buildOption.externalNativeOptions` — controls native/CMake build settings.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawExternalNativeOptions {
    /// ABI filter list (e.g. `["arm64-v8a", "armeabi-v7a", "x86_64"]`).
    /// When empty, defaults to `["arm64-v8a"]` (hvigor default).
    #[serde(default)]
    pub abi_filters: Vec<String>,
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
    pub runtime_only: Option<RawRuntimeOnly>,
    #[serde(default)]
    pub support_chunks: Option<bool>,
    #[serde(rename = "aotCompileMode", default)]
    pub aot_compile_mode: Option<String>,
    #[serde(rename = "apPath", default)]
    pub ap_path: Option<std::path::PathBuf>,
    #[serde(rename = "hostPGO", default)]
    pub host_pgo: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawRuntimeOnly {
    #[serde(default)]
    pub packages: Vec<String>,
    #[serde(default)]
    pub sources: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawObfuscation {
    pub rule_options: Option<RawRuleOptions>,
}

#[derive(Debug, Deserialize)]
pub struct RawRuleOptions {
    #[serde(default)]
    pub enable: bool,
    /// Paths to rule files (relative to the project root), e.g.
    /// `["./obfuscation-rules.txt"]`.
    #[serde(default)]
    pub rules: Vec<String>,
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
}

/// Source overlay for a target, allowing per-target ability configuration.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawTargetSource {
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
    /// Distribution/distro filter configuration from `target.config`.
    /// `None` when the target has no filter configured.
    pub distribution_filter: Option<serde_json::Value>,
    /// Build modes this target applies to; empty means all modes.
    pub build_mode_set: Vec<String>,
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

    Some(EffectiveTargetConfig {
        name: target.name.clone(),
        resource_directories: target
            .resource
            .as_ref()
            .map(|r| r.directories.clone())
            .unwrap_or_default(),
        source_abilities: target
            .source
            .as_ref()
            .map(|s| s.abilities.iter().cloned().collect())
            .unwrap_or_default(),
        distribution_filter: target.config.as_ref().and_then(|c| {
            c.distribution_filter
                .clone()
                .or_else(|| c.distro_filter.clone())
        }),
        build_mode_set: target
            .build_mode_set
            .iter()
            .map(|m| m.name.clone())
            .collect(),
    })
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
        .map(|target| EffectiveTargetConfig {
            name: target.name.clone(),
            resource_directories: target
                .resource
                .as_ref()
                .map(|r| r.directories.clone())
                .unwrap_or_default(),
            source_abilities: target
                .source
                .as_ref()
                .map(|s| s.abilities.iter().cloned().collect())
                .unwrap_or_default(),
            distribution_filter: target.config.as_ref().and_then(|c| {
                c.distribution_filter
                    .clone()
                    .or_else(|| c.distro_filter.clone())
            }),
            build_mode_set: target
                .build_mode_set
                .iter()
                .map(|m| m.name.clone())
                .collect(),
        })
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
                            obfuscation: {
                                ruleOptions: { enable: false }
                            },
                            runtimeOnly: {
                                packages: ["@ohos/dynamic-feature"],
                                sources: ["src/main/ets/runtime"]
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
    }
}
