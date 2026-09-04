//! Hvigor-compatible ColdReload and HQF artifact preparation.
//!
//! Upstream owners:
//! - `tasks/cold-reload-ark-compile.js`
//! - SDK `module_coldreload_mode.js`
//! - `tasks/prepare-quickfix.js`
//! - `tasks/package-hqf.js`
//! - `tasks/sign-hqf.js`

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use arkdown_abc::bytecode::{Ts2abcReloadMode, Ts2abcReloadPlan};
use arkdown_core::{HvigorModuleSourceFile, NativeDebugSymbolOption};
use arkdown_pack::{AppPackageRunner, HqfPackagePlan};
use arkdown_path::lexical_normalize;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::args::NativeTaskRoot;
use super::layout::ModuleBuildLayout;

const INITIAL_PATCH_VERSION: u64 = 3_000_000;

#[derive(Debug, thiserror::Error)]
pub(super) enum QuickFixError {
  #[error("ARKDOWN_QUICKFIX_IO: {path}: {source}")]
  Io {
    path: PathBuf,
    #[source]
    source: std::io::Error,
  },
  #[error("ARKDOWN_QUICKFIX_JSON: {path}: {source}")]
  Json {
    path: PathBuf,
    #[source]
    source: serde_json::Error,
  },
  #[error(
    "ARKDOWN_QUICKFIX_SYMBOL_MAP_MISSING: ColdReload requires the base-build symbol map: {0}"
  )]
  MissingSymbolMap(PathBuf),
  #[error("ARKDOWN_QUICKFIX_SOURCE_MAP_MISSING: ColdReload source map is missing: {0}")]
  MissingSourceMap(PathBuf),
  #[error("ARKDOWN_QUICKFIX_RESOURCE_PATH: {0}")]
  InvalidResourcePath(String),
  #[error(
    "ARKDOWN_QUICKFIX_HVIGOR_CACHE_DIR: the runtime must provide Hvigor's resolved cache directory"
  )]
  MissingHvigorCacheDir,
  #[error("ARKDOWN_QUICKFIX_PACK: {0}")]
  Pack(#[from] arkdown_pack::PackError),
}

pub(super) type QuickFixResult<T> = Result<T, QuickFixError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum QuickFixTaskBoundary {
  ColdReload,
  Prepare,
  Package,
  Sign,
  Assemble,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct QuickFixTaskPlan {
  boundary: QuickFixTaskBoundary,
}

impl QuickFixTaskPlan {
  pub(super) fn for_module(roots: &[NativeTaskRoot], module_name: &str) -> Option<Self> {
    roots
      .iter()
      .filter(|root| root.module_name == module_name)
      .filter_map(|root| {
        let boundary = match root.task_name.as_str() {
          "ColdReloadArkTS" => QuickFixTaskBoundary::ColdReload,
          "PrepareQuickfix" => QuickFixTaskBoundary::Prepare,
          "PackageHqf" => QuickFixTaskBoundary::Package,
          "SignHqf" => QuickFixTaskBoundary::Sign,
          "assembleDevHqf" => QuickFixTaskBoundary::Assemble,
          _ => return None,
        };
        Some(Self { boundary })
      })
      .max_by_key(|plan| plan.rank())
  }

  pub(super) fn boundary(self) -> QuickFixTaskBoundary {
    self.boundary
  }

  pub(super) fn patch_artifacts_requested(roots: &[NativeTaskRoot]) -> bool {
    roots.iter().any(|root| {
      matches!(
        root.task_name.as_str(),
        "PrepareQuickfix" | "PackageHqf" | "SignHqf" | "assembleDevHqf"
      )
    })
  }

  pub(super) fn needs_patch_artifacts(self) -> bool {
    !matches!(self.boundary, QuickFixTaskBoundary::ColdReload)
  }

  fn rank(self) -> u8 {
    match self.boundary {
      QuickFixTaskBoundary::ColdReload => 0,
      QuickFixTaskBoundary::Prepare => 1,
      QuickFixTaskBoundary::Package => 2,
      QuickFixTaskBoundary::Sign => 3,
      QuickFixTaskBoundary::Assemble => 4,
    }
  }
}

#[derive(Debug, Clone)]
pub(super) struct ColdReloadCompilePlan {
  pub(super) module_source_files: Vec<HvigorModuleSourceFile>,
  pub(super) modules_abc_path: PathBuf,
  pub(super) reload: Ts2abcReloadPlan,
}

pub(super) struct ColdReloadProcessor<'a> {
  layout: &'a ModuleBuildLayout<'a>,
}

