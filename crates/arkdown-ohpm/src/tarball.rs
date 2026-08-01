//! Safe `.tgz` extraction into the OHPM cache.
//!
//! # Safety guarantees
//!
//! - Strips the top-level `package/` directory component from all entries.
//! - Rejects entries with absolute paths.
//! - Rejects entries whose resolved destination would escape the target directory.
//! - Creates parent directories as needed.

use std::path::Path;

use arkdown_path::{lexical_normalize, ArtifactPathSerializer, RelPath};
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
    let extraction_root = lexical_normalize(dest_dir);

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

        // Validate the archive-owned path before stripping its conventional
        // top-level directory. On Windows, `Path::is_absolute()` alone does
        // not reject root-relative (`\foo`) or drive-relative (`C:foo`)
        // entries; RelPath rejects roots, prefixes, and every traversal that
        // could rise above the extraction base.
        let archive_path = RelPath::parse(path.clone()).map_err(|_| {
            OhpmError::UnsafeTarballEntry(UnsafeTarballEntryText::new(&path).string())
        })?;
        if archive_path
            .components()
            .any(|component| component == std::path::Component::ParentDir)
        {
            return Err(OhpmError::UnsafeTarballEntry(
                UnsafeTarballEntryText::new(archive_path.as_path()).string(),
            ));
        }

        // Strip the top-level `package/` component (npm/OHPM convention).
        let stripped = strip_top_component(archive_path.as_path());

        // Skip the top-level directory entry itself.
        if stripped.as_os_str().is_empty() {
            continue;
        }

        let stripped = RelPath::parse(stripped)
            .map_err(|error| OhpmError::UnsafeTarballEntry(error.to_string()))?;

        let dest = extraction_root.join(stripped.as_path());

        // Verify destination stays inside dest_dir after normalization.
        // Use lexical normalization (no canonicalize — dest may not exist yet).
        let normalized_dest = lexical_normalize(&dest);
        if !normalized_dest.starts_with(&extraction_root) {
            return Err(OhpmError::UnsafeTarballEntry(
                UnsafeTarballEntryText::new(stripped.as_path()).string(),
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
}
