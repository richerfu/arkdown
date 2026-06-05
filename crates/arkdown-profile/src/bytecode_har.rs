use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::loader_json::ByteCodeHarEntry;
use rustc_hash::{FxHashMap, FxHashSet};

// ---------------------------------------------------------------------------
// oh-package.json5 subset for ByteCode HAR detection
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OhPkgMinimal {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    compatible_sdk_version: Option<CompatibleSdkVersionField>,
    #[serde(default)]
    main: Option<String>,
    #[serde(default, alias = "typings")]
    types: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    dependencies: IndexMap<String, String>,
    #[serde(default)]
    dynamic_dependencies: IndexMap<String, String>,
    #[serde(default)]
    metadata: Option<OhPkgMetadata>,
    /// Root-level `byteCodeHar: true` — older hvigor format used before the
    /// field was moved inside `metadata`.  Both forms are supported.
    #[serde(default)]
    byte_code_har: bool,
}

impl OhPkgMinimal {
    fn load(pkg_dir: &Path) -> Option<Self> {
        let pkg_json = pkg_dir.join("oh-package.json5");
        if !pkg_json.exists() {
            return None;
        }
        let content = std::fs::read_to_string(&pkg_json).ok()?;
        json5::from_str::<Self>(&content).ok()
    }

    fn resolve_abc_path(&self, pkg_dir: &Path) -> Option<PathBuf> {
        // Support both root-level `byteCodeHar: true` (older hvigor format) and
        // the newer `metadata.byteCodeHar: true` form.
        let is_bytecode_har = self.byte_code_har
            || self
                .metadata
                .as_ref()
                .is_some_and(|metadata| metadata.byte_code_har);
        if is_bytecode_har {
            let abc_path = pkg_dir.join("ets").join("modules.abc");
            if abc_path.exists() {
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

        if let Some(main) = self.main.as_deref().filter(|main| main.ends_with(".abc")) {
            let abc_path = pkg_dir.join(main);
            if abc_path.exists() {
                return Some(abc_path);
            }
        }

        let abc_path = pkg_dir.join("index.abc");
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
            .chain(runtime_only_pkgs.into_iter())
            .filter(|name| seen.insert((*name).clone()))
            .cloned()
            .collect()
    }
}

#[derive(Debug, Deserialize)]
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
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct OhPkgRuntimeOnly {
    #[serde(default)]
    packages: Vec<String>,
    #[serde(default)]
    sources: Vec<String>,
}

/// `compatibleSdkVersion` can be a plain integer (`12`) or a string (`"5.0.0(12)"`).
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum CompatibleSdkVersionField {
    Int(u32),
    Str(String),
}

impl CompatibleSdkVersionField {
    fn as_string(&self) -> String {
        match self {
            CompatibleSdkVersionField::Int(n) => n.to_string(),
            CompatibleSdkVersionField::Str(s) => s.clone(),
        }
    }
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
    /// Compatible SDK version string, e.g. `"5.0.0(12)"` or `"12"`.
    pub compatible_sdk_version: String,
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
        let stem = std::path::Path::new(main)
            .with_extension("")
            .to_string_lossy()
            .into_owned();
        // Normalise path separators to forward slashes (Windows safety).
        let stem = stem.replace('\\', "/");
        Some(format!(
            "@bundle:{}/{}/{}",
            bundle_name, self.package_name, stem
        ))
    }
}

impl ByteCodeHarScanEntry {
    /// Convert into the [`ByteCodeHarEntry`] written to `loader.json`.
    pub fn into_loader_entry(self) -> (String, ByteCodeHarEntry) {
        let entry = ByteCodeHarEntry {
            package_name: self.package_name.clone(),
            abc_path: self.abc_path.to_string_lossy().into_owned(),
            compatible_sdk_version: self.compatible_sdk_version,
            source_maps_path: self
                .source_maps_path
                .map(|p| p.to_string_lossy().into_owned()),
            insight_intent: self
                .insight_intent
                .map(|p| p.to_string_lossy().into_owned()),
        };
        (self.package_name, entry)
    }

