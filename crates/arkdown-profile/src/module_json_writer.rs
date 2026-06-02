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
    let numeric_api = ModuleJsonWriter::compute_numeric_api_version(&config.product.compatible_sdk_version);
    let is_debug = matches!(config.build_mode, arkdown_core::BuildMode::Debug);

    let app_section = ModuleJsonWriter::build_app_section(
        &app_json5_fields,
        numeric_api,
        is_debug,
        &config.sdk_config.compile_sdk_version,
        &config.product.bundle_name,
        resource_ids,
    );
    let module_section = ModuleJsonWriter::manifest_to_json(manifest, resource_ids);

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
        obj
    }

    /// Build the `module` JSON object from a processed [`HapManifest`].
    ///
    /// Field order matches hvigor's canonical output (verified against golden):
    ///   virtualMachine, mainElement, installationFree, deliveryWithInstall, description,
    ///   compileMode, type, dependencies, abilities, deviceTypes, pages, descriptionId,
    ///   name, packageName
    fn manifest_to_json(
        manifest: &HapManifest,
        resource_ids: Option<&FxHashMap<String, u32>>,
    ) -> serde_json::Value {
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

                let mut obj = serde_json::Map::new();
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
        obj.insert("installationFree".into(), false.into());
        obj.insert("deliveryWithInstall".into(), true.into());
        if let Some(desc) = &manifest.description {
            obj.insert("description".into(), desc.clone().into());
        }
        obj.insert(
            "compileMode".into(),
            manifest.compile_mode.to_string().into(),
        );
        obj.insert("type".into(), manifest.module_type.to_string().into());
        obj.insert("dependencies".into(), serde_json::Value::Array(vec![]));
        obj.insert("abilities".into(), serde_json::Value::Array(abilities));
        if !extension_abilities.is_empty() {
            obj.insert(
                "extensionAbilities".into(),
                serde_json::Value::Array(extension_abilities),
            );
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
        obj.insert("packageName".into(), manifest.module_name.clone().into());

        serde_json::Value::Object(obj)
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
