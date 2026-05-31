/// Minimal Source Map v3 builder for the ETS→ViewPU cross-transform source map.
///
/// Encodes a set of per-line `(generated_line, src_line, src_col)` entries into
/// a VLQ-encoded `mappings` string and wraps the result in a Source Map v3 JSON
/// object.  One segment is emitted per generated line at column 0; generated
/// lines without an entry get an empty group (no source reference).
///
/// This module is intentionally dependency-free (no serde_json) to avoid adding
/// a production dependency just for JSON serialisation.

/// Convert a byte `offset` in `source` to a `(0-based line, 0-based col)` pair.
pub fn offset_to_line_col(source: &str, offset: u32) -> (u32, u32) {
    let offset = offset as usize;
    let clamped = offset.min(source.len());
    let prefix = &source[..clamped];
    let line = prefix.bytes().filter(|&b| b == b'\n').count() as u32;
    let col = prefix
        .rfind('\n')
        .map(|pos| clamped - pos - 1)
        .unwrap_or(clamped) as u32;
    (line, col)
}

/// VLQ-encode a single signed integer according to the Source Map v3 spec.
///
/// Bit layout: sign is stored in bit 0 (LSB) of the continuation-group value;
/// magnitude occupies the remaining bits, least-significant group first.
/// Each 5-bit group is offset into the base-64 alphabet, with bit 5 as the
/// continuation flag.
pub fn encode_vlq(value: i32) -> String {
    const BASE64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut vlq: u32 = if value < 0 {
        ((-value as u32) << 1) | 1
    } else {
        (value as u32) << 1
    };
    let mut result = String::new();
    loop {
        let mut digit = vlq & 0x1f;
        vlq >>= 5;
        if vlq > 0 {
            digit |= 0x20;
        }
        result.push(BASE64[digit as usize] as char);
        if vlq == 0 {
            break;
        }
    }
    result
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

/// Build a Source Map v3 JSON string from a list of per-line entries.
///
/// `source_path` is placed verbatim in `sources[0]`.  `generated_line_count` is
/// the total number of lines in the generated code (controls how many `;`
/// separators appear in `mappings`).  Entries must not need to be sorted — the
/// function sorts by `generated_line` internally.
///
/// Lines without an entry receive an empty group (`;` only) which tells consumers
/// that no source position is available for those lines.
pub fn build_ets_source_map(
    source_path: &str,
    generated_line_count: usize,
    entries: &[MapEntry],
) -> String {
    let mut sorted = entries.to_vec();
    sorted.sort_by_key(|e| (e.generated_line, e.generated_col));

    let mut mappings = String::new();
    let mut entry_idx = 0usize;
    let mut prev_src_line = 0i32;
    let mut prev_src_col = 0i32;

    for gen_line in 0..generated_line_count {
        if gen_line > 0 {
            mappings.push(';');
        }
        let mut prev_gen_col = 0i32;
        let mut first_on_line = true;
        while entry_idx < sorted.len() && sorted[entry_idx].generated_line == gen_line {
            let e = &sorted[entry_idx];
            if !first_on_line {
                mappings.push(',');
            }
            first_on_line = false;
            let gen_col_delta = e.generated_col as i32 - prev_gen_col;
            let src_line_delta = e.src_line as i32 - prev_src_line;
            let src_col_delta = e.src_col as i32 - prev_src_col;
            prev_gen_col = e.generated_col as i32;
            prev_src_line = e.src_line as i32;
            prev_src_col = e.src_col as i32;
            // Segment: generated_col_delta, source_idx=0, orig_line_delta, orig_col_delta
            mappings.push_str(&encode_vlq(gen_col_delta));
            mappings.push_str(&encode_vlq(0));
            mappings.push_str(&encode_vlq(src_line_delta));
            mappings.push_str(&encode_vlq(src_col_delta));
            entry_idx += 1;
        }
    }

    let escaped_path = source_path.replace('\\', "/").replace('"', "\\\"");
    format!(
        "{{\"version\":3,\"sources\":[\"{escaped_path}\"],\"sourcesContent\":[null],\"mappings\":\"{mappings}\"}}"
    )
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
    fn test_encode_vlq_zero() {
        assert_eq!(encode_vlq(0), "A");
    }

    #[test]
    fn test_encode_vlq_positive() {
        assert_eq!(encode_vlq(1), "C");
        assert_eq!(encode_vlq(15), "e");
    }

    #[test]
    fn test_encode_vlq_negative() {
        assert_eq!(encode_vlq(-1), "D");
        assert_eq!(encode_vlq(-15), "f");
    }

    #[test]
    fn test_build_source_map_json_structure() {
        let entries = vec![MapEntry {
            generated_line: 2,
            generated_col: 0,
            src_line: 5,
            src_col: 4,
        }];
        let json = build_ets_source_map("src/main/ets/pages/Index.ets", 5, &entries);
        assert!(json.contains("\"version\":3"), "must have version 3");
        assert!(
            json.contains("\"sources\":[\"src/main/ets/pages/Index.ets\"]"),
            "must have source path"
        );
        assert!(json.contains("\"mappings\":"), "must have mappings");
        // Lines 0,1 have no entries → empty groups; line 2 has entry; lines 3,4 empty
        // mappings = ";;AAAK;;"  (roughly — exact encoding depends on src_line/col)
        let mappings_start = json.find("\"mappings\":\"").unwrap() + 12;
        let mappings_end = json.rfind('"').unwrap();
        let mappings = &json[mappings_start..mappings_end];
        // 5 lines → 4 semicolons total in mappings
        assert_eq!(mappings.chars().filter(|&c| c == ';').count(), 4);
        // Line 2 (index 2) has a segment (non-empty between semicolons 1 and 2)
        let groups: Vec<&str> = mappings.split(';').collect();
        assert_eq!(groups.len(), 5);
        assert!(groups[0].is_empty(), "line 0 must be empty");
        assert!(groups[1].is_empty(), "line 1 must be empty");
        assert!(!groups[2].is_empty(), "line 2 must have a segment");
        assert!(groups[3].is_empty(), "line 3 must be empty");
        assert!(groups[4].is_empty(), "line 4 must be empty");
    }
}

use oxc_allocator::Allocator;
use oxc_ast::ast::{ImportDeclarationSpecifier, Statement};
use oxc_parser::config::TokensParserConfig;
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};

