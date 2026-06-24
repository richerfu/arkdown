//! ABC record name derivation.
//!
//! ABC record names identify individual modules within a merged `.abc` file.
//! They appear as keys in the `globalThis` registry and in source maps.
//!
//! Record names can be derived from three sources:
//!
//! 1. **OHM URL**: `@bundle:<bundleName>/<moduleName>/<etsRelPath>` or
//!    `@bundle:<bundleName>/<moduleName>/ets/<relPath>` — used with
//!    `useNormalizedOHMUrl: true` (the default for API >= 9).
//! 2. **Relative source path**: `ets/pages/Index` — used for legacy builds.
//! 3. **Explicit**: Supplied directly from `loader.json` `recordName`.
//!
//! # Upstream correspondence
//!
//! | Type | hvigor/SDK equivalent |
//! |------|-----------------------|
//! | `RecordNameDeriver` | `ark_compiler/transform.js::generateRecord` |
//! | `RecordNameInput` | per-file metadata from `loader.json` |
//! | `PackageRecordName` | ohpm package module record name |
//! | `WorkerRecordName` | worker entry record name |

use std::{
    fmt,
    path::{Path, PathBuf},
};

use arkdown_path::{ArtifactPathSerializer, OhmUrl};

// ---------------------------------------------------------------------------
// RecordNameInput
// ---------------------------------------------------------------------------

/// Inputs required to derive the ABC record name for one source file.
///
/// # Upstream correspondence
///
/// Corresponds to the per-file metadata assembled in
/// `ark_compiler/transform.js::generateRecord`.
#[derive(Debug, Clone)]
pub struct RecordNameInput {
    /// The OHM URL or module ID for this file (from `loader.json`).
    ///
    /// If present and `useNormalizedOHMUrl` is true, this is used directly
    /// as the record name.
    pub module_id: Option<String>,
    /// Source path relative to the module root.
    pub source_rel_path: String,
    /// Package name from `oh-package.json5`.
    pub package_name: Option<String>,
    /// OHOS module name (e.g., `"entry"`, `"feature"`).
    pub module_name: String,
    /// Whether `useNormalizedOHMUrl` is active.
    pub use_normalized_ohm_url: bool,
}

// ---------------------------------------------------------------------------
// PackageRecordName
// ---------------------------------------------------------------------------

/// Record name for a module within an ohpm package.
///
/// Package modules use a `<pkg_name>/<rel_path>` record name format in the
/// non-OHM-URL mode.  With `useNormalizedOHMUrl`, the full OHM URL is used.
///
/// # Upstream correspondence
///
/// Corresponds to `ark_compiler/transform.js::generateRecord` for
/// `oh_modules` entries.
#[derive(Debug, Clone)]
pub struct PackageRecordName {
    /// The derived record name string.
    pub value: String,
}

// ---------------------------------------------------------------------------
// WorkerRecordName
// ---------------------------------------------------------------------------

/// Record name for a worker entry point.
///
/// Worker entries use a special `workers/<name>` prefix in hvigor builds.
///
/// # Upstream correspondence
///
/// Corresponds to `process-oh-package-json.js::addWorkerData` record name
/// derivation for `loadWorker` entries.
#[derive(Debug, Clone)]
pub struct WorkerRecordName {
    /// The derived record name string.
    pub value: String,
}

// ---------------------------------------------------------------------------
// RecordNameDeriver
// ---------------------------------------------------------------------------

/// Derives ABC record names from source file metadata.
///
/// `RecordNameDeriver` is a stateless utility type used by `Ts2abcPlanBuilder`
/// to assign record names to compile units.
///
/// # Upstream correspondence
///
/// Corresponds to `ark_compiler/transform.js::generateRecord` in hvigor.
pub struct RecordNameDeriver {
    bundle_name: String,
}

impl RecordNameDeriver {
    /// Create a new deriver for the given bundle name.
    pub fn new(bundle_name: impl Into<String>) -> Self {
        Self {
            bundle_name: bundle_name.into(),
        }
    }

