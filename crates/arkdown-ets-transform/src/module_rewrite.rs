use std::borrow::Cow;
use std::path::{Path, PathBuf};

use rustc_hash::{FxHashMap, FxHashSet};

use arkdown_path::ModuleSpecifierText;
use oxc_allocator::{Allocator, Vec as OxcVec};
use oxc_ast::{
    ast::{
        BindingPattern, Declaration, ExportAllDeclaration, ExportDefaultDeclarationKind,
        ExportNamedDeclaration, ImportDeclaration, ImportDeclarationSpecifier, ImportOrExportKind,
        Program, Statement, StringLiteral,
    },
    AstBuilder, NONE,
};
use oxc_codegen::{Codegen, CodegenOptions, IndentChar};
use oxc_parser::Parser;
use oxc_semantic::Scoping;
use oxc_span::{SourceType, SPAN};
use oxc_syntax::symbol::SymbolId;

use crate::module_emit::{
    ExportBinding, ImportSplitEmitter, LocalExportSplitEmitter, ModuleBindingName,
    ModuleStringLiteral, NamedImportBinding, ReExportSplitEmitter, TypeImportBinding,
    ValueImportBinding,
};
use crate::source::{SourceClassifier, TransformSourceFile, TransformSourceSnapshot};
use crate::transform::source_emit::CodegenSourceText;
use crate::ModuleExportNameText;

#[cfg(arkdown_yuku_native)]
mod yuku_module_analysis;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModuleAnalyzerKind {
    #[default]
    Oxc,
    Yuku,
}

impl ModuleAnalyzerKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Oxc => "oxc",
            Self::Yuku => "yuku",
        }
    }

    pub fn is_available(self) -> bool {
        match self {
            Self::Oxc => true,
            Self::Yuku => cfg!(arkdown_yuku_native),
        }
    }
}

pub(super) struct TypeOnlyNamedImportSplitter<'a> {
    id: &'a str,
    sources: &'a TransformSourceSnapshot,
    module_analyzer: ModuleAnalyzerKind,
}

impl<'a> TypeOnlyNamedImportSplitter<'a> {
    pub(super) fn new(id: &'a str, sources: &'a TransformSourceSnapshot) -> Self {
        Self {
            id,
            sources,
            module_analyzer: ModuleAnalyzerKind::default(),
        }
    }

    pub(super) fn with_module_analyzer(mut self, module_analyzer: ModuleAnalyzerKind) -> Self {
        self.module_analyzer = module_analyzer;
        self
    }

    pub(crate) fn split_program<'ast>(
        &self,
        allocator: &'ast Allocator,
        program: &mut Program<'ast>,
        scoping: &Scoping,
    ) -> bool {
        let local_bindings = TopLevelBindingKindCollector::new(program).collect();
        let mut type_import_locals = FxHashSet::default();
        let mut binding_classifier =
            ModuleBindingClassifier::new(self.sources, self.module_analyzer);
        let mut split_plans = Vec::with_capacity(program.body.len());
        for statement in &program.body {
            let split_plan = match statement {
                Statement::ImportDeclaration(import) => {
                    let split = ImportDeclarationSplit::new(
                        import,
                        self.id,
                        &mut binding_classifier,
                        scoping,
                    )
                    .split();
                    split.map(|split| {
                        type_import_locals.extend(split.type_local_names().iter().cloned());
                        split.into_emitter()
                    })
                }
                _ => None,
            };
            split_plans.push(split_plan);
        }

        let ast = AstBuilder::new(allocator);
        let mut changed = false;
        let mut body = OxcVec::new_in(&allocator);
        let old_body = std::mem::replace(&mut program.body, OxcVec::new_in(&allocator));
        for (statement, split_plan) in old_body.into_iter().zip(split_plans) {
            if let Some(split) = split_plan {
                changed = true;
                for statement in split.statements(ast) {
                    body.push(statement);
                }
            } else if let Some(split) = match &statement {
                Statement::ExportNamedDeclaration(export) => {
                    LocalTypeOnlyExportSplit::new(export, &type_import_locals, &local_bindings)
                        .split()
                }
                _ => None,
            } {
                changed = true;
                for statement in split.statements(ast) {
                    body.push(statement);
                }
            } else {
                body.push(statement);
            }
        }
        program.body = body;
        changed
    }
}

pub(super) struct DefaultImportReexportRewriter;

impl DefaultImportReexportRewriter {
    pub(super) fn new() -> Self {
        Self
    }

    pub(crate) fn rewrite_program<'ast>(
        &self,
        allocator: &'ast Allocator,
        program: &mut Program<'ast>,
        scoping: &Scoping,
    ) -> bool {
        let Some(plan) = DefaultImportReexportAnalyzer::new(program, scoping).analyze() else {
            return false;
        };

        let ast = AstBuilder::new(allocator);
        let mut body = OxcVec::new_in(&allocator);
        let old_body = std::mem::replace(&mut program.body, OxcVec::new_in(&allocator));
        for (statement_index, statement) in old_body.into_iter().enumerate() {
            if statement_index == plan.import_statement_index {
                continue;
            }
            if statement_index == plan.export_statement_index {
                body.push(plan.export_statement(ast));
            } else {
                body.push(statement);
            }
        }
        program.body = body;
        true
    }
}

struct DefaultImportReexportAnalyzer<'a, 'b> {
    program: &'a Program<'a>,
    scoping: &'b oxc_semantic::Scoping,
}

impl<'a, 'b> DefaultImportReexportAnalyzer<'a, 'b> {
    fn new(program: &'a Program<'a>, scoping: &'b oxc_semantic::Scoping) -> Self {
        Self { program, scoping }
    }

    fn analyze(&self) -> Option<DefaultImportReexportPlan> {
        let default_imports = self.collect_default_imports();
        for (statement_index, statement) in self.program.body.iter().enumerate() {
            let Some(local_name) = Self::default_export_local_name(statement) else {
                continue;
            };
            let import = default_imports.get(&local_name)?;
            if self.has_only_default_export_reference(import.symbol_id) {
                return Some(DefaultImportReexportPlan {
                    import_statement_index: import.statement_index,
                    export_statement_index: statement_index,
                    source: import.source.clone(),
                });
            }
        }
        None
    }

    fn collect_default_imports(&self) -> FxHashMap<String, DefaultImportCandidate> {
        let mut imports = FxHashMap::default();
        for (statement_index, statement) in self.program.body.iter().enumerate() {
            let Statement::ImportDeclaration(import) = statement else {
                continue;
            };
            let Some(candidate) = DefaultImportCandidate::from_import(statement_index, import)
            else {
                continue;
            };
            imports.insert(candidate.local_name.clone(), candidate);
        }
        imports
    }

    fn default_export_local_name(statement: &Statement<'_>) -> Option<String> {
        let Statement::ExportDefaultDeclaration(export) = statement else {
            return None;
        };
        let ExportDefaultDeclarationKind::Identifier(identifier) = &export.declaration else {
            return None;
        };
        Some(identifier.name.to_string())
    }

    fn has_only_default_export_reference(&self, symbol_id: SymbolId) -> bool {
        self.scoping
            .get_resolved_references(symbol_id)
            .filter(|reference| reference.flags().is_value())
            .count()
            == 1
    }
}

struct DefaultImportCandidate {
    statement_index: usize,
    local_name: String,
    source: ModuleStringLiteral,
    symbol_id: SymbolId,
}

impl DefaultImportCandidate {
    fn from_import(statement_index: usize, import: &ImportDeclaration<'_>) -> Option<Self> {
        if import.import_kind.is_type() {
            return None;
        }
        let specifiers = import.specifiers.as_ref()?;
        let [ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier)] = specifiers.as_slice()
        else {
            return None;
        };
        Some(Self {
            statement_index,
            local_name: specifier.local.name.to_string(),
            source: ModuleStringLiteral::from_string_literal(&import.source),
            symbol_id: specifier.local.symbol_id.get()?,
        })
    }
}

struct DefaultImportReexportPlan {
    import_statement_index: usize,
    export_statement_index: usize,
    source: ModuleStringLiteral,
}

impl DefaultImportReexportPlan {
    fn export_statement<'a>(&self, ast: AstBuilder<'a>) -> Statement<'a> {
        let default_name = || ast.module_export_name_identifier_name(SPAN, ast.str("default"));
        let specifiers = ast.vec1(ast.export_specifier(
            SPAN,
            default_name(),
            default_name(),
            ImportOrExportKind::Value,
        ));
        Statement::from(ast.module_declaration_export_named_declaration(
            SPAN,
            None,
            specifiers,
            Some(self.source.to_ast(ast)),
            ImportOrExportKind::Value,
            NONE,
        ))
    }
}

struct ImportDeclarationSplit<'a, 'source, 'classifier> {
    import: &'a ImportDeclaration<'a>,
    id: &'source str,
    binding_classifier: &'classifier mut ModuleBindingClassifier<'source>,
    scoping: &'source oxc_semantic::Scoping,
}

impl<'a, 'source, 'classifier> ImportDeclarationSplit<'a, 'source, 'classifier> {
    fn new(
        import: &'a ImportDeclaration<'a>,
        id: &'source str,
        binding_classifier: &'classifier mut ModuleBindingClassifier<'source>,
        scoping: &'source oxc_semantic::Scoping,
    ) -> Self {
        Self {
            import,
            id,
            binding_classifier,
            scoping,
        }
    }

