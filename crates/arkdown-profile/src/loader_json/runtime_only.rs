use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use arkdown_core::{ArkdownConfig, ModuleConfig};

use super::{to_absolute_path_string, to_unix_path};

pub(super) fn collect_dynamic_import_dep_names(
    config: &ArkdownConfig,
    module_root: &Path,
) -> Vec<String> {
    let mut names = read_dynamic_dep_names(module_root);
    names.extend(read_module_runtime_only(module_root).packages);
    if let Some(runtime_only) = config
        .product
        .build_option
        .ark_options
        .as_ref()
        .and_then(|ark_options| ark_options.runtime_only.as_ref())
    {
        names.extend(runtime_only.packages.iter().cloned());
    }
    names.extend(read_dependency_runtime_only_package_names(
        module_root,
        &config.project_root,
    ));
    let mut seen = std::collections::HashSet::new();
    names.retain(|name| seen.insert(name.clone()));
    names
}

/// Read `dynamicDependencies` package names from `oh-package.json5` in `module_root`.
///
/// Returns an empty vec if the file is absent or unparseable.
fn read_dynamic_dep_names(module_root: &Path) -> Vec<String> {
    let path = module_root.join("oh-package.json5");
    let Ok(content) = std::fs::read_to_string(&path) else {
        return vec![];
    };

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct PkgDynDeps {
        #[serde(default)]
        dynamic_dependencies: HashMap<String, String>,
    }

    json5::from_str::<PkgDynDeps>(&content)
        .map(|p| p.dynamic_dependencies.into_keys().collect())
        .unwrap_or_default()
}

#[derive(Default)]
struct RuntimeOnlyLists {
    packages: Vec<String>,
    sources: Vec<String>,
}

fn read_module_runtime_only(module_root: &Path) -> RuntimeOnlyLists {
    let path = module_root.join("build-profile.json5");
    let Ok(content) = std::fs::read_to_string(&path) else {
        return RuntimeOnlyLists::default();
    };

    #[derive(Default, serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ModuleBuildProfile {
        #[serde(default)]
        build_option: ModuleBuildOption,
    }

    #[derive(Default, serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ModuleBuildOption {
        #[serde(default)]
        ark_options: ModuleArkOptions,
    }

    #[derive(Default, serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ModuleArkOptions {
        #[serde(default)]
        runtime_only: ModuleRuntimeOnly,
    }

    #[derive(Default, serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ModuleRuntimeOnly {
        #[serde(default)]
        packages: Vec<String>,
        #[serde(default)]
        sources: Vec<String>,
    }

    json5::from_str::<ModuleBuildProfile>(&content)
        .map(|profile| RuntimeOnlyLists {
            packages: profile.build_option.ark_options.runtime_only.packages,
            sources: profile.build_option.ark_options.runtime_only.sources,
        })
        .unwrap_or_default()
}

fn read_dependency_runtime_only_package_names(
    module_root: &Path,
    project_root: &Path,
) -> Vec<String> {
    let dep_names = read_package_dep_names(module_root);
    let mut names = Vec::new();
    for dep_name in dep_names {
        let Some(pkg_dir) = resolve_oh_module_dir(&dep_name, module_root, project_root) else {
            continue;
        };
        names.extend(read_pkg_runtime_only_packages(&pkg_dir));
    }
    names
}

fn read_package_dep_names(module_root: &Path) -> Vec<String> {
    let path = module_root.join("oh-package.json5");
    let Ok(content) = std::fs::read_to_string(&path) else {
        return vec![];
    };

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct PkgDeps {
        #[serde(default)]
        dependencies: HashMap<String, String>,
    }

    json5::from_str::<PkgDeps>(&content)
        .map(|p| p.dependencies.into_keys().collect())
        .unwrap_or_default()
}

fn read_pkg_runtime_only_packages(pkg_dir: &Path) -> Vec<String> {
    let path = pkg_dir.join("oh-package.json5");
    let Ok(content) = std::fs::read_to_string(&path) else {
        return vec![];
    };

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct PkgRuntimeOnly {
        #[serde(default)]
        metadata: PkgMetadata,
    }

    #[derive(Default, serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct PkgMetadata {
        #[serde(default)]
        runtime_only: PkgRuntimeOnlyConfig,
    }

    #[derive(Default, serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct PkgRuntimeOnlyConfig {
        #[serde(default)]
        packages: Vec<String>,
    }

    json5::from_str::<PkgRuntimeOnly>(&content)
        .map(|p| p.metadata.runtime_only.packages)
        .unwrap_or_default()
}

