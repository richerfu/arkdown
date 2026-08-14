//! SDK compile plugin snapshot.
//!
//! [`SdkCompilePluginSnapshot`] captures the ETS/JS loader paths, ArkTS
//! feature gates, and supported file extension sets from the OHOS SDK. It is
//! the Rust equivalent of the `DefaultSdk` compile-plugin getter family in
//! `src/sdk/default-sdk.js`.
//!
//! # Upstream correspondence
//!
//! | Rust type / method                              | hvigor `DefaultSdk` getter                    |
//! |-------------------------------------------------|-----------------------------------------------|
//! | [`SdkCompilePluginSnapshot::ets_loader`]        | `DefaultSdk.getEtsLoader`                     |
//! | [`SdkCompilePluginSnapshot::js_loader`]         | `DefaultSdk.getJsLoader`                      |
//! | [`SdkCompilePluginSnapshot::ark_version`]       | `DefaultSdk.getArkVersion`                    |
//! | [`SdkCompilePluginSnapshot::arkui_x_dir`]       | `DefaultSdk.getArkUIXSdkDir`                  |
//! | [`SdkCompilePluginSnapshot::sdk_ets_dir`]       | `DefaultSdk.getSdkEtsDir`                     |
//! | [`SdkCompilePluginSnapshot::sdk_js_dir`]        | `DefaultSdk.getSdkJsDir`                      |
//! | [`SdkCompilePluginSnapshot::component_metadata`]| `DefaultSdk.getEtsComponentVersion` /         |
//! |                                                 | `getEtsComponentReleaseType`                  |
//! | [`SdkCompilePluginSnapshot::support_extensions`]| `DefaultSdk.getSupportedExtensions`           |
//! | [`SdkArkTsFeatureGate`]                         | `ets-loader` feature flags from SDK metadata  |

use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// EtsLoaderPath
// ---------------------------------------------------------------------------

/// Resolved path to the ETS loader plugin directory.
///
/// The ETS loader transforms ArkTS/ETS source files before bundling. In
/// arkdown, this is replaced by `arkdown-plugin-ets-transform` (SPEC-004a),
/// but the SDK path is still needed for compatibility checks.
///
/// Upstream: `DefaultSdk.getEtsLoader`.
#[derive(Debug, Clone)]
pub struct EtsLoaderPath {
    /// Absolute path to the `ets-loader` directory in the SDK.
    pub path: PathBuf,
    /// File extensions this loader handles (e.g., `[".ets", ".ts"]`).
    pub extensions: Vec<String>,
}

impl EtsLoaderPath {
    /// Construct a new ETS loader path.
    pub fn new(path: PathBuf, extensions: Vec<String>) -> Self {
        Self { path, extensions }
    }

    /// Return a reference to the inner path.
    pub fn as_path(&self) -> &Path {
        &self.path
    }
}

// ---------------------------------------------------------------------------
// JsLoaderPath
// ---------------------------------------------------------------------------

/// Resolved path to the JS loader plugin directory.
///
/// The JS loader handles plain `.js` source files. In arkdown, this is
/// replaced by the rolldown pipeline.
///
/// Upstream: `DefaultSdk.getJsLoader`.
#[derive(Debug, Clone)]
pub struct JsLoaderPath {
    /// Absolute path to the `js-loader` directory in the SDK.
    pub path: PathBuf,
}

impl JsLoaderPath {
    /// Construct a new JS loader path.
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Return a reference to the inner path.
    pub fn as_path(&self) -> &Path {
        &self.path
    }
}

// ---------------------------------------------------------------------------
// SdkArkTsFeatureGate
// ---------------------------------------------------------------------------

/// ArkTS compiler feature gates read from the SDK.
///
/// These flags control which ArkTS optimizations and experimental features
/// are enabled for the current SDK version.
///
/// Upstream: `ets-loader` feature flags from SDK metadata / `build-option`
/// defaults in `ets-loader/lib/fast_build/ark_compiler/common/ob_config_resolver.js`.
#[derive(Debug, Clone, Default)]
pub struct SdkArkTsFeatureGate {
    /// Whether partial update (incremental UI recomposition) is enabled.
    ///
    /// Upstream: `partialUpdate` flag in `ets-loader` build options.
    pub partial_update_enabled: bool,
    /// Whether component-level optimization is enabled.
    ///
    /// Upstream: `optimizeComponent` flag in `ets-loader` build options.
    pub optimize_component_enabled: bool,
    /// Whether TypeScript HAR (`.d.ts`-only) output is enabled.
    ///
    /// Upstream: `useTsHar` flag in `ets-loader`.
    pub use_ts_har: bool,
}

