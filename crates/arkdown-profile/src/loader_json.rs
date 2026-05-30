use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use arkdown_core::{ArkdownConfig, BuildMode, HapManifest, ModuleConfig};

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
    /// WebWorker source file paths.
    #[serde(skip)]
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

    // Discover WebWorker files from src/main/ets/workers/*.ets.
    // hvigor emits these as top-level `workers` entries AND includes them in
    // the bundler's compile entry list so they are compiled as separate chunks.
    let workers_dir = module_cfg.src_path.join("src/main/ets/workers");
    let mut discovered_workers: Vec<String> = vec![];
    if workers_dir.is_dir() {
        let mut worker_files: Vec<_> = std::fs::read_dir(&workers_dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("ets"))
            .collect();
        // Sort for deterministic ordering across platforms.
        worker_files.sort_by_key(|e| e.file_name());
        for entry in worker_files {
            if let Some(name) = entry.file_name().to_str() {
                let rel = format!("./ets/workers/{}", name);
                compile_entry.push(rel.clone());
                discovered_workers.push(rel);
            }
        }
    }

    // Discover router map page entries from router_map.json.
    // hvigor reads this profile and adds each destination page to compileEntry
    // so the router pages are compiled even if not imported by any ability.
    let router_map_path = module_cfg
        .src_path
        .join("src/main/resources/base/profile/router_map.json");
    if router_map_path.is_file() {
        if let Ok(content) = std::fs::read_to_string(&router_map_path) {
            #[derive(serde::Deserialize)]
            struct RouterMapFile {
                #[serde(rename = "routerMap")]
                router_map: Vec<RouterMapPage>,
            }
            #[derive(serde::Deserialize)]
            struct RouterMapPage {
                #[serde(rename = "pageSourceFile")]
                page_source_file: String,
            }
            // Attempt top-level `{ "routerMap": [...] }` format first, then
            // fall back to a bare array `[...]`.
            let pages_opt: Option<Vec<RouterMapPage>> =
                serde_json::from_str::<RouterMapFile>(&content)
                    .map(|f| f.router_map)
                    .ok()
                    .or_else(|| serde_json::from_str::<Vec<RouterMapPage>>(&content).ok());
            if let Some(pages) = pages_opt {
                for page in &pages {
                    // pageSourceFile is typically "src/main/ets/pages/Foo.ets";
                    // strip the "src/main/" prefix so it matches the ./ets/... format.
                    let rel =
                        if let Some(stripped) = page.page_source_file.strip_prefix("src/main/") {
                            format!("./{}", stripped)
                        } else {
                            page.page_source_file.clone()
                        };
                    compile_entry.push(rel);
                }
            }
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

    let mut module_path_map = HashMap::new();
    module_path_map.insert(module_cfg.name.clone(), module_root.clone());

    // --- TASK-LOADER-01: scan oh_modules/ for ByteCode HAR dependencies ---
    let byte_code_har_info: HashMap<String, ByteCodeHarEntry> = {
        let scanned = scan_bytecode_har_deps(&module_cfg.src_path, &config.project_root);
        scanned.into_iter().map(|e| e.into_loader_entry()).collect()
    };

    // --- TASK-LOADER-02: populate dynamicImportLibInfo from oh-package.json5 ---
    let dynamic_import_lib_info: HashMap<String, serde_json::Value> = {
        let dynamic_dep_names = read_dynamic_dep_names(&module_cfg.src_path);
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
        declaration_entry: vec![],
        module_name: module_cfg.name.clone(),
        hsp_name_ohm_map: HashMap::new(),
        har_name_ohm_map: HashMap::new(),
        package_manager_type: "ohpm".to_string(),
        hvigor_compile_entry: vec![],
        other_compile_files: vec![],
        dynamic_import_lib_info,
        router_map: vec![],
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
    use std::path::PathBuf;

    use arkdown_core::{
        AbilityEntry, ArkdownConfig, BuildMode, BuildOption, HapManifest, ModuleConfig,
        OhosModuleType, OutputTarget, ProductConfig, SdkConfig, StrictModeConfig,
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
