//! Cached artifact types.
//!
//! A [`CachedArtifact`] represents a build output (transformed source,
//! compiled bytecode, resource index, etc.) that has been stored in the
//! incremental cache.  Subsequent builds restore artifacts from cache instead
//! of recomputing them.
//!
//! # Upstream correspondence
//!
//! Corresponds to hvigor's task-cache entry artifacts written to
//! `build/default/cache/<task>/<module>/`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::key::CacheKeyMaterial;

// ---------------------------------------------------------------------------
// CachedArtifactKind
// ---------------------------------------------------------------------------

/// The kind of cached artifact.
///
/// # Upstream correspondence
///
/// Corresponds to the artifact types tracked by hvigor's task cache
/// service.  Each kind maps to a file extension or directory convention
/// under the cache directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CachedArtifactKind {
    /// Transformed ETS/TS source (JavaScript output of OXC ETS transform).
    TransformedSource,
    /// ABC Panda bytecode (`.abc` file).
    PandaBytecode,
    /// Program proto binary (`.protoBin` file for ProgramCache).
    ProgramProto,
    /// Compiled resource index (`resources.index`).
    ResourceIndex,
    /// Processed `module.json`.
    ModuleJson,
    /// Processed `loader.json`.
    LoaderJson,
    /// Source map JSON.
    SourceMap,
    /// HAR declaration file (`.d.ets`/`.d.ts`).
    Declaration,
}

impl CachedArtifactKind {
    /// Return the file extension used for this artifact kind.
    pub fn file_extension(&self) -> &'static str {
        match self {
            CachedArtifactKind::TransformedSource => "js",
            CachedArtifactKind::PandaBytecode => "abc",
            CachedArtifactKind::ProgramProto => "protoBin",
            CachedArtifactKind::ResourceIndex => "index",
            CachedArtifactKind::ModuleJson | CachedArtifactKind::LoaderJson => "json",
            CachedArtifactKind::SourceMap => "map",
            CachedArtifactKind::Declaration => "d.ets",
        }
    }
}

// ---------------------------------------------------------------------------
// CachedArtifactFingerprint
// ---------------------------------------------------------------------------

/// Content fingerprint of a cached artifact.
///
/// Used to verify that a cached artifact on disk has not been modified since
/// it was written.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CachedArtifactFingerprint {
    /// FNV-1a hex digest of the artifact bytes.
    pub hash: String,
    /// File size in bytes.
    pub size: u64,
}

impl CachedArtifactFingerprint {
    /// Compute a fingerprint from raw bytes.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            hash: CacheKeyMaterial::compute_content_hash_bytes(bytes),
            size: bytes.len() as u64,
        }
    }
}

// ---------------------------------------------------------------------------
// CachedArtifact
// ---------------------------------------------------------------------------

/// A build artifact stored in the incremental cache.
///
/// `CachedArtifact` records the kind, path, and fingerprint of an artifact so
/// that subsequent builds can verify the artifact is still valid and restore
/// it to the output directory.
///
/// # Upstream correspondence
///
/// Corresponds to entries written to hvigor's task cache by
/// `rollup-cache-service.js` and the ABC ProgramCache.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedArtifact {
    /// The kind of artifact.
    pub kind: CachedArtifactKind,
    /// Absolute path to the cached artifact file on disk.
    pub cache_path: PathBuf,
    /// Content fingerprint for integrity verification.
    pub fingerprint: CachedArtifactFingerprint,
    /// The original source module ID (OHM URL or absolute path) that produced
    /// this artifact.
    pub source_id: String,
}

impl CachedArtifact {
    /// Create a [`CachedArtifact`] from a stage output file.
    ///
    /// Reads the file at `stage_output_path`, computes its fingerprint, and
    /// returns a record that can be stored in the cache manifest.
    pub fn from_stage_output(
        kind: CachedArtifactKind,
        source_id: impl Into<String>,
        stage_output_path: &Path,
    ) -> Result<Self, std::io::Error> {
        let bytes = std::fs::read(stage_output_path)?;
        let fingerprint = CachedArtifactFingerprint::from_bytes(&bytes);
        Ok(Self {
            kind,
            cache_path: stage_output_path.to_owned(),
            fingerprint,
            source_id: source_id.into(),
        })
    }

    /// Verify that the artifact at [`Self::cache_path`] matches the stored
    /// fingerprint.
    ///
    /// Returns `true` if the artifact is still valid (same hash and size).
    pub fn verify(&self) -> bool {
        match std::fs::read(&self.cache_path) {
            Ok(bytes) => {
                let current = CachedArtifactFingerprint::from_bytes(&bytes);
                current == self.fingerprint
            }
            Err(_) => false,
        }
    }

    /// Copy the cached artifact to `dest_path`.
    ///
    /// Returns an error if the source file is missing or the copy fails.
    pub fn restore(&self, dest_path: &Path) -> Result<(), std::io::Error> {
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&self.cache_path, dest_path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_from_bytes_is_deterministic() {
        let fp1 = CachedArtifactFingerprint::from_bytes(b"test content");
        let fp2 = CachedArtifactFingerprint::from_bytes(b"test content");
        assert_eq!(fp1, fp2);
        assert_eq!(fp1.size, 12);
    }

    #[test]
    fn fingerprint_differs_for_different_content() {
        let fp1 = CachedArtifactFingerprint::from_bytes(b"abc");
        let fp2 = CachedArtifactFingerprint::from_bytes(b"def");
        assert_ne!(fp1, fp2);
    }

    #[test]
    fn artifact_kind_extension() {
        assert_eq!(CachedArtifactKind::PandaBytecode.file_extension(), "abc");
        assert_eq!(CachedArtifactKind::TransformedSource.file_extension(), "js");
    }
}
