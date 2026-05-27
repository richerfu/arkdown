use std::sync::Arc;
use arkdown_core::{
    ArkdownPlugin, HookLoadOutput, HookResolveIdArgs, HookResolveIdOutput,
    HookTransformOutput, ImportKind, ModuleInfo, PluginContext, PluginResult,
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
    ) -> PluginResult<Option<HookResolveIdOutput>> {
        let ctx = PluginContext::default();
        let args = HookResolveIdArgs {
            specifier,
            importer,
            is_entry: importer.is_none(),
            kind: ImportKind::Import,
        };
        for plugin in &self.plugins {
            if let Some(output) = plugin.resolve_id(&ctx, &args).await? {
                return Ok(Some(output));
            }
        }
        Ok(None)
    }

    /// First plugin returning Some wins; returns None if all return None.
    pub async fn load(&self, id: &str) -> PluginResult<Option<HookLoadOutput>> {
        let ctx = PluginContext::default();
        for plugin in &self.plugins {
            if let Some(output) = plugin.load(&ctx, id).await? {
                return Ok(Some(output));
            }
        }
        Ok(None)
    }

    /// Each plugin may transform sequentially; code threads through the chain.
    pub async fn transform(&self, mut code: String, id: &str) -> PluginResult<String> {
        let ctx = PluginContext::default();
        for plugin in &self.plugins {
            if let Some(HookTransformOutput { code: new_code, .. }) =
                plugin.transform(&ctx, &code, id).await?
            {
                code = new_code;
            }
        }
        Ok(code)
    }

    pub async fn module_parsed(&self, module_info: &ModuleInfo) -> PluginResult<()> {
        let ctx = PluginContext::default();
        for plugin in &self.plugins {
            plugin.module_parsed(&ctx, module_info).await?;
        }
        Ok(())
    }
}
