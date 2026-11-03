use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arkdown_core::{
    ArktsLanguageVersion, ModuleId, OhosResolvedModule, SourceFileId, SourceInventory,
    SourceReadError,
};
use arkdown_path::{
    canonicalize_or_owned, classify::PathClassifier, lexical_normalize, SourceFileName,
};
use arkdown_resolver::{
    OhosResolver, PackageResolutionPlanBuilder, PkgBriefInfo, ResolveImportKind, ResolverConfig,
};
use oxc_allocator::Allocator;
use oxc_ast::ast::{self, Argument, Expression, StringLiteral, TSModuleReference};
use oxc_ast_visit::{walk, Visit};
use oxc_index::IndexVec;
use oxc_parser::Parser;
use oxc_span::SourceType;
use rayon::prelude::*;
use regex::Regex;
use rustc_hash::FxHashMap;

use super::{SourceContent, SourceFile, SourceInventoryPaths, SourceStore};
use crate::source_inputs::PackageSourceInputs;
use crate::{PackageSnapshot, ProjectSnapshot};

/// Startup-only graph expansion. Mirrors ets_checker.ts::resolveModuleNames /
/// TypeScript processImportedModules and Rollup's literal import/require graph;
/// it does not turn candidate files into checker roots or compiler outputs.
pub(super) struct SourceReferenceInventory {
    resolvers: IndexVec<ModuleId, OhosResolver>,
    roots: Vec<(PathBuf, ModuleId)>,
    entries: Vec<(PathBuf, ModuleId)>,
    version: ArktsLanguageVersion,
    seeds: BTreeMap<PathBuf, BTreeSet<PathBuf>>,
    seed_roots: Vec<(PathBuf, PathBuf)>,
}

