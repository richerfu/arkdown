//! Integration tests for `arkdown-loader`'s `PluginDriver`.
//!
//! These tests exercise:
//! - First-plugin-wins semantics for `resolve_id` and `load`
//! - `HookUsage` filtering (plugins without a flag are skipped)
//! - Sequential transform chaining
//! - Short-circuit load semantics

use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};

use async_trait::async_trait;
use arkdown_core::{
    ArkdownPlugin, HookLoadOutput, HookResolveIdOutput, HookTransformOutput, HookUsage,
    ModuleType, PluginContext, PluginResult,
};
use arkdown_loader::PluginDriver;

// ---------------------------------------------------------------------------
// Helpers / spy plugins
// ---------------------------------------------------------------------------

/// A plugin that always returns `Some(HookResolveIdOutput)` for any specifier.
struct AlwaysResolvesPlugin {
    resolved_id: String,
    call_count: Arc<AtomicUsize>,
}

impl AlwaysResolvesPlugin {
    fn new(resolved_id: &str) -> (Self, Arc<AtomicUsize>) {
        let counter = Arc::new(AtomicUsize::new(0));
        (
            AlwaysResolvesPlugin {
                resolved_id: resolved_id.to_string(),
                call_count: Arc::clone(&counter),
            },
            counter,
        )
    }
}

#[async_trait]
impl ArkdownPlugin for AlwaysResolvesPlugin {
    fn name(&self) -> &str {
        "always-resolves"
    }
    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::ResolveId
    }
    async fn resolve_id(
        &self,
        _ctx: &PluginContext,
        _args: &arkdown_core::HookResolveIdArgs<'_>,
    ) -> PluginResult<Option<HookResolveIdOutput>> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        Ok(Some(HookResolveIdOutput {
            id: self.resolved_id.clone(),
            external: false,
            module_type: ModuleType::Ts,
            side_effects: None,
        }))
    }
}

/// A plugin that always returns `None` from `resolve_id`, tracking calls.
struct NeverResolvesPlugin {
    call_count: Arc<AtomicUsize>,
}

impl NeverResolvesPlugin {
    fn new() -> (Self, Arc<AtomicUsize>) {
        let counter = Arc::new(AtomicUsize::new(0));
        (
            NeverResolvesPlugin {
                call_count: Arc::clone(&counter),
            },
            counter,
        )
    }
}

#[async_trait]
impl ArkdownPlugin for NeverResolvesPlugin {
    fn name(&self) -> &str {
        "never-resolves"
    }
    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::ResolveId
    }
    async fn resolve_id(
        &self,
        _ctx: &PluginContext,
        _args: &arkdown_core::HookResolveIdArgs<'_>,
    ) -> PluginResult<Option<HookResolveIdOutput>> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        Ok(None)
    }
}

/// A plugin that panics if `resolve_id` is ever called.
/// Useful for asserting that a plugin is skipped due to HookUsage filtering.
struct PanicOnResolvePlugin;

#[async_trait]
impl ArkdownPlugin for PanicOnResolvePlugin {
    fn name(&self) -> &str {
        "panic-on-resolve"
    }
    // Does NOT declare HookUsage::ResolveId — driver must skip it entirely.
    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::empty()
    }
    async fn resolve_id(
        &self,
        _ctx: &PluginContext,
        _args: &arkdown_core::HookResolveIdArgs<'_>,
    ) -> PluginResult<Option<HookResolveIdOutput>> {
        panic!("PanicOnResolvePlugin.resolve_id was called despite HookUsage not declaring ResolveId");
    }
}

/// A plugin that appends a fixed suffix to any source code it transforms.
struct AppendPlugin {
    suffix: &'static str,
}

#[async_trait]
impl ArkdownPlugin for AppendPlugin {
    fn name(&self) -> &str {
        "append"
    }
    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::Transform
    }
    async fn transform(
        &self,
        _ctx: &PluginContext,
        code: &str,
        _id: &str,
    ) -> PluginResult<Option<HookTransformOutput>> {
        Ok(Some(HookTransformOutput {
            code: format!("{}{}", code, self.suffix),
            source_map: None,
        }))
    }
}

/// A plugin that returns a fixed load output for any module ID.
struct FixedLoadPlugin {
    content: String,
    was_called: Arc<AtomicBool>,
}

