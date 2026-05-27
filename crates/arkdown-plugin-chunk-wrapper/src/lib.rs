//! IIFE chunk wrapper plugin for arkdown.
//!
//! Wraps every output chunk in `(function () { ... })()` and rewrites
//! cross-chunk `require()` calls to `globalThis[...]` lookups as required by
//! ArkRuntime (which cannot execute raw CommonJS or ESM directly).

pub mod plugin;
pub mod registry;
pub mod rewrite;

pub use plugin::ChunkWrapperPlugin;

#[cfg(test)]
mod tests {
    use super::plugin::ChunkWrapperPlugin;
    use arkdown_core::{ArkdownPlugin, PluginContext, RenderedChunkMeta};

    fn make_chunk_meta(name: &str) -> RenderedChunkMeta {
        RenderedChunkMeta {
            name: name.to_string(),
            chunk_hash: "a1b2c3d4".to_string(),
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
    async fn test_iife_wrap_basic() {
        let plugin = ChunkWrapperPlugin::new(false);
        let chunk = make_chunk_meta("Entry");
        let code = "var x = 1;";
        let result = plugin
            .render_chunk(&PluginContext::default(), code, &chunk)
            .await
            .unwrap();
        let rendered = result.unwrap();
        assert!(rendered.code.starts_with("(function () {"));
        assert!(rendered.code.contains("var x = 1;"));
        assert!(rendered.code.contains("})()"));
    }

    #[tokio::test]
    async fn test_require_rewrite() {
        let plugin = ChunkWrapperPlugin::new(true);
        let code = r#"var _helper = require("./helper");"#;
        let chunk = make_chunk_meta("Entry");
        let result = plugin
            .render_chunk(&PluginContext::default(), code, &chunk)
            .await
            .unwrap();
        let rendered = result.unwrap();
        assert!(rendered.code.contains("globalThis["));
        assert!(!rendered.code.contains("require("));
    }

    #[tokio::test]
    async fn test_external_require_unchanged() {
        let plugin = ChunkWrapperPlugin::new(true);
        let code = r#"var fs = require("fs");"#;
        let chunk = make_chunk_meta("Entry");
        let result = plugin
            .render_chunk(&PluginContext::default(), code, &chunk)
            .await
            .unwrap();
        let rendered = result.unwrap();
        // Node built-ins (no ./ prefix) must not be rewritten
        assert!(rendered.code.contains("require(\"fs\")"));
    }

    #[tokio::test]
    async fn test_registry_export_appended() {
        let plugin = ChunkWrapperPlugin::new(false);
        let chunk = make_chunk_meta("Entry");
        let code = "var x = 1;";
        let result = plugin
            .render_chunk(&PluginContext::default(), code, &chunk)
            .await
            .unwrap();
        let rendered = result.unwrap();
        // Registry line must be present and reference the chunk key
        assert!(rendered.code.contains("globalThis[\"Entry_a1b2c3d4\"]"));
    }

    #[tokio::test]
    async fn test_support_chunks_false_no_rewrite() {
        let plugin = ChunkWrapperPlugin::new(false);
        let code = r#"var _h = require("./helper");"#;
        let chunk = make_chunk_meta("Entry");
        let result = plugin
            .render_chunk(&PluginContext::default(), code, &chunk)
            .await
            .unwrap();
        let rendered = result.unwrap();
        // When support_chunks=false, require() calls are NOT rewritten
        assert!(rendered.code.contains("require("));
    }

    #[tokio::test]
    async fn test_iife_structure() {
        let plugin = ChunkWrapperPlugin::new(false);
        let chunk = make_chunk_meta("MyPage");
        let code = "console.log('hello');";
        let result = plugin
            .render_chunk(&PluginContext::default(), code, &chunk)
            .await
            .unwrap();
        let rendered = result.unwrap();
        // Verify the IIFE is syntactically correct: opens and closes
        assert!(rendered.code.contains("(function () {\n"));
        assert!(rendered.code.contains("\n})()"));
    }
}
