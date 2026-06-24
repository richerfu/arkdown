//! Build cache store.
//!
//! [`BuildCache`] is the central in-process cache store that manages the
//! on-disk cache directory.  It supports per-module entry read/write and
//! compile-environment invalidation.
//!
//! # Upstream correspondence
//!
//! | Type | hvigor equivalent |
//! |------|-------------------|
//! | `BuildCache` | `rollup-cache-service.js` combined with ABC ProgramCache |
//! | `BuildCacheOpener` | constructor + `open()` pattern |
//! | `CacheStore` | low-level entry store (shared between namespaces) |
//! | `CacheHit` | result of a successful cache lookup |
//! | `CacheCommit` | pending cache write to be applied at stage end |

use std::path::PathBuf;

use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

use crate::error::CacheError;
use crate::key::{BundleCacheKey, CacheKey, CacheKeyMaterial};

/// File name for the compile-environment discriminator.
const COMPILE_ENV_FILE: &str = ".compile_env";

// ---------------------------------------------------------------------------
// CachedEntry  (migrated from lib.rs)
// ---------------------------------------------------------------------------

/// A single cached module entry persisted to disk between builds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedEntry {
    /// Resolved module ID (OHM URL or absolute path).
    pub id: String,
    /// The key used to validate this entry.
    pub key: CacheKey,
    /// Output of the transform pipeline for this module.
    pub transformed_code: String,
    /// Inline source map (JSON string), if generated.
    pub source_map: Option<String>,
}

// ---------------------------------------------------------------------------
// CacheHit
// ---------------------------------------------------------------------------

/// Result of a successful cache lookup.
///
/// A `CacheHit` confirms that the stored entry is still valid (key matches)
/// and provides access to the cached artifact content.
#[derive(Debug, Clone)]
pub struct CacheHit {
    /// The cached module entry.
    pub entry: CachedEntry,
    /// Path to the on-disk artifact file, if one was written for this entry.
    pub artifact_path: Option<PathBuf>,
}

// ---------------------------------------------------------------------------
// CacheCommit
// ---------------------------------------------------------------------------

/// A pending cache write, to be applied at the end of a build stage.
///
/// Using `CacheCommit` allows writes to be batched and flushed after all
/// stage outputs are confirmed successful, rather than writing on each module.
#[derive(Debug, Clone)]
pub struct CacheCommit {
    /// The entry to write.
    pub entry: CachedEntry,
    /// Optional artifact file path to associate.
    pub artifact_path: Option<PathBuf>,
}

// ---------------------------------------------------------------------------
// CacheStore
// ---------------------------------------------------------------------------

/// Low-level key-value entry store within a single cache namespace.
///
/// `CacheStore` manages a directory of JSON entry files.  It is used by
/// [`BuildCache`] to store per-module cache entries for a specific build stage.
///
/// # Upstream correspondence
///
/// Corresponds to the per-task cache directory storage in hvigor's task cache
/// service.
pub struct CacheStore {
    cache_dir: PathBuf,
    /// In-memory index: module ID → cached entry.
    memory: FxHashMap<String, CachedEntry>,
}

impl CacheStore {
    /// Open (or create) a cache store at `cache_dir`.
    pub fn open(cache_dir: PathBuf) -> Self {
        Self {
            cache_dir,
            memory: FxHashMap::default(),
        }
    }

