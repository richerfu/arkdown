use indexmap::IndexMap;
use oxc_index::{Idx, IndexVec};
use rustc_hash::FxHashSet;
use serde::Deserialize;

use super::catalog::RuntimeTaskNodeKind;
use super::task_info::HvigorTaskInfoCatalog;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct RuntimeTaskReportId(u32);

impl Idx for RuntimeTaskReportId {
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

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
enum RuntimeTaskReportKind {
  Tasks,
  TaskTree,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeTaskReportRequest {
  node_name: String,
  project_task: bool,
  #[serde(default)]
  kind: Option<RuntimeTaskNodeKind>,
  report_kind: RuntimeTaskReportKind,
  tasks: Vec<RuntimeTaskReportTask>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeTaskReportTask {
  node_name: String,
  task_name: String,
  built_in: bool,
  project_task: bool,
  #[serde(default)]
  realized: bool,
  #[serde(default)]
  group: String,
  #[serde(default)]
  description: String,
  #[serde(default)]
  catalog_order: Option<u32>,
  #[serde(default)]
  realization_order: u32,
  #[serde(default)]
  dependencies: Vec<String>,
  #[serde(default)]
  user_dependencies: Vec<String>,
}

#[derive(Debug, Clone)]
struct RuntimeTaskReportDetail {
  name: String,
  group: String,
  description: String,
}

pub(crate) struct RuntimeTaskReportRenderer {
  request: RuntimeTaskReportRequest,
  tasks: IndexVec<RuntimeTaskReportId, RuntimeTaskReportTask>,
}

impl RuntimeTaskReportRenderer {
  pub fn from_json(request_json: &str) -> anyhow::Result<Self> {
    let request: RuntimeTaskReportRequest = serde_json::from_str(request_json)?;
    let tasks = request.tasks.iter().cloned().collect();
    Ok(Self { request, tasks })
  }

  pub fn render(&self) -> anyhow::Result<String> {
    match self.request.report_kind {
      RuntimeTaskReportKind::Tasks => self.render_tasks(),
      RuntimeTaskReportKind::TaskTree => self.render_task_tree(),
    }
  }

  fn render_tasks(&self) -> anyhow::Result<String> {
    let mut details = IndexMap::<String, RuntimeTaskReportDetail>::new();
    let baseline_names: &[&str] =
      if self.request.project_task && self.request.kind == Some(RuntimeTaskNodeKind::App) {
        HvigorTaskInfoCatalog::project_task_names()
      } else {
        &["tasks", "taskTree", "init"]
      };
    for name in baseline_names {
      let detail = self.metadata_detail(name)?;
      details.insert(detail.name.clone(), detail);
    }

    let mut realized = self.node_task_ids();
    realized.retain(|id| self.tasks[*id].realized);
    realized.sort_by_key(|id| (self.tasks[*id].realization_order, id.index()));
    for id in realized {
      let task = &self.tasks[id];
      details
        .entry(task.task_name.clone())
        .or_insert_with(|| self.task_detail(task));
    }

    let mut groups = IndexMap::<String, Vec<RuntimeTaskReportDetail>>::new();
    for detail in details.into_values() {
      groups.entry(detail.group.clone()).or_default().push(detail);
    }
    let mut lines = vec![
      "--------------------------------------------------------------------------".to_string(),
      format!("All tasks from hvigor node '{}'", self.request.node_name),
      "--------------------------------------------------------------------------".to_string(),
    ];
    for (group, tasks) in groups {
      lines.push(format!("{group} tasks"));
      lines.push("-------------".to_string());
      for task in tasks {
        if task.description.is_empty() {
          lines.push(task.name);
        } else {
          lines.push(format!("{} - {}", task.name, task.description));
        }
      }
      lines.push(String::new());
    }
    Ok(lines.join(self.line_ending()))
  }

  fn render_task_tree(&self) -> anyhow::Result<String> {
    let included = self.task_tree_tasks();
    let mut referenced = FxHashSet::default();
    for id in included.values().copied() {
      for dependency in self.tasks[id]
        .dependencies
        .iter()
        .chain(self.tasks[id].user_dependencies.iter())
      {
        let (dependency_node, dependency_name) = self.dependency_parts(dependency);
        if self.is_same_node(dependency_node) {
          referenced.insert(dependency_name.to_string());
        }
      }
    }

    let roots = included
      .iter()
      .filter_map(|(name, id)| {
        let task = &self.tasks[*id];
        (!referenced.contains(name) && task.group != "Help" && task.group != "Sync").then_some(*id)
      })
      .collect::<Vec<_>>();
    let mut lines = vec![
      "--------------------------------------------------------------------------".to_string(),
      format!(
        "All tasks dependencies from hvigor node '{}'",
        self.request.node_name
      ),
      "--------------------------------------------------------------------------".to_string(),
      String::new(),
    ];
    for root in roots {
      lines.push(format!(
        ":{}:{}",
        self.request.node_name, self.tasks[root].task_name
      ));
      let mut visiting = FxHashSet::default();
      visiting.insert(root);
      self.render_dependencies(root, &included, 0, &mut visiting, &mut lines);
    }
    Ok(lines.join(self.line_ending()))
  }

  fn task_tree_tasks(&self) -> IndexMap<String, RuntimeTaskReportId> {
    let node_tasks = self.node_task_ids();
    let mut included = IndexMap::new();
    if self.request.project_task && self.request.kind == Some(RuntimeTaskNodeKind::App) {
      if let Some(id) = node_tasks
        .iter()
        .find(|id| self.tasks[**id].task_name == "assembleApp")
        .copied()
      {
        included.insert("assembleApp".to_string(), id);
      }
    } else {
      let mut catalog_tasks = node_tasks
        .iter()
        .filter(|id| self.tasks[**id].catalog_order.is_some())
        .copied()
        .collect::<Vec<_>>();
      catalog_tasks.sort_by_key(|id| {
        (
          self.tasks[*id].catalog_order.unwrap_or(u32::MAX),
          id.index(),
        )
      });
      for id in catalog_tasks {
        included.insert(self.tasks[id].task_name.clone(), id);
      }
    }

    let mut realized = node_tasks
      .into_iter()
      .filter(|id| self.tasks[*id].realized)
      .collect::<Vec<_>>();
    realized.sort_by_key(|id| (self.tasks[*id].realization_order, id.index()));
    for id in realized {
      let task = &self.tasks[id];
      if self.request.project_task
        && task.built_in
        && matches!(task.task_name.as_str(), "tasks" | "taskTree" | "prune")
      {
        continue;
      }
      included.insert(task.task_name.clone(), id);
    }
    included
  }

  fn render_dependencies(
    &self,
    task_id: RuntimeTaskReportId,
    included: &IndexMap<String, RuntimeTaskReportId>,
    depth: usize,
    visiting: &mut FxHashSet<RuntimeTaskReportId>,
    lines: &mut Vec<String>,
  ) {
    for dependency in self.tasks[task_id]
      .dependencies
      .iter()
      .chain(self.tasks[task_id].user_dependencies.iter())
    {
      let (dependency_node, dependency_name) = self.dependency_parts(dependency);
      let indent = "    ".repeat(depth);
      if !self.is_same_node(dependency_node) {
        lines.push(format!("{indent}\\--- :{dependency}"));
        continue;
      }
      let child = included.get(dependency_name).copied();
      let rendered_name = if self.request.project_task && dependency.starts_with(':') {
        dependency.as_str()
      } else {
        dependency_name
      };
      let Some(child) = child else {
        lines.push(format!("{indent}\\--- :{rendered_name}"));
        continue;
      };
      let child_has_dependencies = !self.tasks[child].dependencies.is_empty()
        || !self.tasks[child].user_dependencies.is_empty();
      if !child_has_dependencies || visiting.contains(&child) {
        lines.push(format!("{indent}\\--- :{rendered_name}"));
        continue;
      }
      lines.push(format!("{indent}+--- :{rendered_name}"));
      visiting.insert(child);
      self.render_dependencies(child, included, depth + 1, visiting, lines);
      visiting.remove(&child);
    }
  }

  fn node_task_ids(&self) -> Vec<RuntimeTaskReportId> {
    self
      .tasks
      .iter_enumerated()
      .filter_map(|(id, task)| {
        (task.node_name == self.request.node_name && task.project_task == self.request.project_task)
          .then_some(id)
      })
      .collect()
  }

  fn metadata_detail(&self, task_name: &str) -> anyhow::Result<RuntimeTaskReportDetail> {
    let task_info = HvigorTaskInfoCatalog::task_info(task_name)?;
    Ok(RuntimeTaskReportDetail {
      name: task_name.to_string(),
      group: task_info.group,
      description: task_info.description,
    })
  }

  fn task_detail(&self, task: &RuntimeTaskReportTask) -> RuntimeTaskReportDetail {
    RuntimeTaskReportDetail {
      name: task.task_name.clone(),
      group: if task.group.is_empty() {
        "Customize".to_string()
      } else {
        task.group.clone()
      },
      description: task.description.clone(),
    }
  }

  fn dependency_parts<'a>(&'a self, dependency: &'a str) -> (&'a str, &'a str) {
    dependency
      .split_once(':')
      .unwrap_or((&self.request.node_name, dependency))
  }

  fn is_same_node(&self, dependency_node: &str) -> bool {
    dependency_node == self.request.node_name
      || (self.request.project_task && dependency_node.is_empty())
  }

  fn line_ending(&self) -> &'static str {
    if cfg!(windows) {
      "\r\n"
    } else {
      "\n"
    }
  }
}
