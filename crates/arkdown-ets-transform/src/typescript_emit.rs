//! TypeScript source helpers for CompileArkTS cache parity.
//!
//! hvigor ESMODULE/CompileArkTS writes transformed TypeScript cache files and
//! passes them to `es2abc` through `filesInfo.txt`/`modules.cache`. Production
//! code keeps that TypeScript shape except for files carrying TypeScript's
//! `@ts-ignore`/`@ts-nocheck` pragmas. SDK `process_kit_import.js` records those
//! files and `module_mode.js` emits JavaScript for them. The OXC emitter below
//! implements that explicit production boundary.

use rustc_hash::{FxHashMap, FxHashSet};
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::sync::RwLock;

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Declaration, ExportAllDeclaration, ExportNamedDeclaration, Expression, ImportDeclaration,
    ImportDeclarationSpecifier, Statement, TSModuleDeclarationBody, TSModuleDeclarationName,
};
use oxc_ast::AstBuilder;
use oxc_ast_visit::{walk_mut, VisitMut};
use oxc_codegen::Codegen;
use oxc_codegen::CodegenOptions;
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{SourceType, SPAN};
use oxc_syntax::{number::NumberBase, operator::UnaryOperator};
use oxc_transformer::{TransformOptions, Transformer};

use crate::source::{TransformSourceFile, TransformSourceSnapshot};
use crate::ModuleExportNameText;

#[derive(Debug, Clone, PartialEq)]
enum EnumValue {
    Number(f64),
    String(String),
}

#[derive(Debug, Clone, Default)]
struct ConstEnumExports {
    enums: FxHashMap<String, FxHashMap<String, EnumValue>>,
    namespaces: FxHashMap<String, FxHashMap<String, FxHashMap<String, EnumValue>>>,
}

#[derive(Debug, Clone)]
enum ConstEnumImport {
    Enum(FxHashMap<String, EnumValue>),
    Namespace(FxHashMap<String, FxHashMap<String, EnumValue>>),
}

pub(crate) struct TypeScriptJavascriptEmitter<'a> {
    source: &'a str,
    id: &'a str,
    sources: &'a TransformSourceSnapshot,
    #[cfg(test)]
    const_enum_exports: Option<&'a ConstEnumExportCache>,
}

impl<'a> TypeScriptJavascriptEmitter<'a> {
    pub(crate) fn new(source: &'a str, id: &'a str, sources: &'a TransformSourceSnapshot) -> Self {
        Self {
            source,
            id,
            sources,
            #[cfg(test)]
            const_enum_exports: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_const_enum_export_cache(mut self, cache: &'a ConstEnumExportCache) -> Self {
        self.const_enum_exports = Some(cache);
        self
    }

    pub(crate) fn emit(&self) -> Option<String> {
        let allocator = Allocator::default();
        let source_type = self.source_type();
        let mut parsed = Parser::new(&allocator, self.source, source_type).parse();
        if !parsed.diagnostics.is_empty() {
            return None;
        }

        let rewriter = ImportedConstEnumRewriter::new(self.id, self.sources);
        #[cfg(test)]
        let rewriter = match self.const_enum_exports {
            Some(cache) => rewriter.with_const_enum_export_cache(cache),
            None => rewriter,
        };
        rewriter.rewrite_program(&allocator, &mut parsed.program);

        let scoping = {
            let semantic_ret = SemanticBuilder::new()
                .with_enum_eval(true)
                .build(&parsed.program);
            if !semantic_ret.diagnostics.is_empty() {
                return None;
            }
            semantic_ret.semantic.into_scoping()
        };

        let mut options = TransformOptions::default();
        options.assumptions.set_public_class_fields = true;
        options.typescript.optimize_const_enums = true;
        options.typescript.remove_class_fields_without_initializer = true;
        let path = Path::new(self.id);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Transformer::new(&allocator, path, &options)
                .build_with_scoping(scoping, &mut parsed.program)
        }));
        if result.is_err() {
            return None;
        }

