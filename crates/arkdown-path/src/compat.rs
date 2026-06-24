//! Compatibility helpers that replace `arkdown-utils` path functions.
//!
//! These functions are migration targets from the deleted `arkdown-utils` crate.
//! New code must use [`ArtifactPathSerializer`](super::ArtifactPathSerializer)
//! or [`AbsPath`](super::AbsPath) instead.

use std::path::{Path, PathBuf};

use crate::ArtifactPathSerializer;

/// Convert a path to a forward-slash string.
///
/// Replaces `arkdown_utils::to_unix_path`. Callers should migrate to
/// [`ArtifactPathSerializer::path_to_slash_string`] for artifact outputs or
/// [`ArtifactPathSerializer::rel_to_slash`] for relative paths.
///
/// # Deprecated usage
///
/// Only for callers that cannot yet use typed artifact paths. Prefer the
/// typed serializer API.
pub fn to_slash(path: &Path) -> String {
    ArtifactPathSerializer::rel_to_slash(path)
}

/// Canonicalize a path, falling back to the original if canonicalization fails.
///
/// Replaces `arkdown_utils::canonicalize_or_owned`. Input paths that exist on
/// the filesystem are canonicalized; non-existent output paths are returned
/// unchanged. New code should use [`AbsPath::canonicalize`] for inputs and
/// [`AbsPath::from_absolute`] for outputs.
pub fn canonicalize_or_owned(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
