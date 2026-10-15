use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::layout::ProfileModuleLayout;
use crate::loader_json::ByteCodeHarEntry;
use arkdown_path::{canonicalize_or_owned, ArtifactPathSerializer, OhmUrl, SourceFileName};
#[cfg(test)]
use arkdown_path::{ArtifactPath, ModuleSpecifierText, SpecifierKind};
#[cfg(test)]
use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;

// ---------------------------------------------------------------------------
// oh-package.json5 subset for ByteCode HAR detection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OhPkgMinimal {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    compatible_sdk_version: Option<CompatibleSdkVersionValue>,
    #[serde(default)]
    compatible_sdk_version_stage: Option<String>,
    #[serde(default)]
    main: Option<String>,
    #[cfg(test)]
    #[serde(default, alias = "typings")]
    types: Option<String>,
    #[cfg(test)]
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    dependencies: IndexMap<String, String>,
    #[serde(default)]
    dynamic_dependencies: IndexMap<String, String>,
    /// `devDependencies` — excluded from the dependency walk unless the
    /// `ohos.byteCodeHar.integratedOptimization` inject is enabled (hvigor
    /// `getFinalNodes` default filters `DEV_DEPENDENCIES`).
    #[cfg(test)]
    #[serde(default)]
    dev_dependencies: IndexMap<String, String>,
    #[serde(default)]
    metadata: Option<OhPkgMetadata>,
}

impl OhPkgMinimal {
    fn from_snapshot(package: &arkdown_session::package::snapshot::PackageSnapshot) -> Self {
        let raw = &package.package_json;
        Self {
            name: (!raw.name.is_empty()).then(|| raw.name.clone()),
            compatible_sdk_version: raw
                .extra
                .get("compatibleSdkVersion")
                .cloned()
                .and_then(|value| serde_json::from_value(value).ok()),
            compatible_sdk_version_stage: raw
                .extra
                .get("compatibleSdkVersionStage")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
            main: raw.main.clone(),
            #[cfg(test)]
            types: raw.types.clone().or_else(|| {
                raw.extra
                    .get("typings")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string)
            }),
            #[cfg(test)]
            version: (!raw.version.is_empty()).then(|| raw.version.clone()),
            dependencies: raw.dependencies.clone(),
            dynamic_dependencies: raw.dynamic_dependencies.clone(),
            #[cfg(test)]
            dev_dependencies: raw.dev_dependencies.clone(),
            metadata: raw
                .extra
                .get("metadata")
                .cloned()
                .and_then(|value| serde_json::from_value(value).ok()),
        }
    }

    /// `metadata.byteCodeHar` — the SDK marker used by
    /// `DefaultNpmDependency.isByteCodeHarDependency()`.
    #[cfg(test)]
    fn is_byte_code_har(&self) -> bool {
        self.metadata
            .as_ref()
            .is_some_and(|metadata| metadata.byte_code_har)
    }
}

impl OhPkgMinimal {
    #[cfg(test)]
    fn load(pkg_dir: &Path) -> Option<Self> {
        let pkg_json = ProfileModuleLayout::new(pkg_dir).oh_package_json5();
        if !pkg_json.exists() {
            return None;
        }
        let content = std::fs::read_to_string(&pkg_json).ok()?;
        json5::from_str::<Self>(&content).ok()
    }

    fn resolve_abc_path(&self, pkg_dir: &Path) -> Option<PathBuf> {
        // `DefaultNpmDependency.isByteCodeHarDependency()` reads the current
        // SDK marker exclusively from `metadata.byteCodeHar`.
        if !self
            .metadata
            .as_ref()
            .is_some_and(|metadata| metadata.byte_code_har)
        {
            return None;
        }

        let layout = ProfileModuleLayout::new(pkg_dir);
        for abc_path in [layout.ets_modules_abc(), layout.ets_modules_static_abc()] {
            if abc_path.is_file() {
                return Some(abc_path);
            }
        }

        if let Some(path) = self
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.abc_path.as_deref())
        {
            let abc_path = pkg_dir.join(path);
            if abc_path.exists() {
                return Some(abc_path);
            }
        }

        if let Some(main) = self
            .main
            .as_deref()
            .filter(|main| SourceFileName::new(Path::new(main)).has_extension("abc"))
        {
            let abc_path = pkg_dir.join(main);
            if abc_path.exists() {
                return Some(abc_path);
            }
        }

        let abc_path = layout.index_abc();
        abc_path.exists().then_some(abc_path)
    }

    fn resolve_source_maps_path(&self, pkg_dir: &Path) -> Option<PathBuf> {
        if let Some(path) = self
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.source_maps_path.as_deref())
        {
            return Some(pkg_dir.join(path));
        }

        let candidate = pkg_dir.join("ets").join("sourceMaps.map");
        if candidate.exists() {
            return Some(candidate);
        }

        let candidate = pkg_dir.join("src").join("sourceMaps.map");
        candidate.exists().then_some(candidate)
    }

    fn package_name<'a>(&'a self, fallback: &'a str) -> &'a str {
        self.name
            .as_deref()
            .filter(|name| !name.is_empty())
            .unwrap_or(fallback)
    }

    fn dependency_names(&self) -> Vec<String> {
        let mut seen = FxHashSet::default();
        // Include standard deps, dynamic deps, AND metadata.runtimeOnly.packages.
        // runtimeOnly packages can be BC HARs that need to appear in byteCodeHarInfo.
        let runtime_only_pkgs: Vec<&String> = self
            .metadata
            .as_ref()
            .and_then(|m| m.runtime_only.as_ref())
            .map(|ro| ro.packages.iter().collect())
            .unwrap_or_default();
        self.dependencies
            .keys()
            .chain(self.dynamic_dependencies.keys())
            .chain(runtime_only_pkgs)
            .filter(|name| seen.insert((*name).clone()))
            .cloned()
            .collect()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OhPkgMetadata {
    #[serde(default)]
    byte_code_har: bool,
    #[serde(default)]
    abc_path: Option<String>,
    #[serde(default)]
    source_maps_path: Option<String>,
    #[serde(default)]
    insight_intent_path: Option<String>,
    #[serde(default)]
    declaration_entry: Vec<String>,
    #[serde(default)]
    dependency_pkg_version: IndexMap<String, String>,
    #[serde(default)]
    extra_importees: Vec<String>,
    /// runtimeOnly packages — may contain BC HAR package names that should
    /// be included in `byteCodeHarInfo` and `compiledImportees`.
    #[serde(default)]
    runtime_only: Option<OhPkgRuntimeOnly>,
}

/// `metadata.runtimeOnly` section of `oh-package.json5`.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct OhPkgRuntimeOnly {
    #[serde(default)]
    packages: Vec<String>,
}

/// Minimal dependency profile used to preserve hvigor's two independent
/// ByteCode HAR predicates.
///
/// `DefaultNpmDependency.isByteCodeHarDependency()` reads
/// `oh-package.json5#metadata.byteCodeHar`, while
/// `TaskService.getHarDependencies()` is classified from
/// `src/main/module.json[5]#module.type`. A package can therefore contribute
/// to `byteCodeHarInfo` without contributing to `updateVersionInfo`.
#[cfg(test)]
#[derive(Debug, Deserialize)]
struct ByteCodeHarDependencyProfile {
    module: ByteCodeHarDependencyProfileModule,
}

#[cfg(test)]
#[derive(Debug, Deserialize)]
struct ByteCodeHarDependencyProfileModule {
    #[serde(rename = "type", default)]
    module_type: Option<String>,
    #[serde(default)]
    distro: Option<ByteCodeHarDependencyProfileDistro>,
}

#[cfg(test)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ByteCodeHarDependencyProfileDistro {
    #[serde(default)]
    module_type: Option<String>,
}

#[cfg(test)]
impl ByteCodeHarDependencyProfile {
    fn is_har_dependency(package_root: &Path) -> bool {
        let layout = ProfileModuleLayout::new(package_root);
        for path in [
            layout.module_json5(),
            layout.module_json(),
            layout.src_main().join("config.json"),
        ] {
            let Ok(content) = std::fs::read_to_string(path) else {
                continue;
            };
            let Ok(profile) = json5::from_str::<Self>(&content) else {
                continue;
            };
            let module_type = profile
                .module
                .module_type
                .or_else(|| profile.module.distro.and_then(|distro| distro.module_type));
            match module_type.as_deref() {
                Some("har") => return true,
                Some("shared" | "entry" | "feature") => return false,
                _ => {}
            }
        }
        false
    }
}

struct PackageDependencySpec<'a> {
    raw: &'a str,
}

impl<'a> PackageDependencySpec<'a> {
    const FILE_PROTOCOL: &'static str = "file:";

    fn new(raw: &'a str) -> Self {
        Self { raw }
    }

    fn is_file_dependency(&self) -> bool {
        self.raw.starts_with(Self::FILE_PROTOCOL)
    }
}

/// `compatibleSdkVersion` can be a plain integer (`12`) or a string (`"5.0.0(12)"`).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum CompatibleSdkVersionValue {
    Int(u32),
    Str(String),
}

// ---------------------------------------------------------------------------
// Scan result
// ---------------------------------------------------------------------------

/// Metadata collected for one ByteCode HAR dependency found under `oh_modules/`.
#[derive(Debug, Clone)]
pub struct ByteCodeHarScanEntry {
    /// Package name from `oh-package.json5`.
    pub package_name: String,
    /// Absolute path to the package root directory.
    pub package_root: PathBuf,
    /// Absolute path to the pre-compiled `.abc` file.
    pub abc_path: PathBuf,
    /// Whether hvigor classifies this package as a HAR dependency from its
    /// module profile. This is independent of `metadata.byteCodeHar` and gates
    /// the non-integrated-optimization `updateVersionInfo` traversal.
    pub is_har_dependency: bool,
    /// Compatible SDK version in its source scalar form. Hvigor preserves
    /// numeric API levels as JSON numbers and named versions as strings.
    pub compatible_sdk_version: Option<CompatibleSdkVersionValue>,
    /// Optional compatible SDK stage from `oh-package.json5`.
    pub compatible_sdk_version_stage: Option<String>,
    /// Optional path to the `sourceMaps.map` file alongside the `.abc`.
    pub source_maps_path: Option<PathBuf>,
    /// Optional path to the ByteCode HAR insight intent metadata.
    pub insight_intent: Option<PathBuf>,
    /// OHM URLs from `metadata.declarationEntry`, generated by hvigor for
    /// ByteCode HAR worker/runtimeOnly/startup/insight-intent reachability.
    pub declaration_entry: Vec<String>,
    /// Extra importee strings from `metadata.extraImportees`.
    pub extra_importees: Vec<String>,
    /// Main entry path from `oh-package.json5#main`, used to compute the
    /// compiled-importee OHM URL for this package.
    pub main_entry: Option<String>,
}

impl ByteCodeHarScanEntry {
    /// Build the OHM URL for this ByteCode HAR package.
    ///
    /// Format: `@bundle:<bundleName>/<packageName>/<mainEntryStem>`
    ///
    /// `mainEntryStem` is the `main` field with the file extension stripped.
    /// When `main` is absent or empty, returns `None`.
    ///
    /// hvigor generates this URL via `OhmUrlUtils.genOhmUrlForByteCodeHar()` and
    /// adds it to `compiledImportees` so ArkRuntime can locate the HAR's compiled
    /// module graph.
    pub fn ohm_url(&self, bundle_name: &str) -> Option<String> {
        let main = self.main_entry.as_deref().filter(|s| !s.is_empty())?;
        // Strip file extension — "index.abc" → "index", "index" → "index".
        let stem = std::path::Path::new(main).with_extension("");
        let stem = ArtifactPathSerializer::rel_to_slash(&stem);
        Some(OhmUrl::bundle(bundle_name, &self.package_name, &stem).into_string())
    }
}

impl ByteCodeHarScanEntry {
    /// Convert into the [`ByteCodeHarEntry`] written to `loader.json`.
    pub fn into_loader_entry(self) -> (String, ByteCodeHarEntry) {
        let entry = ByteCodeHarEntry {
            abc_path: ByteCodeHarPathText::new(&self.abc_path).string(),
            compatible_sdk_version: self.compatible_sdk_version,
            compatible_sdk_version_stage: self.compatible_sdk_version_stage,
            source_maps_path: self
                .source_maps_path
                .map(|p| ByteCodeHarPathText::new(&p).string()),
            insight_intent: self
                .insight_intent
                .map(|p| ByteCodeHarPathText::new(&p).string()),
            // Propagate pre-compiled `metadata.declarationEntry` OHM URLs
            // from the ByteCode HAR's `oh-package.json5` into the consuming
            // HAP/HSP's `loader.json#byteCodeHarInfo` entry.
            declaration_entry: self.declaration_entry,
        };
        (self.package_name, entry)
    }

    // -----------------------------------------------------------------------
    // Private scan helpers
    // -----------------------------------------------------------------------

