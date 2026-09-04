//! Native library collection, cache, strip, and configuration orchestration.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::UNIX_EPOCH;

use arkdown_core::{
  ModuleConfig, NativeLibFilterOption, NativeLibOption, NativeLibPackageSelection,
};
use arkdown_path::{canonicalize_or_owned, ArtifactPathSerializer, HvigorNativeLibraryPattern};
use arkdown_session::dependency_graph::snapshot::DependencyGraphSnapshot;
use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The set of ABI directories that hvigor's native pipeline produces under
/// the CMake / NDK build intermediates.
const KNOWN_ABI_DIRS: &[&str] = &["arm64-v8a", "armeabi-v7a", "x86_64", "x86", "armeabi"];
/// hvigor `BuildDirConst.LIBS`, used by `dependency.getDefaultLibsDir()`.
const HVIGOR_LIBS_DIR: &str = "libs";

use crate::services::native_build::impl_types::*;
use crate::services::native_build::zig::{NativeCmakeCompilerState, NativeZigToolchainWriter};

pub struct NativeLibCollector<'a> {
  pub(crate) module: &'a ModuleConfig,
  pub(crate) abi_filters: &'a [String],
  pub(crate) deps: &'a DependencyGraphSnapshot,
  pub(crate) product_name: String,
  pub(crate) target_name: String,
  pub(crate) policy: NativeLibCollectionPolicy,
  pub(crate) excluded_dependency_roots: FxHashSet<PathBuf>,
}

impl<'a> NativeLibCollector<'a> {
  pub fn new(
    module: &'a ModuleConfig,
    abi_filters: &'a [String],
    deps: &'a DependencyGraphSnapshot,
  ) -> Self {
    Self {
      module,
      abi_filters,
      deps,
      product_name: "default".to_string(),
      target_name: "default".to_string(),
      policy: NativeLibCollectionPolicy::default(),
      excluded_dependency_roots: FxHashSet::default(),
    }
  }

  pub fn with_output_variant(
    mut self,
    product_name: impl Into<String>,
    target_name: impl Into<String>,
  ) -> Self {
    self.product_name = product_name.into();
    self.target_name = target_name.into();
    self
  }

  pub fn with_policy(mut self, policy: NativeLibCollectionPolicy) -> Self {
    self.policy = policy;
    self
  }

  pub fn excluding_dependency_roots(mut self, roots: &[PathBuf]) -> Self {
    self.excluded_dependency_roots = roots
      .iter()
      .map(|root| arkdown_path::canonicalize_or_owned(root))
      .collect();
    self
  }

  pub(crate) fn configure_cmake_dependencies(&self) -> NativeBuildResult<()> {
    let native_config = self.native_build_config();
    if !native_config.should_run_native_tasks() {
      return Ok(());
    }
    NativeCmakeDependencyConfigurator::new(
      self.module,
      self.abi_filters,
      self.deps,
      &self.product_name,
      &self.target_name,
      &self.policy.build_mode,
      &self.policy.cmake_local_dependencies,
    )
    .write()
  }

  pub(crate) fn configure_native(&self) -> NativeBuildResult<()> {
    self.native_build_config().configure()
  }

  pub(crate) fn build_native(&self) -> NativeBuildResult<()> {
    self.native_build_config().build()
  }

  pub(crate) fn native_build_commands(&self) -> NativeBuildResult<Vec<NativeCommandPlan>> {
    self.native_build_config().ninja_command_plans()
  }

  pub(crate) fn report_native_build_success(&self) {
    tracing::info!(
        module = %self.module.src_path.display(),
        "native: CMake/Ninja build succeeded"
    );
  }

  pub(crate) fn process_libs(&self) -> NativeBuildResult<Vec<PathBuf>> {
    let mut candidates = Vec::new();
    let mut seen = FxHashSet::default();

    if self.layout().has_native_code() {
      tracing::debug!(module = %self.module.src_path.display(), "native: scanning for pre-built .so files");
    }
    let native_out = self.native_build_config().scan()?;
    let self_package = self.self_package_info();

    if !(self.module.module_type == arkdown_core::OhosModuleType::Har
      && self.policy.native_lib.exclude_from_har)
    {
      for dep_libs_dir in self.dependency_default_lib_dirs() {
        self.collect_from_libs_dir(&dep_libs_dir, &mut seen, &mut candidates)?;
      }
    }
    // DevEco Studio 6.24.3 `worker/process-libs.js::processLibs` evaluates
    // dependency libraries before the module's local output. Its
    // `pickFirsts`, `pickLasts`, and `enableOverride` behavior is therefore
    // defined by that traversal order. `DependencyGraphSnapshot::edges`
    // retains the session-owned hvigor dependency order; do not sort this
    // inventory by host path before resolving the filter.
    for (_, so_path) in native_out.libs_by_abi {
      let canonical = canonicalize_or_owned(&so_path);
      if seen.insert(canonical) {
        candidates.push(NativeLibCandidate::from_collected_path(
          so_path,
          self_package.clone(),
        )?);
      }
    }
    NativeLibFilterApplier::new(&self.policy.native_lib.filter).apply(&mut candidates);

    let result_libs =
      NativeLibSelectionResolver::new(&self.module.name, &self.policy.native_lib.filter)
        .resolve(candidates)?;
    NativeProcessLibWriter::for_variant(
      &self.module.src_path,
      &self.product_name,
      &self.target_name,
    )
    .materialize(&result_libs)
  }