impl SourceReferenceInventory {
    pub(super) fn new(project: &ProjectSnapshot, version: ArktsLanguageVersion) -> Self {
        let mut identities = SourceRootIndex::default();
        for graph in project.dependency_graphs.values() {
            identities.roots.extend(
                graph
                    .canonical_package_roots
                    .iter()
                    .map(|(alias, root)| (alias.clone(), root.clone())),
            );
            for edge in &graph.edges {
                identities
                    .roots
                    .insert(edge.package_root.clone(), edge.package_root.clone());
                if let Some(alias) = &edge.installed_alias_path {
                    identities
                        .roots
                        .insert(alias.clone(), edge.package_root.clone());
                }
            }
        }
        let identities: Arc<dyn SourceInventory> = Arc::new(identities);
        let mut resolvers = IndexVec::new();
        let mut roots = Vec::new();
        let mut entries = Vec::new();
        let mut module_owners = FxHashMap::default();
        let mut seeds = BTreeMap::<PathBuf, BTreeSet<PathBuf>>::new();
        let mut seed_roots = Vec::new();
        for (name, module) in &project.modules {
            let mut packages = PackageResolutionPlanBuilder::new();
            let index = ModuleId::new(resolvers.len());
            let module_root = canonicalize_or_owned(&module.root);
            module_owners.insert(module_root.clone(), index);
            roots.push((module_root.clone(), index));
            for root in &module.source_roots {
                let root = canonicalize_or_owned(root);
                module_owners.insert(root.clone(), index);
                roots.push((root, index));
            }
            seed_roots.extend(
                module
                    .source_roots
                    .iter()
                    .map(|root| (module_root.clone(), root.clone())),
            );
            seed_roots.push((module_root.clone(), module.src_main.join("supervisual")));
            let graph = project.dependency_graphs.get(name);
            if let Some(graph) = graph {
                for node in graph.nodes.values() {
                    if let Some(root) = &node.package_root {
                        seeds
                            .entry(canonicalize_or_owned(root))
                            .or_default()
                            .extend(node.compile_inputs.source_inventory_files.iter().cloned());
                        let inputs = &node.compile_inputs;
                        for path in inputs
                            .ability_source_files
                            .iter()
                            .chain(inputs.worker_source_files.iter())
                            .chain(inputs.insight_intent_source_files.iter())
                            .chain(inputs.dynamic_import_entry_file.iter())
                            .chain(inputs.other_compile_entry_file.iter())
                            .chain(inputs.extra_importee_source_files.values())
                        {
                            seeds
                                .entry(canonicalize_or_owned(root))
                                .or_default()
                                .insert(path.clone());
                            entries.push((path.clone(), index));
                        }
                    }
                }
                for edge in &graph.edges {
                    let Some(package) = graph
                        .nodes
                        .get(&edge.package_name)
                        .and_then(|node| node.package.as_deref())
                    else {
                        continue;
                    };
                    packages.insert_dependency(&edge.alias, Self::package_info(package, false));
                    roots.push((canonicalize_or_owned(&edge.package_root), index));
                    roots.extend(
                        PackageSourceInputs::new(&package.root, Some(package))
                            .roots
                            .into_iter()
                            .filter(|root| root.is_dir())
                            .map(|root| (canonicalize_or_owned(&root), index)),
                    );
                }
            }
            if let Some(package) = module.package.as_deref() {
                packages.insert_current(Self::package_info(package, true));
            }
            let mut source_roots = module.source_roots.clone();
            source_roots.push(module.src_main.clone());
            let resolver = OhosResolver::new(ResolverConfig {
                project_root: project.root.clone(),
                sdk_api_dirs: Vec::new(),
                sdk_kits_dirs: Vec::new(),
                sdk_arkts_dirs: Vec::new(),
                source_roots,
                module_paths: Vec::new(),
                package_resolution: Arc::new(packages.build()),
                use_normalized_ohm_url: false,
                case_sensitive_check: false,
                source_set_overlays: Vec::new(),
            })
            .with_source_inventory(Some(Arc::clone(&identities)));
            // Directory/extensionless package entries are selected by the
            // existing resolver, rather than by walking their parent directory.
            let importer = module.package_path.as_deref().unwrap_or(&module.root);
            for (alias, root) in graph
                .into_iter()
                .flat_map(|graph| {
                    graph
                        .edges
                        .iter()
                        .map(|edge| (edge.alias.as_str(), edge.package_root.as_path()))
                })
                .chain(
                    module
                        .package
                        .iter()
                        .map(|package| (package.name.as_str(), package.root.as_path())),
                )
            {
                if let Ok(resolved) = resolver.resolve(alias, importer) {
                    if let Some(path) = Self::source_path(resolved) {
                        seeds
                            .entry(canonicalize_or_owned(root))
                            .or_default()
                            .insert(path.clone());
                        entries.push((path, index));
                    }
                }
            }
            for package in graph
                .into_iter()
                .flat_map(|graph| {
                    graph
                        .nodes
                        .values()
                        .filter_map(|node| node.package.as_deref())
                })
                .chain(module.package.iter().map(AsRef::as_ref))
            {
                for entry in PackageSourceInputs::new(&package.root, Some(package)).entries {
                    if entry.is_file() {
                        seeds
                            .entry(canonicalize_or_owned(&package.root))
                            .or_default()
                            .insert(entry.clone());
                        entries.push((entry, index));
                        continue;
                    }
                    let Some(request) = entry
                        .strip_prefix(&package.root)
                        .ok()
                        .and_then(Path::to_str)
                    else {
                        continue;
                    };
                    if let Some(path) = resolver.resolve_package_path(
                        &package.root,
                        request,
                        ResolveImportKind::Import,
                    ) {
                        seeds
                            .entry(canonicalize_or_owned(&package.root))
                            .or_default()
                            .insert(path.clone());
                        entries.push((path, index));
                    }
                }
            }
            resolvers.push(resolver);
        }
        roots.sort_by(|left, right| {
            right
                .0
                .components()
                .count()
                .cmp(&left.0.components().count())
                .then_with(|| left.0.cmp(&right.0))
                .then_with(|| {
                    let preferred = module_owners.get(&left.0);
                    (preferred != Some(&left.1)).cmp(&(preferred != Some(&right.1)))
                })
                .then_with(|| left.1.cmp(&right.1))
        });
        roots.dedup();
        entries.sort_unstable();
        entries.dedup();
        Self {
            resolvers,
            roots,
            entries,
            version,
            seeds,
            seed_roots,
        }
    }

    fn package_info(package: &PackageSnapshot, current: bool) -> PkgBriefInfo {
        let classification = PathClassifier::new(&package.root);
        let local_source = current || !classification.is_inside_dependency_modules();
        let source_roots = if local_source {
            vec![crate::paths::ModulePathLayout::new(&package.root).src_main()]
        } else {
            package
                .package_json
                .extra
                .get("metadata")
                .and_then(|metadata| metadata.get("sourceRoots"))
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(serde_json::Value::as_str)
                .map(|root| package.root.join(root))
                .collect()
        };
        PkgBriefInfo {
            pkg_root: package.root.clone(),
            pkg_name: package.name.clone(),
            manifest: package.resolver_manifest(),
            local_source,
            source_roots,
        }
    }

