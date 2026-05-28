use oxc_allocator::Allocator;
use oxc_codegen::Codegen;
use oxc_parser::{Parser, ParserReturn};
use oxc_semantic::SemanticBuilder;
use oxc_span::SourceType;
use oxc_transformer::{TransformOptions, Transformer};

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

/// Re-parse and re-emit a single module's source, stripping TypeScript type
/// annotations via `oxc_transformer`.
///
/// Pipeline:
/// 1. Parse with TypeScript+module source type.
/// 2. Build semantic information (required by the transformer).
/// 3. Run `Transformer` with default `TransformOptions` — this strips all
///    TypeScript annotations, decorators, and enums from the AST in-place.
/// 4. Emit JavaScript via `Codegen`.
///
/// Falls back to the raw source string on parse errors or transformer panics
/// so that downstream stages can still attempt to process the module.
fn regenerate_module(
    source: &str,
    file_path: &str,
    module_type: ModuleType,
) -> Result<String, ArkdownError> {
    let allocator = Allocator::default();
    let source_type = infer_source_type(file_path, &module_type);
    let path = std::path::Path::new(file_path);

    let ParserReturn { mut program, errors, .. } =
        Parser::new(&allocator, source, source_type).parse();

    if !errors.is_empty() {
        return Ok(source.to_owned());
    }

    // Build semantic analysis — required by the transformer to resolve scopes
    // and symbol bindings.
    let scoping = SemanticBuilder::new().build(&program).semantic.into_scoping();

    // TransformOptions::default() enables TypeScript stripping (TypeScriptOptions
    // is the default preset, strips all type annotations and type-only constructs).
    let transform_options = TransformOptions::default();

    // Catch any transformer panic as a safety net (should not occur in 0.133.0).
    let transform_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Transformer::new(&allocator, path, &transform_options)
                .build_with_scoping(scoping, &mut program)
        }));

    match transform_result {
        Ok(_) => {
            // `program` is now a JavaScript AST — all TypeScript annotations removed.
            let code = Codegen::new().build(&program).code;
            Ok(code)
        }
        Err(_) => Ok(source.to_owned()),
    }
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
    fn test_regenerate_module_ets_strips_typescript() {
        // Verify that TypeScript type annotations are stripped from ETS source.
        // Previously Codegen preserved them; now Transformer removes them so
        // es2abc can parse the output as plain JavaScript.
        let src = r#"import { UIAbility } from "@kit.AbilityKit";
export default class EntryAbility extends UIAbility {
    onCreate(want: any, launchParam: any): void {
        console.log("onCreate");
    }
}"#;
        let result = regenerate_module(src, "EntryAbility.ets", ModuleType::Ets).unwrap();
        // TypeScript annotations MUST be stripped.
        assert!(!result.contains(": any"), "type annotations should be stripped: {result}");
        assert!(!result.contains(": void"), "return type should be stripped: {result}");
        // Class structure must survive.
        assert!(result.contains("class EntryAbility"), "class must survive stripping: {result}");
        assert!(result.contains("onCreate"), "method must survive stripping: {result}");
    }
}
