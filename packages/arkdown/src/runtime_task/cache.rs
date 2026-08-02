use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use rayon::prelude::*;
use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize};

use super::model::{RuntimeTaskDescriptor, RuntimeTaskProperty};

const CACHE_SCHEMA_VERSION: u32 = 1;
const HVIGOR_PARITY_VERSION: &str = "6.24.3";

#[derive(Debug, thiserror::Error)]
pub enum RuntimeTaskCacheError {
  #[error("failed to find the incremental input file: {0}.")]
  MissingInput(String),
  #[error("snapshot runtime task path {path}: {source}")]
  Snapshot {
    path: String,
    #[source]
    source: std::io::Error,
  },
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
    let entries = Self::load_entries(&cache_path);
    Self {
      enabled,
      project_name,
      cache_path,
      entries,
      pending: BTreeMap::new(),
    }
  }

  pub fn begin_cycle(&mut self) {
    self.pending.clear();
  }

  pub fn is_up_to_date(&self, task: &RuntimeTaskDescriptor) -> Result<bool, RuntimeTaskCacheError> {
    if !self.enabled || !task.track_state || !task.is_incremental() {
      return Ok(false);
    }
    let Some(previous) = self.entries.get(&task.path) else {
      return Ok(false);
    };
    if !previous.successful {
      return Ok(false);
    }
    let current = self.snapshot(task, true)?;
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

  fn snapshot(
    &self,
    task: &RuntimeTaskDescriptor,
    require_inputs: bool,
  ) -> Result<TaskSnapshot, RuntimeTaskCacheError> {
    let mut properties = task.input_properties.clone();
    properties.sort_by(|left, right| left.name.cmp(&right.name));
    let mut input_files = FileSnapshotter::snapshot_paths(&task.input_files, require_inputs, true)?;
    let mut output_files = FileSnapshotter::snapshot_paths(&task.output_files, false, false)?;
    input_files.sort_by(|left, right| left.path.cmp(&right.path));
    output_files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(TaskSnapshot {
      project_name: self.project_name.clone(),
      module_name: task.node_name.clone(),
      task_name: task.task_name.clone(),
      properties,
      input_files,
      output_files,
    })
  }

  fn load_entries(cache_path: &Path) -> BTreeMap<String, StoredTaskSnapshot> {
    let Ok(content) = fs::read_to_string(cache_path) else {
      return BTreeMap::new();
    };
    let Ok(cache) = serde_json::from_str::<StoredTaskCache>(&content) else {
      return BTreeMap::new();
    };
    if cache.schema_version != CACHE_SCHEMA_VERSION || cache.hvigor_version != HVIGOR_PARITY_VERSION
    {
      return BTreeMap::new();
    }
    cache.entries
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
    })?;
    fs::write(&self.cache_path, content).map_err(|source| RuntimeTaskCacheError::Persist {
      path: self.cache_path.clone(),
      source,
    })
  }
}

struct FileSnapshotter;

impl FileSnapshotter {
  fn snapshot_paths(
    paths: &[String],
    required: bool,
    input: bool,
  ) -> Result<Vec<DeclaredFileSnapshot>, RuntimeTaskCacheError> {
    let mut unique = BTreeMap::new();
    for path in paths {
      unique.entry(path.clone()).or_insert_with(|| path.clone());
    }
    unique
      .into_values()
      .collect::<Vec<_>>()
      .into_par_iter()
      .map(|path| {
        let snapshot = Self::snapshot_path(&path, required && input)?;
        Ok(DeclaredFileSnapshot { path, snapshot })
      })
      .collect()
  }

  fn snapshot_path(
    declared_path: &str,
    required: bool,
  ) -> Result<Option<FileSnapshot>, RuntimeTaskCacheError> {
    let path = Path::new(declared_path);
    if !path.exists() {
      if required {
        return Err(RuntimeTaskCacheError::MissingInput(
          declared_path.to_string(),
        ));
      }
      return Ok(None);
    }
    let mut visited_links = FxHashSet::default();
    Self::visit(path, &mut visited_links).map_err(|source| RuntimeTaskCacheError::Snapshot {
      path: declared_path.to_string(),
      source,
    })
  }

  fn visit(
    path: &Path,
    visited_links: &mut FxHashSet<PathBuf>,
  ) -> std::io::Result<Option<FileSnapshot>> {
    let metadata = match fs::metadata(path) {
      Ok(metadata) => metadata,
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
      Err(error) => return Err(error),
    };
    let link_metadata = fs::symlink_metadata(path)?;
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
    let path_string = path.to_string_lossy().into_owned();
    if metadata.is_file() {
      let modified_time = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);
      return Ok(Some(FileSnapshot::File {
        name,
        path: path_string,
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
      if let Some(snapshot) = Self::visit(&child, visited_links)? {
        children.push(snapshot);
      }
    }
    Ok(Some(FileSnapshot::Directory {
      name,
      path: path_string,
      symbolic_link,
      children,
    }))
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
  path: String,
  snapshot: Option<FileSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
  tag = "kind",
  rename_all = "lowercase",
  rename_all_fields = "camelCase"
)]
enum FileSnapshot {
  File {
    name: String,
    path: String,
    symbolic_link: bool,
    size: u64,
    modified_time: u64,
  },
  Directory {
    name: String,
    path: String,
    symbolic_link: bool,
    children: Vec<FileSnapshot>,
  },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredTaskCache {
  schema_version: u32,
  hvigor_version: String,
  #[serde(default)]
  entries: BTreeMap<String, StoredTaskSnapshot>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredTaskCacheRef<'a> {
  schema_version: u32,
  hvigor_version: &'a str,
  entries: &'a BTreeMap<String, StoredTaskSnapshot>,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredTaskSnapshot {
  #[serde(flatten)]
  snapshot: TaskSnapshot,
  successful: bool,
}

#[cfg(test)]
mod tests {
  use super::*;

  fn descriptor(root: &Path) -> RuntimeTaskDescriptor {
    RuntimeTaskDescriptor {
      path: "entry:fixture".to_string(),
      node_name: "entry".to_string(),
      task_name: "fixture".to_string(),
      built_in: false,
      project_task: false,
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
        value: "debug".to_string(),
        value_type: "string".to_string(),
      }],
      input_files: vec![root.join("input.txt").to_string_lossy().into_owned()],
      output_files: vec![root.join("output.txt").to_string_lossy().into_owned()],
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
    fs::write(root.path().join("output.txt"), "changed-output").unwrap();
    assert!(!cache.is_up_to_date(&task).unwrap());
  }
}
