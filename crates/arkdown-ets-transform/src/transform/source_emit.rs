//! Source emit helpers for ArkTS transform lowering.
//!
//! Upstream ets-loader keeps original AST nodes as nodes and lets the TypeScript
//! printer preserve their source shape where possible. Centralize remaining
//! source text boundaries here while generated expression text is printed by OXC
//! codegen.

use std::collections::{HashMap, VecDeque};

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    ArkUIComponentExpression, ArrayExpression, ArrayExpressionElement, ArrowFunctionExpression,
    AssignmentExpression, BinaryExpression, BindingIdentifier, CallExpression, CatchClause, Class,
    Comment, ExportDefaultDeclaration, ExportDefaultDeclarationKind, ExportFromDeclaration,
    Expression, ExpressionStatement, Function, IdentifierName, IdentifierReference, IfStatement,
    ImportDeclaration, ImportDeclarationSpecifier, LogicalExpression, MethodDefinition,
    MethodDefinitionKind, NumericLiteral, ObjectExpression, ObjectPropertyKind,
    ParenthesizedExpression, Program, PropertyDefinition, PropertyKey, ReturnStatement, Statement,
    StaticMemberExpression, StringLiteral, TSPropertySignature, TSType, TemplateLiteral,
    ThisExpression, TryStatement, VariableDeclarator,
};
use oxc_ast_visit::{walk, Visit};
use oxc_codegen::{Codegen, CodegenOptions, CommentOptions};
use oxc_parser::Parser;
use oxc_semantic::Scoping;
use oxc_span::{GetSpan, SourceType, Span};
use rolldown_sourcemap::SourceMapBuilder;
use rustc_hash::{FxHashMap, FxHashSet};

pub struct HvigorTypescriptPrinter;

impl HvigorTypescriptPrinter {
    pub(crate) fn print(program: &Program<'_>, options: CodegenOptions) -> String {
        Self::print_internal(
            program,
            options,
            None,
            None,
            TypeScriptSourceTreeKind::TransformedArkts,
        )
        .code
    }

    pub(crate) fn print_with_source_map(
        program: &Program<'_>,
        options: CodegenOptions,
        source_text: &str,
    ) -> HvigorTypescriptPrintOutput {
        Self::print_internal(
            program,
            options,
            Some(source_text),
            None,
            TypeScriptSourceTreeKind::TransformedArkts,
        )
    }

    pub fn print_with_scoping(
        program: &Program<'_>,
        options: CodegenOptions,
        scoping: Scoping,
    ) -> String {
        Self::print_internal(
            program,
            options,
            None,
            Some(scoping),
            TypeScriptSourceTreeKind::ParsedSource,
        )
        .code
    }

    pub fn print_with_source_map_and_scoping(
        program: &Program<'_>,
        options: CodegenOptions,
        source_text: &str,
        scoping: Scoping,
    ) -> HvigorTypescriptPrintOutput {
        Self::print_internal(
            program,
            options,
            Some(source_text),
            Some(scoping),
            TypeScriptSourceTreeKind::ParsedSource,
        )
    }