impl SdkArkTsFeatureGate {
    /// Build a default feature gate for the given API version.
    ///
    /// `partial_update` was enabled starting from API 9.
    /// `optimize_component` was enabled starting from API 11.
    pub fn for_api_version(api_version: u32) -> Self {
        Self {
            partial_update_enabled: api_version >= 9,
            optimize_component_enabled: api_version >= 11,
            use_ts_har: false,
        }
    }
}

// ---------------------------------------------------------------------------
// SdkSupportExtensionSet
// ---------------------------------------------------------------------------

/// Set of file extensions supported by the SDK's ETS/JS compile pipeline.
///
/// Upstream: `DefaultSdk.getSupportedExtensions` (or inferred from loader
/// `options.extensions` in `ets-loader`).
#[derive(Debug, Clone)]
pub struct SdkSupportExtensionSet {
    /// Ordered list of supported extensions (e.g., `[".ets", ".ts", ".js"]`).
    pub extensions: Vec<String>,
}

impl SdkSupportExtensionSet {
    /// Default extension set for OHOS ArkTS projects.
    pub fn default_arkts() -> Self {
        Self {
            extensions: vec![
                ".ets".to_string(),
                ".ts".to_string(),
                ".js".to_string(),
                ".json".to_string(),
                ".json5".to_string(),
            ],
        }
    }

    /// Return `true` if the given extension is in the support set.
    pub fn supports(&self, ext: &str) -> bool {
        self.extensions.iter().any(|e| e == ext)
    }
}

// ---------------------------------------------------------------------------
// SdkCompilePluginSnapshot
// ---------------------------------------------------------------------------

/// Snapshot of the SDK compile plugin configuration.
///
/// Captures the ETS loader path, JS loader path, ArkTS compiler version,
/// feature gates, and supported extensions — all read once at session startup.
///
/// Upstream: `DefaultSdk.getEtsLoader`, `getJsLoader`, `getArkVersion`,
/// `getArkUIXSdkDir`, `getSdkEtsDir`, `getSdkJsDir`,
/// `getEtsComponentVersion`, `getEtsComponentReleaseType`.
#[derive(Debug, Clone)]
pub struct SdkCompilePluginSnapshot {
    /// ETS loader path and extension list.
    pub ets_loader: Option<EtsLoaderPath>,
    /// JS loader path.
    pub js_loader: Option<JsLoaderPath>,
    /// ArkTS compiler version string (e.g., `"5.0.0.47"`).
    pub ark_version: String,
    /// ArkUI-X SDK directory (cross-platform UI).
    pub arkui_x_dir: Option<PathBuf>,
    /// SDK ETS built-in declarations directory.
    pub sdk_ets_dir: Option<PathBuf>,
    /// SDK JS built-in declarations directory.
    pub sdk_js_dir: Option<PathBuf>,
    /// ETS component metadata: version and release type.
    pub component_metadata: Option<ComponentMetadata>,
    /// Supported file extension set.
    pub support_extensions: SdkSupportExtensionSet,
    /// ArkTS feature gates.
    pub feature_gate: SdkArkTsFeatureGate,
}

/// ETS component version and release type metadata.
///
/// Upstream: `DefaultSdk.getEtsComponentVersion` /
/// `DefaultSdk.getEtsComponentReleaseType`.
#[derive(Debug, Clone)]
pub struct ComponentMetadata {
    /// Component library version string.
    pub version: String,
    /// Release type (`"Release"`, `"Beta"`, etc.).
    pub release_type: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ComponentVersion([u64; 4]);

impl ComponentVersion {
    fn parse(version: &str) -> Option<Self> {
        let mut parts = [0_u64; 4];
        let mut count = 0;
        for (index, part) in version.split('.').enumerate() {
            if index >= parts.len() {
                return None;
            }
            parts[index] = part.parse().ok()?;
            count += 1;
        }
        (count > 0).then_some(Self(parts))
    }
}

impl ComponentMetadata {
    /// Read ETS component metadata from `<sdk>/ets/oh-uni-package.json`.
    ///
    /// Upstream: `SdkComponent` metadata populated by sdkmanager from
    /// `oh-uni-package.json`.
    pub fn from_sdk_root(sdk_root: &Path) -> Option<Self> {
        let value = Self::read_package_value(&sdk_root.join("ets").join("oh-uni-package.json"))?;
        let version = Self::json_string_field(&value, "version")?;
        let release_type = Self::json_string_field(&value, "releaseType")?;
        Some(Self {
            version,
            release_type,
        })
    }

