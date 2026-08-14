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
//! - `build/<product>/intermediates/cmake/<target>/obj/<abi>/` — CMake output
//! - `build/<product>/intermediates/cj/extend_libs/<target>/<abi>/` — Cangjie output
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
//! 2. `ninja -C <build-dir> <target...>`
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

use std::collections::BTreeMap;
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::UNIX_EPOCH;

use arkdown_core::{
  BuildMode, ExternalNativeOptions, ModuleConfig, NativeLibFilterOption, NativeLibOption,
  NativeLibPackageSelection, OhosModuleType,
};
use arkdown_path::{canonicalize_or_owned, ArtifactPathSerializer, HvigorNativeLibraryPattern};
use arkdown_session::dependency_graph::snapshot::{
  DependencyGraphSnapshot, DependencyNodeSnapshot,
};
use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

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
  /// Selected product segment used by hvigor's `ModulePathInfoIml`.
  pub product_name: String,
  /// Selected target segment used by native intermediate directories.
  pub target_name: String,
  /// ABI filters (from `buildOption.externalNativeOptions.abiFilters`).
  pub abi_filters: Vec<String>,
  /// Whether to attempt an actual CMake build.
  pub enable_cmake_build: bool,
  /// Effective `externalNativeOptions` after product/build-mode/target merge.
  pub external_native_options: Option<ExternalNativeOptions>,
  /// Sanitizer flags to pass into CMake. Hvigor warns when several are
  /// enabled but still forwards every selected flag.
  pub sanitizers: NativeBuildSanitizers,
  /// Build mode passed to CMake. Mirrors hvigor `targetService.getBuildMode()`.
  pub build_mode: String,
  /// SDK native toolchain paths resolved from the active build session.
  pub sdk_toolchain: NativeSdkToolchain,
  /// `buildOption.nativeLib.collectAllLibs`.
  pub collect_all_libs: bool,
  /// Validated `ohos.defaults.release.cmakebuildtype` value.
  release_cmake_build_type: NativeReleaseCmakeBuildType,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum NativeReleaseCmakeBuildType {
  #[default]
  Release,
  Debug,
  RelWithDebInfo,
}

impl NativeReleaseCmakeBuildType {
  fn from_hvigor_property(value: Option<&str>) -> Self {
    match value {
      Some("Release") | None => Self::Release,
      Some("Debug") => Self::Debug,
      Some("RelWithDebInfo") => Self::RelWithDebInfo,
      Some(value) => {
        tracing::warn!(
          value,
          "invalid release CMake build type; expected Release, Debug, or RelWithDebInfo"
        );
        Self::Release
      }
    }
  }

  fn as_str(self) -> &'static str {
    match self {
      Self::Release => "Release",
      Self::Debug => "Debug",
      Self::RelWithDebInfo => "RelWithDebInfo",
    }
  }
}

/// Native sanitizer flags requested by hvigor-compatible extra config.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeBuildSanitizers {
  asan: bool,
  tsan: bool,
  hwasan: bool,
  ubsan: bool,
}

impl NativeBuildSanitizers {
  pub fn new(asan: bool, tsan: bool, hwasan: bool, ubsan: bool) -> Self {
    Self {
      asan,
      tsan,
      hwasan,
      ubsan,
    }
  }

  fn cmake_args_for_abi(self, abi: &str) -> impl Iterator<Item = &'static str> {
    [
      self.asan.then_some("-DOHOS_ENABLE_ASAN=ON"),
      self.tsan.then_some("-DOHOS_ENABLE_TSAN=ON"),
      (self.hwasan && abi == "arm64-v8a").then_some("-DOHOS_ENABLE_HWASAN=ON"),
      self.ubsan.then_some("-DOHOS_ENABLE_UBSAN=ON"),
    ]
    .into_iter()
    .flatten()
  }

  fn hwasan(self) -> bool {
    self.hwasan
  }
}

#[derive(Debug, Clone, Default)]
pub struct NativeLibCollectionPolicy {
  sanitizers: NativeBuildSanitizers,
  hwasan_config: Option<bool>,
  binxo_config: Option<bool>,
  read_elf: Option<PathBuf>,
  build_mode: String,
  sdk_toolchain: NativeSdkToolchain,
  native_lib: NativeLibOption,
  external_native_options: Option<ExternalNativeOptions>,
  self_package: Option<NativeLibPackageInfo>,
  cmake_local_dependencies: Vec<NativeCmakeLocalDependency>,
  release_cmake_build_type: NativeReleaseCmakeBuildType,
}

impl NativeLibCollectionPolicy {
  pub fn asan() -> Self {
    Self {
      sanitizers: NativeBuildSanitizers::new(true, false, false, false),
      hwasan_config: None,
      binxo_config: None,
      read_elf: None,
      build_mode: String::new(),
      sdk_toolchain: NativeSdkToolchain::default(),
      native_lib: NativeLibOption::default(),
      external_native_options: None,
      self_package: None,
      cmake_local_dependencies: Vec::new(),
      release_cmake_build_type: NativeReleaseCmakeBuildType::default(),
    }
  }

  pub fn hwasan(read_elf: Option<PathBuf>) -> Self {
    Self {
      sanitizers: NativeBuildSanitizers::new(false, false, true, false),
      hwasan_config: None,
      binxo_config: None,
      read_elf,
      build_mode: String::new(),
      sdk_toolchain: NativeSdkToolchain::default(),
      native_lib: NativeLibOption::default(),
      external_native_options: None,
      self_package: None,
      cmake_local_dependencies: Vec::new(),
      release_cmake_build_type: NativeReleaseCmakeBuildType::default(),
    }
  }

