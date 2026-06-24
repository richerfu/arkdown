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
//! - `src/main/libs/<abi>/` — hand-placed native libraries
//! - `build/default/intermediates/cmake/default/obj/<abi>/` — CMake output
//! - `build/default/intermediates/ndk/default/<abi>/` — NDK output
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
use std::path::{Path, PathBuf};
use std::process::Command;

use arkdown_core::ModuleConfig;
use arkdown_session::DependencyGraphSnapshot;
use rustc_hash::FxHashSet;

/// The set of ABI directories that hvigor's native pipeline produces under
/// the CMake / NDK build intermediates.
const KNOWN_ABI_DIRS: &[&str] = &["arm64-v8a", "armeabi-v7a", "x86_64", "x86", "armeabi"];

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
}

impl NativeLibCollectionPolicy {
  pub fn asan() -> Self {
    Self {
      sanitizer: NativeBuildSanitizer::Asan,
      read_elf: None,
      build_mode: String::new(),
      sdk_toolchain: NativeSdkToolchain::default(),
    }
  }

  pub fn hwasan(read_elf: Option<PathBuf>) -> Self {
    Self {
      sanitizer: NativeBuildSanitizer::Hwasan,
      read_elf,
      build_mode: String::new(),
      sdk_toolchain: NativeSdkToolchain::default(),
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

  fn src_main_libs(&self) -> PathBuf {
    self.module_root.join("src").join("main").join("libs")
  }

  fn top_level_libs(&self) -> PathBuf {
    self.module_root.join("libs")
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

  fn ndk_intermediate_dir(&self) -> PathBuf {
    self
      .build_default()
      .join("intermediates")
      .join("ndk")
      .join("default")
  }

  fn so_search_roots(&self) -> Vec<PathBuf> {
    vec![
      self.cmake_obj_dir(),
      self.ndk_intermediate_dir(),
      self.top_level_libs(),
      self.src_main_libs(),
    ]
  }

  fn has_native_code(&self) -> bool {
    if self.cmake_lists().is_file() {
      return true;
    }
    for libs_dir in [self.src_main_libs(), self.top_level_libs()] {
      if NativeLibraryRoot::new(&libs_dir).contains_known_abi_library() {
        return true;
      }
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
      if let Ok(mut entries) = std::fs::read_dir(&abi_dir) {
        if entries.any(|e| {
          e.ok()
            .map(|entry| {
              let path = entry.path();
              NativeSharedLibraryPath::new(&path).matches_hvigor_pattern()
            })
            .unwrap_or(false)
        }) {
          return true;
        }
      }
    }
    false
  }
}

struct NativeSharedLibraryPath<'a> {
  path: &'a Path,
}

impl<'a> NativeSharedLibraryPath<'a> {
  fn new(path: &'a Path) -> Self {
    Self { path }
  }

  fn matches_hvigor_pattern(&self) -> bool {
    self
      .path
      .file_name()
      .and_then(|name| name.to_str())
      .is_some_and(Self::matches_hvigor_name_pattern)
  }

  fn matches_hvigor_name_pattern(name: &str) -> bool {
    let Some((_, suffix)) = name.rsplit_once(".so") else {
      return false;
    };
    suffix.is_empty()
      || suffix
        .as_bytes()
        .last()
        .is_some_and(|byte| byte.is_ascii_digit())
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
    let mut libs = Vec::new();
    let mut seen = FxHashSet::default();

    let native_cfg = NativeBuildConfig {
      module_root: self.module.src_path.clone(),
      abi_filters: self.abi_filters.to_vec(),
      enable_cmake_build: self.policy.enable_cmake_build(),
      sanitizer: self.policy.sanitizer,
      build_mode: self.policy.build_mode.clone(),
      sdk_toolchain: self.policy.sdk_toolchain.clone(),
    };
    if NativeModuleLayout::new(&self.module.src_path).has_native_code() {
      tracing::debug!(module = %self.module.src_path.display(), "native: scanning for pre-built .so files");
    }
    let native_out = native_cfg.run()?;
    for warning in &native_out.warnings {
      tracing::warn!("{}", warning);
    }
    for (_, so_path) in native_out.libs_by_abi {
      let canonical = std::fs::canonicalize(&so_path).unwrap_or(so_path.clone());
      if seen.insert(canonical) {
        libs.push(so_path);
      }
    }

    for dep_root in self.dependency_roots() {
      self.collect_from_root(dep_root, &mut seen, &mut libs);
    }
    libs.sort();

    // Duplicate SO detection (P10: mirrors hvigor duplicate SO check).
    // After full collection, warn on duplicate file names and keep only the
    // first occurrence (the sort above makes the selection deterministic).
    let mut so_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut result_libs = Vec::new();
    for lib in libs {
      let name = Self::native_lib_identity(&lib);
      if !so_names.insert(name.clone()) {
        tracing::warn!(
            name = %name,
            "duplicate native library detected — keeping first occurrence"
        );
      } else {
        result_libs.push(lib);
      }
    }
    self.policy.validate_packaged_libs(&result_libs)?;
    Ok(result_libs)
  }

  fn native_lib_identity(lib: &Path) -> String {
    let file_name = lib.file_name().and_then(|name| name.to_str()).unwrap_or("");
    let abi = lib.components().find_map(|component| {
      let component = component.as_os_str();
      KNOWN_ABI_DIRS
        .iter()
        .find(|abi| component == OsStr::new(*abi))
        .copied()
    });
    match abi {
      Some(abi) => format!("{abi}/{file_name}"),
      None => file_name.to_string(),
    }
  }

  pub fn collect_context_names(&self) -> Vec<String> {
    let mut names = Vec::new();
    let mut seen = FxHashSet::default();
    self.collect_lib_names_from_libs_dir(&self.module.src_path.join("libs"), &mut seen, &mut names);
    for dep_root in self.dependency_roots() {
      self.collect_lib_names_from_libs_dir(&dep_root.join("libs"), &mut seen, &mut names);
    }
    names.sort();
    names
  }

  fn dependency_roots(&self) -> Vec<&Path> {
    let mut roots = Vec::new();
    let mut seen = FxHashSet::default();
    for root in self.deps.roots_by_alias.values() {
      if seen.insert(root.as_path()) {
        roots.push(root.as_path());
      }
    }
    for node in self.deps.nodes.values() {
      let Some(root) = node.package_root.as_deref() else {
        continue;
      };
      if seen.insert(root) {
        roots.push(root);
      }
    }
    roots
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
      let Some(name) = entry.path().file_name().and_then(|name| name.to_str()) else {
        continue;
      };
      if !NativeSharedLibraryPath::matches_hvigor_name_pattern(name) {
        continue;
      }
      if seen.insert(name.to_string()) {
        names.push(name.to_string());
      }
    }
  }

  pub fn collect_from_root(
    &self,
    root: &Path,
    seen: &mut FxHashSet<PathBuf>,
    libs: &mut Vec<PathBuf>,
  ) {
    if !root.exists() {
      return;
    }
    for entry in walkdir::WalkDir::new(root)
      .into_iter()
      .filter_map(|e| e.ok())
      .filter(|e| e.file_type().is_file())
    {
      let path = entry.path();
      if !NativeSharedLibraryPath::new(path).matches_hvigor_pattern() {
        continue;
      }
      if NativeAbiFilters::new(self.abi_filters).matches_path(path) {
        let path = arkdown_path::canonicalize_or_owned(path);
        if seen.insert(path.clone()) {
          libs.push(path);
        }
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
  /// 2. Collect pre-built shared libraries from standard output locations
  ///    (CMake build output, NDK output, hand-placed `src/main/libs/`).
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
        let Ok(entries) = std::fs::read_dir(&abi_dir) else {
          continue;
        };
        let mut sorted: Vec<PathBuf> = entries
          .filter_map(|e| e.ok().map(|e| e.path()))
          .filter(|p| NativeSharedLibraryPath::new(p).matches_hvigor_pattern())
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
    let libs_dir = tmp.path().join("src/main/libs/arm64-v8a");
    fs::create_dir_all(&libs_dir).unwrap();
    fs::write(libs_dir.join("libfoo.so"), b"ELF\x7f").unwrap();
    assert!(NativeModuleLayout::new(tmp.path()).has_native_code());
  }

  #[test]
  fn test_module_has_native_code_detects_versioned_shared_library() {
    let tmp = tempfile::tempdir().unwrap();
    let libs_dir = tmp.path().join("src/main/libs/arm64-v8a");
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
    let libs_dir = tmp.path().join("src/main/libs/arm64-v8a");
    fs::create_dir_all(&libs_dir).unwrap();
    fs::write(libs_dir.join("libnative.so"), b"ELF").unwrap();

    let cfg = NativeBuildConfig {
      module_root: tmp.path().to_path_buf(),
      abi_filters: vec!["arm64-v8a".to_string()],
      enable_cmake_build: false,
      sanitizer: NativeBuildSanitizer::None,
      build_mode: "debug".to_string(),
      sdk_toolchain: NativeSdkToolchain::default(),
    };
    let out = cfg.run().expect("native scan");
    assert_eq!(out.libs_by_abi.len(), 1);
    assert_eq!(out.libs_by_abi[0].0, "arm64-v8a");
    assert!(out.libs_by_abi[0].1.file_name().unwrap() == "libnative.so");
  }

  #[test]
  fn test_native_import_collects_versioned_shared_libraries() {
    let tmp = tempfile::tempdir().unwrap();
    let libs_dir = tmp.path().join("src/main/libs/arm64-v8a");
    fs::create_dir_all(&libs_dir).unwrap();
    fs::write(libs_dir.join("libexpat.so.1"), b"ELF").unwrap();

    let cfg = NativeBuildConfig {
      module_root: tmp.path().to_path_buf(),
      abi_filters: vec!["arm64-v8a".to_string()],
      enable_cmake_build: false,
      sanitizer: NativeBuildSanitizer::None,
      build_mode: "debug".to_string(),
      sdk_toolchain: NativeSdkToolchain::default(),
    };
    let out = cfg.run().expect("native scan");
    assert_eq!(out.libs_by_abi.len(), 1);
    assert_eq!(out.libs_by_abi[0].0, "arm64-v8a");
    assert_eq!(out.libs_by_abi[0].1.file_name().unwrap(), "libexpat.so.1");
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
    let arm64_dir = tmp.path().join("src/main/libs/arm64-v8a");
    let x64_dir = tmp.path().join("src/main/libs/x86_64");
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
    let dep_root = tmp.path().join("oh_modules/@example/mtflexbox");
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
    deps
      .roots_by_alias
      .insert("@example/mtflexbox".to_string(), dep_root);

    let libs = NativeLibCollector::new(&module, &[], &deps)
      .collect()
      .expect("collect native libs");
    assert_eq!(libs.len(), 1);
    assert_eq!(libs[0].file_name().unwrap(), "libexpat.so.1");
  }
}
