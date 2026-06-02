use std::path::{Path, PathBuf};

use anyhow::Result;
use arkdown_core::{ArkdownConfig, ModuleConfig};
use arkdown_pack::minify_package_intermediate_module_json;

use super::compile_arkts_modules::slash_path;

pub(super) fn copy_ets_assets_to_loader_out(
    module: &ModuleConfig,
    loader_json: &arkdown_profile::LoaderJson,
) -> Result<()> {
    let ets_root = module.src_path.join("src/main/ets");
    if !ets_root.exists() {
        return Ok(());
    }
    let out_root = PathBuf::from(&loader_json.ace_module_build);
    for entry in walkdir::WalkDir::new(&ets_root)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
    {
        let path = entry.path();
        let ext = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
        if !matches!(
            ext,
            "json" | "json5" | "png" | "jpg" | "jpeg" | "webp" | "gif" | "svg"
        ) {
            continue;
        }
        let Ok(rel) = path.strip_prefix(&ets_root) else {
            continue;
        };
        copy_if_exists(path, &out_root.join(rel))?;
    }
    Ok(())
}

pub(super) fn write_router_map_intermediates(
    module: &ModuleConfig,
    loader_json: &arkdown_profile::LoaderJson,
    config: &ArkdownConfig,
    res_out: &Path,
) -> Result<()> {
    if loader_json.router_map.is_empty() {
        return Ok(());
    }

    let router_map = serde_json::json!({
        "routerMap": loader_json
            .router_map
            .iter()
            .filter_map(|entry| normalized_router_map_entry(entry, loader_json, config))
            .collect::<Vec<_>>()
    });
    let bytes = serde_json::to_vec(&router_map)?;
    write_bytes(
        &module
            .src_path
            .join("build/default/intermediates/router_map/default/loader-router-map.json"),
        &bytes,
    )?;
    write_bytes(
        &module
            .src_path
            .join("build/default/intermediates/router_map/default/temp-router-map.json"),
        &bytes,
    )?;
    write_bytes(
        &res_out.join("resources/base/profile/default-router-map.json"),
        &bytes,
    )?;
    Ok(())
}

fn normalized_router_map_entry(
    entry: &serde_json::Value,
    loader_json: &arkdown_profile::LoaderJson,
    config: &ArkdownConfig,
) -> Option<serde_json::Value> {
    let obj = entry.as_object()?;
    let page_source = obj.get("pageSourceFile")?.as_str()?;
    let page_path = Path::new(page_source);
    let (package_name, module_root) =
        loader_json
            .module_path_map
            .iter()
            .find_map(|(name, root)| {
                let root = Path::new(root);
                page_path.starts_with(root).then_some((name, root))
            })?;
    let src_main = module_root.join("src/main");
    let rel = page_path
        .strip_prefix(&src_main)
        .ok()
        .map(|path| format!("src/main/{}", slash_path(path)))
        .unwrap_or_else(|| page_source.to_string());

    let mut normalized = serde_json::Map::new();
    if let Some(value) = obj.get("buildFunction") {
        normalized.insert("buildFunction".to_string(), value.clone());
    }
    if let Some(value) = obj.get("data") {
        normalized.insert("data".to_string(), value.clone());
    }
    if let Some(value) = obj.get("name") {
        normalized.insert("name".to_string(), value.clone());
    }
    normalized.insert(
        "moduleName".to_string(),
        serde_json::Value::String(loader_json.module_name.clone()),
    );
    normalized.insert(
        "bundleName".to_string(),
        serde_json::Value::String(config.product.bundle_name.clone()),
    );
    normalized.insert("pageSourceFile".to_string(), serde_json::Value::String(rel));
    normalized.insert(
        "packageName".to_string(),
        serde_json::Value::String(package_name.clone()),
    );
    if let Some(value) = obj.get("ohmurl") {
        normalized.insert("ohmurl".to_string(), value.clone());
    }
    Some(serde_json::Value::Object(normalized))
}