  pub fn with_sanitizers(
    mut self,
    sanitizers: NativeBuildSanitizers,
    read_elf: Option<PathBuf>,
  ) -> Self {
    self.sanitizers = sanitizers;
    self.read_elf = read_elf;
    self
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

  pub fn with_external_native_options(mut self, options: Option<ExternalNativeOptions>) -> Self {
    self.external_native_options = options;
    self
  }

  /// Apply the two extra-config values consumed by
  /// `BinxoValidationService`. `None` means hvigor falls back to the effective
  /// CMake arguments; `Some(false)` is an explicit, truthy non-`"true"`
  /// override and therefore disables that side of the validation.
  pub fn with_binxo_config(
    mut self,
    hwasan_config: Option<bool>,
    binxo_config: Option<bool>,
  ) -> Self {
    self.hwasan_config = hwasan_config;
    self.binxo_config = binxo_config;
    self
  }

  pub fn with_self_package(mut self, package: NativeLibPackageInfo) -> Self {
    self.self_package = Some(package);
    self
  }

  pub(crate) fn with_cmake_local_dependencies(
    mut self,
    dependencies: Vec<NativeCmakeLocalDependency>,
  ) -> Self {
    self.cmake_local_dependencies = dependencies;
    self
  }

  pub fn with_release_cmake_build_type(mut self, value: Option<&str>) -> Self {
    self.release_cmake_build_type = NativeReleaseCmakeBuildType::from_hvigor_property(value);
    self
  }

  fn enable_cmake_build(&self) -> bool {
    self.external_native_options.is_some()
  }

  fn validate_packaged_libs(&self, libs: &[PathBuf]) -> NativeBuildResult<()> {
    if !self.sanitizers.hwasan() {
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

/// Resolved local-module state consumed by `ConfigureCmake`.
///
/// Hvigor reads this from each dependency target service. Arkdown resolves it
/// once from the build-session target model and passes the immutable result to
/// the native domain owner, avoiding module build-profile rereads.
#[derive(Debug, Clone)]
pub(crate) struct NativeCmakeLocalDependency {
  package_root: PathBuf,
  product_name: String,
  target_name: String,
  build_mode: String,
  module_type: OhosModuleType,
  native_lib: NativeLibOption,
}

impl NativeCmakeLocalDependency {
  pub(crate) fn new(
    package_root: PathBuf,
    product_name: String,
    target_name: String,
    build_mode: String,
    module_type: OhosModuleType,
    native_lib: NativeLibOption,
  ) -> Self {
    Self {
      package_root: canonicalize_or_owned(&package_root),
      product_name,
      target_name,
      build_mode,
      module_type,
      native_lib,
    }
  }
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
  llvm_strip: Option<PathBuf>,
  binxo: Option<PathBuf>,
  native_dir: Option<PathBuf>,
  toolchain_file: Option<PathBuf>,
}

impl NativeSdkToolchain {
  pub fn new(
    sdk_root: PathBuf,
    cmake: Option<PathBuf>,
    ninja: Option<PathBuf>,
    llvm_strip: Option<PathBuf>,
    binxo: Option<PathBuf>,
  ) -> Self {
    let native_dir = sdk_root.join("native");
    let toolchain_file = native_dir
      .join("build")
      .join("cmake")
      .join("ohos.toolchain.cmake");
    Self {
      cmake,
      ninja,
      llvm_strip,
      binxo,
      native_dir: Some(native_dir),
      toolchain_file: Some(toolchain_file),
    }
  }

  /// Construct the subset of the SDK toolchain needed by hvigor `clean`.
  pub fn for_clean(ninja: Option<PathBuf>) -> Self {
    Self {
      ninja,
      ..Self::default()
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

  fn llvm_strip_path(&self) -> Option<PathBuf> {
    self.llvm_strip.clone()
  }

  fn ninja_path(&self) -> Option<&Path> {
    self.ninja.as_deref()
  }
}

struct ResolvedNativeSdkToolchain<'a> {
  cmake: &'a Path,
  ninja: &'a Path,
  native_dir: &'a Path,
  toolchain_file: &'a Path,
}

/// One Rust-owned native command with an optional argument tail that Hvigor
/// appends after its JavaScript `CommandBuilderModifier` has run.
#[derive(Debug, Clone)]
pub(crate) struct NativeCommandPlan {
  module_root: PathBuf,
  abi: String,
  program: OsString,
  arguments: Vec<OsString>,
  trailing_arguments: Vec<OsString>,
  current_dir: PathBuf,
}

impl NativeCommandPlan {
  fn ninja(
    module_root: &Path,
    abi: &str,
    executable: &Path,
    build_dir: PathBuf,
    targets: impl IntoIterator<Item = String>,
  ) -> Self {
    Self {
      module_root: module_root.to_path_buf(),
      abi: abi.to_string(),
      program: executable.as_os_str().to_os_string(),
      arguments: vec![OsString::from("-C"), build_dir.as_os_str().to_os_string()],
      trailing_arguments: targets.into_iter().map(OsString::from).collect(),
      current_dir: build_dir,
    }
  }

  pub(crate) fn abi(&self) -> &str {
    &self.abi
  }

  /// Convert the modifier-visible base command into the same `string[]`
  /// contract exposed by hvigor. Non-UTF host paths continue to work when no
  /// JavaScript modifier is registered; they are rejected only at this API
  /// boundary because JavaScript cannot preserve their bytes.
  pub(crate) fn modifier_command(&self) -> NativeBuildResult<Vec<String>> {
    std::iter::once(self.program.as_os_str())
      .chain(self.arguments.iter().map(OsString::as_os_str))
      .map(|argument| {
        argument
          .to_str()
          .map(str::to_string)
          .ok_or_else(|| NativeBuildError::CmakeFailed {
            module_root: self.module_root.clone(),
            message: format!(
              "Ninja CommandBuilderModifier requires UTF-8 command arguments for ABI {}",
              self.abi
            ),
          })
      })
      .collect()
  }

  pub(crate) fn with_modifier_command(mut self, command: Vec<String>) -> NativeBuildResult<Self> {
    let mut command = command.into_iter();
    let program = command
      .next()
      .filter(|program| !program.is_empty())
      .ok_or_else(|| NativeBuildError::CmakeFailed {
        module_root: self.module_root.clone(),
        message: format!(
          "Ninja CommandBuilderModifier returned an empty command for ABI {}",
          self.abi
        ),
      })?;
    self.program = OsString::from(program);
    self.arguments = command.map(OsString::from).collect();
    Ok(self)
  }

  pub(crate) fn execute(self) -> NativeBuildResult<()> {
    let mut command = self.command();
    tracing::info!(
        abi = %self.abi,
        cmd = ?command,
        "native: running Ninja build"
    );
    let status = command
      .status()
      .map_err(|error| NativeBuildError::CmakeFailed {
        module_root: self.module_root.clone(),
        message: format!("failed to run Ninja build: {error}"),
      })?;
    if !status.success() {
      return Err(NativeBuildError::CmakeFailed {
        module_root: self.module_root,
        message: format!(
          "Ninja build for ABI {} exited with status {status}",
          self.abi
        ),
      });
    }
    Ok(())
  }

  fn command(&self) -> Command {
    let mut command = Command::new(&self.program);
    command
      .args(&self.arguments)
      .args(&self.trailing_arguments)
      .current_dir(&self.current_dir);
    command
  }
}

/// Exact DevEco Studio 6.1.1.280 `native-binxo.ts` command contract.
struct NativeBinxoCommand<'a> {
  executable: &'a Path,
  source: &'a Path,
  destination: &'a Path,
}

impl<'a> NativeBinxoCommand<'a> {
  fn new(executable: &'a Path, source: &'a Path, destination: &'a Path) -> Self {
    Self {
      executable,
      source,
      destination,
    }
  }

  fn command(&self) -> Command {
    let mut command = Command::new(self.executable);
    command
      .arg("-A")
      .arg("--hwasan-inputfile")
      .arg(self.source)
      .arg("--hwasan-outputfil")
      .arg(self.destination);
    command
  }

  fn execute(&self) -> std::io::Result<std::process::Output> {
    self.command().output()
  }
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
  BinxoRequiresHwasan {
    profile: PathBuf,
  },
  NativeLibRelativePath {
    path: PathBuf,
    message: String,
  },
  DuplicateCheckFailed {
    path: PathBuf,
    message: String,
  },
  InvalidNativePattern {
    pattern: String,
    message: String,
  },
  NativePostProcessFailed {
    path: PathBuf,
    message: String,
  },
  DuplicatedFilesFoundInLibPath {
    module_name: String,
    libs: Vec<PathBuf>,
  },
  DebugSymbolCollectionFailed {
    path: PathBuf,
    message: String,
  },
  CleanFailed {
    path: PathBuf,
    message: String,
  },
  ConfigureCmakeFailed {
    path: PathBuf,
    message: String,
  },
  UnknownCpuAbi {
    abi: String,
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
      Self::BinxoRequiresHwasan { profile } => write!(
        f,
        "DO_BINXO_MUST_OPEN_HWASAN: enable both ohos-enable-hwasan and ohos-enable-binxo in '{}'",
        profile.display()
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
      Self::InvalidNativePattern { pattern, message } => write!(
        f,
        "invalid buildOption.nativeLib.debugSymbol.exclude pattern '{}': {}",
        pattern, message
      ),
      Self::NativePostProcessFailed { path, message } => write!(
        f,
        "failed to post-process native library '{}': {}",
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
      Self::DebugSymbolCollectionFailed { path, message } => write!(
        f,
        "failed to collect debug symbols at '{}': {}",
        path.display(),
        message
      ),
      Self::CleanFailed { path, message } => write!(
        f,
        "failed to clean hvigor build path '{}': {}",
        path.display(),
        message
      ),
      Self::ConfigureCmakeFailed { path, message } => write!(
        f,
        "failed to generate ConfigureCmake output '{}': {}",
        path.display(),
        message
      ),
      Self::UnknownCpuAbi { abi } => write!(f, "UNKNOWN_CPU_ABI: '{abi}'"),
    }
  }
}

impl Error for NativeBuildError {}

/// Owns the latest-hvigor module `clean` action selected dynamically by
/// `hvigorOrToolChainsChanged`. Build outputs are removed with Windows-safe
/// retries, then valid CMake/Ninja trees under `.cxx` receive `ninja clean`
/// and have their stale `CMakeCache.txt` removed.
pub struct HvigorModuleCleanStage<'a> {
  project_root: &'a Path,
  module_root: &'a Path,
  ninja: Option<&'a Path>,
  project_task: bool,
  configured_build_dir: Option<PathBuf>,
  native_product_names: Vec<String>,
  native_target_names: Vec<String>,
  native_build_mode: String,
}

impl<'a> HvigorModuleCleanStage<'a> {
  pub fn new(
    project_root: &'a Path,
    module_root: &'a Path,
    sdk_toolchain: &'a NativeSdkToolchain,
  ) -> Self {
    Self {
      project_root,
      module_root,
      ninja: sdk_toolchain.ninja_path(),
      project_task: false,
      configured_build_dir: None,
      native_product_names: vec!["default".to_string()],
      native_target_names: vec!["default".to_string()],
      native_build_mode: "debug".to_string(),
    }
  }

  pub fn for_project(project_root: &'a Path) -> Self {
    Self {
      project_root,
      module_root: project_root,
      ninja: None,
      project_task: true,
      configured_build_dir: None,
      native_product_names: Vec::new(),
      native_target_names: Vec::new(),
      native_build_mode: String::new(),
    }
  }

  pub fn with_configured_build_dir(
    mut self,
    configured_build_dir: Option<&Path>,
  ) -> NativeBuildResult<Self> {
    self.configured_build_dir = match configured_build_dir.filter(|path| path.is_absolute()) {
      Some(path) if path.exists() => {
        Some(
          dunce::canonicalize(path).map_err(|error| NativeBuildError::CleanFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
          })?,
        )
      }
      Some(path) => Some(path.to_path_buf()),
      None => None,
    };
    Ok(self)
  }

  pub fn with_native_variants(
    mut self,
    product_names: Vec<String>,
    target_names: Vec<String>,
    build_mode: impl Into<String>,
  ) -> Self {
    if !product_names.is_empty() {
      self.native_product_names = product_names;
    }
    if !target_names.is_empty() {
      self.native_target_names = target_names;
    }
    self.native_build_mode = build_mode.into();
    self
  }

  pub fn execute(&self) -> NativeBuildResult<()> {
    let mut directories = vec![self.project_root.join("build")];
    if !self.project_task {
      directories.push(self.module_root.join("build"));
      directories.push(self.module_root.join(".test"));
    }
    let normal_build_directories = directories
      .iter()
      .filter(|directory| directory.file_name() == Some(OsStr::new("build")))
      .cloned()
      .collect::<Vec<_>>();
    for directory in normal_build_directories {
      if let Some(configured) = self.configured_build_directory(&directory) {
        if configured != directory {
          if Self::is_ci_environment() {
            directories.push(
              configured
                .parent()
                .map_or_else(|| configured.join(".cxx"), |parent| parent.join(".cxx")),
            );
          }
          directories.push(configured);
        }
      }
    }
    directories.sort();
    directories.dedup();
    for directory in directories {
      self.remove_directory(&directory)?;
    }
    if !self.project_task {
      self.clean_native_trees();
    }
    Ok(())
  }

  fn configured_build_directory(&self, build_directory: &Path) -> Option<PathBuf> {
    let configured = self.configured_build_dir.as_ref()?;
    let project_name = self.project_root.file_name()?;
    let relative = build_directory.strip_prefix(self.project_root).ok()?;
    Some(configured.join(project_name).join(relative))
  }

  fn is_ci_environment() -> bool {
    if std::env::var_os("CI").as_deref() == Some(OsStr::new("false")) {
      return false;
    }
    [
      "BUILD_ID",
      "BUILD_NUMBER",
      "CI",
      "CI_APP_ID",
      "CI_BUILD_ID",
      "CI_BUILD_NUMBER",
      "CI_NAME",
      "CONTINUOUS_INTEGRATION",
      "RUN_ID",
    ]
    .into_iter()
    .any(|variable| std::env::var_os(variable).is_some())
  }

  fn remove_directory(&self, directory: &Path) -> NativeBuildResult<()> {
    for attempt in 0..=3 {
      match std::fs::remove_dir_all(directory) {
        Ok(()) => return Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) if attempt < 3 => {
          tracing::debug!(
            path = %directory.display(),
            attempt = attempt + 1,
            error = %error,
            "retrying hvigor clean path removal"
          );
          std::thread::sleep(std::time::Duration::from_millis(100));
        }
        Err(error) => {
          return Err(NativeBuildError::CleanFailed {
            path: directory.to_path_buf(),
            message: error.to_string(),
          });
        }
      }
    }
    Ok(())
  }

  fn clean_native_trees(&self) {
    let Some(ninja) = self.ninja.filter(|path| path.is_file()) else {
      return;
    };
    let cxx_root = self.module_root.join(".cxx");
    if !cxx_root.is_dir() {
      return;
    }
    let mut build_directories = Vec::new();
    for product_name in &self.native_product_names {
      for target_name in &self.native_target_names {
        let variant_root = cxx_root
          .join(product_name)
          .join(target_name)
          .join(&self.native_build_mode);
        let Ok(entries) = std::fs::read_dir(&variant_root) else {
          continue;
        };
        for entry in entries.filter_map(Result::ok) {
          let directory = entry.path();
          if !entry
            .file_name()
            .to_str()
            .is_some_and(|name| KNOWN_ABI_DIRS.contains(&name))
            || std::fs::read_dir(&directory).is_err()
            || !directory.join("CMakeCache.txt").is_file()
            || !directory.join("build.ninja").is_file()
            || !(directory.join("rules.ninja").is_file()
              || directory.join("CMakeFiles").join("rules.ninja").is_file())
          {
            continue;
          }
          build_directories.push(directory);
        }
      }
    }
    build_directories.sort();
    build_directories.dedup();
    for directory in build_directories {
      match Command::new(ninja)
        .arg("-C")
        .arg(&directory)
        .arg("clean")
        .status()
      {
        Ok(status) if status.success() => {}
        Ok(status) => tracing::warn!(
          path = %directory.display(),
          exit_status = %status,
          "hvigor native clean command failed; the native tree is retained"
        ),
        Err(error) => tracing::warn!(
          path = %directory.display(),
          error = %error,
          "hvigor native clean command could not start; the native tree is retained"
        ),
      }
      let cache_path = directory.join("CMakeCache.txt");
      if let Err(error) = std::fs::remove_file(&cache_path) {
        if error.kind() != std::io::ErrorKind::NotFound {
          tracing::warn!(
            path = %cache_path.display(),
            error = %error,
            "failed to remove native CMake cache after clean"
          );
        }
      }
    }
  }
}

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
  product_name: &'a str,
  target_name: &'a str,
}

impl<'a> NativeModuleLayout<'a> {
  fn new(module_root: &'a Path) -> Self {
    Self::for_variant(module_root, "default", "default")
  }

  fn for_variant(module_root: &'a Path, product_name: &'a str, target_name: &'a str) -> Self {
    Self {
      module_root,
      product_name,
      target_name,
    }
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
    self.module_root.join("build").join(self.product_name)
  }

  fn cmake_intermediate_dir(&self) -> PathBuf {
    self
      .build_default()
      .join("intermediates")
      .join("cmake")
      .join(self.target_name)
  }

  /// `ModulePathInfoIml.getNinjaWorkDir()`.
  fn ninja_work_dir(&self, build_mode: &str) -> PathBuf {
    self
      .module_root
      .join(".cxx")
      .join(self.product_name)
      .join(self.target_name)
      .join(build_mode)
  }

  /// Root generated by `ConfigureCmake` and passed to CMake as
  /// `CMAKE_FIND_ROOT_PATH`.
  fn cmake_dependency_root(&self, build_mode: &str) -> PathBuf {
    self.ninja_work_dir(build_mode).join("hvigor")
  }

  fn cmake_obj_dir(&self) -> PathBuf {
    self.cmake_intermediate_dir().join("obj")
  }

  fn stripped_libs_dir(&self) -> PathBuf {
    self
      .build_default()
      .join("intermediates")
      .join("stripped_native_libs")
      .join(self.target_name)
  }

  fn process_libs_dir(&self) -> PathBuf {
    self
      .build_default()
      .join("intermediates")
      .join("libs")
      .join(self.target_name)
  }

  fn native_lib_cache_file(&self) -> PathBuf {
    self
      .build_default()
      .join("intermediates")
      .join("patch")
      .join(self.target_name)
      .join("base_native_libs.json")
  }

  fn binxo_libs_dir(&self) -> PathBuf {
    self
      .build_default()
      .join("intermediates")
      .join("binxo")
      .join(self.target_name)
  }

  fn binxo_native_lib_cache_file(&self) -> PathBuf {
    self
      .build_default()
      .join("intermediates")
      .join("patch")
      .join(self.target_name)
      .join("binxo_native_libs.json")
  }

  fn patch_last_libs_file(&self) -> PathBuf {
    self
      .build_default()
      .join("intermediates")
      .join("patch")
      .join(self.target_name)
      .join("libs.json")
  }

  fn cangjie_intermediate_dir(&self) -> PathBuf {
    self
      .build_default()
      .join("intermediates")
      .join("cj")
      .join("extend_libs")
      .join(self.target_name)
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

/// Files materialized by hvigor's `CollectDebugSymbol` task.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DebugSymbolOutput {
  pub source_map: Option<PathBuf>,
  pub name_cache: Option<PathBuf>,
  pub native_lib_links: Vec<PathBuf>,
}

/// Owns release debug-symbol collection after package/sign stages complete.
///
/// Source selection and output names mirror
/// `hvigor-ohos-plugin/src/tasks/collect-debug-symbol.ts`: prefer the
/// compiler-owned source map (loader output in debug, CompileArkTS cache in
/// release), fall back to an alternate package-owned map, copy the obfuscation
/// name cache when it exists, and link ABI directories from the `ProcessLibs`
/// output.
pub struct DebugSymbolCollector {
  module_root: PathBuf,
  module_name: String,
  build_mode: BuildMode,
  source_map_primary: PathBuf,
  source_map_fallback: Option<PathBuf>,
  name_cache: PathBuf,
  should_collect: bool,
  product_name: String,
  target_name: String,
}

impl DebugSymbolCollector {
  pub fn new(
    module_root: PathBuf,
    module_name: String,
    build_mode: BuildMode,
    source_map_primary: PathBuf,
    source_map_fallback: Option<PathBuf>,
    name_cache: PathBuf,
  ) -> Self {
    let should_collect = matches!(build_mode, BuildMode::Release);
    Self {
      module_root,
      module_name,
      build_mode,
      source_map_primary,
      source_map_fallback,
      name_cache,
      should_collect,
      product_name: "default".to_string(),
      target_name: "default".to_string(),
    }
  }

