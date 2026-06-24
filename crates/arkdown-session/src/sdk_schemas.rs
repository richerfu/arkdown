//! SDK JSON schema path resolution.
//!
//! [`SdkSchemaResolver`] resolves paths to OHOS SDK JSON schema files from an
//! [`SdkSnapshot`]. Schema files are used to validate `module.json5`,
//! `app.json5`, form configs, router maps, etc. during the build.
//!
//! # Upstream correspondence
//!
//! | Rust method                                  | hvigor `DefaultSdk` getter                  |
//! |----------------------------------------------|---------------------------------------------|
//! | [`SdkSchemaResolver::app_schema`]            | `DefaultSdk.getAppSchema`                   |
//! | [`SdkSchemaResolver::module_schema`]         | `DefaultSdk.getModuleSchema`                |
//! | [`SdkSchemaResolver::page_schema`]           | `DefaultSdk.getPageSchema`                  |
//! | [`SdkSchemaResolver::form_schema`]           | `DefaultSdk.getFormSchema`                  |
//! | [`SdkSchemaResolver::router_map_schema`]     | `DefaultSdk.getRouterMapSchema`             |
//! | [`SdkSchemaResolver::share_config_schema`]   | `DefaultSdk.getShareConfigSchema`           |
//! | [`SdkSchemaResolver::startup_schema`]        | `DefaultSdk.getAppStartupSchema` /          |
//! |                                              | `getHarStartupSchema` / `getHspStartupSchema`|
//! | [`SdkSchemaResolver::syscap_schema`]         | `DefaultSdk.getSysCapSchema`                |
//! | [`SdkSchemaResolver::pac_schema`]            | `DefaultSdk.getPacSchema`                   |
//! | [`SdkSchemaResolver::shortcuts_schema`]      | `DefaultSdk.getShortcutsJsonSchema`         |
//! | [`SdkSchemaResolver::insight_intent_schema`] | `DefaultSdk.getInsightIntentSchema`         |

use std::path::{Path, PathBuf};

use crate::sdk::snapshot::SdkSnapshot;

// ---------------------------------------------------------------------------
// SdkSchemaKind
// ---------------------------------------------------------------------------

/// Enumeration of all OHOS SDK JSON schema files.
///
/// Used as a typed key for schema path lookups and error messages.
///
/// Upstream: schema file names in `DefaultSdk` in `src/sdk/default-sdk.js`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SdkSchemaKind {
    /// `app.json5` schema — validates `AppScope/app.json5`.
    App,
    /// `module.json5` schema — validates `src/main/module.json5`.
    Module,
    /// Page JSON schema — validates page route config files.
    Page,
    /// Form JSON schema — validates form widget config files.
    Form,
    /// Router map JSON schema — validates `routerMap` config files.
    RouterMap,
    /// Share config JSON schema — validates HSP share config files.
    ShareConfig,
    /// App startup JSON schema — validates `appStartup.json` for entry modules.
    AppStartup,
    /// HAR startup JSON schema — validates `appStartup.json` for HAR modules.
    HarStartup,
    /// HSP startup JSON schema — validates `appStartup.json` for HSP modules.
    HspStartup,
    /// System capability JSON schema — validates `syscap.json`.
    Syscap,
    /// PAC (Proxy Auto-Config) JSON schema.
    Pac,
    /// Shortcuts JSON schema — validates `shortcuts.json`.
    Shortcuts,
    /// Insight intent JSON schema — validates `insightIntents.json`.
    InsightIntent,
}

impl SdkSchemaKind {
    /// Return the canonical schema file name.
    pub fn file_name(self) -> &'static str {
        match self {
            Self::App => "app.json",
            Self::Module => "module.json",
            Self::Page => "pages.json",
            Self::Form => "form_config.json",
            Self::RouterMap => "router_map.json",
            Self::ShareConfig => "share_config.json",
            Self::AppStartup => "app_startup.json",
            Self::HarStartup => "har_startup.json",
            Self::HspStartup => "hsp_startup.json",
            Self::Syscap => "syscap.json",
            Self::Pac => "pac.json",
            Self::Shortcuts => "shortcuts.json",
            Self::InsightIntent => "insight_intents.json",
        }
    }
}

// ---------------------------------------------------------------------------
// SdkSchemaPath
// ---------------------------------------------------------------------------

/// A resolved path to a single SDK JSON schema file.
///
/// Wraps a [`PathBuf`] and records which schema kind it points to.
///
/// Upstream: return value of `DefaultSdk.getAppSchema()` etc.
#[derive(Debug, Clone)]
pub struct SdkSchemaPath {
    /// The resolved absolute path to the schema file.
    pub path: PathBuf,
    /// Which schema this path refers to.
    pub kind: SdkSchemaKind,
}

impl SdkSchemaPath {
    /// Construct a new schema path.
    pub fn new(path: PathBuf, kind: SdkSchemaKind) -> Self {
        Self { path, kind }
    }

    /// Return a reference to the inner path.
    pub fn as_path(&self) -> &Path {
        &self.path
    }
}

