//! Native (C/C++) build integration for OHOS modules.
//!
//! Implements the "native artifact import/build 模式" gap from the hvigor
//! analysis (sections 3.1 and 3.8): "增加 native artifact import/build 模式".
//!
//! ## Modes
//!
//! ### Import mode (always available)
//!
//! Collects pre-built `.so` files from the paths that DevEco / hvigor produces:
//! - `src/main/libs/<abi>/` — hand-placed native libraries
//! - `build/default/intermediates/cmake/default/obj/<abi>/` — CMake output
//! - `build/default/intermediates/ndk/default/<abi>/` — NDK output
//!
//! This mode is used when the native code has already been compiled by a
//! previous `hvigorw assembleNative` invocation (or when developers provide
//! pre-built `.so` files).
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
//! - Running the native build automatically when all tools are available.
//! - Falling back to the import-mode .so collection otherwise.

use std::path::{Path, PathBuf};

/// The set of ABI directories that hvigor's native pipeline produces under
/// the CMake / NDK build intermediates.
const KNOWN_ABI_DIRS: &[&str] = &["arm64-v8a", "armeabi-v7a", "x86_64", "x86", "armeabi"];

/// Result of a native build or import scan.
#[derive(Debug, Clone, Default)]
pub struct NativeBuildOutput {
    /// Absolute paths to `.so` files ready for packaging, indexed by ABI.
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
    /// Whether to attempt an actual CMake build (requires NDK + CMake on PATH).
    pub enable_cmake_build: bool,
}

impl NativeBuildConfig {
    /// Scan and optionally build native libraries for this module.
    ///
    /// Returns all `.so` files that should be included in the HAP/HSP artifact.
    ///
    /// ## Algorithm
    ///
    /// 1. Check for `CMakeLists.txt` in `src/main/cpp/`.  If present and
    ///    `enable_cmake_build` is true, attempt a CMake build.
    /// 2. Collect pre-built `.so` files from standard output locations
    ///    (CMake build output, NDK output, hand-placed `src/main/libs/`).
    /// 3. Return all discovered `.so` files whose ABI matches `abi_filters`.
    pub fn run(&self) -> NativeBuildOutput {
        let mut output = NativeBuildOutput::default();
        let abi_filters = effective_abi_filters(&self.abi_filters);

        // ── Step 1: detect CMakeLists.txt ──────────────────────────────────
        let cmake_file = self.module_root.join("src/main/cpp/CMakeLists.txt");
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
                    output.warnings.push(format!(
                        "native: CMake build failed for '{}': {}. \
                         Falling back to pre-built library import.",
                        self.module_root.display(),
                        e
                    ));
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

        // ── Step 2: collect pre-built .so files ────────────────────────────
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
                    .filter(|p| p.extension().map(|ext| ext == "so").unwrap_or(false))
                    .collect();
                sorted.sort();
                for so_path in sorted {
                    output.libs_by_abi.push((abi.clone(), so_path));
                }
            }
        }

        if output.libs_by_abi.is_empty() && has_cmake {
            output.warnings.push(format!(
                "native: no .so files found for module '{}' after build/import scan. \
                 Expected paths: {:?}",
                self.module_root.display(),
                search_roots
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
            ));
        }

        output
    }

    /// Directories to scan for pre-built `.so` files, in priority order.
    fn so_search_roots(&self) -> Vec<PathBuf> {
        vec![
            // CMake build output (standard hvigor location).
            self.module_root
                .join("build/default/intermediates/cmake/default/obj"),
            // NDK build output.
            self.module_root
                .join("build/default/intermediates/ndk/default"),
            // Hand-placed native libraries (top-level libs/).
            self.module_root.join("libs"),
            // src/main/libs — common pattern from DevEco templates.
            self.module_root.join("src/main/libs"),
        ]
    }

    /// Attempt a CMake build using the OHOS NDK toolchain.
    ///
    /// Requires:
    /// - `cmake` on `$PATH`
    /// - `$OHOS_NDK_HOME` or `$OHOS_SDK_HOME` pointing to the NDK root
    fn run_cmake_build(&self) -> Result<(), String> {
        let cmake = which_cmake()?;
        let toolchain = ohos_cmake_toolchain()?;

        let build_dir = self
            .module_root
            .join("build/default/intermediates/cmake/default");
        std::fs::create_dir_all(&build_dir)
            .map_err(|e| format!("cannot create CMake build dir: {e}"))?;

        let source_dir = self.module_root.join("src/main/cpp");
        let abi_flags: Vec<String> = effective_abi_filters(&self.abi_filters)
            .iter()
            .map(|abi| format!("-DOHOS_ARCH={}", abi))
            .collect();

        // cmake configure
        let mut configure_cmd = std::process::Command::new(&cmake);
        configure_cmd
            .arg("-G")
            .arg("Ninja")
            .arg(format!("-DCMAKE_TOOLCHAIN_FILE={}", toolchain.display()))
            .arg("-DCMAKE_BUILD_TYPE=Release")
            .args(&abi_flags)
            .arg(source_dir.display().to_string())
            .current_dir(&build_dir);

        tracing::info!(
            cmd = ?configure_cmd,
            "native: running CMake configure"
        );

        let configure_status = configure_cmd
            .status()
            .map_err(|e| format!("failed to run cmake configure: {e}"))?;

        if !configure_status.success() {
            return Err(format!(
                "cmake configure exited with status {configure_status}"
            ));
        }

        // cmake build
        let mut build_cmd = std::process::Command::new(&cmake);
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
            cmd = ?build_cmd,
            "native: running CMake build"
        );

        let build_status = build_cmd
            .status()
            .map_err(|e| format!("failed to run cmake build: {e}"))?;

        if !build_status.success() {
            return Err(format!("cmake build exited with status {build_status}"));
        }

        Ok(())
    }
}