    fn split(mut self) -> Option<NamedImportSplit> {
        if self.import.import_kind.is_type() {
            return None;
        }
        let specifiers = self.import.specifiers.as_ref()?;
        let source = ModuleStringLiteral::from_string_literal(&self.import.source);
        let mut value_bindings = Vec::new();
        let mut type_bindings = Vec::new();
        let mut changed_bindings = false;

        for specifier in specifiers {
            match specifier {
                ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                    let imported_name =
                        ModuleExportNameText::new(&specifier.imported).binding_name();
                    let local_name = specifier.local.name.to_string();
                    let named = NamedImportBinding {
                        imported: ModuleBindingName::from_module_export_name(&specifier.imported),
                        local: local_name.clone(),
                    };
                    match self.binding_decision(
                        specifier.import_kind.is_type(),
                        specifier.local.symbol_id.get(),
                        &imported_name,
                    ) {
                        ImportBindingDecision::Value => {
                            value_bindings.push(ValueImportBinding::Named(named));
                        }
                        ImportBindingDecision::Type => {
                            changed_bindings = true;
                            type_bindings.push(TypeImportBinding::Named(named));
                        }
                        ImportBindingDecision::Drop => {
                            changed_bindings = true;
                        }
                    }
                }
                ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                    let local_name = specifier.local.name.to_string();
                    match self.binding_decision(false, specifier.local.symbol_id.get(), "default") {
                        ImportBindingDecision::Value => {
                            value_bindings.push(ValueImportBinding::Default(local_name));
                        }
                        ImportBindingDecision::Type => {
                            changed_bindings = true;
                            type_bindings.push(TypeImportBinding::Default(local_name));
                        }
                        ImportBindingDecision::Drop => {
                            changed_bindings = true;
                        }
                    }
                }
                ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                    value_bindings.push(ValueImportBinding::Namespace(
                        specifier.local.name.to_string(),
                    ));
                }
            }
        }

        if !changed_bindings {
            return None;
        }

        let type_local_names = type_bindings
            .iter()
            .map(|binding| match binding {
                TypeImportBinding::Default(local) => local.clone(),
                TypeImportBinding::Named(binding) => binding.local.clone(),
            })
            .collect();
        Some(NamedImportSplit::new(
            ImportSplitEmitter::new(source, value_bindings, type_bindings),
            type_local_names,
        ))
    }

    fn imported_binding_is_declared_type(&mut self, imported_name: &str) -> bool {
        self.binding_classifier
            .classify_imported_binding(
                Path::new(self.id),
                self.import.source.value.as_str(),
                imported_name,
            )
            .is_some_and(ModuleBindingKind::is_type_only)
    }

    fn binding_decision(
        &mut self,
        explicit_type: bool,
        symbol_id: Option<SymbolId>,
        imported_name: &str,
    ) -> ImportBindingDecision {
        if explicit_type {
            return ImportBindingDecision::Type;
        }
        let Some(symbol_id) = symbol_id else {
            return if self.imported_binding_is_declared_type(imported_name) {
                ImportBindingDecision::Type
            } else {
                ImportBindingDecision::Value
            };
        };
        let usage = ImportLocalReferenceUsage::new(self.scoping, symbol_id);
        if usage.is_unused() {
            return ImportBindingDecision::Drop;
        }
        if usage.has_value {
            return if self.imported_binding_is_declared_type(imported_name) {
                ImportBindingDecision::Type
            } else {
                ImportBindingDecision::Value
            };
        }
        if usage.has_type || self.imported_binding_is_declared_type(imported_name) {
            return ImportBindingDecision::Type;
        }
        ImportBindingDecision::Drop
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImportBindingDecision {
    Value,
    Type,
    Drop,
}

#[derive(Debug, Clone, Copy, Default)]
struct ImportLocalReferenceUsage {
    has_value: bool,
    has_type: bool,
}

impl ImportLocalReferenceUsage {
    fn new(scoping: &oxc_semantic::Scoping, symbol_id: SymbolId) -> Self {
        let mut usage = Self::default();
        for reference in scoping.get_resolved_references(symbol_id) {
            usage.has_value |= reference.flags().is_value();
            usage.has_type |= reference.flags().is_type();
        }
        usage
    }

    fn is_unused(self) -> bool {
        !self.has_value && !self.has_type
    }
}

struct NamedImportSplit {
    emitter: ImportSplitEmitter,
    type_local_names: Vec<String>,
}

impl NamedImportSplit {
    fn new(emitter: ImportSplitEmitter, type_local_names: Vec<String>) -> Self {
        Self {
            emitter,
            type_local_names,
        }
    }

    fn type_local_names(&self) -> &[String] {
        &self.type_local_names
    }

    fn into_emitter(self) -> ImportSplitEmitter {
        self.emitter
    }
}

struct LocalTypeOnlyExportSplit<'a, 'b> {
    export: &'a ExportNamedDeclaration<'a>,
    type_import_locals: &'b FxHashSet<String>,
    local_bindings: &'b FxHashMap<String, ModuleBindingKind>,
}

impl<'a, 'b> LocalTypeOnlyExportSplit<'a, 'b> {
    fn new(
        export: &'a ExportNamedDeclaration<'a>,
        type_import_locals: &'b FxHashSet<String>,
        local_bindings: &'b FxHashMap<String, ModuleBindingKind>,
    ) -> Self {
        Self {
            export,
            type_import_locals,
            local_bindings,
        }
    }

    fn split(&self) -> Option<LocalExportSplitEmitter> {
        if self.export.export_kind.is_type()
            || self.export.source.is_some()
            || self.export.specifiers.is_empty()
        {
            return None;
        }

        let mut value_specifiers = Vec::new();
        let mut type_specifiers = Vec::new();

        for specifier in &self.export.specifiers {
            let local_name = ModuleExportNameText::new(&specifier.local).binding_name();
            let binding = ExportBinding::from_export_specifier(specifier);
            let is_type = specifier.export_kind.is_type()
                || self.type_import_locals.contains(&local_name)
                || self
                    .local_bindings
                    .get(&local_name)
                    .is_some_and(|kind| *kind == ModuleBindingKind::Type);
            if is_type {
                type_specifiers.push(binding);
            } else {
                value_specifiers.push(binding);
            }
        }

        if type_specifiers.is_empty() {
            return None;
        }

        Some(LocalExportSplitEmitter::new(
            value_specifiers,
            type_specifiers,
        ))
    }
}

pub struct StaticModuleSpecifierRewrite<'a, Rewrite>
where
    Rewrite: FnMut(&str) -> Option<String>,
{
    source: &'a str,
    id: &'a str,
    type_only_policy: TypeOnlySpecifierPolicy,
    rewrite: Rewrite,
}

impl<'a, Rewrite> StaticModuleSpecifierRewrite<'a, Rewrite>
where
    Rewrite: FnMut(&str) -> Option<String>,
{
    /// Rewrites static `import` / `export ... from` module specifiers through
    /// OXC AST mutation. This mirrors the TypeScript visitor shape used by
    /// hvigor's `process_import` / `process_kit_import`: only module specifier
    /// nodes are rewritten and the final source is emitted by OXC codegen.
    pub fn new(source: &'a str, id: &'a str, rewrite: Rewrite) -> Self {
        Self {
            source,
            id,
            type_only_policy: TypeOnlySpecifierPolicy::RewriteNonRelative,
            rewrite,
        }
    }

    /// Rewrites only value-bearing static `import` / `export ... from` module
    /// specifiers. This matches hvigor `import_path_expand`, which does not
    /// expand pure type imports.
    pub fn value_only(source: &'a str, id: &'a str, rewrite: Rewrite) -> Self {
        Self {
            source,
            id,
            type_only_policy: TypeOnlySpecifierPolicy::Skip,
            rewrite,
        }
    }

    pub fn rewrite(mut self) -> Cow<'a, str> {
        let allocator = Allocator::default();
        let source_type =
            SourceType::from_path(self.id).unwrap_or_else(|_| SourceType::ts().with_module(true));
        let mut parsed = Parser::new(&allocator, self.source, source_type).parse();
        if !parsed.diagnostics.is_empty() {
            return Cow::Borrowed(self.source);
        }

        if !self.rewrite_program(&allocator, &mut parsed.program) {
            return Cow::Borrowed(self.source);
        }

        let code = Codegen::new()
            .with_options(CodegenOptions {
                indent_char: IndentChar::Space,
                indent_width: 4,
                ..CodegenOptions::default()
            })
            .build(&parsed.program)
            .code;
        Cow::Owned(CodegenSourceText::new(code).without_trailing_newline())
    }

    pub(crate) fn rewrite_program<'ast>(
        &mut self,
        allocator: &'ast Allocator,
        program: &mut Program<'ast>,
    ) -> bool {
        let mut rewriter =
            StaticModuleSpecifierRewriter::new(allocator, self.type_only_policy, &mut self.rewrite);
        rewriter.rewrite_program(program);
        rewriter.changed()
    }
}

#[derive(Debug, Clone, Copy)]
enum TypeOnlySpecifierPolicy {
    RewriteNonRelative,
    Skip,
}

struct StaticModuleSpecifierRewriter<'a, 'rewrite, Rewrite>
where
    Rewrite: FnMut(&str) -> Option<String>,
{
    ast: AstBuilder<'a>,
    type_only_policy: TypeOnlySpecifierPolicy,
    rewrite: &'rewrite mut Rewrite,
    changed: bool,
}

impl<'a, 'rewrite, Rewrite> StaticModuleSpecifierRewriter<'a, 'rewrite, Rewrite>
where
    Rewrite: FnMut(&str) -> Option<String>,
{
    fn new(
        allocator: &'a Allocator,
        type_only_policy: TypeOnlySpecifierPolicy,
        rewrite: &'rewrite mut Rewrite,
    ) -> Self {
        Self {
            ast: AstBuilder::new(allocator),
            type_only_policy,
            rewrite,
            changed: false,
        }
    }

    fn rewrite_program(&mut self, program: &mut Program<'a>) {
        for statement in &mut program.body {
            self.rewrite_statement(statement);
        }
    }

    fn changed(&self) -> bool {
        self.changed
    }

    fn rewrite_statement(&mut self, statement: &mut Statement<'a>) {
        match statement {
            Statement::ImportDeclaration(import) => {
                if self.should_skip_type_only(import.import_kind.is_type(), &import.source) {
                    return;
                }
                self.rewrite_literal(&mut import.source);
            }
            Statement::ExportNamedDeclaration(export) => {
                let is_type_only = export.export_kind.is_type();
                if let Some(source) = &mut export.source {
                    if self.should_skip_type_only(is_type_only, source) {
                        return;
                    }
                    self.rewrite_literal(source);
                }
            }
            Statement::ExportAllDeclaration(export) => {
                if self.should_skip_type_only(export.export_kind.is_type(), &export.source) {
                    return;
                }
                self.rewrite_literal(&mut export.source);
            }
            _ => {}
        }
    }

    fn should_skip_type_only(&self, is_type_only: bool, source: &StringLiteral<'_>) -> bool {
        if !is_type_only {
            return false;
        }

        match self.type_only_policy {
            TypeOnlySpecifierPolicy::Skip => true,
            TypeOnlySpecifierPolicy::RewriteNonRelative => {
                ModuleSpecifierText::new(source.value.as_str()).is_relative()
            }
        }
    }

    fn rewrite_literal(&mut self, literal: &mut StringLiteral<'a>) {
        let specifier = literal.value.as_str();
        let Some(replacement) = (self.rewrite)(specifier) else {
            return;
        };
        if replacement == specifier {
            return;
        }
        literal.value = self.ast.str(&replacement);
        literal.raw = None;
        self.changed = true;
    }
}

pub(super) struct TypeOnlyReexportSplitter<'a> {
    id: &'a str,
    sources: &'a TransformSourceSnapshot,
    module_analyzer: ModuleAnalyzerKind,
}

impl<'a> TypeOnlyReexportSplitter<'a> {
    pub(super) fn new(id: &'a str, sources: &'a TransformSourceSnapshot) -> Self {
        Self {
            id,
            sources,
            module_analyzer: ModuleAnalyzerKind::default(),
        }
    }

    pub(super) fn with_module_analyzer(mut self, module_analyzer: ModuleAnalyzerKind) -> Self {
        self.module_analyzer = module_analyzer;
        self
    }

    pub(crate) fn split_program<'ast>(
        &self,
        allocator: &'ast Allocator,
        program: &mut Program<'ast>,
    ) -> bool {
        let mut binding_classifier =
            ModuleBindingClassifier::new(self.sources, self.module_analyzer);
        let mut split_plans = Vec::with_capacity(program.body.len());
        for statement in &program.body {
            let split_plan = match statement {
                Statement::ExportNamedDeclaration(export) => {
                    TypeOnlyReexportSplit::new(export, self.id, &mut binding_classifier).split()
                }
                _ => None,
            };
            split_plans.push(split_plan);
        }

        let ast = AstBuilder::new(allocator);
        let mut changed = false;
        let mut body = OxcVec::new_in(&allocator);
        let old_body = std::mem::replace(&mut program.body, OxcVec::new_in(&allocator));
        for (statement, split_plan) in old_body.into_iter().zip(split_plans) {
            if let Some(split) = split_plan {
                changed = true;
                for statement in split.statements(ast) {
                    body.push(statement);
                }
            } else {
                body.push(statement);
            }
        }
        program.body = body;
        changed
    }
}

struct TypeOnlyReexportSplit<'a, 'source, 'classifier> {
    export: &'a ExportNamedDeclaration<'a>,
    id: &'source str,
    binding_classifier: &'classifier mut ModuleBindingClassifier<'source>,
}

impl<'a, 'source, 'classifier> TypeOnlyReexportSplit<'a, 'source, 'classifier> {
    fn new(
        export: &'a ExportNamedDeclaration<'a>,
        id: &'source str,
        binding_classifier: &'classifier mut ModuleBindingClassifier<'source>,
    ) -> Self {
        Self {
            export,
            id,
            binding_classifier,
        }
    }

