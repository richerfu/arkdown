use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use anyhow::Result;
use arkdown_core::{ArkdownConfig, ModuleConfig};
use arkdown_pack::minify_package_intermediate_module_json;
use indexmap::IndexMap;
use rustc_hash::FxHashSet;
use serde::Deserialize;

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
    // Use src/main/resources as the default module resources path when no
    // specific resource directories have been determined.  The hvigor parity
    // report reads resConfig.json to compare resource inputs; callers that
    // know the effective resource directories should call the variant that
    // accepts an explicit list.
    let default_res_dir = module.src_path.join("src/main/resources");
    write_resource_config_files_with_dirs(
        project_root,
        module,
        config,
        res_out,
        &[default_res_dir],
    )
}

/// Like [`write_resource_config_files`] but uses `module_res_dirs` for
/// `moduleResources` in `resConfig.json` instead of the default `src/main/resources`.
///
/// Callers (build.rs) should pass the effective resource directories after
/// target selection so the config matches what the resource compiler actually
/// processes.
pub(super) fn write_resource_config_files_with_dirs(
    project_root: &Path,
    module: &ModuleConfig,
    config: &ArkdownConfig,
    res_out: &Path,
    module_res_dirs: &[PathBuf],
) -> Result<()> {
    let project_root = canonical_or_owned(project_root);
    let module_root = canonical_or_owned(&module.src_path);
    let res_out = canonical_or_owned(res_out);
    let toolchains_dir = resource_toolchains_dir(&config.sdk_config.restool_path);

    // Build the dependencies list from declared HSP resource tables installed in oh_modules/.
    // Matches hvigor's DependencyManager-driven restool config generation closely enough
    // for local/transitive dependencies, while avoiding undeclared installed packages.
    let dependencies: Vec<serde_json::Value> =
        collect_dependency_resource_tables(&module_root, &project_root);

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
            "moduleResources": module_res_dirs.iter().map(|d| {
                serde_json::Value::String(path_string(&canonical_or_owned(d)))
            }).collect::<Vec<_>>(),
            "dependencies": dependencies,
            "iconCheck": true,
            "compression": path_string(&res_out.join("opt-compression.json")),
            "ids": path_string(&res_out.join("ids_map")),
            "definedIds": path_string(&res_out.join("ids_map/id_defined.json")),
            "definedSysIds": path_string(&toolchains_dir.join("id_defined.json")),
        }),
    )?;

    Ok(())
}

/// Collect ResourceTable.txt paths from declared HSP dependencies in oh_modules/.
///
/// Matches hvigor's restool config generation which includes dependency resource
/// tables so that HSP resource IDs can be resolved during HAP/HSP compilation.
fn collect_dependency_resource_tables(
    module_root: &Path,
    project_root: &Path,
) -> Vec<serde_json::Value> {
    let mut tables = Vec::new();
    for pkg_dir in collect_declared_dependency_roots(project_root, module_root) {
        // Check target intermediates first (locally-built HSP layout), then
        // fall back to the package root (remote HSP / pre-built layout).
        // Mirrors hvigor DependencyManager which prefers the target intermediates
        // ResourceTable.txt when the dependency was built in the same project.
        let resource_table = [
            pkg_dir.join("build/default/intermediates/res/default/ResourceTable.txt"),
            pkg_dir.join("ResourceTable.txt"),
        ]
        .into_iter()
        .find(|p| p.is_file());
        if let Some(resource_table) = resource_table {
            tables.push(serde_json::json!({
                "resourceTable": path_string(&resource_table),
            }));
        }
    }
    tables
}

#[derive(Deserialize)]
struct ResourceDependencyPackageJson {
    #[serde(default)]
    dependencies: IndexMap<String, String>,
}

fn collect_declared_dependency_roots(project_root: &Path, module_root: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let mut queue = VecDeque::new();
    let mut seen = FxHashSet::default();
    queue.push_back(module_root.to_path_buf());
    if project_root != module_root {
        queue.push_back(project_root.to_path_buf());
    }

    while let Some(pkg_root) = queue.pop_front() {
        let Some(pkg) = read_dependency_package_json(&pkg_root) else {
            continue;
        };
        for dep_name in pkg.dependencies.keys() {
            let Some(dep_root) = resolve_dependency_root(&pkg_root, project_root, dep_name) else {
                continue;
            };
            let dep_key = dep_root.to_string_lossy().into_owned();
            if !seen.insert(dep_key) {
                continue;
            }
            queue.push_back(dep_root.clone());
            roots.push(dep_root);
        }
    }

    roots
}