  /// `DoNativeBinxo`: run the SDK binxo tool over process-libs outputs when
  /// both `-DOHOS_ENABLE_HWASAN=ON` and `-DOHOS_ENABLE_BINXO=ON` are set
  /// (`BinxoValidationService.isBinxoValid`). Files matching
  /// `nativeLib.excludeSoFromBinXO` are copied instead of binxo'd; unchanged
  /// inputs reuse the previous output (cache keyed on the binxo cache file).
  pub(crate) fn binxo_libs(&self, libs: &[PathBuf]) -> NativeBuildResult<Vec<PathBuf>> {
    if !self.is_binxo_valid() {
      return Err(NativeBuildError::BinxoRequiresHwasan {
        profile: self.module.src_path.join("build-profile.json5"),
      });
    }
    let exclude: Vec<&str> = self
      .policy
      .native_lib
      .exclude_so_from_binxo
      .iter()
      .map(String::as_str)
      .collect();
    let output_root = self.layout().binxo_libs_dir();
    std::fs::create_dir_all(&output_root).map_err(|error| {
      NativeBuildError::NativePostProcessFailed {
        path: output_root.clone(),
        message: error.to_string(),
      }
    })?;

    let binxo_tool = self.policy.sdk_toolchain.binxo.as_deref();
    let available_binxo_tool = binxo_tool.filter(|path| path.is_file());
    if available_binxo_tool.is_none() {
      tracing::warn!(
        tool = binxo_tool.map(|path| path.display().to_string()),
        "HMS native BinXO executable is unavailable; falling back to plain copies"
      );
    }
    let cache = self.read_binxo_cache()?;
    let cached_symbol_matches = cache.binxo_symbol.as_ref().is_some_and(|symbol| {
      symbol.enable_asan_binxo
        && symbol.exclude_so_from_binxo == self.policy.native_lib.exclude_so_from_binxo
    });
    let mut outputs = Vec::with_capacity(libs.len());
    let mut expected = FxHashSet::default();
    for source in libs {
      let relative = Self::native_lib_identity(source);
      if relative.is_empty() {
        return Err(NativeBuildError::NativeLibRelativePath {
          path: source.clone(),
          message: "missing ABI-relative path".to_string(),
        });
      }
      let destination = output_root.join(&relative);
      expected.insert(destination.clone());
      if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
          NativeBuildError::NativePostProcessFailed {
            path: parent.to_path_buf(),
            message: error.to_string(),
          }
        })?;
      }
      let excluded = exclude.iter().any(|pattern| {
        HvigorNativeLibraryPattern::matches_filter_pattern(
          Path::new(relative.as_str()),
          pattern,
          false,
        )
      });
      if excluded || available_binxo_tool.is_none() {
        // `copySoFile`: excluded (or unsupported) libraries are linked.
        std::fs::copy(&source, &destination).map_err(|error| {
          NativeBuildError::NativePostProcessFailed {
            path: destination.clone(),
            message: error.to_string(),
          }
        })?;
      } else {
        let source_key = source.to_string_lossy();
        let destination_key = destination.to_string_lossy();
        let cache_hit = cached_symbol_matches
          && cache.libs.get(source_key.as_ref()) == Some(&Self::native_file_hash(source)?)
          && cache.binxo.get(destination_key.as_ref())
            == Some(&Self::native_file_hash(&destination).unwrap_or_default());
        if cache_hit {
          outputs.push(destination);
          continue;
        }
        let binxo_tool = available_binxo_tool.expect("BinXO availability checked above");
        let output = NativeBinxoCommand::new(binxo_tool, source, &destination)
          .execute()
          .map_err(|error| NativeBuildError::NativePostProcessFailed {
            path: binxo_tool.to_path_buf(),
            message: error.to_string(),
          })?;
        if !output.status.success() {
          tracing::warn!(
            "binxo failed for {} ({}); falling back to a plain copy",
            relative,
            String::from_utf8_lossy(&output.stderr).trim()
          );
          std::fs::copy(&source, &destination).map_err(|error| {
            NativeBuildError::NativePostProcessFailed {
              path: destination.clone(),
              message: error.to_string(),
            }
          })?;
        }
      }
      outputs.push(destination);
    }
    self.remove_stale_native_outputs(&output_root, &expected)?;
    outputs.sort();
    Ok(outputs)
  }

  pub(crate) fn read_binxo_cache(&self) -> NativeBuildResult<NativeBinxoCacheSnapshot> {
    let layout = self.layout();
    let cache_file = if layout.patch_last_libs_file().is_file() {
      layout.patch_last_libs_file()
    } else {
      layout.binxo_native_lib_cache_file()
    };
    if !cache_file.is_file() {
      return Ok(NativeBinxoCacheSnapshot::default());
    }
    let bytes =
      std::fs::read(&cache_file).map_err(|error| NativeBuildError::NativePostProcessFailed {
        path: cache_file.clone(),
        message: error.to_string(),
      })?;
    serde_json::from_slice(&bytes).map_err(|error| NativeBuildError::NativePostProcessFailed {
      path: cache_file,
      message: error.to_string(),
    })
  }

  pub(crate) fn native_file_hash(path: &Path) -> NativeBuildResult<String> {
    NativeLibCacheWriter::file_hash(path)
  }

  pub(crate) fn remove_stale_native_outputs(
    &self,
    output_root: &Path,
    expected: &FxHashSet<PathBuf>,
  ) -> NativeBuildResult<()> {
    if !output_root.is_dir() {
      return Ok(());
    }
    let mut stale_files = walkdir::WalkDir::new(output_root)
      .contents_first(true)
      .into_iter()
      .filter_map(Result::ok)
      .map(|entry| entry.into_path())
      .filter(|path| path.is_file() && !expected.contains(path))
      .collect::<Vec<_>>();
    stale_files.sort();
    for stale in stale_files {
      std::fs::remove_file(&stale).map_err(|error| NativeBuildError::NativePostProcessFailed {
        path: stale,
        message: error.to_string(),
      })?;
    }
    Ok(())
  }

  pub(crate) fn is_binxo_enabled(&self) -> bool {
    self
      .policy
      .binxo_config
      .unwrap_or_else(|| self.has_cmake_argument("-DOHOS_ENABLE_BINXO=ON"))
  }

  pub(crate) fn is_binxo_valid(&self) -> bool {
    let hwasan = self
      .policy
      .hwasan_config
      .unwrap_or_else(|| self.has_cmake_argument("-DOHOS_ENABLE_HWASAN=ON"));
    let binxo = self.is_binxo_enabled();
    hwasan && binxo
  }

  pub(crate) fn has_cmake_argument(&self, expected: &str) -> bool {
    self
      .policy
      .external_native_options
      .as_ref()
      .into_iter()
      .flat_map(|options| options.arguments.iter())
      .flat_map(|argument| argument.trim().split(' '))
      .filter(|argument| argument.starts_with('-'))
      .any(|argument| argument == expected)
  }

  pub(crate) fn strip_libs(&self, libs: &[PathBuf]) -> NativeBuildResult<Vec<PathBuf>> {
    NativeStripProcessor::for_variant(
      &self.module.src_path,
      &self.product_name,
      &self.target_name,
      self.policy.sdk_toolchain.llvm_strip_path(),
      &self.policy.native_lib,
    )
    .process(libs)
  }

  pub(crate) fn cache_libs(&self) -> NativeBuildResult<()> {
    NativeLibCacheWriter::for_variant(
      &self.module.src_path,
      &self.product_name,
      &self.target_name,
      &self.policy.native_lib,
      self.is_binxo_valid(),
    )
    .write()
  }

  pub(crate) fn validate_libs(&self, libs: &[PathBuf]) -> NativeBuildResult<()> {
    self.policy.validate_packaged_libs(libs)
  }

  pub(crate) fn native_build_config(&self) -> NativeBuildConfig {
    NativeBuildConfig {
      module_root: self.module.src_path.clone(),
      product_name: self.product_name.clone(),
      target_name: self.target_name.clone(),
      abi_filters: self.abi_filters.to_vec(),
      enable_cmake_build: self.policy.enable_cmake_build(),
      external_native_options: self.policy.external_native_options.clone(),
      sanitizers: self.policy.sanitizers,
      build_mode: self.policy.build_mode.clone(),
      sdk_toolchain: self.policy.sdk_toolchain.clone(),
      collect_all_libs: self.policy.native_lib.collect_all_libs,
      release_cmake_build_type: self.policy.release_cmake_build_type,
    }
  }

  pub(crate) fn layout(&self) -> NativeModuleLayout<'_> {
    NativeModuleLayout::for_variant(&self.module.src_path, &self.product_name, &self.target_name)
  }

  pub(crate) fn self_package_info(&self) -> NativeLibPackageInfo {
    self
      .policy
      .self_package
      .clone()
      .unwrap_or_else(|| NativeLibPackageInfo::local(self.module.name.clone(), String::new()))
  }

  pub(crate) fn native_lib_identity(lib: &Path) -> String {
    let components = lib
      .components()
      .filter_map(|component| match component {
        std::path::Component::Normal(segment) => segment.to_str(),
        _ => None,
      })
      .collect::<Vec<_>>();
    if let Some(index) = components
      .iter()
      .position(|component| KNOWN_ABI_DIRS.contains(component))
    {
      return components[index..].join("/");
    }
    lib
      .file_name()
      .and_then(|name| name.to_str())
      .unwrap_or("")
      .to_string()
  }

  pub(crate) fn is_duplicate_check_exempt(identity: &str) -> bool {
    matches!(
      Path::new(identity)
        .file_name()
        .and_then(|name| name.to_str()),
      Some("libc++.so") | Some("libc++_shared.so")
    )
  }

  pub fn collect_context_names(&self) -> Vec<String> {
    let mut names = Vec::new();
    let mut seen = FxHashSet::default();
    self.collect_lib_names_from_libs_dir(
      &self.module.src_path.join(HVIGOR_LIBS_DIR),
      &mut seen,
      &mut names,
    );
    for dep_libs_dir in self.dependency_default_lib_dirs() {
      self.collect_lib_names_from_libs_dir(&dep_libs_dir.libs_dir, &mut seen, &mut names);
    }
    names.sort();
    names
  }

  pub(crate) fn dependency_default_lib_dirs(&self) -> Vec<NativeDependencyLibDir> {
    let mut dirs = Vec::new();
    let mut seen = FxHashSet::default();
    let mut push_libs_dir = |libs_dir: PathBuf, package: NativeLibPackageInfo| {
      if seen.insert(libs_dir.clone()) {
        dirs.push(NativeDependencyLibDir { libs_dir, package });
      }
    };
    for edge in &self.deps.edges {
      let node = self.deps.nodes.get(&edge.package_name).or_else(|| {
        self.deps.nodes.values().find(|node| {
          node.package_root.as_deref().is_some_and(|root| {
            canonicalize_or_owned(root) == canonicalize_or_owned(&edge.package_root)
          })
        })
      });
      let Some(node) = node else {
        continue;
      };
      let root = node.package_root.as_deref().unwrap_or(&edge.package_root);
      if self
        .excluded_dependency_roots
        .contains(&arkdown_path::canonicalize_or_owned(root))
      {
        continue;
      }
      let package = NativeLibPackageInfo::dependency(node);
      let canonical_root = canonicalize_or_owned(root);
      if let Some(local) = self
        .policy
        .cmake_local_dependencies
        .iter()
        .find(|dependency| dependency.package_root() == canonical_root)
      {
        // Hvigor `ProcessLibs.collectLocalHarLibs()` registers the local
        // module's ProcessLibs output before its source-tree `libs/` fallback.
        push_libs_dir(local.process_libs_dir(), package.clone());
      }
      push_libs_dir(canonical_root.join(HVIGOR_LIBS_DIR), package);
    }
    // Older snapshots and focused callers may not have dependency edges. Keep
    // their deterministic node fallback, while production sessions take the
    // edge-order path above.
    for node in self.deps.nodes.values() {
      let Some(root) = node.package_root.as_deref() else {
        continue;
      };
      if self
        .excluded_dependency_roots
        .contains(&arkdown_path::canonicalize_or_owned(root))
      {
        continue;
      }
      let package = NativeLibPackageInfo::dependency(node);
      let canonical_root = canonicalize_or_owned(root);
      if let Some(local) = self
        .policy
        .cmake_local_dependencies
        .iter()
        .find(|dependency| dependency.package_root() == canonical_root)
      {
        push_libs_dir(local.process_libs_dir(), package.clone());
      }
      push_libs_dir(canonical_root.join(HVIGOR_LIBS_DIR), package);
    }
    dirs
  }

  pub(crate) fn collect_lib_names_from_libs_dir(
    &self,
    libs_dir: &Path,
    seen: &mut FxHashSet<String>,
    names: &mut Vec<String>,
  ) {
    if !libs_dir.exists() {
      return;
    }
    for entry in walkdir::WalkDir::new(libs_dir)
      .into_iter()
      .filter_map(|e| e.ok())
      .filter(|e| e.file_type().is_file())
    {
      if !HvigorNativeLibraryPattern::matches_path_under_with_collect_all(
        libs_dir,
        entry.path(),
        self.policy.native_lib.collect_all_libs,
      ) {
        continue;
      }
      let Some(name) = entry.path().file_name().and_then(|name| name.to_str()) else {
        continue;
      };
      if seen.insert(name.to_string()) {
        names.push(name.to_string());
      }
    }
  }

  pub(crate) fn collect_from_libs_dir(
    &self,
    input: &NativeDependencyLibDir,
    seen: &mut FxHashSet<PathBuf>,
    libs: &mut Vec<NativeLibCandidate>,
  ) -> NativeBuildResult<()> {
    let libs_dir = &input.libs_dir;
    if !libs_dir.exists() {
      return Ok(());
    }
    for entry in walkdir::WalkDir::new(libs_dir)
      .into_iter()
      .filter_map(|e| e.ok())
      .filter(|e| e.file_type().is_file())
    {
      let path = entry.path();
      if !HvigorNativeLibraryPattern::matches_path_under_with_collect_all(
        libs_dir,
        path,
        self.policy.native_lib.collect_all_libs,
      ) {
        continue;
      }
      if NativeAbiFilters::new(self.abi_filters).matches_path(path) {
        let relative_path = NativeLibCandidate::relative_path(libs_dir, path)?;
        let path = arkdown_path::canonicalize_or_owned(path);
        if seen.insert(path.clone()) {
          libs.push(NativeLibCandidate::new(
            path,
            relative_path,
            input.package.clone(),
          ));
        }
      }
    }
    Ok(())
  }
}