    pub(super) fn capture(mut self, store: &mut SourceStore) -> Result<(), std::io::Error> {
        for (owner, root) in &self.seed_roots {
            self.seeds.entry(owner.clone()).or_default().extend(
                store
                    .source_files_under(root)
                    .into_iter()
                    .map(|file| file.path.clone()),
            );
        }
        let mut state = SourceReferenceCapture::new(store, &self.roots);
        for (path, owner) in &self.entries {
            if Self::accepts(path) && path.is_file() {
                state.capture_file(store, path, *owner)?;
            }
        }
        let mut pending = state
            .paths
            .indices()
            .filter(|id| state.owners[*id].is_some())
            .collect::<Vec<_>>();
        let pragmas = SourceReferencePragmas::new();
        while !pending.is_empty() {
            let requests = pending
                .par_iter()
                .map(|&id| {
                    let file = &store.files[&state.paths[id]];
                    let requests = match file.cached_content().map_err(std::io::Error::other)? {
                        SourceContent::Text(source) => {
                            SourceRequests::collect(&file.path, &source, self.version, &pragmas)
                        }
                        SourceContent::Bytes(_) => Vec::new(),
                    };
                    Ok::<_, std::io::Error>((id, requests))
                })
                .collect::<Vec<_>>();
            pending.clear();
            for result in requests {
                let (id, requests) = result?;
                let owner = state.owners[id].expect("project source has a resolver");
                for request in requests {
                    let resolved = match request {
                        SourceRequest::Module { specifier, kind } => self.resolvers[owner]
                            .resolve_with_kind(&specifier, &state.paths[id], kind)
                            .ok()
                            .and_then(Self::source_path),
                        SourceRequest::Path(path) => state.paths[id]
                            .parent()
                            .map(|parent| lexical_normalize(&parent.join(path)))
                            .filter(|path| path.is_file())
                            .map(|path| canonicalize_or_owned(&path)),
                    };
                    let Some(path) = resolved.filter(|path| Self::accepts(path)) else {
                        continue;
                    };
                    let previous_len = state.paths.len();
                    let dependency = state.capture_file(store, &path, owner)?;
                    store.references[id].push(dependency);
                    if state.paths.len() != previous_len {
                        pending.push(dependency);
                    }
                }
            }
        }
        // Restore path-ordered dense IDs after deterministic graph expansion.
        let sorted = store
            .source_files()
            .iter()
            .map(|file| (file.path.clone(), file.id))
            .collect::<Vec<_>>();
        let mut remap = IndexVec::<SourceFileId, SourceFileId>::from_vec(vec![
            SourceFileId::new(0);
            state.paths.len()
        ]);
        for (index, (_, old_id)) in sorted.iter().enumerate() {
            remap[*old_id] = SourceFileId::new(index);
        }
        let mut references = IndexVec::with_capacity(sorted.len());
        for (index, (path, old_id)) in sorted.into_iter().enumerate() {
            store.files.get_mut(&path).expect("captured source").id = SourceFileId::new(index);
            let mut dependencies = store.references[old_id]
                .iter()
                .map(|id| remap[*id])
                .collect::<Vec<_>>();
            dependencies.sort_unstable();
            dependencies.dedup();
            references.push(dependencies);
        }
        store.references = references;
        store.compiler_canonical_files = (!SourceStore::compiler_uses_case_sensitive_file_names())
            .then(|| SourceStore::compiler_canonical_file_index(store.files.keys()));
        store.input_seeds = self
            .seeds
            .into_iter()
            .map(|(root, paths)| {
                let mut ids = paths
                    .into_iter()
                    .filter_map(|path| store.source_by_path(&path).map(|file| file.id))
                    .collect::<Vec<_>>();
                ids.sort_unstable();
                ids.dedup();
                (root, Arc::from(ids))
            })
            .collect();
        Ok(())
    }

    fn accepts(path: &Path) -> bool {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(super::is_source_inventory_ext)
    }