impl FixedLoadPlugin {
    fn new(content: &str) -> (Self, Arc<AtomicBool>) {
        let flag = Arc::new(AtomicBool::new(false));
        (
            FixedLoadPlugin {
                content: content.to_string(),
                was_called: Arc::clone(&flag),
            },
            flag,
        )
    }
}

#[async_trait]
impl ArkdownPlugin for FixedLoadPlugin {
    fn name(&self) -> &str {
        "fixed-load"
    }
    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::Load
    }
    async fn load(&self, _ctx: &PluginContext, _id: &str) -> PluginResult<Option<HookLoadOutput>> {
        self.was_called.store(true, Ordering::SeqCst);
        Ok(Some(HookLoadOutput {
            code: self.content.clone(),
            source_map: None,
            module_type: Some(ModuleType::Ts),
        }))
    }
}

// ---------------------------------------------------------------------------
// Tests: resolve_id
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_resolve_id_first_plugin_wins() {
    // Two plugins both declare ResolveId; the first one resolves → second skipped.
    let (p1, count1) = AlwaysResolvesPlugin::new("/resolved/by/first.ts");
    let (p2, count2) = AlwaysResolvesPlugin::new("/resolved/by/second.ts");

    let driver = PluginDriver::new(vec![
        Arc::new(p1) as Arc<dyn ArkdownPlugin>,
        Arc::new(p2) as Arc<dyn ArkdownPlugin>,
    ]);

    let result = driver
        .resolve_id("./some/module", Some("/importer.ts"))
        .await
        .unwrap();

    // First plugin resolves.
    assert!(result.is_some());
    assert_eq!(result.unwrap().id, "/resolved/by/first.ts");

    // First plugin called once, second never called (short-circuit).
    assert_eq!(count1.load(Ordering::SeqCst), 1);
    assert_eq!(count2.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn test_resolve_id_first_none_falls_through_to_second() {
    // First plugin returns None → driver tries second plugin.
    let (p1, count1) = NeverResolvesPlugin::new();
    let (p2, count2) = AlwaysResolvesPlugin::new("/resolved/by/second.ts");

    let driver = PluginDriver::new(vec![
        Arc::new(p1) as Arc<dyn ArkdownPlugin>,
        Arc::new(p2) as Arc<dyn ArkdownPlugin>,
    ]);

    let result = driver
        .resolve_id("./module", Some("/importer.ts"))
        .await
        .unwrap();

    assert!(result.is_some());
    assert_eq!(result.unwrap().id, "/resolved/by/second.ts");

    // Both plugins were called.
    assert_eq!(count1.load(Ordering::SeqCst), 1);
    assert_eq!(count2.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_resolve_id_all_none_returns_none() {
    // Both plugins return None → driver returns None.
    let (p1, _) = NeverResolvesPlugin::new();
    let (p2, _) = NeverResolvesPlugin::new();

    let driver = PluginDriver::new(vec![
        Arc::new(p1) as Arc<dyn ArkdownPlugin>,
        Arc::new(p2) as Arc<dyn ArkdownPlugin>,
    ]);

    let result = driver.resolve_id("./absent", None).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_resolve_id_hook_usage_filtering_skips_undeclared_plugin() {
    // PanicOnResolvePlugin does NOT declare HookUsage::ResolveId.
    // If the driver calls it, the test panics.
    let driver = PluginDriver::new(vec![
        Arc::new(PanicOnResolvePlugin) as Arc<dyn ArkdownPlugin>,
    ]);

    // Should complete without panicking because the plugin is skipped.
    let result = driver
        .resolve_id("./module", Some("/importer.ts"))
        .await
        .unwrap();
    assert!(result.is_none(), "no plugin should resolve when only HookUsage::empty() is declared");
}

// ---------------------------------------------------------------------------
// Tests: transform chain
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_transform_chain_two_plugins() {
    // Two transform plugins chain: first appends " ONE", second appends " TWO".
    let driver = PluginDriver::new(vec![
        Arc::new(AppendPlugin { suffix: " ONE" }) as Arc<dyn ArkdownPlugin>,
        Arc::new(AppendPlugin { suffix: " TWO" }) as Arc<dyn ArkdownPlugin>,
    ]);

    let result = driver
        .transform("start".to_string(), "test.ts")
        .await
        .unwrap();

    // Output of first becomes input of second.
    assert_eq!(result, "start ONE TWO");
}

#[tokio::test]
async fn test_transform_no_change_if_plugin_returns_none() {
    // A plugin that returns None leaves code unchanged.
    struct NoopTransformPlugin;
    #[async_trait]
    impl ArkdownPlugin for NoopTransformPlugin {
        fn name(&self) -> &str { "noop-transform" }
        fn register_hook_usage(&self) -> HookUsage { HookUsage::Transform }
        async fn transform(
            &self,
            _ctx: &PluginContext,
            _code: &str,
            _id: &str,
        ) -> PluginResult<Option<HookTransformOutput>> {
            Ok(None) // no-op
        }
    }

    let driver = PluginDriver::new(vec![
        Arc::new(NoopTransformPlugin) as Arc<dyn ArkdownPlugin>,
        Arc::new(AppendPlugin { suffix: " APPENDED" }) as Arc<dyn ArkdownPlugin>,
    ]);

    let result = driver
        .transform("original".to_string(), "test.ts")
        .await
        .unwrap();

    // NoopTransformPlugin leaves it as "original", AppendPlugin makes it "original APPENDED".
    assert_eq!(result, "original APPENDED");
}

#[tokio::test]
async fn test_transform_skips_plugin_without_hook_usage() {
    // A plugin that declares no hooks should not have transform called.
    struct AlwaysPanicsTransform;
    #[async_trait]
    impl ArkdownPlugin for AlwaysPanicsTransform {
        fn name(&self) -> &str { "panics-transform" }
        fn register_hook_usage(&self) -> HookUsage { HookUsage::empty() }
        async fn transform(
            &self,
            _ctx: &PluginContext,
            _code: &str,
            _id: &str,
        ) -> PluginResult<Option<HookTransformOutput>> {
            panic!("transform must not be called when HookUsage::Transform is not declared");
        }
    }

    let driver = PluginDriver::new(vec![
        Arc::new(AlwaysPanicsTransform) as Arc<dyn ArkdownPlugin>,
    ]);

    let result = driver
        .transform("code".to_string(), "test.ts")
        .await
        .unwrap();

    // Code unchanged; panic plugin was never called.
    assert_eq!(result, "code");
}

// ---------------------------------------------------------------------------
// Tests: load
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_load_first_plugin_returns_some_short_circuits() {
    // First plugin returns Some(content) → second plugin must NOT be called.
    let (p1, called1) = FixedLoadPlugin::new("content from plugin 1");
    let (p2, called2) = FixedLoadPlugin::new("content from plugin 2");

    let driver = PluginDriver::new(vec![
        Arc::new(p1) as Arc<dyn ArkdownPlugin>,
        Arc::new(p2) as Arc<dyn ArkdownPlugin>,
    ]);

    let result = driver.load("some/module.ts").await.unwrap();

    let output = result.expect("expected Some(HookLoadOutput)");
    assert_eq!(output.code, "content from plugin 1");

    // First called, second skipped.
    assert!(called1.load(Ordering::SeqCst));
    assert!(!called2.load(Ordering::SeqCst));
}

#[tokio::test]
async fn test_load_all_none_returns_none() {
    // No plugin handles this module → driver returns None.
    struct NeverLoadsPlugin;
    #[async_trait]
    impl ArkdownPlugin for NeverLoadsPlugin {
        fn name(&self) -> &str { "never-loads" }
        fn register_hook_usage(&self) -> HookUsage { HookUsage::Load }
        async fn load(&self, _: &PluginContext, _: &str) -> PluginResult<Option<HookLoadOutput>> {
            Ok(None)
        }
    }

    let driver = PluginDriver::new(vec![
        Arc::new(NeverLoadsPlugin) as Arc<dyn ArkdownPlugin>,
    ]);

    let result = driver.load("unhandled/module.ts").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_load_hook_usage_filtering_skips_undeclared_plugin() {
    // Plugin declares no HookUsage → load must not be called.
    struct PanicOnLoadPlugin;
    #[async_trait]
    impl ArkdownPlugin for PanicOnLoadPlugin {
        fn name(&self) -> &str { "panic-on-load" }
        fn register_hook_usage(&self) -> HookUsage { HookUsage::empty() }
        async fn load(&self, _: &PluginContext, _: &str) -> PluginResult<Option<HookLoadOutput>> {
            panic!("PanicOnLoadPlugin.load was called despite HookUsage::Load not being declared");
        }
    }

    let driver = PluginDriver::new(vec![
        Arc::new(PanicOnLoadPlugin) as Arc<dyn ArkdownPlugin>,
    ]);

    // Should complete without panicking.
    let result = driver.load("any/module.ts").await.unwrap();
    assert!(result.is_none());
}