    /// Derive the legacy default record name from a source or output path.
    ///
    /// This preserves upstream-compatible CLI argument parsing behavior.
    pub fn default_record_name(path: &Path) -> String {
        path.file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("record")
            .to_string()
    }

    /// Derive the record name for a source file.
    ///
    /// If the input has an explicit OHM module ID and `useNormalizedOHMUrl`
    /// is true, the OHM URL is returned directly.  Otherwise, the record
    /// name is derived from the relative source path.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `ark_compiler/transform.js::generateRecord`.
    pub fn derive_record_name(&self, input: &RecordNameInput) -> String {
        if input.use_normalized_ohm_url {
            if let Some(module_id) = &input.module_id {
                return module_id.clone();
            }
            // Fall back to OHM URL construction.
            return self.build_ohm_url(&input.module_name, &input.source_rel_path);
        }
        // Legacy path-based record name.
        self.path_stem(&input.source_rel_path)
    }

    /// Derive a record name for an ohpm package module.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to record name derivation for `oh_modules` entries in
    /// `ark_compiler/transform.js`.
    pub fn derive_package_record(
        &self,
        pkg_name: &str,
        rel_path_in_package: &str,
        use_normalized_ohm_url: bool,
    ) -> PackageRecordName {
        let rel_path = self.path_stem(rel_path_in_package);
        let value = if use_normalized_ohm_url {
            PackageRecordNameText::normalized(&self.bundle_name, pkg_name, &rel_path).to_record()
        } else {
            PackageRecordNameText::legacy(pkg_name, &rel_path).to_record()
        };
        PackageRecordName { value }
    }

    /// Derive a record name for a worker entry.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `process-oh-package-json.js::addWorkerData` record
    /// name derivation.
    pub fn derive_worker_record(
        &self,
        module_name: &str,
        worker_rel_path: &str,
    ) -> WorkerRecordName {
        let value = OhmUrl::bundle(
            &self.bundle_name,
            module_name,
            &self.path_stem(worker_rel_path),
        )
        .into_string();
        WorkerRecordName { value }
    }

    /// Derive a record name from loader.json metadata entry.
    ///
    /// If the loader.json entry has a pre-set `recordName`, use it directly.
    /// Otherwise, derive from the module ID or path.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `GenerateLoaderJson::compileEntry` processing in
    /// `generate-loader-json.js`.
    pub fn derive_from_loader_metadata(
        &self,
        module_id: &str,
        explicit_record_name: Option<&str>,
        module_name: &str,
        use_normalized_ohm_url: bool,
    ) -> String {
        if let Some(explicit) = explicit_record_name {
            return explicit.to_string();
        }
        if use_normalized_ohm_url {
            return module_id.to_string();
        }
        LoaderMetadataRecordName::new(module_id, module_name, self).legacy_record()
    }

    // ── Private helpers ──────────────────────────────────────────────────────

    fn build_ohm_url(&self, module_name: &str, rel_path: &str) -> String {
        let stem = self.path_stem(rel_path);
        OhmUrl::bundle(&self.bundle_name, module_name, &stem).into_string()
    }

    fn path_stem(&self, path: &str) -> String {
        ArtifactPathSerializer::rel_to_slash(&Path::new(path).with_extension(""))
    }
}

struct LoaderMetadataRecordName<'a, 'deriver> {
    module_id: &'a str,
    module_name: &'a str,
    deriver: &'deriver RecordNameDeriver,
}

impl<'a, 'deriver> LoaderMetadataRecordName<'a, 'deriver> {
    fn new(module_id: &'a str, module_name: &'a str, deriver: &'deriver RecordNameDeriver) -> Self {
        Self {
            module_id,
            module_name,
            deriver,
        }
    }

    fn legacy_record(&self) -> String {
        self.legacy_rel_path()
            .filter(|rel_path| !rel_path.as_os_str().is_empty())
            .map(|rel_path| {
                self.deriver
                    .path_stem(&ArtifactPathSerializer::rel_to_slash(&rel_path))
            })
            .unwrap_or_else(|| self.module_name.to_string())
    }

