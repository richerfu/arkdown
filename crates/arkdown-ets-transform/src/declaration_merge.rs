//! Native ArkTS 1.1 bundled-declaration graph.
//!
//! The SDK's `DeclarationMerger` starts at each public HAR/HSP declaration
//! entry, follows re-exports, embeds private type dependencies, and removes
//! declarations that are no longer reachable. OXC owns parsing and reference
//! spans here; the build coordinator only snapshots inputs and persists the
//! returned files.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Declaration, ExportDefaultDeclarationKind, ImportDeclarationSpecifier, ModuleExportName,
    Statement, TSTypeName,
};
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_span::{GetSpan, Span};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::CompileArktsSourceType;

/// Files that remain after one declaration merge.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct BundledDeclarations {
    pub files: BTreeMap<PathBuf, String>,
}

/// OXC-backed declaration graph and emitter.
pub struct DeclarationMerger {
    sources: BTreeMap<PathBuf, String>,
    models: FxHashMap<PathBuf, DeclarationFile>,
    package_entries: BTreeMap<String, PathBuf>,
    package_roots: BTreeMap<String, PathBuf>,
}

impl DeclarationMerger {
    pub fn new(sources: BTreeMap<PathBuf, String>) -> Self {
        Self {
            sources: sources
                .into_iter()
                .map(|(path, source)| (Self::normalize_path(&path), source))
                .collect(),
            models: FxHashMap::default(),
            package_entries: BTreeMap::new(),
            package_roots: BTreeMap::new(),
        }
    }

    /// Register resolved OHPM package declaration entries from the immutable
    /// dependency snapshot. Both dependency aliases and canonical package
    /// names may point at the same entry.
    pub fn with_package_entries<I, S, P>(mut self, entries: I) -> Self
    where
        I: IntoIterator<Item = (S, P)>,
        S: Into<String>,
        P: Into<PathBuf>,
    {
        self.package_entries = entries
            .into_iter()
            .map(|(name, path)| (name.into(), Self::normalize_path(&path.into())))
            .collect();
        self
    }

    /// Register both package roots and declaration entries. Roots allow the
    /// declaration graph to resolve package subpaths with the same extension
    /// and index probing used for relative specifiers.
    pub fn with_package_declarations<I, S, P, E>(mut self, packages: I) -> Self
    where
        I: IntoIterator<Item = (S, P, E)>,
        S: Into<String>,
        P: Into<PathBuf>,
        E: Into<PathBuf>,
    {
        for (name, root, entry) in packages {
            let name = name.into();
            self.package_roots
                .insert(name.clone(), Self::normalize_path(&root.into()));
            self.package_entries
                .insert(name, Self::normalize_path(&entry.into()));
        }
        self
    }

    /// Merge all public entries in declaration order. The output map is path
    /// sorted so archive bytes do not depend on hash iteration order.
    pub fn merge(mut self, entries: &[PathBuf]) -> Result<BundledDeclarations> {
        let mut files = BTreeMap::new();
        for entry in entries {
            let entry = Self::normalize_path(entry);
            if !self.sources.contains_key(&entry) {
                tracing::debug!(
                    entry = %entry.display(),
                    "bundled declaration entry is absent from the declaration snapshot"
                );
                continue;
            }
            let emitted = DeclarationEntryEmitter::new(&mut self, &entry).emit()?;
            files.insert(entry.clone(), emitted.primary);
            if let Some((path, content)) = emitted.companion {
                files.insert(path, content);
            }
        }
        Ok(BundledDeclarations { files })
    }

    fn model(&mut self, path: &Path) -> Result<&DeclarationFile> {
        let path = Self::normalize_path(path);
        if !self.models.contains_key(&path) {
            let source = self.sources.get(&path).with_context(|| {
                format!("declaration module was not snapshotted: {}", path.display())
            })?;
            self.models.insert(
                path.clone(),
                DeclarationFileParser::new(&path, source).parse()?,
            );
        }
        Ok(self.models.get(&path).expect("declaration model inserted"))
    }

    fn resolve(&self, importer: &Path, specifier: &str) -> Option<PathBuf> {
        if !specifier.starts_with('.') {
            if let Some(entry) = self.package_entries.get(specifier) {
                return Some(entry.clone());
            }
            let package = self
                .package_roots
                .iter()
                .filter_map(|(name, root)| {
                    specifier
                        .strip_prefix(name)
                        .and_then(|suffix| suffix.strip_prefix('/'))
                        .map(|subpath| (name.len(), root.join(subpath)))
                })
                .max_by_key(|(name_len, _)| *name_len);
            return package.and_then(|(_, base)| self.resolve_path(base));
        }
        self.resolve_path(importer.parent()?.join(specifier))
    }

    fn resolve_path(&self, base: PathBuf) -> Option<PathBuf> {
        let mut candidates = Vec::with_capacity(8);
        if base.extension().is_some() {
            candidates.push(Self::declaration_path(&base));
            candidates.push(base);
        } else {
            candidates.push(base.with_extension("d.ets"));
            candidates.push(base.with_extension("d.ts"));
            candidates.push(base.join("Index.d.ets"));
            candidates.push(base.join("Index.d.ts"));
            candidates.push(base.join("index.d.ets"));
            candidates.push(base.join("index.d.ts"));
        }
        candidates
            .into_iter()
            .map(|candidate| Self::normalize_path(&candidate))
            .find(|candidate| self.sources.contains_key(candidate))
    }

    fn declaration_path(path: &Path) -> PathBuf {
        let text = path.to_string_lossy();
        if text.ends_with(".d.ets") || text.ends_with(".d.ts") {
            return path.to_path_buf();
        }
        if let Some(prefix) = text.strip_suffix(".ets") {
            return PathBuf::from(format!("{prefix}.d.ets"));
        }
        if let Some(prefix) = text.strip_suffix(".ts") {
            return PathBuf::from(format!("{prefix}.d.ts"));
        }
        path.to_path_buf()
    }

    fn normalize_path(path: &Path) -> PathBuf {
        arkdown_path::lexical_normalize(path)
    }
}

#[derive(Clone, Debug)]
struct DeclarationFile {
    declarations: FxHashMap<String, Vec<OwnedDeclaration>>,
    imports: FxHashMap<String, ImportBinding>,
    direct_exports: FxHashMap<String, String>,
    default_export: Option<String>,
    reexports: Vec<Reexport>,
    export_all: Vec<ExportAll>,
}

impl DeclarationFile {
    fn declaration(&self, name: &str) -> Option<&[OwnedDeclaration]> {
        self.declarations.get(name).map(Vec::as_slice)
    }
}

#[derive(Clone, Debug)]
struct OwnedDeclaration {
    text: String,
    references: Vec<DeclarationReference>,
    is_type_alias: bool,
}

#[derive(Clone, Debug)]
struct DeclarationReference {
    name: String,
    member: Option<String>,
    specifier: Option<String>,
    start: usize,
    end: usize,
}

#[derive(Clone, Debug)]
struct ImportBinding {
    specifier: String,
    imported: ImportedName,
}

#[derive(Clone, Debug)]
enum ImportedName {
    Named(String),
    Default,
    Namespace,
}

