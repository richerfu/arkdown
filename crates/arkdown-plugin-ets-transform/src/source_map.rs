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
    let col = prefix.rfind('\n').map(|pos| clamped - pos - 1).unwrap_or(clamped) as u32;
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
    sorted.sort_by_key(|e| e.generated_line);

    let mut mappings = String::new();
    let mut entry_idx = 0usize;
    let mut prev_src_line = 0i32;
    let mut prev_src_col = 0i32;

    for gen_line in 0..generated_line_count {
        if gen_line > 0 {
            mappings.push(';');
        }
        if entry_idx < sorted.len() && sorted[entry_idx].generated_line == gen_line {
            let e = &sorted[entry_idx];
            let src_line_delta = e.src_line as i32 - prev_src_line;
            let src_col_delta = e.src_col as i32 - prev_src_col;
            prev_src_line = e.src_line as i32;
            prev_src_col = e.src_col as i32;
            // Segment: generated_col=0, source_idx=0, orig_line_delta, orig_col_delta
            mappings.push_str(&encode_vlq(0));
            mappings.push_str(&encode_vlq(0));
            mappings.push_str(&encode_vlq(src_line_delta));
            mappings.push_str(&encode_vlq(src_col_delta));
            entry_idx += 1;
            // Advance past duplicate entries for the same generated line (keep first).
            while entry_idx < sorted.len() && sorted[entry_idx].generated_line == gen_line {
                entry_idx += 1;
            }
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
        let entries = vec![MapEntry { generated_line: 2, src_line: 5, src_col: 4 }];
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
