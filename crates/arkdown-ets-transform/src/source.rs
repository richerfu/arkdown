//! Source text classification types.
//!
//! Corresponds to SDK filters `/(?<!\.d)\.(ets|ts)$/`, `filterCrossplatform`,
//! and the source classification files from the plugin crate.

use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use arkdown_path::{canonicalize_or_owned, lexical_normalize, ModuleSpecifierText, SourceFileName};
use rustc_hash::{FxHashMap, FxHashSet};

/// The kind of source file, used to select the right OXC parser mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceFileKind {
    /// `.ets` file — uses `SourceType::ets()` with ArkUI AST nodes.
    Ets,
    /// `.ts` file — uses OXC TypeScript parser.
    Ts,
    /// `.js` / `.mjs` file — uses OXC JS parser.
    Js,
    /// `.d.ts` or `.d.ets` declaration file — excluded from transform.
    Declaration,
    /// Other file type — not processed by this crate.
    Other,
}

impl SourceFileKind {
    /// Whether this kind is a declaration file (excluded from transform).
    pub fn is_declaration(&self) -> bool {
        matches!(self, SourceFileKind::Declaration)
    }

    /// Whether this file should be processed by the ETS transform pipeline.
    pub fn is_ets_transform_input(&self) -> bool {
        matches!(self, SourceFileKind::Ets | SourceFileKind::Ts)
    }

    /// Whether this is a runtime JavaScript module whose source does not need
    /// to be retained by the ArkTS transform snapshot.
    pub fn is_javascript_runtime(&self) -> bool {
        matches!(self, SourceFileKind::Js)
    }
}

/// Classifies source files by their extension.
///
/// Corresponds to SDK filters `/(?<!\.d)\.(ets|ts)$/` and `filterCrossplatform`.
pub struct SourceClassifier;

impl SourceClassifier {
    /// Classify a file path into a `SourceFileKind`.
    pub fn classify(path: &str) -> SourceFileKind {
        Self::classify_path(Path::new(path))
    }

