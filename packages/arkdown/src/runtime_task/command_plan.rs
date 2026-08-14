use indexmap::IndexMap;
use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::catalog::{RuntimeTaskCatalogRequest, RuntimeTaskNodeKind};
use super::graph::{RuntimeTaskGraph, RuntimeTaskGraphError, RuntimeTaskId};
use super::model::{RuntimeTaskDescriptor, RuntimeTaskModelBuilder};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeCommandPlanRequest {
  project_name: String,
  #[serde(default)]
  execution_mode: Option<String>,
  #[serde(default)]
  sync: bool,
  #[serde(default)]
  sync_version_changed: bool,
  #[serde(default)]
  task_names: Vec<Value>,
  #[serde(default)]
  module_selections: Vec<RuntimeCommandModuleSelection>,
  #[serde(default)]
  build_target_name: Option<String>,
  #[serde(default)]
  nodes: Vec<RuntimeCommandNode>,
  #[serde(default)]
  catalogs: Vec<RuntimeTaskCatalogRequest>,
  #[serde(default)]
  removed_task_paths: Vec<String>,
  #[serde(default)]
  tasks: Vec<RuntimeTaskDescriptor>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeCommandModuleSelection {
  module_name: String,
  #[serde(default)]
  target_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeCommandNode {
  name: String,
  #[serde(default)]
  kind: Option<RuntimeTaskNodeKind>,
  project_task: bool,
  #[serde(default)]
  child_names: Vec<String>,
  #[serde(default)]
  custom_sync_task_paths: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
enum RuntimeCommandKind {
  Assembly,
  Native,
  Custom,
  Tasks,
  TaskTree,
  Prune,
  Clean,
  Sync,
  BuildInfo,
  CollectCoverage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
enum RuntimeNativeOutputTarget {
  Hap,
  Preview,
  Test,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeCommandTask {
  task_path: String,
  command_kind: RuntimeCommandKind,
  #[serde(skip_serializing_if = "Option::is_none")]
  dependency_only: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeNativeTaskRoot {
  module_name: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  target_name: Option<String>,
  task_name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeNativeBuildRequest {
  target: RuntimeTaskNodeKind,
  #[serde(skip_serializing_if = "Option::is_none")]
  output_target: Option<RuntimeNativeOutputTarget>,
  #[serde(skip_serializing_if = "Option::is_none")]
  module: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  target_name: Option<String>,
  node_names: Vec<String>,
  root_task_paths: Vec<String>,
  cutoff_task_roots: Vec<RuntimeNativeTaskRoot>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeCommandPlan {
  tasks: Vec<RuntimeCommandTask>,
  native_requests: Vec<RuntimeNativeBuildRequest>,
  selected_node_names: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeCommandDiagnostic {
  code: &'static str,
  message: String,
  #[serde(flatten)]
  details: IndexMap<String, Value>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeCommandPlanEnvelope {
  #[serde(skip_serializing_if = "Option::is_none")]
  plan: Option<RuntimeCommandPlan>,
  #[serde(skip_serializing_if = "Option::is_none")]
  error: Option<RuntimeCommandDiagnostic>,
}

#[derive(Debug)]
enum RuntimeCommandPlanError {
  TaskNotFound {
    task_names: Vec<Value>,
    project_name: String,
  },
  UnknownExecutionMode(String),
  UnknownModule(String),
  MissingNodeKind {
    task_path: String,
    node_name: String,
  },
  UnsupportedBuiltin {
    task_path: String,
    task_name: String,
    node_name: String,
  },
  Graph(RuntimeTaskGraphError),
}

impl RuntimeCommandPlanError {
  fn diagnostic(self) -> RuntimeCommandDiagnostic {
    match self {
      Self::TaskNotFound {
        task_names,
        project_name,
      } => {
        let mut details = IndexMap::new();
        details.insert("taskNames".to_string(), Value::Array(task_names.clone()));
        RuntimeCommandDiagnostic {
          code: "HVIGOR_TASK_NOT_FOUND",
          message: format!(
            "Task {} was not found in the project {project_name}. Invalid command to execute the build task, please verify the parameters in the command and try again.",
            Value::Array(task_names)
          ),
          details,
        }
      }
      Self::UnknownExecutionMode(execution_mode) => {
        let mut details = IndexMap::new();
        details.insert(
          "executionMode".to_string(),
          Value::String(execution_mode.clone()),
        );
        RuntimeCommandDiagnostic {
          code: "UNKNOWN_HVIGOR_EXECUTION_MODE",
          message: format!(
            "Unknown mode '{execution_mode}' is specified in the command line. Please check."
          ),
          details,
        }
      }
      Self::UnknownModule(module_name) => {
        let mut details = IndexMap::new();
        details.insert(
          "moduleName".to_string(),
          Value::String(module_name.clone()),
        );
        RuntimeCommandDiagnostic {
          code: "UNKNOWN_MODULE_IN_COMMAND_LINE_BUILD_PROFILE",
          message: format!("module '{module_name}' was not found in build-profile.json5"),
          details,
        }
      }
      Self::MissingNodeKind {
        task_path,
        node_name,
      } => RuntimeCommandDiagnostic {
        code: "ARKDOWN_HVIGOR_NODE_KIND_NOT_FOUND",
        message: format!(
          "hvigor task {task_path} belongs to node {node_name}, which has no bound OHOS plugin kind"
        ),
        details: IndexMap::from([
          ("taskPath".to_string(), Value::String(task_path)),
          ("moduleName".to_string(), Value::String(node_name)),
        ]),
      },
      Self::UnsupportedBuiltin {
        task_path,
        task_name,
        node_name,
      } => RuntimeCommandDiagnostic {
        code: "ARKDOWN_UNSUPPORTED_HVIGOR_BUILTIN_TASK",
        message: format!(
          "hvigor task {task_path} is present in the latest 6.24.3 graph but arkdown does not yet own its action"
        ),
        details: IndexMap::from([
          ("taskName".to_string(), Value::String(task_name)),
          ("moduleName".to_string(), Value::String(node_name)),
        ]),
      },
      Self::Graph(error) => Self::graph_diagnostic(error),
    }
  }

  fn graph_diagnostic(error: RuntimeTaskGraphError) -> RuntimeCommandDiagnostic {
    match error {
      RuntimeTaskGraphError::MissingDependency { task, dependency } => RuntimeCommandDiagnostic {
        code: "HVIGOR_TASK_DEPENDENCY_NOT_FOUND",
        message: format!("task {task} depends on missing task {dependency:?}"),
        details: IndexMap::from([
          ("taskPath".to_string(), Value::String(task)),
          ("dependencyPath".to_string(), Value::String(dependency)),
        ]),
      },
      RuntimeTaskGraphError::Cycle(task_path) => RuntimeCommandDiagnostic {
        code: "CYCLIC_HVIGOR_TASK_DEPENDENCY",
        message: format!("cyclic custom task dependency at {task_path}"),
        details: IndexMap::from([("taskPath".to_string(), Value::String(task_path))]),
      },
      RuntimeTaskGraphError::DuplicateTask(task_path) => RuntimeCommandDiagnostic {
        code: "ARKDOWN_DUPLICATE_RUNTIME_TASK",
        message: format!("duplicate runtime task path '{task_path}'"),
        details: IndexMap::from([("taskPath".to_string(), Value::String(task_path))]),
      },
      RuntimeTaskGraphError::MissingSelection(task_path) => RuntimeCommandDiagnostic {
        code: "ARKDOWN_RUNTIME_TASK_NOT_FOUND",
        message: format!("runtime task selection references missing task {task_path:?}"),
        details: IndexMap::from([("taskPath".to_string(), Value::String(task_path))]),
      },
      RuntimeTaskGraphError::BuiltInNotCompleted { task, dependency } => {
        RuntimeCommandDiagnostic {
          code: "ARKDOWN_RUNTIME_TASK_BOUNDARY_NOT_COMPLETED",
          message: format!(
            "custom task {task} requires built-in task {dependency} before its native stage has completed"
          ),
          details: IndexMap::from([
            ("taskPath".to_string(), Value::String(task)),
            ("dependencyPath".to_string(), Value::String(dependency)),
          ]),
        }
      }
      RuntimeTaskGraphError::InvalidPostDependency { task, dependency } => {
        RuntimeCommandDiagnostic {
          code: "ARKDOWN_INVALID_RUNTIME_TASK_POST_DEPENDENCY",
          message: format!(
            "custom task {task} has a post-dependent built-in task {dependency} that cannot run inside it"
          ),
          details: IndexMap::from([
            ("taskPath".to_string(), Value::String(task)),
            ("dependencyPath".to_string(), Value::String(dependency)),
          ]),
        }
      }
    }
  }
}

#[derive(Debug, Clone)]
struct PlannedRuntimeTask {
  id: RuntimeTaskId,
  command_kind: RuntimeCommandKind,
  dependency_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RuntimeNativeRequestIdentity {
  target: RuntimeTaskNodeKind,
  output_target: Option<RuntimeNativeOutputTarget>,
  module_name: Option<String>,
  target_name: Option<String>,
}

struct RuntimeNativeBuildRequestBuilder {
  target: RuntimeTaskNodeKind,
  output_target: Option<RuntimeNativeOutputTarget>,
  module_name: Option<String>,
  target_name: Option<String>,
  node_names: Vec<String>,
  root_task_ids: Vec<RuntimeTaskId>,
  cutoff_task_ids: Vec<RuntimeTaskId>,
  has_assembly_root: bool,
}

pub(crate) struct RuntimeCommandPlanner {
  request: RuntimeCommandPlanRequest,
  graph: RuntimeTaskGraph,
  custom_sync_task_paths: FxHashSet<String>,
}

impl RuntimeCommandPlanner {
  pub fn response_json(request_json: &str) -> anyhow::Result<String> {
    let mut request: RuntimeCommandPlanRequest = serde_json::from_str(request_json)?;
    let descriptors = RuntimeTaskModelBuilder::new(
      std::mem::take(&mut request.catalogs),
      std::mem::take(&mut request.tasks),
      std::mem::take(&mut request.removed_task_paths),
    )
    .build()?;
    let response = match RuntimeTaskGraph::new(descriptors) {
      Ok(graph) => {
        let custom_sync_task_paths = request
          .nodes
          .iter()
          .flat_map(|node| node.custom_sync_task_paths.iter().cloned())
          .collect();
        let mut planner = Self {
          request,
          graph,
          custom_sync_task_paths,
        };
        match planner.plan() {
          Ok(plan) => RuntimeCommandPlanEnvelope {
            plan: Some(plan),
            error: None,
          },
          Err(error) => RuntimeCommandPlanEnvelope {
            plan: None,
            error: Some(error.diagnostic()),
          },
        }
      }
      Err(error) => RuntimeCommandPlanEnvelope {
        plan: None,
        error: Some(RuntimeCommandPlanError::Graph(error).diagnostic()),
      },
    };
    serde_json::to_string(&response).map_err(Into::into)
  }

  fn plan(&mut self) -> Result<RuntimeCommandPlan, RuntimeCommandPlanError> {
    let selected_nodes = self.selected_node_indexes()?;
    let mut tasks = Vec::new();
    let mut seen_tasks = FxHashSet::default();

    if self.request.sync {
      if self.request.sync_version_changed {
        for node_index in &selected_nodes {
          if let Some(id) = self.task_for_node(*node_index, "clean") {
            self.push_task(
              &mut tasks,
              &mut seen_tasks,
              id,
              RuntimeCommandKind::Clean,
              false,
            );
          }
        }
      }
      for node_index in &selected_nodes {
        if let Some(id) = self.task_for_node(*node_index, "init") {
          self.push_task(
            &mut tasks,
            &mut seen_tasks,
            id,
            RuntimeCommandKind::Sync,
            false,
          );
        }
        let sync_paths = self.request.nodes[*node_index]
          .custom_sync_task_paths
          .clone();
        for task_path in sync_paths {
          let id = self
            .graph
            .id_for_path(&task_path)
            .map_err(RuntimeCommandPlanError::Graph)?;
          self.push_task(
            &mut tasks,
            &mut seen_tasks,
            id,
            RuntimeCommandKind::Custom,
            false,
          );
        }
      }
    }

    if self.request.task_names.is_empty() && !self.request.sync {
      return Err(self.task_not_found());
    }
    for task_name in self.request.task_names.clone() {
      let Some(task_name) = task_name.as_str() else {
        return Err(self.task_not_found());
      };
      let mut matching = Vec::new();
      for node_index in &selected_nodes {
        if let Some(id) = self.task_for_node(*node_index, task_name) {
          matching.push(id);
        }
      }
      if matching.is_empty() {
        return Err(self.task_not_found());
      }
      for id in matching {
        let command_kind = self.command_kind(id)?;
        self.push_task(&mut tasks, &mut seen_tasks, id, command_kind, false);
      }
    }

    let roots = tasks.clone();
    let mut seen_native_tasks = tasks
      .iter()
      .filter(|task| {
        matches!(
          task.command_kind,
          RuntimeCommandKind::Assembly | RuntimeCommandKind::Native
        )
      })
      .map(|task| task.id)
      .collect();
    for root in roots {
      let descriptor = &self.graph.task(root.id).descriptor;
      if descriptor.built_in || self.custom_sync_task_paths.contains(&descriptor.path) {
        continue;
      }
      self.collect_native_dependencies(
        root.id,
        &mut tasks,
        &mut seen_native_tasks,
        &mut FxHashSet::default(),
      )?;
    }

    let native_requests = self.native_requests(&tasks)?;
    let selected_node_names = selected_nodes
      .iter()
      .map(|index| self.request.nodes[*index].name.clone())
      .collect();
    let tasks = tasks
      .into_iter()
      .map(|task| RuntimeCommandTask {
        task_path: self.graph.task(task.id).descriptor.path.clone(),
        command_kind: task.command_kind,
        dependency_only: task.dependency_only.then_some(true),
      })
      .collect();
    Ok(RuntimeCommandPlan {
      tasks,
      native_requests,
      selected_node_names,
    })
  }

  fn task_not_found(&self) -> RuntimeCommandPlanError {
    RuntimeCommandPlanError::TaskNotFound {
      task_names: self.request.task_names.clone(),
      project_name: self.request.project_name.clone(),
    }
  }

  fn selected_node_indexes(&self) -> Result<Vec<usize>, RuntimeCommandPlanError> {
    match self.request.execution_mode.as_deref() {
      None | Some("module") | Some("project") => {}
      Some(execution_mode) => {
        return Err(RuntimeCommandPlanError::UnknownExecutionMode(
          execution_mode.to_string(),
        ));
      }
    }

    let module_indexes = self
      .request
      .nodes
      .iter()
      .enumerate()
      .filter_map(|(index, node)| (!node.project_task).then_some(index))
      .collect::<Vec<_>>();
    let project_index = self.request.nodes.iter().position(|node| node.project_task);
    let selected_modules = if self.request.module_selections.is_empty() {
      module_indexes
    } else {
      let mut selected = Vec::new();
      let mut seen = FxHashSet::default();
      for selection in &self.request.module_selections {
        let Some(index) = module_indexes
          .iter()
          .find(|index| self.request.nodes[**index].name == selection.module_name)
          .copied()
        else {
          return Err(RuntimeCommandPlanError::UnknownModule(
            selection.module_name.clone(),
          ));
        };
        if seen.insert(self.request.nodes[index].name.clone()) {
          selected.push(index);
        }
      }
      selected
    };

    Ok(match self.request.execution_mode.as_deref() {
      Some("module") => selected_modules,
      Some("project") => project_index.into_iter().collect(),
      None => selected_modules.into_iter().chain(project_index).collect(),
      Some(execution_mode) => {
        return Err(RuntimeCommandPlanError::UnknownExecutionMode(
          execution_mode.to_string(),
        ));
      }
    })
  }

  fn task_for_node(&self, node_index: usize, task_name: &str) -> Option<RuntimeTaskId> {
    let node = &self.request.nodes[node_index];
    self
      .graph
      .id_for_event(&node.name, task_name)
      .filter(|id| self.graph.task(*id).descriptor.project_task == node.project_task)
  }

  fn push_task(
    &self,
    tasks: &mut Vec<PlannedRuntimeTask>,
    seen: &mut FxHashSet<RuntimeTaskId>,
    id: RuntimeTaskId,
    command_kind: RuntimeCommandKind,
    dependency_only: bool,
  ) {
    if seen.insert(id) {
      tasks.push(PlannedRuntimeTask {
        id,
        command_kind,
        dependency_only,
      });
    }
  }

  fn collect_native_dependencies(
    &self,
    id: RuntimeTaskId,
    output: &mut Vec<PlannedRuntimeTask>,
    seen: &mut FxHashSet<RuntimeTaskId>,
    visiting: &mut FxHashSet<RuntimeTaskId>,
  ) -> Result<(), RuntimeCommandPlanError> {
    if !visiting.insert(id) {
      return Err(RuntimeCommandPlanError::Graph(
        RuntimeTaskGraphError::Cycle(self.graph.task(id).descriptor.path.clone()),
      ));
    }
    let dependencies = self.graph.task(id).dependencies.clone();
    for dependency in dependencies {
      if !self.graph.task(dependency).descriptor.built_in {
        self.collect_native_dependencies(dependency, output, seen, visiting)?;
        continue;
      }
      let command_kind = self.command_kind(dependency)?;
      if seen.insert(dependency) {
        output.push(PlannedRuntimeTask {
          id: dependency,
          command_kind,
          dependency_only: true,
        });
      }
    }
    visiting.remove(&id);
    Ok(())
  }

  fn command_kind(
    &self,
    task_id: RuntimeTaskId,
  ) -> Result<RuntimeCommandKind, RuntimeCommandPlanError> {
    let task = &self.graph.task(task_id).descriptor;
    if !task.built_in {
      return Ok(RuntimeCommandKind::Custom);
    }
    match task.task_name.as_str() {
      "tasks" => return Ok(RuntimeCommandKind::Tasks),
      "taskTree" => return Ok(RuntimeCommandKind::TaskTree),
      "prune" => return Ok(RuntimeCommandKind::Prune),
      "clean" => return Ok(RuntimeCommandKind::Clean),
      "init" => return Ok(RuntimeCommandKind::Sync),
      "buildInfo" => return Ok(RuntimeCommandKind::BuildInfo),
      "collectCoverage" => return Ok(RuntimeCommandKind::CollectCoverage),
      _ => {}
    }

    let node = self.node_for_task(task);
    let assembly_kind = match task.task_name.as_str() {
      "assembleHap" => Some(RuntimeTaskNodeKind::Hap),
      "assembleHsp" => Some(RuntimeTaskNodeKind::Hsp),
      "assembleHar" => Some(RuntimeTaskNodeKind::Har),
      "assembleApp" => Some(RuntimeTaskNodeKind::App),
      _ => None,
    };
    if assembly_kind.is_some_and(|kind| node.and_then(|node| node.kind) == Some(kind)) {
      return Ok(RuntimeCommandKind::Assembly);
    }
    let action_name = task
      .task_name
      .split_once('@')
      .map_or(task.task_name.as_str(), |(_, action)| action);
    if Self::is_native_action(action_name) {
      return Ok(RuntimeCommandKind::Native);
    }
    Err(RuntimeCommandPlanError::UnsupportedBuiltin {
      task_path: task.path.clone(),
      task_name: task.task_name.clone(),
      node_name: task.node_name.clone(),
    })
  }

  fn node_for_task(&self, task: &RuntimeTaskDescriptor) -> Option<&RuntimeCommandNode> {
    self
      .request
      .nodes
      .iter()
      .find(|node| node.name == task.node_name && node.project_task == task.project_task)
  }

  fn native_requests(
    &self,
    tasks: &[PlannedRuntimeTask],
  ) -> Result<Vec<RuntimeNativeBuildRequest>, RuntimeCommandPlanError> {
    let native_tasks = tasks
      .iter()
      .filter(|task| {
        matches!(
          task.command_kind,
          RuntimeCommandKind::Assembly | RuntimeCommandKind::Native
        )
      })
      .collect::<Vec<_>>();
    let mut requests =
      IndexMap::<RuntimeNativeRequestIdentity, RuntimeNativeBuildRequestBuilder>::new();

    for planned in &native_tasks {
      let task = &self.graph.task(planned.id).descriptor;
      let node =
        self
          .node_for_task(task)
          .ok_or_else(|| RuntimeCommandPlanError::MissingNodeKind {
            task_path: task.path.clone(),
            node_name: task.node_name.clone(),
          })?;
      let target = node
        .kind
        .ok_or_else(|| RuntimeCommandPlanError::MissingNodeKind {
          task_path: task.path.clone(),
          node_name: task.node_name.clone(),
        })?;
      let output_target = Self::native_output_target(
        task
          .task_name
          .split_once('@')
          .map_or(task.task_name.as_str(), |(_, action)| action),
        target,
      );
      let is_app_assembly = target == RuntimeTaskNodeKind::App && task.project_task;
      let selected_target = self
        .request
        .module_selections
        .iter()
        .find(|selection| selection.module_name == task.node_name)
        .and_then(|selection| selection.target_name.clone());
      let module_name = if is_app_assembly {
        None
      } else if !self.request.module_selections.is_empty() || planned.dependency_only {
        Some(task.node_name.clone())
      } else {
        None
      };
      let target_name = if is_app_assembly {
        self.request.build_target_name.clone()
      } else {
        selected_target.or_else(|| self.request.build_target_name.clone())
      };
      let identity = RuntimeNativeRequestIdentity {
        target,
        output_target,
        module_name: module_name.clone(),
        target_name: target_name.clone(),
      };
      let request = requests
        .entry(identity)
        .or_insert_with(|| RuntimeNativeBuildRequestBuilder {
          target,
          output_target,
          module_name,
          target_name,
          node_names: Vec::new(),
          root_task_ids: Vec::new(),
          cutoff_task_ids: Vec::new(),
          has_assembly_root: false,
        });
      request.root_task_ids.push(planned.id);
      if planned.command_kind == RuntimeCommandKind::Assembly {
        request.has_assembly_root = true;
      } else {
        request.cutoff_task_ids.push(planned.id);
      }

      let node_names = if is_app_assembly {
        node
          .child_names
          .iter()
          .cloned()
          .chain(std::iter::once(node.name.clone()))
          .collect::<Vec<_>>()
      } else if request.module_name.is_some() {
        vec![task.node_name.clone()]
      } else {
        native_tasks
          .iter()
          .filter_map(|candidate| {
            let candidate_task = &self.graph.task(candidate.id).descriptor;
            self
              .node_for_task(candidate_task)
              .filter(|candidate_node| candidate_node.kind == Some(target))
              .map(|candidate_node| candidate_node.name.clone())
          })
          .collect()
      };
      for node_name in node_names {
        if !request.node_names.contains(&node_name) {
          request.node_names.push(node_name);
        }
      }
    }

    Ok(
      requests
        .into_values()
        .map(|mut request| {
          if request.has_assembly_root {
            request.cutoff_task_ids.clear();
          }
          RuntimeNativeBuildRequest {
            target: request.target,
            output_target: request.output_target,
            module: request.module_name,
            target_name: request.target_name,
            node_names: request.node_names,
            root_task_paths: request
              .root_task_ids
              .into_iter()
              .map(|id| self.graph.task(id).descriptor.path.clone())
              .collect(),
            cutoff_task_roots: request
              .cutoff_task_ids
              .into_iter()
              .map(|id| self.native_task_root(id))
              .collect(),
          }
        })
        .collect(),
    )
  }

  fn native_task_root(&self, id: RuntimeTaskId) -> RuntimeNativeTaskRoot {
    let task = &self.graph.task(id).descriptor;
    let (target_name, task_name) = task.task_name.split_once('@').map_or_else(
      || (None, task.task_name.clone()),
      |(target, action)| (Some(target.to_string()), action.to_string()),
    );
    RuntimeNativeTaskRoot {
      module_name: task.node_name.clone(),
      target_name,
      task_name,
    }
  }

  fn is_native_action(action_name: &str) -> bool {
    matches!(
      action_name,
      "test"
        | "genOnDeviceTestHap"
        | "assembleDevHqf"
        | "ColdReloadArkTS"
        | "PrepareQuickfix"
        | "PackageHqf"
        | "SignHqf"
        | "HotReloadBuild"
        | "buildHotReloadResource"
        | "HotReloadArkTS"
        | "PreviewBuild"
        | "PreviewArkTS"
        | "PreviewUpdateAssets"
        | "buildPreviewerResource"
        | "ReplacePreviewerPage"
        | "PreviewHookCompileResource"
        | "PreviewCompileResource"
        | "PreviewProcessResource"
        | "CopyPreviewProfile"
        | "onDeviceTest"
        | "GenerateDeviceCoverage"
        | "BeforeGenerateDeviceCoverage"
        | "UnitTestBuild"
        | "GenerateUnitTestResult"
        | "UnitTestArkTS"
        | "UnitTestCopyMockConfigJson"
        | "ReplaceSourceMapInCoverageMode"
        | "BuildUnitTestHook"
        | "GenerateUnitTestTemplate"
        | "GenerateOhosTestTemplate"
        | "OhosTestCompileArkTS"
        | "OhosTestCompileJS"
        | "OhosTestBuildArkTS"
        | "OhosTestBuildJS"
        | "OhosTestCopyMockConfigJson"
        | "UnitTestProcessProfile"
        | "ReplaceUnitTestIndexFile"
        | "PreBuild"
        | "CreateModuleInfo"
        | "CreateHarModuleInfo"
        | "GenerateMetadata"
        | "ConfigureCmake"
        | "MergeProfile"
        | "CreateBuildProfile"
        | "CreateHarBuildProfile"
        | "GenerateBuildProfile"
        | "compileNative"
        | "PreCheckSyscap"
        | "GeneratePkgContextInfo"
        | "GeneratePkgSdkInfo"
        | "ProcessModulePrivacyProfile"
        | "ProcessProfile"
        | "ProcessRouterMap"
        | "ProcessShareConfig"
        | "ProcessStartupConfig"
        | "ProcessResource"
        | "CompileResource"
        | "GenerateLoaderJson"
        | "BuildArkTS"
        | "BuildJS"
        | "CompileArkTS"
        | "CompileJS"
        | "HarCompileArkTS"
        | "HarBuildArkTS"
        | "ProcessObfuscationFiles"
        | "ProcessOHPackageJson"
        | "ProcessPackageJson"
        | "ProcessHarArtifacts"
        | "SyscapTransform"
        | "MakePackInfo"
        | "ProcessIntegratedHsp"
        | "SignProjectRemoteHsp"
        | "SignModuleRemoteHsp"
        | "BuildNativeWithCmake"
        | "BuildNativeWithNinja"
        | "ProcessLibs"
        | "DoNativeBinxo"
        | "DoNativeStrip"
        | "CacheNativeLibs"
        | "GeneratePkgModuleJson"
        | "ProcessCompiledResources"
        | "ProcessIdDefinedFile"
        | "CollectDuplicateHarEntriesArkTS"
        | "PrepareSharedHarResource"
        | "PackageSharedHar"
        | "FastPackageHap"
        | "FastPackageHsp"
        | "PackageHap"
        | "PackageHsp"
        | "PackageHar"
        | "PackingCheck"
        | "PackingCheckHar"
        | "SignHap"
        | "SignHsp"
        | "PackageSignHar"
        | "PackageSharedTgz"
        | "LintArkTS"
        | "CollectDebugSymbol"
        | "PreBuildApp"
        | "DuplicateDependencyCheck"
        | "MakeProjectPackInfo"
        | "ProcessProjectPrivacyProfile"
        | "GeneratePackRes"
        | "PackageApp"
        | "SignPackagesFromApp"
        | "SignApp"
    )
  }

  fn native_output_target(
    action_name: &str,
    node_kind: RuntimeTaskNodeKind,
  ) -> Option<RuntimeNativeOutputTarget> {
    // HSP/HAR plugins intentionally register PackageHap/SignHap plus the
    // targetless genOnDeviceTestHap hook. Their actions construct a HAP even
    // though the owning node normally produces HSP/HAR artifacts
    // (`TaskInitializer::{hspTasksInitialize,harTasksInitialize}`).
    if node_kind != RuntimeTaskNodeKind::Hap
      && matches!(action_name, "genOnDeviceTestHap" | "PackageHap" | "SignHap")
    {
      return Some(RuntimeNativeOutputTarget::Hap);
    }
    if matches!(
      action_name,
      "PreviewBuild"
        | "PreviewArkTS"
        | "PreviewUpdateAssets"
        | "buildPreviewerResource"
        | "ReplacePreviewerPage"
        | "PreviewHookCompileResource"
        | "PreviewCompileResource"
        | "PreviewProcessResource"
        | "CopyPreviewProfile"
    ) {
      return Some(RuntimeNativeOutputTarget::Preview);
    }
    matches!(
      action_name,
      "test"
        | "UnitTestBuild"
        | "GenerateUnitTestResult"
        | "UnitTestArkTS"
        | "UnitTestCopyMockConfigJson"
        | "ReplaceSourceMapInCoverageMode"
        | "BuildUnitTestHook"
        | "GenerateUnitTestTemplate"
        | "UnitTestProcessProfile"
        | "ReplaceUnitTestIndexFile"
    )
    .then_some(RuntimeNativeOutputTarget::Test)
  }
}
