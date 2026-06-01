use super::compile_arkts_modules::{
    compile_module_info, find_package_root, package_entry_path, pkg_modules_package_name,
    slash_path, strip_source_extension, symlink_pkg_module_aliases, CompileModuleInfo,
};
use super::*;

#[derive(Default)]
pub(super) struct PackageEntryInfos {
    entries: Vec<(String, String)>,
    seen: HashSet<String>,
}

impl PackageEntryInfos {
    pub(super) fn insert(&mut self, entry_path: impl Into<String>, build_path: impl Into<String>) {
        let entry_path = entry_path.into();
        if self.seen.insert(entry_path.clone()) {
            self.entries.push((entry_path, build_path.into()));
        }
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &(String, String)> {
        self.entries.iter()
    }
}

pub(super) fn add_graph_package_entry(
    entries: &mut PackageEntryInfos,
    info: &CompileModuleInfo,
    loader_json: &arkdown_profile::LoaderJson,
) {
    if is_local_compile_module(&info.source_path, loader_json) {
        add_local_dynamic_entry_aliases(entries, info, loader_json);
        return;
    }

    add_remote_package_entry(entries, &info.source_path, loader_json);
}

pub(super) fn add_dynamic_import_package_entries(
    entries: &mut PackageEntryInfos,
    loader_json: &arkdown_profile::LoaderJson,
    config: &ArkdownConfig,
) {
    for (dep_name, value) in &loader_json.dynamic_import_lib_info {
        if let Some(app_path) = native_shared_object_entry(dep_name, loader_json, config) {
            entries.insert(dep_name.clone(), app_path);
            continue;
        }

        let is_local = value
            .get("isLocalDependency")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let entry_file = value
            .get("entryFilePath")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if entry_file.is_empty() {
            continue;
        }
        let entry_path = canonical_or_owned(Path::new(entry_file));

        if is_local {
            if let Some(local_info) =
                local_compile_info_for_dynamic_entry(&entry_path, loader_json, config)
            {
                add_local_dynamic_entry_aliases(entries, &local_info, loader_json);
            }
        } else {
            add_remote_package_entry(entries, &entry_path, loader_json);
        }
    }
}

fn add_local_dynamic_entry_aliases(
    entries: &mut PackageEntryInfos,
    info: &CompileModuleInfo,
    loader_json: &arkdown_profile::LoaderJson,
) {
    let Some(host_infos) = dynamic_host_infos_for_entry(&info.source_path, loader_json) else {
        return;
    };
    for host in host_infos {
        let host_dependency_name = host
            .get("hostDependencyName")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let host_module_name = host
            .get("hostModuleName")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if host_dependency_name.is_empty() || host_module_name.is_empty() {
            continue;
        }
        entries.insert(
            format!("pkg_modules@{host_module_name}/{host_dependency_name}"),
            info.record_name.clone(),
        );
        entries.insert(
            host_dependency_name.to_string(),
            format!("@bundle.{}", info.record_name),
        );
    }
}

fn dynamic_host_infos_for_entry<'a>(
    entry_path: &Path,
    loader_json: &'a arkdown_profile::LoaderJson,
) -> Option<&'a Vec<serde_json::Value>> {
    let entry_path = canonical_or_owned(entry_path);
    for value in loader_json.dynamic_import_lib_info.values() {
        let Some(dynamic_entry_file) = value.get("entryFilePath").and_then(|v| v.as_str()) else {
            continue;
        };
        if canonical_or_owned(Path::new(dynamic_entry_file)) != entry_path {
            continue;
        }
        return value.get("hostModulesInfo").and_then(|v| v.as_array());
    }
    None
}

fn local_compile_info_for_dynamic_entry(
    entry_path: &Path,
    loader_json: &arkdown_profile::LoaderJson,
    config: &ArkdownConfig,
) -> Option<CompileModuleInfo> {
    let mode_dir = match config.build_mode {
        BuildMode::Debug => "debug",
        BuildMode::Release => "release",
    };
    let module_root = loader_json.module_path_map.get(&loader_json.module_name)?;
    let cache_root = Path::new(module_root)
        .join("build/default/cache/default/default@CompileArkTS/esmodule")
        .join(mode_dir);
    compile_module_info(entry_path, &cache_root, loader_json, config)
}

fn add_remote_package_entry(
    entries: &mut PackageEntryInfos,
    source_path: &Path,
    loader_json: &arkdown_profile::LoaderJson,
) {
    let Some(package_root) = find_package_root(source_path) else {
        return;
    };
    let Some(package_entry_name) = pkg_modules_package_name(&package_root, loader_json) else {
        return;
    };
    let Ok(rel_to_pkg) = source_path.strip_prefix(&package_root) else {
        return;
    };
    let build_path = format!(
        "{}/{}",
        package_entry_name,
        strip_source_extension(&slash_path(rel_to_pkg))
    );
    entries.insert(package_entry_name.clone(), build_path);

    for alias in symlink_pkg_module_aliases(&package_root, loader_json) {
        entries.insert(alias, package_entry_name.clone());
    }
}

fn native_shared_object_entry(
    dep_name: &str,
    loader_json: &arkdown_profile::LoaderJson,
    config: &ArkdownConfig,
) -> Option<String> {
    let stem = dep_name.strip_prefix("lib")?.strip_suffix(".so")?;
    Some(format!(
        "@app.{}/{}/{}",
        config.product.bundle_name, loader_json.module_name, stem
    ))
}

pub(super) fn add_native_package_entries(entries: &mut PackageEntryInfos) {
    for name in [
        "system.app",
        "ohos.app",
        "system.router",
        "system.curves",
        "ohos.curves",
        "system.matrix4",
        "ohos.matrix4",
    ] {
        entries.insert(format!("@{name}"), format!("@native.{name}"));
    }
}

fn is_local_compile_module(source_path: &Path, loader_json: &arkdown_profile::LoaderJson) -> bool {
    let source_path = canonical_or_owned(source_path);
    loader_json.module_path_map.values().any(|module_root| {
        let module_root = canonical_or_owned(Path::new(module_root));
        source_path.starts_with(&module_root)
            && !source_path.components().any(|component| {
                let name = component.as_os_str().to_string_lossy();
                name == "oh_modules" || name == "node_modules"
            })
    })
}

pub(super) fn is_package_entry_file(source_path: &Path) -> bool {
    let source_path = canonical_or_owned(source_path);
    let Some(package_root) = find_package_root(&source_path) else {
        return false;
    };
    package_entry_path(&package_root)
        .map(|entry| canonical_or_owned(&entry) == source_path)
        .unwrap_or(false)
}
