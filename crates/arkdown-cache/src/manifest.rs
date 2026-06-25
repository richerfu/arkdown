//! Cache manifest types.
//!
//! A [`CacheManifest`] is the JSON index file that a cache namespace writes to
//! disk.  It records the set of valid cache entries, their keys, and their
//! artifact fingerprints.  Reading the manifest lets subsequent builds decide
//! quickly which entries are still valid without reading every artifact file.
//!
//! # Upstream correspondence
//!
//! Corresponds to the rollup cache manifest written by
//! `rollup-cache-service.js` after each successful bundle run, and to the
//! task cache service manifest in hvigor.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::CacheError;
use crate::key::CacheKey;

// ---------------------------------------------------------------------------
// CacheManifestVersion
// ---------------------------------------------------------------------------

/// Schema version for the cache manifest file.
///
/// Used to detect and discard manifests from incompatible arkdown versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CacheManifestVersion(pub u32);

impl CacheManifestVersion {
    /// Current manifest schema version.
    pub const CURRENT: CacheManifestVersion = CacheManifestVersion(1);
}

// ---------------------------------------------------------------------------
// CacheManifestEntry
// ---------------------------------------------------------------------------

/// A single entry in a [`CacheManifest`].
///
/// Records the key under which the entry was cached and the relative path to
/// the artifact file within the cache namespace directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheManifestEntry {
    /// The cache key that this entry was stored under.
    pub key: CacheKey,
    /// Path to the artifact file, relative to the manifest's cache directory.
    pub artifact_rel_path: String,
    /// Optional content hash of the artifact (for fast verification without
    /// reading the full file).
    pub artifact_hash: Option<String>,
}

// ---------------------------------------------------------------------------
// CacheManifest
// ---------------------------------------------------------------------------

/// An on-disk index of valid cache entries for a single namespace.
///
/// The manifest is stored as `<cache_dir>/.manifest.json`.  It maps module
/// IDs → [`CacheManifestEntry`] sorted by key for deterministic output.
///
/// # Upstream correspondence
///
/// Corresponds to hvigor `rollup-cache-service.js` bundle cache manifest and
/// the implicit task cache entry index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheManifest {
    /// Schema version.
    pub version: CacheManifestVersion,
    /// Entries, keyed by module/source ID.
    ///
    /// Stored as `BTreeMap` to guarantee sorted, deterministic JSON output.
    pub entries: BTreeMap<String, CacheManifestEntry>,
}

impl CacheManifest {
    /// File name used for the manifest within a cache namespace directory.
    const FILE_NAME: &'static str = ".manifest.json";

    /// Create an empty manifest with the current schema version.
    pub fn empty() -> Self {
        Self {
            version: CacheManifestVersion::CURRENT,
            entries: BTreeMap::new(),
        }
    }

    /// Read the manifest from `cache_dir/.manifest.json`.
    ///
    /// Returns `None` if the file does not exist (first build or cleared
    /// cache).  Returns an error if the file exists but cannot be parsed.
    pub fn load(cache_dir: &Path) -> Result<Option<Self>, CacheError> {
        let path = cache_dir.join(Self::FILE_NAME);
        if !path.exists() {
            return Ok(None);
        }
        let content = fs_err::read_to_string(&path).map_err(CacheError::Io)?;
        let manifest: Self = serde_json::from_str(&content).map_err(CacheError::Decode)?;
        if manifest.version > CacheManifestVersion::CURRENT {
            // Newer manifest; treat as cache miss to avoid schema issues.
            return Ok(None);
        }
        Ok(Some(manifest))
    }

    /// Write the manifest to `cache_dir/.manifest.json`.
    ///
    /// Serializes entries in sorted order for deterministic output.
    pub fn store(&self, cache_dir: &Path) -> Result<(), CacheError> {
        fs_err::create_dir_all(cache_dir).map_err(CacheError::Io)?;
        let path = cache_dir.join(Self::FILE_NAME);
        let json = serde_json::to_string_pretty(self).map_err(CacheError::Decode)?;
        fs_err::write(&path, json).map_err(CacheError::Io)?;
        Ok(())
    }

    /// Look up an entry by its module/source ID.
    pub fn entry(&self, id: &str) -> Option<&CacheManifestEntry> {
        self.entries.get(id)
    }

    /// Insert or update an entry.
    pub fn upsert(&mut self, id: impl Into<String>, entry: CacheManifestEntry) {
        self.entries.insert(id.into(), entry);
    }

    /// Remove all entries whose artifact files no longer exist on disk.
    pub fn remove_stale(&mut self, cache_dir: &Path) {
        self.entries
            .retain(|_, entry| cache_dir.join(&entry.artifact_rel_path).exists());
    }

    /// Return the absolute path to the artifact for an entry.
    pub fn artifact_path_for(&self, id: &str, cache_dir: &Path) -> Option<PathBuf> {
        self.entries
            .get(id)
            .map(|e| cache_dir.join(&e.artifact_rel_path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_manifest_has_no_entries() {
        let m = CacheManifest::empty();
        assert!(m.entries.is_empty());
        assert_eq!(m.version, CacheManifestVersion::CURRENT);
    }

    #[test]
    fn upsert_adds_entry() {
        let mut m = CacheManifest::empty();
        m.upsert(
            "src/Index.ets",
            CacheManifestEntry {
                key: CacheKey {
                    content_hash: "abc".to_string(),
                    config_hash: 0,
                    dependency_hash: 0,
                },
                artifact_rel_path: "abc123.abc".to_string(),
                artifact_hash: None,
            },
        );
        assert!(m.entry("src/Index.ets").is_some());
        assert_eq!(m.entry("src/Index.ets").unwrap().key.content_hash, "abc");
    }

    #[test]
    fn manifest_entries_are_sorted() {
        let mut m = CacheManifest::empty();
        m.upsert(
            "z.ets",
            CacheManifestEntry {
                key: CacheKey {
                    content_hash: "z".to_string(),
                    config_hash: 0,
                    dependency_hash: 0,
                },
                artifact_rel_path: "z.abc".to_string(),
                artifact_hash: None,
            },
        );
        m.upsert(
            "a.ets",
            CacheManifestEntry {
                key: CacheKey {
                    content_hash: "a".to_string(),
                    config_hash: 0,
                    dependency_hash: 0,
                },
                artifact_rel_path: "a.abc".to_string(),
                artifact_hash: None,
            },
        );
        let keys: Vec<_> = m.entries.keys().collect();
        assert_eq!(keys, &["a.ets", "z.ets"]);
    }
}
