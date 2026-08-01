//! [`ArtifactPathSerializer`] — the single serializer for hvigor-compatible
//! artifact path strings.
//!
//! This is the **only** place in the codebase that converts `PathBuf` or
//! `Path` values into slash-separated strings for artifact output. Any other
//! inline `.replace('\\', "/")` call is a violation of this contract.
//!
//! # Allowed call sites
//!
//! - `loader.json` path fields
//! - `pkgContextInfo.json` path fields
//! - Source map `sources` and `sourceRoot` fields
//! - `abc_handoff.json` path fields
//! - ZIP entry names in HAP/HSP/HAR archives
//! - Hvigor parity intermediate artifacts

use std::path::Path;

use thiserror::Error;

use crate::artifact::ArtifactPath;

/// Errors produced by artifact path serialization.
#[derive(Debug, Error)]
pub enum SerializerError {
    /// The path contains non-UTF-8 bytes.
    #[error("path contains non-UTF-8 bytes: {0}")]
    NonUtf8(String),
    /// A Windows device/verbatim path cannot be represented in hvigor output.
    #[error("Windows device path cannot be serialized for hvigor: {0}")]
    UnsupportedWindowsDevicePath(String),
}

/// The single serializer for hvigor-compatible artifact path strings.
///
/// Every other crate must use this serializer to convert filesystem paths
/// into slash-formatted artifact strings. Do not call `.replace('\\', "/")`.
pub struct ArtifactPathSerializer;

impl ArtifactPathSerializer {
    /// Convert a `Path` to a slash-separated [`ArtifactPath`].
    ///
    /// # Errors
    ///
    /// Returns [`SerializerError::NonUtf8`] if the path contains non-UTF-8
    /// bytes.
    pub fn path_to_slash(path: &Path) -> Result<ArtifactPath, SerializerError> {
        let s = path_to_slash_string(path)?;
        Ok(ArtifactPath::from_slash(s))
    }

    /// Convert a `Path` to a raw slash-separated `String`.
    ///
    /// # Errors
    ///
    /// Returns [`SerializerError::NonUtf8`] if the path contains non-UTF-8
    /// bytes.
    pub fn path_to_slash_string(path: &Path) -> Result<String, SerializerError> {
        path_to_slash_string(path)
    }

    /// Convert a relative path to a slash-separated string.
    ///
    /// This function does not error on non-UTF-8 — it uses lossy conversion for
    /// relative paths that are expected to be ASCII module names.
    pub fn rel_to_slash(path: &Path) -> String {
        path_to_slash_lossy(path)
    }

    /// Build a source-file key for `loader.json` and related artifacts.
    ///
    /// The key is the slash-normalized relative path from the project root.
    ///
    /// # Errors
    ///
    /// Returns an error if the path is not inside `project_root`, or if
    /// either path contains non-UTF-8 bytes.
    pub fn source_file_key(
        project_root: &Path,
        source_path: &Path,
    ) -> Result<ArtifactPath, SerializerError> {
        let rel = source_path.strip_prefix(project_root).map_err(|_| {
            SerializerError::NonUtf8(format!(
                "path {} is not inside project root {}",
                source_path.display(),
                project_root.display()
            ))
        })?;
        Self::path_to_slash(rel)
    }
}

/// Convert a `Path` to a slash-separated `String`, with a typed error for
/// non-UTF-8 bytes.
fn path_to_slash_string(path: &Path) -> Result<String, SerializerError> {
    // On Unix the path is already a valid UTF-8 string if the underlying
    // OsStr is valid UTF-8. On Windows we iterate components.
    let s = path
        .to_str()
        .ok_or_else(|| SerializerError::NonUtf8(path.to_string_lossy().into_owned()))?
        .replace('\\', "/");
    normalize_windows_namespace(s)
}

