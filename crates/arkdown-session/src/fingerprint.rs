//! [`InputFingerprintSet`] — content hashes for all startup inputs.
//!
//! Cache keys must be built from `InputFingerprintSet` or explicit content
//! hashes. Placeholder hashes are forbidden.

use std::path::{Path, PathBuf};

use arkdown_core::ArkdownConfig;
use arkdown_path::{lexical_normalize, to_slash};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::paths::ProjectPathLayout;
use crate::profiles::store::ProfileStore;
use crate::project::snapshot::ProjectSnapshot;
use crate::sdk::snapshot::SdkSnapshot;

/// A single content fingerprint (SHA-256 of file content).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Fingerprint(pub [u8; 32]);

impl Fingerprint {
    /// Compute the fingerprint of the given bytes.
    pub fn of_bytes(data: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&result);
        Self(bytes)
    }

    /// Compute the fingerprint of the given file.
    pub fn of_file(path: &std::path::Path) -> std::io::Result<Self> {
        let data = std::fs::read(path)?;
        Ok(Self::of_bytes(&data))
    }
}

/// Errors produced while fingerprinting startup inputs.
#[derive(Debug, Error)]
pub enum FingerprintError {
    /// A file could not be read for hashing.
    #[error("failed to read fingerprint input {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// A directory walk failed.
    #[error("failed to scan fingerprint input {path}: {message}")]
    Walk { path: PathBuf, message: String },

    /// A structured startup value could not be serialized for hashing.
    #[error("failed to serialize fingerprint value {name}: {source}")]
    Serialize {
        name: &'static str,
        #[source]
        source: serde_json::Error,
    },
}

/// The set of all startup input fingerprints.
///
/// Used to build cache keys and to determine which stages need to re-run
/// when inputs change.
#[derive(Debug, Clone)]
pub struct InputFingerprintSet {
    /// Hash of the build configuration.
    pub config: Fingerprint,
    /// Hash of the dependency graph inputs (all `oh-package.json5` files).
    pub package_graph: Fingerprint,
    /// Hash of all profile JSON input files.
    pub profiles: Fingerprint,
    /// Hash of all resource input files.
    pub resources: Fingerprint,
    /// Hash of all source files.
    pub sources: Fingerprint,
    /// Hash of the selected SDK metadata.
    pub sdk: Fingerprint,
}

impl InputFingerprintSet {
    /// Build the fingerprint set from normalized startup snapshots.
    pub fn build(
        config: &ArkdownConfig,
        project: &ProjectSnapshot,
        profiles: &ProfileStore,
        sdk: &SdkSnapshot,
    ) -> Result<Self, FingerprintError> {
        Ok(Self {
            config: Self::compute_config_hash(config, project)?,
            package_graph: Self::compute_package_hash(project)?,
            profiles: profiles.fingerprint(),
            resources: Self::compute_resource_hash(project)?,
            sources: Self::compute_source_hash(project)?,
            sdk: Self::compute_sdk_hash(sdk)?,
        })
    }

    fn compute_config_hash(
        config: &ArkdownConfig,
        project: &ProjectSnapshot,
    ) -> Result<Fingerprint, FingerprintError> {
        let mut builder = FingerprintBuilder::new("config", &project.root);
        let encoded = serde_json::to_vec(config).map_err(|source| FingerprintError::Serialize {
            name: "ArkdownConfig",
            source,
        })?;
        builder.add_value("normalized_config", &encoded);
        builder.add_file_if_present(&project.root.join("build-profile.json5"))?;
        builder
            .add_file_if_present(&ProjectPathLayout::new(&project.root).app_scope_app_json5())?;
        for module in project.modules.values() {
            if let Some(path) = &module.module_json_path {
                builder.add_file(path)?;
            }
        }
        Ok(builder.finish())
    }

    fn compute_package_hash(project: &ProjectSnapshot) -> Result<Fingerprint, FingerprintError> {
        let mut builder = FingerprintBuilder::new("package_graph", &project.root);
        for path in builder.named_files_under(&project.root, "oh-package.json5")? {
            builder.add_file(&path)?;
        }
        Ok(builder.finish())
    }

    fn compute_source_hash(project: &ProjectSnapshot) -> Result<Fingerprint, FingerprintError> {
        let mut builder = FingerprintBuilder::new("sources", &project.root);
        for module in project.modules.values() {
            for root in &module.source_roots {
                for path in builder.files_under(root)? {
                    if FingerprintBuilder::is_source_input(&path) {
                        builder.add_file(&path)?;
                    }
                }
            }
        }
        Ok(builder.finish())
    }

    fn compute_resource_hash(project: &ProjectSnapshot) -> Result<Fingerprint, FingerprintError> {
        let mut builder = FingerprintBuilder::new("resources", &project.root);
        let app_scope_resources = ProjectPathLayout::new(&project.root).app_scope_resources();
        if app_scope_resources.is_dir() {
            for path in builder.files_under(&app_scope_resources)? {
                builder.add_file(&path)?;
            }
        }
        for module in project.modules.values() {
            for root in &module.resource_roots {
                for path in builder.files_under(root)? {
                    builder.add_file(&path)?;
                }
            }
        }
        Ok(builder.finish())
    }

