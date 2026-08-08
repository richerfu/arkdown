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
use arkdown_path::{lexical_normalize, ArtifactPath, ArtifactPathSerializer, OhmUrl};
use indexmap::IndexMap;
use serde::Deserialize;

use crate::layout::{ProfileModuleLayout, ProfileProjectLayout};

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
    extension_abilities: Option<Vec<serde_json::Value>>,
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
    extension_abilities: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct DependencyPackageJson {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    dependencies: IndexMap<String, String>,
}

/// Encapsulates the private helper logic for building `module.json` content.
pub(crate) struct ModuleJsonWriter;

impl ModuleJsonWriter {
    pub(crate) fn write(
        manifest: &HapManifest,
        config: &ArkdownConfig,
        output_path: &Path,
        resource_ids: Option<&FxHashMap<String, u32>>,
    ) -> Result<(), ArkdownError> {
        Self::write_with_optional_dependency_graph(
            manifest,
            config,
            output_path,
            resource_ids,
            None,
        )
    }

    pub(crate) fn write_with_dependency_graph(
        manifest: &HapManifest,
        config: &ArkdownConfig,
        output_path: &Path,
        resource_ids: Option<&FxHashMap<String, u32>>,
        dependency_graph: &crate::dependency_graph::LocalDependencyGraph,
    ) -> Result<(), ArkdownError> {
        Self::write_with_optional_dependency_graph(
            manifest,
            config,
            output_path,
            resource_ids,
            Some(dependency_graph),
        )
    }

    fn write_with_optional_dependency_graph(
        manifest: &HapManifest,
        config: &ArkdownConfig,
        output_path: &Path,
        resource_ids: Option<&FxHashMap<String, u32>>,
        dependency_graph: Option<&crate::dependency_graph::LocalDependencyGraph>,
    ) -> Result<(), ArkdownError> {
        let app_json5_fields = Self::read_app_json5_fields(config)?;
        let project_multi_projects = Self::read_project_multi_projects(config);
        let min_numeric_api =
            Self::compute_numeric_api_version(&config.product.compatible_sdk_version);
        let target_numeric_api = Self::compute_numeric_target_api_version(
            config
                .product
                .target_sdk_version
                .as_deref()
                .unwrap_or(&config.sdk_config.compile_sdk_version),
            config.sdk_config.compile_api_version,
        );
        let is_debug = matches!(config.build_mode, arkdown_core::BuildMode::Debug);

        // Derive packageName from oh-package.json5#name for this module.
        // Falls back to module_name if the file is absent or unparseable.
        let package_name = Self::read_oh_package_name(manifest, config)
            .unwrap_or_else(|| manifest.module_name.clone());

        // Derive bundle_type for installationFree logic (from app.json5).
        let bundle_type = app_json5_fields
            .get("bundleType")
            .and_then(|v| v.as_str())
            .unwrap_or("app")
            .to_string();

        // Read raw module.json5 for preserve-and-mutate: unknown fields from the
        // source are appended verbatim after all known fields are emitted.
        let raw_module5 = Self::read_raw_module5(manifest, config);
        let target_ability_overrides = Self::read_target_ability_overrides(manifest, config);
        let dependency_profile_merge = Self::read_dependency_profile_merge(
            manifest,
            config,
            raw_module5.as_ref(),
            dependency_graph,
        )?;
        let has_insight_intent = Self::read_module_has_insight_intent(raw_module5.as_ref());

        let mut app_section = Self::build_app_section(
            &app_json5_fields,
            min_numeric_api,
            target_numeric_api,
            is_debug,
            &config.sdk_config.compile_sdk_version,
            &config.product.bundle_name,
            &config.product.app_options,
            project_multi_projects,
            resource_ids,
            config.product.version_code,
            config.product.version_name.as_deref(),
            config.product.build_version.as_deref(),
        );
        // MergeProfile.computeAppJsonOpt keeps only the HAR app whitelist.
        // `vendor` is retained for HAP/HSP but is not legal in a HAR's
        // processed package profile (except the ohosTest target).
        if matches!(manifest.module_type, arkdown_core::OhosModuleType::Har) {
            app_section.remove("vendor");
            app_section = Self::order_har_app_section(app_section);
        }
        let mut module_section = Self::manifest_to_json(
            manifest,
            raw_module5.as_ref(),
            resource_ids,
            &package_name,
            &bundle_type,
            &target_ability_overrides,
            &dependency_profile_merge,
            has_insight_intent,
        );
        if matches!(
            manifest.module_type,
            arkdown_core::OhosModuleType::Har | arkdown_core::OhosModuleType::Shared
        ) {
            module_section =
                Self::order_library_module_section(module_section, raw_module5.as_ref());
        }

        let mut combined = serde_json::Map::new();
        combined.insert("app".into(), serde_json::Value::Object(app_section));
        combined.insert("module".into(), module_section);

        if let Some(parent) = output_path.parent() {
            fs_err::create_dir_all(parent).map_err(|e| ArkdownError::Io(e.into()))?;
        }
        let mut json = Vec::new();
        let formatter = serde_json::ser::PrettyFormatter::with_indent(b"\t");
        let mut serializer = serde_json::Serializer::with_formatter(&mut json, formatter);
        serde::Serialize::serialize(&serde_json::Value::Object(combined), &mut serializer)
            .map_err(|e| ArkdownError::Io(std::io::Error::other(e.to_string())))?;
        fs_err::write(output_path, &json).map_err(|e| ArkdownError::Io(e.into()))?;

        Ok(())
    }

