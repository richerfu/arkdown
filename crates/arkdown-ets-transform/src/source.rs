//! Source text classification types.
//!
//! Corresponds to SDK filters `/(?<!\.d)\.(ets|ts)$/`, `filterCrossplatform`,
//! and the source classification files from the plugin crate.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use arkdown_core::ArktsLanguageVersion;
use arkdown_path::{canonicalize_or_owned, lexical_normalize, ModuleSpecifierText, SourceFileName};
use oxc_span::SourceType;
use rustc_hash::{FxHashMap, FxHashSet};
use sha2::{Digest, Sha256};

use crate::VisualGeneratedSource;

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

/// Selects the parser mode used by the SDK CompileArkTS source pipeline.
///
/// OpenHarmony packages may keep ArkTS extensions such as `import lazy` in a
/// file named `*.ts`. SDK `ModuleSourceFile.processTransformedTsModuleRequest`
/// parses those files with the ArkTS-enabled TypeScript frontend, so runtime
/// `.ts` and `.ets` inputs must both use OXC's ArkUI language variant.
pub struct CompileArktsSourceType;

impl CompileArktsSourceType {
    pub fn from_id(id: &str) -> SourceType {
        Self::from_path(Path::new(id))
    }

    pub fn from_path(path: &Path) -> SourceType {
        Self::from_path_with_version(path, ArktsLanguageVersion::default())
    }

    pub fn from_id_with_version(id: &str, version: ArktsLanguageVersion) -> SourceType {
        Self::from_path_with_version(Path::new(id), version)
    }

    pub fn from_path_with_version(path: &Path, version: ArktsLanguageVersion) -> SourceType {
        match SourceClassifier::classify_path(path) {
            SourceFileKind::Ets | SourceFileKind::Ts if version.is_static() => {
                SourceType::ets_static()
            }
            SourceFileKind::Ets | SourceFileKind::Ts => SourceType::ets(),
            SourceFileKind::Declaration | SourceFileKind::Js | SourceFileKind::Other => {
                SourceType::from_path(path).unwrap_or_else(|_| SourceType::ts().with_module(true))
            }
        }
    }
}

/// Selects the language of one source inside an ArkTS 1.2 module graph.
///
/// `bindings/src/common/utils.ts::getFileLanguageVersion` checks the first
/// effective source line after comments for both the main module and its
/// source dependencies. Product ArkTS 1.2 enables the mixed pipeline; it does
/// not by itself turn every main-module `.ets` file into static ArkTS.
pub struct ArktsSourceLanguageDetector<'a> {
    source: &'a str,
}

impl<'a> ArktsSourceLanguageDetector<'a> {
    pub fn new(source: &'a str) -> Self {
        Self { source }
    }

    pub fn version_for_path(
        &self,
        product_version: ArktsLanguageVersion,
        path: &Path,
    ) -> ArktsLanguageVersion {
        if product_version.is_static()
            && SourceClassifier::classify_path(path) == SourceFileKind::Ets
            && self.is_static()
        {
            ArktsLanguageVersion::V1_2
        } else {
            ArktsLanguageVersion::V1_1
        }
    }

    /// Exact comment skipping and first-effective-line test from
    /// `getFileLanguageVersion`; it deliberately checks `contains` because
    /// that is the current upstream behavior.
    pub fn is_static(&self) -> bool {
        let mut in_multi_line_comment = false;
        // JavaScript `String.prototype.trim()` treats a UTF-8 BOM decoded as
        // U+FEFF as whitespace. Rust `str::trim()` does not, so remove it once
        // to preserve the upstream first-effective-line result.
        let source = self.source.strip_prefix('\u{feff}').unwrap_or(self.source);
        for original_line in source.split('\n') {
            let mut line = original_line;
            let owned_line;
            if in_multi_line_comment {
                let Some(end_index) = line.find("*/") else {
                    continue;
                };
                line = &line[end_index + 2..];
                in_multi_line_comment = false;
            }
            if let Some(single_line_index) = line.find("//") {
                line = &line[..single_line_index];
            }
            if let Some(multi_line_start) = line.find("/*") {
                if let Some(relative_end) = line[multi_line_start + 2..].find("*/") {
                    let multi_line_end = multi_line_start + 2 + relative_end;
                    let mut value = String::with_capacity(line.len());
                    value.push_str(&line[..multi_line_start]);
                    value.push_str(&line[multi_line_end + 2..]);
                    owned_line = value;
                    line = &owned_line;
                } else {
                    line = &line[..multi_line_start];
                    in_multi_line_comment = true;
                }
            }
            let effective_line = line.trim();
            if !effective_line.is_empty() {
                return effective_line.contains("use static");
            }
        }
        false
    }

