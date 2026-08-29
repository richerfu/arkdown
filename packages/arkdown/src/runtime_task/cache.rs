use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use rayon::prelude::*;
use regex::{Regex, RegexBuilder};
use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use napi::bindgen_prelude::{
  Array, Env, FnArgs, Function, Generator, JsObjectValue, Null, Object, ToNapiValue, Unknown,
};
use napi::{JsValue, ValueType};
use napi_derive::napi;

use super::inspect::RuntimeTaskNapiInspector;
use super::model::{
  RuntimeTaskDescriptor, RuntimeTaskFileConfig, RuntimeTaskFileDeclaration, RuntimeTaskFileFilter,
  RuntimeTaskProperty,
};

const CACHE_SCHEMA_VERSION: u32 = 3;
const HVIGOR_PARITY_VERSION: &str = "6.24.3";

#[derive(Debug, thiserror::Error)]
pub enum RuntimeTaskCacheError {
  #[error("failed to find the incremental input file: {}.", .0.display())]
  MissingInput(PathBuf),
  #[error("snapshot runtime task path {path}: {source}")]
  Snapshot {
    path: PathBuf,
    #[source]
    source: std::io::Error,
  },
  #[error("invalid Hvigor FileSet filter {pattern:?} for {}: {source}", path.display())]
  InvalidFilter {
    path: PathBuf,
    pattern: String,
    #[source]
    source: regex::Error,
  },
  #[error("unsupported stateful Hvigor FileSet filter flag {flag:?} for {}", path.display())]
  UnsupportedFilterFlag { path: PathBuf, flag: char },
  #[error("persist runtime task cache {path}: {source}")]
  Persist {
    path: PathBuf,
    #[source]
    source: std::io::Error,
  },
  #[error("serialize runtime task cache: {0}")]
  Serialize(#[from] serde_json::Error),
}

pub struct RuntimeTaskCache {
  enabled: bool,
  project_name: String,
  cache_path: PathBuf,
  entries: BTreeMap<String, StoredTaskSnapshot>,
  files: BTreeMap<PathBuf, FileSnapshot>,
  pending: BTreeMap<String, RuntimeTaskDescriptor>,
}

impl RuntimeTaskCache {
  pub fn new(project_root: &Path, enabled: bool) -> Self {
    let cache_path = project_root
      .join(".hvigor")
      .join("cache")
      .join("arkdown-custom-task-cache.json");
    let project_name = project_root
      .file_name()
      .map(|name| name.to_string_lossy().into_owned())
      .unwrap_or_default();
    let stored = Self::load(&cache_path);
    Self {
      enabled,
      project_name,
      cache_path,
      entries: stored.entries,
      files: stored.files,
      pending: BTreeMap::new(),
    }
  }

  pub fn begin_cycle(&mut self) {
    self.pending.clear();
  }

  pub fn is_up_to_date(&self, task: &RuntimeTaskDescriptor) -> Result<bool, RuntimeTaskCacheError> {
    self.is_up_to_date_for_project(task, &self.project_name)
  }

  fn is_up_to_date_for_project(
    &self,
    task: &RuntimeTaskDescriptor,
    project_name: &str,
  ) -> Result<bool, RuntimeTaskCacheError> {
    if !self.enabled || !task.track_state || !task.is_incremental() {
      return Ok(false);
    }
    let Some(previous) = self.entries.get(&task.path) else {
      return Ok(false);
    };
    if !previous.successful {
      return Ok(false);
    }
    let current = self.snapshot_for_project(task, true, project_name)?;
    if current
      .output_files
      .iter()
      .any(|file| file.snapshot.is_none())
    {
      return Ok(false);
    }
    Ok(previous.snapshot == current)
  }

  pub fn record_successful(&mut self, task: &RuntimeTaskDescriptor) {
    if self.enabled && task.track_state && task.is_incremental() {
      self.pending.insert(task.path.clone(), task.clone());
    }
  }

  pub fn finish_cycle(&mut self) -> Result<(), RuntimeTaskCacheError> {
    if !self.enabled || self.pending.is_empty() {
      self.pending.clear();
      return Ok(());
    }
    let pending = std::mem::take(&mut self.pending);
    for (path, task) in pending {
      let snapshot = self.snapshot(&task, false)?;
      self.entries.insert(
        path,
        StoredTaskSnapshot {
          snapshot,
          successful: true,
        },
      );
    }
    self.persist()
  }

  fn input_value_changed(&self, task: &RuntimeTaskDescriptor, input_key: &str) -> bool {
    let previous = self.entries.get(&task.path).map(|entry| &entry.snapshot);
    let current_value = task
      .input_properties
      .iter()
      .find(|property| property.name == input_key)
      .map(|property| &property.value);
    let previous_value = previous
      .and_then(|snapshot| {
        snapshot
          .properties
          .iter()
          .find(|property| property.name == input_key)
      })
      .map(|property| &property.value);
    previous_value != current_value
  }

  fn file_compare(
    &self,
    task: &RuntimeTaskDescriptor,
    declaration: &RuntimeTaskFileDeclaration,
  ) -> Result<(FileSnapshot, bool), RuntimeTaskCacheError> {
    let current =
      FileSnapshotter::snapshot_path(declaration, false)?.unwrap_or_else(FileSnapshot::empty_root);
    let declared_path = Path::new(declaration.path());
    let previous = self.files.get(declared_path).or_else(|| {
      self.entries.get(&task.path).and_then(|entry| {
        entry
          .snapshot
          .input_files
          .iter()
          .chain(entry.snapshot.output_files.iter())
          .find(|file| file.path == declared_path)
          .and_then(|file| file.snapshot.as_ref())
      })
    });
    let changed = previous != Some(&current);
    Ok((current, changed))
  }

  fn task_snapshot(&self, task_path: &str) -> Option<&StoredTaskSnapshot> {
    self.entries.get(task_path)
  }

  fn file_snapshot(&self, path: &Path) -> Option<&FileSnapshot> {
    self.files.get(path).or_else(|| {
      self.entries.values().find_map(|entry| {
        entry
          .snapshot
          .input_files
          .iter()
          .chain(entry.snapshot.output_files.iter())
          .find(|file| file.path == path)
          .and_then(|file| file.snapshot.as_ref())
      })
    })
  }

  fn store_task_snapshot(
    &mut self,
    task_path: String,
    snapshot: TaskSnapshot,
    successful: bool,
  ) -> Result<(), RuntimeTaskCacheError> {
    if !self.enabled {
      return Ok(());
    }
    self.entries.insert(
      task_path,
      StoredTaskSnapshot {
        snapshot,
        successful,
      },
    );
    self.persist()
  }

  fn store_file_snapshot(
    &mut self,
    path: PathBuf,
    snapshot: FileSnapshot,
  ) -> Result<(), RuntimeTaskCacheError> {
    if !self.enabled {
      return Ok(());
    }
    self.files.insert(path, snapshot);
    self.persist()
  }

  fn snapshot(
    &self,
    task: &RuntimeTaskDescriptor,
    require_inputs: bool,
  ) -> Result<TaskSnapshot, RuntimeTaskCacheError> {
    self.snapshot_for_project(task, require_inputs, &self.project_name)
  }