pub(super) fn write_profile_intermediates(
    module_root: &Path,
    final_module_json: &Path,
) -> Result<()> {
    let content = fs_err::read_to_string(final_module_json)?;
    let final_value: serde_json::Value = serde_json::from_str(&content)?;

    let process_value = profile_without_resource_ids(final_value.clone());
    write_json_pretty(
        &module_root.join("build/default/intermediates/process_profile/default/module.json"),
        &process_value,
    )?;
    write_json_minified(
        &module_root.join("build/default/intermediates/res/default/ark_module.json"),
        &process_value,
    )?;

    let mut merge_value = process_value;
    if let Some(module) = merge_value
        .get_mut("module")
        .and_then(|value| value.as_object_mut())
    {
        module.remove("virtualMachine");
        module.remove("compileMode");
        module.remove("dependencies");
    }
    write_json_pretty(
        &module_root.join("build/default/intermediates/merge_profile/default/module.json"),
        &merge_value,
    )?;

    Ok(())
}

fn profile_without_resource_ids(mut value: serde_json::Value) -> serde_json::Value {
    if let Some(app) = value.get_mut("app").and_then(|value| value.as_object_mut()) {
        app.remove("iconId");
        app.remove("labelId");
    }
    if let Some(module) = value
        .get_mut("module")
        .and_then(|value| value.as_object_mut())
    {
        module.remove("descriptionId");
        if let Some(abilities) = module
            .get_mut("abilities")
            .and_then(|value| value.as_array_mut())
        {
            for ability in abilities {
                if let Some(ability) = ability.as_object_mut() {
                    for field in [
                        "descriptionId",
                        "iconId",
                        "labelId",
                        "startWindowIconId",
                        "startWindowBackgroundId",
                    ] {
                        ability.remove(field);
                    }
                }
            }
        }
    }
    value
}

pub(super) fn write_resource_config_files(
    project_root: &Path,
    module: &ModuleConfig,
    config: &ArkdownConfig,
    res_out: &Path,
) -> Result<()> {
    let project_root = canonical_or_owned(project_root);
    let module_root = canonical_or_owned(&module.src_path);
    let res_out = canonical_or_owned(res_out);
    let toolchains_dir = resource_toolchains_dir(&config.sdk_config.restool_path);

    write_json_minified_newline(
        &res_out.join("opt-compression.json"),
        &serde_json::json!({
            "context": {
                "extensionPath": path_string(&toolchains_dir.join("lib/libimage_transcoder_shared.dylib")),
            },
            "compression": {
                "media": {
                    "enable": false,
                },
                "filters": [],
            },
        }),
    )?;

    write_json_minified_newline(
        &res_out.join("resConfig.json"),
        &serde_json::json!({
            "configPath": path_string(&module_root.join("build/default/intermediates/process_profile/default/module.json")),
            "packageName": config.product.bundle_name,
            "output": path_string(&res_out),
            "moduleNames": module.name,
            "ResourceTable": [
                path_string(&module_root.join("build/default/generated/r/default/ResourceTable.h")),
            ],
            "applicationResource": path_string(&project_root.join("AppScope/resources")),
            "moduleResources": [
                path_string(&module_root.join("src/main/resources")),
            ],
            "dependencies": [],
            "iconCheck": true,
            "compression": path_string(&res_out.join("opt-compression.json")),
            "ids": path_string(&res_out.join("ids_map")),
            "definedIds": path_string(&res_out.join("ids_map/id_defined.json")),
            "definedSysIds": path_string(&toolchains_dir.join("id_defined.json")),
        }),
    )?;

    Ok(())
}

