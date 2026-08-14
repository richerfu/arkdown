//! Native build inventory.
//!
//! [`NativeBuildInventory`] captures CMake targets, Ninja outputs, stripped
//! native libraries, and debug symbol policies for a module's native (C/C++)
//! build. It is the Rust equivalent of the native task initializer and
//! `ProcessLibs` / `DoNativeStrip` / `CacheNativeLibs` / `CollectDebugSymbol`
//! tasks in hvigor.
//!
//! # Upstream correspondence
//!
//! | Rust type                         | hvigor task / file                              |
//! |-----------------------------------|-------------------------------------------------|
//! | [`NativeBuildInventory`]          | `native-task-initializer.js`, `ProcessLibs`     |
//! | [`NativeBuildTarget`]             | CMake target entry in `CMakeLists.txt`          |
//! | [`NativeAbi`]                     | ABI list in `build-profile.json5`               |
//! | [`NativeLibCandidate`]            | `CacheNativeLibs` lib path record               |
//! | [`NativeDebugSymbolPolicy`]       | `DoNativeStrip` / `CollectDebugSymbol`          |

use std::path::{Path, PathBuf};

use crate::module::snapshot::ModuleSnapshot;

// ---------------------------------------------------------------------------
// NativeAbi
// ---------------------------------------------------------------------------

/// Target ABI for a native library build.
///
/// Upstream: ABI strings in `build-profile.json5` `abiFilters` and
/// `native-task-initializer.js`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeAbi {
    /// 64-bit ARM (`arm64-v8a`).
    Arm64V8a,
    /// 32-bit ARM (`armeabi-v7a`).
    ArmV7a,
    /// 64-bit x86 (`x86_64`).
    X86_64,
    /// 32-bit x86 (`x86`).
    X86,
}

impl NativeAbi {
    /// Return the canonical ABI string used in build-profile.json5.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Arm64V8a => "arm64-v8a",
            Self::ArmV7a => "armeabi-v7a",
            Self::X86_64 => "x86_64",
            Self::X86 => "x86",
        }
    }

    /// Parse an ABI string from `build-profile.json5`.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "arm64-v8a" => Some(Self::Arm64V8a),
            "armeabi-v7a" => Some(Self::ArmV7a),
            "x86_64" => Some(Self::X86_64),
            "x86" => Some(Self::X86),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// NativeBuildTarget
// ---------------------------------------------------------------------------

/// A single CMake build target for a native library.
///
/// Upstream: CMake target entry in `CMakeLists.txt`, discovered by
/// `native-task-initializer.js`.
#[derive(Debug, Clone)]
pub struct NativeBuildTarget {
    /// CMake target name (e.g., `"mylib"`).
    pub target_name: String,
    /// Absolute path to the `CMakeLists.txt` file.
    pub cmake_file: PathBuf,
    /// ABI(s) this target is built for.
    pub abis: Vec<NativeAbi>,
}

impl NativeBuildTarget {
    /// Construct a new native build target.
    pub fn new(target_name: String, cmake_file: PathBuf, abis: Vec<NativeAbi>) -> Self {
        Self {
            target_name,
            cmake_file,
            abis,
        }
    }

    /// Return the target name.
    pub fn name(&self) -> &str {
        &self.target_name
    }

    /// Return the CMake file path.
    pub fn cmake_file(&self) -> &Path {
        &self.cmake_file
    }
}

// ---------------------------------------------------------------------------
// NativeLibCandidate
// ---------------------------------------------------------------------------

/// A candidate native library file (`.so`) to be packaged.
///
/// Upstream: `CacheNativeLibs` lib path record in
/// `native-task-initializer.js`.
#[derive(Debug, Clone)]
pub struct NativeLibCandidate {
    /// Absolute path to the `.so` file.
    pub lib_path: PathBuf,
    /// Target ABI for this library.
    pub abi: NativeAbi,
    /// Whether this library has been stripped of debug symbols.
    pub is_stripped: bool,
}

impl NativeLibCandidate {
    /// Construct a new native lib candidate.
    pub fn new(lib_path: PathBuf, abi: NativeAbi) -> Self {
        Self {
            lib_path,
            abi,
            is_stripped: false,
        }
    }

    /// Return the library path.
    pub fn as_path(&self) -> &Path {
        &self.lib_path
    }

    /// Return the library file name (without directory).
    pub fn file_name(&self) -> Option<&std::ffi::OsStr> {
        self.lib_path.file_name()
    }
}

// ---------------------------------------------------------------------------
// NativeDebugSymbolPolicy
// ---------------------------------------------------------------------------

/// Policy for debug symbol handling during native library packaging.
///
/// Upstream: `DoNativeStrip` / `CollectDebugSymbol` tasks in hvigor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeDebugSymbolPolicy {
    /// Strip debug symbols from the packaged `.so` files.
    ///
    /// Upstream: `DoNativeStrip` — calls `llvm-strip` on output libs.
    StripLibs,
    /// Preserve debug symbols in the packaged `.so` files.
    ///
    /// Upstream: `CollectDebugSymbol` — copies unstripped libs to symbol dir.
    PreserveDebugSymbols,
}

// ---------------------------------------------------------------------------
// NativeBuildInventory
// ---------------------------------------------------------------------------