    fn print_internal(
        program: &Program<'_>,
        options: CodegenOptions,
        source_text: Option<&str>,
        scoping: Option<Scoping>,
        source_tree_kind: TypeScriptSourceTreeKind,
    ) -> HvigorTypescriptPrintOutput {
        let mut numeric_literals = NumericLiteralInventory::default();
        numeric_literals.visit_program(program);
        let mut string_literals = StringLiteralInventory::new(program.source_text);
        string_literals.visit_program(program);
        let mut original_boundaries = TypeScriptSourceBoundaryInventory::with_source_text(
            program.source_text,
            source_tree_kind,
        );
        original_boundaries.visit_program(program);
        let source_comments = if options.comments == CommentOptions::disabled() {
            TypeScriptSourceCommentSnapshot::default()
        } else {
            TypeScriptSourceCommentSnapshot::from_program(program)
        };
        let mut source_layouts = TypeScriptSourceLayoutInventory::from_program_source(program);
        source_layouts.record_synthetic_type_import_aliases(program);
        let generated = Codegen::new()
            .with_options(options)
            .with_scoping(scoping)
            .build(program);
        let layout = HvigorTypescriptLayout::new(
            generated.code,
            numeric_literals.raw_literals,
            string_literals.literals,
            source_layouts,
            &source_comments,
        )
        .format();
        let source_map = generated.map.map(|source_map| {
            let supplemental = source_text
                .map(|source_text| {
                    TypeScriptSourceBoundaryMapper::new(
                        source_text,
                        &layout.code,
                        original_boundaries.boundaries.as_slice(),
                        &source_comments,
                        source_tree_kind,
                    )
                    .mappings()
                })
                .unwrap_or_default();
            let mut tokens = source_map
                .get_tokens()
                .map(|token| {
                    let (dst_line, dst_col) = layout
                        .offset_map
                        .remap_line_col(token.get_dst_line(), token.get_dst_col());
                    HvigorSourceMapToken {
                        dst_line,
                        dst_col,
                        src_line: token.get_src_line(),
                        src_col: token.get_src_col(),
                        source_id: token.get_source_id(),
                        name_id: token.get_name_id(),
                        supplemental: false,
                    }
                })
                .filter(|token| {
                    !supplemental
                        .suppressed_generated_positions
                        .contains(&(token.dst_line, token.dst_col))
                })
                .chain(
                    supplemental
                        .entries
                        .into_iter()
                        .map(|entry| HvigorSourceMapToken {
                            dst_line: entry.generated_line,
                            dst_col: entry.generated_col,
                            src_line: entry.source_line,
                            src_col: entry.source_col,
                            source_id: Some(0),
                            name_id: None,
                            supplemental: true,
                        }),
                )
                .collect::<Vec<_>>();
            tokens.sort_by_key(|token| (token.dst_line, token.dst_col, !token.supplemental));
            tokens.dedup_by(|right, left| {
                right.dst_line == left.dst_line && right.dst_col == left.dst_col
            });

            let mut builder = SourceMapBuilder::default();
            if let Some(file) = source_map.get_file() {
                builder.set_file(file);
            }
            for name in source_map.get_names() {
                builder.add_name(name);
            }
            for source in source_map.get_sources() {
                builder.set_source_and_content(source, "");
            }
            for token in tokens {
                builder.add_token(
                    token.dst_line,
                    token.dst_col,
                    token.src_line,
                    token.src_col,
                    token.source_id,
                    token.name_id,
                );
            }
            let mut source_map = builder.into_sourcemap();
            source_map.set_source_contents(
                source_map
                    .get_sources()
                    .map(|_| None)
                    .collect::<Vec<Option<&str>>>(),
            );
            source_map.to_json_string()
        });
        HvigorTypescriptPrintOutput {
            code: layout.code,
            source_map,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TypeScriptSourceTreeKind {
    TransformedArkts,
    ParsedSource,
}

pub struct HvigorTypescriptPrintOutput {
    pub code: String,
    pub source_map: Option<String>,
}

struct HvigorSourceMapToken {
    dst_line: u32,
    dst_col: u32,
    src_line: u32,
    src_col: u32,
    source_id: Option<u32>,
    name_id: Option<u32>,
    supplemental: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum TypeScriptSourceBoundaryKind {
    Identifier(String),
    StringLiteral(String),
    NumericLiteral(u64),
    This,
    Type,
    Method {
        name: String,
        constructor: bool,
    },
    AsyncMethod(String),
    /// TypeScript writes a mapping after the closing brace of source-backed
    /// methods. ArkUI lowers `build` to `initialRender`; the generated
    /// constructor retains TypeScript's synthetic file-start closure mapping.
    MethodClosure {
        name: String,
        constructor: bool,
    },
    Property(String),
    ArrayExpression,
    ExpressionStatementClosure,
    VariableDeclaration,
    VariableDeclarator,
    ExportVariablePrefix,
    ClassHeritageClause,
    ClassBodyOpen,
    ModifierClosure(String),
    TypeMemberClosure(String),
    Operator(String),
    ExportNamedClause,
    ImportNamedClause,
    ImportDeclaration,
    ArrowFunction,
    CallExpression(String),
    AssignmentOperator,
    ExportDefaultFunction,
    ExportDefaultClass,
    FunctionClosure(String),
    CatchClause,
    ParenthesizedExpression,
    TemplateInterpolationClosure,
    ObjectExpressionClosure,
    LineBrokenMember(String),
    ReturnStatement {
        has_argument: bool,
    },
    IfStatementClosure,
    TryStatementClosure,
    /// ArkUI component block closure: source components (`Column() { ... }`)
    /// map to their generated `Column.pop()` calls (pop order is LIFO).
    ComponentClosure(String),
}

#[derive(Clone)]
struct TypeScriptSourceBoundary {
    kind: TypeScriptSourceBoundaryKind,
    span: Span,
    property_key_span: Option<Span>,
    method_body_span: Option<Span>,
}

struct TypeScriptSourceBoundaryInventory<'a> {
    boundaries: Vec<TypeScriptSourceBoundary>,
    accessor_prefixes: Vec<Span>,
    source_text: Option<&'a str>,
    source_tree_kind: TypeScriptSourceTreeKind,
}

impl<'a> TypeScriptSourceBoundaryInventory<'a> {
    fn with_source_text(source: &'a str, source_tree_kind: TypeScriptSourceTreeKind) -> Self {
        Self {
            boundaries: Vec::new(),
            accessor_prefixes: Vec::new(),
            source_text: Some(source),
            source_tree_kind,
        }
    }
}

impl<'a> TypeScriptSourceBoundaryInventory<'a> {
    fn push(&mut self, kind: TypeScriptSourceBoundaryKind, span: Span) {
        self.boundaries.push(TypeScriptSourceBoundary {
            kind,
            span,
            property_key_span: None,
            method_body_span: None,
        });
    }

    fn property_name(key: &PropertyKey<'_>) -> String {
        key.static_name()
            .map(|name| name.into_owned())
            .unwrap_or_default()
    }

    fn push_modifier_closures(&mut self, declaration_start: u32, name_start: u32) {
        if self.source_tree_kind != TypeScriptSourceTreeKind::ParsedSource {
            return;
        }
        let Some(prefix) = self
            .source_text
            .and_then(|source| source.get(declaration_start as usize..name_start as usize))
        else {
            return;
        };
        for modifier in [
            "public",
            "private",
            "protected",
            "static",
            "abstract",
            "declare",
            "readonly",
            "override",
            "async",
        ] {
            let mut search_start = 0;
            while let Some(relative_start) = prefix[search_start..].find(modifier) {
                let relative_start = search_start + relative_start;
                let relative_end = relative_start + modifier.len();
                let before = prefix[..relative_start].chars().next_back();
                let after = prefix[relative_end..].chars().next();
                if before.is_none_or(|ch| !ch.is_ascii_alphanumeric() && ch != '_')
                    && after.is_none_or(|ch| !ch.is_ascii_alphanumeric() && ch != '_')
                {
                    self.push(
                        TypeScriptSourceBoundaryKind::ModifierClosure(modifier.to_string()),
                        Span::new(
                            declaration_start + relative_end as u32,
                            declaration_start + relative_end as u32,
                        ),
                    );
                }
                search_start = relative_end;
            }
        }
    }
}

impl<'a> Visit<'a> for TypeScriptSourceBoundaryInventory<'a> {
    fn visit_class(&mut self, class: &Class<'a>) {
        if self.source_tree_kind == TypeScriptSourceTreeKind::ParsedSource {
            if let (Some(source), Some(heritage)) = (self.source_text, class.heritage.as_ref()) {
                let super_start = heritage.expression.span().start;
                let heritage_end = heritage
                    .type_arguments
                    .as_deref()
                    .map_or(heritage.expression.span().end, |arguments| {
                        arguments.span.end
                    });
                if let Some(relative_start) = source
                    .get(class.span.start as usize..super_start as usize)
                    .and_then(|between| between.rfind("extends"))
                {
                    let start = class.span.start + relative_start as u32;
                    self.push(
                        TypeScriptSourceBoundaryKind::ClassHeritageClause,
                        Span::new(start, heritage_end),
                    );
                }
            }
            self.push(TypeScriptSourceBoundaryKind::ClassBodyOpen, class.body.span);
        }
        walk::walk_class(self, class);
    }

    fn visit_export_from_declaration(&mut self, declaration: &ExportFromDeclaration<'a>) {
        if let (Some(source), Some(first), Some(last)) = (
            self.source_text,
            declaration.specifiers.first(),
            declaration.specifiers.last(),
        ) {
            let first_span = first.span();
            let last_span = last.span();
            let open = source
                .get(..first_span.start as usize)
                .and_then(|prefix| prefix.rfind('{'));
            let close = source
                .get(last_span.end as usize..)
                .and_then(|suffix| suffix.find('}'))
                .map(|offset| last_span.end as usize + offset + 1);
            if let (Some(open), Some(close)) = (open, close) {
                self.push(
                    TypeScriptSourceBoundaryKind::ExportNamedClause,
                    Span::new(open as u32, close as u32),
                );
            }
        }
        walk::walk_export_from_declaration(self, declaration);
    }

    fn visit_import_declaration(&mut self, declaration: &ImportDeclaration<'a>) {
        if self.source_tree_kind == TypeScriptSourceTreeKind::ParsedSource {
            self.push(
                TypeScriptSourceBoundaryKind::ImportDeclaration,
                declaration.span,
            );
        }
        if let (Some(source), Some(specifiers)) =
            (self.source_text, declaration.specifiers.as_ref())
        {
            let named_specifiers = specifiers
                .iter()
                .filter_map(|specifier| match specifier {
                    ImportDeclarationSpecifier::ImportSpecifier(specifier) => Some(specifier),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if let (Some(first), Some(last)) = (named_specifiers.first(), named_specifiers.last()) {
                let first_span = first.imported.span();
                let last_span = last.local.span;
                let source_line = source
                    .get(last_span.end as usize..)
                    .unwrap_or_default()
                    .lines()
                    .next()
                    .unwrap_or_default();
                let retains_source_request =
                    source_line.contains(declaration.source.value.as_str());
                if !retains_source_request {
                    walk::walk_import_declaration(self, declaration);
                    return;
                }
                // Module resolution may rebuild an import declaration with a
                // synthetic declaration span while retaining the specifiers'
                // source spans. TypeScript anchors the named-clause braces to
                // those retained specifiers, so they are the authoritative
                // lookup boundary here as well.
                let open = source
                    .get(..first_span.start as usize)
                    .and_then(|prefix| prefix.rfind('{'));
                let close = source
                    .get(last_span.end as usize..)
                    .and_then(|suffix| suffix.find('}'))
                    .map(|offset| last_span.end as usize + offset + 1);
                if let (Some(open), Some(close)) = (open, close) {
                    self.push(
                        TypeScriptSourceBoundaryKind::ImportNamedClause,
                        Span::new(open as u32, close as u32),
                    );
                }
            }
        }
        walk::walk_import_declaration(self, declaration);
    }

    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        self.push(TypeScriptSourceBoundaryKind::ArrowFunction, arrow.span);
        walk::walk_arrow_function_expression(self, arrow);
    }

    fn visit_call_expression(&mut self, expression: &CallExpression<'a>) {
        let callee = self.source_text.map_or_else(String::new, |source| {
            let span = expression.callee.span();
            source
                .get(span.start as usize..span.end as usize)
                .unwrap_or_default()
                .trim()
                .to_string()
        });
        self.push(
            TypeScriptSourceBoundaryKind::CallExpression(callee),
            expression.span,
        );
        walk::walk_call_expression(self, expression);
    }

    fn visit_assignment_expression(&mut self, expression: &AssignmentExpression<'a>) {
        let operator_span = if expression.span.is_empty() {
            expression.span
        } else {
            let left_end = expression.left.span().end as usize;
            let right_start = expression.right.span().start as usize;
            self.source_text
                .and_then(|source| source.get(left_end..right_start))
                .filter(|between| between.contains(expression.operator.as_str()))
                .map_or_else(Span::default, |_| {
                    Span::new(left_end as u32, right_start as u32)
                })
        };
        // `process_struct_componentV2.ts::createPropertyAssignNode` inserts
        // factory-created assignments before source-backed method assignments.
        // Keep every assignment in printer traversal order so a synthesized
        // assignment consumes its emitted counterpart without stealing the
        // next source assignment's boundary. Its empty span is deliberately
        // retained below: TypeScript emits no operator mapping for that node.
        self.push(
            TypeScriptSourceBoundaryKind::AssignmentOperator,
            operator_span,
        );
        walk::walk_assignment_expression(self, expression);
    }

    fn visit_export_default_declaration(&mut self, declaration: &ExportDefaultDeclaration<'a>) {
        match declaration.declaration {
            ExportDefaultDeclarationKind::FunctionDeclaration(_) => self.push(
                TypeScriptSourceBoundaryKind::ExportDefaultFunction,
                declaration.span,
            ),
            ExportDefaultDeclarationKind::ClassDeclaration(_) => self.push(
                TypeScriptSourceBoundaryKind::ExportDefaultClass,
                declaration.span,
            ),
            _ => {}
        }
        walk::walk_export_default_declaration(self, declaration);
    }

    fn visit_return_statement(&mut self, statement: &ReturnStatement<'a>) {
        self.push(
            TypeScriptSourceBoundaryKind::ReturnStatement {
                has_argument: statement.argument.is_some(),
            },
            statement.span,
        );
        walk::walk_return_statement(self, statement);
    }

    fn visit_catch_clause(&mut self, clause: &CatchClause<'a>) {
        self.push(TypeScriptSourceBoundaryKind::CatchClause, clause.span);
        walk::walk_catch_clause(self, clause);
    }

    fn visit_parenthesized_expression(&mut self, expression: &ParenthesizedExpression<'a>) {
        self.push(
            TypeScriptSourceBoundaryKind::ParenthesizedExpression,
            expression.span,
        );
        walk::walk_parenthesized_expression(self, expression);
    }

    fn visit_template_literal(&mut self, literal: &TemplateLiteral<'a>) {
        for expression in &literal.expressions {
            self.push(
                TypeScriptSourceBoundaryKind::TemplateInterpolationClosure,
                Span::new(expression.span().end + 1, expression.span().end + 1),
            );
        }
        walk::walk_template_literal(self, literal);
    }

    fn visit_object_expression(&mut self, expression: &ObjectExpression<'a>) {
        self.push(
            TypeScriptSourceBoundaryKind::ObjectExpressionClosure,
            expression.span,
        );
        walk::walk_object_expression(self, expression);
    }

    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'a>) {
        if self.source_text.is_some_and(|source| {
            source
                .get(member.object.span().end as usize..member.property.span.start as usize)
                .is_some_and(|between| between.contains('\n'))
        }) {
            self.push(
                TypeScriptSourceBoundaryKind::LineBrokenMember(member.property.name.to_string()),
                Span::new(
                    member.property.span.start.saturating_sub(1),
                    member.property.span.start,
                ),
            );
        }
        walk::walk_static_member_expression(self, member);
    }

    fn visit_function(&mut self, function: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        self.push(
            TypeScriptSourceBoundaryKind::FunctionClosure(
                function
                    .id
                    .as_ref()
                    .map_or_else(String::new, |identifier| identifier.name.to_string()),
            ),
            function.span,
        );
        walk::walk_function(self, function, flags);
    }

    fn visit_if_statement(&mut self, statement: &IfStatement<'a>) {
        if let Statement::BlockStatement(block) = &statement.consequent {
            self.push(TypeScriptSourceBoundaryKind::IfStatementClosure, block.span);
        }
        walk::walk_if_statement(self, statement);
    }

    fn visit_try_statement(&mut self, statement: &TryStatement<'a>) {
        self.push(
            TypeScriptSourceBoundaryKind::TryStatementClosure,
            statement.block.span,
        );
        if let Some(handler) = statement.handler.as_ref() {
            self.push(
                TypeScriptSourceBoundaryKind::TryStatementClosure,
                handler.body.span,
            );
        }
        walk::walk_try_statement(self, statement);
    }

    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        self.push(
            TypeScriptSourceBoundaryKind::Identifier(identifier.name.to_string()),
            identifier.span,
        );
    }

    fn visit_identifier_name(&mut self, identifier: &IdentifierName<'a>) {
        self.push(
            TypeScriptSourceBoundaryKind::Identifier(identifier.name.to_string()),
            identifier.span,
        );
    }

    fn visit_binding_identifier(&mut self, identifier: &BindingIdentifier<'a>) {
        self.push(
            TypeScriptSourceBoundaryKind::Identifier(identifier.name.to_string()),
            identifier.span,
        );
    }

    fn visit_string_literal(&mut self, literal: &StringLiteral<'a>) {
        self.push(
            TypeScriptSourceBoundaryKind::StringLiteral(literal.value.to_string()),
            literal.span,
        );
    }

    fn visit_numeric_literal(&mut self, literal: &NumericLiteral<'a>) {
        self.push(
            TypeScriptSourceBoundaryKind::NumericLiteral(literal.value.to_bits()),
            literal.span,
        );
    }

    fn visit_this_expression(&mut self, expression: &ThisExpression) {
        self.push(TypeScriptSourceBoundaryKind::This, expression.span);
    }

    fn visit_ts_type(&mut self, ty: &TSType<'a>) {
        self.push(TypeScriptSourceBoundaryKind::Type, ty.span());
        walk::walk_ts_type(self, ty);
    }

    fn visit_method_definition(&mut self, method: &MethodDefinition<'a>) {
        let name = Self::property_name(&method.key);
        let constructor = method.kind == MethodDefinitionKind::Constructor;
        self.push_modifier_closures(method.span.start, method.key.span().start);
        if method.value.r#async && method.span.start < method.key.span().start {
            self.push(
                TypeScriptSourceBoundaryKind::AsyncMethod(name.clone()),
                Span::new(
                    method.key.span().start.saturating_sub(1),
                    method.key.span().start,
                ),
            );
        }
        self.boundaries.push(TypeScriptSourceBoundary {
            kind: TypeScriptSourceBoundaryKind::Method {
                name: name.clone(),
                constructor,
            },
            // TypeScript anchors method mappings at the property key, skipping
            // `get`/`set`/`static`/accessibility keywords.
            span: method.key.span(),
            property_key_span: None,
            method_body_span: method.value.body.as_deref().map(|body| body.span),
        });
        if self.source_tree_kind == TypeScriptSourceTreeKind::TransformedArkts
            && matches!(
                method.kind,
                MethodDefinitionKind::Get | MethodDefinitionKind::Set
            )
        {
            self.accessor_prefixes
                .push(Span::new(method.span.start, method.span.start));
        }
        if let Some(body) = method.value.body.as_deref() {
            if constructor || name == "initialRender" || !body.span.is_empty() {
                let closure_offset =
                    if constructor && !method.span.is_empty() && method.key.span().is_empty() {
                        self.source_text.map_or(0, |source| {
                            Self::previous_source_token_end(source, method.span.start)
                        })
                    } else if name == "initialRender" && !method.span.is_empty() {
                        method.span.end
                    } else {
                        body.span.end
                    };
                self.push(
                    TypeScriptSourceBoundaryKind::MethodClosure { name, constructor },
                    Span::new(closure_offset, closure_offset),
                );
            }
        }
        walk::walk_method_definition(self, method);
    }

    fn visit_property_definition(&mut self, property: &PropertyDefinition<'a>) {
        self.push_modifier_closures(property.span.start, property.key.span().start);
        self.boundaries.push(TypeScriptSourceBoundary {
            kind: TypeScriptSourceBoundaryKind::Property(Self::property_name(&property.key)),
            // TypeScript maps the whole property declaration (including the
            // `private`/`readonly` keywords) from the source property start;
            // the trailing `;` maps to the source semicolon (span end).
            span: Span::new(property.span.start, property.span.end),
            property_key_span: Some(property.key.span()),
            method_body_span: None,
        });
        walk::walk_property_definition(self, property);
    }

    fn visit_ts_property_signature(&mut self, property: &TSPropertySignature<'a>) {
        if self.source_tree_kind == TypeScriptSourceTreeKind::ParsedSource {
            let name = property
                .key
                .static_name()
                .map(|name| name.into_owned())
                .unwrap_or_default();
            // OXC includes the terminator in this span. TypeScript's generic
            // `emitSourceMapsAfterNode` runs after `writeTrailingSemicolon`, at
            // the span end rather than on the semicolon itself.
            self.push(
                TypeScriptSourceBoundaryKind::TypeMemberClosure(name),
                Span::new(property.span.end, property.span.end),
            );
        }
        walk::walk_ts_property_signature(self, property);
    }

    fn visit_array_expression(&mut self, expression: &ArrayExpression<'a>) {
        // The TypeScript printer emits the closing boundary for source-backed
        // array literals. OXC maps their inner tokens but omits this final
        // range when the literal is moved into a generated assignment.
        self.push(
            TypeScriptSourceBoundaryKind::ArrayExpression,
            expression.span,
        );
        walk::walk_array_expression(self, expression);
    }

