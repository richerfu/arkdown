//! Cache error types.
//!
//! Corresponds to cache service errors in hvigor's rollup cache and task cache
//! systems.

// ---------------------------------------------------------------------------
// CacheError
// ---------------------------------------------------------------------------

/// Errors produced by cache operations.
///
/// # Upstream correspondence
///
/// Corresponds to hvigor `invalid-cache-plugin.js` error events and
/// `rollup-cache-service.js` exceptions.
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
    /// A cache entry was found but is no longer valid (key mismatch).
    ///
    /// The caller should re-run the stage and write a new entry.
    #[error("stale cache entry for key: {key}")]
    Stale { key: String },
}

impl CacheError {
    /// Create an I/O error variant from an underlying error.
    pub fn io(source: std::io::Error) -> Self {
        Self::Io(source)
    }

    /// Create a decode error variant.
    pub fn decode(source: serde_json::Error) -> Self {
        Self::Decode(source)
    }

    /// Create a stale-entry error for `key`.
    pub fn stale(key: impl Into<String>) -> Self {
        Self::Stale { key: key.into() }
    }
}

// ---------------------------------------------------------------------------
// CacheDiagnostic
// ---------------------------------------------------------------------------

/// Structured diagnostic from a cache operation.
///
/// Unlike [`CacheError`], a [`CacheDiagnostic`] is non-fatal: it records an
/// anomalous condition that should be logged but should not abort the build.
#[derive(Debug, Clone)]
pub struct CacheDiagnostic {
    /// Human-readable message describing the anomaly.
    pub message: String,
    /// Optionally, the cache key that triggered this diagnostic.
    pub key: Option<String>,
    /// Optionally, the cache directory path where the anomaly was detected.
    pub cache_dir: Option<std::path::PathBuf>,
}

impl CacheDiagnostic {
    /// Create a new diagnostic with a message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            key: None,
            cache_dir: None,
        }
    }

    /// Attach a cache key to the diagnostic.
    #[must_use]
    pub fn with_key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }

    /// Attach a cache directory path.
    #[must_use]
    pub fn with_cache_dir(mut self, dir: impl Into<std::path::PathBuf>) -> Self {
        self.cache_dir = Some(dir.into());
        self
    }
}

impl std::fmt::Display for CacheDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[cache] {}", self.message)?;
        if let Some(key) = &self.key {
            write!(f, " (key: {key})")?;
        }
        if let Some(dir) = &self.cache_dir {
            write!(f, " (dir: {})", dir.display())?;
        }
        Ok(())
    }
}
