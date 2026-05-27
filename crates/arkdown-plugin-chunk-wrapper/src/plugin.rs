use async_trait::async_trait;
use arkdown_core::{
    ArkdownPlugin, HookTransformOutput, HookUsage, PluginContext, PluginResult, RenderedChunkMeta,
};

use crate::{registry, rewrite};

/// Plugin that wraps every output chunk in an IIFE and rewrites cross-chunk
/// `require()` calls to `globalThis[...]` lookups.
///
/// # ArkRuntime requirement
/// ArkRuntime does **not** execute raw ES module or CommonJS code directly.
/// Every chunk must be wrapped in `(function () { ... })()` before it is
/// handed to `arkdown-abc` for bytecode compilation.
///
/// # `support_chunks`
/// - `false` (default): single entry chunk per OHOS module, no cross-chunk
///   `require()` calls. The IIFE is emitted but no require-rewriting occurs.
/// - `true`: multiple chunks with cross-chunk references. Each
///   `require('./chunkId')` is replaced with `globalThis['chunkId_<hash>']`.
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
        let mut output = String::with_capacity(code.len() + 128);

        output.push_str("(function () {\n");

        if self.support_chunks {
            // Replace var x = require('./chunkId') with globalThis['chunkId_hash'].
            // Without a chunk manifest in the context, we do a best-effort pass
            // using the require path as a hash key placeholder.
            let rewritten = rewrite::replace_require_calls(code, &chunk.chunk_hash);
            output.push_str(&rewritten);
        } else {
            output.push_str(code);
        }

        output.push_str("\n})()");

        // Append globalThis registry export for the current chunk.
        // Only emitted when the chunk has a non-empty hash (all real chunks do).
        if !chunk.chunk_hash.is_empty() {
            let reg = registry::generate_registry_export(&chunk.name, &chunk.chunk_hash);
            if !reg.is_empty() {
                output.push('\n');
                output.push_str(&reg);
            }
        }

        Ok(Some(HookTransformOutput { code: output, source_map: None }))
    }
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

    #[tokio::test]
    async fn test_iife_wrapping_applied() {
        let plugin = ChunkWrapperPlugin::new(false);
        let chunk = make_chunk("Entry", "abc123");
        let result = plugin
            .render_chunk(&PluginContext::default(), "const x = 1;\n", &chunk)
            .await
            .unwrap()
            .unwrap();

        assert!(
            result.code.starts_with("(function () {\n"),
            "output must start with IIFE prefix; got: {:?}",
            &result.code[..result.code.len().min(40)]
        );
        assert!(
            result.code.contains("\n})()"),
            "output must contain IIFE suffix"
        );
    }

    #[tokio::test]
    async fn test_iife_preserves_inner_code() {
        let plugin = ChunkWrapperPlugin::new(false);
        let chunk = make_chunk("Entry", "abc123");
        let result = plugin
            .render_chunk(&PluginContext::default(), "const x = 1;", &chunk)
            .await
            .unwrap()
            .unwrap();

        assert!(
            result.code.contains("const x = 1;"),
            "inner code must be preserved inside the IIFE"
        );
    }

    #[tokio::test]
    async fn test_registry_export_appended_with_hash() {
        let plugin = ChunkWrapperPlugin::new(false);
        let chunk = make_chunk("entry", "deadbeef");
        let result = plugin
            .render_chunk(&PluginContext::default(), "var x = 1;", &chunk)
            .await
            .unwrap()
            .unwrap();

        // Registry line format: globalThis["entry_deadbeef"] = ...
        assert!(
            result.code.contains("globalThis[\"entry_deadbeef\"]"),
            "registry export line must contain globalThis key; code:\n{}",
            result.code
        );
    }

    #[tokio::test]
    async fn test_no_registry_export_when_hash_empty() {
        let plugin = ChunkWrapperPlugin::new(false);
        let chunk = make_chunk("entry", ""); // empty hash
        let result = plugin
            .render_chunk(&PluginContext::default(), "var x = 1;", &chunk)
            .await
            .unwrap()
            .unwrap();

        assert!(
            !result.code.contains("globalThis"),
            "no registry export should be appended when chunk_hash is empty"
        );
    }

    #[tokio::test]
    async fn test_require_rewritten_in_support_chunks_mode() {
        let plugin = ChunkWrapperPlugin::new(true); // support_chunks = true
        let chunk = make_chunk("MainAbility", "ff00ff");
        let code = r#"var _shared = require("./shared");"#;
        let result = plugin
            .render_chunk(&PluginContext::default(), code, &chunk)
            .await
            .unwrap()
            .unwrap();

        assert!(
            result.code.contains("globalThis[\"shared_"),
            "require('./shared') must be rewritten to globalThis[...] in support_chunks mode; got:\n{}",
            result.code
        );
        assert!(
            !result.code.contains("require("),
            "no bare require() calls should remain after rewrite"
        );
    }

    #[tokio::test]
    async fn test_require_not_rewritten_in_single_chunk_mode() {
        let plugin = ChunkWrapperPlugin::new(false); // support_chunks = false
        let chunk = make_chunk("Entry", "abc123");
        // Even though the code contains a require, it must NOT be rewritten
        // in single-chunk mode (there are no cross-chunk references).
        let code = r#"var _h = require("./helper");"#;
        let result = plugin
            .render_chunk(&PluginContext::default(), code, &chunk)
            .await
            .unwrap()
            .unwrap();

        assert!(
            result.code.contains("require("),
            "require() must be left as-is in single-chunk mode"
        );
    }
}
