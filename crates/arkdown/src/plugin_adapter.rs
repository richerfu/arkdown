//! Plugin adapter that bridges [`ArkdownPlugin`] to rolldown's [`Plugin`] trait.
//!
//! [`ArkdownPluginAdapter`] wraps `Arc<dyn ArkdownPlugin>` and implements the
//! rolldown `Plugin` trait so that OHOS-specific plugins (chunk-wrapper,
//! ohos-resolve, ets-transform, etc.) can be driven by rolldown's internal
//! plugin runner without modification.
//!
//! ## Hook mapping
//!
//! | ArkdownPlugin hook | rolldown Plugin hook  |
//! |--------------------|-----------------------|
//! | `build_start`      | `build_start`         |
//! | `resolve_id`       | `resolve_id`          |
//! | `load`             | `load`                |
//! | `transform`        | `transform`           |
//! | `render_chunk`     | `render_chunk`        |
//!
//! Source maps are not converted in this initial implementation (they are set
//! to `None` / `Null`).  All type conversions are explicit to keep the code
//! easy to audit.

use std::borrow::Cow;
use std::sync::Arc;

use arkdown_core::{
    ArkdownPlugin, HookResolveIdArgs as ArkResolveIdArgs, ImportKind as ArkImportKind,
    PluginContext as ArkCtx, RenderedChunkMeta,
};
use rolldown_common::{ImportKind as RdImportKind, ResolvedExternal, RollupRenderedChunk};
use rolldown_plugin::{
    HookBuildStartArgs, HookLoadArgs, HookLoadOutput, HookLoadReturn, HookNoopReturn,
    HookRenderChunkArgs, HookRenderChunkOutput, HookRenderChunkReturn, HookResolveIdArgs,
    HookResolveIdOutput, HookResolveIdReturn, HookTransformArgs, HookTransformOutput,
    HookTransformOutputMap, HookTransformReturn, HookUsage, Plugin, PluginContext,
    SharedLoadPluginContext, SharedTransformPluginContext,
};

// ---------------------------------------------------------------------------
// Adapter struct
// ---------------------------------------------------------------------------

/// Wraps an [`ArkdownPlugin`] as a rolldown [`Plugin`].
pub struct ArkdownPluginAdapter(pub Arc<dyn ArkdownPlugin>);

impl std::fmt::Debug for ArkdownPluginAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ArkdownPluginAdapter")
            .field(&self.0.name())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Plugin implementation
// ---------------------------------------------------------------------------

