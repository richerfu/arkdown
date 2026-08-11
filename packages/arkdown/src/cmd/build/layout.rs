use std::path::{Path, PathBuf};

use arkdown_core::OhosModuleType;

pub(super) struct ModuleSourceLayout<'a> {
  module_root: &'a Path,
}

/// CompileArkTS module-source cache paths owned by ets-loader's
/// `ModuleSourceFile.generateMockConfigFile` lifecycle.
pub(super) struct ModuleSourceCacheLayout<'a> {
  root: &'a Path,
}

impl<'a> ModuleSourceCacheLayout<'a> {
  pub(super) fn new(root: &'a Path) -> Self {
    Self { root }
  }

  pub(super) fn transformed_mock_config_json(&self) -> PathBuf {
    self.root.join("mock-config.json")
  }

  pub(super) fn user_mock_config_json5(&self) -> PathBuf {
    self.root.join("mock-config.json5")
  }
}

impl<'a> ModuleSourceLayout<'a> {
  pub(super) fn new(module_root: &'a Path) -> Self {
    Self { module_root }
  }

  pub(super) fn src_main(&self) -> PathBuf {
    self.module_root.join("src").join("main")
  }

  pub(super) fn ets(&self) -> PathBuf {
    self.src_main().join("ets")
  }

  pub(super) fn resources(&self) -> PathBuf {
    self.src_main().join("resources")
  }

  pub(super) fn module_json5(&self) -> PathBuf {
    self.src_main().join("module.json5")
  }

  /// `CoreModuleModelImpl.getMockConfigPath()` — fixed module mock manifest.
  pub(super) fn mock_config_json5(&self) -> PathBuf {
    self.module_root.join("src").join("mock").join("mock-config.json5")
  }

  pub(super) fn ohos_test_dir(&self) -> PathBuf {
    self.module_root.join("src").join("ohosTest")
  }

  pub(super) fn ohos_test_module_json5(&self) -> PathBuf {
    self.ohos_test_dir().join("module.json5")
  }

  pub(super) fn ohos_test_runner_ets(&self) -> PathBuf {
    self
      .ohos_test_dir()
      .join("ets")
      .join("testrunner")
      .join("OpenHarmonyTestRunner.ets")
  }

  pub(super) fn target_ets(&self, target_name: &str) -> PathBuf {
    self.module_root.join("src").join(target_name).join("ets")
  }

  pub(super) fn target_resources(&self, target_name: &str) -> PathBuf {
    self
      .module_root
      .join("src")
      .join(target_name)
      .join("resources")
  }
}

pub(super) struct ModuleBuildLayout<'a> {
  module_root: &'a Path,
  /// Build root directory name under the module root.
  ///
  /// `"build"` for normal HAP/HSP/HAR builds. Local unit-test builds
  /// (`--target test`, mirroring `hvigorw test`) use `".test"` — hvigor
  /// injects `buildRoot=.test` via `setTestDefaultParameters`.
  build_root: &'a str,
  /// Target name segment in intermediate/loader/cache paths (hvigor
  /// `getIntermediatesProcessProfile()` etc. key by target).
  target_name: &'a str,
}

impl<'a> ModuleBuildLayout<'a> {
  pub(super) fn new(module_root: &'a Path) -> Self {
    Self {
      module_root,
      build_root: "build",
      target_name: "default",
    }
  }

  /// Layout for a named target configuration (e.g. `ohosTest`), which keys
  /// the intermediate/loader/cache directories like hvigor.
  pub(super) fn for_target(module_root: &'a Path, target_name: &'a str) -> Self {
    let _ = target_name;
    Self {
      module_root,
      build_root: "build",
      target_name,
    }
  }

  /// Unit-test build layout rooted at `<module>/.test` (hvigor `buildRoot=.test`).
  pub(super) fn for_test(module_root: &'a Path) -> Self {
    Self {
      module_root,
      build_root: ".test",
      target_name: "default",
    }
  }

