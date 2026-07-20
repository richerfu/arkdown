use std::path::{Path, PathBuf};

use super::layout::{
  ModuleBuildLayout, ModuleSourceLayout, ProjectBuildLayout, ResourceOutputLayout, RestoolLayout,
};
use anyhow::Result;
use arkdown_core::{ArkdownConfig, ModuleConfig};
use arkdown_pack::minify_package_intermediate_module_json;
use arkdown_path::{
  canonicalize_or_owned as canonical_or_owned, ArtifactPath, ArtifactPathSerializer,
};

pub(super) struct IntermediateArtifactWriter {
  restool_layout: RestoolLayout,
}

struct RouterMapSourcePath<'a> {
  rel_path: &'a Path,
}

impl<'a> RouterMapSourcePath<'a> {
  fn new(rel_path: &'a Path) -> Self {
    Self { rel_path }
  }

  fn into_string(self) -> String {
    let rel_path = ArtifactPath::from_slash(ArtifactPathSerializer::rel_to_slash(self.rel_path));
    ArtifactPath::from_slash("src")
      .join("main")
      .join(rel_path.as_str())
      .into_string()
  }
}

struct HapMetadataName<'a> {
  module_name: &'a str,
}

impl<'a> HapMetadataName<'a> {
  fn new(module_name: &'a str) -> Self {
    Self { module_name }
  }

  fn signed_hap(&self) -> String {
    IntermediateText::from_parts([self.module_name, "-default-signed.hap"])
  }
}

struct IntermediateText;

impl IntermediateText {
  fn from_parts<const N: usize>(parts: [&str; N]) -> String {
    parts.into_iter().collect()
  }
}

struct RouterMapOwnership<'a> {
  package_name: &'a str,
  module_root: &'a Path,
  source_file: String,
}

impl IntermediateArtifactWriter {
  pub(super) fn new(restool_path: &Path) -> Self {
    Self {
      restool_layout: RestoolLayout::from_restool_path(restool_path),
    }
  }

  pub(super) fn write_router_map_intermediates(
    &self,
    module: &ModuleConfig,
    loader_json: &arkdown_profile::LoaderJson,
    config: &ArkdownConfig,
    res_out: &Path,
  ) -> Result<()> {
    if loader_json.router_map.is_empty() {
      return Ok(());
    }

    let loader_router_map = serde_json::json!({
        "routerMap": loader_json
            .router_map
            .iter()
            .filter_map(|entry| self.loader_router_map_entry(entry))
            .collect::<Vec<_>>()
    });
    let temp_router_map = serde_json::json!({
        "routerMap": loader_json
            .router_map
            .iter()
            .filter_map(|entry| {
                self.temp_router_map_entry(entry, loader_json, config)
            })
            .collect::<Vec<_>>()
    });
    let resource_router_map = serde_json::json!({
        "routerMap": loader_json
            .router_map
            .iter()
            .filter_map(|entry| {
                self.resource_router_map_entry(entry, loader_json, config)
            })
            .collect::<Vec<_>>()
    });
    let module_layout = ModuleBuildLayout::new(&module.src_path);
    self.write_bytes(
      &module_layout.router_map_loader(),
      &serde_json::to_vec(&loader_router_map)?,
    )?;
    self.write_bytes(
      &module_layout.router_map_temp(),
      &serde_json::to_vec(&temp_router_map)?,
    )?;
    let resource_layout = ResourceOutputLayout::new(res_out);
    self.write_bytes(
      &resource_layout.profile_router_map(),
      &serde_json::to_vec(&resource_router_map)?,
    )?;
    Ok(())
  }

  fn loader_router_map_entry(&self, entry: &serde_json::Value) -> Option<serde_json::Value> {
    let obj = entry.as_object()?;
    let mut normalized = serde_json::Map::new();
    if let Some(value) = obj.get("ohmurl") {
      normalized.insert("ohmurl".to_string(), value.clone());
    }
    if let Some(value) = obj.get("name") {
      normalized.insert("name".to_string(), value.clone());
    }
    if let Some(value) = obj.get("pageSourceFile") {
      normalized.insert("pageSourceFile".to_string(), value.clone());
    }
    if let Some(value) = obj.get("buildFunction") {
      normalized.insert("buildFunction".to_string(), value.clone());
    }
    Some(serde_json::Value::Object(normalized))
  }

