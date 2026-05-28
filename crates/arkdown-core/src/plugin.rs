use async_trait::async_trait;

use crate::chunk::ChunkGraph;
use crate::error::PluginResult;
use crate::module::{ModuleType, SourceMap};

// ---------------------------------------------------------------------------
// HookUsage bitflags — plugins declare which hooks they implement so the
// runner can skip plugins that don't implement a given hook.
// ---------------------------------------------------------------------------

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct HookUsage: u32 {
        const BuildStart            = 1 << 0;
        const ResolveId             = 1 << 1;
        const Load                  = 1 << 2;
        const Transform             = 1 << 3;
        const ModuleParsed          = 1 << 4;
        const BuildEnd              = 1 << 5;
        const RenderStart           = 1 << 6;
        const RenderChunk           = 1 << 7;
        const AugmentChunkHash      = 1 << 8;
        const GenerateBundle        = 1 << 9;
        const WriteBundle           = 1 << 10;
        const CloseBundle           = 1 << 11;
        const WatchChange           = 1 << 12;
        const CloseWatcher          = 1 << 13;
        const Banner                = 1 << 14;
        const Footer                = 1 << 15;
        const ShouldInvalidateCache = 1 << 16;
    }
}

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// The kind of module import that triggered `resolve_id` / `load`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImportKind {
    /// Static `import ... from '...'` or `export ... from '...'`.
    #[default]
    Import,
    /// Dynamic `import('...')`.
    DynamicImport,
    /// CommonJS `require('...')`.
    Require,
    /// `new URL('...')` asset reference.
    New,
}

/// Side-effect hint that a plugin can attach to a resolved module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookSideEffects {
    True,
    False,
    NoTreeshake,
}

/// The type of filesystem event passed to `watch_change`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchChangeKind {
    Create,
    Update,
    Delete,
}

// ---------------------------------------------------------------------------
// Unified plugin context (replaces the many per-hook context structs)
// ---------------------------------------------------------------------------

/// Context passed to every hook call. Intentionally minimal for now.
#[derive(Debug, Clone, Default)]
pub struct PluginContext {
    /// `true` when this is an incremental (watch-mode) rebuild.
    pub is_rebuild: bool,
}

// ---------------------------------------------------------------------------
// Hook argument structs
// ---------------------------------------------------------------------------

/// Arguments passed to the `resolve_id` hook.
pub struct HookResolveIdArgs<'a> {
    /// The import specifier as written in source (e.g. `"./utils/helper"`).
    pub specifier: &'a str,
    /// The module that issued the import, if any.
    pub importer: Option<&'a str>,
    /// Whether this is an entry-point import (no importer).
    pub is_entry: bool,
    /// The kind of import that triggered resolution.
    pub kind: ImportKind,
}

// ---------------------------------------------------------------------------
// Hook output structs
// ---------------------------------------------------------------------------

/// Returned from the `resolve_id` hook when a plugin handles resolution.
#[derive(Debug, Clone)]
pub struct HookResolveIdOutput {
    /// The canonical module identifier after resolution.
    pub id: String,
    /// Whether this module should be treated as external (never bundled).
    pub external: bool,
    /// The resolved module type.
    pub module_type: ModuleType,
    /// Optional side-effect hint for tree-shaking.
    pub side_effects: Option<HookSideEffects>,
}

/// Returned from the `load` hook when a plugin supplies source content.
#[derive(Debug, Clone)]
pub struct HookLoadOutput {
    /// The UTF-8 source code.
    pub code: String,
    /// Optional source map from a prior transformation.
    pub source_map: Option<SourceMap>,
    /// Override for the module type inferred from the resolved ID.
    pub module_type: Option<ModuleType>,
}

/// Returned from `transform` and `render_chunk` hooks when the plugin
/// modifies the source/output code.
#[derive(Debug, Clone)]
pub struct HookTransformOutput {
    /// The (potentially modified) source or rendered code.
    pub code: String,
    /// Optional updated source map.
    pub source_map: Option<SourceMap>,
}

