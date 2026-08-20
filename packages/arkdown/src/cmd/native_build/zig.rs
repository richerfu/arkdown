//! Zig compiler-driver overlay for the OHOS SDK CMake toolchain.

use std::path::{Path, PathBuf};

use arkdown_path::{canonicalize_or_owned, ArtifactPathSerializer, HostPlatform};

use super::impl_types::{
  NativeBuildError, NativeBuildResult, NativeBuildSanitizers, NativeCmakeDependencyConfigurator,
  NativeCompilerDriver, NativeOhosAbi, NativeOhosStl,
};

pub(super) struct NativeZigExecutableResolver;

impl NativeZigExecutableResolver {
  pub(super) fn resolve() -> NativeBuildResult<PathBuf> {
    let executable_name = HostPlatform::CURRENT.executable_name("zig");
    let paths = std::env::var_os("PATH").ok_or(NativeBuildError::ZigCompilerNotFound)?;
    std::env::split_paths(&paths)
      .map(|directory| directory.join(&executable_name))
      .find(|candidate| candidate.is_file())
      .map(|path| canonicalize_or_owned(&path))
      .ok_or(NativeBuildError::ZigCompilerNotFound)
  }
}

/// Owns the CMake cache boundary for compiler-driver selection.
///
/// CMake does not support changing `CMAKE_<LANG>_COMPILER` inside an existing
/// cache. Arkdown therefore records the selected driver per ABI and removes
/// only CMake-owned cache/build metadata when the driver changes. Native
/// outputs live outside this directory; project sources and dependency inputs
/// are never removed.
pub(super) struct NativeCmakeCompilerState<'a> {
  build_dir: &'a Path,
  compiler: &'a NativeCompilerDriver,
  sdk_toolchain_file: &'a Path,
}

impl<'a> NativeCmakeCompilerState<'a> {
  pub(super) fn new(
    build_dir: &'a Path,
    compiler: &'a NativeCompilerDriver,
    sdk_toolchain_file: &'a Path,
  ) -> Self {
    Self {
      build_dir,
      compiler,
      sdk_toolchain_file,
    }
  }

  pub(super) fn prepare(&self) -> NativeBuildResult<()> {
    let expected = self.cache_key()?;
    let existing = self.read()?;
    let has_cmake_cache = self.cache_file().is_file() || self.compiler_metadata_dir().is_dir();
    let reset = existing
      .as_deref()
      .is_some_and(|value| value != expected.as_str())
      || existing.is_none()
        && has_cmake_cache
        && self.compiler.requires_cache_reset_without_state();
    if reset {
      self.remove_file(&self.cache_file())?;
      self.remove_dir(&self.compiler_metadata_dir())?;
      self.remove_dir(&self.file_api_reply_dir())?;
    }
    Ok(())
  }

  pub(super) fn commit(&self) -> NativeBuildResult<()> {
    NativeCmakeDependencyConfigurator::write_if_changed(
      &self.state_file(),
      self.cache_key()?.as_bytes(),
    )
  }

  fn read(&self) -> NativeBuildResult<Option<String>> {
    match std::fs::read_to_string(self.state_file()) {
      Ok(value) => Ok(Some(value)),
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
      Err(error) => Err(self.error(error)),
    }
  }

  fn cache_key(&self) -> NativeBuildResult<String> {
    let sdk_toolchain_file = ArtifactPathSerializer::path_to_slash_string(self.sdk_toolchain_file)
      .map_err(|error| self.error(error))?;
    match self.compiler {
      NativeCompilerDriver::SdkClang => Ok(format!("sdk-clang\n{sdk_toolchain_file}\n")),
      NativeCompilerDriver::Zig { executable } => {
        let executable = ArtifactPathSerializer::path_to_slash_string(executable)
          .map_err(|error| self.error(error))?;
        Ok(format!(
          "zig-wrapper-v6\n{executable}\n{sdk_toolchain_file}\n"
        ))
      }
    }
  }

  fn remove_file(&self, path: &Path) -> NativeBuildResult<()> {
    match std::fs::remove_file(path) {
      Ok(()) => Ok(()),
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
      Err(error) => Err(self.error(error)),
    }
  }

