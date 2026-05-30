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
pub mod har_abilities;
pub mod loader_json;
pub mod process_profile;
pub mod process_share_config;
pub mod process_startup_config;
pub mod router_map;

pub use bytecode_har::{
    collect_dynamic_import_lib_info, scan_bytecode_har_deps, ByteCodeHarScanEntry,
    DynamicImportLibEntry, HostModuleInfo,
};
pub use har_abilities::{collect_har_abilities, HarAbility};
pub use loader_json::{generate_loader_json, ByteCodeHarEntry, LoaderJson, PkgBriefInfo};
pub use process_profile::{
    ark_virtual_machine_from_version, process_profile, process_profile_with_ark_vm_version,
};
pub use process_share_config::{merge_share_config, write_share_config, ShareConfigInput, ShareConfigOutput};
pub use process_startup_config::{
    merge_startup_config, write_startup_config, StartupConfigInput, StartupConfigOutput,
};
pub use router_map::{collect_router_entries, RouterMapEntry};

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use arkdown_config::sdk::discover_ark_vm_version;
use arkdown_core::{
    ArkdownConfig, ArkdownError, CompileMode, HapManifest, ModuleConfig, ModuleJson5,
    OhosModuleType,
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
        let loader = generate_loader_json(&self.config, module_cfg, &manifest);
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
        resource_ids: Option<&HashMap<String, u32>>,
    ) -> Result<(), ArkdownError> {
        // Read AppScope/app.json5 for the app section
        let app_json5_path = self.config.project_root.join("AppScope/app.json5");
        let app_json5_fields: serde_json::Map<String, serde_json::Value> =
            if app_json5_path.exists() {
                let content = fs_err::read_to_string(&app_json5_path)
                    .map_err(|e| ArkdownError::Io(e.into()))?;
                let raw: serde_json::Value = json5::from_str(&content)
                    .map_err(|e| ArkdownError::Io(std::io::Error::other(e.to_string())))?;
                raw.get("app")
                    .and_then(|v| v.as_object())
                    .cloned()
                    .unwrap_or_default()
            } else {
                let mut m = serde_json::Map::new();
                m.insert(
                    "bundleName".into(),
                    self.config.product.bundle_name.clone().into(),
                );
                m.insert("versionCode".into(), 1000000u64.into());
                m.insert("versionName".into(), "1.0.0".into());
                m
            };

        let numeric_api =
            compute_numeric_api_version(&self.config.product.compatible_sdk_version);
        let is_debug = matches!(self.config.build_mode, arkdown_core::BuildMode::Debug);

        // Build app section in hvigor's canonical field order.
        // hvigor emits: iconId, debug, targetPatchAPIVersion, minAPIVersion, icon,
        // buildMode, label, bundleType, versionName, versionCode, appEnvironments,
        // compileSdkType, labelId, compileSdkVersion, targetAPIVersion, vendor,
        // bundleName, apiReleaseType, targetMinorAPIVersion.
        let app_section = build_app_section(
            &app_json5_fields,
            numeric_api,
            is_debug,
            &self.config.sdk_config.compile_sdk_version,
            &self.config.product.bundle_name,
            resource_ids,
        );

        let module_section = manifest_to_json(manifest, resource_ids);

        let mut combined = serde_json::Map::new();
        combined.insert("app".into(), serde_json::Value::Object(app_section));
        combined.insert("module".into(), module_section);

        if let Some(parent) = output_path.parent() {
            fs_err::create_dir_all(parent).map_err(|e| ArkdownError::Io(e.into()))?;
        }

        let json_str = serde_json::to_string_pretty(&serde_json::Value::Object(combined))
            .map_err(|e| ArkdownError::Io(std::io::Error::other(e.to_string())))?;
        fs_err::write(output_path, json_str.as_bytes()).map_err(|e| ArkdownError::Io(e.into()))?;

        Ok(())
    }
}

/// Resolve a resource reference string like `"$media:icon"` or `"$string:app_name"`
/// to a numeric resource ID by looking up the name part in `resource_ids`.
///
/// Returns `None` if `value` is not a `$type:name` reference or the name is not found.
fn resolve_res_ref(value: &str, resource_ids: &HashMap<String, u32>) -> Option<u32> {
    if !value.starts_with('$') {
        return None;
    }
    // Format: "$type:name" — take everything after the ':'
    let name = value.splitn(2, ':').nth(1)?;
    resource_ids.get(name).copied()
}

