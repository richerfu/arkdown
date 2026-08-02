use oxc_index::{Idx, IndexVec};
use rustc_hash::{FxHashMap, FxHashSet};

use super::model::RuntimeTaskDescriptor;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuntimeTaskId(u32);

impl Idx for RuntimeTaskId {
  const MAX: usize = u32::MAX as usize;

  fn from_usize(index: usize) -> Self {
    Self(index as u32)
  }

  unsafe fn from_usize_unchecked(index: usize) -> Self {
    Self(index as u32)
  }

  fn index(self) -> usize {
    self.0 as usize
  }
}

#[derive(Debug, Clone)]
pub struct RuntimeTaskNode {
  pub descriptor: RuntimeTaskDescriptor,
  pub dependencies: Vec<RuntimeTaskId>,
  pub user_dependencies: Vec<RuntimeTaskId>,
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeTaskGraphError {
  #[error("duplicate runtime task path '{0}'")]
  DuplicateTask(String),
  #[error("task {task} depends on missing task {dependency:?}")]
  MissingDependency { task: String, dependency: String },
  #[error("runtime task selection references missing task {0:?}")]
  MissingSelection(String),
  #[error("cyclic custom task dependency at {0}")]
  Cycle(String),
  #[error(
    "custom task {task} requires built-in task {dependency} before its native stage has completed"
  )]
  BuiltInNotCompleted { task: String, dependency: String },
  #[error(
    "custom task {task} has a post-dependent built-in task {dependency} that cannot run inside it"
  )]
  InvalidPostDependency { task: String, dependency: String },
}

pub struct RuntimeTaskGraph {
  nodes: IndexVec<RuntimeTaskId, RuntimeTaskNode>,
  by_path: FxHashMap<String, RuntimeTaskId>,
  by_event: FxHashMap<(String, String), RuntimeTaskId>,
}

impl RuntimeTaskGraph {
  pub fn new(descriptors: Vec<RuntimeTaskDescriptor>) -> Result<Self, RuntimeTaskGraphError> {
    let mut nodes: IndexVec<RuntimeTaskId, RuntimeTaskNode> =
      IndexVec::with_capacity(descriptors.len());
    let mut by_path = FxHashMap::default();
    let mut by_event = FxHashMap::default();

    for descriptor in descriptors {
      if by_path.contains_key(&descriptor.path) {
        return Err(RuntimeTaskGraphError::DuplicateTask(descriptor.path));
      }
      let path = descriptor.path.clone();
      let event_key = (descriptor.node_name.clone(), descriptor.task_name.clone());
      let id = nodes.push(RuntimeTaskNode {
        descriptor,
        dependencies: Vec::new(),
        user_dependencies: Vec::new(),
      });
      by_path.insert(path, id);
      by_event.insert(event_key, id);
    }

    for id in nodes.indices().collect::<Vec<_>>() {
      let task_path = nodes[id].descriptor.path.clone();
      let dependencies =
        Self::resolve_dependencies(&by_path, &task_path, &nodes[id].descriptor.dependencies)?;
      let user_dependencies = Self::resolve_dependencies(
        &by_path,
        &task_path,
        &nodes[id].descriptor.user_dependencies,
      )?;
      nodes[id].dependencies = dependencies;
      nodes[id].user_dependencies = user_dependencies;
    }

    Ok(Self {
      nodes,
      by_path,
      by_event,
    })
  }

  pub fn task(&self, id: RuntimeTaskId) -> &RuntimeTaskNode {
    &self.nodes[id]
  }

  pub fn task_mut(&mut self, id: RuntimeTaskId) -> &mut RuntimeTaskNode {
    &mut self.nodes[id]
  }

  pub fn tasks(&self) -> impl Iterator<Item = (RuntimeTaskId, &RuntimeTaskNode)> {
    self.nodes.iter_enumerated()
  }

  pub fn id_for_path(&self, path: &str) -> Result<RuntimeTaskId, RuntimeTaskGraphError> {
    self
      .by_path
      .get(path)
      .copied()
      .ok_or_else(|| RuntimeTaskGraphError::MissingSelection(path.to_string()))
  }

  pub fn ids_for_paths(
    &self,
    paths: &[String],
  ) -> Result<Vec<RuntimeTaskId>, RuntimeTaskGraphError> {
    paths.iter().map(|path| self.id_for_path(path)).collect()
  }

