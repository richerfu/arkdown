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
    BinaryExpression, BindingIdentifier, Comment, Expression, ExpressionStatement, IdentifierName,
    IdentifierReference, IfStatement, ImportDeclaration, MethodDefinition, MethodDefinitionKind,
    NumericLiteral, ObjectExpression, ObjectPropertyKind, Program, PropertyDefinition, PropertyKey,
    StringLiteral, TSType, ThisExpression,
};
use oxc_ast_visit::{walk, Visit};
use oxc_codegen::{Codegen, CodegenOptions, CommentOptions};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType, Span};
use rolldown_sourcemap::SourceMapBuilder;
use rustc_hash::FxHashMap;

pub(crate) struct HvigorTypescriptPrinter;

impl HvigorTypescriptPrinter {
    pub(crate) fn print(program: &Program<'_>, options: CodegenOptions) -> String {
        Self::print_internal(program, options, None).code
    }

    pub(crate) fn print_with_source_map(
        program: &Program<'_>,
        options: CodegenOptions,
        source_text: &str,
    ) -> HvigorTypescriptPrintOutput {
        Self::print_internal(program, options, Some(source_text))
    }

    fn print_internal(
        program: &Program<'_>,
        options: CodegenOptions,
        source_text: Option<&str>,
    ) -> HvigorTypescriptPrintOutput {
        let mut numeric_literals = NumericLiteralInventory::default();
        numeric_literals.visit_program(program);
        let mut string_literals = StringLiteralInventory::new(program.source_text);
        string_literals.visit_program(program);
        let mut original_boundaries =
            TypeScriptSourceBoundaryInventory::with_source_text(program.source_text);
        original_boundaries.visit_program(program);
        let source_comments = if options.comments == CommentOptions::disabled() {
            TypeScriptSourceCommentSnapshot::default()
        } else {
            TypeScriptSourceCommentSnapshot::from_program(program)
        };
        let source_layouts = TypeScriptSourceLayoutInventory::from_program_source(program);
        let generated = Codegen::new().with_options(options).build(program);
        let layout = HvigorTypescriptLayout::new(
            generated.code,
            numeric_literals.raw_literals,
            string_literals.literals,
            source_layouts,
            source_comments,
        )
        .format();
        let source_map = generated.map.map(|source_map| {
            let supplemental = source_text
                .map(|source_text| {
                    TypeScriptSourceBoundaryMapper::new(
                        source_text,
                        &layout.code,
                        original_boundaries.boundaries.as_slice(),
                    )
                    .entries()
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
                .chain(supplemental.into_iter().map(|entry| HvigorSourceMapToken {
                    dst_line: entry.generated_line,
                    dst_col: entry.generated_col,
                    src_line: entry.source_line,
                    src_col: entry.source_col,
                    source_id: Some(0),
                    name_id: None,
                    supplemental: true,
                }))
                .collect::<Vec<_>>();
            tokens.sort_by_key(|token| {
                (
                    token.dst_line,
                    token.dst_col,
                    !token.supplemental,
                    token.src_line,
                    token.src_col,
                )
            });
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

pub(crate) struct HvigorTypescriptPrintOutput {
    pub(crate) code: String,
    pub(crate) source_map: Option<String>,
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
    Property(String),
    Operator(String),
    /// ArkUI component block closure: source components (`Column() { ... }`)
    /// map to their generated `Column.pop()` calls (pop order is LIFO).
    ComponentClosure(String),
}

#[derive(Clone)]
struct TypeScriptSourceBoundary {
    kind: TypeScriptSourceBoundaryKind,
    span: Span,
}

struct TypeScriptSourceBoundaryInventory<'a> {
    boundaries: Vec<TypeScriptSourceBoundary>,
    source_text: Option<&'a str>,
}

impl<'a> TypeScriptSourceBoundaryInventory<'a> {
    fn with_source_text(source: &'a str) -> Self {
        Self {
            boundaries: Vec::new(),
            source_text: Some(source),
        }
    }
}

impl<'a> TypeScriptSourceBoundaryInventory<'a> {
    fn push(&mut self, kind: TypeScriptSourceBoundaryKind, span: Span) {
        self.boundaries
            .push(TypeScriptSourceBoundary { kind, span });
    }

    fn property_name(key: &PropertyKey<'_>) -> String {
        key.static_name()
            .map(|name| name.into_owned())
            .unwrap_or_default()
    }
}

impl<'a> Visit<'a> for TypeScriptSourceBoundaryInventory<'a> {
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
        self.push(
            TypeScriptSourceBoundaryKind::Method {
                name: Self::property_name(&method.key),
                constructor: method.kind == MethodDefinitionKind::Constructor,
            },
            // TypeScript anchors method mappings at the property key, skipping
            // `get`/`set`/`static`/accessibility keywords.
            method.key.span(),
        );
        walk::walk_method_definition(self, method);
    }

    fn visit_property_definition(&mut self, property: &PropertyDefinition<'a>) {
        self.push(
            TypeScriptSourceBoundaryKind::Property(Self::property_name(&property.key)),
            // TypeScript maps the whole property declaration (including the
            // `private`/`readonly` keywords) from the source property start;
            // the trailing `;` maps to the source semicolon (span end).
            Span::new(property.span.start, property.span.end),
        );
        walk::walk_property_definition(self, property);
    }

    fn visit_ark_ui_component_expression(&mut self, component: &ArkUIComponentExpression<'a>) {
        let callee_name = match &component.callee {
            Expression::Identifier(identifier) => Some(identifier.name.as_str()),
            _ => None,
        };
        if let Some(name) = callee_name {
            self.push(
                TypeScriptSourceBoundaryKind::ComponentClosure(name.to_string()),
                component.span,
            );
        }
        walk::walk_ark_ui_component_expression(self, component);
    }

    /// Generated `Column.pop()` calls close the corresponding source
    /// component block; they are collected as component closures (matched
    /// LIFO because pop order mirrors the reverse of the source nesting).
    fn visit_expression_statement(&mut self, stmt: &ExpressionStatement<'a>) {
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
                            stmt.span,
                        );
                    }
                    // The component name inside a pop call maps to the block
                    // closure, not to the source component identifier — do
                    // not walk the callee.
                    return;
                }
            }
        }
        walk::walk_expression_statement(self, stmt);
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
}