  fn snapshot_for_project(
    &self,
    task: &RuntimeTaskDescriptor,
    require_inputs: bool,
    project_name: &str,
  ) -> Result<TaskSnapshot, RuntimeTaskCacheError> {
    let mut properties = task.input_properties.clone();
    properties.sort_by(|left, right| left.name.cmp(&right.name));
    let mut input_files = FileSnapshotter::snapshot_paths(&task.input_files, require_inputs, true)?;
    let mut output_files = FileSnapshotter::snapshot_paths(&task.output_files, false, false)?;
    input_files.sort_by(|left, right| left.path.cmp(&right.path));
    output_files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(TaskSnapshot {
      project_name: project_name.to_string(),
      module_name: task.node_name.clone(),
      task_name: task.task_name.clone(),
      properties,
      input_files,
      output_files,
    })
  }

  fn load(cache_path: &Path) -> StoredTaskCache {
    let Ok(content) = fs::read_to_string(cache_path) else {
      return StoredTaskCache::empty();
    };
    let Ok(cache) = serde_json::from_str::<StoredTaskCache>(&content) else {
      return StoredTaskCache::empty();
    };
    if cache.schema_version != CACHE_SCHEMA_VERSION || cache.hvigor_version != HVIGOR_PARITY_VERSION
    {
      return StoredTaskCache::empty();
    }
    cache
  }

  fn persist(&self) -> Result<(), RuntimeTaskCacheError> {
    let parent = self.cache_path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|source| RuntimeTaskCacheError::Persist {
      path: self.cache_path.clone(),
      source,
    })?;
    let content = serde_json::to_vec(&StoredTaskCacheRef {
      schema_version: CACHE_SCHEMA_VERSION,
      hvigor_version: HVIGOR_PARITY_VERSION,
      entries: &self.entries,
      files: &self.files,
    })?;
    fs::write(&self.cache_path, content).map_err(|source| RuntimeTaskCacheError::Persist {
      path: self.cache_path.clone(),
      source,
    })
  }
}

/// Stateful NAPI owner for the public Hvigor `TaskProxy` compatibility API.
///
/// Hvigor keeps one project cache service for a command. Keeping the same
/// lifecycle in Rust avoids re-reading and re-deserializing the cache for every
/// `preExecute`, input comparison, and snapshot lookup while leaving V8 only
/// responsible for invoking arbitrary task callbacks.
#[napi]
pub struct RuntimeTaskCacheSession {
  project_root: PathBuf,
  cache: RuntimeTaskCache,
}

#[napi(object)]
pub struct RuntimeTaskPropertyInput {
  pub name: String,
  pub value: String,
  pub value_type: String,
}

#[napi(object)]
pub struct RuntimeTaskFileFilterInput {
  pub source: String,
  pub flags: Option<String>,
}

#[napi(object)]
pub struct RuntimeTaskFileDeclarationInput {
  pub path: String,
  pub is_directory: Option<bool>,
  pub test: Option<RuntimeTaskFileFilterInput>,
  pub depth: Option<u32>,
  pub file_snap_shot_hash_value: Option<String>,
}

impl RuntimeTaskFileDeclarationInput {
  fn into_declaration(self) -> RuntimeTaskFileDeclaration {
    if self.is_directory.is_none()
      && self.test.is_none()
      && self.depth.is_none()
      && self.file_snap_shot_hash_value.is_none()
    {
      return RuntimeTaskFileDeclaration::Path(self.path);
    }
    RuntimeTaskFileDeclaration::Config(RuntimeTaskFileConfig {
      path: self.path,
      is_directory: self.is_directory,
      test: self.test.map(|filter| RuntimeTaskFileFilter {
        source: filter.source,
        flags: filter.flags.unwrap_or_default(),
      }),
      depth: self.depth,
      file_snap_shot_hash_value: self.file_snap_shot_hash_value,
    })
  }
}

#[napi(object)]
pub struct RuntimeTaskSnapshotRequest {
  pub path: String,
  pub project_name: String,
  pub node_name: String,
  pub task_name: String,
  pub project_task: bool,
  pub track_state: bool,
  pub input_properties: Vec<RuntimeTaskPropertyInput>,
  pub input_files: Vec<RuntimeTaskFileDeclarationInput>,
  pub output_files: Vec<RuntimeTaskFileDeclarationInput>,
}

impl RuntimeTaskSnapshotRequest {
  fn into_descriptor(self) -> RuntimeTaskDescriptor {
    RuntimeTaskDescriptor {
      path: self.path,
      node_name: self.node_name,
      task_name: self.task_name,
      built_in: false,
      project_task: self.project_task,
      lazy: false,
      enabled: true,
      track_state: self.track_state,
      refresh_declarations: false,
      has_before_run: false,
      has_after_run: false,
      has_failed: false,
      has_task_should_do: false,
      graph_bound_custom: false,
      dependencies: Vec::new(),
      user_dependencies: Vec::new(),
      input_properties: self
        .input_properties
        .into_iter()
        .map(|property| RuntimeTaskProperty {
          name: property.name,
          value: serde_json::Value::String(property.value),
          value_type: property.value_type,
        })
        .collect(),
      input_files: self
        .input_files
        .into_iter()
        .map(RuntimeTaskFileDeclarationInput::into_declaration)
        .collect(),
      output_files: self
        .output_files
        .into_iter()
        .map(RuntimeTaskFileDeclarationInput::into_declaration)
        .collect(),
    }
  }
}

#[napi]
pub struct RuntimeFileCompareResult {
  snapshot: RuntimeFileSnapshot,
  changed: bool,
}

#[napi]
impl RuntimeFileCompareResult {
  #[napi(getter)]
  pub fn snapshot(&self) -> RuntimeFileSnapshot {
    RuntimeFileSnapshot::from_snapshot(self.snapshot.snapshot.clone())
  }

  #[napi(getter)]
  pub fn changed(&self) -> bool {
    self.changed
  }
}

#[napi]
impl RuntimeTaskCacheSession {
  #[napi(constructor)]
  pub fn new(project_root: String, cache_enabled: bool) -> napi::Result<Self> {
    let project_root = PathBuf::from(project_root);
    let project_root = if project_root.is_absolute() {
      arkdown_path::lexical_normalize(&project_root)
    } else {
      let current = std::env::current_dir().map_err(Self::napi_error)?;
      arkdown_path::lexical_normalize(&current.join(project_root))
    };
    Ok(Self {
      cache: RuntimeTaskCache::new(&project_root, cache_enabled),
      project_root,
    })
  }

  #[napi]
  pub fn create_snapshot(&self, request: RuntimeTaskSnapshotRequest) -> RuntimeTaskSnapshot {
    let project_name = request.project_name.clone();
    let mut descriptor = request.into_descriptor();
    descriptor.normalize_declared_paths(&self.project_root, cfg!(windows));
    RuntimeTaskSnapshot::from_descriptor(project_name, descriptor, false)
  }

  #[napi]
  pub fn create_task_snapshot(
    &self,
    env: Env,
    task: Object<'_>,
  ) -> napi::Result<RuntimeTaskSnapshot> {
    let (project_name, mut descriptor) = self.task_descriptor(env, task)?;
    descriptor.normalize_declared_paths(&self.project_root, cfg!(windows));
    Ok(RuntimeTaskSnapshot::from_descriptor(
      project_name,
      descriptor,
      false,
    ))
  }

  #[napi]
  pub fn get_task_snapshot(
    &self,
    env: Env,
    task: Object<'_>,
  ) -> napi::Result<Option<RuntimeTaskSnapshot>> {
    let (_, descriptor) = self.task_descriptor(env, task)?;
    Ok(self.cache.task_snapshot(&descriptor.path).map(|stored| {
      RuntimeTaskSnapshot::from_stored_snapshot(
        descriptor.path,
        descriptor.node_name,
        descriptor.task_name,
        descriptor.project_task,
        stored,
      )
    }))
  }

