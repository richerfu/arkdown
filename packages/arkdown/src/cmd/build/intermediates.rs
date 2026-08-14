use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::layout::{
  ModuleBuildLayout, ModuleSourceLayout, ProjectBuildLayout, ResourceOutputLayout, RestoolLayout,
};
use anyhow::{Context, Result};
use arkdown::SourceMapInputFingerprint;
use arkdown_core::{ArkdownConfig, ModuleConfig, OhosModuleType, OutputTarget};
use arkdown_pack::minify_package_intermediate_module_json;
use arkdown_path::{
  canonicalize_or_owned as canonical_or_owned, ArtifactPath, ArtifactPathSerializer,
};

pub(super) struct IntermediateArtifactWriter {
  restool_layout: RestoolLayout,
}

/// One entry-module projection emitted for a feature module by Hvigor's
/// `AbstractGenerateMetadata.generateFeatureMetadata`.
pub(super) struct RelatedEntryMetadata {
  pub(super) module_name: String,
  pub(super) device_types: Vec<String>,
  pub(super) distro_filter: Option<serde_json::Value>,
}

/// Stable inputs owned by the `GenerateMetadata` task action.
pub(super) struct PackageMetadataInput<'a> {
  pub(super) project_root: &'a Path,
  pub(super) module_root: &'a Path,
  pub(super) output_target: &'a OutputTarget,
  pub(super) product_name: &'a str,
  pub(super) target_name: &'a str,
  pub(super) module_name: &'a str,
  pub(super) module_type: &'a OhosModuleType,
  pub(super) artifact_name: Option<&'a str>,
  pub(super) device_types: &'a [String],
  pub(super) is_signed: bool,
  pub(super) related_entries: Vec<RelatedEntryMetadata>,
}