        Some(
            Codegen::new()
                .with_options(CodegenOptions::default())
                .build(&parsed.program)
                .code,
        )
    }

    fn source_type(&self) -> SourceType {
        SourceType::from_path(self.id).unwrap_or_else(|_| SourceType::ts().with_module(true))
    }
}

/// Parsed TypeScript pragma facts used by the CompileArkTS cache writer.
///
/// This deliberately reads OXC's comment AST instead of searching arbitrary
/// source strings, so string literals and unrelated comments cannot change a
/// module's output language.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TypeScriptDirectiveFlags {
    suppress_type_check: bool,
    keep_typescript: bool,
    shared_module: bool,
}

impl TypeScriptDirectiveFlags {
    pub fn from_source(source: &str, id: &str) -> Self {
        let allocator = Allocator::default();
        let source_type =
            SourceType::from_path(id).unwrap_or_else(|_| SourceType::ts().with_module(true));
        let parsed = Parser::new(&allocator, source, source_type).parse();
        Self::from_program(&parsed.program)
    }

    pub fn should_emit_javascript(self) -> bool {
        self.suppress_type_check && !self.keep_typescript
    }

    /// Match SDK `check_shared_module.ts::collectSharedModule`: imports may
    /// precede `"use shared"`, but any other statement ends directive lookup.
    pub fn is_shared_module(self) -> bool {
        self.shared_module
    }

    fn from_program(program: &oxc_ast::ast::Program<'_>) -> Self {
        let mut flags = Self::default();
        for comment in &program.comments {
            let span = comment.content_span();
            let Some(content) = program
                .source_text
                .get(span.start as usize..span.end as usize)
            else {
                continue;
            };
            let directive = TypeScriptCommentDirective::new(content, comment.is_line());
            flags.suppress_type_check |= directive.suppresses_type_check();
            flags.keep_typescript |= comment.is_line()
                && program
                    .source_text
                    .get(..comment.span.start as usize)
                    .is_some_and(|prefix| prefix.trim().is_empty())
                && directive.is_keep_typescript();
        }
        flags.shared_module = program
            .body
            .iter()
            .find(|statement| !matches!(statement, oxc_ast::ast::Statement::ImportDeclaration(_)))
            .is_some_and(|statement| {
                let oxc_ast::ast::Statement::ExpressionStatement(statement) = statement else {
                    return false;
                };
                matches!(
                    &statement.expression,
                    oxc_ast::ast::Expression::StringLiteral(literal)
                        if literal.value == "use shared"
                )
            });
        flags
    }
}

struct TypeScriptCommentDirective<'a> {
    final_line: &'a str,
    line_comment: bool,
}

impl<'a> TypeScriptCommentDirective<'a> {
    fn new(content: &'a str, line_comment: bool) -> Self {
        Self {
            final_line: content.rsplit('\n').next().unwrap_or(content).trim_start(),
            line_comment,
        }
    }

    fn suppresses_type_check(&self) -> bool {
        self.directive("@ts-ignore") || (self.line_comment && self.directive("@ts-nocheck"))
    }

    fn is_keep_typescript(&self) -> bool {
        self.line_comment && self.final_line.trim() == "@keepTs"
    }

    fn directive(&self, name: &str) -> bool {
        let line = self
            .final_line
            .trim_start_matches(|character: char| character == '*' || character.is_whitespace());
        let Some(rest) = line.strip_prefix(name) else {
            return false;
        };
        rest.is_empty()
            || rest
                .chars()
                .next()
                .is_some_and(|character| character == ':' || character.is_whitespace())
    }
}

pub(crate) struct ImportedConstEnumRewriter<'a> {
    id: &'a str,
    sources: &'a TransformSourceSnapshot,
    #[cfg(test)]
    const_enum_exports: Option<&'a ConstEnumExportCache>,
}

