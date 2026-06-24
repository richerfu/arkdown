//! [`ArtifactPath`] — a slash-separated UTF-8 artifact path.
//!
//! Used for:
//! - ZIP entry names in HAP/HSP/HAR archives.
//! - Paths inside `loader.json`, `pkgContextInfo.json`, source maps, and
//!   hvigor intermediate artifacts.
//!
//! # Invariants
//!
//! - Always uses `/` as the separator (never `\`).
//! - Valid UTF-8.
//! - Produced only by [`ArtifactPathSerializer`](super::ArtifactPathSerializer)
//!   or explicit construction.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A slash-separated, UTF-8 path for hvigor-compatible artifact outputs.
///
/// Do **not** use this type to represent real filesystem paths. Use
/// [`AbsPath`](super::AbsPath) or [`std::path::PathBuf`] for those.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ArtifactPath(String);

impl ArtifactPath {
    /// Construct from an already-slash-formatted string.
    ///
    /// # Panics (debug only)
    ///
    /// In debug builds, panics if `s` contains `\`.
    pub fn from_slash(s: impl Into<String>) -> Self {
        let s = s.into();
        debug_assert!(
            !s.contains('\\'),
            "ArtifactPath must not contain backslashes: {s}"
        );
        Self(s)
    }

    /// Borrow as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume into the inner `String`.
    pub fn into_string(self) -> String {
        self.0
    }

    /// Append a path segment with `/` as separator.
    pub fn join(&self, segment: &str) -> Self {
        debug_assert!(
            !segment.contains('\\'),
            "segment must not contain backslashes"
        );
        if self.0.is_empty() {
            Self(segment.to_owned())
        } else {
            let mut path = String::with_capacity(self.0.len() + 1 + segment.len());
            path.push_str(&self.0);
            path.push('/');
            path.push_str(segment);
            Self(path)
        }
    }

    /// Return the parent directory component, or `None` for a bare file name.
    pub fn parent(&self) -> Option<Self> {
        let s = self.0.rsplit_once('/')?.0;
        Some(Self(s.to_owned()))
    }

    /// Return the last component (file name) after the final `/`.
    pub fn file_name(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or(&self.0)
    }

    /// Return the file stem when the final component has the expected extension.
    ///
    /// If the extension does not match, returns the full final component.
    pub fn file_stem_with_extension(&self, extension: &str) -> &str {
        let file_name = self.file_name();
        let Some((stem, actual_extension)) = file_name.rsplit_once('.') else {
            return file_name;
        };
        if actual_extension == extension {
            stem
        } else {
            file_name
        }
    }

    /// Return this artifact path with the final component's extension set.
    pub fn with_extension(&self, extension: &str) -> Self {
        let file_start = self.0.rfind('/').map_or(0, |index| index + 1);
        let extension_start = self.0[file_start..]
            .rfind('.')
            .map(|index| file_start + index);
        let mut path = match extension_start {
            Some(index) => self.0[..index].to_owned(),
            None => self.0.clone(),
        };
        path.push('.');
        path.push_str(extension);
        Self(path)
    }
}

impl fmt::Display for ArtifactPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for ArtifactPath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<ArtifactPath> for String {
    fn from(p: ArtifactPath) -> Self {
        p.0
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_artifact_path_from_slash() {
        let p = ArtifactPath::from_slash("ets/modules.abc");
        assert_eq!(p.as_str(), "ets/modules.abc");
    }

    #[test]
    fn test_artifact_path_join() {
        let base = ArtifactPath::from_slash("ets");
        let joined = base.join("modules.abc");
        assert_eq!(joined.as_str(), "ets/modules.abc");
    }

    #[test]
    fn test_artifact_path_join_empty_base() {
        let base = ArtifactPath::from_slash("");
        let joined = base.join("file.json");
        assert_eq!(joined.as_str(), "file.json");
    }

    #[test]
    fn test_artifact_path_parent() {
        let p = ArtifactPath::from_slash("ets/pages/Index.js");
        let parent = p.parent().unwrap();
        assert_eq!(parent.as_str(), "ets/pages");
    }

    #[test]
    fn test_artifact_path_parent_top_level() {
        let p = ArtifactPath::from_slash("module.json");
        assert!(p.parent().is_none());
    }

    #[test]
    fn test_artifact_path_file_name() {
        let p = ArtifactPath::from_slash("ets/pages/Index.js");
        assert_eq!(p.file_name(), "Index.js");
    }

    #[test]
    fn test_artifact_path_file_stem_with_extension() {
        let p = ArtifactPath::from_slash("./chunks/utils.js");
        assert_eq!(p.file_stem_with_extension("js"), "utils");
        assert_eq!(p.file_stem_with_extension("ets"), "utils.js");
    }

    #[test]
    fn test_artifact_path_with_extension_sets_final_component() {
        let p = ArtifactPath::from_slash("entry/build/default/entry");
        assert_eq!(
            p.with_extension("hap").as_str(),
            "entry/build/default/entry.hap"
        );

        let p = ArtifactPath::from_slash("ets/modules.old");
        assert_eq!(p.with_extension("abc").as_str(), "ets/modules.abc");
    }

    #[test]
    fn test_artifact_path_display() {
        let p = ArtifactPath::from_slash("resources/base/media/icon.png");
        assert_eq!(p.to_string(), "resources/base/media/icon.png");
    }

    #[test]
    fn test_artifact_path_no_backslash_in_output() {
        // ArtifactPath must only contain forward slashes.
        let p = ArtifactPath::from_slash("ets/modules.abc");
        assert!(!p.as_str().contains('\\'));
    }
}
