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

mod impl_collector;
mod impl_types;
mod zig;

pub(crate) use impl_collector::NativeLibCollector;
pub(crate) use impl_types::{
  DebugSymbolCollector, HvigorModuleCleanStage, NativeBuildSanitizers, NativeCmakeLocalDependency,
  NativeLibCollectionPolicy, NativeLibPackageInfo, NativeSdkToolchain, ProjectDebugSymbolCollector,
};

#[cfg(test)]
mod tests {
  use super::impl_collector::{
    NativeBinxoCache, NativeBinxoSymbol, NativeLibCacheWriter, NativeStripProcessor,
  };
  use super::impl_types::{
    DebugSymbolOutput, NativeAbiFilters, NativeBinxoCommand, NativeBuildConfig, NativeBuildError,
    NativeBuildResult, NativeCmakeDependencyConfigurator, NativeCompilerDriver, NativeModuleLayout,
    NativeReleaseCmakeBuildType, ResolvedNativeSdkToolchain, SanitizerLibraryEvidence,
  };
  use super::*;
  use arkdown_core::{
    BuildMode, ExternalNativeOptions, ModuleConfig, NativeLibFilterOption, NativeLibOption,
    NativeLibPackageSelection, OhosModuleType,
  };
  use arkdown_path::{canonicalize_or_owned, ArtifactPathSerializer};
  use arkdown_session::dependency_graph::snapshot::{
    DependencyGraphSnapshot, DependencyNodeSnapshot,
  };
  use sha2::{Digest, Sha256};
  use std::collections::BTreeMap;
  use std::ffi::OsStr;
  use std::fs;
  use std::io::Read;
  use std::path::{Path, PathBuf};
  use std::sync::Arc;
  use std::time::UNIX_EPOCH;

  const KNOWN_ABI_DIRS: &[&str] = &["arm64-v8a", "armeabi-v7a", "x86_64", "x86", "armeabi"];

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