// ---------------------------------------------------------------------------
// Module / chunk info structs passed to hooks
// ---------------------------------------------------------------------------

/// A read-only snapshot of a module's graph state, passed to `module_parsed`.
///
/// This mirrors rolldown's `ModuleInfo` and is intentionally decoupled from
/// the internal `NormalModule` struct to keep the plugin API stable.
#[derive(Debug, Clone)]
pub struct ModuleInfo {
    /// Canonical module ID.
    pub id: String,
    /// Whether this module is a build entry point.
    pub is_entry: bool,
    /// Module type.
    pub module_type: ModuleType,
    /// IDs of modules that statically import this module.
    pub importers: Vec<String>,
    /// IDs of modules that dynamically import this module.
    pub dynamic_importers: Vec<String>,
    /// IDs of modules that this module statically imports.
    pub imported_ids: Vec<String>,
    /// IDs of modules that this module dynamically imports.
    pub dynamically_imported_ids: Vec<String>,
    /// Names exported by this module.
    pub exports: Vec<String>,
}

/// A read-only snapshot of a rendered chunk, passed to `render_chunk`,
/// `augment_chunk_hash`, `banner`, and `footer` hooks.
#[derive(Debug, Clone)]
pub struct RenderedChunkMeta {
    /// Chunk file name (without directory).
    pub name: String,
    /// Content hash (FNV-1a hex of the bundled source).
    pub chunk_hash: String,
    /// Whether this is a build entry chunk.
    pub is_entry: bool,
    /// Whether this chunk is a dynamic-import entry.
    pub is_dynamic_entry: bool,
    /// The facade module ID for this chunk, if any.
    pub facade_module_id: Option<String>,
    /// IDs of all modules included in this chunk.
    pub module_ids: Vec<String>,
    /// Names exported by this chunk.
    pub exports: Vec<String>,
    /// Static import specifiers for other chunks.
    pub imports: Vec<String>,
    /// Dynamic import specifiers for other chunks.
    pub dynamic_imports: Vec<String>,
}

// ---------------------------------------------------------------------------
// Cache-invalidation types (keep for `should_invalidate_cache`)
// ---------------------------------------------------------------------------

/// Context passed to `should_invalidate_cache`.
#[derive(Debug, Clone, Default)]
pub struct CacheContext {
    /// Current build configuration fingerprint (compiler version, flags, etc.).
    pub build_fingerprint: String,
}

/// A previously cached module entry.
#[derive(Debug, Clone)]
pub struct CachedModule {
    /// Resolved module ID.
    pub id: String,
    /// Content hash of the source at cache time.
    pub source_hash: String,
}

// ---------------------------------------------------------------------------
// Build error (passed to `build_end`)
// ---------------------------------------------------------------------------

/// An error originating from the build phase.
#[derive(Debug, Clone)]
pub struct BuildError {
    pub message: String,
    pub file: Option<String>,
}

// ---------------------------------------------------------------------------
// Plugin trait
// ---------------------------------------------------------------------------

/// The arkdown plugin trait — mirrors rolldown/rollup plugin hooks.
///
/// All hooks have default no-op implementations. Plugins implement only
/// the hooks they need and declare them via [`register_hook_usage`].
#[async_trait]
pub trait ArkdownPlugin: Send + Sync {
    /// Plugin name (used in diagnostics).
    fn name(&self) -> &str;