impl<'a> ImportedConstEnumRewriter<'a> {
    pub(crate) fn new(id: &'a str, sources: &'a TransformSourceSnapshot) -> Self {
        Self {
            id,
            sources,
            #[cfg(test)]
            const_enum_exports: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_const_enum_export_cache(mut self, cache: &'a ConstEnumExportCache) -> Self {
        self.const_enum_exports = Some(cache);
        self
    }

    pub(crate) fn rewrite_program<'ast>(
        &self,
        allocator: &'ast Allocator,
        program: &mut oxc_ast::ast::Program<'ast>,
    ) -> bool {
        let imports = {
            let collector = ConstEnumImportCollector::new(&program.body, self.id, self.sources);
            #[cfg(test)]
            let collector = collector.with_const_enum_export_cache_opt(self.const_enum_exports);
            collector.collect()
        };
        if imports.is_empty() {
            return false;
        }

        let mut visitor = ConstEnumInlineMutator {
            ast: AstBuilder::new(allocator),
            imports,
            changed: false,
        };
        visitor.visit_program(program);
        visitor.changed
    }
}

struct ConstEnumImportCollector<'a> {
    statements: &'a [Statement<'a>],
    id: &'a str,
    sources: &'a TransformSourceSnapshot,
    #[cfg(test)]
    const_enum_exports: Option<&'a ConstEnumExportCache>,
}

impl<'a> ConstEnumImportCollector<'a> {
    fn new(
        statements: &'a [Statement<'a>],
        id: &'a str,
        sources: &'a TransformSourceSnapshot,
    ) -> Self {
        Self {
            statements,
            id,
            sources,
            #[cfg(test)]
            const_enum_exports: None,
        }
    }

    #[cfg(test)]
    fn with_const_enum_export_cache_opt(mut self, cache: Option<&'a ConstEnumExportCache>) -> Self {
        self.const_enum_exports = cache;
        self
    }

    fn collect(&self) -> FxHashMap<String, ConstEnumImport> {
        let mut imports = FxHashMap::default();
        for statement in self.statements {
            let Statement::ImportDeclaration(import) = statement else {
                continue;
            };
            self.collect_import(import, &mut imports);
        }
        imports
    }

    fn collect_import(
        &self,
        import: &ImportDeclaration<'_>,
        imports: &mut FxHashMap<String, ConstEnumImport>,
    ) {
        if import.import_kind.is_type() {
            return;
        }
        let Some(target) = self
            .sources
            .resolve_static_with_declarations(Path::new(self.id), import.source.value.as_str())
        else {
            return;
        };
        #[cfg(test)]
        let exports = if let Some(cache) = self.const_enum_exports {
            cache.analyze(target)
        } else {
            ConstEnumExportAnalyzer::new(target)
                .with_sources(self.sources)
                .analyze()
        };
        #[cfg(not(test))]
        let exports = ConstEnumExportAnalyzer::new(target)
            .with_sources(self.sources)
            .analyze();
        let Some(exports) = exports else {
            return;
        };
        let Some(specifiers) = &import.specifiers else {
            return;
        };

        for specifier in specifiers {
            match specifier {
                ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                    let imported = ModuleExportNameText::new(&specifier.imported).binding_name();
                    let local = specifier.local.name.to_string();
                    if let Some(members) = exports.enums.get(&imported) {
                        imports.insert(local, ConstEnumImport::Enum(members.clone()));
                    } else if let Some(namespace) = exports.namespaces.get(&imported) {
                        imports.insert(local, ConstEnumImport::Namespace(namespace.clone()));
                    }
                }
                ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                    imports.insert(
                        specifier.local.name.to_string(),
                        ConstEnumImport::Namespace(exports.enums.clone()),
                    );
                }
                ImportDeclarationSpecifier::ImportDefaultSpecifier(_) => {}
            }
        }
    }
}

#[derive(Debug)]
#[cfg(test)]
pub(crate) struct ConstEnumExportCache {
    sources: TransformSourceSnapshot,
    entries: RwLock<FxHashMap<PathBuf, Option<ConstEnumExports>>>,
}

#[cfg(test)]
impl ConstEnumExportCache {
    pub(crate) fn new(sources: TransformSourceSnapshot) -> Self {
        Self {
            sources,
            entries: RwLock::new(FxHashMap::default()),
        }
    }

