use std::path::PathBuf;

use crate::module::ModuleType;

/// The complete resolution result for an OHOS import specifier.
#[derive(Debug, Clone)]
pub enum OhosResolvedModule {
    /// A local source file (relative import or local HAR source).
    LocalSource {
        path: PathBuf,
        module_type: ModuleType,
        pkg_context: Option<PkgContext>,
    },
    /// A remote ohpm package entry file.
    RemotePackage {
        path: PathBuf,
        module_type: ModuleType,
        pkg_name: String,
        pkg_version: String,
        ohm_url: Option<String>,
    },
    /// An SDK API declaration (`@ohos/*`, `@kit.*`).
    SdkDeclaration {
        path: PathBuf,
        /// Original specifier, e.g. `"@ohos/hilog"` or `"@kit.ArkUI"`.
        specifier: String,
    },
    /// A ByteCode HAR — no source, reference `.abc` directly.
    BytecodeHar {
        abc_path: PathBuf,
        declaration_path: PathBuf,
        pkg_name: String,
        ohm_url: String,
    },
    /// An external cross-bundle reference (`@bundle:`, `@shared:`).
    /// Not bundled; resolved at runtime by ArkRuntime.
    ExternalBundle {
        /// e.g. `"@bundle:com.example/module/path"`
        ohm_url: String,
    },
    /// An HSP reference — runtime-resolved.
    ExternalHsp {
        bundle_name: String,
        module_name: String,
        entry_path: String,
    },
}

/// Package context information used for OHM URL generation.
#[derive(Debug, Clone)]
pub struct PkgContext {
    pub package_name: String,
    pub bundle_name: String,
    pub module_name: String,
    pub version: String,
    pub pkg_path: PathBuf,
    pub entry_path: Option<PathBuf>,
}
