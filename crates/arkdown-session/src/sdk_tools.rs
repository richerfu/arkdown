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
//! | [`SdkToolResolver::binxo`]             | `HmsCoreNativeComponent.getBinxoTool`    |

use std::path::{Path, PathBuf};

use arkdown_path::HostPlatform;

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
    /// `app_packing_tool.jar` (or legacy `hmos_app_packing_tool.jar`) packager.
    PackageTool,
    /// `hapsigntoolv2.jar` — HAP signing. Replaced by `arkdown-sign` (SPEC-021).
    HapSignToolV2,
    /// `haptobin_tool.jar` — HAP-to-binary converter.
    HapToBin,
    /// `syscap-tool` — system capability validator.
    SyscapTool,
    /// `hap-sign-tool.jar` — signing config verifier.
    VerifySignConfigTool,
    /// `cmake` — CMake build system (native builds).
    Cmake,
    /// `ninja` — Ninja build system (native builds).
    NativeNinja,
    /// `llvm-strip` — LLVM strip (native debug symbol removal).
    LlvmStrip,
    /// `llvm-readelf` — LLVM readelf (native ELF inspection).
    LlvmReadElf,
    /// `binxo` — HMS native BinXO post-processor.
    Binxo,
}

impl SdkToolKind {
    /// Return the canonical tool binary name.
    pub fn binary_name(self) -> &'static str {
        match self {
            Self::Restool => "restool",
            Self::PackageTool => "app_packing_tool.jar",
            Self::HapSignToolV2 => "hapsigntoolv2.jar",
            Self::HapToBin => "haptobin_tool.jar",
            Self::SyscapTool => "syscap_tool",
            Self::VerifySignConfigTool => "hap-sign-tool.jar",
            Self::Cmake => "cmake",
            Self::NativeNinja => "ninja",
            Self::LlvmStrip => "llvm-strip",
            Self::LlvmReadElf => "llvm-readelf",
            Self::Binxo => "binxo",
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
    compile_sdk_version: String,
    host: HostPlatform,
}

impl SdkToolResolver {
    /// Create a resolver from an [`SdkSnapshot`].
    pub fn new(sdk: &SdkSnapshot) -> Self {
        Self::for_host(sdk, HostPlatform::CURRENT)
    }