impl<'a> ColdReloadProcessor<'a> {
  pub(super) fn new(layout: &'a ModuleBuildLayout<'a>) -> Self {
    Self { layout }
  }

  pub(super) fn compile_plan(
    &self,
    patch_config: Option<&serde_json::Value>,
    module_source_files: &[HvigorModuleSourceFile],
    source_maps_path: Option<&Path>,
  ) -> QuickFixResult<Option<ColdReloadCompilePlan>> {
    if patch_config.is_none() {
      return Ok(None);
    }
    let changed_file_list = self.layout.patch_changed_file_list();
    if !changed_file_list.is_file() {
      return Ok(None);
    }
    let changed: ChangedFileList = self.read_json(&changed_file_list)?;
    if changed.modified_files.is_empty()
      || changed
        .modified_files
        .iter()
        .any(|path| Path::new(path).extension().and_then(|ext| ext.to_str()) == Some("json"))
    {
      return Ok(None);
    }

    let ets_root = self.layout.source().ets();
    let changed_paths = changed
      .modified_files
      .iter()
      .map(|path| lexical_normalize(&ets_root.join(path)))
      .collect::<BTreeSet<_>>();
    let selected = module_source_files
      .iter()
      .filter(|source| {
        let module_id = source
          .module_id
          .split(['?', '#'])
          .next()
          .unwrap_or(source.module_id.as_str());
        changed_paths.contains(&lexical_normalize(Path::new(module_id)))
      })
      .cloned()
      .collect::<Vec<_>>();
    if selected.is_empty() {
      return Ok(None);
    }

    let symbol_map = self.layout.symbol_map();
    if !symbol_map.is_file() {
      return Err(QuickFixError::MissingSymbolMap(symbol_map));
    }
    let source_maps_path = source_maps_path
      .filter(|path| path.is_file())
      .ok_or_else(|| {
        QuickFixError::MissingSourceMap(
          source_maps_path
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.layout.loader_out_source_maps()),
        )
      })?;
    self.write_source_maps(source_maps_path, &selected)?;
    Ok(Some(ColdReloadCompilePlan {
      module_source_files: selected,
      modules_abc_path: self.layout.patch_modules_abc(),
      reload: Ts2abcReloadPlan {
        mode: Ts2abcReloadMode::Cold,
        symbol_table_path: symbol_map,
      },
    }))
  }

  fn write_source_maps(
    &self,
    input: &Path,
    selected: &[HvigorModuleSourceFile],
  ) -> QuickFixResult<()> {
    let source_maps: serde_json::Value = self.read_json(input)?;
    let source_maps = source_maps.as_object().ok_or_else(|| QuickFixError::Json {
      path: input.to_path_buf(),
      source: serde_json::Error::io(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "sourceMaps.map root must be an object",
      )),
    })?;
    let selected_keys = selected
      .iter()
      .map(|source| source.source_file.as_str())
      .collect::<BTreeSet<_>>();
    let filtered = source_maps
      .iter()
      .filter(|(key, _)| selected_keys.contains(key.as_str()))
      .map(|(key, value)| (key.clone(), value.clone()))
      .collect::<serde_json::Map<_, _>>();
    let output = self.layout.patch_source_maps();
    if let Some(parent) = output.parent() {
      std::fs::create_dir_all(parent).map_err(|source| QuickFixError::Io {
        path: parent.to_path_buf(),
        source,
      })?;
    }
    let bytes = serde_json::to_vec(&serde_json::Value::Object(filtered)).map_err(|source| {
      QuickFixError::Json {
        path: output.clone(),
        source,
      }
    })?;
    std::fs::write(&output, bytes).map_err(|source| QuickFixError::Io {
      path: output,
      source,
    })
  }

  fn read_json<T: serde::de::DeserializeOwned>(&self, path: &Path) -> QuickFixResult<T> {
    let bytes = std::fs::read(path).map_err(|source| QuickFixError::Io {
      path: path.to_path_buf(),
      source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| QuickFixError::Json {
      path: path.to_path_buf(),
      source,
    })
  }
}

