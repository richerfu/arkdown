//! Read-only access to source identities and bytes captured at session startup.

use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A captured source cannot be read after its owning build retires the bytes.
#[derive(Debug, thiserror::Error)]
pub enum SourceReadError {
    #[error("ARKDOWN_SOURCE_TEXT_RELEASED: source text was released: {path}")]
    Released { path: PathBuf },
    #[error("source was not captured at session startup: {path}")]
    NotCaptured { path: PathBuf },
    #[error("source is not UTF-8 text: {path}")]
    NotText { path: PathBuf },
}

/// Shared startup inventory boundary for resolvers, transforms and source loaders.
/// Implementations must only consult captured data; misses leave the caller's
/// existing filesystem fallback in control.
pub trait SourceInventory: std::fmt::Debug + Send + Sync {
    fn canonical_path(&self, path: &Path) -> Option<&Path>;
    fn source_text(&self, path: &Path) -> Result<Option<Arc<str>>, SourceReadError>;
}
