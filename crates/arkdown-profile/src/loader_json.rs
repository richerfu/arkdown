use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use arkdown_core::{ArkdownConfig, BuildMode, HapManifest, ModuleConfig, OhosModuleType};

use crate::bytecode_har::{collect_dynamic_import_lib_info, scan_bytecode_har_deps};

// ---------------------------------------------------------------------------
// loader.json schema types
// ---------------------------------------------------------------------------

/// Top-level `loader.json` structure.
///
/// The serialized fields match the hvigor `GenerateLoaderJson` artifact. The
/// skipped fields are retained for arkdown's in-process bundler, which still
/// needs an explicit entry list and output paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoaderJson {
    /// Map of module name → absolute module root path.
    pub module_path_map: HashMap<String, String>,
    /// hvigor compile mode string, e.g. `"esmodule"`.
    pub compile_mode: String,
    /// Absolute path to the project root directory.
    pub project_root_path: String,
    /// hvigor node_modules sandbox path under `intermediates/loader_out`.
    pub node_modules_path: String,
    /// ByteCode HAR dependency metadata.
    pub byte_code_har_info: HashMap<String, ByteCodeHarEntry>,
    /// ByteCode HAR declaration entries. Empty for the minimal HAP fixture.
    pub declaration_entry: Vec<String>,
    /// The OHOS module name (e.g. `"entry"`).
    pub module_name: String,
    /// HSP package name → OHM URL map.
    pub hsp_name_ohm_map: HashMap<String, String>,
    /// HAR package name → OHM URL map.
    pub har_name_ohm_map: HashMap<String, String>,
    /// Package manager type — `"ohpm"` for OHOS projects.
    pub package_manager_type: String,
    /// hvigor's serialized compile entries. Empty for the minimal HAP fixture.
    #[serde(rename = "compileEntry")]
    pub hvigor_compile_entry: Vec<String>,
    /// Additional compile files emitted by hvigor. Empty for the minimal fixture.
    pub other_compile_files: Vec<String>,
    /// Dynamic import metadata. Empty until dependency collection is implemented.
    pub dynamic_import_lib_info: HashMap<String, serde_json::Value>,
    /// Router map entries. Empty for the minimal fixture.
    pub router_map: Vec<serde_json::Value>,
    /// HSP resource table map. Empty for the minimal fixture.
    pub hsp_resources_map: HashMap<String, String>,
    /// Incremental update metadata. Empty for the minimal fixture.
    pub update_version_info: HashMap<String, serde_json::Value>,
    /// Ark Native output path emitted by hvigor.
    pub an_build_out_put: String,
    /// Ark Native build mode emitted by hvigor.
    pub an_build_mode: String,
    /// Non-entry importees in OHM URL format, populated after bundling by
    /// `process_importees`.  Empty until the post-bundle pass runs.
    ///
    /// Not serialized to `loader.json`: hvigor does not write these fields and
    /// the importee data is persisted separately in `importee/default/.../importee.json`.
    #[serde(skip)]
    pub non_entry_importees: Vec<String>,
    /// Compiled importee source paths (relative to `src/main`), populated
    /// after bundling by `process_importees`.  Empty until that pass runs.
    ///
    /// Not serialized to `loader.json`: hvigor does not write these fields and
    /// the importee data is persisted separately in `importee/default/.../importee.json`.
    #[serde(skip)]
    pub compiled_importees: Vec<String>,

    /// Ordered list of entry-point source paths used by arkdown's bundler.
    #[serde(skip)]
    pub compile_entry: Vec<String>,
    /// Absolute path to the module source root directory (e.g. `<project>/entry/src/main`).
    #[serde(skip)]
    pub module_root_path: String,
    /// Absolute path to the incremental compilation cache directory.
    #[serde(skip)]
    pub cache_path: String,
    /// Absolute path where the ArkTS loader writes its output `.js` files.
    #[serde(skip)]
    pub ace_module_build: String,
    /// `"debug"` or `"release"`.
    #[serde(skip, default = "default_build_mode")]
    pub build_mode: String,
    /// Whether the bundler may split output into multiple chunks.
    #[serde(skip)]
    pub support_chunks: bool,
    /// Whether OHM URL module identifiers are used in the bundle graph.
    #[serde(skip)]
    pub use_normalized_ohm_url: bool,
    /// Numeric SDK API version (e.g. `12`).
    #[serde(skip)]
    pub api_version: u32,
    /// Whether to fall back to AN (Ark Native) build.
    #[serde(skip)]
    pub fallback_an_build: bool,
    /// Map of package name → brief package info (source roots, etc.).
    #[serde(skip)]
    pub pkg_name_to_pkg_brief_info: HashMap<String, PkgBriefInfo>,
    /// WebWorker source file paths. hvigor reads this field when present and
    /// expects filesystem paths to `.ets`, `.ts`, or `.js` worker sources.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub workers: Vec<String>,
}

/// Concise metadata about an OHOS package referenced in the bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PkgBriefInfo {
    /// Absolute path to the package root directory.
    pub pkg_root: String,
    /// Package name from `oh-package.json5` / `build-profile.json5`.
    pub pkg_name: String,
    /// Transformed source root directories (may differ from originals after path mapping).
    pub source_roots: Vec<String>,
    /// Original source root directories as declared in the project.
    pub original_source_roots: Vec<String>,
}

/// Metadata for a bytecode-compiled HAR dependency.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ByteCodeHarEntry {
    /// Package name of the HAR.
    pub package_name: String,
    /// Absolute path to the compiled `.abc` file.
    pub abc_path: String,
    /// Compatible SDK version string (e.g. `"5.0.0(12)"`).
    pub compatible_sdk_version: String,
    /// Absolute path to the `sourceMaps.map` file, if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_maps_path: Option<String>,
    /// Absolute path to the insight intent metadata, if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insight_intent: Option<String>,
}

fn default_build_mode() -> String {
    "debug".to_string()
}

// ---------------------------------------------------------------------------
// Generation
// ---------------------------------------------------------------------------

