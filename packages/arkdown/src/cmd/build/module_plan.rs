use rustc_hash::FxHashSet;

use arkdown_core::{ArkdownConfig, ModuleConfig, OutputTarget};
use arkdown_path::{canonicalize_or_owned as canonical_or_owned, ModuleSpecifierText};

pub(super) struct ModuleSelectionPlanner;

impl ModuleSelectionPlanner {
  pub(super) fn build_modules_for_target<'a>(
    config: &'a ArkdownConfig,
    selected_module: Option<&str>,
  ) -> anyhow::Result<Vec<&'a ModuleConfig>> {
    if let Some(selected) = selected_module {
      if !config.modules.iter().any(|module| module.name == selected) {
        anyhow::bail!("selected module '{selected}' does not exist in build-profile.json5");
      }
    }
    let matching: Vec<&ModuleConfig> = config
      .modules
      .iter()
      .filter(|module| {
        selected_module
          .map(|selected| module.name == selected)
          .unwrap_or(true)
      })
      .filter(|module| {
        module
          .module_type
          .matches_output_target(&config.output_target)
      })
      .collect();
    if matching.is_empty() {
      let scope = selected_module
        .map(|name| format!("module '{name}'"))
        .unwrap_or_else(|| "configured modules".to_string());
      anyhow::bail!(
        "{scope} cannot produce the requested {:?} target; module type fallback is not permitted",
        config.output_target
      );
    }
    Ok(matching)
  }

  pub(super) fn resource_modules_for_build<'a>(
    config: &'a ArkdownConfig,
    output_module: &'a ModuleConfig,
  ) -> Vec<&'a ModuleConfig> {
    match config.output_target {
      OutputTarget::Hap | OutputTarget::Hsp => {
        let mut modules = vec![output_module];
        let mut seen = FxHashSet::from_iter([output_module.name.clone()]);
        let mut queue = Self::ordered_local_dependency_modules(config, output_module);
        for dep in &queue {
          if seen.insert(dep.name.clone()) {
            modules.push(*dep);
          }
        }
        let mut index = 0usize;
        while index < queue.len() {
          let module = queue[index];
          index += 1;
          for dep in Self::ordered_local_dependency_modules(config, module) {
            if seen.insert(dep.name.clone()) {
              modules.push(dep);
              queue.push(dep);
            }
          }
        }
        modules
      }
      _ => vec![output_module],
    }
  }

  fn ordered_local_dependency_modules<'a>(
    config: &'a ArkdownConfig,
    module: &'a ModuleConfig,
  ) -> Vec<&'a ModuleConfig> {
    let package = module.src_path.join("oh-package.json5");
    let Ok(content) = fs_err::read_to_string(&package) else {
      return vec![];
    };
    let Ok(value) = json5::from_str::<serde_json::Value>(&content) else {
      return vec![];
    };
    let Some(dependencies) = value
      .get("dependencies")
      .and_then(|value| value.as_object())
    else {
      return vec![];
    };
    dependencies
      .iter()
      .filter_map(|(name, spec)| {
        let spec = spec.as_str().unwrap_or_default();
        Self::module_from_package_dependency(config, module, name, spec)
      })
      .collect()
  }

  fn module_from_package_dependency<'a>(
    config: &'a ArkdownConfig,
    module: &ModuleConfig,
    package_name: &str,
    spec: &str,
  ) -> Option<&'a ModuleConfig> {
    if let Some(path) = PackageDependencySpec::new(spec).file_relative_path() {
      let target = canonical_or_owned(&module.src_path.join(path));
      if let Some(found) = config
        .modules
        .iter()
        .find(|candidate| canonical_or_owned(&candidate.src_path) == target)
      {
        return Some(found);
      }
    }
    let package_specifier = ModuleSpecifierText::new(package_name);
    config
      .modules
      .iter()
      .find(|candidate| package_specifier.matches_package_module_name(&candidate.name))
  }
}

struct PackageDependencySpec<'a> {
  raw: &'a str,
}

impl<'a> PackageDependencySpec<'a> {
  const FILE_PROTOCOL: &'static str = "file:";

  fn new(raw: &'a str) -> Self {
    Self { raw }
  }

  fn file_relative_path(&self) -> Option<&'a str> {
    self.raw.strip_prefix(Self::FILE_PROTOCOL)
  }
}