/// Detect the `cmake` binary on `$PATH`.
fn which_cmake() -> Result<PathBuf, String> {
    // Try `cmake` directly (relies on $PATH).
    let output = std::process::Command::new("cmake")
        .arg("--version")
        .output();
    match output {
        Ok(out) if out.status.success() => Ok(PathBuf::from("cmake")),
        _ => Err(
            "cmake not found on $PATH. Install CMake (>= 3.16) to enable native builds. \
             Download from https://cmake.org/download/ or use your package manager."
                .to_string(),
        ),
    }
}

/// Locate the OHOS CMake toolchain file from the NDK.
///
/// Checks `$OHOS_NDK_HOME` first, then `$OHOS_SDK_HOME/default/openharmony/native`.
fn ohos_cmake_toolchain() -> Result<PathBuf, String> {
    let candidates = [
        // $OHOS_NDK_HOME/build/cmake/ohos.toolchain.cmake
        std::env::var_os("OHOS_NDK_HOME")
            .map(|h| PathBuf::from(h).join("build/cmake/ohos.toolchain.cmake")),
        // $OHOS_SDK_HOME/<api>/native/build/cmake/ohos.toolchain.cmake
        std::env::var_os("OHOS_SDK_HOME").map(|h| {
            PathBuf::from(h).join("default/openharmony/native/build/cmake/ohos.toolchain.cmake")
        }),
    ];

    for candidate in candidates.into_iter().flatten() {
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    Err(
        "OHOS NDK toolchain file not found. Set $OHOS_NDK_HOME or $OHOS_SDK_HOME. \
         Download the OHOS NDK from https://developer.huawei.com/consumer/en/deveco-studio/ \
         and set OHOS_NDK_HOME to its root directory."
            .to_string(),
    )
}

/// Expand an empty ABI filter list to the default set.
fn effective_abi_filters(filters: &[String]) -> Vec<String> {
    if filters.is_empty() {
        vec!["arm64-v8a".to_string()]
    } else {
        filters.to_vec()
    }
}

/// Probe whether a module has any native source code.
///
/// Returns `true` when `CMakeLists.txt` is found in `src/main/cpp/` or
/// when pre-built `.so` files exist under `src/main/libs/` or `libs/`.
pub fn module_has_native_code(module_root: &Path) -> bool {
    // CMake-based native source.
    if module_root.join("src/main/cpp/CMakeLists.txt").is_file() {
        return true;
    }
    // Pre-built native libraries.
    for dir in &["src/main/libs", "libs"] {
        let libs_dir = module_root.join(dir);
        if libs_dir.is_dir() {
            for abi in KNOWN_ABI_DIRS {
                let abi_dir = libs_dir.join(abi);
                if abi_dir.is_dir() {
                    if let Ok(mut entries) = std::fs::read_dir(&abi_dir) {
                        if entries.any(|e| {
                            e.ok()
                                .and_then(|e| e.path().extension().map(|ext| ext == "so"))
                                .unwrap_or(false)
                        }) {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
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
        assert!(module_has_native_code(tmp.path()));
    }

    #[test]
    fn test_module_has_native_code_detects_prebuilt_so() {
        let tmp = tempfile::tempdir().unwrap();
        let libs_dir = tmp.path().join("src/main/libs/arm64-v8a");
        fs::create_dir_all(&libs_dir).unwrap();
        fs::write(libs_dir.join("libfoo.so"), b"ELF\x7f").unwrap();
        assert!(module_has_native_code(tmp.path()));
    }

    #[test]
    fn test_module_without_native_code_returns_false() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!module_has_native_code(tmp.path()));
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
        };
        let out = cfg.run();
        assert_eq!(out.libs_by_abi.len(), 1);
        assert_eq!(out.libs_by_abi[0].0, "arm64-v8a");
        assert!(out.libs_by_abi[0].1.file_name().unwrap() == "libnative.so");
    }

    #[test]
    fn test_effective_abi_filters_defaults_to_arm64() {
        let filtered = effective_abi_filters(&[]);
        assert_eq!(filtered, vec!["arm64-v8a"]);
    }

    #[test]
    fn test_effective_abi_filters_preserves_explicit() {
        let filtered = effective_abi_filters(&["armeabi-v7a".to_string(), "x86_64".to_string()]);
        assert_eq!(filtered, vec!["armeabi-v7a", "x86_64"]);
    }
}
