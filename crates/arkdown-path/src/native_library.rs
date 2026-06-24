use std::path::{Component, Path};

use crate::ArtifactPathSerializer;

/// Hvigor's default native library glob.
///
/// Upstream:
/// `hvigor-ohos-plugin/src/const/common-const.ts::NativeConst.LIBRARY_FILE_PATTERN`
/// and `ProcessLibsCommonProperties.getLibsInfo`.
pub struct HvigorNativeLibraryPattern;

impl HvigorNativeLibraryPattern {
    pub const LIBRARY_FILE_PATTERN: &'static str = "**/*.so*(.*[0-9])";

    const FAST_GLOB_PATTERNS: [&'static str; 4] =
        ["*.so", "**/*.so", "*.so.*[0-9]", "**/*.so.*[0-9]"];
    const FAST_GLOB_FILE_NAME_PATTERNS: [&'static str; 2] = ["*.so", "*.so.*[0-9]"];

    pub fn matches_relative_path(path: &Path) -> bool {
        if Self::has_hvigor_dot_component(path) {
            return false;
        }
        let Ok(path) = ArtifactPathSerializer::path_to_slash_string(path) else {
            return false;
        };
        Self::FAST_GLOB_PATTERNS
            .iter()
            .any(|pattern| fast_glob::glob_match(pattern, &path))
    }

    pub fn matches_path_under(base: &Path, path: &Path) -> bool {
        let Ok(relative) = path.strip_prefix(base) else {
            return false;
        };
        Self::matches_relative_path(relative)
    }

    pub fn matches_file_name(name: &str) -> bool {
        if name.starts_with('.') {
            return false;
        }
        Self::FAST_GLOB_FILE_NAME_PATTERNS
            .iter()
            .any(|pattern| fast_glob::glob_match(pattern, name))
    }

    fn has_hvigor_dot_component(path: &Path) -> bool {
        path.components().any(|component| {
            matches!(component, Component::Normal(name) if name.to_str().is_some_and(|name| name.starts_with('.')))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_library_pattern_matches_hvigor_default_glob() {
        assert!(HvigorNativeLibraryPattern::matches_relative_path(
            Path::new("liba.so")
        ));
        assert!(HvigorNativeLibraryPattern::matches_relative_path(
            Path::new("arm64-v8a/liba.so")
        ));
        assert!(HvigorNativeLibraryPattern::matches_relative_path(
            Path::new("arm64-v8a/liba.so.1")
        ));
        assert!(HvigorNativeLibraryPattern::matches_relative_path(
            Path::new("arm64-v8a/liba.so.1.2")
        ));

        assert!(!HvigorNativeLibraryPattern::matches_relative_path(
            Path::new("arm64-v8a/liba.so1")
        ));
        assert!(!HvigorNativeLibraryPattern::matches_relative_path(
            Path::new("arm64-v8a/liba.so.backup")
        ));
        assert!(!HvigorNativeLibraryPattern::matches_relative_path(
            Path::new("arm64-v8a/liba.solib")
        ));
    }

    #[test]
    fn native_library_pattern_uses_hvigor_glob_dot_default() {
        assert!(!HvigorNativeLibraryPattern::matches_relative_path(
            Path::new("arm64-v8a/._liba.so")
        ));
        assert!(!HvigorNativeLibraryPattern::matches_relative_path(
            Path::new("arm64-v8a/.hidden.so")
        ));
        assert!(!HvigorNativeLibraryPattern::matches_relative_path(
            Path::new(".hidden/liba.so")
        ));
    }
}
