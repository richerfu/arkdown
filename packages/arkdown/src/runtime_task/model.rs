use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};

use super::catalog::{RuntimeTaskCatalog, RuntimeTaskCatalogRequest};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeTaskDescriptor {
  pub path: String,
  pub node_name: String,
  pub task_name: String,
  pub built_in: bool,
  pub project_task: bool,
  #[serde(default)]
  pub lazy: bool,
  pub enabled: bool,
  pub track_state: bool,
  pub refresh_declarations: bool,
  pub has_before_run: bool,
  pub has_after_run: bool,
  pub has_failed: bool,
  pub has_task_should_do: bool,
  pub graph_bound_custom: bool,
  #[serde(default)]
  pub dependencies: Vec<String>,
  #[serde(default)]
  pub user_dependencies: Vec<String>,
  #[serde(default)]
  pub input_properties: Vec<RuntimeTaskProperty>,
  #[serde(default)]
  pub input_files: Vec<RuntimeTaskFileDeclaration>,
  #[serde(default)]
  pub output_files: Vec<RuntimeTaskFileDeclaration>,
}

impl RuntimeTaskDescriptor {
  pub fn is_incremental(&self) -> bool {
    !self.input_properties.is_empty()
      || !self.input_files.is_empty()
      || !self.output_files.is_empty()
  }

  pub(crate) fn normalize_declared_paths(&mut self, project_root: &Path, case_insensitive: bool) {
    Self::normalize_path_list(&mut self.input_files, project_root, case_insensitive);
    Self::normalize_path_list(&mut self.output_files, project_root, case_insensitive);
  }

  fn normalize_path_list(
    paths: &mut Vec<RuntimeTaskFileDeclaration>,
    project_root: &Path,
    case_insensitive: bool,
  ) {
    let mut normalized = BTreeMap::new();
    for mut declaration in std::mem::take(paths) {
      declaration.normalize_against(project_root);
      let value = declaration.path().to_string();
      normalized
        .entry(RuntimeTaskSelection::path_identity(
          &value,
          case_insensitive,
        ))
        .or_insert(declaration);
    }
    *paths = normalized.into_values().collect();
  }
}

/// Expands Rust-owned built-in task catalogs and applies the comparatively
/// small set of JavaScript task overlays. The V8 boundary never needs to
/// serialize the complete built-in graph back to Rust.
pub struct RuntimeTaskModelBuilder {
  catalogs: Vec<RuntimeTaskCatalogRequest>,
  overlays: Vec<RuntimeTaskDescriptor>,
  removed_task_paths: FxHashSet<String>,
}

impl RuntimeTaskModelBuilder {
  pub fn new(
    catalogs: Vec<RuntimeTaskCatalogRequest>,
    overlays: Vec<RuntimeTaskDescriptor>,
    removed_task_paths: Vec<String>,
  ) -> Self {
    Self {
      catalogs,
      overlays,
      removed_task_paths: removed_task_paths.into_iter().collect(),
    }
  }

  pub fn build(self) -> anyhow::Result<Vec<RuntimeTaskDescriptor>> {
    let mut descriptors = Vec::new();
    let mut built_in_indexes = FxHashMap::default();
    for request in self.catalogs {
      let node_name = request.node_name.clone();
      let project_task = request.project_task;
      for entry in RuntimeTaskCatalog::new(request).entries()? {
        let path = if project_task {
          format!(":{}", entry.name)
        } else {
          format!("{node_name}:{}", entry.name)
        };
        if self.removed_task_paths.contains(&path) {
          continue;
        }
        let index = descriptors.len();
        built_in_indexes.insert(path.clone(), index);
        descriptors.push(RuntimeTaskDescriptor {
          path,
          node_name: node_name.clone(),
          task_name: entry.name,
          built_in: true,
          project_task,
          lazy: false,
          enabled: true,
          track_state: true,
          refresh_declarations: false,
          has_before_run: false,
          has_after_run: false,
          has_failed: false,
          has_task_should_do: false,
          graph_bound_custom: false,
          dependencies: entry.dependencies,
          user_dependencies: Vec::new(),
          input_properties: Vec::new(),
          input_files: Vec::new(),
          output_files: Vec::new(),
        });
      }
    }

    for mut overlay in self.overlays {
      if self.removed_task_paths.contains(&overlay.path) {
        continue;
      }
      Self::qualify_overlay_dependencies(&mut overlay);
      if overlay.built_in {
        if let Some(index) = built_in_indexes.get(&overlay.path).copied() {
          descriptors[index] = overlay;
          continue;
        }
      }
      descriptors.push(overlay);
    }
    descriptors.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(descriptors)
  }

