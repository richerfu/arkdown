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
//! | `path.starts_with('/')`              | `Path::has_root()`                 |
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

/// Component-based classifier for filesystem-like module paths.
pub struct PathClassifier<'a> {
    path: &'a Path,
}

impl<'a> PathClassifier<'a> {
    /// Create a classifier for `path`.
    pub fn new(path: &'a Path) -> Self {
        Self { path }
    }

    /// Return `true` if any component of the path is exactly `oh_modules`.
    pub fn is_inside_oh_modules(&self) -> bool {
        self.has_component("oh_modules")
    }

    /// Return `true` if any component of the path is exactly `node_modules`.
    pub fn is_inside_node_modules(&self) -> bool {
        self.has_component("node_modules")
    }

    /// Return `true` if any component of the path is exactly `AppScope`.
    pub fn is_app_scope(&self) -> bool {
        self.has_component("AppScope")
    }

    /// Return `true` for dependency module roots handled as common chunks.
    pub fn is_inside_dependency_modules(&self) -> bool {
        self.is_inside_oh_modules() || self.is_inside_node_modules()
    }

    /// Classify the path into its [`PathKind`].
    pub fn classify(&self) -> PathKind {
        if self.is_inside_oh_modules() {
            PathKind::OhModule
        } else if self.is_app_scope() {
            PathKind::AppScope
        } else if self.has_src_main_sequence() {
            PathKind::SrcMain
        } else {
            PathKind::Project
        }
    }

    fn has_component(&self, component: &str) -> bool {
        self.path.components().any(|c| c.as_os_str() == component)
    }

    fn has_src_main_sequence(&self) -> bool {
        let mut previous_is_src = false;
        for component in self.path.components() {
            let current = component.as_os_str();
            if previous_is_src && current == "main" {
                return true;
            }
            previous_is_src = current == "src";
        }
        false
    }
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
    PathClassifier::new(path).is_inside_oh_modules()
}

/// Return `true` if any component of `path` is exactly `AppScope`.
pub fn is_app_scope(path: &Path) -> bool {
    PathClassifier::new(path).is_app_scope()
}

/// Classify a path into its [`PathKind`].
///
/// Classification is based on path components and does not require the
/// path to exist.
pub fn classify_path(path: &Path) -> PathKind {
    PathClassifier::new(path).classify()
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
    fn test_dependency_modules_are_exact_components() {
        assert!(
            PathClassifier::new(Path::new("/project/node_modules/pkg/index.js"))
                .is_inside_dependency_modules()
        );
        assert!(
            !PathClassifier::new(Path::new("/project/my_node_modules_backup/file.js"))
                .is_inside_dependency_modules()
        );
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
        let k = classify_path(Path::new(
            "/project/AppScope/resources/base/profile/main_pages.json",
        ));
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
        let k = classify_path(Path::new("/project/oh_modules/lib/src/main/ets/Index.ets"));
        assert_eq!(k, PathKind::OhModule);
    }
}
