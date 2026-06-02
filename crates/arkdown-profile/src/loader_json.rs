use rustc_hash::{FxHashMap, FxHashSet};
use std::path::{Component, Path, PathBuf};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use arkdown_core::{
    ArkdownConfig, BuildMode, HapManifest, ModuleConfig, OhosModuleType, PkgBriefInfo,
};

use crate::bytecode_har::{collect_dynamic_import_lib_info, scan_bytecode_har_deps};

mod runtime_only;
use runtime_only::{
    collect_dynamic_import_dep_names, collect_runtime_only_package_source_files,
    collect_runtime_only_source_files, dedupe_paths, discover_worker_files,
};

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
    pub module_path_map: IndexMap<String, String>,
    /// hvigor compile mode string, e.g. `"esmodule"`.
    pub compile_mode: String,
    /// Absolute path to the project root directory.
    pub project_root_path: String,
    /// hvigor node_modules sandbox path under `intermediates/loader_out`.
    pub node_modules_path: String,
    /// ByteCode HAR dependency metadata.
    pub byte_code_har_info: FxHashMap<String, ByteCodeHarEntry>,
    /// ByteCode HAR declaration entries. Empty for the minimal HAP fixture.
    pub declaration_entry: Vec<String>,
    /// The OHOS module name (e.g. `"entry"`).
    pub module_name: String,
    /// HSP package name → OHM URL map.
    pub hsp_name_ohm_map: FxHashMap<String, String>,
    /// HAR package name → OHM URL map.
    pub har_name_ohm_map: FxHashMap<String, String>,
    /// Package manager type — `"ohpm"` for OHOS projects.
    pub package_manager_type: String,
    /// hvigor's serialized compile entries. Empty for the minimal HAP fixture.
    #[serde(rename = "compileEntry")]
    pub hvigor_compile_entry: Vec<String>,
    /// Additional compile files emitted by hvigor. Empty for the minimal fixture.
    pub other_compile_files: Vec<String>,
    /// Dynamic import metadata. Empty until dependency collection is implemented.
    pub dynamic_import_lib_info: IndexMap<String, serde_json::Value>,
    /// Router map entries. Empty for the minimal fixture.
    pub router_map: Vec<serde_json::Value>,
    /// HSP resource table map. Empty for the minimal fixture.
    pub hsp_resources_map: FxHashMap<String, String>,
    /// Incremental update metadata. Empty for the minimal fixture.
    pub update_version_info: FxHashMap<String, serde_json::Value>,
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
    pub pkg_name_to_pkg_brief_info: FxHashMap<String, PkgBriefInfo>,
    /// WebWorker source file paths. hvigor reads this field when present and
    /// expects filesystem paths to `.ets`, `.ts`, or `.js` worker sources.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub workers: Vec<String>,
}

