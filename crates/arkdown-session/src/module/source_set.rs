//! Source-set overlay calculation and source root inventory.

use std::path::PathBuf;

/// A source-set overlay that defines additional source and resource roots.
#[derive(Debug, Clone)]
pub struct SourceSetOverlay {
    /// Source root directories contributed by this overlay.
    pub source_roots: Vec<PathBuf>,
    /// Resource root directories contributed by this overlay.
    pub resource_roots: Vec<PathBuf>,
}

/// A computed set of source roots after applying all overlays for a target.
#[derive(Debug, Clone, Default)]
pub struct SourceRootSet {
    /// Primary source roots.
    pub roots: Vec<PathBuf>,
    /// Overlay source roots (from additional source sets).
    pub overlay_roots: Vec<PathBuf>,
}

impl SourceRootSet {
    /// Return all source roots in priority order (overlay first).
    pub fn all_roots(&self) -> impl Iterator<Item = &PathBuf> {
        self.overlay_roots.iter().chain(self.roots.iter())
    }
}