fn resolve_oh_module_dir(
    dep_name: &str,
    module_root: &Path,
    project_root: &Path,
) -> Option<PathBuf> {
    for base in &[
        module_root.join("oh_modules"),
        project_root.join("oh_modules"),
    ] {
        let candidate = base.join(dep_name);
        if candidate.exists() && candidate.is_dir() {
            return Some(candidate);
        }
    }
    None
}

pub(super) fn discover_worker_files(module_root: &Path) -> Vec<PathBuf> {
    let workers_dir = module_root.join("src/main/ets/workers");
    if !workers_dir.is_dir() {
        return vec![];
    }

    collect_dynamic_import_source_files(&workers_dir)
}

pub(super) fn collect_runtime_only_source_files(
    config: &ArkdownConfig,
    module_cfg: &ModuleConfig,
) -> Vec<PathBuf> {
    let module_runtime_only = read_module_runtime_only(&module_cfg.src_path);
    let mut sources: Vec<String> = module_runtime_only.sources;
    sources.extend(
        config
            .product
            .build_option
            .ark_options
            .as_ref()
            .and_then(|ark_options| ark_options.runtime_only.as_ref())
            .map(|runtime_only| runtime_only.sources.clone())
            .unwrap_or_default(),
    );

    let mut files = Vec::new();
    for source in &sources {
        let path = Path::new(source);
        let source_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            module_cfg.src_path.join(path)
        };

        if source_path.is_file() {
            if is_dynamic_import_source_file(&source_path) {
                files.push(source_path);
            }
            continue;
        }

        if source_path.is_dir() {
            files.extend(collect_dynamic_import_source_files(&source_path));
        }
    }

    let mut seen = std::collections::HashSet::new();
    files.retain(|path| seen.insert(to_unix_path(path)));
    files
}

pub(super) fn collect_runtime_only_package_source_files(
    config: &ArkdownConfig,
    module_cfg: &ModuleConfig,
) -> Vec<PathBuf> {
    let module_runtime_only = read_module_runtime_only(&module_cfg.src_path);
    let mut packages: Vec<String> = module_runtime_only.packages;
    packages.extend(
        config
            .product
            .build_option
            .ark_options
            .as_ref()
            .and_then(|ark_options| ark_options.runtime_only.as_ref())
            .map(|runtime_only| runtime_only.packages.clone())
            .unwrap_or_default(),
    );

    let mut files = Vec::new();
    for package in &packages {
        let Some(pkg_dir) =
            resolve_oh_module_dir(package, &module_cfg.src_path, &config.project_root)
        else {
            continue;
        };
        files.extend(collect_package_source_files(&pkg_dir));
    }
    dedupe_paths(&mut files);
    files
}

fn collect_package_source_files(pkg_dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for stem in ["Index", "index"] {
        for ext in ["ets", "ts", "js"] {
            let candidate = pkg_dir.join(format!("{stem}.{ext}"));
            if candidate.is_file() {
                files.push(candidate);
            }
        }
    }

    let ets_root = pkg_dir.join("src/main/ets");
    if ets_root.is_dir() {
        files.extend(collect_dynamic_import_source_files(&ets_root));
    }
    files.sort_by_key(|path| {
        path.strip_prefix(pkg_dir)
            .map(to_unix_path)
            .unwrap_or_else(|_| to_unix_path(path))
    });
    files
}

fn collect_dynamic_import_source_files(root: &Path) -> Vec<PathBuf> {
    let mut stack = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if is_dynamic_import_source_file(&path) {
                files.push(path);
            }
        }
    }

    files.sort_by_key(|path| to_unix_path(path));
    files
}

pub(super) fn dedupe_paths(files: &mut Vec<PathBuf>) {
    let mut seen = std::collections::HashSet::new();
    files.retain(|path| seen.insert(to_absolute_path_string(path)));
}

fn is_dynamic_import_source_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("ets" | "ts" | "js")
    )
}
