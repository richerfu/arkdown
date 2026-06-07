//! Domain-neutral path newtypes and traits.
//!
//! For the full typed path domain — `AbsPath`, `RelPath`, `ArtifactPath`,
//! `OhmUrl`, `ModuleSpecifier`, and `ArtifactPathSerializer` — see the
//! `arkdown-path` crate.
//!
//! This module re-exports the path types that `arkdown-core` consumers need
//! without directly depending on `arkdown-path`, keeping the foundation
//! dependency graph clean.

// Currently arkdown-core does not depend on arkdown-path to avoid cycles.
// Path helpers that are purely domain-neutral remain here as thin wrappers.

use std::path::{Component, Path, PathBuf};

/// Lexically normalize a path without performing filesystem access.
///
/// Collapses `.` components and resolves `..` components in place. Symlinks
/// are NOT resolved. Suitable for output paths that may not exist yet.
///
/// See also: `arkdown_path::lexical_normalize` (same algorithm).
pub fn lexical_normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push(component);
                }
            }
            c => normalized.push(c),
        }
    }
    normalized
}