  fn remove_dir(&self, path: &Path) -> NativeBuildResult<()> {
    match std::fs::remove_dir_all(path) {
      Ok(()) => Ok(()),
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
      Err(error) => Err(self.error(error)),
    }
  }

  fn state_file(&self) -> PathBuf {
    self.build_dir.join(".arkdown-native-compiler")
  }

  fn cache_file(&self) -> PathBuf {
    self.build_dir.join("CMakeCache.txt")
  }

  fn compiler_metadata_dir(&self) -> PathBuf {
    self.build_dir.join("CMakeFiles")
  }

  fn file_api_reply_dir(&self) -> PathBuf {
    self
      .build_dir
      .join(".cmake")
      .join("api")
      .join("v1")
      .join("reply")
  }

  fn error(&self, error: impl std::fmt::Display) -> NativeBuildError {
    NativeBuildError::ConfigureCmakeFailed {
      path: self.build_dir.to_path_buf(),
      message: format!("prepare native compiler cache: {error}"),
    }
  }
}

/// Writes the CMake toolchain overlay selected by `--enable-zig`.
///
/// The OHOS SDK toolchain remains the source of ABI, sysroot, linker, and
/// archive configuration. The overlay includes it first, then replaces only
/// the C/C++ compiler drivers with Zig. Two diagnostic-only linker switches
/// emitted by current OHOS SDKs are removed because Zig's bundled lld rejects
/// them instead of ignoring them.
pub(super) struct NativeZigToolchainWriter<'a> {
  sdk_toolchain_file: &'a Path,
  native_dir: &'a Path,
  zig: &'a Path,
  ohos_stl: NativeOhosStl,
  sanitizers: NativeBuildSanitizers,
  abi: &'a str,
  output: PathBuf,
}

impl<'a> NativeZigToolchainWriter<'a> {
  pub(super) fn new(
    sdk_toolchain_file: &'a Path,
    native_dir: &'a Path,
    zig: &'a Path,
    ohos_stl: NativeOhosStl,
    sanitizers: NativeBuildSanitizers,
    abi: &'a str,
    build_dir: &Path,
  ) -> Self {
    Self {
      sdk_toolchain_file,
      native_dir,
      zig,
      ohos_stl,
      sanitizers,
      abi,
      output: build_dir.join("arkdown-zig.toolchain.cmake"),
    }
  }