    fn order_har_app_section(
        mut app: serde_json::Map<String, serde_json::Value>,
    ) -> serde_json::Map<String, serde_json::Value> {
        let mut ordered = serde_json::Map::new();
        for key in [
            "bundleName",
            "debug",
            "versionCode",
            "versionName",
            "minCompatibleVersionCode",
            "minAPIVersion",
            "targetAPIVersion",
            "apiReleaseType",
            "targetMinorAPIVersion",
            "targetPatchAPIVersion",
            "compileSdkVersion",
            "compileSdkType",
            "appEnvironments",
            "bundleType",
            "buildMode",
        ] {
            if let Some(value) = app.remove(key) {
                ordered.insert(key.to_string(), value);
            }
        }
        ordered.extend(app);
        ordered
    }

    fn order_library_module_section(
        module: serde_json::Value,
        raw_module5: Option<&serde_json::Value>,
    ) -> serde_json::Value {
        let Some(mut module) = module.as_object().cloned() else {
            return module;
        };
        let mut ordered = serde_json::Map::new();
        if let Some(raw) = raw_module5.and_then(serde_json::Value::as_object) {
            for key in raw.keys() {
                if let Some(value) = module.remove(key) {
                    ordered.insert(key.clone(), value);
                }
            }
        }
        for key in [
            "packageName",
            "metadata",
            "requestPermissions",
            "installationFree",
            "virtualMachine",
            "compileMode",
            "dependencies",
        ] {
            if let Some(value) = module.remove(key) {
                ordered.insert(key.to_string(), value);
            }
        }
        ordered.extend(module);
        serde_json::Value::Object(ordered)
    }