    /// Create a resolver for an explicit host layout.
    ///
    /// This is primarily useful for validating SDK snapshots before they are
    /// transferred to another build host.
    pub fn for_host(sdk: &SdkSnapshot, host: HostPlatform) -> Self {
        Self {
            sdk_root: sdk.sdk_root.clone(),
            compile_sdk_version: sdk.compile_sdk_version.clone(),
            host,
        }
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn toolchains_lib_file(&self, file_name: &str, kind: SdkToolKind) -> SdkToolPath {
        SdkToolPath::new(
            self.sdk_root.join("toolchains").join("lib").join(file_name),
            kind,
        )
    }

    fn executable_at(&self, parent: PathBuf, stem: &str, kind: SdkToolKind) -> SdkToolPath {
        SdkToolPath::new(parent.join(self.host.executable_name(stem)), kind)
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
            SdkToolKind::Binxo => self.binxo(),
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
        Some(self.executable_at(
            self.sdk_root.join("toolchains"),
            "restool",
            SdkToolKind::Restool,
        ))
    }

    /// Path to the app packing tool JAR.
    ///
    /// Upstream: `DefaultSdk.getPackageTool`.
    pub fn package_tool(&self) -> Option<SdkToolPath> {
        let file_name =
            if SdkVersionText::new(&self.compile_sdk_version).is_higher_than([3, 1, 5, 4]) {
                "app_packing_tool.jar"
            } else {
                "hmos_app_packing_tool.jar"
            };
        Some(self.toolchains_lib_file(file_name, SdkToolKind::PackageTool))
    }

    /// Path to `hapsigntoolv2.jar`.
    ///
    /// Upstream: `DefaultSdk.getHapSignToolV2`.
    /// Note: replaced by `arkdown-sign` (SPEC-021) at compile time.
    pub fn hap_sign_tool_v2(&self) -> Option<SdkToolPath> {
        Some(self.toolchains_lib_file("hapsigntoolv2.jar", SdkToolKind::HapSignToolV2))
    }

    /// Path to `haptobin_tool.jar`.
    ///
    /// Upstream: `DefaultSdk.getHapTobin`.
    pub fn hap_to_bin(&self) -> Option<SdkToolPath> {
        let path = self
            .sdk_root
            .join("js")
            .join("build-tools")
            .join("binary-tools")
            .join("haptobin_tool.jar");
        Some(SdkToolPath::new(path, SdkToolKind::HapToBin))
    }

    /// Path to `syscap_tool`.
    ///
    /// Upstream: `DefaultSdk.getSysCapTool`.
    pub fn syscap_tool(&self) -> Option<SdkToolPath> {
        Some(self.executable_at(
            self.sdk_root.join("toolchains"),
            "syscap_tool",
            SdkToolKind::SyscapTool,
        ))
    }

    /// Path to `hap-sign-tool.jar`, used by verify-sign-config tasks.
    ///
    /// Upstream: `DefaultSdk.getVerifySignConfigTool`.
    pub fn verify_sign_config_tool(&self) -> Option<SdkToolPath> {
        Some(self.toolchains_lib_file("hap-sign-tool.jar", SdkToolKind::VerifySignConfigTool))
    }

    /// Path to the CMake binary bundled with the OHOS native SDK.
    ///
    /// Upstream: `DefaultSdk.getCmakeTool`.
    pub fn cmake_tool(&self) -> Option<SdkToolPath> {
        Some(
            self.executable_at(
                self.sdk_root
                    .join("native")
                    .join("build-tools")
                    .join("cmake")
                    .join("bin"),
                "cmake",
                SdkToolKind::Cmake,
            ),
        )
    }

    /// Path to the Ninja binary bundled with the OHOS native SDK.
    ///
    /// Upstream: `DefaultSdk.getNativeNinjaTool`.
    pub fn native_ninja_tool(&self) -> Option<SdkToolPath> {
        Some(
            self.executable_at(
                self.sdk_root
                    .join("native")
                    .join("build-tools")
                    .join("cmake")
                    .join("bin"),
                "ninja",
                SdkToolKind::NativeNinja,
            ),
        )
    }

    /// Path to `llvm-strip` from the OHOS LLVM toolchain.
    ///
    /// Upstream: `DefaultSdk.getSdkLlvmStrip`.
    pub fn llvm_strip(&self) -> Option<SdkToolPath> {
        Some(self.executable_at(
            self.sdk_root.join("native").join("llvm").join("bin"),
            "llvm-strip",
            SdkToolKind::LlvmStrip,
        ))
    }

    /// Path to `llvm-readelf` from the OHOS LLVM toolchain.
    ///
    /// Upstream: `DefaultSdk.getSdkLlvmReadElf`.
    pub fn llvm_read_elf(&self) -> Option<SdkToolPath> {
        Some(self.executable_at(
            self.sdk_root.join("native").join("llvm").join("bin"),
            "llvm-readelf",
            SdkToolKind::LlvmReadElf,
        ))
    }

    /// Path to the BinXO executable from the sibling HMS native component.
    ///
    /// Upstream: `HmsCoreNativeComponent.getBinxoTool` in DevEco Studio
    /// 6.1.1.280 resolves `<sdk-parent>/hms/native/BinXO/binxo[.exe]` instead
    /// of looking below the selected OpenHarmony component.
    pub fn binxo(&self) -> Option<SdkToolPath> {
        let sdk_parent = self.sdk_root.parent()?;
        Some(self.executable_at(
            sdk_parent.join("hms").join("native").join("BinXO"),
            "binxo",
            SdkToolKind::Binxo,
        ))
    }
}

struct SdkVersionText<'a> {
    value: &'a str,
}

impl<'a> SdkVersionText<'a> {
    fn new(value: &'a str) -> Self {
        Self { value }
    }

    fn is_higher_than(&self, expected: [u32; 4]) -> bool {
        let mut actual = self
            .value
            .split('.')
            .map(|component| component.parse::<u32>().unwrap_or(0));
        for expected_component in expected {
            match actual.next().unwrap_or(0).cmp(&expected_component) {
                std::cmp::Ordering::Greater => return true,
                std::cmp::Ordering::Less => return false,
                std::cmp::Ordering::Equal => {}
            }
        }
        actual.any(|component| component != 0)
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
            SdkSnapshot::from_parts(PathBuf::from(root), 12, 12, "5.0.0.47".to_string())
        }
    }