    fn source_path(resolved: OhosResolvedModule) -> Option<PathBuf> {
        match resolved {
            OhosResolvedModule::LocalSource { path, .. }
            | OhosResolvedModule::RemotePackage { path, .. } => Some(path),
            OhosResolvedModule::BytecodeHar {
                declaration_path, ..
            } => declaration_path.is_file().then_some(declaration_path),
            _ => None,
        }
    }
}

struct SourceReferenceCapture<'a> {
    paths: IndexVec<SourceFileId, PathBuf>,
    owners: IndexVec<SourceFileId, Option<ModuleId>>,
    physical_paths: SourceInventoryPaths,
    roots: &'a [(PathBuf, ModuleId)],
}

impl<'a> SourceReferenceCapture<'a> {
    fn new(store: &SourceStore, roots: &'a [(PathBuf, ModuleId)]) -> Self {
        let paths = IndexVec::<SourceFileId, PathBuf>::from_vec(
            store
                .source_files()
                .iter()
                .map(|file| file.path.clone())
                .collect(),
        );
        let owners = IndexVec::from_vec(
            paths
                .iter()
                .map(|path| {
                    roots
                        .iter()
                        .find(|(root, _)| path.starts_with(root))
                        .map(|(_, index)| *index)
                })
                .collect(),
        );
        let mut physical_paths = SourceInventoryPaths::default();
        for path in &paths {
            physical_paths.remember_reference_path(path);
        }
        Self {
            paths,
            owners,
            physical_paths,
            roots,
        }
    }

    fn capture_file(
        &mut self,
        store: &mut SourceStore,
        path: &Path,
        owner: ModuleId,
    ) -> Result<SourceFileId, std::io::Error> {
        if let Some(file) = store.files.get(path) {
            return Ok(file.id);
        }
        let path = self.physical_paths.canonical_referenced_file(path)?;
        if let Some(file) = store.files.get(&path) {
            return Ok(file.id);
        }
        let id = self.paths.push(path.clone());
        let owner = self
            .roots
            .iter()
            .find(|(root, _)| path.starts_with(root))
            .map_or(owner, |(_, index)| *index);
        self.owners.push(Some(owner));
        let file = SourceFile::capture(id, path.clone(), Arc::clone(&store.on_demand_reloads))?;
        store.files.insert(path, file);
        store.references.push(Vec::new());
        Ok(id)
    }
}

/// Directory identities frozen by dependency discovery. Directories have no
/// source text; this index prevents resolver entry lookups from reloading the
/// manifests already projected into the package plan.
#[derive(Debug, Default)]
struct SourceRootIndex {
    roots: FxHashMap<PathBuf, PathBuf>,
}

impl SourceInventory for SourceRootIndex {
    fn canonical_path(&self, path: &Path) -> Option<&Path> {
        self.roots.get(path).map(PathBuf::as_path)
    }
    fn source_text(&self, _path: &Path) -> Result<Option<Arc<str>>, SourceReadError> {
        Ok(None)
    }
}

enum SourceRequest {
    Module {
        specifier: String,
        kind: ResolveImportKind,
    },
    Path(PathBuf),
}

#[derive(Default)]
struct SourceRequests {
    requests: Vec<SourceRequest>,
}

impl SourceRequests {
    fn collect(
        path: &Path,
        source: &str,
        version: ArktsLanguageVersion,
        pragmas: &SourceReferencePragmas,
    ) -> Vec<SourceRequest> {
        if !matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("ets" | "ts" | "js" | "mjs" | "cjs")
        ) {
            return Vec::new();
        }
        let allocator = Allocator::default();
        let source_type = if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("ets" | "ts")
        ) && !SourceFileName::new(path).is_declaration()
        {
            if version.is_static() {
                SourceType::ets_static()
            } else {
                SourceType::ets()
            }
        } else {
            SourceType::from_path(path).unwrap_or_else(|_| SourceType::ts())
        };
        let parsed = Parser::new(&allocator, source, source_type).parse();
        let mut collector = Self::default();
        collector.visit_program(&parsed.program);
        if path.extension().is_some_and(|extension| extension == "js") {
            // ets_checker.ts::resolveModuleNames and OH TypeScript's
            // tryAddingExtensions substitute these supported .js companions.
            for extension in ["ets", "ts", "d.ets", "d.ts"] {
                collector
                    .requests
                    .push(SourceRequest::Path(path.with_extension(extension)));
            }
        }
        pragmas.collect(&parsed.program, source, &mut collector.requests);
        collector.requests
    }

    fn push(&mut self, literal: &StringLiteral<'_>, kind: ResolveImportKind) {
        if let Some(specifier) = literal.value.as_str() {
            self.requests.push(SourceRequest::Module {
                specifier: specifier.to_owned(),
                kind,
            });
        }
    }
}

