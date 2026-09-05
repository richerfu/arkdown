use std::path::PathBuf;
use std::sync::Arc;

use rustc_hash::FxHashMap;

/// Immutable package-resolution metadata shared by the OXC project checker
/// and Rolldown's OHOS resolver.
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
    /// Manifest fields required by package entry and bytecode-HAR resolution.
    pub manifest: Arc<PackageManifestSnapshot>,
    /// Whether this package is a selected local project module.
    pub local_source: bool,
    /// Absolute source roots in hvigor lookup order.
    pub source_roots: Vec<PathBuf>,
}

/// Resolver-owned projection of package manifest data captured by BuildSession.
///
/// Keeping this projection on the package plan prevents resolve hooks from
/// parsing `oh-package.json5` for every import.
#[derive(Debug, Clone, Default)]
pub struct PackageManifestSnapshot {
    pub version: String,
    pub main: Option<String>,
    pub types: Option<String>,
    pub exports: Option<serde_json::Value>,
    pub module: Option<String>,
    pub jsnext_main: Option<String>,
    pub byte_code_har: bool,
    pub abc_path: Option<String>,
}

impl PackageManifestSnapshot {
    pub fn hvigor_main_field(&self) -> Option<&str> {
        self.module
            .as_deref()
            .or(self.jsnext_main.as_deref())
            .or(self.main.as_deref())
    }

    pub fn exports_import_field(&self) -> Option<String> {
        let exports = self.exports.as_ref()?;
        if let Some(root) = exports.get(".") {
            if let Some(value) = root.get("import").and_then(serde_json::Value::as_str) {
                return Some(value.to_string());
            }
            if let Some(value) = root.get("default").and_then(serde_json::Value::as_str) {
                return Some(value.to_string());
            }
        }
        exports.as_str().map(str::to_string)
    }

    pub fn exports_require_field(&self) -> Option<String> {
        self.exports
            .as_ref()?
            .get(".")?
            .get("require")?
            .as_str()
            .map(str::to_string)
    }

    pub fn bytecode_abc_path(&self, package_root: &std::path::Path) -> Option<PathBuf> {
        if self.byte_code_har {
            let path = package_root.join("ets/modules.abc");
            if path.is_file() {
                return Some(path);
            }
        }
        if let Some(path) = self.abc_path.as_deref() {
            let path = package_root.join(path);
            if path.is_file() {
                return Some(path);
            }
        }
        if let Some(main) = self.main.as_deref().filter(|main| main.ends_with(".abc")) {
            let path = package_root.join(main);
            if path.is_file() {
                return Some(path);
            }
        }
        let path = package_root.join("index.abc");
        path.is_file().then_some(path)
    }

    pub fn declaration_entry(&self, package_root: &std::path::Path) -> PathBuf {
        self.types
            .as_deref()
            .map(|types| package_root.join(types))
            .unwrap_or_else(|| package_root.join("index.d.ets"))
    }
}

impl From<&crate::oh_modules::OhPackage> for PackageManifestSnapshot {
    fn from(package: &crate::oh_modules::OhPackage) -> Self {
        Self {
            version: package.version().to_string(),
            main: package.main.clone(),
            types: package.types.clone(),
            exports: package.exports.clone(),
            module: package.module.clone(),
            jsnext_main: package.jsnext_main.clone(),
            byte_code_har: package.metadata.byte_code_har,
            abc_path: package.metadata.abc_path.clone(),
        }
    }
}
