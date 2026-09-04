//! Compile-plugin share snapshot construction and cache invalidation.

use super::*;
use arkdown::plugins::hvigor_compile::{HvigorWatchChange, HvigorWatchChangeEvent};
use arkdown_watcher::WatchEventKind;

pub(crate) struct HvigorCompilePluginShareBuilder<'a> {
  pub(crate) config: &'a ArkdownConfig,
  pub(crate) args: &'a BuildRequest,
  pub(crate) module: &'a ModuleConfig,
  pub(crate) session: &'a BuildSession,
  pub(crate) module_context: &'a ModuleCompileContext,
  pub(crate) package_resolution: &'a PackageResolutionPlan,
  pub(crate) loader_json: &'a LoaderJson,
  pub(crate) loader_json_path: &'a Path,
  pub(crate) target_name: &'a str,
  pub(crate) coverage_enabled: bool,
  pub(crate) arkts_language_version: ArktsLanguageVersion,
  pub(crate) plugin_cache_snapshot: Option<Arc<[u8]>>,
}

impl<'a> HvigorCompilePluginShareBuilder<'a> {
  pub(crate) fn new(
    config: &'a ArkdownConfig,
    args: &'a BuildRequest,
    module: &'a ModuleConfig,
    session: &'a BuildSession,
    module_context: &'a ModuleCompileContext,
    package_resolution: &'a PackageResolutionPlan,
    loader_json: &'a LoaderJson,
    loader_json_path: &'a Path,
    target_name: &'a str,
    coverage_enabled: bool,
    arkts_language_version: ArktsLanguageVersion,
  ) -> Self {
    Self {
      config,
      args,
      module,
      session,
      module_context,
      package_resolution,
      loader_json,
      loader_json_path,
      target_name,
      coverage_enabled,
      arkts_language_version,
      plugin_cache_snapshot: None,
    }
  }

  pub(crate) fn with_plugin_cache_snapshot(mut self, snapshot: Option<Vec<u8>>) -> Self {
    self.plugin_cache_snapshot = snapshot.map(Arc::from);
    self
  }

  fn project_config(&self) -> Result<serde_json::Map<String, serde_json::Value>> {
    let watch_mode = self.args.watch || self.args.watch_invalidation.is_some();
    let module_layout = ModuleBuildLayout::for_output_target_product(
      &self.config.output_target,
      &self.module_context.module.root,
      &self.config.product.name,
      self.target_name,
    );
    let generated_profile_path = module_layout
      .build_profile_for_module(&self.module.module_type, self.target_name)
      .parent()
      .map(Path::to_path_buf)
      .ok_or_else(|| {
        anyhow::anyhow!(
          "ARKDOWN_PLUGIN_PROJECT_CONFIG_PROFILE_PATH: cannot derive generated profile directory for module '{}'",
          self.module.name
        )
      })?;
    let ace_module_root = if self.target_name == "ohosTest" {
      let source = module_layout.source();
      let target_ets = source.target_ets(self.target_name);
      if self
        .module_context
        .module
        .ohos_test_module_json_value
        .as_deref()
        .is_some_and(|module_json| source.ohos_test_templates_after_migration(module_json))
      {
        ModuleBuildLayout::for_ohos_test_template(
          &self.module_context.module.root,
          &self.config.product.name,
        )
        .ohos_test_ets_root()
      } else {
        target_ets
      }
    } else {
      module_layout.source().ets()
    };
    let ace_super_visual_path = if self.target_name == "ohosTest" {
      module_layout.source().ohos_test_dir().join("supervisual")
    } else {
      module_layout.source().src_main().join("supervisual")
    };
    let paths = SdkEtsCheckerPaths::build(
      module_layout.res_profile_dir(),
      module_layout.res_ark_module_json(),
      module_layout.resource_table_txt(),
      module_layout.res_resources_dir().join("rawfile"),
      ace_module_root.clone(),
      ace_super_visual_path,
      self.loader_json_path.to_path_buf(),
      PathBuf::from(&self.loader_json.cache_path),
      PathBuf::from(&self.loader_json.ace_module_build),
      generated_profile_path,
      module_layout.generated_resource_table_typescript(),
    );
    let entries = SdkEtsCheckerEntryCollector::new(self.loader_json, &ace_module_root).collect();
    let value = SdkEtsCheckerPlanBuilder::new(SdkEtsCheckerInput {
      config: self.config,
      session: self.session,
      module: self.module,
      module_context: self.module_context,
      package_resolution: self.package_resolution,
      module_build_option: &self.config.product.build_option,
      loader_json: Some(self.loader_json),
      target_name: self.target_name,
      node_executable: self.args.resolved_node_executable(),
      paths,
      entries,
      coverage_enabled: self.coverage_enabled,
      coverage_path_filter: self.args.coverage_path_filter.as_deref(),
      coverage_mode: self.args.coverage_mode.as_deref(),
      debug_line: false,
      preview_mode: self.args.preview,
      check_entry: self.args.check_entry,
      watch_mode,
      byte_code_har_integrated_optimization: self.args.byte_code_har_integrated_optimization,
      single_file_emit: self.config.build_mode.is_debuggable()
        && self.args.hvigor_properties.single_file_emit
        && !self.coverage_enabled,
      rollup_cache_use_source_hash: self.args.hvigor_properties.rollup_cache_use_source_hash,
    })
    .project_config_value()?;
    let effective = value
      .as_object()
      .cloned()
      .ok_or_else(|| anyhow::anyhow!("ArkTS project configuration must serialize as an object"))?;
    let mut project_config = serde_json::to_value(self.loader_json)?
      .as_object()
      .cloned()
      .ok_or_else(|| anyhow::anyhow!("loader.json must serialize as an object"))?;
    project_config.extend(effective);
    project_config.insert("perf".into(), self.args.analyze.perf_value().into());
    project_config.insert(
      "moduleRootPath".into(),
      self.loader_json.module_root_path.clone().into(),
    );
    project_config.insert(
      "buildPath".into(),
      self.loader_json.ace_module_build.clone().into(),
    );
    project_config.insert(
      "aceBuildJson".into(),
      ArtifactPathSerializer::path_to_slash_string(self.loader_json_path)
        .map(serde_json::Value::String)
        .map_err(|error| anyhow::anyhow!("ARKDOWN_PLUGIN_PROJECT_CONFIG_PATH: {error}"))?,
    );
    project_config.insert("target".into(), self.args.target.clone().into());
    project_config.insert(
      "buildArkMode".into(),
      self.config.build_mode.as_str().into(),
    );
    project_config.insert(
      "isByteCodeHarOptimize".into(),
      self.args.byte_code_har_integrated_optimization.into(),
    );
    project_config.insert(
      "uiTransformOptimization".into(),
      self.args.ui_transform_optimization.into(),
    );
    project_config.insert(
      "useGetCurrentBundleName".into(),
      self.args.integrated_hsp_use_get_current_bundle_name.into(),
    );
    project_config.insert(
      "noExternalImportByPath".into(),
      self
        .config
        .product
        .build_option
        .strict_mode
        .is_no_external_import_by_path()
        .into(),
    );
    project_config.insert(
      "widgetCompile".into(),
      serde_json::Value::String("false".to_string()),
    );
    project_config.insert(
      "isFirstBuild".into(),
      self.args.watch_invalidation.is_none().into(),
    );
    project_config.insert(
      "hotReload".into(),
      (watch_mode && !(self.args.preview || self.args.preview_transform_mode)).into(),
    );
    let ark_options = self.config.product.build_option.ark_options.as_ref();
    project_config.insert(
      "autoLazyFilter".into(),
      serde_json::to_value(
        ark_options
          .and_then(|options| options.auto_lazy_filter.as_ref())
          .cloned()
          .unwrap_or_default(),
      )?,
    );
    project_config.insert(
      "expandImportPath".into(),
      serde_json::to_value(
        ark_options
          .and_then(|options| options.expand_import_path.as_ref())
          .cloned()
          .unwrap_or_default(),
      )?,
    );
    Ok(project_config)
  }