    pub fn classify_path(path: &Path) -> SourceFileKind {
        // Declaration files must be checked before .ts/.ets transform inputs.
        if SourceFileName::new(path).is_declaration() {
            return SourceFileKind::Declaration;
        }

        match path.extension().and_then(|e| e.to_str()) {
            Some("ets") => SourceFileKind::Ets,
            Some("ts") => SourceFileKind::Ts,
            Some("js") | Some("mjs") | Some("cjs") => SourceFileKind::Js,
            _ => SourceFileKind::Other,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TransformSourceSnapshot {
    files: Arc<FxHashMap<PathBuf, Arc<str>>>,
    lookup_paths: Arc<FxHashMap<PathBuf, PathBuf>>,
    runtime_source_paths: Arc<FxHashSet<PathBuf>>,
    package_roots: Arc<FxHashMap<String, PathBuf>>,
    resolved_static_modules: Arc<Vec<TransformResolvedStaticModuleScope>>,
}

impl TransformSourceSnapshot {
    pub fn new(files: FxHashMap<PathBuf, Arc<str>>) -> Self {
        Self::with_resolved_static_modules(files, Vec::new())
    }

    pub fn with_resolved_static_modules(
        files: FxHashMap<PathBuf, Arc<str>>,
        modules: Vec<TransformResolvedStaticModule>,
    ) -> Self {
        let runtime_source_paths = files
            .keys()
            .filter(|path| SourceClassifier::classify_path(path).is_javascript_runtime())
            .cloned()
            .collect();
        Self::with_resolved_static_modules_and_runtime_sources(files, modules, runtime_source_paths)
    }

    pub fn with_resolved_static_modules_and_runtime_sources(
        files: FxHashMap<PathBuf, Arc<str>>,
        modules: Vec<TransformResolvedStaticModule>,
        runtime_source_paths: FxHashSet<PathBuf>,
    ) -> Self {
        let indexed_files = TransformSourceFileIndex::new(files);
        let package_roots = SourcePackageRootCollector::new(indexed_files.files.keys()).collect();
        let resolved_static_modules = TransformResolvedStaticModuleIndex::new(modules).build();
        let runtime_source_paths = runtime_source_paths
            .into_iter()
            .map(|path| canonicalize_or_owned(&lexical_normalize(&path)))
            .collect();
        Self {
            files: Arc::new(indexed_files.files),
            lookup_paths: Arc::new(indexed_files.lookup_paths),
            runtime_source_paths: Arc::new(runtime_source_paths),
            package_roots: Arc::new(package_roots),
            resolved_static_modules: Arc::new(resolved_static_modules),
        }
    }

    pub(crate) fn resolve_relative_with_declarations(
        &self,
        importer: &Path,
        specifier: &str,
    ) -> Option<TransformSourceFile> {
        self.resolve_relative_path(
            importer,
            specifier,
            SourceSnapshotCandidateMode::Declarations,
        )
    }

    pub(crate) fn resolve_static_with_declarations(
        &self,
        importer: &Path,
        specifier: &str,
    ) -> Option<TransformSourceFile> {
        if ModuleSpecifierText::new(specifier).is_relative() {
            return self.resolve_relative_with_declarations(importer, specifier);
        }
        if let Some(resolved) = self.resolve_scoped_static_module(importer, specifier) {
            return Some(resolved);
        }
        self.resolve_package_specifier(specifier, SourceSnapshotCandidateMode::Declarations)
    }

    pub(crate) fn resolve_module_entry(&self, root: &Path) -> Option<TransformSourceFile> {
        let src_main_ets = root.join("src").join("main").join("ets");
        let entry_files = ["Index.ets", "index.ets", "Index.ts", "index.ts"];
        entry_files
            .into_iter()
            .map(|file_name| root.join(file_name))
            .chain(
                entry_files
                    .into_iter()
                    .map(|file_name| src_main_ets.join(file_name)),
            )
            .find_map(|path| self.source_file(lexical_normalize(&path)))
    }

    pub(crate) fn source_files(&self) -> Vec<TransformSourceFile> {
        let mut files = self
            .files
            .iter()
            .map(|(path, source)| TransformSourceFile {
                path: path.clone(),
                source: Arc::clone(source),
            })
            .collect::<Vec<_>>();
        files.sort_by(|left, right| left.path.cmp(&right.path));
        files
    }

    pub fn source_text(&self, path: &Path) -> Option<Arc<str>> {
        self.indexed_path(path)
            .and_then(|indexed| self.files.get(indexed))
            .map(Arc::clone)
            .or_else(|| self.files.get(&canonicalize_or_owned(path)).map(Arc::clone))
    }

    pub fn source_text_for_indexed_path(&self, path: &Path) -> Option<Arc<str>> {
        self.files.get(path).map(Arc::clone)
    }

    pub(crate) fn declaration_has_runtime_source(&self, declaration: &Path) -> bool {
        SourceFileName::new(declaration)
            .declaration_runtime_candidates()
            .is_some_and(|candidates| {
                candidates
                    .iter()
                    .any(|candidate| self.runtime_source_paths.contains(candidate))
            })
    }

    fn resolve_relative_path(
        &self,
        importer: &Path,
        specifier: &str,
        mode: SourceSnapshotCandidateMode,
    ) -> Option<TransformSourceFile> {
        if !ModuleSpecifierText::new(specifier).is_relative() {
            return None;
        }
        let importer = self
            .indexed_path_or_canonical(importer)
            .unwrap_or_else(|| lexical_normalize(importer));
        let base = importer.parent()?;
        SourceSnapshotPathCandidates::new(lexical_normalize(&base.join(specifier)), mode)
            .candidates()
            .into_iter()
            .find_map(|path| self.source_file(path))
    }

    fn resolve_package_specifier(
        &self,
        specifier: &str,
        mode: SourceSnapshotCandidateMode,
    ) -> Option<TransformSourceFile> {
        let package = PackageSpecifier::parse(specifier)?;
        let root = self.package_roots.get(package.name)?;
        if package.subpath.as_os_str().is_empty() {
            return self.resolve_module_entry(root);
        }
        SourceSnapshotPathCandidates::new(lexical_normalize(&root.join(package.subpath)), mode)
            .candidates()
            .into_iter()
            .find_map(|path| self.source_file(path))
    }

    fn resolve_scoped_static_module(
        &self,
        importer: &Path,
        specifier: &str,
    ) -> Option<TransformSourceFile> {
        let importer = self
            .indexed_path_or_canonical(importer)
            .unwrap_or_else(|| lexical_normalize(importer));
        self.resolved_static_modules
            .iter()
            .filter(|scope| importer.starts_with(&scope.importer_root))
            .find_map(|scope| {
                scope
                    .modules
                    .get(specifier)
                    .and_then(|path| self.source_file(path.clone()))
            })
    }

    fn source_file(&self, path: PathBuf) -> Option<TransformSourceFile> {
        let path = self.indexed_path_or_canonical(&path)?;
        self.files.get(&path).map(|source| TransformSourceFile {
            path,
            source: Arc::clone(source),
        })
    }

    fn indexed_path(&self, path: &Path) -> Option<&PathBuf> {
        self.lookup_paths
            .get(path)
            .or_else(|| self.lookup_paths.get(&lexical_normalize(path)))
    }

    fn indexed_path_or_canonical(&self, path: &Path) -> Option<PathBuf> {
        self.indexed_path(path).cloned().or_else(|| {
            let canonical = canonicalize_or_owned(path);
            self.indexed_path(&canonical).cloned()
        })
    }
}

struct TransformSourceFileIndex {
    files: FxHashMap<PathBuf, Arc<str>>,
    lookup_paths: FxHashMap<PathBuf, PathBuf>,
}

impl TransformSourceFileIndex {
    fn new(files: FxHashMap<PathBuf, Arc<str>>) -> Self {
        let mut indexed_files = FxHashMap::default();
        let mut lookup_paths = FxHashMap::default();
        for (path, source) in files {
            let lexical_path = lexical_normalize(&path);
            let indexed_path = canonicalize_or_owned(&lexical_path);
            indexed_files
                .entry(indexed_path.clone())
                .or_insert_with(|| Arc::clone(&source));
            lookup_paths.insert(path, indexed_path.clone());
            lookup_paths.insert(lexical_path, indexed_path.clone());
            lookup_paths.insert(indexed_path.clone(), indexed_path);
        }
        Self {
            files: indexed_files,
            lookup_paths,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TransformResolvedStaticModule {
    importer_root: PathBuf,
    specifier: String,
    resolved_path: PathBuf,
}

impl TransformResolvedStaticModule {
    pub fn new(
        importer_root: impl Into<PathBuf>,
        specifier: impl Into<String>,
        resolved_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            importer_root: importer_root.into(),
            specifier: specifier.into(),
            resolved_path: resolved_path.into(),
        }
    }
}

#[derive(Debug, Clone)]
struct TransformResolvedStaticModuleScope {
    importer_root: PathBuf,
    modules: FxHashMap<String, PathBuf>,
}

struct TransformResolvedStaticModuleIndex {
    modules: Vec<TransformResolvedStaticModule>,
}

impl TransformResolvedStaticModuleIndex {
    fn new(modules: Vec<TransformResolvedStaticModule>) -> Self {
        Self { modules }
    }

    fn build(self) -> Vec<TransformResolvedStaticModuleScope> {
        let mut by_root = FxHashMap::<PathBuf, FxHashMap<String, PathBuf>>::default();
        for module in self.modules {
            let importer_root = canonicalize_or_owned(&module.importer_root);
            let resolved_path = canonicalize_or_owned(&module.resolved_path);
            by_root
                .entry(importer_root)
                .or_default()
                .insert(module.specifier, resolved_path);
        }
        let mut scopes = by_root
            .into_iter()
            .map(
                |(importer_root, modules)| TransformResolvedStaticModuleScope {
                    importer_root,
                    modules,
                },
            )
            .collect::<Vec<_>>();
        scopes.sort_by(|left, right| {
            right
                .importer_root
                .components()
                .count()
                .cmp(&left.importer_root.components().count())
                .then_with(|| left.importer_root.cmp(&right.importer_root))
        });
        scopes
    }
}

struct SourcePackageRootCollector<'a, I>
where
    I: Iterator<Item = &'a PathBuf>,
{
    paths: I,
}

impl<'a, I> SourcePackageRootCollector<'a, I>
where
    I: Iterator<Item = &'a PathBuf>,
{
    fn new(paths: I) -> Self {
        Self { paths }
    }

    fn collect(self) -> FxHashMap<String, PathBuf> {
        let mut roots = FxHashMap::default();
        for path in self.paths {
            if let Some(package_root) = SourcePackageRoot::from_path(path) {
                roots.entry(package_root.name).or_insert(package_root.root);
            }
        }
        roots
    }
}

struct SourcePackageRoot {
    name: String,
    root: PathBuf,
}

impl SourcePackageRoot {
    fn from_path(path: &Path) -> Option<Self> {
        let components = path.components().collect::<Vec<_>>();
        for index in (0..components.len()).rev() {
            if components[index].as_os_str() != OsStr::new("oh_modules") {
                continue;
            }
            let Some(first) = components
                .get(index + 1)
                .and_then(|component| component.as_normal())
            else {
                continue;
            };
            if first == OsStr::new(".ohpm") {
                continue;
            }
            let first = first.to_str()?;
            let (name, root_end) = if first.starts_with('@') {
                let second = components
                    .get(index + 2)?
                    .as_normal()
                    .and_then(OsStr::to_str)?;
                (format!("{first}/{second}"), index + 2)
            } else {
                (first.to_string(), index + 1)
            };
            return Some(Self {
                name,
                root: components_to_path(&components[..=root_end]),
            });
        }
        None
    }
}

trait NormalComponentExt<'a> {
    fn as_normal(&self) -> Option<&'a OsStr>;
}

impl<'a> NormalComponentExt<'a> for Component<'a> {
    fn as_normal(&self) -> Option<&'a OsStr> {
        match self {
            Component::Normal(component) => Some(component),
            _ => None,
        }
    }
}

fn components_to_path(components: &[Component<'_>]) -> PathBuf {
    let mut path = PathBuf::new();
    for component in components {
        path.push(component.as_os_str());
    }
    path
}

struct PackageSpecifier<'a> {
    name: &'a str,
    subpath: PathBuf,
}

impl<'a> PackageSpecifier<'a> {
    fn parse(specifier: &'a str) -> Option<Self> {
        let mut segments = specifier.split('/');
        let first = segments.next()?;
        let name = if first.starts_with('@') {
            let second = segments.next()?;
            &specifier[..first.len() + 1 + second.len()]
        } else {
            first
        };
        let subpath = specifier
            .strip_prefix(name)
            .and_then(|rest| rest.strip_prefix('/'))
            .map(PathBuf::from)
            .unwrap_or_default();
        Some(Self { name, subpath })
    }
}

#[derive(Clone)]
pub(crate) struct TransformSourceFile {
    pub(crate) path: PathBuf,
    pub(crate) source: Arc<str>,
}

#[derive(Clone, Copy)]
enum SourceSnapshotCandidateMode {
    Declarations,
}

struct SourceSnapshotPathCandidates {
    candidate: PathBuf,
    mode: SourceSnapshotCandidateMode,
}

impl SourceSnapshotPathCandidates {
    fn new(candidate: PathBuf, mode: SourceSnapshotCandidateMode) -> Self {
        Self { candidate, mode }
    }

    fn candidates(&self) -> Vec<PathBuf> {
        let mut candidates = vec![
            self.candidate.clone(),
            self.candidate.with_extension("ets"),
            self.candidate.with_extension("ts"),
        ];
        if matches!(self.mode, SourceSnapshotCandidateMode::Declarations) {
            candidates.push(self.candidate.with_extension("d.ets"));
            candidates.push(self.candidate.with_extension("d.ts"));
        }
        candidates.extend([
            self.candidate.join("Index.ets"),
            self.candidate.join("index.ets"),
            self.candidate.join("Index.ts"),
            self.candidate.join("index.ts"),
        ]);
        if matches!(self.mode, SourceSnapshotCandidateMode::Declarations) {
            candidates.extend([
                self.candidate.join("Index.d.ets"),
                self.candidate.join("index.d.ets"),
                self.candidate.join("Index.d.ts"),
                self.candidate.join("index.d.ts"),
            ]);
        }
        candidates
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_ets() {
        assert_eq!(SourceClassifier::classify("foo.ets"), SourceFileKind::Ets);
    }

    #[test]
    fn test_classify_declaration_dts() {
        assert_eq!(
            SourceClassifier::classify("foo.d.ts"),
            SourceFileKind::Declaration
        );
    }

    #[test]
    fn test_classify_declaration_dets() {
        assert_eq!(
            SourceClassifier::classify("bar.d.ets"),
            SourceFileKind::Declaration
        );
    }

    #[test]
    fn test_classify_ts() {
        assert_eq!(SourceClassifier::classify("foo.ts"), SourceFileKind::Ts);
    }

    #[test]
    fn test_classify_js() {
        assert_eq!(SourceClassifier::classify("foo.js"), SourceFileKind::Js);
    }

    #[test]
    fn test_classify_other() {
        assert_eq!(
            SourceClassifier::classify("foo.json"),
            SourceFileKind::Other
        );
    }

    #[test]
    fn test_is_ets_transform_input() {
        assert!(SourceFileKind::Ets.is_ets_transform_input());
        assert!(SourceFileKind::Ts.is_ets_transform_input());
        assert!(!SourceFileKind::Declaration.is_ets_transform_input());
        assert!(!SourceFileKind::Js.is_ets_transform_input());
    }

    #[test]
    fn snapshot_resolves_source_files_through_indexed_aliases() {
        let tmp = tempfile::tempdir().unwrap();
        let real_root = tmp.path().join("real");
        let alias_root = tmp.path().join("alias");
        std::fs::create_dir_all(&real_root).unwrap();
        std::fs::write(real_root.join("main.ets"), "").unwrap();
        std::fs::write(real_root.join("types.d.ts"), "export interface Foo {}").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&real_root, &alias_root).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&real_root, &alias_root).unwrap();

        let mut files = FxHashMap::default();
        files.insert(
            alias_root.join("types.d.ts"),
            Arc::<str>::from("export interface Foo {}"),
        );
        let sources = TransformSourceSnapshot::new(files);

        let source = sources
            .resolve_relative_with_declarations(&alias_root.join("main.ets"), "./types")
            .expect("declaration file should resolve through the alias indexed at snapshot build");
        assert_eq!(
            source.path,
            real_root.join("types.d.ts").canonicalize().unwrap()
        );
    }

    #[test]
    fn snapshot_resolves_relative_declarations_from_alias_importer_to_canonical_files() {
        let tmp = tempfile::tempdir().unwrap();
        let real_root = tmp.path().join("real");
        let alias_root = tmp.path().join("alias");
        let source_path = real_root.join("src/main/ets/core/AES.ets");
        let declaration_path = real_root.join("src/main/cpp/types/libmtcrypto/index.d.ts");
        std::fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        std::fs::create_dir_all(declaration_path.parent().unwrap()).unwrap();
        std::fs::write(&source_path, "").unwrap();
        std::fs::write(&declaration_path, "export interface EncryptionResult {}").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&real_root, &alias_root).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&real_root, &alias_root).unwrap();

        let mut files = FxHashMap::default();
        files.insert(source_path.canonicalize().unwrap(), Arc::<str>::from(""));
        files.insert(
            declaration_path.canonicalize().unwrap(),
            Arc::<str>::from("export interface EncryptionResult {}"),
        );
        let sources = TransformSourceSnapshot::new(files);

        let resolved = sources
            .resolve_relative_with_declarations(
                &alias_root.join("src/main/ets/core/AES.ets"),
                "../../cpp/types/libmtcrypto",
            )
            .expect("alias importer should resolve declaration stored by canonical path");
        assert_eq!(resolved.path, declaration_path.canonicalize().unwrap());
    }
}