    fn analyze(&self, source: TransformSourceFile) -> Option<ConstEnumExports> {
        let mut visited = FxHashSet::default();
        self.analyze_with_visited(source, &mut visited)
    }

    fn analyze_with_visited(
        &self,
        source: TransformSourceFile,
        visited: &mut FxHashSet<PathBuf>,
    ) -> Option<ConstEnumExports> {
        let source_path = source.path.clone();
        if visited.contains(&source_path) {
            return None;
        }
        if let Some(cached) = self
            .entries
            .read()
            .ok()
            .and_then(|entries| entries.get(&source_path).cloned())
        {
            return cached;
        }

        let exports = ConstEnumExportAnalyzer::new(source)
            .with_sources(&self.sources)
            .with_cache(self)
            .analyze_with_visited(visited);
        if let Ok(mut entries) = self.entries.write() {
            entries
                .entry(source_path)
                .or_insert_with(|| exports.clone());
        }
        exports
    }
}

struct ConstEnumExportAnalyzer<'a> {
    source: TransformSourceFile,
    sources: Option<&'a TransformSourceSnapshot>,
    #[cfg(test)]
    cache: Option<&'a ConstEnumExportCache>,
}

impl<'a> ConstEnumExportAnalyzer<'a> {
    fn new(source: TransformSourceFile) -> Self {
        Self {
            source,
            sources: None,
            #[cfg(test)]
            cache: None,
        }
    }

    fn with_sources(mut self, sources: &'a TransformSourceSnapshot) -> Self {
        self.sources = Some(sources);
        self
    }

    #[cfg(test)]
    fn with_cache(mut self, cache: &'a ConstEnumExportCache) -> Self {
        self.cache = Some(cache);
        self
    }

    fn analyze(&self) -> Option<ConstEnumExports> {
        let mut visited = FxHashSet::default();
        self.analyze_with_visited(&mut visited)
    }