  pub fn with_collection_override(mut self, enabled: Option<bool>) -> Self {
    if let Some(enabled) = enabled {
      self.should_collect = enabled;
    }
    self
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

  pub fn collect(&self) -> NativeBuildResult<DebugSymbolOutput> {
    if !self.should_collect {
      return Ok(DebugSymbolOutput::default());
    }

    let destination = self
      .module_root
      .join("build")
      .join(&self.product_name)
      .join("outputs")
      .join(&self.target_name)
      .join("symbol")
      .join(self.build_mode.as_str());
    std::fs::create_dir_all(&destination).map_err(|error| {
      NativeBuildError::DebugSymbolCollectionFailed {
        path: destination.clone(),
        message: error.to_string(),
      }
    })?;

    let source_map = self
      .source_map_input()
      .map(|source| {
        let output = destination.join(format!("{}-sourceMaps.map", self.module_name));
        self.copy_artifact(source, &output)?;
        Ok(output)
      })
      .transpose()?;
    let name_cache = self
      .name_cache
      .is_file()
      .then(|| {
        let output = destination.join(format!("{}-nameCache.json", self.module_name));
        self.copy_artifact(&self.name_cache, &output)?;
        Ok(output)
      })
      .transpose()?;
    let native_lib_links = self.link_native_lib_directories(&destination)?;

    Ok(DebugSymbolOutput {
      source_map,
      name_cache,
      native_lib_links,
    })
  }

  fn source_map_input(&self) -> Option<&Path> {
    if self.source_map_primary.is_file() {
      Some(&self.source_map_primary)
    } else {
      self
        .source_map_fallback
        .as_deref()
        .filter(|path| path.is_file())
    }
  }

  fn copy_artifact(&self, source: &Path, destination: &Path) -> NativeBuildResult<()> {
    std::fs::copy(source, destination)
      .map(|_| ())
      .map_err(|error| NativeBuildError::DebugSymbolCollectionFailed {
        path: source.to_path_buf(),
        message: error.to_string(),
      })
  }

  fn link_native_lib_directories(&self, destination: &Path) -> NativeBuildResult<Vec<PathBuf>> {
    let process_libs =
      NativeModuleLayout::for_variant(&self.module_root, &self.product_name, &self.target_name)
        .process_libs_dir();
    if !process_libs.is_dir() {
      return Ok(Vec::new());
    }
    let mut abi_directories = std::fs::read_dir(&process_libs)
      .map_err(|error| NativeBuildError::DebugSymbolCollectionFailed {
        path: process_libs.clone(),
        message: error.to_string(),
      })?
      .filter_map(Result::ok)
      .filter_map(|entry| {
        entry
          .file_type()
          .ok()
          .filter(|file_type| file_type.is_dir())
          .map(|_| entry.path())
      })
      .collect::<Vec<_>>();
    abi_directories.sort();

    let mut links = Vec::with_capacity(abi_directories.len());
    for source in abi_directories {
      let Some(file_name) = source.file_name() else {
        continue;
      };
      let link = destination.join(file_name);
      if link.exists() {
        links.push(link);
        continue;
      }
      match self.symlink_directory(&source, &link) {
        Ok(()) => links.push(link),
        Err(error) => tracing::warn!(
          source = %source.display(),
          destination = %link.display(),
          error = %error,
          "failed to create native debug-symbol directory link"
        ),
      }
    }
    Ok(links)
  }

  fn symlink_directory(&self, source: &Path, destination: &Path) -> std::io::Result<()> {
    Self::link_directory(source, destination)
  }

  fn link_directory(source: &Path, destination: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
      std::os::unix::fs::symlink(source, destination)
    }
    #[cfg(windows)]
    {
      // Latest hvigor passes `junction` to Node's fs.symlinkSync on Windows.
      // Junctions avoid the Developer Mode/administrator requirement of a
      // directory symbolic link while preserving the same directory view.
      junction::create(source, destination)
    }
    #[cfg(not(any(unix, windows)))]
    {
      let _ = (source, destination);
      Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "directory symlinks are unsupported on this platform",
      ))
    }
  }
}

/// APP-scope symbol directory and ZIP emitted by latest `PackageApp`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectDebugSymbolOutput {
  pub destination: Option<PathBuf>,
  pub archive: Option<PathBuf>,
  pub module_links: Vec<PathBuf>,
}

/// Aggregates per-module symbol directories into
/// `build/outputs/<product>/symbol/<mode>/app-symbol.zip`.
pub struct ProjectDebugSymbolCollector {
  project_root: PathBuf,
  product_name: String,
  target_name: String,
  build_mode: BuildMode,
  modules: Vec<ModuleConfig>,
  should_collect: bool,
}

impl ProjectDebugSymbolCollector {
  pub fn new(
    project_root: PathBuf,
    product_name: String,
    target_name: String,
    build_mode: BuildMode,
    modules: Vec<ModuleConfig>,
    enabled: Option<bool>,
  ) -> Self {
    let should_collect = enabled.unwrap_or(matches!(build_mode, BuildMode::Release));
    Self {
      project_root,
      product_name,
      target_name,
      build_mode,
      modules,
      should_collect,
    }
  }

  pub fn collect(&self) -> NativeBuildResult<ProjectDebugSymbolOutput> {
    if !self.should_collect {
      return Ok(ProjectDebugSymbolOutput::default());
    }
    let destination = self.destination();
    std::fs::create_dir_all(&destination).map_err(|error| {
      NativeBuildError::DebugSymbolCollectionFailed {
        path: destination.clone(),
        message: error.to_string(),
      }
    })?;

    let mut module_links = Vec::new();
    for module in self.modules.iter().filter(|module| {
      matches!(
        module.module_type,
        OhosModuleType::Entry | OhosModuleType::Feature | OhosModuleType::Shared
      )
    }) {
      let source = self.module_destination(module);
      std::fs::create_dir_all(&source).map_err(|error| {
        NativeBuildError::DebugSymbolCollectionFailed {
          path: source.clone(),
          message: error.to_string(),
        }
      })?;
      let link = destination.join(&module.name);
      if link.exists() {
        module_links.push(link);
        continue;
      }
      match DebugSymbolCollector::link_directory(&source, &link) {
        Ok(()) => module_links.push(link),
        Err(error) => tracing::warn!(
          source = %source.display(),
          destination = %link.display(),
          error = %error,
          "failed to create project debug-symbol directory link"
        ),
      }
    }

    let archive = self.build_zip(&destination)?;
    Ok(ProjectDebugSymbolOutput {
      destination: Some(destination),
      archive: Some(archive),
      module_links,
    })
  }

  fn destination(&self) -> PathBuf {
    self
      .project_root
      .join("build")
      .join("outputs")
      .join(&self.product_name)
      .join("symbol")
      .join(self.mode_name())
  }

  fn module_destination(&self, module: &ModuleConfig) -> PathBuf {
    module
      .src_path
      .join("build")
      .join(&self.product_name)
      .join("outputs")
      .join(&self.target_name)
      .join("symbol")
      .join(self.mode_name())
  }

  fn mode_name(&self) -> &'static str {
    self.build_mode.as_str()
  }

  fn build_zip(&self, destination: &Path) -> NativeBuildResult<PathBuf> {
    let archive = destination.join("app-symbol.zip");
    if archive.is_file() {
      std::fs::remove_file(&archive).map_err(|error| {
        NativeBuildError::DebugSymbolCollectionFailed {
          path: archive.clone(),
          message: error.to_string(),
        }
      })?;
    }
    let temp_dir = destination
      .parent()
      .expect("project symbol mode directory has a parent")
      .join("temp");
    if temp_dir.exists() {
      std::fs::remove_dir_all(&temp_dir).map_err(|error| {
        NativeBuildError::DebugSymbolCollectionFailed {
          path: temp_dir.clone(),
          message: error.to_string(),
        }
      })?;
    }
    std::fs::create_dir_all(&temp_dir).map_err(|error| {
      NativeBuildError::DebugSymbolCollectionFailed {
        path: temp_dir.clone(),
        message: error.to_string(),
      }
    })?;
    let temporary_archive = temp_dir.join("app-symbol.zip");
    let result = self
      .write_zip(destination, &temporary_archive)
      .and_then(|()| {
        std::fs::rename(&temporary_archive, &archive).map_err(|error| {
          NativeBuildError::DebugSymbolCollectionFailed {
            path: temporary_archive.clone(),
            message: error.to_string(),
          }
        })
      });
    let cleanup_result = std::fs::remove_dir_all(&temp_dir).map_err(|error| {
      NativeBuildError::DebugSymbolCollectionFailed {
        path: temp_dir,
        message: error.to_string(),
      }
    });
    result?;
    cleanup_result?;
    Ok(archive)
  }

  fn write_zip(&self, source: &Path, output: &Path) -> NativeBuildResult<()> {
    let mut entries = walkdir::WalkDir::new(source)
      .follow_links(true)
      .min_depth(1)
      .into_iter()
      .collect::<Result<Vec<_>, _>>()
      .map_err(|error| NativeBuildError::DebugSymbolCollectionFailed {
        path: error
          .path()
          .map(Path::to_path_buf)
          .unwrap_or_else(|| source.to_path_buf()),
        message: error.to_string(),
      })?;
    entries.sort_by_key(|entry| ArtifactPathSerializer::rel_to_slash(entry.path()));

    let file = std::fs::File::create(output).map_err(|error| {
      NativeBuildError::DebugSymbolCollectionFailed {
        path: output.to_path_buf(),
        message: error.to_string(),
      }
    })?;
    let mut archive = zip::ZipWriter::new(file);
    let options =
      zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for entry in entries {
      let relative = entry.path().strip_prefix(source).map_err(|error| {
        NativeBuildError::DebugSymbolCollectionFailed {
          path: entry.path().to_path_buf(),
          message: error.to_string(),
        }
      })?;
      let name = ArtifactPathSerializer::path_to_slash_string(relative).map_err(|error| {
        NativeBuildError::DebugSymbolCollectionFailed {
          path: entry.path().to_path_buf(),
          message: error.to_string(),
        }
      })?;
      if entry.file_type().is_dir() {
        archive
          .add_directory(format!("{name}/"), options)
          .map_err(|error| NativeBuildError::DebugSymbolCollectionFailed {
            path: entry.path().to_path_buf(),
            message: error.to_string(),
          })?;
        continue;
      }
      archive.start_file(name, options).map_err(|error| {
        NativeBuildError::DebugSymbolCollectionFailed {
          path: entry.path().to_path_buf(),
          message: error.to_string(),
        }
      })?;
      let mut input = std::fs::File::open(entry.path()).map_err(|error| {
        NativeBuildError::DebugSymbolCollectionFailed {
          path: entry.path().to_path_buf(),
          message: error.to_string(),
        }
      })?;
      std::io::copy(&mut input, &mut archive).map_err(|error| {
        NativeBuildError::DebugSymbolCollectionFailed {
          path: entry.path().to_path_buf(),
          message: error.to_string(),
        }
      })?;
    }
    archive
      .finish()
      .map(|_| ())
      .map_err(|error| NativeBuildError::DebugSymbolCollectionFailed {
        path: output.to_path_buf(),
        message: error.to_string(),
      })
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

/// Domain owner for Hvigor 6.24.3 `ConfigureCmake`.
///
/// It converts native OHPM/npm dependencies into CMake config packages under
/// `<module>/.cxx/<product>/<target>/<mode>/hvigor/<abi>` and writes the
/// per-ABI `summary.cmake` included by `BuildNativeWithCmake`.
struct NativeCmakeDependencyConfigurator<'a> {
  module: &'a ModuleConfig,
  abi_filters: &'a [String],
  deps: &'a DependencyGraphSnapshot,
  product_name: &'a str,
  target_name: &'a str,
  build_mode: &'a str,
  local_dependencies: &'a [NativeCmakeLocalDependency],
}

impl<'a> NativeCmakeDependencyConfigurator<'a> {
  fn new(
    module: &'a ModuleConfig,
    abi_filters: &'a [String],
    deps: &'a DependencyGraphSnapshot,
    product_name: &'a str,
    target_name: &'a str,
    build_mode: &'a str,
    local_dependencies: &'a [NativeCmakeLocalDependency],
  ) -> Self {
    Self {
      module,
      abi_filters,
      deps,
      product_name,
      target_name,
      build_mode,
      local_dependencies,
    }
  }

  fn write(&self) -> NativeBuildResult<()> {
    let mode = if self.build_mode.is_empty() {
      "debug"
    } else {
      self.build_mode
    };
    let layout =
      NativeModuleLayout::for_variant(&self.module.src_path, self.product_name, self.target_name);
    let cmake_root = layout.cmake_dependency_root(mode);
    let packages = self.packages();
    for abi in NativeAbiFilters::new(self.abi_filters).build_abis() {
      let cpu_type = Self::cpu_type(&abi)?;
      let abi_root = cmake_root.join(&abi);
      let mut summary_entries = Vec::with_capacity(packages.len());
      for package in &packages {
        let cmake_file = abi_root
          .join("lib")
          .join(cpu_type)
          .join("cmake")
          .join(&package.name)
          .join(format!("{}Config.cmake", package.name));
        let libraries = package.libraries_for_abi(&abi)?;
        let content = package.cmake_content(&libraries)?;
        Self::write_if_changed(&cmake_file, content.as_bytes())?;
        let cmake_file =
          ArtifactPathSerializer::path_to_slash_string(&cmake_file).map_err(|error| {
            NativeBuildError::ConfigureCmakeFailed {
              path: cmake_file.clone(),
              message: error.to_string(),
            }
          })?;
        summary_entries.push(format!(
          "if(EXISTS {cmake_file}){eol}    find_package({package_name} REQUIRED CONFIG){eol}endif(){eol}",
          eol = Self::eol(),
          package_name = package.name,
        ));
      }
      let summary = summary_entries.join(Self::eol());
      Self::write_if_changed(&abi_root.join("summary.cmake"), summary.as_bytes())?;
    }
    Ok(())
  }

  fn packages(&self) -> Vec<NativeCmakePackage> {
    let mut packages = Vec::new();
    let mut seen = FxHashSet::default();
    for node in self.ordered_nodes() {
      let Some(root) = node.package_root.as_deref() else {
        continue;
      };
      let canonical_root = canonicalize_or_owned(root);
      if !seen.insert(canonical_root.clone()) {
        continue;
      }
      let local = self
        .local_dependencies
        .iter()
        .find(|dependency| dependency.package_root == canonical_root);
      if local.is_none() && !Self::remote_has_native_inputs(root) {
        continue;
      }
      packages.push(NativeCmakePackage::new(node, canonical_root, local));
    }
    packages
  }