pub(super) fn write_static_intermediate_files(module_root: &Path) -> Result<()> {
    write_bytes(
        &module_root.join("build/default/intermediates/patch/default/base_native_libs.json"),
        br#"{"libs":{},"stripped":{}}"#,
    )?;
    write_bytes(
        &module_root.join("build/default/intermediates/router_map/default/loader-router-map.json"),
        br#"{"routerMap":[]}"#,
    )?;
    write_bytes(
        &module_root.join("build/default/intermediates/router_map/default/temp-router-map.json"),
        br#"{"routerMap":[]}"#,
    )?;
    write_bytes(
        &module_root
            .join("build/default/intermediates/share_config/default/temp-share-config.json"),
        br#"{"crossAppSharedConfig":[]}"#,
    )?;
    Ok(())
}

pub(super) fn write_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs_err::create_dir_all(parent)?;
    }
    fs_err::write(path, bytes)?;
    Ok(())
}

pub(super) fn write_json_pretty(path: &Path, value: &serde_json::Value) -> Result<()> {
    let content = serde_json::to_string_pretty(value)?;
    write_bytes(path, content.as_bytes())
}

fn write_json_minified(path: &Path, value: &serde_json::Value) -> Result<()> {
    let content = serde_json::to_string(value)?;
    write_bytes(path, content.as_bytes())
}

fn write_json_minified_newline(path: &Path, value: &serde_json::Value) -> Result<()> {
    let mut content = serde_json::to_string(value)?;
    content.push('\n');
    write_bytes(path, content.as_bytes())
}

pub(super) fn rewrite_package_intermediate_module_json(path: &Path) -> Result<()> {
    let content = fs_err::read_to_string(path)?;
    let content = minify_package_intermediate_module_json(&content)
        .map_err(|err| anyhow::anyhow!("rewrite package module.json failed: {}", err))?;
    fs_err::write(path, content.as_bytes())?;
    Ok(())
}

pub(super) fn canonical_or_owned(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn resource_toolchains_dir(restool_path: &Path) -> PathBuf {
    let Some(toolchains_dir) = restool_path.parent() else {
        return PathBuf::new();
    };
    let Some(component_dir) = toolchains_dir.parent() else {
        return toolchains_dir.to_path_buf();
    };
    if component_dir.file_name().and_then(|name| name.to_str()) == Some("openharmony") {
        if let Some(sdk_default_dir) = component_dir.parent() {
            let hms_toolchains = sdk_default_dir.join("hms/toolchains");
            if hms_toolchains.exists() {
                return hms_toolchains;
            }
        }
    }
    toolchains_dir.to_path_buf()
}

pub(super) fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

pub(super) fn write_hap_metadata(
    module_root: &Path,
    module_name: &str,
    device_types: &[String],
) -> Result<()> {
    let metadata_path =
        module_root.join("build/default/intermediates/hap_metadata/default/output_metadata.json");
    if let Some(parent) = metadata_path.parent() {
        fs_err::create_dir_all(parent)?;
    }
    let device_types = if device_types.is_empty() {
        vec!["phone".to_string()]
    } else {
        device_types.to_vec()
    };
    let metadata = serde_json::json!([{
        "hapName": format!("{module_name}-default-signed.hap"),
        "deviceTypes": device_types,
        "isSigned": true,
        "dependRemoteHsps": [],
    }]);
    let mut content = serde_json::to_string(&metadata)?;
    content.push('\n');
    fs_err::write(metadata_path, content.as_bytes())?;
    Ok(())
}

pub(super) fn extract_zip_entry(
    archive_path: &Path,
    entry_name: &str,
    output_path: &Path,
) -> Result<()> {
    if let Some(parent) = output_path.parent() {
        fs_err::create_dir_all(parent)?;
    }
    let file = fs_err::File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut entry = archive.by_name(entry_name)?;
    let mut output = fs_err::File::create(output_path)?;
    std::io::copy(&mut entry, &mut output)?;
    Ok(())
}

pub(super) fn copy_if_exists(from: &Path, to: &Path) -> Result<()> {
    if !from.exists() {
        return Ok(());
    }
    if let Some(parent) = to.parent() {
        fs_err::create_dir_all(parent)?;
    }
    fs_err::copy(from, to)?;
    Ok(())
}
