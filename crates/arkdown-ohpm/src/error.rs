//! Typed errors for the `arkdown-ohpm` crate.

use std::path::PathBuf;

use thiserror::Error;

/// Errors produced by OHPM package acquisition operations.
#[derive(Debug, Error)]
pub enum OhpmError {
    /// The OHPM global cache root could not be determined.
    #[error("OHPM cache root not found: set $OHPM_HOME or ensure ~/.ohpm exists")]
    NoCacheRoot,

    /// A package is not present in the offline cache and offline mode is
    /// enabled.
    #[error(
        "package {name}@{version} not in OHPM cache and offline mode is enabled"
    )]
    OfflineMiss { name: String, version: String },

    /// The package was not found in the registry.
    #[error("package {name} not found in registry: {message}")]
    RegistryNotFound { name: String, message: String },

    /// HTTP request to the OHPM registry failed.
    #[error("OHPM registry request failed: {0}")]
    RegistryHttp(#[source] reqwest::Error),

    /// Integrity verification failed for a downloaded tarball.
    #[error("integrity check failed for {name}@{version}: expected {expected}, got {actual}")]
    IntegrityMismatch {
        name: String,
        version: String,
        expected: String,
        actual: String,
    },

    /// Tarball extraction failed.
    #[error("tarball extraction failed for {0}: {1}")]
    TarballExtract(String, String),

    /// A path traversal was detected in a tarball entry.
    #[error("unsafe tarball entry detected: {0}")]
    UnsafeTarballEntry(String),

    /// An I/O error occurred.
    #[error("I/O error at {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// JSON/JSON5 parse error.
    #[error("parse error in {file}: {message}")]
    Parse { file: PathBuf, message: String },

    /// Generic acquisition error.
    #[error("{0}")]
    Other(String),
}

impl OhpmError {
    /// Construct an I/O error with a path.
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io { path: path.into(), source }
    }
}
