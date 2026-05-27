use async_trait::async_trait;
use arkdown_core::{
    ArkdownPlugin, HookTransformOutput, PluginContext, PluginResult, RenderedChunkMeta,
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