/// Owns hvigor's `ProcessLibs` output materialization. The selected source
/// files are hard-linked into `intermediates/libs/default`; copying is used
/// only when the source and build directories are on different filesystems.
pub(crate) struct NativeProcessLibWriter<'a> {
  pub(crate) layout: NativeModuleLayout<'a>,
}

impl<'a> NativeProcessLibWriter<'a> {
  pub(crate) fn for_variant(
    module_root: &'a Path,
    product_name: &'a str,
    target_name: &'a str,
  ) -> Self {
    Self {
      layout: NativeModuleLayout::for_variant(module_root, product_name, target_name),
    }
  }

  pub(crate) fn materialize(&self, libs: &[PathBuf]) -> NativeBuildResult<Vec<PathBuf>> {
    let output_root = self.layout.process_libs_dir();
    if output_root.exists() {
      std::fs::remove_dir_all(&output_root).map_err(|error| {
        NativeBuildError::NativePostProcessFailed {
          path: output_root.clone(),
          message: error.to_string(),
        }
      })?;
    }
    std::fs::create_dir_all(&output_root).map_err(|error| {
      NativeBuildError::NativePostProcessFailed {
        path: output_root.clone(),
        message: error.to_string(),
      }
    })?;

    let mut outputs = Vec::with_capacity(libs.len());
    for source in libs {
      let relative = NativeLibCollector::native_lib_identity(source);
      if relative.is_empty() {
        return Err(NativeBuildError::NativeLibRelativePath {
          path: source.clone(),
          message: "missing ABI-relative path".to_string(),
        });
      }
      let destination = output_root.join(relative);
      if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
          NativeBuildError::NativePostProcessFailed {
            path: parent.to_path_buf(),
            message: error.to_string(),
          }
        })?;
      }
      if let Err(link_error) = std::fs::hard_link(source, &destination) {
        std::fs::copy(source, &destination).map_err(|copy_error| {
          NativeBuildError::NativePostProcessFailed {
            path: source.clone(),
            message: format!(
              "hard link failed ({link_error}); copy fallback failed ({copy_error})"
            ),
          }
        })?;
      }
      outputs.push(destination);
    }
    outputs.sort();
    Ok(outputs)
  }
}

#[derive(Serialize)]
pub(crate) struct NativeLibCache<'a> {
  pub(crate) libs: &'a BTreeMap<String, String>,
  pub(crate) stripped: &'a BTreeMap<String, String>,
  #[serde(rename = "debugSymbol", skip_serializing_if = "Option::is_none")]
  pub(crate) debug_symbol: Option<&'a arkdown_core::NativeDebugSymbolOption>,
}

#[derive(Serialize)]
pub(crate) struct NativeBinxoCache<'a> {
  pub(crate) libs: &'a BTreeMap<String, String>,
  pub(crate) binxo: &'a BTreeMap<String, String>,
  #[serde(rename = "binxoSymbol")]
  pub(crate) binxo_symbol: NativeBinxoSymbol<'a>,
}