  pub(super) fn write(&self) -> NativeBuildResult<PathBuf> {
    let sdk_toolchain_file = self.cmake_path(self.sdk_toolchain_file)?;
    let sysroot_options = NativeZigSysrootOptions::new(self.native_dir, self.abi);
    let c_include_dirs = sysroot_options.include_dirs()?;
    let cxx_runtime =
      NativeZigCxxRuntime::new(self.native_dir, self.abi, self.ohos_stl, &self.output);
    let cxx_include_dirs = cxx_runtime.compiler_include_dirs(&c_include_dirs)?;
    let compiler_wrappers = NativeZigCompilerWrapperWriter::new(
      self.zig,
      &c_include_dirs,
      &cxx_include_dirs,
      &self.output,
    )
    .write()?;
    let zig_cc = self.cmake_path(&compiler_wrappers.c)?;
    let zig_cxx = self.cmake_path(&compiler_wrappers.cxx)?;
    let sanitizer_runtime =
      NativeZigSanitizerRuntime::new(self.native_dir, self.sanitizers, self.abi, &self.output)
        .resolve()?;
    let eol = NativeCmakeDependencyConfigurator::eol();
    let mut lines = vec![
      "# Generated by arkdown --enable-zig.".to_string(),
      format!("include({sdk_toolchain_file})"),
      format!("set(CMAKE_C_COMPILER {zig_cc})"),
      format!("set(CMAKE_CXX_COMPILER {zig_cxx})"),
      "unset(CMAKE_C_COMPILER_EXTERNAL_TOOLCHAIN)".to_string(),
      "unset(CMAKE_CXX_COMPILER_EXTERNAL_TOOLCHAIN)".to_string(),
      "# clang-tidy cannot model Zig's compiler-internal include search path.".to_string(),
      "function(_arkdown_disable_zig_incompatible_clang_tidy _arkdown_directory)".to_string(),
      "  get_property(_arkdown_targets DIRECTORY \"${_arkdown_directory}\" PROPERTY BUILDSYSTEM_TARGETS)"
        .to_string(),
      "  foreach(_arkdown_target IN LISTS _arkdown_targets)".to_string(),
      "    set_property(TARGET \"${_arkdown_target}\" PROPERTY C_CLANG_TIDY \"\")".to_string(),
      "    set_property(TARGET \"${_arkdown_target}\" PROPERTY CXX_CLANG_TIDY \"\")".to_string(),
      "  endforeach()".to_string(),
      "  get_property(_arkdown_subdirectories DIRECTORY \"${_arkdown_directory}\" PROPERTY SUBDIRECTORIES)"
        .to_string(),
      "  foreach(_arkdown_subdirectory IN LISTS _arkdown_subdirectories)".to_string(),
      "    _arkdown_disable_zig_incompatible_clang_tidy(\"${_arkdown_subdirectory}\")"
        .to_string(),
      "  endforeach()".to_string(),
      "endfunction()".to_string(),
      "cmake_language(DEFER CALL _arkdown_disable_zig_incompatible_clang_tidy \"${CMAKE_SOURCE_DIR}\")"
        .to_string(),
      "foreach(_arkdown_linker_flags IN ITEMS CMAKE_SHARED_LINKER_FLAGS CMAKE_MODULE_LINKER_FLAGS CMAKE_EXE_LINKER_FLAGS)".to_string(),
      r#"  string(REPLACE "-Wl,--warn-shared-textrel" "" ${_arkdown_linker_flags} "${${_arkdown_linker_flags}}")"#.to_string(),
      r#"  string(REPLACE "-Wl,--fatal-warnings" "" ${_arkdown_linker_flags} "${${_arkdown_linker_flags}}")"#.to_string(),
      "endforeach()".to_string(),
    ];
    lines.extend(sysroot_options.cmake_link_options()?);
    lines.extend(cxx_runtime.cmake_link_options()?);
    lines.extend(sanitizer_runtime.cmake_link_options()?);
    let mut content = lines.join(eol);
    content.push_str(eol);
    NativeCmakeDependencyConfigurator::write_if_changed(&self.output, content.as_bytes())?;
    Ok(self.output.clone())
  }

  fn cmake_path(&self, path: &Path) -> NativeBuildResult<String> {
    let path = ArtifactPathSerializer::path_to_slash_string(path).map_err(|error| {
      NativeBuildError::ConfigureCmakeFailed {
        path: self.output.clone(),
        message: error.to_string(),
      }
    })?;
    Ok(Self::bracket_quote(&path))
  }

  fn bracket_quote(value: &str) -> String {
    let mut level = 0;
    loop {
      let delimiter = "=".repeat(level);
      let closing = format!("]{delimiter}]");
      if !value.contains(&closing) {
        return format!("[{delimiter}[{value}{closing}");
      }
      level += 1;
    }
  }
}

struct NativeZigCompilerWrappers {
  c: PathBuf,
  cxx: PathBuf,
}

/// Generates one executable per compiler language so CMake integrations such
/// as `CMAKE_CXX_CLANG_TIDY` see a conventional single compiler executable
/// rather than the two-token `zig c++` command.
///
/// This follows cargo-zigbuild `prepare_zig_linker`/`setup_cmake_toolchain`'s
/// wrapper boundary. OHOS additionally needs its SDK include order and libc++
/// runtime because prebuilt native dependencies use the SDK's C++ ABI.
struct NativeZigCompilerWrapperWriter<'a> {
  zig: &'a Path,
  c_include_dirs: &'a [PathBuf],
  cxx_include_dirs: &'a [PathBuf],
  diagnostic_path: &'a Path,
}

impl<'a> NativeZigCompilerWrapperWriter<'a> {
  fn new(
    zig: &'a Path,
    c_include_dirs: &'a [PathBuf],
    cxx_include_dirs: &'a [PathBuf],
    diagnostic_path: &'a Path,
  ) -> Self {
    Self {
      zig,
      c_include_dirs,
      cxx_include_dirs,
      diagnostic_path,
    }
  }