  fn ordered_nodes(&self) -> Vec<&DependencyNodeSnapshot> {
    let mut nodes = Vec::new();
    let mut seen = FxHashSet::default();
    for edge in &self.deps.edges {
      let node = self.deps.nodes.get(&edge.package_name).or_else(|| {
        self.deps.nodes.values().find(|node| {
          node.package_root.as_deref().is_some_and(|root| {
            canonicalize_or_owned(root) == canonicalize_or_owned(&edge.package_root)
          })
        })
      });
      if let Some(node) = node {
        if seen.insert(node.package_name.as_str()) {
          nodes.push(node);
        }
      }
    }
    for node in self.deps.nodes.values() {
      if seen.insert(node.package_name.as_str()) {
        nodes.push(node);
      }
    }
    nodes
  }

  fn remote_has_native_inputs(root: &Path) -> bool {
    let libs = root.join(HVIGOR_LIBS_DIR);
    let include = root.join("include");
    include.exists()
      || libs
        .read_dir()
        .ok()
        .is_some_and(|mut entries| entries.next().is_some())
  }

  fn cpu_type(abi: &str) -> NativeBuildResult<&'static str> {
    match abi {
      "arm64-v8a" => Ok("aarch64-linux-ohos"),
      "armeabi-v7a" => Ok("arm-linux-ohos"),
      "x86_64" => Ok("x86_64-linux-ohos"),
      _ => Err(NativeBuildError::UnknownCpuAbi {
        abi: abi.to_string(),
      }),
    }
  }

  fn write_if_changed(path: &Path, content: &[u8]) -> NativeBuildResult<()> {
    if std::fs::read(path).is_ok_and(|current| current == content) {
      return Ok(());
    }
    if let Some(parent) = path.parent() {
      std::fs::create_dir_all(parent).map_err(|error| NativeBuildError::ConfigureCmakeFailed {
        path: parent.to_path_buf(),
        message: error.to_string(),
      })?;
    }
    std::fs::write(path, content).map_err(|error| NativeBuildError::ConfigureCmakeFailed {
      path: path.to_path_buf(),
      message: error.to_string(),
    })
  }

  const fn eol() -> &'static str {
    if cfg!(windows) {
      "\r\n"
    } else {
      "\n"
    }
  }
}

struct NativeCmakePackage {
  name: String,
  root: PathBuf,
  header_paths: Vec<PathBuf>,
  local: Option<NativeCmakeLocalDependency>,
  match_rule: Option<NativeLibPackageSelection>,
  link_libraries: BTreeMap<String, Vec<String>>,
}

impl NativeCmakePackage {
  fn new(
    node: &DependencyNodeSnapshot,
    root: PathBuf,
    local: Option<&NativeCmakeLocalDependency>,
  ) -> Self {
    let canonical_name = node.package_name.clone();
    let name = canonical_name
      .strip_prefix('@')
      .and_then(|name| name.split_once('/').map(|(_, package)| package))
      .unwrap_or(&canonical_name)
      .to_string();
    let version = node
      .package
      .as_ref()
      .map(|package| package.version.clone())
      .or_else(|| node.version_spec.clone())
      .unwrap_or_default();
    let header_paths = local.map_or_else(
      || vec![root.join("include")],
      |dependency| {
        if dependency.native_lib.header_paths.is_empty() {
          vec![root.join("include")]
        } else {
          dependency
            .native_lib
            .header_paths
            .iter()
            .map(|path| {
              if path.is_absolute() {
                path.clone()
              } else {
                root.join(path)
              }
            })
            .collect()
        }
      },
    );
    let match_rule = local.and_then(|dependency| {
      dependency
        .native_lib
        .filter
        .select
        .iter()
        .find(|selection| {
          selection.package == canonical_name
            && selection.version.as_deref() == Some(version.as_str())
        })
        .cloned()
    });
    let link_libraries = local.map_or_else(
      || Self::remote_link_libraries(node),
      |dependency| {
        dependency
          .native_lib
          .libraries_info
          .iter()
          .filter(|library| !library.name.is_empty() && !library.link_libraries.is_empty())
          .map(|library| (library.name.clone(), library.link_libraries.clone()))
          .collect()
      },
    );
    Self {
      name,
      root,
      header_paths,
      local: local.cloned(),
      match_rule,
      link_libraries,
    }
  }

  fn libraries_for_abi(&self, abi: &str) -> NativeBuildResult<Vec<NativeCmakeLibrary>> {
    let mut libraries = if let Some(local) = self.local.as_ref() {
      NativeCmakeCodeModelReader::new(local, abi).libraries()?
    } else {
      self.remote_libraries(abi)
    };
    libraries.retain(|library| library.name_on_disk != "libc++_shared.so");
    libraries.sort_by(|left, right| left.path.cmp(&right.path));
    libraries.dedup_by(|left, right| left.path == right.path);
    Ok(libraries)
  }

  fn remote_libraries(&self, abi: &str) -> Vec<NativeCmakeLibrary> {
    let abi_root = self.root.join(HVIGOR_LIBS_DIR).join(abi);
    if !abi_root.is_dir() {
      return Vec::new();
    }
    walkdir::WalkDir::new(&abi_root)
      .into_iter()
      .filter_map(Result::ok)
      .filter(|entry| entry.file_type().is_file())
      .filter(|entry| HvigorNativeLibraryPattern::matches_path_under(&abi_root, entry.path()))
      .filter_map(|entry| NativeCmakeLibrary::from_path(entry.into_path()))
      .collect()
  }

  fn cmake_content(&self, libraries: &[NativeCmakeLibrary]) -> NativeBuildResult<String> {
    let includes = self
      .header_paths
      .iter()
      .filter(|path| {
        path.is_dir()
          && path
            .read_dir()
            .ok()
            .is_some_and(|mut entries| entries.next().is_some())
      })
      .map(|path| {
        ArtifactPathSerializer::path_to_slash_string(path).map_err(|error| {
          NativeBuildError::ConfigureCmakeFailed {
            path: path.clone(),
            message: error.to_string(),
          }
        })
      })
      .collect::<NativeBuildResult<Vec<_>>>()?;
    let mut entries = Vec::with_capacity(libraries.len());
    for library in libraries {
      if self
        .match_rule
        .as_ref()
        .is_some_and(|rule| Self::is_excluded(rule, library))
      {
        entries.push(NativeCmakeDependencyConfigurator::eol().to_string());
        continue;
      }
      let mut content = String::new();
      let target = format!("{}::{}", self.name, library.target_name);
      content.push_str("if(NOT TARGET ");
      content.push_str(&target);
      content.push(')');
      content.push_str(NativeCmakeDependencyConfigurator::eol());
      content.push_str("    add_library(");
      content.push_str(&target);
      content.push(' ');
      content.push_str(if library.is_static {
        "STATIC"
      } else {
        "SHARED"
      });
      content.push_str(" IMPORTED)");
      content.push_str(NativeCmakeDependencyConfigurator::eol());
      content.push_str("    set_target_properties(");
      content.push_str(&target);
      content.push_str(" PROPERTIES");
      content.push_str(NativeCmakeDependencyConfigurator::eol());
      if !includes.is_empty() {
        content.push_str("        INTERFACE_INCLUDE_DIRECTORIES \"");
        content.push_str(&includes.join(";"));
        content.push('"');
        content.push_str(NativeCmakeDependencyConfigurator::eol());
      }
      let location =
        ArtifactPathSerializer::path_to_slash_string(&library.path).map_err(|error| {
          NativeBuildError::ConfigureCmakeFailed {
            path: library.path.clone(),
            message: error.to_string(),
          }
        })?;
      content.push_str("        IMPORTED_LOCATION \"");
      content.push_str(&location);
      content.push('"');
      content.push_str(NativeCmakeDependencyConfigurator::eol());
      if let Some(link_libraries) = self.link_libraries.get(&library.name_on_disk) {
        content.push_str("        INTERFACE_LINK_LIBRARIES \"");
        content.push_str(&link_libraries.join(";"));
        content.push('"');
        content.push_str(NativeCmakeDependencyConfigurator::eol());
      }
      content.push(')');
      content.push_str(NativeCmakeDependencyConfigurator::eol());
      content.push_str("endif()");
      content.push_str(NativeCmakeDependencyConfigurator::eol());
      entries.push(content);
    }
    Ok(entries.join(NativeCmakeDependencyConfigurator::eol()))
  }

  fn is_excluded(rule: &NativeLibPackageSelection, library: &NativeCmakeLibrary) -> bool {
    rule
      .exclude
      .as_ref()
      .is_some_and(|exclude| exclude.iter().any(|name| name == &library.name_on_disk))
      || rule.exclude_pattern.as_ref().is_some_and(|patterns| {
        patterns.iter().any(|pattern| {
          HvigorNativeLibraryPattern::matches_filter_pattern(&library.path, pattern, true)
        })
      })
  }

  fn remote_link_libraries(node: &DependencyNodeSnapshot) -> BTreeMap<String, Vec<String>> {
    node
      .package
      .as_deref()
      .and_then(|package| package.package_json.extra.get("nativeComponents"))
      .and_then(serde_json::Value::as_array)
      .into_iter()
      .flatten()
      .filter_map(|component| {
        let name = component.get("name")?.as_str()?.to_string();
        let libraries = component
          .get("linkLibraries")?
          .as_array()?
          .iter()
          .filter_map(serde_json::Value::as_str)
          .map(str::to_string)
          .collect::<Vec<_>>();
        (!libraries.is_empty()).then_some((name, libraries))
      })
      .collect()
  }
}

struct NativeCmakeCodeModelReader<'a> {
  dependency: &'a NativeCmakeLocalDependency,
  abi: &'a str,
}

impl<'a> NativeCmakeCodeModelReader<'a> {
  fn new(dependency: &'a NativeCmakeLocalDependency, abi: &'a str) -> Self {
    Self { dependency, abi }
  }

  fn libraries(&self) -> NativeBuildResult<Vec<NativeCmakeLibrary>> {
    let work_dir = NativeModuleLayout::for_variant(
      &self.dependency.package_root,
      &self.dependency.product_name,
      &self.dependency.target_name,
    )
    .ninja_work_dir(&self.dependency.build_mode)
    .join(self.abi);
    let reply = work_dir.join(".cmake").join("api").join("v1").join("reply");
    if !reply.is_dir() {
      tracing::warn!(
        path = %reply.display(),
        "ConfigureCmake did not find a local dependency CMake code model"
      );
      return Ok(Vec::new());
    }
    let mut target_files = std::fs::read_dir(&reply)
      .map_err(|error| NativeBuildError::ConfigureCmakeFailed {
        path: reply.clone(),
        message: error.to_string(),
      })?
      .filter_map(Result::ok)
      .map(|entry| entry.path())
      .filter(|path| {
        path
          .file_name()
          .and_then(OsStr::to_str)
          .is_some_and(|name| name.starts_with("target-") && name.ends_with(".json"))
      })
      .collect::<Vec<_>>();
    target_files.sort();
    let mut libraries = Vec::new();
    for target_file in target_files {
      let bytes =
        std::fs::read(&target_file).map_err(|error| NativeBuildError::ConfigureCmakeFailed {
          path: target_file.clone(),
          message: error.to_string(),
        })?;
      let target: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|error| NativeBuildError::ConfigureCmakeFailed {
          path: target_file.clone(),
          message: error.to_string(),
        })?;
      if !matches!(
        target.get("type").and_then(serde_json::Value::as_str),
        Some("STATIC_LIBRARY" | "SHARED_LIBRARY")
      ) {
        continue;
      }
      for artifact in target
        .get("artifacts")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
      {
        let Some(path) = artifact.get("path").and_then(serde_json::Value::as_str) else {
          continue;
        };
        let path = PathBuf::from(path);
        let path = if path.is_absolute() {
          path
        } else {
          work_dir.join(path)
        };
        if let Some(library) = NativeCmakeLibrary::from_path(path) {
          libraries.push(library);
        }
      }
      if matches!(self.dependency.module_type, OhosModuleType::Shared) {
        self.collect_hsp_runtime_files(&target, &work_dir, &mut libraries);
      }
    }
    Ok(libraries)
  }

  fn collect_hsp_runtime_files(
    &self,
    target: &serde_json::Value,
    work_dir: &Path,
    libraries: &mut Vec<NativeCmakeLibrary>,
  ) {
    let sysroot = target
      .pointer("/link/sysroot/path")
      .and_then(serde_json::Value::as_str)
      .map(PathBuf::from);
    for fragment in target
      .pointer("/link/commandFragments")
      .and_then(serde_json::Value::as_array)
      .into_iter()
      .flatten()
      .filter(|fragment| {
        fragment.get("role").and_then(serde_json::Value::as_str) == Some("libraries")
      })
      .filter_map(|fragment| fragment.get("fragment").and_then(serde_json::Value::as_str))
    {
      if fragment.starts_with('-') || fragment.ends_with(".a") {
        continue;
      }
      let fragment_path = PathBuf::from(fragment);
      let path = if fragment_path.is_absolute() {
        fragment_path
      } else {
        work_dir.join(fragment_path)
      };
      if sysroot
        .as_ref()
        .is_some_and(|sysroot| path.starts_with(sysroot))
      {
        continue;
      }
      if let Some(library) = NativeCmakeLibrary::from_path(path) {
        libraries.push(library);
      }
    }
  }
}

struct NativeCmakeLibrary {
  name_on_disk: String,
  target_name: String,
  path: PathBuf,
  is_static: bool,
}

impl NativeCmakeLibrary {
  fn from_path(path: PathBuf) -> Option<Self> {
    let name_on_disk = path.file_name()?.to_str()?.to_string();
    let extension_index = name_on_disk
      .rfind(".so")
      .or_else(|| name_on_disk.rfind(".a"));
    let prefix = usize::from(name_on_disk.to_ascii_lowercase().starts_with("lib")) * 3;
    let target_name = extension_index.map_or_else(
      || name_on_disk.get(prefix..).unwrap_or_default().to_string(),
      |index| {
        name_on_disk
          .get(prefix..index)
          .unwrap_or_default()
          .to_string()
      },
    );
    if target_name.is_empty() {
      return None;
    }
    Some(Self {
      is_static: name_on_disk.contains(".a"),
      name_on_disk,
      target_name,
      path,
    })
  }
}