    fn read_package_value(path: &Path) -> Option<serde_json::Value> {
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    fn json_field<'a>(value: &'a serde_json::Value, field: &str) -> Option<&'a serde_json::Value> {
        value
            .get(field)
            .or_else(|| value.get("data").and_then(|data| data.get(field)))
    }

    fn json_string_field(value: &serde_json::Value, field: &str) -> Option<String> {
        Self::json_field(value, field)?
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    }

    /// `SdkComponent.requireUISyntax()` is true for component versions below
    /// 3.2.2.5, except the one 3.2.1.4 compatibility release. The current ETS
    /// and JS components share this behavior; using the ETS metadata here is
    /// sufficient because both are selected from the same SDK API component.
    pub fn requires_ui_syntax(&self) -> bool {
        let Some(version) = ComponentVersion::parse(&self.version) else {
            return false;
        };
        version < ComponentVersion([3, 2, 2, 5]) && version != ComponentVersion([3, 2, 1, 4])
    }
}

impl SdkCompilePluginSnapshot {
    /// Build a compile plugin snapshot from resolved SDK parts.
    ///
    /// Upstream: `DefaultSdk` getter family.
    pub fn from_sdk_parts(sdk_root: &Path, api_version: u32, compile_sdk_version: &str) -> Self {
        let feature_gate = SdkArkTsFeatureGate::for_api_version(api_version);
        let extensions = SdkSupportExtensionSet::default_arkts();

        let ets_loader_path = sdk_root.join("ets").join("build-tools").join("ets-loader");
        let ets_loader = Some(EtsLoaderPath::new(
            ets_loader_path,
            vec![".ets".to_string(), ".ts".to_string(), ".js".to_string()],
        ));

        let js_loader_path = sdk_root.join("js").join("build-tools").join("js-loader");
        let js_loader = Some(JsLoaderPath::new(js_loader_path));

        let sdk_ets_dir = Some(sdk_root.join("ets").join("api"));
        let sdk_js_dir = Some(sdk_root.join("js").join("api"));
        let arkui_x_dir = Some(sdk_root.join("arkui-x"));

        let component_metadata = ComponentMetadata::from_sdk_root(sdk_root);

        Self {
            ets_loader,
            js_loader,
            ark_version: compile_sdk_version.to_string(),
            arkui_x_dir,
            sdk_ets_dir,
            sdk_js_dir,
            component_metadata,
            support_extensions: extensions,
            feature_gate,
        }
    }

    /// Return the ETS loader path, if present.
    ///
    /// Upstream: `DefaultSdk.getEtsLoader`.
    pub fn ets_loader(&self) -> Option<&EtsLoaderPath> {
        self.ets_loader.as_ref()
    }

    /// Return the JS loader path, if present.
    ///
    /// Upstream: `DefaultSdk.getJsLoader`.
    pub fn js_loader(&self) -> Option<&JsLoaderPath> {
        self.js_loader.as_ref()
    }

    /// Return the ArkTS compiler version string.
    ///
    /// Upstream: `DefaultSdk.getArkVersion`.
    pub fn ark_version(&self) -> &str {
        &self.ark_version
    }

    /// Return the ArkUI-X SDK directory.
    ///
    /// Upstream: `DefaultSdk.getArkUIXSdkDir`.
    pub fn arkui_x_dir(&self) -> Option<&PathBuf> {
        self.arkui_x_dir.as_ref()
    }

    /// Return the SDK ETS built-in declarations directory.
    ///
    /// Upstream: `DefaultSdk.getSdkEtsDir`.
    pub fn sdk_ets_dir(&self) -> Option<&PathBuf> {
        self.sdk_ets_dir.as_ref()
    }

    /// Return the SDK JS built-in declarations directory.
    ///
    /// Upstream: `DefaultSdk.getSdkJsDir`.
    pub fn sdk_js_dir(&self) -> Option<&PathBuf> {
        self.sdk_js_dir.as_ref()
    }

