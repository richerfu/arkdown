//! Incremental build cache for the ABC compiler (SPEC-025).
//!
//! The cache is stored as a JSON file at `cache_path/modules.abc.cache`.
//! Each entry maps a SHA-256 hex key (derived from all module sources and
//! compile options) to the hex-encoded compiled `.abc` bytecode.
//!
//! # Format
//!
//! ```json
//! {
//!   "entries": [
//!     { "key": "<sha256hex>", "bytecode": "<hexencoded>" }
//!   ]
//! }
//! ```
//!
//! # Key derivation
//!
//! The cache key for a compilation run is:
//! ```text
//! SHA-256( record_name_0 || "\0" || source_0 || "\0"
//!        || record_name_1 || "\0" || source_1 || "\0"
//!        || ...
//!        || "module=" || (0 | 1)
//!        || "debug="  || (0 | 1) )
//! ```
//!
//! Any change to a source file, record name, or compile option invalidates the
//! cache entry and triggers a full recompile.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A single cache entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    /// SHA-256 hex digest of the compilation inputs.
    pub key: String,
    /// Hex-encoded compiled `.abc` bytecode.
    pub bytecode: String,
}

/// The on-disk cache file contents.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CacheFile {
    pub entries: Vec<CacheEntry>,
}

impl CacheFile {
    /// Resolve the path to the cache file given a cache directory.
    pub fn path_for(cache_dir: &Path) -> PathBuf {
        cache_dir.join("modules.abc.cache")
    }

    /// Load a `CacheFile` from disk.  Returns an empty `CacheFile` if the file
    /// does not exist or cannot be parsed (e.g. after a format change).
    pub fn load(cache_dir: &Path) -> Self {
        let path = Self::path_for(cache_dir);
        let data = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => return CacheFile::default(),
        };
        serde_json::from_str(&data).unwrap_or_default()
    }

    /// Persist the cache file to disk, creating parent directories as needed.
    pub fn save(&self, cache_dir: &Path) -> io::Result<()> {
        let path = Self::path_for(cache_dir);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        std::fs::write(&path, json)
    }

    /// Look up `key` in the cache.  Returns `Some(bytecode)` on hit.
    pub fn lookup(&self, key: &str) -> Option<Vec<u8>> {
        self.entries
            .iter()
            .find(|e| e.key == key)
            .and_then(|e| hex::decode(&e.bytecode).ok())
    }

    /// Insert or update an entry.
    pub fn upsert(&mut self, key: String, bytecode: &[u8]) {
        let hex_bytes = hex::encode(bytecode);
        if let Some(entry) = self.entries.iter_mut().find(|e| e.key == key) {
            entry.bytecode = hex_bytes;
        } else {
            self.entries.push(CacheEntry {
                key,
                bytecode: hex_bytes,
            });
        }
    }
}

/// Derive a SHA-256 cache key from a slice of `(record_name, source)` pairs
/// and compile options.
///
/// The key covers:
/// - All record names and source texts (in order)
/// - `module` flag
/// - `debug_info` flag
pub fn derive_key(modules: &[(&str, &str)], module_mode: bool, debug_info: bool) -> String {
    let mut hasher = Sha256::new();
    for (record_name, source) in modules {
        hasher.update(record_name.as_bytes());
        hasher.update(b"\x00");
        hasher.update(source.as_bytes());
        hasher.update(b"\x00");
    }
    hasher.update(if module_mode {
        b"module=1"
    } else {
        b"module=0"
    });
    hasher.update(if debug_info { b"debug=1" } else { b"debug=0" });
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_key_deterministic() {
        let modules = vec![("entry/main", "export const x = 1;")];
        let k1 = derive_key(&modules, true, false);
        let k2 = derive_key(&modules, true, false);
        assert_eq!(k1, k2, "key must be deterministic");
    }

    #[test]
    fn test_derive_key_changes_on_source_change() {
        let m1 = vec![("entry/main", "export const x = 1;")];
        let m2 = vec![("entry/main", "export const x = 2;")];
        let k1 = derive_key(&m1, true, false);
        let k2 = derive_key(&m2, true, false);
        assert_ne!(k1, k2, "key must change when source changes");
    }

    #[test]
    fn test_cache_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache = CacheFile::default();
        let bytecode = b"PANDA\x00\x00\x00fake_bytecode";
        cache.upsert("abc123".to_string(), bytecode);
        cache.save(dir.path()).unwrap();

        let loaded = CacheFile::load(dir.path());
        let hit = loaded.lookup("abc123").unwrap();
        assert_eq!(hit, bytecode);
    }

    #[test]
    fn test_cache_miss_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let cache = CacheFile::load(dir.path()); // empty
        assert!(cache.lookup("nonexistent").is_none());
    }
}
