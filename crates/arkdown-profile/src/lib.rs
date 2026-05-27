//! `arkdown-profile` — module.json5 processor and loader.json generator.
//!
//! This crate:
//! 1. Transforms a raw [`ModuleJson5`] into a processed [`HapManifest`]
//!    (sets `virtualMachine`, `compileMode`, maps abilities, etc.).
//! 2. Generates [`LoaderJson`] — the manifest the bundler reads for entry points
//!    and module metadata.
//!
//! ## Primary API
//! Use [`ProfileProcessor`] as the top-level driver:
//! ```rust,ignore
//! let processor = ProfileProcessor::new(Arc::new(config));
//! let (manifest, loader) = processor.process_module(&module_cfg, &module_json5)?;
//! processor.write_loader_json(&loader, Path::new("/module/build/loader.json"))?;
//! ```

pub mod har_abilities;
pub mod loader_json;
pub mod process_profile;
pub mod router_map;

pub use har_abilities::{HarAbility, collect_har_abilities};
pub use loader_json::{ByteCodeHarEntry, LoaderJson, PkgBriefInfo, generate_loader_json};
pub use process_profile::process_profile;
pub use router_map::{RouterMapEntry, collect_router_entries};

use std::path::Path;
use std::sync::Arc;

use arkdown_core::{ArkdownConfig, ArkdownError, CompileMode, HapManifest, ModuleConfig, ModuleJson5, OhosModuleType};

/// Top-level driver for profile processing and loader.json generation.
///
/// Holds a shared reference to the project-wide [`ArkdownConfig`] so it can be
/// reused across multiple modules without cloning.
pub struct ProfileProcessor {
    config: Arc<ArkdownConfig>,
}

impl ProfileProcessor {
    /// Create a new processor for the given project configuration.
    pub fn new(config: Arc<ArkdownConfig>) -> Self {
        ProfileProcessor { config }
    }

    /// Process a single module: transform its `module.json5` into a
    /// [`HapManifest`] and generate a corresponding [`LoaderJson`].
    ///
    /// # Parameters
    /// - `module_cfg` — the module entry from [`ArkdownConfig::modules`]
    /// - `module_json5` — the parsed `module.json5` for this module
    pub fn process_module(
        &self,
        module_cfg: &ModuleConfig,
        module_json5: &ModuleJson5,
    ) -> Result<(HapManifest, LoaderJson), ArkdownError> {
        let manifest = process_profile(
            module_json5,
            self.config.sdk_config.compile_api_version,
            &self.config.product.bundle_name,
            CompileMode::EsModule,
        );
        let loader = generate_loader_json(&self.config, module_cfg, &manifest);
        Ok((manifest, loader))
    }

    /// Serialize `loader` as indented JSON and write it to `path`.
    ///
    /// Creates parent directories if they do not exist.
    pub fn write_loader_json(
        &self,
        loader: &LoaderJson,
        path: &Path,
    ) -> Result<(), ArkdownError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(loader)
            .map_err(|e| ArkdownError::Io(std::io::Error::other(e.to_string())))?;
        fs_err::write(path, json)?;
        Ok(())
    }

    /// Write the processed `module.json` for inclusion in the HAP.
    ///
    /// The output is a standard JSON file (not JSON5) combining:
    /// - `app` section from `AppScope/app.json5` (falls back to config values)
    /// - `module` section from the processed manifest (with `virtualMachine` and `compileMode`)
    pub fn write_module_json(
        &self,
        manifest: &HapManifest,
        output_path: &Path,
    ) -> Result<(), ArkdownError> {
        // Read AppScope/app.json5 for the app section
        let app_json5_path = self.config.project_root.join("AppScope/app.json5");
        let app_section = if app_json5_path.exists() {
            let content = fs_err::read_to_string(&app_json5_path)
                .map_err(|e| ArkdownError::Io(e.into()))?;
            let raw: serde_json::Value = json5::from_str(&content)
                .map_err(|e| ArkdownError::Io(std::io::Error::other(e.to_string())))?;
            raw.get("app")
                .cloned()
                .unwrap_or_else(|| serde_json::Value::Object(Default::default()))
        } else {
            // Fallback: construct from config
            serde_json::json!({
                "bundleName": self.config.product.bundle_name,
                "bundleType": "app",
                "versionCode": 1000000,
                "versionName": "1.0.0"
            })
        };

        let module_section = manifest_to_json(manifest);

        let combined = serde_json::json!({
            "app": app_section,
            "module": module_section
        });

        if let Some(parent) = output_path.parent() {
            fs_err::create_dir_all(parent)
                .map_err(|e| ArkdownError::Io(e.into()))?;
        }

        let json_str = serde_json::to_string_pretty(&combined)
            .map_err(|e| ArkdownError::Io(std::io::Error::other(e.to_string())))?;
        fs_err::write(output_path, json_str.as_bytes())
            .map_err(|e| ArkdownError::Io(e.into()))?;

        Ok(())
    }
}

/// Convert an [`OhosModuleType`] to its lowercase string representation.
fn module_type_str(t: &OhosModuleType) -> &'static str {
    match t {
        OhosModuleType::Entry => "entry",
        OhosModuleType::Feature => "feature",
        OhosModuleType::Shared => "shared",
        OhosModuleType::Har => "har",
    }
}

/// Convert a [`CompileMode`] to its string representation for `module.json`.
fn compile_mode_str(m: &CompileMode) -> &'static str {
    match m {
        CompileMode::EsModule => "esmodule",
        CompileMode::JsBundle => "jsbundle",
    }
}

