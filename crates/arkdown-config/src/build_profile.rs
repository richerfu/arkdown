use std::path::PathBuf;

use serde::{Deserialize, Deserializer};

use crate::error::ConfigError;

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
        let path: PathBuf = root.join("AppScope/app.json5");
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
        if app.app.bundle_name.is_empty() || Self::is_valid_hvigor_bundle_name(&app.app.bundle_name)
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

    fn is_valid_hvigor_bundle_name(value: &str) -> bool {
        let mut segments = value.split('.');
        let Some(first) = segments.next() else {
            return false;
        };
        if !Self::is_valid_bundle_segment(first, true) {
            return false;
        }

        let mut suffix_count = 0;
        for segment in segments {
            suffix_count += 1;
            if !Self::is_valid_bundle_segment(segment, false) {
                return false;
            }
        }
        suffix_count >= 2
    }

    fn is_valid_bundle_segment(segment: &str, first_segment: bool) -> bool {
        let bytes = segment.as_bytes();
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
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawTargetAbility {
    pub name: String,
    pub icon: Option<String>,
    pub label: Option<String>,
    #[serde(default)]
    pub launch_type: Option<String>,
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
        assert!(RawAppJson5::is_valid_hvigor_bundle_name(
            "com.example.myapplication"
        ));
        assert!(RawAppJson5::is_valid_hvigor_bundle_name("a.b.c"));
        assert!(RawAppJson5::is_valid_hvigor_bundle_name(
            "com.example_1.app2"
        ));

        assert!(!RawAppJson5::is_valid_hvigor_bundle_name("Bundle name"));
        assert!(!RawAppJson5::is_valid_hvigor_bundle_name("com.example"));
        assert!(!RawAppJson5::is_valid_hvigor_bundle_name(
            "1com.example.app"
        ));
        assert!(!RawAppJson5::is_valid_hvigor_bundle_name(
            "com._example.app"
        ));
        assert!(!RawAppJson5::is_valid_hvigor_bundle_name(
            "com.example_.app"
        ));
        assert!(!RawAppJson5::is_valid_hvigor_bundle_name(
            "com.example.app_"
        ));
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
}
