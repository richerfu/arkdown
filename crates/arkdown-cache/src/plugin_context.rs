//! Persistent JavaScript plugin-context cache.
//!
//! Hvigor keeps Rollup's `share.cache`, `share.commonCache`,
//! `cacheStoreManager`, and per-plugin `this.cache` warm across build
//! invocations. JavaScript owns the value semantics because entries may be any
//! V8 structured-clone value; Rust owns all filesystem access and publishes a
//! completed snapshot atomically outside plugin hooks.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::CacheError;

const CACHE_FILE_NAME: &str = "plugin-context-v1.bin";
const CACHE_MAGIC: &[u8] = b"ARKDOWN_PLUGIN_CONTEXT\0\x01";
const MAX_CACHE_BYTES: u64 = 128 * 1024 * 1024;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Rust-owned store for one module target's serialized V8 plugin state.
#[derive(Debug, Clone)]
pub struct PluginContextCacheStore {
    path: PathBuf,
}

impl PluginContextCacheStore {
    pub fn new(cache_dir: &Path) -> Self {
        Self {
            path: cache_dir.join(CACHE_FILE_NAME),
        }
    }

    /// Load the opaque V8 payload. Unknown, truncated, or oversized snapshots
    /// are treated as stale cache entries instead of failing the build.
    pub fn load(&self) -> Result<Option<Vec<u8>>, CacheError> {
        let metadata = match fs_err::metadata(&self.path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(CacheError::Io(error)),
        };
        if metadata.len() < CACHE_MAGIC.len() as u64
            || metadata.len() > MAX_CACHE_BYTES + CACHE_MAGIC.len() as u64
        {
            return Ok(None);
        }
        let bytes = fs_err::read(&self.path).map_err(CacheError::Io)?;
        let Some(payload) = bytes.strip_prefix(CACHE_MAGIC) else {
            return Ok(None);
        };
        Ok(Some(payload.to_vec()))
    }

    /// Atomically publish a completed JavaScript cache snapshot.
    pub fn commit(&self, payload: &[u8]) -> Result<(), CacheError> {
        if payload.len() as u64 > MAX_CACHE_BYTES {
            return Err(CacheError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "plugin context cache exceeds 128 MiB",
            )));
        }
        let parent = self.path.parent().ok_or_else(|| {
            CacheError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "plugin context cache path has no parent",
            ))
        })?;
        fs_err::create_dir_all(parent).map_err(CacheError::Io)?;
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temp_path = parent.join(format!(
            ".{CACHE_FILE_NAME}.{}.{}.tmp",
            std::process::id(),
            sequence
        ));
        let mut bytes = Vec::with_capacity(CACHE_MAGIC.len() + payload.len());
        bytes.extend_from_slice(CACHE_MAGIC);
        bytes.extend_from_slice(payload);
        fs_err::write(&temp_path, bytes).map_err(CacheError::Io)?;
        #[cfg(windows)]
        if self.path.exists() {
            fs_err::remove_file(&self.path).map_err(CacheError::Io)?;
        }
        fs_err::rename(&temp_path, &self.path).map_err(CacheError::Io)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opaque_snapshot_round_trips_with_versioned_envelope() {
        let temp = tempfile::tempdir().unwrap();
        let store = PluginContextCacheStore::new(temp.path());

        assert_eq!(store.load().unwrap(), None);
        store.commit(b"v8 structured clone").unwrap();
        assert_eq!(
            store.load().unwrap().as_deref(),
            Some(&b"v8 structured clone"[..])
        );
        assert!(store.path().is_file());
    }

    #[test]
    fn unknown_envelope_is_a_cache_miss() {
        let temp = tempfile::tempdir().unwrap();
        let store = PluginContextCacheStore::new(temp.path());
        fs_err::write(store.path(), b"old cache schema").unwrap();

        assert_eq!(store.load().unwrap(), None);
    }
}
