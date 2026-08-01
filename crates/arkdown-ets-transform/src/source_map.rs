use std::path::Path;

use arkdown_path::ArtifactPathSerializer;
use rolldown_sourcemap::SourceMapBuilder;
use rustc_hash::FxHashMap;

/// Source Map helpers used by the ETS transforms.
///
/// Keep source-map encoding delegated to Rolldown's Rust sourcemap stack. This
/// matches Rolldown's transform pipeline and avoids locally maintaining VLQ or
/// Source Map JSON writers.

/// Convert a byte `offset` in `source` to a `(0-based line, 0-based col)` pair.
pub fn offset_to_line_col(source: &str, offset: u32) -> (u32, u32) {
    SourceOffsetMapper::new(source).line_col(offset)
}

struct SourceOffsetMapper<'a> {
    source: &'a str,
}

impl<'a> SourceOffsetMapper<'a> {
    fn new(source: &'a str) -> Self {
        Self { source }
    }

    fn line_col(&self, offset: u32) -> (u32, u32) {
        let offset = offset as usize;
        let clamped = self.previous_char_boundary(offset.min(self.source.len()));
        let mut line = 0;
        let mut col = 0;
        for byte in self.source.bytes().take(clamped) {
            if byte == b'\n' {
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
        }
        (line, col)
    }

    fn previous_char_boundary(&self, mut offset: usize) -> usize {
        while offset > 0 && !self.source.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    }
}

struct SourceMapSourceText<'a> {
    source: &'a str,
}

impl<'a> SourceMapSourceText<'a> {
    fn new(source: &'a str) -> Self {
        Self { source }
    }

    fn line_end_after(&self, offset: usize) -> usize {
        let bytes = self.source.as_bytes();
        let mut cursor =
            SourceOffsetMapper::new(self.source).previous_char_boundary(offset.min(bytes.len()));
        while cursor < bytes.len() {
            cursor += 1;
            if bytes[cursor - 1] == b'\n' {
                return cursor;
            }
        }
        offset
    }

    fn consume_blank_lines(&self, mut offset: usize) -> usize {
        while offset < self.source.len() {
            let line_end = self.line_end_at_or_after(offset);
            let Some(line) = self.source.get(offset..line_end) else {
                break;
            };
            if !GeneratedSourceMapLine::new(line).is_blank() {
                break;
            }
            offset = line_end;
        }
        offset
    }

    fn line_end_at_or_after(&self, offset: usize) -> usize {
        let bytes = self.source.as_bytes();
        let mut cursor =
            SourceOffsetMapper::new(self.source).previous_char_boundary(offset.min(bytes.len()));
        while cursor < bytes.len() {
            cursor += 1;
            if bytes[cursor - 1] == b'\n' {
                return cursor;
            }
        }
        bytes.len()
    }
}

/// A single source map entry mapping one generated line back to an original source position.
#[derive(Debug, Clone)]
pub struct MapEntry {
    /// 0-based line index in the generated ViewPU TypeScript output.
    pub generated_line: usize,
    /// 0-based column index in the generated ViewPU TypeScript output.
    pub generated_col: u32,
    /// 0-based line index in the original `.ets` source.
    pub src_line: u32,
    /// 0-based column index in the original `.ets` source.
    pub src_col: u32,
}

/// Build a Source Map v3 JSON string from a list of generated/source entries.
pub struct EtsSourceMapBuilder<'a> {
    source_path: &'a str,
    generated_line_count: usize,
    entries: &'a [MapEntry],
}

impl<'a> EtsSourceMapBuilder<'a> {
    pub fn new(source_path: &'a str, generated_line_count: usize, entries: &'a [MapEntry]) -> Self {
        Self {
            source_path,
            generated_line_count,
            entries,
        }
    }