    /// Load all entries from disk into the in-memory index.
    pub fn load(&mut self) -> Result<(), CacheError> {
        if !self.cache_dir.exists() {
            return Ok(());
        }
        for entry in std::fs::read_dir(&self.cache_dir)?.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if let Ok(content) = fs_err::read_to_string(&path) {
                if let Ok(cached) = serde_json::from_str::<CachedEntry>(&content) {
                    self.memory.insert(cached.id.clone(), cached);
                }
            }
        }
        Ok(())
    }

    /// Get a cached entry by module ID.
    pub fn get(&self, id: &str) -> Option<&CachedEntry> {
        self.memory.get(id)
    }

    /// Check if an entry is valid for the given key.
    pub fn is_valid(&self, id: &str, key: &CacheKey) -> bool {
        self.memory.get(id).map(|e| &e.key == key).unwrap_or(false)
    }

    /// Write an entry to disk and update the in-memory index.
    pub fn put(&mut self, entry: CachedEntry) -> Result<(), CacheError> {
        let file_name = CacheEntryFileName::new(&entry.id).file_name();
        let cache_file = self.cache_dir.join(file_name);
        fs_err::create_dir_all(&self.cache_dir).map_err(|e| CacheError::Io(e.into()))?;
        let json = serde_json::to_string_pretty(&entry)?;
        fs_err::write(&cache_file, &json).map_err(|e| CacheError::Io(e.into()))?;
        self.memory.insert(entry.id.clone(), entry);
        Ok(())
    }

    /// Remove all entries from memory and disk.
    pub fn clear_namespace(&mut self) -> Result<(), CacheError> {
        self.memory.clear();
        if self.cache_dir.exists() {
            for entry in std::fs::read_dir(&self.cache_dir)?.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    std::fs::remove_file(path).ok();
                }
            }
        }
        Ok(())
    }
}

struct CacheEntryFileName<'a> {
    id: &'a str,
}

impl<'a> CacheEntryFileName<'a> {
    fn new(id: &'a str) -> Self {
        Self { id }
    }

    fn file_name(&self) -> PathBuf {
        PathBuf::from(CacheKeyMaterial::compute_content_hash(self.id)).with_extension("json")
    }
}

// ---------------------------------------------------------------------------
// BuildCacheOpener
// ---------------------------------------------------------------------------

/// Builder for opening a [`BuildCache`].
///
/// # Upstream correspondence
///
/// Corresponds to the cache service initialization in hvigor's task cache
/// service.
pub struct BuildCacheOpener {
    root: PathBuf,
}

impl BuildCacheOpener {
    /// Create a new opener for `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Open the build cache at the root directory.
    pub fn open(self) -> BuildCache {
        BuildCache::new(self.root)
    }
}

// ---------------------------------------------------------------------------
// BuildCache
// ---------------------------------------------------------------------------

/// Persistent incremental build cache.
///
/// Stores per-module [`CachedEntry`] snapshots on disk so that unchanged
/// modules can skip re-parsing, re-transforming, and re-bundling on the next
/// build.
///
/// # Storage layout
///
/// Each module gets its own JSON file under `cache_dir`:
/// `<fnv1a_hex_of_id>.json`
///
/// The compile-environment discriminator is stored in `.compile_env`.
///
/// # Upstream correspondence
///
/// Corresponds to `rollup-cache-service.js` and the ABC ProgramCache.
pub struct BuildCache {
    cache_dir: PathBuf,
    /// In-memory store for this build session.
    memory: FxHashMap<String, CachedEntry>,
}

impl BuildCache {
    /// Create a new cache instance pointing at `cache_dir`.
    pub fn new(cache_dir: PathBuf) -> Self {
        BuildCache {
            cache_dir,
            memory: FxHashMap::default(),
        }
    }