pub(super) struct QuickFixArtifactInput<'a> {
  pub(super) layout: &'a ModuleBuildLayout<'a>,
  pub(super) module_name: &'a str,
  pub(super) target_name: &'a str,
  pub(super) bundle_name: &'a str,
  pub(super) version_code: u64,
  pub(super) version_name: &'a str,
  pub(super) device_types: &'a [String],
  pub(super) debug_symbol: Option<&'a NativeDebugSymbolOption>,
  pub(super) patch_version: u64,
}

pub(super) struct QuickFixArtifactProcessor<'a> {
  input: QuickFixArtifactInput<'a>,
}

impl<'a> QuickFixArtifactProcessor<'a> {
  pub(super) fn new(input: QuickFixArtifactInput<'a>) -> Self {
    Self { input }
  }

  pub(super) fn unsigned_hqf(&self) -> PathBuf {
    self
      .input
      .layout
      .target_output_dir()
      .join(self.hqf_name(false))
  }

  pub(super) fn signed_hqf(&self) -> PathBuf {
    self
      .input
      .layout
      .target_output_dir()
      .join(self.hqf_name(true))
  }

  pub(super) fn prepare(&self, patch_config: Option<&serde_json::Value>) -> QuickFixResult<()> {
    for directory in [
      self.input.layout.patch_ets_dir(),
      self.input.layout.patch_libs_dir(),
      self.input.layout.patch_resources_dir(),
    ] {
      self.ensure_directory(&directory)?;
    }
    let native_changes = self.collect_native_libs()?;
    let (ets_changed, is_import_new_file) = self.collect_ets(patch_config)?;
    self.collect_resources(patch_config)?;
    self.write_quickfix_log(
      patch_config,
      &native_changes,
      ets_changed,
      is_import_new_file,
    )?;
    self.write_last_native_cache()?;
    self.write_patch_json()
  }

  pub(super) fn package(&self, runner: &AppPackageRunner) -> QuickFixResult<PathBuf> {
    let output = self.unsigned_hqf();
    runner.pack_hqf(&HqfPackagePlan {
      patch_json_path: self.input.layout.patch_json(),
      native_libs_path: self.input.layout.patch_libs_dir(),
      ets_path: self.input.layout.patch_ets_dir(),
      resources_path: self.input.layout.patch_resources_dir(),
      output_path: output.clone(),
    })?;
    Ok(output)
  }

  pub(super) fn update_signed_log(&self) -> QuickFixResult<()> {
    let path = self.input.layout.quickfix_json();
    if !path.is_file() {
      return Ok(());
    }
    let mut value: serde_json::Value = self.read_json(&path)?;
    value["file"] = serde_json::Value::String(self.hqf_name(true));
    self.write_json(&path, &value)
  }

  fn collect_native_libs(&self) -> QuickFixResult<BTreeMap<String, QuickFixState>> {
    let destination = self.input.layout.patch_libs_dir();
    self.empty_directory(&destination)?;
    let base = self.read_native_cache(&self.input.layout.base_native_libs_json())?;
    let last = self.read_native_cache(&self.input.layout.patch_last_libs_json())?;
    let current = self.hash_directory(&self.input.layout.stripped_native_libs_dir())?;
    let changes = current
      .iter()
      .map(|(path, hash)| {
        let state = if last.stripped.get(path).is_some_and(|value| value != hash) {
          QuickFixState::Last
        } else if base.stripped.get(path) == Some(hash) {
          QuickFixState::Unchanged
        } else if last.stripped.contains_key(path) {
          QuickFixState::Base
        } else {
          QuickFixState::Last
        };
        (path.clone(), state)
      })
      .collect::<BTreeMap<_, _>>();
    let source_root = self.input.layout.stripped_native_libs_dir();
    for (path, state) in &changes {
      if *state == QuickFixState::Unchanged {
        continue;
      }
      let source = PathBuf::from(path);
      let relative = source.strip_prefix(&source_root).map_err(|_| {
        QuickFixError::InvalidResourcePath(format!(
          "native library '{}' is outside '{}'",
          source.display(),
          source_root.display()
        ))
      })?;
      let output = destination.join(relative);
      self.copy_file(&source, &output)?;
    }
    Ok(changes)
  }