    pub fn build(&self) -> String {
        let mut sorted = self.entries.to_vec();
        sorted.sort_by_key(|e| (e.generated_line, e.generated_col));

        let mut builder = SourceMapBuilder::default();
        let source_path = ArtifactPathSerializer::rel_to_slash(Path::new(self.source_path));
        let source_id = builder.set_source_and_content(&source_path, "");
        for entry in sorted {
            builder.add_token(
                entry.generated_line as u32,
                entry.generated_col,
                entry.src_line,
                entry.src_col,
                Some(source_id),
                None,
            );
        }
        let mut source_map = builder.into_sourcemap();
        source_map.set_source_contents(vec![None]);
        let _ = self.generated_line_count;
        source_map.to_json_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_offset_to_line_col_single_line() {
        let src = "hello world";
        assert_eq!(offset_to_line_col(src, 0), (0, 0));
        assert_eq!(offset_to_line_col(src, 5), (0, 5));
    }

    #[test]
    fn test_offset_to_line_col_multiline() {
        let src = "line0\nline1\nline2";
        assert_eq!(offset_to_line_col(src, 0), (0, 0));
        assert_eq!(offset_to_line_col(src, 6), (1, 0));
        assert_eq!(offset_to_line_col(src, 11), (1, 5));
        assert_eq!(offset_to_line_col(src, 12), (2, 0));
    }

    #[test]
    fn test_build_source_map_json_structure() {
        let entries = vec![MapEntry {
            generated_line: 2,
            generated_col: 0,
            src_line: 5,
            src_col: 4,
        }];
        let json = EtsSourceMapBuilder::new("src/main/ets/pages/Index.ets", 5, &entries).build();
        assert!(json.contains("\"version\":3"), "must have version 3");
        assert!(
            json.contains("\"sources\":[\"src/main/ets/pages/Index.ets\"]"),
            "must have source path"
        );
        let value: serde_json::Value = serde_json::from_str(&json).expect("source map json");
        let mappings = value
            .get("mappings")
            .and_then(serde_json::Value::as_str)
            .expect("mappings");
        let mut semicolons = 0;
        let mut line_two_has_segment = false;
        for character in mappings.chars() {
            if character == ';' {
                semicolons += 1;
            } else if semicolons == 2 {
                line_two_has_segment = true;
            }
        }
        assert_eq!(semicolons, 2);
        assert!(line_two_has_segment, "line 2 must have a segment");
    }

    #[test]
    fn standard_export_class_final_brace_maps_only_printer_end_position() {
        let source = "export default class Example {\n    run(): void {\n    }\n}\n";
        let source_map = HvigorStandardSourceMapBuilder::new(source, source, "Example.ets")
            .build()
            .expect("standard source map");
        let value: serde_json::Value = serde_json::from_str(&source_map).expect("source map JSON");
        let mappings = value["mappings"].as_str().expect("mappings");
        let final_line = mappings.rsplit(';').next().expect("final mapping line");

        assert!(
            !final_line.contains(','),
            "TypeScript printer maps only the end of a top-level class closing brace: {mappings}"
        );
    }

    #[test]
    fn standard_direct_ohos_imports_map_local_bindings_like_typescript_printer() {
        let original = r#"import StartupConfigEntry from '@ohos.app.appstartup.StartupConfigEntry';
import StartupConfig from '@ohos.app.appstartup.StartupConfig';

export default class ArkdownStartupConfig extends StartupConfigEntry {
  onConfig(): StartupConfig {
    return { timeoutMs: 1 };
  }
}
"#;
        let generated = r#"import StartupConfigEntry from "@ohos:app.appstartup.StartupConfigEntry";
import type StartupConfig from "@ohos:app.appstartup.StartupConfig";
export default class ArkdownStartupConfig extends StartupConfigEntry {
    onConfig(): StartupConfig {
        return { timeoutMs: 1 };
    }
}
"#;
        let source_map =
            HvigorStandardSourceMapBuilder::new(generated, original, "StartupConfig.ets")
                .build()
                .expect("standard source map");
        let value: serde_json::Value = serde_json::from_str(&source_map).expect("source map JSON");

        assert_eq!(
            value["mappings"],
            "OAAO,kBAAkB;YAClB,aAAa;AAEpB,MAAM,CAAC,OAAO,OAAO,oBAAqB,SAAQ,kBAAkB;IAClE,QAAQ,IAAI,aAAa;QACvB,OAAO,EAAE,SAAS,EAAE,CAAC,EAAE,CAAC;IAC1B,CAAC;CACF"
        );
    }

    #[test]
    fn standard_async_method_maps_modifier_end_and_method_start() {
        let original = "export default class MyStartupTask {\n  async init() {\n  }\n}\n";
        let generated = "export default class MyStartupTask {\n    async init() {\n    }\n}\n";
        let source_map =
            HvigorStandardSourceMapBuilder::new(generated, original, "MyStartupTask.ets")
                .build()
                .expect("standard source map");
        let value: serde_json::Value = serde_json::from_str(&source_map).expect("source map JSON");

        assert_eq!(
            value["mappings"],
            "AAAA,MAAM,CAAC,OAAO,OAAO,aAAa;IAChC,KAAK,CAAC,IAAI;IACV,CAAC;CACF"
        );
    }
}

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Declaration, ExportDefaultDeclarationKind, ImportDeclarationSpecifier, Statement,
};
use oxc_parser::config::TokensParserConfig;
use oxc_parser::Kind;
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};

