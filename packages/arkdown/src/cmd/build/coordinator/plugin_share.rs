//! Compile-plugin share snapshot construction and cache invalidation.

use super::*;

pub(crate) struct HvigorCompilePluginShareBuilder<'a> {
  pub(crate) config: &'a ArkdownConfig,
  pub(crate) args: &'a BuildArgs,
  pub(crate) module: Option<&'a ModuleConfig>,
  pub(crate) source_store: &'a arkdown_session::SourceStore,
  pub(crate) package_resolution: &'a PackageResolutionPlan,
  pub(crate) dependency_graph: &'a DependencyGraphSnapshot,
  pub(crate) loader_json: &'a LoaderJson,
  pub(crate) loader_json_path: &'a Path,
  pub(crate) arkts_language_version: ArktsLanguageVersion,
  pub(crate) plugin_cache_snapshot: Option<Arc<[u8]>>,
}

impl<'a> HvigorCompilePluginShareBuilder<'a> {
  pub(crate) fn new(
    config: &'a ArkdownConfig,
    args: &'a BuildArgs,
    module: Option<&'a ModuleConfig>,
    source_store: &'a arkdown_session::SourceStore,
    package_resolution: &'a PackageResolutionPlan,
    dependency_graph: &'a DependencyGraphSnapshot,
    loader_json: &'a LoaderJson,
    loader_json_path: &'a Path,
    arkts_language_version: ArktsLanguageVersion,
  ) -> Self {
    Self {
      config,
      args,
      module,
      source_store,
      package_resolution,
      dependency_graph,
      loader_json,
      loader_json_path,
      arkts_language_version,
      plugin_cache_snapshot: None,
    }
  }

  pub(crate) fn with_plugin_cache_snapshot(mut self, snapshot: Option<Vec<u8>>) -> Self {
    self.plugin_cache_snapshot = snapshot.map(Arc::from);
    self
  }

  /// Build the module-scoped `this.share` snapshot used by latest-hvigor
  /// compile plugins. Paths remain typed until the N-API display boundary.
  pub(crate) fn build(self) -> Result<HvigorCompilePluginShare> {
    let source_files = self
      .source_store
      .source_files()
      .into_iter()
      .map(|source| {
        (
          source.path.clone(),
          HvigorCompilePluginSourceFile {
            content_hash: source.fingerprint.content_hash.clone(),
            size: source.fingerprint.size,
            modified_millis: source.modified_millis,
          },
        )
      })
      .collect::<FxHashMap<_, _>>();
    let import_resolver_entries = self
      .package_resolution
      .sorted_entries()
      .into_iter()
      .map(|(alias, info)| HvigorCompilePluginImportEntry {
        alias: alias.to_string(),
        package_name: info.pkg_name.clone(),
        package_root: info.pkg_root.clone(),
        entry_path: info
          .manifest
          .exports_import_field()
          .or_else(|| info.manifest.hvigor_main_field().map(str::to_string))
          .map(PathBuf::from),
      })
      .collect::<Vec<_>>();
    let mut symlink_map = BTreeMap::<PathBuf, Vec<PathBuf>>::new();
    for edge in &self.dependency_graph.edges {
      let Some(alias_path) = edge.installed_alias_path.as_ref() else {
        continue;
      };
      symlink_map
        .entry(edge.package_root.clone())
        .or_default()
        .push(alias_path.clone());
    }
    for paths in symlink_map.values_mut() {
      paths.sort();
      paths.dedup();
    }
    let mut project_config_fields = serde_json::to_value(self.loader_json)?
      .as_object()
      .cloned()
      .ok_or_else(|| anyhow::anyhow!("loader.json must serialize as an object"))?;
    let module_path = self.module.map(|module| module.src_path.clone());
    let module_type = self.module.map(|module| &module.module_type);
    let path_value = |path: &Path| serde_json::Value::String(path.to_string_lossy().into_owned());
    project_config_fields.insert(
      "compileMode".into(),
      self.loader_json.compile_mode.clone().into(),
    );
    project_config_fields.insert(
      "projectRootPath".into(),
      path_value(&self.config.project_root),
    );
    project_config_fields.insert(
      "projectTopDir".into(),
      path_value(&self.config.project_root),
    );
    project_config_fields.insert(
      "projectPath".into(),
      module_path
        .as_deref()
        .map(path_value)
        .unwrap_or(serde_json::Value::Null),
    );
    project_config_fields.insert(
      "moduleRootPath".into(),
      self.loader_json.module_root_path.clone().into(),
    );
    project_config_fields.insert(
      "cachePath".into(),
      self.loader_json.cache_path.clone().into(),
    );
    project_config_fields.insert(
      "aceModuleBuild".into(),
      self.loader_json.ace_module_build.clone().into(),
    );
    project_config_fields.insert(
      "buildPath".into(),
      self.loader_json.ace_module_build.clone().into(),
    );
    project_config_fields.insert("aceBuildJson".into(), path_value(self.loader_json_path));
    project_config_fields.insert(
      "aceModuleJsonPath".into(),
      module_path
        .as_deref()
        .map(|path| path_value(&path.join("src/main/module.json5")))
        .unwrap_or(serde_json::Value::Null),
    );
    project_config_fields.insert(
      "runtimeOS".into(),
      self.config.product.runtime_os.clone().into(),
    );
    project_config_fields.insert("buildMode".into(), self.config.build_mode.as_str().into());
    project_config_fields.insert(
      "buildArkMode".into(),
      self.config.build_mode.as_str().into(),
    );
    project_config_fields.insert("isPreview".into(), self.args.preview.into());
    project_config_fields.insert(
      "compileHar".into(),
      matches!(self.config.output_target, OutputTarget::Har).into(),
    );
    project_config_fields.insert(
      "compileShared".into(),
      module_type
        .is_some_and(|module_type| matches!(module_type, OhosModuleType::Shared))
        .into(),
    );
    project_config_fields.insert(
      "byteCodeHar".into(),
      (matches!(self.config.output_target, OutputTarget::Har)
        && self.config.product.build_option.har_artifact_type == HarArtifactType::Bytecode)
        .into(),
    );
    project_config_fields.insert(
      "targetName".into(),
      self
        .args
        .build_target_name
        .clone()
        .unwrap_or_else(|| "default".into())
        .into(),
    );
    project_config_fields.insert("watchMode".into(), self.args.watch.to_string().into());
    project_config_fields.insert(
      "arkTSVersion".into(),
      self.arkts_language_version.to_string().into(),
    );
    project_config_fields.insert(
      "isOhosTest".into(),
      (self.args.target == "test" || self.args.build_target_name.as_deref() == Some("ohosTest"))
        .into(),
    );
    Ok(HvigorCompilePluginShare {
      build_instance_id: HvigorCompilePluginShare::next_build_instance_id(),
      project_config: HvigorCompilePluginProjectConfig {
        bundle_name: self.config.product.bundle_name.clone(),
        build_mode: self.config.build_mode.as_str().to_string(),
        target: self.args.target.as_str().to_string(),
        use_normalized_ohm_url: self
          .config
          .product
          .build_option
          .strict_mode
          .use_normalized_ohm_url,
        compile_sdk_version: self.config.sdk_config.compile_sdk_version.clone(),
        compatible_sdk_version: self.config.product.compatible_sdk_version.clone(),
        package_manager_type: "ohpm".to_string(),
        is_byte_code_har_optimize: self.args.byte_code_har_integrated_optimization,
        module_name: self.module.map(|module| module.name.clone()),
        module_path: self.module.map(|module| module.src_path.clone()),
        project_top_dir: self.args.project.clone(),
        watch_mode: self.args.watch,
      },
      project_config_fields: Arc::new(project_config_fields),
      source_files: Arc::new(source_files),
      import_resolver_entries: Arc::from(import_resolver_entries),
      symlink_map: Arc::new(symlink_map),
      plugin_cache_snapshot: self.plugin_cache_snapshot,
    })
  }
}