  #[napi]
  pub fn check(&self, snapshot: &RuntimeTaskSnapshot) -> napi::Result<bool> {
    self
      .cache
      .is_up_to_date_for_project(&snapshot.descriptor, &snapshot.snapshot.project_name)
      .map_err(Self::napi_error)
  }

  #[napi]
  pub fn record_snapshot(
    &mut self,
    task_snapshot: &RuntimeTaskSnapshot,
  ) -> napi::Result<RuntimeTaskSnapshot> {
    let mut descriptor = task_snapshot.descriptor.clone();
    descriptor.normalize_declared_paths(&self.project_root, cfg!(windows));
    let snapshot = self
      .cache
      .snapshot_for_project(&descriptor, false, &task_snapshot.snapshot.project_name)
      .map_err(Self::napi_error)?;
    if task_snapshot.successful {
      self
        .cache
        .store_task_snapshot(descriptor.path.clone(), snapshot.clone(), true)
        .map_err(Self::napi_error)?;
    } else if self.cache.enabled && descriptor.track_state && descriptor.is_incremental() {
      self
        .cache
        .store_task_snapshot(descriptor.path.clone(), snapshot.clone(), false)
        .map_err(Self::napi_error)?;
    }
    Ok(RuntimeTaskSnapshot::from_stored(
      snapshot,
      descriptor,
      task_snapshot.successful,
    ))
  }

  #[napi]
  pub fn input_value_changed(&self, snapshot: &RuntimeTaskSnapshot, input_key: String) -> bool {
    self
      .cache
      .input_value_changed(&snapshot.descriptor, &input_key)
  }

  #[napi]
  pub fn file_compare(
    &self,
    snapshot: &RuntimeTaskSnapshot,
    declaration: RuntimeTaskFileDeclarationInput,
  ) -> napi::Result<RuntimeFileCompareResult> {
    let mut declaration = declaration.into_declaration();
    declaration.normalize_against(&self.project_root);
    let (snapshot, changed) = self
      .cache
      .file_compare(&snapshot.descriptor, &declaration)
      .map_err(Self::napi_error)?;
    Ok(RuntimeFileCompareResult {
      snapshot: RuntimeFileSnapshot::from_snapshot(snapshot),
      changed,
    })
  }

  #[napi]
  pub fn file_compare_path(
    &self,
    snapshot: &RuntimeTaskSnapshot,
    path: String,
    options: Option<Object<'_>>,
  ) -> napi::Result<RuntimeFileCompareResult> {
    self.file_compare(snapshot, Self::file_declaration(path, options.as_ref())?)
  }

  #[napi]
  pub fn get_task(
    &self,
    task_path: String,
    node_name: String,
    task_name: String,
    project_task: bool,
  ) -> Option<RuntimeTaskSnapshot> {
    self.cache.task_snapshot(&task_path).map(|stored| {
      RuntimeTaskSnapshot::from_stored_snapshot(
        task_path,
        node_name,
        task_name,
        project_task,
        stored,
      )
    })
  }

  #[napi]
  pub fn get_file(
    &self,
    declaration: RuntimeTaskFileDeclarationInput,
  ) -> Option<RuntimeFileSnapshot> {
    let mut declaration = declaration.into_declaration();
    declaration.normalize_against(&self.project_root);
    self
      .cache
      .file_snapshot(Path::new(declaration.path()))
      .cloned()
      .map(RuntimeFileSnapshot::from_snapshot)
  }

  #[napi]
  pub fn get_file_path(
    &self,
    path: String,
    options: Option<Object<'_>>,
  ) -> napi::Result<Option<RuntimeFileSnapshot>> {
    Ok(self.get_file(Self::file_declaration(path, options.as_ref())?))
  }

  #[napi]
  pub fn store_file(
    &mut self,
    declaration: RuntimeTaskFileDeclarationInput,
    snapshot: &RuntimeFileSnapshot,
  ) -> napi::Result<()> {
    let mut declaration = declaration.into_declaration();
    declaration.normalize_against(&self.project_root);
    self
      .cache
      .store_file_snapshot(PathBuf::from(declaration.path()), snapshot.snapshot.clone())
      .map_err(Self::napi_error)
  }

  #[napi]
  pub fn store_file_path(
    &mut self,
    path: String,
    options: Option<Object<'_>>,
    snapshot: &RuntimeFileSnapshot,
  ) -> napi::Result<()> {
    self.store_file(Self::file_declaration(path, options.as_ref())?, snapshot)
  }

  #[napi]
  pub fn build_info(&self) -> napi::Result<String> {
    Self::json(&self.build_info_value())
  }

  #[napi]
  pub fn build_info_value(&self) -> serde_json::Value {
    let path = self
      .project_root
      .join(".hvigor")
      .join("cache")
      .join("last-build-info.json");
    fs::read_to_string(path)
      .ok()
      .and_then(|content| serde_json::from_str::<serde_json::Value>(&content).ok())
      .unwrap_or_else(|| serde_json::json!({}))
  }
}

impl RuntimeTaskCacheSession {
  fn task_descriptor(
    &self,
    env: Env,
    task: Object<'_>,
  ) -> napi::Result<(String, RuntimeTaskDescriptor)> {
    let task_context = task.get::<Object<'_>>("taskContext")?.unwrap_or(task);
    let class_incremental_task = ["declareInputs", "declareInputFiles", "declareOutputFiles"]
      .into_iter()
      .map(|name| Self::has_function(&task, name))
      .collect::<napi::Result<Vec<_>>>()?
      .into_iter()
      .all(|value| value);
    let inspector = RuntimeTaskNapiInspector::new(env, task, task_context, class_incremental_task);
    let state =
      serde_json::from_value::<super::model::RuntimeTaskCallbackResult>(inspector.state()?)
        .map_err(Self::napi_error)?;
    let task_name = Self::required_string_method(&task, "getName")?;
    let path = Self::required_string_method(&task, "getPath")?;
    let node = Self::required_object_method(&task, "getNode")?;
    let node_name = Self::node_name(&node)?;
    let project_task = node
      .get::<String>("classKind")?
      .is_some_and(|kind| kind == "project");
    let project = if project_task {
      node
    } else {
      Self::required_object_method(&node, "getProject")?
    };
    let project_name = Self::node_name(&project)?;
    Ok((
      project_name,
      RuntimeTaskDescriptor {
        path,
        node_name,
        task_name,
        built_in: false,
        project_task,
        lazy: false,
        enabled: state.enabled.unwrap_or(true),
        track_state: state.track_state.unwrap_or(true),
        refresh_declarations: state.refresh_declarations.unwrap_or(class_incremental_task),
        has_before_run: state.has_before_run.unwrap_or(false),
        has_after_run: state.has_after_run.unwrap_or(false),
        has_failed: state.has_failed.unwrap_or(false),
        has_task_should_do: state.has_task_should_do.unwrap_or(false),
        graph_bound_custom: false,
        dependencies: Self::required_string_array_method(&task, "getDependsOn")?,
        user_dependencies: Vec::new(),
        input_properties: state.input_properties.unwrap_or_default(),
        input_files: state.input_files.unwrap_or_default(),
        output_files: state.output_files.unwrap_or_default(),
      },
    ))
  }

