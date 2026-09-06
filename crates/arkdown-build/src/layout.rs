use std::path::{Path, PathBuf};

use arkdown_core::OhosModuleType;

pub struct ModuleSourceLayout<'a> {
    module_root: &'a Path,
}

/// CompileArkTS module-source cache paths owned by ets-loader's
/// `ModuleSourceFile.generateMockConfigFile` lifecycle.
pub struct ModuleSourceCacheLayout<'a> {
    root: &'a Path,
}

impl<'a> ModuleSourceCacheLayout<'a> {
    pub fn new(root: &'a Path) -> Self {
        Self { root }
    }

    pub fn transformed_mock_config_json(&self) -> PathBuf {
        self.root.join("mock-config.json")
    }

    pub fn user_mock_config_json5(&self) -> PathBuf {
        self.root.join("mock-config.json5")
    }
}

impl<'a> ModuleSourceLayout<'a> {
    pub fn new(module_root: &'a Path) -> Self {
        Self { module_root }
    }

    pub fn src_main(&self) -> PathBuf {
        self.module_root.join("src").join("main")
    }

    pub fn ets(&self) -> PathBuf {
        self.src_main().join("ets")
    }

    pub fn resources(&self) -> PathBuf {
        self.src_main().join("resources")
    }

    pub fn module_json5(&self) -> PathBuf {
        self.src_main().join("module.json5")
    }

    /// `CoreModuleModelImpl.getMockConfigPath()` — fixed module mock manifest.
    pub fn mock_config_json5(&self) -> PathBuf {
        self.module_root
            .join("src")
            .join("mock")
            .join("mock-config.json5")
    }

    pub fn ohos_test_dir(&self) -> PathBuf {
        self.module_root.join("src").join("ohosTest")
    }

    pub fn ohos_test_module_json5(&self) -> PathBuf {
        self.ohos_test_dir().join("module.json5")
    }

    pub fn ohos_test_runner_ets(&self) -> PathBuf {
        self.ohos_test_dir()
            .join("ets")
            .join("testrunner")
            .join("OpenHarmonyTestRunner.ets")
    }

    pub fn ohos_test_runner_ts(&self) -> PathBuf {
        self.ohos_test_dir()
            .join("ets")
            .join("testrunner")
            .join("OpenHarmonyTestRunner.ts")
    }

    /// Hvigor `ohos-test-util.ts::isTemplatesAfterSourceCodeMigration`.
    ///
    /// New-style `ohosTest` source sets omit the runner implementation and let
    /// `GenerateOhosTestTemplate` materialize it below `build/<product>`. When
    /// abilities are declared, hvigor only inspects the first ability's
    /// `srcEntry`; otherwise it falls back to the two legacy runner files.
    pub fn ohos_test_templates_after_migration(&self, module_json: &serde_json::Value) -> bool {
        if let Some(first_ability) = module_json
            .pointer("/module/abilities")
            .and_then(serde_json::Value::as_array)
            .and_then(|abilities| abilities.first())
        {
            return first_ability
                .get("srcEntry")
                .and_then(serde_json::Value::as_str)
                .is_none_or(str::is_empty);
        }
        !self.ohos_test_runner_ets().is_file() && !self.ohos_test_runner_ts().is_file()
    }

    pub fn target_ets(&self, target_name: &str) -> PathBuf {
        self.module_root.join("src").join(target_name).join("ets")
    }

    pub fn target_resources(&self, target_name: &str) -> PathBuf {
        self.module_root
            .join("src")
            .join(target_name)
            .join("resources")
    }
}

pub struct ModuleBuildLayout<'a> {
    module_root: &'a Path,
    /// Build root directory name under the module root.
    ///
    /// `"build"` for normal HAP/HSP/HAR builds, `".preview"` for PreviewBuild,
    /// and `".test"` for local unit-test builds.
    build_root: &'a str,
    /// Selected product segment used by `ModulePathInfoIml` for every build,
    /// test, and preview product directory.
    product_name: &'a str,
    /// Target name segment in intermediate/loader/cache paths (hvigor
    /// `getIntermediatesProcessProfile()` etc. key by target).
    target_name: &'a str,
}

