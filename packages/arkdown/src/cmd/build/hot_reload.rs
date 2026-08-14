//! Hvigor-compatible persistent HotReload compilation planning.
//!
//! Upstream owners:
//! - `tasks/debugger/build-hot-reload-resource.js`
//! - `tasks/hot-reload-ark-compile.js`
//! - SDK `module_hotreload_mode.js`

use std::collections::{BTreeSet, HashMap};
use std::path::{Component, Path, PathBuf};

use arkdown_core::HvigorModuleSourceFile;
use arkdown_path::lexical_normalize;
use arkdown_watcher::{WatchEvent, WatchEventKind};
use indexmap::IndexSet;

use super::args::NativeTaskRoot;
use super::bytecode::{Ts2abcReloadMode, Ts2abcReloadPlan};
use super::layout::ModuleBuildLayout;

#[derive(Debug, thiserror::Error)]
pub(super) enum HotReloadError {
  #[error(
    "ARKDOWN_HOT_RELOAD_PROHIBIT: HotReload requires a Stage-model esmodule with compile API >= 9 (compileMode={compile_mode}, compileApi={compile_api})"
  )]
  Prohibited {
    compile_mode: String,
    compile_api: u32,
  },
  #[error("ARKDOWN_HOT_RELOAD_PATCH_CONFIG: HotReload requires buildConfig.patchConfig")]
  MissingPatchConfig,
  #[error("ARKDOWN_HOT_RELOAD_PATCH_MODE: HotReload requires patchConfig.mode=hotReload, got {0}")]
  InvalidPatchMode(String),
  #[error("ARKDOWN_HOT_RELOAD_PATCH_FIELD: patchConfig.{field} must be a non-empty path")]
  InvalidPatchPath { field: &'static str },
  #[error("ARKDOWN_HOT_RELOAD_CHANGED_LIST: {path}: {message}")]
  InvalidChangedList { path: PathBuf, message: String },
  #[error("ARKDOWN_HOT_RELOAD_SOURCE_MAP_MISSING: HotReload source map is missing: {0}")]
  MissingSourceMap(PathBuf),
  #[error("ARKDOWN_HOT_RELOAD_IO: {path}: {source}")]
  Io {
    path: PathBuf,
    #[source]
    source: std::io::Error,
  },
  #[error("ARKDOWN_HOT_RELOAD_JSON: {path}: {source}")]
  Json {
    path: PathBuf,
    #[source]
    source: serde_json::Error,
  },
}