    #[cfg(test)]
    fn scan_oh_modules_dir(oh_modules: &Path, seen: &mut FxHashSet<String>, out: &mut Vec<Self>) {
        for entry in Self::read_dir_sorted(oh_modules) {
            let pkg_dir = entry.path();
            if !pkg_dir.is_dir() {
                continue;
            }
            let name = entry.file_name();
            let Some(name_str) = name.to_str() else {
                continue;
            };

            // Handle scoped packages: `@scope/pkg` lives as `oh_modules/@scope/pkg`.
            if matches!(
                ModuleSpecifierText::new(name_str).kind(),
                SpecifierKind::ScopedPackage
            ) {
                // Recurse one level for scoped packages.
                for scoped_entry in Self::read_dir_sorted(&pkg_dir) {
                    let scoped_dir = scoped_entry.path();
                    if !scoped_dir.is_dir() {
                        continue;
                    }
                    let scoped_name = scoped_entry.file_name();
                    let Some(scoped_name) = scoped_name.to_str() else {
                        continue;
                    };
                    let pkg_name = ArtifactPath::from_slash(name_str)
                        .join(scoped_name)
                        .into_string();
                    Self::try_collect(scoped_dir, &pkg_name, seen, out);
                }
            } else {
                Self::try_collect(pkg_dir, name_str, seen, out);
            }
        }
    }

    #[cfg(test)]
    fn read_dir_sorted(dir: &Path) -> Vec<std::fs::DirEntry> {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(|entry| entry.file_name());
        entries
    }

    #[cfg(test)]
    fn try_collect(
        pkg_dir: PathBuf,
        pkg_name: &str,
        seen: &mut FxHashSet<String>,
        out: &mut Vec<Self>,
    ) {
        let Some(pkg) = OhPkgMinimal::load(&pkg_dir) else {
            return;
        };

        let is_har_dependency = ByteCodeHarDependencyProfile::is_har_dependency(pkg_dir.as_path());

        Self::try_collect_loaded(pkg_dir, pkg_name, pkg, is_har_dependency, seen, out);
    }

    fn try_collect_loaded(
        pkg_dir: PathBuf,
        fallback_pkg_name: &str,
        pkg: OhPkgMinimal,
        is_har_dependency: bool,
        seen: &mut FxHashSet<String>,
        out: &mut Vec<Self>,
    ) {
        let package_name = pkg.package_name(fallback_pkg_name).to_string();
        if seen.contains(&package_name) {
            return;
        }

        let Some(abc_path) = pkg.resolve_abc_path(&pkg_dir) else {
            return;
        };

        let compatible_sdk_version = pkg.compatible_sdk_version.clone();
        let compatible_sdk_version_stage = pkg.compatible_sdk_version_stage.clone();
        let source_maps_path = pkg.resolve_source_maps_path(&pkg_dir);
        let insight_intent = pkg
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.insight_intent_path.as_deref())
            .map(|path| pkg_dir.join(path));
        let declaration_entry = pkg
            .metadata
            .as_ref()
            .map(|metadata| metadata.declaration_entry.clone())
            .unwrap_or_default();
        let extra_importees = pkg
            .metadata
            .as_ref()
            .map(|metadata| metadata.extra_importees.clone())
            .unwrap_or_default();
        let main_entry = pkg.main.clone();

        seen.insert(package_name.clone());
        out.push(Self {
            package_name,
            package_root: pkg_dir,
            abc_path,
            is_har_dependency,
            compatible_sdk_version,
            compatible_sdk_version_stage,
            source_maps_path,
            insight_intent,
            declaration_entry,
            extra_importees,
            main_entry,
        });
    }

    fn path_key(path: &Path) -> String {
        let path = canonicalize_or_owned(path);
        ByteCodeHarPathText::new(&path).string()
    }
}

#[cfg(test)]
struct ByteCodeHarGraphScanner<'a> {
    module_root: &'a Path,
    project_root: &'a Path,
    seen_roots: FxHashSet<String>,
    seen_outputs: FxHashSet<String>,
    results: Vec<ByteCodeHarScanEntry>,
}

#[cfg(test)]
impl<'a> ByteCodeHarGraphScanner<'a> {
    fn new(module_root: &'a Path, project_root: &'a Path) -> Self {
        Self {
            module_root,
            project_root,
            seen_roots: FxHashSet::default(),
            seen_outputs: FxHashSet::default(),
            results: Vec::new(),
        }
    }

    fn scan(mut self) -> Vec<ByteCodeHarScanEntry> {
        let mut queue = std::collections::VecDeque::new();
        self.enqueue_root_dependencies(self.module_root, &mut queue);
        if self.project_root != self.module_root {
            self.enqueue_root_dependencies(self.project_root, &mut queue);
        }

        if queue.is_empty() {
            return self.scan_all_oh_modules_fallback();
        }

        while let Some((dep_name, issuer_root)) = queue.pop_front() {
            for pkg_dir in self.candidate_dep_dirs(&dep_name, &issuer_root) {
                let root_key = ByteCodeHarScanEntry::path_key(&pkg_dir);
                if !self.seen_roots.insert(root_key) {
                    continue;
                }

                let Some(pkg) = OhPkgMinimal::load(&pkg_dir) else {
                    continue;
                };
                let child_dep_names = pkg.dependency_names();
                let is_har_dependency =
                    ByteCodeHarDependencyProfile::is_har_dependency(pkg_dir.as_path());

                ByteCodeHarScanEntry::try_collect_loaded(
                    pkg_dir.clone(),
                    &dep_name,
                    pkg,
                    is_har_dependency,
                    &mut self.seen_outputs,
                    &mut self.results,
                );

                for child in child_dep_names {
                    queue.push_back((child, pkg_dir.clone()));
                }
            }
        }

        self.results
    }

    #[cfg(test)]
    fn enqueue_root_dependencies(
        &self,
        root: &Path,
        queue: &mut std::collections::VecDeque<(String, PathBuf)>,
    ) {
        let Some(pkg) = OhPkgMinimal::load(root) else {
            return;
        };
        for dep_name in pkg.dependency_names() {
            queue.push_back((dep_name, root.to_path_buf()));
        }
    }

    fn candidate_dep_dirs(&self, dep_name: &str, issuer_root: &Path) -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        let candidates = [
            issuer_root.join("oh_modules").join(dep_name),
            self.module_root.join("oh_modules").join(dep_name),
            self.project_root.join("oh_modules").join(dep_name),
        ];

