use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Key that uniquely identifies a valid cache entry for a module.
///
/// A cache entry is valid if and only if all three hash components match the
/// values computed from the current build inputs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CacheKey {
    /// FNV-1a hex digest of the source file content.
    /// (A real implementation would use SHA-256; FNV-1a avoids the extra dep.)
    pub content_hash: String,
    /// Fingerprint derived from build config: SDK version, build mode,
    /// `useNormalizedOHMUrl`, obfuscation settings, etc.
    pub config_hash: u64,
    /// Hash of all direct import specifiers' resolved IDs (dependency graph).
    pub dependency_hash: u64,
}

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

/// Errors produced by cache operations.
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

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
/// # Usage
///
/// ```rust,no_run
/// # use arkdown_cache::{BuildCache, CacheKey, CachedEntry, compute_content_hash};
/// let mut cache = BuildCache::new(std::path::PathBuf::from("build/.arkdown_cache"));
/// cache.load_from_disk().unwrap();
///
/// let key = CacheKey {
///     content_hash: compute_content_hash("let x = 1;"),
///     config_hash: 0,
///     dependency_hash: 0,
/// };
/// if cache.is_valid("src/Index.ets", &key) {
///     // use cached output
/// }
/// ```
pub struct BuildCache {
    cache_dir: PathBuf,
    /// In-memory store for this build session (populated on first access).
    memory: HashMap<String, CachedEntry>,
}

impl BuildCache {
    /// Create a new cache instance pointing at `cache_dir`.
    /// Call [`load_from_disk`] to hydrate the in-memory store before the build.
    pub fn new(cache_dir: PathBuf) -> Self {
        BuildCache {
            cache_dir,
            memory: HashMap::new(),
        }
    }

    /// Scan `cache_dir` and load all valid JSON cache files into memory.
    /// Files that fail to parse are silently skipped (stale/corrupt entries).
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

    /// Return `true` if the cached entry for `id` exists and its key matches `key`.
    pub fn is_valid(&self, id: &str, key: &CacheKey) -> bool {
        self.memory
            .get(id)
            .map(|e| &e.key == key)
            .unwrap_or(false)
    }

    /// Insert or update a cache entry both in memory and on disk.
    pub fn insert(&mut self, entry: CachedEntry) -> Result<(), CacheError> {
        let file_name = format!("{}.json", hash_id(&entry.id));
        let cache_file = self.cache_dir.join(file_name);
        fs_err::create_dir_all(&self.cache_dir)?;
        let json = serde_json::to_string_pretty(&entry)?;
        fs_err::write(&cache_file, &json)?;
        self.memory.insert(entry.id.clone(), entry);
        Ok(())
    }

    /// Remove all in-memory entries and delete all on-disk JSON files.
    /// Use this when a config change requires a full rebuild.
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

    /// Number of entries currently loaded in memory.
    pub fn len(&self) -> usize {
        self.memory.len()
    }

    /// Returns `true` if no entries are loaded in memory.
    pub fn is_empty(&self) -> bool {
        self.memory.is_empty()
    }
}

/// Compute a deterministic content hash from source text.
///
/// Uses FNV-1a 64-bit to avoid adding a SHA-256 dependency; the hex string
/// is stored in [`CacheKey::content_hash`].
pub fn compute_content_hash(content: &str) -> String {
    format!("{:016x}", fnv1a_hash(content))
}

/// Derive a stable file-system safe name from a module ID.
fn hash_id(id: &str) -> String {
    format!("{:016x}", fnv1a_hash(id))
}

fn fnv1a_hash(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
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

    #[test]
    fn test_cache_roundtrip() {
        let tmp = std::env::temp_dir().join("arkdown_cache_roundtrip_test");
        let _ = std::fs::remove_dir_all(&tmp);

        let mut cache = BuildCache::new(tmp.clone());
        let entry = make_entry("src/main/ets/Index.ets", "abc123");
        cache.insert(entry.clone()).unwrap();

        assert!(cache.is_valid("src/main/ets/Index.ets", &entry.key));

        // Reload from disk into a fresh instance.
        let mut cache2 = BuildCache::new(tmp.clone());
        cache2.load_from_disk().unwrap();
        assert!(cache2.is_valid("src/main/ets/Index.ets", &entry.key));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_content_hash_deterministic() {
        assert_eq!(compute_content_hash("hello"), compute_content_hash("hello"));
        assert_ne!(compute_content_hash("hello"), compute_content_hash("world"));
    }

    #[test]
    fn test_cache_miss_on_changed_content_hash() {
        let tmp = std::env::temp_dir().join("arkdown_cache_miss_test");
        let _ = std::fs::remove_dir_all(&tmp);

        let mut cache = BuildCache::new(tmp.clone());
        cache.insert(make_entry("a.ts", "old_hash")).unwrap();

        let new_key = CacheKey {
            content_hash: "new_hash".to_string(),
            config_hash: 0,
            dependency_hash: 0,
        };
        assert!(!cache.is_valid("a.ts", &new_key));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_cache_miss_on_changed_config_hash() {
        let tmp = std::env::temp_dir().join("arkdown_cache_config_miss_test");
        let _ = std::fs::remove_dir_all(&tmp);

        let mut cache = BuildCache::new(tmp.clone());
        cache.insert(make_entry("b.ts", "hash_b")).unwrap();

        let different_config = CacheKey {
            content_hash: "hash_b".to_string(),
            config_hash: 42, // differs
            dependency_hash: 0,
        };
        assert!(!cache.is_valid("b.ts", &different_config));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_invalidate_all_clears_memory_and_disk() {
        let tmp = std::env::temp_dir().join("arkdown_cache_invalidate_test");
        let _ = std::fs::remove_dir_all(&tmp);

        let mut cache = BuildCache::new(tmp.clone());
        cache.insert(make_entry("c.ts", "hash_c")).unwrap();
        assert!(!cache.is_empty());

        cache.invalidate_all().unwrap();
        assert!(cache.is_empty());

        // Disk should also be cleared.
        let mut cache2 = BuildCache::new(tmp.clone());
        cache2.load_from_disk().unwrap();
        assert!(cache2.is_empty());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_load_from_empty_dir_is_ok() {
        let tmp = std::env::temp_dir().join("arkdown_cache_empty_dir_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let mut cache = BuildCache::new(tmp.clone());
        cache.load_from_disk().unwrap();
        assert!(cache.is_empty());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_load_from_nonexistent_dir_is_ok() {
        let tmp = std::env::temp_dir().join("arkdown_cache_nodir_test_nonexistent");
        let _ = std::fs::remove_dir_all(&tmp);

        let mut cache = BuildCache::new(tmp);
        cache.load_from_disk().unwrap();
        assert!(cache.is_empty());
    }
}
