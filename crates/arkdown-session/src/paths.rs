//! [`PathResolver`] — project/module/package-relative paths and artifact
//! serialization.

use std::path::{Path, PathBuf};

use arkdown_path::{AbsPath, ArtifactPathSerializer, RelPath};

/// Standard OHOS project-level path layout used by session discovery.
pub struct ProjectPathLayout<'a> {
    project_root: &'a Path,
}

impl<'a> ProjectPathLayout<'a> {
    pub fn new(project_root: &'a Path) -> Self {
        Self { project_root }
    }

    pub fn app_scope_app_json5(&self) -> PathBuf {
        self.project_root.join("AppScope").join("app.json5")
    }

    pub fn app_scope_resources(&self) -> PathBuf {
        self.project_root.join("AppScope").join("resources")
    }
}

/// Standard OHOS module-level path layout used by session discovery.
pub struct ModulePathLayout<'a> {
    module_root: &'a Path,
}

impl<'a> ModulePathLayout<'a> {
    pub fn new(module_root: &'a Path) -> Self {
        Self { module_root }
    }

    pub fn src_main(&self) -> PathBuf {
        self.module_root.join("src").join("main")
    }

    pub fn module_json5(&self) -> PathBuf {
        self.src_main().join("module.json5")
    }

    /// Dependency profile candidates in the same precedence order used by
    /// hvigor's `DependencyManager.getDependencyTypeByProfile`.
    pub fn dependency_profile_candidates(&self) -> [PathBuf; 3] {
        let src_main = self.src_main();
        [
            src_main.join("module.json5"),
            src_main.join("module.json"),
            src_main.join("config.json"),
        ]
    }

    pub fn oh_package_json5(&self) -> PathBuf {
        self.module_root.join("oh-package.json5")
    }

    pub fn ets_source_root(&self) -> PathBuf {
        self.src_main().join("ets")
    }

    pub fn js_source_root(&self) -> PathBuf {
        self.src_main().join("js")
    }

    pub fn resources_root(&self) -> PathBuf {
        self.src_main().join("resources")
    }

    pub fn profile_root(&self) -> PathBuf {
        self.resources_root().join("base").join("profile")
    }

    pub fn bytecode_har_abc_candidates(&self) -> Vec<PathBuf> {
        vec![
            self.module_root.join("modules.abc"),
            self.module_root.join("ets").join("modules.abc"),
            self.module_root
                .join("build")
                .join("default")
                .join("intermediates")
                .join("loader_out")
                .join("default")
                .join("ets")
                .join("modules.abc"),
        ]
    }
}

/// Session path resolver for converting between absolute and relative paths
/// and for producing artifact path strings.
#[derive(Debug, Clone)]
pub struct PathResolver {
    /// Absolute path to the project root.
    pub project_root: PathBuf,
}

impl PathResolver {
    /// Construct a new path resolver for the given project root.
    pub fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }

    /// Return the given path as an `AbsPath`, canonicalizing if it exists or
    /// normalizing lexically if it does not.
    pub fn abs_existing_or_lexical(&self, path: &Path) -> AbsPath {
        if path.exists() {
            AbsPath::canonicalize(path).unwrap_or_else(|_| AbsPath::from_absolute(path).unwrap())
        } else if path.is_absolute() {
            AbsPath::from_absolute(path).unwrap()
        } else {
            let joined = self.project_root.join(path);
            AbsPath::from_absolute(joined.clone())
                .unwrap_or_else(|_| AbsPath::from_raw_unchecked(joined))
        }
    }

    /// Return the path relative to the project root.
    ///
    /// Returns `None` if `path` is not inside the project root.
    pub fn project_relative(&self, path: &Path) -> Option<RelPath> {
        let rel = path.strip_prefix(&self.project_root).ok()?;
        RelPath::parse(rel).ok()
    }

    /// Build an artifact path (slash-separated) from a project-relative path.
    pub fn artifact_path_from_relative(&self, rel: &RelPath) -> String {
        ArtifactPathSerializer::rel_to_slash(rel.as_path())
    }
}
