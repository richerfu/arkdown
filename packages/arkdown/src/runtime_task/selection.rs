use std::path::PathBuf;

use rustc_hash::FxHashSet;
use serde::Deserialize;

use super::model::RuntimeTaskSelection;

#[derive(Debug, Default)]
struct RuntimeTaskSelectionCycle {
  expected_root_paths: Option<Vec<String>>,
  required_boundary_paths: Vec<String>,
  command_root_paths: Vec<String>,
  execution_node_names: Vec<String>,
}

/// Owns the task-root selection for one Hvigor command/build cycle. JavaScript
/// keeps live official task objects, but only their stable graph paths cross
/// this boundary.
pub(crate) struct RuntimeTaskSelectionSession {
  project_root: PathBuf,
  cache_enabled: bool,
  cycle: RuntimeTaskSelectionCycle,
}

impl RuntimeTaskSelectionSession {
  pub(crate) fn new(project_root: PathBuf, cache_enabled: bool) -> Self {
    Self {
      project_root,
      cache_enabled,
      cycle: RuntimeTaskSelectionCycle::default(),
    }
  }

  pub(crate) fn prepare_runtime_cycle(
    &mut self,
    root_paths: Vec<String>,
    execution_node_names: Vec<String>,
  ) {
    self.cycle = RuntimeTaskSelectionCycle {
      expected_root_paths: Some(root_paths.clone()),
      command_root_paths: root_paths,
      execution_node_names: Self::unique_names(execution_node_names),
      ..RuntimeTaskSelectionCycle::default()
    };
  }

  pub(crate) fn prepare_command_cycle(
    &mut self,
    execution_node_names: Vec<String>,
    command_root_paths: Vec<String>,
    required_boundary_paths: Vec<String>,
  ) {
    let expected_root_paths = required_boundary_paths
      .iter()
      .chain(&command_root_paths)
      .cloned()
      .collect();
    self.cycle = RuntimeTaskSelectionCycle {
      expected_root_paths: Some(expected_root_paths),
      required_boundary_paths,
      command_root_paths,
      execution_node_names: Self::unique_names(execution_node_names),
    };
  }

  pub(crate) fn prepare_direct_cycle(
    &mut self,
    root_paths: Vec<String>,
    execution_node_names: Vec<String>,
  ) {
    self.cycle = RuntimeTaskSelectionCycle {
      expected_root_paths: Some(root_paths),
      execution_node_names: Self::unique_names(execution_node_names),
      ..RuntimeTaskSelectionCycle::default()
    };
  }

  pub(crate) fn selection(
    &self,
    validate_boundaries: bool,
  ) -> serde_json::Result<serde_json::Value> {
    serde_json::to_value(RuntimeTaskSelection {
      project_root: self.project_root.clone(),
      cache_enabled: self.cache_enabled,
      expected_root_paths: self.cycle.expected_root_paths.clone(),
      required_boundary_paths: self.cycle.required_boundary_paths.clone(),
      command_root_paths: self.cycle.command_root_paths.clone(),
      execution_node_names: self.cycle.execution_node_names.clone(),
      validate_boundaries,
    })
  }

  pub(crate) fn clear(&mut self) {
    self.cycle = RuntimeTaskSelectionCycle::default();
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeDirectTaskModel {
  #[serde(default)]
  nodes: Vec<RuntimeDirectTaskNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeDirectTaskNode {
  name: String,
  #[serde(default)]
  kind: Option<String>,
  project_task: bool,
  #[serde(default)]
  child_names: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct RuntimeDirectTaskCycle {
  pub(crate) root_paths: Vec<String>,
  pub(crate) execution_node_names: Vec<String>,
}

/// Selects direct-build assembly roots from the same Rust-owned module plan
/// used by command mode. Live JavaScript nodes contribute identity metadata,
/// but do not reimplement target/module/project selection.
pub(crate) struct RuntimeDirectTaskCyclePlanner {
  target: String,
  selected_module_names: FxHashSet<String>,
  model: RuntimeDirectTaskModel,
}

impl RuntimeDirectTaskCyclePlanner {
  pub(crate) fn from_value(
    target: &str,
    selected_module_names: Vec<String>,
    model: serde_json::Value,
  ) -> anyhow::Result<Self> {
    Ok(Self {
      target: target.to_string(),
      selected_module_names: selected_module_names.into_iter().collect(),
      model: serde_json::from_value(model)?,
    })
  }

  pub(crate) fn plan(self) -> RuntimeDirectTaskCycle {
    let mut root_paths = Vec::new();
    let mut execution_node_names = Vec::new();
    let mut target_characters = self.target.chars();
    let task_name = format!(
      "assemble{}{}",
      target_characters
        .next()
        .map(|character| character.to_uppercase().collect::<String>())
        .unwrap_or_default(),
      target_characters.as_str(),
    );
    for node in self.model.nodes {
      if self.target == "app" {
        if node.project_task && node.kind.as_deref() == Some("app") {
          root_paths.push(format!(":{task_name}"));
          execution_node_names.extend(node.child_names);
          execution_node_names.push(node.name);
        }
        continue;
      }
      if node.project_task || node.kind.as_deref() != Some(self.target.as_str()) {
        continue;
      }
      if !self.selected_module_names.is_empty() && !self.selected_module_names.contains(&node.name)
      {
        continue;
      }
      root_paths.push(format!("{}:{task_name}", node.name));
      execution_node_names.push(node.name);
    }
    RuntimeDirectTaskCycle {
      root_paths,
      execution_node_names,
    }
  }
}

impl RuntimeTaskSelectionSession {
  fn unique_names(names: Vec<String>) -> Vec<String> {
    let mut seen = FxHashSet::default();
    names
      .into_iter()
      .filter(|name| seen.insert(name.clone()))
      .collect()
  }
}