/// Inventory of native build inputs and outputs for a module.
///
/// Captures CMake targets, Ninja build outputs, stripped library candidates,
/// and the debug symbol policy. Built once at session startup from the module
/// root and `build-profile.json5` settings.
///
/// Upstream: `native-task-initializer.js`, `ProcessLibs`, `DoNativeStrip`,
/// `CacheNativeLibs`, `CollectDebugSymbol`.
#[derive(Debug, Clone)]
pub struct NativeBuildInventory {
    /// CMake targets discovered for this module.
    pub cmake_targets: Vec<NativeBuildTarget>,
    /// Expected Ninja output paths (`.so` files per ABI).
    pub ninja_outputs: Vec<PathBuf>,
    /// Stripped library candidates for packaging.
    pub stripped_libs: Vec<NativeLibCandidate>,
    /// Unstripped library candidates for debug symbol collection.
    pub debug_symbol_inputs: Vec<NativeLibCandidate>,
    /// Debug symbol policy.
    pub debug_symbol_policy: NativeDebugSymbolPolicy,
    /// Content fingerprint of the native build inputs.
    pub fingerprint: u64,
}

impl NativeBuildInventory {
    /// Discover native build inputs from a module snapshot.
    ///
    /// Scans the module root for a `CMakeLists.txt` and populates targets.
    /// Returns an empty inventory when no native build is configured.
    ///
    /// Upstream: `native-task-initializer.js` initialization.
    pub fn discover(module: &ModuleSnapshot, abis: &[NativeAbi]) -> Self {
        let cmake_file = module.root.join("CMakeLists.txt");
        let cmake_targets = if cmake_file.is_file() {
            vec![NativeBuildTarget::new(
                module.name.clone(),
                cmake_file,
                abis.to_vec(),
            )]
        } else {
            vec![]
        };

        Self {
            cmake_targets,
            ninja_outputs: vec![],
            stripped_libs: vec![],
            debug_symbol_inputs: vec![],
            debug_symbol_policy: NativeDebugSymbolPolicy::StripLibs,
            fingerprint: 0,
        }
    }

    /// Return an iterator over all CMake targets.
    ///
    /// Upstream: `native-task-initializer.js` target list.
    pub fn cmake_targets(&self) -> impl Iterator<Item = &NativeBuildTarget> {
        self.cmake_targets.iter()
    }

    /// Return an iterator over expected Ninja output paths.
    ///
    /// Upstream: `ProcessLibs` output path list.
    pub fn ninja_outputs(&self) -> impl Iterator<Item = &PathBuf> {
        self.ninja_outputs.iter()
    }

    /// Return an iterator over stripped library candidates.
    ///
    /// Upstream: `DoNativeStrip` / `CacheNativeLibs` output.
    pub fn stripped_libs(&self) -> impl Iterator<Item = &NativeLibCandidate> {
        self.stripped_libs.iter()
    }

    /// Return an iterator over debug symbol input libraries.
    ///
    /// Upstream: `CollectDebugSymbol` input.
    pub fn debug_symbol_inputs(&self) -> impl Iterator<Item = &NativeLibCandidate> {
        self.debug_symbol_inputs.iter()
    }

    /// Return the content fingerprint of the native build inputs.
    ///
    /// Used for cache invalidation.
    pub fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    /// Return `true` if this module has any native build targets.
    pub fn has_native_build(&self) -> bool {
        !self.cmake_targets.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn dummy_module(root: &str, name: &str) -> ModuleSnapshot {
        ModuleSnapshot {
            name: name.to_string(),
            root: PathBuf::from(root),
            src_main: PathBuf::from(root).join("src/main"),
            module_json_path: None,
            module_json: None,
            module_json_value: None,
            ohos_test_module_json_value: None,
            module_type: crate::module::snapshot::OhosModuleType::Entry,
            package_path: None,
            package: None,
            source_roots: vec![],
            resource_roots: vec![],
            related_entry_modules: None,
        }
    }

    #[test]
    fn empty_inventory_when_no_cmake() {
        let module = dummy_module("/tmp/no_cmake_module", "entry");
        let inv = NativeBuildInventory::discover(&module, &[NativeAbi::Arm64V8a]);
        assert!(!inv.has_native_build());
        assert_eq!(inv.cmake_targets().count(), 0);
    }

    #[test]
    fn native_abi_round_trip() {
        assert_eq!(NativeAbi::Arm64V8a.as_str(), "arm64-v8a");
        assert_eq!(NativeAbi::parse("arm64-v8a"), Some(NativeAbi::Arm64V8a));
        assert_eq!(NativeAbi::parse("x86_64"), Some(NativeAbi::X86_64));
        assert_eq!(NativeAbi::parse("unknown"), None);
    }

    #[test]
    fn native_lib_candidate_fields() {
        let path = PathBuf::from("/out/arm64-v8a/libfoo.so");
        let candidate = NativeLibCandidate::new(path.clone(), NativeAbi::Arm64V8a);
        assert_eq!(candidate.as_path(), path.as_path());
        assert!(!candidate.is_stripped);
        assert_eq!(candidate.abi, NativeAbi::Arm64V8a);
    }

    #[test]
    fn native_build_target_name() {
        let target = NativeBuildTarget::new(
            "mylib".to_string(),
            PathBuf::from("/module/CMakeLists.txt"),
            vec![NativeAbi::Arm64V8a],
        );
        assert_eq!(target.name(), "mylib");
        assert_eq!(
            target.cmake_file(),
            PathBuf::from("/module/CMakeLists.txt").as_path()
        );
    }

    #[test]
    fn debug_symbol_policy_variants() {
        let strip = NativeDebugSymbolPolicy::StripLibs;
        let preserve = NativeDebugSymbolPolicy::PreserveDebugSymbols;
        assert_ne!(strip, preserve);
    }
}