  fn collect_resources(&self, patch_config: Option<&serde_json::Value>) -> QuickFixResult<()> {
    let output = self.input.layout.patch_resources_dir();
    self.empty_directory(&output)?;
    self.ensure_directory(&output.join("resfile"))?;
    self.ensure_directory(&output.join("rawfile"))?;
    let Some(changed_file_list) = self.patch_changed_file_list(patch_config) else {
      return Ok(());
    };
    if !changed_file_list.is_file() {
      return Ok(());
    }
    let changed: ChangedFileList = self.read_json(&changed_file_list)?;
    for resource in changed
      .resources
      .res_file
      .iter()
      .chain(&changed.resources.raw_file)
    {
      let source = PathBuf::from(&resource.file_path);
      if source.file_name().and_then(|name| name.to_str()) == Some(".DS_Store") || !source.exists()
      {
        continue;
      }
      let resource_root = PathBuf::from(&resource.resource_path);
      let relative = source.strip_prefix(&resource_root).map_err(|_| {
        QuickFixError::InvalidResourcePath(format!(
          "resource '{}' is outside resourcePath '{}'",
          source.display(),
          resource_root.display()
        ))
      })?;
      self.copy_file(&source, &output.join(relative))?;
    }
    Ok(())
  }

  fn collect_ets(&self, patch_config: Option<&serde_json::Value>) -> QuickFixResult<(bool, bool)> {
    self.ensure_directory(&self.input.layout.patch_ets_dir())?;
    let state_path = self.input.layout.patch_ets_state_json();
    let mut state = if state_path.is_file() {
      self.read_json::<EtsPatchState>(&state_path)?
    } else {
      EtsPatchState::default()
    };
    let is_import_new_file = state.is_import_new_file;
    let Some(changed_file_list) = self.patch_changed_file_list(patch_config) else {
      return Ok((false, is_import_new_file));
    };
    if !changed_file_list.is_file() {
      return Ok((false, is_import_new_file));
    }
    let metadata = self.metadata(&changed_file_list)?;
    let modified = metadata.modified().map_err(|source| QuickFixError::Io {
      path: changed_file_list.clone(),
      source,
    })?;
    let milliseconds = modified
      .duration_since(UNIX_EPOCH)
      .map_err(|source| QuickFixError::Io {
        path: changed_file_list.clone(),
        source: std::io::Error::other(source),
      })?
      .as_secs_f64()
      * 1_000.0;
    let fingerprint = milliseconds.to_string();
    if state.changed_file_list.as_deref() == Some(fingerprint.as_str()) {
      return Ok((false, is_import_new_file));
    }
    state.changed_file_list = Some(fingerprint);
    self.write_json(&state_path, &state)?;
    Ok((true, is_import_new_file))
  }

  fn write_quickfix_log(
    &self,
    patch_config: Option<&serde_json::Value>,
    native_changes: &BTreeMap<String, QuickFixState>,
    ets_changed: bool,
    is_import_new_file: bool,
  ) -> QuickFixResult<()> {
    let mut state = native_changes
      .values()
      .copied()
      .max()
      .unwrap_or(QuickFixState::Unchanged);
    if ets_changed {
      state = QuickFixState::Last;
    }
    if state == QuickFixState::Unchanged {
      if let Some(path) = self.patch_changed_file_list(patch_config) {
        if path.is_file() {
          let changed: serde_json::Value = self.read_json(&path)?;
          if JsonChangeDetector::new(&changed).is_changed() {
            state = QuickFixState::Base;
          }
        }
      }
    }
    let changes = native_changes
      .iter()
      .map(|(path, state)| (path.clone(), serde_json::json!(*state as u8)))
      .collect::<serde_json::Map<_, _>>();
    let log = serde_json::json!({
      "changes": changes,
      "state": state as u8,
      "isImportNewFile": is_import_new_file,
      "file": self.hqf_name(false),
    });
    self.write_json(&self.input.layout.quickfix_json(), &log)
  }

