//! Component-based path classification for OHOS module paths.
//!
//! All classification uses [`Path::components`] so that it is not fooled by
//! substrings like `my_oh_modules_backup`.
//!
//! # Forbidden patterns (replaced by this module)
//!
//! | Forbidden                            | Replacement                        |
//! |--------------------------------------|------------------------------------|
//! | `path.contains("oh_modules")`        | [`is_inside_oh_modules`]           |
//! | `path.starts_with('/')`              | `Path::is_absolute()`              |
//! | `starts_with("@ohpm:")`              | [`OhmUrl::schema`]                 |

use std::path::Path;

/// Classification of a path relative to the OHOS module layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathKind {
    /// The path is inside an `oh_modules` directory.
    OhModule,
    /// The path is under `src/main`.
    SrcMain,
    /// The path is an `AppScope` project-level path.
    AppScope,
    /// None of the above; it is a regular project path.
    Project,
}

/// Return `true` if any component of `path` is exactly `oh_modules`.
///
/// Uses component-based matching to avoid false positives from directory
/// names that contain `oh_modules` as a substring.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use arkdown_path::classify::is_inside_oh_modules;
///
/// assert!(is_inside_oh_modules(Path::new("/project/oh_modules/some-lib")));
/// assert!(!is_inside_oh_modules(Path::new("/project/my_oh_modules_backup")));
/// ```
pub fn is_inside_oh_modules(path: &Path) -> bool {
    path.components()
        .any(|c| c.as_os_str() == "oh_modules")
}

/// Return `true` if any component of `path` is exactly `AppScope`.
pub fn is_app_scope(path: &Path) -> bool {
    path.components().any(|c| c.as_os_str() == "AppScope")
}

/// Classify a path into its [`PathKind`].
///
/// Classification is based on path components and does not require the
/// path to exist.
pub fn classify_path(path: &Path) -> PathKind {
    if is_inside_oh_modules(path) {
        PathKind::OhModule
    } else if is_app_scope(path) {
        PathKind::AppScope
    } else {
        // Check for src/main component sequence.
        let components: Vec<_> = path
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        let has_src_main = components
            .windows(2)
            .any(|w| w[0] == "src" && w[1] == "main");
        if has_src_main {
            PathKind::SrcMain
        } else {
            PathKind::Project
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    // ── is_inside_oh_modules ──────────────────────────────────────────────────

    #[test]
    fn test_oh_modules_exact_component() {
        assert!(is_inside_oh_modules(Path::new(
            "/project/oh_modules/some-lib/src/index.ets"
        )));
    }

    #[test]
    fn test_oh_modules_not_substring() {
        // `my_oh_modules_backup` must NOT match.
        assert!(!is_inside_oh_modules(Path::new(
            "/project/my_oh_modules_backup/file.ts"
        )));
    }

    #[test]
    fn test_oh_modules_nested() {
        assert!(is_inside_oh_modules(Path::new(
            "/project/entry/oh_modules/@ohos/hilog/index.d.ts"
        )));
    }

    #[test]
    fn test_oh_modules_absent() {
        assert!(!is_inside_oh_modules(Path::new(
            "/project/src/main/ets/pages/Index.ets"
        )));
    }

    // ── is_app_scope ──────────────────────────────────────────────────────────

    #[test]
    fn test_app_scope_exact_component() {
        assert!(is_app_scope(Path::new("/project/AppScope/app.json5")));
    }

    #[test]
    fn test_app_scope_not_substring() {
        assert!(!is_app_scope(Path::new("/project/MyAppScope/config.json")));
    }

    // ── classify_path ─────────────────────────────────────────────────────────

    #[test]
    fn test_classify_oh_module() {
        let k = classify_path(Path::new("/project/oh_modules/lib/index.ets"));
        assert_eq!(k, PathKind::OhModule);
    }

    #[test]
    fn test_classify_app_scope() {
        let k = classify_path(Path::new("/project/AppScope/resources/base/profile/main_pages.json"));
        assert_eq!(k, PathKind::AppScope);
    }

    #[test]
    fn test_classify_src_main() {
        let k = classify_path(Path::new("/project/entry/src/main/ets/pages/Index.ets"));
        assert_eq!(k, PathKind::SrcMain);
    }

    #[test]
    fn test_classify_project() {
        let k = classify_path(Path::new("/project/hvigorfile.ts"));
        assert_eq!(k, PathKind::Project);
    }

    #[test]
    fn test_oh_modules_takes_precedence_over_src_main() {
        // A path that is both in oh_modules and happens to have src/main should
        // be classified as OhModule.
        let k = classify_path(Path::new(
            "/project/oh_modules/lib/src/main/ets/Index.ets",
        ));
        assert_eq!(k, PathKind::OhModule);
    }
}