#[derive(Serialize)]
pub(crate) struct NativeBinxoSymbol<'a> {
  #[serde(rename = "enableAsanBinxo")]
  pub(crate) enable_asan_binxo: bool,
  #[serde(rename = "excludeSoFromBinxo")]
  pub(crate) exclude_so_from_binxo: &'a [String],
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct NativeBinxoCacheSnapshot {
  #[serde(default)]
  pub(crate) libs: BTreeMap<String, String>,
  #[serde(default)]
  pub(crate) binxo: BTreeMap<String, String>,
  #[serde(rename = "binxoSymbol", default)]
  pub(crate) binxo_symbol: Option<NativeBinxoSymbolSnapshot>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeBinxoSymbolSnapshot {
  #[serde(rename = "enableAsanBinxo", default)]
  pub(crate) enable_asan_binxo: bool,
  #[serde(rename = "excludeSoFromBinxo", default)]
  pub(crate) exclude_so_from_binxo: Vec<String>,
}

/// Mirrors hvigor `CacheNativeLibs` and `LibsFileCacheUtil`.
///
/// This cache deliberately does not use `FileUtil.pipelineAsync` or
/// `ohos.processLib.optimization`; those belong to ProcessLibs task-input
/// fingerprinting. Native strip/binxo caches always hash the JavaScript
/// numeric sum `size + mtime.getTime()`.
pub(crate) struct NativeLibCacheWriter<'a> {
  pub(crate) layout: NativeModuleLayout<'a>,
  pub(crate) option: &'a NativeLibOption,
  pub(crate) binxo_valid: bool,
}

impl<'a> NativeLibCacheWriter<'a> {
  pub(crate) fn for_variant(
    module_root: &'a Path,
    product_name: &'a str,
    target_name: &'a str,
    option: &'a NativeLibOption,
    binxo_valid: bool,
  ) -> Self {
    Self {
      layout: NativeModuleLayout::for_variant(module_root, product_name, target_name),
      option,
      binxo_valid,
    }
  }

  pub(crate) fn write(&self) -> NativeBuildResult<()> {
    // CacheNativeLibs reads each directory once and reuses the process-libs
    // inventory for both cache artifacts. Keep that ownership shape here.
    let process_libs = self.hash_directory(&self.layout.process_libs_dir())?;
    let stripped = self.hash_directory(&self.layout.stripped_libs_dir())?;
    let binxo = self.hash_directory(&self.layout.binxo_libs_dir())?;
    let strip_inputs = if self.binxo_valid {
      &binxo
    } else {
      &process_libs
    };
    let cache = NativeLibCache {
      libs: strip_inputs,
      stripped: &stripped,
      debug_symbol: self.explicit_debug_symbol(),
    };
    self.write_json(&self.layout.native_lib_cache_file(), &cache)?;

    let binxo_cache = NativeBinxoCache {
      libs: &process_libs,
      binxo: &binxo,
      binxo_symbol: NativeBinxoSymbol {
        enable_asan_binxo: self.binxo_valid,
        exclude_so_from_binxo: &self.option.exclude_so_from_binxo,
      },
    };
    self.write_json(&self.layout.binxo_native_lib_cache_file(), &binxo_cache)
  }

  pub(crate) fn write_json<T: Serialize>(
    &self,
    cache_file: &Path,
    cache: &T,
  ) -> NativeBuildResult<()> {
    if let Some(parent) = cache_file.parent() {
      std::fs::create_dir_all(parent).map_err(|error| {
        NativeBuildError::NativePostProcessFailed {
          path: parent.to_path_buf(),
          message: error.to_string(),
        }
      })?;
    }
    let bytes =
      serde_json::to_vec(&cache).map_err(|error| NativeBuildError::NativePostProcessFailed {
        path: cache_file.to_path_buf(),
        message: error.to_string(),
      })?;
    std::fs::write(cache_file, bytes).map_err(|error| NativeBuildError::NativePostProcessFailed {
      path: cache_file.to_path_buf(),
      message: error.to_string(),
    })
  }

  pub(crate) fn explicit_debug_symbol(&self) -> Option<&arkdown_core::NativeDebugSymbolOption> {
    self
      .option
      .debug_symbol_configured
      .then_some(&self.option.debug_symbol)
  }

  pub(crate) fn hash_directory(&self, root: &Path) -> NativeBuildResult<BTreeMap<String, String>> {
    if !root.exists() {
      return Ok(BTreeMap::new());
    }
    let mut files = walkdir::WalkDir::new(root)
      .into_iter()
      .filter_map(Result::ok)
      .filter(|entry| entry.file_type().is_file())
      .map(|entry| entry.into_path())
      .collect::<Vec<_>>();
    files.sort();

    let mut hashes = BTreeMap::new();
    for path in files {
      hashes.insert(path.to_string_lossy().into_owned(), Self::file_hash(&path)?);
    }
    Ok(hashes)
  }

  pub(crate) fn file_hash(path: &Path) -> NativeBuildResult<String> {
    let metadata =
      std::fs::metadata(path).map_err(|error| NativeBuildError::NativePostProcessFailed {
        path: path.to_path_buf(),
        message: error.to_string(),
      })?;
    let modified_ms = metadata
      .modified()
      .and_then(|modified| {
        modified
          .duration_since(UNIX_EPOCH)
          .map_err(std::io::Error::other)
      })
      .map_err(|error| NativeBuildError::NativePostProcessFailed {
        path: path.to_path_buf(),
        message: error.to_string(),
      })?
      .as_millis();
    let fingerprint = (u128::from(metadata.len()) + modified_ms).to_string();
    Ok(format!("{:x}", Sha256::digest(fingerprint.as_bytes())))
  }
}

pub(crate) struct NativeDependencyLibDir {
  pub(crate) libs_dir: PathBuf,
  pub(crate) package: NativeLibPackageInfo,
}

/// Mirrors hvigor's `DoNativeStrip` and `tasks/worker/native-strip.ts`.
///
/// The debug-symbol `exclude` list is evaluated relative to the processed
/// native-library root (the first ABI component). With `strip: true`, matching
/// files are copied and all others are stripped; with `strip: false`, the
/// matching files are stripped and all others are copied.
pub(crate) struct NativeStripProcessor<'a> {
  pub(crate) layout: NativeModuleLayout<'a>,
  pub(crate) llvm_strip: Option<PathBuf>,
  pub(crate) option: &'a NativeLibOption,
}

impl<'a> NativeStripProcessor<'a> {
  pub(crate) fn for_variant(
    module_root: &'a Path,
    product_name: &'a str,
    target_name: &'a str,
    llvm_strip: Option<PathBuf>,
    option: &'a NativeLibOption,
  ) -> Self {
    Self {
      layout: NativeModuleLayout::for_variant(module_root, product_name, target_name),
      llvm_strip,
      option,
    }
  }