    /// Declare which hooks this plugin implements.
    ///
    /// The runner uses this to skip plugins that don't implement a given hook,
    /// improving performance when there are many plugins.
    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::empty()
    }

    // ── Build hooks ──────────────────────────────────────────────────────────

    /// Called once at build start (before any module is resolved).
    async fn build_start(&self, _ctx: &PluginContext) -> PluginResult<()> {
        Ok(())
    }

    /// Resolve a module specifier to a [`HookResolveIdOutput`].
    /// Return `None` to defer to the next plugin.
    async fn resolve_id(
        &self,
        _ctx: &PluginContext,
        _args: &HookResolveIdArgs<'_>,
    ) -> PluginResult<Option<HookResolveIdOutput>> {
        Ok(None)
    }

    /// Load the source content for a resolved module ID.
    /// Return `None` to defer to the default file loader.
    async fn load(&self, _ctx: &PluginContext, _id: &str) -> PluginResult<Option<HookLoadOutput>> {
        Ok(None)
    }

    /// Transform the source of a module.
    /// `code` is the current source text; `id` is the resolved module ID.
    /// Return `None` to leave source unchanged.
    async fn transform(
        &self,
        _ctx: &PluginContext,
        _code: &str,
        _id: &str,
    ) -> PluginResult<Option<HookTransformOutput>> {
        Ok(None)
    }

    /// Called after a module is fully parsed and added to the graph.
    async fn module_parsed(&self, _ctx: &PluginContext, _module: &ModuleInfo) -> PluginResult<()> {
        Ok(())
    }

    /// Called when the build phase ends (all modules parsed).
    /// `error` is `Some` if the build failed.
    async fn build_end(
        &self,
        _ctx: &PluginContext,
        _error: Option<&BuildError>,
    ) -> PluginResult<()> {
        Ok(())
    }

    // ── Output / render hooks ────────────────────────────────────────────────

    /// Called once before any chunk is rendered.
    async fn render_start(&self, _ctx: &PluginContext) -> PluginResult<()> {
        Ok(())
    }

    /// Transform a rendered chunk's code.
    /// `code` is the current chunk source; `chunk` holds metadata.
    /// Return `None` to leave code unchanged.
    async fn render_chunk(
        &self,
        _ctx: &PluginContext,
        _code: &str,
        _chunk: &RenderedChunkMeta,
    ) -> PluginResult<Option<HookTransformOutput>> {
        Ok(None)
    }

    /// Contribute additional data to a chunk's content hash.
    ///
    /// Returning `Some(string)` causes that string to be mixed into the hash
    /// alongside the chunk source, useful for cache-busting based on plugin state.
    async fn augment_chunk_hash(
        &self,
        _ctx: &PluginContext,
        _chunk: &RenderedChunkMeta,
    ) -> PluginResult<Option<String>> {
        Ok(None)
    }

    /// Prepend a banner comment/string to a chunk's output file.
    async fn banner(
        &self,
        _ctx: &PluginContext,
        _chunk: &RenderedChunkMeta,
    ) -> PluginResult<Option<String>> {
        Ok(None)
    }

    /// Append a footer comment/string to a chunk's output file.
    async fn footer(
        &self,
        _ctx: &PluginContext,
        _chunk: &RenderedChunkMeta,
    ) -> PluginResult<Option<String>> {
        Ok(None)
    }

    /// Called after all chunks are rendered, before file write.
    async fn generate_bundle(
        &self,
        _ctx: &PluginContext,
        _bundle: &mut ChunkGraph,
        _is_write: bool,
    ) -> PluginResult<()> {
        Ok(())
    }

    /// Called after all chunks have been written to disk.
    async fn write_bundle(&self, _ctx: &PluginContext, _bundle: &ChunkGraph) -> PluginResult<()> {
        Ok(())
    }

    /// Called after the bundle is fully generated/written. Use for cleanup.
    async fn close_bundle(&self, _ctx: &PluginContext) -> PluginResult<()> {
        Ok(())
    }

    // ── Watch hooks ──────────────────────────────────────────────────────────

    /// Called when a watched file changes (watch mode only).
    async fn watch_change(
        &self,
        _ctx: &PluginContext,
        _path: &str,
        _kind: WatchChangeKind,
    ) -> PluginResult<()> {
        Ok(())
    }

    /// Called when the watcher is closed.
    async fn close_watcher(&self, _ctx: &PluginContext) -> PluginResult<()> {
        Ok(())
    }

    // ── Cache hooks (sync) ───────────────────────────────────────────────────

    /// Decide whether an incremental cache entry is still valid.
    ///
    /// This hook is intentionally synchronous — it runs on the hot path of
    /// an incremental build's cache lookup and must not block on I/O.
    fn should_invalidate_cache(&self, _ctx: &CacheContext, _cached: &CachedModule) -> bool {
        false
    }
}