  fn file_declaration(
    path: String,
    options: Option<&Object<'_>>,
  ) -> napi::Result<RuntimeTaskFileDeclarationInput> {
    let is_directory = options
      .map(|value| value.get::<bool>("isDirectory"))
      .transpose()?
      .flatten();
    let depth = options
      .map(|value| value.get::<u32>("depth"))
      .transpose()?
      .flatten();
    let file_snap_shot_hash_value = options
      .map(|value| value.get::<String>("fileSnapShotHashValue"))
      .transpose()?
      .flatten();
    let test = options
      .map(|value| value.get::<Object<'_>>("test"))
      .transpose()?
      .flatten()
      .map(|value| -> napi::Result<RuntimeTaskFileFilterInput> {
        Ok(RuntimeTaskFileFilterInput {
          source: value.get::<String>("source")?.unwrap_or_default(),
          flags: value.get::<String>("flags")?,
        })
      })
      .transpose()?;
    Ok(RuntimeTaskFileDeclarationInput {
      path,
      is_directory,
      test,
      depth,
      file_snap_shot_hash_value,
    })
  }

  fn required_string_method(object: &Object<'_>, name: &str) -> napi::Result<String> {
    object
      .get::<Function<(), String>>(name)?
      .ok_or_else(|| napi::Error::from_reason(format!("Hvigor task is missing {name}()")))?
      .apply(object, ())
  }

  fn required_string_array_method(object: &Object<'_>, name: &str) -> napi::Result<Vec<String>> {
    object
      .get::<Function<(), Vec<String>>>(name)?
      .ok_or_else(|| napi::Error::from_reason(format!("Hvigor task is missing {name}()")))?
      .apply(object, ())
  }

  fn required_object_method<'env>(object: &Object<'env>, name: &str) -> napi::Result<Object<'env>> {
    object
      .get::<Function<(), Object<'env>>>(name)?
      .ok_or_else(|| napi::Error::from_reason(format!("Hvigor task is missing {name}()")))?
      .apply(object, ())
  }

  fn node_name(node: &Object<'_>) -> napi::Result<String> {
    for name in ["getName", "getNodeName"] {
      if let Some(method) = node.get::<Function<(), String>>(name)? {
        return method.apply(node, ());
      }
    }
    Err(napi::Error::from_reason(
      "Hvigor task node is missing getName()/getNodeName()",
    ))
  }

  fn has_function(object: &Object<'_>, name: &str) -> napi::Result<bool> {
    Ok(object.get::<Unknown<'_>>(name)?.is_some_and(|value| {
      value
        .get_type()
        .is_ok_and(|kind| kind == ValueType::Function)
    }))
  }

  fn json(value: &impl Serialize) -> napi::Result<String> {
    serde_json::to_string(value).map_err(Self::napi_error)
  }

  fn napi_error(error: impl std::fmt::Display) -> napi::Error {
    napi::Error::from_reason(format!("ARKDOWN_RUNTIME_TASK_CACHE_FAILED: {error}"))
  }
}

struct FileSnapshotter;

impl FileSnapshotter {
  fn snapshot_paths(
    paths: &[RuntimeTaskFileDeclaration],
    required: bool,
    input: bool,
  ) -> Result<Vec<DeclaredFileSnapshot>, RuntimeTaskCacheError> {
    let mut unique = BTreeMap::new();
    for declaration in paths {
      unique
        .entry(PathBuf::from(declaration.path()))
        .or_insert_with(|| declaration.clone());
    }
    unique
      .into_values()
      .collect::<Vec<_>>()
      .into_par_iter()
      .map(|declaration| {
        let path = PathBuf::from(declaration.path());
        let snapshot = Self::snapshot_path(&declaration, required && input)?;
        Ok(DeclaredFileSnapshot {
          path,
          declaration,
          snapshot,
        })
      })
      .collect()
  }

  fn snapshot_path(
    declaration: &RuntimeTaskFileDeclaration,
    required: bool,
  ) -> Result<Option<FileSnapshot>, RuntimeTaskCacheError> {
    let path = Path::new(declaration.path());
    if !path.exists() {
      if required {
        return Err(RuntimeTaskCacheError::MissingInput(path.to_path_buf()));
      }
      return Ok(None);
    }
    let filter = declaration
      .filter()
      .map(|filter| Self::compile_filter(path, filter))
      .transpose()?;
    let mut visited_links = FxHashSet::default();
    let snapshot = Self::visit(path, filter.as_ref(), &mut visited_links).map_err(|source| {
      RuntimeTaskCacheError::Snapshot {
        path: path.to_path_buf(),
        source,
      }
    })?;
    Ok(snapshot.or_else(|| Some(FileSnapshot::empty_root())))
  }

  fn compile_filter(
    path: &Path,
    filter: &RuntimeTaskFileFilter,
  ) -> Result<Regex, RuntimeTaskCacheError> {
    if let Some(flag) = filter
      .flags
      .chars()
      .find(|flag| !matches!(flag, 'i' | 'm' | 's' | 'u'))
    {
      return Err(RuntimeTaskCacheError::UnsupportedFilterFlag {
        path: path.to_path_buf(),
        flag,
      });
    }
    let mut builder = RegexBuilder::new(&filter.source);
    builder
      .case_insensitive(filter.flags.contains('i'))
      .multi_line(filter.flags.contains('m'))
      .dot_matches_new_line(filter.flags.contains('s'));
    builder
      .build()
      .map_err(|source| RuntimeTaskCacheError::InvalidFilter {
        path: path.to_path_buf(),
        pattern: filter.source.clone(),
        source,
      })
  }

  fn visit(
    path: &Path,
    filter: Option<&Regex>,
    visited_links: &mut FxHashSet<PathBuf>,
  ) -> std::io::Result<Option<FileSnapshot>> {
    let metadata = match fs::metadata(path) {
      Ok(metadata) => metadata,
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
      Err(error) => return Err(error),
    };
    let link_metadata = fs::symlink_metadata(path)?;
    if filter.is_some_and(|filter| !filter.is_match(&path.to_string_lossy())) {
      return Ok(None);
    }
    let symbolic_link = link_metadata.file_type().is_symlink();
    if symbolic_link {
      let identity = fs::canonicalize(path)?;
      if !visited_links.insert(identity) {
        return Ok(None);
      }
    }
    let name = path
      .file_name()
      .map(|name| name.to_string_lossy().into_owned())
      .unwrap_or_default();
    if metadata.is_file() {
      let modified_time = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(Self::node_date_millis)
        .unwrap_or(0);
      return Ok(Some(FileSnapshot::File {
        name,
        path: path.to_path_buf(),
        symbolic_link,
        size: metadata.len(),
        modified_time,
      }));
    }
    if !metadata.is_dir() {
      return Ok(None);
    }
    let mut entries = match fs::read_dir(path) {
      Ok(entries) => entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<_>>(),
      Err(error) if matches!(error.kind(), std::io::ErrorKind::PermissionDenied) => Vec::new(),
      Err(error) => return Err(error),
    };
    entries.sort();
    let mut children = Vec::with_capacity(entries.len());
    for child in entries {
      if let Some(snapshot) = Self::visit(&child, filter, visited_links)? {
        children.push(snapshot);
      }
    }
    Ok(Some(FileSnapshot::Directory {
      name,
      path: path.to_path_buf(),
      symbolic_link,
      children,
    }))
  }