  fn qualify_overlay_dependencies(overlay: &mut RuntimeTaskDescriptor) {
    for dependency in overlay
      .dependencies
      .iter_mut()
      .chain(overlay.user_dependencies.iter_mut())
    {
      if dependency.contains(':') {
        continue;
      }
      *dependency = if overlay.project_task {
        format!(":{dependency}")
      } else {
        format!("{}:{dependency}", overlay.node_name)
      };
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeTaskProperty {
  pub name: String,
  pub value: serde_json::Value,
  pub value_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RuntimeTaskFileDeclaration {
  Path(String),
  Config(RuntimeTaskFileConfig),
}

impl RuntimeTaskFileDeclaration {
  pub fn path(&self) -> &str {
    match self {
      Self::Path(path) => path,
      Self::Config(config) => &config.path,
    }
  }

  pub(crate) fn normalize_against(&mut self, project_root: &Path) {
    let path = Path::new(self.path());
    let normalized = if path.is_absolute() {
      arkdown_path::lexical_normalize(path)
    } else {
      arkdown_path::lexical_normalize(&project_root.join(path))
    };
    self.set_path(normalized.to_string_lossy().into_owned());
  }

  fn set_path(&mut self, path: String) {
    match self {
      Self::Path(value) => *value = path,
      Self::Config(config) => config.path = path,
    }
  }

  pub fn filter(&self) -> Option<&RuntimeTaskFileFilter> {
    match self {
      Self::Path(_) => None,
      Self::Config(config) => config.test.as_ref(),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeTaskFileConfig {
  pub path: String,
  #[serde(default)]
  pub is_directory: Option<bool>,
  #[serde(default)]
  pub test: Option<RuntimeTaskFileFilter>,
  #[serde(default)]
  pub depth: Option<u32>,
  #[serde(default)]
  pub file_snap_shot_hash_value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeTaskFileFilter {
  pub source: String,
  #[serde(default)]
  pub flags: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeTaskSelection {
  pub project_root: PathBuf,
  pub cache_enabled: bool,
  #[serde(default)]
  pub expected_root_paths: Option<Vec<String>>,
  #[serde(default)]
  pub required_boundary_paths: Vec<String>,
  #[serde(default)]
  pub command_root_paths: Vec<String>,
  #[serde(default)]
  pub execution_node_names: Vec<String>,
  #[serde(default = "RuntimeTaskSelection::default_validate_boundaries")]
  pub validate_boundaries: bool,
}

impl RuntimeTaskSelection {
  const fn default_validate_boundaries() -> bool {
    true
  }

  pub fn normalize_declared_paths(&self, tasks: &mut [RuntimeTaskDescriptor]) {
    for task in tasks {
      task.normalize_declared_paths(&self.project_root, cfg!(windows));
    }
  }

  pub fn declared_input_paths(&self, tasks: &[RuntimeTaskDescriptor]) -> Vec<String> {
    let mut paths = BTreeMap::new();
    for task in tasks.iter().filter(|task| !task.built_in) {
      for input in &task.input_files {
        let identity = Self::path_identity(input.path(), cfg!(windows));
        paths
          .entry(identity)
          .or_insert_with(|| input.path().to_string());
      }
    }
    paths.into_values().collect()
  }

  fn path_identity(path: &str, case_insensitive: bool) -> String {
    if case_insensitive {
      path.to_lowercase()
    } else {
      path.to_string()
    }
  }
}

#[derive(Debug, Clone, Copy)]
pub enum RuntimeTaskOperationPhase {
  BeforeAlways,
  Realize,
  TaskShouldDo,
  BeforeRun,
  Run,
  AfterRun,
  Failed,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeTaskCallbackResult {
  pub enabled: Option<bool>,
  pub track_state: Option<bool>,
  pub refresh_declarations: Option<bool>,
  pub has_before_run: Option<bool>,
  pub has_after_run: Option<bool>,
  pub has_failed: Option<bool>,
  pub has_task_should_do: Option<bool>,
  pub task_should_do: Option<bool>,
  pub input_properties: Option<Vec<RuntimeTaskProperty>>,
  pub input_files: Option<Vec<RuntimeTaskFileDeclaration>>,
  pub output_files: Option<Vec<RuntimeTaskFileDeclaration>>,
  #[serde(default)]
  pub output: Vec<String>,
}

#[cfg(test)]
mod tests {
  use super::*;

  fn task(path: &str, built_in: bool, input_files: Vec<String>) -> RuntimeTaskDescriptor {
    RuntimeTaskDescriptor {
      path: path.to_string(),
      node_name: "entry".to_string(),
      task_name: path.rsplit(':').next().unwrap().to_string(),
      built_in,
      project_task: false,
      lazy: false,
      enabled: true,
      track_state: true,
      refresh_declarations: false,
      has_before_run: false,
      has_after_run: false,
      has_failed: false,
      has_task_should_do: false,
      graph_bound_custom: false,
      dependencies: Vec::new(),
      user_dependencies: Vec::new(),
      input_properties: Vec::new(),
      input_files: input_files
        .into_iter()
        .map(RuntimeTaskFileDeclaration::Path)
        .collect(),
      output_files: Vec::new(),
    }
  }

  #[test]
  fn normalizes_declared_paths_and_applies_windows_identity_rules() {
    let root = tempfile::tempdir().unwrap();
    let selection = RuntimeTaskSelection {
      project_root: root.path().to_path_buf(),
      cache_enabled: true,
      expected_root_paths: None,
      required_boundary_paths: Vec::new(),
      command_root_paths: Vec::new(),
      execution_node_names: Vec::new(),
      validate_boundaries: true,
    };
    let mut tasks = vec![
      task(
        "entry:custom",
        false,
        vec![
          "src/../Input.ts".to_string(),
          "input.ts".to_string(),
          "assets/./icon.png".to_string(),
        ],
      ),
      task("entry:builtIn", true, vec!["ignored.txt".to_string()]),
    ];

    for task in &mut tasks {
      task.normalize_declared_paths(&selection.project_root, true);
    }

    assert_eq!(tasks[0].input_files.len(), 2);
    assert_eq!(
      tasks[0].input_files,
      vec![
        RuntimeTaskFileDeclaration::Path(
          root
            .path()
            .join("assets/icon.png")
            .to_string_lossy()
            .into_owned(),
        ),
        RuntimeTaskFileDeclaration::Path(
          root.path().join("Input.ts").to_string_lossy().into_owned(),
        ),
      ]
    );
    let mut expected_inputs = tasks[0]
      .input_files
      .iter()
      .map(|declaration| declaration.path().to_string())
      .collect::<Vec<_>>();
    expected_inputs.sort_by_key(|path| RuntimeTaskSelection::path_identity(path, cfg!(windows)));
    assert_eq!(selection.declared_input_paths(&tasks), expected_inputs);
  }

  #[test]
  fn expands_builtin_catalogs_and_applies_only_javascript_overlays() {
    let catalog = serde_json::from_value(serde_json::json!({
      "nodeName": "entry",
      "kind": "hap",
      "projectTask": false,
      "buildTargetName": "default",
      "targetNames": ["default"],
      "targetOptions": [{
        "name": "default",
        "compileApiVersion": 12,
        "compatibleApiVersion": 12
      }]
    }))
    .unwrap();
    let mut tasks_overlay = task("entry:tasks", true, Vec::new());
    tasks_overlay.enabled = false;
    let custom_overlay = task("entry:custom", false, Vec::new());

    let model = RuntimeTaskModelBuilder::new(
      vec![catalog],
      vec![tasks_overlay, custom_overlay],
      vec!["entry:default@CollectDebugSymbol".to_string()],
    )
    .build()
    .unwrap();

    assert!(
      model.len() > 70,
      "Rust must expand the complete HAP catalog"
    );
    assert!(model
      .iter()
      .any(|task| task.path == "entry:custom" && !task.built_in));
    assert!(model
      .iter()
      .any(|task| task.path == "entry:tasks" && !task.enabled));
    assert!(!model
      .iter()
      .any(|task| task.path == "entry:default@CollectDebugSymbol"));
  }
}
