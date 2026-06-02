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

impl OhosResolvedModule {
    /// Returns the external module ID string for variants that are resolved as
    /// external (bundle refs, HSP refs, SDK declarations, bytecode HARs).
    ///
    /// Returns `None` for local source files and remote packages that are
    /// bundled normally.
    pub fn external_id(&self) -> Option<std::borrow::Cow<'_, str>> {
        match self {
            OhosResolvedModule::ExternalBundle { ohm_url } => {
                Some(std::borrow::Cow::Borrowed(ohm_url.as_str()))
            }
            OhosResolvedModule::ExternalHsp {
                bundle_name,
                module_name,
                entry_path,
            } => Some(std::borrow::Cow::Owned(format!(
                "@shared:{bundle_name}/{module_name}/{entry_path}"
            ))),
            OhosResolvedModule::SdkDeclaration { path, .. } => {
                Some(std::borrow::Cow::Owned(path.to_string_lossy().into_owned()))
            }
            OhosResolvedModule::BytecodeHar { ohm_url, .. } => {
                Some(std::borrow::Cow::Borrowed(ohm_url.as_str()))
            }
            OhosResolvedModule::LocalSource { .. } | OhosResolvedModule::RemotePackage { .. } => {
                None
            }
        }
    }

    /// Returns the resolved file path for variants that resolve to a local file
    /// (local source and remote packages).
    pub fn local_path(&self) -> Option<&std::path::PathBuf> {
        match self {
            OhosResolvedModule::LocalSource { path, .. }
            | OhosResolvedModule::RemotePackage { path, .. } => Some(path),
            _ => None,
        }
    }
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

impl PkgContext {
    /// Build the OHM URL for a source file within this package.
    ///
    /// Strips `pkg_path` from `path`, removes the source extension, and
    /// formats as `@bundle:<bundle_name>/<module_name>/<rel_path>`.
    pub fn to_ohm_url(&self, path: &std::path::Path) -> String {
        let rel = path.strip_prefix(&self.pkg_path).unwrap_or(path);
        let rel_str = rel.to_string_lossy();
        let rel_no_ext = rel_str
            .trim_end_matches(".ets")
            .trim_end_matches(".ts")
            .trim_end_matches(".js");
        format!("@bundle:{}/{}/{}", self.bundle_name, self.module_name, rel_no_ext)
    }
}

/// Brief package information for `loader.json` and module-meta plugin.
///
/// Represents a resolved ohpm package summary using String paths (as opposed to
/// the resolver-internal [`arkdown_resolver::PkgBriefInfo`] which uses PathBuf).
/// This is the type written into `loader.json`'s `pkgNameToPkgBriefInfo` field
/// and consumed by the module-meta plugin.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PkgBriefInfo {
    /// Absolute path to the directory containing `oh-package.json5`.
    pub pkg_root: String,
    /// The `name` field from `oh-package.json5`.
    pub pkg_name: String,
    /// Source root directories relative to `pkg_root` (default: `["src/main/ets"]`).
    pub source_roots: Vec<String>,
    /// Original (pre-remapping) source roots — empty for most packages.
    #[serde(default)]
    pub original_source_roots: Vec<String>,
}