fn read_dependency_package_json(pkg_root: &Path) -> Option<ResourceDependencyPackageJson> {
    let content = std::fs::read_to_string(pkg_root.join("oh-package.json5")).ok()?;
    json5::from_str::<ResourceDependencyPackageJson>(&content).ok()
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

pub(super) fn write_static_intermediate_files(module_root: &Path) -> Result<()> {
    write_bytes(
        &module_root.join("build/default/intermediates/patch/default/base_native_libs.json"),
        br#"{"libs":{},"stripped":{}}"#,
    )?;
    write_bytes_if_missing(
        &module_root.join("build/default/intermediates/router_map/default/loader-router-map.json"),
        br#"{"routerMap":[]}"#,
    )?;
    write_bytes_if_missing(
        &module_root.join("build/default/intermediates/router_map/default/temp-router-map.json"),
        br#"{"routerMap":[]}"#,
    )?;
    // NOTE: router map files are written by write_router_map_intermediates()
    // before this function when router entries exist. Keep placeholders only
    // for modules without router data; overwriting real maps loses hvigor's
    // ProcessRouterMap output.
    // NOTE: temp-share-config.json is written by merge_share_config() before this
    // function is called. Do NOT write a placeholder here — it would overwrite the
    // real merged share config. (Bug fix: previously this overwrote the real config
    // with {"crossAppSharedConfig":[]}, causing crossAppSharedConfig to be lost.)
    Ok(())
}

pub(super) fn write_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs_err::create_dir_all(parent)?;
    }
    fs_err::write(path, bytes)?;
    Ok(())
}