    fn visit_ark_ui_component_expression(&mut self, component: &ArkUIComponentExpression<'a>) {
        let callee_name = match &component.callee {
            Expression::Identifier(identifier) => Some(identifier.name.as_str()),
            _ => None,
        };
        if let Some(name) = callee_name {
            self.push(
                TypeScriptSourceBoundaryKind::ComponentClosure(name.to_string()),
                component.callee.span(),
            );
        }
        walk::walk_ark_ui_component_expression(self, component);
    }

    /// Generated `Column.pop()` calls close the corresponding source
    /// component block; they are collected as component closures (matched
    /// LIFO because pop order mirrors the reverse of the source nesting).
    fn visit_expression_statement(&mut self, stmt: &ExpressionStatement<'a>) {
        self.boundaries.push(TypeScriptSourceBoundary {
            kind: TypeScriptSourceBoundaryKind::ExpressionStatementClosure,
            span: stmt.span,
            property_key_span: None,
            method_body_span: None,
        });
        if let Expression::CallExpression(call) = &stmt.expression {
            if let Expression::StaticMemberExpression(_) = call.callee.without_parentheses() {}
        }
        if let Expression::CallExpression(call) = &stmt.expression {
            if let Expression::StaticMemberExpression(member) = call.callee.without_parentheses() {
                if member.property.name.as_str() == "pop" {
                    let name = match &member.object {
                        Expression::Identifier(identifier) => Some(identifier.name.as_str()),
                        _ => None,
                    };
                    if let Some(name) = name {
                        self.push(
                            TypeScriptSourceBoundaryKind::ComponentClosure(name.to_string()),
                            member.object.span(),
                        );
                    }
                    if self.source_tree_kind == TypeScriptSourceTreeKind::TransformedArkts {
                        // The component name inside a pop call maps to the
                        // source component block closure in the first printer
                        // stage, not to the generated component identifier.
                        return;
                    }
                }
            }
        }
        walk::walk_expression_statement(self, stmt);
    }

    fn visit_variable_declaration(&mut self, declaration: &oxc_ast::ast::VariableDeclaration<'a>) {
        if self.source_text.is_some_and(|source| {
            source
                .get(..declaration.span.start as usize)
                .is_some_and(|prefix| prefix.ends_with("export "))
        }) {
            self.push(
                TypeScriptSourceBoundaryKind::ExportVariablePrefix,
                Span::new(
                    declaration.span.start.saturating_sub(1),
                    declaration.span.start,
                ),
            );
        }
        self.push(
            TypeScriptSourceBoundaryKind::VariableDeclaration,
            declaration.span,
        );
        walk::walk_variable_declaration(self, declaration);
    }

    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        if self.source_tree_kind == TypeScriptSourceTreeKind::ParsedSource {
            // RenameIdentifierTransformer.updateNameNode creates a synthetic
            // Identifier, while TypeScript's updateVariableDeclaration
            // preserves this parsed parent range through
            // updateWithOriginal/setTextRange.
            self.push(
                TypeScriptSourceBoundaryKind::VariableDeclarator,
                declarator.span,
            );
        }
        walk::walk_variable_declarator(self, declarator);
    }

    fn visit_binary_expression(&mut self, expr: &BinaryExpression<'a>) {
        let left_end = expr.left.span().end as usize;
        let right_start = expr.right.span().start as usize;
        if let Some(source) = self.source_text {
            if left_end < right_start {
                let between = &source[left_end..right_start];
                let operator = between.trim();
                if !operator.is_empty()
                    && operator
                        .chars()
                        .all(|ch| !ch.is_ascii_alphanumeric() && ch != '_')
                {
                    self.push(
                        TypeScriptSourceBoundaryKind::Operator(operator.to_string()),
                        Span::new(left_end as u32, right_start as u32),
                    );
                }
            }
        }
        walk::walk_binary_expression(self, expr);
    }

    fn visit_logical_expression(&mut self, expression: &LogicalExpression<'a>) {
        let left_end = expression.left.span().end as usize;
        let right_start = expression.right.span().start as usize;
        if let Some(source) = self.source_text {
            if let Some(between) = source.get(left_end..right_start) {
                if between.contains(expression.operator.as_str()) {
                    self.push(
                        TypeScriptSourceBoundaryKind::Operator(
                            expression.operator.as_str().to_string(),
                        ),
                        Span::new(left_end as u32, right_start as u32),
                    );
                }
            }
        }
        walk::walk_logical_expression(self, expression);
    }
}

impl TypeScriptSourceBoundaryInventory<'_> {
    fn previous_source_token_end(source: &str, offset: u32) -> u32 {
        source
            .get(..offset as usize)
            .map(str::trim_end)
            .map_or(0, |prefix| prefix.len() as u32)
    }
}

struct TypeScriptSourceBoundaryMapper<'a> {
    source: &'a str,
    generated: &'a str,
    original: &'a [TypeScriptSourceBoundary],
    source_comments: &'a TypeScriptSourceCommentSnapshot,
    source_tree_kind: TypeScriptSourceTreeKind,
}

impl<'a> TypeScriptSourceBoundaryMapper<'a> {
    fn new(
        source: &'a str,
        generated: &'a str,
        original: &'a [TypeScriptSourceBoundary],
        source_comments: &'a TypeScriptSourceCommentSnapshot,
        source_tree_kind: TypeScriptSourceTreeKind,
    ) -> Self {
        Self {
            source,
            generated,
            original,
            source_comments,
            source_tree_kind,
        }
    }

    fn mappings(&self) -> TypeScriptSourceBoundaryMappings {
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, self.generated, SourceType::ets()).parse();
        // Generated ArkTS can carry recoverable parser diagnostics for
        // platform-specific type constructs. OXC still returns the complete
        // recoverable AST in that case; only a parser panic makes boundary
        // reconstruction unsafe.
        if parsed.panicked {
            return TypeScriptSourceBoundaryMappings::default();
        }
        let mut generated = TypeScriptSourceBoundaryInventory::with_source_text(
            parsed.program.source_text,
            self.source_tree_kind,
        );
        generated.visit_program(&parsed.program);
        let mut suppressed_generated_positions = generated
            .accessor_prefixes
            .iter()
            .map(|span| crate::source_map::offset_to_line_col(self.generated, span.start))
            .collect::<FxHashSet<_>>();
        let mut generated_by_kind =
            HashMap::<TypeScriptSourceBoundaryKind, VecDeque<TypeScriptSourceBoundary>>::new();
        for boundary in generated.boundaries {
            generated_by_kind
                .entry(boundary.kind.clone())
                .or_default()
                .push_back(boundary);
        }
        let mut entries = Vec::new();
        self.map_comment_boundaries(&parsed.program.comments, &mut entries);
        for original in self.original {
            let generated_boundary = match &original.kind {
                // Component closures close in pop order (LIFO).
                TypeScriptSourceBoundaryKind::ComponentClosure(_) => generated_by_kind
                    .get_mut(&original.kind)
                    .and_then(VecDeque::pop_back),
                _ => generated_by_kind
                    .get_mut(&original.kind)
                    .and_then(VecDeque::pop_front),
            };
            let Some(generated_boundary) = generated_boundary else {
                continue;
            };
            let generated_span = generated_boundary.span;
            match original.kind {
                TypeScriptSourceBoundaryKind::ClassHeritageClause => {
                    if !original.span.is_empty() && !generated_span.is_empty() {
                        entries.push(TypeScriptSourceMapEntry::from_offsets(
                            self.generated,
                            generated_span.start.saturating_sub(1),
                            self.source,
                            original.span.start,
                        ));
                        entries.push(TypeScriptSourceMapEntry::from_offsets(
                            self.generated,
                            generated_span.end,
                            self.source,
                            original.span.end,
                        ));
                    }
                    continue;
                }
                TypeScriptSourceBoundaryKind::ClassBodyOpen => {
                    suppressed_generated_positions.insert(crate::source_map::offset_to_line_col(
                        self.generated,
                        generated_span.start,
                    ));
                    continue;
                }
                TypeScriptSourceBoundaryKind::ModifierClosure(_)
                | TypeScriptSourceBoundaryKind::TypeMemberClosure(_) => {
                    entries.push(TypeScriptSourceMapEntry::from_offsets(
                        self.generated,
                        generated_span.start,
                        self.source,
                        original.span.start,
                    ));
                    continue;
                }
                TypeScriptSourceBoundaryKind::ArrowFunction => {
                    self.map_arrow_boundaries(
                        original,
                        &generated_boundary,
                        &mut entries,
                        &mut suppressed_generated_positions,
                    );
                    continue;
                }
                TypeScriptSourceBoundaryKind::CallExpression(_) => {
                    self.map_call_closure(
                        original,
                        &generated_boundary,
                        &mut entries,
                        &mut suppressed_generated_positions,
                    );
                    continue;
                }
                TypeScriptSourceBoundaryKind::AssignmentOperator => {
                    if !original.span.is_empty() {
                        entries.push(TypeScriptSourceMapEntry::from_offsets(
                            self.generated,
                            generated_boundary.span.start,
                            self.source,
                            original.span.start,
                        ));
                    }
                    continue;
                }
                TypeScriptSourceBoundaryKind::ExportDefaultFunction => {
                    self.map_export_default_function(
                        original,
                        &generated_boundary,
                        &mut entries,
                        &mut suppressed_generated_positions,
                    );
                    continue;
                }
                TypeScriptSourceBoundaryKind::ExportDefaultClass => {
                    self.map_export_default_class(
                        original,
                        &generated_boundary,
                        &mut entries,
                        &mut suppressed_generated_positions,
                    );
                    continue;
                }
                TypeScriptSourceBoundaryKind::FunctionClosure(_) => {
                    self.map_function_boundaries(
                        original,
                        &generated_boundary,
                        &mut entries,
                        &mut suppressed_generated_positions,
                    );
                    continue;
                }
                TypeScriptSourceBoundaryKind::CatchClause => {
                    entries.push(TypeScriptSourceMapEntry::from_offsets(
                        self.generated,
                        generated_span.start,
                        self.source,
                        original.span.start,
                    ));
                    continue;
                }
                TypeScriptSourceBoundaryKind::ParenthesizedExpression => {
                    if !original.span.is_empty() && !generated_span.is_empty() {
                        for (generated_offset, source_offset) in [
                            (generated_span.start, original.span.start),
                            (generated_span.end - 1, original.span.end - 1),
                            (generated_span.end, original.span.end),
                        ] {
                            entries.push(TypeScriptSourceMapEntry::from_offsets(
                                self.generated,
                                generated_offset,
                                self.source,
                                source_offset,
                            ));
                        }
                    }
                    continue;
                }
                TypeScriptSourceBoundaryKind::ObjectExpressionClosure => {
                    let position = generated_span.end.saturating_sub(1);
                    suppressed_generated_positions.insert(crate::source_map::offset_to_line_col(
                        self.generated,
                        position,
                    ));
                    if !original.span.is_empty() && !generated_span.is_empty() {
                        // emitter.ts wraps every object literal in the generic
                        // source-map pipeline. `emitObjectLiteralExpression`
                        // writes the closing brace through `emitList`, then
                        // `emitSourceMapsAfterNode` records the range end at
                        // the writer position immediately after that brace.
                        entries.push(TypeScriptSourceMapEntry::from_offsets(
                            self.generated,
                            generated_span.end,
                            self.source,
                            original.span.end,
                        ));
                    }
                    let (source_start_line, _) =
                        crate::source_map::offset_to_line_col(self.source, original.span.start);
                    let (source_end_line, _) =
                        crate::source_map::offset_to_line_col(self.source, original.span.end);
                    let (generated_start_line, _) =
                        crate::source_map::offset_to_line_col(self.generated, generated_span.start);
                    let (generated_end_line, _) =
                        crate::source_map::offset_to_line_col(self.generated, generated_span.end);
                    if source_start_line == source_end_line
                        && generated_start_line == generated_end_line
                        && original.span.end >= 2
                        && generated_span.end >= 2
                        && self
                            .generated
                            .as_bytes()
                            .get(generated_span.end.saturating_sub(2) as usize)
                            .is_some_and(u8::is_ascii_whitespace)
                        && self
                            .source
                            .as_bytes()
                            .get(original.span.end.saturating_sub(2) as usize)
                            .is_some_and(u8::is_ascii_whitespace)
                    {
                        entries.push(TypeScriptSourceMapEntry::from_offsets(
                            self.generated,
                            generated_span.end - 2,
                            self.source,
                            original.span.end - 2,
                        ));
                    }
                    continue;
                }
                TypeScriptSourceBoundaryKind::TemplateInterpolationClosure
                | TypeScriptSourceBoundaryKind::LineBrokenMember(_) => {
                    suppressed_generated_positions.insert(crate::source_map::offset_to_line_col(
                        self.generated,
                        generated_span.start,
                    ));
                    continue;
                }
                TypeScriptSourceBoundaryKind::IfStatementClosure
                | TypeScriptSourceBoundaryKind::TryStatementClosure => {
                    if original.span.is_empty() {
                        continue;
                    }
                    if !generated_span.is_empty() {
                        suppressed_generated_positions.insert(
                            crate::source_map::offset_to_line_col(
                                self.generated,
                                generated_span.end - 1,
                            ),
                        );
                        entries.push(TypeScriptSourceMapEntry::from_offsets(
                            self.generated,
                            generated_span.end,
                            self.source,
                            original.span.end,
                        ));
                    }
                    continue;
                }
                TypeScriptSourceBoundaryKind::ReturnStatement { has_argument: true } => {
                    self.map_statement_closure(original, &generated_boundary, &mut entries);
                    continue;
                }
                _ => {}
            }
            if original.span.is_empty()
                && !matches!(
                    original.kind,
                    TypeScriptSourceBoundaryKind::MethodClosure { .. }
                )
            {
                continue;
            }
            if matches!(
                original.kind,
                TypeScriptSourceBoundaryKind::MethodClosure { .. }
            ) {
                entries.push(TypeScriptSourceMapEntry::from_offsets(
                    self.generated,
                    generated_span.end,
                    self.source,
                    original.span.end,
                ));
                continue;
            }
            if matches!(
                original.kind,
                TypeScriptSourceBoundaryKind::ExpressionStatementClosure
            ) {
                self.map_statement_closure(original, &generated_boundary, &mut entries);
                continue;
            }
            if matches!(original.kind, TypeScriptSourceBoundaryKind::Property(_)) {
                self.map_updated_property_name(
                    original,
                    &generated_boundary,
                    &mut entries,
                    &mut suppressed_generated_positions,
                );
            }
            if matches!(original.kind, TypeScriptSourceBoundaryKind::Method { .. }) {
                if let Some(body) = generated_boundary.method_body_span {
                    suppressed_generated_positions.insert(crate::source_map::offset_to_line_col(
                        self.generated,
                        body.start,
                    ));
                }
            }
            if matches!(
                original.kind,
                TypeScriptSourceBoundaryKind::VariableDeclaration
            ) {
                self.map_declare_prefix(original, &generated_boundary, &mut entries);
                self.map_variable_closure(
                    original,
                    &generated_boundary,
                    &mut entries,
                    &mut suppressed_generated_positions,
                );
            }
            let source_offset = original.span.start;
            let generated_start =
                crate::source_map::offset_to_line_col(self.generated, generated_span.start);
            if !suppressed_generated_positions.contains(&generated_start) {
                entries.push(TypeScriptSourceMapEntry::from_offsets(
                    self.generated,
                    generated_span.start,
                    self.source,
                    source_offset,
                ));
            }
            if matches!(
                original.kind,
                TypeScriptSourceBoundaryKind::Method {
                    constructor: true,
                    ..
                }
            ) {
                suppressed_generated_positions.insert(crate::source_map::offset_to_line_col(
                    self.generated,
                    generated_span.end,
                ));
                continue;
            }
            let generated_end =
                crate::source_map::offset_to_line_col(self.generated, generated_span.end);
            if !suppressed_generated_positions.contains(&generated_end) {
                entries.push(TypeScriptSourceMapEntry::from_offsets(
                    self.generated,
                    generated_span.end,
                    self.source,
                    original.span.end,
                ));
            }
        }

