use async_trait::async_trait;
use oxc_allocator::Allocator;
use oxc_ast::ast::Statement;
use oxc_parser::Parser;
use oxc_span::SourceType;

use arkdown_core::{
    ArkdownPlugin, HookTransformOutput, HookUsage, PluginContext, PluginResult, RenderedChunkMeta,
};

use crate::registry;

/// Plugin that produces ArkRuntime-compatible output for each chunk.
///
/// # Output format
///
/// ```js
/// // 1. ESM import declarations hoisted to top level (before IIFE).
/// //    ArkRuntime resolves these from built-in SDK modules at load time.
/// import { UIAbility } from "@kit.AbilityKit";
///
/// // 2. IIFE wraps the module body and the globalThis registry export.
/// //    ArkRuntime requires all user code to be inside an IIFE.
/// (function () {
///   "use strict";
///   <module body — no import/export declarations>
///   globalThis["chunkName_hash"] = globalThis["chunkName_hash"] || { ... };
/// })();
/// ```
///
/// # Why imports must be hoisted
///
/// ES modules only allow `import` declarations at the top level of a file.
/// Placing them inside a function body `(function() { import ... })()` is a
/// syntax error that es2abc and ArkRuntime both reject.
///
/// # Why the registry goes inside the IIFE
///
/// The module's class / function definitions are created inside the IIFE scope.
/// The `globalThis` registry assignment must also be inside the IIFE so that
/// it can reference those locally defined names.
pub struct ChunkWrapperPlugin {
    pub support_chunks: bool,
}

impl ChunkWrapperPlugin {
    pub fn new(support_chunks: bool) -> Self {
        ChunkWrapperPlugin { support_chunks }
    }
}

#[async_trait]
impl ArkdownPlugin for ChunkWrapperPlugin {
    fn name(&self) -> &str {
        "chunk-wrapper"
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::RenderChunk
    }

    async fn render_chunk(
        &self,
        _ctx: &PluginContext,
        code: &str,
        chunk: &RenderedChunkMeta,
    ) -> PluginResult<Option<HookTransformOutput>> {
        // Split the chunk code into top-level import declarations and the body.
        let (imports, body) = split_imports(code);

        // Build IIFE body: stripped code + globalThis registry export.
        let registry = if !chunk.chunk_hash.is_empty() {
            registry::generate_registry_export(&chunk.name, &chunk.chunk_hash)
        } else {
            String::new()
        };

        let iife_content = if self.support_chunks {
            crate::rewrite::replace_require_calls(&body, &chunk.chunk_hash)
        } else {
            body
        };

        let mut output =
            String::with_capacity(imports.len() + iife_content.len() + registry.len() + 64);

        // 1. Hoisted import declarations (top-level, before IIFE).
        if !imports.is_empty() {
            output.push_str(&imports);
            output.push('\n');
        }

        // 2. IIFE wrapping module body + registry export (both inside the function).
        output.push_str("(function () {\n");
        output.push_str(&iife_content);
        if !registry.is_empty() {
            output.push('\n');
            output.push_str(&registry);
        }
        output.push_str("})();\n");

        Ok(Some(HookTransformOutput {
            code: output,
            source_map: None,
        }))
    }
}

/// Split `source` into `(import_declarations, body)`.
///
/// Import declarations (`import ... from "..."`) are extracted and returned
/// as the first element.  The second element contains all remaining source
/// text with those declarations replaced by empty lines (preserving line
/// numbers for source-map fidelity).
///
/// Also strips `export` keywords from declarations in the body because:
/// - `export default class Foo {}` → `class Foo {}` (class stays in scope for the registry)
/// - `export { Foo }` → removed entirely (the registry handles re-exports)
fn split_imports(source: &str) -> (String, String) {
    let allocator = Allocator::default();
    let source_type = SourceType::tsx();
    let parse_result = Parser::new(&allocator, source, source_type).parse();

    if parse_result.panicked {
        // Parser panicked — return source as body unchanged.
        return (String::new(), source.to_owned());
    }

    // Collect byte ranges of import declarations and export declarations.
    let mut import_ranges: Vec<(u32, u32)> = Vec::new();
    let mut export_ranges: Vec<(u32, u32)> = Vec::new();

    for stmt in &parse_result.program.body {
        match stmt {
            Statement::ImportDeclaration(decl) => {
                import_ranges.push((decl.span.start, decl.span.end));
            }
            Statement::ExportNamedDeclaration(d) => {
                export_ranges.push((d.span.start, d.span.end));
            }
            Statement::ExportDefaultDeclaration(d) => {
                export_ranges.push((d.span.start, d.span.end));
            }
            Statement::ExportAllDeclaration(d) => {
                export_ranges.push((d.span.start, d.span.end));
            }
            _ => {}
        }
    }

    if import_ranges.is_empty() && export_ranges.is_empty() {
        return (String::new(), source.to_owned());
    }

    let bytes = source.as_bytes();
    let mut imports = String::new();
    let mut body = String::new();

    // Collect import texts.
    for &(start, end) in &import_ranges {
        imports.push_str(&source[start as usize..end as usize]);
        imports.push('\n');
    }

    // Build body by excluding import spans; strip `export` from export declarations.
    let mut excluded: Vec<(u32, u32, Option<String>)> = Vec::new();
    for &(start, end) in &import_ranges {
        excluded.push((start, end, None)); // replace with empty
    }
    for &(start, end) in &export_ranges {
        // Extract the declaration text and strip the leading `export [default ]` keyword.
        let text = &source[start as usize..end as usize];
        let stripped = strip_export_keyword(text);
        excluded.push((start, end, Some(stripped)));
    }
    excluded.sort_by_key(|(s, _, _)| *s);

    let mut pos = 0usize;
    for (start, end, replacement) in &excluded {
        let s = *start as usize;
        let e = *end as usize;
        if s > pos {
            body.push_str(&source[pos..s]);
        }
        match replacement {
            None => {
                // Preserve line count by emitting one blank line per newline in removed text.
                let newlines = bytes[s..e].iter().filter(|&&b| b == b'\n').count();
                for _ in 0..newlines {
                    body.push('\n');
                }
            }
            Some(stripped) => {
                body.push_str(stripped);
            }
        }
        pos = e;
    }
    if pos < source.len() {
        body.push_str(&source[pos..]);
    }

    (imports, body)
}