  fn collect_native_libs(collector: NativeLibCollector<'_>) -> NativeBuildResult<Vec<PathBuf>> {
    collector.configure_cmake_dependencies()?;
    collector.configure_native()?;
    collector.build_native()?;
    let processed = collector.process_libs()?;
    let strip_inputs = if collector.is_binxo_enabled() {
      collector.binxo_libs(&processed)?
    } else {
      processed
    };
    let stripped = collector.strip_libs(&strip_inputs)?;
    collector.cache_libs()?;
    collector.validate_libs(&stripped)?;
    Ok(stripped)
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
      compiler: &NativeCompilerDriver::SdkClang,
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
      &toolchain_file,
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
  fn enable_zig_wraps_the_ohos_toolchain_and_overrides_profile_compilers() {
    let tmp = tempfile::tempdir().unwrap();
    let module_root = tmp.path().join("entry");
    let build_dir = module_root.join(".cxx/default/default/debug/arm64-v8a");
    let output_dir = module_root.join("build/default/intermediates/cmake/default/obj/arm64-v8a");
    let find_root = module_root.join(".cxx/default/default/debug/hvigor/arm64-v8a");
    fs::create_dir_all(&build_dir).unwrap();

    let cmake = tmp.path().join("sdk/native/build-tools/cmake/bin/cmake");
    let ninja = tmp.path().join("sdk/native/build-tools/cmake/bin/ninja");
    let native_dir = tmp.path().join("sdk/native");
    let sdk_toolchain_file = native_dir.join("build/cmake/ohos.toolchain.cmake");
    let sanitizer_runtime = native_dir.join("llvm/lib/clang/15.0.4/lib/aarch64-linux-ohos");
    let cxx_runtime = native_dir.join("llvm/lib/aarch64-linux-ohos");
    fs::create_dir_all(native_dir.join("llvm/include/libcxx-ohos/include/c++/v1")).unwrap();
    fs::create_dir_all(native_dir.join("llvm/lib/clang/15.0.4/include")).unwrap();
    fs::create_dir_all(&cxx_runtime).unwrap();
    fs::create_dir_all(&sanitizer_runtime).unwrap();
    fs::create_dir_all(
      native_dir
        .join("sysroot/usr/include")
        .join("aarch64-linux-ohos"),
    )
    .unwrap();
    fs::create_dir_all(
      native_dir
        .join("sysroot/usr/lib")
        .join("aarch64-linux-ohos"),
    )
    .unwrap();
    for file_name in [
      "libclang_rt.asan.so",
      "libclang_rt.asan_static.a",
      "libclang_rt.asan-preinit.a",
    ] {
      fs::write(sanitizer_runtime.join(file_name), "runtime").unwrap();
    }
    fs::write(cxx_runtime.join("libc++_shared.so"), "runtime").unwrap();
    let zig = tmp.path().join("zig toolchain/bin/zig");
    let compiler = NativeCompilerDriver::Zig {
      executable: zig.clone(),
    };
    let toolchain = ResolvedNativeSdkToolchain {
      cmake: &cmake,
      ninja: &ninja,
      native_dir: &native_dir,
      toolchain_file: &sdk_toolchain_file,
      compiler: &compiler,
    };
    let config = NativeBuildConfig {
      module_root,
      product_name: "default".to_string(),
      target_name: "default".to_string(),
      abi_filters: vec!["arm64-v8a".to_string()],
      enable_cmake_build: true,
      external_native_options: Some(ExternalNativeOptions {
        arguments: vec![
          "-DCMAKE_TOOLCHAIN_FILE=/ignored/project.toolchain.cmake".to_string(),
          "-DOHOS_ENABLE_ASAN=ON".to_string(),
        ],
        ..Default::default()
      }),
      sanitizers: NativeBuildSanitizers::default(),
      build_mode: "debug".to_string(),
      sdk_toolchain: NativeSdkToolchain::default(),
      collect_all_libs: false,
      release_cmake_build_type: NativeReleaseCmakeBuildType::default(),
    };

    let generated_toolchain = config
      .cmake_toolchain_file(&toolchain, &build_dir, "arm64-v8a")
      .expect("Zig toolchain wrapper should be generated");
    let generated = fs::read_to_string(&generated_toolchain).unwrap();
    let serialized_sdk_toolchain =
      ArtifactPathSerializer::path_to_slash_string(&sdk_toolchain_file).unwrap();
    let serialized_zig = ArtifactPathSerializer::path_to_slash_string(&zig).unwrap();
    let wrapper_suffix =
      if arkdown_path::HostPlatform::CURRENT == arkdown_path::HostPlatform::Windows {
        ".cmd"
      } else {
        ""
      };
    let c_wrapper = build_dir.join(format!("arkdown-zig-cc{wrapper_suffix}"));
    let cxx_wrapper = build_dir.join(format!("arkdown-zig-cxx{wrapper_suffix}"));
    let serialized_c_wrapper = ArtifactPathSerializer::path_to_slash_string(&c_wrapper).unwrap();
    assert!(generated.contains(&format!("include([[{serialized_sdk_toolchain}]])")));
    assert!(generated.contains(&format!("set(CMAKE_C_COMPILER [[{serialized_c_wrapper}]])")));
    assert!(!generated.contains("CMAKE_C_COMPILER_ARG1"));
    assert!(!generated.contains("CMAKE_CXX_COMPILER_ARG1"));
    assert!(generated.contains("unset(CMAKE_C_COMPILER_EXTERNAL_TOOLCHAIN)"));
    assert!(generated.contains("unset(CMAKE_CXX_COMPILER_EXTERNAL_TOOLCHAIN)"));
    assert!(
      generated.contains("set_property(TARGET \"${_arkdown_target}\" PROPERTY C_CLANG_TIDY \"\")")
    );
    assert!(generated
      .contains("set_property(TARGET \"${_arkdown_target}\" PROPERTY CXX_CLANG_TIDY \"\")"));
    assert!(generated.contains(
      "cmake_language(DEFER CALL _arkdown_disable_zig_incompatible_clang_tidy \"${CMAKE_SOURCE_DIR}\")"
    ));
    let c_wrapper_content = fs::read_to_string(c_wrapper).unwrap();
    let cxx_wrapper_content = fs::read_to_string(cxx_wrapper).unwrap();
    assert!(c_wrapper_content.contains(&serialized_zig));
    assert!(c_wrapper_content.contains("cc"));
    assert!(c_wrapper_content.contains("sysroot/usr/include"));
    assert!(cxx_wrapper_content.contains(&serialized_zig));
    assert!(cxx_wrapper_content.contains("c++"));
    assert!(cxx_wrapper_content.contains("-nostdinc++"));
    assert!(cxx_wrapper_content.contains("libcxx-ohos/include/c++/v1"));
    assert!(cxx_wrapper_content.contains("lib/clang/15.0.4/include"));
    assert!(cxx_wrapper_content.contains("sysroot/usr/include"));
    assert!(generated.contains("SHELL:-L/usr/lib/aarch64-linux-ohos"));
    assert!(generated.contains("-nostdlib++"));
    assert!(generated.contains("libc++_shared.so"));
    assert!(generated.contains("-Wl,--warn-shared-textrel"));
    assert!(generated.contains("-Wl,--fatal-warnings"));
    assert!(generated.contains("add_link_options("));
    assert!(generated.contains("libclang_rt.asan.so"));
    assert!(generated.contains("libclang_rt.asan_static.a"));
    assert!(generated.contains("libclang_rt.asan-preinit.a"));

    let command = config.cmake_configure_command(
      &toolchain,
      &generated_toolchain,
      "arm64-v8a",
      &config.module_root.join("src/main/cpp"),
      &build_dir,
      &output_dir,
      &find_root,
    );
    let toolchain_arguments = command
      .get_args()
      .filter_map(OsStr::to_str)
      .filter(|argument| argument.starts_with("-DCMAKE_TOOLCHAIN_FILE="))
      .collect::<Vec<_>>();
    assert_eq!(toolchain_arguments.len(), 1);
    assert_eq!(
      toolchain_arguments[0],
      format!("-DCMAKE_TOOLCHAIN_FILE={}", generated_toolchain.display())
    );
  }

  #[test]
  fn compiler_switch_resets_only_cmake_owned_cache_metadata() {
    let tmp = tempfile::tempdir().unwrap();
    let build_dir = tmp.path().join(".cxx/default/default/debug/arm64-v8a");
    let cache_file = build_dir.join("CMakeCache.txt");
    let compiler_metadata = build_dir.join("CMakeFiles");
    let reply_dir = build_dir.join(".cmake/api/v1/reply");
    let object_file = build_dir.join("CMakeFiles/native.dir/native.cpp.o");
    fs::create_dir_all(object_file.parent().unwrap()).unwrap();
    fs::create_dir_all(&reply_dir).unwrap();
    fs::write(&cache_file, "cached compiler").unwrap();
    fs::write(&object_file, "object").unwrap();
    fs::write(reply_dir.join("index.json"), "{}").unwrap();

    let sdk = NativeCompilerDriver::SdkClang;
    let sdk_toolchain_file = tmp
      .path()
      .join("sdk/native/build/cmake/ohos.toolchain.cmake");
    let sdk_state = zig::NativeCmakeCompilerState::new(&build_dir, &sdk, &sdk_toolchain_file);
    sdk_state.prepare().unwrap();
    assert!(cache_file.is_file());
    sdk_state.commit().unwrap();

    let zig = NativeCompilerDriver::Zig {
      executable: tmp.path().join("bin/zig"),
    };
    let zig_state = zig::NativeCmakeCompilerState::new(&build_dir, &zig, &sdk_toolchain_file);
    zig_state.prepare().unwrap();
    assert!(!cache_file.exists());
    assert!(!compiler_metadata.exists());
    assert!(!reply_dir.exists());
    assert!(!object_file.exists());
    zig_state.commit().unwrap();

    fs::create_dir_all(&compiler_metadata).unwrap();
    fs::write(&cache_file, "zig compiler").unwrap();
    sdk_state.prepare().unwrap();
    assert!(!cache_file.exists());
    assert!(!compiler_metadata.exists());
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
      compiler: &NativeCompilerDriver::SdkClang,
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

    let command = config.ninja_command_plan(&toolchain, "arm64-v8a").command();
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
      compiler: &NativeCompilerDriver::SdkClang,
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
    let process_libs = NativeModuleLayout::for_variant(&module_root, "default", "default")
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
    assert!(NativeModuleLayout::for_variant(tmp.path(), "default", "default").has_native_code());
  }

  #[test]
  fn test_module_has_native_code_detects_prebuilt_so() {
    let tmp = tempfile::tempdir().unwrap();
    let libs_dir = tmp.path().join("libs/arm64-v8a");
    fs::create_dir_all(&libs_dir).unwrap();
    fs::write(libs_dir.join("libfoo.so"), b"ELF\x7f").unwrap();
    assert!(NativeModuleLayout::for_variant(tmp.path(), "default", "default").has_native_code());
  }

  #[test]
  fn test_module_has_native_code_detects_versioned_shared_library() {
    let tmp = tempfile::tempdir().unwrap();
    let libs_dir = tmp.path().join("libs/arm64-v8a");
    fs::create_dir_all(&libs_dir).unwrap();
    fs::write(libs_dir.join("libexpat.so.1"), b"ELF\x7f").unwrap();
    assert!(NativeModuleLayout::for_variant(tmp.path(), "default", "default").has_native_code());
  }

  #[test]
  fn test_module_without_native_code_returns_false() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(!NativeModuleLayout::for_variant(tmp.path(), "default", "default").has_native_code());
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
      .with_cmake_local_dependency_inventory(Arc::from([local_dependency]));

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
    let out = cfg.scan().expect("native scan");
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
    let out = cfg.scan().expect("native scan");
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
    let out = cfg.scan().expect("native scan");
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
    let out = cfg.scan().expect("native scan");
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
    let out = cfg.scan().expect("native scan");
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
    let layout = NativeModuleLayout::for_variant(&module_root, "default", "default");
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
    let out = cfg.scan().expect("native scan");
    assert_eq!(out.libs_by_abi.len(), 2);

    let module = ModuleConfig {
      name: "entry".to_string(),
      src_path: tmp.path().to_path_buf(),
      module_type: arkdown_core::OhosModuleType::Entry,
    };
    let deps = DependencyGraphSnapshot::default();
    let libs = collect_native_libs(NativeLibCollector::new(&module, &[], &deps))
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
    let packaged = collect_native_libs(NativeLibCollector::new(&module, &[], &deps))
      .expect("native stages must succeed");

    let layout = NativeModuleLayout::for_variant(&module_root, "default", "default");
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

    NativeLibCacheWriter::for_variant(tmp.path(), "default", "default", &option, false)
      .write()
      .expect("write empty native caches");

    let layout = NativeModuleLayout::for_variant(tmp.path(), "default", "default");
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

    let hashes =
      NativeLibCacheWriter::for_variant(tmp.path(), "default", "default", &option, false)
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
    let layout = NativeModuleLayout::for_variant(tmp.path(), "default", "default");
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

    NativeLibCacheWriter::for_variant(tmp.path(), "default", "default", &option, true)
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
    collect_native_libs(NativeLibCollector::new(&module, &[], &deps))
      .expect("first native collection must succeed");

    fs::remove_file(stale_source).unwrap();
    fs::write(libs_dir.join("libcurrent.so"), b"current").unwrap();
    collect_native_libs(NativeLibCollector::new(&module, &[], &deps))
      .expect("second native collection must succeed");

    let layout = NativeModuleLayout::for_variant(&module_root, "default", "default");
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
    add_dependency_node(
      &mut deps,
      "@example/mtflexbox",
      "@example/mtflexbox",
      "1.0.0",
      dep_root,
    );

    let libs = collect_native_libs(NativeLibCollector::new(&module, &[], &deps))
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

    let libs = collect_native_libs(NativeLibCollector::new(&module, &[], &deps))
      .expect("dependency native libs must be collected from getDefaultLibsDir");
    assert_eq!(libs.len(), 1);
    assert_eq!(libs[0].file_name().unwrap(), "libkept.so");
  }