    pub(crate) fn annotate_resource_ids(
        module_json_path: &Path,
        resource_ids: &FxHashMap<String, u32>,
    ) -> Result<(), ArkdownError> {
        if resource_ids.is_empty() {
            return Ok(());
        }
        let content =
            fs_err::read_to_string(module_json_path).map_err(|e| ArkdownError::Io(e.into()))?;
        let mut value: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| ArkdownError::Io(std::io::Error::other(e.to_string())))?;
        ModuleJsonResourceIdAnnotator::new(resource_ids).annotate(&mut value);
        let json = serde_json::to_string_pretty(&value)
            .map_err(|e| ArkdownError::Io(std::io::Error::other(e.to_string())))?;
        fs_err::write(module_json_path, json.as_bytes()).map_err(|e| ArkdownError::Io(e.into()))?;
        Ok(())
    }

    fn annotate_object_ref_id(
        object: &mut serde_json::Map<String, serde_json::Value>,
        source_key: &str,
        id_key: &str,
        resource_ids: &FxHashMap<String, u32>,
    ) {
        if object.contains_key(id_key) {
            return;
        }
        let Some(resource_ref) = object.get(source_key).and_then(|value| value.as_str()) else {
            return;
        };
        if let Some(id) = Self::resolve_res_ref(resource_ref, resource_ids) {
            object.insert(id_key.to_string(), (id as u64).into());
        }
    }

    /// Read the `app` object from `AppScope/app.json5`, falling back to minimal
    /// defaults derived from `config` when the file is absent.
    fn read_app_json5_fields(
        config: &ArkdownConfig,
    ) -> Result<serde_json::Map<String, serde_json::Value>, ArkdownError> {
        let app_json5_path = ProfileProjectLayout::new(&config.project_root).app_scope_app_json5();
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
        min_numeric_api: u64,
        target_numeric_api: u64,
        is_debug: bool,
        compile_sdk_version: &str,
        bundle_name_fallback: &str,
        app_options: &arkdown_core::AppOptionOverride,
        project_multi_projects: bool,
        resource_ids: Option<&FxHashMap<String, u32>>,
        version_code_override: Option<u64>,
        version_name_override: Option<&str>,
        build_version_override: Option<&str>,
    ) -> serde_json::Map<String, serde_json::Value> {
        let icon_ref = app_options
            .icon
            .clone()
            .or_else(|| src.get("icon").and_then(|v| v.as_str()).map(str::to_string));
        let label_ref = app_options.label.clone().or_else(|| {
            src.get("label")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        });

        let icon_id = resource_ids
            .zip(icon_ref.as_deref())
            .and_then(|(ids, r)| Self::resolve_res_ref(r, ids));
        let label_id = resource_ids
            .zip(label_ref.as_deref())
            .and_then(|(ids, r)| Self::resolve_res_ref(r, ids));

        // hvigor (`merge-profile.ts::computeAppJsonOpt`/`mergeDslConfig`/
        // `resolveApi`/`mergeAppEnvironmentConfig`) clones `app.json5` and
        // mutates in place, so the emitted field order is the app.json5 order
        // followed by the computed fields in mutation order. Start from the
        // source object to preserve that order; `insert` keeps an existing
        // key at its original position and appends new keys.
        // hvigor (`merge-profile.ts`) clones `app.json5` and mutates in
        // place, so the emitted order is the app.json5 order followed by the
        // computed fields in mutation order: `mergeDslConfig` appends
        // `apiReleaseType`, `resolveApi` appends the API versions,
        // `mergeAppEnvironmentConfig` appends `appEnvironments`, and the
        // build-mode pass appends `bundleType`/`buildMode`/`debug`.
        // `insert` keeps an existing key at its original position and appends
        // new keys, preserving that contract for app.json5 fields that are
        // also computed.
        let mut obj = src.clone();
        // hvigor `merge-profile.ts::computeAppJsonOpt` deletes an
        // AppScope/app.json5 `multiProjects` (it only accepts the
        // build-profile top-level `app.multiProjects`).
        obj.remove("multiProjects");
        obj.insert("apiReleaseType".into(), "Release".into());
        obj.insert("compileSdkVersion".into(), compile_sdk_version.into());
        obj.insert("targetAPIVersion".into(), target_numeric_api.into());
        obj.insert("minAPIVersion".into(), min_numeric_api.into());
        obj.insert("compileSdkType".into(), "HarmonyOS".into());
        obj.insert("targetMinorAPIVersion".into(), 0u64.into());
        obj.insert("targetPatchAPIVersion".into(), 0u64.into());
        obj.insert(
            "appEnvironments".into(),
            src.get("appEnvironments")
                .cloned()
                .unwrap_or_else(|| serde_json::Value::Array(vec![])),
        );
        obj.insert(
            "bundleType".into(),
            app_options
                .bundle_type
                .clone()
                .map(Into::into)
                .or_else(|| src.get("bundleType").cloned())
                .unwrap_or_else(|| "app".into()),
        );
        obj.insert(
            "buildMode".into(),
            if is_debug { "debug" } else { "release" }.into(),
        );
        obj.insert("debug".into(), is_debug.into());
        if let Some(v) = icon_ref.as_ref() {
            obj.insert("icon".into(), v.clone().into());
        }
        if let Some(v) = label_ref.as_ref() {
            obj.insert("label".into(), v.clone().into());
        }
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
        if let Some(v) = app_options
            .vendor
            .clone()
            .map(Into::into)
            .or_else(|| src.get("vendor").cloned())
        {
            obj.insert("vendor".into(), v);
        }
        obj.insert(
            "bundleName".into(),
            if bundle_name_fallback.is_empty() {
                src.get("bundleName").cloned().unwrap_or_default()
            } else {
                bundle_name_fallback.into()
            },
        );
        if let Some(id) = icon_id {
            obj.insert("iconId".into(), (id as u64).into());
        }
        if let Some(id) = label_id {
            obj.insert("labelId".into(), (id as u64).into());
        }
        if project_multi_projects {
            obj.insert("multiProjects".into(), true.into());
        }

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
        // `buildVersion` is kept at its app.json5 position (cloneDeep); a
        // newly supplied override is appended after the source object.
        for (key, value) in src {
            if key == "multiProjects" || key == "buildVersion" {
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
        if let Some(build_version) = build_version_override {
            // MergeProfile.getMergedAppOptObj assigns buildVersion after the
            // source app object has been cloned, so a newly supplied product
            // or invocation value is appended to the generated app object.
            // `insert` keeps the key at its app.json5 position when present
            // and appends it when absent.
            obj.insert("buildVersion".into(), build_version.into());
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
        has_insight_intent: bool,
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
                if !a.skills.is_empty() && !obj.contains_key("skills") {
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

        // hvigor (`merge-profile.ts`) clones `module.json5` and mutates in
        // place: the emitted order is the module.json5 order followed by the
        // computed fields in mutation order (`packageName` via Object.assign,
        // then `virtualMachine`/`compileMode` from profile processing, then
        // `dependencies`). Start from the raw module object so unknown fields
        // keep their positions; `insert` keeps existing keys in place.
        let mut obj = raw_module5
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();
        // packageName from oh-package.json5#name (hvigor: Object.assign using
        // packageJsonObj.name); falls back to module_name when file is absent.
        obj.insert("packageName".into(), package_name.into());
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
        // HAR's stage schema does not allow deliveryWithInstall. Hvigor's
        // MergeProfile only synthesizes this HAP/HSP distro field outside HAR.
        if !matches!(manifest.module_type, arkdown_core::OhosModuleType::Har) {
            let delivery_with_install = raw_module5
                .and_then(|v| v.get("deliveryWithInstall"))
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            obj.insert("deliveryWithInstall".into(), delivery_with_install.into());
        }
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
                dep.insert("moduleName".into(), d.module_name.clone().into());
                dep.insert("versionCode".into(), d.version_code.into());
                if let Some(bundle_name) = d.bundle_name.as_ref() {
                    dep.insert("bundleName".into(), bundle_name.clone().into());
                }
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
                    serde_json::Value::Array(Self::annotate_permission_reason_ids(
                        perms.clone(),
                        resource_ids,
                    )),
                );
            }
        } else if let Some(perms) = raw_module5.and_then(|v| v.get("requestPermissions")) {
            if perms.as_array().is_some_and(|a| !a.is_empty()) {
                let permissions = perms.as_array().cloned().unwrap_or_default();
                obj.insert(
                    "requestPermissions".into(),
                    serde_json::Value::Array(Self::annotate_permission_reason_ids(
                        permissions,
                        resource_ids,
                    )),
                );
            }
        }
        // definePermissions: preserved verbatim from module.json5.
        if let Some(defs) = raw_module5.and_then(|v| v.get("definePermissions")) {
            if defs.as_array().map_or(false, |a| !a.is_empty()) {
                obj.insert("definePermissions".into(), defs.clone());
            }
        }
        if !abilities.is_empty() {
            obj.insert("abilities".into(), serde_json::Value::Array(abilities));
        }
        let extension_abilities = if let Some(dependency_extensions) =
            dependency_profile_merge.extension_abilities.as_ref()
        {
            Self::merge_named_arrays_from_dependencies(
                extension_abilities,
                [("dependencies", dependency_extensions.as_slice())],
                "extensionAbilities",
            )
        } else {
            extension_abilities
        };
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
        if let Some(router_map) = &manifest.router_map {
            obj.insert("routerMap".into(), router_map.clone().into());
        }
        if let (Some(desc_ref), Some(ids)) = (manifest.description.as_deref(), resource_ids) {
            if let Some(id) = Self::resolve_res_ref(desc_ref, ids) {
                obj.insert("descriptionId".into(), (id as u64).into());
            }
        }
        if let Some(app_startup) = raw_module5.and_then(|value| value.get("appStartup")) {
            obj.insert("appStartup".into(), app_startup.clone());
        }
        obj.insert("name".into(), manifest.module_name.clone().into());

        if has_insight_intent {
            obj.insert("hasInsightIntent".into(), true.into());
        }

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

    fn read_module_has_insight_intent(raw_module5: Option<&serde_json::Value>) -> bool {
        raw_module5
            .and_then(|module| module.get("hasInsightIntent"))
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
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
        let path = ProfileModuleLayout::new(src_path).module_json5();
        let content = fs_err::read_to_string(&path).ok()?;
        let raw: serde_json::Value = json5::from_str(&content).ok()?;
        raw.get("module").cloned()
    }

    fn read_dependency_profile_merge(
        manifest: &HapManifest,
        config: &ArkdownConfig,
        raw_module5: Option<&serde_json::Value>,
        dependency_graph: Option<&crate::dependency_graph::LocalDependencyGraph>,
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
            use crate::dependency_graph::DependencyNodeType;
            let owned_graph;
            let graph = if let Some(graph) = dependency_graph {
                graph
            } else {
                owned_graph = crate::dependency_graph::LocalDependencyGraph::build(
                    &module_cfg.src_path,
                    &config.project_root,
                );
                &owned_graph
            };
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
                let profile_path = ProfileModuleLayout::new(&node.pkg_root).module_json5();
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
            !profile.metadata.is_empty()
                || !profile.request_permissions.is_empty()
                || !profile.extension_abilities.is_empty()
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

        if dependency_profiles
            .iter()
            .any(|profile| !profile.extension_abilities.is_empty())
        {
            merge.extension_abilities = Some(Self::merge_named_arrays_from_dependencies(
                Vec::new(),
                dependency_profiles.iter().map(|profile| {
                    (
                        profile.module_name.as_str(),
                        profile.extension_abilities.as_slice(),
                    )
                }),
                "extensionAbilities",
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
                if !seen.insert(dep_root.clone()) {
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
        let package_json = Self::read_dependency_package_json(package_dir);
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
                .or_else(|| package_json.as_ref().and_then(|pkg| pkg.name.clone()))
                .or_else(|| {
                    package_dir
                        .file_name()
                        .and_then(|name| name.to_str())
                        .map(str::to_string)
                })
                .unwrap_or_else(|| "unknown".to_string());
            let package_name = package_json
                .as_ref()
                .and_then(|pkg| pkg.name.clone())
                .unwrap_or_else(|| module_name.clone());
            let package_version = package_json
                .as_ref()
                .and_then(|pkg| pkg.version.clone())
                .unwrap_or_default();
            let module_value = serde_json::Value::Object(module.clone());
            let extension_abilities = Self::normalize_dependency_extension_abilities(
                package_dir,
                &package_name,
                &package_version,
                &candidate,
                Self::array_field(Some(&module_value), "extensionAbilities"),
            );
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
                extension_abilities,
            });
        }
        None
    }

    fn normalize_dependency_extension_abilities(
        package_dir: &Path,
        package_name: &str,
        package_version: &str,
        profile_path: &Path,
        extension_abilities: Vec<serde_json::Value>,
    ) -> Vec<serde_json::Value> {
        extension_abilities
            .into_iter()
            .map(|mut ability| {
                let Some(src_entry) = ability
                    .get("srcEntry")
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
                else {
                    return ability;
                };
                if Self::dependency_src_entry_is_runtime_url(&src_entry) {
                    return ability;
                }
                let Some(normalized) = Self::normalized_dependency_src_entry(
                    package_dir,
                    package_name,
                    package_version,
                    profile_path,
                    &src_entry,
                ) else {
                    return ability;
                };
                if let Some(object) = ability.as_object_mut() {
                    object.insert(
                        "srcEntry".to_string(),
                        serde_json::Value::String(normalized),
                    );
                }
                ability
            })
            .collect()
    }

    fn dependency_src_entry_is_runtime_url(src_entry: &str) -> bool {
        src_entry.starts_with('@')
    }

    fn normalized_dependency_src_entry(
        package_dir: &Path,
        package_name: &str,
        package_version: &str,
        profile_path: &Path,
        src_entry: &str,
    ) -> Option<String> {
        let profile_dir = profile_path.parent()?;
        let package_dir = lexical_normalize(package_dir);
        let source_path = lexical_normalize(&profile_dir.join(src_entry));
        let rel_source = source_path.strip_prefix(&package_dir).ok()?;
        let mut rel_stem = rel_source.to_path_buf();
        rel_stem.set_extension("");
        let rel_stem = ArtifactPathSerializer::rel_to_slash(&rel_stem);
        let normalized_import = ArtifactPath::from_slash(package_name)
            .join(&rel_stem)
            .into_string();
        Some(OhmUrl::normalized_non_so("", "", &normalized_import, package_version).into_string())
    }

    fn dependency_module_json_candidates(package_dir: &Path) -> Vec<PathBuf> {
        let layout = ProfileModuleLayout::new(package_dir);
        vec![
            layout.merge_profile_module_json(),
            layout.res_module_json(),
            layout.module_json5(),
            layout.module_json(),
            layout.package_module_json5(),
            layout.package_module_json(),
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

    fn annotate_permission_reason_ids(
        permissions: Vec<serde_json::Value>,
        resource_ids: Option<&FxHashMap<String, u32>>,
    ) -> Vec<serde_json::Value> {
        let Some(resource_ids) = resource_ids else {
            return permissions;
        };

        permissions
            .into_iter()
            .map(|mut permission| {
                let Some(object) = permission.as_object_mut() else {
                    return permission;
                };
                if object.contains_key("reasonId") {
                    return permission;
                }
                let Some(reason) = object.get("reason").and_then(|value| value.as_str()) else {
                    return permission;
                };
                if let Some(id) = Self::resolve_res_ref(reason, resource_ids) {
                    object.insert("reasonId".to_string(), (id as u64).into());
                }
                permission
            })
            .collect()
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
                file: ModuleJsonPathText::new(&profile.source_path).string(),
                message: format!(
                    "EMPTY_MIN_API_VERSION: dependency {} app.minAPIVersion is required",
                    profile.module_name
                ),
            });
        };
        if compatible_api < min_api_version {
            return Err(ArkdownError::ParseError {
                file: ModuleJsonPathText::new(&profile.source_path).string(),
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
            file: ModuleJsonPathText::new(&profile.source_path).string(),
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
        let reference = ResourceIdReference::parse(value)?;
        resource_ids
            .get(reference.name())
            .or_else(|| resource_ids.get(&reference.runtime_key()))
            .copied()
    }

    /// Convert a version string like `"5.0.5(17)"` to the numeric API version
    /// that the HAP `module.json` requires: `A*1e7 + B*1e5 + C*1e3 + D`.
    /// Falls back to the digit-in-parens value for simple strings like `"12"`.
    pub(crate) fn compute_numeric_api_version(version: &str) -> u64 {
        CompileSdkVersionText::new(version).numeric_api_version()
    }

    pub(crate) fn compute_numeric_target_api_version(
        compile_sdk_version: &str,
        target_api_version: u32,
    ) -> u64 {
        CompileSdkVersionText::new(compile_sdk_version)
            .numeric_target_api_version(target_api_version)
    }
}

struct CompileSdkVersionText<'a> {
    raw: &'a str,
}

impl<'a> CompileSdkVersionText<'a> {
    const MAJOR_FACTOR: u64 = 10_000_000;
    const MINOR_FACTOR: u64 = 100_000;
    const PATCH_FACTOR: u64 = 1_000;

    fn new(raw: &'a str) -> Self {
        Self { raw }
    }

    fn numeric_api_version(&self) -> u64 {
        let api_int = self
            .parenthesized_api()
            .unwrap_or_else(|| self.raw.parse().unwrap_or(0));
        self.numeric_version(api_int)
    }

    fn numeric_target_api_version(&self, target_api_version: u32) -> u64 {
        self.numeric_version(target_api_version as u64)
    }

    fn numeric_version(&self, api: u64) -> u64 {
        let (major, minor, patch) = self.version_parts();
        major * Self::MAJOR_FACTOR + minor * Self::MINOR_FACTOR + patch * Self::PATCH_FACTOR + api
    }

    fn parenthesized_api(&self) -> Option<u64> {
        let start = self.raw.find('(')?;
        let rest = &self.raw[start + 1..];
        let end = rest.find(')').unwrap_or(rest.len());
        rest[..end].parse().ok()
    }

    fn version_parts(&self) -> (u64, u64, u64) {
        let prefix = self
            .raw
            .split('(')
            .next()
            .unwrap_or("")
            .trim_end_matches('.');
        let mut parts = prefix.split('.').filter_map(|part| part.parse().ok());
        (
            parts.next().unwrap_or(0),
            parts.next().unwrap_or(0),
            parts.next().unwrap_or(0),
        )
    }
}

struct ResourceIdReference<'a> {
    resource_type: &'a str,
    name: &'a str,
}

impl<'a> ResourceIdReference<'a> {
    const PREFIX: char = '$';
    const SEPARATOR: char = ':';

    fn parse(value: &'a str) -> Option<Self> {
        let value = value.strip_prefix(Self::PREFIX)?;
        let (resource_type, name) = value.split_once(Self::SEPARATOR)?;
        Some(Self {
            resource_type,
            name,
        })
    }

    fn name(&self) -> &'a str {
        self.name
    }

    fn runtime_key(&self) -> String {
        let mut key = String::with_capacity(self.resource_type.len() + 1 + self.name.len());
        key.push_str(self.resource_type);
        key.push('.');
        key.push_str(self.name);
        key
    }
}

struct ModuleJsonResourceIdAnnotator<'a> {
    resource_ids: &'a FxHashMap<String, u32>,
}