impl Plugin for ArkdownPluginAdapter {
    fn name(&self) -> Cow<'static, str> {
        Cow::Owned(self.0.name().to_owned())
    }

    // Map our HookUsage bitflags to rolldown's HookUsage bitflags.
    // The bit positions differ, so each flag must be mapped explicitly.
    fn register_hook_usage(&self) -> HookUsage {
        use arkdown_core::HookUsage as ArkHU;
        use rolldown_plugin::HookUsage as RdHU;

        let our = self.0.register_hook_usage();
        let mut rd = RdHU::empty();

        if our.contains(ArkHU::BuildStart) {
            rd |= RdHU::BuildStart;
        }
        if our.contains(ArkHU::ResolveId) {
            rd |= RdHU::ResolveId;
        }
        if our.contains(ArkHU::Load) {
            rd |= RdHU::Load;
        }
        if our.contains(ArkHU::Transform) {
            rd |= RdHU::Transform;
        }
        if our.contains(ArkHU::ModuleParsed) {
            rd |= RdHU::ModuleParsed;
        }
        if our.contains(ArkHU::BuildEnd) {
            rd |= RdHU::BuildEnd;
        }
        if our.contains(ArkHU::RenderStart) {
            rd |= RdHU::RenderStart;
        }
        if our.contains(ArkHU::RenderChunk) {
            rd |= RdHU::RenderChunk;
        }
        if our.contains(ArkHU::AugmentChunkHash) {
            rd |= RdHU::AugmentChunkHash;
        }
        if our.contains(ArkHU::GenerateBundle) {
            rd |= RdHU::GenerateBundle;
        }
        if our.contains(ArkHU::WriteBundle) {
            rd |= RdHU::WriteBundle;
        }
        if our.contains(ArkHU::CloseBundle) {
            rd |= RdHU::CloseBundle;
        }
        if our.contains(ArkHU::WatchChange) {
            rd |= RdHU::WatchChange;
        }
        if our.contains(ArkHU::CloseWatcher) {
            rd |= RdHU::CloseWatcher;
        }
        if our.contains(ArkHU::Banner) {
            rd |= RdHU::Banner;
        }
        if our.contains(ArkHU::Footer) {
            rd |= RdHU::Footer;
        }

        rd
    }

    // -- Build hooks ----------------------------------------------------------

    fn build_start(
        &self,
        _ctx: &PluginContext,
        _args: &HookBuildStartArgs<'_>,
    ) -> impl std::future::Future<Output = HookNoopReturn> + Send {
        let plugin = Arc::clone(&self.0);
        async move {
            plugin
                .build_start(&ArkCtx::default())
                .await
                .map_err(|e| anyhow::anyhow!("{e:?}"))
        }
    }

    fn resolve_id(
        &self,
        _ctx: &PluginContext,
        args: &HookResolveIdArgs<'_>,
    ) -> impl std::future::Future<Output = HookResolveIdReturn> + Send {
        let plugin = Arc::clone(&self.0);
        let specifier = args.specifier.to_owned();
        let importer = args.importer.map(str::to_owned);
        let is_entry = args.is_entry;
        let kind = rd_to_ark_import_kind(args.kind);

        async move {
            let ark_args = ArkResolveIdArgs {
                specifier: &specifier,
                importer: importer.as_deref(),
                is_entry,
                kind,
            };
            let result = plugin
                .resolve_id(&ArkCtx::default(), &ark_args)
                .await
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;

            Ok(result.map(|out| HookResolveIdOutput {
                id: out.id.into(),
                external: if out.external {
                    Some(ResolvedExternal::Bool(true))
                } else {
                    None
                },
                ..Default::default()
            }))
        }
    }

    fn load(
        &self,
        _ctx: SharedLoadPluginContext,
        args: &HookLoadArgs<'_>,
    ) -> impl std::future::Future<Output = HookLoadReturn> + Send {
        let plugin = Arc::clone(&self.0);
        let id = args.id.to_owned();

        async move {
            let result = plugin
                .load(&ArkCtx::default(), &id)
                .await
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;

            Ok(result.map(|out| HookLoadOutput {
                code: out.code.into(),
                // Source map conversion from String → oxc_sourcemap::SourceMap is
                // deferred; set to None for now.
                map: None,
                side_effects: None,
                // Module type override could be mapped here if needed.
                module_type: None,
            }))
        }
    }

    fn transform(
        &self,
        _ctx: SharedTransformPluginContext,
        args: &HookTransformArgs<'_>,
    ) -> impl std::future::Future<Output = HookTransformReturn> + Send {
        let plugin = Arc::clone(&self.0);
        let code = args.code.as_str().to_owned();
        let id = args.id.to_owned();

        async move {
            let result = plugin
                .transform(&ArkCtx::default(), &code, &id)
                .await
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;

            Ok(result.map(|out| HookTransformOutput {
                code: Some(out.code),
                // Explicit Null tells rolldown that no sourcemap is intended,
                // preventing the "possibly broken sourcemap" warning.
                map: HookTransformOutputMap::Null,
                ..Default::default()
            }))
        }
    }

    // -- Output / render hooks ------------------------------------------------

    /// Delegates to `ArkdownPlugin::render_chunk` for plugins that need final
    /// chunk edits, such as legacy chunk-wrapper registry handling.
    fn render_chunk(
        &self,
        _ctx: &PluginContext,
        args: &HookRenderChunkArgs<'_>,
    ) -> impl std::future::Future<Output = HookRenderChunkReturn> + Send {
        let plugin = Arc::clone(&self.0);
        let code = Arc::clone(&args.code);
        let chunk_meta = rollup_chunk_to_meta(&args.chunk);

        async move {
            let result = plugin
                .render_chunk(&ArkCtx::default(), &code, &chunk_meta)
                .await
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;

            Ok(result.map(|out| HookRenderChunkOutput {
                code: out.code,
                map: None,
            }))
        }
    }
}

// ---------------------------------------------------------------------------
// Helper conversions
// ---------------------------------------------------------------------------

/// Map rolldown's [`RdImportKind`] to our [`ArkImportKind`].
///
/// Rolldown has additional CSS-specific variants (`AtImport`, `UrlImport`,
/// `HotAccept`) that have no equivalent in our import model; they all fall
/// back to [`ArkImportKind::Import`].
fn rd_to_ark_import_kind(kind: RdImportKind) -> ArkImportKind {
    match kind {
        RdImportKind::Import => ArkImportKind::Import,
        RdImportKind::DynamicImport => ArkImportKind::DynamicImport,
        RdImportKind::Require => ArkImportKind::Require,
        RdImportKind::NewUrl => ArkImportKind::New,
        // AtImport / UrlImport / HotAccept have no OHOS equivalent.
        _ => ArkImportKind::Import,
    }
}

/// Convert a rolldown [`RollupRenderedChunk`] to our [`RenderedChunkMeta`].
///
/// `chunk_hash` is not provided by rolldown at render time (it is computed
/// after code generation); we leave it empty.  Plugins that require a hash
/// (e.g. `augment_chunk_hash`) can fill it in a later pass if needed.
fn rollup_chunk_to_meta(chunk: &RollupRenderedChunk) -> RenderedChunkMeta {
    RenderedChunkMeta {
        name: chunk.name.to_string(),
        chunk_hash: String::new(),
        is_entry: chunk.is_entry,
        is_dynamic_entry: chunk.is_dynamic_entry,
        facade_module_id: chunk
            .facade_module_id
            .as_ref()
            .map(|id| id.as_str().to_owned()),
        module_ids: chunk
            .module_ids
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect(),
        exports: chunk.exports.iter().map(|e| e.to_string()).collect(),
        imports: chunk.imports.iter().map(|i| i.to_string()).collect(),
        dynamic_imports: chunk
            .dynamic_imports
            .iter()
            .map(|d| d.to_string())
            .collect(),
    }
}