        TypeScriptSourceBoundaryMappings {
            entries,
            suppressed_generated_positions,
        }
    }

    fn map_comment_boundaries(
        &self,
        generated_comments: &[Comment],
        entries: &mut Vec<TypeScriptSourceMapEntry>,
    ) {
        // TypeScript emitter.ts emits `emitPos(commentPos)`, writes the raw
        // comment range, then emits `emitPos(commentEnd)` for both leading and
        // trailing comments. Match comments by the same spelling identity used
        // by the layout pass, because transformed nodes may move a retained
        // source comment to a different generated position.
        let mut source_by_fingerprint =
            FxHashMap::<String, VecDeque<&HvigorSourceComment>>::with_capacity_and_hasher(
                self.source_comments.comments.len(),
                Default::default(),
            );
        for comment in &self.source_comments.comments {
            source_by_fingerprint
                .entry(comment.fingerprint.clone())
                .or_default()
                .push_back(comment);
        }
        for generated_comment in generated_comments {
            let Some(raw) =
                TypeScriptSourceCommentSnapshot::raw_comment(self.generated, generated_comment)
            else {
                continue;
            };
            let fingerprint = TypeScriptSourceCommentSnapshot::fingerprint(raw);
            let Some(source_comment) = source_by_fingerprint
                .get_mut(&fingerprint)
                .and_then(VecDeque::pop_front)
            else {
                continue;
            };
            for (generated_offset, source_offset) in [
                (generated_comment.span.start, source_comment.span.start),
                (generated_comment.span.end, source_comment.span.end),
            ] {
                entries.push(TypeScriptSourceMapEntry::from_offsets(
                    self.generated,
                    generated_offset,
                    self.source,
                    source_offset,
                ));
            }
        }
    }

    fn map_arrow_boundaries(
        &self,
        original: &TypeScriptSourceBoundary,
        generated: &TypeScriptSourceBoundary,
        entries: &mut Vec<TypeScriptSourceMapEntry>,
        suppressed: &mut FxHashSet<(u32, u32)>,
    ) {
        let Some(source_operator) = self.arrow_operator_offset(self.source, original.span) else {
            return;
        };
        let Some(generated_operator) = self.arrow_operator_offset(self.generated, generated.span)
        else {
            return;
        };
        let source_prefix = self
            .source
            .get(original.span.start as usize..source_operator as usize)
            .unwrap_or_default();
        let generated_prefix = self
            .generated
            .get(generated.span.start as usize..generated_operator as usize)
            .unwrap_or_default();
        let source_parameter = source_prefix
            .find('(')
            .map_or(original.span.start, |offset| {
                original.span.start + offset as u32
            });
        let generated_parameter = generated_prefix
            .find('(')
            .map_or(generated.span.start, |offset| {
                generated.span.start + offset as u32
            });
        for (generated_offset, source_offset) in [
            (generated_parameter, source_parameter),
            (generated_operator, source_operator),
            (generated_operator + 2, source_operator + 2),
        ] {
            entries.push(TypeScriptSourceMapEntry::from_offsets(
                self.generated,
                generated_offset,
                self.source,
                source_offset,
            ));
        }
        let generated_body = self
            .generated
            .get(generated_operator as usize + 2..generated.span.end as usize)
            .and_then(|tail| tail.find('{'))
            .map(|offset| generated_operator + 2 + offset as u32);
        if let Some(generated_body) = generated_body {
            suppressed.insert(crate::source_map::offset_to_line_col(
                self.generated,
                generated_body,
            ));
        }
        if !original.span.is_empty() && !generated.span.is_empty() {
            // `emitArrowFunction` is also surrounded by the generic source-map
            // pipeline. A block body writes `}` before
            // `emitSourceMapsAfterNode` records the ArrowFunction range end.
            entries.push(TypeScriptSourceMapEntry::from_offsets(
                self.generated,
                generated.span.end,
                self.source,
                original.span.end,
            ));
        }
        if !source_prefix.contains('(') && !generated_prefix.contains('(') {
            suppressed.insert(crate::source_map::offset_to_line_col(
                self.generated,
                generated_parameter + 1,
            ));
        }
    }

    fn arrow_operator_offset(&self, source: &str, span: Span) -> Option<u32> {
        source
            .get(span.start as usize..span.end as usize)?
            .find("=>")
            .map(|offset| span.start + offset as u32)
    }

    fn map_call_closure(
        &self,
        original: &TypeScriptSourceBoundary,
        generated: &TypeScriptSourceBoundary,
        entries: &mut Vec<TypeScriptSourceMapEntry>,
        suppressed: &mut FxHashSet<(u32, u32)>,
    ) {
        if original.span.is_empty() || generated.span.is_empty() {
            return;
        }
        if self
            .source
            .as_bytes()
            .get(original.span.end.saturating_sub(1) as usize)
            != Some(&b')')
            || self
                .generated
                .as_bytes()
                .get(generated.span.end.saturating_sub(1) as usize)
                != Some(&b')')
        {
            return;
        }
        let source_call = self
            .source
            .get(original.span.start as usize..original.span.end as usize)
            .unwrap_or_default();
        let generated_call = self
            .generated
            .get(generated.span.start as usize..generated.span.end as usize)
            .unwrap_or_default();
        if source_call.ends_with("()") && generated_call.ends_with("()") {
            entries.push(TypeScriptSourceMapEntry::from_offsets(
                self.generated,
                generated.span.end - 2,
                self.source,
                original.span.end - 2,
            ));
            entries.push(TypeScriptSourceMapEntry::from_offsets(
                self.generated,
                generated.span.end,
                self.source,
                original.span.end,
            ));
            suppressed.insert(crate::source_map::offset_to_line_col(
                self.generated,
                generated.span.end - 1,
            ));
            return;
        }
        // `process_module_files.ts::genContentAndSourceMapInfo` writes the
        // transformed SourceFile with TypeScript's printer. Its generic
        // `emitSourceMapsAfterNode` records every source-backed call's end;
        // `emitIfStatement` does not add a separate mapping for `)`.
        entries.push(TypeScriptSourceMapEntry::from_offsets(
            self.generated,
            generated.span.end,
            self.source,
            original.span.end,
        ));
    }

    fn map_statement_closure(
        &self,
        original: &TypeScriptSourceBoundary,
        generated: &TypeScriptSourceBoundary,
        entries: &mut Vec<TypeScriptSourceMapEntry>,
    ) {
        if original.span.is_empty() || generated.span.is_empty() {
            return;
        }
        let trailing_argument_end = self.statement_trailing_argument_end_boundary(original.span);
        let source_end_boundary = trailing_argument_end.unwrap_or_else(|| {
            if self
                .source
                .as_bytes()
                .get(original.span.end.saturating_sub(1) as usize)
                == Some(&b';')
            {
                original.span.end.saturating_sub(1)
            } else {
                original.span.end
            }
        });
        let generated_end_boundary = trailing_argument_end.map_or_else(
            || generated.span.end.saturating_sub(1),
            |_| self.statement_generated_end_boundary(generated.span),
        );
        entries.push(TypeScriptSourceMapEntry::from_offsets(
            self.generated,
            generated_end_boundary,
            self.source,
            source_end_boundary,
        ));
        if trailing_argument_end.is_some() {
            // The source and generated statement closures remain mapped in
            // addition to the last-argument anchor above. TypeScript emits
            // both tokens when it removes the trailing argument comma.
            entries.push(TypeScriptSourceMapEntry::from_offsets(
                self.generated,
                generated.span.end.saturating_sub(1),
                self.source,
                original.span.end.saturating_sub(1),
            ));
        }
        entries.push(TypeScriptSourceMapEntry::from_offsets(
            self.generated,
            generated.span.end,
            self.source,
            original.span.end,
        ));
    }

    fn statement_generated_end_boundary(&self, span: Span) -> u32 {
        let statement = self
            .generated
            .get(span.start as usize..span.end as usize)
            .unwrap_or_default();
        let without_semicolon = statement.strip_suffix(';').unwrap_or(statement);
        let trimmed = without_semicolon.trim_end();
        if trimmed.ends_with(')') {
            return span.start + trimmed.len().saturating_sub(1).min(u32::MAX as usize) as u32;
        }

        span.end.saturating_sub(1)
    }

    /// Return the TypeScript printer's source anchor for the generated closing
    /// token of an expression statement.
    ///
    /// For a multiline call with a trailing comma, TypeScript keeps
    /// `lastSourcePosition` at the end of the final argument.  The generated
    /// printer removes that trailing comma, so mapping the generated outer `)`
    /// to the source statement's outer `)` shifts debugger locations by one
    /// line.  This is observable in the stock EntryAbility template's
    /// `hilog.error(..., JSON.stringify(err),)` call.
    fn statement_trailing_argument_end_boundary(&self, span: Span) -> Option<u32> {
        let statement = self
            .source
            .get(span.start as usize..span.end as usize)
            .unwrap_or_default();
        let without_semicolon = statement.strip_suffix(';').unwrap_or(statement);
        let trimmed = without_semicolon.trim_end();
        if let Some(arguments) = trimmed.strip_suffix(')') {
            let before_closing_paren = arguments.trim_end();
            if let Some(before_trailing_comma) = before_closing_paren.strip_suffix(',') {
                return Some(
                    span.start
                        + before_trailing_comma
                            .trim_end()
                            .len()
                            .min(u32::MAX as usize) as u32,
                );
            }
        }
        None
    }

    fn map_variable_closure(
        &self,
        original: &TypeScriptSourceBoundary,
        generated: &TypeScriptSourceBoundary,
        entries: &mut Vec<TypeScriptSourceMapEntry>,
        suppressed: &mut FxHashSet<(u32, u32)>,
    ) {
        self.map_statement_closure(original, generated, entries);
        let source_text = self
            .source
            .get(original.span.start as usize..original.span.end as usize)
            .unwrap_or_default();
        let generated_text = self
            .generated
            .get(generated.span.start as usize..generated.span.end as usize)
            .unwrap_or_default();
        if source_text.ends_with("};") && generated_text.ends_with("};") {
            suppressed.insert(crate::source_map::offset_to_line_col(
                self.generated,
                generated.span.end - 2,
            ));
        }
    }

    fn map_function_boundaries(
        &self,
        original: &TypeScriptSourceBoundary,
        generated: &TypeScriptSourceBoundary,
        entries: &mut Vec<TypeScriptSourceMapEntry>,
        suppressed: &mut FxHashSet<(u32, u32)>,
    ) {
        if original.span.is_empty() || generated.span.is_empty() {
            return;
        }
        let source_text = self
            .source
            .get(original.span.start as usize..original.span.end as usize)
            .unwrap_or_default();
        let generated_text = self
            .generated
            .get(generated.span.start as usize..generated.span.end as usize)
            .unwrap_or_default();
        if source_text.starts_with("async ") && generated_text.starts_with("async ") {
            entries.push(TypeScriptSourceMapEntry::from_offsets(
                self.generated,
                generated.span.start + 5,
                self.source,
                original.span.start + 5,
            ));
        }
        if let Some(open_brace) = generated_text.find('{') {
            suppressed.insert(crate::source_map::offset_to_line_col(
                self.generated,
                generated.span.start + open_brace as u32,
            ));
        }
        entries.push(TypeScriptSourceMapEntry::from_offsets(
            self.generated,
            generated.span.end,
            self.source,
            original.span.end,
        ));
    }

    fn map_export_default_function(
        &self,
        original: &TypeScriptSourceBoundary,
        generated: &TypeScriptSourceBoundary,
        entries: &mut Vec<TypeScriptSourceMapEntry>,
        suppressed: &mut FxHashSet<(u32, u32)>,
    ) {
        const EXPORT_END: u32 = 6;
        const DEFAULT_START: u32 = 7;
        const DEFAULT_END: u32 = 14;
        for offset in [0, EXPORT_END, DEFAULT_START, DEFAULT_END] {
            entries.push(TypeScriptSourceMapEntry::from_offsets(
                self.generated,
                generated.span.start + offset,
                self.source,
                original.span.start + offset,
            ));
        }
        entries.push(TypeScriptSourceMapEntry::from_offsets(
            self.generated,
            generated.span.end,
            self.source,
            original.span.end,
        ));
        let generated_source = self
            .generated
            .get(generated.span.start as usize..generated.span.end as usize)
            .unwrap_or_default();
        for marker in ["function", "{"] {
            if let Some(offset) = generated_source.find(marker) {
                suppressed.insert(crate::source_map::offset_to_line_col(
                    self.generated,
                    generated.span.start + offset as u32,
                ));
            }
        }
    }

    fn map_export_default_class(
        &self,
        original: &TypeScriptSourceBoundary,
        generated: &TypeScriptSourceBoundary,
        entries: &mut Vec<TypeScriptSourceMapEntry>,
        suppressed: &mut FxHashSet<(u32, u32)>,
    ) {
        for offset in [0, 6, 7, 14] {
            entries.push(TypeScriptSourceMapEntry::from_offsets(
                self.generated,
                generated.span.start + offset,
                self.source,
                original.span.start + offset,
            ));
        }
        let original_text = self
            .source
            .get(original.span.start as usize..original.span.end as usize)
            .unwrap_or_default();
        let generated_text = self
            .generated
            .get(generated.span.start as usize..generated.span.end as usize)
            .unwrap_or_default();
        for heritage in ["extends", "implements"] {
            if let (Some(source_heritage), Some(generated_heritage)) =
                (original_text.find(heritage), generated_text.find(heritage))
            {
                suppressed.insert(crate::source_map::offset_to_line_col(
                    self.generated,
                    generated.span.start + generated_heritage.saturating_sub(1) as u32,
                ));
                entries.push(TypeScriptSourceMapEntry::from_offsets(
                    self.generated,
                    generated.span.start + generated_heritage.saturating_sub(1) as u32,
                    self.source,
                    original.span.start + source_heritage as u32,
                ));
                break;
            }
        }
        for marker in ["class", "{"] {
            if let Some(offset) = generated_text.find(marker) {
                suppressed.insert(crate::source_map::offset_to_line_col(
                    self.generated,
                    generated.span.start + offset as u32,
                ));
            }
        }
        if !generated.span.is_empty() {
            suppressed.insert(crate::source_map::offset_to_line_col(
                self.generated,
                generated.span.end - 1,
            ));
            entries.push(TypeScriptSourceMapEntry::from_offsets(
                self.generated,
                generated.span.end,
                self.source,
                original.span.end,
            ));
        }
    }

    fn map_updated_property_name(
        &self,
        original: &TypeScriptSourceBoundary,
        generated: &TypeScriptSourceBoundary,
        entries: &mut Vec<TypeScriptSourceMapEntry>,
        suppressed: &mut FxHashSet<(u32, u32)>,
    ) {
        let (Some(source_key), Some(generated_key)) =
            (original.property_key_span, generated.property_key_span)
        else {
            return;
        };
        if source_key.is_empty() || generated_key.is_empty() {
            return;
        }
        if source_key.end.saturating_sub(source_key.start)
            == generated_key.end.saturating_sub(generated_key.start)
        {
            return;
        }
        let source_anchor = source_key.start;
        let generated_anchor = generated_key.start.saturating_sub(1);
        let source_has_leading_space = self
            .source
            .as_bytes()
            .get(source_anchor as usize)
            .is_some_and(u8::is_ascii_whitespace);
        let generated_has_leading_space = self
            .generated
            .as_bytes()
            .get(generated_anchor as usize)
            .is_some_and(u8::is_ascii_whitespace);
        if !source_has_leading_space || !generated_has_leading_space {
            return;
        }

        entries.push(TypeScriptSourceMapEntry::from_offsets(
            self.generated,
            generated_anchor,
            self.source,
            source_anchor,
        ));
        suppressed.insert(crate::source_map::offset_to_line_col(
            self.generated,
            generated_key.start,
        ));
        suppressed.insert(crate::source_map::offset_to_line_col(
            self.generated,
            generated_key.end,
        ));
    }

    fn map_declare_prefix(
        &self,
        original: &TypeScriptSourceBoundary,
        generated: &TypeScriptSourceBoundary,
        entries: &mut Vec<TypeScriptSourceMapEntry>,
    ) {
        const DECLARE: &str = "declare";
        let source_start = original.span.start as usize;
        let generated_start = generated.span.start as usize;
        let source_tail = self.source.get(source_start..).unwrap_or_default();
        let generated_tail = self.generated.get(generated_start..).unwrap_or_default();
        if !source_tail.starts_with(DECLARE) || !generated_tail.starts_with(DECLARE) {
            return;
        }
        for offset in [DECLARE.len(), DECLARE.len() + 1] {
            entries.push(TypeScriptSourceMapEntry::from_offsets(
                self.generated,
                generated.span.start + offset as u32,
                self.source,
                original.span.start + offset as u32,
            ));
        }
    }
}