struct TypeScriptSourceBoundaryMapper<'a> {
    source: &'a str,
    generated: &'a str,
    original: &'a [TypeScriptSourceBoundary],
}

impl<'a> TypeScriptSourceBoundaryMapper<'a> {
    fn new(source: &'a str, generated: &'a str, original: &'a [TypeScriptSourceBoundary]) -> Self {
        Self {
            source,
            generated,
            original,
        }
    }

    fn entries(&self) -> Vec<TypeScriptSourceMapEntry> {
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, self.generated, SourceType::ets()).parse();
        if parsed.panicked || !parsed.diagnostics.is_empty() {
            return Vec::new();
        }
        let mut generated =
            TypeScriptSourceBoundaryInventory::with_source_text(parsed.program.source_text);
        generated.visit_program(&parsed.program);
        let mut generated_by_kind = HashMap::<TypeScriptSourceBoundaryKind, VecDeque<Span>>::new();
        for boundary in generated.boundaries {
            generated_by_kind
                .entry(boundary.kind)
                .or_default()
                .push_back(boundary.span);
        }

        let mut entries = Vec::new();
        for original in self.original {
            let generated_span = match &original.kind {
                // Component closures close in pop order (LIFO).
                TypeScriptSourceBoundaryKind::ComponentClosure(_) => generated_by_kind
                    .get_mut(&original.kind)
                    .and_then(VecDeque::pop_back),
                _ => generated_by_kind
                    .get_mut(&original.kind)
                    .and_then(VecDeque::pop_front),
            };
            let Some(generated_span) = generated_span else {
                continue;
            };
            if original.span.is_empty() {
                continue;
            }
            // Component closures map to the source block's closing brace.
            let source_offset = match &original.kind {
                TypeScriptSourceBoundaryKind::ComponentClosure(_) => original.span.end,
                _ => original.span.start,
            };
            entries.push(TypeScriptSourceMapEntry::from_offsets(
                self.generated,
                generated_span.start,
                self.source,
                source_offset,
            ));
            let source_end = match &original.kind {
                TypeScriptSourceBoundaryKind::Method {
                    constructor: true, ..
                } => original.span.start,
                _ => original.span.end,
            };
            entries.push(TypeScriptSourceMapEntry::from_offsets(
                self.generated,
                generated_span.end,
                self.source,
                source_end,
            ));
        }

        entries
    }
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
    collection_depth: usize,
}

impl<'a> TypeScriptSourceLayoutInventory<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            object_layouts: FxHashMap::default(),
            array_layouts: FxHashMap::default(),
            collection_depth: 0,
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
        let Some(layout) = self.collection_layout(object.span, &spans, b'{', b'}') else {
            return;
        };
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
        }
    }
}

impl<'a> Visit<'a> for TypeScriptSourceLayoutInventory<'a> {
    fn visit_object_expression(&mut self, object: &ObjectExpression<'a>) {
        self.record_object(object);
        self.collection_depth += 1;
        walk::walk_object_expression(self, object);
        self.collection_depth -= 1;
    }

    fn visit_array_expression(&mut self, array: &ArrayExpression<'a>) {
        self.record_array(array);
        self.collection_depth += 1;
        walk::walk_array_expression(self, array);
        self.collection_depth -= 1;
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
}

struct HvigorTypescriptLayout {
    code: String,
    raw_numeric_literals: Vec<HvigorRawNumericLiteral>,
    source_string_literals: Vec<HvigorSourceStringLiteral>,
    source_layouts: HvigorSourceLayoutSnapshot,
    source_comments: TypeScriptSourceCommentSnapshot,
}

impl HvigorTypescriptLayout {
    fn new(
        code: String,
        raw_numeric_literals: Vec<HvigorRawNumericLiteral>,
        source_string_literals: Vec<HvigorSourceStringLiteral>,
        source_layouts: HvigorSourceLayoutSnapshot,
        source_comments: TypeScriptSourceCommentSnapshot,
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
        layout.add_source_comment_spelling_edits(&self.source_comments, &parsed.program.comments);
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
        // Expression-bodied arrows cannot be empty; only a block body with no
        // directives/statements renders as `{ }`.
        if let oxc_ast::ast::ArrowFunctionBody::FunctionBody(body) = &arrow.body {
            if body.directives.is_empty() && body.statements.is_empty() {
                self.edits.push(HvigorTextEdit {
                    span: body.span,
                    replacement: "{ }".to_string(),
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
