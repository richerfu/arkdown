use std::path::PathBuf;
use std::sync::Arc;

use rustc_hash::FxHashMap;

/// Immutable package-resolution metadata shared by the SDK checker and
/// Rolldown's OHOS resolver.
///
/// Latest hvigor builds `pkgNameToPkgBriefInfo` once for each ArkCompile
/// target. Dependencies are inserted first and the current package is inserted
/// last, so a project-level installation of the same package name cannot hide
/// the module currently being compiled.
#[derive(Debug, Clone, Default)]
pub struct PackageResolutionPlan {
    entries: FxHashMap<String, Arc<PkgBriefInfo>>,
}

impl PackageResolutionPlan {
    pub fn sorted_entries(&self) -> Vec<(&str, &PkgBriefInfo)> {
        let mut entries = self
            .entries
            .iter()
            .map(|(specifier, info)| (specifier.as_str(), info.as_ref()))
            .collect::<Vec<_>>();
        entries.sort_unstable_by_key(|(specifier, _)| *specifier);
        entries
    }
}

impl From<FxHashMap<String, PkgBriefInfo>> for PackageResolutionPlan {
    fn from(entries: FxHashMap<String, PkgBriefInfo>) -> Self {
        Self {
            entries: entries
                .into_iter()
                .map(|(specifier, info)| (specifier, Arc::new(info)))
                .collect(),
        }
    }
}

/// Builds hvigor's package-name index without exposing mutable state to
/// resolver hooks.
pub struct PackageResolutionPlanBuilder {
    entries: FxHashMap<String, Arc<PkgBriefInfo>>,
}

impl PackageResolutionPlanBuilder {
    pub fn new() -> Self {
        Self {
            entries: FxHashMap::default(),
        }
    }

    /// Insert dependency metadata with first-wins traversal semantics.
    ///
    /// Both the declared alias and the package's own name address the same
    /// immutable metadata allocation.
    pub fn insert_dependency(&mut self, alias: &str, info: PkgBriefInfo) -> &mut Self {
        let package_name = info.pkg_name.clone();
        let info = Arc::new(info);
        self.entries
            .entry(package_name)
            .or_insert_with(|| Arc::clone(&info));
        self.entries.entry(alias.to_string()).or_insert(info);
        self
    }

    /// Insert the active module after dependencies, matching
    /// `ArkCompile.setPkgNameToPkgBriefInfo()`'s self-last object spread.
    pub fn insert_current(&mut self, info: PkgBriefInfo) -> &mut Self {
        self.entries.insert(info.pkg_name.clone(), Arc::new(info));
        self
    }

    pub fn build(self) -> PackageResolutionPlan {
        PackageResolutionPlan {
            entries: self.entries,
        }
    }
}

impl Default for PackageResolutionPlanBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Package information derived before resolver hooks start.
#[derive(Debug, Clone)]
pub struct PkgBriefInfo {
    /// Canonical package root directory.
    pub pkg_root: PathBuf,
    /// Package name from `oh-package.json5`.
    pub pkg_name: String,
    /// Whether this package is a selected local project module.
    pub local_source: bool,
    /// Absolute source roots in hvigor lookup order.
    pub source_roots: Vec<PathBuf>,
}
