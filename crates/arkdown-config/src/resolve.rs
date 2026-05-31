use std::path::Path;

use arkdown_core::{
    ArkOptions, ArkdownConfig, BuildMode, BuildOption, ModuleConfig, ObfuscationConfig,
    OhosModuleType, OutputTarget, ProductConfig, RuntimeOnlyConfig, SigningConfig,
    StrictModeConfig,
};

use crate::build_profile::{load_app_json5, load_build_profile, parse_api_version};
use crate::error::ConfigError;
use crate::module_json::{load_module_json, map_module_type};
use crate::sdk::discover_sdk;

/// Parse the project at `root` and return a fully resolved [`ArkdownConfig`].
///
/// # Parameters
/// - `root` — project root directory (contains `build-profile.json5`)
/// - `mode` — debug or release build
/// - `target` — output artifact type
/// - `product_name` — select a specific product; `None` → first product
/// - `module_name` — restrict to a single module; `None` → all modules
pub fn load_config(
    root: &Path,
    mode: BuildMode,
    target: OutputTarget,
    product_name: Option<&str>,
    module_name: Option<&str>,
) -> Result<ArkdownConfig, ConfigError> {
    // -----------------------------------------------------------------------
    // 1. Parse build-profile.json5 and AppScope/app.json5
    // -----------------------------------------------------------------------
    let profile = load_build_profile(root)?;
    let app_json5 = load_app_json5(root)?;

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
    // 4. Bundle name: AppScope/app.json5 takes priority over product.bundle_name
    // -----------------------------------------------------------------------
    let bundle_name = if !app_json5.app.bundle_name.is_empty() {
        app_json5.app.bundle_name
    } else {
        raw_product.bundle_name.clone()
    };

    // -----------------------------------------------------------------------
    // 5. Build ProductConfig
    // -----------------------------------------------------------------------

    // Extract old-format per-product signing_configs before other field moves.
    let raw_signing_configs_compat = raw_product.signing_configs;

    // Resolve signing config: new format references by name under app.signingConfigs;
    // fall back to old per-product embedded signingConfigs for backward compat.
    let signing_config = raw_product
        .signing_config
        .as_deref()
        .and_then(|name| {
            app_signing_configs
                .iter()
                .find(|sc| sc.name == name)
                .and_then(|sc| sc.material.as_ref())
                .map(|m| SigningConfig {
                    cert_path: m.cert_path.clone().into(),
                    key_alias: m.key_alias.clone(),
                    key_password: m.key_password.clone(),
                    profile_path: m.profile.clone().into(),
                    sign_alg: m.sign_alg.clone(),
                    store_file: m.store_file.clone().into(),
                    store_password: m.store_password.clone(),
                })
        })
        .or_else(|| {
            // Backward compat: old format had signing_configs embedded in products.
            raw_signing_configs_compat
                .into_iter()
                .next()
                .and_then(|sc| sc.material)
                .map(|m| SigningConfig {
                    cert_path: m.cert_path.into(),
                    key_alias: m.key_alias,
                    key_password: m.key_password,
                    profile_path: m.profile.into(),
                    sign_alg: m.sign_alg,
                    store_file: m.store_file.into(),
                    store_password: m.store_password,
                })
        });

    let strict_mode = raw_product
        .build_option
        .as_ref()
        .and_then(|bo| bo.strict_mode.as_ref())
        .map(|sm| StrictModeConfig {
            case_sensitive_check: sm.case_sensitive_check,
            use_normalized_ohm_url: sm.use_normalized_ohm_url,
        })
        .unwrap_or(StrictModeConfig {
            case_sensitive_check: false,
            use_normalized_ohm_url: false,
        });

    let ark_options = raw_product
        .build_option
        .as_ref()
        .and_then(|bo| bo.ark_options.as_ref())
        .map(|ao| {
            let obfuscation = ao.obfuscation.as_ref().map(|o| {
                let enable = o.rule_options.as_ref().map(|r| r.enable).unwrap_or(false);
                // Resolve rule file paths relative to the project root.
                let rule_files: Vec<std::path::PathBuf> = o
                    .rule_options
                    .as_ref()
                    .map(|r| {
                        r.rules
                            .iter()
                            .map(|p| {
                                let p = std::path::Path::new(p);
                                if p.is_absolute() {
                                    p.to_path_buf()
                                } else {
                                    root.join(p)
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                ObfuscationConfig {
                    enable_obfuscation: enable,
                    rule_files,
                }
            });
            let runtime_only = ao
                .runtime_only
                .as_ref()
                .map(|runtime_only| RuntimeOnlyConfig {
                    packages: runtime_only.packages.clone(),
                    sources: runtime_only.sources.clone(),
                });
            ArkOptions {
                obfuscation,
                types: Vec::new(),
                runtime_only,
            }
        });

    let preload_so = raw_product
        .build_option
        .as_ref()
        .map(|bo| bo.preload_so.clone())
        .unwrap_or_default();

    let build_option = BuildOption {
        strict_mode,
        ark_options,
        res_options: None,
        source_option: None,
        preload_so,
    };

    let product = ProductConfig {
        name: raw_product.name,
        bundle_name,
        compatible_sdk_version: raw_product.compatible_sdk_version.clone(),
        runtime_os: raw_product.runtime_os,
        signing_config,
        build_option,
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
        let module_root = root.join(&raw_mod.src_path);

        // module.json5 is optional — if absent we fall back to Entry.
        let module_type = match load_module_json(&module_root) {
            Ok(mj) => map_module_type(&mj.module.module_type),
            Err(ConfigError::FileNotFound(_)) => arkdown_core::OhosModuleType::Entry,
            Err(e) => return Err(e),
        };

        if module_name.is_none() && !module_matches_output_target(&module_type, &target) {
            continue;
        }

        modules.push(ModuleConfig {
            name: raw_mod.name,
            src_path: module_root,
            module_type,
        });
    }

    // -----------------------------------------------------------------------
    // 7. Discover SDK
    // -----------------------------------------------------------------------
    // targetSdkVersion (new format) takes priority over compatibleSdkVersion.
    let sdk_version_str = raw_product
        .target_sdk_version
        .as_deref()
        .unwrap_or(&raw_product.compatible_sdk_version);
    let compile_api_version = parse_api_version(sdk_version_str).unwrap_or(12);

    let sdk_config = discover_sdk(compile_api_version)?;

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

fn module_matches_output_target(module_type: &OhosModuleType, target: &OutputTarget) -> bool {
    match target {
        OutputTarget::Hap => {
            matches!(module_type, OhosModuleType::Entry | OhosModuleType::Feature)
        }
        OutputTarget::Hsp => matches!(module_type, OhosModuleType::Shared),
        OutputTarget::Har => matches!(module_type, OhosModuleType::Har),
        OutputTarget::Preview | OutputTarget::Test => true,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

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
                assert_eq!(sc.key_alias, "testKey");
                assert_eq!(sc.cert_path.to_str().unwrap(), "/test/cert.cer");
                // targetSdkVersion used for compile_api_version
                assert_eq!(cfg.sdk_config.compile_api_version, 17);
            }
            Err(ConfigError::SdkNotFound) => {
                // Acceptable on machines without DevEco Studio
            }
            Err(e) => panic!("unexpected error: {e}"),
        }

        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// Verify that `ruleOptions.rules` paths are resolved relative to the project
    /// root and stored in `ObfuscationConfig.rule_files`.
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
                            obfuscation: {
                                ruleOptions: {
                                    enable: true,
                                    rules: ["./obfuscation-rules.txt", "./extra-rules.txt"]
                                }
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
                let runtime_only = ark_opts.runtime_only.as_ref().expect("runtimeOnly present");
                assert_eq!(runtime_only.packages, vec!["@ohos/runtime-only"]);
                assert_eq!(runtime_only.sources, vec!["src/main/ets/runtime"]);
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
        assert!(module_matches_output_target(
            &arkdown_core::OhosModuleType::Entry,
            &OutputTarget::Hap
        ));
        assert!(module_matches_output_target(
            &arkdown_core::OhosModuleType::Feature,
            &OutputTarget::Hap
        ));
        assert!(!module_matches_output_target(
            &arkdown_core::OhosModuleType::Har,
            &OutputTarget::Hap
        ));
        assert!(module_matches_output_target(
            &arkdown_core::OhosModuleType::Har,
            &OutputTarget::Har
        ));
        assert!(module_matches_output_target(
            &arkdown_core::OhosModuleType::Shared,
            &OutputTarget::Hsp
        ));
    }
}