        let mut seen = FxHashSet::default();
        for dir in candidates {
            if dir.is_dir() && seen.insert(ByteCodeHarScanEntry::path_key(&dir)) {
                dirs.push(dir);
            }
        }
        dirs
    }

    fn scan_all_oh_modules_fallback(mut self) -> Vec<ByteCodeHarScanEntry> {
        for base in &[
            self.module_root.join("oh_modules"),
            self.project_root.join("oh_modules"),
        ] {
            if !base.exists() {
                continue;
            }
            ByteCodeHarScanEntry::scan_oh_modules_dir(
                base,
                &mut self.seen_outputs,
                &mut self.results,
            );
        }
        self.results
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Writes and collects ByteCode HAR metadata used by loader/package stages.
///
/// Production dependency discovery is supplied by `arkdown-session`; this
/// owner only reads ByteCode HAR metadata from typed package roots.
pub struct ByteCodeHarMetadataWriter;

impl ByteCodeHarMetadataWriter {
    /// Create a metadata writer.
    pub fn new() -> Self {
        Self
    }

    /// Test-fixture scanner retained outside production code so metadata tests
    /// can exercise synthetic directory layouts without constructing a full
    /// project session.
    #[cfg(test)]
    pub(crate) fn scan_bytecode_har_deps_from_graph(
        module_root: &Path,
        project_root: &Path,
    ) -> Vec<ByteCodeHarScanEntry> {
        ByteCodeHarGraphScanner::new(module_root, project_root).scan()
    }

    /// Collect ByteCode HAR dependencies from the immutable session graph.
    ///
    /// Package manifests and module profiles were parsed during session
    /// startup. This stage only materializes loader fields from those typed
    /// snapshots and validates the referenced ABC paths.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to hvigor's `DependencyManager.getHspOrByteCodeHarDependencies()`
    /// backed by the dependency graph, used by `generate-loader-json.ts` to
    /// locate ByteCode HAR dependencies without re-scanning the filesystem.
    pub fn scan_from_dependency_graph(
        graph: &arkdown_session::DependencyGraphSnapshot,
    ) -> Vec<ByteCodeHarScanEntry> {
        use arkdown_core::OhosModuleType;

        let mut seen = FxHashSet::default();
        let mut out = Vec::new();
        for node in graph.bytecode_har_nodes_in_traversal_order() {
            let (Some(package_root), Some(package)) =
                (node.package_root.as_ref(), node.package.as_ref())
            else {
                continue;
            };
            ByteCodeHarScanEntry::try_collect_loaded(
                package_root.clone(),
                &node.package_name,
                OhPkgMinimal::from_snapshot(package),
                node.module_profile
                    .as_ref()
                    .and_then(|profile| profile.module_type.as_ref())
                    .is_some_and(|module_type| matches!(module_type, OhosModuleType::Har)),
                &mut seen,
                &mut out,
            );
        }
        out
    }
}

impl Default for ByteCodeHarMetadataWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl ByteCodeHarScanEntry {
    /// Build hvigor's `loader.json#updateVersionInfo` for ByteCode HAR packages.
    pub fn collect_bytecode_har_update_version_info_from_graph(
        deps: &[ByteCodeHarScanEntry],
        graph: &arkdown_session::DependencyGraphSnapshot,
    ) -> IndexMap<String, serde_json::Value> {
        let mut info = IndexMap::new();

        for dep in deps.iter().filter(|dep| dep.is_har_dependency) {
            let Some(node) = graph.nodes.values().find(|node| {
                node.package_root.as_ref() == Some(&dep.package_root)
                    || node.package_name == dep.package_name
            }) else {
                continue;
            };
            let Some(package) = node.package.as_ref() else {
                continue;
            };
            let package = OhPkgMinimal::from_snapshot(package);
            let mut version_info = serde_json::Map::new();
            if let Some(metadata) = package
                .metadata
                .as_ref()
                .filter(|metadata| !metadata.dependency_pkg_version.is_empty())
            {
                for (name, expected_version) in &metadata.dependency_pkg_version {
                    let Some(installed_version) =
                        Self::dependency_version_from_graph(name, &dep.package_root, graph)
                    else {
                        continue;
                    };
                    if installed_version != *expected_version {
                        version_info
                            .insert(name.clone(), serde_json::Value::String(installed_version));
                    }
                }
            } else {
                for name in package.dependency_names() {
                    let Some(installed_version) =
                        Self::dependency_version_from_graph(&name, &dep.package_root, graph)
                    else {
                        continue;
                    };
                    version_info.insert(name, serde_json::Value::String(installed_version));
                }
            }
            info.insert(
                dep.package_name.clone(),
                serde_json::Value::Object(version_info),
            );
        }

        info
    }

    fn dependency_version_from_graph(
        dependency_name: &str,
        issuer_root: &Path,
        graph: &arkdown_session::DependencyGraphSnapshot,
    ) -> Option<String> {
        let issuer_root = graph.canonical_package_root(issuer_root);
        graph
            .edges
            .iter()
            .find(|edge| {
                graph.canonical_package_root(&edge.issuer_root) == issuer_root
                    && (edge.alias == dependency_name || edge.package_name == dependency_name)
            })
            .and_then(|edge| graph.nodes.get(&edge.package_name))
            .and_then(|node| node.package.as_ref())
            .map(|package| package.version.clone())
            .filter(|version| !version.trim().is_empty())
    }

    /// Test-only filesystem fixture helper for synthetic layouts that do not
    /// construct a session dependency graph.
    #[cfg(test)]
    pub fn collect_bytecode_har_update_version_info(
        deps: &[ByteCodeHarScanEntry],
        module_root: &Path,
        project_root: &Path,
    ) -> IndexMap<String, serde_json::Value> {
        let mut info = IndexMap::new();

        for dep in deps {
            // Latest IDE hvigor 6.24.3
            // `GenerateLoaderJson.processUpdateVersionInfo()` uses
            // `getHarDependencies().filter(isByteCodeHarDependency)` unless
            // the separate integrated-optimization property is enabled.
            // Arkdown's currently supported default mode must therefore not
            // confuse every byteCodeHarInfo entry with a HAR dependency.
            if !dep.is_har_dependency {
                continue;
            }
            let mut version_info = serde_json::Map::new();
            let Some(pkg) = OhPkgMinimal::load(&dep.package_root) else {
                continue;
            };

            if let Some(metadata) = pkg
                .metadata
                .as_ref()
                .filter(|m| !m.dependency_pkg_version.is_empty())
            {
                for (name, expected_version) in &metadata.dependency_pkg_version {
                    let Some(installed_version) = Self::resolve_dependency_version(
                        name,
                        &dep.package_root,
                        module_root,
                        project_root,
                    ) else {
                        continue;
                    };
                    if installed_version != *expected_version {
                        version_info
                            .insert(name.clone(), serde_json::Value::String(installed_version));
                    }
                }
            } else {
                for name in pkg.dependency_names() {
                    let Some(installed_version) = Self::resolve_dependency_version(
                        &name,
                        &dep.package_root,
                        module_root,
                        project_root,
                    ) else {
                        continue;
                    };
                    version_info.insert(name, serde_json::Value::String(installed_version));
                }
            }

            // `GenerateLoaderJson.processUpdateVersionInfo()` creates the
            // package key before filtering version mismatches. Consequently a
            // ByteCode HAR with no mismatches is serialized as `{}` rather
            // than being omitted from `updateVersionInfo`.
            info.insert(
                dep.package_name.clone(),
                serde_json::Value::Object(version_info),
            );
        }

        info
    }

    #[cfg(test)]
    fn resolve_dependency_version(
        dep_name: &str,
        issuer_root: &Path,
        module_root: &Path,
        project_root: &Path,
    ) -> Option<String> {
        let candidates = [
            issuer_root.join("oh_modules").join(dep_name),
            module_root.join("oh_modules").join(dep_name),
            project_root.join("oh_modules").join(dep_name),
        ];

        let mut seen = FxHashSet::default();
        for candidate in candidates {
            if !candidate.is_dir() || !seen.insert(Self::path_key(&candidate)) {
                continue;
            }
            let Some(pkg) = OhPkgMinimal::load(&candidate) else {
                continue;
            };
            if let Some(version) = pkg.version.filter(|version| !version.trim().is_empty()) {
                return Some(version);
            }
        }

        None
    }
}

/// Builds ByteCode HAR loader fields.
pub struct LoaderByteCodeHarFields;

impl LoaderByteCodeHarFields {
    /// Create a loader field builder.
    pub fn new() -> Self {
        Self
    }

    /// Build hvigor's `loader.json#otherCompileFiles` contribution.
    pub fn collect_other_compile_files_from_graph(
        deps: &[ByteCodeHarScanEntry],
        graph: &arkdown_session::DependencyGraphSnapshot,
    ) -> Vec<PathBuf> {
        use arkdown_session::dependency_graph::snapshot::{DependencyEdgeKind, DependencyNodeType};

        let mut files = Vec::new();
        for dep in deps {
            let issuer_root = graph.canonical_package_root(&dep.package_root);
            let dependency_node = graph.nodes.values().find(|node| {
                node.package_root
                    .as_ref()
                    .is_some_and(|root| graph.canonical_package_root(root) == issuer_root)
            });
            for edge in graph.edges.iter().filter(|edge| {
                edge.kind != DependencyEdgeKind::DevDependency
                    && graph.canonical_package_root(&edge.issuer_root) == issuer_root
            }) {
                let Some(node) = graph.nodes.get(&edge.package_name) else {
                    continue;
                };
                if node.node_type == DependencyNodeType::BytecodeHar {
                    continue;
                }
                if let Some(entry) = node.compile_inputs.other_compile_entry_file.as_ref() {
                    files.push(entry.clone());
                }
                if node.node_type == DependencyNodeType::SourceHar {
                    files.extend(
                        node.compile_inputs
                            .other_compile_source_files
                            .iter()
                            .cloned(),
                    );
                }
            }
            for importee in &dep.extra_importees {
                if let Some(path) = dependency_node.and_then(|node| {
                    node.compile_inputs
                        .extra_importee_source_files
                        .get(importee)
                }) {
                    files.push(path.clone());
                }
            }
        }
        Self::dedupe_pathbufs(&mut files);
        files
    }

    /// Test-only filesystem fixture helper for synthetic layouts that do not
    /// construct a session dependency graph.
    #[cfg(test)]
    pub fn collect_other_compile_files(
        deps: &[ByteCodeHarScanEntry],
        module_root: &Path,
        project_root: &Path,
    ) -> Vec<PathBuf> {
        let mut files = Vec::new();

        for dep in deps {
            Self::collect_blocked_dependency_compile_files(
                dep,
                module_root,
                project_root,
                &mut files,
            );
        }
        for dep in deps {
            Self::collect_extra_importee_compile_files(dep, module_root, project_root, &mut files);
        }

        Self::dedupe_pathbufs(&mut files);
        files
    }

    /// 6.24.3 `pkgName2OtherCompileFiles`: transitively blocked dependency
    /// compile files grouped by the topmost blocking ByteCode HAR package.
    ///
    /// Mirrors the V2 chain of hvigor's `collectByteCodeInfoAndOtherCompileEntrances`
    /// (`processSourceCodeHarNodeV2` / `processNpmPackageNodeV2` /
    /// `collectExtraImporteesV2`): a BFS over the declared dependency graph
    /// tracks `topmostBlockedNode` — the nearest ByteCode HAR ancestor —
    /// and every source HAR / npm package reached below one contributes its
    /// entry and `src/main/ets` sources to that package's group.
    ///
    /// HSP dependencies, SO dependencies, and (by default) devDependencies
    /// are dropped from the walk entirely (`getFinalNodes`).
    pub fn collect_pkg_name2_other_compile_files_from_graph(
        graph: &arkdown_session::DependencyGraphSnapshot,
    ) -> IndexMap<String, Vec<PathBuf>> {
        use arkdown_session::dependency_graph::snapshot::{
            DependencyEdgeKind, DependencyNodeSnapshot, DependencyNodeType,
        };

        let nodes_by_root = graph
            .nodes
            .values()
            .filter_map(|node| {
                node.package_root
                    .as_ref()
                    .map(|root| (graph.canonical_package_root(root).into_owned(), node))
            })
            .collect::<std::collections::BTreeMap<PathBuf, &DependencyNodeSnapshot>>();
        let mut walkable_issuers = graph
            .edges
            .iter()
            .filter(|edge| !edge.is_transitive)
            .map(|edge| graph.canonical_package_root(&edge.issuer_root).into_owned())
            .collect::<FxHashSet<_>>();
        let mut blockers_by_issuer = std::collections::BTreeMap::<PathBuf, String>::new();
        let mut seen_roots = FxHashSet::default();
        let mut grouped = IndexMap::<String, Vec<PathBuf>>::new();

        for edge in &graph.edges {
            let issuer_root = graph.canonical_package_root(&edge.issuer_root).into_owned();
            if edge.kind == DependencyEdgeKind::DevDependency
                || !walkable_issuers.contains(&issuer_root)
            {
                continue;
            }
            let package_root = graph
                .canonical_package_root(&edge.package_root)
                .into_owned();
            if !seen_roots.insert(package_root.clone()) {
                continue;
            }
            let Some(node) = nodes_by_root.get(&package_root).copied() else {
                continue;
            };
            if matches!(
                node.node_type,
                DependencyNodeType::SharedHsp
                    | DependencyNodeType::IntegratedHsp
                    | DependencyNodeType::NativeSo
            ) {
                continue;
            }
            let inherited_blocker = blockers_by_issuer.get(&issuer_root).cloned();
            match node.node_type {
                DependencyNodeType::BytecodeHar => {
                    let blocker = inherited_blocker.unwrap_or_else(|| node.package_name.clone());
                    let files = grouped.entry(blocker.clone()).or_default();
                    if let Some(extra_importees) = graph
                        .widget_extra_importee_source_files
                        .get(&node.package_name)
                    {
                        files.extend(extra_importees.values().cloned());
                    }
                    walkable_issuers.insert(package_root.clone());
                    blockers_by_issuer.insert(package_root, blocker);
                }
                DependencyNodeType::SourceHar => {
                    if let Some(blocker) = inherited_blocker.as_ref() {
                        let files = grouped.entry(blocker.clone()).or_default();
                        files.extend(node.compile_inputs.other_compile_entry_file.iter().cloned());
                        files.extend(
                            node.compile_inputs
                                .other_compile_source_files
                                .iter()
                                .cloned(),
                        );
                    }
                    walkable_issuers.insert(package_root.clone());
                    if let Some(blocker) = inherited_blocker {
                        blockers_by_issuer.insert(package_root, blocker);
                    }
                }
                DependencyNodeType::OhpmSource => {
                    if let Some(blocker) = inherited_blocker {
                        grouped
                            .entry(blocker)
                            .or_default()
                            .extend(node.compile_inputs.other_compile_entry_file.iter().cloned());
                    }
                }
                DependencyNodeType::SharedHsp
                | DependencyNodeType::IntegratedHsp
                | DependencyNodeType::NativeSo
                | DependencyNodeType::Unknown => {}
            }
        }

        for files in grouped.values_mut() {
            Self::dedupe_pathbufs(files);
        }
        grouped
    }

    /// Test-only filesystem fixture helper for synthetic layouts that do not
    /// construct a session dependency graph.
    #[cfg(test)]
    pub fn collect_pkg_name2_other_compile_files(
        module_root: &Path,
        project_root: &Path,
    ) -> IndexMap<String, Vec<PathBuf>> {
        let mut grouped: IndexMap<String, Vec<PathBuf>> = IndexMap::new();
        let mut extra_importees: IndexMap<String, Vec<String>> = IndexMap::new();
        let mut queue = std::collections::VecDeque::<(String, PathBuf, Option<String>)>::new();
        let mut seen = FxHashSet::default();

        Self::enqueue_root_dependencies(module_root, &mut queue);
        if project_root != module_root {
            Self::enqueue_root_dependencies(project_root, &mut queue);
        }
        if queue.is_empty() {
            // hvigor starts from `getDirectPkgNodes()`; without declared
            // dependencies there is nothing to walk (no oh_modules fallback).
            return grouped;
        }

        while let Some((dep_name, issuer_root, topmost)) = queue.pop_front() {
            for pkg_dir in
                Self::candidate_dep_dirs(&dep_name, &issuer_root, module_root, project_root)
            {
                let root_key = ByteCodeHarScanEntry::path_key(&pkg_dir);
                if !seen.insert(root_key) {
                    continue;
                }

                let Some(pkg) = OhPkgMinimal::load(&pkg_dir) else {
                    continue;
                };
                let package_name = pkg.package_name(&dep_name).to_string();

                // `getFinalNodes`: HSP and SO dependencies are dropped from
                // the walk; devDependencies declared by the issuer are
                // dropped unless the `ohos.byteCodeHar.integratedOptimization`
                // inject is on.
                if Self::is_hsp_dependency(&pkg_dir) || Self::is_so_dependency(&dep_name) {
                    continue;
                }
                if OhPkgMinimal::load(&issuer_root)
                    .is_some_and(|issuer| issuer.dev_dependencies.contains_key(&dep_name))
                {
                    continue;
                }

                let is_har = ByteCodeHarDependencyProfile::is_har_dependency(&pkg_dir);
                if pkg.is_byte_code_har() {
                    // `processByteCodeHarNode`:
                    //   topmostBlockedNode ||= node
                    // then children inherit it. `isBundledDependency` nodes
                    // are handled by `processBundledNodeDependencies` with no
                    // topmost propagation; arkdown resolves bundled BC-HAR
                    // children through the same walk without a topmost.
                    let node_topmost = topmost.clone().or_else(|| Some(package_name.clone()));
                    Self::collect_extra_importees_v2(
                        &pkg,
                        node_topmost.as_deref(),
                        &mut extra_importees,
                    );
                    for child in pkg.dependency_names() {
                        queue.push_back((child, pkg_dir.clone(), node_topmost.clone()));
                    }
                } else {
                    // Source HAR or npm package reached below a BC-HAR
                    // ancestor: contribute to the topmost blocker's group.
                    if let Some(blocker) = topmost.as_deref() {
                        if is_har {
                            // `collectSourceCodeHarInfo`: entry file + the
                            // whole `src/main/ets` source tree.
                            if let Some(entry) = Self::resolve_package_entry_path(&pkg_dir, &pkg) {
                                grouped.entry(blocker.to_string()).or_default().push(entry);
                            }
                            let ets_root = ProfileModuleLayout::new(&pkg_dir).ets_source_root();
                            if ets_root.is_dir() {
                                grouped
                                    .entry(blocker.to_string())
                                    .or_default()
                                    .extend(Self::collect_code_files(&ets_root));
                            }
                        } else if let Some(entry) = Self::resolve_package_entry_path(&pkg_dir, &pkg)
                        {
                            // `collectEntryPath` for ordinary npm packages.
                            grouped.entry(blocker.to_string()).or_default().push(entry);
                        }
                    }
                    // `processNpmPackageNode` does not enqueue children; only
                    // source HARs keep walking the graph.
                    if is_har {
                        for child in pkg.dependency_names() {
                            queue.push_back((child, pkg_dir.clone(), topmost.clone()));
                        }
                    }
                }
            }
        }

        // `processExtraImporteesV2`: resolve each importee string through the
        // package dependency map into an entry or source file.
        for (blocker, importees) in extra_importees {
            let files = grouped.entry(blocker).or_default();
            for importee in importees {
                Self::collect_extra_importee_paths(&importee, module_root, project_root, files);
            }
        }

        for files in grouped.values_mut() {
            Self::dedupe_pathbufs(files);
        }
        grouped
    }

    #[cfg(test)]
    fn enqueue_root_dependencies(
        root: &Path,
        queue: &mut std::collections::VecDeque<(String, PathBuf, Option<String>)>,
    ) {
        let Some(pkg) = OhPkgMinimal::load(root) else {
            return;
        };
        for dep_name in pkg.dependency_names() {
            queue.push_back((dep_name, root.to_path_buf(), None));
        }
    }

    #[cfg(test)]
    #[cfg(test)]
    fn candidate_dep_dirs(
        dep_name: &str,
        issuer_root: &Path,
        module_root: &Path,
        project_root: &Path,
    ) -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        let mut candidates = vec![issuer_root.join("oh_modules").join(dep_name)];
        if let Some(store_root) = Self::issuer_store_oh_modules_root(issuer_root) {
            candidates.push(store_root.join("oh_modules").join(dep_name));
        }
        candidates.extend([
            module_root.join("oh_modules").join(dep_name),
            project_root.join("oh_modules").join(dep_name),
        ]);

        let mut seen = FxHashSet::default();
        for dir in candidates {
            if dir.is_dir() && seen.insert(ByteCodeHarScanEntry::path_key(&dir)) {
                dirs.push(dir);
            }
        }
        dirs
    }

    /// `getFinalNodes` drops HSP dependencies (`isHspDependency`).
    #[cfg(test)]
    #[cfg(test)]
    fn is_hsp_dependency(package_root: &Path) -> bool {
        let layout = ProfileModuleLayout::new(package_root);
        for path in [layout.module_json5(), layout.module_json()] {
            let Ok(content) = std::fs::read_to_string(path) else {
                continue;
            };
            let Ok(profile) = json5::from_str::<ByteCodeHarDependencyProfile>(&content) else {
                continue;
            };
            if profile
                .module
                .module_type
                .as_deref()
                .is_some_and(|module_type| module_type == "shared")
            {
                return true;
            }
        }
        false
    }

    /// `ValidateRegExp.SO_DEPENDENCY_REG_EXP = /lib(.+)\.so/`.
    #[cfg(test)]
    #[cfg(test)]
    fn is_so_dependency(dep_name: &str) -> bool {
        let Some(rest) = dep_name.strip_prefix("lib") else {
            return false;
        };
        !rest.is_empty() && rest.ends_with(".so")
    }

    /// `collectExtraImporteesV2`: a ByteCode HAR's `metadata.extraImportees`
    /// grouped under its topmost blocker's package name.
    ///
    /// Like hvigor's `getOrSetMapValue(extraImporteesV2, ...)`, an empty
    /// importee list still registers the group: `processExtraImporteesV2`
    /// then materializes the group in the compile-file map (hvigor writes it
    /// as an empty array into `pkgName2OtherCompileFiles`).
    #[cfg(test)]
    #[cfg(test)]
    fn collect_extra_importees_v2(
        pkg: &OhPkgMinimal,
        blocker: Option<&str>,
        out: &mut IndexMap<String, Vec<String>>,
    ) {
        let Some(blocker) = blocker else {
            return;
        };
        let Some(importees) = pkg
            .metadata
            .as_ref()
            .map(|metadata| metadata.extra_importees.clone())
        else {
            return;
        };
        let group = out.entry(blocker.to_string()).or_default();
        for importee in importees {
            if !group.contains(&importee) {
                group.push(importee);
            }
        }
    }

    /// `addExtraImporteesToOtherCompileFiles`: resolve one importee string
    /// (`@scope/pkg` or `@scope/pkg/path`) into an entry or source file.
    #[cfg(test)]
    #[cfg(test)]
    fn collect_extra_importee_paths(
        importee: &str,
        module_root: &Path,
        project_root: &Path,
        files: &mut Vec<PathBuf>,
    ) {
        let Some(specifier) = ExtraImporteeSpecifier::new(importee) else {
            return;
        };
        let package_name = specifier.package_name();
        let Some(pkg_root) =
            Self::resolve_dependency_root(package_name, module_root, module_root, project_root)
        else {
            return;
        };
        let Some(pkg) = OhPkgMinimal::load(&pkg_root) else {
            return;
        };

        let subpath = specifier.subpath();
        if subpath.as_os_str().is_empty() {
            if let Some(entry) = Self::resolve_package_entry_path(&pkg_root, &pkg) {
                files.push(entry);
            }
            return;
        }

        if let Some(path) = Self::find_supported_source_file(&pkg_root.join(subpath)) {
            files.push(path);
        }
    }

    /// Build hvigor's `loader.json#dynamicImportLibInfo` entries.
    pub fn collect_dynamic_import_lib_info_from_graph(
        module_name: &str,
        dynamic_dep_names: &[String],
        graph: &arkdown_session::DependencyGraphSnapshot,
    ) -> Vec<(String, DynamicImportLibEntry)> {
        use arkdown_session::dependency_graph::snapshot::DependencyNodeType;

        let mut results = Vec::new();
        for dep_name in dynamic_dep_names {
            let Some(edge) = graph
                .edges
                .iter()
                .find(|edge| edge.alias == *dep_name || edge.package_name == *dep_name)
            else {
                continue;
            };
            let Some(node) = graph.nodes.get(&edge.package_name) else {
                continue;
            };
            let Some(package_root) = node.package_root.as_ref() else {
                continue;
            };
            let package_name = node
                .package
                .as_ref()
                .map(|package| package.name.as_str())
                .filter(|name| !name.is_empty())
                .unwrap_or(&node.package_name);
            let resolved_module_name = if matches!(
                node.node_type,
                DependencyNodeType::SharedHsp | DependencyNodeType::IntegratedHsp
            ) {
                node.module_profile
                    .as_ref()
                    .and_then(|profile| profile.module_name.clone())
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| package_name.to_string())
            } else {
                package_name.to_string()
            };
            let entry_file_path = node
                .compile_inputs
                .dynamic_import_entry_file
                .as_ref()
                .map(|path| ByteCodeHarPathText::new(path).string())
                .unwrap_or_default();
            let is_local_dependency = edge
                .version_spec
                .as_deref()
                .is_some_and(|spec| PackageDependencySpec::new(spec).is_file_dependency());

            results.push((
                dep_name.clone(),
                DynamicImportLibEntry {
                    entry_file_path,
                    module_name: resolved_module_name,
                    is_local_dependency,
                    pkg_path: ByteCodeHarPathText::new(package_root).string(),
                    host_modules_info: vec![HostModuleInfo {
                        host_dependency_name: dep_name.clone(),
                        host_module_name: module_name.to_string(),
                    }],
                },
            ));
        }
        results
    }

    /// Test-only filesystem fixture helper for synthetic layouts that do not
    /// construct a session dependency graph.
    #[cfg(test)]
    pub fn collect_dynamic_import_lib_info(
        module_name: &str,
        dynamic_dep_names: &[String],
        module_root: &Path,
        project_root: &Path,
    ) -> Vec<(String, DynamicImportLibEntry)> {
        let mut results = Vec::new();

        for dep_name in dynamic_dep_names {
            let pkg_dir =
                DynamicImportLibEntry::resolve_oh_module_dir(dep_name, module_root, project_root);
            let Some(pkg_dir) = pkg_dir else {
                continue;
            };

            let is_local = DynamicImportLibEntry::is_local_dependency(dep_name, module_root);
            // Hvigor's `processOtherDynamicImport` always resolves the
            // selected `oh_modules` entry through `realpathSync`.  This is
            // observable for registry packages because OHPM exposes them as
            // symlinks into its content-addressed store.
            let pkg_dir = canonicalize_or_owned(&pkg_dir);
            let entry_file = DynamicImportLibEntry::find_entry_file(&pkg_dir);
            let pkg_module_name = DynamicImportLibEntry::read_module_name(&pkg_dir, dep_name);

            let entry = DynamicImportLibEntry {
                entry_file_path: entry_file,
                module_name: pkg_module_name,
                is_local_dependency: is_local,
                pkg_path: ByteCodeHarPathText::new(&pkg_dir).string(),
                host_modules_info: vec![HostModuleInfo {
                    host_dependency_name: dep_name.clone(),
                    host_module_name: module_name.to_string(),
                }],
            };
            results.push((dep_name.clone(), entry));
        }

        results
    }

    #[cfg(test)]
    fn collect_blocked_dependency_compile_files(
        dep: &ByteCodeHarScanEntry,
        module_root: &Path,
        project_root: &Path,
        files: &mut Vec<PathBuf>,
    ) {
        let Some(pkg) = OhPkgMinimal::load(&dep.package_root) else {
            return;
        };

        for child_name in pkg.dependency_names() {
            let Some(child_root) = Self::resolve_dependency_root(
                &child_name,
                &dep.package_root,
                module_root,
                project_root,
            ) else {
                continue;
            };
            let Some(child_pkg) = OhPkgMinimal::load(&child_root) else {
                continue;
            };

            if child_pkg.resolve_abc_path(&child_root).is_some() {
                continue;
            }

            if Self::is_source_har_package(&child_root) {
                if let Some(entry) = Self::resolve_package_entry_path(&child_root, &child_pkg) {
                    files.push(entry);
                }
                files.extend(Self::collect_code_files(
                    &ProfileModuleLayout::new(&child_root).ets_source_root(),
                ));
            } else if let Some(entry) = Self::resolve_package_entry_path(&child_root, &child_pkg) {
                files.push(entry);
            }
        }
    }

    #[cfg(test)]
    fn collect_extra_importee_compile_files(
        dep: &ByteCodeHarScanEntry,
        module_root: &Path,
        project_root: &Path,
        files: &mut Vec<PathBuf>,
    ) {
        for importee in &dep.extra_importees {
            let Some(importee_specifier) = ExtraImporteeSpecifier::new(importee) else {
                continue;
            };
            let package_name = importee_specifier.package_name();
            let Some(pkg_root) = Self::resolve_dependency_root(
                package_name,
                &dep.package_root,
                module_root,
                project_root,
            ) else {
                continue;
            };
            let Some(pkg) = OhPkgMinimal::load(&pkg_root) else {
                continue;
            };

            let subpath = importee_specifier.subpath();
            if subpath.as_os_str().is_empty() {
                if let Some(entry) = Self::resolve_package_entry_path(&pkg_root, &pkg) {
                    files.push(entry);
                }
                continue;
            }

            if let Some(path) = Self::find_supported_source_file(&pkg_root.join(subpath)) {
                files.push(path);
            }
        }
    }

    #[cfg(test)]
    fn resolve_dependency_root(
        dep_name: &str,
        issuer_root: &Path,
        module_root: &Path,
        project_root: &Path,
    ) -> Option<PathBuf> {
        let mut roots = Vec::new();
        if let Some(root) = Self::issuer_store_oh_modules_root(issuer_root) {
            roots.push(root);
        }
        roots.extend([
            issuer_root.join("oh_modules"),
            module_root.join("oh_modules"),
            project_root.join("oh_modules"),
        ]);

        let mut seen = FxHashSet::default();
        for root in roots {
            let candidate = root.join(dep_name);
            if candidate.is_dir() && seen.insert(ByteCodeHarScanEntry::path_key(&candidate)) {
                return Some(candidate);
            }
        }
        None
    }

    #[cfg(test)]
    fn issuer_store_oh_modules_root(issuer_root: &Path) -> Option<PathBuf> {
        let mut current = issuer_root.parent();
        while let Some(path) = current {
            if path.file_name().is_some_and(|name| name == "oh_modules") {
                return Some(path.to_path_buf());
            }
            current = path.parent();
        }
        None
    }

    #[cfg(test)]
    fn is_source_har_package(pkg_root: &Path) -> bool {
        let layout = ProfileModuleLayout::new(pkg_root);
        layout.module_json5().is_file() || layout.module_json().is_file()
    }

    #[cfg(test)]
    fn resolve_package_entry_path(pkg_root: &Path, pkg: &OhPkgMinimal) -> Option<PathBuf> {
        if let Some(main) = pkg.main.as_deref().filter(|value| !value.is_empty()) {
            let candidate = pkg_root.join(main);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        if let Some(types) = pkg.types.as_deref().filter(|value| !value.is_empty()) {
            let candidate = pkg_root.join(types);
            if candidate.is_file() && Self::is_code_file(&candidate) {
                return Some(candidate);
            }
        }

        for rel in ["src/main/ets/index", "src/main/ets/Index", "index", "Index"] {
            if let Some(path) = Self::find_supported_source_file(&pkg_root.join(rel)) {
                return Some(path);
            }
        }
        None
    }

    #[cfg(test)]
    fn find_supported_source_file(base: &Path) -> Option<PathBuf> {
        if base.is_file() && Self::is_code_file(base) {
            return Some(base.to_path_buf());
        }
        for ext in ["ets", "ts", "js"] {
            let candidate = base.with_extension(ext);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        None
    }

    #[cfg(test)]
    fn collect_code_files(root: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let mut directories = std::collections::VecDeque::from([root.to_path_buf()]);
        while let Some(dir) = directories.pop_front() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            let mut entries: Vec<_> = entries.flatten().collect();
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries {
                let path = entry.path();
                if path.is_dir() {
                    directories.push_back(path);
                } else if path.is_file() && Self::is_code_file(&path) {
                    files.push(path);
                }
            }
        }
        files
    }

    #[cfg(test)]
    fn is_code_file(path: &Path) -> bool {
        if SourceFileName::new(path).is_declaration() {
            return false;
        }
        matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some("ets" | "ts" | "js")
        )
    }

    fn dedupe_pathbufs(paths: &mut Vec<PathBuf>) {
        let mut seen = FxHashSet::default();
        paths.retain(|path| seen.insert(ByteCodeHarScanEntry::path_key(path)));
    }
}