/// Build the `app` JSON object in hvigor's canonical field order.
///
/// hvigor emits fields in this order (verified against golden):
///   iconId, debug, targetPatchAPIVersion, minAPIVersion, icon, buildMode, label,
///   bundleType, versionName, versionCode, appEnvironments, compileSdkType, labelId,
///   compileSdkVersion, targetAPIVersion, vendor, bundleName, apiReleaseType,
///   targetMinorAPIVersion
///
/// Fields that come from `app.json5` (icon, label, bundleType, versionName,
/// versionCode, vendor, bundleName, appEnvironments) are read from `src_fields`.
/// Computed/injected fields are provided as arguments.
fn build_app_section(
    src: &serde_json::Map<String, serde_json::Value>,
    numeric_api: u64,
    is_debug: bool,
    compile_sdk_version: &str,
    bundle_name_fallback: &str,
    resource_ids: Option<&HashMap<String, u32>>,
) -> serde_json::Map<String, serde_json::Value> {
    let icon_ref = src
        .get("icon")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let label_ref = src
        .get("label")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let icon_id = resource_ids
        .zip(icon_ref.as_deref())
        .and_then(|(ids, r)| resolve_res_ref(r, ids));
    let label_id = resource_ids
        .zip(label_ref.as_deref())
        .and_then(|(ids, r)| resolve_res_ref(r, ids));

    let mut obj = serde_json::Map::new();

    // iconId — only present when resource IDs are available
    if let Some(id) = icon_id {
        obj.insert("iconId".into(), (id as u64).into());
    }
    obj.insert("debug".into(), is_debug.into());
    obj.insert("targetPatchAPIVersion".into(), 0u64.into());
    obj.insert("minAPIVersion".into(), numeric_api.into());
    // icon — from app.json5
    if let Some(v) = src.get("icon") {
        obj.insert("icon".into(), v.clone());
    }
    obj.insert(
        "buildMode".into(),
        if is_debug { "debug" } else { "release" }.into(),
    );
    // label — from app.json5
    if let Some(v) = src.get("label") {
        obj.insert("label".into(), v.clone());
    }
    // bundleType — from app.json5, fallback "app"
    obj.insert(
        "bundleType".into(),
        src.get("bundleType")
            .cloned()
            .unwrap_or_else(|| "app".into()),
    );
    // versionName — from app.json5
    if let Some(v) = src.get("versionName") {
        obj.insert("versionName".into(), v.clone());
    }
    // versionCode — from app.json5
    if let Some(v) = src.get("versionCode") {
        obj.insert("versionCode".into(), v.clone());
    }
    // appEnvironments — from app.json5 or empty array
    obj.insert(
        "appEnvironments".into(),
        src.get("appEnvironments")
            .cloned()
            .unwrap_or_else(|| serde_json::Value::Array(vec![])),
    );
    obj.insert("compileSdkType".into(), "HarmonyOS".into());
    // labelId — only present when resource IDs are available
    if let Some(id) = label_id {
        obj.insert("labelId".into(), (id as u64).into());
    }
    obj.insert("compileSdkVersion".into(), compile_sdk_version.into());
    obj.insert("targetAPIVersion".into(), numeric_api.into());
    // vendor — from app.json5
    if let Some(v) = src.get("vendor") {
        obj.insert("vendor".into(), v.clone());
    }
    // bundleName — from app.json5, fallback from config
    obj.insert(
        "bundleName".into(),
        src.get("bundleName")
            .cloned()
            .unwrap_or_else(|| bundle_name_fallback.into()),
    );
    obj.insert("apiReleaseType".into(), "Release".into());
    obj.insert("targetMinorAPIVersion".into(), 0u64.into());

    obj
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
///
/// Field order matches hvigor's canonical output (verified against golden):
///   virtualMachine, mainElement, installationFree, deliveryWithInstall, description,
///   compileMode, type, dependencies, abilities, deviceTypes, pages, descriptionId,
///   name, packageName
///
/// Ability field order:
///   exported, iconId, startWindowIconId, icon, startWindowIcon,
///   startWindowBackgroundId, description, label, skills, srcEntry,
///   descriptionId, labelId, startWindowBackground, name
fn manifest_to_json(
    manifest: &HapManifest,
    resource_ids: Option<&HashMap<String, u32>>,
) -> serde_json::Value {
    let abilities: Vec<serde_json::Value> = manifest
        .abilities
        .iter()
        .map(|a| {
            // Pre-compute resource IDs for this ability.
            let icon_id = resource_ids
                .zip(a.icon.as_deref())
                .and_then(|(ids, r)| resolve_res_ref(r, ids));
            let label_id = resource_ids
                .zip(a.label.as_deref())
                .and_then(|(ids, r)| resolve_res_ref(r, ids));
            let description_id = resource_ids
                .zip(a.description.as_deref())
                .and_then(|(ids, r)| resolve_res_ref(r, ids));
            let start_window_icon_id = resource_ids
                .zip(a.start_window_icon.as_deref())
                .and_then(|(ids, r)| resolve_res_ref(r, ids));
            let start_window_background_id = resource_ids
                .zip(a.start_window_background.as_deref())
                .and_then(|(ids, r)| resolve_res_ref(r, ids));

            // Build ability object in hvigor's canonical order.
            let mut obj = serde_json::Map::new();
            obj.insert("exported".into(), a.exported.into());
            if let Some(id) = icon_id {
                obj.insert("iconId".into(), (id as u64).into());
            }
            if let Some(id) = start_window_icon_id {
                obj.insert("startWindowIconId".into(), (id as u64).into());
            }
            if let Some(v) = &a.icon {
                obj.insert("icon".into(), v.clone().into());
            }
            if let Some(v) = &a.start_window_icon {
                obj.insert("startWindowIcon".into(), v.clone().into());
            }
            if let Some(id) = start_window_background_id {
                obj.insert("startWindowBackgroundId".into(), (id as u64).into());
            }
            if let Some(v) = &a.description {
                obj.insert("description".into(), v.clone().into());
            }
            if let Some(v) = &a.label {
                obj.insert("label".into(), v.clone().into());
            }
            if !a.skills.is_empty() {
                obj.insert("skills".into(), serde_json::json!(a.skills));
            }
            obj.insert("srcEntry".into(), a.src_entry.clone().into());
            if let Some(id) = description_id {
                obj.insert("descriptionId".into(), (id as u64).into());
            }
            if let Some(id) = label_id {
                obj.insert("labelId".into(), (id as u64).into());
            }
            if let Some(v) = &a.start_window_background {
                obj.insert("startWindowBackground".into(), v.clone().into());
            }
            obj.insert("name".into(), a.name.clone().into());

            serde_json::Value::Object(obj)
        })
        .collect();

    // Build module object in hvigor's canonical order.
    let mut obj = serde_json::Map::new();
    obj.insert("virtualMachine".into(), manifest.virtual_machine.clone().into());
    if let Some(main_el) = &manifest.main_element {
        obj.insert("mainElement".into(), main_el.clone().into());
    }
    obj.insert("installationFree".into(), false.into());
    obj.insert("deliveryWithInstall".into(), true.into());
    if let Some(desc) = &manifest.description {
        obj.insert("description".into(), desc.clone().into());
    }
    obj.insert(
        "compileMode".into(),
        compile_mode_str(&manifest.compile_mode).into(),
    );
    obj.insert(
        "type".into(),
        module_type_str(&manifest.module_type).into(),
    );
    obj.insert(
        "dependencies".into(),
        serde_json::Value::Array(vec![]),
    );
    obj.insert("abilities".into(), serde_json::Value::Array(abilities));
    obj.insert(
        "deviceTypes".into(),
        serde_json::Value::Array(
            manifest
                .device_types
                .iter()
                .map(|s| serde_json::Value::String(s.clone()))
                .collect(),
        ),
    );
    if let Some(pages) = &manifest.pages {
        obj.insert("pages".into(), pages.clone().into());
    }
    // Resolve module-level description resource reference to a numeric ID.
    if let (Some(desc_ref), Some(ids)) = (manifest.description.as_deref(), resource_ids) {
        if let Some(id) = resolve_res_ref(desc_ref, ids) {
            obj.insert("descriptionId".into(), (id as u64).into());
        }
    }
    obj.insert("name".into(), manifest.module_name.clone().into());
    obj.insert("packageName".into(), manifest.module_name.clone().into());

    serde_json::Value::Object(obj)
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
                    preload_so: vec![],
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
                cross_app_shared_config: vec![],
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
        let mut resource_ids = HashMap::new();
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

/// Convert a version string like `"5.0.5(17)"` to the numeric API version
/// that the HAP module.json requires: `A*1e7 + B*1e5 + C*1e3 + D`.
/// Falls back to the digit-in-parens value for simple strings like `"12"`.
fn compute_numeric_api_version(version: &str) -> u64 {
    // Extract the part in parentheses, e.g. "5.0.5(17)" → "17"
    let api_int: u64 = if let Some(start) = version.find('(') {
        let rest = &version[start + 1..];
        let end = rest.find(')').unwrap_or(rest.len());
        rest[..end].parse().unwrap_or(0)
    } else {
        version.parse().unwrap_or(0)
    };

    // Extract major.minor.patch prefix, e.g. "5.0.5(17)" → [5, 0, 5]
    let prefix = version
        .split('(')
        .next()
        .unwrap_or("")
        .trim_end_matches('.');
    let parts: Vec<u64> = prefix.split('.').filter_map(|s| s.parse().ok()).collect();
    let major = parts.first().copied().unwrap_or(0);
    let minor = parts.get(1).copied().unwrap_or(0);
    let patch = parts.get(2).copied().unwrap_or(0);

    major * 10_000_000 + minor * 100_000 + patch * 1_000 + api_int
}