    /// Production driver rule from
    /// `driver/build_system/src/plugins/FileManager.ts::isFirstLineUseStatic`.
    /// Unlike module-level language discovery, a hybrid dependency compile
    /// file is static only when its trimmed physical first line is exactly the
    /// single-quoted directive.
    pub fn is_static_dependency_compile_file(&self) -> bool {
        self.source
            .lines()
            .next()
            .map(|line| line.strip_prefix('\u{feff}').unwrap_or(line).trim())
            == Some("'use static'")
    }
}

#[derive(Debug, Clone, Default)]
pub struct TransformSourceSnapshot {
    files: Arc<FxHashMap<PathBuf, Arc<str>>>,
    lookup_paths: Arc<FxHashMap<PathBuf, PathBuf>>,
    runtime_source_paths: Arc<FxHashSet<PathBuf>>,
    package_roots: Arc<FxHashMap<String, PathBuf>>,
    resolved_static_modules: Arc<Vec<TransformResolvedStaticModuleScope>>,
    sdk_declaration_roots: Arc<Vec<PathBuf>>,
    visual_sources: Arc<FxHashMap<PathBuf, VisualGeneratedSource>>,
    visual_lookup_paths: Arc<FxHashMap<PathBuf, PathBuf>>,
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
            sdk_declaration_roots: Arc::default(),
            visual_sources: Arc::default(),
            visual_lookup_paths: Arc::default(),
        }
    }

    /// Pre-index lexical path aliases while the build snapshot is being
    /// assembled. Source roots can enter through `oh_modules` symlinks while
    /// the immutable source store owns their canonical paths; transform hooks
    /// must resolve both identities without calling `realpath` per module.
    pub fn with_path_aliases(mut self, aliases: Vec<(PathBuf, PathBuf)>) -> Self {
        if aliases.is_empty() {
            return self;
        }
        let mut lookup_paths = (*self.lookup_paths).clone();
        for (alias, target) in aliases {
            let alias = lexical_normalize(&alias);
            let target = canonicalize_or_owned(&lexical_normalize(&target));
            for indexed_path in self.files.keys() {
                let Some(relative) = indexed_path.strip_prefix(&target).ok() else {
                    continue;
                };
                lookup_paths.insert(
                    lexical_normalize(&alias.join(relative)),
                    indexed_path.clone(),
                );
            }
        }
        self.lookup_paths = Arc::new(lookup_paths);
        self
    }

    /// Attach the Visual/SuperVisual fragments generated once before
    /// Rolldown starts. Transform hooks only perform OXC AST insertion and do
    /// not launch Node or read `.visual` files.
    pub fn with_visual_sources(
        mut self,
        sources: FxHashMap<PathBuf, VisualGeneratedSource>,
    ) -> Self {
        let mut visual_sources = FxHashMap::default();
        let mut visual_lookup_paths = FxHashMap::default();
        for (path, source) in sources {
            let lexical_path = lexical_normalize(&path);
            let indexed_path = self
                .indexed_path(&path)
                .cloned()
                .unwrap_or_else(|| canonicalize_or_owned(&lexical_path));
            visual_sources.insert(indexed_path.clone(), source);
            visual_lookup_paths.insert(path, indexed_path.clone());
            visual_lookup_paths.insert(lexical_path, indexed_path.clone());
            visual_lookup_paths.insert(indexed_path.clone(), indexed_path);
        }
        self.visual_sources = Arc::new(visual_sources);
        self.visual_lookup_paths = Arc::new(visual_lookup_paths);
        self
    }

    pub(crate) fn visual_source(&self, path: &Path) -> Option<&VisualGeneratedSource> {
        self.indexed_path(path)
            .or_else(|| self.visual_lookup_paths.get(path))
            .or_else(|| self.visual_lookup_paths.get(&lexical_normalize(path)))
            .and_then(|indexed| self.visual_sources.get(indexed))
    }

    /// Attach the ordered SDK declaration roots inventoried by the build
    /// coordinator. The source text for these roots must already be present in
    /// this snapshot, keeping transform hooks free of filesystem reads.
    pub fn with_sdk_declaration_roots(mut self, roots: Vec<PathBuf>) -> Self {
        let mut seen = FxHashSet::default();
        let roots = roots
            .into_iter()
            .map(|root| canonicalize_or_owned(&lexical_normalize(&root)))
            .filter(|root| seen.insert(root.clone()))
            .collect();
        self.sdk_declaration_roots = Arc::new(roots);
        self
    }

    /// Override/complete the package root index with the dependency graph's
    /// authoritative `package_name → package_root` mapping. Path inference
    /// only recognizes `oh_modules/` layouts; local `file:` dependencies are
    /// canonicalized to their real paths and must be supplied explicitly
    /// (hvigor's TS program resolves them through the installed
    /// `oh_modules/` links).
    pub fn with_package_roots(mut self, roots: FxHashMap<String, PathBuf>) -> Self {
        let mut package_roots = (*self.package_roots).clone();
        for (name, root) in roots {
            package_roots.insert(name, lexical_normalize(&root));
        }
        self.package_roots = Arc::new(package_roots);
        self
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
            .or_else(|| self.resolve_sdk_declaration(specifier))
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
    }

    pub fn source_text_for_indexed_path(&self, path: &Path) -> Option<Arc<str>> {
        self.files.get(path).map(Arc::clone)
    }

    /// Build stable per-source hashes for the cross-process OXC transform
    /// cache. Generated Visual input is part of the source identity because it
    /// is inserted by the transform but is not present in Rolldown hook code.
    pub fn transform_cache_source_hashes(&self) -> BTreeMap<String, String> {
        let mut hashes = BTreeMap::new();
        for (path, source) in self.files.iter() {
            let Ok(module_id) = arkdown_path::ArtifactPathSerializer::path_to_slash_string(path)
            else {
                continue;
            };
            hashes.insert(module_id, self.transform_cache_source_hash(path, source));
        }
        hashes
    }

    pub fn transform_cache_source_hash_for(&self, id: &str, hook_source: &str) -> String {
        let path = Path::new(id);
        let Some(indexed) = self.indexed_path(path) else {
            return Self::transform_cache_text_hash(hook_source);
        };
        let Some(source) = self.files.get(indexed) else {
            return Self::transform_cache_text_hash(hook_source);
        };
        self.transform_cache_source_hash(indexed, source)
    }

    fn transform_cache_source_hash(&self, path: &Path, source: &str) -> String {
        let mut hash = Sha256::new();
        hash.update(source.as_bytes());
        if let Some(visual) = self.visual_source(path) {
            for part in [
                visual.build.as_str(),
                visual.ets_import.as_str(),
                visual.ets_variable.as_str(),
                visual.ets_function.as_str(),
                visual.about_to_appear.as_str(),
            ] {
                hash.update([0]);
                hash.update(part.as_bytes());
            }
        }
        format!("{:x}", hash.finalize())
    }

    fn transform_cache_text_hash(source: &str) -> String {
        format!("{:x}", Sha256::digest(source.as_bytes()))
    }

    /// Return the external OHM URL declared by an inventoried SDK declaration.
    ///
    /// HarmonyOS SDK component declarations encode an external runtime target
    /// in their header as `@bundle <record> <version>`. The coordinator has
    /// already read those declarations into this snapshot, so transform hooks
    /// can consume that SDK metadata without filesystem access.
    pub(crate) fn sdk_bundle_ohm_url(
        &self,
        specifier: &str,
        compatible_api_version: u32,
    ) -> Option<String> {
        let declaration = self.resolve_sdk_declaration(specifier)?;
        SdkBundleDeclarationMetadata::new(&declaration.source).ohm_url(compatible_api_version)
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
            .indexed_path_owned(importer)
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
            .indexed_path_owned(importer)
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

    fn resolve_sdk_declaration(&self, specifier: &str) -> Option<TransformSourceFile> {
        let specifier = ModuleSpecifierText::new(specifier);
        let filenames = specifier
            .sdk_api_filenames()
            .into_iter()
            .chain(specifier.sdk_component_api_filenames())
            .chain(specifier.sdk_kit_filenames())
            .chain(specifier.sdk_arkts_filenames())
            .chain(specifier.sdk_runtime_declaration_filenames());
        for filename in filenames {
            for root in self.sdk_declaration_roots.iter() {
                if let Some(source) = self.source_file(root.join(&filename)) {
                    return Some(source);
                }
            }
        }
        None
    }

    fn source_file(&self, path: PathBuf) -> Option<TransformSourceFile> {
        let path = self.indexed_path_owned(&path)?;
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

    fn indexed_path_owned(&self, path: &Path) -> Option<PathBuf> {
        self.indexed_path(path).cloned()
    }
}

/// SDK declaration-header metadata consumed by the final ETS source pass.
///
/// The declaration format is owned by the SDK. This parser intentionally only
/// recognizes the JSDoc line shape used by SDK component declarations rather
/// than interpreting application source comments.
struct SdkBundleDeclarationMetadata<'a> {
    declaration: &'a str,
}