/// Build a per-token source map for `source` (identity mapping).
pub struct TokenSourceMapBuilder<'a> {
    source: &'a str,
    id: &'a str,
}

pub struct TypeScriptCommentOnlySourceMap<'a> {
    id: &'a str,
}

impl<'a> TypeScriptCommentOnlySourceMap<'a> {
    pub fn new(id: &'a str) -> Self {
        Self { id }
    }

    pub fn build(&self) -> String {
        let source = ArtifactPathSerializer::rel_to_slash(Path::new(self.id));
        let file = Path::new(self.id)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(self.id);
        serde_json::to_string(&serde_json::json!({
            "version": 3,
            "file": file,
            "sources": [source],
            "names": [],
            "mappings": ";",
        }))
        .expect("comment-only TypeScript source map is serializable")
    }
}

impl<'a> TokenSourceMapBuilder<'a> {
    pub fn new(source: &'a str, id: &'a str) -> Self {
        Self { source, id }
    }

    /// Emits one 4-field VLQ segment per non-whitespace/comment token.
    pub fn build(&self) -> Option<String> {
        let tokens = SourceMapTokenCollector::new(self.source, self.id).collect()?;
        let entries = tokens
            .iter()
            .map(|token| MapEntry {
                generated_line: token.line as usize,
                generated_col: token.col,
                src_line: token.line,
                src_col: token.col,
            })
            .collect::<Vec<_>>();
        Some(EtsSourceMapBuilder::new(self.id, 0, &entries).build())
    }
}

/// Build a source map for standard non-`@Component` ETS files after hvigor-style
/// formatting and `@kit` import expansion.
///
/// Upstream hvigor rewrites `@kit.*` imports before printing the source file with
/// TypeScript's printer. The emitted source map keeps the generated default
/// import binding mapped to the original named import binding, while the rest of
/// the file follows TypeScript printer token emission points. This mirrors that
/// behavior for our already-formatted Rust output.
pub struct HvigorStandardSourceMapBuilder<'a> {
    generated: &'a str,
    original: &'a str,
    id: &'a str,
}

impl<'a> HvigorStandardSourceMapBuilder<'a> {
    pub fn new(generated: &'a str, original: &'a str, id: &'a str) -> Self {
        Self {
            generated,
            original,
            id,
        }
    }

