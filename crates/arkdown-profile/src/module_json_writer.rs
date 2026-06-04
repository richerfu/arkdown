//! Serialization of [`HapManifest`] and `app.json5` fields to the final
//! `module.json` format required by the HAP packager.
//!
//! The field insertion order in every JSON object **must exactly match
//! hvigor's canonical output** — this is verified by golden tests.
//! Do not use `#[serde(rename_all)]` derives here; order is controlled
//! manually via `serde_json::Map::insert` call order.

use rustc_hash::FxHashMap;
use std::path::Path;

use arkdown_core::{ArkdownConfig, ArkdownError, HapManifest};

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

    let app_section = ModuleJsonWriter::build_app_section(
        &app_json5_fields,
        numeric_api,
        is_debug,
        &config.sdk_config.compile_sdk_version,
        &config.product.bundle_name,
        resource_ids,
    );
    let module_section = ModuleJsonWriter::manifest_to_json(
        manifest,
        raw_module5.as_ref(),
        resource_ids,
        &package_name,
        &bundle_type,
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
    fn build_app_section(
        src: &serde_json::Map<String, serde_json::Value>,
        numeric_api: u64,
        is_debug: bool,
        compile_sdk_version: &str,
        bundle_name_fallback: &str,
        resource_ids: Option<&FxHashMap<String, u32>>,
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
        if let Some(v) = src.get("versionName") {
            obj.insert("versionName".into(), v.clone());
        }
        if let Some(v) = src.get("versionCode") {
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

        // Preserve-and-mutate passthrough: append any app.json5 `app` keys that
        // are not already emitted above.  Matches hvigor's cloneDeep + targeted
        // mutation approach so that `sanitizer`, `multiProjects`, `deviceTypes`,
        // `car`, `wearable`, `tablet`, `startWindowBackgroundColor`, and any
        // future fields survive the build without needing explicit support here.
        for (key, value) in src {
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
    ) -> serde_json::Value {
        // Build a name→raw-ability lookup for preserve-and-mutate.
        // Hvigor cloneDeep-copies the ability array and mutates selected fields
        // (resource IDs, srcEntry), preserving all other fields verbatim.
        let raw_abilities: Vec<serde_json::Map<String, serde_json::Value>> = raw_module5
            .and_then(|m| m.get("abilities"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_object().cloned())
                    .collect()
            })
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
                let icon_id = resource_ids
                    .zip(a.icon.as_deref())
                    .and_then(|(ids, r)| Self::resolve_res_ref(r, ids));
                let label_id = resource_ids
                    .zip(a.label.as_deref())
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

        // Build a name→raw-extensionAbility lookup for preserve-and-mutate.
        let raw_ext_abilities: Vec<serde_json::Map<String, serde_json::Value>> = raw_module5
            .and_then(|m| m.get("extensionAbilities"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_object().cloned())
                    .collect()
            })
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
        // requestPermissions: preserved verbatim from module.json5.
        if let Some(perms) = raw_module5.and_then(|v| v.get("requestPermissions")) {
            if perms.as_array().map_or(false, |a| !a.is_empty()) {
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
    fn read_raw_module5(manifest: &HapManifest, config: &ArkdownConfig) -> Option<serde_json::Value> {
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