/// Build the `module` JSON object from a processed [`HapManifest`].
fn manifest_to_json(manifest: &HapManifest) -> serde_json::Value {
    let abilities: Vec<serde_json::Value> = manifest
        .abilities
        .iter()
        .map(|a| {
            let mut ability = serde_json::json!({
                "name": a.name,
                "srcEntry": a.src_entry,
                "exported": a.exported
            });
            let obj = ability.as_object_mut().unwrap();
            if let Some(v) = &a.icon { obj.insert("icon".into(), v.clone().into()); }
            if let Some(v) = &a.label { obj.insert("label".into(), v.clone().into()); }
            if let Some(v) = &a.description { obj.insert("description".into(), v.clone().into()); }
            if let Some(v) = &a.start_window_icon { obj.insert("startWindowIcon".into(), v.clone().into()); }
            if let Some(v) = &a.start_window_background { obj.insert("startWindowBackground".into(), v.clone().into()); }
            ability
        })
        .collect();

    let mut obj = serde_json::json!({
        "name": manifest.module_name,
        "type": module_type_str(&manifest.module_type),
        "virtualMachine": manifest.virtual_machine,
        "compileMode": compile_mode_str(&manifest.compile_mode),
        "deviceTypes": manifest.device_types,
        "deliveryWithInstall": true,
        "installationFree": false,
        "abilities": abilities
    });

    if let Some(main_el) = &manifest.main_element {
        obj["mainElement"] = serde_json::Value::String(main_el.clone());
    }

    if let Some(pages) = &manifest.pages {
        obj["pages"] = serde_json::Value::String(pages.clone());
    }

    obj
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use arkdown_core::{
        AbilityConfig, ArkdownConfig, BuildMode, BuildOption, ModuleConfig, ModuleJson5,
        ModuleJson5Module, OhosModuleType, OutputTarget, ProductConfig, SdkConfig,
        StrictModeConfig,
    };

    use super::*;

    fn make_config() -> ArkdownConfig {
        ArkdownConfig {
            project_root: PathBuf::from("/project"),
            build_mode: BuildMode::Debug,
            output_target: OutputTarget::Hap,
            product: ProductConfig {
                name: "default".to_string(),
                bundle_name: "com.example.testapp".to_string(),
                compatible_sdk_version: "5.0.0(12)".to_string(),
                runtime_os: "HarmonyOS".to_string(),
                signing_config: None,
                build_option: BuildOption {
                    strict_mode: StrictModeConfig {
                        case_sensitive_check: false,
                        use_normalized_ohm_url: false,
                    },
                    ark_options: None,
                    res_options: None,
                    source_option: None,
                },
            },
            modules: vec![],
            sdk_config: SdkConfig {
                ets_loader_path: PathBuf::from("/sdk/ets-loader"),
                es2abc_path: PathBuf::from("/sdk/es2abc"),
                restool_path: PathBuf::from("/sdk/restool"),
                compile_api_version: 12,
                has_rollup_plugin: true,
            },
        }
    }

    fn make_module_json5() -> ModuleJson5 {
        ModuleJson5 {
            module: ModuleJson5Module {
                name: "entry".to_string(),
                type_: "entry".to_string(),
                main_element: Some("EntryAbility".to_string()),
                device_types: vec!["phone".to_string()],
                pages: Some("$profile:main_pages".to_string()),
                abilities: vec![AbilityConfig {
                    name: "EntryAbility".to_string(),
                    src_entry: "./ets/entryability/EntryAbility.ets".to_string(),
                    exported: true,
                    skills: vec![],
                    icon: None, label: None, description: None,
                    start_window_icon: None, start_window_background: None,
                }],
                extension_abilities: vec![],
                src_entry: None,
                metadata: vec![],
            },
        }
    }

    fn make_module_cfg() -> ModuleConfig {
        ModuleConfig {
            name: "entry".to_string(),
            src_path: PathBuf::from("/project/entry"),
            module_type: OhosModuleType::Entry,
        }
    }

    #[test]
    fn test_profile_processor_process_module() {
        let config = Arc::new(make_config());
        let processor = ProfileProcessor::new(config);
        let module_cfg = make_module_cfg();
        let module_json5 = make_module_json5();

        let (manifest, loader) = processor
            .process_module(&module_cfg, &module_json5)
            .expect("process_module failed");

        // virtualMachine from api version 12 → "ark12.0.1.0"
        assert_eq!(manifest.virtual_machine, "ark12.0.1.0");
        assert_eq!(manifest.module_name, "entry");

        // loader.json should contain one compile entry
        assert_eq!(loader.compile_entry.len(), 1);
        assert_eq!(
            loader.compile_entry[0],
            "./ets/entryability/EntryAbility.ets"
        );
        assert_eq!(loader.build_mode, "debug");
    }

    #[test]
    fn test_write_loader_json_creates_file() {
        let config = Arc::new(make_config());
        let processor = ProfileProcessor::new(config);
        let module_cfg = make_module_cfg();
        let module_json5 = make_module_json5();

        let (_, loader) = processor
            .process_module(&module_cfg, &module_json5)
            .expect("process_module failed");

        let tmp_dir = std::env::temp_dir().join("arkdown_profile_test");
        let out_path = tmp_dir.join("loader.json");

        processor
            .write_loader_json(&loader, &out_path)
            .expect("write_loader_json failed");

        assert!(out_path.exists(), "loader.json was not created");

        // Read back and deserialize
        let content = std::fs::read_to_string(&out_path).unwrap();
        let loaded: LoaderJson = serde_json::from_str(&content).expect("failed to parse written loader.json");
        assert_eq!(loaded.build_mode, "debug");
        assert_eq!(loaded.package_manager_type, "ohpm");

        // Cleanup
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }
}