    pub fn build(&self) -> Option<String> {
        let generated_tokens = SourceMapTokenCollector::new(self.generated, self.id).collect()?;
        let original_tokens = SourceMapTokenCollector::new(self.original, self.id).collect()?;
        let generated_import_end =
            SourceMapImportPrefix::new(self.generated, self.id).generated_end();
        let original_import_end = SourceMapImportPrefix::new(self.original, self.id).original_end();
        let original_bindings =
            OriginalImportBindingCollector::new(self.original, self.id).collect();
        let mut entries = MapEntryCollector::default();

        for token in generated_tokens
            .iter()
            .filter(|token| token.start < generated_import_end)
        {
            if let Some((source_start, source_end)) = original_bindings.get(token.text) {
                let (src_line, src_col) = offset_to_line_col(self.original, *source_start);
                entries.push(token.line, token.col, src_line, src_col);
                let (src_line, src_col) = offset_to_line_col(self.original, *source_end);
                entries.push(token.end_line, token.end_col, src_line, src_col);
            }
        }

        let body_tokens: Vec<&TokenInfo> = generated_tokens
            .iter()
            .filter(|token| token.start >= generated_import_end)
            .collect();
        let mut original_index = original_tokens
            .iter()
            .position(|token| token.start >= original_import_end)
            .unwrap_or(original_tokens.len());
        let generated_text = GeneratedSourceMapText::new(self.generated);
        let final_generated_line = generated_text.final_line();
        let final_statement_is_class = self.final_statement_is_class();

        for (index, token) in body_tokens.iter().enumerate() {
            while original_index < original_tokens.len()
                && original_tokens[original_index].text != token.text
            {
                let original_token = &original_tokens[original_index];
                if original_token.is_comma() && token.is_close_paren() {
                    break;
                }
                if original_token.is_original_alignment_skip() {
                    original_index += 1;
                    continue;
                }
                return None;
            }
            if original_index >= original_tokens.len() {
                return None;
            }

            let original_token = &original_tokens[original_index];
            original_index += 1;
            let neighbors = TokenLineNeighbors::new(&body_tokens, index);
            let line_facts = neighbors.line_facts();
            let decision = StandardTokenMappingDecision::new(
                token,
                neighbors.previous(),
                neighbors.next(),
                line_facts,
                token.line == final_generated_line && final_statement_is_class,
            );

            if decision.should_map_start() {
                entries.push(
                    token.line,
                    token.col,
                    original_token.line,
                    original_token.col,
                );
            }

            if decision.should_map_end() {
                let class_heritage_source_adjust = if neighbors
                    .previous()
                    .is_some_and(SourceMapTokenInfo::is_class_keyword)
                    && neighbors
                        .next()
                        .is_some_and(SourceMapTokenInfo::is_class_heritage_keyword)
                {
                    1
                } else {
                    0
                };
                let return_object_trailing_space_adjust = if token.is_return_keyword()
                    && neighbors
                        .next()
                        .is_some_and(SourceMapTokenInfo::is_open_brace)
                {
                    1
                } else {
                    0
                };
                entries.push(
                    token.end_line,
                    token.end_col + return_object_trailing_space_adjust,
                    original_token.end_line,
                    original_token.end_col
                        + class_heritage_source_adjust
                        + return_object_trailing_space_adjust,
                );
            }
        }

        let entries = entries.into_vec();
        Some(EtsSourceMapBuilder::new(self.id, generated_text.line_count(), &entries).build())
    }

    fn final_statement_is_class(&self) -> bool {
        let allocator = Allocator::default();
        let source_type =
            SourceType::from_path(self.id).unwrap_or_else(|_| SourceType::ts().with_module(true));
        let parsed = Parser::new(&allocator, self.generated, source_type).parse();
        if !parsed.diagnostics.is_empty() {
            return false;
        }
        let Some(statement) = parsed.program.body.last() else {
            return false;
        };
        match statement {
            Statement::ClassDeclaration(_) => true,
            Statement::ExportDefaultDeclaration(export) => matches!(
                &export.declaration,
                ExportDefaultDeclarationKind::ClassDeclaration(_)
            ),
            Statement::ExportNamedDeclaration(export) => {
                matches!(&export.declaration, Some(Declaration::ClassDeclaration(_)))
            }
            _ => false,
        }
    }
}

#[derive(Debug)]
struct TokenInfo<'a> {
    text: &'a str,
    kind: Kind,
    start: u32,
    line: u32,
    col: u32,
    end_line: u32,
    end_col: u32,
}

struct SourceMapTokenCollector<'a> {
    source: &'a str,
    id: &'a str,
}

