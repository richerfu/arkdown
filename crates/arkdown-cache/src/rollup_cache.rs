//! Rollup/Rolldown bundle cache store.
//!
//! `RollupCacheStore` manages the bundle-level cache used between Rolldown
//! runs.  On a cache hit, the entire bundle step (module loading, linking,
//! chunk assignment) is skipped.  The cache is keyed on the `BundleCacheKey`
//! which combines the compile environment with the module graph fingerprints.
//!
//! # Upstream correspondence
//!
//! | Type | hvigor equivalent |
//! |------|-------------------|
//! | `RollupCacheStore` | `rollup-cache-service.js` |
//! | `RollupCacheEntry` | per-build rollup cache object stored on disk |
//! | `RollupCacheReadResult` | `build.js::readCacheWithEvent` return |
//! | `RollupCacheWritePlan` | plan for writing cache at build end |

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::CacheError;
use crate::event_name::CacheEventName;
use crate::key::BundleCacheKey;

// ---------------------------------------------------------------------------
// RollupCacheEntry
// ---------------------------------------------------------------------------

/// The cached rollup/rolldown bundle artifact.
///
/// Stores the output of a successful Rolldown bundle run so subsequent builds
/// can restore the bundle output without re-running the bundler.
///
/// # Upstream correspondence
///
/// Corresponds to the cache object written by `rollup-cache-service.js` after
/// each successful Rollup/Rolldown bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollupCacheEntry {
    /// The cache key for this entry.
    pub key: BundleCacheKey,
    /// Module IDs included in this bundle.
    pub module_ids: Vec<String>,
    /// Event names from the previous build (for watch-mode invalidation).
    pub event_names: Vec<String>,
}

// ---------------------------------------------------------------------------
// RollupCacheReadResult
// ---------------------------------------------------------------------------

/// Result of reading the rollup cache with watch-mode event checking.
///
/// # Upstream correspondence
///
/// Corresponds to `build.js::readCacheWithEvent`, which reads the cache file
/// and checks whether event names have changed since the last build.
#[derive(Debug, Clone)]
pub enum RollupCacheReadResult {
    /// Cache hit — the stored key matches the current key.
    Hit(RollupCacheEntry),
    /// Cache miss — either no cache file, or key mismatch.
    Miss {
        /// Human-readable reason for the cache miss.
        reason: String,
    },
    /// Cache invalidated by a changed event name.
    InvalidatedByEvent {
        /// The event name that triggered invalidation.
        event_name: String,
    },
}

// ---------------------------------------------------------------------------
// RollupCacheWritePlan
// ---------------------------------------------------------------------------

/// Plan for what to write to the rollup cache after a successful build.
///
/// # Upstream correspondence
///
/// Corresponds to `rollup-cache-service.js` write operations after
/// `buildEnd`/`closeBundle` hooks.
#[derive(Debug, Clone)]
pub struct RollupCacheWritePlan {
    /// The cache key to write.
    pub key: BundleCacheKey,
    /// Module IDs to record.
    pub module_ids: Vec<String>,
    /// Current event names (for next build's watch-mode check).
    pub event_names: Vec<String>,
}

// ---------------------------------------------------------------------------
// RollupCacheStore
// ---------------------------------------------------------------------------

/// Manages the Rolldown bundle cache on disk.
///
/// # Upstream correspondence
///
/// Corresponds to `rollup-cache-service.js`, which provides `readCache`,
/// `writeCache`, `removeCache`, and `getCacheDirectory` methods.
pub struct RollupCacheStore {
    cache_dir: PathBuf,
}

impl RollupCacheStore {
    /// File name for the rollup cache entry.
    const ENTRY_FILE: &'static str = "bundle_cache.json";

    /// Open the rollup cache store at `cache_dir`.
    ///
    /// The directory is created lazily when the first entry is written.
    pub fn open(cache_dir: impl Into<PathBuf>) -> Self {
        Self {
            cache_dir: cache_dir.into(),
        }
    }

    /// Read the cached bundle entry and check event names.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `build.js::readCacheWithEvent`.
    pub fn read_cache_with_event(
        &self,
        current_event_name: Option<&CacheEventName>,
    ) -> RollupCacheReadResult {
        let path = self.cache_dir.join(Self::ENTRY_FILE);
        if !path.exists() {
            return RollupCacheReadResult::Miss {
                reason: "no cache file".to_string(),
            };
        }
        let content = match fs_err::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                return RollupCacheReadResult::Miss {
                    reason: format!("read error: {e}"),
                }
            }
        };
        let entry: RollupCacheEntry = match serde_json::from_str(&content) {
            Ok(e) => e,
            Err(_) => {
                return RollupCacheReadResult::Miss {
                    reason: "decode error".to_string(),
                }
            }
        };
        // Check if any of the stored event names have changed.
        if let Some(event_name) = current_event_name {
            for stored_name in &entry.event_names {
                if event_name.value != *stored_name {
                    return RollupCacheReadResult::InvalidatedByEvent {
                        event_name: stored_name.clone(),
                    };
                }
            }
        }
        RollupCacheReadResult::Hit(entry)
    }

    /// Write a cache entry to disk.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `rollup-cache-service.js::writeCache`.
    pub fn write_cache(&self, plan: &RollupCacheWritePlan) -> Result<(), CacheError> {
        fs_err::create_dir_all(&self.cache_dir).map_err(CacheError::Io)?;
        let entry = RollupCacheEntry {
            key: plan.key.clone(),
            module_ids: plan.module_ids.clone(),
            event_names: plan.event_names.clone(),
        };
        let json = serde_json::to_string_pretty(&entry)?;
        fs_err::write(self.cache_dir.join(Self::ENTRY_FILE), json).map_err(CacheError::Io)?;
        Ok(())
    }

    /// Delete the cache entry file.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `rollup-cache-service.js::removeCache`.
    pub fn remove_cache(&self) -> Result<(), CacheError> {
        let path = self.cache_dir.join(Self::ENTRY_FILE);
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }

    /// Return the cache directory path.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `rollup-cache-service.js::getCacheDirectory`.
    pub fn cache_directory(&self) -> &Path {
        &self.cache_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_miss_on_nonexistent_dir() {
        let store = RollupCacheStore::open("/tmp/nonexistent_rollup_cache_test_dir_xyz");
        let result = store.read_cache_with_event(None);
        assert!(matches!(result, RollupCacheReadResult::Miss { .. }));
    }
}
