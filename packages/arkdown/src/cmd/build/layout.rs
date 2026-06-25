use std::path::{Path, PathBuf};

pub(super) struct ModuleSourceLayout<'a> {
  module_root: &'a Path,
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
}

impl<'a> ModuleBuildLayout<'a> {
  pub(super) fn new(module_root: &'a Path) -> Self {
    Self { module_root }
  }

  pub(super) fn source(&self) -> ModuleSourceLayout<'a> {
    ModuleSourceLayout::new(self.module_root)
  }

  pub(super) fn oh_package_json5(&self) -> PathBuf {
    self.module_root.join("oh-package.json5")
  }

  pub(super) fn build_default(&self) -> PathBuf {
    self.module_root.join("build").join("default")
  }

  pub(super) fn default_cache_dir(&self, cache_name: &str) -> PathBuf {
    self.build_default().join("cache").join(cache_name)
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
    self.intermediates().join(stage).join("default")
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

  pub(super) fn preload_so_dir(&self) -> PathBuf {
    self.default_intermediate_dir("preload_so")
  }

  pub(super) fn preload_so_list_json(&self) -> PathBuf {
    self.default_intermediate_file("preload_so", "preload_so_list.json")
  }

  pub(super) fn share_config_temp_json(&self) -> PathBuf {
    self.default_intermediate_file("share_config", "temp-share-config.json")
  }

  pub(super) fn startup_config_temp_json(&self) -> PathBuf {
    self.default_intermediate_file("startup_config", "temp-startup-config.json")
  }

  pub(super) fn loader_json(&self) -> PathBuf {
    self.default_intermediate_file("loader", "loader.json")
  }

  pub(super) fn pkg_context_info_json(&self) -> PathBuf {
    self.default_intermediate_file("pkgContextInfo", "pkgContextInfo.json")
  }

  pub(super) fn router_map_loader(&self) -> PathBuf {
    self.default_intermediate_file("router_map", "loader-router-map.json")
  }

  pub(super) fn router_map_temp(&self) -> PathBuf {
    self.default_intermediate_file("router_map", "temp-router-map.json")
  }

  pub(super) fn router_map_hash_cache(&self) -> PathBuf {
    self
      .intermediates()
      .join("cache")
      .join("router_map_hash.txt")
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
      .join("default")
      .join("ResourceTable.h")
  }

  pub(super) fn base_native_libs_json(&self) -> PathBuf {
    self.default_intermediate_file("patch", "base_native_libs.json")
  }

  pub(super) fn hap_metadata_output_json(&self) -> PathBuf {
    self.default_intermediate_file("hap_metadata", "output_metadata.json")
  }

  pub(super) fn source_maps_intermediate(&self) -> PathBuf {
    self.default_intermediate_file("source_map", "sourceMaps.map")
  }

  pub(super) fn loader_out_source_maps(&self) -> PathBuf {
    self.default_intermediate_nested_file("loader_out", ["ets", "sourceMaps.map"])
  }

  pub(super) fn har_dir(&self) -> PathBuf {
    self.default_intermediate_dir("har")
  }

  pub(super) fn har_extend_info_json(&self) -> PathBuf {
    self.default_intermediate_file("har", "har-extend-info.json")
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

  pub(super) fn obfuscate_cache_dir(&self) -> PathBuf {
    self
      .project_root
      .join("build")
      .join("default")
      .join("cache")
      .join("obfuscate")
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
  toolchains_dir: PathBuf,
}

impl RestoolLayout {
  pub(super) fn new(toolchains_dir: PathBuf) -> Self {
    Self { toolchains_dir }
  }

  pub(super) fn image_transcoder_library(&self) -> PathBuf {
    self
      .toolchains_dir
      .join("lib")
      .join("libimage_transcoder_shared.dylib")
  }

  pub(super) fn id_defined_json(&self) -> PathBuf {
    self.toolchains_dir.join("id_defined.json")
  }
}
