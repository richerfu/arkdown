use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use arkdown_core::{ArkdownConfig, OhosModuleType, OutputTarget};
use arkdown_path::canonicalize_or_owned;
use arkdown_session::dependency_graph::snapshot::{
  DependencyGraphSnapshot, DependencyNodeSnapshot, DependencyNodeType,
};
use arkdown_session::ProjectSnapshot;

/// Project-level result of hvigor's
/// `collect_duplicate_har_dependencies.collectDuplicateHarInfos`.
pub(super) struct HarDedupProjectPlan {
  modules: BTreeMap<String, HarDedupModulePlan>,
}

#[derive(Clone, Default)]
pub(super) struct HarDedupModulePlan {
  duplicate_package_roots: Vec<PathBuf>,
  source_har_roots: Vec<PathBuf>,
  bytecode_har_packages: BTreeSet<String>,
}

impl HarDedupModulePlan {
  pub(super) fn is_empty(&self) -> bool {
    self.duplicate_package_roots.is_empty()
  }

  pub(super) fn source_har_roots(&self) -> &[PathBuf] {
    &self.source_har_roots
  }

  pub(super) fn duplicate_package_roots(&self) -> &[PathBuf] {
    &self.duplicate_package_roots
  }

  pub(super) fn contains_bytecode_package(&self, package_name: &str) -> bool {
    self.bytecode_har_packages.contains(package_name)
  }

  pub(super) fn bytecode_har_packages(&self) -> &BTreeSet<String> {
    &self.bytecode_har_packages
  }

  pub(super) fn collect_bytecode_module_ids<'a>(
    &self,
    external_module_ids: impl IntoIterator<Item = &'a String>,
  ) -> Vec<String> {
    let mut collected = external_module_ids
      .into_iter()
      .filter(|module_id| {
        let module_id = module_id.to_lowercase();
        self.bytecode_har_packages.iter().any(|package_name| {
          let package_name = package_name.to_lowercase();
          module_id == package_name
            || module_id
              .strip_prefix(&package_name)
              .is_some_and(|suffix| suffix.starts_with('/') || suffix.starts_with('\\'))
        })
      })
      .cloned()
      .collect::<Vec<_>>();
    collected.sort();
    collected.dedup();
    collected
  }

  pub(super) fn write_bytecode_module_ids(
    &self,
    path: &Path,
    module_ids: &[String],
  ) -> anyhow::Result<()> {
    if module_ids.is_empty() {
      if path.exists() {
        fs_err::remove_file(path)?;
      }
      return Ok(());
    }
    if let Some(parent) = path.parent() {
      fs_err::create_dir_all(parent)?;
    }
    fs_err::write(path, module_ids.join("\n"))?;
    Ok(())
  }
}