  /// Node's `Stats.mtime.getTime()` rounds the filesystem nanosecond value to
  /// the nearest millisecond. Hvigor's `FileMetaHasher` hashes that number;
  /// `Duration::as_millis()` would truncate it and invalidate unchanged files
  /// whenever the fractional millisecond is at least 0.5.
  fn node_date_millis(duration: std::time::Duration) -> u64 {
    duration
      .as_secs()
      .saturating_mul(1_000)
      .saturating_add((u64::from(duration.subsec_nanos()).saturating_add(500_000)) / 1_000_000)
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskSnapshot {
  project_name: String,
  module_name: String,
  task_name: String,
  properties: Vec<RuntimeTaskProperty>,
  input_files: Vec<DeclaredFileSnapshot>,
  output_files: Vec<DeclaredFileSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeclaredFileSnapshot {
  path: PathBuf,
  declaration: RuntimeTaskFileDeclaration,
  snapshot: Option<FileSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
  tag = "kind",
  rename_all = "lowercase",
  rename_all_fields = "camelCase"
)]
enum FileSnapshot {
  Unknown {
    name: String,
    path: PathBuf,
    symbolic_link: bool,
  },
  File {
    name: String,
    path: PathBuf,
    symbolic_link: bool,
    size: u64,
    modified_time: u64,
  },
  Directory {
    name: String,
    path: PathBuf,
    symbolic_link: bool,
    children: Vec<FileSnapshot>,
  },
}

impl FileSnapshot {
  fn empty_root() -> Self {
    Self::Unknown {
      name: "emptyroot".to_string(),
      path: PathBuf::new(),
      symbolic_link: false,
    }
  }

  fn hash_value(&self) -> String {
    match self {
      Self::Unknown { .. } => String::new(),
      Self::File {
        name,
        size,
        modified_time,
        ..
      } => RuntimeSnapshotHash::digest(format!("{name}{size}{modified_time}")),
      Self::Directory { children, .. } if children.is_empty() => String::new(),
      Self::Directory { children, .. } => {
        let mut value = String::new();
        for child in children {
          value.push_str(child.name());
          value.push_str(&child.hash_value());
        }
        RuntimeSnapshotHash::digest(value)
      }
    }
  }

  fn name(&self) -> &str {
    match self {
      Self::Unknown { name, .. } | Self::File { name, .. } | Self::Directory { name, .. } => name,
    }
  }

  fn public_json(&self) -> serde_json::Value {
    match self {
      Self::Unknown {
        name,
        path,
        symbolic_link,
      } => serde_json::json!({
        "hashValue": self.hash_value(),
        "name": name,
        "path": path,
        "type": "unknown",
        "isSymbolicLink": symbolic_link,
        "children": [],
      }),
      Self::File {
        name,
        path,
        symbolic_link,
        size,
        modified_time,
      } => serde_json::json!({
        "hashValue": self.hash_value(),
        "name": name,
        "path": path,
        "type": "file",
        "isSymbolicLink": symbolic_link,
        "fileMetaData": {
          "size": size,
          "lastModifiedTime": modified_time,
        },
      }),
      Self::Directory {
        name,
        path,
        symbolic_link,
        children,
      } => serde_json::json!({
        "hashValue": self.hash_value(),
        "name": name,
        "path": path,
        "type": "directory",
        "isSymbolicLink": symbolic_link,
        "children": children.iter().map(Self::public_json).collect::<Vec<_>>(),
      }),
    }
  }
}

struct RuntimeSnapshotHash;

impl RuntimeSnapshotHash {
  fn digest(value: impl AsRef<[u8]>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value);
    format!("{:x}", hasher.finalize())
  }
}

#[napi(object)]
pub struct RuntimeFileMetadata {
  pub size: f64,
  pub last_modified_time: f64,
}

/// Native view for the snapshot values returned by Hvigor's public
/// `TaskProxy` API. Filesystem traversal, Hvigor-compatible hashing, and
/// serialization stay in the Rust cache owner; JavaScript only installs the
/// standard iterator symbol that Node-API cannot name directly.
#[napi]
pub struct RuntimeFileSnapshot {
  snapshot: FileSnapshot,
}

#[napi]
impl RuntimeFileSnapshot {
  #[napi(getter)]
  pub fn name(&self) -> String {
    self.snapshot.name().to_string()
  }

  #[napi(getter)]
  pub fn path(&self) -> String {
    match &self.snapshot {
      FileSnapshot::Unknown { path, .. }
      | FileSnapshot::File { path, .. }
      | FileSnapshot::Directory { path, .. } => path.to_string_lossy().into_owned(),
    }
  }

  #[napi(getter, js_name = "type")]
  pub fn snapshot_type(&self) -> String {
    match &self.snapshot {
      FileSnapshot::Unknown { .. } => "unknown",
      FileSnapshot::File { .. } => "file",
      FileSnapshot::Directory { .. } => "directory",
    }
    .to_string()
  }

  #[napi(getter)]
  pub fn is_symbolic_link(&self) -> bool {
    match &self.snapshot {
      FileSnapshot::Unknown { symbolic_link, .. }
      | FileSnapshot::File { symbolic_link, .. }
      | FileSnapshot::Directory { symbolic_link, .. } => *symbolic_link,
    }
  }

  #[napi(getter)]
  pub fn file_meta_data(&self) -> Option<RuntimeFileMetadata> {
    match &self.snapshot {
      FileSnapshot::File {
        size,
        modified_time,
        ..
      } => Some(RuntimeFileMetadata {
        size: *size as f64,
        last_modified_time: *modified_time as f64,
      }),
      _ => None,
    }
  }

  #[napi(getter)]
  pub fn children(&self) -> Option<Vec<RuntimeFileSnapshot>> {
    match &self.snapshot {
      FileSnapshot::Unknown { .. } => Some(Vec::new()),
      FileSnapshot::Directory { children, .. } => Some(
        children
          .iter()
          .cloned()
          .map(|snapshot| RuntimeFileSnapshot { snapshot })
          .collect(),
      ),
      _ => None,
    }
  }

  #[napi(getter)]
  pub fn hash_value(&self) -> String {
    self.snapshot.hash_value()
  }

  #[napi]
  pub fn is_hash_equals(&self, other: &RuntimeFileSnapshot) -> bool {
    self.hash_value() == other.hash_value()
  }

  #[napi]
  pub fn equals(&self, other: &RuntimeFileSnapshot) -> bool {
    let same_class = matches!(self.snapshot, FileSnapshot::File { .. })
      == matches!(other.snapshot, FileSnapshot::File { .. });
    same_class && self.is_hash_equals(other)
  }

  #[napi]
  pub fn accept<'env>(&self, env: Env, visitor: Object<'env>) -> napi::Result<u32> {
    RuntimeFileSnapshotVisitorAdapter::new(env, visitor).accept(&self.snapshot)
  }

  #[napi]
  pub fn to_json_string(&self) -> napi::Result<String> {
    RuntimeTaskCacheSession::json(&self.snapshot.public_json())
  }

  #[napi(ts_return_type = "IterableIterator<RuntimeFileSnapshot>")]
  pub fn walk(&self) -> RuntimeFileSnapshotIterator {
    let mut snapshots = Vec::new();
    self.append_walk(&mut snapshots);
    RuntimeFileSnapshotIterator {
      snapshots: snapshots.into_iter(),
    }
  }
}

struct RuntimeFileSnapshotVisitorAdapter<'env> {
  env: Env,
  visitor: Object<'env>,
}

impl<'env> RuntimeFileSnapshotVisitorAdapter<'env> {
  fn new(env: Env, visitor: Object<'env>) -> Self {
    Self { env, visitor }
  }

  fn accept(&self, snapshot: &FileSnapshot) -> napi::Result<u32> {
    let snapshot_object = self.snapshot_object(snapshot)?;
    let result = self.call("visitEntry", snapshot_object)?;
    if matches!(snapshot, FileSnapshot::File { .. }) {
      return Ok(result);
    }
    match result {
      0 => {
        self.call_void("enterDirectory", snapshot_object)?;
        if let FileSnapshot::Directory { children, .. } = snapshot {
          for child in children {
            self.accept(child)?;
          }
        }
        self.call_void("leaveDirectory", snapshot_object)?;
        Ok(0)
      }
      1 => Ok(1),
      2 => Ok(0),
      _ => Ok(0),
    }
  }

