//! Read-only access to source identities and bytes captured at session startup.

use std::path::Path;
use std::sync::Arc;

/// Shared startup inventory boundary for resolvers, transforms and source loaders.
/// Implementations must only consult captured data; misses leave the caller's
/// existing filesystem fallback in control.
pub trait SourceInventory: std::fmt::Debug + Send + Sync {
    fn canonical_path(&self, path: &Path) -> Option<&Path>;
    fn source_text(&self, path: &Path) -> Option<Arc<str>>;
}