  #[test]
  fn hvigor_local_har_process_libs_precede_source_tree_libs() {
    let tmp = tempfile::tempdir().unwrap();
    let module_root = tmp.path().join("entry");
    let dependency_root = tmp.path().join("runtime");
    let dependency_process_libs =
      NativeModuleLayout::for_variant(&dependency_root, "paid", "phone")
        .process_libs_dir()
        .join("arm64-v8a");
    let dependency_default_libs = dependency_root.join("libs/arm64-v8a");
    fs::create_dir_all(&module_root).unwrap();
    fs::create_dir_all(&dependency_process_libs).unwrap();
    fs::create_dir_all(&dependency_default_libs).unwrap();
    fs::write(
      dependency_process_libs.join("libmsi.so"),
      b"built C++ output",
    )
    .unwrap();
    fs::write(
      dependency_default_libs.join("libfallback.so"),
      b"source fallback",
    )
    .unwrap();

    let module = ModuleConfig {
      name: "entry".to_string(),
      src_path: module_root,
      module_type: OhosModuleType::Entry,
    };
    let mut deps = DependencyGraphSnapshot::default();
    add_dependency_node(
      &mut deps,
      "@msi/jsi",
      "@msi/jsi",
      "1.0.0",
      dependency_root.clone(),
    );
    let local_dependency = NativeCmakeLocalDependency::new(
      dependency_root,
      "paid".to_string(),
      "phone".to_string(),
      "debug".to_string(),
      OhosModuleType::Har,
      NativeLibOption::default(),
    );
    let policy = NativeLibCollectionPolicy::default()
      .with_cmake_local_dependency_inventory(Arc::from([local_dependency]));

    let abi_filters = ["arm64-v8a".to_string()];
    let collector = NativeLibCollector::new(&module, &abi_filters, &deps).with_policy(policy);
    let dependency_dirs = collector.dependency_default_lib_dirs();
    assert_eq!(dependency_dirs.len(), 2);
    assert_eq!(
      dependency_dirs[0].libs_dir,
      canonicalize_or_owned(dependency_process_libs.parent().unwrap())
    );
    assert_eq!(
      dependency_dirs[1].libs_dir,
      canonicalize_or_owned(dependency_default_libs.parent().unwrap())
    );

    let outputs = collector
      .process_libs()
      .expect("local HAR ProcessLibs output must be collected");
    let names = outputs
      .iter()
      .filter_map(|path| path.file_name().and_then(OsStr::to_str))
      .collect::<Vec<_>>();

    assert_eq!(names, ["libfallback.so", "libmsi.so"]);
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
    let libs = collect_native_libs(NativeLibCollector::new(&module, &[], &deps).with_policy(
      NativeLibCollectionPolicy::default().with_native_lib_option(NativeLibOption {
        collect_all_libs: true,
        filter: NativeLibFilterOption::default(),
        ..Default::default()
      }),
    ))
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
    let libs = collect_native_libs(NativeLibCollector::new(&module, &[], &deps).with_policy(
      NativeLibCollectionPolicy::default().with_native_lib_option(NativeLibOption {
        collect_all_libs: false,
        filter: NativeLibFilterOption {
          excludes: vec!["**/libexcluded.so".to_string()],
          ..Default::default()
        },
        ..Default::default()
      }),
    ))
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

    let libs = collect_native_libs(NativeLibCollector::new(&module, &[], &deps).with_policy(
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
    ))
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
          installed_alias_path: None,
          kind: arkdown_session::dependency_graph::snapshot::DependencyEdgeKind::Dependency,
          is_transitive: false,
        },
      );
    }

    let libs = collect_native_libs(NativeLibCollector::new(&module, &[], &deps).with_policy(
      NativeLibCollectionPolicy::default().with_native_lib_option(NativeLibOption {
        collect_all_libs: false,
        filter: NativeLibFilterOption {
          enable_override: true,
          ..Default::default()
        },
        ..Default::default()
      }),
    ))
    .expect("enableOverride must use latest hvigor dependency traversal order");
    assert_eq!(libs.len(), 1);
    assert_eq!(fs::read(&libs[0]).unwrap(), b"ELF A");

    let libs = collect_native_libs(NativeLibCollector::new(&module, &[], &deps).with_policy(
      NativeLibCollectionPolicy::default().with_native_lib_option(NativeLibOption {
        collect_all_libs: false,
        filter: NativeLibFilterOption {
          pick_lasts: vec!["**/libdup.so".to_string()],
          ..Default::default()
        },
        ..Default::default()
      }),
    ))
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

    let err = collect_native_libs(NativeLibCollector::new(&module, &[], &deps))
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

    let libs = collect_native_libs(NativeLibCollector::new(&module, &[], &deps))
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
    let processor =
      NativeStripProcessor::for_variant(tmp.path(), "default", "default", None, &option);
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
    let processor =
      NativeStripProcessor::for_variant(tmp.path(), "default", "default", None, &inverted);
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

    let libs = collect_native_libs(NativeLibCollector::new(&module, &[], &deps))
      .expect("HAR dependency native libraries should be excluded by default");
    assert!(libs.is_empty());

    let libs = collect_native_libs(NativeLibCollector::new(&module, &[], &deps).with_policy(
      NativeLibCollectionPolicy::default().with_native_lib_option(NativeLibOption {
        exclude_from_har: false,
        ..Default::default()
      }),
    ))
    .expect("excludeFromHar=false should include dependency native libraries");
    assert_eq!(libs.len(), 1);
    assert_eq!(libs[0].file_name().unwrap(), "libdependency.so");
  }
}
