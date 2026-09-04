use std::path::{Path, PathBuf};

use arkdown_core::{
    AppOptionOverride, ArkdownConfig, BuildMode, ModuleConfig, OhosModuleType, OutputTarget,
    ProductConfig, SigningConfig,
};

use crate::build_profile::{
    parse_api_version, BuildOptionDecoder, RawAppJson5, RawBuildProfile, RawSigningMaterial,
};
use crate::error::ConfigError;
use crate::module_json::RawModuleJson;
use crate::sdk::{discover_sdk_with_selection, SdkSelection};
use serde::Deserialize;

/// Owns one complete configuration load from the project model on disk.
pub struct ArkdownConfigLoader<'a> {
    root: &'a Path,
    mode: BuildMode,
    target: OutputTarget,
    product_name: Option<&'a str>,
    module_name: Option<&'a str>,
}

/// Resolves signing material at the same project boundary as Hvigor.
///
/// `SignUtil.getSigningConfigInputFiles` passes every configured material path
/// through `FileUtil.convertToAbsolutePath(path, projectDir)`. Keeping that
/// conversion in the config owner makes signing independent of the caller's
/// current working directory.
struct SigningConfigDecoder<'a> {
    project_root: &'a Path,
}

impl<'a> SigningConfigDecoder<'a> {
    fn new(project_root: &'a Path) -> Self {
        Self { project_root }
    }

    fn decode(&self, name: Option<&str>, material: &RawSigningMaterial) -> SigningConfig {
        SigningConfig {
            name: name.map(str::to_string),
            cert_path: self.path(&material.cert_path),
            key_alias: material.key_alias.clone(),
            key_password: material.key_password.clone(),
            profile_path: self.path(&material.profile),
            sign_alg: material.sign_alg.clone(),
            store_file: self.path(&material.store_file),
            store_password: material.store_password.clone(),
        }
    }

    fn path(&self, configured: &str) -> PathBuf {
        let path = PathBuf::from(configured);
        if path.has_root() {
            arkdown_path::lexical_normalize(&path)
        } else {
            arkdown_path::lexical_normalize(&self.project_root.join(path))
        }
    }
}

impl<'a> ArkdownConfigLoader<'a> {
    pub fn new(
        root: &'a Path,
        mode: BuildMode,
        target: OutputTarget,
        product_name: Option<&'a str>,
        module_name: Option<&'a str>,
    ) -> Self {
        Self {
            root,
            mode,
            target,
            product_name,
            module_name,
        }
    }