    fn analyze_with_visited(&self, visited: &mut FxHashSet<PathBuf>) -> Option<ConstEnumExports> {
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
        let semantic_ret = SemanticBuilder::new()
            .with_enum_eval(true)
            .build(&parsed.program);
        let scoping = semantic_ret
            .diagnostics
            .is_empty()
            .then(|| semantic_ret.semantic.scoping());

        let mut exports = ConstEnumExports::default();
        for statement in &parsed.program.body {
            match statement {
                Statement::ExportNamedDeclaration(export) => {
                    self.collect_named_export(export, scoping, visited, &mut exports);
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
        scoping: Option<&oxc_semantic::Scoping>,
        visited: &mut FxHashSet<PathBuf>,
        exports: &mut ConstEnumExports,
    ) {
        if let Some(declaration) = &export.declaration {
            self.collect_declaration(declaration, scoping, exports);
        }

        let Some(source) = &export.source else {
            return;
        };
        let Some(target_exports) = self.analyze_reexport_source(source.value.as_str(), visited)
        else {
            return;
        };
        for specifier in &export.specifiers {
            if export.export_kind.is_type() || specifier.export_kind.is_type() {
                continue;
            }
            let local_name = ModuleExportNameText::new(&specifier.local).binding_name();
            let exported_name = ModuleExportNameText::new(&specifier.exported).binding_name();
            if let Some(members) = target_exports.enums.get(&local_name) {
                exports.enums.insert(exported_name, members.clone());
            } else if let Some(namespace) = target_exports.namespaces.get(&local_name) {
                exports.namespaces.insert(exported_name, namespace.clone());
            }
        }
    }

    fn collect_export_all(
        &self,
        export: &ExportAllDeclaration<'_>,
        visited: &mut FxHashSet<PathBuf>,
        exports: &mut ConstEnumExports,
    ) {
        let Some(target_exports) =
            self.analyze_reexport_source(export.source.value.as_str(), visited)
        else {
            return;
        };
        if let Some(exported) = &export.exported {
            exports.namespaces.insert(
                ModuleExportNameText::new(exported).binding_name(),
                target_exports.enums,
            );
            return;
        }
        exports.merge(target_exports);
    }

    fn collect_declaration(
        &self,
        declaration: &Declaration<'_>,
        scoping: Option<&oxc_semantic::Scoping>,
        exports: &mut ConstEnumExports,
    ) {
        match declaration {
            Declaration::TSEnumDeclaration(declaration) if declaration.r#const => {
                let members = scoping
                    .and_then(|scoping| {
                        ConstEnumMemberCollector::new(declaration, scoping).collect()
                    })
                    .or_else(|| ConstEnumAstMemberCollector::new(declaration).collect());
                if let Some(members) = members {
                    exports
                        .enums
                        .insert(declaration.id.name.to_string(), members);
                }
            }
            Declaration::TSModuleDeclaration(declaration) => {
                let Some(scoping) = scoping else {
                    return;
                };
                if let Some(namespace) =
                    ConstEnumNamespaceCollector::new(declaration, scoping).collect()
                {
                    exports.namespaces.insert(namespace.name, namespace.enums);
                }
            }
            _ => {}
        }
    }

    fn analyze_reexport_source(
        &self,
        specifier: &str,
        visited: &mut FxHashSet<PathBuf>,
    ) -> Option<ConstEnumExports> {
        let sources = self.sources.as_ref()?;
        let target = sources.resolve_static_with_declarations(&self.source.path, specifier)?;
        #[cfg(test)]
        if let Some(cache) = self.cache {
            return cache.analyze_with_visited(target, visited);
        }
        ConstEnumExportAnalyzer::new(target)
            .with_sources(sources)
            .analyze_with_visited(visited)
    }
}

impl ConstEnumExports {
    fn merge(&mut self, other: ConstEnumExports) {
        self.enums.extend(other.enums);
        self.namespaces.extend(other.namespaces);
    }
}

struct ConstEnumNamespace {
    name: String,
    enums: FxHashMap<String, FxHashMap<String, EnumValue>>,
}

struct ConstEnumNamespaceCollector<'a, 'b> {
    declaration: &'a oxc_ast::ast::TSModuleDeclaration<'a>,
    scoping: &'b oxc_semantic::Scoping,
}

impl<'a, 'b> ConstEnumNamespaceCollector<'a, 'b> {
    fn new(
        declaration: &'a oxc_ast::ast::TSModuleDeclaration<'a>,
        scoping: &'b oxc_semantic::Scoping,
    ) -> Self {
        Self {
            declaration,
            scoping,
        }
    }

    fn collect(&self) -> Option<ConstEnumNamespace> {
        let TSModuleDeclarationName::Identifier(name) = &self.declaration.id else {
            return None;
        };
        let Some(TSModuleDeclarationBody::TSModuleBlock(block)) = &self.declaration.body else {
            return None;
        };
        let mut enums = FxHashMap::default();
        for statement in &block.body {
            let Statement::ExportNamedDeclaration(export) = statement else {
                continue;
            };
            let Some(Declaration::TSEnumDeclaration(enum_decl)) = &export.declaration else {
                continue;
            };
            if enum_decl.r#const {
                if let Some(members) =
                    ConstEnumMemberCollector::new(enum_decl, self.scoping).collect()
                {
                    enums.insert(enum_decl.id.name.to_string(), members);
                }
            }
        }
        (!enums.is_empty()).then(|| ConstEnumNamespace {
            name: name.name.to_string(),
            enums,
        })
    }
}

struct ConstEnumMemberCollector<'a, 'b> {
    declaration: &'a oxc_ast::ast::TSEnumDeclaration<'a>,
    scoping: &'b oxc_semantic::Scoping,
}

impl<'a, 'b> ConstEnumMemberCollector<'a, 'b> {
    fn new(
        declaration: &'a oxc_ast::ast::TSEnumDeclaration<'a>,
        scoping: &'b oxc_semantic::Scoping,
    ) -> Self {
        Self {
            declaration,
            scoping,
        }
    }

    fn collect(&self) -> Option<FxHashMap<String, EnumValue>> {
        self.collect_from_semantic()
            .or_else(|| ConstEnumAstMemberCollector::new(self.declaration).collect())
    }