impl<'a> ModuleBuildLayout<'a> {
    pub fn new(module_root: &'a Path) -> Self {
        Self::for_product(module_root, "default")
    }

    pub fn for_product(module_root: &'a Path, product_name: &'a str) -> Self {
        Self {
            module_root,
            build_root: "build",
            product_name,
            target_name: "default",
        }
    }

    pub fn for_target_product(
        module_root: &'a Path,
        product_name: &'a str,
        target_name: &'a str,
    ) -> Self {
        Self {
            module_root,
            build_root: "build",
            product_name,
            target_name,
        }
    }

    pub fn for_test_product(module_root: &'a Path, product_name: &'a str) -> Self {
        Self {
            module_root,
            build_root: ".test",
            product_name,
            target_name: "default",
        }
    }

    /// Layout used by hvigor `GenerateOhosTestTemplate` while compiling the
    /// normal `build/<product>/.../ohosTest` target.
    pub fn for_ohos_test_template(module_root: &'a Path, product_name: &'a str) -> Self {
        Self::for_product(module_root, product_name)
    }

    pub fn for_preview_product(
        module_root: &'a Path,
        product_name: &'a str,
        target_name: &'a str,
    ) -> Self {
        Self {
            module_root,
            build_root: ".preview",
            product_name,
            target_name,
        }
    }

    pub fn for_output_target_product(
        output_target: &arkdown_core::OutputTarget,
        module_root: &'a Path,
        product_name: &'a str,
        target_name: &'a str,
    ) -> Self {
        if matches!(output_target, arkdown_core::OutputTarget::Test) {
            Self::for_test_product(module_root, product_name)
        } else if matches!(output_target, arkdown_core::OutputTarget::Preview) {
            Self::for_preview_product(module_root, product_name, target_name)
        } else if target_name != "default" {
            Self::for_target_product(module_root, product_name, target_name)
        } else {
            Self::for_product(module_root, product_name)
        }
    }

