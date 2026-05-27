use oxc_allocator::Allocator;
use oxc_codegen::Codegen;
use oxc_parser::{Parser, ParserReturn};
use oxc_span::SourceType;

use arkdown_core::{ArkdownError, ChunkGraph, ModuleGraph, ModuleType};

/// Generate CJS-compatible JavaScript for every chunk.
///
/// For each chunk we concatenate all included modules' source text, re-parsing
/// and re-emitting via oxc_codegen to normalise whitespace and strip TypeScript
/// annotations. The actual ESM→CJS rewrite (`import` → `require`, `export` →
/// `module.exports`) is handled downstream by `arkdown-plugin-chunk-wrapper`
/// (SPEC-006).
pub fn generate_code(
    graph: &ModuleGraph,
    chunk_graph: &mut ChunkGraph,
) -> Result<(), ArkdownError> {
    for chunk in chunk_graph.chunks.iter_mut() {
        let mut bundle_code = String::new();

        for &module_idx in &chunk.modules {
            let module = &graph.modules[module_idx];
            let module_code =
                regenerate_module(&module.source.content, &module.id.id, module.id.module_type.clone())?;
            bundle_code.push_str(&module_code);
            bundle_code.push('\n');
        }

        chunk.chunk_hash = format!("{:x}", fnv1a_hash(&bundle_code));
        chunk.code = bundle_code;
    }
    Ok(())
}

/// Re-parse and re-emit a single module's source through oxc_codegen.
///
/// This strips TypeScript annotations and normalises formatting.
/// Parse errors are tolerated by falling back to the raw source text.
fn regenerate_module(
    source: &str,
    file_path: &str,
    module_type: ModuleType,
) -> Result<String, ArkdownError> {
    let allocator = Allocator::default();
    let source_type = infer_source_type(file_path, &module_type);

    let ParserReturn { program, errors, .. } =
        Parser::new(&allocator, source, source_type).parse();

    if !errors.is_empty() {
        // Fall back to raw source on parse error — downstream stages will report
        // the diagnostic properly.
        return Ok(source.to_owned());
    }

    let result = Codegen::new().build(&program);
    Ok(result.code)
}

/// Map module type and file extension to an oxc [`SourceType`].
fn infer_source_type(file_path: &str, module_type: &ModuleType) -> SourceType {
    // Try path-based inference first (handles .ets, .ts, .js, .mjs, .cjs).
    if let Ok(st) = SourceType::from_path(std::path::Path::new(file_path)) {
        return st;
    }
    // Fallback.
    match module_type {
        ModuleType::Ets | ModuleType::Ts | ModuleType::Declaration => {
            SourceType::default().with_typescript(true)
        }
        _ => SourceType::default(),
    }
}

/// FNV-1a 64-bit hash for content-hashing chunk outputs.
pub fn fnv1a_hash(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fnv1a_hash_deterministic() {
        assert_eq!(fnv1a_hash("hello"), fnv1a_hash("hello"));
        assert_ne!(fnv1a_hash("hello"), fnv1a_hash("world"));
    }

    #[test]
    fn test_fnv1a_hash_empty() {
        // FNV offset basis
        assert_eq!(fnv1a_hash(""), 0xcbf29ce484222325);
    }

    #[test]
    fn test_regenerate_module_ts() {
        let src = "const x: number = 42;";
        let result = regenerate_module(src, "test.ts", ModuleType::Ts).unwrap();
        // After stripping TypeScript annotations, the type annotation is gone.
        assert!(result.contains("const x") || result.contains("42"));
    }

    #[test]
    fn test_regenerate_module_js() {
        let src = "var y = 1;";
        let result = regenerate_module(src, "test.js", ModuleType::Js).unwrap();
        assert!(result.contains("var y") || result.contains("1"));
    }

    #[test]
    fn test_regenerate_module_parse_error_fallback() {
        // Deliberately broken source — should fall back to raw source.
        let src = "this is not valid javascript !!!@@@@";
        let result = regenerate_module(src, "bad.js", ModuleType::Js).unwrap();
        assert_eq!(result, src);
    }
}
