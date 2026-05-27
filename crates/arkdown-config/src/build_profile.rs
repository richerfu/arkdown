use std::path::PathBuf;

use serde::Deserialize;

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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawApp {
    pub products: Vec<RawProduct>,
    /// Top-level signing configurations (real OHOS format: under `app`, not per-product).
    #[serde(default)]
    pub signing_configs: Vec<RawSigningConfig>,
    #[serde(default)]
    pub build_mode_set: Vec<RawBuildMode>,
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
pub struct RawArkOptions {
    pub obfuscation: Option<RawObfuscation>,
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
    #[serde(default)]
    pub apply_to_products: Vec<String>,
}

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

/// Parse the contents of `build-profile.json5`.
pub fn parse_build_profile(
    content: &str,
    file: &std::path::Path,
) -> Result<RawBuildProfile, ConfigError> {
    json5::from_str::<RawBuildProfile>(content).map_err(|e| ConfigError::ParseError {
        file: file.to_path_buf(),
        message: e.to_string(),
    })
}

/// Extract the numeric API version from a string like `"5.0.0(12)"` → `12`.
/// Returns `None` if no parenthesised number is found.
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

/// Load and parse a `build-profile.json5` file from the given `root` directory.
pub fn load_build_profile(root: &std::path::Path) -> Result<RawBuildProfile, ConfigError> {
    let path: PathBuf = root.join("build-profile.json5");
    if !path.exists() {
        return Err(ConfigError::FileNotFound(path));
    }
    let content = fs_err::read_to_string(&path)?;
    parse_build_profile(&content, &path)
}

/// Load and parse `AppScope/app.json5` from the project root.
///
/// Returns a default (all-empty) value if the file does not exist, so callers
/// can use it without failing on older-layout projects.
pub fn load_app_json5(root: &std::path::Path) -> Result<RawAppJson5, ConfigError> {
    let path: PathBuf = root.join("AppScope/app.json5");
    if !path.exists() {
        return Ok(RawAppJson5 {
            app: RawAppJson5App::default(),
        });
    }
    let content = fs_err::read_to_string(&path)?;
    json5::from_str(&content).map_err(|e| ConfigError::ParseError {
        file: path,
        message: e.to_string(),
    })
}

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
            parse_build_profile(json, std::path::Path::new("build-profile.json5")).unwrap();
        assert_eq!(profile.app.products.len(), 1);
        assert_eq!(profile.app.products[0].name, "default");
        assert_eq!(profile.app.products[0].bundle_name, "com.example.app");
        assert_eq!(
            parse_api_version(&profile.app.products[0].compatible_sdk_version),
            Some(12)
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
            parse_build_profile(json, std::path::Path::new("build-profile.json5")).unwrap();
        assert_eq!(profile.modules[0].name, "entry");
        assert_eq!(profile.modules[0].src_path, "./entry");
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
            parse_build_profile(json, std::path::Path::new("build-profile.json5")).unwrap();
        assert_eq!(profile.app.products[0].bundle_name, "com.example.myapp");
    }

    #[test]
    fn test_parse_build_profile_missing_required_field_returns_err() {
        // `app` key is missing entirely — required by `RawBuildProfile`
        let json = r#"{ modules: [] }"#;
        let result = parse_build_profile(json, std::path::Path::new("build-profile.json5"));
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
            parse_build_profile(json, std::path::Path::new("build-profile.json5")).unwrap();
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
                            }
                        }
                    }
                }],
            },
            modules: []
        }"#;
        let profile =
            parse_build_profile(json, std::path::Path::new("build-profile.json5")).unwrap();
        let product = &profile.app.products[0];
        assert_eq!(product.signing_configs.len(), 1);
        let material = product.signing_configs[0].material.as_ref().unwrap();
        assert_eq!(material.cert_path, "/path/to/cert");
        assert_eq!(material.sign_alg, "SHA256withECDSA");
        let build_option = product.build_option.as_ref().unwrap();
        let strict = build_option.strict_mode.as_ref().unwrap();
        assert!(strict.case_sensitive_check);
        assert!(strict.use_normalized_ohm_url);
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
            parse_build_profile(json, std::path::Path::new("build-profile.json5")).unwrap();

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

        let result = load_app_json5(&tmp).unwrap();
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

        let app = load_app_json5(&tmp).unwrap();
        assert_eq!(app.app.bundle_name, "com.example.myapplication");
        assert_eq!(app.app.vendor, "example");
        assert_eq!(app.app.version_code, 1000000);
        assert_eq!(app.app.version_name, "1.0.0");

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