  fn snapshot_object(&self, snapshot: &FileSnapshot) -> napi::Result<Object<'env>> {
    let raw = unsafe {
      RuntimeFileSnapshot::to_napi_value(
        self.env.raw(),
        RuntimeFileSnapshot::from_snapshot(snapshot.clone()),
      )?
    };
    Ok(Object::from_raw(self.env.raw(), raw))
  }

  fn call(&self, name: &str, snapshot: Object<'env>) -> napi::Result<u32> {
    let callback = self
      .visitor
      .get::<Function<FnArgs<(Object<'env>,)>, u32>>(name)?
      .ok_or_else(|| {
        napi::Error::from_reason(format!(
          "ARKDOWN_RUNTIME_TASK_CACHE_FAILED: snapshot visitor is missing {name}()"
        ))
      })?;
    callback.apply(self.visitor, FnArgs::from((snapshot,)))
  }

  fn call_void(&self, name: &str, snapshot: Object<'env>) -> napi::Result<()> {
    let callback = self
      .visitor
      .get::<Function<FnArgs<(Object<'env>,)>, Unknown<'env>>>(name)?
      .ok_or_else(|| {
        napi::Error::from_reason(format!(
          "ARKDOWN_RUNTIME_TASK_CACHE_FAILED: snapshot visitor is missing {name}()"
        ))
      })?;
    callback.apply(self.visitor, FnArgs::from((snapshot,)))?;
    Ok(())
  }
}

impl RuntimeFileSnapshot {
  fn from_snapshot(snapshot: FileSnapshot) -> Self {
    Self { snapshot }
  }

  fn append_walk(&self, snapshots: &mut Vec<Self>) {
    snapshots.push(Self {
      snapshot: self.snapshot.clone(),
    });
    if let FileSnapshot::Directory { children, .. } = &self.snapshot {
      for child in children {
        Self {
          snapshot: child.clone(),
        }
        .append_walk(snapshots);
      }
    }
  }
}

#[napi(iterator)]
pub struct RuntimeFileSnapshotIterator {
  snapshots: std::vec::IntoIter<RuntimeFileSnapshot>,
}

impl Generator for RuntimeFileSnapshotIterator {
  type Yield = RuntimeFileSnapshot;
  type Next = ();
  type Return = ();

  fn next(&mut self, _value: Option<Self::Next>) -> Option<Self::Yield> {
    self.snapshots.next()
  }
}

#[napi]
pub struct RuntimeTaskValueEntry {
  property: RuntimeTaskProperty,
}

#[napi]
impl RuntimeTaskValueEntry {
  #[napi]
  pub fn get_name(&self) -> String {
    self.property.name.clone()
  }

  #[napi(ts_return_type = "unknown")]
  pub fn get_value<'env>(&self, env: Env) -> napi::Result<Unknown<'env>> {
    let value = self.value_string();
    match self.property.value_type.as_str() {
      "boolean" => Self::unknown(env, value == "true"),
      "number" => value
        .parse::<f64>()
        .map_err(RuntimeTaskCacheSession::napi_error)
        .and_then(|value| Self::unknown(env, value)),
      "object" if value == "null" => Self::unknown(env, Null),
      "undefined" => Self::unknown(env, ()),
      _ => Self::unknown(env, value),
    }
  }

  fn value_string(&self) -> String {
    self
      .property
      .value
      .as_str()
      .map(str::to_string)
      .unwrap_or_else(|| self.property.value.to_string())
  }

  #[napi]
  pub fn get_value_type(&self) -> String {
    self.property.value_type.clone()
  }

  #[napi]
  pub fn get_hash(&self) -> String {
    RuntimeSnapshotHash::digest(format!(
      "{}{}{}",
      self.get_name(),
      self.value_string(),
      self.get_value_type()
    ))
  }

  fn value_type(value_type: ValueType) -> &'static str {
    match value_type {
      ValueType::Undefined => "undefined",
      ValueType::Null | ValueType::Object | ValueType::External => "object",
      ValueType::Boolean => "boolean",
      ValueType::Number => "number",
      ValueType::String => "string",
      ValueType::Symbol => "symbol",
      ValueType::Function => "function",
      ValueType::Unknown => "undefined",
    }
  }

  fn unknown<'env>(env: Env, value: impl ToNapiValue) -> napi::Result<Unknown<'env>> {
    let raw = unsafe { ToNapiValue::to_napi_value(env.raw(), value)? };
    Ok(unsafe { Unknown::from_raw_unchecked(env.raw(), raw) })
  }
}

/// Native snapshot model behind the public Hvigor `TaskProxy` result. The
/// class keeps comparison, hashing, success state, and serialization out of
/// `js/runtime.js` while retaining a JavaScript object with the official methods.
#[napi]
pub struct RuntimeTaskSnapshot {
  snapshot: TaskSnapshot,
  descriptor: RuntimeTaskDescriptor,
  successful: bool,
  execution_id: String,
}

#[napi]
impl RuntimeTaskSnapshot {
  #[napi]
  pub fn get_key(&self) -> String {
    Self::snapshot_key(&self.snapshot, &self.descriptor)
  }

  #[napi]
  pub fn compare_to(&self, other: &RuntimeTaskSnapshot) -> bool {
    self.successful
      && self.snapshot.project_name == other.snapshot.project_name
      && self.snapshot.module_name == other.snapshot.module_name
      && self.snapshot.task_name == other.snapshot.task_name
      && self.compare_inputs(other)
      && self.compare_input_files(other)
      && self.compare_output_files(other)
  }

  #[napi]
  pub fn get_module_name(&self) -> String {
    self.snapshot.module_name.clone()
  }

  #[napi]
  pub fn get_task_name(&self) -> String {
    self.snapshot.task_name.clone()
  }

  #[napi]
  pub fn get_project_name(&self) -> String {
    self.snapshot.project_name.clone()
  }

  #[napi]
  pub fn is_successful(&self) -> bool {
    self.successful
  }

  #[napi]
  pub fn is_incremental(&self) -> bool {
    self.descriptor.is_incremental()
  }

  #[napi]
  pub fn set_executed_successful(&mut self) {
    self.successful = true;
  }

  #[napi]
  pub fn add_input(&mut self, name: String, value: Unknown<'_>) -> napi::Result<()> {
    let value_type = RuntimeTaskValueEntry::value_type(value.get_type()?).to_string();
    let value = value.coerce_to_string()?.into_utf8()?.as_str()?.to_string();
    self
      .snapshot
      .properties
      .retain(|property| property.name != name);
    self.snapshot.properties.push(RuntimeTaskProperty {
      name,
      value: serde_json::Value::String(value),
      value_type,
    });
    Ok(())
  }

  #[napi]
  pub fn get_inputs(&self) -> Vec<RuntimeTaskValueEntry> {
    self
      .snapshot
      .properties
      .iter()
      .cloned()
      .map(|property| RuntimeTaskValueEntry { property })
      .collect()
  }

  #[napi]
  pub fn get_execution_id(&self) -> String {
    self.execution_id.clone()
  }

  #[napi]
  pub fn compare_inputs(&self, other: &RuntimeTaskSnapshot) -> bool {
    let mut left = self
      .get_inputs()
      .into_iter()
      .map(|entry| (entry.get_name(), entry.get_hash()))
      .collect::<Vec<_>>();
    let mut right = other
      .get_inputs()
      .into_iter()
      .map(|entry| (entry.get_name(), entry.get_hash()))
      .collect::<Vec<_>>();
    left.sort_by(|a, b| a.0.cmp(&b.0));
    right.sort_by(|a, b| a.0.cmp(&b.0));
    left == right
  }