  pub fn id_for_event(&self, node_name: &str, task_name: &str) -> Option<RuntimeTaskId> {
    self
      .by_event
      .get(&(node_name.to_string(), task_name.to_string()))
      .copied()
  }

  pub fn custom_execution_plan(
    &self,
    root: RuntimeTaskId,
    completed: &FxHashSet<RuntimeTaskId>,
  ) -> Result<Vec<RuntimeTaskId>, RuntimeTaskGraphError> {
    let mut planned = FxHashSet::default();
    let mut active = FxHashSet::default();
    let mut output = Vec::new();
    self.visit_custom(root, completed, &mut planned, &mut active, &mut output)?;
    Ok(output)
  }

  pub fn dependency_closure(
    &self,
    roots: &[RuntimeTaskId],
  ) -> Result<FxHashSet<RuntimeTaskId>, RuntimeTaskGraphError> {
    let mut closure = FxHashSet::default();
    let mut active = FxHashSet::default();
    for root in roots {
      self.visit_dependency_closure(*root, &mut closure, &mut active)?;
    }
    Ok(closure)
  }

  pub fn custom_task_is_ready(
    &self,
    root: RuntimeTaskId,
    completed: &FxHashSet<RuntimeTaskId>,
  ) -> Result<bool, RuntimeTaskGraphError> {
    let mut visited = FxHashSet::default();
    let mut active = FxHashSet::default();
    self.visit_ready(root, completed, &mut visited, &mut active)
  }

  fn resolve_dependencies(
    by_path: &FxHashMap<String, RuntimeTaskId>,
    task_path: &str,
    dependencies: &[String],
  ) -> Result<Vec<RuntimeTaskId>, RuntimeTaskGraphError> {
    dependencies
      .iter()
      .map(|dependency| {
        by_path
          .get(dependency)
          .copied()
          .ok_or_else(|| RuntimeTaskGraphError::MissingDependency {
            task: task_path.to_string(),
            dependency: dependency.clone(),
          })
      })
      .collect()
  }

  fn visit_custom(
    &self,
    id: RuntimeTaskId,
    completed: &FxHashSet<RuntimeTaskId>,
    planned: &mut FxHashSet<RuntimeTaskId>,
    active: &mut FxHashSet<RuntimeTaskId>,
    output: &mut Vec<RuntimeTaskId>,
  ) -> Result<(), RuntimeTaskGraphError> {
    if completed.contains(&id) || planned.contains(&id) {
      return Ok(());
    }
    let node = &self.nodes[id];
    if !active.insert(id) {
      return Err(RuntimeTaskGraphError::Cycle(node.descriptor.path.clone()));
    }
    for dependency in &node.user_dependencies {
      let dependency_node = &self.nodes[*dependency];
      if dependency_node.descriptor.built_in {
        return Err(RuntimeTaskGraphError::InvalidPostDependency {
          task: node.descriptor.path.clone(),
          dependency: dependency_node.descriptor.path.clone(),
        });
      }
      self.visit_custom(*dependency, completed, planned, active, output)?;
    }
    for dependency in &node.dependencies {
      let dependency_node = &self.nodes[*dependency];
      if dependency_node.descriptor.built_in {
        if !completed.contains(dependency) {
          return Err(RuntimeTaskGraphError::BuiltInNotCompleted {
            task: node.descriptor.path.clone(),
            dependency: dependency_node.descriptor.path.clone(),
          });
        }
      } else {
        self.visit_custom(*dependency, completed, planned, active, output)?;
      }
    }
    active.remove(&id);
    if planned.insert(id) {
      output.push(id);
    }
    Ok(())
  }

  fn visit_dependency_closure(
    &self,
    id: RuntimeTaskId,
    closure: &mut FxHashSet<RuntimeTaskId>,
    active: &mut FxHashSet<RuntimeTaskId>,
  ) -> Result<(), RuntimeTaskGraphError> {
    if closure.contains(&id) {
      return Ok(());
    }
    let node = &self.nodes[id];
    if !active.insert(id) {
      return Err(RuntimeTaskGraphError::Cycle(node.descriptor.path.clone()));
    }
    for dependency in node
      .dependencies
      .iter()
      .chain(node.user_dependencies.iter())
    {
      self.visit_dependency_closure(*dependency, closure, active)?;
    }
    active.remove(&id);
    closure.insert(id);
    Ok(())
  }