/// Remove the `export` (and optional `default`) prefix from a declaration.
///
/// Examples:
/// - `export default class Foo {}` → `class Foo {}`
/// - `export class Bar {}` → `class Bar {}`
/// - `export function fn() {}` → `function fn() {}`
/// - `export const x = 1;` → `const x = 1;`
/// - `export { Foo, Bar };` → `` (empty — namespace exports have no standalone value)
fn strip_export_keyword(text: &str) -> String {
    let t = text.trim_start();
    // export default ...
    if let Some(rest) = t.strip_prefix("export default ") {
        return rest.to_string();
    }
    if let Some(rest) = t.strip_prefix("export default\n") {
        return rest.to_string();
    }
    // export { ... } — remove entirely
    if t.starts_with("export {") || t.starts_with("export type {") {
        return String::new();
    }
    // export class/function/const/let/var/type
    if let Some(rest) = t.strip_prefix("export ") {
        return rest.to_string();
    }
    t.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_chunk(name: &str, hash: &str) -> RenderedChunkMeta {
        RenderedChunkMeta {
            name: name.to_string(),
            chunk_hash: hash.to_string(),
            is_entry: true,
            is_dynamic_entry: false,
            facade_module_id: None,
            module_ids: vec![],
            exports: vec![],
            imports: vec![],
            dynamic_imports: vec![],
        }
    }

    #[test]
    fn test_split_imports_basic() {
        let code = r#"import { UIAbility } from "@kit.AbilityKit";
const x = 1;"#;
        let (imports, body) = split_imports(code);
        assert!(imports.contains(r#"import { UIAbility } from "@kit.AbilityKit""#));
        assert!(!body.contains("import"));
        assert!(body.contains("const x = 1;"));
    }

    #[test]
    fn test_split_imports_strips_export_default() {
        let code = r#"export default class Foo {}"#;
        let (imports, body) = split_imports(code);
        assert!(imports.is_empty());
        assert!(!body.contains("export"));
        assert!(body.contains("class Foo {}"));
    }

    #[test]
    fn test_split_imports_no_imports() {
        let code = "const x = 1;\n";
        let (imports, body) = split_imports(code);
        assert!(imports.is_empty());
        assert_eq!(body, code);
    }

    #[tokio::test]
    async fn test_imports_hoisted_before_iife() {
        let plugin = ChunkWrapperPlugin::new(false);
        let chunk = make_chunk("Entry", "abc123");
        let code = r#"import { UIAbility } from "@kit.AbilityKit";
const x = 1;"#;
        let result = plugin
            .render_chunk(&PluginContext::default(), code, &chunk)
            .await
            .unwrap()
            .unwrap();

        // Imports must come BEFORE the IIFE
        let import_pos = result.code.find("import {").unwrap();
        let iife_pos = result.code.find("(function ()").unwrap();
        assert!(import_pos < iife_pos, "imports must be before IIFE");
    }

    #[tokio::test]
    async fn test_registry_inside_iife() {
        let plugin = ChunkWrapperPlugin::new(false);
        let chunk = make_chunk("entry", "deadbeef");
        let result = plugin
            .render_chunk(&PluginContext::default(), "var x = 1;", &chunk)
            .await
            .unwrap()
            .unwrap();

        // Registry must come BEFORE the IIFE closing
        let reg_pos = result.code.find("globalThis[\"entry_deadbeef\"]").unwrap();
        let close_pos = result.code.find("})();").unwrap();
        assert!(
            reg_pos < close_pos,
            "registry must be inside IIFE; code:\n{}",
            result.code
        );
    }

    #[tokio::test]
    async fn test_iife_present() {
        let plugin = ChunkWrapperPlugin::new(false);
        let chunk = make_chunk("Entry", "abc123");
        let result = plugin
            .render_chunk(&PluginContext::default(), "const x = 1;\n", &chunk)
            .await
            .unwrap()
            .unwrap();
        assert!(result.code.contains("(function () {"));
        assert!(result.code.contains("})();"));
    }
}