  fn write(&self) -> NativeBuildResult<NativeZigCompilerWrappers> {
    let parent =
      self
        .diagnostic_path
        .parent()
        .ok_or_else(|| NativeBuildError::ConfigureCmakeFailed {
          path: self.diagnostic_path.to_path_buf(),
          message: "Zig toolchain output has no parent directory".to_string(),
        })?;
    let windows = HostPlatform::CURRENT == HostPlatform::Windows;
    let suffix = if windows { ".cmd" } else { "" };
    let c = parent.join(format!("arkdown-zig-cc{suffix}"));
    let cxx = parent.join(format!("arkdown-zig-cxx{suffix}"));
    NativeCmakeDependencyConfigurator::write_if_changed(
      &c,
      self.content("cc", windows)?.as_bytes(),
    )?;
    NativeCmakeDependencyConfigurator::write_if_changed(
      &cxx,
      self.content("c++", windows)?.as_bytes(),
    )?;
    self.make_executable(&c)?;
    self.make_executable(&cxx)?;
    Ok(NativeZigCompilerWrappers { c, cxx })
  }

  fn content(&self, subcommand: &str, windows: bool) -> NativeBuildResult<String> {
    let zig = ArtifactPathSerializer::path_to_slash_string(self.zig).map_err(|error| {
      NativeBuildError::ConfigureCmakeFailed {
        path: self.diagnostic_path.to_path_buf(),
        message: error.to_string(),
      }
    })?;
    let mut compiler_arguments = String::new();
    if subcommand == "c++" {
      compiler_arguments.push_str(" -nostdinc++");
    }
    let include_dirs = if subcommand == "c++" {
      self.cxx_include_dirs
    } else {
      self.c_include_dirs
    };
    for path in include_dirs {
      let path = ArtifactPathSerializer::path_to_slash_string(path).map_err(|error| {
        NativeBuildError::ConfigureCmakeFailed {
          path: self.diagnostic_path.to_path_buf(),
          message: error.to_string(),
        }
      })?;
      if windows {
        compiler_arguments.push_str(&format!(" -I\"{}\"", path.replace('%', "%%")));
      } else {
        compiler_arguments.push_str(&format!(" -I'{}'", path.replace('\'', "'\"'\"'")));
      }
    }
    if windows {
      return Ok(format!(
        "@echo off\r\n\"{}\" {subcommand}{} %*\r\n",
        zig.replace('%', "%%"),
        compiler_arguments
      ));
    }
    let zig = zig.replace('\'', "'\"'\"'");
    Ok(format!(
      "#!/bin/sh\nexec '{zig}' {subcommand}{compiler_arguments} \"$@\"\n"
    ))
  }

  #[cfg(unix)]
  fn make_executable(&self, path: &Path) -> NativeBuildResult<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = std::fs::metadata(path)
      .map_err(|error| NativeBuildError::ConfigureCmakeFailed {
        path: path.to_path_buf(),
        message: error.to_string(),
      })?
      .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).map_err(|error| {
      NativeBuildError::ConfigureCmakeFailed {
        path: path.to_path_buf(),
        message: error.to_string(),
      }
    })
  }

  #[cfg(not(unix))]
  fn make_executable(&self, _path: &Path) -> NativeBuildResult<()> {
    Ok(())
  }
}

struct NativeZigSysrootOptions<'a> {
  native_dir: &'a Path,
  abi: &'a str,
}

impl<'a> NativeZigSysrootOptions<'a> {
  fn new(native_dir: &'a Path, abi: &'a str) -> Self {
    Self { native_dir, abi }
  }

  fn include_dirs(&self) -> NativeBuildResult<Vec<PathBuf>> {
    let target = NativeOhosAbi::new(self.abi).target_triple()?;
    let sysroot = self.native_dir.join("sysroot");
    let include_dirs = [
      sysroot.join("usr").join("include").join(target),
      sysroot.join("include"),
      sysroot.join("usr").join("include"),
    ]
    .into_iter()
    .filter(|path| path.is_dir())
    .collect::<Vec<_>>();
    if include_dirs.is_empty() {
      return Err(NativeBuildError::ConfigureCmakeFailed {
        path: sysroot,
        message: format!("Zig toolchain found no OHOS system include directories for {target}"),
      });
    }
    Ok(include_dirs)
  }