impl<'a> ModuleJsonResourceIdAnnotator<'a> {
    fn new(resource_ids: &'a FxHashMap<String, u32>) -> Self {
        Self { resource_ids }
    }

    fn annotate(&self, value: &mut serde_json::Value) {
        self.annotate_app(value);
        self.annotate_module(value);
    }

    fn annotate_app(&self, value: &mut serde_json::Value) {
        let Some(app) = value.get_mut("app").and_then(|value| value.as_object_mut()) else {
            return;
        };
        ModuleJsonWriter::annotate_object_ref_id(app, "icon", "iconId", self.resource_ids);
        ModuleJsonWriter::annotate_object_ref_id(app, "label", "labelId", self.resource_ids);
    }

    fn annotate_module(&self, value: &mut serde_json::Value) {
        let Some(module) = value
            .get_mut("module")
            .and_then(|value| value.as_object_mut())
        else {
            return;
        };
        ModuleJsonWriter::annotate_object_ref_id(
            module,
            "description",
            "descriptionId",
            self.resource_ids,
        );
        self.annotate_abilities(module);
        self.annotate_extension_abilities(module);
        self.annotate_request_permissions(module);
    }

    fn annotate_abilities(&self, module: &mut serde_json::Map<String, serde_json::Value>) {
        let Some(abilities) = module
            .get_mut("abilities")
            .and_then(|value| value.as_array_mut())
        else {
            return;
        };
        for ability in abilities {
            let Some(object) = ability.as_object_mut() else {
                continue;
            };
            ModuleJsonWriter::annotate_object_ref_id(object, "icon", "iconId", self.resource_ids);
            ModuleJsonWriter::annotate_object_ref_id(object, "label", "labelId", self.resource_ids);
            ModuleJsonWriter::annotate_object_ref_id(
                object,
                "description",
                "descriptionId",
                self.resource_ids,
            );
            ModuleJsonWriter::annotate_object_ref_id(
                object,
                "startWindowIcon",
                "startWindowIconId",
                self.resource_ids,
            );
            ModuleJsonWriter::annotate_object_ref_id(
                object,
                "startWindowBackground",
                "startWindowBackgroundId",
                self.resource_ids,
            );
        }
    }

