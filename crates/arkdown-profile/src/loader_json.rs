use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use arkdown_core::{ArkdownConfig, BuildMode, HapManifest, ModuleConfig};

// ---------------------------------------------------------------------------
// loader.json schema types
// ---------------------------------------------------------------------------

/// Top-level `loader.json` structure consumed by `arkdown-loader`.
///
/// This is generated from [`generate_loader_json`] and written by
/// [`crate::ProfileProcessor::write_loader_json`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoaderJson {
    /// Ordered list of entry-point source paths / OHM URLs.
    pub compile_entry: Vec<String>,
    /// Absolute path to the project root directory.
    pub project_root_path: String,
    /// Absolute path to the module source root directory (e.g. `<project>/entry/src/main`).
    pub module_root_path: String,
    /// Absolute path to the incremental compilation cache directory.
    pub cache_path: String,
    /// Absolute path where the ArkTS loader writes its output `.js` files.
    pub ace_module_build: String,
    /// `"debug"` or `"release"`.
    pub build_mode: String,
    /// Whether the bundler may split output into multiple chunks.
    pub support_chunks: bool,
    /// Whether OHM URL module identifiers are used in the bundle graph.
    pub use_normalized_ohm_url: bool,
    /// Package manager type — always `"ohpm"` for OHOS projects.
    pub package_manager_type: String,
    /// The OHOS module name (e.g. `"entry"`).
    pub module_name: String,
    /// Numeric SDK API version (e.g. `12`).
    pub api_version: u32,
    /// Map of module name → absolute module root path.
    #[serde(rename = "modulePathMap")]
    pub module_path_map: HashMap<String, String>,
    /// Whether to fall back to AN (Ark Native) build. Always `false`.
    pub fallback_an_build: bool,
    /// Map of package name → brief package info (source roots, etc.).
    #[serde(rename = "pkgNameToPkgBriefInfo")]
    pub pkg_name_to_pkg_brief_info: HashMap<String, PkgBriefInfo>,
    /// ByteCode HAR dependency metadata.
    #[serde(rename = "byteCodeHarInfo")]
    pub byte_code_har_info: HashMap<String, ByteCodeHarEntry>,
    /// HSP package name → OHM URL map.
    #[serde(rename = "hspNameOhmMap")]
    pub hsp_name_ohm_map: HashMap<String, String>,
    /// HAR package name → OHM URL map.
    #[serde(rename = "harNameOhmMap")]
    pub har_name_ohm_map: HashMap<String, String>,
    /// WebWorker source file paths.
    #[serde(default)]
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

    // Deduplicate while preserving order.
    {
        let mut seen = std::collections::HashSet::new();
        compile_entry.retain(|e| seen.insert(e.clone()));
    }

    let module_root = module_cfg.src_path.to_string_lossy().to_string();
    let project_root = config.project_root.to_string_lossy().to_string();
    let src_main = format!("{}/src/main", module_root);

    let build_mode_str = match config.build_mode {
        BuildMode::Debug => "debug".to_string(),
        BuildMode::Release => "release".to_string(),
    };

    let use_normalized_ohm_url = config
        .product
        .build_option
        .strict_mode
        .use_normalized_ohm_url;

    let mut module_path_map = HashMap::new();
    module_path_map.insert(module_cfg.name.clone(), module_root.clone());

    LoaderJson {
        compile_entry,
        project_root_path: project_root.clone(),
        module_root_path: src_main.clone(),
        cache_path: format!(
            "{}/build/default/intermediates/ets/default/cache/",
            module_root
        ),
        ace_module_build: format!(
            "{}/build/default/intermediates/ets/default/loader_out/",
            module_root
        ),
        build_mode: build_mode_str,
        support_chunks: false,
        use_normalized_ohm_url,
        package_manager_type: "ohpm".to_string(),
        module_name: module_cfg.name.clone(),
        api_version: config.sdk_config.compile_api_version,
        module_path_map,
        fallback_an_build: false,
        pkg_name_to_pkg_brief_info: build_pkg_brief_info(config, module_cfg),
        byte_code_har_info: HashMap::new(),
        hsp_name_ohm_map: HashMap::new(),
        har_name_ohm_map: HashMap::new(),
        workers: vec![],
    }
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
    let module_root = module_cfg.src_path.to_string_lossy().to_string();
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
                icon: None, label: None, description: None,
                start_window_icon: None, start_window_background: None,
            }],
            extension_abilities: vec![],
            dependencies: vec![],
            router_map: None,
            pages: Some("$profile:main_pages".to_string()),
            main_element: Some("EntryAbility".to_string()),
            description: None,
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
        let loader2: LoaderJson =
            serde_json::from_str(&json_str).expect("deserialization failed");

        // Verify key fields survived the round-trip
        assert_eq!(loader2.build_mode, "debug");
        assert_eq!(loader2.package_manager_type, "ohpm");
        assert!(!loader2.use_normalized_ohm_url);
        assert!(!loader2.support_chunks);
        assert_eq!(loader2.project_root_path, "/project");
        assert_eq!(loader2.module_root_path, "/project/entry/src/main");
        assert_eq!(loader2.compile_entry.len(), 1);
        assert_eq!(
            loader2.compile_entry[0],
            "./ets/entryability/EntryAbility.ets"
        );
        assert_eq!(loader2.module_name, "entry");
        assert_eq!(loader2.api_version, 12);
        assert!(loader2.module_path_map.contains_key("entry"));
        assert_eq!(loader2.module_path_map["entry"], "/project/entry");
        assert!(!loader2.fallback_an_build);
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
    fn test_loader_json_camel_case_keys() {
        let config = make_config(false);
        let module_cfg = make_module_cfg();
        let manifest = make_manifest();

        let loader = generate_loader_json(&config, &module_cfg, &manifest);
        let json_str = serde_json::to_string(&loader).expect("serialization failed");

        // camelCase keys must be present
        assert!(json_str.contains("compileEntry"), "missing compileEntry");
        assert!(json_str.contains("projectRootPath"), "missing projectRootPath");
        assert!(json_str.contains("moduleRootPath"), "missing moduleRootPath");
        assert!(json_str.contains("cachePath"), "missing cachePath");
        assert!(
            json_str.contains("aceModuleBuild"),
            "missing aceModuleBuild"
        );
        assert!(json_str.contains("buildMode"), "missing buildMode");
        assert!(json_str.contains("supportChunks"), "missing supportChunks");
        assert!(
            json_str.contains("useNormalizedOHMUrl") || json_str.contains("useNormalizedOhmUrl"),
            "missing useNormalizedOHMUrl"
        );
        assert!(
            json_str.contains("pkgNameToPkgBriefInfo"),
            "missing pkgNameToPkgBriefInfo"
        );
        assert!(json_str.contains("moduleName"), "missing moduleName");
        assert!(json_str.contains("apiVersion"), "missing apiVersion");
        assert!(json_str.contains("modulePathMap"), "missing modulePathMap");
        assert!(json_str.contains("fallbackAnBuild"), "missing fallbackAnBuild");
        assert!(json_str.contains("\"workers\""), "missing workers");
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
    fn test_loader_json_fallback_an_build_is_false() {
        let config = make_config(false);
        let module_cfg = make_module_cfg();
        let manifest = make_manifest();

        let loader = generate_loader_json(&config, &module_cfg, &manifest);
        assert!(!loader.fallback_an_build);
    }
}