    fn split(self) -> Option<ReExportSplitEmitter> {
        if self.export.export_kind.is_type() || self.export.specifiers.is_empty() {
            return None;
        }
        let source = self.export.source.as_ref()?;
        let specifier = source.value.as_str();
        let target_exports = self
            .binding_classifier
            .classify_specifier_exports(Path::new(self.id), specifier)?;

        let mut value_specifiers = Vec::new();
        let mut type_specifiers = Vec::new();

        for specifier in &self.export.specifiers {
            let local_name = ModuleExportNameText::new(&specifier.local).binding_name();
            let binding = ExportBinding::from_export_specifier(specifier);
            let is_type = specifier.export_kind.is_type()
                || target_exports
                    .get(&local_name)
                    .is_some_and(|kind| *kind == ModuleBindingKind::Type);
            if is_type {
                type_specifiers.push(binding);
            } else {
                value_specifiers.push(binding);
            }
        }

        if type_specifiers.is_empty() {
            return None;
        }

        Some(ReExportSplitEmitter::new(
            ModuleStringLiteral::from_string_literal(source),
            value_specifiers,
            type_specifiers,
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ModuleBindingKind {
    Type,
    Value,
}

impl ModuleBindingKind {
    fn is_type_only(self) -> bool {
        self == Self::Type
    }

    fn record(bindings: &mut FxHashMap<String, Self>, name: impl Into<String>, kind: Self) {
        let name = name.into();
        match bindings.get_mut(&name) {
            Some(existing) if *existing == Self::Value || kind == Self::Type => {}
            Some(existing) => *existing = Self::Value,
            None => {
                bindings.insert(name, kind);
            }
        }
    }
}

struct ModuleBindingClassifier<'a> {
    sources: &'a TransformSourceSnapshot,
    module_analyzer: ModuleAnalyzerKind,
    exports: FxHashMap<PathBuf, Option<FxHashMap<String, ModuleBindingKind>>>,
    #[cfg(arkdown_yuku_native)]
    yuku_classifier: Option<yuku_module_analysis::YukuModuleExportClassifier<'a>>,
}

impl<'a> ModuleBindingClassifier<'a> {
    fn new(sources: &'a TransformSourceSnapshot, module_analyzer: ModuleAnalyzerKind) -> Self {
        Self {
            sources,
            module_analyzer,
            exports: FxHashMap::default(),
            #[cfg(arkdown_yuku_native)]
            yuku_classifier: None,
        }
    }

    fn classify_imported_binding(
        &mut self,
        importer: &Path,
        specifier: &str,
        imported_name: &str,
    ) -> Option<ModuleBindingKind> {
        let target = self
            .sources
            .resolve_static_with_declarations(importer, specifier)?;
        let is_declaration = SourceClassifier::classify_path(&target.path).is_declaration();
        let is_runtime_backed_declaration =
            is_declaration && self.sources.declaration_has_runtime_source(&target.path);
        let exports = self.classify_source_exports(target)?;
        if is_declaration
            && imported_name == "default"
            && NativeSharedObjectSpecifier::new(specifier).is_native_shared_object()
        {
            return Some(ModuleBindingKind::Value);
        }
        exports.get(imported_name).copied().or_else(|| {
            if !is_declaration {
                return None;
            }
            // TypeScript's declaration resolver can select `index.d.ts` for a
            // CommonJS `index.js`. An export absent from that declaration is
            // still a runtime binding supplied by the JavaScript module (for
            // example its synthetic default export). Hvigor's TS checker keeps
            // that import as a value; do not turn it into `import type`.
            Some(if is_runtime_backed_declaration {
                ModuleBindingKind::Value
            } else {
                ModuleBindingKind::Type
            })
        })
    }

    fn classify_specifier_exports(
        &mut self,
        importer: &Path,
        specifier: &str,
    ) -> Option<FxHashMap<String, ModuleBindingKind>> {
        let target = self
            .sources
            .resolve_static_with_declarations(importer, specifier)?;
        self.classify_source_exports(target)
    }

    fn classify_source_exports(
        &mut self,
        source: TransformSourceFile,
    ) -> Option<FxHashMap<String, ModuleBindingKind>> {
        if let Some(exports) = self.exports.get(&source.path) {
            return exports.clone();
        }

        let path = source.path.clone();
        let exports = self.analyze_source_exports(source);
        self.exports.insert(path, exports.clone());
        exports
    }

    fn analyze_source_exports(
        &mut self,
        source: TransformSourceFile,
    ) -> Option<FxHashMap<String, ModuleBindingKind>> {
        #[cfg(not(arkdown_yuku_native))]
        let _ = self.module_analyzer;

        #[cfg(arkdown_yuku_native)]
        if self.module_analyzer == ModuleAnalyzerKind::Yuku {
            let classifier = self.yuku_classifier.get_or_insert_with(|| {
                yuku_module_analysis::YukuModuleExportClassifier::new(self.sources)
            });
            if let Some(exports) = classifier.analyze(source.clone()) {
                return Some(exports);
            }
        }

        ModuleExportClassifier::new(source)
            .with_sources(self.sources)
            .analyze()
    }
}

struct NativeSharedObjectSpecifier<'a> {
    specifier: &'a str,
}

impl<'a> NativeSharedObjectSpecifier<'a> {
    fn new(specifier: &'a str) -> Self {
        Self { specifier }
    }

    fn is_native_shared_object(&self) -> bool {
        self.specifier.starts_with("lib") && self.specifier.ends_with(".so")
    }
}

struct ModuleExportClassifier<'a> {
    source: TransformSourceFile,
    sources: Option<&'a TransformSourceSnapshot>,
}

impl<'a> ModuleExportClassifier<'a> {
    fn new(source: TransformSourceFile) -> Self {
        Self {
            source,
            sources: None,
        }
    }

    fn with_sources(mut self, sources: &'a TransformSourceSnapshot) -> Self {
        self.sources = Some(sources);
        self
    }

    fn analyze(&self) -> Option<FxHashMap<String, ModuleBindingKind>> {
        let mut visited = FxHashSet::default();
        self.analyze_with_visited(&mut visited)
    }

    fn analyze_with_visited(
        &self,
        visited: &mut FxHashSet<PathBuf>,
    ) -> Option<FxHashMap<String, ModuleBindingKind>> {
        let source_path = self.source.path.clone();
        if !visited.insert(source_path.clone()) {
            return None;
        }

        let allocator = Allocator::default();
        let source_type = SourceType::from_path(&self.source.path)
            .unwrap_or_else(|_| SourceType::ts().with_module(true));
        let parsed = Parser::new(&allocator, &self.source.source, source_type).parse();
        if !parsed.diagnostics.is_empty() {
            visited.remove(&source_path);
            return None;
        }

        let local_bindings = TopLevelBindingKindCollector::new(&parsed.program).collect();
        let needed_import_locals = ExportImportLocalCollector::new(&parsed.program).collect();
        let imported_bindings = ImportBindingKindAnalyzer::new(&self.source, self.sources, visited)
            .analyze(&parsed.program, &needed_import_locals);
        let mut exports = FxHashMap::default();
        for statement in &parsed.program.body {
            match statement {
                Statement::ExportNamedDeclaration(export) => {
                    self.collect_named_export(
                        export,
                        &local_bindings,
                        &imported_bindings,
                        visited,
                        &mut exports,
                    );
                }
                Statement::ExportDefaultDeclaration(export) => {
                    DefaultExportBindingCollector::new(
                        &export.declaration,
                        &local_bindings,
                        &imported_bindings,
                        &mut exports,
                    )
                    .collect();
                }
                Statement::ExportAllDeclaration(export) => {
                    self.collect_export_all(export, visited, &mut exports);
                }
                _ => {}
            }
        }
        visited.remove(&source_path);
        Some(exports)
    }

    fn collect_named_export(
        &self,
        export: &ExportNamedDeclaration<'_>,
        local_bindings: &FxHashMap<String, ModuleBindingKind>,
        imported_bindings: &FxHashMap<String, ModuleBindingKind>,
        visited: &mut FxHashSet<PathBuf>,
        exports: &mut FxHashMap<String, ModuleBindingKind>,
    ) {
        if let Some(declaration) = &export.declaration {
            ExportedDeclarationCollector::new(declaration, exports).collect();
        }
        let reexport_bindings = export.source.as_ref().and_then(|source| {
            let sources = self.sources?;
            let target = sources
                .resolve_static_with_declarations(&self.source.path, source.value.as_str())?;
            ModuleExportClassifier::new(target)
                .with_sources(sources)
                .analyze_with_visited(visited)
        });
        for specifier in &export.specifiers {
            let exported_name = ModuleExportNameText::new(&specifier.exported).binding_name();
            let local_name = ModuleExportNameText::new(&specifier.local).binding_name();
            if specifier.export_kind.is_type() || export.export_kind.is_type() {
                ModuleBindingKind::record(exports, exported_name, ModuleBindingKind::Type);
            } else if let Some(kind) = reexport_bindings
                .as_ref()
                .and_then(|bindings| bindings.get(&local_name))
                .copied()
            {
                ModuleBindingKind::record(exports, exported_name, kind);
            } else if export.source.is_none() {
                if let Some(kind) = local_bindings.get(&local_name).copied() {
                    ModuleBindingKind::record(exports, exported_name, kind);
                } else if let Some(kind) = exports.get(&local_name).copied() {
                    ModuleBindingKind::record(exports, exported_name, kind);
                } else if let Some(kind) = imported_bindings.get(&local_name).copied() {
                    ModuleBindingKind::record(exports, exported_name, kind);
                } else {
                    ModuleBindingKind::record(exports, exported_name, ModuleBindingKind::Value);
                }
            } else if let Some(kind) = exports.get(&local_name).copied() {
                ModuleBindingKind::record(exports, exported_name, kind);
            } else if let Some(kind) = imported_bindings.get(&local_name).copied() {
                ModuleBindingKind::record(exports, exported_name, kind);
            } else {
                ModuleBindingKind::record(exports, exported_name, ModuleBindingKind::Value);
            }
        }
    }

    fn collect_export_all(
        &self,
        export: &ExportAllDeclaration<'_>,
        visited: &mut FxHashSet<PathBuf>,
        exports: &mut FxHashMap<String, ModuleBindingKind>,
    ) {
        if let Some(exported) = &export.exported {
            let kind = if export.export_kind.is_type() {
                ModuleBindingKind::Type
            } else {
                ModuleBindingKind::Value
            };
            ModuleBindingKind::record(
                exports,
                ModuleExportNameText::new(exported).binding_name(),
                kind,
            );
            return;
        }

        let Some(sources) = self.sources else {
            return;
        };
        let Some(target) = sources
            .resolve_static_with_declarations(&self.source.path, export.source.value.as_str())
        else {
            return;
        };
        let target_exports = ModuleExportClassifier::new(target)
            .with_sources(sources)
            .analyze_with_visited(visited);
        let Some(target_exports) = target_exports else {
            return;
        };
        for (name, kind) in target_exports {
            if name != "default" {
                let export_kind = if export.export_kind.is_type() {
                    ModuleBindingKind::Type
                } else {
                    kind
                };
                ModuleBindingKind::record(exports, name, export_kind);
            }
        }
    }
}

struct DefaultExportBindingCollector<'a, 'b> {
    declaration: &'a ExportDefaultDeclarationKind<'a>,
    local_bindings: &'b FxHashMap<String, ModuleBindingKind>,
    imported_bindings: &'b FxHashMap<String, ModuleBindingKind>,
    exports: &'b mut FxHashMap<String, ModuleBindingKind>,
}

impl<'a, 'b> DefaultExportBindingCollector<'a, 'b> {
    fn new(
        declaration: &'a ExportDefaultDeclarationKind<'a>,
        local_bindings: &'b FxHashMap<String, ModuleBindingKind>,
        imported_bindings: &'b FxHashMap<String, ModuleBindingKind>,
        exports: &'b mut FxHashMap<String, ModuleBindingKind>,
    ) -> Self {
        Self {
            declaration,
            local_bindings,
            imported_bindings,
            exports,
        }
    }

