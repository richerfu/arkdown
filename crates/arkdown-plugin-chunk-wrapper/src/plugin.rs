use std::borrow::Cow;

use rolldown_plugin::{
    HookRenderChunkArgs, HookRenderChunkOutput, HookRenderChunkReturn, HookUsage, Plugin,
    PluginContext,
};

use crate::module_split::split_imports;
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