#[derive(Default)]
struct TypeScriptSourceBoundaryMappings {
    entries: Vec<TypeScriptSourceMapEntry>,
    suppressed_generated_positions: FxHashSet<(u32, u32)>,
}

struct TypeScriptSourceMapEntry {
    generated_line: u32,
    generated_col: u32,
    source_line: u32,
    source_col: u32,
}

impl TypeScriptSourceMapEntry {
    fn from_offsets(
        generated: &str,
        generated_offset: u32,
        source: &str,
        source_offset: u32,
    ) -> Self {
        let (generated_line, generated_col) =
            crate::source_map::offset_to_line_col(generated, generated_offset);
        let (source_line, source_col) =
            crate::source_map::offset_to_line_col(source, source_offset);
        Self {
            generated_line,
            generated_col,
            source_line,
            source_col,
        }
    }
}

#[derive(Default)]
struct NumericLiteralInventory {
    raw_literals: Vec<HvigorRawNumericLiteral>,
}

impl<'a> Visit<'a> for NumericLiteralInventory {
    fn visit_numeric_literal(&mut self, literal: &NumericLiteral<'a>) {
        let Some(raw) = literal.raw.as_ref() else {
            return;
        };
        let raw = raw.as_str();
        // TypeScript's printer keeps the raw spelling of every source-backed
        // numeric literal. That includes ordinary decimal spellings such as
        // `0.5` and `1000`, not only base-prefixed forms. OXC intentionally
        // chooses shorter equivalent spellings (`.5`, `1e3`), which changes
        // CompileArkTS cache text and shifts downstream debug positions.
        // Synthetic resource literals have an empty span and must retain the
        // explicit decimal spelling created by the transform as well. A raw
        // spelling is present at this point, so preserve every one.
        self.raw_literals.push(HvigorRawNumericLiteral {
            value: literal.value,
            raw: raw.to_string(),
        });
    }
}

struct HvigorRawNumericLiteral {
    value: f64,
    raw: String,
}

struct StringLiteralInventory<'a> {
    source: &'a str,
    literals: Vec<HvigorSourceStringLiteral>,
    synthetic_type_import_literals: VecDeque<HvigorSourceStringLiteral>,
}

impl<'a> StringLiteralInventory<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            literals: Vec::new(),
            synthetic_type_import_literals: VecDeque::new(),
        }
    }

    fn raw_literal(&self, literal: &StringLiteral<'_>) -> Option<String> {
        if literal.span.is_empty() {
            return None;
        }
        let raw = self
            .source
            .get(literal.span.start as usize..literal.span.end as usize)?;
        Self::quoted_raw(raw)
    }

    fn synthetic_type_import_literal(&self, literal: &StringLiteral<'_>) -> Option<String> {
        literal.raw.as_ref().and_then(|raw| Self::quoted_raw(raw))
    }

    fn quoted_raw(raw: &str) -> Option<String> {
        let bytes = raw.as_bytes();
        let quote = *bytes.first()?;
        (bytes.len() >= 2 && matches!(quote, b'\'' | b'"') && bytes.last().copied() == Some(quote))
            .then(|| raw.to_string())
    }

    fn take_synthetic_type_import_raw(&mut self, literal: &StringLiteral<'_>) -> Option<String> {
        let position = self
            .synthetic_type_import_literals
            .iter()
            .position(|candidate| literal.value == candidate.value)?;
        self.synthetic_type_import_literals
            .remove(position)
            .and_then(|candidate| candidate.raw)
    }
}

impl<'a> Visit<'a> for StringLiteralInventory<'_> {
    fn visit_import_declaration(&mut self, declaration: &ImportDeclaration<'a>) {
        // TypeOnlyNamedImportSplitter builds a fresh `import type` declaration
        // with `SPAN`, while preserving the source literal in `raw`. TypeScript
        // keeps that raw quote spelling. Other synthetic literal kinds (notably
        // `import lazy` after ArkUI lowering) are generated by etsTransform and
        // intentionally use the printer's double-quote style.
        if declaration.import_kind.is_type() && declaration.source.span.is_empty() {
            if let Some(raw) = self.synthetic_type_import_literal(&declaration.source) {
                self.synthetic_type_import_literals
                    .push_back(HvigorSourceStringLiteral {
                        value: declaration.source.value.to_string(),
                        raw: Some(raw),
                    });
            }
        }
        walk::walk_import_declaration(self, declaration);
    }

    fn visit_string_literal(&mut self, literal: &StringLiteral<'a>) {
        let raw = self
            .raw_literal(literal)
            .or_else(|| self.take_synthetic_type_import_raw(literal));
        self.literals.push(HvigorSourceStringLiteral {
            value: literal.value.to_string(),
            raw,
        });
    }
}

struct HvigorSourceStringLiteral {
    value: String,
    raw: Option<String>,
}