    // -----------------------------------------------------------------------
    // Private scan helpers
    // -----------------------------------------------------------------------

    fn scan_oh_modules_dir(oh_modules: &Path, seen: &mut FxHashSet<String>, out: &mut Vec<Self>) {
        for entry in Self::read_dir_sorted(oh_modules) {
            let pkg_dir = entry.path();
            if !pkg_dir.is_dir() {
                continue;
            }
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            // Handle scoped packages: `@scope/pkg` lives as `oh_modules/@scope/pkg`.
            if name_str.starts_with('@') {
                // Recurse one level for scoped packages.
                for scoped_entry in Self::read_dir_sorted(&pkg_dir) {
                    let scoped_dir = scoped_entry.path();
                    if !scoped_dir.is_dir() {
                        continue;
                    }
                    let pkg_name = format!(
                        "{}/{}",
                        name_str,
                        scoped_entry.file_name().to_string_lossy()
                    );
                    Self::try_collect(scoped_dir, &pkg_name, seen, out);
                }
            } else {
                Self::try_collect(pkg_dir, &name_str, seen, out);
            }
        }
    }

    fn read_dir_sorted(dir: &Path) -> Vec<std::fs::DirEntry> {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(|entry| entry.file_name());
        entries
    }

    fn try_collect(
        pkg_dir: PathBuf,
        pkg_name: &str,
        seen: &mut FxHashSet<String>,
        out: &mut Vec<Self>,
    ) {
        let Some(pkg) = OhPkgMinimal::load(&pkg_dir) else {
            return;
        };

        Self::try_collect_loaded(pkg_dir, pkg_name, pkg, seen, out);
    }

    fn try_collect_loaded(
        pkg_dir: PathBuf,
        fallback_pkg_name: &str,
        pkg: OhPkgMinimal,
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

        let compatible_sdk_version = pkg
            .compatible_sdk_version
            .as_ref()
            .map(CompatibleSdkVersionField::as_string)
            .unwrap_or_default();
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
            compatible_sdk_version,
            source_maps_path,
            insight_intent,
            declaration_entry,
            extra_importees,
            main_entry,
        });
    }
}