    pub fn source(&self) -> ModuleSourceLayout<'a> {
        ModuleSourceLayout::new(self.module_root)
    }

    pub fn oh_package_json5(&self) -> PathBuf {
        self.module_root.join("oh-package.json5")
    }

    pub fn build_default(&self) -> PathBuf {
        self.module_root
            .join(self.build_root)
            .join(self.product_name)
    }

    /// `.test/testability/` — generated local unit-test template directory
    /// (hvigor `getUnitTestTemplatePath`). Note this sits directly under
    /// `.test/`, not under `.test/<product>/`.
    pub fn testability_dir(&self) -> PathBuf {
        self.module_root.join(".test").join("testability")
    }

    pub fn test_ability_ets(&self) -> PathBuf {
        self.testability_dir().join("TestAbility.ets")
    }

    pub fn testability_pages_dir(&self) -> PathBuf {
        self.testability_dir().join("pages")
    }

    pub fn unit_test_index_ets(&self) -> PathBuf {
        self.testability_pages_dir().join("Index.ets")
    }

    pub fn test_phone_settings_json(&self) -> PathBuf {
        self.testability_dir().join("phoneSettingConfig_Phone.json")
    }

    pub fn ohos_test_source_root(&self) -> PathBuf {
        self.intermediates().join("src").join("ohosTest")
    }

    pub fn ohos_test_ets_root(&self) -> PathBuf {
        self.ohos_test_source_root().join("ets")
    }

    pub fn ohos_test_resources_root(&self) -> PathBuf {
        self.ohos_test_source_root().join("resources")
    }

    pub fn ohos_test_runner_dir(&self) -> PathBuf {
        self.ohos_test_ets_root().join("testrunner")
    }

    pub fn ohos_test_ability_dir(&self) -> PathBuf {
        self.ohos_test_ets_root().join("testability")
    }

    pub fn ohos_test_pages_dir(&self) -> PathBuf {
        self.ohos_test_ability_dir().join("pages")
    }

    pub fn ohos_test_resources_base(&self) -> PathBuf {
        self.ohos_test_resources_root().join("base")
    }

    pub fn ohos_test_resource_element_dir(&self) -> PathBuf {
        self.ohos_test_resources_base().join("element")
    }

    pub fn ohos_test_resource_profile_dir(&self) -> PathBuf {
        self.ohos_test_resources_base().join("profile")
    }

    /// `.test/<product>/intermediates/test/` — coverage data and init files.
    pub fn test_intermediates_dir(&self) -> PathBuf {
        self.module_root
            .join(self.build_root)
            .join(self.product_name)
            .join("intermediates")
            .join("test")
    }

    /// `.test/<product>/intermediates/test/init_coverage.json` — hvigor writes
    /// the instrumented-file inventory here for unit-test coverage collection.
    pub fn test_init_coverage_json(&self) -> PathBuf {
        self.test_intermediates_dir().join("init_coverage.json")
    }

    /// `.test/config/buildConfig.json` — unit-test build config handoff
    /// (hvigor `getTestBuildConfigPath`), referenced by loader.json
    /// `buildConfigPath` in local-test mode.
    pub fn test_build_config_json(&self) -> PathBuf {
        self.module_root
            .join(".test")
            .join("config")
            .join("buildConfig.json")
    }

    /// `.preview/config/buildConfig.json` — DevEco Previewer handoff.
    pub fn preview_build_config_json(&self) -> PathBuf {
        self.module_root
            .join(".preview")
            .join("config")
            .join("buildConfig.json")
    }

    /// `.preview/PreviewBuildParam.json` — incremental restool handoff.
    pub fn preview_build_param_json(&self) -> PathBuf {
        self.module_root
            .join(".preview")
            .join("PreviewBuildParam.json")
    }

    /// `src/test/` — local unit-test source directory (`List.test.ets` etc.).
    pub fn test_sources_dir(&self) -> PathBuf {
        self.module_root.join("src").join("test")
    }

    pub fn default_cache_dir(&self, cache_name: &str) -> PathBuf {
        self.build_default().join("cache").join(cache_name)
    }

    pub fn package_har_cache_dir(&self, target_name: &str) -> PathBuf {
        self.default_cache_dir(target_name)
            .join(format!("{target_name}@PackageHar"))
    }

    pub fn package_sign_har_cache_dir(&self, target_name: &str) -> PathBuf {
        self.default_cache_dir(target_name)
            .join(format!("{target_name}@PackageSignHar"))
    }

    pub fn outputs_default(&self) -> PathBuf {
        self.build_default().join("outputs").join("default")
    }

    pub fn outputs_root(&self) -> PathBuf {
        self.build_default().join("outputs")
    }

    pub fn target_output_dir(&self) -> PathBuf {
        self.outputs_root().join(self.target_name)
    }

    pub fn intermediates(&self) -> PathBuf {
        self.build_default().join("intermediates")
    }

    pub fn default_intermediate_dir(&self, stage: &str) -> PathBuf {
        self.intermediates().join(stage).join(self.target_name)
    }

    pub fn intermediate_dir(&self, stage: &str) -> PathBuf {
        self.intermediates().join(stage)
    }

    pub fn default_intermediate_file(&self, stage: &str, file_name: &str) -> PathBuf {
        self.default_intermediate_dir(stage).join(file_name)
    }

    pub fn default_intermediate_nested_file<const N: usize>(
        &self,
        stage: &str,
        components: [&str; N],
    ) -> PathBuf {
        components
            .into_iter()
            .fold(self.default_intermediate_dir(stage), |path, component| {
                path.join(component)
            })
    }

    pub fn res_default_dir(&self) -> PathBuf {
        self.default_intermediate_dir("res")
    }

    pub fn res_module_json(&self) -> PathBuf {
        self.default_intermediate_file("res", "module.json")
    }

    pub fn res_ark_module_json(&self) -> PathBuf {
        self.default_intermediate_file("res", "ark_module.json")
    }

    pub fn res_resources_index(&self) -> PathBuf {
        self.default_intermediate_file("res", "resources.index")
    }

    pub fn res_resources_dir(&self) -> PathBuf {
        self.default_intermediate_nested_file("res", ["resources"])
    }

    pub fn res_resource_str_dir(&self) -> PathBuf {
        self.default_intermediate_nested_file("res", ["resource_str"])
    }

    pub fn res_profile_dir(&self) -> PathBuf {
        self.default_intermediate_nested_file("res", ["resources", "base", "profile"])
    }

    pub fn preload_so_dir(&self) -> PathBuf {
        self.default_intermediate_dir("preload_so")
    }

    pub fn preload_so_list_json(&self) -> PathBuf {
        self.default_intermediate_file("preload_so", "preload_so_list.json")
    }

    pub fn preload_system_so_json(&self) -> PathBuf {
        self.default_intermediate_file("preload", "preload.json")
    }

    pub fn share_config_temp_json(&self) -> PathBuf {
        self.default_intermediate_file("share_config", "temp-share-config.json")
    }

    pub fn startup_config_temp_json(&self) -> PathBuf {
        self.default_intermediate_file("startup", "startup_config.json")
    }

    pub fn loader_json(&self) -> PathBuf {
        self.default_intermediate_file("loader", "loader.json")
    }

    /// hvigor `BuildArtifactConst.WIDGET_LOADER_JSON` — the widget compile
    /// handoff loader (`GenerateLoaderJson.aceWidgetLoaderJson`). Only written
    /// when the module submits ArkTS widgets (`needSubmitArkTsWidget`).
    pub fn widget_loader_json(&self) -> PathBuf {
        self.default_intermediate_file("loader", "widget_loader.json")
    }

    pub fn pkg_context_info_json(&self) -> PathBuf {
        self.default_intermediate_file("loader", "pkgContextInfo.json")
    }

    pub fn router_map_loader(&self) -> PathBuf {
        self.default_intermediate_file("router_map", "loader-router-map.json")
    }

    pub fn router_map_temp(&self) -> PathBuf {
        self.default_intermediate_file("router_map", "temp-router-map.json")
    }

    pub fn importer_map_dir(&self) -> PathBuf {
        self.default_intermediate_dir("importer_map")
    }

    pub fn importer_map_json(&self) -> PathBuf {
        self.default_intermediate_file("importer_map", "importer_map.json")
    }

    pub fn process_profile_module_json(&self) -> PathBuf {
        self.default_intermediate_file("process_profile", "module.json")
    }

    pub fn merge_profile_module_json(&self) -> PathBuf {
        self.default_intermediate_file("merge_profile", "module.json")
    }

    pub fn generated_resource_table_header(&self) -> PathBuf {
        self.build_default()
            .join("generated")
            .join("r")
            .join(self.target_name)
            .join("ResourceTable.h")
    }

    pub fn generated_resource_table_typescript(&self) -> PathBuf {
        self.build_default()
            .join("generated")
            .join("r")
            .join(self.target_name)
            .join("ResourceTable.ts")
    }

    pub fn generated_build_profile(&self, target_name: &str) -> PathBuf {
        self.build_default()
            .join("generated")
            .join("profile")
            .join(target_name)
            .join("BuildProfile.ets")
    }

    /// Returns the exact `BuildProfile.ets` task output path for a module.
    ///
    /// Latest hvigor keeps application profiles below `build/.../generated`,
    /// while `ModulePathInfoIml.getHarGenerateBuildProfilePath` deliberately
    /// writes a HAR profile at the module root so `ProcessHarArtifacts` can
    /// include it in the package input tree.
    pub fn build_profile_for_module(
        &self,
        module_type: &OhosModuleType,
        target_name: &str,
    ) -> PathBuf {
        if matches!(module_type, OhosModuleType::Har) {
            self.module_root.join("BuildProfile.ets")
        } else {
            self.generated_build_profile(target_name)
        }
    }

    pub fn generated_module_info(&self, target_name: &str) -> PathBuf {
        self.build_default()
            .join("generated")
            .join("profile")
            .join(target_name)
            .join("ModuleInfo.ts")
    }

    pub fn generated_pm_oh_package(&self, target_name: &str) -> PathBuf {
        self.generated_pm_manifest(target_name, "oh-package.json5")
    }

    pub fn generated_pm_package_json(&self, target_name: &str) -> PathBuf {
        self.generated_pm_manifest(target_name, "package.json")
    }

    fn generated_pm_manifest(&self, target_name: &str, file_name: &str) -> PathBuf {
        self.build_default()
            .join("generated")
            .join("pm")
            .join(target_name)
            .join(file_name)
    }

    pub fn base_native_libs_json(&self) -> PathBuf {
        self.default_intermediate_file("patch", "base_native_libs.json")
    }

    pub fn patch_dir(&self) -> PathBuf {
        self.default_intermediate_dir("patch")
    }

    pub fn patch_changed_file_list(&self) -> PathBuf {
        self.patch_dir().join("changedFileList.json")
    }

    pub fn patch_ets_dir(&self) -> PathBuf {
        self.patch_dir().join("ets")
    }

    pub fn patch_modules_abc(&self) -> PathBuf {
        self.patch_ets_dir().join("modules.abc")
    }

    pub fn patch_source_maps(&self) -> PathBuf {
        self.patch_ets_dir().join("sourceMaps.map")
    }

    pub fn patch_resources_dir(&self) -> PathBuf {
        self.patch_dir().join("resources")
    }

    pub fn patch_libs_dir(&self) -> PathBuf {
        self.patch_dir().join("libs")
    }

    pub fn patch_json(&self) -> PathBuf {
        self.patch_dir().join("patch.json")
    }

    pub fn patch_ets_state_json(&self) -> PathBuf {
        self.patch_dir().join("ets.json")
    }

    pub fn patch_last_libs_json(&self) -> PathBuf {
        self.patch_dir().join("libs.json")
    }

    pub fn quickfix_json(&self) -> PathBuf {
        self.target_output_dir().join("quickfix.json")
    }

    pub fn stripped_native_libs_dir(&self) -> PathBuf {
        self.default_intermediate_dir("stripped_native_libs")
    }

    pub fn process_libs_dir(&self) -> PathBuf {
        self.default_intermediate_dir("libs")
    }

    pub fn symbol_map(&self) -> PathBuf {
        self.loader_out_dir().join("ets").join("symbolMap.map")
    }

    pub fn hap_metadata_output_json(&self) -> PathBuf {
        self.default_intermediate_file("hap_metadata", "output_metadata.json")
    }

    pub fn hsp_metadata_output_json(&self) -> PathBuf {
        self.default_intermediate_file("hsp_metadata", "output_metadata.json")
    }

    pub fn source_maps_intermediate(&self) -> PathBuf {
        self.default_intermediate_file("source_map", "sourceMaps.map")
    }

    pub fn loader_out_source_maps(&self) -> PathBuf {
        self.default_intermediate_nested_file("loader_out", ["ets", "sourceMaps.map"])
    }

    pub fn loader_out_dir(&self) -> PathBuf {
        self.default_intermediate_dir("loader_out")
    }

    /// `intermediates/loader_out/<target>/an` — root passed to the SDK packing
    /// tool as `--an-path`.
    pub fn aot_an_dir(&self) -> PathBuf {
        self.loader_out_dir().join("an")
    }

    /// AOT compiler output directory used by ets-loader for the only supported
    /// target triple in Hvigor 6.1.1.280.
    pub fn aot_output_dir(&self) -> PathBuf {
        self.aot_an_dir().join("arm64-v8a")
    }

    /// `intermediates/assets/<target>/ets` — UnitTestArkTS output consumed by
    /// `@ohos/coverage` Previewer execution.
    pub fn test_assets_ets_dir(&self) -> PathBuf {
        self.default_intermediate_nested_file("assets", ["ets"])
    }

    pub fn test_assets_modules_abc(&self) -> PathBuf {
        self.test_assets_ets_dir().join("modules.abc")
    }

    pub fn test_assets_source_maps(&self) -> PathBuf {
        self.test_assets_ets_dir().join("sourceMaps.map")
    }

    pub fn intermediates_mock_config_json(&self) -> PathBuf {
        self.default_intermediate_file("res", "mock-config.json")
    }

    pub fn test_mock_config_output_json(&self) -> PathBuf {
        self.res_resources_dir()
            .join("rawfile")
            .join("mock")
            .join("mock-config.json")
    }

    pub fn test_coverage_source_map(&self) -> PathBuf {
        self.test_intermediates_dir()
            .join("coverage_sourcemap")
            .join("unitTestSourceMap.map")
    }

    pub fn declarations_dir(&self) -> PathBuf {
        self.loader_out_dir().join("etsFortgz")
    }

    /// ArkTS 1.2 interop outputs observed in the current
    /// `CompileArkTS`/`InteropManager` task handoff.
    pub fn arkts_interop_dir(&self) -> PathBuf {
        self.default_intermediate_dir("declgen")
    }

    pub fn arkts_declgen_v1_dir(&self) -> PathBuf {
        self.arkts_interop_dir().join("declgenV1")
    }

    pub fn arkts_declgen_v2_dir(&self) -> PathBuf {
        self.arkts_interop_dir().join("declgenV2")
    }

    pub fn arkts_declgen_bridge_dir(&self) -> PathBuf {
        self.arkts_interop_dir().join("declgenBridgeCode")
    }

    pub fn arkts_decl_file_info(&self, package_name: &str) -> PathBuf {
        self.arkts_interop_dir()
            .join("decl-file-info")
            .join(package_name)
            .join("decl-fileInfo.json")
    }

    pub fn resource_table_txt(&self) -> PathBuf {
        self.default_intermediate_file("res", "ResourceTable.txt")
    }

    pub fn syscap_dir(&self) -> PathBuf {
        self.default_intermediate_dir("syscap")
    }

    pub fn har_duplicate_dir(&self) -> PathBuf {
        self.default_intermediate_dir("har_duplicate")
    }

    pub fn source_har_duplicate_files_info(&self) -> PathBuf {
        self.har_duplicate_dir()
            .join("sourceHarDuplicateFilesInfo.txt")
    }

    pub fn bytecode_har_duplicate_files_info(&self) -> PathBuf {
        self.har_duplicate_dir()
            .join("byteCodeHarDuplicateFilesInfo.txt")
    }

    pub fn integrated_hsp_dir(&self) -> PathBuf {
        self.intermediate_dir("integrated_hsp")
    }

    pub fn package_module_json(&self) -> PathBuf {
        self.default_intermediate_file("package", "module.json")
    }

    pub fn output_pack_info(&self) -> PathBuf {
        self.target_output_dir().join("pack.info")
    }

    /// `ModulePathInfoIml.getObfuscationExportRulePath()`.
    ///
    /// Hvigor scopes the exported consumer rules to both the selected product
    /// and target. Keeping this on the module layout prevents project-level
    /// `build/default` paths from leaking into named products and targets.
    pub fn obfuscation_export_rule_path(&self) -> PathBuf {
        self.default_intermediate_file("obfuscation", "obfuscation.txt")
    }

    pub fn output_mapping_source_maps(&self) -> PathBuf {
        self.target_output_dir()
            .join("mapping")
            .join("sourceMaps.map")
    }
}

