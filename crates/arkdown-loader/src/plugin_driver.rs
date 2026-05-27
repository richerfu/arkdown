use std::sync::Arc;
use arkdown_core::{
    ArkdownPlugin, LoadContext, ModuleParsedContext, Module, ModuleSource, PluginResult,
    ResolveContext, ResolvedId, TransformContext,
};

pub struct PluginDriver {
    pub plugins: Vec<Arc<dyn ArkdownPlugin>>,
}

impl PluginDriver {
    pub fn new(plugins: Vec<Arc<dyn ArkdownPlugin>>) -> Self {
        PluginDriver { plugins }
    }

    /// First plugin returning Some wins; returns None if all return None.
    pub async fn resolve_id(
        &self,
        specifier: &str,
        importer: Option<&str>,
    ) -> PluginResult<Option<ResolvedId>> {
        let ctx = ResolveContext::default();
        for plugin in &self.plugins {
            if let Some(id) = plugin.resolve_id(&ctx, specifier, importer).await? {
                return Ok(Some(id));
            }
        }
        Ok(None)
    }

    /// First plugin returning Some wins; returns None if all return None.
    pub async fn load(&self, id: &str) -> PluginResult<Option<ModuleSource>> {
        let ctx = LoadContext::default();
        for plugin in &self.plugins {
            if let Some(src) = plugin.load(&ctx, id).await? {
                return Ok(Some(src));
            }
        }
        Ok(None)
    }

    /// Each plugin may transform sequentially; source threads through the chain.
    pub async fn transform(&self, mut source: ModuleSource) -> PluginResult<ModuleSource> {
        let ctx = TransformContext::default();
        for plugin in &self.plugins {
            if let Some(new_src) = plugin.transform(&ctx, source.clone()).await? {
                source = new_src;
            }
        }
        Ok(source)
    }

    pub async fn module_parsed(&self, module: &Module) -> PluginResult<()> {
        let ctx = ModuleParsedContext::default();
        for plugin in &self.plugins {
            plugin.module_parsed(&ctx, module).await?;
        }
        Ok(())
    }
}