impl<'a> SourceMapTokenCollector<'a> {
    fn new(source: &'a str, id: &'a str) -> Self {
        Self { source, id }
    }

    fn collect(&self) -> Option<Vec<TokenInfo<'a>>> {
        let allocator = Allocator::default();
        let source_type =
            SourceType::from_path(self.id).unwrap_or_else(|_| SourceType::ts().with_module(true));
        let parsed = Parser::new(&allocator, self.source, source_type)
            .with_config(TokensParserConfig)
            .parse();
        if !parsed.diagnostics.is_empty() {
            return None;
        }
        let line_offsets = SourceMapLineOffsets::new(self.source);
        let mut tokens = Vec::new();
        for tok in &parsed.tokens {
            use oxc_parser::Kind;
            if matches!(
                tok.kind(),
                Kind::Eof | Kind::Undetermined | Kind::Skip | Kind::HashbangComment
            ) {
                continue;
            }
            let start = tok.start();
            let end = tok.end();
            let (line, col) = line_offsets.line_col(start);
            let (end_line, end_col) = line_offsets.line_col(end);
            tokens.push(TokenInfo {
                text: self
                    .source
                    .get(start as usize..end as usize)
                    .unwrap_or_default(),
                kind: tok.kind(),
                start,
                line,
                col,
                end_line,
                end_col,
            });
        }
        Some(tokens)
    }
}

struct SourceMapImportPrefix<'a> {
    source: &'a str,
    id: &'a str,
}

impl<'a> SourceMapImportPrefix<'a> {
    fn new(source: &'a str, id: &'a str) -> Self {
        Self { source, id }
    }

    fn generated_end(&self) -> u32 {
        self.end(false)
    }

    fn original_end(&self) -> u32 {
        self.end(true)
    }

    fn end(&self, include_trailing_blank_lines: bool) -> u32 {
        let allocator = Allocator::default();
        let source_type =
            SourceType::from_path(self.id).unwrap_or_else(|_| SourceType::ts().with_module(true));
        let parsed = Parser::new(&allocator, self.source, source_type).parse();
        if !parsed.diagnostics.is_empty() {
            return 0;
        }

        let source_text = SourceMapSourceText::new(self.source);
        let mut end = 0usize;
        for statement in &parsed.program.body {
            if matches!(statement, Statement::ImportDeclaration(_)) {
                end = source_text.line_end_after(statement.span().end as usize);
                continue;
            }
            break;
        }

        if include_trailing_blank_lines {
            end = source_text.consume_blank_lines(end);
        }
        end as u32
    }
}

struct OriginalImportBindingCollector<'a> {
    source: &'a str,
    id: &'a str,
}

impl<'a> OriginalImportBindingCollector<'a> {
    fn new(source: &'a str, id: &'a str) -> Self {
        Self { source, id }
    }

    fn collect(&self) -> FxHashMap<String, (u32, u32)> {
        let allocator = Allocator::default();
        let source_type =
            SourceType::from_path(self.id).unwrap_or_else(|_| SourceType::ts().with_module(true));
        let parsed = Parser::new(&allocator, self.source, source_type).parse();
        if !parsed.diagnostics.is_empty() {
            return FxHashMap::default();
        }

        let mut bindings = FxHashMap::default();
        for statement in &parsed.program.body {
            let Statement::ImportDeclaration(import) = statement else {
                continue;
            };
            let Some(specifiers) = import.specifiers.as_ref() else {
                continue;
            };
            for specifier in specifiers {
                let local = match specifier {
                    ImportDeclarationSpecifier::ImportSpecifier(specifier) => &specifier.local,
                    ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                        &specifier.local
                    }
                    ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                        &specifier.local
                    }
                };
                let name = local.name.as_str();
                let span = local.span;
                bindings.insert(name.to_string(), (span.start, span.end));
            }
        }
        bindings
    }
}

struct TokenLineNeighbors<'a, 'b, 'source> {
    tokens: &'a [&'b TokenInfo<'source>],
    index: usize,
}

