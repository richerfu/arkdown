//! Native (C/C++) build integration for OHOS modules.
//!
//! Implements the "native artifact import/build 模式" gap from the hvigor
//! analysis (sections 3.1 and 3.8): "增加 native artifact import/build 模式".
//!
//! ## Modes
//!
//! ### Import mode (always available)
//!
//! Collects pre-built shared libraries from the paths that DevEco / hvigor produces:
//! - `build/default/intermediates/cmake/default/obj/<abi>/` — CMake output
//! - `build/default/intermediates/cj/extend_libs/default/<abi>/` — Cangjie output
//! - `libs/<abi>/` — module-local prebuilt native libraries
//!
//! This mode is used when the native code has already been compiled by a
//! previous `hvigorw assembleNative` invocation (or when developers provide
//! pre-built shared libraries).
//!
//! ### Build mode (requires CMake toolchain)
//!
//! When `CMakeLists.txt` is present in the module's source tree AND a CMake
//! binary can be located on `$PATH`, arkdown can invoke CMake to build the
//! native code in-process before packaging.
//!
//! hvigor's native build task sequence:
//! 1. `cmake -G Ninja -DCMAKE_TOOLCHAIN_FILE=<ndk>/build/cmake/ohos.toolchain.cmake`
//! 2. `cmake --build . --target <target> -- -j<cores>`
//!
//! We replicate this sequence using `std::process::Command`.  The NDK
//! toolchain file path is derived from `$OHOS_NDK_HOME` or `$OHOS_SDK_HOME`.
//!
//! ## Relationship to the gap analysis
//!
//! The gap analysis section 3.1 notes: "阻塞：native CMake 构建（需要 CMake
//! 工具链和 native toolchain，外部工具依赖）". This module bridges that gap by:
//!
//! - Detecting `CMakeLists.txt` and emitting structured diagnostics when the
//!   NDK/CMake toolchain is absent (instead of silently skipping).
//! - Running the native build automatically from the active SDK toolchain.
//! - Failing with a structured diagnostic when required native tools are absent.

use std::error::Error;
use std::ffi::OsStr;
use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use arkdown_core::{
  ModuleConfig, NativeLibFilterOption, NativeLibOption, NativeLibPackageSelection,
};
use arkdown_path::{ArtifactPathSerializer, HvigorNativeLibraryPattern};
use arkdown_session::dependency_graph::snapshot::{
  DependencyGraphSnapshot, DependencyNodeSnapshot,
};
use rustc_hash::FxHashSet;

/// The set of ABI directories that hvigor's native pipeline produces under
/// the CMake / NDK build intermediates.
const KNOWN_ABI_DIRS: &[&str] = &["arm64-v8a", "armeabi-v7a", "x86_64", "x86", "armeabi"];
/// hvigor `BuildDirConst.LIBS`, used by `dependency.getDefaultLibsDir()`.
const HVIGOR_LIBS_DIR: &str = "libs";

/// Result of a native build or import scan.
#[derive(Debug, Clone, Default)]
pub struct NativeBuildOutput {
  /// Absolute paths to native shared libraries ready for packaging, indexed by ABI.
  pub libs_by_abi: Vec<(String, PathBuf)>,
  /// Warning/diagnostic messages to emit.  Non-empty when the build was
  /// partially skipped (missing toolchain, etc.).
  pub warnings: Vec<String>,
}

/// Describes the native build configuration for one module.
#[derive(Debug, Clone)]
pub struct NativeBuildConfig {
  /// Module root directory.
  pub module_root: PathBuf,
  /// ABI filters (from `buildOption.externalNativeOptions.abiFilters`).
  pub abi_filters: Vec<String>,
  /// Whether to attempt an actual CMake build.
  pub enable_cmake_build: bool,
  /// Sanitizer mode to pass into CMake. Mirrors hvigor extra native args.
  pub sanitizer: NativeBuildSanitizer,
  /// Build mode passed to CMake. Mirrors hvigor `targetService.getBuildMode()`.
  pub build_mode: String,
  /// SDK native toolchain paths resolved from the active build session.
  pub sdk_toolchain: NativeSdkToolchain,
  /// `buildOption.nativeLib.collectAllLibs`.
  pub collect_all_libs: bool,
}

/// Native sanitizer mode requested by hvigor-compatible extra config.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeBuildSanitizer {
  #[default]
  None,
  Asan,
  Hwasan,
}

impl NativeBuildSanitizer {
  fn cmake_arg_for_abi(self, abi: &str) -> Option<&'static str> {
    match self {
      Self::None => None,
      Self::Asan => Some("-DOHOS_ENABLE_ASAN=ON"),
      Self::Hwasan if abi == "arm64-v8a" => Some("-DOHOS_ENABLE_HWASAN=ON"),
      Self::Hwasan => None,
    }
  }
}

#[derive(Debug, Clone, Default)]
pub struct NativeLibCollectionPolicy {
  sanitizer: NativeBuildSanitizer,
  read_elf: Option<PathBuf>,
  build_mode: String,
  sdk_toolchain: NativeSdkToolchain,
  native_lib: NativeLibOption,
  self_package: Option<NativeLibPackageInfo>,
}

impl NativeLibCollectionPolicy {
  pub fn asan() -> Self {
    Self {
      sanitizer: NativeBuildSanitizer::Asan,
      read_elf: None,
      build_mode: String::new(),
      sdk_toolchain: NativeSdkToolchain::default(),
      native_lib: NativeLibOption::default(),
      self_package: None,
    }
  }

  pub fn hwasan(read_elf: Option<PathBuf>) -> Self {
    Self {
      sanitizer: NativeBuildSanitizer::Hwasan,
      read_elf,
      build_mode: String::new(),
      sdk_toolchain: NativeSdkToolchain::default(),
      native_lib: NativeLibOption::default(),
      self_package: None,
    }
  }

  pub fn with_build_mode(mut self, build_mode: String) -> Self {
    self.build_mode = build_mode;
    self
  }

  pub fn with_sdk_toolchain(mut self, sdk_toolchain: NativeSdkToolchain) -> Self {
    self.sdk_toolchain = sdk_toolchain;
    self
  }

  pub fn with_native_lib_option(mut self, native_lib: NativeLibOption) -> Self {
    self.native_lib = native_lib;
    self
  }

  pub fn with_self_package(mut self, package: NativeLibPackageInfo) -> Self {
    self.self_package = Some(package);
    self
  }

  fn enable_cmake_build(&self) -> bool {
    false
  }