/// SDK-specific package handoff for the aggregated source map.
///
/// HAP/HSP and source HAR stages materialize `intermediates/source_map`, while
/// current ByteCode HAR packaging reads the compiler-owned map directly and
/// removes that stale intermediate when a project switches artifact kind.
pub(super) enum SourceMapPackageHandoff<'a> {
  Materialize(&'a Path),
  Suppress(&'a Path),
}

/// Arkdown-owned cache sidecar for the source inputs behind `sourceMaps.json`.
///
/// The SDK records only serialized source maps. A source edit can keep that
/// JSON byte-identical (for example when it changes a string literal without
/// moving a mapped span), while `SourceMapGenerator` still puts that freshly
/// generated entry before cached entries. This compact sidecar carries the
/// already-inventoried source fingerprints needed to preserve that observable
/// insertion order without adding source reads to the Rolldown/plugin path.
#[derive(Debug, Default, serde::Deserialize, serde::Serialize)]
struct SourceMapInputFingerprintCache {
  #[serde(default)]
  schema_version: u32,
  #[serde(default)]
  entries: BTreeMap<String, String>,
}

impl SourceMapInputFingerprintCache {
  const SCHEMA_VERSION: u32 = 1;
  const FILE_NAME: &'static str = ".arkdown_source_map_inputs.json";

  fn load(compile_cache: &Path) -> Self {
    let path = Self::path(compile_cache);
    let Ok(content) = fs_err::read_to_string(path) else {
      return Self::default();
    };
    let Ok(cache) = serde_json::from_str::<Self>(&content) else {
      return Self::default();
    };
    (cache.schema_version == Self::SCHEMA_VERSION)
      .then_some(cache)
      .unwrap_or_default()
  }

  fn changed_keys(&self, current: &[SourceMapInputFingerprint]) -> BTreeSet<String> {
    current
      .iter()
      .filter(|entry| self.entries.get(&entry.key) != Some(&entry.content_hash))
      .map(|entry| entry.key.clone())
      .collect()
  }

  fn replace(&mut self, current: &[SourceMapInputFingerprint]) {
    self.schema_version = Self::SCHEMA_VERSION;
    self.entries = current
      .iter()
      .map(|entry| (entry.key.clone(), entry.content_hash.clone()))
      .collect();
  }

  fn save(&self, writer: &IntermediateArtifactWriter, compile_cache: &Path) -> Result<()> {
    writer.write_json_pretty(
      &Self::path(compile_cache),
      &serde_json::to_value(self).context("serialize source-map input cache")?,
    )
  }

  fn path(compile_cache: &Path) -> PathBuf {
    compile_cache.with_file_name(Self::FILE_NAME)
  }
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

  /// Match hvigor 6.24.3 CreateModuleInfo/CreateHarModuleInfo.
  ///
  /// Their `beforeTask()` always removes a stale output before `taskShouldDo()`
  /// decides whether the exact SDK helper module must be regenerated.
  pub(super) fn write_module_info(
    &self,
    module_root: &Path,
    product_name: &str,
    target_name: &str,
    should_write: bool,
  ) -> Result<()> {
    let output =
      ModuleBuildLayout::for_product(module_root, product_name).generated_module_info(target_name);
    if output.is_file() {
      fs_err::remove_file(&output)?;
    }
    if should_write {
      self.write_bytes(
        &output,
        b"\n//@ts-noCheck\nexport const __MODULE_NAME__: string = __getCurrentModuleName__()\nexport const __BUNDLE_NAME__: string = __getCurrentBundleName__()\n",
      )?;
    }
    Ok(())
  }

  pub(super) fn write_router_map_intermediates(
    &self,
    module: &ModuleConfig,
    loader_json: &arkdown_profile::LoaderJson,
    config: &ArkdownConfig,
    res_out: &Path,
  ) -> Result<()> {
    if loader_json.router_map.is_empty() {
      return self.write_empty_router_map_intermediates(&module.src_path, &config.product.name);
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
    let module_layout = ModuleBuildLayout::for_product(&module.src_path, &config.product.name);
    self.write_router_map_task_outputs(&module_layout, &loader_router_map, &temp_router_map)?;
    let resource_layout = ResourceOutputLayout::new(res_out);
    self.write_bytes(
      &resource_layout.profile_router_map(),
      &serde_json::to_vec(&resource_router_map)?,
    )?;
    Ok(())
  }

  /// Latest IDE owner: `ProcessRouterMap.generateTempRouterMap` always writes
  /// both task outputs when its router-map input is an empty array. The
  /// resource profile remains absent because there is no route to advertise.
  pub(super) fn write_empty_router_map_intermediates(
    &self,
    module_root: &Path,
    product_name: &str,
  ) -> Result<()> {
    let empty_router_map = serde_json::json!({ "routerMap": [] });
    self.write_router_map_task_outputs(
      &ModuleBuildLayout::for_product(module_root, product_name),
      &empty_router_map,
      &empty_router_map,
    )
  }

  fn write_router_map_task_outputs(
    &self,
    layout: &ModuleBuildLayout<'_>,
    loader_router_map: &serde_json::Value,
    temp_router_map: &serde_json::Value,
  ) -> Result<()> {
    self.write_bytes(
      &layout.router_map_loader(),
      &serde_json::to_vec(loader_router_map)?,
    )?;
    self.write_bytes(
      &layout.router_map_temp(),
      &serde_json::to_vec(temp_router_map)?,
    )?;
    Ok(())
  }

  /// Apply metadata owned by Hvigor's post-profile resource tasks.
  ///
  /// `ProcessRouterMap` and insight-intent processing run after
  /// `ProcessProfile`: merge/process intermediates therefore omit these
  /// derived fields, while the resource input and packaged module profile
  /// advertise the generated resource profiles.
  pub(super) fn apply_compile_resource_module_metadata(
    &self,
    module_json: &Path,
    has_router_map_entries: bool,
    insight_intent_profile: &Path,
  ) -> Result<()> {
    let content = fs_err::read_to_string(module_json)?;
    let mut value: serde_json::Value = serde_json::from_str(&content)?;
    let module = value
      .get_mut("module")
      .and_then(serde_json::Value::as_object_mut)
      .context("resource module.json must contain a module object")?;
    if has_router_map_entries {
      module
        .entry("routerMap".to_string())
        .or_insert_with(|| serde_json::Value::String("$profile:default-router-map".to_string()));
    }
    if self.insight_intent_profile_has_entries(insight_intent_profile) {
      module.insert(
        "hasInsightIntent".to_string(),
        serde_json::Value::Bool(true),
      );
    }
    self.write_json_minified(module_json, &value)
  }

  fn insight_intent_profile_has_entries(&self, path: &Path) -> bool {
    let Ok(content) = fs_err::read_to_string(path) else {
      return false;
    };
    let Ok(profile) = json5::from_str::<serde_json::Value>(&content) else {
      return false;
    };
    [
      "insightIntents",
      "insightIntentsSrcEntry",
      "extractInsightIntents",
    ]
    .into_iter()
    .any(|field| {
      profile
        .get(field)
        .and_then(serde_json::Value::as_array)
        .is_some_and(|entries| !entries.is_empty())
    })
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

  pub(super) fn write_merge_profile_intermediate(
    &self,
    module_layout: &ModuleBuildLayout<'_>,
    final_value: &serde_json::Value,
    current_extension_ability_count: usize,
  ) -> Result<()> {
    let mut merge_value = self.profile_without_resource_ids(final_value.clone());
    self.retain_current_extension_abilities(&mut merge_value, current_extension_ability_count);
    if let Some(module) = merge_value
      .get_mut("module")
      .and_then(|value| value.as_object_mut())
    {
      // Hvigor's MergeProfile contains only the current module's extension
      // abilities. ProcessProfile subsequently appends dependency HAR
      // extension abilities and its runtime compiler fields.
      module.remove("virtualMachine");
      module.remove("compileMode");
      module.remove("dependencies");
    }
    self.write_profile_json(&module_layout.merge_profile_module_json(), &merge_value)
  }

  pub(super) fn write_process_profile_intermediates(
    &self,
    module_layout: &ModuleBuildLayout<'_>,
    final_value: &serde_json::Value,
    current_extension_ability_count: usize,
    form_widget_abilities: &[serde_json::Value],
  ) -> Result<()> {
    let process_value = self.profile_without_resource_ids(final_value.clone());
    self.write_profile_json(&module_layout.process_profile_module_json(), &process_value)?;

    // Restool's ark_module.json describes the current resource module. Hvigor
    // appends dependency HAR extension abilities in ProcessProfile, after the
    // resource compiler has produced ark_module.json, so those dependency
    // entries must not leak back into the resource-stage artifact. Form
    // widget modules additionally carry the form extension module's merged
    // `type: "form"` abilities (`getTargetModuleOptObj`).
    let mut resource_ark_value = process_value.clone();
    self
      .retain_current_extension_abilities(&mut resource_ark_value, current_extension_ability_count);
    if !form_widget_abilities.is_empty() {
      let abilities = resource_ark_value
        .get_mut("module")
        .and_then(|module| module.as_object_mut())
        .map(|module| {
          module
            .entry("extensionAbilities")
            .or_insert_with(|| serde_json::Value::Array(Vec::new()))
        });
      if let Some(serde_json::Value::Array(abilities)) = abilities {
        abilities.extend(form_widget_abilities.iter().cloned());
      }
    }
    self.write_json_minified(&module_layout.res_ark_module_json(), &resource_ark_value)?;

    Ok(())
  }

  fn retain_current_extension_abilities(
    &self,
    value: &mut serde_json::Value,
    current_extension_ability_count: usize,
  ) {
    let Some(module) = value
      .get_mut("module")
      .and_then(serde_json::Value::as_object_mut)
    else {
      return;
    };
    let Some(extension_abilities) = module
      .get_mut("extensionAbilities")
      .and_then(serde_json::Value::as_array_mut)
    else {
      return;
    };
    extension_abilities.truncate(current_extension_ability_count);
    if extension_abilities.is_empty() {
      module.remove("extensionAbilities");
    }
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
    self.write_resource_config_files_with_dirs_and_names(
      project_root,
      module,
      config,
      res_out,
      module_res_dirs,
      dependency_res_dirs,
      &[],
      "default",
    )
  }

  /// Variant with an explicit restool module-name list (`moduleNames` in
  /// `resConfig.json`); the `ohosTest` target packs under its own module
  /// name (`entry_test`) instead of the configured module list.
  pub(super) fn write_resource_config_files_with_dirs_and_names(
    &self,
    project_root: &Path,
    module: &ModuleConfig,
    config: &ArkdownConfig,
    res_out: &Path,
    module_res_dirs: &[PathBuf],
    dependency_res_dirs: &[PathBuf],
    explicit_module_names: &[String],
    target_name: &str,
  ) -> Result<()> {
    let project_root = canonical_or_owned(project_root);
    let module_root = canonical_or_owned(&module.src_path);
    let res_out = canonical_or_owned(res_out);
    let module_layout = ModuleBuildLayout::for_output_target_product(
      &config.output_target,
      &module_root,
      &config.product.name,
      target_name,
    );
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
    let module_names = if !explicit_module_names.is_empty() {
      explicit_module_names.join(",")
    } else if config.modules.is_empty() {
      module.name.clone()
    } else {
      config
        .modules
        .iter()
        .map(|module| module.name.as_str())
        .collect::<Vec<_>>()
        .join(",")
    };

    let mut all_resource_dirs = module_res_dirs.to_vec();
    all_resource_dirs.extend_from_slice(dependency_res_dirs);
    let include_app_resource = !matches!(config.output_target, OutputTarget::Har)
      && (!matches!(module.module_type, OhosModuleType::Shared)
        || config
          .product
          .build_option
          .res_options
          .as_ref()
          .map(|options| options.include_app_scope_res)
          .unwrap_or(true));
    if include_app_resource {
      all_resource_dirs.push(project_layout.app_scope_resources());
    }
    let compression = self.opt_compression_config(config, &extension_path, &all_resource_dirs)?;
    self.write_json_minified_newline(&resource_layout.opt_compression_json(), &compression)?;

    // Restool consumes resource roots in JSON property order. Hvigor's
    // RestoolConfigBuilder declares applicationResource before moduleResources
    // and dependencies, so duplicate resources are owned by AppScope first.
    // Keep this insertion order identical to
    // hvigor-ohos-plugin/src/builder/restool-file-config-builder.js.
    let mut res_config = serde_json::Map::new();
    res_config.insert(
      "configPath".to_string(),
      serde_json::Value::String(IntermediateArtifactWriter::path_string(
        &module_layout.process_profile_module_json(),
      )),
    );
    res_config.insert(
      "packageName".to_string(),
      serde_json::Value::String(config.product.bundle_name.clone()),
    );
    res_config.insert(
      "output".to_string(),
      serde_json::Value::String(IntermediateArtifactWriter::path_string(&res_out)),
    );
    res_config.insert(
      "moduleNames".to_string(),
      serde_json::Value::String(module_names),
    );
    let resource_tables = self
      .resource_table_outputs(&module_layout, &module.module_type, &config.output_target)
      .iter()
      .map(|path| {
        serde_json::Value::String(IntermediateArtifactWriter::path_string(path.as_path()))
      })
      .collect();
    res_config.insert(
      "ResourceTable".to_string(),
      serde_json::Value::Array(resource_tables),
    );
    if include_app_resource {
      res_config.insert(
        "applicationResource".to_string(),
        serde_json::Value::String(IntermediateArtifactWriter::path_string(
          &project_layout.app_scope_resources(),
        )),
      );
    }
    res_config.insert(
      "moduleResources".to_string(),
      serde_json::Value::Array(
        module_res_dirs
          .iter()
          .map(|dir| {
            serde_json::Value::String(IntermediateArtifactWriter::path_string(
              &canonical_or_owned(dir),
            ))
          })
          .collect(),
      ),
    );
    res_config.insert(
      "dependencies".to_string(),
      serde_json::Value::Array(
        dependencies
          .into_iter()
          .map(serde_json::Value::String)
          .collect(),
      ),
    );
    res_config.insert(
      "iconCheck".to_string(),
      serde_json::Value::Bool(
        config.sdk_config.compile_api_version >= 10
          && matches!(
            module.module_type,
            OhosModuleType::Entry | OhosModuleType::Feature
          ),
      ),
    );
    res_config.insert(
      "compression".to_string(),
      serde_json::Value::String(IntermediateArtifactWriter::path_string(
        &resource_layout.opt_compression_json(),
      )),
    );
    let defined_sys_ids = self.restool_layout.defined_sys_ids();
    if let Some(options) = config.product.build_option.res_options.as_ref() {
      if let Some(threads) = options.res_compile_threads.filter(|threads| *threads > 0) {
        let max_threads = std::thread::available_parallelism()
          .map(|parallelism| parallelism.get())
          .unwrap_or(threads);
        res_config.insert(
          "thread".to_string(),
          serde_json::json!(threads.min(max_threads)),
        );
      }
      if !options.ignore_resource_pattern.is_empty() {
        let patterns = options
          .ignore_resource_pattern
          .iter()
          .map(|pattern| self.resource_ignore_regex(pattern))
          .collect::<Result<Vec<_>>>()?;
        res_config.insert(
          "ignoreResourcePattern".to_string(),
          serde_json::json!(patterns),
        );
      }
      if !matches!(config.output_target, OutputTarget::Har) {
        if let Some(qualifiers) = options.qualifiers_config.as_ref() {
          let mut qualifiers = qualifiers.clone();
          if let Some(object) = qualifiers.as_object_mut() {
            if let Some(mcc_mnc) = object.remove("Mcc&Mnc") {
              object.insert("MccMnc".to_string(), mcc_mnc);
            }
          }
          res_config.insert("qualifiersConfig".to_string(), qualifiers);
        }
      }
    }
    res_config.insert(
      "ids".to_string(),
      serde_json::Value::String(IntermediateArtifactWriter::path_string(
        &resource_layout.ids_map(),
      )),
    );
    res_config.insert(
      "definedIds".to_string(),
      serde_json::Value::String(IntermediateArtifactWriter::path_string(
        &resource_layout.id_defined_json(),
      )),
    );
    if let Some(defined_sys_ids) = defined_sys_ids {
      res_config.insert(
        "definedSysIds".to_string(),
        serde_json::Value::String(IntermediateArtifactWriter::path_string(&defined_sys_ids)),
      );
    }
    self.write_json_minified_newline(
      &resource_layout.res_config_json(),
      &serde_json::Value::Object(res_config),
    )?;

    Ok(())
  }

  pub(super) fn resource_table_outputs(
    &self,
    module_layout: &ModuleBuildLayout<'_>,
    module_type: &OhosModuleType,
    output_target: &OutputTarget,
  ) -> Vec<PathBuf> {
    let mut outputs = vec![module_layout.generated_resource_table_header()];
    // Latest hvigor ProcessResource asks restool for the TypeScript resource
    // table for every non-HAR, non-preview module. The installed SDK version
    // only gates whether GenerateLoaderJson compiles that generated file.
    if !matches!(module_type, OhosModuleType::Har)
      && !matches!(output_target, OutputTarget::Preview)
    {
      outputs.push(module_layout.generated_resource_table_typescript());
    }
    outputs
  }

  fn opt_compression_config(
    &self,
    config: &ArkdownConfig,
    extension_path: &str,
    resource_dirs: &[PathBuf],
  ) -> Result<serde_json::Value> {
    let mut output = serde_json::json!({
      "context": { "extensionPath": extension_path },
      "compression": {
        "media": { "enable": false },
        "filters": [],
      },
    });
    let Some(input) = config
      .product
      .build_option
      .res_options
      .as_ref()
      .and_then(|options| options.compression.as_ref())
    else {
      return Ok(output);
    };
    let Some(input_object) = input.as_object() else {
      anyhow::bail!("resOptions.compression must be an object");
    };
    if let Some(media) = input_object.get("media") {
      output["compression"]["media"] = media.clone();
    }
    if input_object
      .get("sizeLimit")
      .and_then(|value| value.get("ratio"))
      .is_some()
    {
      output["compression"]["sizeLimit"] = input_object["sizeLimit"].clone();
    }
    let mut resource_files = Vec::new();
    for resource_dir in resource_dirs.iter().filter(|path| path.is_dir()) {
      for entry in walkdir::WalkDir::new(resource_dir)
        .follow_links(false)
        .min_depth(1)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
      {
        resource_files.push(IntermediateArtifactWriter::path_string(entry.path()));
      }
    }
    resource_files.sort();
    resource_files.dedup();
    let filters = input_object
      .get("filters")
      .and_then(|value| value.as_array())
      .map(|filters| {
        filters
          .iter()
          .map(|filter| self.compression_filter(filter, &resource_files))
          .collect::<Result<Vec<_>>>()
      })
      .transpose()?
      .unwrap_or_default();
    output["compression"]["filters"] = serde_json::Value::Array(filters);
    let media_enabled = output["compression"]["media"]
      .get("enable")
      .and_then(serde_json::Value::as_bool)
      .unwrap_or(false);
    if media_enabled
      && output["compression"]["filters"]
        .as_array()
        .is_some_and(Vec::is_empty)
    {
      output["compression"]["filters"] = serde_json::json!([{
        "method": { "type": "sut", "blocks": "4x4" }
      }]);
    }
    Ok(output)
  }

  fn compression_filter(
    &self,
    filter: &serde_json::Value,
    resource_files: &[String],
  ) -> Result<serde_json::Value> {
    let object = filter
      .as_object()
      .context("resOptions.compression.filters entries must be objects")?;
    let mut output = serde_json::Map::new();
    output.insert(
      "method".to_string(),
      object
        .get("method")
        .cloned()
        .context("resource compression filter is missing method")?,
    );
    self.insert_compression_side(
      &mut output,
      "files",
      "path",
      "rules_origin",
      object,
      resource_files,
    )?;
    self.insert_compression_side(
      &mut output,
      "exclude",
      "exclude_path",
      "rules_exclude",
      object,
      resource_files,
    )?;
    if object
      .get("exclude")
      .and_then(serde_json::Value::as_object)
      .map(|exclude| {
        exclude.contains_key("path")
          || exclude.contains_key("size")
          || exclude.contains_key("resolution")
      })
      != Some(true)
    {
      output.insert("exclude_path".to_string(), serde_json::json!([]));
    }
    Ok(serde_json::Value::Object(output))
  }

  fn insert_compression_side(
    &self,
    output: &mut serde_json::Map<String, serde_json::Value>,
    input_key: &str,
    path_key: &str,
    rules_key: &str,
    filter: &serde_json::Map<String, serde_json::Value>,
    resource_files: &[String],
  ) -> Result<()> {
    let Some(side) = filter.get(input_key).and_then(serde_json::Value::as_object) else {
      return Ok(());
    };
    if let Some(patterns) = side.get("path").and_then(serde_json::Value::as_array) {
      let patterns = patterns
        .iter()
        .map(|pattern| {
          pattern
            .as_str()
            .map(|pattern| ArtifactPathSerializer::rel_to_slash(Path::new(pattern)))
            .context("resource compression paths must be strings")
        })
        .collect::<Result<Vec<_>>>()?;
      let matches = resource_files
        .iter()
        .filter(|path| {
          patterns
            .iter()
            .any(|pattern| fast_glob::glob_match(pattern, *path))
        })
        .cloned()
        .collect::<Vec<_>>();
      output.insert(path_key.to_string(), serde_json::json!(matches));
    }
    let mut rules = serde_json::Map::new();
    if let Some(size) = side.get("size") {
      rules.insert("size".to_string(), self.compression_size_ranges(size)?);
    }
    if let Some(resolution) = side.get("resolution") {
      rules.insert(
        "resolution".to_string(),
        self.valid_compression_resolution_ranges(resolution)?,
      );
    }
    if !rules.is_empty() {
      output.insert(rules_key.to_string(), serde_json::Value::Object(rules));
    }
    Ok(())
  }

  fn compression_size_ranges(&self, value: &serde_json::Value) -> Result<serde_json::Value> {
    let ranges = value
      .as_array()
      .context("resource compression size must be an array")?;
    let mut parsed = Vec::new();
    for range in ranges {
      let values = range
        .as_array()
        .context("resource compression size range must be an array")?;
      if values.is_empty() || values.len() > 2 {
        anyhow::bail!("resource compression size ranges must contain one or two values");
      }
      let start = self.compression_size_value(&values[0])?;
      let end = values
        .get(1)
        .map(|value| self.compression_size_value(value))
        .transpose()?;
      if end.is_some_and(|end| start > end) {
        continue;
      }
      parsed.push((start, end));
    }
    parsed.sort_by_key(|range| range.0);
    let mut merged: Vec<(u64, Option<u64>)> = Vec::new();
    for (start, end) in parsed {
      let Some(previous) = merged.last_mut() else {
        merged.push((start, end));
        continue;
      };
      match previous.1 {
        None => continue,
        Some(previous_end) if previous_end.saturating_add(1) >= start => {
          previous.1 = match (previous.1, end) {
            (_, None) => None,
            (Some(left), Some(right)) => Some(left.max(right)),
            (None, _) => None,
          };
        }
        _ => merged.push((start, end)),
      }
    }
    Ok(serde_json::Value::Array(
      merged
        .into_iter()
        .map(|(start, end)| match end {
          Some(end) => serde_json::json!([start, end]),
          None => serde_json::json!([start]),
        })
        .collect(),
    ))
  }

  fn compression_size_value(&self, value: &serde_json::Value) -> Result<u64> {
    if let Some(number) = value.as_u64() {
      return Ok(number);
    }
    let text = value
      .as_str()
      .context("resource compression size must be a positive integer or unit string")?;
    let (number, multiplier) = match text.as_bytes().last().copied() {
      Some(b'k' | b'K') => (&text[..text.len() - 1], 1024u64),
      Some(b'm' | b'M') => (&text[..text.len() - 1], 1024u64.pow(2)),
      Some(b'g' | b'G') => (&text[..text.len() - 1], 1024u64.pow(3)),
      Some(b't' | b'T') => (&text[..text.len() - 1], 1024u64.pow(4)),
      _ => (text, 1),
    };
    number
      .parse::<u64>()
      .map(|number| number.saturating_mul(multiplier))
      .with_context(|| format!("invalid resource compression size '{text}'"))
  }

  fn valid_compression_resolution_ranges(
    &self,
    value: &serde_json::Value,
  ) -> Result<serde_json::Value> {
    let ranges = value
      .as_array()
      .context("resource compression resolution must be an array")?;
    let valid = ranges
      .iter()
      .filter(|range| {
        let Some(values) = range.as_array() else {
          return false;
        };
        let Some(minimum) = values.first() else {
          return false;
        };
        let Some(maximum) = values.get(1) else {
          return true;
        };
        let min_width = minimum.get("width").and_then(serde_json::Value::as_u64);
        let min_height = minimum.get("height").and_then(serde_json::Value::as_u64);
        let max_width = maximum.get("width").and_then(serde_json::Value::as_u64);
        let max_height = maximum.get("height").and_then(serde_json::Value::as_u64);
        matches!(
          (min_width, min_height, max_width, max_height),
          (Some(min_width), Some(min_height), Some(max_width), Some(max_height))
            if min_width <= max_width && min_height <= max_height
        )
      })
      .cloned()
      .collect::<Vec<_>>();
    Ok(serde_json::Value::Array(valid))
  }

  fn resource_ignore_regex(&self, pattern: &str) -> Result<String> {
    let glob = globset::GlobBuilder::new(pattern)
      .literal_separator(false)
      .backslash_escape(false)
      .build()
      .with_context(|| format!("invalid ignoreResourcePattern '{pattern}'"))?;
    Ok(glob.regex().to_string())
  }
}

impl IntermediateArtifactWriter {
  pub(super) fn write_json_pretty(&self, path: &Path, value: &serde_json::Value) -> Result<()> {
    let content = serde_json::to_string_pretty(value)?;
    self.write_bytes(path, content.as_bytes())
  }

  fn write_profile_json(&self, path: &Path, value: &serde_json::Value) -> Result<()> {
    let mut content = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"\t");
    let mut serializer = serde_json::Serializer::with_formatter(&mut content, formatter);
    serde::Serialize::serialize(value, &mut serializer)?;
    content.push(b'\n');
    self.write_bytes(path, &content)
  }

  pub(super) fn rewrite_package_intermediate_module_json(&self, path: &Path) -> Result<()> {
    let content = fs_err::read_to_string(path)?;
    let content = minify_package_intermediate_module_json(&content)
      .map_err(|err| anyhow::anyhow!("rewrite package module.json failed: {}", err))?;
    fs_err::write(path, content.as_bytes())?;
    Ok(())
  }

  /// Execute Hvigor 6.24.3 `AbstractGenerateMetadata.doTaskAction` before
  /// packaging. The task predicts the output name from target configuration;
  /// it does not inspect a package that has already been produced.
  pub(super) fn write_package_metadata(&self, input: PackageMetadataInput<'_>) -> Result<()> {
    if matches!(input.module_type, OhosModuleType::Har) && input.target_name != "ohosTest" {
      return Ok(());
    }
    let module_layout = ModuleBuildLayout::for_output_target_product(
      input.output_target,
      input.module_root,
      input.product_name,
      input.target_name,
    );
    let metadata_path = if matches!(input.module_type, OhosModuleType::Shared) {
      module_layout.hsp_metadata_output_json()
    } else {
      module_layout.hap_metadata_output_json()
    };
    if let Some(parent) = metadata_path.parent() {
      fs_err::create_dir_all(parent)?;
    }
    let device_types = if input.device_types.is_empty() {
      vec!["phone".to_string()]
    } else {
      input.device_types.to_vec()
    };
    let remote_hsps =
      self.signed_remote_hsp_metadata(input.project_root, input.product_name, input.is_signed)?;
    let related_entries: &[RelatedEntryMetadata] = if matches!(
      input.module_type,
      OhosModuleType::Feature
    ) && !input.related_entries.is_empty()
    {
      &input.related_entries
    } else {
      &[]
    };
    let metadata_entries = if related_entries.is_empty() {
      vec![self.package_metadata_entry(&input, &device_types, None, remote_hsps.as_ref())]
    } else {
      related_entries
        .iter()
        .map(|related| {
          self.package_metadata_entry(&input, &device_types, Some(related), remote_hsps.as_ref())
        })
        .collect()
    };
    let metadata = serde_json::Value::Array(metadata_entries);
    let mut content = serde_json::to_string(&metadata)?;
    content.push('\n');
    fs_err::write(metadata_path, content.as_bytes())?;
    Ok(())
  }

  fn package_metadata_entry(
    &self,
    input: &PackageMetadataInput<'_>,
    device_types: &[String],
    related_entry: Option<&RelatedEntryMetadata>,
    remote_hsps: Option<&serde_json::Value>,
  ) -> serde_json::Value {
    let extension = if matches!(input.module_type, OhosModuleType::Shared) {
      "hsp"
    } else {
      "hap"
    };
    let default_base = related_entry.map_or_else(
      || format!("{}-{}", input.module_name, input.target_name),
      |related| {
        format!(
          "{}-{}-{}",
          input.module_name, related.module_name, input.target_name
        )
      },
    );
    let package_name = if let Some(artifact_name) = input.artifact_name {
      if input.is_signed {
        format!("{artifact_name}.{extension}")
      } else {
        format!("{artifact_name}-unsigned.{extension}")
      }
    } else if input.is_signed {
      format!("{default_base}-signed.{extension}")
    } else {
      format!("{default_base}-unsigned.{extension}")
    };

    let mut entry = serde_json::Map::new();
    entry.insert(
      if matches!(input.module_type, OhosModuleType::Shared) {
        "hspName"
      } else {
        "hapName"
      }
      .to_string(),
      package_name.into(),
    );
    entry.insert("deviceTypes".to_string(), serde_json::json!(device_types));
    entry.insert("isSigned".to_string(), input.is_signed.into());
    if let Some(related) = related_entry {
      let related_device_types = if related.device_types.is_empty() {
        vec!["phone".to_string()]
      } else {
        related.device_types.clone()
      };
      entry.insert(
        "entryModule".to_string(),
        serde_json::json!({
          "moduleName": related.module_name,
          "distroFilter": related.distro_filter,
          "deviceType": related_device_types,
        }),
      );
    }
    if let Some(remote_hsps) = remote_hsps {
      entry.insert("dependRemoteHsps".to_string(), remote_hsps.clone());
    }
    serde_json::Value::Object(entry)
  }

  fn signed_remote_hsp_metadata(
    &self,
    project_root: &Path,
    product_name: &str,
    is_signed: bool,
  ) -> Result<Option<serde_json::Value>> {
    if !is_signed {
      return Ok(None);
    }
    let cache_path = project_root
      .join("build")
      .join("cache")
      .join(product_name)
      .join("remote_hsp")
      .join("remoteHspCache.json");
    if !cache_path.is_file() {
      return Ok(Some(serde_json::Value::Array(Vec::new())));
    }
    let cache: serde_json::Value = serde_json::from_slice(&fs_err::read(&cache_path)?)?;
    let entries = cache
      .get("signedRemoteHsps")
      .and_then(serde_json::Value::as_object)
      .into_iter()
      .flat_map(|entries| entries.values())
      .map(|entry| {
        let signed_path = entry
          .get("signedRemoteHspPath")
          .or_else(|| entry.get("signedHspPath"))
          .and_then(serde_json::Value::as_str)
          .unwrap_or_default();
        let hsp_name = Path::new(signed_path)
          .file_name()
          .and_then(|name| name.to_str())
          .unwrap_or_default();
        serde_json::json!({
          "hspName": hsp_name,
          "hspVersion": entry.get("version").cloned().unwrap_or(serde_json::Value::Null),
          "hspPath": signed_path,
          "soPath": entry.get("soPath").cloned().unwrap_or_else(|| "".into()),
        })
      })
      .collect::<Vec<_>>();
    Ok(Some(serde_json::Value::Array(entries)))
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

  /// Publish the source map after packaging. Latest IDE owner:
  /// `AbstractPackageHar.moveSourceMap`; HAP/HSP packaging has the same output
  /// mapping handoff. The map path is the compiler output in debug and the
  /// `HarCompileArkTS/.../<mode>/sourceMaps.map` cache artifact in release.
  pub(super) fn publish_package_source_maps(
    &self,
    source_maps: Option<&Path>,
    output: &Path,
  ) -> Result<()> {
    if let Some(source_maps) = source_maps.filter(|path| path.is_file()) {
      self.copy_if_exists(source_maps, output)?;
    }
    Ok(())
  }

  /// Materialize the source-map handoffs consumed by CompileArkTS,
  /// `PackageHap`, and `CollectDebugSymbol`. Latest hvigor materializes the
  /// `intermediates/source_map` handoff for HAP/HSP and source-HAR targets,
  /// while ByteCode HAR's `ProcessHarArtifacts` bypasses that handoff. It
  /// leaves debug maps in `loader_out/.../ets`, but moves release maps to its
  /// `CompileArkTS/.../<mode>/sourceMaps.map` cache output.
  pub(super) fn stage_source_maps(
    &self,
    origin: &Path,
    compile_cache: &Path,
    release_output: &Path,
    package_handoff: SourceMapPackageHandoff<'_>,
    build_mode: &arkdown_core::BuildMode,
    source_map_input_fingerprints: &[SourceMapInputFingerprint],
  ) -> Result<PathBuf> {
    let package_intermediate = match package_handoff {
      SourceMapPackageHandoff::Materialize(path) => Some(path),
      SourceMapPackageHandoff::Suppress(stale_path) => {
        if stale_path.is_file() {
          fs_err::remove_file(stale_path).with_context(|| {
            format!(
              "failed to remove stale ByteCode HAR source-map intermediate {}",
              stale_path.display()
            )
          })?;
        }
        None
      }
    };
    if !origin.is_file() {
      return Ok(origin.to_path_buf());
    }
    let fingerprint_cache = SourceMapInputFingerprintCache::load(compile_cache);
    let regenerated_keys = fingerprint_cache.changed_keys(source_map_input_fingerprints);
    self.merge_incremental_source_maps(origin, compile_cache, &regenerated_keys)?;
    self.copy_if_exists(origin, compile_cache)?;
    if let Some(package_intermediate) = package_intermediate {
      self.copy_if_exists(origin, package_intermediate)?;
    }
    if matches!(build_mode, arkdown_core::BuildMode::Release) {
      self.copy_if_exists(origin, release_output)?;
      fs_err::remove_file(origin).with_context(|| {
        format!(
          "failed to remove release source map from loader output {}",
          origin.display()
        )
      })?;
      return Ok(package_intermediate.unwrap_or(release_output).to_path_buf());
    }
    Ok(package_intermediate.unwrap_or(origin).to_path_buf())
  }

  /// Commit source fingerprints only after CompileArkTS succeeds.
  ///
  /// The early source-map handoff consumes the previous successful snapshot;
  /// delaying this write ensures a failed ABC pass cannot mark an uncompiled
  /// source map as reusable on the next build.
  pub(super) fn commit_source_map_input_fingerprints(
    &self,
    compile_cache: &Path,
    source_map_input_fingerprints: &[SourceMapInputFingerprint],
  ) -> Result<()> {
    let cache_path = SourceMapInputFingerprintCache::path(compile_cache);
    if source_map_input_fingerprints.is_empty() {
      if cache_path.is_file() {
        fs_err::remove_file(cache_path)?;
      }
      return Ok(());
    }
    let mut fingerprint_cache = SourceMapInputFingerprintCache::default();
    fingerprint_cache.replace(source_map_input_fingerprints);
    fingerprint_cache.save(self, compile_cache)
  }

  /// Match latest ets-loader `SourceMapGenerator` incremental serialization.
  ///
  /// `writeModifiedSourceMapToFile()` first inserts source maps generated in
  /// the current Rollup pass, then
  /// `writeUsedAndUnmodifiedSourceMapToFile()` appends valid cached entries in
  /// their previous insertion order. A process-local Rolldown scan collects
  /// maps for every reachable module, so restore the SDK-visible distinction
  /// at the cache handoff: entries with an already-inventoried changed source,
  /// a changed structured map, or a new key are current-pass entries; equal
  /// entries retain the cached value and order. Deleted entries are omitted
  /// because they are not present in the current compile-file set.
  ///
  /// Installed SDK 6.1.1 ets-loader source:
  /// `fast_build/ark_compiler/generate_sourcemap.js::
  /// SourceMapGenerator.writeModifiedSourceMapToFile` and
  /// `writeUsedAndUnmodifiedSourceMapToFile`.
  fn merge_incremental_source_maps(
    &self,
    origin: &Path,
    compile_cache: &Path,
    regenerated_keys: &BTreeSet<String>,
  ) -> Result<()> {
    if origin == compile_cache || !compile_cache.is_file() {
      return Ok(());
    }

    let current = self.read_source_map_object(origin)?;
    let Some(cached) = self.read_cached_source_map_object(compile_cache)? else {
      return Ok(());
    };
    let merged = self.merge_source_map_objects(&current, &cached, regenerated_keys);
    if !self.source_map_object_order_matches(&merged, &current) {
      self.write_json_pretty(origin, &serde_json::Value::Object(merged))?;
    }
    Ok(())
  }

  fn source_map_object_order_matches(
    &self,
    left: &serde_json::Map<String, serde_json::Value>,
    right: &serde_json::Map<String, serde_json::Value>,
  ) -> bool {
    left.len() == right.len()
      && left
        .iter()
        .zip(right)
        .all(|((left_key, left_value), (right_key, right_value))| {
          left_key == right_key && left_value == right_value
        })
  }

  fn read_source_map_object(
    &self,
    path: &Path,
  ) -> Result<serde_json::Map<String, serde_json::Value>> {
    let source = fs_err::read_to_string(path)
      .with_context(|| format!("failed to read source map {}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&source)
      .with_context(|| format!("failed to parse source map {}", path.display()))?;
    value
      .as_object()
      .cloned()
      .with_context(|| format!("source map {} must be a JSON object", path.display()))
  }

  fn read_cached_source_map_object(
    &self,
    path: &Path,
  ) -> Result<Option<serde_json::Map<String, serde_json::Value>>> {
    let source = fs_err::read_to_string(path)
      .with_context(|| format!("failed to read cached source map {}", path.display()))?;
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&source) else {
      // ets-loader catches a stale/corrupt cache parse and continues with the
      // new maps rather than failing CompileArkTS.
      return Ok(None);
    };
    Ok(value.as_object().cloned())
  }

  fn merge_source_map_objects(
    &self,
    current: &serde_json::Map<String, serde_json::Value>,
    cached: &serde_json::Map<String, serde_json::Value>,
    regenerated_keys: &BTreeSet<String>,
  ) -> serde_json::Map<String, serde_json::Value> {
    let mut merged = serde_json::Map::new();

    // The SDK's current-pass `sourceMaps` object is inserted before its cache.
    // Source content fingerprints distinguish a fresh map whose JSON happens
    // to be byte-identical from an unchanged cached map. A changed/new map is
    // also a current-pass entry if no input fingerprint was available.
    for (key, current_value) in current {
      if regenerated_keys.contains(key) || cached.get(key) != Some(current_value) {
        merged.insert(key.clone(), current_value.clone());
      }
    }

    // Preserve the prior cache insertion order for source maps that remain
    // valid and were not regenerated in this compile pass.
    for (key, cached_value) in cached {
      if current.contains_key(key) && !merged.contains_key(key) {
        merged.insert(key.clone(), cached_value.clone());
      }
    }

    // Be conservative for an entry absent from a malformed/partial cache:
    // it is a current map and must not disappear from the compiler handoff.
    for (key, current_value) in current {
      merged
        .entry(key.clone())
        .or_insert_with(|| current_value.clone());
    }
    merged
  }

  fn write_bytes(&self, path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
      fs_err::create_dir_all(parent)?;
    }
    fs_err::write(path, bytes)?;
    Ok(())
  }

  pub(super) fn write_json_minified(&self, path: &Path, value: &serde_json::Value) -> Result<()> {
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