/// Build a per-token source map for `source` (identity mapping).
/// Emits one 4-field VLQ segment per non-whitespace/comment token.
pub fn build_token_source_map(source: &str, id: &str) -> Option<String> {
    let allocator = Allocator::default();
    let source_type =
        SourceType::from_path(id).unwrap_or_else(|_| SourceType::ts().with_module(true));
    let parsed = Parser::new(&allocator, source, source_type)
        .with_config(TokensParserConfig)
        .parse();
    if !parsed.errors.is_empty() {
        return None;
    }
    let line_offsets = build_line_offsets_table(source);
    let mut prev_src_line = 0i32;
    let mut prev_src_col = 0i32;
    let mut mappings = String::new();
    let mut cur_gen_line = 0u32;
    let mut prev_gen_col = 0i32;
    let mut first_on_line = true;
    for tok in &parsed.tokens {
        use oxc_parser::Kind;
        if matches!(
            tok.kind(),
            Kind::Eof | Kind::Undetermined | Kind::Skip | Kind::HashbangComment
        ) {
            continue;
        }
        let start = tok.start();
        let (line, col) = offset_to_line_col_table(&line_offsets, start);
        while cur_gen_line < line {
            mappings.push(';');
            cur_gen_line += 1;
            prev_gen_col = 0;
            first_on_line = true;
        }
        if !first_on_line {
            mappings.push(',');
        }
        first_on_line = false;
        let gen_col = col as i32;
        mappings.push_str(&encode_vlq(gen_col - prev_gen_col));
        prev_gen_col = gen_col;
        mappings.push_str(&encode_vlq(0));
        mappings.push_str(&encode_vlq(line as i32 - prev_src_line));
        mappings.push_str(&encode_vlq(col as i32 - prev_src_col));
        prev_src_line = line as i32;
        prev_src_col = col as i32;
    }
    let escaped = id.replace('\\', "/").replace('"', "\\\"");
    Some(format!(
        r#"{{"version":3,"sources":["{escaped}"],"sourcesContent":[null],"names":[],"mappings":"{mappings}"}}"#
    ))
}