    /// Parse and resolve the complete project configuration.
    pub fn load(self) -> Result<ArkdownConfig, ConfigError> {
        let Self {
            root,
            mode,
            target,
            product_name,
            module_name,
        } = self;
        if !root.is_dir() {
            return Err(ConfigError::FileNotFound(root.join("build-profile.json5")));
        }
        let root = arkdown_path::AbsPath::canonicalize(root)?.into_path_buf();
        let root = root.as_path();

        // -----------------------------------------------------------------------
        // 1. Parse build-profile.json5 and AppScope/app.json5
        // -----------------------------------------------------------------------
        let profile = RawBuildProfile::load(root)?;
        let app_json5 = RawAppJson5::load(root)?;

        // -----------------------------------------------------------------------
        // 2. Destructure app-level fields before consuming products
        // -----------------------------------------------------------------------
        // We destructure here so we can access both products and app.signing_configs
        // without partial-move issues.
        let app_signing_configs = profile.app.signing_configs;
        let raw_modules_list = profile.modules;

        // -----------------------------------------------------------------------
        // 3. Select product
        // -----------------------------------------------------------------------
        let raw_product = match product_name {
            Some(name) => profile
                .app
                .products
                .into_iter()
                .find(|p| p.name == name)
                .ok_or_else(|| ConfigError::ProductNotFound(name.to_string()))?,
            None => profile
                .app
                .products
                .into_iter()
                .next()
                .ok_or_else(|| ConfigError::ProductNotFound("<first>".to_string()))?,
        };

        // -----------------------------------------------------------------------
        // 4. Bundle name: product takes priority over AppScope/app.json5.
        //
        // Upstream: AbstractMergeProfile constructor resolves
        // hvigorfile appOpt.bundleName → product.bundleName → app.json5.
        // -----------------------------------------------------------------------
        let bundle_name = if !raw_product.bundle_name.is_empty() {
            raw_product.bundle_name.clone()
        } else {
            app_json5.app.bundle_name
        };

        // -----------------------------------------------------------------------
        // 5. Build ProductConfig
        // -----------------------------------------------------------------------

        // Extract old-format per-product signing_configs before other field moves.
        let raw_signing_configs_compat = raw_product.signing_configs;

        // Resolve signing config: new format references by name under app.signingConfigs;
        // fall back to old per-product embedded signingConfigs for backward compat.
        let signing_config_decoder = SigningConfigDecoder::new(root);
        let signing_config = raw_product
            .signing_config
            .as_deref()
            .and_then(|name| {
                app_signing_configs
                    .iter()
                    .find(|sc| sc.name == name)
                    .and_then(|sc| sc.material.as_ref())
                    .map(|material| signing_config_decoder.decode(Some(name), material))
            })
            .or_else(|| {
                // Backward compat: old format had signing_configs embedded in products.
                raw_signing_configs_compat
                    .into_iter()
                    .next()
                    .and_then(|sc| sc.material.map(|material| (sc.name, material)))
                    .map(|(name, material)| signing_config_decoder.decode(Some(&name), &material))
            });

        let build_option = BuildOptionDecoder::new(
            raw_product.build_option.as_ref(),
            root,
            &root.join("build-profile.json5"),
        )
        .decode()?;

        let product = ProductConfig {
            name: raw_product.name,
            artifact_name: raw_product
                .output
                .and_then(|output| output.artifact_name)
                .filter(|name| !name.is_empty()),
            bundle_name,
            compatible_sdk_version: raw_product.compatible_sdk_version.clone(),
            compatible_sdk_version_stage: raw_product.compatible_sdk_version_stage.clone(),
            target_sdk_version: raw_product.target_sdk_version.clone(),
            runtime_os: raw_product.runtime_os,
            signing_config,
            build_option,
            version_code: raw_product.version_code,
            version_name: raw_product.version_name,
            build_version: raw_product.build_version,
            app_options: AppOptionOverride {
                vendor: raw_product.vendor,
                bundle_type: raw_product.bundle_type,
                icon: raw_product.icon,
                label: raw_product.label,
            },
        };

        // -----------------------------------------------------------------------
        // 6. Filter & build ModuleConfig list
        // -----------------------------------------------------------------------
        let raw_modules: Vec<_> = match module_name {
            Some(name) => raw_modules_list
                .into_iter()
                .filter(|m| m.name == name)
                .collect(),
            None => raw_modules_list,
        };

        let mut modules: Vec<ModuleConfig> = Vec::with_capacity(raw_modules.len());
        for raw_mod in raw_modules {
            // Hvigor resolves descriptor `srcPath` values before exposing the
            // module directory to downstream tasks. Preserve the canonical
            // project identity while removing lexical `.`/`..` components so
            // plugin and SDK-facing projectConfig paths are stable.
            let module_root = arkdown_path::lexical_normalize(&root.join(&raw_mod.src_path));

            // module.json5 is optional — if absent we fall back to Entry.
            let module_type = match RawModuleJson::load(&module_root) {
                Ok(mj) => OhosModuleType::from_type_str(&mj.module.module_type),
                Err(ConfigError::FileNotFound(_)) => arkdown_core::OhosModuleType::Entry,
                Err(e) => return Err(e),
            };

            modules.push(ModuleConfig {
                name: raw_mod.name,
                src_path: module_root,
                module_type,
            });
        }

        // -----------------------------------------------------------------------
        // 7. Discover SDK
        // -----------------------------------------------------------------------
        // Explicit targetSdkVersion overrides the SDK component apiVersion. When
        // targetSdkVersion is absent, hvigor compiles for the selected SDK API.
        let explicit_target_api_version = raw_product
            .target_sdk_version
            .as_deref()
            .and_then(parse_api_version);
        let hvigor_sdk_cache = HvigorSdkCache::load(root);
        let target_api_version = explicit_target_api_version.or_else(|| {
            hvigor_sdk_cache
                .as_ref()
                .and_then(|cache| cache.compile_api_version)
        });
        let sdk_config = discover_sdk_with_selection(SdkSelection {
            target_api_version,
            compile_sdk_version: hvigor_sdk_cache
                .as_ref()
                .and_then(|cache| cache.toolchains_version.clone()),
            preferred_sdk_root: hvigor_sdk_cache
                .as_ref()
                .and_then(|cache| cache.preferred_sdk_root.clone()),
        })?;

        // -----------------------------------------------------------------------
        // 6. Assemble and return
        // -----------------------------------------------------------------------
        Ok(ArkdownConfig {
            project_root: root.to_path_buf(),
            build_mode: mode,
            output_target: target,
            product,
            modules,
            sdk_config,
        })
    }
}