impl<'a, 'b, 'source> TokenLineNeighbors<'a, 'b, 'source> {
    fn new(tokens: &'a [&'b TokenInfo<'source>], index: usize) -> Self {
        Self { tokens, index }
    }

    fn previous(&self) -> Option<&'b TokenInfo<'source>> {
        self.index
            .checked_sub(1)
            .and_then(|prev| self.tokens.get(prev).copied())
            .filter(|prev| prev.line == self.tokens[self.index].line)
    }

    fn next(&self) -> Option<&'b TokenInfo<'source>> {
        self.tokens
            .get(self.index + 1)
            .copied()
            .filter(|next| next.line == self.tokens[self.index].line)
    }

    fn line_facts(&self) -> StandardTokenLineFacts {
        let current_line = self.tokens[self.index].line;
        let mut first = None;
        let mut count = 0usize;
        let mut contains_if = false;
        let mut contains_return = false;
        for token in self
            .tokens
            .iter()
            .copied()
            .filter(|token| token.line == current_line)
        {
            if first.is_none() {
                first = Some(token);
            }
            count += 1;
            contains_if |= token.is_if_keyword();
            contains_return |= token.is_return_keyword();
        }

        let standalone_close_brace =
            count == 1 && first.is_some_and(SourceMapTokenInfo::is_close_brace);
        Self::line_facts_from_first_token(
            first,
            contains_if,
            contains_return,
            standalone_close_brace,
        )
    }

    fn line_facts_from_first_token(
        first: Option<&TokenInfo<'_>>,
        contains_if: bool,
        contains_return: bool,
        standalone_close_brace: bool,
    ) -> StandardTokenLineFacts {
        StandardTokenLineFacts {
            contains_if,
            contains_return,
            standalone_close_brace,
            deep_standalone_close_brace: standalone_close_brace
                && first.is_some_and(|token| token.col >= 12),
            starts_const_declaration: first
                .is_some_and(SourceMapTokenInfo::starts_const_initializer),
        }
    }
}

#[derive(Clone, Copy)]
struct StandardTokenLineFacts {
    contains_if: bool,
    contains_return: bool,
    standalone_close_brace: bool,
    deep_standalone_close_brace: bool,
    starts_const_declaration: bool,
}

struct StandardTokenMappingDecision<'a> {
    token: &'a TokenInfo<'a>,
    previous: Option<&'a TokenInfo<'a>>,
    next: Option<&'a TokenInfo<'a>>,
    line_facts: StandardTokenLineFacts,
    suppress_final_close_brace_start: bool,
}

impl<'a> StandardTokenMappingDecision<'a> {
    fn new(
        token: &'a TokenInfo<'a>,
        previous: Option<&'a TokenInfo<'a>>,
        next: Option<&'a TokenInfo<'a>>,
        line_facts: StandardTokenLineFacts,
        suppress_final_close_brace_start: bool,
    ) -> Self {
        Self {
            token,
            previous,
            next,
            line_facts,
            suppress_final_close_brace_start,
        }
    }

    fn should_map_start(&self) -> bool {
        let context = StandardTokenMapContext::new(self.previous, self.line_facts);
        if self.token.is_function_keyword()
            && self
                .previous
                .is_some_and(SourceMapTokenInfo::is_export_modifier)
        {
            return false;
        }
        if self.token.is_standard_start_suppressed() {
            return false;
        }
        if self.token.is_open_paren()
            && self.previous.is_some_and(SourceMapTokenInfo::is_if_keyword)
        {
            return false;
        }
        if self.token.is_close_paren()
            && self.previous.is_some_and(SourceMapTokenInfo::is_open_paren)
        {
            return false;
        }
        if self.token.is_colon() && self.next.is_some_and(SourceMapTokenInfo::is_void_keyword) {
            return false;
        }
        if self.token.is_colon()
            && self
                .previous
                .is_some_and(SourceMapTokenInfo::is_close_paren)
        {
            return false;
        }
        if self.token.is_open_brace() && !context.is_conditional_block_open() {
            return false;
        }
        if self.token.is_semicolon()
            && self
                .previous
                .is_some_and(SourceMapTokenInfo::is_return_keyword)
        {
            return false;
        }
        if self.token.is_close_brace() && self.line_facts.deep_standalone_close_brace {
            return false;
        }
        if self.token.is_close_brace()
            && self.line_facts.contains_return
            && !self.line_facts.standalone_close_brace
        {
            return false;
        }
        if self.token.is_close_brace() && self.suppress_final_close_brace_start {
            return false;
        }
        true
    }