// ---------------------------------------------------------------------------
// SdkSchemaResolver
// ---------------------------------------------------------------------------

/// Resolves OHOS SDK JSON schema paths from an [`SdkSnapshot`].
///
/// All methods mirror the `DefaultSdk` getter family in
/// `src/sdk/default-sdk.js`.
///
/// Schemas are located under `<sdk_root>/toolchains/lib/checkConfig/`.
#[derive(Debug, Clone)]
pub struct SdkSchemaResolver {
    sdk_root: PathBuf,
}

impl SdkSchemaResolver {
    /// Create a resolver from an [`SdkSnapshot`].
    pub fn new(sdk: &SdkSnapshot) -> Self {
        Self {
            sdk_root: sdk.sdk_root.clone(),
        }
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Resolve a schema path under `<sdk_root>/toolchains/lib/checkConfig/<name>`.
    fn schema_at(&self, name: &str, kind: SdkSchemaKind) -> Option<SdkSchemaPath> {
        let path = self
            .sdk_root
            .join("toolchains")
            .join("lib")
            .join("checkConfig")
            .join(name);
        Some(SdkSchemaPath::new(path, kind))
    }

    /// Return a schema path by kind.
    pub fn schema_path(&self, kind: SdkSchemaKind) -> Option<SdkSchemaPath> {
        match kind {
            SdkSchemaKind::App => self.app_schema(),
            SdkSchemaKind::Module => self.module_schema(),
            SdkSchemaKind::Page => self.page_schema(),
            SdkSchemaKind::Form => self.form_schema(),
            SdkSchemaKind::RouterMap => self.router_map_schema(),
            SdkSchemaKind::ShareConfig => self.share_config_schema(),
            SdkSchemaKind::AppStartup => self.app_startup_schema(),
            SdkSchemaKind::HarStartup => self.har_startup_schema(),
            SdkSchemaKind::HspStartup => self.hsp_startup_schema(),
            SdkSchemaKind::Syscap => self.syscap_schema(),
            SdkSchemaKind::Pac => self.pac_schema(),
            SdkSchemaKind::Shortcuts => self.shortcuts_schema(),
            SdkSchemaKind::InsightIntent => self.insight_intent_schema(),
        }
    }

    // -----------------------------------------------------------------------
    // Public API — mirrors DefaultSdk getters
    // -----------------------------------------------------------------------

    /// Path to the `app.json5` JSON schema.
    ///
    /// Upstream: `DefaultSdk.getAppSchema`.
    pub fn app_schema(&self) -> Option<SdkSchemaPath> {
        self.schema_at(SdkSchemaKind::App.file_name(), SdkSchemaKind::App)
    }

    /// Path to the `module.json5` JSON schema.
    ///
    /// Upstream: `DefaultSdk.getModuleSchema`.
    pub fn module_schema(&self) -> Option<SdkSchemaPath> {
        self.schema_at(SdkSchemaKind::Module.file_name(), SdkSchemaKind::Module)
    }

    /// Path to the page route config JSON schema.
    ///
    /// Upstream: `DefaultSdk.getPageSchema`.
    pub fn page_schema(&self) -> Option<SdkSchemaPath> {
        self.schema_at(SdkSchemaKind::Page.file_name(), SdkSchemaKind::Page)
    }

    /// Path to the form widget config JSON schema.
    ///
    /// Upstream: `DefaultSdk.getFormSchema`.
    pub fn form_schema(&self) -> Option<SdkSchemaPath> {
        self.schema_at(SdkSchemaKind::Form.file_name(), SdkSchemaKind::Form)
    }

    /// Path to the router map JSON schema.
    ///
    /// Upstream: `DefaultSdk.getRouterMapSchema`.
    pub fn router_map_schema(&self) -> Option<SdkSchemaPath> {
        self.schema_at(
            SdkSchemaKind::RouterMap.file_name(),
            SdkSchemaKind::RouterMap,
        )
    }

    /// Path to the HSP share config JSON schema.
    ///
    /// Upstream: `DefaultSdk.getShareConfigSchema`.
    pub fn share_config_schema(&self) -> Option<SdkSchemaPath> {
        self.schema_at(
            SdkSchemaKind::ShareConfig.file_name(),
            SdkSchemaKind::ShareConfig,
        )
    }

    /// Path to the app-module startup JSON schema.
    ///
    /// Upstream: `DefaultSdk.getAppStartupSchema`.
    pub fn app_startup_schema(&self) -> Option<SdkSchemaPath> {
        self.schema_at(
            SdkSchemaKind::AppStartup.file_name(),
            SdkSchemaKind::AppStartup,
        )
    }

    /// Path to the HAR-module startup JSON schema.
    ///
    /// Upstream: `DefaultSdk.getHarStartupSchema`.
    pub fn har_startup_schema(&self) -> Option<SdkSchemaPath> {
        self.schema_at(
            SdkSchemaKind::HarStartup.file_name(),
            SdkSchemaKind::HarStartup,
        )
    }

    /// Path to the HSP-module startup JSON schema.
    ///
    /// Upstream: `DefaultSdk.getHspStartupSchema`.
    pub fn hsp_startup_schema(&self) -> Option<SdkSchemaPath> {
        self.schema_at(
            SdkSchemaKind::HspStartup.file_name(),
            SdkSchemaKind::HspStartup,
        )
    }

    /// Convenience: return the startup schema for the given module kind string.
    ///
    /// Dispatches to `app_startup_schema`, `har_startup_schema`, or
    /// `hsp_startup_schema` based on `module_type`.
    pub fn startup_schema(&self, module_type: &str) -> Option<SdkSchemaPath> {
        match module_type {
            "har" => self.har_startup_schema(),
            "shared" => self.hsp_startup_schema(),
            _ => self.app_startup_schema(),
        }
    }

    /// Path to the system capability JSON schema.
    ///
    /// Upstream: `DefaultSdk.getSysCapSchema`.
    pub fn syscap_schema(&self) -> Option<SdkSchemaPath> {
        self.schema_at(SdkSchemaKind::Syscap.file_name(), SdkSchemaKind::Syscap)
    }

    /// Path to the PAC (Proxy Auto-Config) JSON schema.
    ///
    /// Upstream: `DefaultSdk.getPacSchema`.
    pub fn pac_schema(&self) -> Option<SdkSchemaPath> {
        self.schema_at(SdkSchemaKind::Pac.file_name(), SdkSchemaKind::Pac)
    }

    /// Path to the shortcuts JSON schema.
    ///
    /// Upstream: `DefaultSdk.getShortcutsJsonSchema`.
    pub fn shortcuts_schema(&self) -> Option<SdkSchemaPath> {
        self.schema_at(
            SdkSchemaKind::Shortcuts.file_name(),
            SdkSchemaKind::Shortcuts,
        )
    }

    /// Path to the insight intent JSON schema.
    ///
    /// Upstream: `DefaultSdk.getInsightIntentSchema`.
    pub fn insight_intent_schema(&self) -> Option<SdkSchemaPath> {
        self.schema_at(
            SdkSchemaKind::InsightIntent.file_name(),
            SdkSchemaKind::InsightIntent,
        )
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
        let resolver = SdkSchemaResolver::new(&sdk);
        assert_eq!(
            resolver.app_schema().unwrap().path,
            PathBuf::from("/sdk/ohos/12/toolchains/lib/checkConfig/app.json")
        );
    }

    #[test]
    fn app_schema_under_check_config() {
        let sdk = TestSdk::snapshot("/sdk/ohos/12");
        let resolver = SdkSchemaResolver::new(&sdk);
        let schema = resolver.app_schema().unwrap();
        assert!(schema.path.to_str().unwrap().contains("checkConfig"));
        assert_eq!(schema.kind, SdkSchemaKind::App);
    }

    #[test]
    fn module_schema_kind() {
        let sdk = TestSdk::snapshot("/sdk/ohos/12");
        let resolver = SdkSchemaResolver::new(&sdk);
        let schema = resolver.module_schema().unwrap();
        assert_eq!(schema.kind, SdkSchemaKind::Module);
    }

    #[test]
    fn startup_schema_dispatches_by_module_type() {
        let sdk = TestSdk::snapshot("/sdk/ohos/12");
        let resolver = SdkSchemaResolver::new(&sdk);
        let har = resolver.startup_schema("har").unwrap();
        let hsp = resolver.startup_schema("shared").unwrap();
        let entry = resolver.startup_schema("entry").unwrap();
        assert_eq!(har.kind, SdkSchemaKind::HarStartup);
        assert_eq!(hsp.kind, SdkSchemaKind::HspStartup);
        assert_eq!(entry.kind, SdkSchemaKind::AppStartup);
    }

    #[test]
    fn all_schemas_under_same_root() {
        let sdk = TestSdk::snapshot("/sdk/ohos/12");
        let resolver = SdkSchemaResolver::new(&sdk);
        let expected_prefix = PathBuf::from("/sdk/ohos/12/toolchains/lib/checkConfig");
        let schemas = [
            resolver.app_schema(),
            resolver.module_schema(),
            resolver.page_schema(),
            resolver.form_schema(),
            resolver.router_map_schema(),
            resolver.share_config_schema(),
            resolver.syscap_schema(),
            resolver.pac_schema(),
            resolver.shortcuts_schema(),
            resolver.insight_intent_schema(),
        ];
        for schema in schemas.into_iter().flatten() {
            assert!(
                schema.path.starts_with(&expected_prefix),
                "schema {:?} not under expected prefix",
                schema.path
            );
        }
    }

    #[test]
    fn schema_kind_file_names() {
        assert_eq!(SdkSchemaKind::App.file_name(), "app.json");
        assert_eq!(SdkSchemaKind::RouterMap.file_name(), "router_map.json");
        assert_eq!(
            SdkSchemaKind::InsightIntent.file_name(),
            "insight_intents.json"
        );
    }
}