/// Generate a [`LoaderJson`] from the project config, module config, and
/// processed manifest.
///
/// Entry points are collected from:
/// 1. Each ability's `src_entry` path in the manifest
/// 2. The module-level `src_entry` from the module.json5 (if recorded)
///
/// When `useNormalizedOHMUrl` is `true`, callers are expected to convert the
/// `src_entry` paths to OHM URLs (`@bundle:…`) before passing the manifest —
/// this function records whatever values are in `manifest.abilities[].src_entry`.
pub fn generate_loader_json(
    config: &ArkdownConfig,
    module_cfg: &ModuleConfig,
    manifest: &HapManifest,
) -> LoaderJson {
    // Collect compile_entry from abilities' srcEntry values.
    let mut compile_entry: Vec<String> = manifest
        .abilities
        .iter()
        .map(|a| a.src_entry.clone())
        .collect();

    // Also collect from extension abilities.
    for ext in &manifest.extension_abilities {
        compile_entry.push(ext.src_entry.clone());
    }

    // Module-level srcEntry (used by HSP shared packages that export a single file
    // without declaring abilities).
    if let Some(src_entry) = &manifest.src_entry {
        compile_entry.push(src_entry.clone());
    }
    if compile_entry.is_empty()
        && matches!(
            manifest.module_type,
            OhosModuleType::Har | OhosModuleType::Shared
        )
    {
        if let Some(main) = read_package_main(&module_cfg.src_path) {
            compile_entry.push(to_absolute_path_string(&module_cfg.src_path.join(main)));
        }
    }

    // Include pages from $profile:main_pages so that page components are
    // compiled alongside ability entry points.  The EtsTransformPlugin stubs
    // out ArkUI build() DSL bodies before the bundler parses the TypeScript,
    // so page files can now be processed safely.
    if let Some(pages_ref) = &manifest.pages {
        // pages_ref is typically "$profile:main_pages"; strip the "$profile:" prefix
        // to get the profile file name.
        if let Some(profile_name) = pages_ref.strip_prefix("$profile:") {
            let profile_path = module_cfg
                .src_path
                .join("src/main/resources/base/profile")
                .join(format!("{}.json", profile_name));

            if let Ok(content) = std::fs::read_to_string(&profile_path) {
                #[derive(serde::Deserialize)]
                struct MainPages {
                    src: Vec<String>,
                }
                if let Ok(pages) = serde_json::from_str::<MainPages>(&content) {
                    for page in &pages.src {
                        // page is e.g. "pages/Index"; map to "./ets/pages/Index.ets"
                        let entry = format!("./ets/{}.ets", page);
                        compile_entry.push(entry);
                    }
                }
            }
        }
    }

    // Discover WebWorker files from src/main/ets/workers. Upstream
    // `main.js::readWorkerFile()` accepts explicit loader.json `workers` entries
    // with .ets/.ts/.js suffixes; `loadWorker()` otherwise scans this directory
    // recursively. Keep both surfaces aligned: serialize absolute worker paths
    // for loader consumers and add relative `./ets/...` entries for arkdown's
    // in-process bundler.
    let worker_files = discover_worker_files(&module_cfg.src_path);
    let mut discovered_workers: Vec<String> = vec![];
    let ets_root = module_cfg.src_path.join("src/main/ets");
    for path in &worker_files {
        if let Ok(rel) = path.strip_prefix(&ets_root) {
            compile_entry.push(format!("./ets/{}", to_unix_path(rel)));
        }
        discovered_workers.push(to_absolute_path_string(path));
    }

    let runtime_only_source_files = collect_runtime_only_source_files(config, module_cfg);
    let hvigor_compile_entry: Vec<String> = runtime_only_source_files
        .iter()
        .map(|path| to_absolute_path_string(path))
        .collect();
    let src_main_root = module_cfg.src_path.join("src/main");
    for path in &runtime_only_source_files {
        if let Ok(rel) = path.strip_prefix(&src_main_root) {
            compile_entry.push(format!("./{}", to_unix_path(rel)));
        }
    }

    // Deduplicate while preserving order.
    {
        let mut seen = std::collections::HashSet::new();
        compile_entry.retain(|e| seen.insert(e.clone()));
    }

    let module_root = to_absolute_path_string(&module_cfg.src_path);
    let project_root = to_absolute_path_string(&config.project_root);
    let src_main = format!("{}/src/main", module_root);

    let build_mode_str = match config.build_mode {
        BuildMode::Debug => "debug".to_string(),
        BuildMode::Release => "release".to_string(),
    };

    let compile_mode = match manifest.compile_mode {
        arkdown_core::CompileMode::EsModule => "esmodule",
        arkdown_core::CompileMode::JsBundle => "jsbundle",
    }
    .to_string();

    let use_normalized_ohm_url = config
        .product
        .build_option
        .strict_mode
        .use_normalized_ohm_url;
    let router_map = read_loader_router_map(module_cfg, manifest.router_map.as_deref());

    let mut module_path_map = HashMap::new();
    module_path_map.insert(module_cfg.name.clone(), module_root.clone());

    // --- TASK-LOADER-01: scan oh_modules/ for ByteCode HAR dependencies ---
    let bytecode_har_deps = scan_bytecode_har_deps(&module_cfg.src_path, &config.project_root);

    let byte_code_har_info: HashMap<String, ByteCodeHarEntry> = bytecode_har_deps
        .iter()
        .cloned()
        .map(|e| e.into_loader_entry())
        .collect();

    let declaration_entry = if use_normalized_ohm_url {
        let mut entries: Vec<String> = bytecode_har_deps
            .iter()
            .flat_map(|dep| dep.declaration_entry.iter().cloned())
            .collect();
        let mut seen = std::collections::HashSet::new();
        entries.retain(|entry| seen.insert(entry.clone()));
        entries
    } else {
        Vec::new()
    };

    // --- TASK-LOADER-02: populate dynamicImportLibInfo from oh-package.json5 ---
    let dynamic_import_lib_info: HashMap<String, serde_json::Value> = {
        let dynamic_dep_names = collect_dynamic_import_dep_names(config, &module_cfg.src_path);
        let entries = collect_dynamic_import_lib_info(
            &module_cfg.name,
            &dynamic_dep_names,
            &module_cfg.src_path,
            &config.project_root,
        );
        entries
            .into_iter()
            .filter_map(|(k, v)| serde_json::to_value(v).ok().map(|json| (k, json)))
            .collect()
    };

    LoaderJson {
        module_path_map,
        compile_mode,
        project_root_path: project_root.clone(),
        node_modules_path: format!(
            "{}/build/default/intermediates/loader_out/default/node_modules",
            module_root
        ),
        byte_code_har_info,
        declaration_entry,
        module_name: module_cfg.name.clone(),
        hsp_name_ohm_map: HashMap::new(),
        har_name_ohm_map: HashMap::new(),
        package_manager_type: "ohpm".to_string(),
        hvigor_compile_entry,
        other_compile_files: vec![],
        dynamic_import_lib_info,
        router_map,
        hsp_resources_map: HashMap::new(),
        update_version_info: HashMap::new(),
        an_build_out_put: format!(
            "{}/build/default/intermediates/loader_out/default/an/arm64-v8a",
            module_root
        ),
        an_build_mode: "type".to_string(),
        non_entry_importees: vec![],
        compiled_importees: vec![],

        compile_entry,
        module_root_path: src_main.clone(),
        cache_path: format!(
            "{}/build/default/intermediates/loader_out/default/ets/cache/",
            module_root
        ),
        ace_module_build: format!(
            "{}/build/default/intermediates/loader_out/default/ets/",
            module_root
        ),
        build_mode: build_mode_str,
        support_chunks: false,
        use_normalized_ohm_url,
        api_version: config.sdk_config.compile_api_version,
        fallback_an_build: false,
        pkg_name_to_pkg_brief_info: build_pkg_brief_info(config, module_cfg),
        workers: discovered_workers,
    }
}