    fn compute_sdk_hash(sdk: &SdkSnapshot) -> Result<Fingerprint, FingerprintError> {
        let mut builder = FingerprintBuilder::new("sdk", &sdk.sdk_root);
        builder.add_value("sdk_root", to_slash(&sdk.sdk_root).as_bytes());
        builder.add_value("api_version", sdk.api_version.to_string().as_bytes());
        builder.add_value("compile_sdk_version", sdk.compile_sdk_version.as_bytes());

        for relative in [
            "ets/oh-uni-package.json",
            "native/oh-uni-package.json",
            "toolchains/oh-uni-package.json",
            "js/oh-uni-package.json",
            "previewer/oh-uni-package.json",
            "ets/build-tools/ets-loader/package.json",
            "js/build-tools/js-loader/package.json",
        ] {
            builder.add_file_if_present(&sdk.sdk_root.join(relative))?;
        }

        for schema in [
            crate::sdk_schemas::SdkSchemaKind::App,
            crate::sdk_schemas::SdkSchemaKind::Module,
            crate::sdk_schemas::SdkSchemaKind::Page,
            crate::sdk_schemas::SdkSchemaKind::Form,
            crate::sdk_schemas::SdkSchemaKind::RouterMap,
            crate::sdk_schemas::SdkSchemaKind::ShareConfig,
            crate::sdk_schemas::SdkSchemaKind::AppStartup,
            crate::sdk_schemas::SdkSchemaKind::HarStartup,
            crate::sdk_schemas::SdkSchemaKind::HspStartup,
            crate::sdk_schemas::SdkSchemaKind::Syscap,
            crate::sdk_schemas::SdkSchemaKind::Pac,
            crate::sdk_schemas::SdkSchemaKind::Shortcuts,
            crate::sdk_schemas::SdkSchemaKind::InsightIntent,
        ] {
            if let Some(path) = sdk.schema_path(schema) {
                builder.add_file_if_present(path.as_path())?;
            }
        }

        Ok(builder.finish())
    }
}

/// Builder for deterministic, typed fingerprint inputs.
#[derive(Debug)]
pub struct FingerprintBuilder {
    base: PathBuf,
    hasher: Sha256,
    entries: usize,
}

impl FingerprintBuilder {
    /// Create a builder for one logical fingerprint kind.
    pub fn new(kind: &'static str, base: &Path) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(b"arkdown:fingerprint:v1");
        hasher.update([0]);
        hasher.update(kind.as_bytes());
        hasher.update([0]);
        Self {
            base: lexical_normalize(base),
            hasher,
            entries: 0,
        }
    }

    /// Add a structured value to the fingerprint.
    pub fn add_value(&mut self, name: &'static str, value: &[u8]) {
        self.hasher.update(b"value");
        self.hasher.update([0]);
        self.hasher.update(name.as_bytes());
        self.hasher.update([0]);
        self.hasher.update((value.len() as u64).to_le_bytes());
        self.hasher.update([0]);
        self.hasher.update(value);
        self.hasher.update([0xff]);
        self.entries += 1;
    }

    /// Add a file path and content to the fingerprint.
    pub fn add_file(&mut self, path: &Path) -> Result<(), FingerprintError> {
        let normalized = lexical_normalize(path);
        let data = std::fs::read(&normalized).map_err(|source| FingerprintError::Io {
            path: normalized.clone(),
            source,
        })?;
        let key = self.path_key(&normalized);
        self.hasher.update(b"file");
        self.hasher.update([0]);
        self.hasher.update(key.as_bytes());
        self.hasher.update([0]);
        self.hasher.update((data.len() as u64).to_le_bytes());
        self.hasher.update([0]);
        self.hasher.update(&data);
        self.hasher.update([0xff]);
        self.entries += 1;
        Ok(())
    }

    pub(crate) fn add_file_fingerprint(&mut self, path: &Path, fingerprint: &Fingerprint) {
        let normalized = lexical_normalize(path);
        let key = self.path_key(&normalized);
        self.hasher.update(b"file_fingerprint");
        self.hasher.update([0]);
        self.hasher.update(key.as_bytes());
        self.hasher.update([0]);
        self.hasher.update(fingerprint.0);
        self.hasher.update([0xff]);
        self.entries += 1;
    }

    fn add_file_if_present(&mut self, path: &Path) -> Result<(), FingerprintError> {
        if path.is_file() {
            self.add_file(path)?;
        }
        Ok(())
    }

    fn named_files_under(&self, root: &Path, name: &str) -> Result<Vec<PathBuf>, FingerprintError> {
        let mut files = Vec::new();
        for path in self.files_under(root)? {
            if path.file_name().and_then(|value| value.to_str()) == Some(name) {
                files.push(path);
            }
        }
        Ok(files)
    }

    fn files_under(&self, root: &Path) -> Result<Vec<PathBuf>, FingerprintError> {
        if !root.is_dir() {
            return Ok(Vec::new());
        }
        let mut files = Vec::new();
        for entry in walkdir::WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|entry| !Self::is_generated_or_external_dir(entry.path(), root))
        {
            let entry = entry.map_err(|error| FingerprintError::Walk {
                path: error
                    .path()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| root.to_path_buf()),
                message: error.to_string(),
            })?;
            if entry.file_type().is_file() {
                files.push(entry.path().to_path_buf());
            }
        }
        files.sort();
        Ok(files)
    }

    fn is_generated_or_external_dir(path: &Path, root: &Path) -> bool {
        if path == root {
            return false;
        }
        matches!(
            path.file_name().and_then(|value| value.to_str()),
            Some(".git" | "build" | "out" | "node_modules")
        )
    }

    fn is_source_input(path: &Path) -> bool {
        matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("ets" | "ts" | "js" | "json" | "json5")
        )
    }

    fn path_key(&self, path: &Path) -> String {
        path.strip_prefix(&self.base)
            .map(to_slash)
            .unwrap_or_else(|_| to_slash(path))
    }

    /// Finish the builder.
    pub fn finish(mut self) -> Fingerprint {
        if self.entries == 0 {
            self.hasher.update(b"empty");
        }
        let result = self.hasher.finalize();
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&result);
        Fingerprint(bytes)
    }
}
