//! Cache namespace types.
//!
//! Each build stage uses its own cache subdirectory so that entries from
//! different stages never collide.  [`CacheNamespace`] and
//! [`StageCacheNamespace`] provide stable, filesystem-safe path components for
//! the stage-scoped cache directories.
//!
//! # Upstream correspondence
//!
//! Corresponds to the task cache subdirectory structure in hvigor's build
//! cache service, where each task writes to a separate directory under
//! `build/default/cache/<task>/`.

use std::path::PathBuf;

// ---------------------------------------------------------------------------
// CacheNamespace
// ---------------------------------------------------------------------------

/// A top-level cache namespace (e.g., `"bundle"`, `"transform"`, `"abc"`).
///
/// Cache namespaces map directly to subdirectories under the cache root.
///
/// # Upstream correspondence
///
/// Corresponds to hvigor task cache directories created by
/// `rollup-cache-service.js` and the task cache service.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheNamespace {
    /// The namespace name.  Used as a filesystem path component.
    pub name: String,
}

impl CacheNamespace {
    /// The bundle-level rolldown cache namespace.
    ///
    /// Stores the Rolldown bundle output manifest and per-chunk hashes.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `build/default/cache/bundle/` in hvigor.
    pub fn for_bundle() -> Self {
        Self {
            name: "bundle".to_string(),
        }
    }

    /// The per-module transform cache namespace.
    ///
    /// Stores OXC parse/transform outputs (JS/TS source after ETS transform).
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `build/default/cache/transform/` in hvigor.
    pub fn for_transform() -> Self {
        Self {
            name: "transform".to_string(),
        }
    }

    /// The ABC (Panda bytecode) compile cache namespace.
    ///
    /// Stores compiled `.abc` and `.proto` artifacts per compile unit.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `build/default/cache/abc/` in hvigor (ProgramCache).
    pub fn for_abc() -> Self {
        Self {
            name: "abc".to_string(),
        }
    }

    /// The profile-processing cache namespace.
    ///
    /// Stores processed `module.json`, `loader.json`, `pkgContextInfo.json`.
    pub fn for_profile() -> Self {
        Self {
            name: "profile".to_string(),
        }
    }

    /// The resource compilation cache namespace.
    ///
    /// Stores `resources.index` and ResourceTable artifacts.
    pub fn for_resource() -> Self {
        Self {
            name: "resource".to_string(),
        }
    }

    /// Return the path component used in cache directory layout.
    ///
    /// Guaranteed to be a single path segment with no slashes.
    pub fn path_component(&self) -> &str {
        &self.name
    }
}

// ---------------------------------------------------------------------------
// StageCacheNamespace
// ---------------------------------------------------------------------------

/// A stage-scoped cache namespace combining a [`CacheNamespace`] with a module
/// identifier.
///
/// `StageCacheNamespace` uniquely identifies the cache directory for a
/// specific stage × module combination.
///
/// # Upstream correspondence
///
/// Corresponds to the per-module, per-task subdirectory structure under
/// hvigor's task cache root.
#[derive(Debug, Clone)]
pub struct StageCacheNamespace {
    /// Top-level namespace (stage name).
    pub namespace: CacheNamespace,
    /// Module identifier (e.g., `"entry"`, `"feature"`, `"my-har"`).
    pub module_name: String,
}

impl StageCacheNamespace {
    /// Create a new stage-scoped cache namespace.
    pub fn new(namespace: CacheNamespace, module_name: impl Into<String>) -> Self {
        Self {
            namespace,
            module_name: module_name.into(),
        }
    }

    /// Resolve to a subdirectory path relative to the cache root.
    ///
    /// Returns `<namespace>/<module_name>/` as a [`PathBuf`].
    pub fn resolve_under(&self, cache_root: &std::path::Path) -> PathBuf {
        cache_root
            .join(self.namespace.path_component())
            .join(&self.module_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_namespace_path_component() {
        assert_eq!(CacheNamespace::for_bundle().path_component(), "bundle");
    }

    #[test]
    fn stage_namespace_resolves_correctly() {
        let ns = StageCacheNamespace::new(CacheNamespace::for_abc(), "entry");
        let root = std::path::Path::new("/build/cache");
        let resolved = ns.resolve_under(root);
        assert_eq!(resolved, std::path::PathBuf::from("/build/cache/abc/entry"));
    }
}