impl HarDedupProjectPlan {
  pub(super) fn collect(config: &ArkdownConfig, project: &ProjectSnapshot) -> anyhow::Result<Self> {
    if !config.product.build_option.deduplicate_har
      || !matches!(config.output_target, OutputTarget::Hsp)
    {
      return Ok(Self {
        modules: BTreeMap::new(),
      });
    }

    let hap_modules = config
      .modules
      .iter()
      .filter(|module| matches!(module.module_type, OhosModuleType::Entry))
      .collect::<Vec<_>>();
    if hap_modules.len() != 1 {
      anyhow::bail!(
        "HAR deduplication supports exactly one entry HAP in product '{}', found {}: {}",
        config.product.name,
        hap_modules.len(),
        hap_modules
          .iter()
          .map(|module| module.name.as_str())
          .collect::<Vec<_>>()
          .join(",")
      );
    }

    let hap = hap_modules[0];
    let hap_graph = project
      .dependency_graph(&hap.name)
      .ok_or_else(|| anyhow::anyhow!("missing dependency graph for entry HAP '{}'", hap.name))?;
    let hap_inventory = HarDependencyInventory::new(hap_graph.as_ref());

    let local_hsp_roots = config
      .modules
      .iter()
      .filter(|module| matches!(module.module_type, OhosModuleType::Shared))
      .map(|module| (canonicalize_or_owned(&module.src_path), module.name.clone()))
      .collect::<BTreeMap<_, _>>();
    let mut merged_hsp_names = BTreeSet::new();
    for node in hap_graph.nodes.values().filter(|node| {
      matches!(
        node.node_type,
        DependencyNodeType::SharedHsp | DependencyNodeType::IntegratedHsp
      )
    }) {
      let Some(root) = node.package_root.as_deref() else {
        continue;
      };
      if let Some(module_name) = local_hsp_roots.get(&canonicalize_or_owned(root)) {
        merged_hsp_names.insert(module_name.clone());
      }
    }

    let mut hsp_inventories = BTreeMap::new();
    for module_name in merged_hsp_names {
      let graph = project
        .dependency_graph(&module_name)
        .ok_or_else(|| anyhow::anyhow!("missing dependency graph for HSP '{}'", module_name))?;
      hsp_inventories.insert(module_name, HarDependencyInventory::new(graph.as_ref()));
    }

    let mut duplicate_roots = hsp_inventories
      .keys()
      .map(|name| (name.clone(), BTreeSet::<PathBuf>::new()))
      .collect::<BTreeMap<_, _>>();
    let mut errors = BTreeSet::new();

    for (hsp_name, inventory) in &hsp_inventories {
      for root in hap_inventory.intersection(inventory) {
        if inventory.direct.contains(root) && !hap_inventory.direct.contains(root) {
          errors.insert(Self::dependency_error(
            &hap.name,
            hsp_name,
            root,
            &hap_inventory,
            inventory,
          ));
        }
        duplicate_roots
          .get_mut(hsp_name)
          .expect("HSP duplicate set initialized")
          .insert(root.clone());
      }
    }

    let hsp_names = hsp_inventories.keys().cloned().collect::<Vec<_>>();
    for left_index in 0..hsp_names.len() {
      for right_index in (left_index + 1)..hsp_names.len() {
        let left_name = &hsp_names[left_index];
        let right_name = &hsp_names[right_index];
        let left = &hsp_inventories[left_name];
        let right = &hsp_inventories[right_name];
        for root in left.intersection(right) {
          if (left.direct.contains(root) || right.direct.contains(root))
            && !hap_inventory.direct.contains(root)
          {
            let package_name = left
              .dependencies
              .get(root)
              .or_else(|| right.dependencies.get(root))
              .map(|dependency| dependency.package_name.as_str())
              .unwrap_or("<unknown>");
            errors.insert(format!(
              "{package_name}: the common dependency of {left_name} and {right_name} must be declared in dependencies of {}",
              hap.name
            ));
          }
          duplicate_roots
            .get_mut(left_name)
            .expect("left HSP duplicate set initialized")
            .insert(root.clone());
          duplicate_roots
            .get_mut(right_name)
            .expect("right HSP duplicate set initialized")
            .insert(root.clone());
        }
      }
    }

    if !errors.is_empty() {
      anyhow::bail!(
        "HAR deduplication dependency check failed for entry HAP '{}':\n{}\nDeclare the listed common dependencies in {}",
        hap.name,
        errors.into_iter().collect::<Vec<_>>().join("\n"),
        hap.src_path.join("oh-package.json5").display()
      );
    }

    let mut modules = BTreeMap::new();
    for (hsp_name, roots) in duplicate_roots {
      let inventory = &hsp_inventories[&hsp_name];
      let mut plan = HarDedupModulePlan::default();
      for root in roots {
        let Some(dependency) = inventory.dependencies.get(&root) else {
          continue;
        };
        plan.duplicate_package_roots.push(root.clone());
        match dependency.node_type {
          DependencyNodeType::SourceHar => plan.source_har_roots.push(root),
          DependencyNodeType::BytecodeHar => {
            plan
              .bytecode_har_packages
              .insert(dependency.package_name.clone());
          }
          _ => {}
        }
      }
      plan.duplicate_package_roots.sort();
      plan.duplicate_package_roots.dedup();
      plan.source_har_roots.sort();
      plan.source_har_roots.dedup();
      modules.insert(hsp_name, plan);
    }
    Ok(Self { modules })
  }

  pub(super) fn module(&self, module_name: &str) -> HarDedupModulePlan {
    self.modules.get(module_name).cloned().unwrap_or_default()
  }

  fn dependency_error(
    hap_name: &str,
    hsp_name: &str,
    root: &Path,
    hap: &HarDependencyInventory,
    hsp: &HarDependencyInventory,
  ) -> String {
    let package_name = hsp
      .dependencies
      .get(root)
      .or_else(|| hap.dependencies.get(root))
      .map(|dependency| dependency.package_name.as_str())
      .unwrap_or("<unknown>");
    format!(
      "{package_name}: the common dependency of {hap_name} and {hsp_name} must be declared in dependencies of {hap_name}"
    )
  }
}

struct HarDependencyInventory {
  dependencies: BTreeMap<PathBuf, HarDependency>,
  direct: BTreeSet<PathBuf>,
}

struct HarDependency {
  package_name: String,
  node_type: DependencyNodeType,
}

impl HarDependencyInventory {
  fn new(graph: &DependencyGraphSnapshot) -> Self {
    let mut dependencies = BTreeMap::new();
    let mut direct = BTreeSet::new();
    for node in graph.nodes.values().filter(|node| !Self::is_hsp(node)) {
      let Some(root) = node.package_root.as_deref() else {
        continue;
      };
      let root = canonicalize_or_owned(root);
      dependencies.insert(
        root.clone(),
        HarDependency {
          package_name: node.package_name.clone(),
          node_type: node.node_type,
        },
      );
      if !node.is_transitive {
        direct.insert(root);
      }
    }
    Self {
      dependencies,
      direct,
    }
  }

  fn intersection<'a>(&'a self, other: &'a Self) -> impl Iterator<Item = &'a PathBuf> {
    self
      .dependencies
      .keys()
      .filter(|root| other.dependencies.contains_key(*root))
  }

  fn is_hsp(node: &DependencyNodeSnapshot) -> bool {
    matches!(
      node.node_type,
      DependencyNodeType::SharedHsp | DependencyNodeType::IntegratedHsp
    )
  }
}