  fn dependency_info(&self) -> HvigorCompilePluginDependencyInfo {
    let Some(batch) = self.args.watch_invalidation.as_ref() else {
      return HvigorCompilePluginDependencyInfo {
        enable_incremental: false,
        changed_dependencies: Arc::from([]),
      };
    };
    match self
      .module_context
      .deps
      .changed_package_names_for_paths(batch.raw_events.iter().map(|event| event.path.as_path()))
    {
      Some(changed_dependencies) if !changed_dependencies.is_empty() => {
        HvigorCompilePluginDependencyInfo {
          enable_incremental: true,
          changed_dependencies: Arc::from(changed_dependencies),
        }
      }
      _ => HvigorCompilePluginDependencyInfo {
        enable_incremental: false,
        changed_dependencies: Arc::from([]),
      },
    }
  }

  fn watch_changes(&self) -> Arc<[HvigorWatchChange]> {
    let Some(batch) = self.args.watch_invalidation.as_ref() else {
      return Arc::from([]);
    };
    batch
      .raw_events
      .iter()
      .map(|event| HvigorWatchChange {
        path: event.path.clone(),
        event: match event.kind {
          WatchEventKind::Created => HvigorWatchChangeEvent::Create,
          WatchEventKind::Modified => HvigorWatchChangeEvent::Update,
          WatchEventKind::Deleted => HvigorWatchChangeEvent::Delete,
        },
      })
      .collect::<Vec<_>>()
      .into()
  }

  /// Build the module-scoped `this.share` snapshot used by latest-hvigor
  /// compile plugins. Paths remain typed until the N-API display boundary.
  pub(crate) fn build(self) -> Result<HvigorCompilePluginShare> {
    let source_files = self
      .session
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
    for edge in &self.module_context.deps.edges {
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
    let mut project_config_fields = self.project_config()?;
    project_config_fields.insert(
      "arkTSVersion".into(),
      self.arkts_language_version.to_string().into(),
    );
    Ok(HvigorCompilePluginShare {
      build_instance_id: HvigorCompilePluginShare::next_build_instance_id(),
      project_config: Arc::new(project_config_fields),
      source_files: Arc::new(source_files),
      import_resolver_entries: Arc::from(import_resolver_entries),
      symlink_map: Arc::new(symlink_map),
      dependency_info: self.dependency_info(),
      watch_changes: self.watch_changes(),
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