#[cfg(test)]
struct ExtraImporteeSpecifier {
    package_name: String,
    subpath: PathBuf,
}

#[cfg(test)]
impl ExtraImporteeSpecifier {
    fn new(importee: &str) -> Option<Self> {
        let candidates = ModuleSpecifierText::new(importee).package_candidates();
        let candidate = candidates.last()?;
        Some(Self {
            package_name: candidate.package_name().to_string(),
            subpath: Path::new(candidate.subpath()).to_path_buf(),
        })
    }

    fn package_name(&self) -> &str {
        &self.package_name
    }

    fn subpath(&self) -> &Path {
        &self.subpath
    }
}

impl Default for LoaderByteCodeHarFields {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Dynamic import lib info
// ---------------------------------------------------------------------------

/// Entry written to `loader.json` `dynamicImportLibInfo`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicImportLibEntry {
    /// Absolute path to the package's main entry file.
    pub entry_file_path: String,
    /// Module name from the package's `module.json` (or package name fallback).
    pub module_name: String,
    /// Whether this is a local (file:) dependency.
    pub is_local_dependency: bool,
    /// Absolute path to the package root directory.
    pub pkg_path: String,
    /// Host module info (which module declares this dynamic import).
    pub host_modules_info: Vec<HostModuleInfo>,
}