impl<'a> SdkBundleDeclarationMetadata<'a> {
    fn new(declaration: &'a str) -> Self {
        Self { declaration }
    }

    fn ohm_url(&self, compatible_api_version: u32) -> Option<String> {
        self.declaration.lines().find_map(|line| {
            let annotation = line.trim_start().strip_prefix('*')?.trim_start();
            let mut fields = annotation.split_ascii_whitespace();
            if fields.next()? != "@bundle" {
                return None;
            }
            let record = fields.next()?;
            let version = fields.next()?;
            if fields.next().is_some()
                || !SdkBundleVersion::new(version).is_compatible_with(compatible_api_version)
            {
                return None;
            }
            Some(["@bundle:", record].concat())
        })
    }
}

/// API-level requirement encoded in an SDK declaration's `@bundle` version.
///
/// This mirrors `api_check_utils.js::checkBundleVersion`: the number inside
/// parentheses is the minimum compatible API level; a non-numeric SDK version
/// without that suffix cannot select an external bundle target.
struct SdkBundleVersion<'a> {
    raw: &'a str,
}

impl<'a> SdkBundleVersion<'a> {
    fn new(raw: &'a str) -> Self {
        Self { raw }
    }

    fn is_compatible_with(&self, compatible_api_version: u32) -> bool {
        if compatible_api_version == 0 {
            return true;
        }
        let api_version = self
            .raw
            .rsplit_once('(')
            .and_then(|(_, value)| value.strip_suffix(')'))
            .and_then(|value| value.parse::<u32>().ok())
            .or_else(|| self.raw.parse::<u32>().ok());
        api_version.is_some_and(|api_version| api_version <= compatible_api_version)
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
            // TypeScript module resolution appends declaration extensions to
            // dotted SDK basenames such as `@ohos.deviceInfo`; Path::with_extension
            // alone would incorrectly replace `.deviceInfo`.
            candidates.push(self.with_appended_extension("d.ets"));
            candidates.push(self.with_appended_extension("d.ts"));
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

    fn with_appended_extension(&self, extension: &str) -> PathBuf {
        let Some(file_name) = self.candidate.file_name() else {
            return self.candidate.clone();
        };
        let mut appended = file_name.to_os_string();
        appended.push(".");
        appended.push(extension);
        self.candidate.with_file_name(appended)
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
    fn source_language_matches_current_hybrid_module_detection() {
        let static_source = ArktsSourceLanguageDetector::new(
            "\u{feff}// heading\n/* block\n * comment\n */ 'use static'\nexport class A {}",
        );
        assert!(static_source.is_static());
        assert_eq!(
            static_source.version_for_path(ArktsLanguageVersion::V1_2, Path::new("Index.ets")),
            ArktsLanguageVersion::V1_2
        );

        let generated_static_source =
            ArktsSourceLanguageDetector::new("'use static'\nexport class A {}");
        assert!(generated_static_source.is_static_dependency_compile_file());
        assert_eq!(
            generated_static_source
                .version_for_path(ArktsLanguageVersion::V1_2, Path::new("Index.ets")),
            ArktsLanguageVersion::V1_2
        );
        assert_eq!(
            generated_static_source
                .version_for_path(ArktsLanguageVersion::V1_2, Path::new("Index.ts")),
            ArktsLanguageVersion::V1_1
        );

        let dynamic_source = ArktsSourceLanguageDetector::new(
            "/* heading */\nimport('./static/Init');\n'use static'",
        );
        assert!(!dynamic_source.is_static());
        assert_eq!(
            dynamic_source.version_for_path(ArktsLanguageVersion::V1_2, Path::new("Index.ets")),
            ArktsLanguageVersion::V1_1
        );
        assert_eq!(
            dynamic_source.version_for_path(ArktsLanguageVersion::V1_2, Path::new("Index.ets")),
            ArktsLanguageVersion::V1_1
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
    fn compile_arkts_source_type_treats_runtime_ts_as_arkts_but_keeps_declarations() {
        let ts = CompileArktsSourceType::from_id("src/main/ts/Player.ts");
        assert!(ts.is_typescript());
        assert!(ts.is_arkui());
        assert!(CompileArktsSourceType::from_id("src/main/ets/Page.ets").is_arkui());

        let declaration = CompileArktsSourceType::from_id("types/libplayer/index.d.ts");
        assert!(declaration.is_typescript_definition());
        assert!(!declaration.is_arkui());
        assert!(!CompileArktsSourceType::from_id("src/main/js/index.js").is_typescript());
    }

    #[test]
    fn transform_cache_source_hash_tracks_generated_visual_input() {
        let source_path = PathBuf::from("/project/entry/src/main/ets/Index.ets");
        let source = Arc::<str>::from("@Component struct Index { build() {} }");
        let files = FxHashMap::from_iter([(source_path.clone(), Arc::clone(&source))]);
        let plain = TransformSourceSnapshot::new(files.clone());
        let visual =
            TransformSourceSnapshot::new(files).with_visual_sources(FxHashMap::from_iter([(
                source_path.clone(),
                VisualGeneratedSource {
                    build: "Text(\"visual\")".to_string(),
                    ..VisualGeneratedSource::default()
                },
            )]));
        let id = arkdown_path::ArtifactPathSerializer::path_to_slash_string(&source_path)
            .expect("portable module id");

        assert_ne!(
            plain.transform_cache_source_hash_for(&id, &source),
            visual.transform_cache_source_hash_for(&id, &source),
            "Visual fragments are transform inputs even though Rolldown hook code is unchanged"
        );
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
        junction::create(&real_root, &alias_root).unwrap();

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
            arkdown_path::canonicalize_or_owned(&real_root.join("types.d.ts"))
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
        junction::create(&real_root, &alias_root).unwrap();

        let mut files = FxHashMap::default();
        files.insert(
            arkdown_path::canonicalize_or_owned(&source_path),
            Arc::<str>::from(""),
        );
        files.insert(
            arkdown_path::canonicalize_or_owned(&declaration_path),
            Arc::<str>::from("export interface EncryptionResult {}"),
        );
        let sources = TransformSourceSnapshot::new(files)
            .with_path_aliases(vec![(alias_root.clone(), real_root.clone())]);

        let resolved = sources
            .resolve_relative_with_declarations(
                &alias_root.join("src/main/ets/core/AES.ets"),
                "../../cpp/types/libmtcrypto",
            )
            .expect("alias importer should resolve declaration stored by canonical path");
        assert_eq!(
            resolved.path,
            arkdown_path::canonicalize_or_owned(&declaration_path)
        );
    }

    #[test]
    fn snapshot_resolves_source_and_runtime_sdk_specifiers_from_inventoried_roots() {
        let tmp = tempfile::tempdir().unwrap();
        let api_root = tmp.path().join("ets/api");
        std::fs::create_dir_all(&api_root).unwrap();
        let declaration = api_root.join("@ohos.multimedia.media.d.ts");
        let external_declaration = api_root.join("@arkui-x.telephony.d.ts");
        std::fs::write(
            &declaration,
            "declare namespace media {}\nexport default media;",
        )
        .unwrap();
        std::fs::write(
            &external_declaration,
            "export declare function dial(): void;",
        )
        .unwrap();

        let mut files = FxHashMap::default();
        files.insert(
            declaration.clone(),
            Arc::<str>::from("declare namespace media {}\nexport default media;"),
        );
        files.insert(
            external_declaration.clone(),
            Arc::<str>::from("export declare function dial(): void;"),
        );
        let sources =
            TransformSourceSnapshot::new(files).with_sdk_declaration_roots(vec![api_root]);
        let importer = tmp.path().join("entry/src/main/ets/Index.ets");

        for specifier in ["@ohos.multimedia.media", "@ohos:multimedia.media"] {
            let resolved = sources
                .resolve_static_with_declarations(&importer, specifier)
                .unwrap_or_else(|| panic!("SDK declaration must resolve for {specifier}"));
            assert_eq!(
                resolved.path,
                arkdown_path::canonicalize_or_owned(&declaration)
            );
        }
        let external = sources
            .resolve_static_with_declarations(&importer, "@arkui-x.telephony")
            .expect("external SDK component declaration must resolve");
        assert_eq!(
            external.path,
            arkdown_path::canonicalize_or_owned(&external_declaration)
        );
    }

    #[test]
    fn snapshot_reads_harmony_component_bundle_annotation_from_sdk_declaration() {
        let tmp = tempfile::tempdir().unwrap();
        let hms_api_root = tmp.path().join("hms/ets/api");
        std::fs::create_dir_all(&hms_api_root).unwrap();
        let component_declaration = hms_api_root.join("@hms.core.map.MapComponent.d.ets");
        let navi_declaration = hms_api_root.join("@hms.core.map.navi.d.ts");
        let component_source = r#"/**
 * @bundle com.huawei.hms.mapservice.kit/mapLibrary/ets/MapComponent 5.0.0(12)
 * @kit MapKit
 */
export struct MapComponent {}"#;
        std::fs::write(&component_declaration, component_source).unwrap();
        std::fs::write(&navi_declaration, "export default navi;").unwrap();

        let mut files = FxHashMap::default();
        files.insert(component_declaration, Arc::<str>::from(component_source));
        files.insert(navi_declaration, Arc::<str>::from("export default navi;"));
        let sources =
            TransformSourceSnapshot::new(files).with_sdk_declaration_roots(vec![hms_api_root]);

        assert_eq!(
            sources.sdk_bundle_ohm_url("@hms.core.map.MapComponent", 12),
            Some("@bundle:com.huawei.hms.mapservice.kit/mapLibrary/ets/MapComponent".to_string())
        );
        assert_eq!(
            sources.sdk_bundle_ohm_url("@hms.core.map.MapComponent", 11),
            None,
            "latest SDK api_check_utils::checkBundleVersion rejects an annotation above compatibleSdkVersion"
        );
        assert_eq!(sources.sdk_bundle_ohm_url("@hms.core.map.navi", 12), None);
    }
}