/// Raw source text for comments that OXC keeps at the correct AST location.
///
/// The OXC printer can normalize interior block-comment whitespace while the
/// TypeScript printer used by ets-loader retains the original text. We only
/// restore a comment after codegen emitted a comment with the same content;
/// comments attached to replaced ArkUI nodes remain deliberately untouched.
#[derive(Default)]
struct TypeScriptSourceCommentSnapshot {
    comments: Vec<HvigorSourceComment>,
}

struct HvigorSourceComment {
    fingerprint: String,
    raw: String,
    source_indent: usize,
    span: Span,
}

impl TypeScriptSourceCommentSnapshot {
    fn from_program(program: &Program<'_>) -> Self {
        let mut comments = Vec::with_capacity(program.comments.len());
        for comment in &program.comments {
            let Some(raw) = Self::raw_comment(program.source_text, comment) else {
                continue;
            };
            comments.push(HvigorSourceComment {
                fingerprint: Self::fingerprint(raw),
                raw: raw.to_string(),
                source_indent: Self::line_indent_width(program.source_text, comment.span.start),
                span: comment.span,
            });
        }
        Self { comments }
    }

    fn raw_comment<'a>(source: &'a str, comment: &Comment) -> Option<&'a str> {
        source.get(comment.span.start as usize..comment.span.end as usize)
    }

    fn fingerprint(raw: &str) -> String {
        let (kind, content) = if let Some(content) = raw.strip_prefix("//") {
            ("line", content)
        } else if let Some(content) = raw
            .strip_prefix("/*")
            .and_then(|content| content.strip_suffix("*/"))
        {
            ("block", content)
        } else {
            ("other", raw)
        };
        let mut fingerprint = String::from(kind);
        fingerprint.push(':');
        fingerprint.extend(content.split_whitespace());
        fingerprint
    }

    fn line_indent_width(source: &str, offset: u32) -> usize {
        let offset = offset as usize;
        let line_start = source[..offset.min(source.len())]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        source[line_start..offset.min(source.len())]
            .chars()
            .take_while(|character| character.is_whitespace())
            .count()
    }
}

impl HvigorSourceComment {
    fn text_at_generated_indent(&self, generated_indent: usize) -> String {
        let adjustment = generated_indent as isize - self.source_indent as isize;
        if adjustment == 0 || !self.raw.contains('\n') {
            return self.raw.clone();
        }

        let mut adjusted = String::with_capacity(
            self.raw
                .len()
                .saturating_add(adjustment.max(0) as usize * self.raw.lines().count()),
        );
        for (line_index, line) in self.raw.split_inclusive('\n').enumerate() {
            if line_index == 0 {
                adjusted.push_str(line);
                continue;
            }
            let prefix_end = line
                .find(|character: char| !matches!(character, ' ' | '\t'))
                .unwrap_or(line.len());
            if prefix_end == 0 {
                adjusted.push_str(line);
            } else if adjustment > 0 {
                adjusted.push_str(&" ".repeat(adjustment as usize));
                adjusted.push_str(line);
            } else {
                let mut remaining = (-adjustment) as usize;
                let mut cut = 0;
                for (offset, character) in line.char_indices() {
                    if remaining == 0 || !matches!(character, ' ' | '\t') {
                        break;
                    }
                    remaining -= 1;
                    cut = offset + character.len_utf8();
                }
                adjusted.push_str(&line[cut..]);
            }
        }
        adjusted
    }
}

/// Source-shape metadata that TypeScript's printer retains on original literal
/// nodes. `etsTransform` replaces resource-call children with synthetic nodes,
/// but the surrounding object and array nodes retain their original spans and
/// `multiLine`/trailing-comma behavior. OXC intentionally does not store that
/// printer metadata, so capture it before codegen and match it structurally
/// after reparsing OXC's output.
#[derive(Default)]
struct TypeScriptSourceLayoutInventory<'a> {
    source: &'a str,
    object_layouts: FxHashMap<HvigorObjectLayoutKey, VecDeque<HvigorCollectionLayout>>,
    array_layouts: FxHashMap<HvigorArrayLayoutKey, VecDeque<HvigorCollectionLayout>>,
    member_line_breaks: FxHashMap<String, VecDeque<bool>>,
    logical_line_breaks: FxHashMap<String, VecDeque<bool>>,
    single_arrow_parenthesized: VecDeque<bool>,
    empty_arrow_body_multiline: VecDeque<bool>,
    collection_depth: usize,
    collect_collections: bool,
}

impl<'a> TypeScriptSourceLayoutInventory<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            object_layouts: FxHashMap::default(),
            array_layouts: FxHashMap::default(),
            member_line_breaks: FxHashMap::default(),
            logical_line_breaks: FxHashMap::default(),
            single_arrow_parenthesized: VecDeque::new(),
            empty_arrow_body_multiline: VecDeque::new(),
            collection_depth: 0,
            collect_collections: true,
        }
    }

    fn from_program_source(program: &Program<'a>) -> HvigorSourceLayoutSnapshot {
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, program.source_text, program.source_type).parse();
        if parsed.panicked || !parsed.diagnostics.is_empty() {
            return HvigorSourceLayoutSnapshot::default();
        }
        let mut inventory = Self::new(program.source_text);
        inventory.visit_program(&parsed.program);
        // Collection layout is inherited structurally from original source
        // nodes. Chain/operator line breaks, however, survive only when the
        // transformed program retained the source-backed expression node;
        // ArkUI lowering creates synthetic member calls that must use the
        // TypeScript printer's generated layout instead.
        inventory.member_line_breaks.clear();
        inventory.logical_line_breaks.clear();
        inventory.single_arrow_parenthesized.clear();
        inventory.empty_arrow_body_multiline.clear();
        inventory.collect_collections = false;
        inventory.visit_program(program);
        inventory.into_snapshot()
    }

    fn collection_layout(
        &self,
        span: Span,
        item_spans: &[Span],
        opening: u8,
        closing: u8,
    ) -> Option<HvigorCollectionLayout> {
        if span.is_empty() {
            return None;
        }
        let source = self.source.get(span.start as usize..span.end as usize)?;
        if source.as_bytes().first().copied() != Some(opening)
            || source.as_bytes().last().copied() != Some(closing)
        {
            return None;
        }
        let first = item_spans.first().copied()?;
        let last = item_spans.last().copied()?;
        if first.start < span.start + 1 || last.end > span.end.saturating_sub(1) {
            return None;
        }
        Some(HvigorCollectionLayout {
            leading_newline: self.has_line_break(span.start + 1, first.start),
            separator_newlines: item_spans
                .windows(2)
                .map(|pair| self.has_line_break(pair[0].end, pair[1].start))
                .collect(),
            closing_newline: self.has_line_break(last.end, span.end - 1),
            trailing_comma: self
                .source
                .get(last.end as usize..span.end.saturating_sub(1) as usize)?
                .trim_end()
                .ends_with(','),
            collection_depth: self.collection_depth,
            explicit_same_name_properties: Vec::new(),
        })
    }

    fn has_line_break(&self, start: u32, end: u32) -> bool {
        self.source
            .get(start as usize..end as usize)
            .is_some_and(|source| source.contains('\n') || source.contains('\r'))
    }

    fn record_object(&mut self, object: &ObjectExpression<'_>) {
        let spans = object
            .properties
            .iter()
            .map(GetSpan::span)
            .collect::<Vec<_>>();
        let Some(mut layout) = self.collection_layout(object.span, &spans, b'{', b'}') else {
            return;
        };
        layout.explicit_same_name_properties = object
            .properties
            .iter()
            .map(|property| match property {
                ObjectPropertyKind::ObjectProperty(property) if !property.shorthand => {
                    property.key.static_name().is_some_and(|key| {
                        matches!(
                            property.value.without_parentheses(),
                            Expression::Identifier(identifier) if key == identifier.name
                        )
                    })
                }
                _ => false,
            })
            .collect();
        self.object_layouts
            .entry(HvigorObjectLayoutKey::from_object(object))
            .or_default()
            .push_back(layout);
    }

    fn record_array(&mut self, array: &ArrayExpression<'_>) {
        let spans = array.elements.iter().map(GetSpan::span).collect::<Vec<_>>();
        let Some(layout) = self.collection_layout(array.span, &spans, b'[', b']') else {
            return;
        };
        if !layout.requires_array_edit() {
            return;
        };
        self.array_layouts
            .entry(HvigorArrayLayoutKey::from_array(array))
            .or_default()
            .push_back(layout);
    }

    fn into_snapshot(self) -> HvigorSourceLayoutSnapshot {
        HvigorSourceLayoutSnapshot {
            object_layouts: self.object_layouts,
            array_layouts: self.array_layouts,
            member_line_breaks: self.member_line_breaks,
            logical_line_breaks: self.logical_line_breaks,
            single_arrow_parenthesized: self.single_arrow_parenthesized,
            empty_arrow_body_multiline: self.empty_arrow_body_multiline,
            synthetic_type_import_aliases: FxHashMap::default(),
        }
    }

    fn has_line_break_between(&self, left: Span, right: Span) -> bool {
        left.end <= right.start && self.has_line_break(left.end, right.start)
    }
}

impl<'a> Visit<'a> for TypeScriptSourceLayoutInventory<'a> {
    fn visit_object_expression(&mut self, object: &ObjectExpression<'a>) {
        if self.collect_collections {
            self.record_object(object);
        }
        self.collection_depth += 1;
        walk::walk_object_expression(self, object);
        self.collection_depth -= 1;
    }

    fn visit_array_expression(&mut self, array: &ArrayExpression<'a>) {
        if self.collect_collections {
            self.record_array(array);
        }
        self.collection_depth += 1;
        walk::walk_array_expression(self, array);
        self.collection_depth -= 1;
    }

    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'a>) {
        let has_line_break = matches!(
            member.object.without_parentheses(),
            Expression::CallExpression(_)
        ) && self
            .has_line_break_between(member.object.span(), member.property.span);
        self.member_line_breaks
            .entry(member.property.name.to_string())
            .or_default()
            .push_back(has_line_break);
        walk::walk_static_member_expression(self, member);
    }

    fn visit_logical_expression(&mut self, expression: &LogicalExpression<'a>) {
        let has_line_break =
            self.has_line_break_between(expression.left.span(), expression.right.span());
        self.logical_line_breaks
            .entry(expression.operator.as_str().to_string())
            .or_default()
            .push_back(has_line_break);
        walk::walk_logical_expression(self, expression);
    }

    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        if arrow.params.items.len() == 1
            && arrow.params.rest.is_none()
            && arrow.type_parameters.is_none()
            && arrow.return_type.is_none()
        {
            let parenthesized = arrow.span.is_empty()
                || self
                    .source
                    .get(arrow.span.start as usize..arrow.body.span().start as usize)
                    .and_then(|prefix| prefix.split_once("=>").map(|(params, _)| params.trim()))
                    .is_none_or(|params| params.starts_with('('));
            self.single_arrow_parenthesized.push_back(parenthesized);
        }
        if let oxc_ast::ast::ArrowFunctionBody::FunctionBody(body) = &arrow.body {
            if body.directives.is_empty() && body.statements.is_empty() {
                // TypeScript emitter.ts::shouldEmitBlockFunctionBodyOnSingleLine
                // keeps a non-synthesized function body multiline when its
                // retained source range crosses a line boundary.
                self.empty_arrow_body_multiline.push_back(
                    !body.span.is_empty() && self.has_line_break(body.span.start, body.span.end),
                );
            }
        }
        walk::walk_arrow_function_expression(self, arrow);
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct HvigorObjectLayoutKey(Vec<HvigorObjectPropertyLayoutKey>);

