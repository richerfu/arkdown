use rustc_hash::{FxHashMap, FxHashSet};
use std::path::{Component, Path, PathBuf};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use arkdown_config::build_profile::parse_api_version;
use arkdown_core::{
    ArkdownConfig, ArkdownError, BuildMode, HapManifest, ModuleConfig, OhosModuleType,
    OutputTarget, PkgBriefInfo,
};

use crate::bytecode_har::{
    collect_bytecode_har_other_compile_files, collect_bytecode_har_update_version_info,
    collect_dynamic_import_lib_info, scan_bytecode_har_deps,
};
use crate::form_profile::{collect_module_form_sources, ModuleFormProfileInput};

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
    /// Ark Native output path emitted by hvigor when AOT is enabled for this module.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub an_build_out_put: Option<String>,
    /// Ark Native build mode emitted by hvigor when AOT is enabled for this module.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub an_build_mode: Option<String>,
    /// AOT profile path emitted by hvigor when a valid `.ap` file participates.
    #[serde(rename = "apPath", skip_serializing_if = "Option::is_none")]
    pub ap_path: Option<String>,
    /// Preview/local-test build config path emitted by hvigor when requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_config_path: Option<String>,
    /// Quick-fix patch config copied from build config when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch_config: Option<serde_json::Value>,
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

struct AotLoaderFields {
    an_build_out_put: Option<String>,
    an_build_mode: Option<String>,
    ap_path: Option<String>,
    fallback_an_build: bool,
}