pub(crate) struct PluginContextCacheRunner<'a> {
  plugins: &'a [PackagePlugin],
  store: arkdown_cache::PluginContextCacheStore,
}

impl<'a> PluginContextCacheRunner<'a> {
  pub(crate) fn new(plugins: &'a [PackagePlugin], cache_dir: &'a Path) -> Self {
    Self {
      plugins,
      store: arkdown_cache::PluginContextCacheStore::new(cache_dir),
    }
  }

  pub(crate) fn load(&self) -> Result<Option<Vec<u8>>> {
    self.store.load().map_err(Into::into)
  }

  pub(crate) async fn commit(&self) -> Result<()> {
    for plugin in self.plugins {
      let Some(snapshot) = plugin.snapshot_hvigor_context_cache().await? else {
        continue;
      };
      self.store.commit(&snapshot)?;
      break;
    }
    Ok(())
  }
}

pub(crate) struct PluginCacheInvalidationRunner<'a> {
  pub(crate) plugins: &'a [PackagePlugin],
  pub(crate) cache_dir: &'a Path,
}

impl<'a> PluginCacheInvalidationRunner<'a> {
  pub(crate) fn new(plugins: &'a [PackagePlugin], cache_dir: &'a Path) -> Self {
    Self { plugins, cache_dir }
  }

  pub(crate) async fn should_invalidate(&self) -> Result<bool> {
    let cache_plugins = self
      .plugins
      .iter()
      .filter(|plugin| plugin.has_should_invalid_cache())
      .cloned()
      .collect::<Vec<_>>();
    if cache_plugins.is_empty() {
      return Ok(false);
    }

    let inventory = match PluginCacheModuleInventory::load(self.cache_dir) {
      Ok(Some(inventory)) if !inventory.is_empty() => inventory,
      Ok(_) => {
        tracing::info!(
          cache_dir = %self.cache_dir.display(),
          "plugin module cache is absent; rebuilding before shouldInvalidCache can run"
        );
        return Ok(true);
      }
      Err(error) => {
        tracing::warn!(
          cache_dir = %self.cache_dir.display(),
          error = %error,
          "plugin module cache is unreadable; rebuilding"
        );
        return Ok(true);
      }
    };

    for record in inventory.modules() {
      let module = match inventory.read_module(record, self.cache_dir) {
        Ok(module) => Arc::new(module),
        Err(error) => {
          tracing::warn!(
            cache_dir = %self.cache_dir.display(),
            error = %error,
            "cached transformed module is unreadable; rebuilding"
          );
          return Ok(true);
        }
      };
      // Latest invalid-cache-plugin.js evaluates every custom plugin for the
      // current cached module with Promise.all. JoinSet preserves that parallel
      // boundary while keeping modules themselves in deterministic ID order.
      let mut tasks = tokio::task::JoinSet::new();
      for plugin in cache_plugins.iter().cloned() {
        let module = Arc::clone(&module);
        tasks.spawn(async move { plugin.should_invalid_cache(module.as_ref()).await });
      }
      let mut should_invalidate = false;
      while let Some(result) = tasks.join_next().await {
        should_invalidate |=
          result.map_err(|error| anyhow::anyhow!("shouldInvalidCache task failed: {error}"))??;
      }
      if should_invalidate {
        return Ok(true);
      }
    }
    Ok(false)
  }
}