    fn annotate_extension_abilities(
        &self,
        module: &mut serde_json::Map<String, serde_json::Value>,
    ) {
        let Some(extension_abilities) = module
            .get_mut("extensionAbilities")
            .and_then(|value| value.as_array_mut())
        else {
            return;
        };
        for ability in extension_abilities {
            let Some(object) = ability.as_object_mut() else {
                continue;
            };
            ModuleJsonWriter::annotate_object_ref_id(object, "label", "labelId", self.resource_ids);
            ModuleJsonWriter::annotate_object_ref_id(
                object,
                "description",
                "descriptionId",
                self.resource_ids,
            );
            self.annotate_metadata(object);
        }
    }

    fn annotate_metadata(&self, object: &mut serde_json::Map<String, serde_json::Value>) {
        let Some(metadata) = object
            .get_mut("metadata")
            .and_then(|value| value.as_array_mut())
        else {
            return;
        };
        for entry in metadata {
            let Some(entry_object) = entry.as_object_mut() else {
                continue;
            };
            ModuleJsonWriter::annotate_object_ref_id(
                entry_object,
                "resource",
                "resourceId",
                self.resource_ids,
            );
        }
    }

    fn annotate_request_permissions(
        &self,
        module: &mut serde_json::Map<String, serde_json::Value>,
    ) {
        let Some(permissions) = module
            .get_mut("requestPermissions")
            .and_then(|value| value.as_array_mut())
        else {
            return;
        };
        for permission in permissions {
            let Some(object) = permission.as_object_mut() else {
                continue;
            };
            ModuleJsonWriter::annotate_object_ref_id(
                object,
                "reason",
                "reasonId",
                self.resource_ids,
            );
        }
    }
}