/// Identifies the host module that declares a dynamic import.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostModuleInfo {
    pub host_dependency_name: String,
    pub host_module_name: String,
}

#[cfg(test)]
impl DynamicImportLibEntry {
    // -----------------------------------------------------------------------
    // Private resolution helpers
    // -----------------------------------------------------------------------

    fn resolve_oh_module_dir(
        dep_name: &str,
        module_root: &Path,
        project_root: &Path,
    ) -> Option<PathBuf> {
        for base in &[
            module_root.join("oh_modules"),
            project_root.join("oh_modules"),
        ] {
            let candidate = base.join(dep_name);
            if candidate.exists() && candidate.is_dir() {
                return Some(candidate);
            }
        }
        None
    }

    fn is_local_dependency(dep_name: &str, module_root: &Path) -> bool {
        let pkg_json = module_root.join("oh-package.json5");
        let Ok(content) = std::fs::read_to_string(&pkg_json) else {
            return false;
        };

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct PkgDeps {
            #[serde(default)]
            dependencies: FxHashMap<String, String>,
            #[serde(default)]
            dynamic_dependencies: FxHashMap<String, String>,
        }

        let Ok(parsed) = json5::from_str::<PkgDeps>(&content) else {
            return false;
        };

        parsed
            .dynamic_dependencies
            .get(dep_name)
            .or_else(|| parsed.dependencies.get(dep_name))
            .map(|value| PackageDependencySpec::new(value).is_file_dependency())
            .unwrap_or(false)
    }

    fn find_entry_file(pkg_dir: &Path) -> String {
        let layout = ProfileModuleLayout::new(pkg_dir);
        let pkg_json = layout.oh_package_json5();
        if let Ok(content) = std::fs::read_to_string(&pkg_json) {
            #[derive(Deserialize)]
            struct PkgEntry {
                #[serde(default)]
                main: Option<String>,
                #[serde(default)]
                types: Option<String>,
            }
            if let Ok(parsed) = json5::from_str::<PkgEntry>(&content) {
                if let Some(main) = parsed.main.filter(|s| !s.is_empty()) {
                    let candidate = pkg_dir.join(&main);
                    if candidate.exists() {
                        return ByteCodeHarPathText::new(&candidate).string();
                    }
                }
                if let Some(types) = parsed.types.filter(|s| !s.is_empty()) {
                    let candidate = pkg_dir.join(&types);
                    if candidate.exists() {
                        return ByteCodeHarPathText::new(&candidate).string();
                    }
                }
            }
        }

        // Fall back to conventional entry points.
        for candidate_name in &["index.ets", "index.ts", "index.js"] {
            let candidate = layout.ets_source_root().join(candidate_name);
            if candidate.exists() {
                return ByteCodeHarPathText::new(&candidate).string();
            }
            let candidate = pkg_dir.join(candidate_name);
            if candidate.exists() {
                return ByteCodeHarPathText::new(&candidate).string();
            }
        }

        String::new()
    }

    fn read_module_name(pkg_dir: &Path, fallback: &str) -> String {
        #[derive(Deserialize)]
        struct PackageJson {
            name: Option<String>,
        }
        let package_name =
            std::fs::read_to_string(ProfileModuleLayout::new(pkg_dir).oh_package_json5())
                .ok()
                .and_then(|content| json5::from_str::<PackageJson>(&content).ok())
                .and_then(|package| package.name)
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| fallback.to_string());

        // `generate-loader-json.ts::processOtherDynamicImport` retains the
        // package name for HARs and only substitutes `module.name` for HSPs.
        let layout = ProfileModuleLayout::new(pkg_dir);
        for path in [layout.module_json(), layout.module_json5()] {
            if let Ok(content) = std::fs::read_to_string(&path) {
                #[derive(Deserialize)]
                struct ModuleJson {
                    module: ModuleSection,
                }
                #[derive(Deserialize)]
                struct ModuleSection {
                    name: Option<String>,
                    #[serde(rename = "type")]
                    module_type: Option<String>,
                }
                if let Ok(parsed) = json5::from_str::<ModuleJson>(&content) {
                    if parsed.module.module_type.as_deref() == Some("shared") {
                        if let Some(name) = parsed.module.name.filter(|s| !s.is_empty()) {
                            return name;
                        }
                    }
                }
            }
        }
        package_name
    }
}

struct ByteCodeHarPathText<'a> {
    path: &'a Path,
}

impl<'a> ByteCodeHarPathText<'a> {
    fn new(path: &'a Path) -> Self {
        Self { path }
    }

    fn string(&self) -> String {
        ArtifactPathSerializer::path_to_slash_string(self.path)
            .unwrap_or_else(|_| self.path.display().to_string())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("create temp dir")
    }

