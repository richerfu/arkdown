//! Maps module/profile references to absolute input files and fingerprints.

use std::path::PathBuf;

/// A profile file reference with fingerprint.
#[derive(Debug, Clone)]
pub struct ProfileFileRef {
    /// Absolute path to the profile file.
    pub path: PathBuf,
    /// Content hash (SHA-256) for cache invalidation.
    pub fingerprint: Option<[u8; 32]>,
}
