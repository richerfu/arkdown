use std::borrow::Cow;

use oxc_allocator::Allocator;
use oxc_ast::ast::Statement;
use oxc_parser::Parser;
use oxc_span::SourceType;
use rolldown_plugin::{
    HookRenderChunkArgs, HookRenderChunkOutput, HookRenderChunkReturn, HookUsage, Plugin,
    PluginContext,
};

use crate::registry;

pub struct ChunkWrapperPlugin {
    pub support_chunks: bool,
}

impl std::fmt::Debug for ChunkWrapperPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChunkWrapperPlugin")
            .field("support_chunks", &self.support_chunks)
            .finish()
    }
}

impl ChunkWrapperPlugin {
    pub fn new(support_chunks: bool) -> Self {
        ChunkWrapperPlugin { support_chunks }
    }

    pub(crate) fn wrap_chunk(
        &self,
        code: &str,
        chunk_name: &str,
    ) -> anyhow::Result<Option<HookRenderChunkOutput>> {
        let (imports, body) = split_imports(code);

        let registry = registry::generate_registry_export(chunk_name, "");

        let iife_content = if self.support_chunks {
            crate::rewrite::replace_require_calls(&body, "")
        } else {
            body
        };

        let mut output =
            String::with_capacity(imports.len() + iife_content.len() + registry.len() + 64);

        if !imports.is_empty() {
            output.push_str(&imports);
            output.push('\n');
        }

        output.push_str("(function () {\n");
        output.push_str(&iife_content);
        if !registry.is_empty() {
            output.push('\n');
            output.push_str(&registry);
        }
        if !output.ends_with('\n') {
            output.push('\n');
        }
        output.push_str("})();\n");

        Ok(Some(HookRenderChunkOutput {
            code: output,
            map: None,
        }))
    }
}

impl Plugin for ChunkWrapperPlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("chunk-wrapper")
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::RenderChunk
    }

    fn render_chunk(
        &self,
        _ctx: &PluginContext,
        args: &HookRenderChunkArgs<'_>,
    ) -> impl std::future::Future<Output = HookRenderChunkReturn> + Send {
        let code = args.code.as_str().to_owned();
        let chunk_name = args.chunk.name.to_string();
        let support_chunks = self.support_chunks;
        async move {
            let plugin = ChunkWrapperPlugin { support_chunks };
            plugin.wrap_chunk(&code, &chunk_name)
        }
    }
}

fn split_imports(source: &str) -> (String, String) {
    let allocator = Allocator::default();
    let source_type = SourceType::tsx();
    let parse_result = Parser::new(&allocator, source, source_type).parse();

    if parse_result.panicked {
        return (String::new(), source.to_owned());
    }

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

    for &(start, end) in &import_ranges {
        imports.push_str(&source[start as usize..end as usize]);
        imports.push('\n');
    }

    let mut excluded: Vec<(u32, u32, Option<String>)> = Vec::new();
    for &(start, end) in &import_ranges {
        excluded.push((start, end, None));
    }
    for &(start, end) in &export_ranges {
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

fn strip_export_keyword(text: &str) -> String {
    let t = text.trim_start();
    if let Some(rest) = t.strip_prefix("export default ") {
        return rest.to_string();
    }
    if let Some(rest) = t.strip_prefix("export default\n") {
        return rest.to_string();
    }
    if t.starts_with("export {") || t.starts_with("export type {") {
        return String::new();
    }
    if let Some(rest) = t.strip_prefix("export ") {
        return rest.to_string();
    }
    t.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_imports_hoisted_before_iife() {
        let plugin = ChunkWrapperPlugin::new(false);
        let code = r#"import { UIAbility } from "@kit.AbilityKit";
const x = 1;"#;
        let result = plugin.wrap_chunk(code, "Entry").unwrap().unwrap();

        let import_pos = result.code.find("import {").unwrap();
        let iife_pos = result.code.find("(function ()").unwrap();
        assert!(import_pos < iife_pos, "imports must be before IIFE");
    }

    #[test]
    fn test_iife_present() {
        let plugin = ChunkWrapperPlugin::new(false);
        let result = plugin
            .wrap_chunk("const x = 1;\n", "Entry")
            .unwrap()
            .unwrap();
        assert!(result.code.contains("(function () {"));
        assert!(result.code.contains("})();"));
    }

    #[test]
    fn test_iife_wrap_basic() {
        let plugin = ChunkWrapperPlugin::new(false);
        let result = plugin.wrap_chunk("var x = 1;", "Entry").unwrap().unwrap();
        assert!(result.code.starts_with("(function () {"));
        assert!(result.code.contains("var x = 1;"));
        assert!(result.code.contains("})()"));
    }

    #[test]
    fn test_require_rewrite() {
        let plugin = ChunkWrapperPlugin::new(true);
        let code = r#"var _helper = require("./helper");"#;
        let result = plugin.wrap_chunk(code, "Entry").unwrap().unwrap();
        assert!(result.code.contains("globalThis["));
        assert!(!result.code.contains("require("));
    }

    #[test]
    fn test_external_require_unchanged() {
        let plugin = ChunkWrapperPlugin::new(true);
        let code = r#"var fs = require("fs");"#;
        let result = plugin.wrap_chunk(code, "Entry").unwrap().unwrap();
        assert!(result.code.contains("require(\"fs\")"));
    }

    #[test]
    fn test_support_chunks_false_no_rewrite() {
        let plugin = ChunkWrapperPlugin::new(false);
        let code = r#"var _h = require("./helper");"#;
        let result = plugin.wrap_chunk(code, "Entry").unwrap().unwrap();
        assert!(result.code.contains("require("));
    }

    #[test]
    fn test_iife_structure() {
        let plugin = ChunkWrapperPlugin::new(false);
        let result = plugin
            .wrap_chunk("console.log('hello');", "MyPage")
            .unwrap()
            .unwrap();
        assert!(result.code.contains("(function () {\n"));
        assert!(result.code.contains("\n})()"));
    }
}