/// Parse the project at `root` and return a fully resolved [`ArkdownConfig`].
pub fn load_config(
    root: &Path,
    mode: BuildMode,
    target: OutputTarget,
    product_name: Option<&str>,
    module_name: Option<&str>,
) -> Result<ArkdownConfig, ConfigError> {
    ArkdownConfigLoader::new(root, mode, target, product_name, module_name).load()
}

#[derive(Debug)]
struct HvigorSdkCache {
    compile_api_version: Option<u32>,
    toolchains_version: Option<String>,
    preferred_sdk_root: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawHvigorCacheMeta {
    #[serde(default)]
    tool_chains_version: Option<String>,
    #[serde(default)]
    compile_sdk_version: Option<String>,
}

/// SDK selection materialized by hvigor's ArkTS task configuration.
///
/// `sdkPath` is the SDK distribution root while `etsLoaderPath` identifies
/// the exact OpenHarmony component. Both are emitted with host-native path
/// syntax, so arkdown keeps them as `PathBuf` and never rewrites separators.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawHvigorProjectConfig {
    #[serde(default)]
    sdk_path: Option<PathBuf>,
    #[serde(default)]
    ets_loader_path: Option<PathBuf>,
}

impl RawHvigorProjectConfig {
    fn existing_sdk_root(self, project_root: &Path) -> Option<PathBuf> {
        self.sdk_path
            .and_then(|path| Self::existing_path(project_root, path))
            .or_else(|| {
                self.ets_loader_path
                    .and_then(|path| Self::existing_path(project_root, path))
                    .and_then(|loader| {
                        loader
                            .ancestors()
                            .find(|ancestor| ancestor.file_name().is_some_and(|name| name == "ets"))
                            .and_then(Path::parent)
                            .map(Path::to_path_buf)
                    })
            })
    }

    fn existing_path(project_root: &Path, path: PathBuf) -> Option<PathBuf> {
        let path = if path.is_absolute() {
            path
        } else {
            project_root.join(path)
        };
        path.is_dir().then_some(path)
    }
}