/// Read dynamic import package names from hvigor-aligned sources.
///
/// hvigor's `GenerateLoaderJson.processDynamicImport()` consumes
/// `buildOption.arkOptions.runtimeOnly.packages`; `dynamicDependencies` remains
/// a package metadata source for direct dynamic dependencies.
fn collect_dynamic_import_dep_names(config: &ArkdownConfig, module_root: &Path) -> Vec<String> {
    let mut names = read_dynamic_dep_names(module_root);
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

fn read_package_main(module_root: &Path) -> Option<String> {
    let path = module_root.join("oh-package.json5");
    let content = std::fs::read_to_string(path).ok()?;

    #[derive(serde::Deserialize)]
    struct PackageMain {
        main: Option<String>,
    }

    json5::from_str::<PackageMain>(&content).ok()?.main
}

fn read_loader_router_map(
    module_cfg: &ModuleConfig,
    router_map_ref: Option<&str>,
) -> Vec<serde_json::Value> {
    let Some(profile_name) = router_map_ref.and_then(|value| value.strip_prefix("$profile:"))
    else {
        return vec![];
    };

    let path = module_cfg
        .src_path
        .join("src/main/resources/base/profile")
        .join(format!("{profile_name}.json"));
    let Ok(content) = std::fs::read_to_string(path) else {
        return vec![];
    };
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&content) else {
        return vec![];
    };

    let Some(entries) = value
        .get_mut("routerMap")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return value.as_array().cloned().unwrap_or_default();
    };

    let src_main = module_cfg.src_path.join("src/main");
    for entry in entries.iter_mut() {
        let Some(obj) = entry.as_object_mut() else {
            continue;
        };
        let Some(page_source_file) = obj
            .get("pageSourceFile")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
        else {
            continue;
        };
        if Path::new(&page_source_file).is_absolute() {
            continue;
        }
        let absolute = to_absolute_path_string(
            &src_main.join(page_source_file.trim_start_matches("src/main/")),
        );
        obj.insert(
            "pageSourceFile".to_string(),
            serde_json::Value::String(absolute),
        );
    }

    entries.clone()
}

/// Build the `pkgNameToPkgBriefInfo` map for the loader.json.
///
/// Produces one entry for the current module. HAR/HSP dependencies would be
/// added by a later pass (see `arkdown-plugin-har-dedup`).
fn build_pkg_brief_info(
    _config: &ArkdownConfig,
    module_cfg: &ModuleConfig,
) -> HashMap<String, PkgBriefInfo> {
    let mut map = HashMap::new();
    let module_root = to_absolute_path_string(&module_cfg.src_path);
    let ets_src = format!("{}/src/main/ets", module_root);

    map.insert(
        module_cfg.name.clone(),
        PkgBriefInfo {
            pkg_root: module_root.clone(),
            pkg_name: module_cfg.name.clone(),
            source_roots: vec![ets_src.clone()],
            original_source_roots: vec![ets_src],
        },
    );
    map
}

fn discover_worker_files(module_root: &Path) -> Vec<PathBuf> {
    let workers_dir = module_root.join("src/main/ets/workers");
    if !workers_dir.is_dir() {
        return vec![];
    }

    collect_dynamic_import_source_files(&workers_dir)
}

fn collect_runtime_only_source_files(
    config: &ArkdownConfig,
    module_cfg: &ModuleConfig,
) -> Vec<PathBuf> {
    let sources = config
        .product
        .build_option
        .ark_options
        .as_ref()
        .and_then(|ark_options| ark_options.runtime_only.as_ref())
        .map(|runtime_only| runtime_only.sources.as_slice())
        .unwrap_or_default();

    let mut files = Vec::new();
    for source in sources {
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

fn is_dynamic_import_source_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("ets" | "ts" | "js")
    )
}

fn to_unix_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn normalize_path_string(path: &Path) -> String {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push(component.as_os_str());
                }
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    if normalized.as_os_str().is_empty() {
        ".".to_string()
    } else {
        normalized.to_string_lossy().to_string()
    }
}