    fn collect(&mut self) {
        match self.declaration {
            ExportDefaultDeclarationKind::TSInterfaceDeclaration(_) => {
                ModuleBindingKind::record(self.exports, "default", ModuleBindingKind::Type);
            }
            ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
                if !function.declare {
                    ModuleBindingKind::record(self.exports, "default", ModuleBindingKind::Value);
                }
            }
            ExportDefaultDeclarationKind::ClassDeclaration(class) => {
                if !class.declare {
                    ModuleBindingKind::record(self.exports, "default", ModuleBindingKind::Value);
                }
            }
            ExportDefaultDeclarationKind::Identifier(identifier) => {
                let local_name = identifier.name.as_str();
                let kind = self
                    .local_bindings
                    .get(local_name)
                    .or_else(|| self.imported_bindings.get(local_name))
                    .copied()
                    .unwrap_or(ModuleBindingKind::Value);
                ModuleBindingKind::record(self.exports, "default", kind);
            }
            _ => {
                ModuleBindingKind::record(self.exports, "default", ModuleBindingKind::Value);
            }
        }
    }
}

struct ImportBindingKindAnalyzer<'a> {
    source: &'a TransformSourceFile,
    sources: Option<&'a TransformSourceSnapshot>,
    visited: &'a mut FxHashSet<PathBuf>,
}

impl<'a> ImportBindingKindAnalyzer<'a> {
    fn new(
        source: &'a TransformSourceFile,
        sources: Option<&'a TransformSourceSnapshot>,
        visited: &'a mut FxHashSet<PathBuf>,
    ) -> Self {
        Self {
            source,
            sources,
            visited,
        }
    }

    fn analyze(
        &mut self,
        program: &Program<'_>,
        needed_locals: &FxHashSet<String>,
    ) -> FxHashMap<String, ModuleBindingKind> {
        let mut imports = FxHashMap::default();
        for statement in &program.body {
            let Statement::ImportDeclaration(import) = statement else {
                continue;
            };
            self.collect_import(import, needed_locals, &mut imports);
        }
        imports
    }

    fn collect_import(
        &mut self,
        import: &ImportDeclaration<'_>,
        needed_locals: &FxHashSet<String>,
        imports: &mut FxHashMap<String, ModuleBindingKind>,
    ) {
        let Some(specifiers) = &import.specifiers else {
            return;
        };
        if !ImportLocalNeed::new(specifiers, needed_locals).has_needed_local() {
            return;
        }
        if import.import_kind.is_type() {
            for specifier in specifiers {
                self.collect_explicit_type_import(specifier, needed_locals, imports);
            }
            return;
        }

        let Some(sources) = self.sources else {
            return;
        };
        let Some(target) = sources
            .resolve_static_with_declarations(&self.source.path, import.source.value.as_str())
        else {
            return;
        };
        let target_exports = ModuleExportClassifier::new(target)
            .with_sources(sources)
            .analyze_with_visited(self.visited);
        let Some(target_exports) = target_exports else {
            return;
        };

        for specifier in specifiers {
            match specifier {
                ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                    let imported_name =
                        ModuleExportNameText::new(&specifier.imported).binding_name();
                    let local_name = specifier.local.name.to_string();
                    if !needed_locals.contains(&local_name) {
                        continue;
                    }
                    if specifier.import_kind.is_type()
                        || target_exports
                            .get(&imported_name)
                            .is_some_and(|kind| *kind == ModuleBindingKind::Type)
                    {
                        imports.insert(local_name, ModuleBindingKind::Type);
                    }
                }
                ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                    let local_name = specifier.local.name.to_string();
                    if !needed_locals.contains(&local_name) {
                        continue;
                    }
                    if target_exports
                        .get("default")
                        .is_some_and(|kind| *kind == ModuleBindingKind::Type)
                    {
                        imports.insert(local_name, ModuleBindingKind::Type);
                    }
                }
                ImportDeclarationSpecifier::ImportNamespaceSpecifier(_) => {}
            }
        }
    }

    fn collect_explicit_type_import(
        &self,
        specifier: &ImportDeclarationSpecifier<'_>,
        needed_locals: &FxHashSet<String>,
        imports: &mut FxHashMap<String, ModuleBindingKind>,
    ) {
        match specifier {
            ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                let local_name = specifier.local.name.to_string();
                if needed_locals.contains(&local_name) {
                    imports.insert(local_name, ModuleBindingKind::Type);
                }
            }
            ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                let local_name = specifier.local.name.to_string();
                if needed_locals.contains(&local_name) {
                    imports.insert(local_name, ModuleBindingKind::Type);
                }
            }
            ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                let local_name = specifier.local.name.to_string();
                if needed_locals.contains(&local_name) {
                    imports.insert(local_name, ModuleBindingKind::Type);
                }
            }
        }
    }
}

struct ExportImportLocalCollector<'a> {
    program: &'a Program<'a>,
}

impl<'a> ExportImportLocalCollector<'a> {
    fn new(program: &'a Program<'a>) -> Self {
        Self { program }
    }

    fn collect(&self) -> FxHashSet<String> {
        let mut locals = FxHashSet::default();
        for statement in &self.program.body {
            let Statement::ExportNamedDeclaration(export) = statement else {
                if let Statement::ExportDefaultDeclaration(export) = statement {
                    self.collect_default_export(export, &mut locals);
                }
                continue;
            };
            if export.source.is_some() {
                continue;
            }
            for specifier in &export.specifiers {
                locals.insert(ModuleExportNameText::new(&specifier.local).binding_name());
            }
        }
        locals
    }

    fn collect_default_export(
        &self,
        export: &oxc_ast::ast::ExportDefaultDeclaration<'_>,
        locals: &mut FxHashSet<String>,
    ) {
        if let ExportDefaultDeclarationKind::Identifier(identifier) = &export.declaration {
            locals.insert(identifier.name.to_string());
        }
    }
}

struct ImportLocalNeed<'a, 'b> {
    specifiers: &'a [ImportDeclarationSpecifier<'a>],
    needed_locals: &'b FxHashSet<String>,
}

impl<'a, 'b> ImportLocalNeed<'a, 'b> {
    fn new(
        specifiers: &'a [ImportDeclarationSpecifier<'a>],
        needed_locals: &'b FxHashSet<String>,
    ) -> Self {
        Self {
            specifiers,
            needed_locals,
        }
    }

    fn has_needed_local(&self) -> bool {
        self.specifiers
            .iter()
            .any(|specifier| self.needed_locals.contains(&self.local_name(specifier)))
    }

    fn local_name(&self, specifier: &ImportDeclarationSpecifier<'_>) -> String {
        match specifier {
            ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                specifier.local.name.to_string()
            }
            ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                specifier.local.name.to_string()
            }
            ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                specifier.local.name.to_string()
            }
        }
    }
}

struct TopLevelBindingKindCollector<'a> {
    program: &'a Program<'a>,
}

impl<'a> TopLevelBindingKindCollector<'a> {
    fn new(program: &'a Program<'a>) -> Self {
        Self { program }
    }

    fn collect(&self) -> FxHashMap<String, ModuleBindingKind> {
        let mut bindings = FxHashMap::default();
        for statement in &self.program.body {
            self.collect_statement(statement, &mut bindings);
        }
        bindings
    }

    fn collect_statement(
        &self,
        statement: &Statement<'_>,
        bindings: &mut FxHashMap<String, ModuleBindingKind>,
    ) {
        match statement {
            Statement::ExportNamedDeclaration(export) => {
                if let Some(declaration) = &export.declaration {
                    ExportedDeclarationCollector::new(declaration, bindings).collect();
                }
            }
            Statement::VariableDeclaration(declaration) => {
                for declarator in &declaration.declarations {
                    BindingNameCollector::new(&declarator.id, bindings).collect();
                }
            }
            Statement::FunctionDeclaration(declaration) => {
                if let Some(id) = &declaration.id {
                    ModuleBindingKind::record(
                        bindings,
                        id.name.to_string(),
                        ModuleBindingKind::Value,
                    );
                }
            }
            Statement::ClassDeclaration(declaration) => {
                if let Some(id) = &declaration.id {
                    ModuleBindingKind::record(
                        bindings,
                        id.name.to_string(),
                        ModuleBindingKind::Value,
                    );
                }
            }
            Statement::TSInterfaceDeclaration(declaration) => {
                ModuleBindingKind::record(
                    bindings,
                    declaration.id.name.to_string(),
                    ModuleBindingKind::Type,
                );
            }
            Statement::TSTypeAliasDeclaration(declaration) => {
                ModuleBindingKind::record(
                    bindings,
                    declaration.id.name.to_string(),
                    ModuleBindingKind::Type,
                );
            }
            Statement::TSEnumDeclaration(declaration) if declaration.r#const => {
                ModuleBindingKind::record(
                    bindings,
                    declaration.id.name.to_string(),
                    ModuleBindingKind::Type,
                );
            }
            Statement::TSEnumDeclaration(declaration) => {
                ModuleBindingKind::record(
                    bindings,
                    declaration.id.name.to_string(),
                    ModuleBindingKind::Value,
                );
            }
            Statement::TSModuleDeclaration(declaration) => {
                if !declaration.declare && declaration.body.is_some() {
                    if let Some(name) = ModuleDeclarationName::new(&declaration.id).binding_name() {
                        ModuleBindingKind::record(bindings, name, ModuleBindingKind::Value);
                    }
                }
            }
            Statement::TSImportEqualsDeclaration(declaration) => {
                ModuleBindingKind::record(
                    bindings,
                    declaration.id.name.to_string(),
                    ModuleBindingKind::Value,
                );
            }
            _ => {}
        }
    }
}

struct ExportedDeclarationCollector<'a, 'b> {
    declaration: &'a Declaration<'a>,
    exports: &'b mut FxHashMap<String, ModuleBindingKind>,
}

impl<'a, 'b> ExportedDeclarationCollector<'a, 'b> {
    fn new(
        declaration: &'a Declaration<'a>,
        exports: &'b mut FxHashMap<String, ModuleBindingKind>,
    ) -> Self {
        Self {
            declaration,
            exports,
        }
    }

    fn collect(&mut self) {
        match self.declaration {
            Declaration::TSInterfaceDeclaration(declaration) => {
                ModuleBindingKind::record(
                    self.exports,
                    declaration.id.name.to_string(),
                    ModuleBindingKind::Type,
                );
            }
            Declaration::TSTypeAliasDeclaration(declaration) => {
                ModuleBindingKind::record(
                    self.exports,
                    declaration.id.name.to_string(),
                    ModuleBindingKind::Type,
                );
            }
            Declaration::TSEnumDeclaration(declaration) if declaration.r#const => {
                ModuleBindingKind::record(
                    self.exports,
                    declaration.id.name.to_string(),
                    ModuleBindingKind::Type,
                );
            }
            Declaration::TSEnumDeclaration(declaration) => {
                ModuleBindingKind::record(
                    self.exports,
                    declaration.id.name.to_string(),
                    ModuleBindingKind::Value,
                );
            }
            Declaration::FunctionDeclaration(declaration) => {
                if let Some(id) = &declaration.id {
                    ModuleBindingKind::record(
                        self.exports,
                        id.name.to_string(),
                        ModuleBindingKind::Value,
                    );
                }
            }
            Declaration::ClassDeclaration(declaration) => {
                if let Some(id) = &declaration.id {
                    ModuleBindingKind::record(
                        self.exports,
                        id.name.to_string(),
                        ModuleBindingKind::Value,
                    );
                }
            }
            Declaration::VariableDeclaration(declaration) => {
                for declarator in &declaration.declarations {
                    BindingNameCollector::new(&declarator.id, self.exports).collect();
                }
            }
            Declaration::TSModuleDeclaration(declaration) => {
                if !declaration.declare && declaration.body.is_some() {
                    if let Some(name) = ModuleDeclarationName::new(&declaration.id).binding_name() {
                        ModuleBindingKind::record(self.exports, name, ModuleBindingKind::Value);
                    }
                }
            }
            Declaration::TSGlobalDeclaration(_)
            | Declaration::StructStatement(_)
            | Declaration::AnnotationDeclaration(_) => {}
            Declaration::TSImportEqualsDeclaration(declaration) => {
                ModuleBindingKind::record(
                    self.exports,
                    declaration.id.name.to_string(),
                    ModuleBindingKind::Value,
                );
            }
        }
    }
}

