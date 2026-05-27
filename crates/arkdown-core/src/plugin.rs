use async_trait::async_trait;

use crate::chunk::{ArkChunk, ChunkGraph};
use crate::error::PluginResult;
use crate::graph::Module;
use crate::module::{ModuleSource, ResolvedId, SourceMap};

// ---------------------------------------------------------------------------
// Context types
// ---------------------------------------------------------------------------

/// Context available during the build start / before-build-end / build-end hooks.
#[derive(Debug, Clone)]
pub struct BuildContext {
    /// Whether this is an incremental (watch) rebuild.
    pub is_rebuild: bool,
}

/// Context available during the `resolve_id` hook.
#[derive(Debug, Clone)]
pub struct ResolveContext {
    /// The module that issued the import, if any.
    pub importer: Option<String>,
}

/// Context available during the `load` hook.
#[derive(Debug, Clone)]
pub struct LoadContext {
    /// The resolved module ID being loaded.
    pub id: String,
}

/// Context available during the `transform` hook.
#[derive(Debug, Clone)]
pub struct TransformContext {
    /// The resolved module ID being transformed.
    pub id: String,
}

/// Context available during the `module_parsed` hook.
#[derive(Debug, Clone)]
pub struct ModuleParsedContext {
    /// The resolved module ID that was just parsed.
    pub id: String,
}

/// Context available during the `render_chunk` hook.
#[derive(Debug, Clone)]
pub struct RenderChunkContext {
    /// Chunk name / file name.
    pub chunk_name: String,
}

/// Context available during the `generate_bundle` hook.
#[derive(Debug, Clone)]
pub struct GenerateBundleContext {
    /// Output directory for the bundle.
    pub output_dir: std::path::PathBuf,
}

/// Context available during the `should_invalidate_cache` hook.
#[derive(Debug, Clone)]
pub struct CacheContext {
    /// Current build configuration fingerprint (e.g. compiler version, flags).
    pub build_fingerprint: String,
}

/// A previously cached module entry (used by `should_invalidate_cache`).
#[derive(Debug, Clone)]
pub struct CachedModule {
    /// Resolved module ID.
    pub id: String,
    /// Content hash of the source at cache time.
    pub source_hash: String,
}

/// An error originating from the build phase (passed to `build_end`).
#[derive(Debug, Clone)]
pub struct BuildError {
    pub message: String,
    pub file: Option<String>,
}

/// The output of a `render_chunk` hook invocation.
#[derive(Debug, Clone)]
pub struct RenderedChunk {
    pub code: String,
    pub map: Option<SourceMap>,
}

// ---------------------------------------------------------------------------
// Plugin trait
// ---------------------------------------------------------------------------

/// The arkdown plugin trait — mirrors rolldown/rollup plugin hooks.
///
/// All hooks have default no-op implementations. Plugins implement only
/// the hooks they need.
#[async_trait]
pub trait ArkdownPlugin: Send + Sync {
    /// Plugin name (used in diagnostics).
    fn name(&self) -> &str;

    /// Called once at build start.
    async fn build_start(&self, _ctx: &mut BuildContext) -> PluginResult<()> {
        Ok(())
    }

    /// Resolve a module specifier to a [`ResolvedId`].
    /// Return `None` to defer to the next plugin.
    async fn resolve_id(
        &self,
        _ctx: &ResolveContext,
        _specifier: &str,
        _importer: Option<&str>,
    ) -> PluginResult<Option<ResolvedId>> {
        Ok(None)
    }

    /// Load the source content for a resolved module ID.
    /// Return `None` to defer to the default file loader.
    async fn load(
        &self,
        _ctx: &LoadContext,
        _id: &str,
    ) -> PluginResult<Option<ModuleSource>> {
        Ok(None)
    }

    /// Transform the source of a module.
    /// Return `None` to leave source unchanged.
    async fn transform(
        &self,
        _ctx: &TransformContext,
        _source: ModuleSource,
    ) -> PluginResult<Option<ModuleSource>> {
        Ok(None)
    }

    /// Called after a module is parsed.
    async fn module_parsed(
        &self,
        _ctx: &ModuleParsedContext,
        _module: &Module,
    ) -> PluginResult<()> {
        Ok(())
    }

    /// Called before the build ends (all modules parsed).
    async fn before_build_end(&self, _ctx: &mut BuildContext) -> PluginResult<()> {
        Ok(())
    }

    /// Called when the build ends (all chunks generated).
    async fn build_end(
        &self,
        _ctx: &BuildContext,
        _error: Option<&BuildError>,
    ) -> PluginResult<()> {
        Ok(())
    }

    /// Transform a rendered chunk's code. Used by `arkdown-plugin-chunk-wrapper`.
    async fn render_chunk(
        &self,
        _ctx: &RenderChunkContext,
        _code: String,
        _chunk: &ArkChunk,
    ) -> PluginResult<Option<RenderedChunk>> {
        Ok(None)
    }

    /// Called after all chunks are rendered, before file write.
    async fn generate_bundle(
        &self,
        _ctx: &mut GenerateBundleContext,
        _bundle: &mut ChunkGraph,
        _is_write: bool,
    ) -> PluginResult<()> {
        Ok(())
    }

    /// Decide whether an incremental cache entry is still valid.
    fn should_invalidate_cache(
        &self,
        _ctx: &CacheContext,
        _cached: &CachedModule,
    ) -> bool {
        false
    }
}

/// Convenience alias — a boxed, object-safe plugin.
pub type BoxedPlugin = Box<dyn ArkdownPlugin>;