    /// Scan `cache_dir` and load all valid JSON cache files into memory.
    pub fn load_from_disk(&mut self) -> Result<(), CacheError> {
        if !self.cache_dir.exists() {
            return Ok(());
        }
        for entry in std::fs::read_dir(&self.cache_dir)?.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if let Ok(content) = fs_err::read_to_string(&path) {
                if let Ok(cached) = serde_json::from_str::<CachedEntry>(&content) {
                    self.memory.insert(cached.id.clone(), cached);
                }
            }
        }
        Ok(())
    }

    /// Look up a module by its resolved ID.
    pub fn get(&self, id: &str) -> Option<&CachedEntry> {
        self.memory.get(id)
    }

    /// Return `true` if the cached entry for `id` exists and its key matches.
    pub fn is_valid(&self, id: &str, key: &CacheKey) -> bool {
        self.memory.get(id).map(|e| &e.key == key).unwrap_or(false)
    }

    /// Insert or update a cache entry both in memory and on disk.
    pub fn insert(&mut self, entry: CachedEntry) -> Result<(), CacheError> {
        let file_name = CacheEntryFileName::new(&entry.id).file_name();
        let cache_file = self.cache_dir.join(file_name);
        fs_err::create_dir_all(&self.cache_dir).map_err(|e| CacheError::Io(e.into()))?;
        let json = serde_json::to_string_pretty(&entry)?;
        fs_err::write(&cache_file, &json).map_err(|e| CacheError::Io(e.into()))?;
        self.memory.insert(entry.id.clone(), entry);
        Ok(())
    }

    /// Remove all in-memory entries and delete all on-disk JSON files.
    pub fn invalidate_all(&mut self) -> Result<(), CacheError> {
        self.memory.clear();
        if self.cache_dir.exists() {
            for entry in std::fs::read_dir(&self.cache_dir)?.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    std::fs::remove_file(path).ok();
                }
            }
        }
        Ok(())
    }

    /// Compare `current_env` against the persisted compile-environment string.
    pub fn check_and_invalidate_compile_env(
        &mut self,
        current_env: &str,
    ) -> Result<bool, CacheError> {
        let env_file = self.cache_dir.join(COMPILE_ENV_FILE);
        let stored = if env_file.exists() {
            fs_err::read_to_string(&env_file)
                .map_err(|e| CacheError::Io(e.into()))
                .unwrap_or_default()
        } else {
            String::new()
        };
        if stored == current_env {
            return Ok(false);
        }
        self.invalidate_all()?;
        fs_err::create_dir_all(&self.cache_dir).map_err(|e| CacheError::Io(e.into()))?;
        fs_err::write(&env_file, current_env).map_err(|e| CacheError::Io(e.into()))?;
        Ok(true)
    }

    /// Number of entries currently loaded in memory.
    pub fn len(&self) -> usize {
        self.memory.len()
    }

    /// Returns `true` if no entries are loaded.
    pub fn is_empty(&self) -> bool {
        self.memory.is_empty()
    }

    /// Return the path to the bundle-level cache manifest file.
    pub fn bundle_manifest_path(&self) -> PathBuf {
        self.cache_dir.join("bundle.json")
    }

    /// Persist `key` as the bundle-level cache manifest.
    pub fn write_bundle_manifest(&self, key: &BundleCacheKey) -> Result<(), CacheError> {
        fs_err::create_dir_all(&self.cache_dir).map_err(|e| CacheError::Io(e.into()))?;
        let json = serde_json::to_string_pretty(key)?;
        fs_err::write(self.bundle_manifest_path(), json).map_err(|e| CacheError::Io(e.into()))?;
        Ok(())
    }

    /// Read the stored bundle-level cache manifest.
    pub fn read_bundle_manifest(&self) -> Option<BundleCacheKey> {
        let path = self.bundle_manifest_path();
        let content = fs_err::read_to_string(&path).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// Remove all entries in a specific namespace subdirectory.
    pub fn commit(&mut self, pending: Vec<CacheCommit>) -> Result<(), CacheError> {
        for commit in pending {
            self.insert(commit.entry)?;
        }
        Ok(())
    }

    /// Remove all entries in a specific named namespace.
    pub fn remove(&mut self, id: &str) -> Result<(), CacheError> {
        if self.memory.remove(id).is_some() {
            let file_name = CacheEntryFileName::new(id).file_name();
            let path = self.cache_dir.join(file_name);
            if path.exists() {
                std::fs::remove_file(&path)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(id: &str, content_hash: &str) -> CachedEntry {
        CachedEntry {
            id: id.to_string(),
            key: CacheKey {
                content_hash: content_hash.to_string(),
                config_hash: 0,
                dependency_hash: 0,
            },
            transformed_code: String::new(),
            source_map: None,
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let seq = NEXT.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("arkdown_store_test_{name}_{seq}"))
    }

    #[test]
    fn insert_and_retrieve() {
        let dir = temp_dir("insert");
        let _ = std::fs::remove_dir_all(&dir);
        let mut cache = BuildCache::new(dir.clone());
        cache.insert(make_entry("a.ets", "hash1")).unwrap();
        assert!(cache.get("a.ets").is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn build_cache_opener() {
        let dir = temp_dir("opener");
        let _ = std::fs::remove_dir_all(&dir);
        let cache = BuildCacheOpener::new(dir.clone()).open();
        assert_eq!(cache.cache_dir, dir);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