  pub(crate) fn process(&self, libs: &[PathBuf]) -> NativeBuildResult<Vec<PathBuf>> {
    let output_root = self.layout.stripped_libs_dir();
    std::fs::create_dir_all(&output_root).map_err(|error| {
      NativeBuildError::NativePostProcessFailed {
        path: output_root.clone(),
        message: error.to_string(),
      }
    })?;
    let exclude_matchers = self.exclude_matchers()?;
    let mut outputs = Vec::with_capacity(libs.len());
    let mut expected = FxHashSet::default();

    for source in libs {
      let relative = NativeLibCollector::native_lib_identity(source);
      if relative.is_empty() {
        return Err(NativeBuildError::NativeLibRelativePath {
          path: source.clone(),
          message: "missing ABI-relative path".to_string(),
        });
      }
      let destination = output_root.join(Path::new(&relative));
      expected.insert(destination.clone());
      if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
          NativeBuildError::NativePostProcessFailed {
            path: parent.to_path_buf(),
            message: error.to_string(),
          }
        })?;
      }

      let excluded = exclude_matchers
        .iter()
        .any(|matcher| matcher.is_match(relative.as_str()));
      let should_strip = self.should_strip(excluded);
      if should_strip {
        self.strip_or_copy(source, &destination)?;
      } else {
        self.copy(source, &destination)?;
      }
      outputs.push(destination);
    }

    self.remove_stale_outputs(&output_root, &expected)?;
    outputs.sort();
    Ok(outputs)
  }

  pub(crate) fn exclude_matchers(&self) -> NativeBuildResult<Vec<globset::GlobMatcher>> {
    self
      .option
      .debug_symbol
      .exclude
      .iter()
      .map(|pattern| {
        let normalized = ArtifactPathSerializer::rel_to_slash(Path::new(pattern));
        globset::GlobBuilder::new(&normalized)
          .literal_separator(false)
          .build()
          .map(|glob| glob.compile_matcher())
          .map_err(|error| NativeBuildError::InvalidNativePattern {
            pattern: pattern.clone(),
            message: error.to_string(),
          })
      })
      .collect()
  }

  pub(crate) fn should_strip(&self, excluded: bool) -> bool {
    if self.option.debug_symbol.strip {
      !excluded
    } else {
      excluded
    }
  }

  pub(crate) fn strip_or_copy(&self, source: &Path, destination: &Path) -> NativeBuildResult<()> {
    let Some(llvm_strip) = self.llvm_strip.as_deref() else {
      tracing::warn!(
        source = %source.display(),
        "SDK llvm-strip is unavailable; preserving the unstripped native library"
      );
      return self.copy(source, destination);
    };
    let output = Command::new(llvm_strip)
      .arg("--keep-section=.permission")
      .arg(format!("-o{}", destination.display()))
      .arg(source)
      .output();
    match output {
      Ok(output) if output.status.success() => Ok(()),
      Ok(output) => {
        tracing::warn!(
          source = %source.display(),
          stderr = %String::from_utf8_lossy(&output.stderr).trim(),
          "llvm-strip failed; preserving the unstripped native library"
        );
        self.copy(source, destination)
      }
      Err(error) => {
        tracing::warn!(
          source = %source.display(),
          error = %error,
          "llvm-strip could not be started; preserving the unstripped native library"
        );
        self.copy(source, destination)
      }
    }
  }

  pub(crate) fn copy(&self, source: &Path, destination: &Path) -> NativeBuildResult<()> {
    if destination.exists() {
      std::fs::remove_file(destination).map_err(|error| {
        NativeBuildError::NativePostProcessFailed {
          path: destination.to_path_buf(),
          message: error.to_string(),
        }
      })?;
    }
    if std::fs::hard_link(source, destination).is_ok() {
      return Ok(());
    }
    std::fs::copy(source, destination)
      .map(|_| ())
      .map_err(|error| NativeBuildError::NativePostProcessFailed {
        path: source.to_path_buf(),
        message: error.to_string(),
      })
  }

  pub(crate) fn remove_stale_outputs(
    &self,
    output_root: &Path,
    expected: &FxHashSet<PathBuf>,
  ) -> NativeBuildResult<()> {
    if !output_root.is_dir() {
      return Ok(());
    }
    let mut stale_files = walkdir::WalkDir::new(output_root)
      .contents_first(true)
      .into_iter()
      .filter_map(Result::ok)
      .map(|entry| entry.into_path())
      .filter(|path| path.is_file() && !expected.contains(path))
      .collect::<Vec<_>>();
    stale_files.sort();
    for stale in stale_files {
      std::fs::remove_file(&stale).map_err(|error| NativeBuildError::NativePostProcessFailed {
        path: stale,
        message: error.to_string(),
      })?;
    }
    Ok(())
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeLibSelectionState {
  None,
  Include,
  Exclude,
}

#[derive(Debug, Clone)]
pub(crate) struct NativeLibCandidate {
  pub(crate) path: PathBuf,
  pub(crate) relative_path: String,
  pub(crate) name: String,
  pub(crate) package: NativeLibPackageInfo,
  pub(crate) selected: NativeLibSelectionState,
}

impl NativeLibCandidate {
  pub(crate) fn new(path: PathBuf, relative_path: String, package: NativeLibPackageInfo) -> Self {
    let name = Path::new(&relative_path)
      .file_name()
      .and_then(|name| name.to_str())
      .unwrap_or("")
      .to_string();
    Self {
      path,
      relative_path,
      name,
      package,
      selected: NativeLibSelectionState::None,
    }
  }

  pub(crate) fn from_collected_path(
    path: PathBuf,
    package: NativeLibPackageInfo,
  ) -> NativeBuildResult<Self> {
    let relative_path = NativeLibCollector::native_lib_identity(&path);
    if relative_path.is_empty() {
      return Err(NativeBuildError::NativeLibRelativePath {
        path,
        message: "missing file name".to_string(),
      });
    }
    Ok(Self::new(path, relative_path, package))
  }

  pub(crate) fn relative_path(base: &Path, path: &Path) -> NativeBuildResult<String> {
    let relative =
      path
        .strip_prefix(base)
        .map_err(|error| NativeBuildError::NativeLibRelativePath {
          path: path.to_path_buf(),
          message: error.to_string(),
        })?;
    ArtifactPathSerializer::path_to_slash_string(relative).map_err(|error| {
      NativeBuildError::NativeLibRelativePath {
        path: path.to_path_buf(),
        message: error.to_string(),
      }
    })
  }

  pub(crate) fn matches_package_selection(&self, selection: &NativeLibPackageSelection) -> bool {
    self.package.name == selection.package
      && selection
        .version
        .as_ref()
        .map(|version| self.package.version == *version)
        .unwrap_or(true)
  }

  pub(crate) fn relative_path_as_path(&self) -> &Path {
    Path::new(&self.relative_path)
  }
}

pub(crate) struct NativeLibFilterApplier<'a> {
  pub(crate) filter: &'a NativeLibFilterOption,
}

impl<'a> NativeLibFilterApplier<'a> {
  pub(crate) fn new(filter: &'a NativeLibFilterOption) -> Self {
    Self { filter }
  }

  pub(crate) fn apply(&self, candidates: &mut [NativeLibCandidate]) {
    for selection in &self.filter.select {
      for index in self.filter_libs(candidates, selection, true) {
        candidates[index].selected = NativeLibSelectionState::Include;
      }
      for index in self.filter_libs(candidates, selection, false) {
        candidates[index].selected = NativeLibSelectionState::Exclude;
      }
    }
  }

  pub(crate) fn filter_libs(
    &self,
    candidates: &[NativeLibCandidate],
    selection: &NativeLibPackageSelection,
    include: bool,
  ) -> Vec<usize> {
    let names = if include {
      selection.include.as_ref()
    } else {
      selection.exclude.as_ref()
    };
    let patterns = if include {
      selection.include_pattern.as_ref()
    } else {
      selection.exclude_pattern.as_ref()
    };
    candidates
      .iter()
      .enumerate()
      .filter_map(|(index, candidate)| {
        if !candidate.matches_package_selection(selection) {
          return None;
        }
        if let Some(names) = names {
          return names
            .iter()
            .any(|name| name == &candidate.name)
            .then_some(index);
        }
        if let Some(patterns) = patterns {
          return patterns
            .iter()
            .any(|pattern| {
              HvigorNativeLibraryPattern::matches_filter_pattern(
                candidate.relative_path_as_path(),
                pattern,
                true,
              )
            })
            .then_some(index);
        }
        None
      })
      .collect()
  }
}

pub(crate) struct NativeLibSelectionResolver<'a> {
  pub(crate) module_name: &'a str,
  pub(crate) filter: &'a NativeLibFilterOption,
}