/// Normalize the one Windows namespace form that hvigor represents without a
/// namespace prefix (UNC), and reject device paths that SDK tools cannot
/// consume. `AbsPath::canonicalize` already removes safe drive prefixes.
fn normalize_windows_namespace(path: String) -> Result<String, SerializerError> {
    const VERBATIM_UNC: &str = "//?/UNC/";
    if path
        .get(..VERBATIM_UNC.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(VERBATIM_UNC))
    {
        return Ok(format!("//{}", &path[VERBATIM_UNC.len()..]));
    }
    if path.starts_with("//?/") || path.starts_with("//./") {
        return Err(SerializerError::UnsupportedWindowsDevicePath(path));
    }
    Ok(path)
}

/// Lossy conversion: produces `/`-separated string, replacing non-UTF-8 with
/// the Unicode replacement character.
fn path_to_slash_lossy(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_path_to_slash_unix() {
        let p = Path::new("ets/pages/Index.ets");
        let result = ArtifactPathSerializer::path_to_slash(p).unwrap();
        assert_eq!(result.as_str(), "ets/pages/Index.ets");
    }

    #[test]
    fn test_path_to_slash_string_unix() {
        let p = Path::new("ets/modules.abc");
        let result = ArtifactPathSerializer::path_to_slash_string(p).unwrap();
        assert_eq!(result, "ets/modules.abc");
    }

    #[test]
    fn test_rel_to_slash_no_backslash() {
        let p = Path::new("ets/pages/Index.ets");
        let s = ArtifactPathSerializer::rel_to_slash(p);
        assert!(!s.contains('\\'));
        assert_eq!(s, "ets/pages/Index.ets");
    }

    #[test]
    fn test_source_file_key() {
        let root = Path::new("/project");
        let source = Path::new("/project/entry/src/main/ets/pages/Index.ets");
        let key = ArtifactPathSerializer::source_file_key(root, source).unwrap();
        assert_eq!(key.as_str(), "entry/src/main/ets/pages/Index.ets");
    }

    #[test]
    fn test_source_file_key_not_under_root() {
        let root = Path::new("/project");
        let source = Path::new("/other/src/main/ets/Index.ets");
        let err = ArtifactPathSerializer::source_file_key(root, source);
        assert!(err.is_err());
    }

    #[test]
    fn test_output_never_contains_backslash() {
        // This test must pass on every platform — the serializer must always
        // emit forward slashes.
        let p = Path::new("resources/base/media/icon.png");
        let result = ArtifactPathSerializer::path_to_slash_string(p).unwrap();
        assert!(!result.contains('\\'), "artifact path must not contain \\");
    }

    #[test]
    fn test_verbatim_unc_matches_hvigor_unc_spelling_on_every_host() {
        let result = ArtifactPathSerializer::path_to_slash_string(Path::new(
            r"\\?\UNC\server\share\project\entry",
        ))
        .unwrap();
        assert_eq!(result, "//server/share/project/entry");
    }

    #[test]
    fn test_verbatim_drive_path_is_rejected_instead_of_leaking_into_artifacts() {
        let error =
            ArtifactPathSerializer::path_to_slash_string(Path::new(r"\\?\C:\project\entry"))
                .unwrap_err();
        assert!(matches!(
            error,
            SerializerError::UnsupportedWindowsDevicePath(_)
        ));
    }

    #[test]
    fn test_regular_unc_path_is_preserved() {
        let result = ArtifactPathSerializer::path_to_slash_string(Path::new(
            r"\\server\share\project\entry",
        ))
        .unwrap();
        assert_eq!(result, "//server/share/project/entry");
    }

    // On Windows, verify that native path separators are converted.
    #[cfg(windows)]
    #[test]
    fn test_path_to_slash_windows_converts_separator() {
        use std::path::PathBuf;
        let p = PathBuf::from(r"ets\pages\Index.ets");
        let result = ArtifactPathSerializer::path_to_slash(&p).unwrap();
        assert_eq!(result.as_str(), "ets/pages/Index.ets");
        assert!(!result.as_str().contains('\\'));
    }
}