    #[test]
    fn resolver_returns_paths_from_sdk_root() {
        let sdk = TestSdk::snapshot("/sdk/ohos/12");
        let resolver = SdkToolResolver::for_host(&sdk, HostPlatform::Linux);
        assert_eq!(
            resolver.package_tool().unwrap().path,
            PathBuf::from("/sdk/ohos/12/toolchains/lib/app_packing_tool.jar")
        );
    }

    #[test]
    fn restool_path_under_toolchains() {
        let sdk = TestSdk::snapshot("/sdk/ohos/12");
        let resolver = SdkToolResolver::for_host(&sdk, HostPlatform::Linux);
        let path = resolver.restool().unwrap();
        assert_eq!(path.path, PathBuf::from("/sdk/ohos/12/toolchains/restool"));
        assert_eq!(path.kind, SdkToolKind::Restool);
    }

    #[test]
    fn hap_sign_tool_v2_under_lib() {
        let sdk = TestSdk::snapshot("/sdk/ohos/12");
        let resolver = SdkToolResolver::for_host(&sdk, HostPlatform::Linux);
        let path = resolver.hap_sign_tool_v2().unwrap();
        assert_eq!(
            path.path,
            PathBuf::from("/sdk/ohos/12/toolchains/lib/hapsigntoolv2.jar")
        );
    }

    #[test]
    fn latest_sdk_jar_tools_match_hvigor_component_layout() {
        let sdk = TestSdk::snapshot("/sdk/ohos/12");
        let resolver = SdkToolResolver::for_host(&sdk, HostPlatform::Windows);

        assert_eq!(
            resolver.hap_to_bin().unwrap().path,
            PathBuf::from("/sdk/ohos/12/js/build-tools/binary-tools/haptobin_tool.jar")
        );
        assert_eq!(
            resolver.verify_sign_config_tool().unwrap().path,
            PathBuf::from("/sdk/ohos/12/toolchains/lib/hap-sign-tool.jar")
        );
        assert!(!resolver
            .package_tool()
            .unwrap()
            .path
            .to_string_lossy()
            .ends_with(".exe"));
    }

    #[test]
    fn legacy_sdk_uses_hmos_app_packing_tool() {
        let sdk =
            SdkSnapshot::from_parts(PathBuf::from("/sdk/ohos/8"), 8, 8, "3.1.5.4".to_string());
        let resolver = SdkToolResolver::for_host(&sdk, HostPlatform::Linux);
        assert_eq!(
            resolver.package_tool().unwrap().path,
            PathBuf::from("/sdk/ohos/8/toolchains/lib/hmos_app_packing_tool.jar")
        );
    }

    #[test]
    fn cmake_under_build_tools() {
        let sdk = TestSdk::snapshot("/sdk/ohos/12");
        let resolver = SdkToolResolver::for_host(&sdk, HostPlatform::Linux);
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
        let resolver = SdkToolResolver::for_host(&sdk, HostPlatform::Linux);
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

    #[test]
    fn windows_sdk_tools_use_explicit_exe_names() {
        let sdk = TestSdk::snapshot(r"C:\Huawei\Sdk\openharmony");
        let resolver = SdkToolResolver::for_host(&sdk, HostPlatform::Windows);

        for (kind, expected) in [
            (SdkToolKind::Restool, "restool.exe"),
            (SdkToolKind::SyscapTool, "syscap_tool.exe"),
            (SdkToolKind::Cmake, "cmake.exe"),
            (SdkToolKind::NativeNinja, "ninja.exe"),
            (SdkToolKind::LlvmStrip, "llvm-strip.exe"),
            (SdkToolKind::LlvmReadElf, "llvm-readelf.exe"),
            (SdkToolKind::Binxo, "binxo.exe"),
        ] {
            assert_eq!(
                resolver
                    .tool_path(kind)
                    .and_then(|tool| tool.path.file_name().map(|name| name.to_owned()))
                    .as_deref(),
                Some(std::ffi::OsStr::new(expected))
            );
        }
    }

    #[test]
    fn binxo_uses_sibling_hms_native_component() {
        let sdk = TestSdk::snapshot("/sdk/default/openharmony");

        assert_eq!(
            SdkToolResolver::for_host(&sdk, HostPlatform::Linux)
                .binxo()
                .unwrap()
                .path,
            PathBuf::from("/sdk/default/hms/native/BinXO/binxo")
        );
        assert_eq!(
            SdkToolResolver::for_host(&sdk, HostPlatform::Windows)
                .binxo()
                .unwrap()
                .path,
            PathBuf::from("/sdk/default/hms/native/BinXO/binxo.exe")
        );
    }
}