impl LoaderJson {
    /// Build a [`LoaderJson`] from the processed profile data.
    ///
    /// Mirrors hvigor's `GenerateLoaderJson` task, which produces the manifest
    /// the bundler reads for entry points, module paths, and dependency metadata.
    pub fn generate(
        config: &ArkdownConfig,
        module_cfg: &ModuleConfig,
        manifest: &HapManifest,
    ) -> Self {
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
        compile_entry.extend(Self::collect_form_src_entries(module_cfg));

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
            if let Some(main) = Self::read_package_main(&module_cfg.src_path) {
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

        let router_map =
            Self::read_loader_router_maps(config, module_cfg, manifest.router_map.as_deref());

        let mut serialized_compile_entry = collect_runtime_only_source_files(config, module_cfg);
        serialized_compile_entry.extend(collect_runtime_only_package_source_files(
            config, module_cfg,
        ));
        serialized_compile_entry.extend(Self::collect_dependency_router_map_source_files(
            config, module_cfg,
        ));
        dedupe_paths(&mut serialized_compile_entry);

        let hvigor_compile_entry: Vec<String> = serialized_compile_entry
            .iter()
            .map(|path| to_absolute_path_string(path))
            .collect();
        let src_main_root = module_cfg.src_path.join("src/main");
        for path in &serialized_compile_entry {
            if let Ok(rel) = path.strip_prefix(&src_main_root) {
                compile_entry.push(format!("./{}", to_unix_path(rel)));
            } else {
                compile_entry.push(to_absolute_path_string(path));
            }
        }

        // Deduplicate while preserving order.
        {
            let mut seen = FxHashSet::default();
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
        let mut module_path_map = IndexMap::new();
        for project_module in &config.modules {
            module_path_map.insert(
                project_module.name.clone(),
                to_absolute_path_string(&project_module.src_path),
            );
        }
        if module_path_map.is_empty() {
            module_path_map.insert(module_cfg.name.clone(), module_root.clone());
        }

        // --- TASK-LOADER-01: scan oh_modules/ for ByteCode HAR dependencies ---
        let bytecode_har_deps = scan_bytecode_har_deps(&module_cfg.src_path, &config.project_root);

        let byte_code_har_info: FxHashMap<String, ByteCodeHarEntry> = bytecode_har_deps
            .iter()
            .cloned()
            .map(|e| e.into_loader_entry())
            .collect();

        let declaration_entry = if use_normalized_ohm_url {
            let mut entries: Vec<String> = bytecode_har_deps
                .iter()
                .flat_map(|dep| dep.declaration_entry.iter().cloned())
                .collect();
            let mut seen = FxHashSet::default();
            entries.retain(|entry| seen.insert(entry.clone()));
            entries
        } else {
            Vec::new()
        };

        // --- TASK-LOADER-02: populate dynamicImportLibInfo from oh-package.json5 ---
        let dynamic_import_lib_info: IndexMap<String, serde_json::Value> = {
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
            hsp_name_ohm_map: FxHashMap::default(),
            har_name_ohm_map: FxHashMap::default(),
            package_manager_type: "ohpm".to_string(),
            hvigor_compile_entry,
            other_compile_files: vec![],
            dynamic_import_lib_info,
            router_map,
            hsp_resources_map: FxHashMap::default(),
            update_version_info: FxHashMap::default(),
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
            pkg_name_to_pkg_brief_info: Self::build_pkg_brief_info(config, module_cfg),
            workers: discovered_workers,
        }
    }
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

/// Build the `pkgNameToPkgBriefInfo` map for the loader.json.
///
/// Produces one entry for the current module. HAR/HSP dependencies would be
/// added by a later pass (see `arkdown-plugin-har-dedup`).

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

pub(super) use arkdown_utils::to_unix_path;

pub(super) fn to_absolute_path_string(path: &Path) -> String {
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

pub(super) fn normalize_path_string(path: &Path) -> String {
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

impl LoaderJson {

    fn collect_form_src_entries(module_cfg: &ModuleConfig) -> Vec<String> {
        let profile_dir = module_cfg.src_path.join("src/main/resources/base/profile");
        let Ok(entries) = std::fs::read_dir(profile_dir) else {
            return Vec::new();
        };

        let mut form_entries = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(path) else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) else {
                continue;
            };
            let Some(forms) = value.get("forms").and_then(|forms| forms.as_array()) else {
                continue;
            };
            for form in forms {
                if let Some(src) = form.get("src").and_then(|src| src.as_str()) {
                    form_entries.push(src.to_string());
                }
            }
        }
        form_entries
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

    fn read_loader_router_maps(
        config: &ArkdownConfig,
        host_module: &ModuleConfig,
        router_map_ref: Option<&str>,
    ) -> Vec<serde_json::Value> {
        let mut router_maps = Vec::new();
        router_maps.extend(Self::read_loader_router_map(
            &config.product.bundle_name,
            &host_module.name,
            host_module,
            router_map_ref,
        ));

        for module_cfg in &config.modules {
            if module_cfg.name == host_module.name {
                continue;
            }
            let Some(router_map_ref) = Self::read_module_router_map_ref(module_cfg) else {
                continue;
            };
            router_maps.extend(Self::read_loader_router_map(
                &config.product.bundle_name,
                &host_module.name,
                module_cfg,
                Some(&router_map_ref),
            ));
        }

        router_maps
    }

    fn read_loader_router_map(
        bundle_name: &str,
        host_module_name: &str,
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
            let rel_ets = page_source_file
                .trim_start_matches("src/main/ets/")
                .trim_end_matches(".ets")
                .trim_end_matches(".ts")
                .trim_end_matches(".js");
            let ohmurl = if module_cfg.name == host_module_name {
                format!("@bundle:{bundle_name}/{host_module_name}/ets/{rel_ets}")
            } else {
                format!(
                    "@bundle:{bundle_name}/{host_module_name}@{}/ets/{rel_ets}",
                    module_cfg.name
                )
            };
            obj.insert("ohmurl".to_string(), serde_json::Value::String(ohmurl));
            obj.insert(
                "pageSourceFile".to_string(),
                serde_json::Value::String(absolute),
            );
        }

        entries.clone()
    }

    fn read_module_router_map_ref(module_cfg: &ModuleConfig) -> Option<String> {
        let path = module_cfg.src_path.join("src/main/module.json5");
        let content = std::fs::read_to_string(path).ok()?;
        #[derive(serde::Deserialize)]
        struct ModuleJson {
            module: ModuleRouterMap,
        }
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ModuleRouterMap {
            router_map: Option<String>,
        }
        json5::from_str::<ModuleJson>(&content)
            .ok()?
            .module
            .router_map
    }

    fn collect_dependency_router_map_source_files(
        config: &ArkdownConfig,
        host_module: &ModuleConfig,
    ) -> Vec<PathBuf> {
        let mut files = Vec::new();
        for module_cfg in &config.modules {
            if module_cfg.name == host_module.name {
                continue;
            }
            let Some(router_map_ref) = Self::read_module_router_map_ref(module_cfg) else {
                continue;
            };
            let router_map = Self::read_loader_router_map(
                &config.product.bundle_name,
                &host_module.name,
                module_cfg,
                Some(&router_map_ref),
            );
            files.extend(router_map.iter().filter_map(|entry| {
                entry
                    .get("pageSourceFile")
                    .and_then(serde_json::Value::as_str)
                    .map(PathBuf::from)
            }));
        }
        files
    }

    fn build_pkg_brief_info(
        _config: &ArkdownConfig,
        module_cfg: &ModuleConfig,
    ) -> FxHashMap<String, PkgBriefInfo> {
        let mut map = FxHashMap::default();
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
}

fn default_build_mode() -> String {
    "debug".to_string()
}

#[cfg(test)]
mod tests;