    fn collect_from_semantic(&self) -> Option<FxHashMap<String, EnumValue>> {
        let enum_symbol = self.declaration.id.symbol_id();
        let enum_scope = self
            .scoping
            .get_enum_body_scopes(enum_symbol)?
            .first()
            .copied()?;
        let mut members = FxHashMap::default();
        for member in &self.declaration.body.members {
            let member_name = member.id.static_name().to_string();
            let symbol = self
                .scoping
                .get_binding(enum_scope, member_name.as_str().into())?;
            let value = self.scoping.get_enum_member_value(symbol)?;
            let value = match value {
                oxc_syntax::constant_value::ConstantValue::Number(number) => {
                    EnumValue::Number(*number)
                }
                oxc_syntax::constant_value::ConstantValue::String(text) => {
                    EnumValue::String(text.to_string())
                }
            };
            members.insert(member_name, value);
        }
        Some(members)
    }
}

struct ConstEnumAstMemberCollector<'a> {
    declaration: &'a oxc_ast::ast::TSEnumDeclaration<'a>,
}

impl<'a> ConstEnumAstMemberCollector<'a> {
    fn new(declaration: &'a oxc_ast::ast::TSEnumDeclaration<'a>) -> Self {
        Self { declaration }
    }

    fn collect(&self) -> Option<FxHashMap<String, EnumValue>> {
        let mut members = FxHashMap::default();
        let mut previous_number = None;
        for member in &self.declaration.body.members {
            let member_name = member.id.static_name().to_string();
            let value = if let Some(initializer) = &member.initializer {
                self.literal_value(initializer)?
            } else {
                EnumValue::Number(previous_number.map_or(0.0, |number| number + 1.0))
            };
            previous_number = match value {
                EnumValue::Number(number) => Some(number),
                EnumValue::String(_) => None,
            };
            members.insert(member_name, value);
        }
        Some(members)
    }

    fn literal_value(&self, expression: &Expression<'_>) -> Option<EnumValue> {
        match expression {
            Expression::NumericLiteral(literal) => Some(EnumValue::Number(literal.value)),
            Expression::StringLiteral(literal) => {
                Some(EnumValue::String(literal.value.to_string()))
            }
            Expression::UnaryExpression(unary) => {
                let Expression::NumericLiteral(literal) = &unary.argument else {
                    return None;
                };
                match unary.operator {
                    UnaryOperator::UnaryPlus => Some(EnumValue::Number(literal.value)),
                    UnaryOperator::UnaryNegation => Some(EnumValue::Number(-literal.value)),
                    _ => None,
                }
            }
            Expression::ParenthesizedExpression(parenthesized) => {
                self.literal_value(&parenthesized.expression)
            }
            _ => None,
        }
    }
}

struct ConstEnumInlineMutator<'a> {
    ast: AstBuilder<'a>,
    imports: FxHashMap<String, ConstEnumImport>,
    changed: bool,
}

impl<'a> VisitMut<'a> for ConstEnumInlineMutator<'a> {
    fn visit_expression(&mut self, expr: &mut Expression<'a>) {
        if let Some(replacement) = self.const_enum_replacement(expr).cloned() {
            *expr = ConstEnumValueEmitter::new(&replacement).expression(self.ast);
            self.changed = true;
            return;
        }
        walk_mut::walk_expression(self, expr);
    }
}

impl ConstEnumInlineMutator<'_> {
    fn const_enum_replacement<'a>(&'a self, expr: &Expression<'_>) -> Option<&'a EnumValue> {
        let Expression::StaticMemberExpression(member) = expr else {
            return None;
        };
        let member_name = member.property.name.as_str();

        if let Expression::Identifier(object) = &member.object {
            if let Some(ConstEnumImport::Enum(members)) = self.imports.get(object.name.as_str()) {
                return members.get(member_name);
            }
        }

        let Expression::StaticMemberExpression(namespace_member) = &member.object else {
            return None;
        };
        let Expression::Identifier(namespace) = &namespace_member.object else {
            return None;
        };
        let enum_name = namespace_member.property.name.as_str();
        let ConstEnumImport::Namespace(enums) = self.imports.get(namespace.name.as_str())? else {
            return None;
        };
        enums
            .get(enum_name)
            .and_then(|members| members.get(member_name))
    }
}