    /// Return the ETS component metadata.
    ///
    /// Upstream: `DefaultSdk.getEtsComponentVersion` /
    /// `DefaultSdk.getEtsComponentReleaseType`.
    pub fn component_metadata(&self) -> Option<&ComponentMetadata> {
        self.component_metadata.as_ref()
    }

    /// Return the supported file extension set.
    ///
    /// Upstream: `DefaultSdk.getSupportedExtensions`.
    pub fn support_extensions(&self) -> &SdkSupportExtensionSet {
        &self.support_extensions
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
        fn snapshot(root: &str) -> SdkCompilePluginSnapshot {
            SdkCompilePluginSnapshot::from_sdk_parts(Path::new(root), 12, "5.0.0.47")
        }

        fn temp_root(name: &str) -> PathBuf {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time before unix epoch")
                .as_nanos();
            std::env::temp_dir().join(format!("arkdown-sdk-{name}-{}-{nanos}", std::process::id()))
        }

        fn snapshot_with_ets_metadata(metadata: &str) -> (SdkCompilePluginSnapshot, PathBuf) {
            let root = Self::temp_root("compile-plugin");
            let ets_root = root.join("ets");
            std::fs::create_dir_all(&ets_root).expect("create ets root");
            std::fs::write(ets_root.join("oh-uni-package.json"), metadata)
                .expect("write ets metadata");
            let snapshot = SdkCompilePluginSnapshot::from_sdk_parts(&root, 12, "5.0.0.47");
            (snapshot, root)
        }
    }

    #[test]
    fn snapshot_populates_loaders() {
        let snap = TestSdk::snapshot("/sdk/ohos/12");
        assert!(snap.ets_loader().is_some());
        assert!(snap.js_loader().is_some());
        assert!(snap.arkui_x_dir().is_some());
    }

    #[test]
    fn snapshot_has_extensions() {
        let snap = TestSdk::snapshot("/sdk/ohos/12");
        assert!(snap.support_extensions().supports(".ets"));
        assert!(snap.support_extensions().supports(".ts"));
        assert!(!snap.support_extensions().supports(".cpp"));
    }

    #[test]
    fn full_sdk_populates_loaders() {
        let snap = TestSdk::snapshot("/sdk/ohos/12");
        let ets = snap.ets_loader().unwrap();
        assert!(ets.path.to_str().unwrap().contains("ets-loader"));
        assert!(ets.extensions.contains(&".ets".to_string()));
        let js = snap.js_loader().unwrap();
        assert!(js.path.to_str().unwrap().contains("js-loader"));
    }

    #[test]
    fn ark_version_matches_sdk() {
        let snap = TestSdk::snapshot("/sdk/ohos/12");
        assert_eq!(snap.ark_version(), "5.0.0.47");
    }

    #[test]
    fn feature_gate_api9_enables_partial_update() {
        let gate = SdkArkTsFeatureGate::for_api_version(9);
        assert!(gate.partial_update_enabled);
        assert!(!gate.optimize_component_enabled);
    }

    #[test]
    fn feature_gate_api11_enables_optimize_component() {
        let gate = SdkArkTsFeatureGate::for_api_version(11);
        assert!(gate.partial_update_enabled);
        assert!(gate.optimize_component_enabled);
    }

    #[test]
    fn component_metadata_present_with_sdk() {
        let (snap, root) = TestSdk::snapshot_with_ets_metadata(
            r#"{"version":"6.0.1.112","releaseType":"Canary"}"#,
        );
        let meta = snap.component_metadata().unwrap();
        assert_eq!(meta.version, "6.0.1.112");
        assert_eq!(meta.release_type, "Canary");
        std::fs::remove_dir_all(root).expect("remove temp sdk root");
    }

    #[test]
    fn ui_syntax_requirement_matches_sdk_component_thresholds() {
        let metadata = |version: &str| ComponentMetadata {
            version: version.to_string(),
            release_type: "Release".to_string(),
        };

        assert!(metadata("3.2.1.3").requires_ui_syntax());
        assert!(!metadata("3.2.1.4").requires_ui_syntax());
        assert!(metadata("3.2.2.4").requires_ui_syntax());
        assert!(!metadata("3.2.2.5").requires_ui_syntax());
        assert!(!metadata("6.1.1.125").requires_ui_syntax());
        assert!(!metadata("invalid").requires_ui_syntax());
    }
}