#[derive(Clone, Debug)]
struct Reexport {
    specifier: String,
    imported: String,
    exported: String,
    original: String,
}

#[derive(Clone, Debug)]
struct ExportAll {
    specifier: String,
    namespace: Option<String>,
    original: String,
}

struct DeclarationFileParser<'a> {
    path: &'a Path,
    source: &'a str,
}

impl<'a> DeclarationFileParser<'a> {
    fn new(path: &'a Path, source: &'a str) -> Self {
        Self { path, source }
    }

    fn parse(&self) -> Result<DeclarationFile> {
        let sanitized = DeclarationEntryEmitter::sanitize_declaration(self.source);
        Self::parse_sanitized(self.path, &sanitized)
    }

    fn parse_sanitized(path: &Path, source: &str) -> Result<DeclarationFile> {
        let parser = DeclarationFileParser { path, source };
        let allocator = Allocator::default();
        // SDK declaration programs accept merge inputs such as
        // `let value = 1; export default value;`. OXC's declaration-file mode
        // diagnoses that initializer as ambient, so use the matching language
        // grammar and let this graph select declaration reachability.
        let grammar_path = if path.to_string_lossy().ends_with(".d.ets") {
            Path::new("__arkdown_merge__.ets")
        } else {
            Path::new("__arkdown_merge__.ts")
        };
        let parsed = Parser::new(
            &allocator,
            source,
            CompileArktsSourceType::from_path(grammar_path),
        )
        .parse();
        if !parsed.diagnostics.is_empty() {
            anyhow::bail!(
                "parse bundled declaration {} failed:\n{}",
                path.display(),
                parsed
                    .diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            );
        }
        let mut file = DeclarationFile {
            declarations: FxHashMap::default(),
            imports: FxHashMap::default(),
            direct_exports: FxHashMap::default(),
            default_export: None,
            reexports: Vec::new(),
            export_all: Vec::new(),
        };
        for statement in &parsed.program.body {
            parser.statement(statement, &mut file);
        }
        Ok(file)
    }

    fn statement(&self, statement: &Statement<'_>, file: &mut DeclarationFile) {
        let references = DeclarationReferenceCollector::collect(statement);
        match statement {
            Statement::ImportDeclaration(import) => {
                let Some(specifiers) = &import.specifiers else {
                    return;
                };
                for specifier in specifiers {
                    let (local, imported) = match specifier {
                        ImportDeclarationSpecifier::ImportSpecifier(specifier) => (
                            specifier.local.name.to_string(),
                            ImportedName::Named(Self::module_export_name(&specifier.imported)),
                        ),
                        ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                            (specifier.local.name.to_string(), ImportedName::Default)
                        }
                        ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                            (specifier.local.name.to_string(), ImportedName::Namespace)
                        }
                    };
                    file.imports.insert(
                        local,
                        ImportBinding {
                            specifier: import.source.value.to_string(),
                            imported,
                        },
                    );
                }
            }
            Statement::ExportDeclaration(export) => {
                self.declaration(&export.declaration, file, true, &references);
            }
            Statement::ExportNamedDeclaration(export) => {
                for specifier in &export.specifiers {
                    file.direct_exports.insert(
                        Self::module_export_name(&specifier.exported),
                        Self::module_export_name(&specifier.local),
                    );
                }
            }
            Statement::ExportFromDeclaration(export) => {
                let original = self.slice(export.span).trim().to_string();
                for specifier in &export.specifiers {
                    file.reexports.push(Reexport {
                        specifier: export.source.value.to_string(),
                        imported: Self::module_export_name(&specifier.local),
                        exported: Self::module_export_name(&specifier.exported),
                        original: original.clone(),
                    });
                }
            }
            Statement::ExportAllDeclaration(export) => file.export_all.push(ExportAll {
                specifier: export.source.value.to_string(),
                namespace: export.exported.as_ref().map(Self::module_export_name),
                original: self.slice(export.span).trim().to_string(),
            }),
            Statement::ExportDefaultDeclaration(export) => match &export.declaration {
                ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
                    if let Some(name) = function.name().map(|name| name.to_string()) {
                        self.owned_declaration(
                            function.span,
                            file,
                            vec![name.clone()],
                            false,
                            &references,
                        );
                        file.default_export = Some(name);
                    }
                }
                ExportDefaultDeclarationKind::ClassDeclaration(class) => {
                    if let Some(name) = class.id.as_ref().map(|id| id.name.to_string()) {
                        self.owned_declaration(
                            class.span,
                            file,
                            vec![name.clone()],
                            false,
                            &references,
                        );
                        file.default_export = Some(name);
                    }
                }
                ExportDefaultDeclarationKind::TSInterfaceDeclaration(interface) => {
                    let name = interface.id.name.to_string();
                    self.owned_declaration(
                        interface.span,
                        file,
                        vec![name.clone()],
                        false,
                        &references,
                    );
                    file.default_export = Some(name);
                }
                ExportDefaultDeclarationKind::StructStatement(statement) => {
                    let name = statement.id.name.to_string();
                    self.owned_declaration(
                        statement.span,
                        file,
                        vec![name.clone()],
                        false,
                        &references,
                    );
                    file.default_export = Some(name);
                }
                ExportDefaultDeclarationKind::Identifier(identifier) => {
                    file.default_export = Some(identifier.name.to_string());
                }
                _ => {}
            },
            Statement::VariableDeclaration(declaration) => self.owned_declaration(
                declaration.span,
                file,
                declaration
                    .declarations
                    .iter()
                    .flat_map(|declaration| declaration.id.get_binding_identifiers())
                    .map(|identifier| identifier.name.to_string())
                    .collect(),
                false,
                &references,
            ),
            Statement::FunctionDeclaration(declaration) => {
                if let Some(name) = declaration.name().map(|name| name.to_string()) {
                    self.owned_declaration(declaration.span, file, vec![name], false, &references);
                }
            }
            Statement::ClassDeclaration(declaration) => {
                if let Some(name) = declaration.id.as_ref().map(|id| id.name.to_string()) {
                    self.owned_declaration(declaration.span, file, vec![name], false, &references);
                }
            }
            Statement::TSTypeAliasDeclaration(declaration) => self.owned_declaration(
                declaration.span,
                file,
                vec![declaration.id.name.to_string()],
                true,
                &references,
            ),
            Statement::TSInterfaceDeclaration(declaration) => self.owned_declaration(
                declaration.span,
                file,
                vec![declaration.id.name.to_string()],
                false,
                &references,
            ),
            Statement::TSEnumDeclaration(declaration) => self.owned_declaration(
                declaration.span,
                file,
                vec![declaration.id.name.to_string()],
                false,
                &references,
            ),
            Statement::TSNamespaceDeclaration(declaration) => self.owned_declaration(
                declaration.span,
                file,
                vec![declaration.id.name.to_string()],
                false,
                &references,
            ),
            Statement::StructStatement(declaration) => self.owned_declaration(
                declaration.span,
                file,
                vec![declaration.id.name.to_string()],
                false,
                &references,
            ),
            Statement::AnnotationDeclaration(declaration) => self.owned_declaration(
                declaration.span,
                file,
                vec![declaration.id.name.to_string()],
                false,
                &references,
            ),
            _ => {}
        }
    }

    fn declaration(
        &self,
        declaration: &Declaration<'_>,
        file: &mut DeclarationFile,
        exported: bool,
        references: &[AbsoluteReference],
    ) {
        let (names, type_alias) = Self::declaration_names(declaration);
        self.owned_declaration(
            declaration.span(),
            file,
            names.clone(),
            type_alias,
            references,
        );
        if exported {
            for name in names {
                file.direct_exports.insert(name.clone(), name);
            }
        }
    }

    fn owned_declaration(
        &self,
        span: Span,
        file: &mut DeclarationFile,
        names: Vec<String>,
        is_type_alias: bool,
        references: &[AbsoluteReference],
    ) {
        let owned = OwnedDeclaration {
            text: self.slice(span).trim().to_string(),
            references: references
                .iter()
                .filter(|reference| {
                    reference.span.start >= span.start && reference.span.end <= span.end
                })
                .map(|reference| DeclarationReference {
                    name: reference.name.clone(),
                    member: reference.member.clone(),
                    specifier: reference.specifier.clone(),
                    start: (reference.span.start - span.start) as usize,
                    end: (reference.span.end - span.start) as usize,
                })
                .collect(),
            is_type_alias,
        };
        for name in names {
            file.declarations
                .entry(name)
                .or_default()
                .push(owned.clone());
        }
    }

    fn declaration_names(declaration: &Declaration<'_>) -> (Vec<String>, bool) {
        let names = match declaration {
            Declaration::VariableDeclaration(variable) => variable
                .declarations
                .iter()
                .flat_map(|declaration| declaration.id.get_binding_identifiers())
                .map(|identifier| identifier.name.to_string())
                .collect(),
            Declaration::FunctionDeclaration(function) => function
                .name()
                .map(|name| name.to_string())
                .into_iter()
                .collect(),
            Declaration::ClassDeclaration(class) => class
                .id
                .as_ref()
                .map(|id| id.name.to_string())
                .into_iter()
                .collect(),
            Declaration::TSTypeAliasDeclaration(declaration) => {
                vec![declaration.id.name.to_string()]
            }
            Declaration::TSInterfaceDeclaration(declaration) => {
                vec![declaration.id.name.to_string()]
            }
            Declaration::TSEnumDeclaration(declaration) => vec![declaration.id.name.to_string()],
            Declaration::TSNamespaceDeclaration(declaration) => {
                vec![declaration.id.name.to_string()]
            }
            Declaration::StructStatement(declaration) => vec![declaration.id.name.to_string()],
            Declaration::AnnotationDeclaration(declaration) => {
                vec![declaration.id.name.to_string()]
            }
            _ => Vec::new(),
        };
        (
            names,
            matches!(declaration, Declaration::TSTypeAliasDeclaration(_)),
        )
    }

    fn module_export_name(name: &ModuleExportName<'_>) -> String {
        name.name().to_string()
    }

    fn slice(&self, span: Span) -> &str {
        &self.source[span.start as usize..span.end as usize]
    }
}