  fn validate_packaged_libs(&self, libs: &[PathBuf]) -> NativeBuildResult<()> {
    if self.sanitizer != NativeBuildSanitizer::Hwasan {
      return Ok(());
    }
    let read_elf = self
      .read_elf
      .as_ref()
      .ok_or(NativeBuildError::MissingReadElf)?;
    NativeHwasanValidator::new(read_elf).validate(libs)
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeLibPackageInfo {
  name: String,
  version: String,
  is_local: bool,
}

impl NativeLibPackageInfo {
  pub fn local(name: impl Into<String>, version: impl Into<String>) -> Self {
    Self {
      name: name.into(),
      version: version.into(),
      is_local: true,
    }
  }

  fn dependency(node: &DependencyNodeSnapshot) -> Self {
    Self {
      name: node.package_name.clone(),
      version: node
        .package
        .as_ref()
        .map(|package| package.version.clone())
        .or_else(|| node.version_spec.clone())
        .unwrap_or_default(),
      is_local: false,
    }
  }
}

#[derive(Debug, Clone, Default)]
pub struct NativeSdkToolchain {
  cmake: Option<PathBuf>,
  ninja: Option<PathBuf>,
  native_dir: Option<PathBuf>,
  toolchain_file: Option<PathBuf>,
}

impl NativeSdkToolchain {
  pub fn new(sdk_root: PathBuf, cmake: Option<PathBuf>, ninja: Option<PathBuf>) -> Self {
    let native_dir = sdk_root.join("native");
    let toolchain_file = native_dir
      .join("build")
      .join("cmake")
      .join("ohos.toolchain.cmake");
    Self {
      cmake,
      ninja,
      native_dir: Some(native_dir),
      toolchain_file: Some(toolchain_file),
    }
  }

  fn require(&self) -> Result<ResolvedNativeSdkToolchain<'_>, String> {
    let cmake = self
      .cmake
      .as_deref()
      .ok_or_else(|| "SDK CMake tool path is not available".to_string())?;
    let ninja = self
      .ninja
      .as_deref()
      .ok_or_else(|| "SDK Ninja tool path is not available".to_string())?;
    let native_dir = self
      .native_dir
      .as_deref()
      .ok_or_else(|| "SDK native directory is not available".to_string())?;
    let toolchain_file = self
      .toolchain_file
      .as_deref()
      .ok_or_else(|| "SDK native toolchain file path is not available".to_string())?;
    if !cmake.is_file() {
      return Err(format!(
        "SDK CMake tool does not exist: {}",
        cmake.display()
      ));
    }
    if !ninja.is_file() {
      return Err(format!(
        "SDK Ninja tool does not exist: {}",
        ninja.display()
      ));
    }
    if !native_dir.is_dir() {
      return Err(format!(
        "SDK native directory does not exist: {}",
        native_dir.display()
      ));
    }
    if !toolchain_file.is_file() {
      return Err(format!(
        "SDK native toolchain file does not exist: {}",
        toolchain_file.display()
      ));
    }
    Ok(ResolvedNativeSdkToolchain {
      cmake,
      ninja,
      native_dir,
      toolchain_file,
    })
  }
}

struct ResolvedNativeSdkToolchain<'a> {
  cmake: &'a Path,
  ninja: &'a Path,
  native_dir: &'a Path,
  toolchain_file: &'a Path,
}

type NativeBuildResult<T> = Result<T, NativeBuildError>;

#[derive(Debug)]
pub enum NativeBuildError {
  CmakeFailed {
    module_root: PathBuf,
    message: String,
  },
  MissingReadElf,
  ReadElfFailed {
    path: PathBuf,
    message: String,
  },
  HwasanDisabled {
    libs: Vec<InvalidSanitizerNativeLib>,
  },
  UnsupportedNativeLibOrderSensitiveFilter,
  NativeLibRelativePath {
    path: PathBuf,
    message: String,
  },
  DuplicateCheckFailed {
    path: PathBuf,
    message: String,
  },
  DuplicatedFilesFoundInLibPath {
    module_name: String,
    libs: Vec<PathBuf>,
  },
}

impl fmt::Display for NativeBuildError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::CmakeFailed {
        module_root,
        message,
      } => write!(
        f,
        "native CMake build failed for '{}': {}",
        module_root.display(),
        message
      ),
      Self::MissingReadElf => write!(
        f,
        "ohos-enable-hwasan requires SDK llvm-readelf to validate native libraries"
      ),
      Self::ReadElfFailed { path, message } => write!(
        f,
        "failed to inspect native library '{}' with llvm-readelf: {}",
        path.display(),
        message
      ),
      Self::HwasanDisabled { libs } => {
        writeln!(
          f,
          "ohos-enable-hwasan requires every packaged native library to be built with HWASAN"
        )?;
        writeln!(f, "invalid native libraries:")?;
        for lib in libs {
          writeln!(f, "  - {} ({})", lib.path.display(), lib.evidence)?;
        }
        write!(
                    f,
                    "rebuild these native libraries with -DOHOS_ENABLE_HWASAN=ON or disable ohos-enable-hwasan"
                )
      }
      Self::UnsupportedNativeLibOrderSensitiveFilter => write!(
        f,
        "buildOption.nativeLib.filter pickFirsts, pickLasts, or enableOverride requires hvigor dependency order, which is not represented in the current dependency snapshot"
      ),
      Self::NativeLibRelativePath { path, message } => write!(
        f,
        "failed to compute hvigor native library relativePath for '{}': {}",
        path.display(),
        message
      ),
      Self::DuplicateCheckFailed { path, message } => write!(
        f,
        "failed to check duplicate native library '{}': {}",
        path.display(),
        message
      ),
      Self::DuplicatedFilesFoundInLibPath { module_name, libs } => {
        writeln!(
          f,
          "duplicated native libraries found in lib path for module '{}':",
          module_name
        )?;
        for lib in libs {
          writeln!(f, "  - {}", lib.display())?;
        }
        write!(f, "remove the duplicate native libraries or configure hvigor-compatible nativeLib.filter rules")
      }
    }
  }
}

impl Error for NativeBuildError {}

#[derive(Debug, Clone)]
pub struct InvalidSanitizerNativeLib {
  path: PathBuf,
  evidence: SanitizerLibraryEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SanitizerLibraryEvidence {
  Hwasan,
  Asan,
  Unsanitized,
}

impl SanitizerLibraryEvidence {
  fn from_readelf_output(output: &str) -> Self {
    if output.contains("libclang_rt.hwasan.so") {
      Self::Hwasan
    } else if output.contains("libclang_rt.asan.so") {
      Self::Asan
    } else {
      Self::Unsanitized
    }
  }
}

impl fmt::Display for SanitizerLibraryEvidence {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Hwasan => write!(f, "HWASAN"),
      Self::Asan => write!(f, "ASAN"),
      Self::Unsanitized => write!(f, "missing sanitizer runtime dependency"),
    }
  }
}

struct NativeHwasanValidator<'a> {
  read_elf: &'a Path,
}

impl<'a> NativeHwasanValidator<'a> {
  fn new(read_elf: &'a Path) -> Self {
    Self { read_elf }
  }

