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

pub mod bytecode_har;
pub mod form_profile;
pub mod har_abilities;
pub mod loader_json;
pub(crate) mod module_json_writer;
pub mod process_distro_filter;
pub mod process_permission_reason;
pub mod process_privacy_profile;
pub mod process_profile;
pub mod process_share_config;
pub mod process_shortcut_profile;
pub mod process_startup_config;
pub mod router_map;
pub mod validate_permissions;

pub use arkdown_core::PkgBriefInfo;
pub use bytecode_har::{
    collect_bytecode_har_other_compile_files, collect_bytecode_har_update_version_info,
    collect_dynamic_import_lib_info, scan_bytecode_har_deps, ByteCodeHarScanEntry,
    DynamicImportLibEntry, HostModuleInfo,
};
pub use form_profile::{
    collect_form_profiles, collect_form_sources, collect_module_form_sources, FormProfileEntry,
    FormProfileInput, FormSourceEntry, ModuleFormProfileInput,
};
pub use har_abilities::{collect_har_abilities, HarAbility};
pub use loader_json::{ByteCodeHarEntry, LoaderJson};
pub use process_distro_filter::{
    process_distro_filter_profile, DistroFilterProfileInput, DistroFilterProfileOutput,
};
pub use process_permission_reason::{
    merge_dependency_permission_reason_resources, process_permission_reason_resources,
    DependencyPermissionReasonResourceInput, PermissionReasonResourceInput,
};
pub use process_privacy_profile::{
    privacy_profile_is_enabled, process_module_privacy_profile, process_project_privacy_profile,
    PrivacyProfileInput, PrivacyProfileOutput, ProjectPrivacyProfileInput,
    ProjectPrivacyProfileOutput,
};
pub use process_profile::{
    ark_virtual_machine_from_version, process_profile, process_profile_with_ark_vm_version,
};
pub use process_share_config::{
    merge_share_config, write_share_config, ShareConfigInput, ShareConfigOutput,
};
pub use process_shortcut_profile::{process_shortcut_profiles, ShortcutProfileInput};
pub use process_startup_config::{
    merge_startup_config, write_startup_config, StartupConfigInput, StartupConfigOutput,
};
pub use router_map::{collect_router_entries, RouterMapEntry};
pub use validate_permissions::{
    collect_sdk_permission_data, collect_sdk_permission_names, validate_request_permissions,
    RequestPermissionValidationInput, SdkPermissionData,
};

use rustc_hash::FxHashMap;
use std::path::Path;
use std::sync::Arc;

