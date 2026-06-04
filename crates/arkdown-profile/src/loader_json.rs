use rustc_hash::{FxHashMap, FxHashSet};
use std::path::{Component, Path, PathBuf};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use arkdown_core::{
    ArkdownConfig, BuildMode, HapManifest, ModuleConfig, OhosModuleType, PkgBriefInfo,
};

use crate::bytecode_har::{
    collect_bytecode_har_other_compile_files, collect_bytecode_har_update_version_info,
    collect_dynamic_import_lib_info, scan_bytecode_har_deps,
};

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

        // Merge in explicit sourceOption.workers entries from build-profile.json5.
        // Hvigor's `targetService.getWorkerConfig()` resolves these to absolute paths;
        // we do the same in resolve.rs, so they arrive here already absolute.
        if let Some(source_opt) = config
            .product
            .build_option
            .source_option
            .as_ref()
        {
            for worker_path in &source_opt.workers {
                let abs = to_absolute_path_string(worker_path);
                if !discovered_workers.contains(&abs) {
                    // Also add a compile entry if the file is under src/main/ets.
                    if let Ok(rel) = worker_path.strip_prefix(&ets_root) {
                        let entry = format!("./ets/{}", to_unix_path(rel));
                        if !compile_entry.contains(&entry) {
                            compile_entry.push(entry);
                        }
                    }
                    discovered_workers.push(abs);
                }
            }
        }

        // Add startup task srcEntry paths to compileEntry.
        // Matches hvigor's GenerateLoaderJson which includes startup task source files
        // so that the bundler processes them alongside ability entry points.
        // Read the appStartup reference directly from module.json5 since HapManifest
        // doesn't carry it forward.
        let module_json5_app_startup: Option<String> = {
            let path = module_cfg.src_path.join("src/main/module.json5");
            std::fs::read_to_string(&path).ok().and_then(|s| {
                #[derive(serde::Deserialize)]
                struct ModuleWrapper {
                    module: ModuleAppStartup,
                }
                #[derive(serde::Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct ModuleAppStartup {
                    #[serde(default)]
                    app_startup: Option<String>,
                }
                json5::from_str::<ModuleWrapper>(&s).ok()?.module.app_startup
            })
        };
        if let Some(app_startup_ref) = &module_json5_app_startup {
            if let Some(profile_name) = app_startup_ref.strip_prefix("$profile:") {
                let profile_path = module_cfg
                    .src_path
                    .join("src/main/resources/base/profile")
                    .join(format!("{}.json", profile_name));
                if let Ok(content) = std::fs::read_to_string(&profile_path) {
                    #[derive(serde::Deserialize)]
                    #[serde(rename_all = "camelCase")]
                    struct StartupProfile {
                        #[serde(default)]
                        startup_tasks: Vec<StartupTaskEntry>,
                    }
                    #[derive(serde::Deserialize)]
                    #[serde(rename_all = "camelCase")]
                    struct StartupTaskEntry {
                        src_entry: String,
                    }
                    if let Ok(profile) = json5::from_str::<StartupProfile>(&content) {
                        for task in &profile.startup_tasks {
                            let src = task.src_entry.trim_start_matches("./");
                            let entry = format!("./ets/{}", src.trim_start_matches("ets/"));
                            if !compile_entry.contains(&entry) {
                                compile_entry.push(entry);
                            }
                        }
                    }
                }
            }
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

        let bytecode_har_deps = scan_bytecode_har_deps(&module_cfg.src_path, &config.project_root);
        let other_compile_paths = collect_bytecode_har_other_compile_files(
            &bytecode_har_deps,
            &module_cfg.src_path,
            &config.project_root,
        );
        let other_compile_files: Vec<String> = other_compile_paths
            .iter()
            .map(|path| to_absolute_path_string(path))
            .collect();
        compile_entry.extend(other_compile_files.iter().cloned());

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

            // D2: When this module is itself a ByteCode HAR (metadata.byteCodeHar=true),
            // generate its own declarationEntry from abilities + extension abilities.
            // This provides the OHM URLs that downstream consumers of THIS ByteCode HAR
            // will put into their own loader.json declarationEntry.
            let bundle_name = &config.product.bundle_name;
            let mod_name = &module_cfg.name;
            let is_bytecode_har = {
                let pkg_path = module_cfg.src_path.join("oh-package.json5");
                std::fs::read_to_string(&pkg_path)
                    .ok()
                    .and_then(|s| {
                        #[derive(serde::Deserialize)]
                        struct PkgMetadata {
                            #[serde(default)]
                            metadata: MetadataSection,
                        }
                        #[derive(Default, serde::Deserialize)]
                        #[serde(rename_all = "camelCase")]
                        struct MetadataSection {
                            #[serde(default)]
                            byte_code_har: bool,
                        }
                        json5::from_str::<PkgMetadata>(&s).ok()
                    })
                    .map_or(false, |p| p.metadata.byte_code_har)
            };
            if is_bytecode_har {
                for ability_entry in &manifest.abilities {
                    let path = ability_entry.src_entry.trim_start_matches("./");
                    let ohm = format!(
                        "@bundle:{}/{}/{}",
                        bundle_name,
                        mod_name,
                        path.trim_end_matches(".ets").trim_end_matches(".ts")
                    );
                    entries.push(ohm);
                }
                for ext_ability in &manifest.extension_abilities {
                    let path = ext_ability.src_entry.trim_start_matches("./");
                    let ohm = format!(
                        "@bundle:{}/{}/{}",
                        bundle_name,
                        mod_name,
                        path.trim_end_matches(".ets").trim_end_matches(".ts")
                    );
                    entries.push(ohm);
                }
            }

            let mut seen = FxHashSet::default();
            entries.retain(|entry| seen.insert(entry.clone()));
            entries
        } else {
            Vec::new()
        };
        let update_version_info = collect_bytecode_har_update_version_info(
            &bytecode_har_deps,
            &module_cfg.src_path,
            &config.project_root,
        );

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

        // --- D5: basic hspNameOhmMap/harNameOhmMap from direct oh_modules deps ---
        let (hsp_name_ohm_map, har_name_ohm_map) =
            Self::collect_hsp_har_ohm_maps(config, module_cfg);

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
            hsp_name_ohm_map,
            har_name_ohm_map,
            package_manager_type: "ohpm".to_string(),
            hvigor_compile_entry,
            other_compile_files,
            dynamic_import_lib_info,
            router_map,
            hsp_resources_map: Self::collect_hsp_resources_map(&module_cfg.src_path, &config.project_root),
            update_version_info,
            an_build_out_put: format!(
                "{}/build/default/intermediates/loader_out/default/an/arm64-v8a",
                module_root
            ),
            // anBuildMode from buildOption.aotCompileMode (hvigor handleAotFields).
            // Defaults to "type" (compile-by-type AOT) when not specified.
            an_build_mode: config
                .product
                .build_option
                .aot_compile_mode
                .clone()
                .unwrap_or_else(|| "type".to_string()),
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
            support_chunks: config
                .product
                .build_option
                .ark_options
                .as_ref()
                .and_then(|ao| ao.support_chunks.then_some(true))
                .unwrap_or(false),
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

    /// Walk `oh_modules/` under `module_root`, find installed HSP packages,
    /// and collect their `ResourceTable.txt` paths.
    ///
    /// Matches hvigor's `getHspResourcesMap()` for remote HSP dependencies:
    /// `<dep_root>/ResourceTable.txt` is checked for each HSP package.
    fn collect_hsp_resources_map(module_root: &Path, project_root: &Path) -> FxHashMap<String, String> {
        let mut map = FxHashMap::default();
        // Check both module-level and project-level oh_modules for HSP packages.
        for base in &[
            module_root.join("oh_modules"),
            project_root.join("oh_modules"),
        ] {
        let Ok(entries) = std::fs::read_dir(base) else {
            continue;
        };
        for entry in entries.flatten() {
            let pkg_dir = entry.path();
            if !pkg_dir.is_dir() {
                continue;
            }
            // Only consider HSP packages (module.json5#module.type == "shared").
            let module_json5 = pkg_dir.join("src/main/module.json5");
            let module_json = pkg_dir.join("src/main/module.json");
            let content = std::fs::read_to_string(&module_json5)
                .or_else(|_| std::fs::read_to_string(&module_json));
            let Ok(content) = content else {
                continue;
            };
            #[derive(serde::Deserialize)]
            struct Wrapper {
                module: ModuleType,
            }
            #[derive(serde::Deserialize)]
            struct ModuleType {
                #[serde(rename = "type", default)]
                module_type: String,
            }
            let Ok(w) = json5::from_str::<Wrapper>(&content) else {
                continue;
            };
            if w.module.module_type != "shared" {
                continue;
            }
            // Check for ResourceTable.txt at the dep root (remote HSP layout).
            let resource_table = pkg_dir.join("ResourceTable.txt");
            if resource_table.is_file() {
                let pkg_name = pkg_dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default()
                    .to_string();
                map.entry(pkg_name).or_insert_with(|| to_absolute_path_string(&resource_table));
            }
        }
        } // end for base
        map
    }

    /// Scan HAR/HSP dependencies (direct and transitive) declared in
    /// `oh-package.json5` and build the `hspNameOhmMap` / `harNameOhmMap`.
    ///
    /// Matches hvigor's `getHspNameOhmMap()` / `getHarNameOhmMap()` including
    /// transitive dependency traversal via the DependencyManager graph.
    ///
    /// Key:   dependency alias (from `oh-package.json5#dependencies`)
    /// Value: `@bundle:<bundleName>/<moduleName>/<entryRelPath>`
    fn collect_hsp_har_ohm_maps(
        config: &ArkdownConfig,
        module_cfg: &ModuleConfig,
    ) -> (FxHashMap<String, String>, FxHashMap<String, String>) {
        let bundle_name = &config.product.bundle_name;
        let oh_modules_root = module_cfg.src_path.join("oh_modules");
        let project_oh_modules = config.project_root.join("oh_modules");

        let mut hsp_map: FxHashMap<String, String> = FxHashMap::default();
        let mut har_map: FxHashMap<String, String> = FxHashMap::default();
        let mut visited: FxHashSet<String> = FxHashSet::default();

        // Recursive BFS over the dependency graph.
        let mut queue: std::collections::VecDeque<PathBuf> = std::collections::VecDeque::new();
        queue.push_back(module_cfg.src_path.clone());

        while let Some(pkg_root) = queue.pop_front() {
            #[derive(Deserialize)]
            struct OhPkgDeps {
                #[serde(default)]
                dependencies: FxHashMap<String, String>,
            }
            let pkg_json_path = pkg_root.join("oh-package.json5");
            let Ok(pkg_content) = std::fs::read_to_string(&pkg_json_path) else {
                continue;
            };
            let deps: FxHashMap<String, String> =
                json5::from_str::<OhPkgDeps>(&pkg_content)
                    .map(|p| p.dependencies)
                    .unwrap_or_default();

            for dep_alias in deps.keys() {
                if !visited.insert(dep_alias.clone()) {
                    continue; // already processed this alias
                }

                // Resolve package directory under oh_modules.
                let pkg_dir =
                    Self::resolve_dep_dir(dep_alias, &oh_modules_root, &project_oh_modules);
                let Some(pkg_dir) = pkg_dir else {
                    continue;
                };

                // Read module.json5/module.json to determine type (shared=HSP).
                #[derive(Deserialize)]
                struct ModuleJson {
                    module: ModuleSection,
                }
                #[derive(Deserialize)]
                struct ModuleSection {
                    #[serde(rename = "type", default)]
                    module_type: String,
                    #[serde(default)]
                    name: Option<String>,
                }
                let module_json_content = ["src/main/module.json5", "src/main/module.json"]
                    .iter()
                    .find_map(|n| std::fs::read_to_string(pkg_dir.join(n)).ok());
                let Some(content) = module_json_content else {
                    // No module.json — still enqueue for further transitive deps.
                    queue.push_back(pkg_dir);
                    continue;
                };
                let Ok(parsed) = json5::from_str::<ModuleJson>(&content) else {
                    queue.push_back(pkg_dir);
                    continue;
                };
                let module_name = parsed
                    .module
                    .name
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| {
                        dep_alias.rsplit('/').next().unwrap_or(dep_alias).to_string()
                    });

                let entry_rel = Self::dep_entry_rel_path(&pkg_dir);
                let ohm_url =
                    format!("@bundle:{}/{}/{}", bundle_name, module_name, entry_rel);

                if parsed.module.module_type == "shared" {
                    hsp_map.insert(dep_alias.clone(), ohm_url);
                } else {
                    har_map.insert(dep_alias.clone(), ohm_url);
                }

                // Enqueue this dep for its own transitive deps.
                queue.push_back(pkg_dir);
            }
        }

        (hsp_map, har_map)
    }

    /// Resolve a dependency alias to its package directory under `oh_modules/`.
    fn resolve_dep_dir(
        dep_alias: &str,
        module_oh_modules: &Path,
        project_oh_modules: &Path,
    ) -> Option<PathBuf> {
        for base in &[module_oh_modules, project_oh_modules] {
            let candidate = base.join(dep_alias);
            if candidate.is_dir() {
                return Some(candidate);
            }
        }
        None
    }

    /// Return the entry file path relative to `pkg_dir` with extension stripped.
    ///
    /// Reads `oh-package.json5#main`; falls back to conventional candidates.
    fn dep_entry_rel_path(pkg_dir: &Path) -> String {
        #[derive(Deserialize)]
        struct PkgEntry {
            #[serde(default)]
            main: Option<String>,
        }
        let main_file: Option<String> = std::fs::read_to_string(pkg_dir.join("oh-package.json5"))
            .ok()
            .and_then(|s| json5::from_str::<PkgEntry>(&s).ok())
            .and_then(|p| p.main)
            .filter(|m| !m.is_empty());

        if let Some(main) = main_file {
            // Strip leading ./ and known extensions.
            let rel = main
                .trim_start_matches("./")
                .trim_end_matches(".ets")
                .trim_end_matches(".ts")
                .trim_end_matches(".js");
            if !rel.is_empty() {
                return rel.to_string();
            }
        }

        // Conventional fallbacks: these cover the common package layouts.
        for candidate_rel in &[
            "src/main/ets/index",
            "src/main/ets/Index",
            "index",
            "Index",
        ] {
            let candidate = pkg_dir.join(format!("{}.ets", candidate_rel));
            if candidate.exists() {
                return candidate_rel.to_string();
            }
            let candidate = pkg_dir.join(format!("{}.ts", candidate_rel));
            if candidate.exists() {
                return candidate_rel.to_string();
            }
        }

        "index".to_string()
    }

    fn build_pkg_brief_info(
        config: &ArkdownConfig,
        module_cfg: &ModuleConfig,
    ) -> FxHashMap<String, PkgBriefInfo> {
        let mut map = FxHashMap::default();
        let module_root = to_absolute_path_string(&module_cfg.src_path);
        let ets_src = format!("{}/src/main/ets", module_root);

        // Add the current module.
        map.insert(
            module_cfg.name.clone(),
            PkgBriefInfo {
                pkg_root: module_root.clone(),
                pkg_name: module_cfg.name.clone(),
                source_roots: vec![ets_src.clone()],
                original_source_roots: vec![ets_src],
            },
        );

        // Add all other workspace modules (hvigor includes the full target module graph).
        for project_module in &config.modules {
            if project_module.name == module_cfg.name {
                continue; // already added above
            }
            let pkg_root = to_absolute_path_string(&project_module.src_path);
            let pkg_ets_src = format!("{}/src/main/ets", pkg_root);
            map.insert(
                project_module.name.clone(),
                PkgBriefInfo {
                    pkg_root: pkg_root.clone(),
                    pkg_name: project_module.name.clone(),
                    source_roots: vec![pkg_ets_src.clone()],
                    original_source_roots: vec![pkg_ets_src],
                },
            );
        }

        // Add direct oh_modules dependencies (source packages with src/main/ets).
        // This mirrors hvigor's DependencyManager providing pkg context for all resolved pkgs.
        for base in &[
            module_cfg.src_path.join("oh_modules"),
            config.project_root.join("oh_modules"),
        ] {
            let Ok(entries) = std::fs::read_dir(base) else {
                continue;
            };
            for entry in entries.flatten() {
                let pkg_dir = entry.path();
                if !pkg_dir.is_dir() {
                    continue;
                }
                let ets_dir = pkg_dir.join("src/main/ets");
                if !ets_dir.is_dir() {
                    continue; // skip non-source packages (ByteCode HAR, etc.)
                }
                let pkg_name = pkg_dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default()
                    .to_string();
                if map.contains_key(&pkg_name) {
                    continue; // already present (prefer workspace module)
                }
                let pkg_root_str = to_absolute_path_string(&pkg_dir);
                let pkg_ets_src = to_absolute_path_string(&ets_dir);
                map.insert(
                    pkg_name.clone(),
                    PkgBriefInfo {
                        pkg_root: pkg_root_str,
                        pkg_name,
                        source_roots: vec![pkg_ets_src.clone()],
                        original_source_roots: vec![pkg_ets_src],
                    },
                );
            }
        }

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