impl<'a> NativeLibSelectionResolver<'a> {
  pub(crate) fn new(module_name: &'a str, filter: &'a NativeLibFilterOption) -> Self {
    Self {
      module_name,
      filter,
    }
  }

  pub(crate) fn resolve(
    &self,
    candidates: Vec<NativeLibCandidate>,
  ) -> NativeBuildResult<Vec<PathBuf>> {
    let mut groups = Vec::<(String, Vec<PathBuf>)>::new();
    let mut group_indexes = rustc_hash::FxHashMap::<String, usize>::default();
    let mut selected_relative_paths = FxHashSet::default();
    for candidate in candidates {
      if matches!(candidate.selected, NativeLibSelectionState::Exclude) {
        continue;
      }
      let relative_path = candidate.relative_path.clone();
      if selected_relative_paths.contains(relative_path.as_str()) {
        continue;
      }
      if matches!(candidate.selected, NativeLibSelectionState::Include) {
        self.replace_group(
          &mut groups,
          &mut group_indexes,
          relative_path.clone(),
          candidate.path,
        );
        selected_relative_paths.insert(relative_path);
        continue;
      }
      if self.is_excluded(&candidate) {
        continue;
      }
      if self.should_replace_with_last(&candidate, &group_indexes)
        || self.should_replace_with_first_or_override(&candidate)
      {
        self.replace_group(
          &mut groups,
          &mut group_indexes,
          relative_path,
          candidate.path,
        );
      } else {
        self.append_to_group(
          &mut groups,
          &mut group_indexes,
          relative_path,
          candidate.path,
        );
      }
    }

    let mut resolved = Vec::new();
    let mut conflicts = Vec::new();
    for (identity, group) in groups {
      if group.len() < 2 {
        resolved.extend(group);
        continue;
      }
      if NativeLibCollector::is_duplicate_check_exempt(&identity) {
        if let Some(lib) = group.last() {
          resolved.push(lib.clone());
        }
        continue;
      }
      let unique = NativeDuplicateGroup::new(&group).unique_files()?;
      if unique.len() < 2 {
        if let Some(lib) = group.last() {
          resolved.push(lib.clone());
        }
      } else {
        conflicts.extend(unique);
      }
    }

    if conflicts.is_empty() {
      Ok(resolved)
    } else {
      Err(NativeBuildError::DuplicatedFilesFoundInLibPath {
        module_name: self.module_name.to_string(),
        libs: conflicts,
      })
    }
  }

  pub(crate) fn is_excluded(&self, candidate: &NativeLibCandidate) -> bool {
    self.filter_excludes().iter().any(|pattern| {
      HvigorNativeLibraryPattern::matches_filter_pattern(
        candidate.relative_path_as_path(),
        pattern,
        false,
      )
    })
  }

  pub(crate) fn filter_excludes(&self) -> &[String] {
    &self.filter.excludes
  }

  pub(crate) fn should_replace_with_last(
    &self,
    candidate: &NativeLibCandidate,
    group_indexes: &rustc_hash::FxHashMap<String, usize>,
  ) -> bool {
    group_indexes.contains_key(candidate.relative_path.as_str())
      && self.filter.pick_lasts.iter().any(|pattern| {
        HvigorNativeLibraryPattern::matches_filter_pattern(
          candidate.relative_path_as_path(),
          pattern,
          false,
        )
      })
  }

  pub(crate) fn should_replace_with_first_or_override(
    &self,
    candidate: &NativeLibCandidate,
  ) -> bool {
    self.filter.enable_override
      || self.filter.pick_firsts.iter().any(|pattern| {
        HvigorNativeLibraryPattern::matches_filter_pattern(
          candidate.relative_path_as_path(),
          pattern,
          false,
        )
      })
  }

  pub(crate) fn replace_group(
    &self,
    groups: &mut Vec<(String, Vec<PathBuf>)>,
    group_indexes: &mut rustc_hash::FxHashMap<String, usize>,
    relative_path: String,
    path: PathBuf,
  ) {
    if let Some(index) = group_indexes.get(relative_path.as_str()).copied() {
      groups[index].1 = vec![path];
      return;
    }
    let index = groups.len();
    group_indexes.insert(relative_path.clone(), index);
    groups.push((relative_path, vec![path]));
  }

  pub(crate) fn append_to_group(
    &self,
    groups: &mut Vec<(String, Vec<PathBuf>)>,
    group_indexes: &mut rustc_hash::FxHashMap<String, usize>,
    relative_path: String,
    path: PathBuf,
  ) {
    if let Some(index) = group_indexes.get(relative_path.as_str()).copied() {
      groups[index].1.push(path);
      return;
    }
    let index = groups.len();
    group_indexes.insert(relative_path.clone(), index);
    groups.push((relative_path, vec![path]));
  }
}

pub(crate) struct NativeDuplicateGroup<'a> {
  pub(crate) libs: &'a [PathBuf],
}

impl<'a> NativeDuplicateGroup<'a> {
  pub(crate) fn new(libs: &'a [PathBuf]) -> Self {
    Self { libs }
  }

  pub(crate) fn unique_files(&self) -> NativeBuildResult<Vec<PathBuf>> {
    let mut unique = Vec::<PathBuf>::new();
    for lib in self.libs {
      let mut matched = false;
      for existing in &unique {
        if NativeDuplicateFile::new(lib).is_same_file(existing)? {
          matched = true;
          break;
        }
      }
      if !matched {
        unique.push(lib.clone());
      }
    }
    Ok(unique)
  }
}

pub(crate) struct NativeDuplicateFile<'a> {
  pub(crate) path: &'a Path,
}

impl<'a> NativeDuplicateFile<'a> {
  pub(crate) fn new(path: &'a Path) -> Self {
    Self { path }
  }

  pub(crate) fn is_same_file(&self, other: &Path) -> NativeBuildResult<bool> {
    if self.canonical_equals(other) {
      return Ok(true);
    }
    let left =
      std::fs::metadata(self.path).map_err(|err| NativeBuildError::DuplicateCheckFailed {
        path: self.path.to_path_buf(),
        message: err.to_string(),
      })?;
    let right = std::fs::metadata(other).map_err(|err| NativeBuildError::DuplicateCheckFailed {
      path: other.to_path_buf(),
      message: err.to_string(),
    })?;
    if left.len() != right.len() {
      return Ok(false);
    }
    self.content_equals(other)
  }

  pub(crate) fn canonical_equals(&self, other: &Path) -> bool {
    let Ok(left) = arkdown_path::AbsPath::canonicalize(self.path) else {
      return false;
    };
    let Ok(right) = arkdown_path::AbsPath::canonicalize(other) else {
      return false;
    };
    left == right
  }

  pub(crate) fn content_equals(&self, other: &Path) -> NativeBuildResult<bool> {
    let mut left =
      std::fs::File::open(self.path).map_err(|err| NativeBuildError::DuplicateCheckFailed {
        path: self.path.to_path_buf(),
        message: err.to_string(),
      })?;
    let mut right =
      std::fs::File::open(other).map_err(|err| NativeBuildError::DuplicateCheckFailed {
        path: other.to_path_buf(),
        message: err.to_string(),
      })?;
    let mut left_buf = [0u8; 8192];
    let mut right_buf = [0u8; 8192];
    loop {
      let left_read =
        left
          .read(&mut left_buf)
          .map_err(|err| NativeBuildError::DuplicateCheckFailed {
            path: self.path.to_path_buf(),
            message: err.to_string(),
          })?;
      let right_read =
        right
          .read(&mut right_buf)
          .map_err(|err| NativeBuildError::DuplicateCheckFailed {
            path: other.to_path_buf(),
            message: err.to_string(),
          })?;
      if left_read != right_read {
        return Ok(false);
      }
      if left_read == 0 {
        return Ok(true);
      }
      if left_buf[..left_read] != right_buf[..right_read] {
        return Ok(false);
      }
    }
  }
}

