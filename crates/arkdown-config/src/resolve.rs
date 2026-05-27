use std::path::Path;

use arkdown_core::{
    ArkdownConfig, ArkOptions, BuildMode, BuildOption, ModuleConfig, ObfuscationConfig,
    OutputTarget, ProductConfig, SigningConfig, StrictModeConfig,
};

use crate::build_profile::{load_build_profile, parse_api_version};
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
    // 1. Parse build-profile.json5
    // -----------------------------------------------------------------------
    let profile = load_build_profile(root)?;

    // -----------------------------------------------------------------------
    // 2. Select product
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
    // 3. Build ProductConfig
    // -----------------------------------------------------------------------
    let signing_config = raw_product
        .signing_configs
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
            let obfuscation = ao.obfuscation.as_ref().map(|o| ObfuscationConfig {
                enable_obfuscation: o
                    .rule_options
                    .as_ref()
                    .map(|r| r.enable)
                    .unwrap_or(false),
                rules_file: None,
            });
            ArkOptions {
                obfuscation,
                types: Vec::new(),
                runtime_only: None,
            }
        });

    let build_option = BuildOption {
        strict_mode,
        ark_options,
        res_options: None,
        source_option: None,
    };

    let product = ProductConfig {
        name: raw_product.name,
        bundle_name: raw_product.bundle_name,
        compatible_sdk_version: raw_product.compatible_sdk_version.clone(),
        runtime_os: raw_product.runtime_os,
        signing_config,
        build_option,
    };

    // -----------------------------------------------------------------------
    // 4. Filter & build ModuleConfig list
    // -----------------------------------------------------------------------
    let raw_modules: Vec<_> = match module_name {
        Some(name) => profile
            .modules
            .into_iter()
            .filter(|m| m.name == name)
            .collect(),
        None => profile.modules,
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

        modules.push(ModuleConfig {
            name: raw_mod.name,
            src_path: module_root,
            module_type,
        });
    }

    // -----------------------------------------------------------------------
    // 5. Discover SDK
    // -----------------------------------------------------------------------
    let compile_api_version =
        parse_api_version(&raw_product.compatible_sdk_version).unwrap_or(12);

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

    /// Smoke test: `load_config` with a valid temp project directory.
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
        let result = load_config(
            &tmp,
            BuildMode::Debug,
            OutputTarget::Hap,
            None,
            None,
        );

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
}
