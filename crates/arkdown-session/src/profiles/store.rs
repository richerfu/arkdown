//! [`ProfileStore`] — session-owned profile JSON/JSON5 store.
//!
//! Provides typed access to profile files without exposing path construction
//! to individual processors. Profile code no longer constructs profile paths
//! manually.

use crate::project::snapshot::ProjectSnapshot;

/// Session-owned profile file store.
///
/// Feature code calls typed accessors; path construction is internal.
#[derive(Debug, Clone, Default)]
pub struct ProfileStore {
    // TODO: Implement indexed profile store with path lookup.
    // For now, this is a placeholder for the API shape.
}

impl ProfileStore {
    /// Construct a new profile store from the project snapshot.
    pub fn new(_project: &ProjectSnapshot) -> Self {
        Self {}
    }
}