impl HvigorObjectLayoutKey {
    fn from_object(object: &ObjectExpression<'_>) -> Self {
        Self(
            object
                .properties
                .iter()
                .map(|property| match property {
                    ObjectPropertyKind::ObjectProperty(property) if !property.computed => property
                        .key
                        .static_name()
                        .map(|name| HvigorObjectPropertyLayoutKey::Named(name.into_owned()))
                        .unwrap_or(HvigorObjectPropertyLayoutKey::Computed),
                    ObjectPropertyKind::ObjectProperty(_) => {
                        HvigorObjectPropertyLayoutKey::Computed
                    }
                    ObjectPropertyKind::SpreadProperty(_) => HvigorObjectPropertyLayoutKey::Spread,
                })
                .collect(),
        )
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum HvigorObjectPropertyLayoutKey {
    Named(String),
    Computed,
    Spread,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct HvigorArrayLayoutKey(usize);

impl HvigorArrayLayoutKey {
    fn from_array(array: &ArrayExpression<'_>) -> Self {
        Self(array.elements.len())
    }
}

#[derive(Clone)]
struct HvigorCollectionLayout {
    leading_newline: bool,
    separator_newlines: Vec<bool>,
    closing_newline: bool,
    trailing_comma: bool,
    collection_depth: usize,
    explicit_same_name_properties: Vec<bool>,
}

impl HvigorCollectionLayout {
    fn requires_array_edit(&self) -> bool {
        self.leading_newline
            || self.separator_newlines.iter().any(|newline| *newline)
            || self.closing_newline
            || self.trailing_comma
    }
}

#[derive(Default)]
struct HvigorSourceLayoutSnapshot {
    object_layouts: FxHashMap<HvigorObjectLayoutKey, VecDeque<HvigorCollectionLayout>>,
    array_layouts: FxHashMap<HvigorArrayLayoutKey, VecDeque<HvigorCollectionLayout>>,
    member_line_breaks: FxHashMap<String, VecDeque<bool>>,
    logical_line_breaks: FxHashMap<String, VecDeque<bool>>,
    single_arrow_parenthesized: VecDeque<bool>,
    empty_arrow_body_multiline: VecDeque<bool>,
    synthetic_type_import_aliases: FxHashMap<String, VecDeque<Vec<String>>>,
}

impl HvigorSourceLayoutSnapshot {
    fn record_synthetic_type_import_aliases(&mut self, program: &Program<'_>) {
        for statement in &program.body {
            let Statement::ImportDeclaration(declaration) = statement else {
                continue;
            };
            let source = declaration.source.value.as_str();
            if !declaration.import_kind.is_type()
                || !(source.starts_with("@ohos:") || source.starts_with("@ohos."))
            {
                continue;
            }
            let aliases = declaration
                .specifiers
                .iter()
                .flatten()
                .filter_map(|specifier| {
                    let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else {
                        return None;
                    };
                    let imported = specifier.imported.name();
                    (imported.as_str() == specifier.local.name.as_str())
                        .then(|| imported.to_string())
                })
                .collect::<Vec<_>>();
            if !aliases.is_empty() {
                self.synthetic_type_import_aliases
                    .entry(declaration.source.value.to_string())
                    .or_default()
                    .push_back(aliases);
            }
        }
    }
}

struct HvigorTypescriptLayout<'a> {
    code: String,
    raw_numeric_literals: Vec<HvigorRawNumericLiteral>,
    source_string_literals: Vec<HvigorSourceStringLiteral>,
    source_layouts: HvigorSourceLayoutSnapshot,
    source_comments: &'a TypeScriptSourceCommentSnapshot,
}

impl<'a> HvigorTypescriptLayout<'a> {
    fn new(
        code: String,
        raw_numeric_literals: Vec<HvigorRawNumericLiteral>,
        source_string_literals: Vec<HvigorSourceStringLiteral>,
        source_layouts: HvigorSourceLayoutSnapshot,
        source_comments: &'a TypeScriptSourceCommentSnapshot,
    ) -> Self {
        Self {
            code,
            raw_numeric_literals,
            source_string_literals,
            source_layouts,
            source_comments,
        }
    }

    fn format(self) -> HvigorTypescriptLayoutOutput {
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &self.code, SourceType::ets()).parse();
        if parsed.panicked || !parsed.diagnostics.is_empty() {
            return HvigorTypescriptLayoutOutput {
                offset_map: HvigorLayoutOffsetMap::identity(&self.code),
                code: self.code,
            };
        }

        let mut layout = HvigorLayoutInventory::new(&self.code, self.source_layouts);
        layout.visit_program(&parsed.program);
        layout.add_source_comment_spelling_edits(self.source_comments, &parsed.program.comments);
        layout.add_numeric_literal_edits(&self.raw_numeric_literals);
        layout.add_string_literal_edits(&self.source_string_literals);
        layout.apply()
    }
}

struct HvigorTypescriptLayoutOutput {
    code: String,
    offset_map: HvigorLayoutOffsetMap,
}

struct HvigorLayoutOffsetMap {
    original_line_offsets: Vec<usize>,
    formatted: String,
    edits: Vec<HvigorAppliedTextEdit>,
}

impl HvigorLayoutOffsetMap {
    fn identity(source: &str) -> Self {
        Self {
            original_line_offsets: Self::line_offsets(source),
            formatted: source.to_string(),
            edits: Vec::new(),
        }
    }

    fn new(original: &str, formatted: String, edits: Vec<HvigorAppliedTextEdit>) -> Self {
        Self {
            original_line_offsets: Self::line_offsets(original),
            formatted,
            edits,
        }
    }

    fn remap_line_col(&self, line: u32, col: u32) -> (u32, u32) {
        let Some(line_start) = self.original_line_offsets.get(line as usize).copied() else {
            return (line, col);
        };
        let original_offset = line_start.saturating_add(col as usize);
        let formatted_offset = self.remap_offset(original_offset);
        crate::source_map::offset_to_line_col(&self.formatted, formatted_offset as u32)
    }

    fn remap_offset(&self, original_offset: usize) -> usize {
        let mut delta = 0isize;
        for edit in &self.edits {
            let start = edit.span.start as usize;
            let end = edit.span.end as usize;
            if original_offset < start {
                break;
            }
            if original_offset <= end {
                return (start as isize + delta + edit.remap_inside(original_offset - start)).max(0)
                    as usize;
            }
            delta += edit.replacement.len() as isize - (end - start) as isize;
        }
        (original_offset as isize + delta).max(0) as usize
    }

    fn line_offsets(source: &str) -> Vec<usize> {
        let mut offsets = vec![0];
        offsets.extend(
            source
                .bytes()
                .enumerate()
                .filter_map(|(index, byte)| (byte == b'\n').then_some(index + 1)),
        );
        offsets
    }
}

struct HvigorAppliedTextEdit {
    span: Span,
    original: String,
    replacement: String,
}

impl HvigorAppliedTextEdit {
    fn remap_inside(&self, original_offset: usize) -> isize {
        if original_offset >= self.original.len() {
            return self.replacement.len() as isize;
        }

        let rank = self.original.as_bytes()[..original_offset]
            .iter()
            .filter(|byte| !byte.is_ascii_whitespace())
            .count();
        let original_non_whitespace = self
            .original
            .bytes()
            .filter(|byte| !byte.is_ascii_whitespace())
            .collect::<Vec<_>>();
        let replacement_non_whitespace = self
            .replacement
            .bytes()
            .filter(|byte| !byte.is_ascii_whitespace())
            .collect::<Vec<_>>();
        if original_non_whitespace == replacement_non_whitespace {
            if rank == replacement_non_whitespace.len() {
                return self.replacement.len() as isize;
            }
            if let Some((index, _)) = self
                .replacement
                .bytes()
                .enumerate()
                .filter(|(_, byte)| !byte.is_ascii_whitespace())
                .nth(rank)
            {
                return index as isize;
            }
        }

        original_offset.min(self.replacement.len()) as isize
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HvigorTextEditKind {
    Object,
    Other,
}

struct HvigorTextEdit {
    span: Span,
    replacement: String,
    kind: HvigorTextEditKind,
}

struct HvigorLayoutInventory<'a> {
    source: &'a str,
    edits: Vec<HvigorTextEdit>,
    numeric_literals: Vec<(Span, f64)>,
    string_literals: Vec<(Span, String)>,
    object_layouts: FxHashMap<HvigorObjectLayoutKey, VecDeque<HvigorCollectionLayout>>,
    array_layouts: FxHashMap<HvigorArrayLayoutKey, VecDeque<HvigorCollectionLayout>>,
    member_line_breaks: FxHashMap<String, VecDeque<bool>>,
    logical_line_breaks: FxHashMap<String, VecDeque<bool>>,
    single_arrow_parenthesized: VecDeque<bool>,
    empty_arrow_body_multiline: VecDeque<bool>,
    synthetic_type_import_aliases: FxHashMap<String, VecDeque<Vec<String>>>,
}

impl<'a> HvigorLayoutInventory<'a> {
    fn new(source: &'a str, source_layouts: HvigorSourceLayoutSnapshot) -> Self {
        Self {
            source,
            edits: Vec::new(),
            numeric_literals: Vec::new(),
            string_literals: Vec::new(),
            object_layouts: source_layouts.object_layouts,
            array_layouts: source_layouts.array_layouts,
            member_line_breaks: source_layouts.member_line_breaks,
            logical_line_breaks: source_layouts.logical_line_breaks,
            single_arrow_parenthesized: source_layouts.single_arrow_parenthesized,
            empty_arrow_body_multiline: source_layouts.empty_arrow_body_multiline,
            synthetic_type_import_aliases: source_layouts.synthetic_type_import_aliases,
        }
    }

    fn add_numeric_literal_edits(&mut self, raw_literals: &[HvigorRawNumericLiteral]) {
        let mut output_index = 0;
        for raw_literal in raw_literals {
            let Some(relative_index) = self.numeric_literals[output_index..]
                .iter()
                .position(|(_, value)| *value == raw_literal.value)
            else {
                continue;
            };
            output_index += relative_index;
            let span = self.numeric_literals[output_index].0;
            output_index += 1;
            let Some(current) = self.source.get(span.start as usize..span.end as usize) else {
                continue;
            };
            if current != raw_literal.raw {
                self.edits.push(HvigorTextEdit {
                    span,
                    replacement: raw_literal.raw.clone(),
                    kind: HvigorTextEditKind::Other,
                });
            }
        }
    }

    fn add_string_literal_edits(&mut self, source_literals: &[HvigorSourceStringLiteral]) {
        let mut output_index = 0;
        for source_literal in source_literals {
            let Some(relative_index) = self.string_literals[output_index..]
                .iter()
                .position(|(_, generated_value)| source_literal.value == *generated_value)
            else {
                continue;
            };
            output_index += relative_index;
            let (span, _) = &self.string_literals[output_index];
            output_index += 1;
            let Some(raw) = source_literal.raw.as_ref() else {
                continue;
            };
            let Some(current) = self.source.get(span.start as usize..span.end as usize) else {
                continue;
            };
            if current != raw {
                self.edits.push(HvigorTextEdit {
                    span: *span,
                    replacement: raw.clone(),
                    kind: HvigorTextEditKind::Other,
                });
            }
        }
    }

    fn add_source_comment_spelling_edits(
        &mut self,
        source_comments: &TypeScriptSourceCommentSnapshot,
        generated_comments: &[Comment],
    ) {
        let mut source_by_fingerprint =
            FxHashMap::<String, VecDeque<&HvigorSourceComment>>::with_capacity_and_hasher(
                source_comments.comments.len(),
                Default::default(),
            );
        for comment in &source_comments.comments {
            source_by_fingerprint
                .entry(comment.fingerprint.clone())
                .or_default()
                .push_back(comment);
        }
        for comment in generated_comments {
            let Some(current) = TypeScriptSourceCommentSnapshot::raw_comment(self.source, comment)
            else {
                continue;
            };
            let fingerprint = TypeScriptSourceCommentSnapshot::fingerprint(current);
            let Some(source_comment) = source_by_fingerprint
                .get_mut(&fingerprint)
                .and_then(VecDeque::pop_front)
            else {
                continue;
            };
            let generated_indent =
                TypeScriptSourceCommentSnapshot::line_indent_width(self.source, comment.span.start);
            let raw = source_comment.text_at_generated_indent(generated_indent);
            if current != raw {
                self.edits.push(HvigorTextEdit {
                    span: comment.span,
                    replacement: raw,
                    kind: HvigorTextEditKind::Other,
                });
            }
        }
    }

    fn line_indent_at(&self, offset: usize) -> &str {
        let line_start = self.source[..offset]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        self.source[line_start..offset]
            .split_at(
                self.source[line_start..offset]
                    .find(|character: char| !character.is_whitespace())
                    .unwrap_or(offset - line_start),
            )
            .0
    }

    fn source_object_layout(
        &mut self,
        object: &ObjectExpression<'_>,
    ) -> Option<HvigorCollectionLayout> {
        self.object_layouts
            .get_mut(&HvigorObjectLayoutKey::from_object(object))
            .and_then(VecDeque::pop_front)
    }

    fn source_array_layout(
        &mut self,
        array: &ArrayExpression<'_>,
    ) -> Option<HvigorCollectionLayout> {
        self.array_layouts
            .get_mut(&HvigorArrayLayoutKey::from_array(array))
            .and_then(VecDeque::pop_front)
    }

    fn source_member_has_line_break(&mut self, member: &StaticMemberExpression<'_>) -> bool {
        self.member_line_breaks
            .get_mut(member.property.name.as_str())
            .and_then(VecDeque::pop_front)
            .unwrap_or(false)
    }