struct ByteCodeHarGraphScanner<'a> {
    module_root: &'a Path,
    project_root: &'a Path,
    seen_roots: FxHashSet<String>,
    seen_outputs: FxHashSet<String>,
    results: Vec<ByteCodeHarScanEntry>,
}

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
                let root_key = path_key(&pkg_dir);
                if !self.seen_roots.insert(root_key) {
                    continue;
                }

                let Some(pkg) = OhPkgMinimal::load(&pkg_dir) else {
                    continue;
                };
                let child_dep_names = pkg.dependency_names();

                ByteCodeHarScanEntry::try_collect_loaded(
                    pkg_dir.clone(),
                    &dep_name,
                    pkg,
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
            if dir.is_dir() && seen.insert(path_key(&dir)) {
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

fn path_key(path: &Path) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Scan `oh_modules/` directories for ByteCode HAR packages.
///
/// hvigor marks a dependency as ByteCode HAR with
/// `oh-package.json5#metadata.byteCodeHar` and expects the compiled artifact at
/// `ets/modules.abc`. For compatibility with older fixtures this also accepts
/// `metadata.abcPath`, `main` values ending in `.abc`, and a root-level
/// `index.abc`.
///
/// When the module or project `oh-package.json5` declares dependencies, this
/// follows that dependency graph breadth-first, matching hvigor's
/// DependencyManager-backed ByteCode HAR collection. Older fixtures without a
/// root package manifest still fall back to scanning:
/// 1. `<module_root>/oh_modules/`
/// 2. `<project_root>/oh_modules/`
///
/// Returns one entry per discovered ByteCode HAR, keyed by package name.
pub fn scan_bytecode_har_deps(
    module_root: &Path,
    project_root: &Path,
) -> Vec<ByteCodeHarScanEntry> {
    ByteCodeHarGraphScanner::new(module_root, project_root).scan()
}

/// Build hvigor's `loader.json#updateVersionInfo` for ByteCode HAR packages.
///
/// hvigor writes one object per ByteCode HAR. When
/// `metadata.dependencyPkgVersion` is present, only version mismatches are
/// emitted; otherwise the installed versions of the HAR's declared
/// dependencies are emitted.
pub fn collect_bytecode_har_update_version_info(
    deps: &[ByteCodeHarScanEntry],
    module_root: &Path,
    project_root: &Path,
) -> FxHashMap<String, serde_json::Value> {
    let mut info = FxHashMap::default();

    for dep in deps {
        let mut version_info = serde_json::Map::new();
        let Some(pkg) = OhPkgMinimal::load(&dep.package_root) else {
            info.insert(
                dep.package_name.clone(),
                serde_json::Value::Object(version_info),
            );
            continue;
        };

        if let Some(metadata) = pkg
            .metadata
            .as_ref()
            .filter(|m| !m.dependency_pkg_version.is_empty())
        {
            for (name, expected_version) in &metadata.dependency_pkg_version {
                let Some(installed_version) =
                    resolve_dependency_version(name, &dep.package_root, module_root, project_root)
                else {
                    continue;
                };
                if installed_version != *expected_version {
                    version_info.insert(name.clone(), serde_json::Value::String(installed_version));
                }
            }
        } else {
            for name in pkg.dependency_names() {
                let Some(installed_version) =
                    resolve_dependency_version(&name, &dep.package_root, module_root, project_root)
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

/// Build hvigor's `loader.json#otherCompileFiles` contribution for ByteCode
/// HAR dependencies.
///
/// This ports the deterministic subset of
/// `collectByteCodeInfoAndOtherCompileEntrances()` that does not require the
/// full hvigor `DependencyManager` object model:
/// * direct source HAR/npm dependencies blocked by a ByteCode HAR are compiled
/// * `metadata.extraImportees` are resolved to concrete source files
pub fn collect_bytecode_har_other_compile_files(
    deps: &[ByteCodeHarScanEntry],
    module_root: &Path,
    project_root: &Path,
) -> Vec<PathBuf> {
    let mut files = Vec::new();

    for dep in deps {
        collect_blocked_dependency_compile_files(dep, module_root, project_root, &mut files);
    }
    for dep in deps {
        collect_extra_importee_compile_files(dep, module_root, project_root, &mut files);
    }

    dedupe_pathbufs(&mut files);
    files
}

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
        let Some(child_root) =
            resolve_dependency_root(&child_name, &dep.package_root, module_root, project_root)
        else {
            continue;
        };
        let Some(child_pkg) = OhPkgMinimal::load(&child_root) else {
            continue;
        };

        if child_pkg.resolve_abc_path(&child_root).is_some() {
            continue;
        }

        if is_source_har_package(&child_root) {
            if let Some(entry) = resolve_package_entry_path(&child_root, &child_pkg) {
                files.push(entry);
            }
            files.extend(collect_code_files(&child_root.join("src/main/ets")));
        } else if let Some(entry) = resolve_package_entry_path(&child_root, &child_pkg) {
            files.push(entry);
        }
    }
}

fn collect_extra_importee_compile_files(
    dep: &ByteCodeHarScanEntry,
    module_root: &Path,
    project_root: &Path,
    files: &mut Vec<PathBuf>,
) {
    for importee in &dep.extra_importees {
        let package_name = package_name_from_importee(importee);
        let Some(pkg_root) =
            resolve_dependency_root(&package_name, &dep.package_root, module_root, project_root)
        else {
            continue;
        };
        let Some(pkg) = OhPkgMinimal::load(&pkg_root) else {
            continue;
        };

        if importee == &package_name {
            if let Some(entry) = resolve_package_entry_path(&pkg_root, &pkg) {
                files.push(entry);
            }
            continue;
        }

        let rel = importee
            .strip_prefix(&package_name)
            .unwrap_or_default()
            .trim_start_matches(['/', '\\']);
        if let Some(path) = find_supported_source_file(&pkg_root.join(rel)) {
            files.push(path);
        }
    }
}

fn resolve_dependency_root(
    dep_name: &str,
    issuer_root: &Path,
    module_root: &Path,
    project_root: &Path,
) -> Option<PathBuf> {
    let candidates = [
        issuer_root.join("oh_modules").join(dep_name),
        module_root.join("oh_modules").join(dep_name),
        project_root.join("oh_modules").join(dep_name),
    ];

    let mut seen = FxHashSet::default();
    for candidate in candidates {
        if candidate.is_dir() && seen.insert(path_key(&candidate)) {
            return Some(candidate);
        }
    }
    None
}

fn is_source_har_package(pkg_root: &Path) -> bool {
    pkg_root.join("src/main/module.json5").is_file()
        || pkg_root.join("src/main/module.json").is_file()
}

fn resolve_package_entry_path(pkg_root: &Path, pkg: &OhPkgMinimal) -> Option<PathBuf> {
    if let Some(main) = pkg.main.as_deref().filter(|value| !value.is_empty()) {
        let candidate = pkg_root.join(main);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    if let Some(types) = pkg.types.as_deref().filter(|value| !value.is_empty()) {
        let candidate = pkg_root.join(types);
        if candidate.is_file() && is_code_file(&candidate) {
            return Some(candidate);
        }
    }

    for rel in ["src/main/ets/index", "src/main/ets/Index", "index", "Index"] {
        if let Some(path) = find_supported_source_file(&pkg_root.join(rel)) {
            return Some(path);
        }
    }
    None
}

fn find_supported_source_file(base: &Path) -> Option<PathBuf> {
    if base.is_file() && is_code_file(base) {
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

fn collect_code_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.is_file() && is_code_file(&path) {
                files.push(path);
            }
        }
    }
    files
}

fn is_code_file(path: &Path) -> bool {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if file_name.ends_with(".d.ts") || file_name.ends_with(".d.ets") {
        return false;
    }
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("ets" | "ts" | "js")
    )
}

fn package_name_from_importee(importee: &str) -> String {
    let mut parts = importee.split(['/', '\\']);
    let first = parts.next().unwrap_or_default();
    if first.starts_with('@') {
        if let Some(second) = parts.next() {
            return format!("{first}/{second}");
        }
    }
    first.to_string()
}

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
        if !candidate.is_dir() || !seen.insert(path_key(&candidate)) {
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

fn dedupe_pathbufs(paths: &mut Vec<PathBuf>) {
    let mut seen = FxHashSet::default();
    paths.retain(|path| seen.insert(path_key(path)));
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
            .map(|v| v.starts_with("file:"))
            .unwrap_or(false)
    }

    fn find_entry_file(pkg_dir: &Path) -> String {
        let pkg_json = pkg_dir.join("oh-package.json5");
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
                        return candidate.to_string_lossy().into_owned();
                    }
                }
                if let Some(types) = parsed.types.filter(|s| !s.is_empty()) {
                    let candidate = pkg_dir.join(&types);
                    if candidate.exists() {
                        return candidate.to_string_lossy().into_owned();
                    }
                }
            }
        }

        // Fall back to conventional entry points.
        for candidate_name in &["index.ets", "index.ts", "index.js"] {
            let candidate = pkg_dir.join("src/main/ets").join(candidate_name);
            if candidate.exists() {
                return candidate.to_string_lossy().into_owned();
            }
            let candidate = pkg_dir.join(candidate_name);
            if candidate.exists() {
                return candidate.to_string_lossy().into_owned();
            }
        }

        String::new()
    }

    fn read_module_name_from_pkg(pkg_dir: &Path, fallback: &str) -> String {
        // Try module.json first (compiled HAR), then module.json5 (source HAR).
        for json_name in &["src/main/module.json", "src/main/module.json5"] {
            let path = pkg_dir.join(json_name);
            if let Ok(content) = std::fs::read_to_string(&path) {
                #[derive(Deserialize)]
                struct ModuleJson {
                    module: ModuleSection,
                }
                #[derive(Deserialize)]
                struct ModuleSection {
                    name: Option<String>,
                }
                if let Ok(parsed) = json5::from_str::<ModuleJson>(&content) {
                    if let Some(name) = parsed.module.name.filter(|s| !s.is_empty()) {
                        return name;
                    }
                }
            }
        }

        // Fallback: use the last path segment of the package name.
        fallback.rsplit('/').next().unwrap_or(fallback).to_string()
    }
}

