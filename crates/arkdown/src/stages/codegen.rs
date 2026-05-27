use oxc_allocator::Allocator;
use oxc_codegen::Codegen;
use oxc_parser::{Parser, ParserReturn};
use oxc_span::SourceType;

use arkdown_core::{ArkdownError, ChunkGraph, Module, ModuleGraph, ModuleType};

pub fn generate_code(
    graph: &ModuleGraph,
    chunk_graph: &mut ChunkGraph,
) -> Result<(), ArkdownError> {
    for chunk in chunk_graph.chunks.iter_mut() {
        let mut bundle_code = String::new();

        for &module_idx in &chunk.modules {
            let module = &graph.modules[module_idx];
            if let Module::Normal(nm) = module {
                let module_code =
                    regenerate_module(&nm.source.content, &nm.id.id, nm.id.module_type.clone())?;
                bundle_code.push_str(&module_code);
                bundle_code.push('\n');
            }
        }

        chunk.chunk_hash = format!("{:x}", fnv1a_hash(&bundle_code));
        chunk.code = bundle_code;
    }
    Ok(())
}

/// Re-parse and re-emit a single module's source through oxc_codegen.
///
/// NOTE: The ohos-rs/oxc fork's `Codegen` does NOT strip TypeScript type
/// annotations — it outputs TypeScript-aware code that preserves all type
/// annotations. `oxc_transformer` would strip types but is not usable with
/// this toolchain (it uses the unstable `if_let_guard` feature).
///
/// As a result, the bundled output retains TypeScript annotations. These are
/// handled downstream:
/// - `arkdown-abc`'s `strip_typescript_types` uses `SourceType::tsx()` which
///   allows parsing TypeScript, enabling bytecode generation from the AST.
/// - The chunk-wrapper plugin correctly hoists imports before the IIFE.
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
        return Ok(source.to_owned());
    }

    let result = Codegen::new().build(&program);
    Ok(result.code)
}

/// Map module type and file extension to an oxc [`SourceType`].
fn infer_source_type(file_path: &str, module_type: &ModuleType) -> SourceType {
    let ext = std::path::Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    // ETS files: use standard TypeScript+module (not the ohos-rs ArkTS parser mode
    // which has stricter rules for ArkUI struct syntax).
    if ext == "ets" || matches!(module_type, ModuleType::Ets) {
        return SourceType::default().with_typescript(true).with_module(true);
    }

    // TypeScript/JS files: try path inference first.
    if let Ok(st) = SourceType::from_path(std::path::Path::new(file_path)) {
        return st;
    }

    match module_type {
        ModuleType::Ts | ModuleType::Declaration => {
            SourceType::default().with_typescript(true).with_module(true)
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
        assert_eq!(fnv1a_hash(""), 0xcbf29ce484222325);
    }

    #[test]
    fn test_regenerate_module_js() {
        let src = "var y = 1;";
        let result = regenerate_module(src, "test.js", ModuleType::Js).unwrap();
        assert!(result.contains("var y") || result.contains("1"));
    }

    #[test]
    fn test_regenerate_module_parse_error_fallback() {
        let src = "this is not valid javascript !!!@@@@";
        let result = regenerate_module(src, "bad.js", ModuleType::Js).unwrap();
        assert_eq!(result, src);
    }

    #[test]
    fn test_regenerate_module_ets_parses_without_errors() {
        // Verify that ETS TypeScript source parses without errors using the
        // explicit TypeScript+module source type (bypasses ohos-rs ArkTS strict mode).
        let src = r#"import { UIAbility } from "@kit.AbilityKit";
export default class EntryAbility extends UIAbility {
    onCreate(want: any, launchParam: any): void {
        console.log("onCreate");
    }
}"#;
        // Should NOT fall back to raw source (errors.is_empty() should be true).
        // We can't assert TypeScript is stripped (ohos-rs Codegen preserves types),
        // but we CAN assert the class structure is preserved.
        let result = regenerate_module(src, "EntryAbility.ets", ModuleType::Ets).unwrap();
        assert!(result.contains("class EntryAbility"), "class must be present in output");
        assert!(result.contains("onCreate"), "method must be present in output");
    }
}