  fn temp_router_map_entry(
    &self,
    entry: &serde_json::Value,
    loader_json: &arkdown_profile::LoaderJson,
    config: &ArkdownConfig,
  ) -> Option<serde_json::Value> {
    let obj = entry.as_object()?;
    let ownership = self.router_map_ownership(entry, loader_json)?;

    let mut normalized = serde_json::Map::new();
    if let Some(value) = obj.get("name") {
      normalized.insert("name".to_string(), value.clone());
    }
    normalized.insert(
      "pageSourceFile".to_string(),
      serde_json::Value::String(ownership.source_file),
    );
    if let Some(value) = obj.get("buildFunction") {
      normalized.insert("buildFunction".to_string(), value.clone());
    }
    if let Some(value) = obj.get("data") {
      normalized.insert("data".to_string(), value.clone());
    }
    normalized.insert(
      "moduleNodeDir".to_string(),
      serde_json::Value::String(Self::path_string(ownership.module_root)),
    );
    if let Some(value) = obj.get("ohmurl") {
      normalized.insert("ohmurl".to_string(), value.clone());
    }
    normalized.insert(
      "bundleName".to_string(),
      serde_json::Value::String(config.product.bundle_name.clone()),
    );
    normalized.insert(
      "moduleName".to_string(),
      serde_json::Value::String(loader_json.module_name.clone()),
    );
    normalized.insert(
      "packageName".to_string(),
      serde_json::Value::String(ownership.package_name.to_string()),
    );
    Some(serde_json::Value::Object(normalized))
  }

  fn resource_router_map_entry(
    &self,
    entry: &serde_json::Value,
    loader_json: &arkdown_profile::LoaderJson,
    config: &ArkdownConfig,
  ) -> Option<serde_json::Value> {
    let obj = entry.as_object()?;
    let ownership = self.router_map_ownership(entry, loader_json)?;

    let mut normalized = serde_json::Map::new();
    if let Some(value) = obj.get("name") {
      normalized.insert("name".to_string(), value.clone());
    }
    normalized.insert(
      "pageSourceFile".to_string(),
      serde_json::Value::String(ownership.source_file),
    );
    if let Some(value) = obj.get("buildFunction") {
      normalized.insert("buildFunction".to_string(), value.clone());
    }
    if let Some(value) = obj.get("data") {
      normalized.insert("data".to_string(), value.clone());
    }
    if let Some(value) = obj.get("ohmurl") {
      normalized.insert("ohmurl".to_string(), value.clone());
    }
    normalized.insert(
      "bundleName".to_string(),
      serde_json::Value::String(config.product.bundle_name.clone()),
    );
    normalized.insert(
      "moduleName".to_string(),
      serde_json::Value::String(loader_json.module_name.clone()),
    );
    normalized.insert(
      "packageName".to_string(),
      serde_json::Value::String(ownership.package_name.to_string()),
    );
    Some(serde_json::Value::Object(normalized))
  }