pub struct ResourceOutputLayout<'a> {
    res_out: &'a Path,
}

impl<'a> ResourceOutputLayout<'a> {
    pub fn new(res_out: &'a Path) -> Self {
        Self { res_out }
    }

    pub fn profile_dir(&self) -> PathBuf {
        self.res_out.join("resources").join("base").join("profile")
    }

    pub fn profile_router_map(&self) -> PathBuf {
        self.profile_dir().join("default-router-map.json")
    }

    pub fn opt_compression_json(&self) -> PathBuf {
        self.res_out.join("opt-compression.json")
    }

    pub fn res_config_json(&self) -> PathBuf {
        self.res_out.join("resConfig.json")
    }

    pub fn ids_map(&self) -> PathBuf {
        self.res_out.join("ids_map")
    }

    pub fn id_defined_json(&self) -> PathBuf {
        self.ids_map().join("id_defined.json")
    }
}

pub struct ProjectBuildLayout<'a> {
    project_root: &'a Path,
}

impl<'a> ProjectBuildLayout<'a> {
    pub fn new(project_root: &'a Path) -> Self {
        Self { project_root }
    }

    pub fn app_scope_resources(&self) -> PathBuf {
        self.project_root.join("AppScope").join("resources")
    }
}

pub struct RestoolLayout {
    image_transcoder_library: Option<PathBuf>,
    defined_sys_ids: Option<PathBuf>,
}