struct ModuleDeclarationName<'a> {
    name: &'a oxc_ast::ast::TSModuleDeclarationName<'a>,
}

impl<'a> ModuleDeclarationName<'a> {
    fn new(name: &'a oxc_ast::ast::TSModuleDeclarationName<'a>) -> Self {
        Self { name }
    }

    fn binding_name(&self) -> Option<String> {
        match self.name {
            oxc_ast::ast::TSModuleDeclarationName::Identifier(identifier) => {
                Some(identifier.name.to_string())
            }
            oxc_ast::ast::TSModuleDeclarationName::StringLiteral(_) => None,
        }
    }
}

struct BindingNameCollector<'a, 'b> {
    pattern: &'a BindingPattern<'a>,
    exports: &'b mut FxHashMap<String, ModuleBindingKind>,
}

impl<'a, 'b> BindingNameCollector<'a, 'b> {
    fn new(
        pattern: &'a BindingPattern<'a>,
        exports: &'b mut FxHashMap<String, ModuleBindingKind>,
    ) -> Self {
        Self { pattern, exports }
    }

    fn collect(&mut self) {
        match self.pattern {
            BindingPattern::BindingIdentifier(identifier) => {
                ModuleBindingKind::record(
                    self.exports,
                    identifier.name.to_string(),
                    ModuleBindingKind::Value,
                );
            }
            BindingPattern::ObjectPattern(pattern) => {
                for property in &pattern.properties {
                    BindingNameCollector::new(&property.value, self.exports).collect();
                }
                if let Some(rest) = &pattern.rest {
                    BindingNameCollector::new(&rest.argument, self.exports).collect();
                }
            }
            BindingPattern::ArrayPattern(pattern) => {
                for element in pattern.elements.iter().flatten() {
                    BindingNameCollector::new(element, self.exports).collect();
                }
                if let Some(rest) = &pattern.rest {
                    BindingNameCollector::new(&rest.argument, self.exports).collect();
                }
            }
            BindingPattern::AssignmentPattern(pattern) => {
                BindingNameCollector::new(&pattern.left, self.exports).collect();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Arc;

    use crate::typescript_emit::TypeScriptJavascriptEmitter;
    use oxc_semantic::SemanticBuilder;

    fn codegen_source(program: &Program<'_>) -> String {
        CodegenSourceText::new(
            Codegen::new()
                .with_options(CodegenOptions {
                    indent_char: IndentChar::Space,
                    indent_width: 4,
                    ..CodegenOptions::default()
                })
                .build(program)
                .code,
        )
        .without_trailing_newline()
    }

    fn split_type_only_named_imports<'a>(
        source: &'a str,
        id: &str,
        sources: &TransformSourceSnapshot,
    ) -> Cow<'a, str> {
        split_type_only_named_imports_with_analyzer(
            source,
            id,
            sources,
            ModuleAnalyzerKind::default(),
        )
    }

    fn split_type_only_named_imports_with_analyzer<'a>(
        source: &'a str,
        id: &str,
        sources: &TransformSourceSnapshot,
        module_analyzer: ModuleAnalyzerKind,
    ) -> Cow<'a, str> {
        let allocator = Allocator::default();
        let source_type =
            SourceType::from_path(id).unwrap_or_else(|_| SourceType::ts().with_module(true));
        let mut parsed = Parser::new(&allocator, source, source_type).parse();
        if !parsed.diagnostics.is_empty() {
            return Cow::Borrowed(source);
        }
        let semantic_ret = SemanticBuilder::new().build(&parsed.program);
        if !semantic_ret.diagnostics.is_empty() {
            return Cow::Borrowed(source);
        }
        let scoping = semantic_ret.semantic.into_scoping();
        let changed = TypeOnlyNamedImportSplitter::new(id, sources)
            .with_module_analyzer(module_analyzer)
            .split_program(&allocator, &mut parsed.program, &scoping);
        if changed {
            Cow::Owned(codegen_source(&parsed.program))
        } else {
            Cow::Borrowed(source)
        }
    }

    fn split_type_only_reexports<'a>(
        source: &'a str,
        id: &str,
        sources: &TransformSourceSnapshot,
    ) -> Cow<'a, str> {
        split_type_only_reexports_with_analyzer(source, id, sources, ModuleAnalyzerKind::default())
    }

    fn split_type_only_reexports_with_analyzer<'a>(
        source: &'a str,
        id: &str,
        sources: &TransformSourceSnapshot,
        module_analyzer: ModuleAnalyzerKind,
    ) -> Cow<'a, str> {
        let allocator = Allocator::default();
        let source_type =
            SourceType::from_path(id).unwrap_or_else(|_| SourceType::ts().with_module(true));
        let mut parsed = Parser::new(&allocator, source, source_type).parse();
        if !parsed.diagnostics.is_empty() {
            return Cow::Borrowed(source);
        }
        let changed = TypeOnlyReexportSplitter::new(id, sources)
            .with_module_analyzer(module_analyzer)
            .split_program(&allocator, &mut parsed.program);
        if changed {
            Cow::Owned(codegen_source(&parsed.program))
        } else {
            Cow::Borrowed(source)
        }
    }

    fn available_module_analyzers() -> Vec<ModuleAnalyzerKind> {
        [ModuleAnalyzerKind::Oxc, ModuleAnalyzerKind::Yuku]
            .into_iter()
            .filter(|analyzer| analyzer.is_available())
            .collect()
    }

    #[test]
    fn module_binding_classifier_reuses_target_exports_within_pass() {
        let importer = PathBuf::from("/project/src/main.ets");
        let target = PathBuf::from("/project/src/bindings.ts");
        let mut files = FxHashMap::default();
        files.insert(
            importer.clone(),
            Arc::<str>::from(
                r#"import { TypeName, RuntimeName } from "./bindings";

const value = RuntimeName;
type Alias = TypeName;"#,
            ),
        );
        files.insert(
            target,
            Arc::<str>::from(
                r#"export interface TypeName {}
export class RuntimeName {}"#,
            ),
        );
        let sources = TransformSourceSnapshot::new(files);

        for analyzer in available_module_analyzers() {
            let mut classifier = ModuleBindingClassifier::new(&sources, analyzer);
            assert_eq!(
                classifier.classify_imported_binding(&importer, "./bindings", "TypeName"),
                Some(ModuleBindingKind::Type)
            );
            assert_eq!(
                classifier.classify_imported_binding(&importer, "./bindings", "RuntimeName"),
                Some(ModuleBindingKind::Value)
            );
            assert_eq!(
                classifier.exports.len(),
                1,
                "{analyzer:?} must reuse target export analysis within one split pass"
            );
        }
    }

    fn rewrite_default_import_reexport<'a>(source: &'a str, id: &str) -> Cow<'a, str> {
        let allocator = Allocator::default();
        let source_type =
            SourceType::from_path(id).unwrap_or_else(|_| SourceType::ts().with_module(true));
        let mut parsed = Parser::new(&allocator, source, source_type).parse();
        if !parsed.diagnostics.is_empty() {
            return Cow::Borrowed(source);
        }
        let semantic_ret = SemanticBuilder::new().build(&parsed.program);
        if !semantic_ret.diagnostics.is_empty() {
            return Cow::Borrowed(source);
        }
        let scoping = semantic_ret.semantic.into_scoping();
        let changed = DefaultImportReexportRewriter::new().rewrite_program(
            &allocator,
            &mut parsed.program,
            &scoping,
        );
        if changed {
            Cow::Owned(codegen_source(&parsed.program))
        } else {
            Cow::Borrowed(source)
        }
    }

    #[test]
    fn type_only_named_import_from_directory_declaration_is_elided() {
        let importer = PathBuf::from("/project/pkg/src/main/ets/core/AES.ets");
        let declaration = PathBuf::from("/project/pkg/src/main/cpp/types/libmtcrypto/index.d.ts");
        let source = r#"import libmtcrypto from 'libmtcrypto.so';
import { EncryptionResult } from '../../cpp/types/libmtcrypto'

export class AESUtil {
    static encrypt(input: string): libmtcrypto.EncryptionResult {
        return libmtcrypto.AESEncrypt(input, "", "");
    }
}

export { EncryptionResult };
"#;

        let mut files = FxHashMap::default();
        files.insert(importer.clone(), Arc::<str>::from(source));
        files.insert(
            declaration,
            Arc::<str>::from(
                r#"export interface EncryptionResult {
    result: string;
    errorCode: number;
}
export const AESEncrypt: (input: string, key: string, iv: string) => EncryptionResult;
"#,
            ),
        );
        let sources = TransformSourceSnapshot::new(files);
        let importer_id = importer.to_str().unwrap();

        let split = split_type_only_named_imports(source, importer_id, &sources);
        assert!(split
            .contains("import type { EncryptionResult } from \"../../cpp/types/libmtcrypto\";"));
        assert!(split.contains("export type { EncryptionResult };"));

        let emitted = TypeScriptJavascriptEmitter::new(&split, importer_id, &sources)
            .emit()
            .expect("type-only import/export must be valid TypeScript");
        assert!(emitted.contains("libmtcrypto.so"));
        assert!(!emitted.contains("../../cpp/types/libmtcrypto"));
        assert!(!emitted.contains("EncryptionResult"));
    }

    #[test]
    fn type_only_named_import_split_drops_unused_value_specifiers_like_hvigor() {
        let importer = PathBuf::from("/project/pkg/src/main/ets/FormAbility.ets");
        let source = r#"import { UsedType, usedValue, unusedValue, UnusedType } from './dep';

const value = usedValue();
let typed: UsedType;
"#;
        let sources = TransformSourceSnapshot::default();
        let importer_id = importer.to_str().unwrap();

        let split = split_type_only_named_imports(source, importer_id, &sources);
        assert!(
            split.contains("import { usedValue } from \"./dep\";"),
            "{split}"
        );
        assert!(
            split.contains("import type { UsedType } from \"./dep\";"),
            "{split}"
        );
        assert!(!split.contains("unusedValue"), "{split}");
        assert!(!split.contains("UnusedType"), "{split}");

        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &split, SourceType::ts().with_module(true)).parse();
        assert!(
            parsed.diagnostics.is_empty(),
            "type-only import split must emit valid TypeScript: {:?}\n{split}",
            parsed.diagnostics
        );
    }

    #[test]
    fn reexport_of_declaration_import_is_elided() {
        let index = PathBuf::from("/project/pkg/Index.ets");
        let aes = PathBuf::from("/project/pkg/src/main/ets/core/AES.ets");
        let declaration = PathBuf::from("/project/pkg/src/main/cpp/types/libmtcrypto/index.d.ts");
        let index_source =
            r#"export { AESUtil, EncryptionResult } from './src/main/ets/core/AES';"#;
        let aes_source = r#"import libmtcrypto from 'libmtcrypto.so';
import { EncryptionResult } from '../../cpp/types/libmtcrypto'

export class AESUtil {
    static encrypt(input: string): libmtcrypto.EncryptionResult {
        return libmtcrypto.AESEncrypt(input, "", "");
    }
}

export { EncryptionResult };
"#;

        let mut files = FxHashMap::default();
        files.insert(index.clone(), Arc::<str>::from(index_source));
        files.insert(aes, Arc::<str>::from(aes_source));
        files.insert(
            declaration,
            Arc::<str>::from(
                r#"export interface EncryptionResult {
    result: string;
    errorCode: number;
}
"#,
            ),
        );
        let sources = TransformSourceSnapshot::new(files);
        let index_id = index.to_str().unwrap();

        let split = split_type_only_reexports(index_source, index_id, &sources);
        assert!(
            split.contains("export { AESUtil } from \"./src/main/ets/core/AES\";"),
            "{split}"
        );
        assert!(
            split.contains("export type { EncryptionResult } from \"./src/main/ets/core/AES\";"),
            "{split}"
        );

        let emitted = TypeScriptJavascriptEmitter::new(&split, index_id, &sources)
            .emit()
            .expect("type-only reexport must be valid TypeScript");
        assert!(emitted.contains("AESUtil"));
        assert!(!emitted.contains("EncryptionResult"));
    }

    #[test]
    fn reexport_of_imported_source_interface_is_elided_through_chain() {
        let index = PathBuf::from("/project/pkg/Index.ets");
        let switch_test_env = PathBuf::from("/project/pkg/src/main/ets/switchTestEnv.ets");
        let dispatcher = PathBuf::from("/project/pkg/src/main/ets/dispatcher.ets");
        let callback = PathBuf::from("/project/pkg/src/main/ets/model/ResultCallBack.ets");
        let switch_source = r#"export { setSwitchTestEnvListener, removeSwitchTestEnvListener, switchTestEnvListener } from "./dispatcher";"#;
        let index_source = r#"export { switchTestEnvListener, setSwitchTestEnvListener, removeSwitchTestEnvListener } from './src/main/ets/switchTestEnv';"#;
        let dispatcher_source = r#"import { switchTestEnvListener } from "./model/ResultCallBack";

const setSwitchTestEnvListener = (source: string, listener: switchTestEnvListener): void => {};
const removeSwitchTestEnvListener = (source: string, listener: switchTestEnvListener): void => {};

export { setSwitchTestEnvListener, removeSwitchTestEnvListener, switchTestEnvListener };"#;

        let mut files = FxHashMap::default();
        files.insert(index.clone(), Arc::<str>::from(index_source));
        files.insert(switch_test_env.clone(), Arc::<str>::from(switch_source));
        files.insert(dispatcher.clone(), Arc::<str>::from(dispatcher_source));
        files.insert(
            callback,
            Arc::<str>::from(
                r#"export interface switchTestEnvListener {
    switchTestEnvFinish: (isOnlineUrl: boolean) => void;
}"#,
            ),
        );
        let sources = TransformSourceSnapshot::new(files);

        let dispatcher_split = split_type_only_named_imports(
            dispatcher_source,
            dispatcher.to_str().unwrap(),
            &sources,
        );
        assert!(
            dispatcher_split
                .contains("import type { switchTestEnvListener } from \"./model/ResultCallBack\";"),
            "{dispatcher_split}"
        );
        assert!(
            dispatcher_split
                .contains("export { setSwitchTestEnvListener, removeSwitchTestEnvListener };"),
            "{dispatcher_split}"
        );
        assert!(
            dispatcher_split.contains("export type { switchTestEnvListener };"),
            "{dispatcher_split}"
        );
        let dispatcher_emitted = TypeScriptJavascriptEmitter::new(
            &dispatcher_split,
            dispatcher.to_str().unwrap(),
            &sources,
        )
        .emit()
        .expect("type-only local export must be valid TypeScript");
        assert!(dispatcher_emitted.contains("setSwitchTestEnvListener"));
        assert!(!dispatcher_emitted.contains("switchTestEnvListener"));

        let switch_split =
            split_type_only_reexports(switch_source, switch_test_env.to_str().unwrap(), &sources);
        assert!(
            switch_split.contains(
                "export { setSwitchTestEnvListener, removeSwitchTestEnvListener } from \"./dispatcher\";"
            ),
            "{switch_split}"
        );
        assert!(
            switch_split.contains("export type { switchTestEnvListener } from \"./dispatcher\";"),
            "{switch_split}"
        );
        let switch_emitted = TypeScriptJavascriptEmitter::new(
            &switch_split,
            switch_test_env.to_str().unwrap(),
            &sources,
        )
        .emit()
        .expect("type-only reexport must be valid TypeScript");
        assert!(switch_emitted.contains("setSwitchTestEnvListener"));
        assert!(!switch_emitted.contains("switchTestEnvListener"));

        let index_split =
            split_type_only_reexports(index_source, index.to_str().unwrap(), &sources);
        assert!(
            index_split.contains(
                "export { setSwitchTestEnvListener, removeSwitchTestEnvListener } from \"./src/main/ets/switchTestEnv\";"
            ),
            "{index_split}"
        );
        assert!(
            index_split.contains(
                "export type { switchTestEnvListener } from \"./src/main/ets/switchTestEnv\";"
            ),
            "{index_split}"
        );
        let index_emitted =
            TypeScriptJavascriptEmitter::new(&index_split, index.to_str().unwrap(), &sources)
                .emit()
                .expect("type-only reexport must be valid TypeScript");
        assert!(index_emitted.contains("setSwitchTestEnvListener"));
        assert!(!index_emitted.contains("switchTestEnvListener"));
    }

    #[test]
    fn reexport_of_local_interface_export_list_is_elided_through_index() {
        let index = PathBuf::from("/project/pkg/index.ets");
        let http = PathBuf::from("/project/pkg/src/main/ets/form_utils/httpForNewline.ets");
        let http_source = r#"interface ShowData {
    title: string;
}