  fn router_map_ownership<'a>(
    &self,
    entry: &serde_json::Value,
    loader_json: &'a arkdown_profile::LoaderJson,
  ) -> Option<RouterMapOwnership<'a>> {
    let obj = entry.as_object()?;
    let page_source = obj.get("pageSourceFile")?.as_str()?;
    let page_path = Path::new(page_source);
    let (package_name, module_root) =
      loader_json
        .module_path_map
        .iter()
        .find_map(|(name, root)| {
          let root = Path::new(root);
          page_path.starts_with(root).then_some((name.as_str(), root))
        })?;
    let src_main = ModuleSourceLayout::new(module_root).src_main();
    let source_file = page_path
      .strip_prefix(&src_main)
      .ok()
      .map(|path| RouterMapSourcePath::new(path).into_string())
      .unwrap_or_else(|| page_source.to_string());
    Some(RouterMapOwnership {
      package_name,
      module_root,
      source_file,
    })
  }

  pub(super) fn write_profile_intermediates(
    &self,
    module_root: &Path,
    final_module_json: &Path,
  ) -> Result<()> {
    let content = fs_err::read_to_string(final_module_json)?;
    let final_value: serde_json::Value = serde_json::from_str(&content)?;
    let module_layout = ModuleBuildLayout::new(module_root);

    let process_value = self.profile_without_resource_ids(final_value.clone());
    self.write_json_pretty(&module_layout.process_profile_module_json(), &process_value)?;
    self.write_json_minified(&module_layout.res_ark_module_json(), &process_value)?;

    let mut merge_value = process_value;
    if let Some(module) = merge_value
      .get_mut("module")
      .and_then(|value| value.as_object_mut())
    {
      module.remove("virtualMachine");
      module.remove("compileMode");
      module.remove("dependencies");
    }
    self.write_json_pretty(&module_layout.merge_profile_module_json(), &merge_value)?;

    Ok(())
  }

  fn profile_without_resource_ids(&self, mut value: serde_json::Value) -> serde_json::Value {
    if let Some(app) = value.get_mut("app").and_then(|value| value.as_object_mut()) {
      app.remove("iconId");
      app.remove("labelId");
    }
    if let Some(module) = value
      .get_mut("module")
      .and_then(|value| value.as_object_mut())
    {
      module.remove("descriptionId");
      if let Some(abilities) = module
        .get_mut("abilities")
        .and_then(|value| value.as_array_mut())
      {
        for ability in abilities {
          if let Some(ability) = ability.as_object_mut() {
            for field in [
              "descriptionId",
              "iconId",
              "labelId",
              "startWindowIconId",
              "startWindowBackgroundId",
            ] {
              ability.remove(field);
            }
          }
        }
      }
    }
    value
  }

  pub(super) fn write_resource_config_files(
    &self,
    project_root: &Path,
    module: &ModuleConfig,
    config: &ArkdownConfig,
    res_out: &Path,
  ) -> Result<()> {
    // Use src/main/resources as the default module resources path when no
    // specific resource directories have been determined.  The hvigor parity
    // report reads resConfig.json to compare resource inputs; callers that
    // know the effective resource directories should call the variant that
    // accepts an explicit list.
    let default_res_dir = ModuleSourceLayout::new(&module.src_path).resources();
    self.write_resource_config_files_with_dirs(
      project_root,
      module,
      config,
      res_out,
      &[default_res_dir],
      &[],
    )
  }

  /// Like [`Self::write_resource_config_files`] but uses `module_res_dirs` for
  /// `moduleResources` in `resConfig.json` instead of the default `src/main/resources`.
  pub(super) fn write_resource_config_files_with_dirs(
    &self,
    project_root: &Path,
    module: &ModuleConfig,
    config: &ArkdownConfig,
    res_out: &Path,
    module_res_dirs: &[PathBuf],
    dependency_res_dirs: &[PathBuf],
  ) -> Result<()> {
    let project_root = canonical_or_owned(project_root);
    let module_root = canonical_or_owned(&module.src_path);
    let res_out = canonical_or_owned(res_out);
    let module_layout = ModuleBuildLayout::new(&module_root);
    let resource_layout = ResourceOutputLayout::new(&res_out);
    let project_layout = ProjectBuildLayout::new(&project_root);
    let extension_path = self
      .restool_layout
      .image_transcoder_library()
      .map(IntermediateArtifactWriter::path_string)
      .unwrap_or_default();

    let dependencies: Vec<String> = dependency_res_dirs
      .iter()
      .map(|dir| IntermediateArtifactWriter::path_string(&canonical_or_owned(dir)))
      .collect();
    let module_names = if config.modules.is_empty() {
      module.name.clone()
    } else {
      config
        .modules
        .iter()
        .map(|module| module.name.as_str())
        .collect::<Vec<_>>()
        .join(",")
    };

    self.write_json_minified_newline(
      &resource_layout.opt_compression_json(),
      &serde_json::json!({
          "context": {
              "extensionPath": extension_path,
          },
          "compression": {
              "media": {
                  "enable": false,
              },
              "filters": [],
          },
      }),
    )?;

    let mut res_config = serde_json::json!({
        "configPath": IntermediateArtifactWriter::path_string(&module_layout.process_profile_module_json()),
        "packageName": config.product.bundle_name,
        "output": IntermediateArtifactWriter::path_string(&res_out),
        "moduleNames": module_names,
        "ResourceTable": [
            IntermediateArtifactWriter::path_string(&module_layout.generated_resource_table_header()),
        ],
        "applicationResource": IntermediateArtifactWriter::path_string(&project_layout.app_scope_resources()),
        "moduleResources": module_res_dirs.iter().map(|d| {
            serde_json::Value::String(IntermediateArtifactWriter::path_string(&canonical_or_owned(d)))
        }).collect::<Vec<_>>(),
        "dependencies": dependencies,
        "iconCheck": true,
        "compression": IntermediateArtifactWriter::path_string(&resource_layout.opt_compression_json()),
        "ids": IntermediateArtifactWriter::path_string(&resource_layout.ids_map()),
        "definedIds": IntermediateArtifactWriter::path_string(&resource_layout.id_defined_json()),
    });
    if let Some(defined_sys_ids) = self.restool_layout.defined_sys_ids() {
      res_config["definedSysIds"] =
        serde_json::Value::String(IntermediateArtifactWriter::path_string(&defined_sys_ids));
    }
    self.write_json_minified_newline(&resource_layout.res_config_json(), &res_config)?;

    Ok(())
  }
}

