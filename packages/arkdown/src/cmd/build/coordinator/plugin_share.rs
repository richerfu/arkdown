//! Compile-plugin share snapshot construction and cache invalidation.

use super::*;

pub(crate) struct HvigorCompilePluginShareBuilder<'a> {
  pub(crate) config: &'a ArkdownConfig,
  pub(crate) args: &'a BuildArgs,
  pub(crate) module: Option<&'a ModuleConfig>,
}

impl<'a> HvigorCompilePluginShareBuilder<'a> {
  pub(crate) fn new(
    config: &'a ArkdownConfig,
    args: &'a BuildArgs,
    module: Option<&'a ModuleConfig>,
  ) -> Self {
    Self {
      config,
      args,
      module,
    }
  }

  /// Build the module-scoped `this.share` snapshot used by latest-hvigor
  /// compile plugins. Paths remain typed until the N-API display boundary.
  pub(crate) fn build(self) -> HvigorCompilePluginShare {
    HvigorCompilePluginShare {
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
      },
    }
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