impl NativeBuildConfig {
  /// Execute the latest-hvigor `BuildNativeWithCmake` action: validate and
  /// generate the ABI-specific CMake/Ninja build trees without compiling.
  pub fn configure(&self) -> NativeBuildResult<()> {
    if !self.should_run_native_tasks() {
      return Ok(());
    }
    self.validate_cmake_script()?;
    self
      .run_cmake_configure()
      .map_err(|message| NativeBuildError::CmakeFailed {
        module_root: self.module_root.clone(),
        message,
      })
  }

  /// Execute the latest-hvigor `BuildNativeWithNinja` action for every
  /// effective ABI. Configuration is intentionally not repeated here.
  pub fn build(&self) -> NativeBuildResult<()> {
    let commands = self.ninja_command_plans()?;
    if commands.is_empty() {
      return Ok(());
    }
    for command in commands {
      command.execute()?;
    }
    tracing::info!(
        module = %self.module_root.display(),
        "native: CMake/Ninja build succeeded"
    );
    Ok(())
  }

  /// Execute only the latest-hvigor `ProcessLibs` import scan. Native command
  /// execution is owned by [`Self::configure`] and [`Self::build`].
  pub fn scan(&self) -> NativeBuildResult<NativeBuildOutput> {
    let mut output = NativeBuildOutput::default();
    let abi_filters = NativeAbiFilters::new(&self.abi_filters).package_abis();
    let search_roots = self.so_search_roots();
    for root in &search_roots {
      for abi in &abi_filters {
        let abi_dir = root.join(abi.as_str());
        if !abi_dir.is_dir() {
          continue;
        }
        let mut sorted: Vec<PathBuf> = walkdir::WalkDir::new(&abi_dir)
          .into_iter()
          .filter_map(|e| e.ok())
          .filter(|e| e.file_type().is_file())
          .map(|e| e.into_path())
          .filter(|p| {
            HvigorNativeLibraryPattern::matches_path_under_with_collect_all(
              &abi_dir,
              p,
              self.collect_all_libs,
            )
          })
          .collect();
        sorted.sort();
        for so_path in sorted {
          output.libs_by_abi.push((abi.clone(), so_path));
        }
      }
    }

    Ok(output)
  }

  pub(crate) fn validate_cmake_script(&self) -> NativeBuildResult<()> {
    let cmake_file = self.cmake_lists_path();
    if cmake_file.file_name() != Some(OsStr::new("CMakeLists.txt")) {
      return Err(NativeBuildError::CmakeFailed {
        module_root: self.module_root.clone(),
        message: format!(
          "externalNativeOptions.path must name CMakeLists.txt: {}",
          cmake_file.display()
        ),
      });
    }
    if !cmake_file.is_file() {
      return Err(NativeBuildError::CmakeFailed {
        module_root: self.module_root.clone(),
        message: format!(
          "configured CMake script does not exist: {}",
          cmake_file.display()
        ),
      });
    }
    Ok(())
  }

  /// Directories to scan for pre-built `.so` files, in priority order.
  pub(crate) fn so_search_roots(&self) -> Vec<PathBuf> {
    self.layout().so_search_roots()
  }

  pub(crate) fn run_cmake_configure(&self) -> Result<(), String> {
    let toolchain = self.sdk_toolchain.require()?;
    let layout = self.layout();
    let cmake_lists = self.cmake_lists_path();
    let source_dir = cmake_lists
      .parent()
      .ok_or_else(|| format!("CMake script has no parent: {}", cmake_lists.display()))?;
    for abi in NativeAbiFilters::new(&self.abi_filters).build_abis() {
      let build_dir = layout
        .ninja_work_dir(self.effective_build_mode())
        .join(&abi);
      let output_dir = layout.cmake_obj_dir().join(&abi);
      let cmake_find_root = layout
        .cmake_dependency_root(self.effective_build_mode())
        .join(&abi);
      std::fs::create_dir_all(&build_dir)
        .map_err(|e| format!("cannot create CMake build dir: {e}"))?;
      std::fs::create_dir_all(&output_dir)
        .map_err(|e| format!("cannot create CMake output dir: {e}"))?;
      let compiler_state =
        NativeCmakeCompilerState::new(&build_dir, toolchain.compiler, toolchain.toolchain_file);
      compiler_state
        .prepare()
        .map_err(|error| error.to_string())?;
      self.create_cmake_file_api_queries(&build_dir)?;
      let cmake_toolchain_file = self
        .cmake_toolchain_file(&toolchain, &build_dir, &abi)
        .map_err(|error| error.to_string())?;
      let mut configure_cmd = self.cmake_configure_command(
        &toolchain,
        &cmake_toolchain_file,
        &abi,
        source_dir,
        &build_dir,
        &output_dir,
        &cmake_find_root,
      );

      tracing::info!(
          abi = %abi,
          cmd = ?configure_cmd,
          "native: running CMake configure"
      );

      let configure_status = configure_cmd
        .status()
        .map_err(|e| format!("failed to run cmake configure: {e}"))?;

      if !configure_status.success() {
        return Err(format!(
          "cmake configure for ABI {abi} exited with status {configure_status}"
        ));
      }
      compiler_state.commit().map_err(|error| error.to_string())?;
    }
    Ok(())
  }

  pub(crate) fn cmake_toolchain_file(
    &self,
    toolchain: &ResolvedNativeSdkToolchain<'_>,
    build_dir: &Path,
    abi: &str,
  ) -> NativeBuildResult<PathBuf> {
    let Some(zig) = toolchain.compiler.zig_executable() else {
      return Ok(toolchain.toolchain_file.to_path_buf());
    };
    NativeZigToolchainWriter::new(
      toolchain.toolchain_file,
      toolchain.native_dir,
      zig,
      self.effective_ohos_stl(abi)?,
      self.effective_sanitizers(abi),
      abi,
      build_dir,
    )
    .write()
  }

  pub(crate) fn ninja_command_plans(&self) -> NativeBuildResult<Vec<NativeCommandPlan>> {
    if !self.should_run_native_tasks() {
      return Ok(Vec::new());
    }
    self.validate_cmake_script()?;
    let toolchain =
      self
        .sdk_toolchain
        .require()
        .map_err(|message| NativeBuildError::CmakeFailed {
          module_root: self.module_root.clone(),
          message,
        })?;
    Ok(
      NativeAbiFilters::new(&self.abi_filters)
        .build_abis()
        .into_iter()
        .map(|abi| self.ninja_command_plan(&toolchain, &abi))
        .collect(),
    )
  }