pub struct NativeLibCollector<'a> {
  module: &'a ModuleConfig,
  abi_filters: &'a [String],
  deps: &'a DependencyGraphSnapshot,
  product_name: String,
  target_name: String,
  policy: NativeLibCollectionPolicy,
  excluded_dependency_roots: FxHashSet<PathBuf>,
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

  pub fn collect(&self) -> NativeBuildResult<Vec<PathBuf>> {
    self.configure_cmake_dependencies()?;
    self.configure_native()?;
    self.build_native()?;
    let processed = self.process_libs()?;
    let strip_inputs = if self.is_binxo_enabled() {
      self.binxo_libs(&processed)?
    } else {
      processed
    };
    let stripped = self.strip_libs(&strip_inputs)?;
    self.cache_libs()?;
    self.validate_libs(&stripped)?;
    Ok(stripped)
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
    for warning in &native_out.warnings {
      tracing::warn!("{}", warning);
    }
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

  fn read_binxo_cache(&self) -> NativeBuildResult<NativeBinxoCacheSnapshot> {
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

  fn native_file_hash(path: &Path) -> NativeBuildResult<String> {
    NativeLibCacheWriter::file_hash(path)
  }

  fn remove_stale_native_outputs(
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

  fn has_cmake_argument(&self, expected: &str) -> bool {
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

  fn native_build_config(&self) -> NativeBuildConfig {
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

  fn layout(&self) -> NativeModuleLayout<'_> {
    NativeModuleLayout::for_variant(&self.module.src_path, &self.product_name, &self.target_name)
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
      push_default_libs_dir(root, NativeLibPackageInfo::dependency(node));
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

/// Owns hvigor's `ProcessLibs` output materialization. The selected source
/// files are hard-linked into `intermediates/libs/default`; copying is used
/// only when the source and build directories are on different filesystems.
struct NativeProcessLibWriter<'a> {
  layout: NativeModuleLayout<'a>,
}

impl<'a> NativeProcessLibWriter<'a> {
  fn new(module_root: &'a Path) -> Self {
    Self::for_variant(module_root, "default", "default")
  }

  fn for_variant(module_root: &'a Path, product_name: &'a str, target_name: &'a str) -> Self {
    Self {
      layout: NativeModuleLayout::for_variant(module_root, product_name, target_name),
    }
  }

  fn materialize(&self, libs: &[PathBuf]) -> NativeBuildResult<Vec<PathBuf>> {
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
struct NativeLibCache<'a> {
  libs: &'a BTreeMap<String, String>,
  stripped: &'a BTreeMap<String, String>,
  #[serde(rename = "debugSymbol", skip_serializing_if = "Option::is_none")]
  debug_symbol: Option<&'a arkdown_core::NativeDebugSymbolOption>,
}

#[derive(Serialize)]
struct NativeBinxoCache<'a> {
  libs: &'a BTreeMap<String, String>,
  binxo: &'a BTreeMap<String, String>,
  #[serde(rename = "binxoSymbol")]
  binxo_symbol: NativeBinxoSymbol<'a>,
}

#[derive(Serialize)]
struct NativeBinxoSymbol<'a> {
  #[serde(rename = "enableAsanBinxo")]
  enable_asan_binxo: bool,
  #[serde(rename = "excludeSoFromBinxo")]
  exclude_so_from_binxo: &'a [String],
}

#[derive(Debug, Default, Deserialize)]
struct NativeBinxoCacheSnapshot {
  #[serde(default)]
  libs: BTreeMap<String, String>,
  #[serde(default)]
  binxo: BTreeMap<String, String>,
  #[serde(rename = "binxoSymbol", default)]
  binxo_symbol: Option<NativeBinxoSymbolSnapshot>,
}

#[derive(Debug, Deserialize)]
struct NativeBinxoSymbolSnapshot {
  #[serde(rename = "enableAsanBinxo", default)]
  enable_asan_binxo: bool,
  #[serde(rename = "excludeSoFromBinxo", default)]
  exclude_so_from_binxo: Vec<String>,
}

/// Mirrors hvigor `CacheNativeLibs` and `LibsFileCacheUtil`.
///
/// This cache deliberately does not use `FileUtil.pipelineAsync` or
/// `ohos.processLib.optimization`; those belong to ProcessLibs task-input
/// fingerprinting. Native strip/binxo caches always hash the JavaScript
/// numeric sum `size + mtime.getTime()`.
struct NativeLibCacheWriter<'a> {
  layout: NativeModuleLayout<'a>,
  option: &'a NativeLibOption,
  binxo_valid: bool,
}

impl<'a> NativeLibCacheWriter<'a> {
  fn new(module_root: &'a Path, option: &'a NativeLibOption, binxo_valid: bool) -> Self {
    Self::for_variant(module_root, "default", "default", option, binxo_valid)
  }

  fn for_variant(
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

  fn write(&self) -> NativeBuildResult<()> {
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

  fn write_json<T: Serialize>(&self, cache_file: &Path, cache: &T) -> NativeBuildResult<()> {
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

  fn explicit_debug_symbol(&self) -> Option<&arkdown_core::NativeDebugSymbolOption> {
    self
      .option
      .debug_symbol_configured
      .then_some(&self.option.debug_symbol)
  }

  fn hash_directory(&self, root: &Path) -> NativeBuildResult<BTreeMap<String, String>> {
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

  fn file_hash(path: &Path) -> NativeBuildResult<String> {
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

struct NativeDependencyLibDir {
  libs_dir: PathBuf,
  package: NativeLibPackageInfo,
}

/// Mirrors hvigor's `DoNativeStrip` and `tasks/worker/native-strip.ts`.
///
/// The debug-symbol `exclude` list is evaluated relative to the processed
/// native-library root (the first ABI component). With `strip: true`, matching
/// files are copied and all others are stripped; with `strip: false`, the
/// matching files are stripped and all others are copied.
struct NativeStripProcessor<'a> {
  layout: NativeModuleLayout<'a>,
  llvm_strip: Option<PathBuf>,
  option: &'a NativeLibOption,
}

impl<'a> NativeStripProcessor<'a> {
  fn new(module_root: &'a Path, llvm_strip: Option<PathBuf>, option: &'a NativeLibOption) -> Self {
    Self::for_variant(module_root, "default", "default", llvm_strip, option)
  }

  fn for_variant(
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

  fn process(&self, libs: &[PathBuf]) -> NativeBuildResult<Vec<PathBuf>> {
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

  fn exclude_matchers(&self) -> NativeBuildResult<Vec<globset::GlobMatcher>> {
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

  fn should_strip(&self, excluded: bool) -> bool {
    if self.option.debug_symbol.strip {
      !excluded
    } else {
      excluded
    }
  }

  fn strip_or_copy(&self, source: &Path, destination: &Path) -> NativeBuildResult<()> {
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

  fn copy(&self, source: &Path, destination: &Path) -> NativeBuildResult<()> {
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

  fn remove_stale_outputs(
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

  fn should_replace_with_last(
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

  fn should_replace_with_first_or_override(&self, candidate: &NativeLibCandidate) -> bool {
    self.filter.enable_override
      || self.filter.pick_firsts.iter().any(|pattern| {
        HvigorNativeLibraryPattern::matches_filter_pattern(
          candidate.relative_path_as_path(),
          pattern,
          false,
        )
      })
  }

  fn replace_group(
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

  fn append_to_group(
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
    let Ok(left) = arkdown_path::AbsPath::canonicalize(self.path) else {
      return false;
    };
    let Ok(right) = arkdown_path::AbsPath::canonicalize(other) else {
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
    self.configure()?;
    self.build()?;
    self.scan()
  }

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
    let cmake_file = self.cmake_lists_path();
    let has_cmake = cmake_file.is_file();
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

  fn validate_cmake_script(&self) -> NativeBuildResult<()> {
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
  fn so_search_roots(&self) -> Vec<PathBuf> {
    self.layout().so_search_roots()
  }

  fn run_cmake_configure(&self) -> Result<(), String> {
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
      self.create_cmake_file_api_queries(&build_dir)?;
      let mut configure_cmd = self.cmake_configure_command(
        &toolchain,
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
    }
    Ok(())
  }

  fn ninja_command_plans(&self) -> NativeBuildResult<Vec<NativeCommandPlan>> {
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
  fn cmake_configure_command(
    &self,
    toolchain: &ResolvedNativeSdkToolchain<'_>,
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
      self.prefixed_path_argument("-DCMAKE_TOOLCHAIN_FILE=", toolchain.toolchain_file),
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

    let custom = self.custom_cmake_arguments(abi);
    let mut command = Command::new(toolchain.cmake);
    command
      .args(self.merged_cmake_arguments(defaults, custom))
      .current_dir(build_dir);
    command
  }

  /// Build the base Ninja command wrapped by hvigor's
  /// `CommandBuilderModifier`, before native targets are appended.
  fn ninja_command(&self, toolchain: &ResolvedNativeSdkToolchain<'_>, abi: &str) -> Command {
    self.ninja_command_plan(toolchain, abi).command()
  }

  fn ninja_command_plan(
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
  fn custom_cmake_arguments(&self, abi: &str) -> Vec<OsString> {
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

  /// Mirrors hvigor `CmakeUtil.mergeCommandLine`: native arguments are parsed
  /// first, the first value for a flag key wins, and remaining defaults retain
  /// insertion order.
  fn merged_cmake_arguments(
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

  fn normalized_cmake_argument(&self, argument: &OsStr) -> Option<(String, OsString)> {
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

  fn cmake_argument_identity(&self, argument: &str) -> Option<String> {
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

  fn prefixed_path_argument(&self, prefix: &str, path: &Path) -> OsString {
    let mut argument = OsString::from(prefix);
    argument.push(path.as_os_str());
    argument
  }

  fn cmake_build_type(&self) -> &'static str {
    match self.build_mode.as_str() {
      "debug" | "Debug" => "Debug",
      "release" | "Release" => self.release_cmake_build_type.as_str(),
      _ => "Debug",
    }
  }

  fn effective_build_mode(&self) -> &str {
    if self.build_mode.is_empty() {
      "debug"
    } else {
      &self.build_mode
    }
  }

  fn should_run_native_tasks(&self) -> bool {
    self.enable_cmake_build
      && (self
        .external_native_options
        .as_ref()
        .is_some_and(|options| options.path.is_some())
        || self.layout().cmake_lists().is_file())
  }

  fn create_cmake_file_api_queries(&self, build_dir: &Path) -> Result<(), String> {
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

  fn cmake_lists_path(&self) -> PathBuf {
    self
      .external_native_options
      .as_ref()
      .and_then(|options| options.path.clone())
      .unwrap_or_else(|| self.layout().cmake_lists())
  }

  fn layout(&self) -> NativeModuleLayout<'_> {
    NativeModuleLayout::for_variant(&self.module_root, &self.product_name, &self.target_name)
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
        module_profile: None,
        node_type: arkdown_session::dependency_graph::snapshot::DependencyNodeType::SourceHar,
        is_runtime_only: false,
        is_dynamic: false,
        is_transitive: false,
        direct_deps: Vec::new(),
      },
    );
  }

  #[test]
  fn hvigor_cmake_command_applies_native_overrides_before_defaults() {
    let tmp = tempfile::tempdir().unwrap();
    let module_root = tmp.path().join("entry");
    let build_dir = module_root.join(".cxx/default/default/debug/arm64-v8a");
    let output_dir = module_root.join("build/default/intermediates/cmake/default/obj/arm64-v8a");
    let find_root = module_root.join(".cxx/default/default/debug/hvigor/arm64-v8a");
    fs::create_dir_all(&build_dir).unwrap();
    fs::create_dir_all(&find_root).unwrap();
    fs::write(find_root.join("summary.cmake"), b"").unwrap();

    let cmake = tmp.path().join("sdk/native/build-tools/cmake/bin/cmake");
    let ninja = tmp.path().join("sdk/native/build-tools/cmake/bin/ninja");
    let native_dir = tmp.path().join("sdk/native");
    let toolchain_file = native_dir.join("build/cmake/ohos.toolchain.cmake");
    let toolchain = ResolvedNativeSdkToolchain {
      cmake: &cmake,
      ninja: &ninja,
      native_dir: &native_dir,
      toolchain_file: &toolchain_file,
    };
    let config = NativeBuildConfig {
      module_root,
      product_name: "default".to_string(),
      target_name: "default".to_string(),
      abi_filters: vec!["arm64-v8a".to_string()],
      enable_cmake_build: true,
      external_native_options: Some(ExternalNativeOptions {
        arguments: vec![
          "-DCMAKE_BUILD_TYPE=Custom -DFIRST=value".to_string(),
          "-DFIRST=ignored".to_string(),
        ],
        c_flags: Some("-fvisibility=hidden".to_string()),
        ..Default::default()
      }),
      sanitizers: NativeBuildSanitizers::new(true, false, false, false),
      build_mode: "debug".to_string(),
      sdk_toolchain: NativeSdkToolchain::default(),
      collect_all_libs: false,
      release_cmake_build_type: NativeReleaseCmakeBuildType::default(),
    };

    let command = config.cmake_configure_command(
      &toolchain,
      "arm64-v8a",
      &config.module_root.join("src/main/cpp"),
      &build_dir,
      &output_dir,
      &find_root,
    );
    let arguments = command
      .get_args()
      .map(|argument| argument.to_string_lossy().into_owned())
      .collect::<Vec<_>>();

    assert_eq!(command.get_program(), cmake.as_os_str());
    assert_eq!(command.get_current_dir(), Some(build_dir.as_path()));
    assert_eq!(
      &arguments[..3],
      [
        "-DCMAKE_BUILD_TYPE=Custom",
        "-DFIRST=value",
        "-DOHOS_ENABLE_ASAN=ON",
      ]
    );
    assert_eq!(
      arguments
        .iter()
        .filter(|argument| argument.starts_with("-DCMAKE_BUILD_TYPE="))
        .count(),
      1
    );
    assert!(arguments
      .iter()
      .any(|argument| argument.starts_with("-DPACKAGE_FIND_FILE=")));
    assert!(!arguments
      .iter()
      .any(|argument| argument.starts_with("-DOHOS_PACKAGE_FIND_SCRIPT=")));
    assert!(arguments.contains(&"-DCMAKE_C_FLAGS=-fvisibility=hidden".to_string()));
  }

  #[test]
  fn hvigor_ninja_command_uses_sdk_executable_without_implicit_parallel_flag() {
    let tmp = tempfile::tempdir().unwrap();
    let module_root = tmp.path().join("entry");
    let cmake = tmp.path().join("sdk/native/build-tools/cmake/bin/cmake");
    let ninja = tmp.path().join("sdk/native/build-tools/cmake/bin/ninja");
    let native_dir = tmp.path().join("sdk/native");
    let toolchain_file = native_dir.join("build/cmake/ohos.toolchain.cmake");
    let toolchain = ResolvedNativeSdkToolchain {
      cmake: &cmake,
      ninja: &ninja,
      native_dir: &native_dir,
      toolchain_file: &toolchain_file,
    };
    let config = NativeBuildConfig {
      module_root: module_root.clone(),
      product_name: "paid".to_string(),
      target_name: "phone".to_string(),
      abi_filters: vec!["arm64-v8a".to_string()],
      enable_cmake_build: true,
      external_native_options: Some(ExternalNativeOptions {
        targets: vec!["entry".to_string(), "helper".to_string()],
        ..Default::default()
      }),
      sanitizers: NativeBuildSanitizers::default(),
      build_mode: "release".to_string(),
      sdk_toolchain: NativeSdkToolchain::default(),
      collect_all_libs: false,
      release_cmake_build_type: NativeReleaseCmakeBuildType::default(),
    };

    let command = config.ninja_command(&toolchain, "arm64-v8a");
    let build_dir = module_root.join(".cxx/paid/phone/release/arm64-v8a");
    let arguments = command.get_args().collect::<Vec<_>>();

    assert_eq!(command.get_program(), ninja.as_os_str());
    assert_eq!(command.get_current_dir(), Some(build_dir.as_path()));
    assert_eq!(
      arguments,
      [
        OsStr::new("-C"),
        build_dir.as_os_str(),
        OsStr::new("entry"),
        OsStr::new("helper"),
      ]
    );
    assert!(!arguments
      .iter()
      .any(|argument| argument.to_string_lossy().starts_with("-j")));
  }

  #[test]
  fn hvigor_ninja_modifier_wraps_base_command_before_targets_are_appended() {
    let tmp = tempfile::tempdir().unwrap();
    let module_root = tmp.path().join("entry");
    let cmake = tmp.path().join("sdk/native/build-tools/cmake/bin/cmake");
    let ninja = tmp.path().join("sdk/native/build-tools/cmake/bin/ninja");
    let native_dir = tmp.path().join("sdk/native");
    let toolchain_file = native_dir.join("build/cmake/ohos.toolchain.cmake");
    let toolchain = ResolvedNativeSdkToolchain {
      cmake: &cmake,
      ninja: &ninja,
      native_dir: &native_dir,
      toolchain_file: &toolchain_file,
    };
    let config = NativeBuildConfig {
      module_root: module_root.clone(),
      product_name: "paid".to_string(),
      target_name: "phone".to_string(),
      abi_filters: vec!["arm64-v8a".to_string()],
      enable_cmake_build: true,
      external_native_options: Some(ExternalNativeOptions {
        targets: vec!["entry".to_string(), "helper".to_string()],
        ..Default::default()
      }),
      sanitizers: NativeBuildSanitizers::default(),
      build_mode: "release".to_string(),
      sdk_toolchain: NativeSdkToolchain::default(),
      collect_all_libs: false,
      release_cmake_build_type: NativeReleaseCmakeBuildType::default(),
    };
    let build_dir = module_root.join(".cxx/paid/phone/release/arm64-v8a");
    let plan = config.ninja_command_plan(&toolchain, "arm64-v8a");

    assert_eq!(
      plan.modifier_command().unwrap(),
      [
        ninja.to_string_lossy().into_owned(),
        "-C".to_string(),
        build_dir.to_string_lossy().into_owned(),
      ]
    );

    let modified = plan
      .with_modifier_command(vec![
        "native-command-wrapper".to_string(),
        "--profile".to_string(),
      ])
      .unwrap();
    let command = modified.command();

    assert_eq!(command.get_program(), OsStr::new("native-command-wrapper"));
    assert_eq!(command.get_current_dir(), Some(build_dir.as_path()));
    assert_eq!(
      command.get_args().collect::<Vec<_>>(),
      [
        OsStr::new("--profile"),
        OsStr::new("entry"),
        OsStr::new("helper"),
      ]
    );
  }

  #[test]
  fn hvigor_module_clean_removes_outputs_and_cmake_cache_cross_platform() {
    let tmp = tempfile::tempdir().unwrap();
    let project_root = tmp.path().join("project");
    let module_root = project_root.join("entry");
    let configured_build_root = tmp.path().join("custom-build");
    let configured_project_build = configured_build_root.join("project").join("build");
    let configured_module_build = configured_build_root
      .join("project")
      .join("entry")
      .join("build");
    let native_tree = module_root
      .join(".cxx/default/default/debug")
      .join("arm64-v8a");
    let inactive_native_tree = module_root
      .join(".cxx/default/default/release")
      .join("arm64-v8a");
    for directory in [
      project_root.join("build"),
      module_root.join("build"),
      module_root.join(".test"),
      configured_build_root.clone(),
      configured_project_build.clone(),
      configured_module_build.clone(),
      native_tree.clone(),
      inactive_native_tree.clone(),
    ] {
      fs::create_dir_all(directory).unwrap();
    }
    fs::write(project_root.join("build/project.bin"), b"project").unwrap();
    fs::write(module_root.join("build/module.bin"), b"module").unwrap();
    fs::write(module_root.join(".test/result.bin"), b"test").unwrap();
    fs::write(configured_project_build.join("project.bin"), b"project").unwrap();
    fs::write(configured_module_build.join("module.bin"), b"module").unwrap();
    fs::write(native_tree.join("CMakeCache.txt"), b"cache").unwrap();
    fs::write(native_tree.join("build.ninja"), b"ninja").unwrap();
    fs::write(native_tree.join("rules.ninja"), b"rules").unwrap();
    fs::write(inactive_native_tree.join("CMakeCache.txt"), b"cache").unwrap();
    fs::write(inactive_native_tree.join("build.ninja"), b"ninja").unwrap();
    fs::write(inactive_native_tree.join("rules.ninja"), b"rules").unwrap();

    let toolchain = NativeSdkToolchain::for_clean(Some(std::env::current_exe().unwrap()));
    HvigorModuleCleanStage::new(&project_root, &module_root, &toolchain)
      .with_configured_build_dir(Some(&configured_build_root))
      .unwrap()
      .execute()
      .expect("hvigor clean stage must remove build outputs");

    assert!(!project_root.join("build").exists());
    assert!(!module_root.join("build").exists());
    assert!(!module_root.join(".test").exists());
    assert!(!configured_project_build.exists());
    assert!(!configured_module_build.exists());
    assert!(!native_tree.join("CMakeCache.txt").exists());
    assert!(native_tree.join("build.ninja").is_file());
    // DevEco Studio 6.1.1 Clean.getArchDirectories only visits the active
    // build mode; a debug clean must not invalidate a release CMake cache.
    assert!(inactive_native_tree.join("CMakeCache.txt").is_file());
  }

  #[test]
  fn collect_debug_symbol_prefers_intermediate_map_and_copies_name_cache() {
    let tmp = tempfile::tempdir().unwrap();
    let module_root = tmp.path().join("entry");
    let source_map_intermediate = tmp.path().join("source_map/sourceMaps.map");
    let source_map_origin = tmp.path().join("cache/sourceMaps.map");
    let name_cache = tmp.path().join("cache/obfuscation/nameCache.json");
    let process_libs = NativeModuleLayout::new(&module_root)
      .process_libs_dir()
      .join("arm64-v8a");
    for directory in [
      source_map_intermediate.parent().unwrap(),
      source_map_origin.parent().unwrap(),
      name_cache.parent().unwrap(),
      process_libs.as_path(),
    ] {
      fs::create_dir_all(directory).unwrap();
    }
    fs::write(&source_map_intermediate, b"intermediate-map").unwrap();
    fs::write(&source_map_origin, b"origin-map").unwrap();
    fs::write(&name_cache, b"name-cache").unwrap();
    fs::write(process_libs.join("libentry.so"), b"symbol").unwrap();

    let output = DebugSymbolCollector::new(
      module_root,
      "entry".to_string(),
      BuildMode::Release,
      source_map_intermediate,
      Some(source_map_origin),
      name_cache,
    )
    .collect()
    .expect("CollectDebugSymbol must materialize release artifacts");

    assert_eq!(
      fs::read(output.source_map.unwrap()).unwrap(),
      b"intermediate-map"
    );
    assert_eq!(fs::read(output.name_cache.unwrap()).unwrap(), b"name-cache");
    #[cfg(unix)]
    assert!(fs::symlink_metadata(&output.native_lib_links[0])
      .unwrap()
      .file_type()
      .is_symlink());
  }

  #[test]
  fn collect_debug_symbol_is_disabled_by_default_for_debug_builds() {
    let tmp = tempfile::tempdir().unwrap();
    let module_root = tmp.path().join("entry");
    let output = DebugSymbolCollector::new(
      module_root.clone(),
      "entry".to_string(),
      BuildMode::Debug,
      tmp.path().join("sourceMaps.map"),
      None,
      tmp.path().join("nameCache.json"),
    )
    .collect()
    .expect("debug CollectDebugSymbol should be skipped");

    assert_eq!(output, DebugSymbolOutput::default());
    assert!(!module_root
      .join("build/default/outputs/default/symbol")
      .exists());
  }

  #[test]
  fn project_debug_symbols_link_modules_and_build_app_symbol_zip() {
    let tmp = tempfile::tempdir().unwrap();
    let module_root = tmp.path().join("entry");
    let module_symbols = module_root.join("build/default/outputs/default/symbol/release");
    fs::create_dir_all(&module_symbols).unwrap();
    fs::write(module_symbols.join("entry-sourceMaps.map"), b"source-map").unwrap();
    let modules = vec![ModuleConfig {
      name: "entry".to_string(),
      src_path: module_root,
      module_type: OhosModuleType::Entry,
    }];

    let output = ProjectDebugSymbolCollector::new(
      tmp.path().to_path_buf(),
      "default".to_string(),
      "default".to_string(),
      BuildMode::Release,
      modules,
      None,
    )
    .collect()
    .expect("PackageApp must aggregate project debug symbols");

    let archive_path = output.archive.expect("app-symbol.zip");
    assert_eq!(
      archive_path,
      tmp
        .path()
        .join("build/outputs/default/symbol/release/app-symbol.zip")
    );
    let file = fs::File::open(archive_path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let mut source_map = Vec::new();
    archive
      .by_name("entry/entry-sourceMaps.map")
      .unwrap()
      .read_to_end(&mut source_map)
      .unwrap();
    assert_eq!(source_map, b"source-map");
    assert!(!tmp
      .path()
      .join("build/outputs/default/symbol/temp")
      .exists());
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
  fn native_intermediates_follow_selected_product_and_target() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = NativeModuleLayout::for_variant(tmp.path(), "paid", "release");

    assert_eq!(
      layout.cmake_obj_dir(),
      tmp
        .path()
        .join("build")
        .join("paid")
        .join("intermediates")
        .join("cmake")
        .join("release")
        .join("obj")
    );
    assert_eq!(
      layout.stripped_libs_dir(),
      tmp
        .path()
        .join("build")
        .join("paid")
        .join("intermediates")
        .join("stripped_native_libs")
        .join("release")
    );
    assert_eq!(
      layout.native_lib_cache_file(),
      tmp
        .path()
        .join("build")
        .join("paid")
        .join("intermediates")
        .join("patch")
        .join("release")
        .join("base_native_libs.json")
    );
  }

  #[test]
  fn configure_cmake_materializes_remote_package_configs_in_hvigor_work_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let module_root = tmp.path().join("entry");
    let cpp_dir = module_root.join("src").join("main").join("cpp");
    fs::create_dir_all(&cpp_dir).unwrap();
    fs::write(cpp_dir.join("CMakeLists.txt"), "project(entry)").unwrap();

    let dependency_root = tmp.path().join("oh_modules").join("@scope").join("native");
    let dependency_lib = dependency_root
      .join("libs")
      .join("arm64-v8a")
      .join("libnative.so");
    let dependency_header = dependency_root.join("include").join("native.h");
    fs::create_dir_all(dependency_lib.parent().unwrap()).unwrap();
    fs::create_dir_all(dependency_header.parent().unwrap()).unwrap();
    fs::write(&dependency_lib, b"ELF").unwrap();
    fs::write(dependency_header, b"#pragma once").unwrap();

    let module = ModuleConfig {
      name: "entry".to_string(),
      src_path: module_root.clone(),
      module_type: OhosModuleType::Entry,
    };
    let mut dependencies = DependencyGraphSnapshot::default();
    add_dependency_node(
      &mut dependencies,
      "@scope/native",
      "@scope/native",
      "1.2.3",
      dependency_root.clone(),
    );
    let policy = NativeLibCollectionPolicy::default()
      .with_build_mode("release".to_string())
      .with_external_native_options(Some(ExternalNativeOptions::default()));

    NativeLibCollector::new(&module, &["arm64-v8a".to_string()], &dependencies)
      .with_output_variant("paid", "phone")
      .with_policy(policy)
      .configure_cmake_dependencies()
      .expect("ConfigureCmake must materialize remote package metadata");

    let abi_root = module_root
      .join(".cxx")
      .join("paid")
      .join("phone")
      .join("release")
      .join("hvigor")
      .join("arm64-v8a");
    let config_path = abi_root
      .join("lib")
      .join("aarch64-linux-ohos")
      .join("cmake")
      .join("native")
      .join("nativeConfig.cmake");
    let config = fs::read_to_string(&config_path).unwrap();
    let serialized_lib =
      ArtifactPathSerializer::path_to_slash_string(&canonicalize_or_owned(&dependency_lib))
        .unwrap();
    assert!(config.contains("add_library(native::native SHARED IMPORTED)"));
    assert!(config.contains(&format!("IMPORTED_LOCATION \"{serialized_lib}\"")));
    assert!(config.contains("INTERFACE_INCLUDE_DIRECTORIES"));

    let serialized_config = ArtifactPathSerializer::path_to_slash_string(&config_path).unwrap();
    assert_eq!(
      fs::read_to_string(abi_root.join("summary.cmake")).unwrap(),
      format!(
        "if(EXISTS {serialized_config}){eol}    find_package(native REQUIRED CONFIG){eol}endif(){eol}",
        eol = NativeCmakeDependencyConfigurator::eol(),
      )
    );
  }

  #[test]
  fn configure_cmake_reads_local_code_model_headers_and_link_interface() {
    let tmp = tempfile::tempdir().unwrap();
    let module_root = tmp.path().join("entry");
    let cpp_dir = module_root.join("src").join("main").join("cpp");
    fs::create_dir_all(&cpp_dir).unwrap();
    fs::write(cpp_dir.join("CMakeLists.txt"), "project(entry)").unwrap();

    let dependency_root = tmp.path().join("native_module");
    let header = dependency_root.join("public").join("native.h");
    fs::create_dir_all(header.parent().unwrap()).unwrap();
    fs::write(header, b"#pragma once").unwrap();
    let dependency_work_dir = dependency_root
      .join(".cxx")
      .join("paid")
      .join("phone")
      .join("release")
      .join("arm64-v8a");
    let reply_dir = dependency_work_dir
      .join(".cmake")
      .join("api")
      .join("v1")
      .join("reply");
    fs::create_dir_all(&reply_dir).unwrap();
    let dependency_lib = dependency_work_dir.join("out").join("libnative.so");
    fs::write(
      reply_dir.join("target-native.json"),
      serde_json::to_vec(&serde_json::json!({
        "type": "SHARED_LIBRARY",
        "artifacts": [{ "path": dependency_lib }]
      }))
      .unwrap(),
    )
    .unwrap();

    let module = ModuleConfig {
      name: "entry".to_string(),
      src_path: module_root.clone(),
      module_type: OhosModuleType::Entry,
    };
    let mut dependencies = DependencyGraphSnapshot::default();
    add_dependency_node(
      &mut dependencies,
      "native_module",
      "native_module",
      "1.0.0",
      dependency_root.clone(),
    );
    let native_lib = NativeLibOption {
      header_paths: vec![PathBuf::from("public")],
      libraries_info: vec![arkdown_core::NativeLibraryInfo {
        name: "libnative.so".to_string(),
        link_libraries: vec!["hilog_ndk.z".to_string()],
      }],
      ..Default::default()
    };
    let local_dependency = NativeCmakeLocalDependency::new(
      dependency_root,
      "paid".to_string(),
      "phone".to_string(),
      "release".to_string(),
      OhosModuleType::Har,
      native_lib,
    );
    let policy = NativeLibCollectionPolicy::default()
      .with_build_mode("release".to_string())
      .with_external_native_options(Some(ExternalNativeOptions::default()))
      .with_cmake_local_dependencies(vec![local_dependency]);

    NativeLibCollector::new(&module, &["arm64-v8a".to_string()], &dependencies)
      .with_output_variant("paid", "phone")
      .with_policy(policy)
      .configure_cmake_dependencies()
      .expect("ConfigureCmake must resolve local CMake code-model outputs");

    let config_path = module_root
      .join(".cxx")
      .join("paid")
      .join("phone")
      .join("release")
      .join("hvigor")
      .join("arm64-v8a")
      .join("lib")
      .join("aarch64-linux-ohos")
      .join("cmake")
      .join("native_module")
      .join("native_moduleConfig.cmake");
    let config = fs::read_to_string(config_path).unwrap();
    assert!(config.contains("add_library(native_module::native SHARED IMPORTED)"));
    assert!(config.contains("INTERFACE_LINK_LIBRARIES \"hilog_ndk.z\""));
    assert!(config.contains("public"));
  }

  #[test]
  fn configure_cmake_rejects_unknown_cpu_abi_like_hvigor() {
    let tmp = tempfile::tempdir().unwrap();
    let module_root = tmp.path().join("entry");
    let cpp_dir = module_root.join("src").join("main").join("cpp");
    fs::create_dir_all(&cpp_dir).unwrap();
    fs::write(cpp_dir.join("CMakeLists.txt"), "project(entry)").unwrap();
    let module = ModuleConfig {
      name: "entry".to_string(),
      src_path: module_root,
      module_type: OhosModuleType::Entry,
    };
    let dependencies = DependencyGraphSnapshot::default();
    let policy = NativeLibCollectionPolicy::default()
      .with_external_native_options(Some(ExternalNativeOptions::default()));

    let error = NativeLibCollector::new(&module, &["mips64".to_string()], &dependencies)
      .with_policy(policy)
      .configure_cmake_dependencies()
      .expect_err("unknown ABI must not silently fall back to arm64-v8a");

    assert!(matches!(
      error,
      NativeBuildError::UnknownCpuAbi { ref abi } if abi == "mips64"
    ));
  }

  #[test]
  fn test_native_import_collects_prebuilt_sos() {
    let tmp = tempfile::tempdir().unwrap();
    let libs_dir = tmp.path().join("libs/arm64-v8a");
    fs::create_dir_all(&libs_dir).unwrap();
    fs::write(libs_dir.join("libnative.so"), b"ELF").unwrap();

    let cfg = NativeBuildConfig {
      module_root: tmp.path().to_path_buf(),
      product_name: "default".to_string(),
      target_name: "default".to_string(),
      abi_filters: vec!["arm64-v8a".to_string()],
      enable_cmake_build: false,
      external_native_options: None,
      sanitizers: NativeBuildSanitizers::default(),
      build_mode: "debug".to_string(),
      sdk_toolchain: NativeSdkToolchain::default(),
      collect_all_libs: false,
      release_cmake_build_type: NativeReleaseCmakeBuildType::default(),
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
      product_name: "default".to_string(),
      target_name: "default".to_string(),
      abi_filters: vec!["arm64-v8a".to_string()],
      enable_cmake_build: false,
      external_native_options: None,
      sanitizers: NativeBuildSanitizers::default(),
      build_mode: "debug".to_string(),
      sdk_toolchain: NativeSdkToolchain::default(),
      collect_all_libs: false,
      release_cmake_build_type: NativeReleaseCmakeBuildType::default(),
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
      product_name: "default".to_string(),
      target_name: "default".to_string(),
      abi_filters: vec!["arm64-v8a".to_string()],
      enable_cmake_build: false,
      external_native_options: None,
      sanitizers: NativeBuildSanitizers::default(),
      build_mode: "debug".to_string(),
      sdk_toolchain: NativeSdkToolchain::default(),
      collect_all_libs: false,
      release_cmake_build_type: NativeReleaseCmakeBuildType::default(),
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
      product_name: "default".to_string(),
      target_name: "default".to_string(),
      abi_filters: vec!["arm64-v8a".to_string()],
      enable_cmake_build: false,
      external_native_options: None,
      sanitizers: NativeBuildSanitizers::default(),
      build_mode: "debug".to_string(),
      sdk_toolchain: NativeSdkToolchain::default(),
      collect_all_libs: false,
      release_cmake_build_type: NativeReleaseCmakeBuildType::default(),
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
      product_name: "default".to_string(),
      target_name: "default".to_string(),
      abi_filters: vec!["arm64-v8a".to_string()],
      enable_cmake_build: false,
      external_native_options: None,
      sanitizers: NativeBuildSanitizers::default(),
      build_mode: "debug".to_string(),
      sdk_toolchain: NativeSdkToolchain::default(),
      collect_all_libs: false,
      release_cmake_build_type: NativeReleaseCmakeBuildType::default(),
    };
    let out = cfg.run().expect("native scan");
    assert!(out.libs_by_abi.is_empty());
  }

  #[test]
  fn test_asan_cmake_arg_applies_to_all_abis() {
    assert_eq!(
      NativeBuildSanitizers::new(true, false, false, false)
        .cmake_args_for_abi("arm64-v8a")
        .collect::<Vec<_>>(),
      ["-DOHOS_ENABLE_ASAN=ON"]
    );
    assert_eq!(
      NativeBuildSanitizers::new(true, false, false, false)
        .cmake_args_for_abi("x86_64")
        .collect::<Vec<_>>(),
      ["-DOHOS_ENABLE_ASAN=ON"]
    );
  }

  #[test]
  fn test_hwasan_cmake_arg_only_applies_to_arm64() {
    assert_eq!(
      NativeBuildSanitizers::new(false, false, true, false)
        .cmake_args_for_abi("arm64-v8a")
        .collect::<Vec<_>>(),
      ["-DOHOS_ENABLE_HWASAN=ON"]
    );
    assert_eq!(
      NativeBuildSanitizers::new(false, false, true, false)
        .cmake_args_for_abi("x86_64")
        .collect::<Vec<_>>(),
      Vec::<&str>::new()
    );
  }

  #[test]
  fn test_hvigor_forwards_every_enabled_sanitizer_flag() {
    assert_eq!(
      NativeBuildSanitizers::new(true, true, true, true)
        .cmake_args_for_abi("arm64-v8a")
        .collect::<Vec<_>>(),
      [
        "-DOHOS_ENABLE_ASAN=ON",
        "-DOHOS_ENABLE_TSAN=ON",
        "-DOHOS_ENABLE_HWASAN=ON",
        "-DOHOS_ENABLE_UBSAN=ON",
      ]
    );
  }

  #[test]
  fn binxo_command_matches_hvigor_native_binxo_worker_arguments() {
    let command = NativeBinxoCommand::new(
      Path::new("binxo"),
      Path::new("input/libentry.so"),
      Path::new("output/libentry.so"),
    )
    .command();

    assert_eq!(
      command.get_args().collect::<Vec<_>>(),
      vec![
        OsStr::new("-A"),
        OsStr::new("--hwasan-inputfile"),
        OsStr::new("input/libentry.so"),
        OsStr::new("--hwasan-outputfil"),
        OsStr::new("output/libentry.so"),
      ]
    );
  }

  #[test]
  fn binxo_validation_uses_truthy_extra_config_before_cmake_fallback() {
    let tmp = tempfile::tempdir().unwrap();
    let module = ModuleConfig {
      name: "entry".to_string(),
      src_path: tmp.path().join("entry"),
      module_type: arkdown_core::OhosModuleType::Entry,
    };
    let deps = DependencyGraphSnapshot::default();
    let cmake = Some(ExternalNativeOptions {
      arguments: vec![
        "-DOHOS_ENABLE_HWASAN=ON".to_string(),
        "-DOHOS_ENABLE_BINXO=ON".to_string(),
      ],
      ..Default::default()
    });

    let fallback = NativeLibCollector::new(&module, &[], &deps).with_policy(
      NativeLibCollectionPolicy::default().with_external_native_options(cmake.clone()),
    );
    assert!(fallback.is_binxo_enabled());
    assert!(fallback.is_binxo_valid());

    let disabled = NativeLibCollector::new(&module, &[], &deps).with_policy(
      NativeLibCollectionPolicy::default()
        .with_external_native_options(cmake)
        .with_binxo_config(Some(false), Some(true)),
    );
    assert!(disabled.is_binxo_enabled());
    assert!(!disabled.is_binxo_valid());
    assert!(matches!(
      disabled.binxo_libs(&[]),
      Err(NativeBuildError::BinxoRequiresHwasan { .. })
    ));
  }

  #[test]
  fn binxo_reuses_hvigor_file_cache_without_reexecuting_the_tool() {
    let tmp = tempfile::tempdir().unwrap();
    let module_root = tmp.path().join("entry");
    let module = ModuleConfig {
      name: "entry".to_string(),
      src_path: module_root.clone(),
      module_type: arkdown_core::OhosModuleType::Entry,
    };
    let deps = DependencyGraphSnapshot::default();
    let layout = NativeModuleLayout::new(&module_root);
    let source = layout.process_libs_dir().join("arm64-v8a/libentry.so");
    let destination = layout.binxo_libs_dir().join("arm64-v8a/libentry.so");
    for path in [&source, &destination] {
      fs::create_dir_all(path.parent().unwrap()).unwrap();
    }
    fs::write(&source, b"source").unwrap();
    fs::write(&destination, b"cached-binxo").unwrap();
    let libs = BTreeMap::from([(
      source.to_string_lossy().into_owned(),
      NativeLibCacheWriter::file_hash(&source).unwrap(),
    )]);
    let binxo = BTreeMap::from([(
      destination.to_string_lossy().into_owned(),
      NativeLibCacheWriter::file_hash(&destination).unwrap(),
    )]);
    let cache = NativeBinxoCache {
      libs: &libs,
      binxo: &binxo,
      binxo_symbol: NativeBinxoSymbol {
        enable_asan_binxo: true,
        exclude_so_from_binxo: &[],
      },
    };
    fs::create_dir_all(layout.binxo_native_lib_cache_file().parent().unwrap()).unwrap();
    fs::write(
      layout.binxo_native_lib_cache_file(),
      serde_json::to_vec(&cache).unwrap(),
    )
    .unwrap();

    let policy = NativeLibCollectionPolicy::default()
      .with_external_native_options(Some(ExternalNativeOptions {
        arguments: vec!["-DOHOS_ENABLE_HWASAN=ON -DOHOS_ENABLE_BINXO=ON".to_string()],
        ..Default::default()
      }))
      .with_sdk_toolchain(NativeSdkToolchain {
        binxo: Some(std::env::current_exe().unwrap()),
        ..Default::default()
      });
    let outputs = NativeLibCollector::new(&module, &[], &deps)
      .with_policy(policy)
      .binxo_libs(std::slice::from_ref(&source))
      .unwrap();

    assert_eq!(outputs, [destination.clone()]);
    assert_eq!(fs::read(destination).unwrap(), b"cached-binxo");
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
      product_name: "default".to_string(),
      target_name: "default".to_string(),
      abi_filters: vec![],
      enable_cmake_build: false,
      external_native_options: None,
      sanitizers: NativeBuildSanitizers::default(),
      build_mode: "debug".to_string(),
      sdk_toolchain: NativeSdkToolchain::default(),
      collect_all_libs: false,
      release_cmake_build_type: NativeReleaseCmakeBuildType::default(),
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
  fn test_native_collector_materializes_and_caches_hvigor_native_stages() {
    let tmp = tempfile::tempdir().unwrap();
    let module_root = tmp.path().join("entry");
    let source = module_root.join("libs/arm64-v8a/libnative.so");
    fs::create_dir_all(source.parent().unwrap()).unwrap();
    fs::write(&source, b"ELF native").unwrap();

    let module = ModuleConfig {
      name: "entry".to_string(),
      src_path: module_root.clone(),
      module_type: arkdown_core::OhosModuleType::Entry,
    };
    let deps = DependencyGraphSnapshot::default();
    let packaged = NativeLibCollector::new(&module, &[], &deps)
      .collect()
      .expect("native stages must succeed");

    let layout = NativeModuleLayout::new(&module_root);
    let processed = layout.process_libs_dir().join("arm64-v8a/libnative.so");
    let stripped = layout.stripped_libs_dir().join("arm64-v8a/libnative.so");
    assert_eq!(packaged, vec![stripped.clone()]);
    assert_eq!(fs::read(processed).unwrap(), b"ELF native");
    assert_eq!(fs::read(stripped).unwrap(), b"ELF native");

    let cache: serde_json::Value =
      serde_json::from_slice(&fs::read(layout.native_lib_cache_file()).unwrap()).unwrap();
    assert_eq!(cache["libs"].as_object().unwrap().len(), 1);
    assert_eq!(cache["stripped"].as_object().unwrap().len(), 1);
    assert!(cache.get("debugSymbol").is_none());
    let binxo_cache: serde_json::Value =
      serde_json::from_slice(&fs::read(layout.binxo_native_lib_cache_file()).unwrap()).unwrap();
    assert_eq!(binxo_cache["libs"].as_object().unwrap().len(), 1);
    assert!(binxo_cache["binxo"].as_object().unwrap().is_empty());
    assert_eq!(binxo_cache["binxoSymbol"]["enableAsanBinxo"], false);
    assert_eq!(
      binxo_cache["binxoSymbol"]["excludeSoFromBinxo"],
      serde_json::json!([])
    );
    for hash in cache["libs"]
      .as_object()
      .unwrap()
      .values()
      .chain(cache["stripped"].as_object().unwrap().values())
    {
      let hash = hash.as_str().unwrap();
      assert_eq!(hash.len(), 64);
      assert!(hash.bytes().all(|byte| byte.is_ascii_hexdigit()));
    }
  }

  #[test]
  fn test_native_cache_empty_artifacts_match_latest_hvigor_bytes() {
    let tmp = tempfile::tempdir().unwrap();
    let option = NativeLibOption::default();

    NativeLibCacheWriter::new(tmp.path(), &option, false)
      .write()
      .expect("write empty native caches");

    let layout = NativeModuleLayout::new(tmp.path());
    assert_eq!(
      fs::read_to_string(layout.native_lib_cache_file()).unwrap(),
      r#"{"libs":{},"stripped":{}}"#
    );
    assert_eq!(
      fs::read_to_string(layout.binxo_native_lib_cache_file()).unwrap(),
      r#"{"libs":{},"binxo":{},"binxoSymbol":{"enableAsanBinxo":false,"excludeSoFromBinxo":[]}}"#
    );
  }

  #[test]
  fn native_cache_uses_libs_file_cache_numeric_size_plus_mtime_hash() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("libs");
    fs::create_dir_all(&root).unwrap();
    let library = root.join("libnative.so");
    fs::write(&library, b"ELF native").unwrap();
    let option = NativeLibOption::default();

    let hashes = NativeLibCacheWriter::new(tmp.path(), &option, false)
      .hash_directory(&root)
      .unwrap();
    let metadata = fs::metadata(&library).unwrap();
    let modified_ms = metadata
      .modified()
      .unwrap()
      .duration_since(UNIX_EPOCH)
      .unwrap()
      .as_millis();
    let fingerprint = (u128::from(metadata.len()) + modified_ms).to_string();
    let expected_hash = format!("{:x}", Sha256::digest(fingerprint.as_bytes()));
    assert_eq!(
      hashes.get(&library.to_string_lossy().into_owned()),
      Some(&expected_hash)
    );
  }

  #[test]
  fn native_cache_uses_binxo_as_strip_input_only_when_binxo_is_valid() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = NativeModuleLayout::new(tmp.path());
    let process = layout.process_libs_dir().join("arm64-v8a/libnative.so");
    let binxo = layout.binxo_libs_dir().join("arm64-v8a/libnative.so");
    let stripped = layout.stripped_libs_dir().join("arm64-v8a/libnative.so");
    for (path, bytes) in [
      (&process, b"process".as_slice()),
      (&binxo, b"binxo".as_slice()),
      (&stripped, b"stripped".as_slice()),
    ] {
      fs::create_dir_all(path.parent().unwrap()).unwrap();
      fs::write(path, bytes).unwrap();
    }
    let option = NativeLibOption::default();

    NativeLibCacheWriter::new(tmp.path(), &option, true)
      .write()
      .unwrap();
    let strip_cache: serde_json::Value =
      serde_json::from_slice(&fs::read(layout.native_lib_cache_file()).unwrap()).unwrap();
    let binxo_cache: serde_json::Value =
      serde_json::from_slice(&fs::read(layout.binxo_native_lib_cache_file()).unwrap()).unwrap();

    assert!(strip_cache["libs"]
      .get(binxo.to_string_lossy().as_ref())
      .is_some());
    assert!(strip_cache["libs"]
      .get(process.to_string_lossy().as_ref())
      .is_none());
    assert!(binxo_cache["libs"]
      .get(process.to_string_lossy().as_ref())
      .is_some());
    assert_eq!(binxo_cache["binxoSymbol"]["enableAsanBinxo"], true);
  }

  #[test]
  fn release_cmake_build_type_accepts_only_hvigor_values() {
    assert_eq!(
      NativeReleaseCmakeBuildType::from_hvigor_property(Some("Debug")).as_str(),
      "Debug"
    );
    assert_eq!(
      NativeReleaseCmakeBuildType::from_hvigor_property(Some("RelWithDebInfo")).as_str(),
      "RelWithDebInfo"
    );
    assert_eq!(
      NativeReleaseCmakeBuildType::from_hvigor_property(Some("MinSizeRel")).as_str(),
      "Release"
    );
  }

  #[test]
  fn test_native_collector_removes_stale_process_and_stripped_outputs() {
    let tmp = tempfile::tempdir().unwrap();
    let module_root = tmp.path().join("entry");
    let libs_dir = module_root.join("libs/arm64-v8a");
    fs::create_dir_all(&libs_dir).unwrap();
    let stale_source = libs_dir.join("libstale.so");
    fs::write(&stale_source, b"stale").unwrap();

    let module = ModuleConfig {
      name: "entry".to_string(),
      src_path: module_root.clone(),
      module_type: arkdown_core::OhosModuleType::Entry,
    };
    let deps = DependencyGraphSnapshot::default();
    NativeLibCollector::new(&module, &[], &deps)
      .collect()
      .expect("first native collection must succeed");

    fs::remove_file(stale_source).unwrap();
    fs::write(libs_dir.join("libcurrent.so"), b"current").unwrap();
    NativeLibCollector::new(&module, &[], &deps)
      .collect()
      .expect("second native collection must succeed");

    let layout = NativeModuleLayout::new(&module_root);
    assert!(!layout
      .process_libs_dir()
      .join("arm64-v8a/libstale.so")
      .exists());
    assert!(!layout
      .stripped_libs_dir()
      .join("arm64-v8a/libstale.so")
      .exists());
    assert!(layout
      .stripped_libs_dir()
      .join("arm64-v8a/libcurrent.so")
      .is_file());
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
          ..Default::default()
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
          ..Default::default()
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
          ..Default::default()
        }),
      )
      .collect()
      .expect("nativeLib.filter.select must be applied like hvigor ProcessLibs");
    assert_eq!(libs.len(), 1);
    assert_eq!(libs[0].file_name().unwrap(), "libinclude.so");
  }

  #[test]
  fn test_native_collector_applies_order_sensitive_filters_in_dependency_edge_order() {
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
    add_dependency_node(&mut deps, "a", "a", "1.0.0", dep_a.clone());
    add_dependency_node(&mut deps, "b", "b", "1.0.0", dep_b.clone());
    // The BTreeMap node fallback would visit `a`, then `b`. Latest hvigor
    // consumes dependency traversal order instead, so make it observably
    // different here: `b`, then `a`.
    for (alias, root) in [("b", dep_b), ("a", dep_a)] {
      deps.edges.push(
        arkdown_session::dependency_graph::snapshot::DependencyEdgeSnapshot {
          issuer_root: tmp.path().join("entry"),
          alias: alias.to_string(),
          version_spec: Some("1.0.0".to_string()),
          package_name: alias.to_string(),
          package_root: root,
          kind: arkdown_session::dependency_graph::snapshot::DependencyEdgeKind::Dependency,
          is_transitive: false,
        },
      );
    }

    let libs = NativeLibCollector::new(&module, &[], &deps)
      .with_policy(
        NativeLibCollectionPolicy::default().with_native_lib_option(NativeLibOption {
          collect_all_libs: false,
          filter: NativeLibFilterOption {
            enable_override: true,
            ..Default::default()
          },
          ..Default::default()
        }),
      )
      .collect()
      .expect("enableOverride must use latest hvigor dependency traversal order");
    assert_eq!(libs.len(), 1);
    assert_eq!(fs::read(&libs[0]).unwrap(), b"ELF A");

    let libs = NativeLibCollector::new(&module, &[], &deps)
      .with_policy(
        NativeLibCollectionPolicy::default().with_native_lib_option(NativeLibOption {
          collect_all_libs: false,
          filter: NativeLibFilterOption {
            pick_lasts: vec!["**/libdup.so".to_string()],
            ..Default::default()
          },
          ..Default::default()
        }),
      )
      .collect()
      .expect("pickLasts must use latest hvigor dependency traversal order");
    assert_eq!(libs.len(), 1);
    assert_eq!(fs::read(&libs[0]).unwrap(), b"ELF A");
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

  #[test]
  fn test_native_strip_debug_symbol_exclude_inverts_with_strip_flag_like_hvigor() {
    let tmp = tempfile::tempdir().unwrap();
    let option = NativeLibOption {
      debug_symbol: arkdown_core::NativeDebugSymbolOption {
        strip: true,
        exclude: vec!["**/libkeep-symbols.so".to_string()],
      },
      ..Default::default()
    };
    let processor = NativeStripProcessor::new(tmp.path(), None, &option);
    let matchers = processor.exclude_matchers().expect("valid glob");
    assert!(matchers[0].is_match("arm64-v8a/libkeep-symbols.so"));
    assert!(!processor.should_strip(true));
    assert!(processor.should_strip(false));

    let inverted = NativeLibOption {
      debug_symbol: arkdown_core::NativeDebugSymbolOption {
        strip: false,
        exclude: vec!["**/libstrip-symbols.so".to_string()],
      },
      ..Default::default()
    };
    let processor = NativeStripProcessor::new(tmp.path(), None, &inverted);
    assert!(processor.should_strip(true));
    assert!(!processor.should_strip(false));
  }

  #[test]
  fn test_har_excludes_dependency_native_libs_by_default_like_hvigor() {
    let tmp = tempfile::tempdir().unwrap();
    let module_root = tmp.path().join("library");
    let dep_root = tmp.path().join("oh_modules/native_pkg");
    fs::create_dir_all(&module_root).unwrap();
    fs::create_dir_all(dep_root.join("libs/arm64-v8a")).unwrap();
    fs::write(dep_root.join("libs/arm64-v8a/libdependency.so"), b"ELF").unwrap();

    let module = ModuleConfig {
      name: "library".to_string(),
      src_path: module_root,
      module_type: arkdown_core::OhosModuleType::Har,
    };
    let mut deps = DependencyGraphSnapshot::default();
    add_dependency_node(&mut deps, "native_pkg", "native_pkg", "1.0.0", dep_root);

    let libs = NativeLibCollector::new(&module, &[], &deps)
      .collect()
      .expect("HAR dependency native libraries should be excluded by default");
    assert!(libs.is_empty());

    let libs = NativeLibCollector::new(&module, &[], &deps)
      .with_policy(
        NativeLibCollectionPolicy::default().with_native_lib_option(NativeLibOption {
          exclude_from_har: false,
          ..Default::default()
        }),
      )
      .collect()
      .expect("excludeFromHar=false should include dependency native libraries");
    assert_eq!(libs.len(), 1);
    assert_eq!(libs[0].file_name().unwrap(), "libdependency.so");
  }
}
