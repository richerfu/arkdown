//! Native build types, CMake discovery, sanitizers, and layout helpers.

use std::collections::BTreeMap;
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use arkdown_core::{
    BuildMode, ExternalNativeOptions, ModuleConfig, NativeLibOption, NativeLibPackageSelection,
    OhosModuleType,
};
use arkdown_path::{canonicalize_or_owned, ArtifactPathSerializer, HvigorNativeLibraryPattern};
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
    pub(crate) release_cmake_build_type: NativeReleaseCmakeBuildType,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum NativeReleaseCmakeBuildType {
    #[default]
    Release,
    Debug,
    RelWithDebInfo,
}

impl NativeReleaseCmakeBuildType {
    pub(crate) fn from_hvigor_property(value: Option<&str>) -> Self {
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

    pub(crate) fn as_str(self) -> &'static str {
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
    pub(crate) asan: bool,
    pub(crate) tsan: bool,
    pub(crate) hwasan: bool,
    pub(crate) ubsan: bool,
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

    pub(crate) fn cmake_args_for_abi(self, abi: &str) -> impl Iterator<Item = &'static str> {
        [
            self.asan.then_some("-DOHOS_ENABLE_ASAN=ON"),
            self.tsan.then_some("-DOHOS_ENABLE_TSAN=ON"),
            (self.hwasan && abi == "arm64-v8a").then_some("-DOHOS_ENABLE_HWASAN=ON"),
            self.ubsan.then_some("-DOHOS_ENABLE_UBSAN=ON"),
        ]
        .into_iter()
        .flatten()
    }

    pub(crate) fn hwasan(self) -> bool {
        self.hwasan
    }

    pub(super) fn from_cmake_arguments(arguments: &[OsString]) -> Self {
        Self {
            asan: Self::cmake_value(arguments, "OHOS_ENABLE_ASAN") == Some("ON"),
            tsan: Self::cmake_value(arguments, "OHOS_ENABLE_TSAN") == Some("ON"),
            hwasan: Self::cmake_value(arguments, "OHOS_ENABLE_HWASAN") == Some("ON"),
            ubsan: Self::cmake_value(arguments, "OHOS_ENABLE_UBSAN")
                .is_some_and(Self::cmake_truthy),
        }
    }

    pub(super) fn cmake_value<'a>(arguments: &'a [OsString], key: &str) -> Option<&'a str> {
        arguments.iter().find_map(|argument| {
            argument
                .to_str()?
                .strip_prefix("-D")?
                .split_once('=')
                .filter(|(candidate, _)| *candidate == key)
                .map(|(_, value)| value)
        })
    }

    fn cmake_truthy(value: &str) -> bool {
        matches!(
            value.to_ascii_uppercase().as_str(),
            "1" | "ON" | "YES" | "TRUE" | "Y"
        )
    }
}

pub(super) struct NativeOhosAbi<'a> {
    name: &'a str,
}

impl<'a> NativeOhosAbi<'a> {
    pub(super) fn new(name: &'a str) -> Self {
        Self { name }
    }