fn write_bytes_if_missing(path: &Path, bytes: &[u8]) -> Result<()> {
    if path.exists() {
        return Ok(());
    }
    write_bytes(path, bytes)
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

pub(super) use arkdown_utils::canonicalize_or_owned as canonical_or_owned;

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

#[cfg(test)]
mod tests {
    use super::*;
    use arkdown_core::{
        BuildMode, BuildOption, OhosModuleType, OutputTarget, ProductConfig, SdkConfig,
        StrictModeConfig,
    };

    fn test_config(project_root: &Path) -> ArkdownConfig {
        ArkdownConfig {
            project_root: project_root.to_path_buf(),
            build_mode: BuildMode::Debug,
            output_target: OutputTarget::Hap,
            product: ProductConfig {
                name: "default".to_string(),
                bundle_name: "com.example.app".to_string(),
                compatible_sdk_version: "5.0.0(12)".to_string(),
                compatible_sdk_version_stage: None,
                runtime_os: "HarmonyOS".to_string(),
                signing_config: None,
                build_option: BuildOption {
                    strict_mode: StrictModeConfig {
                        case_sensitive_check: false,
                        use_normalized_ohm_url: true,
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
                ets_loader_path: project_root.join("sdk/ets-loader"),
                es2abc_path: project_root.join("sdk/es2abc"),
                restool_path: project_root.join("sdk/default/openharmony/toolchains/restool"),
                compile_api_version: 12,
                has_rollup_plugin: true,
                compile_sdk_version: "6.0.0.47".to_string(),
            },
        }
    }

    /// Regression test: write_static_intermediate_files() must NOT overwrite
    /// the share config written by merge_share_config().
    ///
    /// This was a P0 bug: previously temp-share-config.json was written as
    /// {"crossAppSharedConfig":[]} by write_static_intermediate_files() AFTER
    /// the real merged share config was written, losing all entries.
    #[test]
    fn test_write_static_intermediates_does_not_overwrite_share_config() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let module_root = tmp.path();

        // Simulate writing the real share config first.
        let share_config_path = module_root
            .join("build/default/intermediates/share_config/default/temp-share-config.json");
        std::fs::create_dir_all(share_config_path.parent().unwrap()).unwrap();
        std::fs::write(
            &share_config_path,
            r#"{"crossAppSharedConfig":[{"uri":"myapp://shared/Foo","packageName":"com.example.hsp"}]}"#,
        )
        .unwrap();

        // Now call write_static_intermediate_files.
        write_static_intermediate_files(module_root).expect("write_static_intermediate_files");

        // The share config should still have the real content, NOT be overwritten.
        let content = std::fs::read_to_string(&share_config_path).expect("read share config");
        assert!(
            content.contains("myapp://shared/Foo"),
            "share config must not be overwritten by write_static_intermediate_files; got: {content}"
        );
        assert!(
            !content.contains("[]"),
            "share config must not be empty after write_static_intermediate_files; got: {content}"
        );
    }

    #[test]
    fn test_write_static_intermediates_does_not_overwrite_router_map() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let module_root = tmp.path();

        let router_dir = module_root.join("build/default/intermediates/router_map/default");
        let loader_router_map = router_dir.join("loader-router-map.json");
        let temp_router_map = router_dir.join("temp-router-map.json");
        std::fs::create_dir_all(&router_dir).unwrap();
        let real_map = r#"{"routerMap":[{"name":"Home","ohmurl":"@bundle:com.example/entry/ets/pages/Home"}]}"#;
        std::fs::write(&loader_router_map, real_map).unwrap();
        std::fs::write(&temp_router_map, real_map).unwrap();

        write_static_intermediate_files(module_root).expect("write_static_intermediate_files");

        let loader_content =
            std::fs::read_to_string(&loader_router_map).expect("read loader router map");
        let temp_content = std::fs::read_to_string(&temp_router_map).expect("read temp router map");
        assert!(loader_content.contains("@bundle:com.example/entry/ets/pages/Home"));
        assert!(temp_content.contains("@bundle:com.example/entry/ets/pages/Home"));
        assert!(
            !loader_content.contains(r#""routerMap":[]"#),
            "loader router map must not be overwritten: {loader_content}"
        );
        assert!(
            !temp_content.contains(r#""routerMap":[]"#),
            "temp router map must not be overwritten: {temp_content}"
        );
    }

    #[test]
    fn test_resource_config_includes_dependency_resource_tables() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let project_root = tmp.path();
        let module_root = project_root.join("entry");
        let hsp_root = module_root.join("oh_modules/shared-hsp");
        let res_out = module_root.join("build/default/intermediates/res/default");
        std::fs::create_dir_all(&hsp_root).unwrap();
        std::fs::write(
            module_root.join("oh-package.json5"),
            r#"{ dependencies: { "shared-hsp": "1.0.0" } }"#,
        )
        .unwrap();
        std::fs::write(hsp_root.join("ResourceTable.txt"), "res table").unwrap();

        let module = ModuleConfig {
            name: "entry".to_string(),
            src_path: module_root.clone(),
            module_type: OhosModuleType::Entry,
        };
        write_resource_config_files(project_root, &module, &test_config(project_root), &res_out)
            .expect("write resource config");

        let content =
            std::fs::read_to_string(res_out.join("resConfig.json")).expect("read resConfig");
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let dependencies = parsed["dependencies"].as_array().unwrap();
        assert_eq!(dependencies.len(), 1);
        assert!(
            dependencies[0]["resourceTable"]
                .as_str()
                .unwrap()
                .ends_with("entry/oh_modules/shared-hsp/ResourceTable.txt"),
            "dependency resource table must be written into resConfig.json: {content}"
        );
    }

    #[test]
    fn test_resource_config_follows_declared_transitive_scoped_resource_tables() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let project_root = tmp.path();
        let module_root = project_root.join("entry");
        let source_har = module_root.join("oh_modules/source-har");
        let scoped_hsp = module_root.join("oh_modules/@scope/shared-hsp");
        let undeclared_hsp = module_root.join("oh_modules/undeclared-hsp");
        let res_out = module_root.join("build/default/intermediates/res/default");

        std::fs::create_dir_all(&source_har).unwrap();
        std::fs::create_dir_all(&scoped_hsp).unwrap();
        std::fs::create_dir_all(&undeclared_hsp).unwrap();
        std::fs::write(
            module_root.join("oh-package.json5"),
            r#"{ dependencies: { "source-har": "1.0.0" } }"#,
        )
        .unwrap();
        std::fs::write(
            source_har.join("oh-package.json5"),
            r#"{ dependencies: { "@scope/shared-hsp": "1.0.0" } }"#,
        )
        .unwrap();
        std::fs::write(scoped_hsp.join("ResourceTable.txt"), "scoped table").unwrap();
        std::fs::write(undeclared_hsp.join("ResourceTable.txt"), "undeclared table").unwrap();

        let module = ModuleConfig {
            name: "entry".to_string(),
            src_path: module_root.clone(),
            module_type: OhosModuleType::Entry,
        };
        write_resource_config_files(project_root, &module, &test_config(project_root), &res_out)
            .expect("write resource config");

        let content =
            std::fs::read_to_string(res_out.join("resConfig.json")).expect("read resConfig");
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let dependencies = parsed["dependencies"].as_array().unwrap();
        assert_eq!(dependencies.len(), 1);
        let resource_table = dependencies[0]["resourceTable"].as_str().unwrap();
        assert!(
            resource_table.ends_with("entry/oh_modules/@scope/shared-hsp/ResourceTable.txt"),
            "transitive scoped HSP table must be written into resConfig.json: {content}"
        );
        assert!(
            !content.contains("undeclared-hsp"),
            "undeclared HSP resource table must not be included: {content}"
        );
    }
}