/// Convenience alias — a boxed, object-safe plugin.
pub type BoxedPlugin = Box<dyn ArkdownPlugin>;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── HookUsage bitflags ───────────────────────────────────────────────────

    #[test]
    fn test_hook_usage_empty_is_default() {
        let default: HookUsage = Default::default();
        assert_eq!(default, HookUsage::empty());
        assert!(default.is_empty());
    }

    #[test]
    fn test_hook_usage_all_contains_all_flags() {
        let all = HookUsage::all();
        assert!(all.contains(HookUsage::BuildStart));
        assert!(all.contains(HookUsage::ResolveId));
        assert!(all.contains(HookUsage::Load));
        assert!(all.contains(HookUsage::Transform));
        assert!(all.contains(HookUsage::ModuleParsed));
        assert!(all.contains(HookUsage::BuildEnd));
        assert!(all.contains(HookUsage::RenderStart));
        assert!(all.contains(HookUsage::RenderChunk));
        assert!(all.contains(HookUsage::AugmentChunkHash));
        assert!(all.contains(HookUsage::GenerateBundle));
        assert!(all.contains(HookUsage::WriteBundle));
        assert!(all.contains(HookUsage::CloseBundle));
        assert!(all.contains(HookUsage::WatchChange));
        assert!(all.contains(HookUsage::CloseWatcher));
        assert!(all.contains(HookUsage::Banner));
        assert!(all.contains(HookUsage::Footer));
        assert!(all.contains(HookUsage::ShouldInvalidateCache));
    }

    #[test]
    fn test_hook_usage_combine_flags() {
        let flags = HookUsage::ResolveId | HookUsage::Load;
        assert!(flags.contains(HookUsage::ResolveId));
        assert!(flags.contains(HookUsage::Load));
        assert!(!flags.contains(HookUsage::Transform));
        assert!(!flags.contains(HookUsage::BuildStart));
    }

    #[test]
    fn test_hook_usage_contains_single_flag() {
        let flags = HookUsage::Transform;
        assert!(flags.contains(HookUsage::Transform));
        assert!(!flags.contains(HookUsage::Load));
    }

    // ── HookResolveIdArgs ────────────────────────────────────────────────────

    #[test]
    fn test_hook_resolve_id_args_no_importer() {
        let args = HookResolveIdArgs {
            specifier: "./index",
            importer: None,
            is_entry: true,
            kind: ImportKind::Import,
        };
        assert_eq!(args.specifier, "./index");
        assert!(args.importer.is_none());
        assert!(args.is_entry);
        assert_eq!(args.kind, ImportKind::Import);
    }

    #[test]
    fn test_hook_resolve_id_args_with_importer() {
        let importer_path = "/project/entry/src/main.ets";
        let args = HookResolveIdArgs {
            specifier: "@ohos/hilog",
            importer: Some(importer_path),
            is_entry: false,
            kind: ImportKind::DynamicImport,
        };
        assert_eq!(args.importer, Some(importer_path));
        assert!(!args.is_entry);
        assert_eq!(args.kind, ImportKind::DynamicImport);
    }

    #[test]
    fn test_hook_resolve_id_args_all_import_kinds() {
        for kind in [
            ImportKind::Import,
            ImportKind::DynamicImport,
            ImportKind::Require,
            ImportKind::New,
        ] {
            let args = HookResolveIdArgs {
                specifier: "some/module",
                importer: None,
                is_entry: false,
                kind,
            };
            assert_eq!(args.kind, kind);
        }
    }

    #[test]
    fn test_import_kind_default_is_import() {
        let kind: ImportKind = Default::default();
        assert_eq!(kind, ImportKind::Import);
    }

    // ── HookLoadOutput ───────────────────────────────────────────────────────

    #[test]
    fn test_hook_load_output_none_optionals() {
        let out = HookLoadOutput {
            code: "const x = 1;".to_string(),
            source_map: None,
            module_type: None,
        };
        assert_eq!(out.code, "const x = 1;");
        assert!(out.source_map.is_none());
        assert!(out.module_type.is_none());
    }

    #[test]
    fn test_hook_load_output_with_optionals() {
        use crate::module::ModuleType;
        let out = HookLoadOutput {
            code: "export default 42;".to_string(),
            source_map: Some("{\"version\":3}".to_string()),
            module_type: Some(ModuleType::Ts),
        };
        assert!(out.source_map.is_some());
        assert_eq!(out.module_type, Some(ModuleType::Ts));
    }

    // ── HookTransformOutput ──────────────────────────────────────────────────

    #[test]
    fn test_hook_transform_output_construction() {
        let out = HookTransformOutput {
            code: "transformed code".to_string(),
            source_map: Some("{\"mappings\":\"\"}".to_string()),
        };
        assert_eq!(out.code, "transformed code");
        assert!(out.source_map.is_some());
    }

    #[test]
    fn test_hook_transform_output_no_map() {
        let out = HookTransformOutput {
            code: "code".to_string(),
            source_map: None,
        };
        assert!(out.source_map.is_none());
    }

    // ── ModuleInfo ───────────────────────────────────────────────────────────

    #[test]
    fn test_module_info_construction_and_access() {
        use crate::module::ModuleType;
        let info = ModuleInfo {
            id: "@bundle:com.example/entry/ets/entryability/EntryAbility".to_string(),
            is_entry: true,
            module_type: ModuleType::Ets,
            importers: vec![],
            dynamic_importers: vec![],
            imported_ids: vec!["./utils/helper".to_string()],
            dynamically_imported_ids: vec![],
            exports: vec!["default".to_string()],
        };
        assert!(info.is_entry);
        assert_eq!(info.module_type, ModuleType::Ets);
        assert_eq!(info.imported_ids.len(), 1);
        assert_eq!(info.exports[0], "default");
    }

    // ── RenderedChunkMeta ────────────────────────────────────────────────────

    #[test]
    fn test_rendered_chunk_meta_construction() {
        let meta = RenderedChunkMeta {
            name: "entry".to_string(),
            chunk_hash: "abcd1234".to_string(),
            is_entry: true,
            is_dynamic_entry: false,
            facade_module_id: Some("EntryAbility.ets".to_string()),
            module_ids: vec!["id1".to_string(), "id2".to_string()],
            exports: vec!["default".to_string()],
            imports: vec!["commons".to_string()],
            dynamic_imports: vec![],
        };
        assert_eq!(meta.name, "entry");
        assert_eq!(meta.chunk_hash, "abcd1234");
        assert!(meta.is_entry);
        assert!(!meta.is_dynamic_entry);
        assert_eq!(meta.facade_module_id.as_deref(), Some("EntryAbility.ets"));
        assert_eq!(meta.module_ids.len(), 2);
    }

    #[test]
    fn test_rendered_chunk_meta_no_facade() {
        let meta = RenderedChunkMeta {
            name: "commons".to_string(),
            chunk_hash: "beef0000".to_string(),
            is_entry: false,
            is_dynamic_entry: false,
            facade_module_id: None,
            module_ids: vec![],
            exports: vec![],
            imports: vec![],
            dynamic_imports: vec![],
        };
        assert!(meta.facade_module_id.is_none());
    }
}