  #[napi]
  pub fn compare_input_files(&self, other: &RuntimeTaskSnapshot) -> bool {
    self.descriptor.input_files == other.descriptor.input_files
  }

  #[napi]
  pub fn compare_output_files(&self, other: &RuntimeTaskSnapshot) -> bool {
    self.descriptor.output_files == other.descriptor.output_files
  }

  #[napi(ts_return_type = "Map<string, object>")]
  pub fn get_input_files<'env>(&self, env: Env) -> napi::Result<Unknown<'env>> {
    RuntimeTaskFileMapNapiAdapter::new(env).create(&self.descriptor.input_files)
  }

  #[napi(ts_return_type = "Map<string, object>")]
  pub fn get_output_files<'env>(&self, env: Env) -> napi::Result<Unknown<'env>> {
    RuntimeTaskFileMapNapiAdapter::new(env).create(&self.descriptor.output_files)
  }

  #[napi]
  pub fn update_files_options(&mut self, cache_service: Object<'_>) -> napi::Result<()> {
    let cache = cache_service
      .get::<&RuntimeTaskCacheSession>("nativeCache")?
      .ok_or_else(|| {
        napi::Error::from_reason(
          "ARKDOWN_RUNTIME_TASK_CACHE_FAILED: ProjectCacheService is missing nativeCache",
        )
      })?;
    self.update_file_snapshot_hashes(cache)
  }

  #[napi]
  pub fn update_cache_files_options(&mut self, cache_service: Object<'_>) -> napi::Result<()> {
    self.update_files_options(cache_service)
  }

  #[napi]
  pub fn stringify(&self) -> napi::Result<String> {
    let value = self
      .serialization_value()
      .map_err(RuntimeTaskCacheSession::napi_error)?;
    RuntimeTaskCacheSession::json(&value)
  }
}

impl RuntimeTaskSnapshot {
  fn from_descriptor(
    project_name: String,
    descriptor: RuntimeTaskDescriptor,
    successful: bool,
  ) -> Self {
    let snapshot = TaskSnapshot {
      project_name,
      module_name: descriptor.node_name.clone(),
      task_name: descriptor.task_name.clone(),
      properties: descriptor.input_properties.clone(),
      input_files: Vec::new(),
      output_files: Vec::new(),
    };
    Self::from_stored(snapshot, descriptor, successful)
  }

  fn from_stored(
    snapshot: TaskSnapshot,
    descriptor: RuntimeTaskDescriptor,
    successful: bool,
  ) -> Self {
    let execution_id = format!(
      "{}:{}",
      Self::snapshot_key(&snapshot, &descriptor),
      std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
    );
    Self {
      snapshot,
      descriptor,
      successful,
      execution_id,
    }
  }

  fn from_stored_snapshot(
    path: String,
    node_name: String,
    task_name: String,
    project_task: bool,
    stored: &StoredTaskSnapshot,
  ) -> Self {
    let descriptor = RuntimeTaskDescriptor {
      path,
      node_name,
      task_name,
      built_in: false,
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
      dependencies: Vec::new(),
      user_dependencies: Vec::new(),
      input_properties: stored.snapshot.properties.clone(),
      input_files: stored
        .snapshot
        .input_files
        .iter()
        .map(|file| file.declaration.clone())
        .collect(),
      output_files: stored
        .snapshot
        .output_files
        .iter()
        .map(|file| file.declaration.clone())
        .collect(),
    };
    Self::from_stored(stored.snapshot.clone(), descriptor, stored.successful)
  }

  fn update_file_snapshot_hashes(&mut self, cache: &RuntimeTaskCacheSession) -> napi::Result<()> {
    for declarations in [
      &mut self.descriptor.input_files,
      &mut self.descriptor.output_files,
    ] {
      for declaration in declarations.iter_mut() {
        let hash = cache
          .cache
          .file_snapshot(Path::new(declaration.path()))
          .map(FileSnapshot::hash_value);
        match declaration {
          RuntimeTaskFileDeclaration::Path(path) => {
            *declaration = RuntimeTaskFileDeclaration::Config(RuntimeTaskFileConfig {
              path: std::mem::take(path),
              is_directory: None,
              test: None,
              depth: None,
              file_snap_shot_hash_value: hash,
            });
          }
          RuntimeTaskFileDeclaration::Config(config) => {
            config.file_snap_shot_hash_value = hash;
          }
        }
      }
    }
    Ok(())
  }

  fn serialization_value(&self) -> Result<serde_json::Value, serde_json::Error> {
    let inputs = self
      .snapshot
      .properties
      .iter()
      .map(|property| {
        let entry = serde_json::json!({
          "_name": property.name,
          "_value": Self::serialized_property_value(property),
          "_valueType": property.value_type,
        });
        Ok(serde_json::json!({
          "dataType": "ValueEntry",
          "value": serde_json::to_string(&entry)?,
        }))
      })
      .collect::<Result<Vec<_>, serde_json::Error>>()?;
    Ok(serde_json::json!({
      "_inputs": inputs,
      "_successful": self.successful,
      "_projectName": self.snapshot.project_name,
      "_moduleName": self.snapshot.module_name,
      "_taskName": self.snapshot.task_name,
      "_key": self.get_key(),
      "_executionId": self.execution_id,
      "_inputFiles": Self::serialized_file_map(&self.descriptor.input_files),
      "_outputFiles": Self::serialized_file_map(&self.descriptor.output_files),
    }))
  }

  fn serialized_property_value(property: &RuntimeTaskProperty) -> serde_json::Value {
    let value = property
      .value
      .as_str()
      .map(str::to_string)
      .unwrap_or_else(|| property.value.to_string());
    match property.value_type.as_str() {
      "boolean" => serde_json::Value::Bool(value == "true"),
      "number" => value
        .parse::<f64>()
        .ok()
        .and_then(serde_json::Number::from_f64)
        .map(serde_json::Value::Number)
        .unwrap_or(serde_json::Value::Null),
      "object" if value == "null" => serde_json::Value::Null,
      _ => serde_json::Value::String(value),
    }
  }

  fn serialized_file_map(declarations: &[RuntimeTaskFileDeclaration]) -> serde_json::Value {
    let entries = declarations
      .iter()
      .map(|declaration| {
        let options = match declaration {
          RuntimeTaskFileDeclaration::Path(_) => serde_json::json!({}),
          RuntimeTaskFileDeclaration::Config(config) => {
            let mut options = serde_json::Map::new();
            if let Some(is_directory) = config.is_directory {
              options.insert("isDirectory".to_string(), serde_json::json!(is_directory));
            }
            if let Some(test) = &config.test {
              options.insert(
                "test".to_string(),
                serde_json::json!({ "dataType": "RegExp", "value": test.source }),
              );
            }
            if let Some(depth) = config.depth {
              options.insert("depth".to_string(), serde_json::json!(depth));
            }
            if let Some(hash) = &config.file_snap_shot_hash_value {
              options.insert("fileSnapShotHashValue".to_string(), serde_json::json!(hash));
            }
            serde_json::Value::Object(options)
          }
        };
        serde_json::json!([declaration.path(), options])
      })
      .collect::<Vec<_>>();
    serde_json::json!({ "dataType": "Map", "value": entries })
  }