    fn legacy_rel_path(&self) -> Option<PathBuf> {
        if let Some(parts) = OhmUrl::parse_bundle_parts(self.module_id) {
            return Some(PathBuf::from(parts.path));
        }

        let mut rel_path = PathBuf::new();
        for component in Path::new(self.module_id).components().skip(2) {
            rel_path.push(component.as_os_str());
        }
        Some(rel_path)
    }
}

enum PackageRecordNameText<'a> {
    Normalized {
        bundle_name: &'a str,
        package_name: &'a str,
        rel_path: &'a str,
    },
    Legacy {
        package_name: &'a str,
        rel_path: &'a str,
    },
}

impl<'a> PackageRecordNameText<'a> {
    fn normalized(bundle_name: &'a str, package_name: &'a str, rel_path: &'a str) -> Self {
        Self::Normalized {
            bundle_name,
            package_name,
            rel_path,
        }
    }

    fn legacy(package_name: &'a str, rel_path: &'a str) -> Self {
        Self::Legacy {
            package_name,
            rel_path,
        }
    }

    fn to_record(&self) -> String {
        self.to_string()
    }
}

impl fmt::Display for PackageRecordNameText<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Normalized {
                bundle_name,
                package_name,
                rel_path,
            } => write!(f, "@package:{bundle_name}@{package_name}/{rel_path}"),
            Self::Legacy {
                package_name,
                rel_path,
            } => write!(f, "{package_name}/{rel_path}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_record_name_with_ohm_url() {
        let deriver = RecordNameDeriver::new("com.example.app");
        let input = RecordNameInput {
            module_id: Some("@bundle:com.example.app/entry/ets/pages/Index".to_string()),
            source_rel_path: "ets/pages/Index.ets".to_string(),
            package_name: None,
            module_name: "entry".to_string(),
            use_normalized_ohm_url: true,
        };
        assert_eq!(
            deriver.derive_record_name(&input),
            "@bundle:com.example.app/entry/ets/pages/Index"
        );
    }

    #[test]
    fn derive_record_name_legacy_strips_extension() {
        let deriver = RecordNameDeriver::new("com.example.app");
        let input = RecordNameInput {
            module_id: None,
            source_rel_path: "ets/pages/Index.ets".to_string(),
            package_name: None,
            module_name: "entry".to_string(),
            use_normalized_ohm_url: false,
        };
        assert_eq!(deriver.derive_record_name(&input), "ets/pages/Index");
    }

    #[test]
    fn default_record_name_uses_file_stem_or_record_fallback() {
        assert_eq!(
            RecordNameDeriver::default_record_name(Path::new("modules.abc")),
            "modules"
        );
        assert_eq!(
            RecordNameDeriver::default_record_name(Path::new("")),
            "record"
        );
    }

    #[test]
    fn derive_worker_record() {
        let deriver = RecordNameDeriver::new("com.example.app");
        let worker = deriver.derive_worker_record("entry", "workers/AudioWorker.ts");
        assert_eq!(
            worker.value,
            "@bundle:com.example.app/entry/workers/AudioWorker"
        );
    }

    #[test]
    fn derive_package_record_with_ohm_url() {
        let deriver = RecordNameDeriver::new("com.example.app");
        let pkg = deriver.derive_package_record("@ohos/utils", "src/index.ts", true);
        assert!(pkg.value.starts_with("@package:com.example.app@"));
        assert!(pkg.value.contains("@ohos/utils"));
    }

    #[test]
    fn derive_loader_metadata_legacy_uses_ohm_bundle_parts() {
        let deriver = RecordNameDeriver::new("com.example.app");
        assert_eq!(
            deriver.derive_from_loader_metadata(
                "@bundle:com.example.app/entry/ets/pages/Index",
                None,
                "entry",
                false,
            ),
            "ets/pages/Index"
        );
    }
}