  fn write_last_native_cache(&self) -> QuickFixResult<()> {
    let cache = NativeLibCacheSnapshot {
      libs: self.hash_directory(&self.input.layout.process_libs_dir())?,
      stripped: self.hash_directory(&self.input.layout.stripped_native_libs_dir())?,
      debug_symbol: self
        .input
        .debug_symbol
        .and_then(|option| serde_json::to_value(option).ok())
        .unwrap_or(serde_json::Value::Null),
    };
    self.write_json(&self.input.layout.patch_last_libs_json(), &cache)
  }

  fn write_patch_json(&self) -> QuickFixResult<()> {
    let patch = serde_json::json!({
      "app": {
        "bundleName": self.input.bundle_name,
        "versionCode": self.input.version_code,
        "versionName": self.input.version_name,
        "patchVersionCode": self.input.patch_version,
        "patchVersionName": self.input.patch_version.to_string(),
      },
      "module": {
        "name": self.input.module_name,
        "type": "patch",
        "deviceTypes": self.input.device_types,
        "originalModuleHash": "",
      },
    });
    self.write_json(&self.input.layout.patch_json(), &patch)
  }

  fn patch_changed_file_list(&self, patch_config: Option<&serde_json::Value>) -> Option<PathBuf> {
    patch_config
      .and_then(|config| config.get("changedFileList"))
      .and_then(serde_json::Value::as_str)
      .filter(|path| !path.is_empty())
      .map(PathBuf::from)
      .or_else(|| Some(self.input.layout.patch_changed_file_list()))
  }

  fn hqf_name(&self, signed: bool) -> String {
    format!(
      "{}-{}-{}.hqf",
      self.input.module_name,
      self.input.target_name,
      if signed { "signed" } else { "unsigned" }
    )
  }

  fn read_native_cache(&self, path: &Path) -> QuickFixResult<NativeLibCacheSnapshot> {
    if !path.is_file() {
      return Ok(NativeLibCacheSnapshot::default());
    }
    self.read_json(path)
  }

  fn hash_directory(&self, root: &Path) -> QuickFixResult<BTreeMap<String, String>> {
    if !root.is_dir() {
      return Ok(BTreeMap::new());
    }
    let mut files = walkdir::WalkDir::new(root)
      .into_iter()
      .collect::<Result<Vec<_>, _>>()
      .map_err(|source| QuickFixError::Io {
        path: source.path().unwrap_or(root).to_path_buf(),
        source: std::io::Error::other(source),
      })?
      .into_iter()
      .filter(|entry| entry.file_type().is_file())
      .map(|entry| entry.into_path())
      .collect::<Vec<_>>();
    files.sort();
    files
      .into_iter()
      .map(|path| {
        let metadata = self.metadata(&path)?;
        let modified_ms = metadata
          .modified()
          .and_then(|modified| {
            modified
              .duration_since(UNIX_EPOCH)
              .map_err(std::io::Error::other)
          })
          .map_err(|source| QuickFixError::Io {
            path: path.clone(),
            source,
          })?
          .as_millis();
        let fingerprint = (u128::from(metadata.len()) + modified_ms).to_string();
        let hash = format!("{:x}", Sha256::digest(fingerprint.as_bytes()));
        Ok((path.to_string_lossy().into_owned(), hash))
      })
      .collect()
  }