impl<'a> Visit<'a> for SourceRequests {
    fn visit_import_declaration(&mut self, node: &ast::ImportDeclaration<'a>) {
        self.push(&node.source, ResolveImportKind::Import);
        walk::walk_import_declaration(self, node);
    }
    fn visit_lazy_import_declaration(&mut self, node: &ast::LazyImportDeclaration<'a>) {
        self.push(&node.source, ResolveImportKind::Import);
        walk::walk_lazy_import_declaration(self, node);
    }
    fn visit_export_all_declaration(&mut self, node: &ast::ExportAllDeclaration<'a>) {
        self.push(&node.source, ResolveImportKind::Import);
        walk::walk_export_all_declaration(self, node);
    }
    fn visit_export_from_declaration(&mut self, node: &ast::ExportFromDeclaration<'a>) {
        self.push(&node.source, ResolveImportKind::Import);
        walk::walk_export_from_declaration(self, node);
    }
    fn visit_import_expression(&mut self, node: &ast::ImportExpression<'a>) {
        if let Expression::StringLiteral(source) = &node.source {
            self.push(source, ResolveImportKind::Import);
        }
        walk::walk_import_expression(self, node);
    }
    fn visit_ts_import_type(&mut self, node: &ast::TSImportType<'a>) {
        self.push(&node.source, ResolveImportKind::Import);
        walk::walk_ts_import_type(self, node);
    }
    fn visit_ts_import_equals_declaration(&mut self, node: &ast::TSImportEqualsDeclaration<'a>) {
        if let TSModuleReference::ExternalModuleReference(reference) = &node.module_reference {
            self.push(&reference.expression, ResolveImportKind::Require);
        }
        walk::walk_ts_import_equals_declaration(self, node);
    }
    fn visit_call_expression(&mut self, node: &ast::CallExpression<'a>) {
        if matches!(&node.callee, Expression::Identifier(identifier) if identifier.name == "require")
            && node.arguments.len() == 1
        {
            if let Argument::StringLiteral(source) = &node.arguments[0] {
                self.push(source, ResolveImportKind::Require);
            }
        }
        walk::walk_call_expression(self, node);
    }
}

/// OH TypeScript processCommentPragmas/extractPragmas: only leading comments,
/// XML reference attributes, with no-default-lib/types/lib precedence over path.
struct SourceReferencePragmas {
    reference: Regex,
    attributes: Regex,
}

impl SourceReferencePragmas {
    fn new() -> Self {
        Self {
            reference: Regex::new(r"(?im)^///\s*<reference\s.*?/>").unwrap(),
            attributes: Regex::new(
                r#"(?im)\s(path|types|lib|no-default-lib)\s*=\s*(?:'([^']*)'|"([^"]*)")"#,
            )
            .unwrap(),
        }
    }

    fn collect(&self, program: &ast::Program<'_>, source: &str, requests: &mut Vec<SourceRequest>) {
        let mut previous = program
            .hashbang
            .as_ref()
            .map_or(0, |hashbang| hashbang.span.end as usize);
        for comment in &program.comments {
            let start = comment.span.start as usize;
            let end = comment.span.end as usize;
            if !source[previous..start]
                .trim_matches(|ch: char| ch.is_whitespace() || ch == '\u{feff}')
                .is_empty()
            {
                break;
            }
            previous = end;
            let text = &source[start..end];
            if comment.kind != ast::CommentKind::Line || !self.reference.is_match(text) {
                continue;
            }
            let mut path = None;
            let mut other = false;
            let mut seen = rustc_hash::FxHashSet::default();
            for attribute in self.attributes.captures_iter(text) {
                let name = attribute[1].to_ascii_lowercase();
                if !seen.insert(name.clone()) {
                    continue;
                }
                let value = attribute
                    .get(2)
                    .or_else(|| attribute.get(3))
                    .expect("quoted pragma argument")
                    .as_str();
                if name == "path" {
                    path = Some(PathBuf::from(value));
                } else {
                    other |= name != "no-default-lib" || !value.is_empty();
                }
            }
            if !other {
                if let Some(path) = path {
                    requests.push(SourceRequest::Path(path));
                }
            }
        }
    }
}
