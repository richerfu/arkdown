//! Read-only access to source identities and content frozen by startup fingerprints.

use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Failures reading fingerprint-frozen source content, including retired bytes.
#[derive(Debug, thiserror::Error)]
pub enum SourceReadError {
    #[error("ARKDOWN_SOURCE_TEXT_RELEASED: source text was released: {path}")]
    Released { path: PathBuf },
    #[error("source was not captured at session startup: {path}")]
    NotCaptured { path: PathBuf },
    #[error("source is not UTF-8 text: {path}")]
    NotText { path: PathBuf },
    #[error("ARKDOWN_SOURCE_RELOAD_IO: cannot reload source {path}: {source}")]
    ReloadIo {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("ARKDOWN_SOURCE_SNAPSHOT_MISMATCH: source changed since startup: {path} (expected {expected_size} bytes/{expected_hash}, got {actual_size} bytes/{actual_hash})")]
    SnapshotMismatch {
        path: PathBuf,
        expected_size: u64,
        actual_size: u64,
        expected_hash: String,
        actual_hash: String,
    },
}

/// A captured file identity without a text lease. The owner verifies any reload
/// against its startup fingerprint; consumers retain the returned text as needed.
pub trait SourceTextHandle: std::fmt::Debug + Send + Sync {
    fn size_bytes(&self) -> u64;
    fn read_text(&self) -> Result<Arc<str>, SourceReadError>;
}

/// Shared startup inventory boundary for resolvers, transforms and source loaders.
/// Captured identities and fingerprints are immutable. Evicted text may be
/// reloaded only after fingerprint verification; released text must fail. A
/// captured file's read failure must never fall through to an unchecked disk read.
/// Unknown identities leave the caller's existing filesystem fallback in control.
pub trait SourceInventory: std::fmt::Debug + Send + Sync {
    fn canonical_path(&self, path: &Path) -> Option<&Path>;
    fn source_text(&self, path: &Path) -> Result<Option<Arc<str>>, SourceReadError>;
}