    fn should_map_end(&self) -> bool {
        if self.token.should_map_end_directly() {
            return true;
        }
        if self.token.is_semicolon() {
            return true;
        }
        if self.token.is_close_brace() && self.line_facts.standalone_close_brace {
            return true;
        }
        if self.line_facts.starts_const_declaration
            && self
                .previous
                .is_some_and(SourceMapTokenInfo::starts_const_initializer)
        {
            return true;
        }
        if self
            .previous
            .is_some_and(SourceMapTokenInfo::starts_type_name)
            && self.token.is_identifier_name()
        {
            return true;
        }
        if self.token.is_identifier_name()
            && self.next.is_some_and(SourceMapTokenInfo::is_open_brace)
        {
            return true;
        }
        false
    }
}

struct StandardTokenMapContext<'a> {
    previous: Option<&'a TokenInfo<'a>>,
    line_facts: StandardTokenLineFacts,
}

impl<'a> StandardTokenMapContext<'a> {
    fn new(previous: Option<&'a TokenInfo<'a>>, line_facts: StandardTokenLineFacts) -> Self {
        Self {
            previous,
            line_facts,
        }
    }

    fn is_conditional_block_open(&self) -> bool {
        self.previous
            .is_some_and(SourceMapTokenInfo::is_close_paren)
            && self.line_facts.contains_if
    }
}

trait SourceMapTokenInfo {
    fn is_function_keyword(&self) -> bool;
    fn is_export_modifier(&self) -> bool;
    fn is_class_keyword(&self) -> bool;
    fn is_class_heritage_keyword(&self) -> bool;
    fn is_if_keyword(&self) -> bool;
    fn is_return_keyword(&self) -> bool;
    fn is_void_keyword(&self) -> bool;
    fn is_open_paren(&self) -> bool;
    fn is_close_paren(&self) -> bool;
    fn is_open_brace(&self) -> bool;
    fn is_close_brace(&self) -> bool;
    fn is_colon(&self) -> bool;
    fn is_semicolon(&self) -> bool;
    fn is_comma(&self) -> bool;
    fn is_standard_start_suppressed(&self) -> bool;
    fn should_map_end_directly(&self) -> bool;
    fn starts_const_initializer(&self) -> bool;
    fn starts_type_name(&self) -> bool;
    fn is_identifier_name(&self) -> bool;
    fn is_original_alignment_skip(&self) -> bool;
}

impl SourceMapTokenInfo for TokenInfo<'_> {
    fn is_function_keyword(&self) -> bool {
        self.kind == Kind::Function
    }

    fn is_export_modifier(&self) -> bool {
        matches!(self.kind, Kind::Export | Kind::Default)
    }

    fn is_class_keyword(&self) -> bool {
        self.kind == Kind::Class
    }

    fn is_class_heritage_keyword(&self) -> bool {
        matches!(self.kind, Kind::Extends | Kind::Implements)
    }

    fn is_if_keyword(&self) -> bool {
        self.kind == Kind::If
    }

    fn is_return_keyword(&self) -> bool {
        self.kind == Kind::Return
    }

    fn is_void_keyword(&self) -> bool {
        self.kind == Kind::Void
    }

    fn is_open_paren(&self) -> bool {
        self.kind == Kind::LParen
    }

    fn is_close_paren(&self) -> bool {
        self.kind == Kind::RParen
    }

    fn is_open_brace(&self) -> bool {
        self.kind == Kind::LCurly
    }

    fn is_close_brace(&self) -> bool {
        self.kind == Kind::RCurly
    }

    fn is_colon(&self) -> bool {
        self.kind == Kind::Colon
    }

    fn is_semicolon(&self) -> bool {
        self.kind == Kind::Semicolon
    }

    fn is_comma(&self) -> bool {
        self.kind == Kind::Comma
    }

    fn is_standard_start_suppressed(&self) -> bool {
        matches!(self.kind, Kind::Class | Kind::Extends | Kind::Eq)
    }

    fn should_map_end_directly(&self) -> bool {
        matches!(
            self.kind,
            Kind::Export | Kind::Default | Kind::Async | Kind::Void | Kind::Arrow
        ) || self.kind.is_number()
    }

    fn starts_const_initializer(&self) -> bool {
        matches!(self.kind, Kind::Const | Kind::Eq)
    }

    fn starts_type_name(&self) -> bool {
        matches!(self.kind, Kind::Class | Kind::Extends)
    }

    fn is_identifier_name(&self) -> bool {
        self.kind.is_identifier_name()
    }

    fn is_original_alignment_skip(&self) -> bool {
        matches!(self.kind, Kind::Comma | Kind::Semicolon | Kind::RParen)
    }
}