#[derive(Clone)]
struct AbsoluteReference {
    name: String,
    member: Option<String>,
    specifier: Option<String>,
    span: Span,
}

struct DeclarationReferenceCollector {
    references: Vec<AbsoluteReference>,
}

impl DeclarationReferenceCollector {
    fn collect(statement: &Statement<'_>) -> Vec<AbsoluteReference> {
        let mut collector = Self {
            references: Vec::new(),
        };
        collector.visit_statement(statement);
        collector.references
    }
}

impl<'a> Visit<'a> for DeclarationReferenceCollector {
    fn visit_identifier_reference(&mut self, identifier: &oxc_ast::ast::IdentifierReference<'a>) {
        self.references.push(AbsoluteReference {
            name: identifier.name.to_string(),
            member: None,
            specifier: None,
            span: identifier.span,
        });
    }

    fn visit_ts_qualified_name(&mut self, name: &oxc_ast::ast::TSQualifiedName<'a>) {
        if let TSTypeName::IdentifierReference(left) = &name.left {
            self.references.push(AbsoluteReference {
                name: left.name.to_string(),
                member: Some(name.right.name.to_string()),
                specifier: None,
                span: name.span,
            });
        }
    }

    fn visit_ts_import_type(&mut self, import: &oxc_ast::ast::TSImportType<'a>) {
        let Some(qualifier) = &import.qualifier else {
            return;
        };
        let name = match qualifier {
            oxc_ast::ast::TSImportTypeQualifier::Identifier(identifier) => {
                identifier.name.to_string()
            }
            oxc_ast::ast::TSImportTypeQualifier::QualifiedName(name) => name.right.name.to_string(),
        };
        self.references.push(AbsoluteReference {
            name,
            member: None,
            specifier: Some(import.source.value.to_string()),
            span: Span::new(import.span.start, qualifier.span().end),
        });
    }

    fn visit_static_member_expression(
        &mut self,
        member: &oxc_ast::ast::StaticMemberExpression<'a>,
    ) {
        if let oxc_ast::ast::Expression::Identifier(object) = &member.object {
            self.references.push(AbsoluteReference {
                name: object.name.to_string(),
                member: Some(member.property.name.to_string()),
                specifier: None,
                span: member.span,
            });
        } else {
            walk::walk_static_member_expression(self, member);
        }
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
struct DeclarationId {
    path: PathBuf,
    name: String,
}

#[derive(Clone, Debug)]
enum ResolvedExport {
    Declaration(DeclarationId),
    External(String),
    Namespace(PathBuf),
}

struct EntryEmission {
    primary: String,
    companion: Option<(PathBuf, String)>,
}

struct DeclarationEntryEmitter<'a> {
    merger: &'a mut DeclarationMerger,
    entry: &'a Path,
    public: BTreeMap<String, ResolvedExport>,
    public_ids: BTreeMap<DeclarationId, Vec<String>>,
    namespace_members: BTreeMap<String, Vec<(String, DeclarationId)>>,
    dependencies: BTreeSet<DeclarationId>,
    dependency_aliases: BTreeMap<DeclarationId, BTreeSet<String>>,
    expanded_aliases: BTreeSet<DeclarationId>,
    external_imports: BTreeSet<(bool, String)>,
    companion_public_names: BTreeSet<String>,
    companion_import_names: BTreeSet<String>,
    active_exports: FxHashSet<(PathBuf, String)>,
}

impl<'a> DeclarationEntryEmitter<'a> {
    fn new(merger: &'a mut DeclarationMerger, entry: &'a Path) -> Self {
        Self {
            merger,
            entry,
            public: BTreeMap::new(),
            public_ids: BTreeMap::new(),
            namespace_members: BTreeMap::new(),
            dependencies: BTreeSet::new(),
            dependency_aliases: BTreeMap::new(),
            expanded_aliases: BTreeSet::new(),
            external_imports: BTreeSet::new(),
            companion_public_names: BTreeSet::new(),
            companion_import_names: BTreeSet::new(),
            active_exports: FxHashSet::default(),
        }
    }