impl HvigorSdkCache {
    fn load(project_root: &Path) -> Option<Self> {
        let cache_dir = project_root.join(".hvigor").join("cache");
        let contents = fs_err::read_to_string(cache_dir.join("meta.json")).ok()?;
        let raw = serde_json::from_str::<RawHvigorCacheMeta>(&contents).ok()?;
        let compile_api_version = raw
            .compile_sdk_version
            .as_deref()
            .and_then(parse_api_version);
        let preferred_sdk_root = fs_err::read_to_string(cache_dir.join("project-config.json"))
            .ok()
            .and_then(|contents| serde_json::from_str::<RawHvigorProjectConfig>(&contents).ok())
            .and_then(|config| config.existing_sdk_root(project_root));
        Some(Self {
            compile_api_version,
            toolchains_version: raw.tool_chains_version.filter(|value| !value.is_empty()),
            preferred_sdk_root,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signing_material_paths_follow_hvigor_project_directory_resolution() {
        let project_root = std::env::temp_dir().join("arkdown-signing-material");
        let material = RawSigningMaterial {
            cert_path: "./SignFile/debug.cer".to_string(),
            key_alias: "debug".to_string(),
            key_password: "key-password".to_string(),
            profile: "SignFile/debug.p7b".to_string(),
            sign_alg: "SHA256withECDSA".to_string(),
            store_file: "./SignFile/debug.p12".to_string(),
            store_password: "store-password".to_string(),
        };

        let signing = SigningConfigDecoder::new(&project_root).decode(Some("default"), &material);

        assert_eq!(signing.name.as_deref(), Some("default"));
        assert_eq!(signing.cert_path, project_root.join("SignFile/debug.cer"));
        assert_eq!(
            signing.profile_path,
            project_root.join("SignFile/debug.p7b")
        );
        assert_eq!(signing.store_file, project_root.join("SignFile/debug.p12"));
    }

    /// Smoke test: `load_config` on a non-existent root returns `FileNotFound`.
    #[test]
    fn test_load_config_missing_root() {
        let result = load_config(
            Path::new("/nonexistent/project"),
            BuildMode::Debug,
            OutputTarget::Hap,
            None,
            None,
        );
        assert!(
            matches!(result, Err(ConfigError::FileNotFound(_))),
            "expected FileNotFound, got: {result:?}"
        );
    }

    #[test]
    fn test_hvigor_sdk_cache_loads_toolchain_api_selection() {
        let tmp =
            std::env::temp_dir().join(format!("arkdown_hvigor_sdk_cache_{}", std::process::id()));
        let cache_dir = tmp.join(".hvigor").join("cache");
        std::fs::create_dir_all(&cache_dir).expect("create hvigor cache dir");
        std::fs::write(
            cache_dir.join("meta.json"),
            r#"{"toolChainsVersion":"6.0.0.47","compileSdkVersion":"6.0.0(20)"}"#,
        )
        .expect("write hvigor cache meta");
        let sdk_root = tmp.join("selected-sdk");
        std::fs::create_dir_all(&sdk_root).expect("create selected SDK root");
        std::fs::write(
            cache_dir.join("project-config.json"),
            serde_json::json!({ "sdkPath": sdk_root }).to_string(),
        )
        .expect("write hvigor project config");

        let cache = HvigorSdkCache::load(&tmp).expect("load hvigor sdk cache");

        assert_eq!(cache.compile_api_version, Some(20));
        assert_eq!(cache.toolchains_version.as_deref(), Some("6.0.0.47"));
        assert_eq!(
            cache.preferred_sdk_root.as_deref(),
            Some(sdk_root.as_path())
        );

        let _ = std::fs::remove_dir_all(tmp);
    }

    /// Smoke test: `load_config` with a valid temp project directory (old format, bundleName in product).
    #[test]
    fn test_load_config_temp_project() {
        use std::io::Write;

        let tmp = std::env::temp_dir().join("arkdown_config_test_project");
        std::fs::create_dir_all(&tmp).unwrap();

        // Write build-profile.json5
        let bp_content = r#"{
            app: {
                products: [{
                    name: "default",
                    bundleName: "com.example.testapp",
                    compatibleSdkVersion: "5.0.0(12)",
                    runtimeOS: "HarmonyOS",
                }],
            },
            modules: [{
                name: "entry",
                srcPath: "./entry",
            }]
        }"#;
        let mut f = std::fs::File::create(tmp.join("build-profile.json5")).unwrap();
        f.write_all(bp_content.as_bytes()).unwrap();

        // No module.json5 — should fall back to Entry type, and SDK may or may not be found.
        let result = load_config(&tmp, BuildMode::Debug, OutputTarget::Hap, None, None);

        match result {
            Ok(cfg) => {
                assert_eq!(cfg.product.bundle_name, "com.example.testapp");
                assert_eq!(cfg.modules.len(), 1);
                assert_eq!(cfg.modules[0].name, "entry");
                assert_eq!(
                    cfg.modules[0].src_path,
                    std::fs::canonicalize(&tmp).unwrap().join("entry")
                );
                assert_eq!(
                    cfg.modules[0].module_type,
                    arkdown_core::OhosModuleType::Entry
                );
            }
            Err(ConfigError::SdkNotFound) => {
                // Acceptable on machines without DevEco Studio
            }
            Err(e) => panic!("unexpected error: {e}"),
        }

        // Cleanup
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// load_config with real OHOS format: bundleName from AppScope/app.json5,
    /// signingConfigs at app level, product references signing by name.
    #[test]
    fn test_load_config_real_format_appscope_bundle_name() {
        use std::io::Write;

        let tmp = std::env::temp_dir().join("arkdown_config_real_format_test");
        std::fs::create_dir_all(tmp.join("AppScope")).unwrap();

        // AppScope/app.json5
        let app_content = r#"{
            "app": {
                "bundleName": "com.example.realapp",
                "vendor": "example",
                "versionCode": 1000000,
                "versionName": "1.0.0"
            }
        }"#;
        let mut f = std::fs::File::create(tmp.join("AppScope/app.json5")).unwrap();
        f.write_all(app_content.as_bytes()).unwrap();

        // build-profile.json5 — no bundleName in product, signingConfigs at app level
        let bp_content = r#"{
            app: {
                signingConfigs: [
                    {
                        name: "default",
                        type: "HarmonyOS",
                        material: {
                            certpath: "/test/cert.cer",
                            keyAlias: "testKey",
                            keyPassword: "enc_kp",
                            profile: "/test/debug.p7b",
                            signAlg: "SHA256withECDSA",
                            storeFile: "/test/debug.p12",
                            storePassword: "enc_sp"
                        }
                    }
                ],
                products: [{
                    name: "default",
                    signingConfig: "default",
                    targetSdkVersion: "5.0.5(17)",
                    compatibleSdkVersion: "5.0.5(17)",
                    runtimeOS: "HarmonyOS",
                }],
                buildModeSet: [{ name: "debug" }, { name: "release" }]
            },
            modules: [{
                name: "entry",
                srcPath: "./entry",
            }]
        }"#;
        let mut f = std::fs::File::create(tmp.join("build-profile.json5")).unwrap();
        f.write_all(bp_content.as_bytes()).unwrap();

        let result = load_config(&tmp, BuildMode::Debug, OutputTarget::Hap, None, None);

        match result {
            Ok(cfg) => {
                // bundleName comes from AppScope/app.json5
                assert_eq!(cfg.product.bundle_name, "com.example.realapp");
                // Signing config resolved by name
                let sc = cfg
                    .product
                    .signing_config
                    .as_ref()
                    .expect("signing config present");
                assert_eq!(sc.name.as_deref(), Some("default"));
                assert_eq!(sc.key_alias, "testKey");
                assert_eq!(sc.cert_path.to_str().unwrap(), "/test/cert.cer");
                // targetSdkVersion used for compile_api_version
                assert_eq!(cfg.sdk_config.compile_api_version, 17);
                assert_eq!(cfg.product.target_sdk_version.as_deref(), Some("5.0.5(17)"));
            }
            Err(ConfigError::SdkNotFound) => {
                // Acceptable on machines without DevEco Studio
            }
            Err(e) => panic!("unexpected error: {e}"),
        }

        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// Verify that `ruleOptions.rules` and `consumerFiles` paths are resolved
    /// relative to the project root and stored in `ObfuscationConfig`.
    #[test]
    fn test_load_config_obfuscation_rule_files_resolved() {
        use std::io::Write;

        let tmp = std::env::temp_dir().join("arkdown_config_obf_rules_test");
        std::fs::create_dir_all(&tmp).unwrap();

        let bp_content = r#"{
            app: {
                products: [{
                    name: "default",
                    bundleName: "com.example.obftest",
                    compatibleSdkVersion: "5.0.0(12)",
                    runtimeOS: "HarmonyOS",
                    buildOption: {
                        arkOptions: {
                            integratedHsp: true,
                            obfuscation: {
                                ruleOptions: {
                                    enable: true,
                                    rules: ["./obfuscation-rules.txt", "./extra-rules.txt"]
                                },
                                consumerFiles: "./consumer-rules.txt"
                            },
                            runtimeOnly: {
                                packages: ["@ohos/runtime-only"],
                                sources: ["src/main/ets/runtime"]
                            }
                        }
                    }
                }],
            },
            modules: []
        }"#;
        let mut f = std::fs::File::create(tmp.join("build-profile.json5")).unwrap();
        f.write_all(bp_content.as_bytes()).unwrap();

        let result = load_config(&tmp, BuildMode::Release, OutputTarget::Hap, None, None);

        match result {
            Ok(cfg) => {
                let ark_opts = cfg
                    .product
                    .build_option
                    .ark_options
                    .as_ref()
                    .expect("ark_options present");
                let obf = ark_opts.obfuscation.as_ref().expect("obfuscation present");
                assert!(obf.enable_obfuscation);
                assert_eq!(obf.rule_files.len(), 2);
                // Paths must be absolute (resolved against project root).
                assert!(obf.rule_files[0].is_absolute());
                assert!(obf.rule_files[0].ends_with("obfuscation-rules.txt"));
                assert!(obf.rule_files[1].ends_with("extra-rules.txt"));
                assert_eq!(obf.consumer_rule_files.len(), 1);
                assert!(obf.consumer_rule_files[0].is_absolute());
                assert!(obf.consumer_rule_files[0].ends_with("consumer-rules.txt"));
                let runtime_only = ark_opts.runtime_only.as_ref().expect("runtimeOnly present");
                assert_eq!(runtime_only.packages, vec!["@ohos/runtime-only"]);
                assert_eq!(runtime_only.sources, vec!["src/main/ets/runtime"]);
                assert_eq!(ark_opts.integrated_hsp, Some(true));
            }
            Err(ConfigError::SdkNotFound) => {
                // Acceptable on machines without DevEco Studio
            }
            Err(e) => panic!("unexpected error: {e}"),
        }

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_module_type_matches_output_target() {
        use arkdown_core::OhosModuleType;
        assert!(OhosModuleType::Entry.matches_output_target(&OutputTarget::Hap));
        assert!(OhosModuleType::Feature.matches_output_target(&OutputTarget::Hap));
        assert!(!OhosModuleType::Har.matches_output_target(&OutputTarget::Hap));
        assert!(OhosModuleType::Har.matches_output_target(&OutputTarget::Har));
        assert!(OhosModuleType::Shared.matches_output_target(&OutputTarget::Hsp));
    }
}