async function getCardDataAndFormat(): Promise<ShowData> {
    return { title: "" };
}

export {
    getCardDataAndFormat,
    ShowData
};"#;
        let index_source = r#"export {
    getCardDataAndFormat,
    ShowData
} from "./src/main/ets/form_utils/httpForNewline";"#;

        let mut files = FxHashMap::default();
        files.insert(index.clone(), Arc::<str>::from(index_source));
        files.insert(http.clone(), Arc::<str>::from(http_source));
        let sources = TransformSourceSnapshot::new(files);

        let http_split =
            split_type_only_named_imports(http_source, http.to_str().unwrap(), &sources);
        assert!(
            http_split.contains("export { getCardDataAndFormat };"),
            "{http_split}"
        );
        assert!(
            http_split.contains("export type { ShowData };"),
            "{http_split}"
        );
        let http_emitted =
            TypeScriptJavascriptEmitter::new(&http_split, http.to_str().unwrap(), &sources)
                .emit()
                .expect("local interface export list must be valid TypeScript");
        assert!(http_emitted.contains("getCardDataAndFormat"));
        assert!(!http_emitted.contains("ShowData"));

        let index_split =
            split_type_only_reexports(index_source, index.to_str().unwrap(), &sources);
        assert!(
            index_split.contains(
                "export { getCardDataAndFormat } from \"./src/main/ets/form_utils/httpForNewline\";"
            ),
            "{index_split}"
        );
        assert!(
            index_split.contains(
                "export type { ShowData } from \"./src/main/ets/form_utils/httpForNewline\";"
            ),
            "{index_split}"
        );
        let index_emitted =
            TypeScriptJavascriptEmitter::new(&index_split, index.to_str().unwrap(), &sources)
                .emit()
                .expect("type-only index reexport must be valid TypeScript");
        assert!(index_emitted.contains("getCardDataAndFormat"));
        assert!(!index_emitted.contains("ShowData"));
    }

    #[test]
    fn imported_source_interface_used_as_arkui_dsl_name_is_type_only() {
        let card_container =
            PathBuf::from("/project/pkg/src/main/ets/list/container/CardContainer.ets");
        let video_content =
            PathBuf::from("/project/pkg/src/main/ets/list/cardContent/VideoCardContent.ets");
        let card_container_source = r#"export interface Rect {
    x: number;
    y: number;
}

export interface CardContentController {
    onCardItemUpdated?: () => void;
}

export struct CardContainer {
    build() {}
}"#;
        let video_content_source = r#"import { CardContentController, Rect } from "../container/CardContainer";

@Component
export struct VideoCardContent {
    getCardContentController?: (controller?: CardContentController) => void;
    @Link playerVisibleRect: Rect;

    build() {
        Rect()
            .width("100%");
    }
}"#;

        let mut files = FxHashMap::default();
        files.insert(card_container, Arc::<str>::from(card_container_source));
        files.insert(
            video_content.clone(),
            Arc::<str>::from(video_content_source),
        );
        let sources = TransformSourceSnapshot::new(files);
        let split = split_type_only_named_imports(
            video_content_source,
            video_content.to_str().unwrap(),
            &sources,
        );
        assert!(
            split.contains(
                "import type { CardContentController, Rect } from \"../container/CardContainer\";"
            ),
            "{split}"
        );
        assert!(
            !split.contains("import { CardContentController, Rect } from"),
            "{split}"
        );
    }

    #[test]
    fn imported_source_value_type_merged_binding_keeps_runtime_import() {
        let event = PathBuf::from("/project/pkg/src/main/ets/core/IncomingEvent.ts");
        let vector = PathBuf::from("/project/pkg/src/main/ets/core/Vector2D.ts");
        let event_source = r#"const LEFT = 2;
const RIGHT = 1;

export const Directions = {
    LEFT,
    RIGHT,
} as const;

export type Directions = typeof Directions[keyof typeof Directions];"#;
        let vector_source = r#"import { Directions } from "./IncomingEvent";

export class Vector2D {
    private static VECTOR_BY_DIRECTION = new Map<Directions, Vector2D>([
        [Directions.LEFT, new Vector2D()],
    ]);
}"#;

        let mut files = FxHashMap::default();
        files.insert(event, Arc::<str>::from(event_source));
        files.insert(vector.clone(), Arc::<str>::from(vector_source));
        let sources = TransformSourceSnapshot::new(files);
        let split =
            split_type_only_named_imports(vector_source, vector.to_str().unwrap(), &sources);
        assert!(
            split.contains("import { Directions } from \"./IncomingEvent\";"),
            "{split}"
        );
        assert!(
            !split.contains("import type { Directions } from \"./IncomingEvent\";"),
            "{split}"
        );
    }

    #[test]
    fn local_export_of_unexported_declaration_interface_is_elided() {
        let index = PathBuf::from("/project/pkg/Index.ets");
        let declaration =
            PathBuf::from("/project/pkg/src/main/ets/mrouter/types/Interceptor.d.ets");
        let index_source = r#"import { InterruptConfig } from './src/main/ets/mrouter/types/Interceptor';

export { InterruptConfig };"#;

        let mut files = FxHashMap::default();
        files.insert(index.clone(), Arc::<str>::from(index_source));
        files.insert(
            declaration,
            Arc::<str>::from(
                r#"interface InterruptConfig {
    redirectToUri?: string;
}"#,
            ),
        );
        let sources = TransformSourceSnapshot::new(files);
        let index_id = index.to_str().unwrap();

        let split = split_type_only_named_imports(index_source, index_id, &sources);
        assert!(
            split.contains(
                "import type { InterruptConfig } from \"./src/main/ets/mrouter/types/Interceptor\";"
            ),
            "{split}"
        );
        assert!(
            split.contains("export type { InterruptConfig };"),
            "{split}"
        );
        let emitted = TypeScriptJavascriptEmitter::new(&split, index_id, &sources)
            .emit()
            .expect("type-only local export must be valid TypeScript");
        assert!(!emitted.contains("InterruptConfig"));
        assert!(!emitted.contains("types/Interceptor"));
    }

    #[test]
    fn reexport_from_explicit_d_specifier_is_elided() {
        let index = PathBuf::from("/project/pkg/Index.ets");
        let declaration = PathBuf::from("/project/pkg/src/main/ets/types/Index.d.ets");
        let index_source = r#"export { QueryRerouteConfigInfo, RerouteOptions } from './src/main/ets/types/Index.d';"#;

        let mut files = FxHashMap::default();
        files.insert(index.clone(), Arc::<str>::from(index_source));
        files.insert(
            declaration,
            Arc::<str>::from(
                r#"export interface QueryRerouteConfigInfo {
    srcPattern: string;
}
export interface RerouteOptions {
    hornKey: string;
}"#,
            ),
        );
        let sources = TransformSourceSnapshot::new(files);
        let index_id = index.to_str().unwrap();

        let split = split_type_only_reexports(index_source, index_id, &sources);
        assert!(
            split.contains(
                "export type { QueryRerouteConfigInfo, RerouteOptions } from \"./src/main/ets/types/Index.d\";"
            ),
            "{split}"
        );
        let emitted = TypeScriptJavascriptEmitter::new(&split, index_id, &sources)
            .emit()
            .expect("type-only reexport must be valid TypeScript");
        assert!(!emitted.contains("QueryRerouteConfigInfo"));
        assert!(!emitted.contains("RerouteOptions"));
        assert!(!emitted.contains("types/Index.d"));
    }

    #[test]
    fn reexport_of_type_from_export_all_chain_is_elided() {
        let entry = PathBuf::from("/project/pkg/ets.ets");
        let barrel = PathBuf::from("/project/pkg/src/main/ets/RNOH/ets.ets");
        let coordinator =
            PathBuf::from("/project/pkg/src/main/ets/RNOH/RNInstancesCoordinator.ets");
        let entry_source = r#"export {
    RNInstancesCoordinator,
    RNInstancesCoordinatorOptions
} from "./src/main/ets/RNOH/ets";"#;
        let barrel_source = r#"export * from "./RNInstancesCoordinator";"#;
        let coordinator_source = r#"export interface RNInstancesCoordinatorOptions {
    initialProps?: object;
}
export class RNInstancesCoordinator {}"#;

        let mut files = FxHashMap::default();
        files.insert(entry.clone(), Arc::<str>::from(entry_source));
        files.insert(barrel, Arc::<str>::from(barrel_source));
        files.insert(coordinator, Arc::<str>::from(coordinator_source));
        let sources = TransformSourceSnapshot::new(files);
        let entry_id = entry.to_str().unwrap();

        let split = split_type_only_reexports(entry_source, entry_id, &sources);
        assert!(
            split.contains("export { RNInstancesCoordinator } from \"./src/main/ets/RNOH/ets\";"),
            "{split}"
        );
        assert!(
            split.contains(
                "export type { RNInstancesCoordinatorOptions } from \"./src/main/ets/RNOH/ets\";"
            ),
            "{split}"
        );
        let emitted = TypeScriptJavascriptEmitter::new(&split, entry_id, &sources)
            .emit()
            .expect("type-only reexport must be valid TypeScript");
        assert!(emitted.contains("RNInstancesCoordinator"));
        assert!(!emitted.contains("RNInstancesCoordinatorOptions"));
    }

    #[test]
    fn reexport_of_interface_namespace_merge_keeps_runtime_namespace() {
        let index = PathBuf::from("/project/pkg/src/main/ets/index_h.ets");
        let data_center =
            PathBuf::from("/project/pkg/src/main/ets/data/met-address-data-center_h.ets");
        let index_source =
            r#"export { AddressDataCenter } from './data/met-address-data-center_h';"#;
        let data_center_source = r#"export interface AddressDataCenter {
    value: string;
}

