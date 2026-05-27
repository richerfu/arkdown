/// Replace `var X = require('./chunkId')` with `var X = globalThis['chunkId_<hash>']`.
/// External requires (e.g. node builtins) are left unchanged.
pub fn replace_require_calls(code: &str, _current_chunk_hash: &str) -> String {
    // Simple regex-free line-by-line replacement
    let mut result = String::with_capacity(code.len());
    for line in code.lines() {
        let replaced = try_replace_require_line(line);
        result.push_str(&replaced);
        result.push('\n');
    }
    // Remove trailing newline added by loop
    if result.ends_with('\n') {
        result.pop();
    }
    result
}

fn try_replace_require_line(line: &str) -> std::borrow::Cow<'_, str> {
    // Match: var <name> = require('<path>')
    // Very common patterns in rolldown CJS output
    let trimmed = line.trim();
    if !trimmed.contains("require(") {
        return std::borrow::Cow::Borrowed(line);
    }

    // Find require('<path>') and extract the path — double-quote form first
    if let Some(start) = trimmed.find("require(\"") {
        let after = &trimmed[start + 9..];
        if let Some(end) = after.find('"') {
            let chunk_id = &after[..end];
            // Only rewrite if it looks like a local chunk (starts with ./ or ../)
            if chunk_id.starts_with("./") || chunk_id.starts_with("../") {
                let name = chunk_id
                    .trim_start_matches("./")
                    .trim_start_matches("../")
                    .trim_end_matches(".js");
                let hash = chunk_name_hash(name);
                let key = format!("{}_{:x}", name, hash);
                let new_line = line.replace(
                    &format!("require(\"{}\")", chunk_id),
                    &format!("globalThis[\"{}\"]", key),
                );
                return std::borrow::Cow::Owned(new_line);
            }
        }
    }

    // Also try single-quote form
    if let Some(start) = trimmed.find("require('") {
        let after = &trimmed[start + 9..];
        if let Some(end) = after.find('\'') {
            let chunk_id = &after[..end];
            if chunk_id.starts_with("./") || chunk_id.starts_with("../") {
                let name = chunk_id
                    .trim_start_matches("./")
                    .trim_start_matches("../")
                    .trim_end_matches(".js");
                let hash = chunk_name_hash(name);
                let key = format!("{}_{:x}", name, hash);
                let new_line = line.replace(
                    &format!("require('{}')", chunk_id),
                    &format!("globalThis[\"{}\"]", key),
                );
                return std::borrow::Cow::Owned(new_line);
            }
        }
    }

    std::borrow::Cow::Borrowed(line)
}

fn chunk_name_hash(name: &str) -> u32 {
    let mut h: u32 = 0x811c9dc5;
    for b in name.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x01000193);
    }
    h & 0xFFFFFFFF
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_require_unchanged() {
        let code = "var x = 1;";
        assert_eq!(replace_require_calls(code, ""), code);
    }

    #[test]
    fn test_external_require_unchanged() {
        let code = r#"var fs = require("fs");"#;
        let result = replace_require_calls(code, "");
        assert!(result.contains(r#"require("fs")"#));
    }

    #[test]
    fn test_local_require_double_quote() {
        let code = r#"var _h = require("./helper");"#;
        let result = replace_require_calls(code, "");
        assert!(result.contains("globalThis[\"helper_"));
        assert!(!result.contains("require("));
    }

    #[test]
    fn test_local_require_single_quote() {
        let code = "var _h = require('./helper');";
        let result = replace_require_calls(code, "");
        assert!(result.contains("globalThis[\"helper_"));
        assert!(!result.contains("require("));
    }

    #[test]
    fn test_local_require_with_js_extension() {
        let code = r#"var _h = require("./helper.js");"#;
        let result = replace_require_calls(code, "");
        // .js suffix is stripped from the key name
        assert!(result.contains("globalThis[\"helper_"));
    }

    #[test]
    fn test_multiline_code() {
        let code = "var a = 1;\nvar _h = require(\"./shared\");\nvar b = 2;";
        let result = replace_require_calls(code, "");
        assert!(result.contains("var a = 1;"));
        assert!(result.contains("globalThis[\"shared_"));
        assert!(result.contains("var b = 2;"));
    }
}