struct ModuleJsonPathText<'a> {
    path: &'a Path,
}

impl<'a> ModuleJsonPathText<'a> {
    fn new(path: &'a Path) -> Self {
        Self { path }
    }

    fn string(&self) -> String {
        ArtifactPathSerializer::path_to_slash_string(self.path)
            .unwrap_or_else(|_| self.path.display().to_string())
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
            20,
            true,
            "5.0.0",
            "com.example.app",
            &arkdown_core::AppOptionOverride::default(),
            false,
            None,
            None, // no version_code override
            None, // no version_name override
            None, // no build_version override
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
            20,
            true,
            "5.0.0",
            "com.example.app",
            &arkdown_core::AppOptionOverride::default(),
            false,
            None,
            Some(2_000_000),     // product override: versionCode
            Some("2.0.0-alpha"), // product override: versionName
            None,
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

    #[test]
    fn test_build_app_section_applies_hvigor_app_option_overlay() {
        let mut src = make_app_json5_src(1_000_000, "1.0.0");
        src.insert("vendor".into(), "source-vendor".into());
        src.insert("bundleType".into(), "app".into());
        src.insert("icon".into(), "$media:source".into());
        src.insert("label".into(), "$string:source".into());
        let app_options = arkdown_core::AppOptionOverride {
            vendor: Some("script-vendor".to_string()),
            bundle_type: Some("atomicService".to_string()),
            icon: Some("$media:script".to_string()),
            label: Some("$string:script".to_string()),
        };
        let obj = ModuleJsonWriter::build_app_section(
            &src,
            12,
            20,
            true,
            "5.0.0",
            "com.example.script",
            &app_options,
            false,
            None,
            None,
            None,
            None,
        );

        assert_eq!(obj["bundleName"], "com.example.script");
        assert_eq!(obj["vendor"], "script-vendor");
        assert_eq!(obj["bundleType"], "atomicService");
        assert_eq!(obj["icon"], "$media:script");
        assert_eq!(obj["label"], "$string:script");
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
            20,
            false,
            "5.0.0",
            "com.example.app",
            &arkdown_core::AppOptionOverride::default(),
            false,
            None,
            Some(1_500_000),
            None,
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

    #[test]
    fn test_build_app_section_splits_min_and_target_api() {
        let src = make_app_json5_src(1_000_000, "1.0.0");
        let obj = ModuleJsonWriter::build_app_section(
            &src,
            60000020,
            60001021,
            false,
            "6.0.1.112",
            "com.example.app",
            &arkdown_core::AppOptionOverride::default(),
            false,
            None,
            None,
            None,
            None,
        );

        assert_eq!(
            obj.get("minAPIVersion"),
            Some(&serde_json::Value::Number(60000020u64.into()))
        );
        assert_eq!(
            obj.get("targetAPIVersion"),
            Some(&serde_json::Value::Number(60001021u64.into()))
        );
    }

    #[test]
    fn test_build_app_section_build_version_uses_product_then_app_precedence() {
        let mut src = make_app_json5_src(1_000_000, "1.0.0");
        src.insert("buildVersion".into(), "app-build".into());
        let from_app = ModuleJsonWriter::build_app_section(
            &src,
            12,
            20,
            true,
            "5.0.0",
            "com.example.app",
            &arkdown_core::AppOptionOverride::default(),
            false,
            None,
            None,
            None,
            None,
        );
        let from_product = ModuleJsonWriter::build_app_section(
            &src,
            12,
            20,
            true,
            "5.0.0",
            "com.example.app",
            &arkdown_core::AppOptionOverride::default(),
            false,
            None,
            None,
            None,
            Some("product-build"),
        );

        assert_eq!(from_app["buildVersion"], "app-build");
        assert_eq!(from_product["buildVersion"], "product-build");
        // An app.json5 buildVersion keeps its position (`cloneDeep` +
        // targeted mutation); a newly supplied override for a source object
        // without buildVersion is appended.
        let position = from_product
            .keys()
            .position(|key| key == "buildVersion")
            .expect("buildVersion present");
        let source_position = src.keys().position(|key| key == "buildVersion").unwrap();
        assert_eq!(
            position, source_position,
            "an existing app.json5 buildVersion keeps its original position"
        );

        let mut without = make_app_json5_src(1_000_000, "1.0.0");
        let from_override = ModuleJsonWriter::build_app_section(
            &without,
            12,
            20,
            true,
            "5.0.0",
            "com.example.app",
            &arkdown_core::AppOptionOverride::default(),
            false,
            None,
            None,
            None,
            Some("cli-build"),
        );
        assert_eq!(from_override["buildVersion"], "cli-build");
        assert_eq!(
            from_override.keys().last().map(String::as_str),
            Some("buildVersion"),
            "a newly supplied buildVersion is appended after the source object"
        );
        let _ = &mut without;
    }
}
