use std::collections::BTreeSet;

use async_trait::async_trait;
use rustc_hash::FxHashSet;

use super::cache::{RuntimeTaskCache, RuntimeTaskCacheError};
use super::graph::{RuntimeTaskGraph, RuntimeTaskGraphError, RuntimeTaskId};
use super::model::{
  RuntimeTaskCallbackResult, RuntimeTaskDescriptor, RuntimeTaskOperationPhase, RuntimeTaskSelection,
};

#[async_trait]
pub trait RuntimeTaskCallback: Send {
  async fn invoke_task(
    &mut self,
    task_path: &str,
    phase: RuntimeTaskOperationPhase,
    error: Option<String>,
  ) -> anyhow::Result<RuntimeTaskCallbackResult>;

  async fn transform_abc(&mut self, event: serde_json::Value) -> anyhow::Result<()>;

  async fn modify_native_command(
    &mut self,
    event: serde_json::Value,
  ) -> anyhow::Result<Vec<String>>;
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeTaskEngineError {
  #[error(transparent)]
  Graph(#[from] RuntimeTaskGraphError),
  #[error(transparent)]
  Cache(#[from] RuntimeTaskCacheError),
  #[error("runtime task callback {phase} for {task} failed: {source}")]
  Callback {
    phase: &'static str,
    task: String,
    #[source]
    source: anyhow::Error,
  },
  #[error("native task {node_name}:{task_name} is not present in the selected runtime task graph")]
  MissingNativeTask {
    node_name: String,
    task_name: String,
  },
  #[error("native build did not execute task boundaries required by TypeScript plugins: {0}")]
  BoundariesNotExecuted(String),
}

pub struct RuntimeTaskEngine {
  graph: RuntimeTaskGraph,
  callback: Box<dyn RuntimeTaskCallback>,
  cache: RuntimeTaskCache,
  expected: Option<FxHashSet<RuntimeTaskId>>,
  required_boundaries: FxHashSet<RuntimeTaskId>,
  command_roots: Vec<RuntimeTaskId>,
  execution_node_names: FxHashSet<String>,
  validate_boundaries: bool,
  completed: FxHashSet<RuntimeTaskId>,
  active_built_ins: FxHashSet<RuntimeTaskId>,
  command_output: Vec<String>,
}

impl RuntimeTaskEngine {
  pub fn new(
    descriptors: Vec<RuntimeTaskDescriptor>,
    selection: RuntimeTaskSelection,
    callback: Box<dyn RuntimeTaskCallback>,
  ) -> Result<Self, RuntimeTaskEngineError> {
    let graph = RuntimeTaskGraph::new(descriptors)?;
    let expected = selection
      .expected_root_paths
      .as_ref()
      .map(|paths| {
        graph
          .ids_for_paths(paths)
          .and_then(|roots| graph.dependency_closure(&roots))
      })
      .transpose()?;
    let required_boundaries = graph
      .ids_for_paths(&selection.required_boundary_paths)?
      .into_iter()
      .collect();
    let command_roots = graph.ids_for_paths(&selection.command_root_paths)?;
    let cache = RuntimeTaskCache::new(&selection.project_root, selection.cache_enabled);
    Ok(Self {
      graph,
      callback,
      cache,
      expected,
      required_boundaries,
      command_roots,
      execution_node_names: selection.execution_node_names.into_iter().collect(),
      validate_boundaries: selection.validate_boundaries,
      completed: FxHashSet::default(),
      active_built_ins: FxHashSet::default(),
      command_output: Vec::new(),
    })
  }

  pub fn begin_cycle(&mut self) {
    self.completed.clear();
    self.active_built_ins.clear();
    self.command_output.clear();
    self.cache.begin_cycle();
  }

  pub async fn finish_cycle(&mut self) -> Result<(), RuntimeTaskEngineError> {
    self.drain_ready_roots().await?;
    if self.validate_boundaries {
      self.validate_completed_boundaries()?;
    }
    self.cache.finish_cycle()?;
    Ok(())
  }

  pub async fn before_native_task(
    &mut self,
    node_name: &str,
    target_name: Option<&str>,
    task_name: &str,
  ) -> Result<bool, RuntimeTaskEngineError> {
    let Some(id) = self.event_task_id(node_name, target_name, task_name)? else {
      return Ok(false);
    };
    if self.completed.contains(&id) {
      return Ok(false);
    }
    self.execute_user_dependencies(id).await?;
    let task = self.graph.task(id).descriptor.clone();
    if !task.enabled {
      self.completed.insert(id);
      return Ok(false);
    }
    if task.has_before_run {
      self
        .invoke_task(id, RuntimeTaskOperationPhase::BeforeRun)
        .await?;
    }
    self.active_built_ins.insert(id);
    Ok(true)
  }

  pub async fn after_native_task(
    &mut self,
    node_name: &str,
    target_name: Option<&str>,
    task_name: &str,
  ) -> Result<(), RuntimeTaskEngineError> {
    let Some(id) = self.event_task_id(node_name, target_name, task_name)? else {
      return Ok(());
    };
    if self.active_built_ins.remove(&id) {
      if self.graph.task(id).descriptor.has_after_run {
        self
          .invoke_task(id, RuntimeTaskOperationPhase::AfterRun)
          .await?;
      }
      self.completed.insert(id);
      self.drain_ready_roots().await?;
    }
    Ok(())
  }

  pub async fn native_task_up_to_date(
    &mut self,
    node_name: &str,
    target_name: Option<&str>,
    task_name: &str,
  ) -> Result<(), RuntimeTaskEngineError> {
    let Some(id) = self.event_task_id(node_name, target_name, task_name)? else {
      return Ok(());
    };
    self.execute_user_dependencies(id).await?;
    self.completed.insert(id);
    self.drain_ready_roots().await
  }

  pub async fn transform_abc(
    &mut self,
    native_event: serde_json::Value,
  ) -> Result<(), RuntimeTaskEngineError> {
    self
      .callback
      .transform_abc(native_event)
      .await
      .map_err(|source| RuntimeTaskEngineError::Callback {
        phase: "transformAbc",
        task: "<native-abc>".to_string(),
        source,
      })
  }

  pub async fn modify_native_command(
    &mut self,
    native_event: serde_json::Value,
  ) -> Result<Vec<String>, RuntimeTaskEngineError> {
    self
      .callback
      .modify_native_command(native_event)
      .await
      .map_err(|source| RuntimeTaskEngineError::Callback {
        phase: "modifyNativeCommand",
        task: "<native-command>".to_string(),
        source,
      })
  }

  pub async fn execute_command_roots(&mut self) -> Result<Vec<String>, RuntimeTaskEngineError> {
    self.begin_cycle();
    let roots = self.command_roots.clone();
    for root in roots {
      self.execute_custom(root).await?;
    }
    self.cache.finish_cycle()?;
    Ok(std::mem::take(&mut self.command_output))
  }

  fn event_task_id(
    &self,
    node_name: &str,
    target_name: Option<&str>,
    task_name: &str,
  ) -> Result<Option<RuntimeTaskId>, RuntimeTaskEngineError> {
    let task_name = target_name.map_or_else(
      || task_name.to_string(),
      |target_name| format!("{target_name}@{task_name}"),
    );
    let id = self.graph.id_for_event(node_name, &task_name);
    let Some(id) = id else {
      if self.expected.is_some() {
        return Ok(None);
      }
      return Err(RuntimeTaskEngineError::MissingNativeTask {
        node_name: node_name.to_string(),
        task_name,
      });
    };
    if self
      .expected
      .as_ref()
      .is_some_and(|expected| !expected.contains(&id))
    {
      return Ok(None);
    }
    Ok(Some(id))
  }

  async fn execute_user_dependencies(
    &mut self,
    id: RuntimeTaskId,
  ) -> Result<(), RuntimeTaskEngineError> {
    let dependencies = self.graph.task(id).user_dependencies.clone();
    for dependency in dependencies {
      if self.graph.task(dependency).descriptor.built_in {
        return Err(
          RuntimeTaskGraphError::InvalidPostDependency {
            task: self.graph.task(id).descriptor.path.clone(),
            dependency: self.graph.task(dependency).descriptor.path.clone(),
          }
          .into(),
        );
      }
      self.execute_custom(dependency).await?;
    }
    Ok(())
  }

  async fn drain_ready_roots(&mut self) -> Result<(), RuntimeTaskEngineError> {
    let roots = self.command_roots.clone();
    for root in roots {
      if self.graph.custom_task_is_ready(root, &self.completed)? {
        self.execute_custom(root).await?;
      }
    }
    Ok(())
  }

  async fn execute_custom(&mut self, root: RuntimeTaskId) -> Result<(), RuntimeTaskEngineError> {
    let plan = self.graph.custom_execution_plan(root, &self.completed)?;
    for id in plan {
      self.execute_custom_action(id).await?;
    }
    Ok(())
  }

  async fn execute_custom_action(
    &mut self,
    id: RuntimeTaskId,
  ) -> Result<(), RuntimeTaskEngineError> {
    if self.completed.contains(&id) {
      return Ok(());
    }
    self.realize_lazy_task(id).await?;
    let task = self.graph.task(id).descriptor.clone();
    if !task.enabled {
      self.completed.insert(id);
      return Ok(());
    }
    let result = self.execute_enabled_custom_action(id).await;
    if let Err(error) = result {
      if task.has_failed {
        let _ = self.invoke_failed(id, error.to_string()).await;
      }
      return Err(error);
    }
    self.completed.insert(id);
    Ok(())
  }

  async fn realize_lazy_task(&mut self, id: RuntimeTaskId) -> Result<(), RuntimeTaskEngineError> {
    if !self.graph.task(id).descriptor.lazy {
      return Ok(());
    }
    let update = self
      .invoke_task(id, RuntimeTaskOperationPhase::Realize)
      .await?;
    let descriptor = &mut self.graph.task_mut(id).descriptor;
    descriptor.lazy = false;
    if let Some(enabled) = update.enabled {
      descriptor.enabled = enabled;
    }
    if let Some(track_state) = update.track_state {
      descriptor.track_state = track_state;
    }
    if let Some(refresh_declarations) = update.refresh_declarations {
      descriptor.refresh_declarations = refresh_declarations;
    }
    if let Some(has_before_run) = update.has_before_run {
      descriptor.has_before_run = has_before_run;
    }
    if let Some(has_after_run) = update.has_after_run {
      descriptor.has_after_run = has_after_run;
    }
    if let Some(has_failed) = update.has_failed {
      descriptor.has_failed = has_failed;
    }
    if let Some(has_task_should_do) = update.has_task_should_do {
      descriptor.has_task_should_do = has_task_should_do;
    }
    if let Some(properties) = update.input_properties {
      descriptor.input_properties = properties;
    }
    if let Some(files) = update.input_files {
      descriptor.input_files = files;
    }
    if let Some(files) = update.output_files {
      descriptor.output_files = files;
    }
    Ok(())
  }

  async fn execute_enabled_custom_action(
    &mut self,
    id: RuntimeTaskId,
  ) -> Result<(), RuntimeTaskEngineError> {
    let task = self.graph.task(id).descriptor.clone();
    if task.refresh_declarations {
      let update = self
        .invoke_task(id, RuntimeTaskOperationPhase::BeforeAlways)
        .await?;
      let descriptor = &mut self.graph.task_mut(id).descriptor;
      if let Some(properties) = update.input_properties {
        descriptor.input_properties = properties;
      }
      if let Some(files) = update.input_files {
        descriptor.input_files = files;
      }
      if let Some(files) = update.output_files {
        descriptor.output_files = files;
      }
    }
    let task = self.graph.task(id).descriptor.clone();
    if self.cache.is_up_to_date(&task)? {
      Self::emit_runtime_task_boundary(&task.path, true);
      self.task_should_record(id).await?;
      return Ok(());
    }
    Self::emit_runtime_task_boundary(&task.path, false);
    self.execute_custom_callbacks(id).await?;
    if self.task_should_record(id).await? {
      self
        .cache
        .record_successful(&self.graph.task(id).descriptor);
    }
    Ok(())
  }

  fn emit_runtime_task_boundary(task_path: &str, skipped: bool) {
    if let Some(observability) = crate::runtime_observe::RuntimeObservability::global() {
      let build_id = observability.build_id();
      tracing::info!(
        target: "arkdown_napi::runtime_task",
        task_path,
        skipped,
        build_id = %build_id,
        "runtime task boundary"
      );
    }
  }

  async fn execute_custom_callbacks(
    &mut self,
    id: RuntimeTaskId,
  ) -> Result<(), RuntimeTaskEngineError> {
    if self.graph.task(id).descriptor.has_before_run {
      self
        .invoke_task(id, RuntimeTaskOperationPhase::BeforeRun)
        .await?;
    }
    let result = self.invoke_task(id, RuntimeTaskOperationPhase::Run).await?;
    self.command_output.extend(result.output);
    if self.graph.task(id).descriptor.has_after_run {
      self
        .invoke_task(id, RuntimeTaskOperationPhase::AfterRun)
        .await?;
    }
    Ok(())
  }

  async fn task_should_record(
    &mut self,
    id: RuntimeTaskId,
  ) -> Result<bool, RuntimeTaskEngineError> {
    let task = &self.graph.task(id).descriptor;
    if !task.is_incremental() || !task.has_task_should_do {
      return Ok(true);
    }
    let result = self
      .invoke_task(id, RuntimeTaskOperationPhase::TaskShouldDo)
      .await?;
    Ok(result.task_should_do != Some(false))
  }

  async fn invoke_task(
    &mut self,
    id: RuntimeTaskId,
    phase: RuntimeTaskOperationPhase,
  ) -> Result<RuntimeTaskCallbackResult, RuntimeTaskEngineError> {
    let task_path = self.graph.task(id).descriptor.path.clone();
    let phase_name = Self::phase_name(phase);
    self
      .callback
      .invoke_task(&task_path, phase, None)
      .await
      .map_err(|source| RuntimeTaskEngineError::Callback {
        phase: phase_name,
        task: task_path,
        source,
      })
  }

  async fn invoke_failed(
    &mut self,
    id: RuntimeTaskId,
    error: String,
  ) -> Result<(), RuntimeTaskEngineError> {
    let task_path = self.graph.task(id).descriptor.path.clone();
    self
      .callback
      .invoke_task(&task_path, RuntimeTaskOperationPhase::Failed, Some(error))
      .await
      .map(|_| ())
      .map_err(|source| RuntimeTaskEngineError::Callback {
        phase: "failed",
        task: task_path,
        source,
      })
  }

  fn validate_completed_boundaries(&self) -> Result<(), RuntimeTaskEngineError> {
    let mut pending = BTreeSet::new();
    for (id, node) in self.graph.tasks() {
      let task = &node.descriptor;
      if !self.execution_node_names.is_empty()
        && !task.project_task
        && !self.execution_node_names.contains(&task.node_name)
      {
        continue;
      }
      if self
        .expected
        .as_ref()
        .is_some_and(|expected| !expected.contains(&id))
        || self.completed.contains(&id)
      {
        continue;
      }
      let has_lifecycle_hooks = task.has_before_run || task.has_after_run;
      if has_lifecycle_hooks || task.graph_bound_custom || self.required_boundaries.contains(&id) {
        pending.insert(task.path.clone());
      }
    }
    if pending.is_empty() {
      return Ok(());
    }
    Err(RuntimeTaskEngineError::BoundariesNotExecuted(
      pending.into_iter().collect::<Vec<_>>().join(", "),
    ))
  }

  pub(super) const fn phase_name(phase: RuntimeTaskOperationPhase) -> &'static str {
    match phase {
      RuntimeTaskOperationPhase::BeforeAlways => "beforeAlways",
      RuntimeTaskOperationPhase::Realize => "realize",
      RuntimeTaskOperationPhase::TaskShouldDo => "taskShouldDo",
      RuntimeTaskOperationPhase::BeforeRun => "beforeRun",
      RuntimeTaskOperationPhase::Run => "run",
      RuntimeTaskOperationPhase::AfterRun => "afterRun",
      RuntimeTaskOperationPhase::Failed => "failed",
    }
  }
}

#[cfg(test)]
mod tests {
  use std::collections::BTreeMap;
  use std::path::Path;
  use std::sync::{Arc, Mutex};

  use super::*;

  #[derive(Default)]
  struct RecordingCallback {
    operations: Arc<Mutex<Vec<String>>>,
    task_should_do: Option<bool>,
    fail_run_for: Option<String>,
    command_output: BTreeMap<String, Vec<String>>,
    realize_with_hooks: bool,
  }

  #[async_trait]
  impl RuntimeTaskCallback for RecordingCallback {
    async fn invoke_task(
      &mut self,
      task_path: &str,
      phase: RuntimeTaskOperationPhase,
      _error: Option<String>,
    ) -> anyhow::Result<RuntimeTaskCallbackResult> {
      self.operations.lock().unwrap().push(format!(
        "{}:{}",
        RuntimeTaskEngine::phase_name(phase),
        task_path
      ));
      if matches!(phase, RuntimeTaskOperationPhase::Run)
        && self.fail_run_for.as_deref() == Some(task_path)
      {
        anyhow::bail!("fixture run failed");
      }
      Ok(RuntimeTaskCallbackResult {
        enabled: matches!(phase, RuntimeTaskOperationPhase::Realize).then_some(true),
        has_before_run: (matches!(phase, RuntimeTaskOperationPhase::Realize)
          && self.realize_with_hooks)
          .then_some(true),
        has_after_run: (matches!(phase, RuntimeTaskOperationPhase::Realize)
          && self.realize_with_hooks)
          .then_some(true),
        task_should_do: self.task_should_do,
        output: if matches!(phase, RuntimeTaskOperationPhase::Run) {
          self
            .command_output
            .get(task_path)
            .cloned()
            .unwrap_or_default()
        } else {
          Vec::new()
        },
        ..RuntimeTaskCallbackResult::default()
      })
    }

    async fn transform_abc(&mut self, _event: serde_json::Value) -> anyhow::Result<()> {
      Ok(())
    }

    async fn modify_native_command(
      &mut self,
      event: serde_json::Value,
    ) -> anyhow::Result<Vec<String>> {
      serde_json::from_value(event).map_err(Into::into)
    }
  }

  fn task(path: &str, built_in: bool, dependencies: &[&str]) -> RuntimeTaskDescriptor {
    RuntimeTaskDescriptor {
      path: path.to_string(),
      node_name: "entry".to_string(),
      task_name: path.rsplit(':').next().unwrap().to_string(),
      built_in,
      project_task: false,
      lazy: false,
      enabled: true,
      track_state: false,
      refresh_declarations: false,
      has_before_run: !built_in,
      has_after_run: !built_in,
      has_failed: !built_in,
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

  fn selection(root: &Path, root_path: &str, cache_enabled: bool) -> RuntimeTaskSelection {
    RuntimeTaskSelection {
      project_root: root.to_path_buf(),
      cache_enabled,
      expected_root_paths: Some(vec![root_path.to_string()]),
      required_boundary_paths: Vec::new(),
      command_root_paths: vec![root_path.to_string()],
      execution_node_names: vec!["entry".to_string()],
      validate_boundaries: true,
    }
  }

  #[tokio::test]
  async fn native_boundary_unlocks_rust_owned_custom_task_execution() {
    let root = tempfile::tempdir().unwrap();
    let operations = Arc::new(Mutex::new(Vec::new()));
    let tasks = vec![
      task("entry:Compile", true, &[]),
      task("entry:custom", false, &["entry:Compile"]),
    ];
    let mut engine = RuntimeTaskEngine::new(
      tasks,
      selection(root.path(), "entry:custom", false),
      Box::new(RecordingCallback {
        operations: Arc::clone(&operations),
        ..RecordingCallback::default()
      }),
    )
    .unwrap();
    engine.begin_cycle();
    assert!(engine
      .before_native_task("entry", None, "Compile")
      .await
      .unwrap());
    engine
      .after_native_task("entry", None, "Compile")
      .await
      .unwrap();
    engine.finish_cycle().await.unwrap();
    assert_eq!(
      operations.lock().unwrap().as_slice(),
      [
        "beforeRun:entry:custom",
        "run:entry:custom",
        "afterRun:entry:custom",
      ]
    );
  }

  #[tokio::test]
  async fn lazy_javascript_task_is_realized_only_when_selected_for_execution() {
    let root = tempfile::tempdir().unwrap();
    let operations = Arc::new(Mutex::new(Vec::new()));
    let mut lazy = task("entry:lazy", false, &[]);
    lazy.lazy = true;
    lazy.has_before_run = false;
    lazy.has_after_run = false;
    let mut engine = RuntimeTaskEngine::new(
      vec![lazy],
      selection(root.path(), "entry:lazy", false),
      Box::new(RecordingCallback {
        operations: Arc::clone(&operations),
        realize_with_hooks: true,
        ..RecordingCallback::default()
      }),
    )
    .unwrap();

    engine.execute_command_roots().await.unwrap();

    assert_eq!(
      operations.lock().unwrap().as_slice(),
      [
        "realize:entry:lazy",
        "beforeRun:entry:lazy",
        "run:entry:lazy",
        "afterRun:entry:lazy",
      ]
    );
  }

  #[tokio::test]
  async fn task_should_do_false_executes_but_never_reuses_the_snapshot() {
    let root = tempfile::tempdir().unwrap();
    let input = root.path().join("input.txt");
    let output = root.path().join("output.txt");
    std::fs::write(&input, "input").unwrap();
    std::fs::write(&output, "output").unwrap();
    let operations = Arc::new(Mutex::new(Vec::new()));
    let mut incremental = task("entry:incremental", false, &[]);
    incremental.track_state = true;
    incremental.has_task_should_do = true;
    incremental.input_files = vec![super::super::model::RuntimeTaskFileDeclaration::Path(
      input.to_string_lossy().into_owned(),
    )];
    incremental.output_files = vec![super::super::model::RuntimeTaskFileDeclaration::Path(
      output.to_string_lossy().into_owned(),
    )];
    let mut engine = RuntimeTaskEngine::new(
      vec![incremental],
      selection(root.path(), "entry:incremental", true),
      Box::new(RecordingCallback {
        operations: Arc::clone(&operations),
        task_should_do: Some(false),
        ..RecordingCallback::default()
      }),
    )
    .unwrap();

    engine.execute_command_roots().await.unwrap();
    engine.execute_command_roots().await.unwrap();

    let operations = operations.lock().unwrap();
    assert_eq!(
      operations
        .iter()
        .filter(|operation| operation.as_str() == "run:entry:incremental")
        .count(),
      2
    );
    assert_eq!(
      operations
        .iter()
        .filter(|operation| operation.as_str() == "taskShouldDo:entry:incremental")
        .count(),
      2
    );
    assert!(!root
      .path()
      .join(".hvigor/cache/arkdown-custom-task-cache.json")
      .exists());
  }

  #[tokio::test]
  async fn standalone_builtin_runs_custom_user_dependencies_and_returns_output() {
    let root = tempfile::tempdir().unwrap();
    let operations = Arc::new(Mutex::new(Vec::new()));
    let custom = task("entry:custom", false, &[]);
    let mut built_in = task("entry:tasks", true, &[]);
    built_in.user_dependencies = vec!["entry:custom".to_string()];
    let mut command_output = BTreeMap::new();
    command_output.insert("entry:tasks".to_string(), vec!["task report".to_string()]);
    let mut engine = RuntimeTaskEngine::new(
      vec![custom, built_in],
      selection(root.path(), "entry:tasks", false),
      Box::new(RecordingCallback {
        operations: Arc::clone(&operations),
        command_output,
        ..RecordingCallback::default()
      }),
    )
    .unwrap();

    assert_eq!(
      engine.execute_command_roots().await.unwrap(),
      ["task report"]
    );
    assert_eq!(
      operations.lock().unwrap().as_slice(),
      [
        "beforeRun:entry:custom",
        "run:entry:custom",
        "afterRun:entry:custom",
        "run:entry:tasks",
      ]
    );
  }

  #[tokio::test]
  async fn custom_task_failure_invokes_on_failed_after_the_original_error() {
    let root = tempfile::tempdir().unwrap();
    let operations = Arc::new(Mutex::new(Vec::new()));
    let mut engine = RuntimeTaskEngine::new(
      vec![task("entry:failing", false, &[])],
      selection(root.path(), "entry:failing", false),
      Box::new(RecordingCallback {
        operations: Arc::clone(&operations),
        fail_run_for: Some("entry:failing".to_string()),
        ..RecordingCallback::default()
      }),
    )
    .unwrap();

    let error = engine.execute_command_roots().await.unwrap_err();
    assert!(matches!(
      error,
      RuntimeTaskEngineError::Callback { phase: "run", .. }
    ));
    assert_eq!(
      operations.lock().unwrap().as_slice(),
      [
        "beforeRun:entry:failing",
        "run:entry:failing",
        "failed:entry:failing",
      ]
    );
  }

  #[tokio::test]
  async fn disabled_native_boundary_is_completed_without_js_callbacks() {
    let root = tempfile::tempdir().unwrap();
    let operations = Arc::new(Mutex::new(Vec::new()));
    let mut disabled = task("entry:PackageHap", true, &[]);
    disabled.enabled = false;
    let mut selected = selection(root.path(), "entry:PackageHap", false);
    selected.command_root_paths.clear();
    selected.required_boundary_paths = vec!["entry:PackageHap".to_string()];
    let mut engine = RuntimeTaskEngine::new(
      vec![disabled],
      selected,
      Box::new(RecordingCallback {
        operations: Arc::clone(&operations),
        ..RecordingCallback::default()
      }),
    )
    .unwrap();

    engine.begin_cycle();
    assert!(!engine
      .before_native_task("entry", None, "PackageHap")
      .await
      .unwrap());
    engine.finish_cycle().await.unwrap();
    assert!(operations.lock().unwrap().is_empty());
  }
}