struct GeneratedSourceMapText<'a> {
    lines: Vec<GeneratedSourceMapLine<'a>>,
}

impl<'a> GeneratedSourceMapText<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            lines: GeneratedSourceMapLines::new(source).collect(),
        }
    }

    fn line_count(&self) -> usize {
        self.lines.len().max(1)
    }

    fn final_line(&self) -> u32 {
        self.lines.len().saturating_sub(1) as u32
    }
}

struct GeneratedSourceMapLines<'a> {
    source: &'a str,
    offset: usize,
    finished: bool,
}

impl<'a> GeneratedSourceMapLines<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            offset: 0,
            finished: false,
        }
    }
}

impl<'a> Iterator for GeneratedSourceMapLines<'a> {
    type Item = GeneratedSourceMapLine<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        if self.offset == self.source.len() {
            self.finished = true;
            return None;
        }
        let bytes = self.source.as_bytes();
        let start = self.offset;
        while self.offset < bytes.len() && bytes[self.offset] != b'\n' {
            self.offset += 1;
        }
        let mut end = self.offset;
        if end > start && bytes[end - 1] == b'\r' {
            end -= 1;
        }
        if self.offset < bytes.len() {
            self.offset += 1;
        } else {
            self.finished = true;
        }
        self.source.get(start..end).map(GeneratedSourceMapLine::new)
    }
}

#[derive(Clone, Copy)]
struct GeneratedSourceMapLine<'a> {
    text: &'a str,
}

impl<'a> GeneratedSourceMapLine<'a> {
    fn new(text: &'a str) -> Self {
        Self { text }
    }

    fn is_blank(&self) -> bool {
        self.text.bytes().all(|byte| byte.is_ascii_whitespace())
    }
}

#[derive(Default)]
struct MapEntryCollector {
    entries: Vec<MapEntry>,
}

impl MapEntryCollector {
    fn push(&mut self, line: u32, col: u32, src_line: u32, src_col: u32) {
        if self.entries.last().is_some_and(|entry| {
            entry.generated_line == line as usize
                && entry.generated_col == col
                && entry.src_line == src_line
                && entry.src_col == src_col
        }) {
            return;
        }
        self.entries.push(MapEntry {
            generated_line: line as usize,
            generated_col: col,
            src_line,
            src_col,
        });
    }

    fn into_vec(self) -> Vec<MapEntry> {
        self.entries
    }
}

struct SourceMapLineOffsets {
    offsets: Vec<u32>,
}

impl SourceMapLineOffsets {
    fn new(source: &str) -> Self {
        let mut offsets = vec![0u32];
        for (index, byte) in source.bytes().enumerate() {
            if byte == b'\n' {
                offsets.push((index + 1) as u32);
            }
        }
        Self { offsets }
    }

    fn line_col(&self, offset: u32) -> (u32, u32) {
        let line = self
            .offsets
            .partition_point(|&line_offset| line_offset <= offset)
            .saturating_sub(1);
        (line as u32, offset - self.offsets[line])
    }
}