  fn validate(&self, libs: &[PathBuf]) -> NativeBuildResult<()> {
    if !self.read_elf.is_file() {
      return Err(NativeBuildError::ReadElfFailed {
        path: self.read_elf.to_path_buf(),
        message: "llvm-readelf path does not exist".to_string(),
      });
    }

    let mut invalid = Vec::new();
    for lib in libs {
      if self.is_sanitizer_exempt(lib) {
        continue;
      }
      let evidence = self.inspect(lib)?;
      if evidence != SanitizerLibraryEvidence::Hwasan {
        invalid.push(InvalidSanitizerNativeLib {
          path: lib.clone(),
          evidence,
        });
      }
    }
    if invalid.is_empty() {
      Ok(())
    } else {
      Err(NativeBuildError::HwasanDisabled { libs: invalid })
    }
  }

  fn inspect(&self, lib: &Path) -> NativeBuildResult<SanitizerLibraryEvidence> {
    let output = Command::new(self.read_elf)
      .arg("-d")
      .arg(lib)
      .output()
      .map_err(|e| NativeBuildError::ReadElfFailed {
        path: lib.to_path_buf(),
        message: e.to_string(),
      })?;
    if !output.status.success() {
      return Err(NativeBuildError::ReadElfFailed {
        path: lib.to_path_buf(),
        message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
      });
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(SanitizerLibraryEvidence::from_readelf_output(&stdout))
  }

  fn is_sanitizer_exempt(&self, lib: &Path) -> bool {
    matches!(
      lib.file_name().and_then(|name| name.to_str()),
      Some("libc++_shared.so") | Some("libc++.so")
    )
  }
}

struct NativeModuleLayout<'a> {
  module_root: &'a Path,
}

impl<'a> NativeModuleLayout<'a> {
  fn new(module_root: &'a Path) -> Self {
    Self { module_root }
  }

  fn cpp_source_dir(&self) -> PathBuf {
    self.module_root.join("src").join("main").join("cpp")
  }

  fn cmake_lists(&self) -> PathBuf {
    self.cpp_source_dir().join("CMakeLists.txt")
  }

  fn top_level_libs(&self) -> PathBuf {
    self.module_root.join(HVIGOR_LIBS_DIR)
  }

  fn build_default(&self) -> PathBuf {
    self.module_root.join("build").join("default")
  }

  fn cmake_intermediate_dir(&self) -> PathBuf {
    self
      .build_default()
      .join("intermediates")
      .join("cmake")
      .join("default")
  }

  fn cmake_obj_dir(&self) -> PathBuf {
    self.cmake_intermediate_dir().join("obj")
  }

  fn cangjie_intermediate_dir(&self) -> PathBuf {
    self
      .build_default()
      .join("intermediates")
      .join("cj")
      .join("extend_libs")
      .join("default")
  }

  fn so_search_roots(&self) -> Vec<PathBuf> {
    vec![
      self.cmake_obj_dir(),
      self.cangjie_intermediate_dir(),
      self.top_level_libs(),
    ]
  }

  fn has_native_code(&self) -> bool {
    if self.cmake_lists().is_file() {
      return true;
    }
    if NativeLibraryRoot::new(&self.top_level_libs()).contains_known_abi_library() {
      return true;
    }
    false
  }
}

struct NativeLibraryRoot<'a> {
  libs_dir: &'a Path,
}

impl<'a> NativeLibraryRoot<'a> {
  fn new(libs_dir: &'a Path) -> Self {
    Self { libs_dir }
  }

  fn contains_known_abi_library(&self) -> bool {
    if !self.libs_dir.is_dir() {
      return false;
    }
    for abi in KNOWN_ABI_DIRS {
      let abi_dir = self.libs_dir.join(abi);
      if !abi_dir.is_dir() {
        continue;
      }
      for entry in walkdir::WalkDir::new(&abi_dir)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
      {
        if HvigorNativeLibraryPattern::matches_path_under(&abi_dir, entry.path()) {
          return true;
        }
      }
    }
    false
  }
}

struct NativeAbiFilters<'a> {
  filters: &'a [String],
}

impl<'a> NativeAbiFilters<'a> {
  fn new(filters: &'a [String]) -> Self {
    Self { filters }
  }

  fn build_abis(&self) -> Vec<String> {
    if self.filters.is_empty() {
      vec!["arm64-v8a".to_string()]
    } else {
      self.filters.to_vec()
    }
  }

  fn package_abis(&self) -> Vec<String> {
    if self.filters.is_empty() {
      KNOWN_ABI_DIRS
        .iter()
        .map(|abi| (*abi).to_string())
        .collect()
    } else {
      self.filters.to_vec()
    }
  }

  fn matches_path(&self, path: &Path) -> bool {
    let filters = self.package_abis();
    path.components().any(|component| {
      let component = component.as_os_str();
      filters
        .iter()
        .any(|filter| component == OsStr::new(filter.as_str()))
    })
  }
}