/// Build a source map for standard non-`@Component` ETS files after hvigor-style
/// formatting and `@kit` import expansion.
///
/// Upstream hvigor rewrites `@kit.*` imports before printing the source file with
/// TypeScript's printer. The emitted source map keeps the generated default
/// import binding mapped to the original named import binding, while the rest of
/// the file follows TypeScript printer token emission points. This mirrors that
/// behavior for our already-formatted Rust output.
pub fn build_hvigor_standard_source_map(
    generated: &str,
    original: &str,
    id: &str,
) -> Option<String> {
    let generated_tokens = collect_tokens(generated, id)?;
    let original_tokens = collect_tokens(original, id)?;
    let generated_import_end = generated_import_prefix_end(generated, id);
    let original_import_end = original_import_prefix_end(original, id);
    let original_bindings = collect_original_kit_bindings(original, id);
    let mut entries = Vec::new();

    for token in generated_tokens
        .iter()
        .filter(|token| token.start < generated_import_end)
    {
        if let Some((source_start, source_end)) = original_bindings.get(token.text.as_str()) {
            let (src_line, src_col) = offset_to_line_col(original, *source_start);
            push_entry(&mut entries, token.line, token.col, src_line, src_col);
            let (src_line, src_col) = offset_to_line_col(original, *source_end);
            push_entry(
                &mut entries,
                token.end_line,
                token.end_col,
                src_line,
                src_col,
            );
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
    let generated_lines: Vec<&str> = generated.lines().collect();
    let final_generated_line = generated_lines.len().saturating_sub(1) as u32;

    for (index, token) in body_tokens.iter().enumerate() {
        while original_index < original_tokens.len()
            && original_tokens[original_index].text != token.text
        {
            let original_text = original_tokens[original_index].text.as_str();
            if original_text == "," && token.text == ")" {
                break;
            }
            if matches!(original_text, "," | ";" | ")") {
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
        let line = generated_lines
            .get(token.line as usize)
            .copied()
            .unwrap_or("");
        let previous = previous_token_on_line(&body_tokens, index);
        let next = next_token_on_line(&body_tokens, index);
        let is_final_line = token.line == final_generated_line;

        if should_map_standard_token_start(token, previous, next, line, is_final_line) {
            push_entry(
                &mut entries,
                token.line,
                token.col,
                original_token.line,
                original_token.col,
            );
        }

        if should_map_standard_token_end(token, previous, line) {
            let source_col_adjust = if previous.is_some_and(|previous| previous.text == "class") {
                1
            } else {
                0
            };
            push_entry(
                &mut entries,
                token.end_line,
                token.end_col,
                original_token.end_line,
                original_token.end_col + source_col_adjust,
            );
        }
    }

    Some(build_ets_source_map(
        id,
        generated.lines().count().max(1),
        &entries,
    ))
}

#[derive(Debug)]
struct TokenInfo {
    text: String,
    start: u32,
    line: u32,
    col: u32,
    end_line: u32,
    end_col: u32,
}

fn collect_tokens(source: &str, id: &str) -> Option<Vec<TokenInfo>> {
    let allocator = Allocator::default();
    let source_type =
        SourceType::from_path(id).unwrap_or_else(|_| SourceType::ts().with_module(true));
    let parsed = Parser::new(&allocator, source, source_type)
        .with_config(TokensParserConfig)
        .parse();
    if !parsed.errors.is_empty() {
        return None;
    }
    let line_offsets = build_line_offsets_table(source);
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
        let (line, col) = offset_to_line_col_table(&line_offsets, start);
        let (end_line, end_col) = offset_to_line_col_table(&line_offsets, end);
        tokens.push(TokenInfo {
            text: source[start as usize..end as usize].to_string(),
            start,
            line,
            col,
            end_line,
            end_col,
        });
    }
    Some(tokens)
}

fn generated_import_prefix_end(source: &str, id: &str) -> u32 {
    import_prefix_end(source, id, false)
}

fn original_import_prefix_end(source: &str, id: &str) -> u32 {
    import_prefix_end(source, id, true)
}

fn import_prefix_end(source: &str, id: &str, include_trailing_blank_lines: bool) -> u32 {
    let allocator = Allocator::default();
    let source_type =
        SourceType::from_path(id).unwrap_or_else(|_| SourceType::ts().with_module(true));
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.errors.is_empty() {
        return 0;
    }

    let mut end = 0usize;
    for statement in &parsed.program.body {
        if matches!(statement, Statement::ImportDeclaration(_)) {
            end = line_end_after(source, statement.span().end as usize);
            continue;
        }
        break;
    }

    if include_trailing_blank_lines {
        end = consume_blank_lines(source, end);
    }
    end as u32
}

fn line_end_after(source: &str, offset: usize) -> usize {
    source
        .get(offset..)
        .and_then(|rest| rest.find('\n').map(|newline| offset + newline + 1))
        .unwrap_or(offset)
}

fn consume_blank_lines(source: &str, mut offset: usize) -> usize {
    while let Some(rest) = source.get(offset..) {
        if rest.is_empty() {
            break;
        }
        let line_end = rest
            .find('\n')
            .map(|newline| offset + newline + 1)
            .unwrap_or(source.len());
        let Some(line) = source.get(offset..line_end) else {
            break;
        };
        if !line.trim().is_empty() {
            break;
        }
        offset = line_end;
    }
    offset
}

fn collect_original_kit_bindings(
    source: &str,
    id: &str,
) -> std::collections::HashMap<String, (u32, u32)> {
    let allocator = Allocator::default();
    let source_type =
        SourceType::from_path(id).unwrap_or_else(|_| SourceType::ts().with_module(true));
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.errors.is_empty() {
        return std::collections::HashMap::new();
    }

    let mut bindings = std::collections::HashMap::new();
    for statement in &parsed.program.body {
        let Statement::ImportDeclaration(import) = statement else {
            continue;
        };
        if !crate::kit_map::is_kit_specifier(import.source.value.as_str()) {
            continue;
        }
        let Some(specifiers) = import.specifiers.as_ref() else {
            continue;
        };
        for specifier in specifiers {
            let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else {
                continue;
            };
            let name = specifier.local.name.as_str();
            let span = specifier.local.span;
            bindings.insert(name.to_string(), (span.start, span.end));
        }
    }
    bindings
}

fn previous_token_on_line<'a>(tokens: &[&'a TokenInfo], index: usize) -> Option<&'a TokenInfo> {
    index
        .checked_sub(1)
        .and_then(|prev| tokens.get(prev).copied())
        .filter(|prev| prev.line == tokens[index].line)
}