  fn visit_ready(
    &self,
    id: RuntimeTaskId,
    completed: &FxHashSet<RuntimeTaskId>,
    visited: &mut FxHashSet<RuntimeTaskId>,
    active: &mut FxHashSet<RuntimeTaskId>,
  ) -> Result<bool, RuntimeTaskGraphError> {
    if completed.contains(&id) || visited.contains(&id) {
      return Ok(true);
    }
    let node = &self.nodes[id];
    if !active.insert(id) {
      return Err(RuntimeTaskGraphError::Cycle(node.descriptor.path.clone()));
    }
    for dependency in node
      .dependencies
      .iter()
      .chain(node.user_dependencies.iter())
    {
      let dependency_node = &self.nodes[*dependency];
      if dependency_node.descriptor.built_in {
        if !completed.contains(dependency) {
          active.remove(&id);
          return Ok(false);
        }
      } else if !self.visit_ready(*dependency, completed, visited, active)? {
        active.remove(&id);
        return Ok(false);
      }
    }
    active.remove(&id);
    visited.insert(id);
    Ok(true)
  }
}

#[cfg(test)]
mod tests {
  use rustc_hash::FxHashSet;

  use super::*;

  fn task(path: &str, built_in: bool, dependencies: &[&str]) -> RuntimeTaskDescriptor {
    RuntimeTaskDescriptor {
      path: path.to_string(),
      node_name: "entry".to_string(),
      task_name: path.rsplit(':').next().unwrap().to_string(),
      built_in,
      project_task: false,
      enabled: true,
      track_state: true,
      refresh_declarations: false,
      has_before_run: false,
      has_after_run: false,
      has_failed: false,
      has_task_should_do: false,
      graph_bound_custom: !built_in && !dependencies.is_empty(),
      dependencies: dependencies
        .iter()
        .map(|value| (*value).to_string())
        .collect(),
      user_dependencies: Vec::new(),
      input_properties: Vec::new(),
      input_files: Vec::new(),
      output_files: Vec::new(),
    }
  }

  #[test]
  fn plans_custom_dependencies_in_stable_postorder() {
    let graph = RuntimeTaskGraph::new(vec![
      task("entry:a", false, &[]),
      task("entry:b", false, &["entry:a"]),
      task("entry:c", false, &["entry:b"]),
    ])
    .unwrap();
    let root = graph.id_for_path("entry:c").unwrap();
    let plan = graph
      .custom_execution_plan(root, &FxHashSet::default())
      .unwrap();
    assert_eq!(
      plan
        .iter()
        .map(|id| graph.task(*id).descriptor.path.as_str())
        .collect::<Vec<_>>(),
      ["entry:a", "entry:b", "entry:c"]
    );
  }

  #[test]
  fn selects_only_the_requested_dependency_closure() {
    let graph = RuntimeTaskGraph::new(vec![
      task("entry:Compile", true, &[]),
      task("entry:dependency", false, &["entry:Compile"]),
      task("entry:root", false, &["entry:dependency"]),
      task("entry:unrelated", false, &[]),
    ])
    .unwrap();
    let closure = graph
      .dependency_closure(&[graph.id_for_path("entry:root").unwrap()])
      .unwrap();
    let paths = graph
      .tasks()
      .filter_map(|(id, node)| {
        closure
          .contains(&id)
          .then_some(node.descriptor.path.as_str())
      })
      .collect::<Vec<_>>();

    assert_eq!(paths, ["entry:Compile", "entry:dependency", "entry:root"]);
  }

  #[test]
  fn rejects_cycles_and_incomplete_native_dependencies() {
    let cycle = RuntimeTaskGraph::new(vec![
      task("entry:a", false, &["entry:b"]),
      task("entry:b", false, &["entry:a"]),
    ])
    .unwrap();
    let error = cycle
      .custom_execution_plan(cycle.id_for_path("entry:a").unwrap(), &FxHashSet::default())
      .unwrap_err();
    assert!(matches!(error, RuntimeTaskGraphError::Cycle(_)));

    let native = RuntimeTaskGraph::new(vec![
      task("entry:Compile", true, &[]),
      task("entry:custom", false, &["entry:Compile"]),
    ])
    .unwrap();
    let error = native
      .custom_execution_plan(
        native.id_for_path("entry:custom").unwrap(),
        &FxHashSet::default(),
      )
      .unwrap_err();
    assert!(matches!(
      error,
      RuntimeTaskGraphError::BuiltInNotCompleted { .. }
    ));
  }
}
