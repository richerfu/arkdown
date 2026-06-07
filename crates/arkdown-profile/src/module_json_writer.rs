//! Serialization of [`HapManifest`] and `app.json5` fields to the final
//! `module.json` format required by the HAP packager.
//!
//! The field insertion order in every JSON object **must exactly match
//! hvigor's canonical output** — this is verified by golden tests.
//! Do not use `#[serde(rename_all)]` derives here; order is controlled
//! manually via `serde_json::Map::insert` call order.

use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use arkdown_config::build_profile::{parse_api_version, RawBuildProfile};
use arkdown_core::{ArkdownConfig, ArkdownError, HapManifest, OhosModuleType};
use indexmap::IndexMap;
use serde::Deserialize;

#[derive(Debug, Clone)]
struct TargetAbilityOverride {
    name: String,
    icon: Option<String>,
    label: Option<String>,
    launch_type: Option<String>,
}

#[derive(Debug, Default)]
struct DependencyProfileMerge {
    metadata: Option<Vec<serde_json::Value>>,
    request_permissions: Option<Vec<serde_json::Value>>,
}

#[derive(Debug)]
struct DependencyModuleProfile {
    module_name: String,
    module_type: String,
    source_path: PathBuf,
    min_api_version: Option<u32>,
    device_types: Vec<String>,
    metadata: Vec<serde_json::Value>,
    request_permissions: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct DependencyPackageJson {
    #[serde(default)]
    dependencies: IndexMap<String, String>,
}

/// Write the processed `module.json` for inclusion in the HAP.
///
/// Combines:
/// - `app` section from `AppScope/app.json5` (falls back to config values)
/// - `module` section from the processed `manifest`
///
/// Resource references like `"$media:icon"` are resolved to numeric IDs
/// when `resource_ids` is provided.
pub fn write_module_json(
    manifest: &HapManifest,
    config: &ArkdownConfig,
    output_path: &Path,
    resource_ids: Option<&FxHashMap<String, u32>>,
) -> Result<(), ArkdownError> {
    let app_json5_fields = ModuleJsonWriter::read_app_json5_fields(config)?;
    let project_multi_projects = ModuleJsonWriter::read_project_multi_projects(config);
    let numeric_api =
        ModuleJsonWriter::compute_numeric_api_version(&config.product.compatible_sdk_version);
    let is_debug = matches!(config.build_mode, arkdown_core::BuildMode::Debug);

    // Derive packageName from oh-package.json5#name for this module.
    // Falls back to module_name if the file is absent or unparseable.
    let package_name = ModuleJsonWriter::read_oh_package_name(manifest, config)
        .unwrap_or_else(|| manifest.module_name.clone());

    // Derive bundle_type for installationFree logic (from app.json5).
    let bundle_type = app_json5_fields
        .get("bundleType")
        .and_then(|v| v.as_str())
        .unwrap_or("app")
        .to_string();

    // Read raw module.json5 for preserve-and-mutate: unknown fields from the
    // source are appended verbatim after all known fields are emitted.
    let raw_module5 = ModuleJsonWriter::read_raw_module5(manifest, config);
    let target_ability_overrides =
        ModuleJsonWriter::read_target_ability_overrides(manifest, config);
    let dependency_profile_merge =
        ModuleJsonWriter::read_dependency_profile_merge(manifest, config, raw_module5.as_ref())?;

    let app_section = ModuleJsonWriter::build_app_section(
        &app_json5_fields,
        numeric_api,
        is_debug,
        &config.sdk_config.compile_sdk_version,
        &config.product.bundle_name,
        project_multi_projects,
        resource_ids,
        config.product.version_code,
        config.product.version_name.as_deref(),
    );
    let module_section = ModuleJsonWriter::manifest_to_json(
        manifest,
        raw_module5.as_ref(),
        resource_ids,
        &package_name,
        &bundle_type,
        &target_ability_overrides,
        &dependency_profile_merge,
    );

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

/// Encapsulates the private helper logic for building `module.json` content.
struct ModuleJsonWriter;

impl ModuleJsonWriter {
    /// Read the `app` object from `AppScope/app.json5`, falling back to minimal
    /// defaults derived from `config` when the file is absent.
    fn read_app_json5_fields(
        config: &ArkdownConfig,
    ) -> Result<serde_json::Map<String, serde_json::Value>, ArkdownError> {
        let app_json5_path = config.project_root.join("AppScope/app.json5");
        if app_json5_path.exists() {
            let content =
                fs_err::read_to_string(&app_json5_path).map_err(|e| ArkdownError::Io(e.into()))?;
            let raw: serde_json::Value = json5::from_str(&content)
                .map_err(|e| ArkdownError::Io(std::io::Error::other(e.to_string())))?;
            Ok(raw
                .get("app")
                .and_then(|v| v.as_object())
                .cloned()
                .unwrap_or_default())
        } else {
            let mut m = serde_json::Map::new();
            m.insert(
                "bundleName".into(),
                config.product.bundle_name.clone().into(),
            );
            m.insert("versionCode".into(), 1000000u64.into());
            m.insert("versionName".into(), "1.0.0".into());
            Ok(m)
        }
    }

    /// Build the `app` JSON object in hvigor's canonical field order.
    ///
    /// hvigor emits fields in this order (verified against golden):
    ///   iconId, debug, targetPatchAPIVersion, minAPIVersion, icon, buildMode, label,
    ///   bundleType, versionName, versionCode, appEnvironments, compileSdkType, labelId,
    ///   compileSdkVersion, targetAPIVersion, vendor, bundleName, apiReleaseType,
    ///   targetMinorAPIVersion
    ///
    /// `version_code_override` / `version_name_override`: when present, these
    /// product-level values from `build-profile.json5` take priority over the
    /// corresponding fields in `AppScope/app.json5`.  Matches hvigor's
    /// product-level app override semantics.
    #[allow(clippy::too_many_arguments)]
    fn build_app_section(
        src: &serde_json::Map<String, serde_json::Value>,
        numeric_api: u64,
        is_debug: bool,
        compile_sdk_version: &str,
        bundle_name_fallback: &str,
        project_multi_projects: bool,
        resource_ids: Option<&FxHashMap<String, u32>>,
        version_code_override: Option<u64>,
        version_name_override: Option<&str>,
    ) -> serde_json::Map<String, serde_json::Value> {
        let icon_ref = src.get("icon").and_then(|v| v.as_str()).map(str::to_string);
        let label_ref = src
            .get("label")
            .and_then(|v| v.as_str())
            .map(str::to_string);

        let icon_id = resource_ids
            .zip(icon_ref.as_deref())
            .and_then(|(ids, r)| Self::resolve_res_ref(r, ids));
        let label_id = resource_ids
            .zip(label_ref.as_deref())
            .and_then(|(ids, r)| Self::resolve_res_ref(r, ids));

        let mut obj = serde_json::Map::new();
        if let Some(id) = icon_id {
            obj.insert("iconId".into(), (id as u64).into());
        }
        obj.insert("debug".into(), is_debug.into());
        obj.insert("targetPatchAPIVersion".into(), 0u64.into());
        obj.insert("minAPIVersion".into(), numeric_api.into());
        if let Some(v) = src.get("icon") {
            obj.insert("icon".into(), v.clone());
        }
        obj.insert(
            "buildMode".into(),
            if is_debug { "debug" } else { "release" }.into(),
        );
        if let Some(v) = src.get("label") {
            obj.insert("label".into(), v.clone());
        }
        obj.insert(
            "bundleType".into(),
            src.get("bundleType")
                .cloned()
                .unwrap_or_else(|| "app".into()),
        );
        // versionName: product override takes priority over app.json5.
        if let Some(name) = version_name_override {
            obj.insert("versionName".into(), name.into());
        } else if let Some(v) = src.get("versionName") {
            obj.insert("versionName".into(), v.clone());
        }
        // versionCode: product override takes priority over app.json5.
        if let Some(code) = version_code_override {
            obj.insert("versionCode".into(), code.into());
        } else if let Some(v) = src.get("versionCode") {
            obj.insert("versionCode".into(), v.clone());
        }
        obj.insert(
            "appEnvironments".into(),
            src.get("appEnvironments")
                .cloned()
                .unwrap_or_else(|| serde_json::Value::Array(vec![])),
        );
        obj.insert("compileSdkType".into(), "HarmonyOS".into());
        if let Some(id) = label_id {
            obj.insert("labelId".into(), (id as u64).into());
        }
        obj.insert("compileSdkVersion".into(), compile_sdk_version.into());
        obj.insert("targetAPIVersion".into(), numeric_api.into());
        if let Some(v) = src.get("vendor") {
            obj.insert("vendor".into(), v.clone());
        }
        obj.insert(
            "bundleName".into(),
            src.get("bundleName")
                .cloned()
                .unwrap_or_else(|| bundle_name_fallback.into()),
        );
        obj.insert("apiReleaseType".into(), "Release".into());
        obj.insert("targetMinorAPIVersion".into(), 0u64.into());
        if project_multi_projects {
            obj.insert("multiProjects".into(), true.into());
        }

        // Preserve-and-mutate passthrough: append any app.json5 `app` keys that
        // are not already emitted above.  Matches hvigor's cloneDeep + targeted
        // mutation approach so that `sanitizer`, `deviceTypes`,
        // `car`, `wearable`, `tablet`, `startWindowBackgroundColor`, and any
        // future fields survive the build without needing explicit support here.
        //
        // `multiProjects` is special: hvigor ignores it from AppScope/app.json5
        // and only writes the build-profile top-level `app.multiProjects`.
        for (key, value) in src {
            if key == "multiProjects" {
                continue;
            }
            if !obj.contains_key(key.as_str()) {
                // Skip empty arrays to avoid emitting noise.
                if let serde_json::Value::Array(arr) = value {
                    if arr.is_empty() {
                        continue;
                    }
                }
                obj.insert(key.clone(), value.clone());
            }
        }

        obj
    }

    fn read_project_multi_projects(config: &ArkdownConfig) -> bool {
        RawBuildProfile::load(&config.project_root)
            .map(|profile| profile.app.multi_projects)
            .unwrap_or(false)
    }

    /// Build the `module` JSON object from a processed [`HapManifest`].
    ///
    /// Field order matches hvigor's canonical output (verified against golden):
    ///   virtualMachine, mainElement, installationFree, deliveryWithInstall, description,
    ///   compileMode, type, dependencies, abilities, deviceTypes, pages, descriptionId,
    ///   name, packageName
    ///
    /// Unknown fields from the raw `module.json5` are appended verbatim after
    /// `packageName` (preserve-and-mutate semantics, matching hvigor's `cloneDeep` + mutate).
    fn manifest_to_json(
        manifest: &HapManifest,
        raw_module5: Option<&serde_json::Value>,
        resource_ids: Option<&FxHashMap<String, u32>>,
        package_name: &str,
        bundle_type: &str,
        target_ability_overrides: &[TargetAbilityOverride],
        dependency_profile_merge: &DependencyProfileMerge,
    ) -> serde_json::Value {
        // Build a name→raw-ability lookup for preserve-and-mutate.
        // Hvigor cloneDeep-copies the ability array and mutates selected fields
        // (resource IDs, srcEntry), preserving all other fields verbatim.
        let raw_abilities: Vec<serde_json::Map<String, serde_json::Value>> = raw_module5
            .and_then(|m| m.get("abilities"))
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_object().cloned()).collect())
            .unwrap_or_default();
        let find_raw_ability = |name: &str| {
            raw_abilities
                .iter()
                .find(|raw| raw.get("name").and_then(|n| n.as_str()) == Some(name))
                .cloned()
        };

        let abilities: Vec<serde_json::Value> = manifest
            .abilities
            .iter()
            .map(|a| {
                let target_override = target_ability_overrides
                    .iter()
                    .find(|target_override| target_override.name == a.name);
                let icon = target_override
                    .and_then(|target_override| target_override.icon.as_deref())
                    .or(a.icon.as_deref());
                let label = target_override
                    .and_then(|target_override| target_override.label.as_deref())
                    .or(a.label.as_deref());
                let launch_type = target_override
                    .and_then(|target_override| target_override.launch_type.as_deref());
                let icon_id = resource_ids
                    .zip(icon)
                    .and_then(|(ids, r)| Self::resolve_res_ref(r, ids));
                let label_id = resource_ids
                    .zip(label)
                    .and_then(|(ids, r)| Self::resolve_res_ref(r, ids));
                let description_id = resource_ids
                    .zip(a.description.as_deref())
                    .and_then(|(ids, r)| Self::resolve_res_ref(r, ids));
                let start_window_icon_id = resource_ids
                    .zip(a.start_window_icon.as_deref())
                    .and_then(|(ids, r)| Self::resolve_res_ref(r, ids));
                let start_window_background_id = resource_ids
                    .zip(a.start_window_background.as_deref())
                    .and_then(|(ids, r)| Self::resolve_res_ref(r, ids));

                // Start from the raw ability object (preserves launchType,
                // visible, continuable, orientation, windowMode, and any other
                // fields not modelled in AbilityEntry).
                let mut obj = find_raw_ability(&a.name).unwrap_or_default();

                // Overlay fields that arkdown computes or normalises.
                obj.insert("exported".into(), a.exported.into());
                if let Some(id) = icon_id {
                    obj.insert("iconId".into(), (id as u64).into());
                }
                if let Some(id) = start_window_icon_id {
                    obj.insert("startWindowIconId".into(), (id as u64).into());
                }
                if let Some(v) = icon {
                    obj.insert("icon".into(), v.into());
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
                if let Some(v) = label {
                    obj.insert("label".into(), v.into());
                }
                if !a.skills.is_empty() {
                    obj.insert("skills".into(), serde_json::json!(a.skills));
                }
                if let Some(v) = launch_type {
                    obj.insert("launchType".into(), v.into());
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

        // Build a name→raw-extensionAbility lookup for preserve-and-mutate.
        let raw_ext_abilities: Vec<serde_json::Map<String, serde_json::Value>> = raw_module5
            .and_then(|m| m.get("extensionAbilities"))
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_object().cloned()).collect())
            .unwrap_or_default();
        let find_raw_ext_ability = |name: &str| {
            raw_ext_abilities
                .iter()
                .find(|raw| raw.get("name").and_then(|n| n.as_str()) == Some(name))
                .cloned()
        };

        let extension_abilities: Vec<serde_json::Value> = manifest
            .extension_abilities
            .iter()
            .map(|a| {
                let label_id = resource_ids
                    .zip(a.label.as_deref())
                    .and_then(|(ids, r)| Self::resolve_res_ref(r, ids));
                let description_id = resource_ids
                    .zip(a.description.as_deref())
                    .and_then(|(ids, r)| Self::resolve_res_ref(r, ids));

                let metadata: Vec<serde_json::Value> = a
                    .metadata
                    .iter()
                    .map(|entry| {
                        let mut obj = serde_json::Map::new();
                        obj.insert("name".into(), entry.name.clone().into());
                        if let Some(value) = &entry.value {
                            obj.insert("value".into(), value.clone().into());
                        }
                        if let Some(resource) = &entry.resource {
                            obj.insert("resource".into(), resource.clone().into());
                            if let Some(id) =
                                resource_ids.and_then(|ids| Self::resolve_res_ref(resource, ids))
                            {
                                obj.insert("resourceId".into(), (id as u64).into());
                            }
                        }
                        serde_json::Value::Object(obj)
                    })
                    .collect();

                // Start from the raw extensionAbility object to preserve unknown
                // fields (permissions, readPermission, writePermission, forms, etc.).
                let mut obj = find_raw_ext_ability(&a.name).unwrap_or_default();
                obj.insert("name".into(), a.name.clone().into());
                obj.insert("srcEntry".into(), a.src_entry.clone().into());
                if let Some(label) = &a.label {
                    obj.insert("label".into(), label.clone().into());
                }
                if let Some(description) = &a.description {
                    obj.insert("description".into(), description.clone().into());
                }
                obj.insert("type".into(), a.extension_type.clone().into());
                if !metadata.is_empty() {
                    obj.insert("metadata".into(), serde_json::Value::Array(metadata));
                }
                if let Some(id) = label_id {
                    obj.insert("labelId".into(), (id as u64).into());
                }
                if let Some(id) = description_id {
                    obj.insert("descriptionId".into(), (id as u64).into());
                }
                serde_json::Value::Object(obj)
            })
            .collect();

        let mut obj = serde_json::Map::new();
        obj.insert(
            "virtualMachine".into(),
            manifest.virtual_machine.clone().into(),
        );
        if let Some(main_el) = &manifest.main_element {
            obj.insert("mainElement".into(), main_el.clone().into());
        }
        // installationFree: true only for atomicService bundles (hvigor sets this
        // unconditionally from bundleType, ignoring whatever is in module.json5).
        obj.insert(
            "installationFree".into(),
            (bundle_type == "atomicService").into(),
        );
        // atomicService config block: only emitted for atomicService bundles,
        // preserved from module.json5 (hvigor deletes it for app/shared types).
        if bundle_type == "atomicService" {
            if let Some(atomic) = raw_module5.and_then(|v| v.get("atomicService")) {
                obj.insert("atomicService".into(), atomic.clone());
            }
        }
        // deliveryWithInstall: respect value from module.json5; default true.
        let delivery_with_install = raw_module5
            .and_then(|v| v.get("deliveryWithInstall"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        obj.insert("deliveryWithInstall".into(), delivery_with_install.into());
        if let Some(desc) = &manifest.description {
            obj.insert("description".into(), desc.clone().into());
        }
        obj.insert(
            "compileMode".into(),
            manifest.compile_mode.to_string().into(),
        );
        obj.insert("type".into(), manifest.module_type.to_string().into());
        // dependencies: serialise from manifest (populated for HSP modules).
        let deps: Vec<serde_json::Value> = manifest
            .dependencies
            .iter()
            .map(|d| {
                let mut dep = serde_json::Map::new();
                dep.insert("bundleName".into(), d.bundle_name.clone().into());
                dep.insert("moduleName".into(), d.module_name.clone().into());
                serde_json::Value::Object(dep)
            })
            .collect();
        obj.insert("dependencies".into(), serde_json::Value::Array(deps));
        // requestPermissions: start with the current module permissions, then
        // append HAR/HSP dependency permissions by name. Hvigor's
        // removePermissions only filters dependency permissions after removing
        // names already declared by the current module from the removal set.
        if let Some(perms) = dependency_profile_merge.request_permissions.as_ref() {
            if !perms.is_empty() {
                obj.insert(
                    "requestPermissions".into(),
                    serde_json::Value::Array(perms.clone()),
                );
            }
        } else if let Some(perms) = raw_module5.and_then(|v| v.get("requestPermissions")) {
            if perms.as_array().is_some_and(|a| !a.is_empty()) {
                obj.insert("requestPermissions".into(), perms.clone());
            }
        }
        // definePermissions: preserved verbatim from module.json5.
        if let Some(defs) = raw_module5.and_then(|v| v.get("definePermissions")) {
            if defs.as_array().map_or(false, |a| !a.is_empty()) {
                obj.insert("definePermissions".into(), defs.clone());
            }
        }
        obj.insert("abilities".into(), serde_json::Value::Array(abilities));
        if !extension_abilities.is_empty() {
            obj.insert(
                "extensionAbilities".into(),
                serde_json::Value::Array(extension_abilities),
            );
        }
        // proxyData: preserved verbatim from module.json5.
        // (bundleName URI mutation only applies when hvigor receives a CLI
        // --bundleName override; arkdown does not implement that yet.)
        if let Some(proxy) = raw_module5.and_then(|v| v.get("proxyData")) {
            if proxy.as_array().map_or(false, |a| !a.is_empty()) {
                obj.insert("proxyData".into(), proxy.clone());
            }
        }
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
        if let (Some(desc_ref), Some(ids)) = (manifest.description.as_deref(), resource_ids) {
            if let Some(id) = Self::resolve_res_ref(desc_ref, ids) {
                obj.insert("descriptionId".into(), (id as u64).into());
            }
        }
        obj.insert("name".into(), manifest.module_name.clone().into());
        // packageName from oh-package.json5#name (hvigor: Object.assign using
        // packageJsonObj.name); falls back to module_name when file is absent.
        obj.insert("packageName".into(), package_name.into());

        // Preserve-and-mutate passthrough: append any module.json5 keys that
        // were not already emitted above.  This mirrors hvigor's cloneDeep +
        // targeted-mutation approach so that metadata, appStartup,
        // crossAppSharedConfig, and any future fields survive the build.
        if let Some(raw_map) = raw_module5.and_then(|v| v.as_object()) {
            for (key, value) in raw_map {
                if !obj.contains_key(key.as_str()) {
                    // Skip empty arrays to avoid emitting noise.
                    if let serde_json::Value::Array(arr) = value {
                        if arr.is_empty() {
                            continue;
                        }
                    }
                    obj.insert(key.clone(), value.clone());
                }
            }
        }
        if let Some(metadata) = dependency_profile_merge.metadata.as_ref() {
            if !metadata.is_empty() {
                obj.insert(
                    "metadata".into(),
                    serde_json::Value::Array(metadata.clone()),
                );
            }
        }
        serde_json::Value::Object(obj)
    }

    /// Read `oh-package.json5#name` from the module's source directory.
    ///
    /// Returns `None` when the module is not in `config.modules`, the file is
    /// absent, or parsing fails; callers fall back to `manifest.module_name`.
    fn read_oh_package_name(manifest: &HapManifest, config: &ArkdownConfig) -> Option<String> {
        let src_path = config
            .modules
            .iter()
            .find(|m| m.name == manifest.module_name)
            .map(|m| m.src_path.as_path())?;
        let path = src_path.join("oh-package.json5");
        let content = fs_err::read_to_string(&path).ok()?;
        let raw: serde_json::Value = json5::from_str(&content).ok()?;
        raw.get("name").and_then(|v| v.as_str()).map(str::to_string)
    }

    /// Read the `module` object from `src/main/module.json5` as a raw JSON value.
    ///
    /// Used for preserve-and-mutate field passthrough.  Returns `None` when the
    /// module is not in `config.modules`, the file is absent, or parsing fails.
    fn read_raw_module5(
        manifest: &HapManifest,
        config: &ArkdownConfig,
    ) -> Option<serde_json::Value> {
        let src_path = config
            .modules
            .iter()
            .find(|m| m.name == manifest.module_name)
            .map(|m| m.src_path.as_path())?;
        let path = src_path.join("src/main/module.json5");
        let content = fs_err::read_to_string(&path).ok()?;
        let raw: serde_json::Value = json5::from_str(&content).ok()?;
        raw.get("module").cloned()
    }

    fn read_dependency_profile_merge(
        manifest: &HapManifest,
        config: &ArkdownConfig,
        raw_module5: Option<&serde_json::Value>,
    ) -> Result<DependencyProfileMerge, ArkdownError> {
        if matches!(manifest.module_type, OhosModuleType::Har) {
            return Ok(DependencyProfileMerge::default());
        }

        let Some(module_cfg) = config
            .modules
            .iter()
            .find(|module| module.name == manifest.module_name)
        else {
            return Ok(DependencyProfileMerge::default());
        };

        let dependency_profiles =
            Self::read_dependency_module_profiles(&module_cfg.src_path, &config.project_root)?;
        let compatible_api = parse_api_version(&config.product.compatible_sdk_version)
            .unwrap_or(config.sdk_config.compile_api_version);

        // Validate ALL local dependency nodes (SourceHar + SharedHsp) for API/device
        // compatibility, not just those that contribute metadata/requestPermissions.
        // Matches hvigor's checkHarApiStatus() / checkHarDeviceTypes() behavior
        // which validates the entire dependency graph, not just the merging subset.
        {
            use crate::dependency_graph::{DependencyNodeType, LocalDependencyGraph};
            let graph = LocalDependencyGraph::build(&module_cfg.src_path, &config.project_root);
            for node in graph.nodes.values() {
                if !matches!(
                    node.node_type,
                    DependencyNodeType::SourceHar | DependencyNodeType::SharedHsp
                ) {
                    continue;
                }
                if node.pkg_root.as_os_str().is_empty() {
                    continue;
                }
                // Read module profile for this node and validate API/device.
                let profile_path = node.pkg_root.join("src/main/module.json5");
                if !profile_path.is_file() {
                    continue;
                }
                let Ok(content) = std::fs::read_to_string(&profile_path) else {
                    continue;
                };
                // Only validate API/device; skip nodes without minAPIVersion info.
                if let Ok(profile_json) = json5::from_str::<serde_json::Value>(&content) {
                    let app_min_api = profile_json
                        .pointer("/app/minAPIVersion")
                        .or_else(|| profile_json.pointer("/module/minAPIVersion"));
                    if let Some(min_api) = app_min_api.and_then(|v| v.as_u64()) {
                        if min_api > compatible_api as u64 {
                            return Err(ArkdownError::Bundler(format!(
                                "HAR_UNSUPPORTED_API_VERSION: dependency '{}' requires \
                                 minAPIVersion {}, current compatible API is {}",
                                node.package_name, min_api, compatible_api
                            )));
                        }
                    }
                }
            }
        }

        if dependency_profiles.is_empty() {
            return Ok(DependencyProfileMerge::default());
        }

        for profile in dependency_profiles.iter().filter(|profile| {
            !profile.metadata.is_empty() || !profile.request_permissions.is_empty()
        }) {
            Self::validate_dependency_module_profile(
                profile,
                &manifest.device_types,
                compatible_api,
            )?;
        }

        let current_metadata = Self::array_field(raw_module5, "metadata");
        let current_request_permissions = Self::array_field(raw_module5, "requestPermissions");

        let mut merge = DependencyProfileMerge::default();
        if dependency_profiles
            .iter()
            .any(|profile| !profile.metadata.is_empty())
        {
            merge.metadata = Some(Self::merge_named_arrays_from_dependencies(
                current_metadata,
                dependency_profiles
                    .iter()
                    .map(|profile| (profile.module_name.as_str(), profile.metadata.as_slice())),
                "metadata",
            ));
        }

        if dependency_profiles
            .iter()
            .any(|profile| !profile.request_permissions.is_empty())
        {
            let remove_permissions = Self::effective_remove_permission_set(
                &config.product.build_option.remove_permissions,
                &current_request_permissions,
            );
            merge.request_permissions = Some(Self::merge_request_permissions_from_dependencies(
                current_request_permissions,
                dependency_profiles.iter().map(|profile| {
                    (
                        profile.module_name.as_str(),
                        profile.request_permissions.as_slice(),
                    )
                }),
                &remove_permissions,
            ));
        }

        Ok(merge)
    }

    fn read_dependency_module_profiles(
        module_root: &Path,
        project_root: &Path,
    ) -> Result<Vec<DependencyModuleProfile>, ArkdownError> {
        let mut profiles = Vec::new();
        for package_dir in Self::collect_dependency_package_dirs(module_root, project_root)? {
            if let Some(profile) = Self::read_dependency_module_profile(&package_dir) {
                profiles.push(profile);
            }
        }
        Ok(profiles)
    }

    fn collect_dependency_package_dirs(
        module_root: &Path,
        project_root: &Path,
    ) -> Result<Vec<PathBuf>, ArkdownError> {
        let mut queue = VecDeque::new();
        let mut seen = FxHashSet::default();
        let mut packages = Vec::new();
        queue.push_back(module_root.to_path_buf());
        if project_root != module_root {
            queue.push_back(project_root.to_path_buf());
        }

        while let Some(pkg_root) = queue.pop_front() {
            let Some(pkg) = Self::read_dependency_package_json(&pkg_root) else {
                continue;
            };

            for dep_name in pkg.dependencies.keys() {
                let Some(dep_root) =
                    Self::resolve_dependency_root(&pkg_root, project_root, dep_name)
                else {
                    continue;
                };
                let key = arkdown_path::to_slash(&dep_root);
                if !seen.insert(key) {
                    continue;
                }
                queue.push_back(dep_root.clone());
                packages.push(dep_root);
            }
        }

        Ok(packages)
    }

    fn read_dependency_package_json(pkg_root: &Path) -> Option<DependencyPackageJson> {
        let content = std::fs::read_to_string(pkg_root.join("oh-package.json5")).ok()?;
        json5::from_str::<DependencyPackageJson>(&content).ok()
    }

    fn resolve_dependency_root(
        issuer_root: &Path,
        project_root: &Path,
        dep_name: &str,
    ) -> Option<PathBuf> {
        let mut current = Some(issuer_root);
        while let Some(root) = current {
            let candidate = root.join("oh_modules").join(dep_name);
            if candidate.is_dir() {
                return Some(candidate);
            }
            if root == project_root {
                break;
            }
            current = root.parent();
        }
        let candidate = project_root.join("oh_modules").join(dep_name);
        candidate.is_dir().then_some(candidate)
    }

    fn read_dependency_module_profile(package_dir: &Path) -> Option<DependencyModuleProfile> {
        for candidate in Self::dependency_module_json_candidates(package_dir) {
            if !candidate.is_file() {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&candidate) else {
                continue;
            };
            let Ok(raw) = json5::from_str::<serde_json::Value>(&content) else {
                continue;
            };
            let Some(module) = raw.get("module").and_then(|value| value.as_object()) else {
                continue;
            };
            let Some(module_type) = module.get("type").and_then(|value| value.as_str()) else {
                continue;
            };
            if module_type != "har" && module_type != "shared" {
                return None;
            }
            let app = raw.get("app").and_then(|value| value.as_object());
            let module_name = module
                .get("packageName")
                .or_else(|| module.get("name"))
                .and_then(|value| value.as_str())
                .map(str::to_string)
                .or_else(|| {
                    package_dir
                        .file_name()
                        .and_then(|name| name.to_str())
                        .map(str::to_string)
                })
                .unwrap_or_else(|| "unknown".to_string());
            let module_value = serde_json::Value::Object(module.clone());
            return Some(DependencyModuleProfile {
                module_name,
                module_type: module_type.to_string(),
                source_path: candidate,
                min_api_version: app
                    .and_then(|app| app.get("minAPIVersion"))
                    .and_then(Self::parse_dependency_min_api_version),
                device_types: module
                    .get("deviceTypes")
                    .and_then(|value| value.as_array())
                    .map(|values| {
                        values
                            .iter()
                            .filter_map(|value| value.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default(),
                metadata: Self::array_field(Some(&module_value), "metadata"),
                request_permissions: Self::array_field(Some(&module_value), "requestPermissions"),
            });
        }
        None
    }

    fn dependency_module_json_candidates(package_dir: &Path) -> Vec<PathBuf> {
        vec![
            package_dir.join("build/default/intermediates/merge_profile/default/module.json"),
            package_dir.join("build/default/intermediates/res/default/module.json"),
            package_dir.join("src/main/module.json5"),
            package_dir.join("src/main/module.json"),
            package_dir.join("module.json5"),
            package_dir.join("module.json"),
        ]
    }

    fn array_field(raw_module: Option<&serde_json::Value>, field: &str) -> Vec<serde_json::Value> {
        raw_module
            .and_then(|module| module.get(field))
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default()
    }

    fn effective_remove_permission_set(
        configured: &[String],
        current_request_permissions: &[serde_json::Value],
    ) -> FxHashSet<String> {
        let mut remove_permissions: FxHashSet<String> = configured.iter().cloned().collect();
        if remove_permissions.is_empty() {
            return remove_permissions;
        }

        let mut kept_current = Vec::new();
        for permission in current_request_permissions {
            let Some(name) = Self::object_name(permission) else {
                continue;
            };
            if remove_permissions.remove(name) {
                kept_current.push(name.to_string());
            }
        }
        if !kept_current.is_empty() {
            tracing::warn!(
                permissions = kept_current.join(", "),
                "permissions configured in removePermissions also exist in the current module and will not be removed"
            );
        }
        remove_permissions
    }

    fn merge_request_permissions_from_dependencies<'a, I>(
        current: Vec<serde_json::Value>,
        dependencies: I,
        remove_permissions: &FxHashSet<String>,
    ) -> Vec<serde_json::Value>
    where
        I: IntoIterator<Item = (&'a str, &'a [serde_json::Value])>,
    {
        let mut merged = Vec::new();
        let mut seen = FxHashSet::default();
        for permission in current {
            if let Some(name) = Self::object_name(&permission) {
                seen.insert(name.to_string());
            }
            merged.push(permission);
        }

        for (module_name, permissions) in dependencies {
            for permission in permissions {
                let Some(name) = Self::object_name(permission) else {
                    tracing::warn!(
                        module = module_name,
                        "dependency requestPermissions entry with empty name will not be merged"
                    );
                    continue;
                };
                if seen.contains(name) || remove_permissions.contains(name) {
                    continue;
                }
                seen.insert(name.to_string());
                merged.push(permission.clone());
            }
        }
        merged
    }

    fn merge_named_arrays_from_dependencies<'a, I>(
        current: Vec<serde_json::Value>,
        dependencies: I,
        field_name: &str,
    ) -> Vec<serde_json::Value>
    where
        I: IntoIterator<Item = (&'a str, &'a [serde_json::Value])>,
    {
        let mut merged = Vec::new();
        let mut seen = FxHashSet::default();
        for item in current {
            if let Some(name) = Self::object_name(&item) {
                seen.insert(name.to_string());
            }
            merged.push(item);
        }

        for (module_name, items) in dependencies {
            for item in items {
                let Some(name) = Self::object_name(item) else {
                    tracing::warn!(
                        module = module_name,
                        field = field_name,
                        "dependency profile array entry with empty name will not be merged"
                    );
                    continue;
                };
                if seen.contains(name) {
                    continue;
                }
                seen.insert(name.to_string());
                merged.push(item.clone());
            }
        }
        merged
    }

    fn object_name(value: &serde_json::Value) -> Option<&str> {
        value
            .get("name")
            .and_then(|name| name.as_str())
            .filter(|name| !name.is_empty())
    }

    fn validate_dependency_module_profile(
        profile: &DependencyModuleProfile,
        current_device_types: &[String],
        compatible_api: u32,
    ) -> Result<(), ArkdownError> {
        let Some(min_api_version) = profile.min_api_version else {
            return Err(ArkdownError::ParseError {
                file: profile.source_path.to_string_lossy().into_owned(),
                message: format!(
                    "EMPTY_MIN_API_VERSION: dependency {} app.minAPIVersion is required",
                    profile.module_name
                ),
            });
        };
        if compatible_api < min_api_version {
            return Err(ArkdownError::ParseError {
                file: profile.source_path.to_string_lossy().into_owned(),
                message: format!(
                    "HAR_UNSUPPORTED_API_VERSION: dependency {} requires minAPIVersion {}, current compatible API is {}",
                    profile.module_name, min_api_version, compatible_api
                ),
            });
        }

        if profile.module_type != "har" {
            Self::validate_dependency_device_types(profile, current_device_types)?;
        }
        Ok(())
    }

    fn validate_dependency_device_types(
        profile: &DependencyModuleProfile,
        current_device_types: &[String],
    ) -> Result<(), ArkdownError> {
        let mut supported: FxHashSet<&str> =
            profile.device_types.iter().map(String::as_str).collect();
        if supported.contains("default") || supported.contains("phone") {
            supported.insert("default");
            supported.insert("phone");
        }

        let unsupported: Vec<&str> = current_device_types
            .iter()
            .map(String::as_str)
            .filter(|device_type| !supported.contains(device_type))
            .collect();
        if unsupported.is_empty() {
            return Ok(());
        }

        Err(ArkdownError::ParseError {
            file: profile.source_path.to_string_lossy().into_owned(),
            message: format!(
                "CURRENT_MODULE_NOT_SUPPORT_THE_DEVICE_TYPE: dependency {} module type {} does not support {}",
                profile.module_name,
                profile.module_type,
                unsupported.join(", ")
            ),
        })
    }

    fn parse_dependency_min_api_version(value: &serde_json::Value) -> Option<u32> {
        let text = match value {
            serde_json::Value::Number(number) => number.to_string(),
            serde_json::Value::String(text) => text.clone(),
            _ => return None,
        };
        let candidate = if text.len() >= 4 {
            &text[text.len().saturating_sub(3)..]
        } else {
            text.as_str()
        };
        let digits: String = candidate
            .chars()
            .skip_while(|ch| !ch.is_ascii_digit())
            .take_while(|ch| ch.is_ascii_digit())
            .collect();
        digits.parse().ok()
    }

    /// Read the current effective target's `source.abilities` overrides from
    /// build-profile.json5 and apply hvigor's HAP-only merge-profile rule.
    fn read_target_ability_overrides(
        manifest: &HapManifest,
        config: &ArkdownConfig,
    ) -> Vec<TargetAbilityOverride> {
        let Some(module_cfg) = config
            .modules
            .iter()
            .find(|module| module.name == manifest.module_name)
        else {
            return Vec::new();
        };

        let Ok(profile) = RawBuildProfile::load(&config.project_root) else {
            return Vec::new();
        };
        let Some(raw_module) = profile
            .modules
            .into_iter()
            .find(|module| module.name == manifest.module_name)
        else {
            return Vec::new();
        };
        let build_mode_name = match config.build_mode {
            arkdown_core::BuildMode::Debug => "debug",
            arkdown_core::BuildMode::Release => "release",
        };
        // Use centralised target selection so all pipeline stages apply the
        // same priority: explicit buildModeSet targets take precedence over
        // catch-all "default" targets.
        let Some(target) = arkdown_config::build_profile::pick_effective_raw_target(
            &raw_module.targets,
            &config.product.name,
            build_mode_name,
        ) else {
            return Vec::new();
        };
        let Some(source) = target.source.as_ref() else {
            return Vec::new();
        };
        if source.abilities.is_empty() {
            return Vec::new();
        }

        if !matches!(
            module_cfg.module_type,
            OhosModuleType::Entry | OhosModuleType::Feature
        ) {
            if matches!(module_cfg.module_type, OhosModuleType::Shared) {
                tracing::warn!(
                    module = manifest.module_name,
                    "hsp module does not support custom icon/label/lauchType in buildProfile"
                );
            }
            return Vec::new();
        }

        let ability_names: FxHashSet<&str> = manifest
            .abilities
            .iter()
            .map(|ability| ability.name.as_str())
            .collect();
        source
            .abilities
            .iter()
            .filter_map(|ability| {
                if ability.name.is_empty() {
                    return None;
                }
                if !ability_names.contains(ability.name.as_str()) {
                    tracing::warn!(
                        ability = ability.name,
                        "The ability name in build-profile can't find correspond ability name in module.json"
                    );
                    return None;
                }
                Some(TargetAbilityOverride {
                    name: ability.name.clone(),
                    icon: ability.icon.clone(),
                    label: ability.label.clone(),
                    launch_type: ability.launch_type.clone(),
                })
            })
            .collect()
    }

    // Target selection methods (select_effective_target etc.) have been moved
    // to arkdown_config::build_profile::pick_effective_raw_target for centralisation.

    /// Resolve a `$type:name` resource reference to a numeric ID.
    ///
    /// Returns `None` for non-reference values or missing IDs.
    fn resolve_res_ref(value: &str, resource_ids: &FxHashMap<String, u32>) -> Option<u32> {
        let name = value.strip_prefix('$')?.splitn(2, ':').nth(1)?;
        resource_ids.get(name).copied()
    }

    /// Convert a version string like `"5.0.5(17)"` to the numeric API version
    /// that the HAP `module.json` requires: `A*1e7 + B*1e5 + C*1e3 + D`.
    /// Falls back to the digit-in-parens value for simple strings like `"12"`.
    pub(crate) fn compute_numeric_api_version(version: &str) -> u64 {
        let api_int: u64 = if let Some(start) = version.find('(') {
            let rest = &version[start + 1..];
            let end = rest.find(')').unwrap_or(rest.len());
            rest[..end].parse().unwrap_or(0)
        } else {
            version.parse().unwrap_or(0)
        };

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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_app_json5_src(
        version_code: u64,
        version_name: &str,
    ) -> serde_json::Map<String, serde_json::Value> {
        let mut m = serde_json::Map::new();
        m.insert("bundleName".into(), "com.example.app".into());
        m.insert("versionCode".into(), version_code.into());
        m.insert("versionName".into(), version_name.into());
        m
    }

    /// When no product-level overrides are present, `build_app_section` uses
    /// the values from `AppScope/app.json5` unchanged.
    #[test]
    fn test_build_app_section_uses_app_json5_when_no_override() {
        let src = make_app_json5_src(1_000_000, "1.0.0");
        let obj = ModuleJsonWriter::build_app_section(
            &src,
            12,
            true,
            "5.0.0",
            "com.example.app",
            false,
            None,
            None, // no version_code override
            None, // no version_name override
        );
        assert_eq!(
            obj.get("versionCode"),
            Some(&serde_json::Value::Number(1_000_000u64.into()))
        );
        assert_eq!(
            obj.get("versionName"),
            Some(&serde_json::Value::String("1.0.0".to_string()))
        );
    }

    /// When product-level overrides are present, `build_app_section` uses them
    /// and ignores the corresponding values from `AppScope/app.json5`.
    /// This implements hvigor's product-level app override semantics.
    #[test]
    fn test_build_app_section_product_version_overrides_app_json5() {
        let src = make_app_json5_src(1_000_000, "1.0.0");
        let obj = ModuleJsonWriter::build_app_section(
            &src,
            12,
            true,
            "5.0.0",
            "com.example.app",
            false,
            None,
            Some(2_000_000),     // product override: versionCode
            Some("2.0.0-alpha"), // product override: versionName
        );
        assert_eq!(
            obj.get("versionCode"),
            Some(&serde_json::Value::Number(2_000_000u64.into()))
        );
        assert_eq!(
            obj.get("versionName"),
            Some(&serde_json::Value::String("2.0.0-alpha".to_string()))
        );
    }

    /// When a product override is present for only one field, the other field
    /// still falls back to `AppScope/app.json5`.
    #[test]
    fn test_build_app_section_partial_version_override() {
        let src = make_app_json5_src(1_000_000, "1.0.0");
        // Only versionCode overridden; versionName comes from app.json5.
        let obj = ModuleJsonWriter::build_app_section(
            &src,
            12,
            false,
            "5.0.0",
            "com.example.app",
            false,
            None,
            Some(1_500_000),
            None,
        );
        assert_eq!(
            obj.get("versionCode"),
            Some(&serde_json::Value::Number(1_500_000u64.into()))
        );
        assert_eq!(
            obj.get("versionName"),
            Some(&serde_json::Value::String("1.0.0".to_string()))
        );
    }
}