fn next_token_on_line<'a>(tokens: &[&'a TokenInfo], index: usize) -> Option<&'a TokenInfo> {
    tokens
        .get(index + 1)
        .copied()
        .filter(|next| next.line == tokens[index].line)
}

fn should_map_standard_token_start(
    token: &TokenInfo,
    previous: Option<&TokenInfo>,
    next: Option<&TokenInfo>,
    line: &str,
    is_final_line: bool,
) -> bool {
    if is_final_line && token.text == "}" {
        return false;
    }
    if matches!(token.text.as_str(), "class" | "extends" | "=") {
        return false;
    }
    if token.text == "(" && previous.is_some_and(|previous| previous.text == "if") {
        return false;
    }
    if token.text == ")" && previous.is_some_and(|previous| previous.text == "(") {
        return false;
    }
    if token.text == ":" && next.is_some_and(|next| next.text == "void") {
        return false;
    }
    if token.text == "{"
        && !(previous.is_some_and(|previous| previous.text == ")") && line.contains("if "))
    {
        return false;
    }
    if token.text == ";" && previous.is_some_and(|previous| previous.text == "return") {
        return false;
    }
    if token.text == "}" && line.starts_with("            }") && line.trim() == "}" {
        return false;
    }
    true
}

fn should_map_standard_token_end(
    token: &TokenInfo,
    previous: Option<&TokenInfo>,
    line: &str,
) -> bool {
    if matches!(token.text.as_str(), "export" | "default" | "void" | "=>") {
        return true;
    }
    if token.text == ";" {
        return true;
    }
    if token.text == "}" && line.trim() == "}" {
        return true;
    }
    if line.trim_start().starts_with("const ")
        && (previous.is_some_and(|previous| previous.text == "const" || previous.text == "="))
    {
        return true;
    }
    if previous.is_some_and(|previous| previous.text == "class" || previous.text == "extends")
        && is_identifier_like(&token.text)
    {
        return true;
    }
    false
}

fn is_identifier_like(text: &str) -> bool {
    text.as_bytes()
        .first()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || *byte == b'_' || *byte == b'$')
}

fn push_entry(entries: &mut Vec<MapEntry>, line: u32, col: u32, src_line: u32, src_col: u32) {
    if entries.last().is_some_and(|entry| {
        entry.generated_line == line as usize
            && entry.generated_col == col
            && entry.src_line == src_line
            && entry.src_col == src_col
    }) {
        return;
    }
    entries.push(MapEntry {
        generated_line: line as usize,
        generated_col: col,
        src_line,
        src_col,
    });
}

fn build_line_offsets_table(source: &str) -> Vec<u32> {
    let mut v = vec![0u32];
    for (i, b) in source.bytes().enumerate() {
        if b == b'\n' {
            v.push((i + 1) as u32);
        }
    }
    v
}

fn offset_to_line_col_table(offsets: &[u32], offset: u32) -> (u32, u32) {
    let idx = offsets.partition_point(|&o| o <= offset).saturating_sub(1);
    (idx as u32, offset - offsets[idx])
}