export namespace AddressDataCenter {
    export function getInstance(): AddressDataCenter {
        return { value: "ok" };
    }
}"#;

        let mut files = FxHashMap::default();
        files.insert(index.clone(), Arc::<str>::from(index_source));
        files.insert(data_center, Arc::<str>::from(data_center_source));
        let sources = TransformSourceSnapshot::new(files);
        let index_id = index.to_str().unwrap();

        let split = split_type_only_reexports(index_source, index_id, &sources);
        assert_eq!(split, index_source);

        let emitted = TypeScriptJavascriptEmitter::new(&split, index_id, &sources)
            .emit()
            .expect("runtime namespace reexport must be valid TypeScript");
        assert!(
            emitted.contains("AddressDataCenter"),
            "runtime namespace reexport must survive JS emission:\n{emitted}"
        );
    }

    #[test]
    fn local_export_of_default_interface_import_is_elided() {
        let unread = PathBuf::from("/project/pkg/src/main/ets/apis/unread/index.ets");
        let models = PathBuf::from("/project/pkg/src/main/ets/interfaces/MessageCenterModels.ets");
        let unread_source = r#"import UnreadResponseData from '../../interfaces/MessageCenterModels';
import { captureService } from '../utils';

export async function requestUnread(): Promise<UnreadResponseData> {
    return captureService<UnreadResponseData>({});
}

export { UnreadResponseData }"#;
        let models_source = r#"export default interface MessageCenterModel {
    unread_count?: number;
}"#;

        let mut files = FxHashMap::default();
        files.insert(unread.clone(), Arc::<str>::from(unread_source));
        files.insert(models, Arc::<str>::from(models_source));
        let sources = TransformSourceSnapshot::new(files);
        let unread_id = unread.to_str().unwrap();

        let split = split_type_only_named_imports(unread_source, unread_id, &sources);
        assert!(
            split.contains(
                "import type UnreadResponseData from \"../../interfaces/MessageCenterModels\";"
            ),
            "{split}"
        );
        assert!(
            split.contains("export type { UnreadResponseData };"),
            "{split}"
        );

        let emitted = TypeScriptJavascriptEmitter::new(&split, unread_id, &sources)
            .emit()
            .expect("default interface import/export must be valid TypeScript");
        assert!(emitted.contains("requestUnread"));
        assert!(!emitted.contains("UnreadResponseData"));
        assert!(!emitted.contains("MessageCenterModels"));
    }

    #[test]
    fn commonjs_default_absent_from_declaration_stays_a_runtime_import() {
        let index = PathBuf::from("/project/pkg/index.ts");
        let runtime = PathBuf::from("/project/pkg/src/main/js/index.js");
        let declaration = PathBuf::from("/project/pkg/src/main/js/index.d.ts");
        let index_source = r#"import jsbn from "./src/main/js/index";

export default jsbn;
export { jsbn };"#;
        let mut files = FxHashMap::default();
        files.insert(index.clone(), Arc::<str>::from(index_source));
        files.insert(
            runtime,
            Arc::<str>::from("module.exports = { default: BigInteger };"),
        );
        files.insert(declaration, Arc::<str>::from("export class BigInteger {}"));
        let sources = TransformSourceSnapshot::new(files);

        let split = split_type_only_named_imports(index_source, index.to_str().unwrap(), &sources);
        assert_eq!(split, index_source);
    }

    #[test]
    fn decorator_reference_keeps_its_runtime_import() {
        let config = PathBuf::from("/project/pkg/Config.ets");
        let annotations = PathBuf::from("/project/pkg/annotations.ets");
        let config_source = r#"import { ImmediateUpdateField } from "./annotations";

export class Config {
    @ImmediateUpdateField
    enabled: boolean = false;
}"#;
        let mut files = FxHashMap::default();
        files.insert(config.clone(), Arc::<str>::from(config_source));
        files.insert(
            annotations,
            Arc::<str>::from("export function ImmediateUpdateField() {}"),
        );
        let sources = TransformSourceSnapshot::new(files);

        let split =
            split_type_only_named_imports(config_source, config.to_str().unwrap(), &sources);
        assert_eq!(split, config_source);
    }

    #[test]
    fn default_import_used_only_for_default_export_becomes_live_reexport() {
        let index = PathBuf::from("/project/pkg/Index.ets");
        let index_source = r#"import LX from './src/main/ets/LX';
/**
 * 灵犀
 */
export default LX;
export { generatePageInfoKey } from './src/main/ets/utils/Utils';"#;
        let sources = TransformSourceSnapshot::new(FxHashMap::default());
        let index_id = index.to_str().unwrap();

        let rewritten = rewrite_default_import_reexport(index_source, index_id);
        assert!(
            rewritten.contains("export { default } from \"./src/main/ets/LX\";"),
            "{rewritten}"
        );
        assert!(!rewritten.contains("import LX from"), "{rewritten}");
        assert!(!rewritten.contains("export default LX;"), "{rewritten}");
        assert!(
            rewritten
                .contains("export { generatePageInfoKey } from \"./src/main/ets/utils/Utils\";"),
            "{rewritten}"
        );

        let emitted = TypeScriptJavascriptEmitter::new(&rewritten, index_id, &sources)
            .emit()
            .expect("live default reexport must be valid TypeScript");
        assert!(emitted.contains("export { default } from \"./src/main/ets/LX\";"));
        assert!(!emitted.contains("import LX from"));
    }

    #[test]
    fn default_import_with_other_runtime_use_is_not_rewritten() {
        let index = PathBuf::from("/project/pkg/Index.ets");
        let index_source = r#"import LX from './src/main/ets/LX';

export const init = LX.init;
export default LX;"#;
        let index_id = index.to_str().unwrap();

        let rewritten = rewrite_default_import_reexport(index_source, index_id);
        assert_eq!(rewritten, index_source);
    }

    #[test]
    fn mixed_default_and_named_type_only_imports_emit_separate_declarations() {
        let page = PathBuf::from("/project/pkg/src/main/ets/Page.ets");
        let models = PathBuf::from("/project/pkg/src/main/ets/models.ets");
        let page_source = r#"import Model, { ModelOptions } from './models';

export interface PageProps {
    model: Model;
    options: ModelOptions;
}"#;
        let models_source = r#"export default interface Model {
    name: string;
}

export interface ModelOptions {
    size: number;
}"#;

        let mut files = FxHashMap::default();
        files.insert(page.clone(), Arc::<str>::from(page_source));
        files.insert(models, Arc::<str>::from(models_source));
        let sources = TransformSourceSnapshot::new(files);
        let page_id = page.to_str().unwrap();

        let split = split_type_only_named_imports(page_source, page_id, &sources);
        assert!(
            split.contains("import type Model from \"./models\";"),
            "{split}"
        );
        assert!(
            split.contains("import type { ModelOptions } from \"./models\";"),
            "{split}"
        );
        assert!(
            !split.contains("import type Model,"),
            "ArkTS rejects mixed default/named type import:\n{split}"
        );
    }

    #[test]
    fn reexport_of_default_alias_to_local_interface_is_elided() {
        let index = PathBuf::from("/project/pkg/src/main/ets/index.ets");
        let models = PathBuf::from("/project/pkg/src/main/ets/models.ets");
        let index_source = r#"export { default as Model } from "./models";"#;
        let models_source = r#"interface Model {
    value: string;
}

export { Model as default };"#;

        let mut files = FxHashMap::default();
        files.insert(index.clone(), Arc::<str>::from(index_source));
        files.insert(models, Arc::<str>::from(models_source));
        let sources = TransformSourceSnapshot::new(files);
        let index_id = index.to_str().unwrap();

        let split = split_type_only_reexports(index_source, index_id, &sources);
        assert!(
            split.contains("export type { default as Model } from \"./models\";"),
            "{split}"
        );
        let emitted = TypeScriptJavascriptEmitter::new(&split, index_id, &sources)
            .emit()
            .expect("default type alias reexport must be valid TypeScript");
        assert!(!emitted.contains("Model"));
        assert!(!emitted.contains("./models"));
    }

    #[test]
    fn reexport_from_type_export_all_chain_is_elided() {
        let index = PathBuf::from("/project/pkg/src/main/ets/index.ets");
        let barrel = PathBuf::from("/project/pkg/src/main/ets/barrel.ets");
        let models = PathBuf::from("/project/pkg/src/main/ets/models.ets");
        let index_source = r#"export { Model } from "./barrel";"#;
        let barrel_source = r#"export type * from "./models";"#;
        let models_source = r#"export interface Model {
    value: string;
}