    fn emit(mut self) -> Result<EntryEmission> {
        self.collect_public_exports()?;
        self.collect_dependencies()?;

        let primary_ext = DeclarationExtension::from_path(self.entry);
        let emit_names = self.emit_names();
        let mut primary = DeclarationOutput::default();
        let mut companion = DeclarationOutput::default();

        for (is_companion, statement) in &self.external_imports {
            if *is_companion {
                companion.imports.insert(statement.clone());
            } else {
                primary.imports.insert(statement.clone());
            }
        }

        for resolved in self.public.values() {
            if let ResolvedExport::External(statement) = resolved {
                primary.imports.insert(statement.clone());
            }
        }

        for (path, ids) in self.dependency_groups() {
            let is_companion = DeclarationExtension::from_path(&path) != primary_ext;
            let target = if is_companion {
                &mut companion
            } else {
                &mut primary
            };
            let namespace = Self::namespace_name(&path);
            let mut members = Vec::new();
            for id in ids {
                let declarations = self
                    .merger
                    .model(&id.path)?
                    .declaration(&id.name)
                    .unwrap_or_default()
                    .to_vec();
                for declaration in declarations {
                    let rewritten = self.rewrite(&id, &declaration, &emit_names)?;
                    members.push(Self::namespace_member(&rewritten));
                }
                if let Some(aliases) = self.dependency_aliases.get(&id) {
                    members.extend(
                        aliases
                            .iter()
                            .filter(|alias| alias.as_str() != id.name)
                            .map(|alias| format!("export {{ {} as {alias} }};", id.name)),
                    );
                }
            }
            if !members.is_empty() {
                target.blocks.push(format!(
                    "{}declare namespace {namespace} {{\n{}\n}}",
                    if is_companion { "export " } else { "" },
                    Self::indent(&members.join("\n\n"))
                ));
            }
        }

        let namespace_members = self.namespace_members.clone();
        for (namespace, members) in namespace_members {
            let all_companion = members
                .iter()
                .all(|(_, id)| DeclarationExtension::from_path(&id.path) != primary_ext);
            let mut primary_declarations = Vec::new();
            let mut companion_declarations = Vec::new();
            for (export_name, id) in members {
                let is_companion = DeclarationExtension::from_path(&id.path) != primary_ext;
                let owned = self
                    .merger
                    .model(&id.path)?
                    .declaration(&id.name)
                    .unwrap_or_default()
                    .to_vec();
                for declaration in owned {
                    let rewritten = self.rewrite(&id, &declaration, &emit_names)?;
                    let rewritten =
                        Self::replace_identifier_once(&rewritten, &id.name, &export_name);
                    if is_companion && !all_companion {
                        companion.blocks.push(Self::public_declaration(
                            &rewritten,
                            declaration.is_type_alias,
                        ));
                        self.companion_import_names.insert(export_name.clone());
                    } else if is_companion {
                        companion_declarations.push(Self::namespace_member(&rewritten));
                    } else {
                        primary_declarations.push(Self::namespace_member(&rewritten));
                    }
                }
                if is_companion && !all_companion {
                    primary_declarations.push(format!("export {{ {export_name} }};"));
                }
            }
            if all_companion && !companion_declarations.is_empty() {
                companion.blocks.push(format!(
                    "export declare namespace {namespace} {{\n{}\n}}",
                    Self::indent(&companion_declarations.join("\n\n"))
                ));
                self.companion_public_names.insert(namespace);
            } else if !primary_declarations.is_empty() {
                primary.blocks.push(format!(
                    "declare namespace {namespace} {{\n{}\n}}",
                    Self::indent(&primary_declarations.join("\n\n"))
                ));
                primary.exports.insert(format!("export {{ {namespace} }};"));
            }
        }

        let public_ids = self
            .public_ids
            .iter()
            .map(|(id, names)| (id.clone(), names.clone()))
            .collect::<Vec<_>>();
        for (id, export_names) in public_ids {
            let target = if DeclarationExtension::from_path(&id.path) == primary_ext {
                &mut primary
            } else {
                &mut companion
            };
            let emit_name = emit_names.get(&id).expect("public emit name");
            let declarations = self
                .merger
                .model(&id.path)?
                .declaration(&id.name)
                .unwrap_or_default()
                .to_vec();
            for declaration in declarations {
                let rewritten = self.rewrite(&id, &declaration, &emit_names)?;
                target.blocks.push(Self::public_declaration(
                    &rewritten,
                    declaration.is_type_alias,
                ));
            }
            for export_name in &export_names {
                if export_name == "default" {
                    target
                        .exports
                        .insert(format!("export default {emit_name};"));
                } else if export_name != emit_name {
                    target
                        .exports
                        .insert(format!("export {{ {emit_name} as {export_name} }};"));
                }
            }
        }

        if companion.is_empty() {
            return Ok(EntryEmission {
                primary: primary.finish(),
                companion: None,
            });
        }

        let companion_path = self.companion_name();
        let companion_module = companion_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Index-declarations.d.ts")
            .trim_end_matches(".d.ts")
            .trim_end_matches(".d.ets")
            .to_string();
        let mut companion_exports = self.companion_public_names.clone();
        companion_exports.extend(self.public.iter().filter_map(|(name, resolved)| {
            let ResolvedExport::Declaration(id) = resolved else {
                return None;
            };
            (DeclarationExtension::from_path(&id.path) != primary_ext).then(|| name.clone())
        }));
        for name in companion_exports {
            if primary
                .blocks
                .iter()
                .any(|block| Self::contains_identifier(block, &name))
            {
                primary
                    .imports
                    .insert(format!("import {{ {name} }} from './{companion_module}';"));
                primary.exports.insert(format!("export {{ {name} }};"));
            } else {
                primary
                    .imports
                    .insert(format!("export {{ {name} }} from './{companion_module}';"));
            }
        }
        let companion_namespaces = self
            .dependencies
            .iter()
            .filter(|id| DeclarationExtension::from_path(&id.path) != primary_ext)
            .map(|id| Self::namespace_name(&id.path))
            .collect::<BTreeSet<_>>();
        if !companion_namespaces.is_empty() {
            primary.imports.insert(format!(
                "import {{ {} }} from './{companion_module}';",
                companion_namespaces
                    .into_iter()
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if !self.companion_import_names.is_empty() {
            primary.imports.insert(format!(
                "import {{ {} }} from './{companion_module}';",
                self.companion_import_names
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        Ok(EntryEmission {
            primary: primary.finish(),
            companion: Some((companion_path, companion.finish())),
        })
    }

    fn collect_public_exports(&mut self) -> Result<()> {
        let model = self.merger.model(self.entry)?.clone();
        let mut names = model
            .direct_exports
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        names.extend(model.reexports.iter().map(|export| export.exported.clone()));
        names.extend(
            model
                .export_all
                .iter()
                .filter_map(|export| export.namespace.clone()),
        );
        if model.default_export.is_some() {
            names.insert("default".to_string());
        }
        for export in &model.export_all {
            if export.namespace.is_none() {
                if let Some(target) = self.merger.resolve(self.entry, &export.specifier) {
                    names.extend(
                        self.exported_names(&target)?
                            .into_iter()
                            .filter(|name| name != "default"),
                    );
                } else {
                    self.public.insert(
                        export.original.clone(),
                        ResolvedExport::External(export.original.clone()),
                    );
                }
            }
        }
        for name in names {
            if let Some(resolved) = self.resolve_export(self.entry, &name)? {
                if let ResolvedExport::Declaration(id) = &resolved {
                    self.public_ids
                        .entry(id.clone())
                        .or_default()
                        .push(name.clone());
                } else if let ResolvedExport::Namespace(path) = &resolved {
                    let exported_names = self.exported_names(path)?;
                    let mut members = Vec::new();
                    for exported_name in exported_names {
                        if exported_name == "default" {
                            continue;
                        }
                        if let Some(ResolvedExport::Declaration(id)) =
                            self.resolve_export(path, &exported_name)?
                        {
                            members.push((exported_name, id));
                        }
                    }
                    self.namespace_members.insert(name.clone(), members);
                }
                self.public.insert(name, resolved);
            }
        }
        Ok(())
    }

    fn exported_names(&mut self, path: &Path) -> Result<BTreeSet<String>> {
        let active_key = (path.to_path_buf(), "*".to_string());
        if !self.active_exports.insert(active_key.clone()) {
            return Ok(BTreeSet::new());
        }
        let names = self.exported_names_inner(path);
        self.active_exports.remove(&active_key);
        names
    }

    fn exported_names_inner(&mut self, path: &Path) -> Result<BTreeSet<String>> {
        let model = self.merger.model(path)?.clone();
        let mut names = model
            .direct_exports
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        names.extend(model.reexports.iter().map(|export| export.exported.clone()));
        if model.default_export.is_some() {
            names.insert("default".to_string());
        }
        for export in model.export_all {
            if export.namespace.is_none() {
                if let Some(target) = self.merger.resolve(path, &export.specifier) {
                    names.extend(
                        self.exported_names(&target)?
                            .into_iter()
                            .filter(|name| name != "default"),
                    );
                }
            }
        }
        Ok(names)
    }

    fn resolve_export(&mut self, path: &Path, name: &str) -> Result<Option<ResolvedExport>> {
        let key = (path.to_path_buf(), name.to_string());
        if !self.active_exports.insert(key.clone()) {
            return Ok(None);
        }
        let result = self.resolve_export_inner(path, name);
        self.active_exports.remove(&key);
        result
    }

    fn resolve_export_inner(&mut self, path: &Path, name: &str) -> Result<Option<ResolvedExport>> {
        let model = self.merger.model(path)?.clone();
        if name == "default" {
            if let Some(local) = &model.default_export {
                return self.resolve_local(path, &model, local);
            }
        }
        if let Some(local) = model.direct_exports.get(name) {
            return self.resolve_local(path, &model, local);
        }
        if let Some(export) = model
            .reexports
            .iter()
            .find(|export| export.exported == name)
        {
            if let Some(target) = self.merger.resolve(path, &export.specifier) {
                return self.resolve_export(&target, &export.imported);
            }
            return Ok(Some(ResolvedExport::External(export.original.clone())));
        }
        for export in model.export_all {
            if export.namespace.as_deref() == Some(name) {
                if let Some(target) = self.merger.resolve(path, &export.specifier) {
                    return Ok(Some(ResolvedExport::Namespace(target)));
                }
                return Ok(Some(ResolvedExport::External(export.original)));
            }
            if export.namespace.is_none() && name != "default" {
                if let Some(target) = self.merger.resolve(path, &export.specifier) {
                    if let Some(resolved) = self.resolve_export(&target, name)? {
                        return Ok(Some(resolved));
                    }
                }
            }
        }
        Ok(None)
    }

    fn resolve_local(
        &mut self,
        path: &Path,
        model: &DeclarationFile,
        local: &str,
    ) -> Result<Option<ResolvedExport>> {
        if model.declarations.contains_key(local) {
            return Ok(Some(ResolvedExport::Declaration(DeclarationId {
                path: path.to_path_buf(),
                name: local.to_string(),
            })));
        }
        let Some(import) = model.imports.get(local) else {
            return Ok(None);
        };
        let Some(target) = self.merger.resolve(path, &import.specifier) else {
            return Ok(Some(ResolvedExport::External(Self::external_export(
                import, local,
            ))));
        };
        match &import.imported {
            ImportedName::Named(name) => self.resolve_export(&target, name),
            ImportedName::Default => self.resolve_export(&target, "default"),
            ImportedName::Namespace => Ok(Some(ResolvedExport::Namespace(target))),
        }
    }

    fn collect_dependencies(&mut self) -> Result<()> {
        let mut public = self.public_ids.keys().cloned().collect::<BTreeSet<_>>();
        public.extend(
            self.namespace_members
                .values()
                .flatten()
                .map(|(_, id)| id.clone()),
        );
        let mut pending = public.iter().cloned().collect::<Vec<_>>();
        let mut visited = BTreeSet::new();
        while let Some(id) = pending.pop() {
            if !visited.insert(id.clone()) {
                continue;
            }
            let model = self.merger.model(&id.path)?.clone();
            let declarations = model.declaration(&id.name).unwrap_or_default().to_vec();
            for declaration in declarations {
                for reference in declaration.references {
                    if let Some(import) = model.imports.get(&reference.name) {
                        if self.merger.resolve(&id.path, &import.specifier).is_none() {
                            self.external_imports.insert((
                                DeclarationExtension::from_path(&id.path)
                                    != DeclarationExtension::from_path(self.entry),
                                Self::external_import(import, &reference.name),
                            ));
                            continue;
                        }
                    }
                    let Some(target) = self.resolve_reference(&id.path, &model, &reference)? else {
                        continue;
                    };
                    let expands_alias = reference.specifier.is_some()
                        && self
                            .merger
                            .model(&target.path)?
                            .declaration(&target.name)
                            .and_then(|declarations| declarations.first())
                            .is_some_and(|declaration| declaration.is_type_alias);
                    if expands_alias {
                        self.expanded_aliases.insert(target.clone());
                    } else if !public.contains(&target) {
                        self.dependencies.insert(target.clone());
                        if reference.specifier.is_none()
                            && reference.member.is_none()
                            && reference.name != target.name
                        {
                            self.dependency_aliases
                                .entry(target.clone())
                                .or_default()
                                .insert(reference.name.clone());
                        }
                    }
                    pending.push(target);
                }
            }
        }
        Ok(())
    }

    fn resolve_reference(
        &mut self,
        path: &Path,
        model: &DeclarationFile,
        reference: &DeclarationReference,
    ) -> Result<Option<DeclarationId>> {
        if let Some(specifier) = &reference.specifier {
            let Some(target) = self.merger.resolve(path, specifier) else {
                return Ok(None);
            };
            return Ok(match self.resolve_export(&target, &reference.name)? {
                Some(ResolvedExport::Declaration(id)) => Some(id),
                _ => None,
            });
        }
        if let Some(member) = &reference.member {
            if let Some(namespace) = self.imported_namespace(path, model, &reference.name)? {
                return Ok(match self.resolve_export(&namespace, member)? {
                    Some(ResolvedExport::Declaration(id)) => Some(id),
                    _ => None,
                });
            }
        }
        let name = &reference.name;
        if model.declarations.contains_key(name) {
            return Ok(Some(DeclarationId {
                path: path.to_path_buf(),
                name: name.to_string(),
            }));
        }
        let Some(import) = model.imports.get(name) else {
            return Ok(None);
        };
        let Some(target) = self.merger.resolve(path, &import.specifier) else {
            return Ok(None);
        };
        let imported = match &import.imported {
            ImportedName::Named(name) => name.as_str(),
            ImportedName::Default => "default",
            ImportedName::Namespace => return Ok(None),
        };
        Ok(match self.resolve_export(&target, imported)? {
            Some(ResolvedExport::Declaration(id)) => Some(id),
            _ => None,
        })
    }

    fn imported_namespace(
        &mut self,
        path: &Path,
        model: &DeclarationFile,
        local: &str,
    ) -> Result<Option<PathBuf>> {
        let Some(import) = model.imports.get(local) else {
            return Ok(None);
        };
        let Some(target) = self.merger.resolve(path, &import.specifier) else {
            return Ok(None);
        };
        match &import.imported {
            ImportedName::Namespace => Ok(Some(target)),
            ImportedName::Named(name) => Ok(match self.resolve_export(&target, name)? {
                Some(ResolvedExport::Namespace(path)) => Some(path),
                _ => None,
            }),
            ImportedName::Default => Ok(match self.resolve_export(&target, "default")? {
                Some(ResolvedExport::Namespace(path)) => Some(path),
                _ => None,
            }),
        }
    }

    fn emit_names(&self) -> BTreeMap<DeclarationId, String> {
        let mut names = BTreeMap::new();
        let mut used = BTreeSet::new();
        for (id, exports) in &self.public_ids {
            let preferred = id.name.clone();
            let mut emit = preferred.clone();
            if used.contains(&emit) {
                if let Some(export_name) = exports
                    .iter()
                    .find(|name| name.as_str() != "default" && !used.contains(*name))
                {
                    emit = export_name.clone();
                }
            }
            let mut suffix = 1usize;
            while !used.insert(emit.clone()) {
                emit = format!("{preferred}_{suffix}");
                suffix += 1;
            }
            names.insert(id.clone(), emit);
        }
        names
    }

    fn dependency_groups(&self) -> BTreeMap<PathBuf, Vec<DeclarationId>> {
        let mut groups = BTreeMap::<PathBuf, Vec<DeclarationId>>::new();
        for id in &self.dependencies {
            groups.entry(id.path.clone()).or_default().push(id.clone());
        }
        groups
    }

    fn rewrite(
        &mut self,
        owner: &DeclarationId,
        declaration: &OwnedDeclaration,
        emit_names: &BTreeMap<DeclarationId, String>,
    ) -> Result<String> {
        let model = self.merger.model(&owner.path)?.clone();
        let mut replacements = Vec::<(usize, usize, String)>::new();
        for reference in &declaration.references {
            let Some(target) = self.resolve_reference(&owner.path, &model, reference)? else {
                continue;
            };
            if reference.specifier.is_some() && self.expanded_aliases.contains(&target) {
                if let Some(expression) = self.expanded_alias_expression(&target, emit_names)? {
                    replacements.push((reference.start, reference.end, expression));
                    continue;
                }
            }
            let mut replacement = if let Some(name) = emit_names.get(&target) {
                name.clone()
            } else if self.dependencies.contains(&target) {
                let reference_name = self
                    .dependency_aliases
                    .get(&target)
                    .filter(|aliases| aliases.contains(&reference.name))
                    .map_or(target.name.as_str(), |_| reference.name.as_str());
                if self.dependencies.contains(owner) && target.path == owner.path {
                    reference_name.to_string()
                } else {
                    format!("{}.{}", Self::namespace_name(&target.path), reference_name)
                }
            } else {
                continue;
            };
            let namespace_import = reference.member.is_some()
                && self
                    .imported_namespace(&owner.path, &model, &reference.name)?
                    .is_some();
            if reference.member.is_some() && !namespace_import {
                replacement.push('.');
                replacement.push_str(reference.member.as_deref().expect("qualified member"));
            }
            if replacement != reference.name {
                replacements.push((reference.start, reference.end, replacement));
            }
        }
        replacements.sort_by_key(|replacement| replacement.0);
        let mut text = declaration.text.clone();
        for (start, end, replacement) in replacements.into_iter().rev() {
            if end <= text.len() && start <= end {
                text.replace_range(start..end, &replacement);
            }
        }
        if let Some(emit_name) = emit_names.get(owner) {
            text = Self::replace_identifier_once(&text, &owner.name, emit_name);
        }
        Ok(text)
    }

    fn expanded_alias_expression(
        &mut self,
        target: &DeclarationId,
        emit_names: &BTreeMap<DeclarationId, String>,
    ) -> Result<Option<String>> {
        let Some(declaration) = self
            .merger
            .model(&target.path)?
            .declaration(&target.name)
            .and_then(|declarations| declarations.first())
            .cloned()
        else {
            return Ok(None);
        };
        let rewritten = self.rewrite(target, &declaration, emit_names)?;
        let Some((_, expression)) = rewritten.split_once('=') else {
            return Ok(None);
        };
        Ok(Some(
            expression.trim().trim_end_matches(';').trim().to_string(),
        ))
    }

    fn contains_identifier(text: &str, name: &str) -> bool {
        text.match_indices(name).any(|(start, _)| {
            let end = start + name.len();
            let boundary = |character: Option<char>| {
                character.is_none_or(|character| {
                    !character.is_ascii_alphanumeric() && character != '_' && character != '$'
                })
            };
            boundary(text[..start].chars().next_back()) && boundary(text[end..].chars().next())
        })
    }

    fn replace_identifier_once(text: &str, from: &str, to: &str) -> String {
        if from == to {
            return text.to_string();
        }
        for (start, _) in text.match_indices(from) {
            let end = start + from.len();
            let left = text[..start].chars().next_back();
            let right = text[end..].chars().next();
            let boundary = |character: Option<char>| {
                character.is_none_or(|character| {
                    !character.is_ascii_alphanumeric() && character != '_' && character != '$'
                })
            };
            if boundary(left) && boundary(right) {
                let mut replaced = text.to_string();
                replaced.replace_range(start..end, to);
                return replaced;
            }
        }
        text.to_string()
    }

    fn namespace_name(path: &Path) -> String {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("declarations")
            .trim_end_matches(".d.ets")
            .trim_end_matches(".d.ts");
        let mut namespace = String::from("_");
        for character in name.chars() {
            namespace.push(if character.is_ascii_alphanumeric() || character == '_' {
                character
            } else {
                '_'
            });
        }
        namespace
    }

    fn public_declaration(text: &str, is_type_alias: bool) -> String {
        let sanitized = Self::sanitize_declaration(text);
        let (leading, body) = Self::split_declaration_prefix(&sanitized);
        if is_type_alias || body.starts_with("interface ") {
            format!("{leading}export {body}")
        } else {
            format!("{leading}export declare {body}")
        }
    }

    fn namespace_member(text: &str) -> String {
        let sanitized = Self::sanitize_declaration(text);
        let (leading, body) = Self::split_declaration_prefix(&sanitized);
        format!("{leading}export {body}")
    }

    fn sanitize_declaration(text: &str) -> String {
        let mut text = text.replace(" extends {}", "");
        while let Some(constructor) = text.find("constructor(?:") {
            let line_start = text[..constructor]
                .rfind('\n')
                .map_or(0, |newline| newline + 1);
            let Some(open_paren) = text[constructor..].find('(').map(|at| constructor + at) else {
                break;
            };
            let Some(close_paren) = Self::matching_delimiter(&text, open_paren, '(', ')') else {
                break;
            };
            let after = text[close_paren + 1..].trim_start();
            let whitespace = text[close_paren + 1..].len() - after.len();
            let body_start = close_paren + 1 + whitespace;
            let end = if text.as_bytes().get(body_start) == Some(&b'{') {
                Self::matching_delimiter(&text, body_start, '{', '}')
                    .map(|end| end + 1)
                    .unwrap_or(body_start)
            } else {
                text[body_start..]
                    .find(';')
                    .map(|at| body_start + at + 1)
                    .unwrap_or(body_start)
            };
            let line_end = text[end..]
                .find('\n')
                .map_or(end, |newline| end + newline + 1);
            text.replace_range(line_start..line_end, "");
        }
        text
    }

    fn matching_delimiter(text: &str, start: usize, open: char, close: char) -> Option<usize> {
        let mut depth = 0usize;
        for (offset, character) in text[start..].char_indices() {
            if character == open {
                depth += 1;
            } else if character == close {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(start + offset);
                }
            }
        }
        None
    }

    fn split_declaration_prefix(text: &str) -> (String, String) {
        let text = text.trim();
        let declaration_keywords = [
            "class ",
            "function ",
            "interface ",
            "type ",
            "enum ",
            "const enum ",
            "let ",
            "const ",
            "var ",
            "struct ",
            "namespace ",
            "module ",
            "@interface ",
        ];
        let modifiers = [
            "export default declare ",
            "export declare ",
            "export default ",
            "declare ",
            "export ",
        ];
        let mut offset = 0usize;
        for line in text.split_inclusive('\n') {
            let indent = line.len() - line.trim_start().len();
            let candidate = line.trim_start();
            let body = modifiers
                .iter()
                .find_map(|modifier| candidate.strip_prefix(modifier))
                .unwrap_or(candidate);
            if declaration_keywords
                .iter()
                .any(|keyword| body.starts_with(keyword))
            {
                let body_start = offset + indent + candidate.len() - body.len();
                let mut leading = text[..offset + indent].to_string();
                let body = format!(
                    "{}{}",
                    &text[body_start..offset + line.len()],
                    &text[offset + line.len()..]
                );
                if !leading.is_empty() && !leading.ends_with('\n') {
                    leading.push('\n');
                }
                return (leading, body.trim().to_string());
            }
            offset += line.len();
        }
        (String::new(), text.to_string())
    }

    fn indent(text: &str) -> String {
        text.lines()
            .map(|line| format!("    {line}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn external_export(import: &ImportBinding, local: &str) -> String {
        format!(
            "{}\n\nexport {{ {local} }};",
            Self::external_import(import, local)
        )
    }

    fn external_import(import: &ImportBinding, local: &str) -> String {
        match &import.imported {
            ImportedName::Named(imported) if imported == local => {
                format!("import {{ {local} }} from '{}';", import.specifier)
            }
            ImportedName::Named(imported) => format!(
                "import {{ {imported} as {local} }} from '{}';",
                import.specifier
            ),
            ImportedName::Default => format!("import {local} from '{}';", import.specifier),
            ImportedName::Namespace => {
                format!("import * as {local} from '{}';", import.specifier)
            }
        }
    }

    fn companion_name(&self) -> PathBuf {
        let extension = if self.entry.to_string_lossy().ends_with(".d.ets") {
            "d.ts"
        } else {
            "d.ets"
        };
        let stem = self
            .entry
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Index")
            .trim_end_matches(".d.ets")
            .trim_end_matches(".d.ts");
        self.entry
            .with_file_name(format!("{stem}-declarations.{extension}"))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DeclarationExtension {
    Ets,
    TypeScript,
}

impl DeclarationExtension {
    fn from_path(path: &Path) -> Self {
        if path.to_string_lossy().ends_with(".d.ets") {
            Self::Ets
        } else {
            Self::TypeScript
        }
    }
}

#[derive(Default)]
struct DeclarationOutput {
    imports: BTreeSet<String>,
    blocks: Vec<String>,
    exports: BTreeSet<String>,
}

impl DeclarationOutput {
    fn is_empty(&self) -> bool {
        self.imports.is_empty() && self.blocks.is_empty() && self.exports.is_empty()
    }

    fn finish(self) -> String {
        let mut sections = Vec::new();
        sections.extend(self.imports);
        sections.extend(self.blocks.into_iter().filter(|block| !block.is_empty()));
        sections.extend(self.exports);
        if sections.is_empty() {
            "export {};".to_string()
        } else {
            sections.join("\n\n")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DeclarationMerger;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    #[test]
    fn flattens_reexports_and_private_type_dependencies() {
        let root = PathBuf::from("/project/library");
        let entry = root.join("Index.d.ets");
        let sources = BTreeMap::from([
            (
                entry.clone(),
                "export { Service as PublicService } from './public';".to_string(),
            ),
            (
                root.join("public.d.ets"),
                concat!(
                    "import { Model } from './types';\n",
                    "export declare class Service { load(): Model; }\n"
                )
                .to_string(),
            ),
            (
                root.join("types.d.ets"),
                "export interface Model { id: number; }\n".to_string(),
            ),
        ]);

        let output = DeclarationMerger::new(sources)
            .merge(std::slice::from_ref(&entry))
            .expect("merge declarations");
        let declaration = output.files.get(&entry).expect("entry output");
        assert!(
            declaration.contains("export declare class Service"),
            "{declaration}"
        );
        assert!(
            declaration.contains("export { Service as PublicService }"),
            "{declaration}"
        );
        assert!(
            declaration.contains("declare namespace _types"),
            "{declaration}"
        );
        assert!(
            declaration.contains("load(): _types.Model"),
            "{declaration}"
        );
        assert_eq!(output.files.len(), 1);
    }

    #[test]
    fn emits_cross_extension_companion() {
        let root = PathBuf::from("/project/library");
        let entry = root.join("Index.d.ets");
        let sources = BTreeMap::from([
            (
                entry.clone(),
                "export { ExternalType } from './types';".to_string(),
            ),
            (
                root.join("types.d.ts"),
                "export interface ExternalType { value: string; }".to_string(),
            ),
        ]);

        let output = DeclarationMerger::new(sources)
            .merge(std::slice::from_ref(&entry))
            .expect("merge declarations");
        let primary = output.files.get(&entry).expect("primary");
        let companion = output
            .files
            .get(&root.join("Index-declarations.d.ts"))
            .expect("companion");
        assert!(primary.contains("export { ExternalType } from './Index-declarations';"));
        assert!(
            companion.contains("export interface ExternalType"),
            "{companion}"
        );
    }

    #[test]
    fn preserves_decorators_before_export_modifiers() {
        let root = PathBuf::from("/project/library");
        let entry = root.join("Index.d.ets");
        let sources = BTreeMap::from([(
            entry.clone(),
            "@Component\nexport declare struct Card {\n    @State title: string;\n}".to_string(),
        )]);

        let output = DeclarationMerger::new(sources)
            .merge(std::slice::from_ref(&entry))
            .expect("merge declarations");
        let declaration = output.files.get(&entry).expect("entry output");
        assert!(
            declaration.contains("@Component\nexport declare struct Card"),
            "{declaration}"
        );
    }

    #[test]
    fn rewrites_namespace_import_type_references() {
        let root = PathBuf::from("/project/library");
        let entry = root.join("Index.d.ets");
        let sources = BTreeMap::from([
            (
                entry.clone(),
                concat!(
                    "import * as Types from './types';\n",
                    "export declare class Service { load(): Types.Model; }"
                )
                .to_string(),
            ),
            (
                root.join("types.d.ets"),
                "export interface Model { id: number; }".to_string(),
            ),
        ]);

        let output = DeclarationMerger::new(sources)
            .merge(std::slice::from_ref(&entry))
            .expect("merge declarations");
        let declaration = output.files.get(&entry).expect("entry output");
        assert!(
            declaration.contains("load(): _types.Model"),
            "{declaration}"
        );
        assert!(!declaration.contains("Types.Model"), "{declaration}");
    }

    #[test]
    fn isolates_private_type_from_same_file() {
        let entry = PathBuf::from("/project/library/Index.d.ets");
        let sources = BTreeMap::from([(
            entry.clone(),
            concat!(
                "interface Internal { value: string; }\n",
                "export declare class Public { get(): Internal; }"
            )
            .to_string(),
        )]);

        let output = DeclarationMerger::new(sources)
            .merge(std::slice::from_ref(&entry))
            .expect("merge declarations");
        let declaration = output.files.get(&entry).expect("entry output");
        assert!(
            declaration.contains("declare namespace _Index"),
            "{declaration}"
        );
        assert!(
            declaration.contains("get(): _Index.Internal"),
            "{declaration}"
        );
    }

    #[test]
    fn expands_namespace_reexport_without_leaking_local_from_clause() {
        let root = PathBuf::from("/project/library");
        let entry = root.join("Index.d.ets");
        let sources = BTreeMap::from([
            (
                entry.clone(),
                "export * as Models from './models';".to_string(),
            ),
            (
                root.join("models.d.ets"),
                concat!(
                    "export interface User { id: number; }\n",
                    "export declare function load(): User;"
                )
                .to_string(),
            ),
        ]);

        let output = DeclarationMerger::new(sources)
            .merge(std::slice::from_ref(&entry))
            .expect("merge declarations");
        let declaration = output.files.get(&entry).expect("entry output");
        assert!(
            declaration.contains("declare namespace Models"),
            "{declaration}"
        );
        assert!(declaration.contains("export { Models }"), "{declaration}");
        assert!(
            declaration.contains("export interface User"),
            "{declaration}"
        );
        assert!(!declaration.contains("from './models'"), "{declaration}");
    }

    #[test]
    fn resolves_import_type_reference() {
        let root = PathBuf::from("/project/library");
        let entry = root.join("Index.d.ets");
        let sources = BTreeMap::from([
            (
                entry.clone(),
                "export declare const value: import('./types').Kind;".to_string(),
            ),
            (
                root.join("types.d.ets"),
                "export declare enum Kind { A = 1 }".to_string(),
            ),
        ]);

        let output = DeclarationMerger::new(sources)
            .merge(std::slice::from_ref(&entry))
            .expect("merge declarations");
        let declaration = output.files.get(&entry).expect("entry output");
        assert!(declaration.contains("value: _types.Kind"), "{declaration}");
        assert!(!declaration.contains("import('./types')"), "{declaration}");
    }

    #[test]
    fn preserves_only_referenced_external_type_imports() {
        let entry = PathBuf::from("/project/library/Index.d.ets");
        let sources = BTreeMap::from([(
            entry.clone(),
            concat!(
                "import { Ability } from '@ohos.app.ability';\n",
                "import { Unused } from '@ohos.unused';\n",
                "export declare function start(value: Ability): void;"
            )
            .to_string(),
        )]);

        let output = DeclarationMerger::new(sources)
            .merge(std::slice::from_ref(&entry))
            .expect("merge declarations");
        let declaration = output.files.get(&entry).expect("entry output");
        assert!(
            declaration.contains("import { Ability } from '@ohos.app.ability';"),
            "{declaration}"
        );
        assert!(!declaration.contains("@ohos.unused"), "{declaration}");
    }

    #[test]
    fn export_star_excludes_default_declaration() {
        let root = PathBuf::from("/project/library");
        let entry = root.join("Index.d.ets");
        let sources = BTreeMap::from([
            (entry.clone(), "export * from './default';".to_string()),
            (
                root.join("default.d.ets"),
                "let value = 1; export default value;".to_string(),
            ),
        ]);

        let output = DeclarationMerger::new(sources)
            .merge(std::slice::from_ref(&entry))
            .expect("merge declarations");
        assert_eq!(
            output.files.get(&entry).map(String::as_str),
            Some("export {};")
        );
    }

    #[test]
    fn resolves_indirect_namespace_member_once() {
        let root = PathBuf::from("/project/library");
        let entry = root.join("Index.d.ets");
        let sources = BTreeMap::from([
            (
                entry.clone(),
                "import { config } from './two'; export let value: config.Item;".to_string(),
            ),
            (
                root.join("two.d.ets"),
                "export * as config from './one';".to_string(),
            ),
            (
                root.join("one.d.ets"),
                "export type { Item } from './types';".to_string(),
            ),
            (
                root.join("types.d.ets"),
                "export interface Item { id: number; }".to_string(),
            ),
        ]);

        let output = DeclarationMerger::new(sources)
            .merge(std::slice::from_ref(&entry))
            .expect("merge declarations");
        let declaration = output.files.get(&entry).expect("entry output");
        assert!(declaration.contains("value: _types.Item"), "{declaration}");
        assert!(!declaration.contains("Item.Item"), "{declaration}");
        assert!(!declaration.contains("from './"), "{declaration}");
    }

    #[test]
    fn resolves_scoped_dependency_package_subpath() {
        let root = PathBuf::from("/project/library");
        let dependency_root = PathBuf::from("/project/oh_modules/@scope/models");
        let entry = root.join("Index.d.ets");
        let dependency_entry = dependency_root.join("Index.d.ets");
        let sources = BTreeMap::from([
            (
                entry.clone(),
                concat!(
                    "import { User } from '@scope/models/public/User';\n",
                    "export declare function load(): User;"
                )
                .to_string(),
            ),
            (
                dependency_entry.clone(),
                "export interface Root {}".to_string(),
            ),
            (
                dependency_root.join("public/User.d.ets"),
                "export interface User { id: number; }".to_string(),
            ),
        ]);

        let output = DeclarationMerger::new(sources)
            .with_package_declarations([("@scope/models", dependency_root, dependency_entry)])
            .merge(std::slice::from_ref(&entry))
            .expect("merge declarations");
        let declaration = output.files.get(&entry).expect("entry output");
        assert!(declaration.contains("interface User"), "{declaration}");
        assert!(declaration.contains("load(): _User.User"), "{declaration}");
        assert!(!declaration.contains("@scope/models"), "{declaration}");
    }
}
