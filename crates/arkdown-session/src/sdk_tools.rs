//! SDK tool path resolution.
//!
//! [`SdkToolResolver`] resolves paths to OHOS SDK command-line tools from an
//! [`SdkSnapshot`]. All logic mirrors `DefaultSdk` getter methods in
//! `src/sdk/default-sdk.js`.
//!
//! # Upstream correspondence
//!
//! | Rust method                            | hvigor `DefaultSdk` getter               |
//! |----------------------------------------|------------------------------------------|
//! | [`SdkToolResolver::restool`]           | `DefaultSdk.getRestool`                  |
//! | [`SdkToolResolver::package_tool`]      | `DefaultSdk.getPackageTool`              |
//! | [`SdkToolResolver::hap_sign_tool_v2`]  | `DefaultSdk.getHapSignToolV2`            |
//! | [`SdkToolResolver::hap_to_bin`]        | `DefaultSdk.getHapTobin`                 |
//! | [`SdkToolResolver::syscap_tool`]       | `DefaultSdk.getSysCapTool`               |
//! | [`SdkToolResolver::verify_sign_config_tool`] | `DefaultSdk.getVerifySignConfigTool` |
//! | [`SdkToolResolver::cmake_tool`]        | `DefaultSdk.getCmakeTool`                |
//! | [`SdkToolResolver::native_ninja_tool`] | `DefaultSdk.getNativeNinjaTool`          |
//! | [`SdkToolResolver::llvm_strip`]        | `DefaultSdk.getSdkLlvmStrip`             |
//! | [`SdkToolResolver::llvm_read_elf`]     | `DefaultSdk.getSdkLlvmReadElf`           |

use std::path::{Path, PathBuf};

use crate::sdk::snapshot::SdkSnapshot;

// ---------------------------------------------------------------------------
// SdkToolKind
// ---------------------------------------------------------------------------

/// Enumeration of all OHOS SDK command-line tools.
///
/// Used as a typed key for tool path lookups and error messages.
///
/// Upstream: tool names in `DefaultSdk` in `src/sdk/default-sdk.js`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SdkToolKind {
    /// `restool` — resource compiler. Replaced by `arkdown-resource` (SPEC-011).
    Restool,
    /// `package-tool` — HAP/HSP/HAR packager.
    PackageTool,
    /// `hap-sign-tool.jar` v2 — HAP signing. Replaced by `arkdown-sign` (SPEC-021).
    HapSignToolV2,
    /// `hap-to-bin` — HAP-to-binary converter.
    HapToBin,
    /// `syscap-tool` — system capability validator.
    SyscapTool,
    /// `verify-sign-config` — signing config verifier.
    VerifySignConfigTool,
    /// `cmake` — CMake build system (native builds).
    Cmake,
    /// `ninja` — Ninja build system (native builds).
    NativeNinja,
    /// `llvm-strip` — LLVM strip (native debug symbol removal).
    LlvmStrip,
    /// `llvm-readelf` — LLVM readelf (native ELF inspection).
    LlvmReadElf,
}

impl SdkToolKind {
    /// Return the canonical tool binary name.
    pub fn binary_name(self) -> &'static str {
        match self {
            Self::Restool => "restool",
            Self::PackageTool => "package-tool",
            Self::HapSignToolV2 => "hap-sign-tool.jar",
            Self::HapToBin => "hap-to-bin",
            Self::SyscapTool => "syscap_tool",
            Self::VerifySignConfigTool => "verify-sign-config",
            Self::Cmake => "cmake",
            Self::NativeNinja => "ninja",
            Self::LlvmStrip => "llvm-strip",
            Self::LlvmReadElf => "llvm-readelf",
        }
    }
}

// ---------------------------------------------------------------------------
// SdkToolPath
// ---------------------------------------------------------------------------

/// A resolved path to a single SDK tool binary.
///
/// Wraps a [`PathBuf`] and records which tool kind it points to.
///
/// Upstream: return value of `DefaultSdk.getRestool()` etc.
#[derive(Debug, Clone)]
pub struct SdkToolPath {
    /// The resolved absolute path to the tool binary.
    pub path: PathBuf,
    /// Which tool this path refers to.
    pub kind: SdkToolKind,
}

impl SdkToolPath {
    /// Construct a new tool path.
    pub fn new(path: PathBuf, kind: SdkToolKind) -> Self {
        Self { path, kind }
    }

    /// Return a reference to the inner path.
    pub fn as_path(&self) -> &Path {
        &self.path
    }
}