pub struct NativeLibCollector<'a> {
  module: &'a ModuleConfig,
  abi_filters: &'a [String],
  deps: &'a DependencyGraphSnapshot,
  policy: NativeLibCollectionPolicy,
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
      policy: NativeLibCollectionPolicy::default(),
    }
  }

  pub fn with_policy(mut self, policy: NativeLibCollectionPolicy) -> Self {
    self.policy = policy;
    self
  }

  pub fn collect(&self) -> NativeBuildResult<Vec<PathBuf>> {
    let mut candidates = Vec::new();
    let mut seen = FxHashSet::default();

    let native_cfg = NativeBuildConfig {
      module_root: self.module.src_path.clone(),
      abi_filters: self.abi_filters.to_vec(),
      enable_cmake_build: self.policy.enable_cmake_build(),
      sanitizer: self.policy.sanitizer,
      build_mode: self.policy.build_mode.clone(),
      sdk_toolchain: self.policy.sdk_toolchain.clone(),
      collect_all_libs: self.policy.native_lib.collect_all_libs,
    };
    if self.policy.native_lib.filter.has_order_sensitive_rules() {
      return Err(NativeBuildError::UnsupportedNativeLibOrderSensitiveFilter);
    }
    if NativeModuleLayout::new(&self.module.src_path).has_native_code() {
      tracing::debug!(module = %self.module.src_path.display(), "native: scanning for pre-built .so files");
    }
    let native_out = native_cfg.run()?;
    for warning in &native_out.warnings {
      tracing::warn!("{}", warning);
    }
    let self_package = self.self_package_info();
    for (_, so_path) in native_out.libs_by_abi {
      let canonical = std::fs::canonicalize(&so_path).unwrap_or(so_path.clone());
      if seen.insert(canonical) {
        candidates.push(NativeLibCandidate::from_collected_path(
          so_path,
          self_package.clone(),
        )?);
      }
    }

    for dep_libs_dir in self.dependency_default_lib_dirs() {
      self.collect_from_libs_dir(&dep_libs_dir, &mut seen, &mut candidates)?;
    }
    candidates.sort_by(|left, right| left.path.cmp(&right.path));
    NativeLibFilterApplier::new(&self.policy.native_lib.filter).apply(&mut candidates);

    let result_libs =
      NativeLibSelectionResolver::new(&self.module.name, &self.policy.native_lib.filter)
        .resolve(candidates)?;
    self.policy.validate_packaged_libs(&result_libs)?;
    Ok(result_libs)
  }

  fn self_package_info(&self) -> NativeLibPackageInfo {
    self
      .policy
      .self_package
      .clone()
      .unwrap_or_else(|| NativeLibPackageInfo::local(self.module.name.clone(), String::new()))
  }

  fn native_lib_identity(lib: &Path) -> String {
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

  fn is_duplicate_check_exempt(identity: &str) -> bool {
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

  fn dependency_default_lib_dirs(&self) -> Vec<NativeDependencyLibDir> {
    let mut dirs = Vec::new();
    let mut seen = FxHashSet::default();
    let mut push_default_libs_dir = |root: &Path, package: NativeLibPackageInfo| {
      if seen.insert(root.to_path_buf()) {
        dirs.push(NativeDependencyLibDir {
          libs_dir: root.join(HVIGOR_LIBS_DIR),
          package,
        });
      }
    };
    for node in self.deps.nodes.values() {
      let Some(root) = node.package_root.as_deref() else {
        continue;
      };
      push_default_libs_dir(root, NativeLibPackageInfo::dependency(node));
    }
    dirs
  }

  fn collect_lib_names_from_libs_dir(
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

  fn collect_from_libs_dir(
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

struct NativeDependencyLibDir {
  libs_dir: PathBuf,
  package: NativeLibPackageInfo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeLibSelectionState {
  None,
  Include,
  Exclude,
}

#[derive(Debug, Clone)]
struct NativeLibCandidate {
  path: PathBuf,
  relative_path: String,
  name: String,
  package: NativeLibPackageInfo,
  selected: NativeLibSelectionState,
}

impl NativeLibCandidate {
  fn new(path: PathBuf, relative_path: String, package: NativeLibPackageInfo) -> Self {
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

  fn from_collected_path(path: PathBuf, package: NativeLibPackageInfo) -> NativeBuildResult<Self> {
    let relative_path = NativeLibCollector::native_lib_identity(&path);
    if relative_path.is_empty() {
      return Err(NativeBuildError::NativeLibRelativePath {
        path,
        message: "missing file name".to_string(),
      });
    }
    Ok(Self::new(path, relative_path, package))
  }

  fn relative_path(base: &Path, path: &Path) -> NativeBuildResult<String> {
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

  fn matches_package_selection(&self, selection: &NativeLibPackageSelection) -> bool {
    self.package.name == selection.package
      && selection
        .version
        .as_ref()
        .map(|version| self.package.version == *version)
        .unwrap_or(true)
  }

  fn relative_path_as_path(&self) -> &Path {
    Path::new(&self.relative_path)
  }
}

struct NativeLibFilterApplier<'a> {
  filter: &'a NativeLibFilterOption,
}

impl<'a> NativeLibFilterApplier<'a> {
  fn new(filter: &'a NativeLibFilterOption) -> Self {
    Self { filter }
  }

  fn apply(&self, candidates: &mut [NativeLibCandidate]) {
    for selection in &self.filter.select {
      for index in self.filter_libs(candidates, selection, true) {
        candidates[index].selected = NativeLibSelectionState::Include;
      }
      for index in self.filter_libs(candidates, selection, false) {
        candidates[index].selected = NativeLibSelectionState::Exclude;
      }
    }
  }

  fn filter_libs(
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

struct NativeLibSelectionResolver<'a> {
  module_name: &'a str,
  filter: &'a NativeLibFilterOption,
}

impl<'a> NativeLibSelectionResolver<'a> {
  fn new(module_name: &'a str, filter: &'a NativeLibFilterOption) -> Self {
    Self {
      module_name,
      filter,
    }
  }

  fn resolve(&self, candidates: Vec<NativeLibCandidate>) -> NativeBuildResult<Vec<PathBuf>> {
    let mut groups = std::collections::BTreeMap::<String, Vec<PathBuf>>::new();
    let mut selected_relative_paths = FxHashSet::default();
    for candidate in candidates {
      if matches!(candidate.selected, NativeLibSelectionState::Exclude) {
        continue;
      }
      if selected_relative_paths.contains(candidate.relative_path.as_str()) {
        continue;
      }
      if matches!(candidate.selected, NativeLibSelectionState::Include) {
        groups.insert(candidate.relative_path.clone(), vec![candidate.path]);
        selected_relative_paths.insert(candidate.relative_path);
        continue;
      }
      if self.is_excluded(&candidate) {
        continue;
      }
      groups
        .entry(candidate.relative_path.clone())
        .or_default()
        .push(candidate.path);
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

  fn is_excluded(&self, candidate: &NativeLibCandidate) -> bool {
    self.filter_excludes().iter().any(|pattern| {
      HvigorNativeLibraryPattern::matches_filter_pattern(
        candidate.relative_path_as_path(),
        pattern,
        false,
      )
    })
  }

  fn filter_excludes(&self) -> &[String] {
    &self.filter.excludes
  }
}

struct NativeDuplicateGroup<'a> {
  libs: &'a [PathBuf],
}

impl<'a> NativeDuplicateGroup<'a> {
  fn new(libs: &'a [PathBuf]) -> Self {
    Self { libs }
  }

  fn unique_files(&self) -> NativeBuildResult<Vec<PathBuf>> {
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

struct NativeDuplicateFile<'a> {
  path: &'a Path,
}

impl<'a> NativeDuplicateFile<'a> {
  fn new(path: &'a Path) -> Self {
    Self { path }
  }

  fn is_same_file(&self, other: &Path) -> NativeBuildResult<bool> {
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

  fn canonical_equals(&self, other: &Path) -> bool {
    let Ok(left) = std::fs::canonicalize(self.path) else {
      return false;
    };
    let Ok(right) = std::fs::canonicalize(other) else {
      return false;
    };
    left == right
  }

  fn content_equals(&self, other: &Path) -> NativeBuildResult<bool> {
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
  /// Scan and optionally build native libraries for this module.
  ///
  /// Returns all native shared libraries that should be included in the HAP/HSP artifact.
  ///
  /// ## Algorithm
  ///
  /// 1. Check for `CMakeLists.txt` in `src/main/cpp/`.  If present and
  ///    `enable_cmake_build` is true, attempt a CMake build.
  /// 2. Collect pre-built shared libraries from hvigor ProcessLibs inputs
  ///    (CMake output, Cangjie output, and module-root `libs/`).
  /// 3. Return all discovered shared libraries whose ABI matches `abi_filters`.
  pub fn run(&self) -> NativeBuildResult<NativeBuildOutput> {
    let mut output = NativeBuildOutput::default();
    let layout = NativeModuleLayout::new(&self.module_root);
    let abi_filters = NativeAbiFilters::new(&self.abi_filters).package_abis();

    // ── Step 1: detect CMakeLists.txt ──────────────────────────────────
    let cmake_file = layout.cmake_lists();
    let has_cmake = cmake_file.is_file();

    if has_cmake && self.enable_cmake_build {
      match self.run_cmake_build() {
        Ok(()) => {
          tracing::info!(
              module = %self.module_root.display(),
              "native: CMake build succeeded"
          );
        }
        Err(e) => {
          return Err(NativeBuildError::CmakeFailed {
            module_root: self.module_root.clone(),
            message: e,
          });
        }
      }
    } else if has_cmake && !self.enable_cmake_build {
      output.warnings.push(format!(
        "native: CMakeLists.txt found in '{}' but --enable-cmake-build is not set. \
                 Run `hvigorw assembleNative` first, or set enable_cmake_build=true. \
                 Collecting pre-built .so files only.",
        self.module_root.display()
      ));
    }

    // ── Step 2: collect pre-built native shared libraries ──────────────
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

    if output.libs_by_abi.is_empty() && has_cmake {
      output.warnings.push(format!(
        "native: no shared libraries found for module '{}' after build/import scan. \
                 Expected paths: {:?}",
        self.module_root.display(),
        search_roots
          .iter()
          .map(|p| p.display().to_string())
          .collect::<Vec<_>>()
      ));
    }

    Ok(output)
  }

  /// Directories to scan for pre-built `.so` files, in priority order.
  fn so_search_roots(&self) -> Vec<PathBuf> {
    NativeModuleLayout::new(&self.module_root).so_search_roots()
  }

  /// Attempt a CMake build using the OHOS NDK toolchain.
  ///
  /// Uses the CMake, Ninja, and native toolchain paths resolved from the
  /// active SDK snapshot, matching hvigor's `sdkInfo` accessors.
  fn run_cmake_build(&self) -> Result<(), String> {
    let toolchain = self.sdk_toolchain.require()?;
    let layout = NativeModuleLayout::new(&self.module_root);
    let source_dir = layout.cpp_source_dir();
    for abi in NativeAbiFilters::new(&self.abi_filters).build_abis() {
      let build_dir = layout.cmake_intermediate_dir().join(&abi);
      let output_dir = layout.cmake_obj_dir().join(&abi);
      std::fs::create_dir_all(&build_dir)
        .map_err(|e| format!("cannot create CMake build dir: {e}"))?;
      std::fs::create_dir_all(&output_dir)
        .map_err(|e| format!("cannot create CMake output dir: {e}"))?;

      let mut configure_cmd = std::process::Command::new(toolchain.cmake);
      configure_cmd
        .arg(format!("-H{}", source_dir.display()))
        .arg(format!("-B{}", build_dir.display()))
        .arg(format!("-DOHOS_ARCH={abi}"))
        .arg(format!(
          "-DCMAKE_LIBRARY_OUTPUT_DIRECTORY={}",
          output_dir.display()
        ))
        .arg(format!("-DCMAKE_BUILD_TYPE={}", self.cmake_build_type()))
        .arg(format!(
          "-DOHOS_SDK_NATIVE={}",
          toolchain.native_dir.display()
        ))
        .arg("-DCMAKE_SYSTEM_NAME=OHOS")
        .arg(format!("-DCMAKE_OHOS_ARCH_ABI={abi}"))
        .arg("-DCMAKE_EXPORT_COMPILE_COMMANDS=ON")
        .arg(format!(
          "-DCMAKE_TOOLCHAIN_FILE={}",
          toolchain.toolchain_file.display()
        ))
        .arg("-GNinja")
        .arg(format!(
          "-DCMAKE_MAKE_PROGRAM={}",
          toolchain.ninja.display()
        ))
        .arg("--no-warn-unused-cli");
      if let Some(arg) = self.sanitizer.cmake_arg_for_abi(&abi) {
        configure_cmd.arg(arg);
      }

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

      let mut build_cmd = std::process::Command::new(toolchain.cmake);
      build_cmd
        .arg("--build")
        .arg(&build_dir)
        .arg("--")
        .arg(format!(
          "-j{}",
          std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
        ));

      tracing::info!(
          abi = %abi,
          cmd = ?build_cmd,
          "native: running CMake build"
      );

      let build_status = build_cmd
        .status()
        .map_err(|e| format!("failed to run cmake build: {e}"))?;

      if !build_status.success() {
        return Err(format!(
          "cmake build for ABI {abi} exited with status {build_status}"
        ));
      }
    }

    Ok(())
  }

  fn cmake_build_type(&self) -> &'static str {
    match self.build_mode.as_str() {
      "debug" | "Debug" => "Debug",
      "release" | "Release" => "Release",
      _ => "Debug",
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::fs;

  fn add_dependency_node(
    deps: &mut DependencyGraphSnapshot,
    alias: &str,
    package_name: &str,
    version: &str,
    root: PathBuf,
  ) {
    deps.roots_by_alias.insert(alias.to_string(), root.clone());
    deps.nodes.insert(
      package_name.to_string(),
      DependencyNodeSnapshot {
        alias: alias.to_string(),
        package_name: package_name.to_string(),
        version_spec: Some(version.to_string()),
        package_root: Some(root),
        package: None,
        node_type: arkdown_session::dependency_graph::snapshot::DependencyNodeType::SourceHar,
        is_runtime_only: false,
        is_dynamic: false,
        is_transitive: false,
        direct_deps: Vec::new(),
      },
    );
  }

  #[test]
  fn test_module_has_native_code_detects_cmakelists() {
    let tmp = tempfile::tempdir().unwrap();
    let cpp_dir = tmp.path().join("src/main/cpp");
    fs::create_dir_all(&cpp_dir).unwrap();
    fs::write(
      cpp_dir.join("CMakeLists.txt"),
      "cmake_minimum_required(VERSION 3.16)",
    )
    .unwrap();
    assert!(NativeModuleLayout::new(tmp.path()).has_native_code());
  }

  #[test]
  fn test_module_has_native_code_detects_prebuilt_so() {
    let tmp = tempfile::tempdir().unwrap();
    let libs_dir = tmp.path().join("libs/arm64-v8a");
    fs::create_dir_all(&libs_dir).unwrap();
    fs::write(libs_dir.join("libfoo.so"), b"ELF\x7f").unwrap();
    assert!(NativeModuleLayout::new(tmp.path()).has_native_code());
  }

  #[test]
  fn test_module_has_native_code_detects_versioned_shared_library() {
    let tmp = tempfile::tempdir().unwrap();
    let libs_dir = tmp.path().join("libs/arm64-v8a");
    fs::create_dir_all(&libs_dir).unwrap();
    fs::write(libs_dir.join("libexpat.so.1"), b"ELF\x7f").unwrap();
    assert!(NativeModuleLayout::new(tmp.path()).has_native_code());
  }

  #[test]
  fn test_module_without_native_code_returns_false() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(!NativeModuleLayout::new(tmp.path()).has_native_code());
  }

  #[test]
  fn test_native_import_collects_prebuilt_sos() {
    let tmp = tempfile::tempdir().unwrap();
    let libs_dir = tmp.path().join("libs/arm64-v8a");
    fs::create_dir_all(&libs_dir).unwrap();
    fs::write(libs_dir.join("libnative.so"), b"ELF").unwrap();

    let cfg = NativeBuildConfig {
      module_root: tmp.path().to_path_buf(),
      abi_filters: vec!["arm64-v8a".to_string()],
      enable_cmake_build: false,
      sanitizer: NativeBuildSanitizer::None,
      build_mode: "debug".to_string(),
      sdk_toolchain: NativeSdkToolchain::default(),
      collect_all_libs: false,
    };
    let out = cfg.run().expect("native scan");
    assert_eq!(out.libs_by_abi.len(), 1);
    assert_eq!(out.libs_by_abi[0].0, "arm64-v8a");
    assert!(out.libs_by_abi[0].1.file_name().unwrap() == "libnative.so");
  }

  #[test]
  fn test_native_import_collects_versioned_shared_libraries() {
    let tmp = tempfile::tempdir().unwrap();
    let libs_dir = tmp.path().join("libs/arm64-v8a");
    fs::create_dir_all(&libs_dir).unwrap();
    fs::write(libs_dir.join("libexpat.so.1"), b"ELF").unwrap();

    let cfg = NativeBuildConfig {
      module_root: tmp.path().to_path_buf(),
      abi_filters: vec!["arm64-v8a".to_string()],
      enable_cmake_build: false,
      sanitizer: NativeBuildSanitizer::None,
      build_mode: "debug".to_string(),
      sdk_toolchain: NativeSdkToolchain::default(),
      collect_all_libs: false,
    };
    let out = cfg.run().expect("native scan");
    assert_eq!(out.libs_by_abi.len(), 1);
    assert_eq!(out.libs_by_abi[0].0, "arm64-v8a");
    assert_eq!(out.libs_by_abi[0].1.file_name().unwrap(), "libexpat.so.1");
  }

  #[test]
  fn test_native_import_collects_hvigor_process_libs_output_dirs() {
    let tmp = tempfile::tempdir().unwrap();
    let cmake_dir = tmp
      .path()
      .join("build/default/intermediates/cmake/default/obj/arm64-v8a");
    let cangjie_dir = tmp
      .path()
      .join("build/default/intermediates/cj/extend_libs/default/arm64-v8a");
    fs::create_dir_all(&cmake_dir).unwrap();
    fs::create_dir_all(&cangjie_dir).unwrap();
    fs::write(cmake_dir.join("libcmake.so"), b"ELF cmake").unwrap();
    fs::write(cangjie_dir.join("libcj.so"), b"ELF cangjie").unwrap();

    let cfg = NativeBuildConfig {
      module_root: tmp.path().to_path_buf(),
      abi_filters: vec!["arm64-v8a".to_string()],
      enable_cmake_build: false,
      sanitizer: NativeBuildSanitizer::None,
      build_mode: "debug".to_string(),
      sdk_toolchain: NativeSdkToolchain::default(),
      collect_all_libs: false,
    };
    let out = cfg.run().expect("native scan");
    let names = out
      .libs_by_abi
      .iter()
      .filter_map(|(_, path)| path.file_name().and_then(|name| name.to_str()))
      .collect::<Vec<_>>();
    assert_eq!(names, vec!["libcmake.so", "libcj.so"]);
  }

  #[test]
  fn test_native_import_skips_macos_appledouble_shared_libraries() {
    let tmp = tempfile::tempdir().unwrap();
    let libs_dir = tmp.path().join("libs/arm64-v8a");
    fs::create_dir_all(&libs_dir).unwrap();
    fs::write(libs_dir.join("libnative.so"), b"ELF").unwrap();
    fs::write(libs_dir.join("._libnative.so"), b"AppleDouble").unwrap();

    let cfg = NativeBuildConfig {
      module_root: tmp.path().to_path_buf(),
      abi_filters: vec!["arm64-v8a".to_string()],
      enable_cmake_build: false,
      sanitizer: NativeBuildSanitizer::None,
      build_mode: "debug".to_string(),
      sdk_toolchain: NativeSdkToolchain::default(),
      collect_all_libs: false,
    };
    let out = cfg.run().expect("native scan");
    assert_eq!(out.libs_by_abi.len(), 1);
    assert_eq!(out.libs_by_abi[0].1.file_name().unwrap(), "libnative.so");
  }

  #[test]
  fn test_native_import_ignores_src_main_libs_not_used_by_hvigor_process_libs() {
    let tmp = tempfile::tempdir().unwrap();
    let libs_dir = tmp.path().join("src/main/libs/arm64-v8a");
    fs::create_dir_all(&libs_dir).unwrap();
    fs::write(libs_dir.join("libignored.so"), b"ELF").unwrap();

    let cfg = NativeBuildConfig {
      module_root: tmp.path().to_path_buf(),
      abi_filters: vec!["arm64-v8a".to_string()],
      enable_cmake_build: false,
      sanitizer: NativeBuildSanitizer::None,
      build_mode: "debug".to_string(),
      sdk_toolchain: NativeSdkToolchain::default(),
      collect_all_libs: false,
    };
    let out = cfg.run().expect("native scan");
    assert!(out.libs_by_abi.is_empty());
  }

  #[test]
  fn test_asan_cmake_arg_applies_to_all_abis() {
    assert_eq!(
      NativeBuildSanitizer::Asan.cmake_arg_for_abi("arm64-v8a"),
      Some("-DOHOS_ENABLE_ASAN=ON")
    );
    assert_eq!(
      NativeBuildSanitizer::Asan.cmake_arg_for_abi("x86_64"),
      Some("-DOHOS_ENABLE_ASAN=ON")
    );
  }

  #[test]
  fn test_hwasan_cmake_arg_only_applies_to_arm64() {
    assert_eq!(
      NativeBuildSanitizer::Hwasan.cmake_arg_for_abi("arm64-v8a"),
      Some("-DOHOS_ENABLE_HWASAN=ON")
    );
    assert_eq!(
      NativeBuildSanitizer::Hwasan.cmake_arg_for_abi("x86_64"),
      None
    );
  }

  #[test]
  fn test_hwasan_readelf_output_classification() {
    assert_eq!(
      SanitizerLibraryEvidence::from_readelf_output("NEEDED libclang_rt.hwasan.so"),
      SanitizerLibraryEvidence::Hwasan
    );
    assert_eq!(
      SanitizerLibraryEvidence::from_readelf_output("NEEDED libclang_rt.asan.so"),
      SanitizerLibraryEvidence::Asan
    );
    assert_eq!(
      SanitizerLibraryEvidence::from_readelf_output("NEEDED libc++_shared.so"),
      SanitizerLibraryEvidence::Unsanitized
    );
  }

  #[test]
  fn test_build_abi_filters_defaults_to_arm64() {
    let filtered = NativeAbiFilters::new(&[]).build_abis();
    assert_eq!(filtered, vec!["arm64-v8a"]);
  }

  #[test]
  fn test_package_abi_filters_default_to_known_abis() {
    let filtered = NativeAbiFilters::new(&[]).package_abis();
    assert_eq!(
      filtered,
      KNOWN_ABI_DIRS
        .iter()
        .map(|abi| (*abi).to_string())
        .collect::<Vec<_>>()
    );
  }

  #[test]
  fn test_abi_filters_preserve_explicit() {
    let filtered =
      NativeAbiFilters::new(&["armeabi-v7a".to_string(), "x86_64".to_string()]).package_abis();
    assert_eq!(filtered, vec!["armeabi-v7a", "x86_64"]);
  }

  #[test]
  fn test_native_import_keeps_same_library_name_across_abis() {
    let tmp = tempfile::tempdir().unwrap();
    let arm64_dir = tmp.path().join("libs/arm64-v8a");
    let x64_dir = tmp.path().join("libs/x86_64");
    fs::create_dir_all(&arm64_dir).unwrap();
    fs::create_dir_all(&x64_dir).unwrap();
    fs::write(arm64_dir.join("libnative.so"), b"ELF arm64").unwrap();
    fs::write(x64_dir.join("libnative.so"), b"ELF x64").unwrap();

    let cfg = NativeBuildConfig {
      module_root: tmp.path().to_path_buf(),
      abi_filters: vec![],
      enable_cmake_build: false,
      sanitizer: NativeBuildSanitizer::None,
      build_mode: "debug".to_string(),
      sdk_toolchain: NativeSdkToolchain::default(),
      collect_all_libs: false,
    };
    let out = cfg.run().expect("native scan");
    assert_eq!(out.libs_by_abi.len(), 2);

    let module = ModuleConfig {
      name: "entry".to_string(),
      src_path: tmp.path().to_path_buf(),
      module_type: arkdown_core::OhosModuleType::Entry,
    };
    let deps = DependencyGraphSnapshot::default();
    let libs = NativeLibCollector::new(&module, &[], &deps)
      .collect()
      .expect("collect native libs");
    assert_eq!(libs.len(), 2);
  }

  #[test]
  fn test_native_collector_collects_versioned_dependency_shared_library() {
    let tmp = tempfile::tempdir().unwrap();
    let module_root = tmp.path().join("entry");
    let dep_root = tmp.path().join("oh_modules/@meituan/mtflexbox");
    let libs_dir = dep_root.join("libs/arm64-v8a");
    fs::create_dir_all(&module_root).unwrap();
    fs::create_dir_all(&libs_dir).unwrap();
    fs::write(libs_dir.join("libexpat.so.1"), b"ELF").unwrap();

    let module = ModuleConfig {
      name: "entry".to_string(),
      src_path: module_root,
      module_type: arkdown_core::OhosModuleType::Entry,
    };
    let mut deps = DependencyGraphSnapshot::default();
    add_dependency_node(
      &mut deps,
      "@meituan/mtflexbox",
      "@meituan/mtflexbox",
      "1.0.0",
      dep_root,
    );

    let libs = NativeLibCollector::new(&module, &[], &deps)
      .collect()
      .expect("collect native libs");
    assert_eq!(libs.len(), 1);
    assert_eq!(libs[0].file_name().unwrap(), "libexpat.so.1");
  }

  #[test]
  fn test_native_collector_scans_dependency_get_default_libs_dir_like_hvigor() {
    let tmp = tempfile::tempdir().unwrap();
    let module_root = tmp.path().join("entry");
    let dep_root = tmp.path().join("oh_modules/@scope/native");
    let hvigor_default_libs = dep_root.join("libs/arm64-v8a");
    let non_default_lib = dep_root.join("lib/arm64-v8a");
    fs::create_dir_all(&module_root).unwrap();
    fs::create_dir_all(&hvigor_default_libs).unwrap();
    fs::create_dir_all(&non_default_lib).unwrap();
    fs::write(hvigor_default_libs.join("libkept.so"), b"ELF kept").unwrap();
    fs::write(non_default_lib.join("libignored.so"), b"ELF ignored").unwrap();

    let module = ModuleConfig {
      name: "entry".to_string(),
      src_path: module_root,
      module_type: arkdown_core::OhosModuleType::Entry,
    };
    let mut deps = DependencyGraphSnapshot::default();
    add_dependency_node(
      &mut deps,
      "@scope/native",
      "@scope/native",
      "1.0.0",
      dep_root,
    );

    let libs = NativeLibCollector::new(&module, &[], &deps)
      .collect()
      .expect("dependency native libs must be collected from getDefaultLibsDir");
    assert_eq!(libs.len(), 1);
    assert_eq!(libs[0].file_name().unwrap(), "libkept.so");
  }

  #[test]
  fn test_native_collector_collect_all_libs_uses_hvigor_all_file_glob() {
    let tmp = tempfile::tempdir().unwrap();
    let module_root = tmp.path().join("entry");
    let libs_dir = module_root.join("libs/arm64-v8a");
    fs::create_dir_all(&libs_dir).unwrap();
    fs::write(libs_dir.join("libnative.sym"), b"symbol").unwrap();

    let module = ModuleConfig {
      name: "entry".to_string(),
      src_path: module_root,
      module_type: arkdown_core::OhosModuleType::Entry,
    };
    let deps = DependencyGraphSnapshot::default();
    let libs = NativeLibCollector::new(&module, &[], &deps)
      .with_policy(
        NativeLibCollectionPolicy::default().with_native_lib_option(NativeLibOption {
          collect_all_libs: true,
          filter: NativeLibFilterOption::default(),
        }),
      )
      .collect()
      .expect("collectAllLibs must collect non-.so files under libs like hvigor **/*");
    assert_eq!(libs.len(), 1);
    assert_eq!(libs[0].file_name().unwrap(), "libnative.sym");
  }

  #[test]
  fn test_native_collector_applies_hvigor_excludes_filter() {
    let tmp = tempfile::tempdir().unwrap();
    let module_root = tmp.path().join("entry");
    let libs_dir = module_root.join("libs/arm64-v8a");
    fs::create_dir_all(&libs_dir).unwrap();
    fs::write(libs_dir.join("libkept.so"), b"ELF kept").unwrap();
    fs::write(libs_dir.join("libexcluded.so"), b"ELF excluded").unwrap();
    fs::create_dir_all(&module_root).unwrap();

    let module = ModuleConfig {
      name: "entry".to_string(),
      src_path: module_root,
      module_type: arkdown_core::OhosModuleType::Entry,
    };
    let deps = DependencyGraphSnapshot::default();
    let libs = NativeLibCollector::new(&module, &[], &deps)
      .with_policy(
        NativeLibCollectionPolicy::default().with_native_lib_option(NativeLibOption {
          collect_all_libs: false,
          filter: NativeLibFilterOption {
            excludes: vec!["**/libexcluded.so".to_string()],
            ..Default::default()
          },
        }),
      )
      .collect()
      .expect("nativeLib.filter.excludes must be applied like hvigor ProcessLibs");
    assert_eq!(libs.len(), 1);
    assert_eq!(libs[0].file_name().unwrap(), "libkept.so");
  }

  #[test]
  fn test_native_collector_applies_hvigor_select_include_and_exclude() {
    let tmp = tempfile::tempdir().unwrap();
    let module_root = tmp.path().join("entry");
    let dep_root = tmp.path().join("oh_modules/native_pkg");
    fs::create_dir_all(dep_root.join("libs/arm64-v8a")).unwrap();
    fs::create_dir_all(&module_root).unwrap();
    fs::write(
      dep_root.join("libs/arm64-v8a/libinclude.so"),
      b"ELF include",
    )
    .unwrap();
    fs::write(
      dep_root.join("libs/arm64-v8a/libexclude.so"),
      b"ELF exclude",
    )
    .unwrap();

    let module = ModuleConfig {
      name: "entry".to_string(),
      src_path: module_root,
      module_type: arkdown_core::OhosModuleType::Entry,
    };
    let mut deps = DependencyGraphSnapshot::default();
    add_dependency_node(&mut deps, "native_pkg", "native_pkg", "1.2.3", dep_root);

    let libs = NativeLibCollector::new(&module, &[], &deps)
      .with_policy(
        NativeLibCollectionPolicy::default().with_native_lib_option(NativeLibOption {
          collect_all_libs: false,
          filter: NativeLibFilterOption {
            select: vec![NativeLibPackageSelection {
              package: "native_pkg".to_string(),
              version: Some("1.2.3".to_string()),
              include: Some(vec!["libinclude.so".to_string()]),
              exclude: Some(vec!["libexclude.so".to_string()]),
              ..Default::default()
            }],
            ..Default::default()
          },
        }),
      )
      .collect()
      .expect("nativeLib.filter.select must be applied like hvigor ProcessLibs");
    assert_eq!(libs.len(), 1);
    assert_eq!(libs[0].file_name().unwrap(), "libinclude.so");
  }

  #[test]
  fn test_native_collector_rejects_order_sensitive_filter_without_hvigor_dependency_order() {
    let tmp = tempfile::tempdir().unwrap();
    let module_root = tmp.path().join("entry");
    fs::create_dir_all(&module_root).unwrap();

    let module = ModuleConfig {
      name: "entry".to_string(),
      src_path: module_root,
      module_type: arkdown_core::OhosModuleType::Entry,
    };
    let deps = DependencyGraphSnapshot::default();
    let err = NativeLibCollector::new(&module, &[], &deps)
      .with_policy(
        NativeLibCollectionPolicy::default().with_native_lib_option(NativeLibOption {
          collect_all_libs: false,
          filter: NativeLibFilterOption {
            pick_firsts: vec!["**/libdup.so".to_string()],
            ..Default::default()
          },
        }),
      )
      .collect()
      .expect_err("order-sensitive nativeLib.filter rules require hvigor dependency order");
    assert!(
      matches!(
        err,
        NativeBuildError::UnsupportedNativeLibOrderSensitiveFilter
      ),
      "unexpected error: {err}"
    );
  }

  #[test]
  fn test_native_collector_rejects_conflicting_duplicate_relative_libs_like_hvigor() {
    let tmp = tempfile::tempdir().unwrap();
    let module_root = tmp.path().join("entry");
    let dep_a = tmp.path().join("oh_modules/a");
    let dep_b = tmp.path().join("oh_modules/b");
    fs::create_dir_all(dep_a.join("libs/arm64-v8a")).unwrap();
    fs::create_dir_all(dep_b.join("libs/arm64-v8a")).unwrap();
    fs::create_dir_all(&module_root).unwrap();
    fs::write(dep_a.join("libs/arm64-v8a/libdup.so"), b"ELF A").unwrap();
    fs::write(dep_b.join("libs/arm64-v8a/libdup.so"), b"ELF B").unwrap();

    let module = ModuleConfig {
      name: "entry".to_string(),
      src_path: module_root,
      module_type: arkdown_core::OhosModuleType::Entry,
    };
    let mut deps = DependencyGraphSnapshot::default();
    add_dependency_node(&mut deps, "a", "a", "1.0.0", dep_a);
    add_dependency_node(&mut deps, "b", "b", "1.0.0", dep_b);

    let err = NativeLibCollector::new(&module, &[], &deps)
      .collect()
      .expect_err("conflicting duplicate native libs must fail like hvigor ProcessLibs");
    assert!(
      matches!(err, NativeBuildError::DuplicatedFilesFoundInLibPath { .. }),
      "unexpected error: {err}"
    );
  }

  #[test]
  fn test_native_collector_collapses_same_content_duplicate_relative_libs_like_hvigor() {
    let tmp = tempfile::tempdir().unwrap();
    let module_root = tmp.path().join("entry");
    let dep_a = tmp.path().join("oh_modules/a");
    let dep_b = tmp.path().join("oh_modules/b");
    fs::create_dir_all(dep_a.join("libs/arm64-v8a")).unwrap();
    fs::create_dir_all(dep_b.join("libs/arm64-v8a")).unwrap();
    fs::create_dir_all(&module_root).unwrap();
    fs::write(dep_a.join("libs/arm64-v8a/libdup.so"), b"ELF SAME").unwrap();
    fs::write(dep_b.join("libs/arm64-v8a/libdup.so"), b"ELF SAME").unwrap();

    let module = ModuleConfig {
      name: "entry".to_string(),
      src_path: module_root,
      module_type: arkdown_core::OhosModuleType::Entry,
    };
    let mut deps = DependencyGraphSnapshot::default();
    add_dependency_node(&mut deps, "a", "a", "1.0.0", dep_a);
    add_dependency_node(&mut deps, "b", "b", "1.0.0", dep_b);

    let libs = NativeLibCollector::new(&module, &[], &deps)
      .collect()
      .expect("same-content duplicate native libs must collapse like hvigor ProcessLibs");
    assert_eq!(libs.len(), 1);
    assert_eq!(libs[0].file_name().unwrap(), "libdup.so");
  }
}