impl RestoolLayout {
    /// Resolve the resource toolchain layout once for the selected SDK.
    ///
    /// This mirrors hvigor's SDK split: `OhosSdkInfo` exposes no image
    /// transcoder extension, while `HosSdkInfo` resolves it from the sibling
    /// HMS toolchains component.
    pub fn from_restool_path(restool_path: &Path) -> Self {
        let toolchains_dir = restool_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        let hms_toolchains_dir = toolchains_dir
            .parent()
            .filter(|component_dir| {
                component_dir.file_name().and_then(|name| name.to_str()) == Some("openharmony")
            })
            .and_then(Path::parent)
            .map(|sdk_default_dir| sdk_default_dir.join("hms").join("toolchains"))
            .filter(|path| path.is_dir());

        if let Some(hms_toolchains_dir) = hms_toolchains_dir {
            let image_transcoder_library =
                Some(Self::image_transcoder_library_in(&hms_toolchains_dir));
            let defined_sys_ids = Self::defined_sys_ids_in(&hms_toolchains_dir);
            return Self {
                image_transcoder_library,
                defined_sys_ids,
            };
        }

        let image_transcoder_library = if toolchains_dir.join("oh-uni-package.json").is_file() {
            None
        } else {
            Some(Self::image_transcoder_library_in(&toolchains_dir))
        };
        Self {
            defined_sys_ids: image_transcoder_library
                .as_ref()
                .and_then(|_| Self::defined_sys_ids_in(&toolchains_dir)),
            image_transcoder_library,
        }
    }

    pub fn image_transcoder_library(&self) -> Option<&Path> {
        self.image_transcoder_library.as_deref()
    }

    fn image_transcoder_library_in(toolchains_dir: &Path) -> PathBuf {
        let file_name = if cfg!(target_os = "windows") {
            "libimage_transcoder_shared.dll"
        } else if cfg!(target_os = "linux") {
            "libimage_transcoder_shared.so"
        } else {
            "libimage_transcoder_shared.dylib"
        };
        toolchains_dir.join("lib").join(file_name)
    }

    fn defined_sys_ids_in(toolchains_dir: &Path) -> Option<PathBuf> {
        let path = toolchains_dir.join("id_defined.json");
        path.is_file().then_some(path)
    }

    pub fn defined_sys_ids(&self) -> Option<PathBuf> {
        self.defined_sys_ids.clone()
    }
}