  /// Build the exact command emitted by hvigor 6.1.1.280
  /// `BuildNativeWithCmake.buildCommand` after `CmakeUtil.mergeCommandLine`.
  pub(crate) fn cmake_configure_command(
    &self,
    toolchain: &ResolvedNativeSdkToolchain<'_>,
    cmake_toolchain_file: &Path,
    abi: &str,
    source_dir: &Path,
    build_dir: &Path,
    output_dir: &Path,
    cmake_find_root: &Path,
  ) -> Command {
    let mut defaults = vec![
      self.prefixed_path_argument("-H", source_dir),
      self.prefixed_path_argument("-B", build_dir),
      OsString::from(format!("-DOHOS_ARCH={abi}")),
      self.prefixed_path_argument("-DCMAKE_LIBRARY_OUTPUT_DIRECTORY=", output_dir),
      OsString::from(format!("-DCMAKE_BUILD_TYPE={}", self.cmake_build_type())),
      self.prefixed_path_argument("-DOHOS_SDK_NATIVE=", toolchain.native_dir),
      OsString::from("-DCMAKE_SYSTEM_NAME=OHOS"),
      OsString::from(format!("-DCMAKE_OHOS_ARCH_ABI={abi}")),
      OsString::from("-DCMAKE_EXPORT_COMPILE_COMMANDS=ON"),
      self.prefixed_path_argument("-DCMAKE_TOOLCHAIN_FILE=", cmake_toolchain_file),
      OsString::from("-GNinja"),
      self.prefixed_path_argument("-DCMAKE_MAKE_PROGRAM=", toolchain.ninja),
    ];
    if cmake_find_root.exists() {
      defaults.push(self.prefixed_path_argument("-DCMAKE_FIND_ROOT_PATH=", cmake_find_root));
    }
    let package_summary = cmake_find_root.join("summary.cmake");
    if package_summary.exists() {
      defaults.push(self.prefixed_path_argument("-DPACKAGE_FIND_FILE=", &package_summary));
    }
    defaults.push(OsString::from("--no-warn-unused-cli"));
    if let Some(options) = self.external_native_options.as_ref() {
      if let Some(c_flags) = options
        .c_flags
        .as_deref()
        .filter(|flags| !flags.trim().is_empty())
      {
        defaults.push(OsString::from(format!("-DCMAKE_C_FLAGS={c_flags}")));
      }
      if let Some(cpp_flags) = options
        .cpp_flags
        .as_deref()
        .filter(|flags| !flags.trim().is_empty())
      {
        defaults.push(OsString::from(format!("-DCMAKE_CXX_FLAGS={cpp_flags}")));
      }
    }

    let mut custom = self.custom_cmake_arguments(abi);
    if toolchain.compiler.zig_executable().is_some() {
      // `--enable-zig` is an explicit CLI override. Put the generated
      // toolchain before build-profile arguments so hvigor's first-key-wins
      // merge cannot replace it with a project-supplied toolchain.
      custom.insert(
        0,
        self.prefixed_path_argument("-DCMAKE_TOOLCHAIN_FILE=", cmake_toolchain_file),
      );
    }
    let mut command = Command::new(toolchain.cmake);
    command
      .args(self.merged_cmake_arguments(defaults, custom))
      .current_dir(build_dir);
    command
  }

  pub(crate) fn ninja_command_plan(
    &self,
    toolchain: &ResolvedNativeSdkToolchain<'_>,
    abi: &str,
  ) -> NativeCommandPlan {
    let build_dir = self
      .layout()
      .ninja_work_dir(self.effective_build_mode())
      .join(abi);
    NativeCommandPlan::ninja(
      &self.module_root,
      abi,
      toolchain.ninja,
      build_dir,
      self
        .external_native_options
        .iter()
        .flat_map(|options| options.targets.iter().cloned()),
    )
  }

  /// Hvigor treats arguments from `externalNativeOptions.arguments` and
  /// sanitizer switches as native overrides, in that order.
  pub(crate) fn custom_cmake_arguments(&self, abi: &str) -> Vec<OsString> {
    let configured = self
      .external_native_options
      .iter()
      .flat_map(|options| options.arguments.iter())
      .flat_map(|argument| argument.trim().split(' '))
      .filter(|argument| argument.starts_with('-'))
      .map(OsString::from);
    configured
      .chain(self.sanitizers.cmake_args_for_abi(abi).map(OsString::from))
      .collect()
  }

  fn effective_sanitizers(&self, abi: &str) -> NativeBuildSanitizers {
    let arguments = self.merged_cmake_arguments(Vec::new(), self.custom_cmake_arguments(abi));
    NativeBuildSanitizers::from_cmake_arguments(&arguments)
  }

  fn effective_ohos_stl(&self, abi: &str) -> NativeBuildResult<NativeOhosStl> {
    let arguments = self.merged_cmake_arguments(Vec::new(), self.custom_cmake_arguments(abi));
    NativeOhosStl::from_cmake_arguments(&arguments).map_err(|message| {
      NativeBuildError::ConfigureCmakeFailed {
        path: self.module_root.clone(),
        message,
      }
    })
  }

  /// Mirrors hvigor `CmakeUtil.mergeCommandLine`: native arguments are parsed
  /// first, the first value for a flag key wins, and remaining defaults retain
  /// insertion order.
  pub(crate) fn merged_cmake_arguments(
    &self,
    defaults: Vec<OsString>,
    custom: Vec<OsString>,
  ) -> Vec<OsString> {
    let mut identities = FxHashSet::default();
    let mut merged = Vec::with_capacity(defaults.len() + custom.len());
    for argument in custom.into_iter().chain(defaults) {
      let Some((identity, normalized)) = self.normalized_cmake_argument(&argument) else {
        continue;
      };
      if identities.insert(identity) {
        merged.push(normalized);
      }
    }
    merged
  }

  pub(crate) fn normalized_cmake_argument(&self, argument: &OsStr) -> Option<(String, OsString)> {
    let Some(argument) = argument.to_str() else {
      let identity = self.cmake_argument_identity(&argument.to_string_lossy())?;
      return Some((identity, argument.to_os_string()));
    };
    let argument = argument.trim();
    let identity = self.cmake_argument_identity(argument)?;
    let mut chars = argument.chars();
    let prefix = [chars.next()?, chars.next()?]
      .into_iter()
      .collect::<String>();
    let remainder = chars.as_str();
    let normalized = if let Some((key, value)) = remainder.split_once('=') {
      format!("{prefix}{}={}", key.trim(), value.trim())
    } else {
      format!("{prefix}{remainder}")
    };
    Some((identity, OsString::from(normalized)))
  }

  pub(crate) fn cmake_argument_identity(&self, argument: &str) -> Option<String> {
    let argument = argument.trim();
    if !argument.starts_with('-') {
      return None;
    }
    let mut chars = argument.chars();
    let first = chars.next()?;
    let second = chars.next()?;
    let key = chars
      .as_str()
      .split_once('=')
      .map_or(chars.as_str(), |(key, _)| key);
    Some(format!("{first}{second}{}", key.trim()))
  }

  pub(crate) fn prefixed_path_argument(&self, prefix: &str, path: &Path) -> OsString {
    let mut argument = OsString::from(prefix);
    argument.push(path.as_os_str());
    argument
  }

  pub(crate) fn cmake_build_type(&self) -> &'static str {
    match self.build_mode.as_str() {
      "debug" | "Debug" => "Debug",
      "release" | "Release" => self.release_cmake_build_type.as_str(),
      _ => "Debug",
    }
  }

  pub(crate) fn effective_build_mode(&self) -> &str {
    if self.build_mode.is_empty() {
      "debug"
    } else {
      &self.build_mode
    }
  }

  pub(crate) fn should_run_native_tasks(&self) -> bool {
    self.enable_cmake_build
      && (self
        .external_native_options
        .as_ref()
        .is_some_and(|options| options.path.is_some())
        || self.layout().cmake_lists().is_file())
  }

  pub(crate) fn create_cmake_file_api_queries(&self, build_dir: &Path) -> Result<(), String> {
    let query_root = build_dir
      .join(".cmake")
      .join("api")
      .join("v1")
      .join("query");
    for query in ["codemodel-v2", "cmakeFiles-v1", "cache-v2"] {
      std::fs::create_dir_all(query_root.join(query))
        .map_err(|error| format!("cannot create CMake file API query '{query}': {error}"))?;
    }
    Ok(())
  }

  pub(crate) fn cmake_lists_path(&self) -> PathBuf {
    self
      .external_native_options
      .as_ref()
      .and_then(|options| options.path.clone())
      .unwrap_or_else(|| self.layout().cmake_lists())
  }

  pub(crate) fn layout(&self) -> NativeModuleLayout<'_> {
    NativeModuleLayout::for_variant(&self.module_root, &self.product_name, &self.target_name)
  }
}