/// Return an absolute, normalized path string for `path`.
///
/// Strategy (matches hvigor's behavior of always writing absolute paths):
/// 1. Try `std::fs::canonicalize` — works perfectly when the path already exists
///    on disk (project root, module source root).
/// 2. If canonicalize fails (path not yet on disk, or a hypothetical path used in
///    tests), fall back to:
///    - If `path` is already absolute: normalize `.`/`..` segments in place.
///    - If `path` is relative: join it onto the current working directory, then
///      normalize.
fn to_absolute_path_string(path: &Path) -> String {
    // Fast path: canonicalize works for real, existing paths.
    if let Ok(abs) = std::fs::canonicalize(path) {
        return abs.to_string_lossy().into_owned();
    }

    // Fallback for non-existent or constructed paths.
    let base = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };
    normalize_path_string(&base)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use arkdown_core::{
        AbilityEntry, ArkOptions, ArkdownConfig, BuildMode, BuildOption, HapManifest, ModuleConfig,
        OhosModuleType, OutputTarget, ProductConfig, RuntimeOnlyConfig, SdkConfig,
        StrictModeConfig,
    };

    use super::*;

    fn make_config(use_normalized_ohm_url: bool) -> ArkdownConfig {
        ArkdownConfig {
            project_root: PathBuf::from("/project"),
            build_mode: BuildMode::Debug,
            output_target: OutputTarget::Hap,
            product: ProductConfig {
                name: "default".to_string(),
                bundle_name: "com.example.app".to_string(),
                compatible_sdk_version: "5.0.0(12)".to_string(),
                runtime_os: "HarmonyOS".to_string(),
                signing_config: None,
                build_option: BuildOption {
                    strict_mode: StrictModeConfig {
                        case_sensitive_check: false,
                        use_normalized_ohm_url,
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

    fn make_module_cfg() -> ModuleConfig {
        ModuleConfig {
            name: "entry".to_string(),
            src_path: PathBuf::from("/project/entry"),
            module_type: OhosModuleType::Entry,
        }
    }

    fn make_manifest() -> HapManifest {
        HapManifest {
            module_name: "entry".to_string(),
            module_type: OhosModuleType::Entry,
            device_types: vec!["phone".to_string()],
            virtual_machine: "ark1.2.0.0".to_string(),
            compile_mode: arkdown_core::CompileMode::EsModule,
            abilities: vec![AbilityEntry {
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
            dependencies: vec![],
            router_map: None,
            pages: Some("$profile:main_pages".to_string()),
            main_element: Some("EntryAbility".to_string()),
            description: None,
            src_entry: None,
        }
    }

    #[test]
    fn test_loader_json_collects_bytecode_har_declaration_entry() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let project_root = tmp.path().to_path_buf();
        let module_root = project_root.join("entry");
        let pkg_dir = module_root.join("oh_modules").join("bytecode-har");
        fs::create_dir_all(pkg_dir.join("ets")).expect("create bytecode har");
        fs::write(
            pkg_dir.join("ets").join("modules.abc"),
            b"PANDA\x00\x00\x00",
        )
        .expect("write abc");
        fs::write(
            pkg_dir.join("oh-package.json5"),
            r#"{
              name: "bytecode-har",
              compatibleSdkVersion: 12,
              metadata: {
                byteCodeHar: true,
                declarationEntry: [
                  "@normalized:N&entry&com.example&bytecode-har/src/main/ets/Worker&1.0.0",
                  "@normalized:N&entry&com.example&bytecode-har/src/main/ets/Worker&1.0.0",
                  "@normalized:N&entry&com.example&bytecode-har/src/main/ets/runtime&1.0.0"
                ]
              }
            }"#,
        )
        .expect("write oh-package");

        let mut config = make_config(true);
        config.project_root = project_root;
        let mut module_cfg = make_module_cfg();
        module_cfg.src_path = module_root;
        let loader = generate_loader_json(&config, &module_cfg, &make_manifest());

        assert_eq!(
            loader.declaration_entry,
            vec![
                "@normalized:N&entry&com.example&bytecode-har/src/main/ets/Worker&1.0.0",
                "@normalized:N&entry&com.example&bytecode-har/src/main/ets/runtime&1.0.0",
            ]
        );
        let entry = loader
            .byte_code_har_info
            .get("bytecode-har")
            .expect("bytecode har info");
        assert!(entry.abc_path.ends_with("ets/modules.abc"));
    }

    /// Verify loader.json round-trips through serde correctly.
    #[test]
    fn test_loader_json_serialization() {
        let config = make_config(false);
        let module_cfg = make_module_cfg();
        let manifest = make_manifest();

        let loader = generate_loader_json(&config, &module_cfg, &manifest);

        // Serialize to JSON string
        let json_str = serde_json::to_string_pretty(&loader).expect("serialization failed");

        // Deserialize back
        let loader2: LoaderJson = serde_json::from_str(&json_str).expect("deserialization failed");

        // Verify serialized hvigor fields survived the round-trip. Internal
        // bundler-only fields are intentionally not part of loader.json.
        assert_eq!(loader2.compile_mode, "esmodule");
        assert_eq!(loader2.package_manager_type, "ohpm");
        assert_eq!(loader2.project_root_path, "/project");
        assert_eq!(
            loader2.node_modules_path,
            "/project/entry/build/default/intermediates/loader_out/default/node_modules"
        );
        assert!(loader2.hvigor_compile_entry.is_empty());
        assert!(loader2.other_compile_files.is_empty());
        assert_eq!(loader2.module_name, "entry");
        assert!(loader2.module_path_map.contains_key("entry"));
        assert_eq!(loader2.module_path_map["entry"], "/project/entry");
        assert_eq!(loader2.an_build_mode, "type");
    }

    #[test]
    fn test_loader_json_uses_normalized_ohm_url() {
        let config = make_config(true);
        let module_cfg = make_module_cfg();
        let manifest = make_manifest();

        let loader = generate_loader_json(&config, &module_cfg, &manifest);
        assert!(loader.use_normalized_ohm_url);
    }

    #[test]
    fn test_loader_json_hvigor_keys() {
        let config = make_config(false);
        let module_cfg = make_module_cfg();
        let manifest = make_manifest();

        let loader = generate_loader_json(&config, &module_cfg, &manifest);
        let value = serde_json::to_value(&loader).expect("serialization failed");
        // serde_json::to_value produces a BTreeMap (alphabetical) without the
        // preserve_order feature, so compare key *sets* rather than ordered vecs.
        let keys: std::collections::BTreeSet<String> = value
            .as_object()
            .expect("loader.json must serialize as object")
            .keys()
            .cloned()
            .collect();

        let expected_keys: std::collections::BTreeSet<String> = vec![
            "modulePathMap",
            "compileMode",
            "projectRootPath",
            "nodeModulesPath",
            "byteCodeHarInfo",
            "declarationEntry",
            "moduleName",
            "hspNameOhmMap",
            "harNameOhmMap",
            "packageManagerType",
            "compileEntry",
            "otherCompileFiles",
            "dynamicImportLibInfo",
            "routerMap",
            "hspResourcesMap",
            "updateVersionInfo",
            "anBuildOutPut",
            "anBuildMode",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        assert_eq!(keys, expected_keys);
        assert!(value.get("apiVersion").is_none());
        assert!(value.get("buildMode").is_none());
        assert!(value.get("supportChunks").is_none());
        assert!(value.get("useNormalizedOHMUrl").is_none());
        assert!(value.get("pkgNameToPkgBriefInfo").is_none());
        assert!(value.get("workers").is_none());
    }

    #[test]
    fn test_loader_json_serializes_minimal_hvigor_shape() {
        let config = make_config(false);
        let module_cfg = make_module_cfg();
        let manifest = make_manifest();

        let loader = generate_loader_json(&config, &module_cfg, &manifest);
        let json = serde_json::to_string_pretty(&loader).expect("serialization failed");

        assert_eq!(
            json,
            r#"{
  "modulePathMap": {
    "entry": "/project/entry"
  },
  "compileMode": "esmodule",
  "projectRootPath": "/project",
  "nodeModulesPath": "/project/entry/build/default/intermediates/loader_out/default/node_modules",
  "byteCodeHarInfo": {},
  "declarationEntry": [],
  "moduleName": "entry",
  "hspNameOhmMap": {},
  "harNameOhmMap": {},
  "packageManagerType": "ohpm",
  "compileEntry": [],
  "otherCompileFiles": [],
  "dynamicImportLibInfo": {},
  "routerMap": [],
  "hspResourcesMap": {},
  "updateVersionInfo": {},
  "anBuildOutPut": "/project/entry/build/default/intermediates/loader_out/default/an/arm64-v8a",
  "anBuildMode": "type"
}"#
        );
    }

    #[test]
    fn test_loader_json_pkg_brief_info_populated() {
        let config = make_config(false);
        let module_cfg = make_module_cfg();
        let manifest = make_manifest();

        let loader = generate_loader_json(&config, &module_cfg, &manifest);
        assert!(
            loader.pkg_name_to_pkg_brief_info.contains_key("entry"),
            "expected 'entry' key in pkgNameToPkgBriefInfo"
        );
        let info = &loader.pkg_name_to_pkg_brief_info["entry"];
        assert_eq!(info.pkg_name, "entry");
        assert_eq!(info.pkg_root, "/project/entry");
    }

    #[test]
    fn test_loader_json_cache_and_build_paths() {
        let config = make_config(false);
        let module_cfg = make_module_cfg();
        let manifest = make_manifest();

        let loader = generate_loader_json(&config, &module_cfg, &manifest);
        assert!(
            loader.cache_path.starts_with("/project/entry"),
            "cache_path should be under module root"
        );
        assert!(
            loader.ace_module_build.starts_with("/project/entry"),
            "ace_module_build should be under module root"
        );
        assert_eq!(
            loader.ace_module_build,
            "/project/entry/build/default/intermediates/loader_out/default/ets/"
        );
    }

    #[test]
    fn test_loader_json_module_root_path_is_src_main() {
        let config = make_config(false);
        let module_cfg = make_module_cfg();
        let manifest = make_manifest();

        let loader = generate_loader_json(&config, &module_cfg, &manifest);
        assert_eq!(
            loader.module_root_path, "/project/entry/src/main",
            "module_root_path must point to src/main, not the module root"
        );
    }

    #[test]
    fn test_loader_json_module_name_populated() {
        let config = make_config(false);
        let module_cfg = make_module_cfg();
        let manifest = make_manifest();

        let loader = generate_loader_json(&config, &module_cfg, &manifest);
        assert_eq!(loader.module_name, "entry");
    }

    #[test]
    fn test_loader_json_api_version() {
        let config = make_config(false);
        let module_cfg = make_module_cfg();
        let manifest = make_manifest();

        let loader = generate_loader_json(&config, &module_cfg, &manifest);
        assert_eq!(loader.api_version, 12);
    }

    #[test]
    fn test_loader_json_module_path_map() {
        let config = make_config(false);
        let module_cfg = make_module_cfg();
        let manifest = make_manifest();

        let loader = generate_loader_json(&config, &module_cfg, &manifest);
        assert!(
            loader.module_path_map.contains_key("entry"),
            "modulePathMap must contain the module name key"
        );
        assert_eq!(
            loader.module_path_map["entry"], "/project/entry",
            "modulePathMap value must be the module root (not src/main)"
        );
    }

    #[test]
    fn test_loader_json_normalizes_dot_segments_in_hvigor_paths() {
        let config = ArkdownConfig {
            project_root: PathBuf::from("/project/./app"),
            ..make_config(false)
        };
        let module_cfg = ModuleConfig {
            src_path: PathBuf::from("/project/./app/./entry"),
            ..make_module_cfg()
        };
        let manifest = make_manifest();

        let loader = generate_loader_json(&config, &module_cfg, &manifest);
        assert_eq!(loader.project_root_path, "/project/app");
        assert_eq!(loader.module_path_map["entry"], "/project/app/entry");
        assert_eq!(
            loader.node_modules_path,
            "/project/app/entry/build/default/intermediates/loader_out/default/node_modules"
        );
        assert_eq!(
            loader.an_build_out_put,
            "/project/app/entry/build/default/intermediates/loader_out/default/an/arm64-v8a"
        );
    }

    #[test]
    fn test_loader_json_fallback_an_build_is_false() {
        let config = make_config(false);
        let module_cfg = make_module_cfg();
        let manifest = make_manifest();

        let loader = generate_loader_json(&config, &module_cfg, &manifest);
        assert!(!loader.fallback_an_build);
    }

    /// Verify that relative src_path / project_root inputs produce absolute paths in the
    /// serialized loader.json fields.  hvigor always writes absolute paths; arkdown must
    /// match that behaviour regardless of how the CLI was invoked.
    #[test]
    fn test_loader_json_relative_paths_become_absolute() {
        let cwd = std::env::current_dir().expect("cwd");
        let config = ArkdownConfig {
            project_root: PathBuf::from("relative_proj_root_test"),
            ..make_config(false)
        };
        let module_cfg = ModuleConfig {
            src_path: PathBuf::from("relative_proj_root_test/entry"),
            ..make_module_cfg()
        };
        let manifest = make_manifest();

        let loader = generate_loader_json(&config, &module_cfg, &manifest);

        let expected_project = cwd.join("relative_proj_root_test");
        let expected_module = cwd.join("relative_proj_root_test/entry");

        assert!(
            std::path::Path::new(&loader.project_root_path).is_absolute(),
            "projectRootPath must be absolute: {}",
            loader.project_root_path
        );
        assert_eq!(
            loader.project_root_path,
            expected_project.to_string_lossy().as_ref()
        );

        assert!(
            std::path::Path::new(&loader.node_modules_path).is_absolute(),
            "nodeModulesPath must be absolute: {}",
            loader.node_modules_path
        );
        assert!(
            loader
                .node_modules_path
                .starts_with(expected_module.to_string_lossy().as_ref()),
            "nodeModulesPath must be under module root"
        );

        assert!(
            std::path::Path::new(&loader.an_build_out_put).is_absolute(),
            "anBuildOutPut must be absolute: {}",
            loader.an_build_out_put
        );

        for (_, path) in &loader.module_path_map {
            assert!(
                std::path::Path::new(path).is_absolute(),
                "modulePathMap value must be absolute: {path}"
            );
        }
    }

    /// Verify the serialized key set exactly matches the golden hvigor output (18 fields).
    #[test]
    fn test_loader_json_golden_hvigor_key_set() {
        let expected_keys: std::collections::BTreeSet<&str> = [
            "modulePathMap",
            "compileMode",
            "projectRootPath",
            "nodeModulesPath",
            "byteCodeHarInfo",
            "declarationEntry",
            "moduleName",
            "hspNameOhmMap",
            "harNameOhmMap",
            "packageManagerType",
            "compileEntry",
            "otherCompileFiles",
            "dynamicImportLibInfo",
            "routerMap",
            "hspResourcesMap",
            "updateVersionInfo",
            "anBuildOutPut",
            "anBuildMode",
        ]
        .iter()
        .copied()
        .collect();

        let config = make_config(false);
        let module_cfg = make_module_cfg();
        let manifest = make_manifest();

        let loader = generate_loader_json(&config, &module_cfg, &manifest);
        let value = serde_json::to_value(&loader).expect("serialization failed");
        let actual_keys: std::collections::BTreeSet<&str> = value
            .as_object()
            .unwrap()
            .keys()
            .map(|k| k.as_str())
            .collect();

        assert_eq!(
            actual_keys, expected_keys,
            "serialized key set must match golden hvigor output"
        );
    }

    /// Verify all path-valued fields in the serialized loader.json are absolute.
    #[test]
    fn test_loader_json_serialized_paths_are_absolute() {
        let config = make_config(false); // project_root = /project
        let module_cfg = make_module_cfg(); // src_path = /project/entry
        let manifest = make_manifest();

        let loader = generate_loader_json(&config, &module_cfg, &manifest);
        let value = serde_json::to_value(&loader).expect("serialization failed");

        let project_root = value["projectRootPath"].as_str().unwrap();
        assert!(
            std::path::Path::new(project_root).is_absolute(),
            "projectRootPath must be absolute: {project_root}"
        );

        for (_, path) in value["modulePathMap"].as_object().unwrap() {
            let p = path.as_str().unwrap();
            assert!(
                std::path::Path::new(p).is_absolute(),
                "modulePathMap value must be absolute: {p}"
            );
        }

        let npm = value["nodeModulesPath"].as_str().unwrap();
        assert!(
            std::path::Path::new(npm).is_absolute(),
            "nodeModulesPath must be absolute: {npm}"
        );

        let an = value["anBuildOutPut"].as_str().unwrap();
        assert!(
            std::path::Path::new(an).is_absolute(),
            "anBuildOutPut must be absolute: {an}"
        );
    }

    #[test]
    fn test_loader_json_serializes_recursive_workers_when_present() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let project_root = tmp.path();
        let module_root = project_root.join("entry");
        let workers_dir = module_root.join("src/main/ets/workers/nested");
        std::fs::create_dir_all(&workers_dir).expect("create workers dir");
        std::fs::write(module_root.join("src/main/ets/workers/MainWorker.ets"), "").unwrap();
        std::fs::write(workers_dir.join("ChildWorker.ts"), "").unwrap();
        std::fs::write(workers_dir.join("ignored.txt"), "").unwrap();

        let mut config = make_config(false);
        config.project_root = project_root.to_path_buf();
        let mut module_cfg = make_module_cfg();
        module_cfg.src_path = module_root.clone();
        let manifest = make_manifest();

        let loader = generate_loader_json(&config, &module_cfg, &manifest);
        assert_eq!(
            loader.workers,
            vec![
                to_absolute_path_string(&module_root.join("src/main/ets/workers/MainWorker.ets")),
                to_absolute_path_string(
                    &module_root.join("src/main/ets/workers/nested/ChildWorker.ts")
                ),
            ]
        );
        assert!(loader
            .compile_entry
            .contains(&"./ets/workers/MainWorker.ets".to_string()));
        assert!(loader
            .compile_entry
            .contains(&"./ets/workers/nested/ChildWorker.ts".to_string()));

        let value = serde_json::to_value(&loader).expect("serialize loader");
        assert_eq!(
            value
                .get("workers")
                .and_then(serde_json::Value::as_array)
                .map(Vec::len),
            Some(2)
        );
    }

    #[test]
    fn test_loader_json_serialized_compile_entry_is_separate_from_internal_entries() {
        let config = make_config(false);
        let module_cfg = make_module_cfg();
        let manifest = make_manifest();

        let loader = generate_loader_json(&config, &module_cfg, &manifest);
        assert_eq!(
            loader.compile_entry,
            vec!["./ets/entryability/EntryAbility.ets".to_string()]
        );
        assert!(loader.hvigor_compile_entry.is_empty());

        let value = serde_json::to_value(&loader).expect("serialization failed");
        assert_eq!(
            value
                .get("compileEntry")
                .and_then(|v| v.as_array())
                .unwrap()
                .len(),
            0
        );
    }

    #[test]
    fn test_har_uses_oh_package_main_as_internal_entry() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let module_root = tmp.path().join("feature");
        std::fs::create_dir_all(&module_root).unwrap();
        std::fs::write(
            module_root.join("oh-package.json5"),
            r#"{ name: "feature", main: "Index.ets" }"#,
        )
        .unwrap();
        std::fs::write(module_root.join("Index.ets"), "export {}").unwrap();

        let config = make_config(false);
        let module_cfg = ModuleConfig {
            name: "feature".to_string(),
            src_path: module_root.clone(),
            module_type: OhosModuleType::Har,
        };
        let mut manifest = make_manifest();
        manifest.module_name = "feature".to_string();
        manifest.module_type = OhosModuleType::Har;
        manifest.abilities.clear();
        manifest.pages = None;

        let loader = generate_loader_json(&config, &module_cfg, &manifest);
        assert_eq!(
            loader.compile_entry,
            vec![to_absolute_path_string(&module_root.join("Index.ets"))]
        );
        assert!(loader.hvigor_compile_entry.is_empty());
    }

    #[test]
    fn test_router_map_metadata_does_not_become_compile_entry() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let module_root = tmp.path().join("entry");
        let profile_dir = module_root.join("src/main/resources/base/profile");
        std::fs::create_dir_all(&profile_dir).unwrap();
        std::fs::write(
            profile_dir.join("router_map.json"),
            r#"{"routerMap":[{"name":"detail","pageSourceFile":"src/main/ets/pages/Detail.ets","buildFunction":"DetailBuilder"}]}"#,
        )
        .unwrap();

        let config = make_config(false);
        let module_cfg = ModuleConfig {
            name: "entry".to_string(),
            src_path: module_root.clone(),
            module_type: OhosModuleType::Entry,
        };
        let mut manifest = make_manifest();
        manifest.pages = None;
        manifest.router_map = Some("$profile:router_map".to_string());

        let loader = generate_loader_json(&config, &module_cfg, &manifest);
        assert_eq!(
            loader.compile_entry,
            vec!["./ets/entryability/EntryAbility.ets".to_string()]
        );
        assert_eq!(loader.router_map.len(), 1);
        assert!(loader.router_map[0]["pageSourceFile"]
            .as_str()
            .is_some_and(|path| path.ends_with("src/main/ets/pages/Detail.ets")));
    }

    /// Verify `dynamicImportLibInfo` is populated when an oh_modules dep exists.
    #[test]
    fn test_loader_json_dynamic_import_lib_info_populated_from_oh_modules() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let project_root = tmp.path().to_path_buf();
        let module_root = project_root.join("entry");

        // Write oh-package.json5 with a dynamicDependency.
        let oh_pkg_content = r#"{
            name: "entry",
            version: "1.0.0",
            dynamicDependencies: {
                "@remote/shared": "1.0.0"
            }
        }"#;
        std::fs::create_dir_all(&module_root).expect("create module dir");
        std::fs::write(module_root.join("oh-package.json5"), oh_pkg_content)
            .expect("write oh-package.json5");

        // Create the package under oh_modules with an entry file.
        let pkg_dir = project_root
            .join("oh_modules")
            .join("@remote")
            .join("shared");
        std::fs::create_dir_all(&pkg_dir).expect("create pkg dir");
        std::fs::write(
            pkg_dir.join("oh-package.json5"),
            r#"{ name: "@remote/shared" }"#,
        )
        .expect("write pkg oh-package.json5");
        let ets_dir = pkg_dir.join("src").join("main").join("ets");
        std::fs::create_dir_all(&ets_dir).expect("create ets dir");
        std::fs::write(ets_dir.join("index.ets"), "export default class Shared {}")
            .expect("write index.ets");

        let mut config = make_config(false);
        config.project_root = project_root.clone();
        let module_cfg = ModuleConfig {
            name: "entry".to_string(),
            src_path: module_root,
            module_type: OhosModuleType::Entry,
        };
        let manifest = make_manifest();

        let loader = generate_loader_json(&config, &module_cfg, &manifest);

        assert!(
            loader
                .dynamic_import_lib_info
                .contains_key("@remote/shared"),
            "dynamicImportLibInfo must contain @remote/shared"
        );
        let entry = &loader.dynamic_import_lib_info["@remote/shared"];
        assert!(
            entry.get("entryFilePath").is_some(),
            "entry must have entryFilePath"
        );
        assert!(
            entry.get("hostModulesInfo").is_some(),
            "entry must have hostModulesInfo"
        );
    }

    #[test]
    fn test_loader_json_dynamic_import_lib_info_uses_runtime_only_packages() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let project_root = tmp.path().to_path_buf();
        let module_root = project_root.join("entry");
        std::fs::create_dir_all(&module_root).expect("create module dir");

        let pkg_dir = project_root
            .join("oh_modules")
            .join("@runtime")
            .join("feature");
        std::fs::create_dir_all(&pkg_dir).expect("create pkg dir");
        std::fs::write(
            pkg_dir.join("oh-package.json5"),
            r#"{ name: "@runtime/feature", main: "src/main/ets/Index.ets" }"#,
        )
        .expect("write pkg oh-package.json5");
        let ets_dir = pkg_dir.join("src").join("main").join("ets");
        std::fs::create_dir_all(&ets_dir).expect("create ets dir");
        std::fs::write(ets_dir.join("Index.ets"), "export default class Feature {}")
            .expect("write index.ets");

        let mut config = make_config(false);
        config.project_root = project_root.clone();
        config.product.build_option.ark_options = Some(ArkOptions {
            obfuscation: None,
            types: vec![],
            runtime_only: Some(RuntimeOnlyConfig {
                packages: vec!["@runtime/feature".to_string()],
                sources: vec![],
            }),
        });
        let module_cfg = ModuleConfig {
            name: "entry".to_string(),
            src_path: module_root,
            module_type: OhosModuleType::Entry,
        };

        let loader = generate_loader_json(&config, &module_cfg, &make_manifest());

        assert!(
            loader
                .dynamic_import_lib_info
                .contains_key("@runtime/feature"),
            "runtimeOnly.packages must feed dynamicImportLibInfo"
        );
        let entry = &loader.dynamic_import_lib_info["@runtime/feature"];
        assert_eq!(
            entry
                .get("hostModulesInfo")
                .and_then(|value| value.as_array())
                .and_then(|values| values.first())
                .and_then(|value| value.get("hostModuleName"))
                .and_then(|value| value.as_str()),
            Some("entry")
        );
    }

    #[test]
    fn test_loader_json_compile_entry_uses_runtime_only_sources() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let project_root = tmp.path().to_path_buf();
        let module_root = project_root.join("entry");
        let runtime_dir = module_root.join("src/main/ets/runtime");
        let nested_dir = runtime_dir.join("nested");
        std::fs::create_dir_all(&nested_dir).expect("create runtime dirs");
        let keep = runtime_dir.join("Keep.ets");
        let side = nested_dir.join("Side.ts");
        std::fs::write(&keep, "export default class Keep {}").expect("write keep");
        std::fs::write(&side, "export const side = true;").expect("write side");
        std::fs::write(runtime_dir.join("ignored.txt"), "ignored").expect("write ignored");

        let mut config = make_config(false);
        config.project_root = project_root;
        config.product.build_option.ark_options = Some(ArkOptions {
            obfuscation: None,
            types: vec![],
            runtime_only: Some(RuntimeOnlyConfig {
                packages: vec![],
                sources: vec!["src/main/ets/runtime".to_string()],
            }),
        });
        let module_cfg = ModuleConfig {
            name: "entry".to_string(),
            src_path: module_root.clone(),
            module_type: OhosModuleType::Entry,
        };

        let loader = generate_loader_json(&config, &module_cfg, &make_manifest());

        assert_eq!(
            loader.hvigor_compile_entry,
            vec![
                to_absolute_path_string(&keep),
                to_absolute_path_string(&side)
            ]
        );
        assert!(loader
            .compile_entry
            .contains(&"./ets/runtime/Keep.ets".to_string()));
        assert!(loader
            .compile_entry
            .contains(&"./ets/runtime/nested/Side.ts".to_string()));
        assert!(!loader
            .compile_entry
            .contains(&"./ets/runtime/ignored.txt".to_string()));

        let value = serde_json::to_value(&loader).expect("serialize loader");
        assert_eq!(
            value
                .get("compileEntry")
                .and_then(|value| value.as_array())
                .map(Vec::len),
            Some(2)
        );
    }

    #[test]
    fn test_loader_json_dynamic_import_lib_info_uses_dependency_runtime_only_packages() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let project_root = tmp.path().to_path_buf();
        let module_root = project_root.join("entry");
        std::fs::create_dir_all(&module_root).expect("create module dir");
        std::fs::write(
            module_root.join("oh-package.json5"),
            r#"{
              name: "entry",
              dependencies: {
                "source-har": "1.0.0"
              }
            }"#,
        )
        .expect("write entry oh-package");

        let dep_dir = project_root.join("oh_modules").join("source-har");
        std::fs::create_dir_all(&dep_dir).expect("create dep dir");
        std::fs::write(
            dep_dir.join("oh-package.json5"),
            r#"{
              name: "source-har",
              metadata: {
                runtimeOnly: {
                  packages: ["@runtime/indirect"]
                }
              }
            }"#,
        )
        .expect("write dep oh-package");

        let runtime_pkg = project_root
            .join("oh_modules")
            .join("@runtime")
            .join("indirect");
        std::fs::create_dir_all(runtime_pkg.join("src/main/ets")).expect("create runtime dep");
        std::fs::write(
            runtime_pkg.join("oh-package.json5"),
            r#"{ name: "@runtime/indirect", main: "src/main/ets/Index.ets" }"#,
        )
        .expect("write runtime oh-package");
        std::fs::write(
            runtime_pkg.join("src/main/ets/Index.ets"),
            "export default class RuntimeIndirect {}",
        )
        .expect("write runtime index");

        let mut config = make_config(false);
        config.project_root = project_root.clone();
        let module_cfg = ModuleConfig {
            name: "entry".to_string(),
            src_path: module_root,
            module_type: OhosModuleType::Entry,
        };

        let loader = generate_loader_json(&config, &module_cfg, &make_manifest());

        assert!(
            loader
                .dynamic_import_lib_info
                .contains_key("@runtime/indirect"),
            "dependency metadata.runtimeOnly.packages must feed dynamicImportLibInfo"
        );
    }

    /// Verify `byteCodeHarInfo` is populated when an oh_modules package has index.abc.
    #[test]
    fn test_loader_json_byte_code_har_info_populated_from_oh_modules() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let project_root = tmp.path().to_path_buf();
        let module_root = project_root.join("entry");

        // Create a ByteCode HAR package under oh_modules.
        let pkg_dir = project_root
            .join("oh_modules")
            .join("@ohos")
            .join("abc-lib");
        std::fs::create_dir_all(&pkg_dir).expect("create pkg dir");
        std::fs::write(pkg_dir.join("index.abc"), b"PANDA\x00\x00\x00").expect("write index.abc");
        std::fs::write(
            pkg_dir.join("oh-package.json5"),
            r#"{ name: "@ohos/abc-lib", compatibleSdkVersion: "5.0.0(12)" }"#,
        )
        .expect("write oh-package.json5");

        let mut config = make_config(false);
        config.project_root = project_root.clone();
        let module_cfg = ModuleConfig {
            name: "entry".to_string(),
            src_path: module_root,
            module_type: OhosModuleType::Entry,
        };
        let manifest = make_manifest();

        let loader = generate_loader_json(&config, &module_cfg, &manifest);

        assert!(
            loader.byte_code_har_info.contains_key("@ohos/abc-lib"),
            "byteCodeHarInfo must contain @ohos/abc-lib"
        );
        let entry = &loader.byte_code_har_info["@ohos/abc-lib"];
        assert_eq!(entry.compatible_sdk_version, "5.0.0(12)");
        assert!(entry.abc_path.ends_with("index.abc"));
        assert!(entry.source_maps_path.is_none());
    }
}