/// Scan `dynamicDependencies` from `oh-package.json5` and resolve each dep
/// under `oh_modules/` to produce `dynamicImportLibInfo` entries.
///
/// For each package name in `dynamic_dep_names` that resolves to a directory
/// under `<module_root>/oh_modules/` or `<project_root>/oh_modules/`, one
/// [`DynamicImportLibEntry`] is produced.
pub fn collect_dynamic_import_lib_info(
    module_name: &str,
    dynamic_dep_names: &[String],
    module_root: &Path,
    project_root: &Path,
) -> Vec<(String, DynamicImportLibEntry)> {
    let mut results = Vec::new();

    for dep_name in dynamic_dep_names {
        // Resolve the package directory under oh_modules.
        let pkg_dir =
            DynamicImportLibEntry::resolve_oh_module_dir(dep_name, module_root, project_root);
        let Some(pkg_dir) = pkg_dir else {
            continue;
        };

        let is_local = DynamicImportLibEntry::is_local_dependency(dep_name, module_root);
        let pkg_dir = if is_local {
            std::fs::canonicalize(&pkg_dir).unwrap_or(pkg_dir)
        } else {
            pkg_dir
        };
        let entry_file = DynamicImportLibEntry::find_entry_file(&pkg_dir);
        let pkg_module_name = DynamicImportLibEntry::read_module_name_from_pkg(&pkg_dir, dep_name);

        let entry = DynamicImportLibEntry {
            entry_file_path: entry_file,
            module_name: pkg_module_name,
            is_local_dependency: is_local,
            pkg_path: pkg_dir.to_string_lossy().into_owned(),
            host_modules_info: vec![HostModuleInfo {
                host_dependency_name: dep_name.clone(),
                host_module_name: module_name.to_string(),
            }],
        };
        results.push((dep_name.clone(), entry));
    }

    results
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

    #[test]
    fn test_scan_bytecode_har_empty_oh_modules() {
        let tmp = make_temp_dir();
        let results = scan_bytecode_har_deps(tmp.path(), tmp.path());
        assert!(results.is_empty());
    }

    #[test]
    fn test_scan_bytecode_har_finds_abc_package() {
        let tmp = make_temp_dir();
        let oh_modules = tmp.path().join("oh_modules");
        let pkg_dir = oh_modules.join("@ohos").join("test-har");

        write_abc(&pkg_dir.join("index.abc"));
        write_file(
            &pkg_dir.join("oh-package.json5"),
            r#"{ name: "@ohos/test-har", compatibleSdkVersion: "5.0.0(12)" }"#,
        );

        let results = scan_bytecode_har_deps(tmp.path(), tmp.path());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].package_name, "@ohos/test-har");
        assert_eq!(results[0].compatible_sdk_version, "5.0.0(12)");
        assert!(results[0].source_maps_path.is_none());
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

        let results = scan_bytecode_har_deps(tmp.path(), tmp.path());
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
    fn test_scan_bytecode_har_finds_source_maps() {
        let tmp = make_temp_dir();
        let oh_modules = tmp.path().join("oh_modules");
        let pkg_dir = oh_modules.join("my-har");

        write_abc(&pkg_dir.join("index.abc"));
        write_file(&pkg_dir.join("src").join("sourceMaps.map"), "{}");
        write_file(
            &pkg_dir.join("oh-package.json5"),
            r#"{ name: "my-har", compatibleSdkVersion: 12 }"#,
        );

        let results = scan_bytecode_har_deps(tmp.path(), tmp.path());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].compatible_sdk_version, "12");
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
                abcPath: "dist/metadata.abc",
                sourceMapsPath: "maps/sourceMaps.map",
                insightIntentPath: "insight/intent.json"
              }
            }"#,
        );

        let results = scan_bytecode_har_deps(tmp.path(), tmp.path());
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

        let results = scan_bytecode_har_deps(tmp.path(), tmp.path());
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
            write_file(&pkg.join("oh-package.json5"), r#"{ name: "shared-har" }"#);
        }

        let results = scan_bytecode_har_deps(&module_dir, project_dir);
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

        let results = scan_bytecode_har_deps(tmp.path(), tmp.path());
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

        let results = scan_bytecode_har_deps(tmp.path(), tmp.path());
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

        let results = scan_bytecode_har_deps(tmp.path(), tmp.path());
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

        let results = scan_bytecode_har_deps(tmp.path(), tmp.path());
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
            &oh_modules.join("dep-a").join("oh-package.json5"),
            r#"{ name: "dep-a", version: "1.1.0" }"#,
        );
        write_file(
            &oh_modules.join("dep-b").join("oh-package.json5"),
            r#"{ name: "dep-b", version: "2.0.0" }"#,
        );

        let deps = scan_bytecode_har_deps(tmp.path(), tmp.path());
        let info = collect_bytecode_har_update_version_info(&deps, tmp.path(), tmp.path());
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
            &oh_modules.join("dep-a").join("oh-package.json5"),
            r#"{ name: "dep-a", version: "1.0.1" }"#,
        );
        write_file(
            &oh_modules.join("dep-b").join("oh-package.json5"),
            r#"{ name: "dep-b", version: "2.0.1" }"#,
        );

        let deps = scan_bytecode_har_deps(tmp.path(), tmp.path());
        let info = collect_bytecode_har_update_version_info(&deps, tmp.path(), tmp.path());
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

        let deps = scan_bytecode_har_deps(tmp.path(), tmp.path());
        let files = collect_bytecode_har_other_compile_files(&deps, tmp.path(), tmp.path());
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

        let deps = scan_bytecode_har_deps(tmp.path(), tmp.path());
        let files = collect_bytecode_har_other_compile_files(&deps, tmp.path(), tmp.path());
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

        let results = scan_bytecode_har_deps(&module_dir, project_dir);
        assert_eq!(results.len(), 1);
        assert!(results[0].abc_path.ends_with("ets/modules.abc"));
    }

    #[test]
    fn test_into_loader_entry() {
        let entry = ByteCodeHarScanEntry {
            package_name: "@scope/pkg".to_string(),
            package_root: PathBuf::from("/project/oh_modules/@scope/pkg"),
            abc_path: PathBuf::from("/project/oh_modules/@scope/pkg/index.abc"),
            compatible_sdk_version: "5.0.0(12)".to_string(),
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
        assert_eq!(loader_entry.package_name, "@scope/pkg");
        assert_eq!(
            loader_entry.abc_path,
            "/project/oh_modules/@scope/pkg/index.abc"
        );
        assert_eq!(loader_entry.compatible_sdk_version, "5.0.0(12)");
        assert!(loader_entry.source_maps_path.is_some());
        assert!(loader_entry.insight_intent.is_some());
        let json = serde_json::to_value(&loader_entry).expect("serialize bytecode har entry");
        assert_eq!(
            json.get("insightIntent").and_then(|value| value.as_str()),
            Some("/project/oh_modules/@scope/pkg/insight/intent.json")
        );
    }

    #[test]
    fn test_collect_dynamic_import_lib_info_empty() {
        let tmp = make_temp_dir();
        let results = collect_dynamic_import_lib_info("entry", &[], tmp.path(), tmp.path());
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

        let deps = vec!["@remote/shared".to_string()];
        let results = collect_dynamic_import_lib_info("entry", &deps, tmp.path(), tmp.path());

        assert_eq!(results.len(), 1);
        let (key, entry) = &results[0];
        assert_eq!(key, "@remote/shared");
        assert!(!entry.entry_file_path.is_empty());
        assert_eq!(entry.host_modules_info[0].host_module_name, "entry");
        assert_eq!(
            entry.host_modules_info[0].host_dependency_name,
            "@remote/shared"
        );
    }

    #[test]
    fn test_collect_dynamic_import_lib_info_missing_package_skipped() {
        let tmp = make_temp_dir();
        let deps = vec!["@missing/pkg".to_string()];
        let results = collect_dynamic_import_lib_info("entry", &deps, tmp.path(), tmp.path());
        assert!(results.is_empty());
    }

    // ── ohm_url ─────────────────────────────────────────────────────────────

    fn make_entry(pkg: &str, main: Option<&str>) -> ByteCodeHarScanEntry {
        ByteCodeHarScanEntry {
            package_name: pkg.to_string(),
            package_root: PathBuf::from("/oh_modules").join(pkg),
            abc_path: PathBuf::from("/oh_modules").join(pkg).join("ets/modules.abc"),
            compatible_sdk_version: "12".to_string(),
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

    // ── root-level byteCodeHar detection ─────────────────────────────────────

    #[test]
    fn test_scan_bytecode_har_detects_root_level_bytecodehar() {
        // Fixture mirrors tests/fixtures/minimal-hap-with-bc-har/entry/oh_modules/bc-har-dep
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

        let results = scan_bytecode_har_deps(tmp.path(), tmp.path());
        assert_eq!(results.len(), 1, "root-level byteCodeHar must be detected");
        assert_eq!(results[0].package_name, "bc-har-dep");
        assert!(results[0].abc_path.ends_with("ets/modules.abc"));
        assert_eq!(
            results[0].main_entry.as_deref(),
            Some("index"),
            "main_entry must be populated from oh-package.json5#main"
        );
    }

    #[test]
    fn test_scan_bytecode_har_root_level_ohm_url() {
        let tmp = make_temp_dir();
        let oh_modules = tmp.path().join("oh_modules");
        let pkg_dir = oh_modules.join("bc-har-dep");

        write_abc(&pkg_dir.join("ets").join("modules.abc"));
        write_file(
            &pkg_dir.join("oh-package.json5"),
            r#"{
              "name": "bc-har-dep",
              "main": "index",
              "byteCodeHar": true
            }"#,
        );

        let results = scan_bytecode_har_deps(tmp.path(), tmp.path());
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].ohm_url("com.example.arkdown.test"),
            Some("@bundle:com.example.arkdown.test/bc-har-dep/index".to_string())
        );
    }

    #[test]
    fn test_scan_bytecode_har_metadata_and_root_level_equivalent() {
        let tmp = make_temp_dir();
        let oh_modules = tmp.path().join("oh_modules");

        // Package A: root-level byteCodeHar
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

        let results = scan_bytecode_har_deps(tmp.path(), tmp.path());
        assert_eq!(results.len(), 2, "both forms of byteCodeHar must be detected");
        let names: std::collections::HashSet<&str> =
            results.iter().map(|e| e.package_name.as_str()).collect();
        assert!(names.contains("pkg-a"));
        assert!(names.contains("pkg-b"));
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
        write_file(&host_lib.join("src/main/ets/index.ets"), "export default 1;");

        // runtime-bc-har is a ByteCode HAR.
        let runtime_bc_har = oh_modules.join("runtime-bc-har");
        write_abc(&runtime_bc_har.join("ets").join("modules.abc"));
        write_file(
            &runtime_bc_har.join("oh-package.json5"),
            r#"{ "name": "runtime-bc-har", "metadata": { "byteCodeHar": true } }"#,
        );

        let results = scan_bytecode_har_deps(tmp.path(), tmp.path());
        let names: Vec<_> = results
            .iter()
            .map(|e| e.package_name.as_str())
            .collect();
        assert!(
            names.contains(&"runtime-bc-har"),
            "BC HAR in metadata.runtimeOnly.packages must be detected; got: {:?}",
            names
        );
    }
}