#[derive(Deserialize)]
struct DependencyPackageJson {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    dependencies: IndexMap<String, String>,
    #[serde(default)]
    main: Option<String>,
    #[serde(default)]
    metadata: Option<DependencyPackageMetadata>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DependencyPackageMetadata {
    #[serde(default)]
    byte_code_har: bool,
    #[serde(default)]
    abc_path: Option<String>,
    #[serde(default)]
    resource: Option<DependencyResourceMetadata>,
}

#[derive(Deserialize)]
struct DependencyResourceMetadata {
    #[serde(default)]
    directories: Vec<String>,
}

#[derive(Deserialize)]
struct DependencyModuleJson {
    module: DependencyModuleSection,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DependencyModuleSection {
    #[serde(rename = "type", default)]
    module_type: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    abilities: Vec<DependencyAbilityEntry>,
    #[serde(default)]
    extension_abilities: Vec<DependencyAbilityEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DependencyAbilityEntry {
    #[serde(default)]
    src_entry: Option<String>,
}

impl AotLoaderFields {
    fn from_config(
        config: &ArkdownConfig,
        module_cfg: &ModuleConfig,
        manifest: &HapManifest,
        module_root: &str,
    ) -> Self {
        let fallback_an_build = matches!(manifest.module_type, OhosModuleType::Har);
        if fallback_an_build {
            return Self::empty(fallback_an_build);
        }

        let api_version = config.sdk_config.compile_api_version;
        let an_build_mode = Self::resolve_an_build_mode(config);
        let ap_path = Self::resolve_ap_path(config, module_cfg);
        let ap_exists = ap_path.as_ref().is_some_and(|path| path.exists());
        let has_ap_path = ap_path.is_some();
        let ap_suffix_valid = ap_path.as_ref().map_or(true, |path| {
            path.to_string_lossy().replace('\\', "/").ends_with("ap")
        });

        if api_version == 9 && an_build_mode != "partial" && has_ap_path {
            panic!(
                "MISMATCH_AOT_COMPILE_MODE: apPath is only valid with aotCompileMode=partial on API 9"
            );
        }
        if api_version == 9 && an_build_mode == "partial" && !has_ap_path {
            panic!("PARTIAL_EMPTY_PATH: aotCompileMode=partial requires apPath on API 9");
        }
        if !ap_suffix_valid {
            panic!("PARTIAL_INVALID_SUFFIX: apPath must end with .ap");
        }
        if an_build_mode == "partial" && !ap_exists {
            let display_path = ap_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_default();
            panic!("PARTIAL_INVALID_PATH: apPath does not exist: {display_path}");
        }
        if ((api_version == 9 && !has_ap_path) || api_version >= 10) && an_build_mode.is_empty() {
            return Self::empty(fallback_an_build);
        }

        let an_build_out_put =
            format!("{module_root}/build/default/intermediates/loader_out/default/an/arm64-v8a");

        let should_write_ap_path =
            ((api_version == 9 && an_build_mode == "partial") || api_version >= 10) && ap_exists;
        if should_write_ap_path {
            let ap_path = ap_path.expect("ap_exists implies ap_path");
            Self::copy_ap_file(module_cfg, &ap_path);
            return Self {
                an_build_out_put: Some(an_build_out_put),
                an_build_mode: Some(an_build_mode),
                ap_path: Some(to_absolute_path_string(&ap_path)),
                fallback_an_build,
            };
        }

        let without_ap_path = (api_version == 9 && an_build_mode == "type" && !has_ap_path)
            || (api_version >= 10 && an_build_mode == "type" && !ap_exists);
        if without_ap_path {
            return Self {
                an_build_out_put: Some(an_build_out_put),
                an_build_mode: Some(an_build_mode),
                ap_path: None,
                fallback_an_build,
            };
        }

        Self::empty(fallback_an_build)
    }

    fn empty(fallback_an_build: bool) -> Self {
        Self {
            an_build_out_put: None,
            an_build_mode: None,
            ap_path: None,
            fallback_an_build,
        }
    }

    fn resolve_an_build_mode(config: &ArkdownConfig) -> String {
        let api_version = config.sdk_config.compile_api_version;
        if api_version == 9 {
            return config
                .product
                .build_option
                .aot_compile_mode
                .clone()
                .unwrap_or_default();
        }

        let host_pgo = config.product.build_option.host_pgo == Some(true);
        let is_openharmony = !config.product.runtime_os.eq_ignore_ascii_case("HarmonyOS");
        if is_openharmony
            && !aot_fixed_sdk_version(&config.sdk_config.compile_sdk_version)
            && !host_pgo
        {
            return String::new();
        }

        if host_pgo {
            "partial".to_string()
        } else {
            "type".to_string()
        }
    }

    fn resolve_ap_path(config: &ArkdownConfig, module_cfg: &ModuleConfig) -> Option<PathBuf> {
        if let Some(path) = config.product.build_option.ap_path.as_ref() {
            return Some(if path.is_absolute() {
                path.clone()
            } else {
                module_cfg.src_path.join(path)
            });
        }

        (config.sdk_config.compile_api_version >= 10)
            .then(|| module_cfg.src_path.join("modules.ap"))
    }

    fn copy_ap_file(module_cfg: &ModuleConfig, source: &Path) {
        let dest_dir = module_cfg.src_path.join("build/default/intermediates/ap");
        std::fs::create_dir_all(&dest_dir).unwrap_or_else(|error| {
            panic!(
                "failed to create hvigor AOT intermediates directory {}: {error}",
                dest_dir.display()
            )
        });
        let dest = dest_dir.join(format!("{}.ap", module_cfg.name));
        std::fs::copy(source, &dest).unwrap_or_else(|error| {
            panic!(
                "failed to copy AOT profile {} to {}: {error}",
                source.display(),
                dest.display()
            )
        });
    }
}

fn aot_fixed_sdk_version(version: &str) -> bool {
    let version = version.trim();
    if version.is_empty() {
        return true;
    }

    let Some(current) = parse_sdk_version_tuple(version) else {
        return true;
    };
    current > (4, 0, 8, 1)
}

fn parse_sdk_version_tuple(version: &str) -> Option<(u32, u32, u32, u32)> {
    let mut parts = version.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let revision = parts.next()?.parse().ok()?;
    let build = parts.next()?.parse().ok()?;
    parts
        .next()
        .is_none()
        .then_some((major, minor, revision, build))
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
        Self::try_generate(config, module_cfg, manifest)
            .unwrap_or_else(|err| panic!("failed to generate loader.json: {err}"))
    }

    /// Fallible form of [`LoaderJson::generate`].
    pub fn try_generate(
        config: &ArkdownConfig,
        module_cfg: &ModuleConfig,
        manifest: &HapManifest,
    ) -> Result<Self, ArkdownError> {
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
        let module_json_path = module_cfg.src_path.join("src/main/module.json5");
        let _form_sources = collect_module_form_sources(ModuleFormProfileInput {
            module_name: &module_cfg.name,
            module_root: &module_cfg.src_path,
            module_type: &module_cfg.module_type,
            module_json_path: &module_json_path,
            extension_abilities: &manifest.extension_abilities,
            project_modules: &config.modules,
            compatible_api_version: parse_api_version(&config.product.compatible_sdk_version)
                .or(Some(config.sdk_config.compile_api_version)),
        })?;

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
        if let Some(source_opt) = config.product.build_option.source_option.as_ref() {
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

        let router_map =
            Self::read_loader_router_maps(config, module_cfg, manifest.router_map.as_deref());

        let mut serialized_compile_entry = Self::collect_insight_intent_src_entries(module_cfg);
        serialized_compile_entry.extend(Self::collect_mock_compile_entries(config, module_cfg));
        serialized_compile_entry.extend(Self::collect_app_startup_compile_entries(module_cfg));
        serialized_compile_entry.extend(collect_runtime_only_source_files(config, module_cfg));
        serialized_compile_entry.extend(collect_runtime_only_package_source_files(
            config, module_cfg,
        ));
        serialized_compile_entry.extend(Self::collect_dependency_router_map_source_files(
            config, module_cfg,
        ));
        serialized_compile_entry.extend(Self::collect_dependency_ability_source_files(
            config, module_cfg,
        ));
        serialized_compile_entry.extend(Self::collect_dependency_insight_intent_src_entries(
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
        // Add resolved dependency packages to modulePathMap.
        // hvigor populates this from the DependencyManager graph; use the
        // declared dependency BFS as the local approximation.
        for pkg_dir in Self::collect_declared_dependency_roots(config, module_cfg) {
            let key = Self::dependency_package_name(&pkg_dir);
            module_path_map
                .entry(key)
                .or_insert_with(|| to_absolute_path_string(&pkg_dir));
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
                        // Support both root-level `byteCodeHar: true` (older hvigor
                        // format) and the newer `metadata.byteCodeHar: true` form.
                        // Keep in sync with bytecode_har::OhPkgMinimal.
                        #[derive(serde::Deserialize)]
                        #[serde(rename_all = "camelCase")]
                        struct PkgMetadata {
                            /// Root-level `byteCodeHar` (older hvigor format).
                            #[serde(default)]
                            byte_code_har: bool,
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
                    .map_or(false, |p| p.byte_code_har || p.metadata.byte_code_har)
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

        let aot_fields = AotLoaderFields::from_config(config, module_cfg, manifest, &module_root);
        let build_config_path = Self::loader_build_config_path(module_cfg);
        let build_config = Self::read_loader_build_config(&build_config_path);
        let patch_config = (!matches!(config.output_target, OutputTarget::Test))
            .then(|| {
                build_config
                    .as_ref()
                    .and_then(|config| config.get("patchConfig").cloned())
            })
            .flatten();
        let serialized_build_config_path = matches!(config.output_target, OutputTarget::Preview)
            .then(|| {
                build_config_path
                    .strip_prefix(&module_cfg.src_path)
                    .map(|path| to_unix_path(path))
                    .unwrap_or_else(|_| to_absolute_path_string(&build_config_path))
            });

        Ok(LoaderJson {
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
            hsp_resources_map: Self::collect_hsp_resources_map(config, module_cfg),
            update_version_info,
            an_build_out_put: aot_fields.an_build_out_put,
            an_build_mode: aot_fields.an_build_mode,
            ap_path: aot_fields.ap_path,
            build_config_path: serialized_build_config_path,
            patch_config,
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
            fallback_an_build: aot_fields.fallback_an_build,
            pkg_name_to_pkg_brief_info: Self::build_pkg_brief_info(config, module_cfg),
            workers: discovered_workers,
        })
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

    fn collect_dependency_ability_source_files(
        config: &ArkdownConfig,
        host_module: &ModuleConfig,
    ) -> Vec<PathBuf> {
        Self::collect_dependency_source_har_roots(config, host_module)
            .into_iter()
            .flat_map(|dep_root| Self::collect_dependency_module_ability_source_files(&dep_root))
            .collect()
    }

    fn collect_dependency_insight_intent_src_entries(
        config: &ArkdownConfig,
        host_module: &ModuleConfig,
    ) -> Vec<PathBuf> {
        Self::collect_dependency_source_har_roots(config, host_module)
            .into_iter()
            .flat_map(|dep_root| {
                Self::collect_dependency_module_insight_intent_src_entries(&dep_root)
            })
            .collect()
    }

    fn collect_dependency_source_har_roots(
        config: &ArkdownConfig,
        host_module: &ModuleConfig,
    ) -> Vec<PathBuf> {
        Self::collect_declared_dependency_roots(config, host_module)
            .into_iter()
            .filter(|dep_root| Self::is_source_har_package(dep_root))
            .collect()
    }

    fn collect_declared_dependency_roots(
        config: &ArkdownConfig,
        host_module: &ModuleConfig,
    ) -> Vec<PathBuf> {
        let host_oh_modules = host_module.src_path.join("oh_modules");
        let project_oh_modules = config.project_root.join("oh_modules");
        let mut roots = Vec::new();
        let mut visited: FxHashSet<String> = FxHashSet::default();
        let mut queue: std::collections::VecDeque<PathBuf> = std::collections::VecDeque::new();
        queue.push_back(host_module.src_path.clone());
        if config.project_root != host_module.src_path {
            queue.push_back(config.project_root.clone());
        }

        while let Some(pkg_root) = queue.pop_front() {
            let Some(pkg) = Self::read_dependency_package_json(&pkg_root) else {
                continue;
            };

            for dep_alias in pkg.dependencies.keys() {
                let current_oh_modules = pkg_root.join("oh_modules");
                let Some(dep_root) =
                    Self::resolve_dep_dir(dep_alias, &current_oh_modules, &project_oh_modules)
                        .or_else(|| {
                            Self::resolve_dep_dir(dep_alias, &host_oh_modules, &project_oh_modules)
                        })
                else {
                    continue;
                };

                let dep_root_key = to_absolute_path_string(&dep_root);
                if !visited.insert(dep_root_key) {
                    continue;
                }

                roots.push(dep_root.clone());
                queue.push_back(dep_root);
            }
        }

        roots
    }

    fn is_source_har_package(pkg_root: &Path) -> bool {
        if Self::is_bytecode_har_package(pkg_root) {
            return false;
        }
        Self::read_dependency_module_json(pkg_root)
            .is_some_and(|module_json| module_json.module.module_type == "har")
    }

    fn collect_dependency_module_ability_source_files(dep_root: &Path) -> Vec<PathBuf> {
        let Some(module_json) = Self::read_dependency_module_json(dep_root) else {
            return vec![];
        };

        let src_main = dep_root.join("src/main");
        module_json
            .module
            .abilities
            .iter()
            .chain(module_json.module.extension_abilities.iter())
            .filter_map(|ability| ability.src_entry.as_deref())
            .map(|src_entry| src_main.join(src_entry.trim_start_matches("./")))
            .filter(|path| Self::is_existing_loader_code_file(path))
            .collect()
    }

    fn collect_dependency_module_insight_intent_src_entries(dep_root: &Path) -> Vec<PathBuf> {
        let mut entries = Vec::new();
        for resource_dir in Self::dependency_resource_dirs(dep_root) {
            let profile_path = resource_dir.join("base/profile/insight_intent.json");
            let Ok(content) = std::fs::read_to_string(profile_path) else {
                continue;
            };

            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct InsightIntentProfile {
                #[serde(default)]
                insight_intents_src_entry: Vec<InsightIntentEntry>,
            }

            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct InsightIntentEntry {
                src_entry: Option<String>,
            }

            let Ok(profile) = json5::from_str::<InsightIntentProfile>(&content) else {
                continue;
            };
            let src_main = dep_root.join("src/main");
            entries.extend(
                profile
                    .insight_intents_src_entry
                    .iter()
                    .filter_map(|entry| entry.src_entry.as_deref())
                    .map(|src_entry| src_main.join(src_entry.trim_start_matches("./")))
                    .filter(|path| Self::is_existing_loader_code_file(path)),
            );
        }
        entries
    }

    fn dependency_resource_dirs(dep_root: &Path) -> Vec<PathBuf> {
        let directories = Self::read_dependency_package_json(dep_root)
            .and_then(|pkg| pkg.metadata)
            .and_then(|metadata| metadata.resource)
            .map(|resource| resource.directories)
            .unwrap_or_default();

        if directories.is_empty() {
            return vec![dep_root.join("src/main/resources")];
        }

        directories
            .into_iter()
            .map(|directory| dep_root.join(directory.trim_start_matches("./")))
            .collect()
    }

    fn read_dependency_package_json(pkg_root: &Path) -> Option<DependencyPackageJson> {
        let content = std::fs::read_to_string(pkg_root.join("oh-package.json5")).ok()?;
        json5::from_str::<DependencyPackageJson>(&content).ok()
    }

    fn read_dependency_module_json(pkg_root: &Path) -> Option<DependencyModuleJson> {
        ["src/main/module.json5", "src/main/module.json"]
            .iter()
            .find_map(|rel| std::fs::read_to_string(pkg_root.join(rel)).ok())
            .and_then(|content| json5::from_str::<DependencyModuleJson>(&content).ok())
    }

    fn loader_build_config_path(module_cfg: &ModuleConfig) -> PathBuf {
        [
            module_cfg
                .src_path
                .join("build/default/config/buildConfig.json"),
            module_cfg
                .src_path
                .join("build/default/intermediates/build_config/default/buildConfig.json"),
            module_cfg
                .src_path
                .join("build/default/intermediates/build_config/default/build-config.json"),
            module_cfg
                .src_path
                .join("build/default/intermediates/build_config/default/build_config.json"),
        ]
        .into_iter()
        .find(|path| path.is_file())
        .unwrap_or_else(|| {
            module_cfg
                .src_path
                .join("build/default/config/buildConfig.json")
        })
    }

    fn read_loader_build_config(path: &Path) -> Option<serde_json::Value> {
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content)
            .or_else(|_| json5::from_str(&content))
            .ok()
    }

    fn is_bytecode_har_package(pkg_root: &Path) -> bool {
        let Some(pkg) = Self::read_dependency_package_json(pkg_root) else {
            return false;
        };
        if pkg
            .metadata
            .as_ref()
            .is_some_and(|metadata| metadata.byte_code_har)
        {
            return true;
        }
        if pkg
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.abc_path.as_deref())
            .is_some_and(|abc_path| pkg_root.join(abc_path).is_file())
        {
            return true;
        }
        if pkg
            .main
            .as_deref()
            .is_some_and(|main| main.ends_with(".abc"))
        {
            return true;
        }

        pkg_root.join("ets/modules.abc").is_file() || pkg_root.join("index.abc").is_file()
    }

    fn collect_insight_intent_src_entries(module_cfg: &ModuleConfig) -> Vec<PathBuf> {
        let profile_path = module_cfg
            .src_path
            .join("src/main/resources/base/profile/insight_intent.json");
        let Ok(content) = std::fs::read_to_string(&profile_path) else {
            return vec![];
        };

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct InsightIntentProfile {
            #[serde(default)]
            insight_intents: Vec<InsightIntentEntry>,
            #[serde(default)]
            insight_intents_src_entry: Vec<InsightIntentEntry>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct InsightIntentEntry {
            src_entry: Option<String>,
        }

        let Ok(profile) = json5::from_str::<InsightIntentProfile>(&content) else {
            return vec![];
        };
        let src_main = module_cfg.src_path.join("src/main");
        let mut entries = Vec::new();
        for entry in profile
            .insight_intents
            .iter()
            .chain(profile.insight_intents_src_entry.iter())
        {
            let Some(src_entry) = entry.src_entry.as_deref() else {
                continue;
            };
            let path = src_main.join(src_entry.trim_start_matches("./"));
            if Self::is_existing_loader_code_file(&path) {
                entries.push(path);
            }
        }
        entries
    }

    fn collect_mock_compile_entries(
        config: &ArkdownConfig,
        module_cfg: &ModuleConfig,
    ) -> Vec<PathBuf> {
        if !matches!(
            config.output_target,
            OutputTarget::Preview | OutputTarget::Test
        ) {
            return vec![];
        }

        let mock_config_path = module_cfg.src_path.join("src/mock/mock-config.json5");
        let Ok(content) = std::fs::read_to_string(mock_config_path) else {
            return vec![];
        };
        let Ok(mock_entries) = json5::from_str::<IndexMap<String, serde_json::Value>>(&content)
        else {
            return vec![];
        };

        mock_entries
            .into_values()
            .filter_map(|entry| {
                let source = entry.get("source").and_then(|source| source.as_str())?;
                if source.is_empty() {
                    return None;
                }
                let path = module_cfg.src_path.join(source);
                Self::is_existing_hvigor_compile_entry_path(&path).then_some(path)
            })
            .collect()
    }

    fn collect_app_startup_compile_entries(module_cfg: &ModuleConfig) -> Vec<PathBuf> {
        let mut entries = Vec::new();
        entries.extend(Self::collect_app_startup_config_entry(module_cfg));
        entries.extend(Self::collect_app_startup_task_entries(module_cfg));
        entries
    }

    fn collect_app_startup_config_entry(module_cfg: &ModuleConfig) -> Vec<PathBuf> {
        let Some(app_startup_ref) = Self::read_module_app_startup_ref(module_cfg) else {
            return vec![];
        };
        let Some(profile_name) = app_startup_ref.strip_prefix("$profile:") else {
            return vec![];
        };
        let profile_path = module_cfg
            .src_path
            .join("src/main/resources/base/profile")
            .join(format!("{profile_name}.json"));
        let Ok(content) = std::fs::read_to_string(profile_path) else {
            return vec![];
        };

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct StartupProfile {
            config_entry: Option<String>,
        }

        let Ok(profile) = json5::from_str::<StartupProfile>(&content) else {
            return vec![];
        };
        let Some(config_entry) = profile.config_entry else {
            return vec![];
        };
        let path = module_cfg
            .src_path
            .join("src/main")
            .join(config_entry.trim_start_matches("./"));
        Self::is_existing_loader_code_file(&path)
            .then_some(path)
            .into_iter()
            .collect()
    }

    fn collect_app_startup_task_entries(module_cfg: &ModuleConfig) -> Vec<PathBuf> {
        let temp_startup_path = module_cfg
            .src_path
            .join("build/default/intermediates/startup_config/default/temp-startup-config.json");
        let Ok(content) = std::fs::read_to_string(temp_startup_path) else {
            return vec![];
        };

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct TempStartupConfig {
            #[serde(default)]
            startup_tasks: Vec<TempStartupTask>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct TempStartupTask {
            src_entry_absolute_path: Option<PathBuf>,
        }

        let Ok(config) = serde_json::from_str::<TempStartupConfig>(&content)
            .or_else(|_| json5::from_str::<TempStartupConfig>(&content))
        else {
            return vec![];
        };
        config
            .startup_tasks
            .into_iter()
            .filter_map(|task| task.src_entry_absolute_path)
            .filter(|path| Self::is_existing_loader_code_file(path))
            .collect()
    }

    fn read_module_app_startup_ref(module_cfg: &ModuleConfig) -> Option<String> {
        let path = module_cfg.src_path.join("src/main/module.json5");
        let content = std::fs::read_to_string(path).ok()?;
        #[derive(Deserialize)]
        struct ModuleJson {
            module: ModuleAppStartup,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ModuleAppStartup {
            app_startup: Option<String>,
        }
        json5::from_str::<ModuleJson>(&content)
            .ok()?
            .module
            .app_startup
    }

    fn is_existing_loader_code_file(path: &Path) -> bool {
        if !path.is_file() {
            return false;
        }
        let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
            return false;
        };
        matches!(extension.to_ascii_lowercase().as_str(), "ets" | "ts" | "js")
            && !path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    let lower = name.to_ascii_lowercase();
                    lower.ends_with(".d.ts") || lower.ends_with(".d.ets")
                })
    }

    fn is_existing_hvigor_compile_entry_path(path: &Path) -> bool {
        path.exists()
            && !path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    let lower = name.to_ascii_lowercase();
                    lower.ends_with(".d.ts") || lower.ends_with(".d.ets")
                })
    }

    /// Walk declared `oh_modules` dependencies, find installed HSP packages,
    /// and collect their `ResourceTable.txt` paths.
    ///
    /// Matches hvigor's `getHspResourcesMap()` for remote HSP dependencies:
    /// `<dep_root>/ResourceTable.txt` is checked for each HSP package.
    fn collect_hsp_resources_map(
        config: &ArkdownConfig,
        module_cfg: &ModuleConfig,
    ) -> FxHashMap<String, String> {
        let mut map = FxHashMap::default();
        for pkg_dir in Self::collect_declared_dependency_roots(config, module_cfg) {
            let Some(module_json) = Self::read_dependency_module_json(&pkg_dir) else {
                continue;
            };
            if module_json.module.module_type != "shared" {
                continue;
            }

            // Check for ResourceTable.txt at the dep root (remote HSP layout).
            let resource_table = pkg_dir.join("ResourceTable.txt");
            if resource_table.is_file() {
                let pkg_name = Self::dependency_package_name(&pkg_dir);
                map.entry(pkg_name)
                    .or_insert_with(|| to_absolute_path_string(&resource_table));
            }
        }
        map
    }

    fn dependency_package_name(pkg_dir: &Path) -> String {
        Self::read_dependency_package_json(pkg_dir)
            .and_then(|pkg| pkg.name)
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| Self::dependency_package_name_from_path(pkg_dir))
    }

    fn dependency_package_name_from_path(pkg_dir: &Path) -> String {
        let name = pkg_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let parent = pkg_dir
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|name| name.to_str());
        if let Some(parent) = parent.filter(|parent| parent.starts_with('@')) {
            format!("{parent}/{name}")
        } else {
            name.to_string()
        }
    }

    /// Scan HAR/HSP dependencies (direct and transitive) declared in
    /// `oh-package.json5` and build the `hspNameOhmMap` / `harNameOhmMap`.
    ///
    /// Matches hvigor's `getHspNameOhmMap()` / `getHarNameOhmMap()` including
    /// transitive dependency traversal via the DependencyManager graph.
    ///
    /// Key:   dependency package name
    /// Value: `@bundle:<bundleName>/<moduleName>/<entryRelPath>`
    fn collect_hsp_har_ohm_maps(
        config: &ArkdownConfig,
        module_cfg: &ModuleConfig,
    ) -> (FxHashMap<String, String>, FxHashMap<String, String>) {
        let bundle_name = &config.product.bundle_name;

        let mut hsp_map: FxHashMap<String, String> = FxHashMap::default();
        let mut har_map: FxHashMap<String, String> = FxHashMap::default();

        for pkg_dir in Self::collect_declared_dependency_roots(config, module_cfg) {
            let Some(parsed) = Self::read_dependency_module_json(&pkg_dir) else {
                continue;
            };
            let dep_name = Self::dependency_package_name(&pkg_dir);
            let module_name = parsed
                .module
                .name
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| dep_name.rsplit('/').next().unwrap_or(&dep_name).to_string());

            let entry_rel = Self::dep_entry_rel_path(&pkg_dir);
            let ohm_url = format!("@bundle:{}/{}/{}", bundle_name, module_name, entry_rel);

            if parsed.module.module_type == "shared" {
                hsp_map.insert(dep_name, ohm_url);
            } else if parsed.module.module_type == "har" {
                har_map.insert(dep_name, ohm_url);
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
        for candidate_rel in &["src/main/ets/index", "src/main/ets/Index", "index", "Index"] {
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

        // Detect effective target to include target-specific source root
        // (`src/<target>/ets/`) when a non-default target is active.
        //
        // hvigor adds `src/<target>` as an additional `sourceRoot` in
        // `pkgNameToPkgBriefInfo` when the effective target is not `default`.
        // This allows the compiler to resolve target-specific source files.
        let build_mode_name = match config.build_mode {
            arkdown_core::BuildMode::Debug => "debug",
            arkdown_core::BuildMode::Release => "release",
        };
        let target_ets_src = Self::detect_target_specific_ets_src(
            &module_cfg.src_path,
            &config.project_root,
            &module_cfg.name,
            &config.product.name,
            build_mode_name,
        );

        // Add the current module with original (src/main/ets) as originalSourceRoots
        // and target-specific source root (when present) prepended to sourceRoots.
        let source_roots = if let Some(ref target_ets) = target_ets_src {
            vec![target_ets.clone(), ets_src.clone()]
        } else {
            vec![ets_src.clone()]
        };
        map.insert(
            module_cfg.name.clone(),
            PkgBriefInfo {
                pkg_root: module_root.clone(),
                pkg_name: module_cfg.name.clone(),
                source_roots,
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

        // Add declared source dependencies with `src/main/ets`.
        // This mirrors hvigor's pkg context being based on resolved dependency
        // graph nodes rather than a blind `oh_modules` directory listing.
        for pkg_dir in Self::collect_declared_dependency_roots(config, module_cfg) {
            let ets_dir = pkg_dir.join("src/main/ets");
            if !ets_dir.is_dir() {
                continue; // skip non-source packages (ByteCode HAR, etc.)
            }
            let pkg_name = Self::dependency_package_name(&pkg_dir);
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

        map
    }

    /// Detect an active non-default target's `src/<target>/ets/` directory.
    ///
    /// hvigor includes this path in `pkgNameToPkgBriefInfo.sourceRoots` when
    /// a non-default effective target is selected.  The `originalSourceRoots`
    /// keeps `src/main/ets` so the compiler can resolve both roots.
    ///
    /// Returns `None` when:
    /// - There is no non-default effective target for this module.
    /// - The effective target's `src/<name>/ets/` directory does not exist.
    fn detect_target_specific_ets_src(
        module_root: &std::path::Path,
        project_root: &std::path::Path,
        module_name: &str,
        product_name: &str,
        build_mode_name: &str,
    ) -> Option<String> {
        use arkdown_config::build_profile::RawBuildProfile;

        let profile = RawBuildProfile::load(project_root).ok()?;
        let raw_module = profile
            .modules
            .iter()
            .find(|m| m.name == module_name)?;

        // Use centralised target selection for consistent priority rules.
        let target = arkdown_config::build_profile::pick_effective_raw_target(
            &raw_module.targets,
            product_name,
            build_mode_name,
        )?;

        // Only emit a target source root for non-default targets.
        if target.name == "default" {
            return None;
        }

        let target_ets = module_root.join("src").join(&target.name).join("ets");
        if target_ets.is_dir() {
            Some(to_absolute_path_string(&target_ets))
        } else {
            None
        }
    }
}

/// Select the effective target for pkgBriefInfo source-root detection.
///
// Target selection is now centralised in
// arkdown_config::build_profile::pick_effective_raw_target.

fn default_build_mode() -> String {
    "debug".to_string()
}

#[cfg(test)]
mod tests;