export const RuntimeModel = {
    value: "runtime",
};"#;

        let mut files = FxHashMap::default();
        files.insert(index.clone(), Arc::<str>::from(index_source));
        files.insert(barrel, Arc::<str>::from(barrel_source));
        files.insert(models, Arc::<str>::from(models_source));
        let sources = TransformSourceSnapshot::new(files);
        let index_id = index.to_str().unwrap();

        let split = split_type_only_reexports(index_source, index_id, &sources);
        assert!(
            split.contains("export type { Model } from \"./barrel\";"),
            "{split}"
        );
        let emitted = TypeScriptJavascriptEmitter::new(&split, index_id, &sources)
            .emit()
            .expect("type export-all chain must be valid TypeScript");
        assert!(!emitted.contains("Model"));
        assert!(!emitted.contains("./barrel"));
    }

    #[test]
    fn reexport_from_export_all_namespace_chain_keeps_runtime_namespace() {
        let index = PathBuf::from("/project/pkg/src/main/ets/index.ets");
        let barrel = PathBuf::from("/project/pkg/src/main/ets/barrel.ets");
        let models = PathBuf::from("/project/pkg/src/main/ets/models.ets");
        let index_source = r#"export { ModelTypes } from "./barrel";"#;
        let barrel_source = r#"export * as ModelTypes from "./models";"#;
        let models_source = r#"export interface Model {
    value: string;
}"#;

        let mut files = FxHashMap::default();
        files.insert(index.clone(), Arc::<str>::from(index_source));
        files.insert(barrel, Arc::<str>::from(barrel_source));
        files.insert(models, Arc::<str>::from(models_source));
        let sources = TransformSourceSnapshot::new(files);
        let index_id = index.to_str().unwrap();

        let split = split_type_only_reexports(index_source, index_id, &sources);
        assert_eq!(split, index_source);
    }

    #[test]
    fn package_reexport_of_type_from_export_all_chain_is_elided() {
        let core_index = PathBuf::from(
            "/project/oh_modules/.ohpm/@knb+core@hash/oh_modules/@knb/core/Index.ets",
        );
        let protocol_index = PathBuf::from(
            "/project/oh_modules/.ohpm/@knb+protocol@hash/oh_modules/@knb/protocol/Index.ets",
        );
        let init_config = PathBuf::from(
            "/project/oh_modules/.ohpm/@knb+protocol@hash/oh_modules/@knb/protocol/src/main/ets/IKNBInitConfig.ets",
        );
        let core_source = r#"export { IKNBInitConfig, NetworkType } from "@knb/protocol";"#;
        let protocol_source = r#"export * from "./src/main/ets/IKNBInitConfig";
export enum NetworkType {
    WIFI = "wifi"
}"#;
        let init_config_source = r#"export interface IKNBInitConfig {
    appId: string;
}"#;

        let mut files = FxHashMap::default();
        files.insert(core_index.clone(), Arc::<str>::from(core_source));
        files.insert(protocol_index, Arc::<str>::from(protocol_source));
        files.insert(init_config, Arc::<str>::from(init_config_source));
        let sources = TransformSourceSnapshot::new(files);
        let core_id = core_index.to_str().unwrap();

        let split = split_type_only_reexports(core_source, core_id, &sources);
        assert!(
            split.contains("export { NetworkType } from \"@knb/protocol\";"),
            "{split}"
        );
        assert!(
            split.contains("export type { IKNBInitConfig } from \"@knb/protocol\";"),
            "{split}"
        );
        let emitted = TypeScriptJavascriptEmitter::new(&split, core_id, &sources)
            .emit()
            .expect("type-only package reexport must be valid TypeScript");
        assert!(emitted.contains("NetworkType"));
        assert!(!emitted.contains("IKNBInitConfig"));
    }

    #[test]
    fn package_reexport_from_native_declaration_splits_const_enum_type() {
        let index = PathBuf::from("/project/oh_modules/@scope/widget/Index.ets");
        let native_index = PathBuf::from(
            "/project/oh_modules/@scope/widget/native-declarations/custom-entry/Entry.d.ts",
        );
        let native_types = PathBuf::from(
            "/project/oh_modules/@scope/widget/native-declarations/custom-entry/Types.d.ts",
        );
        let index_source = r#"export { EdgeType, nativeCall } from "libwidget.so";"#;
        let native_index_source = r#"export * from "./Types";
export const nativeCall: () => void;"#;
        let native_types_source = r#"export const enum EdgeType {
    TOP = 1
}"#;

        let mut files = FxHashMap::default();
        files.insert(index.clone(), Arc::<str>::from(index_source));
        files.insert(native_index.clone(), Arc::<str>::from(native_index_source));
        files.insert(native_types, Arc::<str>::from(native_types_source));
        let sources = TransformSourceSnapshot::with_resolved_static_modules(
            files,
            vec![crate::source::TransformResolvedStaticModule::new(
                "/project/oh_modules/@scope/widget",
                "libwidget.so",
                &native_index,
            )],
        );
        let index_id = index.to_str().unwrap();

        let split = split_type_only_reexports(index_source, index_id, &sources);

        assert!(
            split.contains("export { nativeCall } from \"libwidget.so\";"),
            "{split}"
        );
        assert!(
            split.contains("export type { EdgeType } from \"libwidget.so\";"),
            "{split}"
        );
        let emitted = TypeScriptJavascriptEmitter::new(&split, index_id, &sources)
            .emit()
            .expect("native declaration type reexport must be valid TypeScript");
        assert!(emitted.contains("nativeCall"));
        assert!(!emitted.contains("EdgeType"));
    }

    #[test]
    fn declared_runtime_functions_keep_value_imports_like_hvigor() {
        let sign_utils =
            PathBuf::from("/project/oh_modules/@paydebugkit/base/src/main/ets/SignUtils.ts");
        let utils = PathBuf::from(
            "/project/oh_modules/@cashier_alipay/cashiersdk/src/main/ets/utils/Utils.ts",
        );
        let sign_source = r#"import { stringToUint8ArrayWithEncoder, uint8ArrayToStringWithDecoder } from "@cashier_alipay/cashiersdk/src/main/ets/utils/Utils";

export function encode(value: string): string {
    return uint8ArrayToStringWithDecoder(stringToUint8ArrayWithEncoder(value));
}"#;
        let utils_source = r#"export declare function stringToUint8ArrayWithEncoder(value: string): Uint8Array;
export declare function uint8ArrayToStringWithDecoder(value: Uint8Array): string;

export function stringToUint8ArrayWithEncoder(value: string): Uint8Array {
    return new Uint8Array();
}

export function uint8ArrayToStringWithDecoder(value: Uint8Array): string {
    return "";
}"#;

        let mut files = FxHashMap::default();
        files.insert(sign_utils.clone(), Arc::<str>::from(sign_source));
        files.insert(utils, Arc::<str>::from(utils_source));
        let sources = TransformSourceSnapshot::new(files);
        let sign_id = sign_utils.to_str().unwrap();

        for analyzer in available_module_analyzers() {
            let split = split_type_only_named_imports_with_analyzer(
                sign_source,
                sign_id,
                &sources,
                analyzer,
            );
            assert!(
                split.contains(
                    "import { stringToUint8ArrayWithEncoder, uint8ArrayToStringWithDecoder }"
                ),
                "{analyzer:?} must keep declared runtime functions as value imports:\n{split}"
            );
            assert!(
                !split.contains("import type { stringToUint8ArrayWithEncoder"),
                "{analyzer:?} must not convert runtime functions to type-only imports:\n{split}"
            );
        }
    }

    #[test]
    fn native_declaration_runtime_bindings_match_hvigor_type_split() {
        let index = PathBuf::from("/project/oh_modules/@msi/jsi/src/main/ets/index.ts");
        let native_index =
            PathBuf::from("/project/oh_modules/@msi/jsi/src/main/cpp/types/libmsi/Index.d.ts");
        let index_source = r#"import { ArkTSMSIModule as ArkTSMSIBridge, setMetadataInfo as msiSetMetadataInfo, recordMSIMoniter as msiRecordMSIMoniter, Options } from "libmsi.so";

const bridge = new ArkTSMSIBridge({} as Options);
msiSetMetadataInfo([]);
msiRecordMSIMoniter("");
export { bridge };"#;
        let native_source = r#"export interface Options {
    jsInvoker: Object;
}

export declare class ArkTSMSIModule {
    constructor(option: Options);
}

export declare function setMetadataInfo(metadataInfo: string[]): void;
export declare function recordMSIMoniter(pointer: string): void;"#;

        let mut files = FxHashMap::default();
        files.insert(index.clone(), Arc::<str>::from(index_source));
        files.insert(native_index.clone(), Arc::<str>::from(native_source));
        let sources = TransformSourceSnapshot::with_resolved_static_modules(
            files,
            vec![crate::source::TransformResolvedStaticModule::new(
                "/project/oh_modules/@msi/jsi",
                "libmsi.so",
                &native_index,
            )],
        );
        let index_id = index.to_str().unwrap();

        for analyzer in available_module_analyzers() {
            let split = split_type_only_named_imports_with_analyzer(
                index_source,
                index_id,
                &sources,
                analyzer,
            );
            assert!(
                split.contains("import { ArkTSMSIModule as ArkTSMSIBridge, setMetadataInfo as msiSetMetadataInfo, recordMSIMoniter as msiRecordMSIMoniter } from \"libmsi.so\";"),
                "{analyzer:?} must keep native runtime class/functions as value imports:\n{split}"
            );
            assert!(
                split.contains("import type { Options } from \"libmsi.so\";"),
                "{analyzer:?} must split native interface as type import:\n{split}"
            );
        }
    }

    #[test]
    fn native_declaration_reexport_keeps_runtime_bindings_like_hvigor() {
        let index = PathBuf::from("/project/oh_modules/@scope/widget/Index.ets");
        let native_index = PathBuf::from(
            "/project/oh_modules/@scope/widget/native-declarations/custom-entry/Entry.d.ts",
        );
        let index_source = r#"export { EdgeType, NativeWidget, nativeCall } from "libwidget.so";"#;
        let native_source = r#"export const enum EdgeType {
    TOP = 1
}

export declare class NativeWidget {}
export declare function nativeCall(): void;"#;

        let mut files = FxHashMap::default();
        files.insert(index.clone(), Arc::<str>::from(index_source));
        files.insert(native_index.clone(), Arc::<str>::from(native_source));
        let sources = TransformSourceSnapshot::with_resolved_static_modules(
            files,
            vec![crate::source::TransformResolvedStaticModule::new(
                "/project/oh_modules/@scope/widget",
                "libwidget.so",
                &native_index,
            )],
        );
        let index_id = index.to_str().unwrap();

        for analyzer in available_module_analyzers() {
            let split =
                split_type_only_reexports_with_analyzer(index_source, index_id, &sources, analyzer);
            assert!(
                split.contains("export { NativeWidget, nativeCall } from \"libwidget.so\";"),
                "{analyzer:?} must keep native runtime reexports live:\n{split}"
            );
            assert!(
                split.contains("export type { EdgeType } from \"libwidget.so\";"),
                "{analyzer:?} must keep const enum reexport type-only:\n{split}"
            );
        }
    }

    #[test]
    fn native_default_import_without_declaration_default_stays_runtime_value() {
        let index = PathBuf::from("/project/oh_modules/@scope/logan/Index.ets");
        let native_index =
            PathBuf::from("/project/oh_modules/@scope/logan/native-types/index.d.ts");
        let index_source = r#"import loganNapi from "liblogan.so";

export const logan = loganNapi;"#;
        let native_source = r#"export const loganLog: (type: number, log: string) => number;"#;

        let mut files = FxHashMap::default();
        files.insert(index.clone(), Arc::<str>::from(index_source));
        files.insert(native_index.clone(), Arc::<str>::from(native_source));
        let sources = TransformSourceSnapshot::with_resolved_static_modules(
            files,
            vec![crate::source::TransformResolvedStaticModule::new(
                "/project/oh_modules/@scope/logan",
                "liblogan.so",
                &native_index,
            )],
        );
        let index_id = index.to_str().unwrap();

        let split = split_type_only_named_imports(index_source, index_id, &sources);

        assert!(
            split.contains("import loganNapi from \"liblogan.so\";"),
            "{split}"
        );
        assert!(split.contains("export const logan = loganNapi;"), "{split}");
        let emitted = TypeScriptJavascriptEmitter::new(&split, index_id, &sources)
            .emit()
            .expect("native default import must be valid TypeScript");
        assert!(emitted.contains("loganNapi"), "{emitted}");
    }
}
