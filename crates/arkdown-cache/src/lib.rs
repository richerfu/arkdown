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

    // ── get() and len() ─────────────────────────────────────────────────

    #[test]
    fn test_get_returns_inserted_entry() {
        let tmp = std::env::temp_dir().join("arkdown_cache_get_test");
        let _ = std::fs::remove_dir_all(&tmp);

        let mut cache = BuildCache::new(tmp.clone());
        let entry = make_entry("src/Index.ets", "hash_get");
        cache.insert(entry.clone()).unwrap();

        let found = cache.get("src/Index.ets");
        assert!(found.is_some(), "get must return the inserted entry");
        assert_eq!(found.unwrap().key.content_hash, "hash_get");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_get_returns_none_for_unknown_id() {
        let tmp = std::env::temp_dir().join("arkdown_cache_get_none_test");
        let _ = std::fs::remove_dir_all(&tmp);

        let mut cache = BuildCache::new(tmp.clone());
        cache.insert(make_entry("a.ts", "h")).unwrap();

        assert!(cache.get("b.ts").is_none(), "get must return None for an ID that was never inserted");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_len_tracks_insertions() {
        let tmp = std::env::temp_dir().join("arkdown_cache_len_test");
        let _ = std::fs::remove_dir_all(&tmp);

        let mut cache = BuildCache::new(tmp.clone());
        assert_eq!(cache.len(), 0);

        cache.insert(make_entry("a.ts", "h1")).unwrap();
        assert_eq!(cache.len(), 1);

        cache.insert(make_entry("b.ts", "h2")).unwrap();
        assert_eq!(cache.len(), 2);

        // Re-inserting the same id should update, not grow
        cache.insert(make_entry("a.ts", "h1_updated")).unwrap();
        assert_eq!(cache.len(), 2, "re-inserting same ID must not increase len");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // ── CacheKey equality semantics ──────────────────────────────────────

    #[test]
    fn test_cache_key_equality() {
        let k1 = CacheKey {
            content_hash: "abc".to_string(),
            config_hash: 42,
            dependency_hash: 7,
        };
        let k2 = CacheKey {
            content_hash: "abc".to_string(),
            config_hash: 42,
            dependency_hash: 7,
        };
        assert_eq!(k1, k2, "identical CacheKeys must be equal");
    }

    #[test]
    fn test_cache_key_inequality_on_dependency_hash() {
        let k1 = CacheKey { content_hash: "x".to_string(), config_hash: 0, dependency_hash: 1 };
        let k2 = CacheKey { content_hash: "x".to_string(), config_hash: 0, dependency_hash: 2 };
        assert_ne!(k1, k2, "differing dependency_hash must make keys unequal");
    }

    // ── compute_content_hash ─────────────────────────────────────────────

    #[test]
    fn test_content_hash_is_16_hex_chars() {
        let h = compute_content_hash("some source code");
        assert_eq!(h.len(), 16, "content hash must be a 16-char hex string (FNV-1a 64-bit)");
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()), "content hash must be hex digits only");
    }

    #[test]
    fn test_content_hash_empty_string() {
        let h = compute_content_hash("");
        assert_eq!(h.len(), 16, "content hash for empty string must still be 16 chars");
    }

    // ── Concurrency: two threads insert different keys ───────────────────

    #[test]
    fn test_concurrent_inserts_to_different_dirs_do_not_corrupt() {
        // Each thread gets its own cache directory so they never conflict.
        let base = std::env::temp_dir().join("arkdown_cache_concurrent_test");
        let _ = std::fs::remove_dir_all(&base);

        let dir_a = base.join("a");
        let dir_b = base.join("b");

        let t1 = std::thread::spawn({
            let dir = dir_a.clone();
            move || {
                let mut cache = BuildCache::new(dir);
                for i in 0..20u32 {
                    let id = format!("module_{i}.ts");
                    let hash = compute_content_hash(&format!("content_{i}"));
                    cache
                        .insert(CachedEntry {
                            id: id.clone(),
                            key: CacheKey { content_hash: hash, config_hash: 0, dependency_hash: 0 },
                            transformed_code: format!("// module {i}"),
                            source_map: None,
                        })
                        .expect("insert must not fail");
                }
                cache.len()
            }
        });

        let t2 = std::thread::spawn({
            let dir = dir_b.clone();
            move || {
                let mut cache = BuildCache::new(dir);
                for i in 0..20u32 {
                    let id = format!("util_{i}.ts");
                    let hash = compute_content_hash(&format!("util_content_{i}"));
                    cache
                        .insert(CachedEntry {
                            id: id.clone(),
                            key: CacheKey { content_hash: hash, config_hash: 1, dependency_hash: 0 },
                            transformed_code: format!("// util {i}"),
                            source_map: None,
                        })
                        .expect("insert must not fail");
                }
                cache.len()
            }
        });

        let len_a = t1.join().expect("thread A must not panic");
        let len_b = t2.join().expect("thread B must not panic");

        assert_eq!(len_a, 20, "thread A cache must have 20 entries");
        assert_eq!(len_b, 20, "thread B cache must have 20 entries");

        // Verify disk state for both caches.
        let mut cache_a = BuildCache::new(dir_a);
        cache_a.load_from_disk().expect("reload A");
        assert_eq!(cache_a.len(), 20, "reloaded cache A must have 20 entries");

        let mut cache_b = BuildCache::new(dir_b);
        cache_b.load_from_disk().expect("reload B");
        assert_eq!(cache_b.len(), 20, "reloaded cache B must have 20 entries");

        let _ = std::fs::remove_dir_all(&base);
    }
}