// ---------------------------------------------------------------------------
// SdkToolResolver
// ---------------------------------------------------------------------------

/// Resolves OHOS SDK tool paths from an [`SdkSnapshot`].
///
/// All methods mirror the `DefaultSdk` getter family in
/// `src/sdk/default-sdk.js`.
///
/// Note: `arkdown-resource` (SPEC-011) replaces `restool`,
/// `arkdown-sign` (SPEC-021) replaces `hap-sign-tool.jar`. The resolver
/// still exposes these paths for SDK-level debugging and compatibility
/// checks.
#[derive(Debug, Clone)]
pub struct SdkToolResolver {
    sdk_root: PathBuf,
}

impl SdkToolResolver {
    /// Create a resolver from an [`SdkSnapshot`].
    pub fn new(sdk: &SdkSnapshot) -> Self {
        Self {
            sdk_root: sdk.sdk_root.clone(),
        }
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Resolve a tool path under `<sdk_root>/toolchains/<subpath>` with an
    /// explicit kind.
    fn tool_at(&self, subpath: &str, kind: SdkToolKind) -> Option<SdkToolPath> {
        let path = self.sdk_root.join("toolchains").join(subpath);
        Some(SdkToolPath::new(path, kind))
    }

    /// Return a tool path by kind.
    pub fn tool_path(&self, kind: SdkToolKind) -> Option<SdkToolPath> {
        match kind {
            SdkToolKind::Restool => self.restool(),
            SdkToolKind::PackageTool => self.package_tool(),
            SdkToolKind::HapSignToolV2 => self.hap_sign_tool_v2(),
            SdkToolKind::HapToBin => self.hap_to_bin(),
            SdkToolKind::SyscapTool => self.syscap_tool(),
            SdkToolKind::VerifySignConfigTool => self.verify_sign_config_tool(),
            SdkToolKind::Cmake => self.cmake_tool(),
            SdkToolKind::NativeNinja => self.native_ninja_tool(),
            SdkToolKind::LlvmStrip => self.llvm_strip(),
            SdkToolKind::LlvmReadElf => self.llvm_read_elf(),
        }
    }

    // -----------------------------------------------------------------------
    // Public API — mirrors DefaultSdk getters
    // -----------------------------------------------------------------------

    /// Path to `restool`.
    ///
    /// Upstream: `DefaultSdk.getRestool`.
    /// Note: replaced by `arkdown-resource` (SPEC-011) at compile time.
    pub fn restool(&self) -> Option<SdkToolPath> {
        self.tool_at("restool", SdkToolKind::Restool)
    }

    /// Path to `package-tool`.
    ///
    /// Upstream: `DefaultSdk.getPackageTool`.
    pub fn package_tool(&self) -> Option<SdkToolPath> {
        self.tool_at("package-tool", SdkToolKind::PackageTool)
    }

    /// Path to `hap-sign-tool.jar` (v2).
    ///
    /// Upstream: `DefaultSdk.getHapSignToolV2`.
    /// Note: replaced by `arkdown-sign` (SPEC-021) at compile time.
    pub fn hap_sign_tool_v2(&self) -> Option<SdkToolPath> {
        let path = self
            .sdk_root
            .join("toolchains")
            .join("lib")
            .join("hap-sign-tool.jar");
        Some(SdkToolPath::new(path, SdkToolKind::HapSignToolV2))
    }

    /// Path to `hap-to-bin`.
    ///
    /// Upstream: `DefaultSdk.getHapTobin`.
    pub fn hap_to_bin(&self) -> Option<SdkToolPath> {
        self.tool_at("hap-to-bin", SdkToolKind::HapToBin)
    }

    /// Path to `syscap_tool`.
    ///
    /// Upstream: `DefaultSdk.getSysCapTool`.
    pub fn syscap_tool(&self) -> Option<SdkToolPath> {
        let binary = if cfg!(windows) {
            "syscap_tool.exe"
        } else {
            "syscap_tool"
        };
        self.tool_at(binary, SdkToolKind::SyscapTool)
    }

    /// Path to `verify-sign-config`.
    ///
    /// Upstream: `DefaultSdk.getVerifySignConfigTool`.
    pub fn verify_sign_config_tool(&self) -> Option<SdkToolPath> {
        self.tool_at("verify-sign-config", SdkToolKind::VerifySignConfigTool)
    }

    /// Path to the CMake binary bundled with the OHOS native SDK.
    ///
    /// Upstream: `DefaultSdk.getCmakeTool`.
    pub fn cmake_tool(&self) -> Option<SdkToolPath> {
        let path = self
            .sdk_root
            .join("native")
            .join("build-tools")
            .join("cmake")
            .join("bin")
            .join("cmake");
        Some(SdkToolPath::new(path, SdkToolKind::Cmake))
    }

    /// Path to the Ninja binary bundled with the OHOS native SDK.
    ///
    /// Upstream: `DefaultSdk.getNativeNinjaTool`.
    pub fn native_ninja_tool(&self) -> Option<SdkToolPath> {
        let path = self
            .sdk_root
            .join("native")
            .join("build-tools")
            .join("cmake")
            .join("bin")
            .join("ninja");
        Some(SdkToolPath::new(path, SdkToolKind::NativeNinja))
    }

    /// Path to `llvm-strip` from the OHOS LLVM toolchain.
    ///
    /// Upstream: `DefaultSdk.getSdkLlvmStrip`.
    pub fn llvm_strip(&self) -> Option<SdkToolPath> {
        let path = self
            .sdk_root
            .join("native")
            .join("llvm")
            .join("bin")
            .join("llvm-strip");
        Some(SdkToolPath::new(path, SdkToolKind::LlvmStrip))
    }

    /// Path to `llvm-readelf` from the OHOS LLVM toolchain.
    ///
    /// Upstream: `DefaultSdk.getSdkLlvmReadElf`.
    pub fn llvm_read_elf(&self) -> Option<SdkToolPath> {
        let path = self
            .sdk_root
            .join("native")
            .join("llvm")
            .join("bin")
            .join("llvm-readelf");
        Some(SdkToolPath::new(path, SdkToolKind::LlvmReadElf))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    struct TestSdk;

    impl TestSdk {
        fn snapshot(root: &str) -> SdkSnapshot {
            SdkSnapshot::from_parts(PathBuf::from(root), 12, "5.0.0.47".to_string())
        }
    }

    #[test]
    fn resolver_returns_paths_from_sdk_root() {
        let sdk = TestSdk::snapshot("/sdk/ohos/12");
        let resolver = SdkToolResolver::new(&sdk);
        assert_eq!(
            resolver.package_tool().unwrap().path,
            PathBuf::from("/sdk/ohos/12/toolchains/package-tool")
        );
    }

    #[test]
    fn restool_path_under_toolchains() {
        let sdk = TestSdk::snapshot("/sdk/ohos/12");
        let resolver = SdkToolResolver::new(&sdk);
        let path = resolver.restool().unwrap();
        assert_eq!(path.path, PathBuf::from("/sdk/ohos/12/toolchains/restool"));
        assert_eq!(path.kind, SdkToolKind::Restool);
    }

    #[test]
    fn hap_sign_tool_v2_under_lib() {
        let sdk = TestSdk::snapshot("/sdk/ohos/12");
        let resolver = SdkToolResolver::new(&sdk);
        let path = resolver.hap_sign_tool_v2().unwrap();
        assert_eq!(
            path.path,
            PathBuf::from("/sdk/ohos/12/toolchains/lib/hap-sign-tool.jar")
        );
    }

    #[test]
    fn cmake_under_build_tools() {
        let sdk = TestSdk::snapshot("/sdk/ohos/12");
        let resolver = SdkToolResolver::new(&sdk);
        let path = resolver.cmake_tool().unwrap();
        assert_eq!(
            path.path,
            PathBuf::from("/sdk/ohos/12/native/build-tools/cmake/bin/cmake")
        );
        assert_eq!(
            resolver.native_ninja_tool().unwrap().path,
            PathBuf::from("/sdk/ohos/12/native/build-tools/cmake/bin/ninja")
        );
    }

    #[test]
    fn llvm_strip_under_native_llvm() {
        let sdk = TestSdk::snapshot("/sdk/ohos/12");
        let resolver = SdkToolResolver::new(&sdk);
        let path = resolver.llvm_strip().unwrap();
        assert!(path.path.to_str().unwrap().contains("native/llvm/bin"));
        assert!(path.path.to_str().unwrap().ends_with("llvm-strip"));
    }

    #[test]
    fn tool_kind_binary_names() {
        assert_eq!(SdkToolKind::Restool.binary_name(), "restool");
        assert_eq!(SdkToolKind::Cmake.binary_name(), "cmake");
        assert_eq!(SdkToolKind::LlvmStrip.binary_name(), "llvm-strip");
    }
}