  /// Layout for the active output target and module target name: local
  /// unit-test builds root at `<module>/.test`; named targets (e.g.
  /// `ohosTest`) key intermediate/loader/cache directories like hvigor.
  pub(super) fn for_output_target(
    output_target: &arkdown_core::OutputTarget,
    module_root: &'a Path,
    target_name: &'a str,
  ) -> Self {
    if matches!(output_target, arkdown_core::OutputTarget::Test) {
      Self::for_test(module_root)
    } else if target_name != "default" {
      Self::for_target(module_root, target_name)
    } else {
      Self::new(module_root)
    }
  }

  pub(super) fn is_test(&self) -> bool {
    self.build_root == ".test"
  }

  pub(super) fn source(&self) -> ModuleSourceLayout<'a> {
    ModuleSourceLayout::new(self.module_root)
  }

  pub(super) fn oh_package_json5(&self) -> PathBuf {
    self.module_root.join("oh-package.json5")
  }

  pub(super) fn build_default(&self) -> PathBuf {
    self.module_root.join(self.build_root).join("default")
  }

  /// `.test/testability/` — generated local unit-test template directory
  /// (hvigor `getUnitTestTemplatePath`). Note this sits directly under
  /// `.test/`, not under `.test/<product>/`.
  pub(super) fn testability_dir(&self) -> PathBuf {
    self.module_root.join(".test").join("testability")
  }

  pub(super) fn test_ability_ets(&self) -> PathBuf {
    self.testability_dir().join("TestAbility.ets")
  }

  pub(super) fn testability_pages_dir(&self) -> PathBuf {
    self.testability_dir().join("pages")
  }

  pub(super) fn unit_test_index_ets(&self) -> PathBuf {
    self.testability_pages_dir().join("Index.ets")
  }

  pub(super) fn test_phone_settings_json(&self) -> PathBuf {
    self
      .testability_dir()
      .join("phoneSettingConfig_Phone.json")
  }

  pub(super) fn ohos_test_source_root(&self) -> PathBuf {
    self
      .intermediates()
      .join("src")
      .join("ohosTest")
  }

  pub(super) fn ohos_test_ets_root(&self) -> PathBuf {
    self.ohos_test_source_root().join("ets")
  }

  pub(super) fn ohos_test_resources_root(&self) -> PathBuf {
    self.ohos_test_source_root().join("resources")
  }

  pub(super) fn ohos_test_runner_dir(&self) -> PathBuf {
    self.ohos_test_ets_root().join("testrunner")
  }

  pub(super) fn ohos_test_ability_dir(&self) -> PathBuf {
    self.ohos_test_ets_root().join("testability")
  }

  pub(super) fn ohos_test_pages_dir(&self) -> PathBuf {
    self.ohos_test_ability_dir().join("pages")
  }

  pub(super) fn ohos_test_resources_base(&self) -> PathBuf {
    self.ohos_test_resources_root().join("base")
  }

  pub(super) fn ohos_test_resource_element_dir(&self) -> PathBuf {
    self.ohos_test_resources_base().join("element")
  }

  pub(super) fn ohos_test_resource_profile_dir(&self) -> PathBuf {
    self.ohos_test_resources_base().join("profile")
  }

  /// `.test/<product>/intermediates/test/` — coverage data and init files.
  pub(super) fn test_intermediates_dir(&self) -> PathBuf {
    self
      .module_root
      .join(self.build_root)
      .join("default")
      .join("intermediates")
      .join("test")
  }

  /// `.test/<product>/intermediates/test/init_coverage.json` — hvigor writes
  /// the instrumented-file inventory here for unit-test coverage collection.
  pub(super) fn test_init_coverage_json(&self) -> PathBuf {
    self.test_intermediates_dir().join("init_coverage.json")
  }

  /// `.test/config/buildConfig.json` — unit-test build config handoff
  /// (hvigor `getTestBuildConfigPath`), referenced by loader.json
  /// `buildConfigPath` in local-test mode.
  pub(super) fn test_build_config_json(&self) -> PathBuf {
    self.module_root.join(".test").join("config").join("buildConfig.json")
  }

  /// `src/test/` — local unit-test source directory (`List.test.ets` etc.).
  pub(super) fn test_sources_dir(&self) -> PathBuf {
    self.module_root.join("src").join("test")
  }

  pub(super) fn default_cache_dir(&self, cache_name: &str) -> PathBuf {
    self.build_default().join("cache").join(cache_name)
  }

  pub(super) fn package_har_cache_dir(&self, target_name: &str) -> PathBuf {
    self
      .default_cache_dir(target_name)
      .join(format!("{target_name}@PackageHar"))
  }

  pub(super) fn outputs_default(&self) -> PathBuf {
    self.build_default().join("outputs").join("default")
  }

  pub(super) fn outputs_root(&self) -> PathBuf {
    self.build_default().join("outputs")
  }

  pub(super) fn intermediates(&self) -> PathBuf {
    self.build_default().join("intermediates")
  }

  pub(super) fn default_intermediate_dir(&self, stage: &str) -> PathBuf {
    self.intermediates().join(stage).join(self.target_name)
  }

  pub(super) fn intermediate_dir(&self, stage: &str) -> PathBuf {
    self.intermediates().join(stage)
  }

  pub(super) fn default_intermediate_file(&self, stage: &str, file_name: &str) -> PathBuf {
    self.default_intermediate_dir(stage).join(file_name)
  }

  pub(super) fn default_intermediate_nested_file<const N: usize>(
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

  pub(super) fn res_default_dir(&self) -> PathBuf {
    self.default_intermediate_dir("res")
  }

  pub(super) fn res_module_json(&self) -> PathBuf {
    self.default_intermediate_file("res", "module.json")
  }

  pub(super) fn res_ark_module_json(&self) -> PathBuf {
    self.default_intermediate_file("res", "ark_module.json")
  }

  pub(super) fn res_resources_index(&self) -> PathBuf {
    self.default_intermediate_file("res", "resources.index")
  }

  pub(super) fn res_resources_dir(&self) -> PathBuf {
    self.default_intermediate_nested_file("res", ["resources"])
  }

  pub(super) fn res_resource_str_dir(&self) -> PathBuf {
    self.default_intermediate_nested_file("res", ["resource_str"])
  }

  pub(super) fn res_profile_dir(&self) -> PathBuf {
    self.default_intermediate_nested_file("res", ["resources", "base", "profile"])
  }

  /// `res/profile/main_pages.json` — unit-test page override written by
  /// ReplaceUnitTestIndexFile (`{ "src": ["<unit.test.replace.page>"] }`).
  pub(super) fn res_main_pages_json(&self) -> PathBuf {
    self.res_profile_dir().join("main_pages.json")
  }

  pub(super) fn preload_so_dir(&self) -> PathBuf {
    self.default_intermediate_dir("preload_so")
  }

  pub(super) fn preload_so_list_json(&self) -> PathBuf {
    self.default_intermediate_file("preload_so", "preload_so_list.json")
  }

  pub(super) fn preload_system_so_json(&self) -> PathBuf {
    self.default_intermediate_file("preload", "preload.json")
  }

  pub(super) fn share_config_temp_json(&self) -> PathBuf {
    self.default_intermediate_file("share_config", "temp-share-config.json")
  }

  pub(super) fn startup_config_temp_json(&self) -> PathBuf {
    self.default_intermediate_file("startup", "startup_config.json")
  }

  pub(super) fn loader_json(&self) -> PathBuf {
    self.default_intermediate_file("loader", "loader.json")
  }

  /// hvigor `BuildArtifactConst.WIDGET_LOADER_JSON` — the widget compile
  /// handoff loader (`GenerateLoaderJson.aceWidgetLoaderJson`). Only written
  /// when the module submits ArkTS widgets (`needSubmitArkTsWidget`).
  pub(super) fn widget_loader_json(&self) -> PathBuf {
    self.default_intermediate_file("loader", "widget_loader.json")
  }

  pub(super) fn pkg_context_info_json(&self) -> PathBuf {
    self.default_intermediate_file("loader", "pkgContextInfo.json")
  }

  pub(super) fn pkg_sdk_info_json(&self) -> PathBuf {
    self.default_intermediate_file("loader", "pkgSdkInfo.json")
  }

  pub(super) fn router_map_loader(&self) -> PathBuf {
    self.default_intermediate_file("router_map", "loader-router-map.json")
  }

  pub(super) fn router_map_temp(&self) -> PathBuf {
    self.default_intermediate_file("router_map", "temp-router-map.json")
  }

  pub(super) fn importer_map_dir(&self) -> PathBuf {
    self.default_intermediate_dir("importer_map")
  }

  pub(super) fn importer_map_json(&self) -> PathBuf {
    self.default_intermediate_file("importer_map", "importer_map.json")
  }

  pub(super) fn process_profile_module_json(&self) -> PathBuf {
    self.default_intermediate_file("process_profile", "module.json")
  }

  pub(super) fn merge_profile_module_json(&self) -> PathBuf {
    self.default_intermediate_file("merge_profile", "module.json")
  }

  pub(super) fn generated_resource_table_header(&self) -> PathBuf {
    self
      .build_default()
      .join("generated")
      .join("r")
      .join(self.target_name)
      .join("ResourceTable.h")
  }

  pub(super) fn generated_resource_table_typescript(&self) -> PathBuf {
    self
      .build_default()
      .join("generated")
      .join("r")
      .join(self.target_name)
      .join("ResourceTable.ts")
  }

  pub(super) fn generated_build_profile(&self, target_name: &str) -> PathBuf {
    self
      .build_default()
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
  pub(super) fn build_profile_for_module(
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

  pub(super) fn generated_module_info(&self, target_name: &str) -> PathBuf {
    self
      .build_default()
      .join("generated")
      .join("profile")
      .join(target_name)
      .join("ModuleInfo.ts")
  }

  pub(super) fn generated_pm_oh_package(&self, target_name: &str) -> PathBuf {
    self
      .build_default()
      .join("generated")
      .join("pm")
      .join(target_name)
      .join("oh-package.json5")
  }

  pub(super) fn base_native_libs_json(&self) -> PathBuf {
    self.default_intermediate_file("patch", "base_native_libs.json")
  }

  pub(super) fn hap_metadata_output_json(&self) -> PathBuf {
    self.default_intermediate_file("hap_metadata", "output_metadata.json")
  }

  pub(super) fn hsp_metadata_output_json(&self) -> PathBuf {
    self.default_intermediate_file("hsp_metadata", "output_metadata.json")
  }

  pub(super) fn source_maps_intermediate(&self) -> PathBuf {
    self.default_intermediate_file("source_map", "sourceMaps.map")
  }

  pub(super) fn loader_out_source_maps(&self) -> PathBuf {
    self.default_intermediate_nested_file("loader_out", ["ets", "sourceMaps.map"])
  }

  pub(super) fn loader_out_dir(&self) -> PathBuf {
    self.default_intermediate_dir("loader_out")
  }

  /// `intermediates/assets/<target>/ets` — UnitTestArkTS output consumed by
  /// `@ohos/coverage` Previewer execution.
  pub(super) fn test_assets_ets_dir(&self) -> PathBuf {
    self.default_intermediate_nested_file("assets", ["ets"])
  }

  pub(super) fn test_assets_modules_abc(&self) -> PathBuf {
    self.test_assets_ets_dir().join("modules.abc")
  }

  pub(super) fn test_assets_source_maps(&self) -> PathBuf {
    self.test_assets_ets_dir().join("sourceMaps.map")
  }

  pub(super) fn intermediates_mock_config_json(&self) -> PathBuf {
    self.default_intermediate_file("res", "mock-config.json")
  }

  pub(super) fn test_mock_config_output_json(&self) -> PathBuf {
    self
      .res_resources_dir()
      .join("rawfile")
      .join("mock")
      .join("mock-config.json")
  }

  pub(super) fn test_coverage_source_map(&self) -> PathBuf {
    self
      .test_intermediates_dir()
      .join("coverage_sourcemap")
      .join("unitTestSourceMap.map")
  }

  pub(super) fn declarations_dir(&self) -> PathBuf {
    self.loader_out_dir().join("etsFortgz")
  }

  pub(super) fn resource_table_txt(&self) -> PathBuf {
    self.default_intermediate_file("res", "ResourceTable.txt")
  }

  pub(super) fn syscap_dir(&self) -> PathBuf {
    self.default_intermediate_dir("syscap")
  }

  pub(super) fn har_duplicate_dir(&self) -> PathBuf {
    self.default_intermediate_dir("har_duplicate")
  }

  pub(super) fn source_har_duplicate_files_info(&self) -> PathBuf {
    self
      .har_duplicate_dir()
      .join("sourceHarDuplicateFilesInfo.txt")
  }

  pub(super) fn bytecode_har_duplicate_files_info(&self) -> PathBuf {
    self
      .har_duplicate_dir()
      .join("byteCodeHarDuplicateFilesInfo.txt")
  }

  pub(super) fn integrated_hsp_dir(&self) -> PathBuf {
    self.intermediate_dir("integrated_hsp")
  }

  pub(super) fn package_module_json(&self) -> PathBuf {
    self.default_intermediate_file("package", "module.json")
  }

  pub(super) fn output_pack_info(&self) -> PathBuf {
    self.outputs_default().join("pack.info")
  }

  pub(super) fn output_mapping_source_maps(&self) -> PathBuf {
    self
      .outputs_default()
      .join("mapping")
      .join("sourceMaps.map")
  }
}

pub(super) struct ResourceOutputLayout<'a> {
  res_out: &'a Path,
}

impl<'a> ResourceOutputLayout<'a> {
  pub(super) fn new(res_out: &'a Path) -> Self {
    Self { res_out }
  }

  pub(super) fn profile_dir(&self) -> PathBuf {
    self.res_out.join("resources").join("base").join("profile")
  }

  pub(super) fn profile_router_map(&self) -> PathBuf {
    self.profile_dir().join("default-router-map.json")
  }

  pub(super) fn opt_compression_json(&self) -> PathBuf {
    self.res_out.join("opt-compression.json")
  }

  pub(super) fn res_config_json(&self) -> PathBuf {
    self.res_out.join("resConfig.json")
  }

  pub(super) fn ids_map(&self) -> PathBuf {
    self.res_out.join("ids_map")
  }

  pub(super) fn id_defined_json(&self) -> PathBuf {
    self.ids_map().join("id_defined.json")
  }
}

pub(super) struct ProjectBuildLayout<'a> {
  project_root: &'a Path,
}