impl IntermediateArtifactWriter {
  pub(super) fn write_static_intermediate_files(&self, module_root: &Path) -> Result<()> {
    let module_layout = ModuleBuildLayout::new(module_root);
    self.write_bytes(
      &module_layout.base_native_libs_json(),
      br#"{"libs":{},"stripped":{}}"#,
    )?;
    self.write_bytes_if_missing(&module_layout.router_map_loader(), br#"{"routerMap":[]}"#)?;
    self.write_bytes_if_missing(&module_layout.router_map_temp(), br#"{"routerMap":[]}"#)?;
    // Router/share config files are written by their owning processors before
    // this placeholder pass. Missing-only writes preserve real hvigor outputs.
    Ok(())
  }

  pub(super) fn write_json_pretty(&self, path: &Path, value: &serde_json::Value) -> Result<()> {
    let content = serde_json::to_string_pretty(value)?;
    self.write_bytes(path, content.as_bytes())
  }

  pub(super) fn rewrite_package_intermediate_module_json(&self, path: &Path) -> Result<()> {
    let content = fs_err::read_to_string(path)?;
    let content = minify_package_intermediate_module_json(&content)
      .map_err(|err| anyhow::anyhow!("rewrite package module.json failed: {}", err))?;
    fs_err::write(path, content.as_bytes())?;
    Ok(())
  }

  pub(super) fn write_hap_metadata(
    &self,
    module_root: &Path,
    module_name: &str,
    device_types: &[String],
  ) -> Result<()> {
    let metadata_path = ModuleBuildLayout::new(module_root).hap_metadata_output_json();
    if let Some(parent) = metadata_path.parent() {
      fs_err::create_dir_all(parent)?;
    }
    let device_types = if device_types.is_empty() {
      vec!["phone".to_string()]
    } else {
      device_types.to_vec()
    };
    let metadata = serde_json::json!([{
        "hapName": HapMetadataName::new(module_name).signed_hap(),
        "deviceTypes": device_types,
        "isSigned": true,
        "dependRemoteHsps": [],
    }]);
    let mut content = serde_json::to_string(&metadata)?;
    content.push('\n');
    fs_err::write(metadata_path, content.as_bytes())?;
    Ok(())
  }

  pub(super) fn extract_zip_entry(
    &self,
    archive_path: &Path,
    entry_name: &str,
    output_path: &Path,
  ) -> Result<()> {
    if let Some(parent) = output_path.parent() {
      fs_err::create_dir_all(parent)?;
    }
    let file = fs_err::File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut entry = archive.by_name(entry_name)?;
    let mut output = fs_err::File::create(output_path)?;
    std::io::copy(&mut entry, &mut output)?;
    Ok(())
  }

  pub(super) fn copy_if_exists(&self, from: &Path, to: &Path) -> Result<()> {
    if !from.exists() {
      return Ok(());
    }
    if let Some(parent) = to.parent() {
      fs_err::create_dir_all(parent)?;
    }
    fs_err::copy(from, to)?;
    Ok(())
  }

  fn write_bytes(&self, path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
      fs_err::create_dir_all(parent)?;
    }
    fs_err::write(path, bytes)?;
    Ok(())
  }

  fn write_bytes_if_missing(&self, path: &Path, bytes: &[u8]) -> Result<()> {
    if path.exists() {
      return Ok(());
    }
    self.write_bytes(path, bytes)
  }

  fn write_json_minified(&self, path: &Path, value: &serde_json::Value) -> Result<()> {
    let content = serde_json::to_string(value)?;
    self.write_bytes(path, content.as_bytes())
  }

  fn write_json_minified_newline(&self, path: &Path, value: &serde_json::Value) -> Result<()> {
    let mut content = serde_json::to_string(value)?;
    content.push('\n');
    self.write_bytes(path, content.as_bytes())
  }

  fn path_string(path: &Path) -> String {
    ArtifactPathSerializer::path_to_slash_string(path)
      .unwrap_or_else(|_| ArtifactPathSerializer::rel_to_slash(path))
  }
}