    fn write_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        fs::write(path, content).expect("write file");
    }

    fn write_abc(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        fs::write(path, b"PANDA\x00\x00\x00").expect("write abc");
    }

    #[cfg(unix)]
    #[test]
    fn graph_loader_fields_use_captured_roots_after_symlink_removal() {
        use std::os::unix::fs::symlink;
        let tmp = tempfile::tempdir().unwrap();
        let issuer = tmp.path().join("Issuer");
        let dependency = issuer.join("oh_modules/leaf");
        write_file(
            &issuer.join("oh-package.json5"),
            r#"{name:'issuer', dependencies:{leaf:'1.0.0'}}"#,
        );
        write_file(
            &dependency.join("oh-package.json5"),
            r#"{name:'leaf', version:'1.2.3', main:'Index.ets'}"#,
        );
        write_file(&dependency.join("Index.ets"), "export const x = 1;");
        let alias = tmp.path().join("Alias");
        symlink(&issuer, &alias).unwrap();
        let graph = arkdown_session::dependency_graph::builder::DependencyGraphBuilder::new(
            "issuer",
            tmp.path(),
            &alias,
        )
        .build();
        let expected_entry = graph.nodes["leaf"]
            .compile_inputs
            .other_compile_entry_file
            .clone()
            .unwrap();
        std::fs::remove_file(&alias).unwrap();
        assert_eq!(
            ByteCodeHarScanEntry::dependency_version_from_graph("leaf", &alias, &graph).as_deref(),
            Some("1.2.3")
        );
        let dep = ByteCodeHarScanEntry {
            package_name: "issuer".into(),
            package_root: alias,
            abc_path: issuer.join("ets/modules.abc"),
            is_har_dependency: true,
            compatible_sdk_version: None,
            compatible_sdk_version_stage: None,
            source_maps_path: None,
            insight_intent: None,
            declaration_entry: Vec::new(),
            extra_importees: Vec::new(),
            main_entry: None,
        };
        assert_eq!(
            LoaderByteCodeHarFields::collect_other_compile_files_from_graph(&[dep], &graph),
            vec![expected_entry]
        );
    }

    #[test]
    fn test_scan_bytecode_har_empty_oh_modules() {
        let tmp = make_temp_dir();
        let results =
            ByteCodeHarMetadataWriter::scan_bytecode_har_deps_from_graph(tmp.path(), tmp.path());
        assert!(results.is_empty());
    }

    #[test]
    fn typed_empty_bytecode_har_inventory_is_authoritative() {
        let tmp = make_temp_dir();
        let pkg_dir = tmp.path().join("oh_modules/bytecode-har");
        write_abc(&pkg_dir.join("ets/modules.abc"));
        write_file(
            &pkg_dir.join("oh-package.json5"),
            r#"{ name: "bytecode-har", metadata: { byteCodeHar: true } }"#,
        );
        assert_eq!(
            ByteCodeHarMetadataWriter::scan_bytecode_har_deps_from_graph(tmp.path(), tmp.path())
                .len(),
            1
        );
        assert!(
            ByteCodeHarMetadataWriter::scan_from_dependency_graph(
                &arkdown_session::DependencyGraphSnapshot::default()
            )
            .is_empty(),
            "GenerateLoaderJson must not revive dependencies excluded by the typed hvigor view"
        );
    }

    #[test]
    fn test_scan_bytecode_har_ignores_unmarked_abc_package() {
        let tmp = make_temp_dir();
        let oh_modules = tmp.path().join("oh_modules");
        let pkg_dir = oh_modules.join("@ohos").join("test-har");

        write_abc(&pkg_dir.join("index.abc"));
        write_file(
            &pkg_dir.join("oh-package.json5"),
            r#"{ name: "@ohos/test-har", compatibleSdkVersion: "5.0.0(12)" }"#,
        );

        let results =
            ByteCodeHarMetadataWriter::scan_bytecode_har_deps_from_graph(tmp.path(), tmp.path());
        assert!(results.is_empty());
    }

    #[test]
    fn test_scan_bytecode_har_uses_hvigor_bytecode_marker() {
        let tmp = make_temp_dir();
        let oh_modules = tmp.path().join("oh_modules");
        let pkg_dir = oh_modules.join("bytecode-har");

        write_abc(&pkg_dir.join("ets").join("modules.abc"));
        write_file(&pkg_dir.join("ets").join("sourceMaps.map"), "{}");
        write_file(
            &pkg_dir.join("oh-package.json5"),
            r#"{
              name: "bytecode-har",
              compatibleSdkVersion: 12,
              metadata: {
                byteCodeHar: true,
                declarationEntry: [
                  "@normalized:N&entry&com.example&bytecode-har/src/main/ets/Worker&1.0.0"
                ]
              }
            }"#,
        );

        let results =
            ByteCodeHarMetadataWriter::scan_bytecode_har_deps_from_graph(tmp.path(), tmp.path());
        assert_eq!(results.len(), 1);
        assert!(results[0].abc_path.ends_with("ets/modules.abc"));
        assert!(results[0]
            .source_maps_path
            .as_ref()
            .is_some_and(|path| path.ends_with("ets/sourceMaps.map")));
        assert_eq!(
            results[0].declaration_entry,
            vec!["@normalized:N&entry&com.example&bytecode-har/src/main/ets/Worker&1.0.0"]
        );
    }

    #[test]
    fn test_scan_static_bytecode_har_preserves_static_abc_path() {
        let tmp = make_temp_dir();
        let pkg_dir = tmp.path().join("oh_modules/static-bytecode-har");
        write_abc(&pkg_dir.join("ets/modules_static.abc"));
        write_file(
            &pkg_dir.join("oh-package.json5"),
            r#"{
              name: "static-bytecode-har",
              metadata: { byteCodeHar: true }
            }"#,
        );

        let results =
            ByteCodeHarMetadataWriter::scan_bytecode_har_deps_from_graph(tmp.path(), tmp.path());

        assert_eq!(results.len(), 1);
        assert!(results[0].abc_path.ends_with("ets/modules_static.abc"));
        let (_, loader_entry) = results.into_iter().next().unwrap().into_loader_entry();
        assert!(loader_entry.abc_path.ends_with("ets/modules_static.abc"));
    }

    #[test]
    fn test_scan_bytecode_har_finds_source_maps() {
        let tmp = make_temp_dir();
        let oh_modules = tmp.path().join("oh_modules");
        let pkg_dir = oh_modules.join("my-har");

        write_abc(&pkg_dir.join("index.abc"));
        write_file(&pkg_dir.join("src").join("sourceMaps.map"), "{}");
        write_file(
            &pkg_dir.join("oh-package.json5"),
            r#"{
              name: "my-har",
              compatibleSdkVersion: 12,
              metadata: { byteCodeHar: true }
            }"#,
        );

        let results =
            ByteCodeHarMetadataWriter::scan_bytecode_har_deps_from_graph(tmp.path(), tmp.path());
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].compatible_sdk_version,
            Some(CompatibleSdkVersionValue::Int(12))
        );
        assert!(results[0].source_maps_path.is_some());
    }

    #[test]
    fn test_scan_bytecode_har_uses_metadata_paths() {
        let tmp = make_temp_dir();
        let oh_modules = tmp.path().join("oh_modules");
        let pkg_dir = oh_modules.join("metadata-har");

        write_abc(&pkg_dir.join("dist").join("metadata.abc"));
        write_file(&pkg_dir.join("maps").join("sourceMaps.map"), "{}");
        write_file(&pkg_dir.join("insight").join("intent.json"), "{}");
        write_file(
            &pkg_dir.join("oh-package.json5"),
            r#"{
              name: "metadata-har",
              compatibleSdkVersion: "5.0.0(12)",
              metadata: {
                byteCodeHar: true,
                abcPath: "dist/metadata.abc",
                sourceMapsPath: "maps/sourceMaps.map",
                insightIntentPath: "insight/intent.json"
              }
            }"#,
        );

        let results =
            ByteCodeHarMetadataWriter::scan_bytecode_har_deps_from_graph(tmp.path(), tmp.path());
        assert_eq!(results.len(), 1);
        assert!(results[0].abc_path.ends_with("dist/metadata.abc"));
        assert!(results[0]
            .source_maps_path
            .as_ref()
            .is_some_and(|path| path.ends_with("maps/sourceMaps.map")));
        assert!(results[0]
            .insight_intent
            .as_ref()
            .is_some_and(|path| path.ends_with("insight/intent.json")));

        let (_, loader_entry) = results[0].clone().into_loader_entry();
        assert!(loader_entry.abc_path.ends_with("dist/metadata.abc"));
        assert!(loader_entry
            .source_maps_path
            .as_ref()
            .is_some_and(|path| path.ends_with("maps/sourceMaps.map")));
        assert!(loader_entry
            .insight_intent
            .as_ref()
            .is_some_and(|path| path.ends_with("insight/intent.json")));
    }

    #[test]
    fn test_scan_bytecode_har_skips_non_abc_packages() {
        let tmp = make_temp_dir();
        let oh_modules = tmp.path().join("oh_modules");
        let pkg_dir = oh_modules.join("source-har");

        // No index.abc — this is a source HAR, not a ByteCode HAR.
        write_file(
            &pkg_dir.join("oh-package.json5"),
            r#"{ name: "source-har" }"#,
        );
        write_file(
            &pkg_dir.join("src/main/ets/index.ets"),
            "export default class Foo {}",
        );

        let results =
            ByteCodeHarMetadataWriter::scan_bytecode_har_deps_from_graph(tmp.path(), tmp.path());
        assert!(results.is_empty());
    }

    #[test]
    fn test_scan_bytecode_har_deduplicates_module_and_project() {
        let tmp = make_temp_dir();
        let module_dir = tmp.path().join("entry");
        let project_dir = tmp.path();

        // Same package in both module oh_modules and project oh_modules.
        for base in &[
            module_dir.join("oh_modules"),
            project_dir.join("oh_modules"),
        ] {
            let pkg = base.join("shared-har");
            write_abc(&pkg.join("index.abc"));
            write_file(
                &pkg.join("oh-package.json5"),
                r#"{ name: "shared-har", metadata: { byteCodeHar: true } }"#,
            );
        }

        let results =
            ByteCodeHarMetadataWriter::scan_bytecode_har_deps_from_graph(&module_dir, project_dir);
        assert_eq!(results.len(), 1, "same package must not appear twice");
    }

    #[test]
    fn test_scan_bytecode_har_order_is_deterministic() {
        let tmp = make_temp_dir();
        let oh_modules = tmp.path().join("oh_modules");

        for pkg_name in ["z-har", "a-har", "m-har"] {
            let pkg = oh_modules.join(pkg_name);
            write_abc(&pkg.join("ets").join("modules.abc"));
            write_file(
                &pkg.join("oh-package.json5"),
                &format!(r#"{{ name: "{pkg_name}", metadata: {{ byteCodeHar: true }} }}"#),
            );
        }

        let results =
            ByteCodeHarMetadataWriter::scan_bytecode_har_deps_from_graph(tmp.path(), tmp.path());
        let names: Vec<_> = results
            .iter()
            .map(|entry| entry.package_name.as_str())
            .collect();
        assert_eq!(names, vec!["a-har", "m-har", "z-har"]);
    }

    #[test]
    fn test_scan_bytecode_har_follows_declared_dependencies_only() {
        let tmp = make_temp_dir();
        let oh_modules = tmp.path().join("oh_modules");

        write_file(
            &tmp.path().join("oh-package.json5"),
            r#"{ dependencies: { "used-har": "1.0.0" } }"#,
        );

        for pkg_name in ["used-har", "unused-har"] {
            let pkg = oh_modules.join(pkg_name);
            write_abc(&pkg.join("ets").join("modules.abc"));
            write_file(
                &pkg.join("oh-package.json5"),
                &format!(r#"{{ name: "{pkg_name}", metadata: {{ byteCodeHar: true }} }}"#),
            );
        }

        let results =
            ByteCodeHarMetadataWriter::scan_bytecode_har_deps_from_graph(tmp.path(), tmp.path());
        let names: Vec<_> = results
            .iter()
            .map(|entry| entry.package_name.as_str())
            .collect();
        assert_eq!(names, vec!["used-har"]);
    }

    #[test]
    fn test_scan_bytecode_har_follows_transitive_dependencies() {
        let tmp = make_temp_dir();
        let oh_modules = tmp.path().join("oh_modules");
        let source_har = oh_modules.join("source-har");
        let bytecode_har = oh_modules.join("bytecode-har");

        write_file(
            &tmp.path().join("oh-package.json5"),
            r#"{ dependencies: { "source-har": "1.0.0" } }"#,
        );
        write_file(
            &source_har.join("oh-package.json5"),
            r#"{ name: "source-har", dependencies: { "bytecode-har": "1.0.0" } }"#,
        );
        write_abc(&bytecode_har.join("ets").join("modules.abc"));
        write_file(
            &bytecode_har.join("oh-package.json5"),
            r#"{ name: "bytecode-har", metadata: { byteCodeHar: true } }"#,
        );

        let results =
            ByteCodeHarMetadataWriter::scan_bytecode_har_deps_from_graph(tmp.path(), tmp.path());
        let names: Vec<_> = results
            .iter()
            .map(|entry| entry.package_name.as_str())
            .collect();
        assert_eq!(names, vec!["bytecode-har"]);
    }

    #[test]
    fn test_scan_bytecode_har_uses_package_json_name() {
        let tmp = make_temp_dir();
        let alias_dir = tmp.path().join("oh_modules").join("alias-har");

        write_file(
            &tmp.path().join("oh-package.json5"),
            r#"{ dependencies: { "alias-har": "file:../alias-har" } }"#,
        );
        write_abc(&alias_dir.join("ets").join("modules.abc"));
        write_file(
            &alias_dir.join("oh-package.json5"),
            r#"{ name: "actual-har", metadata: { byteCodeHar: true } }"#,
        );

        let results =
            ByteCodeHarMetadataWriter::scan_bytecode_har_deps_from_graph(tmp.path(), tmp.path());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].package_name, "actual-har");
    }

    #[test]
    fn test_collect_update_version_info_uses_dependency_pkg_version_mismatch() {
        let tmp = make_temp_dir();
        let oh_modules = tmp.path().join("oh_modules");
        let bytecode_har = oh_modules.join("bytecode-har");

        write_file(
            &tmp.path().join("oh-package.json5"),
            r#"{ dependencies: { "bytecode-har": "1.0.0" } }"#,
        );
        write_abc(&bytecode_har.join("ets").join("modules.abc"));
        write_file(
            &bytecode_har.join("oh-package.json5"),
            r#"{
              name: "bytecode-har",
              metadata: {
                byteCodeHar: true,
                dependencyPkgVersion: {
                  "dep-a": "1.0.0",
                  "dep-b": "2.0.0"
                }
              }
            }"#,
        );
        write_file(
            &bytecode_har.join("src/main/module.json5"),
            r#"{ module: { name: "bytecode-har", type: "har" } }"#,
        );
        write_file(
            &oh_modules.join("dep-a").join("oh-package.json5"),
            r#"{ name: "dep-a", version: "1.1.0" }"#,
        );
        write_file(
            &oh_modules.join("dep-b").join("oh-package.json5"),
            r#"{ name: "dep-b", version: "2.0.0" }"#,
        );

        let deps =
            ByteCodeHarMetadataWriter::scan_bytecode_har_deps_from_graph(tmp.path(), tmp.path());
        let info = ByteCodeHarScanEntry::collect_bytecode_har_update_version_info(
            &deps,
            tmp.path(),
            tmp.path(),
        );
        assert_eq!(
            info.get("bytecode-har").and_then(|v| v.get("dep-a")),
            Some(&serde_json::Value::String("1.1.0".to_string()))
        );
        assert!(
            info.get("bytecode-har")
                .and_then(|v| v.get("dep-b"))
                .is_none(),
            "matching dependencyPkgVersion entries must not be emitted"
        );
    }

    #[test]
    fn test_collect_update_version_info_keeps_empty_bytecode_har_entry() {
        let tmp = make_temp_dir();
        let bytecode_har = tmp.path().join("oh_modules").join("bytecode-har");

        write_file(
            &tmp.path().join("oh-package.json5"),
            r#"{ dependencies: { "bytecode-har": "1.0.0" } }"#,
        );
        write_abc(&bytecode_har.join("ets").join("modules.abc"));
        write_file(
            &bytecode_har.join("oh-package.json5"),
            r#"{
              name: "bytecode-har",
              metadata: { byteCodeHar: true, dependencyPkgVersion: {} }
            }"#,
        );
        write_file(
            &bytecode_har.join("src/main/module.json5"),
            r#"{ module: { name: "bytecode-har", type: "har" } }"#,
        );

        let deps =
            ByteCodeHarMetadataWriter::scan_bytecode_har_deps_from_graph(tmp.path(), tmp.path());
        let info = ByteCodeHarScanEntry::collect_bytecode_har_update_version_info(
            &deps,
            tmp.path(),
            tmp.path(),
        );

        assert_eq!(
            info.get("bytecode-har"),
            Some(&serde_json::json!({})),
            "Hvigor initializes every ByteCode HAR updateVersionInfo entry before filtering"
        );
    }

    #[test]
    fn test_collect_update_version_info_omits_bytecode_other_dependency() {
        let tmp = make_temp_dir();
        let bytecode_package = tmp.path().join("oh_modules").join("bytecode-package");

        write_file(
            &tmp.path().join("oh-package.json5"),
            r#"{ dependencies: { "bytecode-package": "1.0.0" } }"#,
        );
        write_abc(&bytecode_package.join("ets").join("modules.abc"));
        write_file(
            &bytecode_package.join("oh-package.json5"),
            r#"{
              name: "bytecode-package",
              metadata: { byteCodeHar: true, dependencyPkgVersion: {} }
            }"#,
        );

        let deps =
            ByteCodeHarMetadataWriter::scan_bytecode_har_deps_from_graph(tmp.path(), tmp.path());
        assert!(deps
            .iter()
            .any(|dep| dep.package_name == "bytecode-package"));

        let info = ByteCodeHarScanEntry::collect_bytecode_har_update_version_info(
            &deps,
            tmp.path(),
            tmp.path(),
        );
        assert!(
            !info.contains_key("bytecode-package"),
            "GenerateLoaderJson.processUpdateVersionInfo only traverses HAR-typed ByteCode dependencies"
        );
    }

    #[test]
    fn test_collect_update_version_info_uses_declared_dependency_versions() {
        let tmp = make_temp_dir();
        let oh_modules = tmp.path().join("oh_modules");
        let bytecode_har = oh_modules.join("bytecode-har");

        write_file(
            &tmp.path().join("oh-package.json5"),
            r#"{ dependencies: { "bytecode-har": "1.0.0" } }"#,
        );
        write_abc(&bytecode_har.join("ets").join("modules.abc"));
        write_file(
            &bytecode_har.join("oh-package.json5"),
            r#"{
              name: "bytecode-har",
              dependencies: { "dep-a": "1.0.0" },
              dynamicDependencies: { "dep-b": "2.0.0" },
              metadata: { byteCodeHar: true }
            }"#,
        );
        write_file(
            &bytecode_har.join("src/main/module.json5"),
            r#"{ module: { name: "bytecode-har", type: "har" } }"#,
        );
        write_file(
            &oh_modules.join("dep-a").join("oh-package.json5"),
            r#"{ name: "dep-a", version: "1.0.1" }"#,
        );
        write_file(
            &oh_modules.join("dep-b").join("oh-package.json5"),
            r#"{ name: "dep-b", version: "2.0.1" }"#,
        );

        let deps =
            ByteCodeHarMetadataWriter::scan_bytecode_har_deps_from_graph(tmp.path(), tmp.path());
        let info = ByteCodeHarScanEntry::collect_bytecode_har_update_version_info(
            &deps,
            tmp.path(),
            tmp.path(),
        );
        assert_eq!(
            info.get("bytecode-har").and_then(|v| v.get("dep-a")),
            Some(&serde_json::Value::String("1.0.1".to_string()))
        );
        assert_eq!(
            info.get("bytecode-har").and_then(|v| v.get("dep-b")),
            Some(&serde_json::Value::String("2.0.1".to_string()))
        );
    }

    #[test]
    fn test_collect_other_compile_files_uses_extra_importees() {
        let tmp = make_temp_dir();
        let oh_modules = tmp.path().join("oh_modules");
        let bytecode_har = oh_modules.join("bytecode-har");
        let extra_pkg = oh_modules.join("extra-pkg");

        write_file(
            &tmp.path().join("oh-package.json5"),
            r#"{ dependencies: { "bytecode-har": "1.0.0" } }"#,
        );
        write_abc(&bytecode_har.join("ets").join("modules.abc"));
        write_file(
            &bytecode_har.join("oh-package.json5"),
            r#"{
              name: "bytecode-har",
              metadata: {
                byteCodeHar: true,
                extraImportees: ["extra-pkg", "extra-pkg/src/feature"]
              }
            }"#,
        );
        write_file(
            &extra_pkg.join("oh-package.json5"),
            r#"{ name: "extra-pkg", main: "src/main.ets" }"#,
        );
        write_file(&extra_pkg.join("src/main.ets"), "export default 1;\n");
        write_file(&extra_pkg.join("src/feature.ets"), "export default 2;\n");

        let deps =
            ByteCodeHarMetadataWriter::scan_bytecode_har_deps_from_graph(tmp.path(), tmp.path());
        let files =
            LoaderByteCodeHarFields::collect_other_compile_files(&deps, tmp.path(), tmp.path());
        assert!(
            files.iter().any(|path| path.ends_with("src/main.ets")),
            "package extraImportee must resolve to package entry"
        );
        assert!(
            files.iter().any(|path| path.ends_with("src/feature.ets")),
            "subpath extraImportee must resolve with supported extension probing"
        );
    }

    #[test]
    fn test_collect_other_compile_files_uses_blocked_source_har_dependency() {
        let tmp = make_temp_dir();
        let oh_modules = tmp.path().join("oh_modules");
        let bytecode_har = oh_modules.join("bytecode-har");
        let source_har = oh_modules.join("source-har");

        write_file(
            &tmp.path().join("oh-package.json5"),
            r#"{ dependencies: { "bytecode-har": "1.0.0" } }"#,
        );
        write_abc(&bytecode_har.join("ets").join("modules.abc"));
        write_file(
            &bytecode_har.join("oh-package.json5"),
            r#"{
              name: "bytecode-har",
              dependencies: { "source-har": "1.0.0" },
              metadata: { byteCodeHar: true }
            }"#,
        );
        write_file(
            &source_har.join("oh-package.json5"),
            r#"{ name: "source-har", main: "src/main/ets/index.ets" }"#,
        );
        write_file(
            &source_har.join("src/main/module.json5"),
            r#"{ module: { name: "source", type: "har" } }"#,
        );
        write_file(
            &source_har.join("src/main/ets/index.ets"),
            "export default 1;\n",
        );
        write_file(
            &source_har.join("src/main/ets/pages/Page.ets"),
            "export default 2;\n",
        );
        write_file(
            &source_har.join("src/main/ets/index.d.ts"),
            "export type Ignored = string;\n",
        );

        let deps =
            ByteCodeHarMetadataWriter::scan_bytecode_har_deps_from_graph(tmp.path(), tmp.path());
        let files =
            LoaderByteCodeHarFields::collect_other_compile_files(&deps, tmp.path(), tmp.path());
        assert!(
            files
                .iter()
                .any(|path| path.ends_with("src/main/ets/index.ets")),
            "source HAR entry must be included"
        );
        assert!(
            files
                .iter()
                .any(|path| path.ends_with("src/main/ets/pages/Page.ets")),
            "source HAR ets directory files must be included"
        );
        assert!(
            !files
                .iter()
                .any(|path| path.ends_with("src/main/ets/index.d.ts")),
            "declaration files must be filtered like hvigor processCodeFiles"
        );
        let source_files = files
            .iter()
            .filter_map(|path| {
                path.strip_prefix(&source_har)
                    .ok()
                    .map(ArtifactPathSerializer::rel_to_slash)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            source_files,
            ["src/main/ets/index.ets", "src/main/ets/pages/Page.ets",],
            "GenerateLoaderJson.collectCompileFiles uses a breadth-first queue"
        );
    }

    #[test]
    fn test_collect_other_compile_files_uses_ohpm_store_dependency_root() {
        let tmp = make_temp_dir();
        let module_root = tmp.path().join("entry");
        let store_oh_modules = tmp
            .path()
            .join("oh_modules/.ohpm/bytecode-har@hash/oh_modules");
        let bytecode_har = store_oh_modules.join("bytecode-har");
        let source_har = store_oh_modules.join("source-har");

        write_abc(&bytecode_har.join("ets").join("modules.abc"));
        write_file(
            &bytecode_har.join("oh-package.json5"),
            r#"{
              name: "bytecode-har",
              dependencies: { "source-har": "1.0.0" },
              metadata: { byteCodeHar: true }
            }"#,
        );
        write_file(
            &source_har.join("oh-package.json5"),
            r#"{ name: "source-har", main: "src/main/ets/index.ets" }"#,
        );
        write_file(
            &source_har.join("src/main/module.json5"),
            r#"{ module: { name: "source", type: "har" } }"#,
        );
        write_file(
            &source_har.join("src/main/ets/index.ets"),
            "export default 1;\n",
        );

        let deps = vec![ByteCodeHarScanEntry {
            package_name: "bytecode-har".to_string(),
            package_root: bytecode_har.clone(),
            abc_path: bytecode_har.join("ets").join("modules.abc"),
            is_har_dependency: true,
            compatible_sdk_version: Some(CompatibleSdkVersionValue::Int(12)),
            compatible_sdk_version_stage: None,
            source_maps_path: None,
            insight_intent: None,
            declaration_entry: vec!["@normalized:N&&&@scope/pkg/index&1.0.0".to_string()],
            extra_importees: vec![],
            main_entry: None,
        }];
        let files =
            LoaderByteCodeHarFields::collect_other_compile_files(&deps, &module_root, tmp.path());

        assert!(
            files
                .iter()
                .any(|path| path.ends_with("source-har/src/main/ets/index.ets")),
            "blocked source HAR dependency under the issuer OHPM store root must be included"
        );
    }

    #[test]
    fn test_scan_bytecode_har_invalid_shadow_does_not_mark_seen() {
        let tmp = make_temp_dir();
        let module_dir = tmp.path().join("entry");
        let project_dir = tmp.path();

        let invalid_pkg = module_dir.join("oh_modules").join("shared-har");
        write_file(
            &invalid_pkg.join("oh-package.json5"),
            r#"{ name: "shared-har", metadata: { byteCodeHar: true } }"#,
        );

        let valid_pkg = project_dir.join("oh_modules").join("shared-har");
        write_abc(&valid_pkg.join("ets").join("modules.abc"));
        write_file(
            &valid_pkg.join("oh-package.json5"),
            r#"{ name: "shared-har", metadata: { byteCodeHar: true } }"#,
        );

        let results =
            ByteCodeHarMetadataWriter::scan_bytecode_har_deps_from_graph(&module_dir, project_dir);
        assert_eq!(results.len(), 1);
        assert!(results[0].abc_path.ends_with("ets/modules.abc"));
    }

    #[test]
    fn test_into_loader_entry() {
        let entry = ByteCodeHarScanEntry {
            package_name: "@scope/pkg".to_string(),
            package_root: PathBuf::from("/project/oh_modules/@scope/pkg"),
            abc_path: PathBuf::from("/project/oh_modules/@scope/pkg/index.abc"),
            is_har_dependency: true,
            compatible_sdk_version: Some(CompatibleSdkVersionValue::Str("5.0.0(12)".to_string())),
            compatible_sdk_version_stage: Some("beta1".to_string()),
            source_maps_path: Some(PathBuf::from(
                "/project/oh_modules/@scope/pkg/src/sourceMaps.map",
            )),
            insight_intent: Some(PathBuf::from(
                "/project/oh_modules/@scope/pkg/insight/intent.json",
            )),
            declaration_entry: vec![],
            extra_importees: vec![],
            main_entry: Some("index".to_string()),
        };
        let (key, loader_entry) = entry.into_loader_entry();
        assert_eq!(key, "@scope/pkg");
        assert_eq!(
            loader_entry.abc_path,
            "/project/oh_modules/@scope/pkg/index.abc"
        );
        assert_eq!(
            loader_entry.compatible_sdk_version,
            Some(CompatibleSdkVersionValue::Str("5.0.0(12)".to_string()))
        );
        assert_eq!(
            loader_entry.compatible_sdk_version_stage.as_deref(),
            Some("beta1")
        );
        assert!(loader_entry.source_maps_path.is_some());
        assert!(loader_entry.insight_intent.is_some());
        let json = serde_json::to_value(&loader_entry).expect("serialize bytecode har entry");
        assert_eq!(
            json.get("insightIntent").and_then(|value| value.as_str()),
            Some("/project/oh_modules/@scope/pkg/insight/intent.json")
        );
        assert!(json.get("declarationEntry").is_none());
    }

    #[test]
    fn test_collect_dynamic_import_lib_info_empty() {
        let tmp = make_temp_dir();
        let results = LoaderByteCodeHarFields::collect_dynamic_import_lib_info(
            "entry",
            &[],
            tmp.path(),
            tmp.path(),
        );
        assert!(results.is_empty());
    }

    #[test]
    fn test_collect_dynamic_import_lib_info_finds_oh_module() {
        let tmp = make_temp_dir();
        let oh_modules = tmp.path().join("oh_modules");
        let pkg_dir = oh_modules.join("@remote").join("shared");

        // Create package with an entry file.
        write_file(
            &pkg_dir.join("src/main/ets/index.ets"),
            "export default class Shared {}",
        );
        write_file(
            &pkg_dir.join("oh-package.json5"),
            r#"{ name: "@remote/shared" }"#,
        );
        write_file(
            &pkg_dir.join("src/main/module.json"),
            r#"{ module: { name: "internalHarName", type: "har" } }"#,
        );

        let deps = vec!["@remote/shared".to_string()];
        let results = LoaderByteCodeHarFields::collect_dynamic_import_lib_info(
            "entry",
            &deps,
            tmp.path(),
            tmp.path(),
        );

        assert_eq!(results.len(), 1);
        let (key, entry) = &results[0];
        assert_eq!(key, "@remote/shared");
        assert!(!entry.entry_file_path.is_empty());
        assert_eq!(entry.host_modules_info[0].host_module_name, "entry");
        assert_eq!(
            entry.host_modules_info[0].host_dependency_name,
            "@remote/shared"
        );
        assert_eq!(entry.module_name, "@remote/shared");
    }

    #[test]
    fn test_collect_dynamic_import_lib_info_missing_package_skipped() {
        let tmp = make_temp_dir();
        let deps = vec!["@missing/pkg".to_string()];
        let results = LoaderByteCodeHarFields::collect_dynamic_import_lib_info(
            "entry",
            &deps,
            tmp.path(),
            tmp.path(),
        );
        assert!(results.is_empty());
    }

    // ── ohm_url ─────────────────────────────────────────────────────────────

    fn make_entry(pkg: &str, main: Option<&str>) -> ByteCodeHarScanEntry {
        ByteCodeHarScanEntry {
            package_name: pkg.to_string(),
            package_root: PathBuf::from("/oh_modules").join(pkg),
            abc_path: PathBuf::from("/oh_modules")
                .join(pkg)
                .join("ets/modules.abc"),
            is_har_dependency: true,
            compatible_sdk_version: Some(CompatibleSdkVersionValue::Int(12)),
            compatible_sdk_version_stage: None,
            source_maps_path: None,
            insight_intent: None,
            declaration_entry: vec![],
            extra_importees: vec![],
            main_entry: main.map(str::to_string),
        }
    }

    #[test]
    fn test_ohm_url_basic() {
        let entry = make_entry("bc-har-dep", Some("index"));
        assert_eq!(
            entry.ohm_url("com.example.app"),
            Some("@bundle:com.example.app/bc-har-dep/index".to_string())
        );
    }

    #[test]
    fn test_ohm_url_strips_extension() {
        let entry = make_entry("my-har", Some("index.abc"));
        assert_eq!(
            entry.ohm_url("com.example"),
            Some("@bundle:com.example/my-har/index".to_string())
        );
    }

    #[test]
    fn test_ohm_url_scoped_package() {
        let entry = make_entry("@scope/pkg", Some("src/main/ets/index"));
        assert_eq!(
            entry.ohm_url("com.example"),
            Some("@bundle:com.example/@scope/pkg/src/main/ets/index".to_string())
        );
    }

    #[test]
    fn test_ohm_url_no_main_returns_none() {
        let entry = make_entry("no-main", None);
        assert_eq!(entry.ohm_url("com.example"), None);
    }

    #[test]
    fn test_ohm_url_empty_main_returns_none() {
        let entry = make_entry("empty-main", Some(""));
        assert_eq!(entry.ohm_url("com.example"), None);
    }

    // ── current hvigor metadata.byteCodeHar detection ────────────────────────

    #[test]
    fn test_scan_bytecode_har_ignores_root_level_bytecodehar() {
        let tmp = make_temp_dir();
        let oh_modules = tmp.path().join("oh_modules");
        let pkg_dir = oh_modules.join("bc-har-dep");

        write_abc(&pkg_dir.join("ets").join("modules.abc"));
        write_file(
            &pkg_dir.join("oh-package.json5"),
            r#"{
              "name": "bc-har-dep",
              "version": "1.0.0",
              "main": "index",
              "byteCodeHar": true,
              "dependencies": {}
            }"#,
        );

        let results =
            ByteCodeHarMetadataWriter::scan_bytecode_har_deps_from_graph(tmp.path(), tmp.path());
        assert!(results.is_empty());
    }

    #[test]
    fn test_scan_bytecode_har_metadata_marker_ohm_url() {
        let tmp = make_temp_dir();
        let oh_modules = tmp.path().join("oh_modules");
        let pkg_dir = oh_modules.join("bc-har-dep");

        write_abc(&pkg_dir.join("ets").join("modules.abc"));
        write_file(
            &pkg_dir.join("oh-package.json5"),
            r#"{
              "name": "bc-har-dep",
              "main": "index",
              "metadata": { "byteCodeHar": true }
            }"#,
        );

        let results =
            ByteCodeHarMetadataWriter::scan_bytecode_har_deps_from_graph(tmp.path(), tmp.path());
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].ohm_url("com.example.arkdown.test"),
            Some("@bundle:com.example.arkdown.test/bc-har-dep/index".to_string())
        );
    }

    #[test]
    fn test_scan_bytecode_har_only_accepts_metadata_marker() {
        let tmp = make_temp_dir();
        let oh_modules = tmp.path().join("oh_modules");

        // Package A: unsupported root-level byteCodeHar.
        let pkg_a = oh_modules.join("pkg-a");
        write_abc(&pkg_a.join("ets").join("modules.abc"));
        write_file(
            &pkg_a.join("oh-package.json5"),
            r#"{ "name": "pkg-a", "byteCodeHar": true }"#,
        );

        // Package B: metadata.byteCodeHar
        let pkg_b = oh_modules.join("pkg-b");
        write_abc(&pkg_b.join("ets").join("modules.abc"));
        write_file(
            &pkg_b.join("oh-package.json5"),
            r#"{ "name": "pkg-b", "metadata": { "byteCodeHar": true } }"#,
        );

        let results =
            ByteCodeHarMetadataWriter::scan_bytecode_har_deps_from_graph(tmp.path(), tmp.path());
        assert_eq!(
            results.len(),
            1,
            "current hvigor only accepts metadata.byteCodeHar"
        );
        let names: std::collections::HashSet<&str> =
            results.iter().map(|e| e.package_name.as_str()).collect();
        assert!(!names.contains("pkg-a"));
        assert!(names.contains("pkg-b"));
    }

    /// Verify that a BC HAR bundled INSIDE another HAR's own `oh_modules/` is detected.
    /// This proves the "bundled edge" case: the HAR hosts the BC HAR in its package.
    ///
    /// Addresses section 3.4: 'runtimeOnly ByteCode HAR、bundled edge 未证明'
    #[test]
    fn test_scan_bytecode_har_finds_bundled_bc_har_in_nested_oh_modules() {
        let tmp = make_temp_dir();
        let oh_modules = tmp.path().join("oh_modules");

        write_file(
            &tmp.path().join("oh-package.json5"),
            r#"{ "dependencies": { "host-har": "1.0.0" } }"#,
        );

        // host-har has a BC HAR bundled in ITS OWN oh_modules/ (bundled dependency).
        let host_har = oh_modules.join("host-har");
        write_file(
            &host_har.join("oh-package.json5"),
            r#"{ "name": "host-har", "dependencies": { "bundled-bc-har": "1.0.0" } }"#,
        );
        write_file(
            &host_har.join("src/main/ets/index.ets"),
            "export default 1;",
        );

        // bundled-bc-har lives inside host-har's own oh_modules/ (bundled).
        let bundled_bc = host_har.join("oh_modules/bundled-bc-har");
        write_abc(&bundled_bc.join("ets/modules.abc"));
        write_file(
            &bundled_bc.join("oh-package.json5"),
            r#"{ "name": "bundled-bc-har", "metadata": { "byteCodeHar": true } }"#,
        );

        let results =
            ByteCodeHarMetadataWriter::scan_bytecode_har_deps_from_graph(tmp.path(), tmp.path());
        let names: Vec<_> = results.iter().map(|e| e.package_name.as_str()).collect();
        assert!(
            names.contains(&"bundled-bc-har"),
            "BC HAR bundled inside a HAR's own oh_modules/ must be detected; got: {:?}",
            names
        );
    }

    /// Verify that BC HAR packages listed in `metadata.runtimeOnly.packages`
    /// are also scanned and appear in byteCodeHarInfo.
    ///
    /// Addresses section 3.4: 'runtimeOnly ByteCode HAR、bundled edge 未证明'
    #[test]
    fn test_scan_bytecode_har_finds_runtime_only_packages() {
        let tmp = make_temp_dir();
        let oh_modules = tmp.path().join("oh_modules");

        // Host module declares a regular dep that has runtimeOnly.packages.
        write_file(
            &tmp.path().join("oh-package.json5"),
            r#"{ "dependencies": { "host-lib": "1.0.0" } }"#,
        );

        // host-lib itself declares a runtimeOnly BC HAR.
        let host_lib = oh_modules.join("host-lib");
        write_file(
            &host_lib.join("oh-package.json5"),
            r#"{
              "name": "host-lib",
              "metadata": {
                "runtimeOnly": {
                  "packages": ["runtime-bc-har"]
                }
              }
            }"#,
        );
        write_file(
            &host_lib.join("src/main/ets/index.ets"),
            "export default 1;",
        );

        // runtime-bc-har is a ByteCode HAR.
        let runtime_bc_har = oh_modules.join("runtime-bc-har");
        write_abc(&runtime_bc_har.join("ets").join("modules.abc"));
        write_file(
            &runtime_bc_har.join("oh-package.json5"),
            r#"{ "name": "runtime-bc-har", "metadata": { "byteCodeHar": true } }"#,
        );

        let results =
            ByteCodeHarMetadataWriter::scan_bytecode_har_deps_from_graph(tmp.path(), tmp.path());
        let names: Vec<_> = results.iter().map(|e| e.package_name.as_str()).collect();
        assert!(
            names.contains(&"runtime-bc-har"),
            "BC HAR in metadata.runtimeOnly.packages must be detected; got: {:?}",
            names
        );
    }

    /// 6.24.3 `pkgName2OtherCompileFiles`: transitively blocked source HAR /
    /// npm packages group under the topmost ByteCode HAR package.
    #[test]
    fn test_collect_pkg_name2_other_compile_files_groups_by_topmost_blocker() {
        let tmp = make_temp_dir();
        let oh_modules = tmp.path().join("oh_modules");

        // bc-har-a (BC HAR) -> source-har-b (source HAR) -> source-har-c
        //                          \-> npm-pkg-d (npm package)
        write_file(
            &tmp.path().join("oh-package.json5"),
            r#"{ dependencies: { "bc-har-a": "1.0.0" } }"#,
        );
        let bc_har_a = oh_modules.join("bc-har-a");
        write_abc(&bc_har_a.join("ets").join("modules.abc"));
        write_file(
            &bc_har_a.join("oh-package.json5"),
            r#"{
              name: "bc-har-a",
              metadata: {
                byteCodeHar: true,
                extraImportees: ["extra-pkg/src/feature"]
              },
              dependencies: {
                "source-har-b": "1.0.0",
                "npm-pkg-d": "1.0.0"
              }
            }"#,
        );
        write_file(
            &bc_har_a.join("src/main/module.json5"),
            r#"{ module: { name: "bc-har-a", type: "har" } }"#,
        );

        let source_har_b = oh_modules.join("source-har-b");
        write_file(
            &source_har_b.join("oh-package.json5"),
            r#"{
              name: "source-har-b",
              main: "src/main/ets/index.ets",
              dependencies: {
                "source-har-c": "1.0.0",
                "npm-pkg-d": "1.0.0"
              }
            }"#,
        );
        write_file(
            &source_har_b.join("src/main/module.json5"),
            r#"{ module: { name: "source-har-b", type: "har" } }"#,
        );
        write_file(
            &source_har_b.join("src/main/ets/index.ets"),
            "export default 1;",
        );
        write_file(
            &source_har_b.join("src/main/ets/other.ets"),
            "export default 2;",
        );

        let source_har_c = oh_modules.join("source-har-c");
        write_file(
            &source_har_c.join("oh-package.json5"),
            r#"{ name: "source-har-c", main: "src/main/ets/index.ets" }"#,
        );
        write_file(
            &source_har_c.join("src/main/module.json5"),
            r#"{ module: { name: "source-har-c", type: "har" } }"#,
        );
        write_file(
            &source_har_c.join("src/main/ets/index.ets"),
            "export default 3;",
        );

        let npm_pkg_d = oh_modules.join("npm-pkg-d");
        write_file(
            &npm_pkg_d.join("oh-package.json5"),
            r#"{ name: "npm-pkg-d", main: "index.js" }"#,
        );
        write_file(&npm_pkg_d.join("index.js"), "export default 4;");

        let extra_pkg = oh_modules.join("extra-pkg");
        write_file(
            &extra_pkg.join("oh-package.json5"),
            r#"{ name: "extra-pkg" }"#,
        );
        write_file(&extra_pkg.join("src/feature.ets"), "export default 5;");

        let graph = arkdown_session::dependency_graph::builder::DependencyGraphBuilder::new(
            "entry",
            tmp.path(),
            tmp.path(),
        )
        .build();
        let grouped =
            LoaderByteCodeHarFields::collect_pkg_name2_other_compile_files_from_graph(&graph);

        let files = grouped
            .get("bc-har-a")
            .expect("blocked files group under the topmost BC HAR");
        let rel: Vec<String> = files
            .iter()
            .map(|path| {
                arkdown_path::ArtifactPathSerializer::path_to_slash_string(
                    path.strip_prefix(tmp.path()).unwrap_or(path),
                )
                .expect("serialize collected source path")
            })
            .collect();
        assert!(
            rel.iter()
                .any(|p| p.ends_with("source-har-b/src/main/ets/index.ets")),
            "source HAR entry must be collected; got: {rel:?}"
        );
        assert!(
            rel.iter()
                .any(|p| p.ends_with("source-har-b/src/main/ets/other.ets")),
            "source HAR ets tree must be collected; got: {rel:?}"
        );
        assert!(
            rel.iter()
                .any(|p| p.ends_with("source-har-c/src/main/ets/index.ets")),
            "transitive source HAR (two levels below the BC HAR) must be collected; got: {rel:?}"
        );
        assert!(
            rel.iter().any(|p| p.ends_with("npm-pkg-d/index.js")),
            "npm package entry below the BC HAR must be collected; got: {rel:?}"
        );
        assert!(
            rel.iter().any(|p| p.ends_with("extra-pkg/src/feature.ets")),
            "extraImportees must resolve from the host package inventory; got: {rel:?}"
        );
        assert!(
            !rel.iter().any(|p| p.contains("bc-har-a/ets")),
            "the BC HAR's own abc must not be collected; got: {rel:?}"
        );
        assert_eq!(
            grouped.len(),
            1,
            "only the topmost BC HAR receives a group; got: {:?}",
            grouped.keys().collect::<Vec<_>>()
        );
    }

    /// `getFinalNodes` drops HSP and SO dependencies from the walk.
    #[test]
    fn test_collect_pkg_name2_other_compile_files_skips_hsp_and_so_deps() {
        let tmp = make_temp_dir();
        let oh_modules = tmp.path().join("oh_modules");

        write_file(
            &tmp.path().join("oh-package.json5"),
            r#"{ dependencies: { "bc-har-a": "1.0.0" } }"#,
        );
        let bc_har_a = oh_modules.join("bc-har-a");
        write_abc(&bc_har_a.join("ets").join("modules.abc"));
        write_file(
            &bc_har_a.join("oh-package.json5"),
            r#"{
              name: "bc-har-a",
              metadata: { byteCodeHar: true },
              dependencies: {
                "shared-hsp": "1.0.0",
                "libnative.so": "1.0.0"
              }
            }"#,
        );

        let shared_hsp = oh_modules.join("shared-hsp");
        write_file(
            &shared_hsp.join("oh-package.json5"),
            r#"{ name: "shared-hsp", main: "src/main/ets/index.ets" }"#,
        );
        write_file(
            &shared_hsp.join("src/main/module.json5"),
            r#"{ module: { name: "shared-hsp", type: "shared" } }"#,
        );
        write_file(
            &shared_hsp.join("src/main/ets/index.ets"),
            "export default 5;",
        );

        let grouped =
            LoaderByteCodeHarFields::collect_pkg_name2_other_compile_files(tmp.path(), tmp.path());
        let files = grouped.get("bc-har-a").expect("group exists");
        assert!(
            !files
                .iter()
                .any(|path| path.to_string_lossy().contains("shared-hsp")),
            "HSP dependencies are dropped by getFinalNodes; got: {files:?}"
        );
    }

    /// devDependencies declared by the issuer are dropped from the walk.
    #[test]
    fn test_collect_pkg_name2_other_compile_files_skips_dev_dependencies() {
        let tmp = make_temp_dir();
        let oh_modules = tmp.path().join("oh_modules");

        write_file(
            &tmp.path().join("oh-package.json5"),
            r#"{
              dependencies: { "bc-har-a": "1.0.0" },
              devDependencies: { "dev-only": "1.0.0" }
            }"#,
        );
        let bc_har_a = oh_modules.join("bc-har-a");
        write_abc(&bc_har_a.join("ets").join("modules.abc"));
        write_file(
            &bc_har_a.join("oh-package.json5"),
            r#"{
              name: "bc-har-a",
              metadata: { byteCodeHar: true }
            }"#,
        );

        let dev_only = oh_modules.join("dev-only");
        write_file(
            &dev_only.join("oh-package.json5"),
            r#"{ name: "dev-only", main: "index.js" }"#,
        );
        write_file(&dev_only.join("index.js"), "export default 6;");

        let grouped =
            LoaderByteCodeHarFields::collect_pkg_name2_other_compile_files(tmp.path(), tmp.path());
        assert!(
            grouped
                .values()
                .flatten()
                .all(|path| !path.to_string_lossy().contains("dev-only")),
            "devDependencies are dropped from the walk"
        );
    }
}
