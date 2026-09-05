//! Cache error types.
//!
//! Errors cross the cache-domain boundary; build owners decide whether a cache
//! failure is fatal or should degrade to a miss.

// ---------------------------------------------------------------------------
// CacheError
// ---------------------------------------------------------------------------

/// Errors produced by cache operations.
///
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    /// I/O error reading or writing a cache file.
    #[error("cache I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// Serialization error decoding a cache entry.
    ///
    /// Typically indicates a stale or corrupt cache file from a prior build.
    #[error("cache decode error: {0}")]
    Decode(#[from] serde_json::Error),
}