  fn cmake_link_options(&self) -> NativeBuildResult<Vec<String>> {
    let target = NativeOhosAbi::new(self.abi).target_triple()?;
    let library_dir = self
      .native_dir
      .join("sysroot")
      .join("usr")
      .join("lib")
      .join(target);
    if !library_dir.is_dir() {
      return Err(NativeBuildError::ConfigureCmakeFailed {
        path: library_dir,
        message: format!("Zig toolchain found no OHOS system library directory for {target}"),
      });
    }
    let option = format!("SHELL:-L/usr/lib/{target}");
    Ok(vec![
      "add_link_options(".to_string(),
      format!("  {}", NativeZigToolchainWriter::bracket_quote(&option)),
      ")".to_string(),
    ])
  }
}

struct NativeSdkClangResource<'a> {
  native_dir: &'a Path,
}

impl<'a> NativeSdkClangResource<'a> {
  fn new(native_dir: &'a Path) -> Self {
    Self { native_dir }
  }

  fn root(&self, target: &str) -> Result<PathBuf, String> {
    let search_root = self.native_dir.join("llvm").join("lib").join("clang");
    let entries = std::fs::read_dir(&search_root)
      .map_err(|error| format!("read {}: {error}", search_root.display()))?;
    let mut candidates = entries
      .filter_map(Result::ok)
      .map(|entry| entry.path())
      .filter(|root| root.join("include").is_dir() && root.join("lib").join(target).is_dir())
      .collect::<Vec<_>>();
    candidates.sort();
    if candidates.len() != 1 {
      return Err(format!(
        "expected one Clang resource directory below {}, found {}",
        search_root.display(),
        candidates.len()
      ));
    }
    Ok(candidates.remove(0))
  }
}

/// Selects the OHOS libc++ headers and libraries that the SDK Clang driver
/// discovers implicitly. Zig's built-in C/C++ headers precede normal system
/// paths, so the wrapper supplies the SDK hierarchy as ordered user includes.
struct NativeZigCxxRuntime<'a> {
  native_dir: &'a Path,
  abi: &'a str,
  ohos_stl: NativeOhosStl,
  diagnostic_path: &'a Path,
}

impl<'a> NativeZigCxxRuntime<'a> {
  fn new(
    native_dir: &'a Path,
    abi: &'a str,
    ohos_stl: NativeOhosStl,
    diagnostic_path: &'a Path,
  ) -> Self {
    Self {
      native_dir,
      abi,
      ohos_stl,
      diagnostic_path,
    }
  }

  fn compiler_include_dirs(
    &self,
    system_include_dirs: &[PathBuf],
  ) -> NativeBuildResult<Vec<PathBuf>> {
    if matches!(self.ohos_stl, NativeOhosStl::None) {
      return Ok(system_include_dirs.to_vec());
    }
    let target = NativeOhosAbi::new(self.abi).target_triple()?;
    let clang_resource = NativeSdkClangResource::new(self.native_dir)
      .root(target)
      .map_err(|message| self.error(message))?;
    let mut paths = vec![
      self
        .native_dir
        .join("llvm/include/libcxx-ohos/include/c++/v1"),
      clang_resource.join("include"),
    ];
    paths.extend_from_slice(system_include_dirs);
    for path in &paths {
      if !path.is_dir() {
        return Err(self.error(format!(
          "Zig toolchain requires OHOS C++ include directory '{}'",
          path.display()
        )));
      }
    }
    Ok(paths)
  }