pub(super) type HotReloadResult<T> = Result<T, HotReloadError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HotReloadTaskBoundary {
  Compile,
  Build,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct HotReloadTaskPlan {
  boundary: HotReloadTaskBoundary,
}

impl HotReloadTaskPlan {
  pub(super) fn for_module(roots: &[NativeTaskRoot], module_name: &str) -> Option<Self> {
    roots
      .iter()
      .filter(|root| root.module_name == module_name)
      .filter_map(|root| match root.task_name.as_str() {
        "HotReloadArkTS" => Some(Self {
          boundary: HotReloadTaskBoundary::Compile,
        }),
        "HotReloadBuild" => Some(Self {
          boundary: HotReloadTaskBoundary::Build,
        }),
        _ => None,
      })
      .max_by_key(|plan| match plan.boundary {
        HotReloadTaskBoundary::Compile => 0,
        HotReloadTaskBoundary::Build => 1,
      })
  }

  pub(super) fn boundary(self) -> HotReloadTaskBoundary {
    self.boundary
  }

  pub(super) fn requested(roots: &[NativeTaskRoot]) -> bool {
    roots
      .iter()
      .any(|root| matches!(root.task_name.as_str(), "HotReloadArkTS" | "HotReloadBuild"))
  }
}

/// The targetless resource hook is a dependency of both hot- and cold-reload
/// compilation. It may also be selected directly by DevEco.
pub(super) struct ReloadResourceTaskPlan;

impl ReloadResourceTaskPlan {
  pub(super) fn requested_for_module(roots: &[NativeTaskRoot], module_name: &str) -> bool {
    roots.iter().any(|root| {
      root.module_name == module_name
        && matches!(
          root.task_name.as_str(),
          "buildHotReloadResource"
            | "HotReloadArkTS"
            | "HotReloadBuild"
            | "ColdReloadArkTS"
            | "PrepareQuickfix"
            | "PackageHqf"
            | "SignHqf"
            | "assembleDevHqf"
        )
    })
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HotReloadCompilePhase {
  Initial,
  Incremental,
}

#[derive(Debug, Clone)]
pub(super) struct HotReloadCompilePlan {
  pub(super) phase: HotReloadCompilePhase,
  pub(super) module_source_files: Vec<HvigorModuleSourceFile>,
  pub(super) modules_abc_path: PathBuf,
  pub(super) reload: Ts2abcReloadPlan,
  /// Exact `entryObj` replacement used by the normalized-OHM compile-context
  /// generator on incremental builds.
  pub(super) compile_entry_override: Option<Vec<String>>,
}

struct HotReloadPatchConfig {
  old_map_dir: PathBuf,
  changed_file_list: Option<PathBuf>,
  patch_abc_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChangedFileListVersion {
  V1,
  V2,
}

struct HotReloadChangedFiles {
  version: ChangedFileListVersion,
  paths: Vec<String>,
}

pub(super) struct HotReloadProcessor<'a> {
  layout: &'a ModuleBuildLayout<'a>,
  project_root: &'a Path,
}

impl<'a> HotReloadProcessor<'a> {
  pub(super) fn new(layout: &'a ModuleBuildLayout<'a>, project_root: &'a Path) -> Self {
    Self {
      layout,
      project_root,
    }
  }

  pub(super) fn compile_plan(
    &self,
    patch_config: Option<&serde_json::Value>,
    compile_mode: &str,
    compile_api: u32,
    incremental: bool,
    module_source_files: &[HvigorModuleSourceFile],
    base_modules_abc_path: &Path,
    source_maps_path: Option<&Path>,
  ) -> HotReloadResult<Option<HotReloadCompilePlan>> {
    self.validate_resource_model(compile_mode, compile_api)?;
    let config = self.parse_patch_config(patch_config)?;
    let symbol_map_path = config.old_map_dir.join("symbolMap.map");
    if !incremental {
      return Ok(Some(HotReloadCompilePlan {
        phase: HotReloadCompilePhase::Initial,
        module_source_files: module_source_files.to_vec(),
        modules_abc_path: base_modules_abc_path.to_path_buf(),
        reload: Ts2abcReloadPlan {
          mode: Ts2abcReloadMode::Dump,
          symbol_table_path: symbol_map_path,
        },
        compile_entry_override: None,
      }));
    }

    let Some(changed_file_list) = config.changed_file_list else {
      return Err(HotReloadError::InvalidPatchPath {
        field: "changedFileList",
      });
    };
    if !changed_file_list.is_file() {
      // SDK ModuleHotreloadMode logs and skips when the configured list has
      // not been materialized yet.
      return Ok(None);
    }
    let changed = self.read_changed_files(&changed_file_list)?;
    if changed.paths.is_empty() || changed.paths.iter().any(|path| path.ends_with(".json")) {
      return Ok(None);
    }
    let compile_entry_override = changed
      .paths
      .iter()
      .map(|path| match changed.version {
        ChangedFileListVersion::V1 => self
          .layout
          .source()
          .ets()
          .join(path)
          .to_string_lossy()
          .into_owned(),
        ChangedFileListVersion::V2 => path.clone(),
      })
      .collect::<Vec<_>>();
    let changed_paths = changed
      .paths
      .iter()
      .map(|path| self.resolve_changed_path(path, changed.version))
      .collect::<Vec<_>>();
    let selected = self.select_module_sources(module_source_files, &changed_paths);
    if selected.is_empty() {
      return Ok(None);
    }

    let Some(patch_abc_dir) = config.patch_abc_dir else {
      return Err(HotReloadError::InvalidPatchPath {
        field: "patchAbcPath",
      });
    };
    let source_maps_path = source_maps_path
      .filter(|path| path.is_file())
      .ok_or_else(|| {
        HotReloadError::MissingSourceMap(
          source_maps_path
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.layout.loader_out_source_maps()),
        )
      })?;
    self.write_patch_source_maps(source_maps_path, &patch_abc_dir, &selected)?;

    Ok(Some(HotReloadCompilePlan {
      phase: HotReloadCompilePhase::Incremental,
      module_source_files: selected,
      modules_abc_path: patch_abc_dir.join("modules.abc"),
      reload: Ts2abcReloadPlan {
        mode: Ts2abcReloadMode::Hot,
        symbol_table_path: symbol_map_path,
      },
      compile_entry_override: Some(compile_entry_override),
    }))
  }

  /// Materialize the same v1 changed-file handoff produced by the SDK
  /// `watchChangedFiles.beforeBuild` plugin. The plugin accumulates modified
  /// files for the lifetime of the watch worker and records paths relative to
  /// `projectConfig.projectPath` (`src/main/ets`).
  pub(super) fn write_watch_changed_file_list(
    &self,
    patch_config: Option<&serde_json::Value>,
    events: &[WatchEvent],
  ) -> HotReloadResult<()> {
    let config = self.parse_patch_config(patch_config)?;
    if patch_config
      .and_then(|config| config.get("removeChangedFileListInSdk"))
      .and_then(serde_json::Value::as_str)
      == Some("true")
    {
      return Ok(());
    }
    let changed_file_list = config
      .changed_file_list
      .ok_or(HotReloadError::InvalidPatchPath {
        field: "changedFileList",
      })?;
    let mut modified_files = IndexSet::new();
    if changed_file_list.is_file() {
      let bytes = std::fs::read(&changed_file_list).map_err(|source| HotReloadError::Io {
        path: changed_file_list.clone(),
        source,
      })?;
      let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|source| HotReloadError::Json {
          path: changed_file_list.clone(),
          source,
        })?;
      if let Some(paths) = value
        .get("modifiedFiles")
        .and_then(serde_json::Value::as_array)
      {
        modified_files.extend(
          paths
            .iter()
            .filter_map(serde_json::Value::as_str)
            .map(str::to_string),
        );
      }
    }
    let source_root = self.layout.source().ets();
    // `rollup-plugin-watch-change.js::watchChangeFiles` resets each cache
    // array while `projectConfig.hotReload` is true. Consequently only the
    // last update and the last create/delete event reach `beforeBuild`.
    let modified = events
      .iter()
      .rev()
      .find(|event| matches!(event.kind, WatchEventKind::Modified));
    let removed = events.iter().rev().find(|event| {
      matches!(
        event.kind,
        WatchEventKind::Created | WatchEventKind::Deleted
      )
    });
    if let Some(event) = modified.filter(|event| event.path.is_file()) {
      modified_files.insert(
        Self::relative_path(&source_root, &event.path)
          .to_string_lossy()
          .into_owned(),
      );
    }
    let removed_files = removed
      .map(|event| {
        Self::relative_path(&source_root, &event.path)
          .to_string_lossy()
          .into_owned()
      })
      .into_iter()
      .collect::<IndexSet<_>>();
    modified_files.retain(|path| !removed_files.contains(path));
    if let Some(parent) = changed_file_list.parent() {
      std::fs::create_dir_all(parent).map_err(|source| HotReloadError::Io {
        path: parent.to_path_buf(),
        source,
      })?;
    }
    let bytes = serde_json::to_vec(&serde_json::json!({
      "modifiedFiles": modified_files,
      "removedFiles": removed_files,
    }))
    .map_err(|source| HotReloadError::Json {
      path: changed_file_list.clone(),
      source,
    })?;
    std::fs::write(&changed_file_list, bytes).map_err(|source| HotReloadError::Io {
      path: changed_file_list,
      source,
    })
  }

  pub(super) fn validate_resource_model(
    &self,
    compile_mode: &str,
    compile_api: u32,
  ) -> HotReloadResult<()> {
    if compile_mode.eq_ignore_ascii_case("esmodule") && compile_api >= 9 {
      return Ok(());
    }
    Err(HotReloadError::Prohibited {
      compile_mode: compile_mode.to_string(),
      compile_api,
    })
  }

  fn parse_patch_config(
    &self,
    patch_config: Option<&serde_json::Value>,
  ) -> HotReloadResult<HotReloadPatchConfig> {
    let patch_config = patch_config
      .and_then(serde_json::Value::as_object)
      .ok_or(HotReloadError::MissingPatchConfig)?;
    let mode = patch_config
      .get("mode")
      .and_then(serde_json::Value::as_str)
      .unwrap_or_default();
    if mode != "hotReload" {
      return Err(HotReloadError::InvalidPatchMode(mode.to_string()));
    }
    let old_map_dir = self.required_config_path(patch_config, "oldMapFilePath")?;
    let changed_file_list = self.optional_config_path(patch_config, "changedFileList")?;
    let patch_abc_dir = self.optional_config_path(patch_config, "patchAbcPath")?;
    Ok(HotReloadPatchConfig {
      old_map_dir,
      changed_file_list,
      patch_abc_dir,
    })
  }

  fn required_config_path(
    &self,
    patch_config: &serde_json::Map<String, serde_json::Value>,
    field: &'static str,
  ) -> HotReloadResult<PathBuf> {
    self
      .optional_config_path(patch_config, field)?
      .ok_or(HotReloadError::InvalidPatchPath { field })
  }

  fn optional_config_path(
    &self,
    patch_config: &serde_json::Map<String, serde_json::Value>,
    field: &'static str,
  ) -> HotReloadResult<Option<PathBuf>> {
    let Some(value) = patch_config.get(field) else {
      return Ok(None);
    };
    let raw = value
      .as_str()
      .filter(|path| !path.is_empty())
      .ok_or(HotReloadError::InvalidPatchPath { field })?;
    let path = PathBuf::from(raw);
    Ok(Some(if path.is_absolute() {
      lexical_normalize(&path)
    } else {
      lexical_normalize(&self.project_root.join(path))
    }))
  }

  fn read_changed_files(&self, path: &Path) -> HotReloadResult<HotReloadChangedFiles> {
    let bytes = std::fs::read(path).map_err(|source| HotReloadError::Io {
      path: path.to_path_buf(),
      source,
    })?;
    let value: serde_json::Value =
      serde_json::from_slice(&bytes).map_err(|source| HotReloadError::Json {
        path: path.to_path_buf(),
        source,
      })?;
    let object = value
      .as_object()
      .ok_or_else(|| HotReloadError::InvalidChangedList {
        path: path.to_path_buf(),
        message: "root must be an object".to_string(),
      })?;
    if let Some(v2) = object.get("modifiedFilesV2") {
      let entries = v2
        .as_array()
        .ok_or_else(|| HotReloadError::InvalidChangedList {
          path: path.to_path_buf(),
          message: "modifiedFilesV2 must be an array".to_string(),
        })?;
      let mut paths = Vec::new();
      for entry in entries {
        let Some(entry) = entry.as_object() else {
          continue;
        };
        let Some(file_path) = entry.get("filePath") else {
          continue;
        };
        let file_path = file_path
          .as_str()
          .ok_or_else(|| HotReloadError::InvalidChangedList {
            path: path.to_path_buf(),
            message: "modifiedFilesV2[].filePath must be a string".to_string(),
          })?;
        paths.push(file_path.to_string());
      }
      return Ok(HotReloadChangedFiles {
        version: ChangedFileListVersion::V2,
        paths,
      });
    }
    if let Some(v1) = object.get("modifiedFiles") {
      let entries = v1
        .as_array()
        .ok_or_else(|| HotReloadError::InvalidChangedList {
          path: path.to_path_buf(),
          message: "modifiedFiles must be an array".to_string(),
        })?;
      let paths = entries
        .iter()
        .map(|entry| {
          entry
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| HotReloadError::InvalidChangedList {
              path: path.to_path_buf(),
              message: "modifiedFiles[] must be a string".to_string(),
            })
        })
        .collect::<HotReloadResult<Vec<_>>>()?;
      return Ok(HotReloadChangedFiles {
        version: ChangedFileListVersion::V1,
        paths,
      });
    }
    Ok(HotReloadChangedFiles {
      version: ChangedFileListVersion::V1,
      paths: Vec::new(),
    })
  }

  fn resolve_changed_path(&self, path: &str, version: ChangedFileListVersion) -> PathBuf {
    let path = Path::new(path);
    let resolved = match version {
      ChangedFileListVersion::V1 => self.layout.source().ets().join(path),
      ChangedFileListVersion::V2 if path.is_absolute() => path.to_path_buf(),
      ChangedFileListVersion::V2 => self.project_root.join(path),
    };
    self.canonical_or_lexical(&resolved)
  }

  fn select_module_sources(
    &self,
    module_source_files: &[HvigorModuleSourceFile],
    changed_paths: &[PathBuf],
  ) -> Vec<HvigorModuleSourceFile> {
    let mut source_by_path = HashMap::with_capacity(module_source_files.len());
    for (index, source) in module_source_files.iter().enumerate() {
      let module_id = source
        .module_id
        .split(['?', '#'])
        .next()
        .unwrap_or(source.module_id.as_str());
      source_by_path
        .entry(self.canonical_or_lexical(Path::new(module_id)))
        .or_insert(index);
    }
    let mut selected = Vec::new();
    let mut selected_indices = BTreeSet::new();
    for path in changed_paths {
      let Some(index) = source_by_path.get(path).copied() else {
        continue;
      };
      if selected_indices.insert(index) {
        selected.push(module_source_files[index].clone());
      }
    }
    selected
  }

  fn write_patch_source_maps(
    &self,
    input: &Path,
    patch_abc_dir: &Path,
    selected: &[HvigorModuleSourceFile],
  ) -> HotReloadResult<()> {
    let bytes = std::fs::read(input).map_err(|source| HotReloadError::Io {
      path: input.to_path_buf(),
      source,
    })?;
    let source_maps: serde_json::Value =
      serde_json::from_slice(&bytes).map_err(|source| HotReloadError::Json {
        path: input.to_path_buf(),
        source,
      })?;
    let source_maps =
      source_maps
        .as_object()
        .ok_or_else(|| HotReloadError::InvalidChangedList {
          path: input.to_path_buf(),
          message: "sourceMaps.map root must be an object".to_string(),
        })?;
    let mut filtered = serde_json::Map::new();
    for source in selected {
      if let Some(value) = source_maps.get(&source.source_file) {
        filtered.insert(source.source_file.clone(), value.clone());
      }
    }
    std::fs::create_dir_all(patch_abc_dir).map_err(|source| HotReloadError::Io {
      path: patch_abc_dir.to_path_buf(),
      source,
    })?;
    let output = patch_abc_dir.join("sourceMaps.map");
    let bytes =
      serde_json::to_vec_pretty(&serde_json::Value::Object(filtered)).map_err(|source| {
        HotReloadError::Json {
          path: output.clone(),
          source,
        }
      })?;
    std::fs::write(&output, bytes).map_err(|source| HotReloadError::Io {
      path: output,
      source,
    })
  }

  fn canonical_or_lexical(&self, path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| lexical_normalize(path))
  }

  /// Platform-native lexical equivalent of Node's `path.relative`.
  fn relative_path(from: &Path, to: &Path) -> PathBuf {
    let from = lexical_normalize(from);
    let to = lexical_normalize(to);
    let from_components = from.components().collect::<Vec<_>>();
    let to_components = to.components().collect::<Vec<_>>();
    let same_anchor = match (from_components.first(), to_components.first()) {
      (Some(Component::Prefix(left)), Some(Component::Prefix(right))) => left == right,
      (Some(Component::RootDir), Some(Component::RootDir)) => true,
      (Some(Component::Prefix(_) | Component::RootDir), _)
      | (_, Some(Component::Prefix(_) | Component::RootDir)) => false,
      _ => true,
    };
    if !same_anchor {
      return to;
    }
    let common = from_components
      .iter()
      .zip(&to_components)
      .take_while(|(left, right)| left == right)
      .count();
    let mut relative = PathBuf::new();
    for component in &from_components[common..] {
      if matches!(component, Component::Normal(_)) {
        relative.push("..");
      }
    }
    for component in &to_components[common..] {
      relative.push(component.as_os_str());
    }
    relative
  }
}
