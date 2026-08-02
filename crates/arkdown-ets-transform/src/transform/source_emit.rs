//! Source emit helpers for ArkTS transform lowering.
//!
//! Upstream ets-loader keeps original AST nodes as nodes and lets the TypeScript
//! printer preserve their source shape where possible. Centralize remaining
//! source text boundaries here while generated expression text is printed by OXC
//! codegen.

use std::collections::{HashMap, VecDeque};

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    ArrowFunctionExpression, BindingIdentifier, IdentifierName, IdentifierReference,
    MethodDefinition, MethodDefinitionKind, NumericLiteral, ObjectExpression, Program,
    PropertyDefinition, PropertyKey, StringLiteral, TSType, ThisExpression,
};
use oxc_ast_visit::{walk, Visit};
use oxc_codegen::{Codegen, CodegenOptions};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType, Span};
use rolldown_sourcemap::SourceMapBuilder;

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
        let mut original_boundaries = TypeScriptSourceBoundaryInventory::default();
        original_boundaries.visit_program(program);
        let generated = Codegen::new().with_options(options).build(program);
        let layout = HvigorTypescriptLayout::new(
            generated.code,
            numeric_literals.raw_literals,
            string_literals.literals,
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

#[derive(Clone, PartialEq, Eq, Hash)]
enum TypeScriptSourceBoundaryKind {
    Identifier(String),
    StringLiteral(String),
    NumericLiteral(u64),
    This,
    Type,
    Method { name: String, constructor: bool },
    Property(String),
}

#[derive(Clone)]
struct TypeScriptSourceBoundary {
    kind: TypeScriptSourceBoundaryKind,
    span: Span,
}

#[derive(Default)]
struct TypeScriptSourceBoundaryInventory {
    boundaries: Vec<TypeScriptSourceBoundary>,
}

impl TypeScriptSourceBoundaryInventory {
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

impl<'a> Visit<'a> for TypeScriptSourceBoundaryInventory {
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
            method.span,
        );
        walk::walk_method_definition(self, method);
    }

    fn visit_property_definition(&mut self, property: &PropertyDefinition<'a>) {
        self.push(
            TypeScriptSourceBoundaryKind::Property(Self::property_name(&property.key)),
            property.span,
        );
        walk::walk_property_definition(self, property);
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
        let mut generated = TypeScriptSourceBoundaryInventory::default();
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
            let Some(generated_span) = generated_by_kind
                .get_mut(&original.kind)
                .and_then(VecDeque::pop_front)
            else {
                continue;
            };
            if original.span.is_empty() {
                continue;
            }
            entries.push(TypeScriptSourceMapEntry::from_offsets(
                self.generated,
                generated_span.start,
                self.source,
                original.span.start,
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
        let preserves_source_notation = raw.starts_with("0x")
            || raw.starts_with("0X")
            || raw.starts_with("0o")
            || raw.starts_with("0O")
            || raw.starts_with("0b")
            || raw.starts_with("0B")
            || raw.contains('_');
        if literal.span.start > 0 && preserves_source_notation {
            self.raw_literals.push(HvigorRawNumericLiteral {
                value: literal.value,
                raw: raw.to_string(),
            });
        }
    }
}

struct HvigorRawNumericLiteral {
    value: f64,
    raw: String,
}

struct StringLiteralInventory<'a> {
    source: &'a str,
    literals: Vec<HvigorSourceStringLiteral>,
}

impl<'a> StringLiteralInventory<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            literals: Vec::new(),
        }
    }

    fn raw_literal(&self, literal: &StringLiteral<'_>) -> Option<String> {
        if literal.span.is_empty() {
            return None;
        }
        let raw = self
            .source
            .get(literal.span.start as usize..literal.span.end as usize)?;
        let bytes = raw.as_bytes();
        let quote = *bytes.first()?;
        (bytes.len() >= 2 && matches!(quote, b'\'' | b'"') && bytes.last().copied() == Some(quote))
            .then(|| raw.to_string())
    }
}

impl<'a> Visit<'a> for StringLiteralInventory<'_> {
    fn visit_string_literal(&mut self, literal: &StringLiteral<'a>) {
        self.literals.push(HvigorSourceStringLiteral {
            value: literal.value.to_string(),
            raw: self.raw_literal(literal),
        });
    }
}

struct HvigorSourceStringLiteral {
    value: String,
    raw: Option<String>,
}

struct HvigorTypescriptLayout {
    code: String,
    raw_numeric_literals: Vec<HvigorRawNumericLiteral>,
    source_string_literals: Vec<HvigorSourceStringLiteral>,
}

impl HvigorTypescriptLayout {
    fn new(
        code: String,
        raw_numeric_literals: Vec<HvigorRawNumericLiteral>,
        source_string_literals: Vec<HvigorSourceStringLiteral>,
    ) -> Self {
        Self {
            code,
            raw_numeric_literals,
            source_string_literals,
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

        let mut layout = HvigorLayoutInventory::new(&self.code);
        layout.visit_program(&parsed.program);
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
}

impl<'a> HvigorLayoutInventory<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            edits: Vec::new(),
            numeric_literals: Vec::new(),
            string_literals: Vec::new(),
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
        for (source_literal, (span, generated_value)) in
            source_literals.iter().zip(&self.string_literals)
        {
            if source_literal.value != *generated_value {
                continue;
            }
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

    fn simple_object_replacement(&self, object: &ObjectExpression<'_>) -> Option<String> {
        if object.properties.len() <= 1 {
            return None;
        }
        let mut properties = Vec::with_capacity(object.properties.len());
        for property in &object.properties {
            let span = property.span();
            let source = self.source.get(span.start as usize..span.end as usize)?;
            if source.contains('\n') || source.contains('\r') {
                return None;
            }
            properties.push(source.trim());
        }
        Some(format!("{{ {} }}", properties.join(", ")))
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
        if !arrow.expression && arrow.body.is_empty() {
            self.edits.push(HvigorTextEdit {
                span: arrow.body.span,
                replacement: "{ }".to_string(),
                kind: HvigorTextEditKind::Other,
            });
        }
        walk::walk_arrow_function_expression(self, arrow);
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
        if let Some(replacement) = self.simple_object_replacement(object) {
            self.edits.push(HvigorTextEdit {
                span: object.span,
                replacement,
                kind: HvigorTextEditKind::Object,
            });
        }
        walk::walk_object_expression(self, object);
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