  fn cmake_link_options(&self) -> NativeBuildResult<Vec<String>> {
    let mut lines = vec![
      "add_link_options(".to_string(),
      "  \"$<$<LINK_LANGUAGE:CXX>:-nostdlib++>\"".to_string(),
    ];
    if matches!(self.ohos_stl, NativeOhosStl::None) {
      lines.push(")".to_string());
      return Ok(lines);
    }
    let target = NativeOhosAbi::new(self.abi).target_triple()?;
    let library_dir = self.native_dir.join("llvm").join("lib").join(target);
    let path = self.require_file(library_dir.join("libc++_shared.so"))?;
    let path = ArtifactPathSerializer::path_to_slash_string(&path).map_err(|error| {
      NativeBuildError::ConfigureCmakeFailed {
        path: self.diagnostic_path.to_path_buf(),
        message: error.to_string(),
      }
    })?;
    lines.push(format!(
      "  {}",
      NativeZigToolchainWriter::bracket_quote(&format!("$<$<LINK_LANGUAGE:CXX>:{path}>"))
    ));
    lines.push(")".to_string());
    Ok(lines)
  }

  fn require_file(&self, path: PathBuf) -> NativeBuildResult<PathBuf> {
    if path.is_file() {
      return Ok(path);
    }
    Err(self.error(format!(
      "Zig toolchain requires OHOS C++ runtime path '{}'",
      path.display()
    )))
  }

  fn error(&self, message: String) -> NativeBuildError {
    NativeBuildError::ConfigureCmakeFailed {
      path: self.diagnostic_path.to_path_buf(),
      message,
    }
  }
}

/// Resolves the sanitizer runtimes that the OHOS Clang driver normally adds
/// during link. Zig accepts the SDK sanitizer instrumentation flags but does
/// not discover the SDK's compiler-rt directory by itself.
struct NativeZigSanitizerRuntime<'a> {
  native_dir: &'a Path,
  sanitizers: NativeBuildSanitizers,
  abi: &'a str,
  diagnostic_path: &'a Path,
}

impl<'a> NativeZigSanitizerRuntime<'a> {
  fn new(
    native_dir: &'a Path,
    sanitizers: NativeBuildSanitizers,
    abi: &'a str,
    diagnostic_path: &'a Path,
  ) -> Self {
    Self {
      native_dir,
      sanitizers,
      abi,
      diagnostic_path,
    }
  }

  fn resolve(&self) -> NativeBuildResult<NativeZigSanitizerLinkOptions> {
    let files = self.runtime_files();
    if files.is_empty() {
      return Ok(NativeZigSanitizerLinkOptions::default());
    }
    let target = NativeOhosAbi::new(self.abi).target_triple()?;
    let resource_root = NativeSdkClangResource::new(self.native_dir)
      .root(target)
      .map_err(|message| self.unavailable_error(message))?;
    let runtime_root = resource_root.join("lib").join(target);
    let missing = files
      .iter()
      .filter(|runtime| !runtime_root.join(runtime.file_name).is_file())
      .map(|runtime| runtime.file_name)
      .collect::<Vec<_>>();
    if !missing.is_empty() {
      return Err(self.unavailable_error(format!(
        "compiler-rt directory '{}' is missing {}",
        runtime_root.display(),
        missing.join(", ")
      )));
    }
    let mut options = NativeZigSanitizerLinkOptions::default();
    for runtime in files {
      let path = runtime_root.join(runtime.file_name);
      match runtime.kind {
        NativeZigSanitizerRuntimeKind::Common => options.common.push(path),
        NativeZigSanitizerRuntimeKind::WholeArchive => options.whole_archive.push(path),
        NativeZigSanitizerRuntimeKind::ExecutableWholeArchive => {
          options.executable_whole_archive.push(path)
        }
      }
    }
    Ok(options)
  }

  fn runtime_files(&self) -> Vec<NativeZigSanitizerRuntimeFile> {
    let mut files = Vec::new();
    if self.sanitizers.asan {
      files.extend([
        NativeZigSanitizerRuntimeFile::common("libclang_rt.asan.so"),
        NativeZigSanitizerRuntimeFile::whole_archive("libclang_rt.asan_static.a"),
        NativeZigSanitizerRuntimeFile::executable_whole_archive("libclang_rt.asan-preinit.a"),
      ]);
    }
    if self.sanitizers.hwasan && self.abi == "arm64-v8a" {
      files.extend([
        NativeZigSanitizerRuntimeFile::common("libclang_rt.hwasan.so"),
        NativeZigSanitizerRuntimeFile::executable_whole_archive("libclang_rt.hwasan-preinit.a"),
      ]);
    }
    if self.sanitizers.tsan && self.abi == "arm64-v8a" {
      files.push(NativeZigSanitizerRuntimeFile::common("libclang_rt.tsan.so"));
    }
    let has_primary_runtime = self.sanitizers.asan
      || self.sanitizers.hwasan && self.abi == "arm64-v8a"
      || self.sanitizers.tsan && self.abi == "arm64-v8a";
    if self.sanitizers.ubsan && !has_primary_runtime {
      files.push(NativeZigSanitizerRuntimeFile::common(
        "libclang_rt.ubsan_standalone.so",
      ));
    }
    files
  }

