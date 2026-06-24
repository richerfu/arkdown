//! Safe `.tgz` extraction into the OHPM cache.
//!
//! # Safety guarantees
//!
//! - Strips the top-level `package/` directory component from all entries.
//! - Rejects entries with absolute paths.
//! - Rejects entries whose resolved destination would escape the target directory.
//! - Creates parent directories as needed.

use std::path::Path;

use arkdown_path::ArtifactPathSerializer;
use flate2::read::GzDecoder;
use tar::Archive;

use crate::error::OhpmError;

/// Extract a `.tgz` tarball into `dest_dir`.
///
/// The top-level `package/` directory component is stripped from every
/// entry path, matching the npm/OHPM tarball convention. Absolute paths
/// and `..`-traversal entries are rejected.
///
/// # Errors
///
/// Returns an [`OhpmError`] if:
/// - I/O fails during extraction.
/// - An entry contains an absolute path or `..` traversal.
pub fn extract_tgz(tarball_data: &[u8], dest_dir: &Path) -> Result<(), OhpmError> {
    let gz = GzDecoder::new(tarball_data);
    let mut archive = Archive::new(gz);

    std::fs::create_dir_all(dest_dir).map_err(|e| OhpmError::io(dest_dir, e))?;

    for entry in archive
        .entries()
        .map_err(|e| OhpmError::TarballExtract("archive".into(), e.to_string()))?
    {
        let mut entry =
            entry.map_err(|e| OhpmError::TarballExtract("entry".into(), e.to_string()))?;

        let path = entry
            .path()
            .map_err(|e| OhpmError::TarballExtract("entry path".into(), e.to_string()))?
            .into_owned();

        // Strip the top-level `package/` component (npm/OHPM convention).
        let stripped = strip_top_component(&path);

        // Skip the top-level directory entry itself.
        if stripped.as_os_str().is_empty() {
            continue;
        }

        // Reject absolute paths.
        if stripped.is_absolute() {
            return Err(OhpmError::UnsafeTarballEntry(
                UnsafeTarballEntryText::new(&stripped).string(),
            ));
        }

        // Reject path traversal.
        if stripped.components().any(|c| c.as_os_str() == "..") {
            return Err(OhpmError::UnsafeTarballEntry(
                UnsafeTarballEntryText::new(&stripped).string(),
            ));
        }

        let dest = dest_dir.join(&stripped);

        // Verify destination stays inside dest_dir after normalization.
        // Use lexical normalization (no canonicalize — dest may not exist yet).
        let normalized_dest = crate_lexical_normalize(&dest);
        if !normalized_dest.starts_with(dest_dir) {
            return Err(OhpmError::UnsafeTarballEntry(
                UnsafeTarballEntryText::new(&stripped).string(),
            ));
        }

        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| OhpmError::io(parent, e))?;
        }

        let mut file = std::fs::File::create(&dest).map_err(|e| OhpmError::io(&dest, e))?;
        std::io::copy(&mut entry, &mut file).map_err(|e| OhpmError::io(&dest, e))?;
    }

    Ok(())
}

struct UnsafeTarballEntryText<'a> {
    path: &'a Path,
}

impl<'a> UnsafeTarballEntryText<'a> {
    fn new(path: &'a Path) -> Self {
        Self { path }
    }

    fn string(self) -> String {
        ArtifactPathSerializer::path_to_slash_string(self.path)
            .unwrap_or_else(|_| self.path.display().to_string())
    }
}

/// Strip the first path component from `path`.
///
/// `"package/src/index.ets"` → `"src/index.ets"`.
/// A path with only one component returns an empty path.
fn strip_top_component(path: &Path) -> std::path::PathBuf {
    let mut components = path.components();
    components.next(); // skip first component
    components.as_path().to_path_buf()
}

/// Lexically normalize a path without touching the filesystem.
fn crate_lexical_normalize(path: &Path) -> std::path::PathBuf {
    use std::path::Component;
    let mut result = std::path::PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            c => result.push(c),
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_strip_top_component_package_prefix() {
        let path = Path::new("package/src/index.ets");
        let stripped = strip_top_component(path);
        assert_eq!(stripped, Path::new("src/index.ets"));
    }

    #[test]
    fn test_strip_top_component_single_component() {
        let path = Path::new("package");
        let stripped = strip_top_component(path);
        assert!(stripped.as_os_str().is_empty());
    }

    #[test]
    fn test_strip_top_component_nested() {
        let path = Path::new("package/a/b/c.ts");
        let stripped = strip_top_component(path);
        assert_eq!(stripped, Path::new("a/b/c.ts"));
    }

    #[test]
    fn test_lexical_normalize_removes_dotdot() {
        let path = Path::new("/dest/a/../b");
        let norm = crate_lexical_normalize(path);
        assert_eq!(norm, Path::new("/dest/b"));
    }
}
