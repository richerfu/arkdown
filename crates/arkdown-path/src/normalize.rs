//! Lexical path normalization that does not require path existence.

use std::path::{Path, PathBuf};

/// Lexically normalize a path by collapsing `.` components and resolving `..`
/// where possible without touching the filesystem.
///
/// This is safe to call on both existing and non-existing paths.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use arkdown_path::normalize::lexical_normalize;
///
/// let p = lexical_normalize(Path::new("/foo/./bar/../baz"));
/// assert_eq!(p, Path::new("/foo/baz"));
/// ```
pub fn lexical_normalize(path: &Path) -> PathBuf {
    crate::abs::lexical_normalize_pathbuf(path.to_path_buf())
}

/// Strip leading `./` or `.` from a relative path string.
///
/// This is the module-specifier–safe variant: it operates on the string
/// representation and is only used to normalize bare specifiers like
/// `"./pages/Index"` into `"pages/Index"` for module ID comparisons.
///
/// Do **not** use this on filesystem `PathBuf` values; use
/// [`lexical_normalize`] instead.
pub fn strip_relative_prefix(s: &str) -> &str {
    if let Some(rest) = s.strip_prefix("./") {
        rest
    } else if s == "." {
        ""
    } else {
        s
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_lexical_normalize_cur_dir() {
        let p = lexical_normalize(Path::new("/foo/./bar"));
        assert_eq!(p, Path::new("/foo/bar"));
    }

    #[test]
    fn test_lexical_normalize_parent_dir() {
        let p = lexical_normalize(Path::new("/foo/bar/../baz"));
        assert_eq!(p, Path::new("/foo/baz"));
    }

    #[test]
    fn test_lexical_normalize_multiple() {
        let p = lexical_normalize(Path::new("/a/b/./c/../d/e"));
        assert_eq!(p, Path::new("/a/b/d/e"));
    }

    #[test]
    fn test_lexical_normalize_trailing_slash_stripped() {
        // PathBuf treats "foo/bar/" == "foo/bar" after round-trip through components.
        let p = lexical_normalize(Path::new("/foo/bar/"));
        assert_eq!(p, Path::new("/foo/bar"));
    }

    #[test]
    fn test_lexical_normalize_preserves_unmatched_relative_parent() {
        let p = lexical_normalize(Path::new("a/../../b"));
        assert_eq!(p, Path::new("../b"));
    }

    #[test]
    fn test_strip_relative_prefix_dot_slash() {
        assert_eq!(strip_relative_prefix("./pages/Index"), "pages/Index");
    }

    #[test]
    fn test_strip_relative_prefix_dot() {
        assert_eq!(strip_relative_prefix("."), "");
    }

    #[test]
    fn test_strip_relative_prefix_no_prefix() {
        assert_eq!(strip_relative_prefix("pages/Index"), "pages/Index");
    }

    #[test]
    fn test_strip_relative_prefix_dot_dot_unchanged() {
        // `..` must not be silently stripped — it is a parent traversal.
        assert_eq!(strip_relative_prefix("../other"), "../other");
    }
}