impl<'a> ProjectBuildLayout<'a> {
  pub(super) fn new(project_root: &'a Path) -> Self {
    Self { project_root }
  }

  pub(super) fn app_scope_resources(&self) -> PathBuf {
    self.project_root.join("AppScope").join("resources")
  }

  pub(super) fn obfuscation_default_dir(&self) -> PathBuf {
    self
      .project_root
      .join("build")
      .join("default")
      .join("intermediates")
      .join("obfuscation")
      .join("default")
  }

  pub(super) fn obfuscation_txt(&self) -> PathBuf {
    self.obfuscation_default_dir().join("obfuscation.txt")
  }
}

pub(super) struct RestoolLayout {
  image_transcoder_library: Option<PathBuf>,
  defined_sys_ids: Option<PathBuf>,
}

impl RestoolLayout {
  /// Resolve the resource toolchain layout once for the selected SDK.
  ///
  /// This mirrors hvigor's SDK split: `OhosSdkInfo` exposes no image
  /// transcoder extension, while `HosSdkInfo` resolves it from the sibling
  /// HMS toolchains component.
  pub(super) fn from_restool_path(restool_path: &Path) -> Self {
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
      let image_transcoder_library = Some(Self::image_transcoder_library_in(&hms_toolchains_dir));
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

  pub(super) fn image_transcoder_library(&self) -> Option<&Path> {
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

  pub(super) fn defined_sys_ids(&self) -> Option<PathBuf> {
    self.defined_sys_ids.clone()
  }
}