    pub(super) fn target_triple(&self) -> NativeBuildResult<&'static str> {
        match self.name {
            "arm64-v8a" => Ok("aarch64-linux-ohos"),
            "armeabi-v7a" => Ok("arm-linux-ohos"),
            "x86_64" => Ok("x86_64-linux-ohos"),
            _ => Err(NativeBuildError::UnknownCpuAbi {
                abi: self.name.to_string(),
            }),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum NativeOhosStl {
    Shared,
    None,
}

impl NativeOhosStl {
    pub(super) fn from_cmake_arguments(arguments: &[OsString]) -> Result<Self, String> {
        match NativeBuildSanitizers::cmake_value(arguments, "OHOS_STL").unwrap_or("c++_shared") {
            "c++_shared" => Ok(Self::Shared),
            "none" => Ok(Self::None),
            value => Err(format!(
                "unsupported OHOS_STL value for Zig toolchain: {value}"
            )),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct NativeLibCollectionPolicy {
    pub(crate) sanitizers: NativeBuildSanitizers,
    pub(crate) hwasan_config: Option<bool>,
    pub(crate) binxo_config: Option<bool>,
    pub(crate) read_elf: Option<PathBuf>,
    pub(crate) build_mode: String,
    pub(crate) sdk_toolchain: NativeSdkToolchain,
    pub(crate) native_lib: NativeLibOption,
    pub(crate) external_native_options: Option<ExternalNativeOptions>,
    pub(crate) self_package: Option<NativeLibPackageInfo>,
    pub(crate) cmake_local_dependencies: Arc<[NativeCmakeLocalDependency]>,
    pub(crate) release_cmake_build_type: NativeReleaseCmakeBuildType,
}

impl NativeLibCollectionPolicy {
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

    pub fn with_cmake_local_dependency_inventory(
        mut self,
        dependencies: Arc<[NativeCmakeLocalDependency]>,
    ) -> Self {
        self.cmake_local_dependencies = dependencies;
        self
    }

    pub fn with_release_cmake_build_type(mut self, value: Option<&str>) -> Self {
        self.release_cmake_build_type = NativeReleaseCmakeBuildType::from_hvigor_property(value);
        self
    }

    pub(crate) fn enable_cmake_build(&self) -> bool {
        self.external_native_options.is_some()
    }

    pub(crate) fn validate_packaged_libs(&self, libs: &[PathBuf]) -> NativeBuildResult<()> {
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
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) is_local: bool,
}

/// Resolved local-module state consumed by `ConfigureCmake`.
///
/// Hvigor reads this from each dependency target service. Arkdown resolves it
/// once from the build-session target model and passes the immutable result to
/// the native domain owner, avoiding module build-profile rereads.
#[derive(Debug, Clone)]
pub struct NativeCmakeLocalDependency {
    pub(crate) package_root: PathBuf,
    pub(crate) product_name: String,
    pub(crate) target_name: String,
    pub(crate) build_mode: String,
    pub(crate) module_type: OhosModuleType,
    pub(crate) native_lib: NativeLibOption,
}

impl NativeCmakeLocalDependency {
    pub fn new(
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

    pub(crate) fn package_root(&self) -> &Path {
        &self.package_root
    }

    pub(crate) fn process_libs_dir(&self) -> PathBuf {
        NativeModuleLayout::for_variant(&self.package_root, &self.product_name, &self.target_name)
            .process_libs_dir()
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

    pub(crate) fn dependency(node: &DependencyNodeSnapshot) -> Self {
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
    pub(crate) cmake: Option<PathBuf>,
    pub(crate) ninja: Option<PathBuf>,
    pub(crate) llvm_strip: Option<PathBuf>,
    pub(crate) binxo: Option<PathBuf>,
    pub(crate) native_dir: Option<PathBuf>,
    pub(crate) toolchain_file: Option<PathBuf>,
    pub(super) compiler: NativeCompilerDriver,
}

#[derive(Debug, Clone, Default)]
pub(super) enum NativeCompilerDriver {
    #[default]
    SdkClang,
    Zig {
        executable: PathBuf,
    },
}

impl NativeCompilerDriver {
    pub(super) fn zig_executable(&self) -> Option<&Path> {
        match self {
            Self::SdkClang => None,
            Self::Zig { executable } => Some(executable),
        }
    }

    pub(super) fn requires_cache_reset_without_state(&self) -> bool {
        matches!(self, Self::Zig { .. })
    }
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
            compiler: NativeCompilerDriver::default(),
        }
    }

    pub fn with_zig_enabled(mut self, enabled: bool) -> NativeBuildResult<Self> {
        if !enabled {
            return Ok(self);
        }
        self.compiler = NativeCompilerDriver::Zig {
            executable: super::zig::NativeZigExecutableResolver::resolve()?,
        };
        Ok(self)
    }

    /// Construct the subset of the SDK toolchain needed by hvigor `clean`.
    pub fn for_clean(ninja: Option<PathBuf>) -> Self {
        Self {
            ninja,
            ..Self::default()
        }
    }

    pub(crate) fn require(&self) -> Result<ResolvedNativeSdkToolchain<'_>, String> {
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
            compiler: &self.compiler,
        })
    }

    pub(crate) fn llvm_strip_path(&self) -> Option<PathBuf> {
        self.llvm_strip.clone()
    }

    pub(crate) fn ninja_path(&self) -> Option<&Path> {
        self.ninja.as_deref()
    }
}

pub(crate) struct ResolvedNativeSdkToolchain<'a> {
    pub(crate) cmake: &'a Path,
    pub(crate) ninja: &'a Path,
    pub(crate) native_dir: &'a Path,
    pub(crate) toolchain_file: &'a Path,
    pub(super) compiler: &'a NativeCompilerDriver,
}

/// One Rust-owned native command with an optional argument tail that Hvigor
/// appends after its JavaScript `CommandBuilderModifier` has run.
#[derive(Debug, Clone)]
pub struct NativeCommandPlan {
    pub(crate) module_root: PathBuf,
    pub(crate) abi: String,
    pub(crate) program: OsString,
    pub(crate) arguments: Vec<OsString>,
    pub(crate) trailing_arguments: Vec<OsString>,
    pub(crate) current_dir: PathBuf,
}

impl NativeCommandPlan {
    pub(crate) fn ninja(
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

    pub fn abi(&self) -> &str {
        &self.abi
    }

    /// Convert the modifier-visible base command into the same `string[]`
    /// contract exposed by hvigor. Non-UTF host paths continue to work when no
    /// JavaScript modifier is registered; they are rejected only at this API
    /// boundary because JavaScript cannot preserve their bytes.
    pub fn modifier_command(&self) -> NativeBuildResult<Vec<String>> {
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

    pub fn with_modifier_command(mut self, command: Vec<String>) -> NativeBuildResult<Self> {
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

    pub fn execute(self) -> NativeBuildResult<()> {
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

    pub(crate) fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command
            .args(&self.arguments)
            .args(&self.trailing_arguments)
            .current_dir(&self.current_dir);
        command
    }
}

/// Exact DevEco Studio 6.1.1.280 `native-binxo.ts` command contract.
pub(crate) struct NativeBinxoCommand<'a> {
    pub(crate) executable: &'a Path,
    pub(crate) source: &'a Path,
    pub(crate) destination: &'a Path,
}

impl<'a> NativeBinxoCommand<'a> {
    pub(crate) fn new(executable: &'a Path, source: &'a Path, destination: &'a Path) -> Self {
        Self {
            executable,
            source,
            destination,
        }
    }

    pub(crate) fn command(&self) -> Command {
        let mut command = Command::new(self.executable);
        command
            .arg("-A")
            .arg("--hwasan-inputfile")
            .arg(self.source)
            .arg("--hwasan-outputfil")
            .arg(self.destination);
        command
    }

    pub(crate) fn execute(&self) -> std::io::Result<std::process::Output> {
        self.command().output()
    }
}

pub(crate) type NativeBuildResult<T> = Result<T, NativeBuildError>;

#[derive(Debug)]
pub enum NativeBuildError {
    ZigCompilerNotFound,
    ZigSanitizerRuntimeUnavailable {
        path: PathBuf,
        abi: String,
        message: String,
    },
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
      Self::ZigCompilerNotFound => write!(
        f,
        "ARKDOWN_ZIG_COMPILER_NOT_FOUND: --enable-zig requires the Zig executable on PATH"
      ),
      Self::ZigSanitizerRuntimeUnavailable { path, abi, message } => write!(
        f,
        "ARKDOWN_ZIG_SANITIZER_RUNTIME_UNAVAILABLE: cannot configure OHOS sanitizer runtime for ABI '{abi}' at '{}': {message}",
        path.display()
      ),
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
pub struct NativeCleanStage<'a> {
    pub(crate) project_root: &'a Path,
    pub(crate) module_root: &'a Path,
    pub(crate) ninja: Option<&'a Path>,
    pub(crate) project_task: bool,
    pub(crate) native_product_names: Vec<String>,
    pub(crate) native_target_names: Vec<String>,
    pub(crate) native_build_mode: String,
}

impl<'a> NativeCleanStage<'a> {
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
            native_product_names: Vec::new(),
            native_target_names: Vec::new(),
            native_build_mode: String::new(),
        }
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
            directories.push(self.module_root.join(".preview"));
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

    pub(crate) fn remove_directory(&self, directory: &Path) -> NativeBuildResult<()> {
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

    pub(crate) fn clean_native_trees(&self) {
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
    pub(crate) path: PathBuf,
    pub(crate) evidence: SanitizerLibraryEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SanitizerLibraryEvidence {
    Hwasan,
    Asan,
    Unsanitized,
}

impl SanitizerLibraryEvidence {
    pub(crate) fn from_readelf_output(output: &str) -> Self {
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

pub(crate) struct NativeHwasanValidator<'a> {
    pub(crate) read_elf: &'a Path,
}

impl<'a> NativeHwasanValidator<'a> {
    pub(crate) fn new(read_elf: &'a Path) -> Self {
        Self { read_elf }
    }

    pub(crate) fn validate(&self, libs: &[PathBuf]) -> NativeBuildResult<()> {
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

    pub(crate) fn inspect(&self, lib: &Path) -> NativeBuildResult<SanitizerLibraryEvidence> {
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

    pub(crate) fn is_sanitizer_exempt(&self, lib: &Path) -> bool {
        matches!(
            lib.file_name().and_then(|name| name.to_str()),
            Some("libc++_shared.so") | Some("libc++.so")
        )
    }
}

pub(crate) struct NativeModuleLayout<'a> {
    pub(crate) module_root: &'a Path,
    pub(crate) product_name: &'a str,
    pub(crate) target_name: &'a str,
}

impl<'a> NativeModuleLayout<'a> {
    pub(crate) fn for_variant(
        module_root: &'a Path,
        product_name: &'a str,
        target_name: &'a str,
    ) -> Self {
        Self {
            module_root,
            product_name,
            target_name,
        }
    }

    pub(crate) fn cpp_source_dir(&self) -> PathBuf {
        self.module_root.join("src").join("main").join("cpp")
    }

    pub(crate) fn cmake_lists(&self) -> PathBuf {
        self.cpp_source_dir().join("CMakeLists.txt")
    }

    pub(crate) fn top_level_libs(&self) -> PathBuf {
        self.module_root.join(HVIGOR_LIBS_DIR)
    }

    pub(crate) fn build_default(&self) -> PathBuf {
        self.module_root.join("build").join(self.product_name)
    }

    pub(crate) fn cmake_intermediate_dir(&self) -> PathBuf {
        self.build_default()
            .join("intermediates")
            .join("cmake")
            .join(self.target_name)
    }

    /// `ModulePathInfoIml.getNinjaWorkDir()`.
    pub(crate) fn ninja_work_dir(&self, build_mode: &str) -> PathBuf {
        self.module_root
            .join(".cxx")
            .join(self.product_name)
            .join(self.target_name)
            .join(build_mode)
    }

    /// Root generated by `ConfigureCmake` and passed to CMake as
    /// `CMAKE_FIND_ROOT_PATH`.
    pub(crate) fn cmake_dependency_root(&self, build_mode: &str) -> PathBuf {
        self.ninja_work_dir(build_mode).join("hvigor")
    }

    pub(crate) fn cmake_obj_dir(&self) -> PathBuf {
        self.cmake_intermediate_dir().join("obj")
    }

    pub(crate) fn stripped_libs_dir(&self) -> PathBuf {
        self.build_default()
            .join("intermediates")
            .join("stripped_native_libs")
            .join(self.target_name)
    }

    pub(crate) fn process_libs_dir(&self) -> PathBuf {
        self.build_default()
            .join("intermediates")
            .join("libs")
            .join(self.target_name)
    }

    pub(crate) fn native_lib_cache_file(&self) -> PathBuf {
        self.build_default()
            .join("intermediates")
            .join("patch")
            .join(self.target_name)
            .join("base_native_libs.json")
    }

    pub(crate) fn binxo_libs_dir(&self) -> PathBuf {
        self.build_default()
            .join("intermediates")
            .join("binxo")
            .join(self.target_name)
    }

    pub(crate) fn binxo_native_lib_cache_file(&self) -> PathBuf {
        self.build_default()
            .join("intermediates")
            .join("patch")
            .join(self.target_name)
            .join("binxo_native_libs.json")
    }

    pub(crate) fn patch_last_libs_file(&self) -> PathBuf {
        self.build_default()
            .join("intermediates")
            .join("patch")
            .join(self.target_name)
            .join("libs.json")
    }

    pub(crate) fn cangjie_intermediate_dir(&self) -> PathBuf {
        self.build_default()
            .join("intermediates")
            .join("cj")
            .join("extend_libs")
            .join(self.target_name)
    }

    pub(crate) fn so_search_roots(&self) -> Vec<PathBuf> {
        vec![
            self.cmake_obj_dir(),
            self.cangjie_intermediate_dir(),
            self.top_level_libs(),
        ]
    }

    pub(crate) fn has_native_code(&self) -> bool {
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
    pub(crate) module_root: PathBuf,
    pub(crate) module_name: String,
    pub(crate) build_mode: BuildMode,
    pub(crate) source_map_primary: PathBuf,
    pub(crate) source_map_fallback: Option<PathBuf>,
    pub(crate) name_cache: PathBuf,
    pub(crate) should_collect: bool,
    pub(crate) product_name: String,
    pub(crate) target_name: String,
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

    pub(crate) fn source_map_input(&self) -> Option<&Path> {
        if self.source_map_primary.is_file() {
            Some(&self.source_map_primary)
        } else {
            self.source_map_fallback
                .as_deref()
                .filter(|path| path.is_file())
        }
    }

    pub(crate) fn copy_artifact(&self, source: &Path, destination: &Path) -> NativeBuildResult<()> {
        std::fs::copy(source, destination)
            .map(|_| ())
            .map_err(|error| NativeBuildError::DebugSymbolCollectionFailed {
                path: source.to_path_buf(),
                message: error.to_string(),
            })
    }

    pub(crate) fn link_native_lib_directories(
        &self,
        destination: &Path,
    ) -> NativeBuildResult<Vec<PathBuf>> {
        let process_libs = NativeModuleLayout::for_variant(
            &self.module_root,
            &self.product_name,
            &self.target_name,
        )
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

    pub(crate) fn symlink_directory(
        &self,
        source: &Path,
        destination: &Path,
    ) -> std::io::Result<()> {
        Self::link_directory(source, destination)
    }

    pub(crate) fn link_directory(source: &Path, destination: &Path) -> std::io::Result<()> {
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
    pub(crate) project_root: PathBuf,
    pub(crate) product_name: String,
    pub(crate) target_name: String,
    pub(crate) build_mode: BuildMode,
    pub(crate) modules: Vec<ModuleConfig>,
    pub(crate) should_collect: bool,
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

    pub(crate) fn destination(&self) -> PathBuf {
        self.project_root
            .join("build")
            .join("outputs")
            .join(&self.product_name)
            .join("symbol")
            .join(self.mode_name())
    }

    pub(crate) fn module_destination(&self, module: &ModuleConfig) -> PathBuf {
        module
            .src_path
            .join("build")
            .join(&self.product_name)
            .join("outputs")
            .join(&self.target_name)
            .join("symbol")
            .join(self.mode_name())
    }

    pub(crate) fn mode_name(&self) -> &'static str {
        self.build_mode.as_str()
    }

    pub(crate) fn build_zip(&self, destination: &Path) -> NativeBuildResult<PathBuf> {
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

    pub(crate) fn write_zip(&self, source: &Path, output: &Path) -> NativeBuildResult<()> {
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
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
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
        archive.finish().map(|_| ()).map_err(|error| {
            NativeBuildError::DebugSymbolCollectionFailed {
                path: output.to_path_buf(),
                message: error.to_string(),
            }
        })
    }
}

pub(crate) struct NativeLibraryRoot<'a> {
    pub(crate) libs_dir: &'a Path,
}

impl<'a> NativeLibraryRoot<'a> {
    pub(crate) fn new(libs_dir: &'a Path) -> Self {
        Self { libs_dir }
    }

    pub(crate) fn contains_known_abi_library(&self) -> bool {
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

pub(crate) struct NativeAbiFilters<'a> {
    pub(crate) filters: &'a [String],
}

impl<'a> NativeAbiFilters<'a> {
    pub(crate) fn new(filters: &'a [String]) -> Self {
        Self { filters }
    }

    pub(crate) fn build_abis(&self) -> Vec<String> {
        if self.filters.is_empty() {
            vec!["arm64-v8a".to_string()]
        } else {
            self.filters.to_vec()
        }
    }

    pub(crate) fn package_abis(&self) -> Vec<String> {
        if self.filters.is_empty() {
            KNOWN_ABI_DIRS
                .iter()
                .map(|abi| (*abi).to_string())
                .collect()
        } else {
            self.filters.to_vec()
        }
    }

    pub(crate) fn matches_path(&self, path: &Path) -> bool {
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
pub(crate) struct NativeCmakeDependencyConfigurator<'a> {
    pub(crate) module: &'a ModuleConfig,
    pub(crate) abi_filters: &'a [String],
    pub(crate) deps: &'a DependencyGraphSnapshot,
    pub(crate) product_name: &'a str,
    pub(crate) target_name: &'a str,
    pub(crate) build_mode: &'a str,
    pub(crate) local_dependencies: &'a [NativeCmakeLocalDependency],
}

impl<'a> NativeCmakeDependencyConfigurator<'a> {
    pub(crate) fn new(
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

    pub(crate) fn write(&self) -> NativeBuildResult<()> {
        let mode = if self.build_mode.is_empty() {
            "debug"
        } else {
            self.build_mode
        };
        let layout = NativeModuleLayout::for_variant(
            &self.module.src_path,
            self.product_name,
            self.target_name,
        );
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
                let cmake_file = ArtifactPathSerializer::path_to_slash_string(&cmake_file)
                    .map_err(|error| NativeBuildError::ConfigureCmakeFailed {
                        path: cmake_file.clone(),
                        message: error.to_string(),
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

    pub(crate) fn packages(&self) -> Vec<NativeCmakePackage> {
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

    pub(crate) fn ordered_nodes(&self) -> Vec<&DependencyNodeSnapshot> {
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

    pub(crate) fn remote_has_native_inputs(root: &Path) -> bool {
        let libs = root.join(HVIGOR_LIBS_DIR);
        let include = root.join("include");
        include.exists()
            || libs
                .read_dir()
                .ok()
                .is_some_and(|mut entries| entries.next().is_some())
    }

    pub(crate) fn cpu_type(abi: &str) -> NativeBuildResult<&'static str> {
        NativeOhosAbi::new(abi).target_triple()
    }

    pub(crate) fn write_if_changed(path: &Path, content: &[u8]) -> NativeBuildResult<()> {
        if std::fs::read(path).is_ok_and(|current| current == content) {
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                NativeBuildError::ConfigureCmakeFailed {
                    path: parent.to_path_buf(),
                    message: error.to_string(),
                }
            })?;
        }
        std::fs::write(path, content).map_err(|error| NativeBuildError::ConfigureCmakeFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        })
    }

    pub(crate) const fn eol() -> &'static str {
        if cfg!(windows) {
            "\r\n"
        } else {
            "\n"
        }
    }
}

pub(crate) struct NativeCmakePackage {
    pub(crate) name: String,
    pub(crate) root: PathBuf,
    pub(crate) header_paths: Vec<PathBuf>,
    pub(crate) local: Option<NativeCmakeLocalDependency>,
    pub(crate) match_rule: Option<NativeLibPackageSelection>,
    pub(crate) link_libraries: BTreeMap<String, Vec<String>>,
}

impl NativeCmakePackage {
    pub(crate) fn new(
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
                    .filter(|library| {
                        !library.name.is_empty() && !library.link_libraries.is_empty()
                    })
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

    pub(crate) fn libraries_for_abi(
        &self,
        abi: &str,
    ) -> NativeBuildResult<Vec<NativeCmakeLibrary>> {
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

    pub(crate) fn remote_libraries(&self, abi: &str) -> Vec<NativeCmakeLibrary> {
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

    pub(crate) fn cmake_content(
        &self,
        libraries: &[NativeCmakeLibrary],
    ) -> NativeBuildResult<String> {
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

    pub(crate) fn is_excluded(
        rule: &NativeLibPackageSelection,
        library: &NativeCmakeLibrary,
    ) -> bool {
        rule.exclude
            .as_ref()
            .is_some_and(|exclude| exclude.iter().any(|name| name == &library.name_on_disk))
            || rule.exclude_pattern.as_ref().is_some_and(|patterns| {
                patterns.iter().any(|pattern| {
                    HvigorNativeLibraryPattern::matches_filter_pattern(&library.path, pattern, true)
                })
            })
    }

    pub(crate) fn remote_link_libraries(
        node: &DependencyNodeSnapshot,
    ) -> BTreeMap<String, Vec<String>> {
        node.package
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

pub(crate) struct NativeCmakeCodeModelReader<'a> {
    pub(crate) dependency: &'a NativeCmakeLocalDependency,
    pub(crate) abi: &'a str,
}

impl<'a> NativeCmakeCodeModelReader<'a> {
    pub(crate) fn new(dependency: &'a NativeCmakeLocalDependency, abi: &'a str) -> Self {
        Self { dependency, abi }
    }

    pub(crate) fn libraries(&self) -> NativeBuildResult<Vec<NativeCmakeLibrary>> {
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
                path.file_name()
                    .and_then(OsStr::to_str)
                    .is_some_and(|name| name.starts_with("target-") && name.ends_with(".json"))
            })
            .collect::<Vec<_>>();
        target_files.sort();
        let mut libraries = Vec::new();
        for target_file in target_files {
            let bytes = std::fs::read(&target_file).map_err(|error| {
                NativeBuildError::ConfigureCmakeFailed {
                    path: target_file.clone(),
                    message: error.to_string(),
                }
            })?;
            let target: serde_json::Value = serde_json::from_slice(&bytes).map_err(|error| {
                NativeBuildError::ConfigureCmakeFailed {
                    path: target_file.clone(),
                    message: error.to_string(),
                }
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

    pub(crate) fn collect_hsp_runtime_files(
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

pub(crate) struct NativeCmakeLibrary {
    pub(crate) name_on_disk: String,
    pub(crate) target_name: String,
    pub(crate) path: PathBuf,
    pub(crate) is_static: bool,
}

impl NativeCmakeLibrary {
    pub(crate) fn from_path(path: PathBuf) -> Option<Self> {
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