  fn read_json<T: serde::de::DeserializeOwned>(&self, path: &Path) -> QuickFixResult<T> {
    let bytes = std::fs::read(path).map_err(|source| QuickFixError::Io {
      path: path.to_path_buf(),
      source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| QuickFixError::Json {
      path: path.to_path_buf(),
      source,
    })
  }

  fn write_json<T: Serialize + ?Sized>(&self, path: &Path, value: &T) -> QuickFixResult<()> {
    if let Some(parent) = path.parent() {
      self.ensure_directory(parent)?;
    }
    let bytes = serde_json::to_vec(value).map_err(|source| QuickFixError::Json {
      path: path.to_path_buf(),
      source,
    })?;
    std::fs::write(path, bytes).map_err(|source| QuickFixError::Io {
      path: path.to_path_buf(),
      source,
    })
  }

  fn metadata(&self, path: &Path) -> QuickFixResult<std::fs::Metadata> {
    std::fs::metadata(path).map_err(|source| QuickFixError::Io {
      path: path.to_path_buf(),
      source,
    })
  }

  fn ensure_directory(&self, path: &Path) -> QuickFixResult<()> {
    std::fs::create_dir_all(path).map_err(|source| QuickFixError::Io {
      path: path.to_path_buf(),
      source,
    })
  }

  fn empty_directory(&self, path: &Path) -> QuickFixResult<()> {
    if path.exists() {
      std::fs::remove_dir_all(path).map_err(|source| QuickFixError::Io {
        path: path.to_path_buf(),
        source,
      })?;
    }
    self.ensure_directory(path)
  }

  fn copy_file(&self, source: &Path, output: &Path) -> QuickFixResult<()> {
    if let Some(parent) = output.parent() {
      self.ensure_directory(parent)?;
    }
    std::fs::copy(source, output)
      .map(|_| ())
      .map_err(|source_error| QuickFixError::Io {
        path: source.to_path_buf(),
        source: source_error,
      })
  }
}

pub(super) struct QuickFixVersionStore {
  path: PathBuf,
}

impl QuickFixVersionStore {
  pub(super) fn new(hvigor_cache_dir: &Path) -> Self {
    Self {
      path: hvigor_cache_dir
        .join("outputs")
        .join("patch")
        .join("patch.json"),
    }
  }

  pub(super) fn from_runtime(hvigor_cache_dir: Option<&Path>) -> QuickFixResult<Self> {
    hvigor_cache_dir
      .map(Self::new)
      .ok_or(QuickFixError::MissingHvigorCacheDir)
  }

  pub(super) fn current(&self) -> QuickFixResult<u64> {
    if !self.path.is_file() {
      return Ok(INITIAL_PATCH_VERSION);
    }
    let bytes = std::fs::read(&self.path).map_err(|source| QuickFixError::Io {
      path: self.path.clone(),
      source,
    })?;
    let value: serde_json::Value =
      serde_json::from_slice(&bytes).map_err(|source| QuickFixError::Json {
        path: self.path.clone(),
        source,
      })?;
    Ok(
      value
        .get("patchVersionCode")
        .and_then(|value| {
          value
            .as_u64()
            .or_else(|| value.as_str().and_then(|value| value.parse::<u64>().ok()))
        })
        .unwrap_or(INITIAL_PATCH_VERSION),
    )
  }

  pub(super) fn advance_after_assemble(&self, current: u64) -> QuickFixResult<()> {
    if let Some(parent) = self.path.parent() {
      std::fs::create_dir_all(parent).map_err(|source| QuickFixError::Io {
        path: parent.to_path_buf(),
        source,
      })?;
    }
    let bytes = serde_json::to_vec(&serde_json::json!({
      "patchVersionCode": current.saturating_add(1),
    }))
    .map_err(|source| QuickFixError::Json {
      path: self.path.clone(),
      source,
    })?;
    std::fs::write(&self.path, bytes).map_err(|source| QuickFixError::Io {
      path: self.path.clone(),
      source,
    })
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[repr(u8)]
enum QuickFixState {
  Unchanged = 0,
  Base = 1,
  Last = 2,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeLibCacheSnapshot {
  #[serde(default)]
  libs: BTreeMap<String, String>,
  #[serde(default)]
  stripped: BTreeMap<String, String>,
  #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
  debug_symbol: serde_json::Value,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChangedFileList {
  #[serde(default)]
  modified_files: Vec<String>,
  #[serde(default)]
  resources: ChangedResources,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChangedResources {
  #[serde(default)]
  res_file: Vec<ChangedResource>,
  #[serde(default)]
  raw_file: Vec<ChangedResource>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChangedResource {
  file_path: String,
  resource_path: String,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct EtsPatchState {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  changed_file_list: Option<String>,
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  is_import_new_file: bool,
}

struct JsonChangeDetector<'a> {
  value: &'a serde_json::Value,
}

impl<'a> JsonChangeDetector<'a> {
  fn new(value: &'a serde_json::Value) -> Self {
    Self { value }
  }

  fn is_changed(&self) -> bool {
    match self.value {
      serde_json::Value::Array(values) => !values.is_empty(),
      serde_json::Value::Object(values) => {
        values.values().any(|value| Self::new(value).is_changed())
      }
      _ => false,
    }
  }
}
