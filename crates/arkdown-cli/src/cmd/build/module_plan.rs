use rustc_hash::FxHashSet;

use arkdown_core::{ArkdownConfig, ModuleConfig, OutputTarget};

use super::canonical_or_owned;

pub(super) fn build_modules_for_target<'a>(
    config: &'a ArkdownConfig,
    selected_module: Option<&str>,
) -> Vec<&'a ModuleConfig> {
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
        config
            .modules
            .iter()
            .filter(|module| {
                selected_module
                    .map(|selected| module.name == selected)
                    .unwrap_or(true)
            })
            .collect()
    } else {
        matching
    }
}

pub(super) fn resource_modules_for_build<'a>(
    config: &'a ArkdownConfig,
    output_module: &'a ModuleConfig,
) -> Vec<&'a ModuleConfig> {
    match config.output_target {
        OutputTarget::Hap | OutputTarget::Hsp => {
            let mut modules = vec![output_module];
            let mut seen = FxHashSet::from_iter([output_module.name.clone()]);
            let mut queue = ordered_local_dependency_modules(config, output_module);
            for dep in &queue {
                if seen.insert(dep.name.clone()) {
                    modules.push(*dep);
                }
            }
            let mut index = 0usize;
            while index < queue.len() {
                let module = queue[index];
                index += 1;
                for dep in ordered_local_dependency_modules(config, module) {
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
            module_from_package_dependency(config, module, name, spec)
        })
        .collect()
}

fn module_from_package_dependency<'a>(
    config: &'a ArkdownConfig,
    module: &ModuleConfig,
    package_name: &str,
    spec: &str,
) -> Option<&'a ModuleConfig> {
    if let Some(path) = spec.strip_prefix("file:") {
        let target = canonical_or_owned(&module.src_path.join(path));
        if let Some(found) = config
            .modules
            .iter()
            .find(|candidate| canonical_or_owned(&candidate.src_path) == target)
        {
            return Some(found);
        }
    }
    let short_name = package_name.rsplit('/').next().unwrap_or(package_name);
    config
        .modules
        .iter()
        .find(|candidate| candidate.name == short_name)
}