    fn source_logical_has_line_break(&mut self, expression: &LogicalExpression<'_>) -> bool {
        self.logical_line_breaks
            .get_mut(expression.operator.as_str())
            .and_then(VecDeque::pop_front)
            .unwrap_or(false)
    }

    fn add_explicit_same_name_property_edits(
        &mut self,
        object: &ObjectExpression<'_>,
        layout: &HvigorCollectionLayout,
    ) {
        for (property, explicit) in object
            .properties
            .iter()
            .zip(&layout.explicit_same_name_properties)
        {
            if !explicit {
                continue;
            }
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                continue;
            };
            if !property.shorthand {
                continue;
            }
            let Some(name) = property.key.static_name() else {
                continue;
            };
            self.edits.push(HvigorTextEdit {
                span: Span::new(property.span.end, property.span.end),
                replacement: format!(": {name}"),
                kind: HvigorTextEditKind::Other,
            });
        }
    }

    fn can_compact_object(&self, object: &ObjectExpression<'_>) -> bool {
        object.properties.len() > 1
            && object.properties.iter().all(|property| {
                self.source
                    .get(property.span().start as usize..property.span().end as usize)
                    .is_some_and(|source| !source.contains('\n') && !source.contains('\r'))
            })
    }

    /// Apply TypeScript's source-derived literal-list boundaries with narrow
    /// punctuation-span edits. Keeping the edits outside child spans lets a
    /// source object and its nested source array preserve their layouts in the
    /// same pass, just as the SDK printer does.
    fn add_collection_layout_edits(
        &mut self,
        span: Span,
        item_spans: Vec<Span>,
        layout: HvigorCollectionLayout,
        opening: u8,
        closing: u8,
    ) {
        let Some(first) = item_spans.first().copied() else {
            return;
        };
        let Some(last) = item_spans.last().copied() else {
            return;
        };
        let start = span.start as usize;
        let end = span.end as usize;
        if start >= end
            || self.source.as_bytes().get(start).copied() != Some(opening)
            || self.source.as_bytes().get(end.saturating_sub(1)).copied() != Some(closing)
            || first.start < span.start + 1
            || last.end > span.end.saturating_sub(1)
        {
            return;
        }

        let indent = self.line_indent_at(start).to_string();
        let item_indent = format!("{indent}{}", "    ".repeat(layout.collection_depth + 1));
        let has_object_brace_spacing = opening == b'{';
        let prefix = if layout.leading_newline {
            format!("\n{item_indent}")
        } else if has_object_brace_spacing {
            " ".to_string()
        } else {
            String::new()
        };
        self.edits.push(HvigorTextEdit {
            span: Span::new(span.start + 1, first.start),
            replacement: prefix,
            kind: HvigorTextEditKind::Object,
        });
        for (index, pair) in item_spans.windows(2).enumerate() {
            let [previous, next] = pair else {
                continue;
            };
            if previous.end > next.start {
                continue;
            }
            let separator = if layout
                .separator_newlines
                .get(index)
                .copied()
                .unwrap_or(false)
            {
                format!(",\n{item_indent}")
            } else {
                ", ".to_string()
            };
            self.edits.push(HvigorTextEdit {
                span: Span::new(previous.end, next.start),
                replacement: separator,
                kind: HvigorTextEditKind::Object,
            });
        }
        let suffix = if layout.closing_newline {
            format!("{}\n{indent}", if layout.trailing_comma { "," } else { "" })
        } else if has_object_brace_spacing {
            format!("{} ", if layout.trailing_comma { "," } else { "" })
        } else {
            if layout.trailing_comma {
                ",".to_string()
            } else {
                String::new()
            }
        };
        self.edits.push(HvigorTextEdit {
            span: Span::new(last.end, span.end - 1),
            replacement: suffix,
            kind: HvigorTextEditKind::Object,
        });
    }

    fn apply(mut self) -> HvigorTypescriptLayoutOutput {
        let protected = self
            .edits
            .iter()
            .filter(|edit| edit.kind == HvigorTextEditKind::Other)
            .map(|edit| edit.span)
            .collect::<Vec<_>>();
        self.edits.retain(|edit| {
            edit.kind != HvigorTextEditKind::Object
                || !protected
                    .iter()
                    .any(|span| edit.span.start <= span.start && span.end <= edit.span.end)
        });
        self.edits
            .sort_by_key(|edit| (edit.span.start, edit.span.end));

        let applied_edits = self
            .edits
            .iter()
            .filter_map(|edit| {
                let original = self
                    .source
                    .get(edit.span.start as usize..edit.span.end as usize)?;
                Some(HvigorAppliedTextEdit {
                    span: edit.span,
                    original: original.to_string(),
                    replacement: edit.replacement.clone(),
                })
            })
            .collect::<Vec<_>>();

        let mut code = self.source.to_string();
        let mut last_start = code.len();
        for edit in self.edits.into_iter().rev() {
            let start = edit.span.start as usize;
            let end = edit.span.end as usize;
            if start > end || end > last_start {
                continue;
            }
            code.replace_range(start..end, &edit.replacement);
            last_start = start;
        }
        HvigorTypescriptLayoutOutput {
            offset_map: HvigorLayoutOffsetMap::new(self.source, code.clone(), applied_edits),
            code,
        }
    }
}

impl<'a> Visit<'a> for HvigorLayoutInventory<'_> {
    fn visit_numeric_literal(&mut self, literal: &NumericLiteral<'a>) {
        self.numeric_literals.push((literal.span, literal.value));
    }

    fn visit_string_literal(&mut self, literal: &StringLiteral<'a>) {
        self.string_literals
            .push((literal.span, literal.value.to_string()));
    }

    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        if arrow.params.items.len() == 1
            && arrow.params.rest.is_none()
            && arrow.type_parameters.is_none()
            && arrow.return_type.is_none()
            && !self.single_arrow_parenthesized.pop_front().unwrap_or(true)
        {
            let parameter = &arrow.params.items[0];
            if parameter.decorators.is_empty()
                && !parameter.has_modifier()
                && parameter.pattern.is_binding_identifier()
                && parameter.type_annotation.is_none()
                && parameter.initializer.is_none()
                && !parameter.optional
            {
                let span = arrow.params.span;
                if let Some(current) = self.source.get(span.start as usize..span.end as usize) {
                    if let Some(inner) = current
                        .strip_prefix('(')
                        .and_then(|current| current.strip_suffix(')'))
                    {
                        self.edits.push(HvigorTextEdit {
                            span,
                            replacement: inner.to_string(),
                            kind: HvigorTextEditKind::Other,
                        });
                    }
                }
            }
        }
        // Expression-bodied arrows cannot be empty; only a block body with no
        // directives/statements renders as `{ }`.
        if let oxc_ast::ast::ArrowFunctionBody::FunctionBody(body) = &arrow.body {
            if body.directives.is_empty() && body.statements.is_empty() {
                let multiline = self.empty_arrow_body_multiline.pop_front().unwrap_or(false);
                let replacement = if multiline {
                    let indent = self.line_indent_at(arrow.span.start as usize);
                    format!("{{\n{indent}}}")
                } else {
                    "{ }".to_string()
                };
                self.edits.push(HvigorTextEdit {
                    span: body.span,
                    replacement,
                    kind: HvigorTextEditKind::Other,
                });
            }
        }
        walk::walk_arrow_function_expression(self, arrow);
    }

    fn visit_if_statement(&mut self, statement: &IfStatement<'a>) {
        if let Some(alternate) = &statement.alternate {
            let consequent_end = statement.consequent.span().end;
            let alternate_start = alternate.span().start;
            let between = self
                .source
                .get(consequent_end as usize..alternate_start as usize);
            if between.is_some_and(|between| between.contains("else")) {
                let indent = self.line_indent_at(statement.span.start as usize);
                self.edits.push(HvigorTextEdit {
                    span: Span::new(consequent_end, alternate_start),
                    replacement: format!("\n{indent}else "),
                    kind: HvigorTextEditKind::Other,
                });
            }
        }
        walk::walk_if_statement(self, statement);
    }

    fn visit_method_definition(&mut self, method: &MethodDefinition<'a>) {
        if let Some(body) = &method.value.body {
            if body.is_empty() {
                let indent = self.line_indent_at(method.span.start as usize);
                self.edits.push(HvigorTextEdit {
                    span: body.span,
                    replacement: format!("{{\n{indent}}}"),
                    kind: HvigorTextEditKind::Other,
                });
            }
        }
        walk::walk_method_definition(self, method);
    }

    fn visit_object_expression(&mut self, object: &ObjectExpression<'a>) {
        if let Some(layout) = self.source_object_layout(object) {
            self.add_explicit_same_name_property_edits(object, &layout);
            self.add_collection_layout_edits(
                object.span,
                object.properties.iter().map(GetSpan::span).collect(),
                layout,
                b'{',
                b'}',
            );
        } else if self.can_compact_object(object) {
            self.add_collection_layout_edits(
                object.span,
                object.properties.iter().map(GetSpan::span).collect(),
                HvigorCollectionLayout {
                    leading_newline: false,
                    separator_newlines: vec![false; object.properties.len().saturating_sub(1)],
                    closing_newline: false,
                    trailing_comma: false,
                    collection_depth: 0,
                    explicit_same_name_properties: Vec::new(),
                },
                b'{',
                b'}',
            );
        }
        walk::walk_object_expression(self, object);
    }

    fn visit_array_expression(&mut self, array: &ArrayExpression<'a>) {
        if !array
            .elements
            .iter()
            .any(|element| matches!(element, ArrayExpressionElement::Elision(_)))
        {
            if let Some(layout) = self.source_array_layout(array) {
                self.add_collection_layout_edits(
                    array.span,
                    array.elements.iter().map(GetSpan::span).collect(),
                    layout,
                    b'[',
                    b']',
                );
            }
        }
        walk::walk_array_expression(self, array);
    }

    fn visit_try_statement(&mut self, statement: &TryStatement<'a>) {
        if let Some(handler) = &statement.handler {
            let indent = self.line_indent_at(statement.span.start as usize);
            let start = statement.block.span.end;
            let end = handler.span.start;
            if start <= end {
                self.edits.push(HvigorTextEdit {
                    span: Span::new(start, end),
                    replacement: format!("\n{indent}"),
                    kind: HvigorTextEditKind::Other,
                });
            }
        }
        walk::walk_try_statement(self, statement);
    }

    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'a>) {
        if self.source_member_has_line_break(member) {
            let start = member.object.span().end;
            let end = member.property.span.start;
            if start <= end {
                let indent = self.line_indent_at(member.span.start as usize);
                let separator = self
                    .source
                    .get(start as usize..end as usize)
                    .unwrap_or(".")
                    .trim();
                self.edits.push(HvigorTextEdit {
                    span: Span::new(start, end),
                    replacement: format!("\n{indent}    {separator}"),
                    kind: HvigorTextEditKind::Other,
                });
            }
        }
        walk::walk_static_member_expression(self, member);
    }

    fn visit_logical_expression(&mut self, expression: &LogicalExpression<'a>) {
        if self.source_logical_has_line_break(expression) {
            let start = expression.left.span().end;
            let end = expression.right.span().start;
            if start <= end {
                let indent = self.line_indent_at(expression.span.start as usize);
                self.edits.push(HvigorTextEdit {
                    span: Span::new(start, end),
                    replacement: format!("\n{indent}    {} ", expression.operator.as_str()),
                    kind: HvigorTextEditKind::Other,
                });
            }
        }
        walk::walk_logical_expression(self, expression);
    }

    fn visit_import_declaration(&mut self, declaration: &ImportDeclaration<'a>) {
        let aliases = self
            .synthetic_type_import_aliases
            .get_mut(declaration.source.value.as_str())
            .and_then(VecDeque::pop_front)
            .unwrap_or_default();
        if !aliases.is_empty() {
            for specifier in declaration.specifiers.iter().flatten() {
                let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else {
                    continue;
                };
                let imported = specifier.imported.name();
                if aliases.iter().any(|alias| alias == imported.as_str())
                    && imported.as_str() == specifier.local.name.as_str()
                {
                    self.edits.push(HvigorTextEdit {
                        span: Span::new(specifier.span.end, specifier.span.end),
                        replacement: format!(" as {imported}"),
                        kind: HvigorTextEditKind::Other,
                    });
                }
            }
        }
        walk::walk_import_declaration(self, declaration);
    }
}

pub(crate) struct CodegenSourceText {
    code: String,
}

impl CodegenSourceText {
    pub(crate) fn new(code: String) -> Self {
        Self { code }
    }

    pub(crate) fn without_trailing_newline(mut self) -> String {
        if self.code.ends_with('\n') {
            self.code.pop();
        }
        self.code
    }
}