  fn snapshot_key(snapshot: &TaskSnapshot, descriptor: &RuntimeTaskDescriptor) -> String {
    if snapshot.project_name.is_empty() {
      return descriptor.path.clone();
    }
    if descriptor.project_task {
      format!(":{}:{}", snapshot.project_name, snapshot.task_name)
    } else {
      format!(
        ":{}:{}:{}",
        snapshot.project_name, snapshot.module_name, snapshot.task_name
      )
    }
  }
}

struct RuntimeTaskFileMapNapiAdapter {
  env: Env,
}

impl RuntimeTaskFileMapNapiAdapter {
  fn new(env: Env) -> Self {
    Self { env }
  }

  fn create<'env>(
    &self,
    declarations: &[RuntimeTaskFileDeclaration],
  ) -> napi::Result<Unknown<'env>> {
    let global = self.env.get_global()?;
    let map_constructor: Function<FnArgs<(Array<'env>,)>, Unknown<'env>> =
      global.get_named_property("Map")?;
    let regexp_constructor: Function<FnArgs<(String, String)>, Unknown<'env>> =
      global.get_named_property("RegExp")?;
    let mut entries = self.env.create_array(declarations.len() as u32)?;
    for (index, declaration) in declarations.iter().enumerate() {
      let mut entry = self.env.create_array(2)?;
      entry.set(0, declaration.path().to_string())?;
      entry.set(1, self.options(declaration, &regexp_constructor)?)?;
      entries.set(index as u32, entry)?;
    }
    map_constructor.new_instance(FnArgs::from((entries,)))
  }

  fn options<'env>(
    &self,
    declaration: &RuntimeTaskFileDeclaration,
    regexp_constructor: &Function<'env, FnArgs<(String, String)>, Unknown<'env>>,
  ) -> napi::Result<Object<'env>> {
    let mut options = Object::new(&self.env)?;
    let RuntimeTaskFileDeclaration::Config(config) = declaration else {
      return Ok(options);
    };
    if let Some(is_directory) = config.is_directory {
      options.set("isDirectory", is_directory)?;
    }
    if let Some(test) = &config.test {
      options.set(
        "test",
        regexp_constructor.new_instance(FnArgs::from((test.source.clone(), test.flags.clone())))?,
      )?;
    }
    if let Some(depth) = config.depth {
      options.set("depth", depth)?;
    }
    if let Some(hash) = &config.file_snap_shot_hash_value {
      options.set("fileSnapShotHashValue", hash.clone())?;
    }
    Ok(options)
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredTaskCache {
  schema_version: u32,
  hvigor_version: String,
  #[serde(default)]
  entries: BTreeMap<String, StoredTaskSnapshot>,
  #[serde(default)]
  files: BTreeMap<PathBuf, FileSnapshot>,
}

impl StoredTaskCache {
  fn empty() -> Self {
    Self {
      schema_version: CACHE_SCHEMA_VERSION,
      hvigor_version: HVIGOR_PARITY_VERSION.to_string(),
      entries: BTreeMap::new(),
      files: BTreeMap::new(),
    }
  }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredTaskCacheRef<'a> {
  schema_version: u32,
  hvigor_version: &'a str,
  entries: &'a BTreeMap<String, StoredTaskSnapshot>,
  files: &'a BTreeMap<PathBuf, FileSnapshot>,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredTaskSnapshot {
  #[serde(flatten)]
  snapshot: TaskSnapshot,
  successful: bool,
}

#[cfg(test)]
mod tests {
  use super::super::model::RuntimeTaskFileConfig;
  use super::*;

  fn descriptor(root: &Path) -> RuntimeTaskDescriptor {
    RuntimeTaskDescriptor {
      path: "entry:fixture".to_string(),
      node_name: "entry".to_string(),
      task_name: "fixture".to_string(),
      built_in: false,
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
      input_properties: vec![RuntimeTaskProperty {
        name: "mode".to_string(),
        value: serde_json::json!("debug"),
        value_type: "string".to_string(),
      }],
      input_files: vec![RuntimeTaskFileDeclaration::Path(
        root.join("input.txt").to_string_lossy().into_owned(),
      )],
      output_files: vec![RuntimeTaskFileDeclaration::Path(
        root.join("output.txt").to_string_lossy().into_owned(),
      )],
    }
  }

  #[test]
  fn persists_and_invalidates_incremental_task_snapshots() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("input.txt"), "input").unwrap();
    fs::write(root.path().join("output.txt"), "output").unwrap();
    let task = descriptor(root.path());
    let mut cache = RuntimeTaskCache::new(root.path(), true);
    assert!(!cache.is_up_to_date(&task).unwrap());
    cache.record_successful(&task);
    cache.finish_cycle().unwrap();

    let cache = RuntimeTaskCache::new(root.path(), true);
    assert!(cache.is_up_to_date(&task).unwrap());
    let session =
      RuntimeTaskCacheSession::new(root.path().to_string_lossy().into_owned(), true).unwrap();
    let response = session
      .get_file(RuntimeTaskFileDeclarationInput {
        path: "input.txt".to_string(),
        is_directory: None,
        test: None,
        depth: None,
        file_snap_shot_hash_value: None,
      })
      .unwrap();
    assert_eq!(
      response.path(),
      root.path().join("input.txt").to_string_lossy(),
    );
    fs::write(root.path().join("output.txt"), "changed-output").unwrap();
    assert!(!cache.is_up_to_date(&task).unwrap());
  }

  #[test]
  fn fileset_filter_matches_hvigor_snapshot_selection() {
    let root = tempfile::tempdir().unwrap();
    let source_dir = root.path().join("sources");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(source_dir.join("included.ets"), "included").unwrap();
    fs::write(source_dir.join("ignored.txt"), "ignored").unwrap();
    fs::write(root.path().join("output.txt"), "output").unwrap();

    let mut task = descriptor(root.path());
    task.input_files = vec![RuntimeTaskFileDeclaration::Config(RuntimeTaskFileConfig {
      path: source_dir.to_string_lossy().into_owned(),
      is_directory: Some(true),
      test: Some(RuntimeTaskFileFilter {
        source: String::from(r"sources$|\.ets$"),
        flags: String::new(),
      }),
      depth: None,
      file_snap_shot_hash_value: None,
    })];

    let mut cache = RuntimeTaskCache::new(root.path(), true);
    cache.record_successful(&task);
    cache.finish_cycle().unwrap();
    let cache = RuntimeTaskCache::new(root.path(), true);
    assert!(cache.is_up_to_date(&task).unwrap());

    fs::write(source_dir.join("ignored.txt"), "ignored but changed").unwrap();
    assert!(cache.is_up_to_date(&task).unwrap());
    fs::write(source_dir.join("included.ets"), "included and changed").unwrap();
    assert!(!cache.is_up_to_date(&task).unwrap());
  }

  #[test]
  fn fileset_filter_rejects_javascript_stateful_flags_explicitly() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("input.txt"), "input").unwrap();
    fs::write(root.path().join("output.txt"), "output").unwrap();
    let mut task = descriptor(root.path());
    task.input_files = vec![RuntimeTaskFileDeclaration::Config(RuntimeTaskFileConfig {
      path: root.path().join("input.txt").to_string_lossy().into_owned(),
      is_directory: None,
      test: Some(RuntimeTaskFileFilter {
        source: String::from("input"),
        flags: String::from("g"),
      }),
      depth: None,
      file_snap_shot_hash_value: None,
    })];

    let snapshot_error = RuntimeTaskCache::new(root.path(), true)
      .snapshot(&task, true)
      .unwrap_err();
    assert!(matches!(
      snapshot_error,
      RuntimeTaskCacheError::UnsupportedFilterFlag { flag: 'g', .. }
    ));
  }
}