use arkdown_config::sdk::discover_ark_vm_version;
use arkdown_core::{
    ArkdownConfig, ArkdownError, CompileMode, HapManifest, ModuleConfig, ModuleJson5,
};

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
        let discovered_ark_vm_version = discover_ark_vm_version(
            &self.config.sdk_config.ets_loader_path,
            self.config.sdk_config.compile_api_version,
        );
        let manifest = if let Some(ark_vm_version) = discovered_ark_vm_version {
            process_profile_with_ark_vm_version(
                module_json5,
                &ark_vm_version,
                &self.config.product.bundle_name,
                CompileMode::EsModule,
            )
        } else {
            process_profile(
                module_json5,
                self.config.sdk_config.compile_api_version,
                &self.config.product.bundle_name,
                CompileMode::EsModule,
            )
        };
        let loader = LoaderJson::try_generate(&self.config, module_cfg, &manifest)?;
        Ok((manifest, loader))
    }

    /// Serialize `loader` as indented JSON and write it to `path`.
    ///
    /// Creates parent directories if they do not exist.
    pub fn write_loader_json(&self, loader: &LoaderJson, path: &Path) -> Result<(), ArkdownError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(loader)
            .map_err(|e| ArkdownError::Io(std::io::Error::other(e.to_string())))?;
        fs_err::write(path, json)?;

        // Also write pkgContextInfo.json alongside loader.json.
        //
        // hvigor writes `build/default/intermediates/pkgContextInfo/default/pkgContextInfo.json`
        // containing the package context map.  For compatibility, we write the
        // pkgNameToPkgBriefInfo data from the loader into the same directory.
        // The parity test searches for this file using the "pkgContextInfo" path fragment.
        if let Some(loader_dir) = path.parent() {
            let pkg_ctx_dir = loader_dir
                .parent()
                .unwrap_or(loader_dir)
                .join("pkgContextInfo/default");
            if let Ok(()) = std::fs::create_dir_all(&pkg_ctx_dir) {
                let pkg_ctx_path = pkg_ctx_dir.join("pkgContextInfo.json");
                if let Ok(pkg_ctx_json) = serde_json::to_string_pretty(&loader.pkg_name_to_pkg_brief_info) {
                    let _ = fs_err::write(&pkg_ctx_path, pkg_ctx_json);
                }
            }
        }

        Ok(())
    }

    /// Write the processed `module.json` for inclusion in the HAP.
    ///
    /// The output is a standard JSON file (not JSON5) combining:
    /// - `app` section from `AppScope/app.json5` (falls back to config values)
    /// - `module` section from the processed manifest (with `virtualMachine` and `compileMode`)
    ///
    /// If `resource_ids` is provided, resource references in `icon`/`label` (app section)
    /// and `description` (module section) are resolved to numeric IDs and written as
    /// `iconId`, `labelId`, and `descriptionId` respectively.
    pub fn write_module_json(
        &self,
        manifest: &HapManifest,
        output_path: &Path,
        resource_ids: Option<&FxHashMap<String, u32>>,
    ) -> Result<(), ArkdownError> {
        module_json_writer::write_module_json(manifest, &self.config, output_path, resource_ids)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

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
                compatible_sdk_version_stage: None,
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
                    preload_system_so: false,
                    preload_so: vec![],
                    remove_permissions: vec![],
                    aot_compile_mode: None,
                    host_pgo: None,
                    ap_path: None,
                    abi_filters: vec![],
                },
            },
            modules: vec![],
            sdk_config: SdkConfig {
                ets_loader_path: PathBuf::from("/sdk/ets-loader"),
                es2abc_path: PathBuf::from("/sdk/es2abc"),
                restool_path: PathBuf::from("/sdk/restool"),
                compile_api_version: 12,
                has_rollup_plugin: true,
                compile_sdk_version: "6.0.0.47".to_string(),
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
                router_map: None,
                description: None,
                abilities: vec![AbilityConfig {
                    name: "EntryAbility".to_string(),
                    src_entry: "./ets/entryability/EntryAbility.ets".to_string(),
                    exported: true,
                    skills: vec![],
                    icon: None,
                    label: None,
                    description: None,
                    start_window_icon: None,
                    start_window_background: None,
                }],
                extension_abilities: vec![],
                src_entry: None,
                metadata: vec![],
                app_startup: None,
                cross_app_shared_config: Default::default(),
                ..Default::default()
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

    #[cfg(unix)]
    fn write_fake_es2abc(es2abc: &Path) {
        fs::create_dir_all(es2abc.parent().expect("es2abc parent"))
            .expect("create fake es2abc dir");
        fs::write(
            es2abc,
            r#"#!/bin/sh
stage=""
previous=""
for arg in "$@"; do
  if [ "$previous" = "--target-api-sub-version" ]; then
    stage="$arg"
  fi
  previous="$arg"
done
if [ "$stage" = "Release" ]; then
  printf '12.0.6.0\n'
else
  printf '12.0.2.0\n'
fi
"#,
        )
        .expect("write fake es2abc");
        let mut permissions = fs::metadata(es2abc)
            .expect("fake es2abc metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(es2abc, permissions).expect("chmod fake es2abc");
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

    #[cfg(unix)]
    #[test]
    fn test_profile_processor_uses_discovered_ark_vm_version() {
        let temp_dir = tempfile::tempdir().expect("create temp sdk");
        let ets_dir = temp_dir.path().join("ets");
        let ets_loader = ets_dir.join("build-tools").join("ets-loader");
        let es2abc = ets_loader
            .join("bin")
            .join("ark")
            .join(if cfg!(target_os = "macos") {
                "build-mac"
            } else if cfg!(target_os = "windows") {
                "build-win"
            } else {
                "build-linux"
            })
            .join("bin")
            .join("es2abc");

        fs::create_dir_all(&ets_dir).expect("create fake ets dir");
        fs::write(
            ets_dir.join("oh-uni-package.json"),
            r#"{"version":"6.0.1.112","releaseType":"Release"}"#,
        )
        .expect("write fake ets metadata");
        write_fake_es2abc(&es2abc);

        let mut config = make_config();
        config.sdk_config.ets_loader_path = ets_loader;
        config.sdk_config.es2abc_path = es2abc;
        let processor = ProfileProcessor::new(Arc::new(config));
        let module_cfg = make_module_cfg();
        let module_json5 = make_module_json5();

        let (manifest, _) = processor
            .process_module(&module_cfg, &module_json5)
            .expect("process_module failed");

        assert_eq!(manifest.virtual_machine, "ark12.0.6.0");
    }

    /// Verify that `write_module_json` produces output that matches the golden
    /// `tests/fixtures/golden/minimal-hap/module.json` exactly (both field
    /// values and insertion order).
    #[test]
    fn test_write_module_json_golden_parity() {
        use arkdown_core::{AbilityConfig, ModuleJson5Module, SkillConfig};

        // Locate the repo root from the crate manifest dir.
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent() // crates/arkdown-profile
            .unwrap()
            .parent() // crates/
            .unwrap()
            .parent() // repo root
            .unwrap()
            .to_path_buf();

        let golden_path = repo_root.join("tests/fixtures/golden/minimal-hap/module.json");
        if !golden_path.exists() {
            // Skip when the golden file is absent (e.g. in a stripped checkout).
            eprintln!("Skipping golden test: {:?} not found", golden_path);
            return;
        }

        // Config that matches the minimal-hap fixture.
        let fixture = repo_root.join("tests/fixtures/minimal-hap");
        let mut config = make_config();
        config.project_root = fixture.clone();
        config.product.bundle_name = "com.example.arkdown.test".to_string();
        config.product.compatible_sdk_version = "5.0.5(17)".to_string(); // → 50005017
        config.sdk_config.compile_sdk_version = "6.0.1.112".to_string();
        config.build_mode = BuildMode::Debug;

        let processor = ProfileProcessor::new(Arc::new(config));

        // Construct the manifest matching module.json5.
        let module_json5 = ModuleJson5 {
            module: ModuleJson5Module {
                name: "entry".to_string(),
                type_: "entry".to_string(),
                main_element: Some("EntryAbility".to_string()),
                device_types: vec!["phone".to_string()],
                pages: Some("$profile:main_pages".to_string()),
                router_map: None,
                description: Some("$string:module_desc".to_string()),
                abilities: vec![AbilityConfig {
                    name: "EntryAbility".to_string(),
                    src_entry: "./ets/entryability/EntryAbility.ets".to_string(),
                    exported: true,
                    skills: vec![SkillConfig {
                        entities: vec!["entity.system.home".to_string()],
                        actions: vec!["ohos.want.action.home".to_string()],
                        uris: vec![],
                    }],
                    icon: Some("$media:layered_image".to_string()),
                    label: Some("$string:EntryAbility_label".to_string()),
                    description: Some("$string:EntryAbility_desc".to_string()),
                    start_window_icon: Some("$media:startIcon".to_string()),
                    start_window_background: Some("$color:start_window_background".to_string()),
                }],
                extension_abilities: vec![],
                src_entry: None,
                metadata: vec![],
                app_startup: None,
                cross_app_shared_config: Default::default(),
                ..Default::default()
            },
        };

        let module_cfg = ModuleConfig {
            name: "entry".to_string(),
            src_path: fixture.join("entry"),
            module_type: OhosModuleType::Entry,
        };

        let manifest = process_profile_with_ark_vm_version(
            &module_json5,
            "12.0.6.0",
            "com.example.arkdown.test",
            arkdown_core::CompileMode::EsModule,
        );
        let _ = module_cfg; // silence unused warning

        // Resource IDs from the golden.
        let mut resource_ids = FxHashMap::default();
        resource_ids.insert("layered_image".to_string(), 16777219u32);
        resource_ids.insert("app_name".to_string(), 16777216u32);
        resource_ids.insert("EntryAbility_desc".to_string(), 16777220u32);
        resource_ids.insert("EntryAbility_label".to_string(), 16777221u32);
        resource_ids.insert("start_window_background".to_string(), 16777223u32);
        resource_ids.insert("startIcon".to_string(), 16777224u32);
        resource_ids.insert("module_desc".to_string(), 16777222u32);

        let tmp_dir = tempfile::tempdir().expect("create temp dir");
        let out_path = tmp_dir.path().join("module.json");

        processor
            .write_module_json(&manifest, &out_path, Some(&resource_ids))
            .expect("write_module_json failed");

        let produced_raw = fs::read_to_string(&out_path).expect("read produced module.json");
        let golden_raw = fs::read_to_string(&golden_path).expect("read golden module.json");

        // Compare as JSON values first (content check).
        let produced: serde_json::Value =
            serde_json::from_str(&produced_raw).expect("parse produced module.json");
        let golden: serde_json::Value =
            serde_json::from_str(&golden_raw).expect("parse golden module.json");
        assert_eq!(
            produced, golden,
            "module.json content mismatch vs golden.\nProduced:\n{}\nGolden:\n{}",
            produced_raw, golden_raw
        );

        // Compare key order in app section.
        let produced_app_keys: Vec<&str> = produced["app"]
            .as_object()
            .unwrap()
            .keys()
            .map(|s| s.as_str())
            .collect();
        let golden_app_keys: Vec<&str> = golden["app"]
            .as_object()
            .unwrap()
            .keys()
            .map(|s| s.as_str())
            .collect();
        assert_eq!(
            produced_app_keys, golden_app_keys,
            "app field order mismatch vs golden"
        );

        // Compare key order in module section.
        let produced_module_keys: Vec<&str> = produced["module"]
            .as_object()
            .unwrap()
            .keys()
            .map(|s| s.as_str())
            .collect();
        let golden_module_keys: Vec<&str> = golden["module"]
            .as_object()
            .unwrap()
            .keys()
            .map(|s| s.as_str())
            .collect();
        assert_eq!(
            produced_module_keys, golden_module_keys,
            "module field order mismatch vs golden"
        );

        // Compare key order in abilities[0].
        let produced_ability_keys: Vec<&str> = produced["module"]["abilities"][0]
            .as_object()
            .unwrap()
            .keys()
            .map(|s| s.as_str())
            .collect();
        let golden_ability_keys: Vec<&str> = golden["module"]["abilities"][0]
            .as_object()
            .unwrap()
            .keys()
            .map(|s| s.as_str())
            .collect();
        assert_eq!(
            produced_ability_keys, golden_ability_keys,
            "abilities[0] field order mismatch vs golden"
        );
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
        let loaded: LoaderJson =
            serde_json::from_str(&content).expect("failed to parse written loader.json");
        assert_eq!(loaded.build_mode, "debug");
        assert_eq!(loaded.package_manager_type, "ohpm");

        // Cleanup
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }
}