struct ConstEnumValueEmitter<'a> {
    value: &'a EnumValue,
}

impl<'a> ConstEnumValueEmitter<'a> {
    fn new(value: &'a EnumValue) -> Self {
        Self { value }
    }

    fn expression<'b>(&self, ast: AstBuilder<'b>) -> Expression<'b> {
        match self.value {
            EnumValue::Number(number) => ast.expression_numeric_literal(
                SPAN,
                *number,
                None,
                if number.fract() == 0.0 {
                    NumberBase::Decimal
                } else {
                    NumberBase::Float
                },
            ),
            EnumValue::String(text) => ast.expression_string_literal(SPAN, ast.str(text), None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn snapshot_file(path: &Path, source: &str) -> TransformSourceSnapshot {
        let mut files = FxHashMap::default();
        files.insert(arkdown_path::lexical_normalize(path), Arc::from(source));
        TransformSourceSnapshot::new(files)
    }

    #[test]
    fn typescript_directive_flags_use_comment_ast_not_string_matches() {
        assert!(!TypeScriptDirectiveFlags::from_source(
            "const marker = '// @ts-ignore';\n// ordinary comment",
            "Index.ts",
        )
        .should_emit_javascript());
        assert!(
            TypeScriptDirectiveFlags::from_source(
                "// @ts-ignore: runtime declaration is intentionally untyped\nconst value: string = runtime.value;",
                "Index.ts",
            )
            .should_emit_javascript()
        );
        assert!(!TypeScriptDirectiveFlags::from_source(
            "// @keepTs\n// @ts-ignore\nconst value: string = runtime.value;",
            "Index.ts",
        )
        .should_emit_javascript());
    }

    #[test]
    fn shared_module_directive_matches_sdk_first_non_import_statement_contract() {
        assert!(TypeScriptDirectiveFlags::from_source(
            "import collections from '@ohos:arkts.collections';\nimport emitter from '@ohos:events.emitter';\n\"use shared\";\nexport class ResourceWatcher {}",
            "ResourceWatcher.ts",
        )
        .is_shared_module());
        assert!(!TypeScriptDirectiveFlags::from_source(
            "const marker = 'not an import';\n\"use shared\";",
            "NotShared.ts",
        )
        .is_shared_module());
        assert!(!TypeScriptDirectiveFlags::from_source(
            "import value from './value';\nconst marker = \"use shared\";",
            "StringValue.ts",
        )
        .is_shared_module());
    }

    #[test]
    fn emits_js_and_inlines_imported_const_enum() {
        let dir = tempfile::tempdir().unwrap();
        let enum_path = dir.path().join("ErrorCode.ts");
        let enum_source = "export const enum ErrorCode { OK, INTERNAL_ERROR = 1240001 }\n";
        let sources = snapshot_file(&enum_path, enum_source);
        let source_path = dir.path().join("Use.ts");
        let source = "import { ErrorCode } from './ErrorCode';\nconst x: ErrorCode = ErrorCode.INTERNAL_ERROR;\n";
        let cache = ConstEnumExportCache::new(sources.clone());
        let output =
            TypeScriptJavascriptEmitter::new(source, source_path.to_str().unwrap(), &sources)
                .with_const_enum_export_cache(&cache)
                .emit()
                .unwrap();
        assert!(
            output.contains("1240001") && !output.contains("from './ErrorCode'"),
            "const enum import must be inlined and removed; got:\n{output}"
        );
        assert!(
            !output.contains(": ErrorCode"),
            "Type annotations must be stripped; got:\n{output}"
        );
    }

    #[test]
    fn inlines_namespace_const_enum() {
        let dir = tempfile::tempdir().unwrap();
        let enum_path = dir.path().join("Constant.ts");
        let enum_source =
            "export namespace Constant { export const enum Key { MapProviderKey = 'provider' } }\n";
        let sources = snapshot_file(&enum_path, enum_source);
        let source_path = dir.path().join("Use.ts");
        let source =
            "import { Constant } from './Constant';\nconst key = Constant.Key.MapProviderKey;\n";
        let output =
            TypeScriptJavascriptEmitter::new(source, source_path.to_str().unwrap(), &sources)
                .emit()
                .unwrap();
        assert!(
            output.contains("\"provider\"") && !output.contains("Constant.Key.MapProviderKey"),
            "namespace const enum must be inlined; got:\n{output}"
        );
    }

    #[test]
    fn emit_js_matches_typescript_class_field_semantics() {
        let dir = tempfile::tempdir().unwrap();
        let source_path = dir.path().join("Model.ts");
        let source =
            "class Model { value: string; count = 1; constructor() { this.value = 'ok'; } }\n";
        let sources = TransformSourceSnapshot::default();
        let output =
            TypeScriptJavascriptEmitter::new(source, source_path.to_str().unwrap(), &sources)
                .emit()
                .unwrap();
        assert!(
            !output.contains("value;"),
            "type-only class fields must be removed like TypeScript useDefineForClassFields=false; got:\n{output}"
        );
        assert!(
            output.contains("this.count = 1"),
            "initialized class fields must be lowered into constructor assignments; got:\n{output}"
        );
    }

    #[test]
    fn inlines_const_enum_reexported_from_package_native_declaration() {
        let widget_root = PathBuf::from("/project/oh_modules/@scope/widget");
        let widget_index = widget_root.join("Index.ets");
        let native_root = widget_root.join("native-declarations/custom-location");
        let native_index = native_root.join("Entry.d.ts");
        let native_types = native_root.join("Types.d.ts");

        let mut files = FxHashMap::default();
        files.insert(
            widget_index.clone(),
            Arc::<str>::from(r#"export { EdgeType } from "libwidget.so";"#),
        );
        files.insert(
            native_index.clone(),
            Arc::<str>::from(r#"export * from "./Types";"#),
        );
        files.insert(
            native_types,
            Arc::<str>::from(r#"export const enum EdgeType { TOP = 1, BOTTOM = 3 }"#),
        );
        let sources = TransformSourceSnapshot::with_resolved_static_modules(
            files,
            vec![crate::source::TransformResolvedStaticModule::new(
                &widget_root,
                "libwidget.so",
                &native_index,
            )],
        );
        let source_path = PathBuf::from("/project/Use.ts");
        let source = r#"import { EdgeType } from "@scope/widget";
const top = EdgeType.TOP;"#;
        let resolved_package = sources
            .resolve_static_with_declarations(&source_path, "@scope/widget")
            .expect("package entry resolves");
        assert_eq!(resolved_package.path, widget_index);
        let resolved_native = sources
            .resolve_static_with_declarations(&resolved_package.path, "libwidget.so")
            .expect("native declaration resolves");
        assert_eq!(resolved_native.path, native_index);
        let native_exports = ConstEnumExportAnalyzer::new(resolved_native)
            .with_sources(&sources)
            .analyze()
            .expect("native const enum exports");
        assert!(
            native_exports.enums.contains_key("EdgeType"),
            "native declaration export-all must expose EdgeType const enum"
        );
        let exports = ConstEnumExportAnalyzer::new(resolved_package)
            .with_sources(&sources)
            .analyze()
            .expect("const enum exports");
        assert!(
            exports.enums.contains_key("EdgeType"),
            "package native reexport must expose EdgeType const enum"
        );

        let output =
            TypeScriptJavascriptEmitter::new(source, source_path.to_str().unwrap(), &sources)
                .emit()
                .unwrap();

        assert!(
            output.contains("const top = 1"),
            "native declaration const enum must be inlined; got:\n{output}"
        );
        assert!(
            !output.contains("@scope/widget") && !output.contains("EdgeType.TOP"),
            "const enum import must be erased after inlining; got:\n{output}"
        );
    }
}