  fn unavailable_error(&self, message: String) -> NativeBuildError {
    NativeBuildError::ZigSanitizerRuntimeUnavailable {
      path: self.diagnostic_path.to_path_buf(),
      abi: self.abi.to_string(),
      message,
    }
  }
}

#[derive(Clone, Copy)]
struct NativeZigSanitizerRuntimeFile {
  file_name: &'static str,
  kind: NativeZigSanitizerRuntimeKind,
}

impl NativeZigSanitizerRuntimeFile {
  fn common(file_name: &'static str) -> Self {
    Self {
      file_name,
      kind: NativeZigSanitizerRuntimeKind::Common,
    }
  }

  fn whole_archive(file_name: &'static str) -> Self {
    Self {
      file_name,
      kind: NativeZigSanitizerRuntimeKind::WholeArchive,
    }
  }

  fn executable_whole_archive(file_name: &'static str) -> Self {
    Self {
      file_name,
      kind: NativeZigSanitizerRuntimeKind::ExecutableWholeArchive,
    }
  }
}

#[derive(Clone, Copy)]
enum NativeZigSanitizerRuntimeKind {
  Common,
  WholeArchive,
  ExecutableWholeArchive,
}

#[derive(Default)]
struct NativeZigSanitizerLinkOptions {
  common: Vec<PathBuf>,
  whole_archive: Vec<PathBuf>,
  executable_whole_archive: Vec<PathBuf>,
}

impl NativeZigSanitizerLinkOptions {
  fn cmake_link_options(&self) -> NativeBuildResult<Vec<String>> {
    if self.common.is_empty()
      && self.whole_archive.is_empty()
      && self.executable_whole_archive.is_empty()
    {
      return Ok(Vec::new());
    }

    let mut lines = vec!["add_link_options(".to_string()];
    for path in &self.common {
      lines.push(format!("  {}", self.cmake_path(path)?));
    }
    if !self.whole_archive.is_empty() {
      lines.push("  \"-Wl,--whole-archive\"".to_string());
      for path in &self.whole_archive {
        lines.push(format!("  {}", self.cmake_path(path)?));
      }
      lines.push("  \"-Wl,--no-whole-archive\"".to_string());
    }
    if !self.executable_whole_archive.is_empty() {
      lines.push(
        "  \"$<$<STREQUAL:$<TARGET_PROPERTY:TYPE>,EXECUTABLE>:-Wl,--whole-archive>\"".to_string(),
      );
      for path in &self.executable_whole_archive {
        let serialized = ArtifactPathSerializer::path_to_slash_string(path).map_err(|error| {
          NativeBuildError::ConfigureCmakeFailed {
            path: path.clone(),
            message: error.to_string(),
          }
        })?;
        lines.push(format!(
          "  \"$<$<STREQUAL:$<TARGET_PROPERTY:TYPE>,EXECUTABLE>:{serialized}>\""
        ));
      }
      lines.push(
        "  \"$<$<STREQUAL:$<TARGET_PROPERTY:TYPE>,EXECUTABLE>:-Wl,--no-whole-archive>\""
          .to_string(),
      );
    }
    lines.push(")".to_string());
    Ok(lines)
  }

  fn cmake_path(&self, path: &Path) -> NativeBuildResult<String> {
    let serialized = ArtifactPathSerializer::path_to_slash_string(path).map_err(|error| {
      NativeBuildError::ConfigureCmakeFailed {
        path: path.to_path_buf(),
        message: error.to_string(),
      }
    })?;
    Ok(NativeZigToolchainWriter::bracket_quote(&serialized))
  }
}
