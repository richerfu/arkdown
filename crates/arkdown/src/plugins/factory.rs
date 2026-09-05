//! Plugin factory — assembles the ordered plugin set for a module build.
//!
//! `PluginFactory` owns the construction logic that was previously in
//! `packages/arkdown/src/cmd/build/plugin_factory.rs`. Moving it here makes the
//! ordered plugin set an internal concern of the bundler crate rather than
//! a build-service concern, and allows the build-service to depend only on `arkdown` for plugin
//! instantiation.
//!
//! # Plugin order
//!
//! The order matches hvigor's `arkts-pack.js::getBuiltInPlugins` sequence:
//!
//! | # | hvigor name | Rust plugin | Always active |
//! |---|-------------|-------------|---------------|
//! | 1 | `checkDuplicatedFileNames` | `DuplicateCheckPlugin` | yes |
//! | 2 | `moduleMetaInfo` (ESMODULE) | — (session/package plan + scan output) | n/a |
//! | 3 | `resolve` | `OhosResolvePlugin` | yes |
//! | 4 | `sensitiveCheck` | `ImportCheckPlugin` | yes |
//! | 5 | `json` | `Json5Plugin` | yes |
//! | 6 | `wrapperChunk` | — (deleted; slot kept as JS-plugin anchor) | n/a |
//! | 7 | `sdkPlugins` (etsTransform) | `EtsTransformPlugin` | yes |
//! | 8 | `apiTransform` | `ApiTransformPlugin` | yes |
//! | 9 | `HarDedup` (HSP) | `HarDedupPlugin` | HSP builds only |
//! | 10 | SDK `terser` (release) | — (deleted; release minification is the | n/a |
//! |   |  | obfuscation stage / ArkGuard, not a render hook) |  |
//!
//! hvigor entries without a Rust equivalent (`commonPlugin`, `invalidCachePlugin`,
//! `routerMap` planning, `newOhmUrl` absorbed by resolve, `commonjs` Rolldown built-in,
//! `gc` no-op, `waitAsync` no-op, `collectImporters` post-bundle coordinator)
//! are omitted from this list.
//!
//! JavaScript compile plugins are supplied by the package runtime as normalized
//! Rolldown plugin objects. The factory does not parse or execute hvigorfile
//! sources.
//!
//! # Upstream correspondence
//!
//! Replaces `packages/arkdown/src/cmd/build/plugin_factory.rs`. The build-service shim
//! delegates to this factory once the migration is complete.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;

use arkdown_core::{ArkTscTarget, InsightIntentFormBinding, OutputTarget};
use arkdown_path::{
    lexical_normalize, strip_relative_prefix, AbsPath, ModuleSpecifier, SpecifierKind,
};
use arkdown_resolver::{OhosResolver, PackageResolutionPlan, ResolverConfig};
use rolldown_plugin::{
    __inner::SharedPluginable, HookBuildEndArgs, HookBuildStartArgs, HookNoopReturn, HookUsage,
    Plugin, PluginContext, PluginHookMeta, PluginOrder as RolldownPluginOrder,
};

use crate::plan::BundlePlan;
use crate::plugins::api_transform::ApiTransformPlugin;
use crate::plugins::duplicate_check::DuplicateCheckPlugin;
use crate::plugins::ets_transform::{
    CheckerMode, ComponentMetadata, EtsTransformPlugin, InsightIntentSchemaCatalog,
    NavigationRouteMap, ResourceTransformContext, RolldownEtsTransformPlan, SystemApiCheckOptions,
    TransformEntrySourceId, TransformSourceSnapshot,
};
use crate::plugins::har_dedup::HarDedupPlugin;
use crate::plugins::hvigor_compile::{
    HvigorCompilePluginShare, HvigorNativePluginSlot, HvigorWatchChange,
};
use crate::plugins::import_check::ImportCheckPlugin;
use crate::plugins::json5::Json5Plugin;
use crate::plugins::ohos_resolve::OhosResolvePlugin;
use crate::PluginCacheModule;

/// Placement of a latest-hvigor compile plugin in the Rolldown plugin list.
///
/// String orders map directly to the two outer positions. Function-valued
/// orders are evaluated by the Rust planner against hvigor's plugin-name map.
/// The result is anchored to the next canonical native adapter so optional or
/// disabled adapters cannot shift a custom plugin onto the wrong lifecycle
/// boundary. `Position` remains for the older public N-API representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HvigorPluginOrder {
    First,
    #[default]
    Last,
    Anchor {
        rank: u16,
        sequence: u32,
    },
    Position(usize),
}

/// Input data for `PluginFactory` that supplements `BundlePlan` with
/// session-level information not yet captured in the plan itself.
///
/// These fields come from `BuildSession` / `ModuleCompileContext` at the
/// build-service layer and are passed down to the factory. Once `BundlePlan`
/// carries the full session data, this struct can be collapsed into it.
#[derive(Clone)]
pub struct PluginFactoryInput {
    /// Effective module target currently being bundled.
    pub target_name: String,
    /// SDK `api/` directories for `@ohos/*` resolution.
    pub sdk_api_dirs: Vec<PathBuf>,
    /// SDK `kits/` directories for `@kit.*` resolution.
    pub sdk_kits_dirs: Vec<PathBuf>,
    /// SDK `arkts/` directories for `@arkts.*` resolution.
    pub sdk_arkts_dirs: Vec<PathBuf>,
    /// Ordered non-package roots from hvigor
    /// `arkts-pack.getResolveModules()`: active `src/main`, then the generated
    /// profile directory.
    pub resolve_source_roots: Vec<PathBuf>,
    /// All module roots in the project: `(module_name, module_root)`.
    ///
    /// Used to build the `RolldownEtsTransformPlan::module_roots` list.
    /// When only the current module is known (e.g. in unit tests), pass a
    /// single-entry vec derived from `BundlePlan::input`.
    pub all_module_roots: Vec<(String, PathBuf)>,
    /// Resource references collected from the resource compilation stage.
    ///
    /// Maps resource identifiers to their resolved `ResourceReference` values.
    pub resource_refs: FxHashMap<String, crate::plugins::ets_transform::ResourceReference>,
    /// Source-compiler resource lowering context for API 22/23, HAR, and
    /// integrated-HSP runtime name behavior.
    pub resource_context: ResourceTransformContext,
    /// SDK ArkUI component metadata loaded before Rolldown hooks run.
    pub component_metadata: ComponentMetadata,
    /// SDK built-in InsightIntent schemas loaded once before hook execution.
    pub insight_intent_schema_catalog: InsightIntentSchemaCatalog,
    /// System-API compatibility inputs normalized before Rolldown starts.
    pub system_api: SystemApiCheckOptions,
    /// Source files loaded before Rolldown hooks run for cross-file AST analysis.
    pub transform_sources: TransformSourceSnapshot,
    /// Router-map metadata indexed before transform hooks begin.
    ///
    /// This feeds the SDK-equivalent `NavigationBuilderRegister` emission for
    /// local global `@Builder` functions.
    pub navigation_routes: NavigationRouteMap,
    /// Package index resolved from the current module's declared dependency
    /// graph.
    ///
    /// OhosResolver consumes this instead of scanning arbitrary `oh_modules`
    /// entries so package visibility matches hvigor's ModuleDependencyInfo.
    pub package_resolution: Arc<PackageResolutionPlan>,
    /// Source-set overlays for build-target overrides.
    ///
    /// Each entry is `(src_main_ets_root, target_ets_root)`.
    pub source_set_overlays: Vec<(PathBuf, PathBuf)>,
    /// ArkTS checker mode forwarded from package/build-service config.
    pub checker_mode: CheckerMode,
    /// `arkOptions.tscConfig.targetESVersion` selected for source emission.
    pub tsc_target: ArkTscTarget,
    /// Whether import path casing must match the filesystem exactly.
    pub case_sensitive_check: bool,
    /// Effective `strictMode.noExternalImportByPath` diagnostic severity.
    pub no_external_import_by_path: bool,
    /// Explicit phase for hvigor-compatible HSP HAR deduplication.
    pub har_dedup: HarDedupFactoryMode,
    /// Built-in plugin names to disable for this build.
    ///
    /// Populated from `arkdown.config.toml` `plugins.disabled` (or its legacy
    /// JSON fallback) via the coordinator.
    pub disabled_plugins: Vec<String>,
    /// `arkOptions.autoLazyImport` + `autoLazyFilter` + owning module package
    /// name, forwarded to the SDK `process_lazy_import.js` rewrite.
    pub auto_lazy_import: bool,
    pub auto_lazy_filter: Option<arkdown_core::AutoLazyFilter>,
    pub module_pkg_name: Option<String>,
    /// `arkOptions.expandImportPath` (SDK `import_path_expand.js`).
    pub expand_import_path: Option<arkdown_core::ExpandImportPath>,
    /// Remote HSP/HAR package names (OHM maps) that never expand.
    pub hsp_har_package_names: FxHashSet<String>,
    /// Package-supplied Rolldown-compatible plugins from the Node.js public API.
    ///
    /// These are inserted using hvigor's `getPlugins()` order convention:
    /// `first` plugins run before built-ins, and `last` plugins run after
    /// built-ins.
    pub package_plugins: Vec<PackagePlugin>,
}

impl PluginFactoryInput {
    /// Construct a minimal `PluginFactoryInput` from a `BundlePlan`.
    ///
    /// Suitable for use in tests or when session data is not available.
    /// SDK dirs and resource refs are left empty; `all_module_roots` is
    /// derived from the plan's single module.
    pub fn from_plan(plan: &BundlePlan) -> Self {
        Self {
            target_name: "default".to_string(),
            sdk_api_dirs: vec![],
            sdk_kits_dirs: vec![],
            sdk_arkts_dirs: vec![],
            resolve_source_roots: vec![plan.input.module_root.clone()],
            all_module_roots: vec![(
                plan.input.module_name.clone(),
                plan.input.module_root.clone(),
            )],
            resource_refs: FxHashMap::default(),
            resource_context: ResourceTransformContext::default(),
            component_metadata: ComponentMetadata::default(),
            insight_intent_schema_catalog: InsightIntentSchemaCatalog::default(),
            system_api: SystemApiCheckOptions::default()
                .with_compatible_sdk_version(plan.compatible_sdk_version.clone())
                .with_compile_sdk_version(plan.target_api_version.to_string())
                .with_runtime_os(plan.runtime_os.clone()),
            transform_sources: TransformSourceSnapshot::default(),
            navigation_routes: NavigationRouteMap::default(),
            package_resolution: Arc::new(PackageResolutionPlan::default()),
            source_set_overlays: vec![],
            checker_mode: CheckerMode::Oxc,
            tsc_target: ArkTscTarget::default(),
            case_sensitive_check: false,
            no_external_import_by_path: plan.use_normalized_ohm_url,
            har_dedup: HarDedupFactoryMode::Disabled,
            disabled_plugins: vec![],
            package_plugins: vec![],
            auto_lazy_import: false,
            auto_lazy_filter: None,
            module_pkg_name: None,
            expand_import_path: None,
            hsp_har_package_names: FxHashSet::default(),
        }
    }
}

/// HAR deduplication phase selected by the build-service coordinator.
///
/// The plugin factory never infers a phase from stale files on disk. Hvigor
/// schedules both tasks explicitly in one build, so arkdown does the same.
#[derive(Debug, Clone, Default)]
pub enum HarDedupFactoryMode {
    #[default]
    Disabled,
    Collect {
        collect_file: PathBuf,
        source_har_roots: Vec<PathBuf>,
    },
    RealBuild {
        collect_file: PathBuf,
    },
}

/// A package-supplied plugin plus its hvigor insertion order.
#[derive(Clone)]
pub struct PackagePlugin {
    plugin: SharedPluginable,
    order: HvigorPluginOrder,
    gap_hooks: Option<Arc<dyn HvigorPluginGapHooks>>,
    module_root: Option<PathBuf>,
    target_name: Option<String>,
}

impl PackagePlugin {
    /// Create a package plugin entry.
    pub fn new(plugin: SharedPluginable, order: HvigorPluginOrder) -> Self {
        Self {
            plugin,
            order,
            gap_hooks: None,
            module_root: None,
            target_name: None,
        }
    }

    /// Create a Rolldown plugin with the small set of hvigor-only gap hooks.
    pub fn with_gap_hooks(
        plugin: SharedPluginable,
        order: HvigorPluginOrder,
        gap_hooks: Arc<dyn HvigorPluginGapHooks>,
    ) -> Self {
        Self {
            plugin,
            order,
            gap_hooks: Some(gap_hooks),
            module_root: None,
            target_name: None,
        }
    }

    /// Limit a latest-hvigor compile plugin to the module whose effective
    /// `compilePluginPath`/`hvigorPluginFile` declared it.
    ///
    /// Explicit package API plugins have no scope and participate in every
    /// module build, matching a top-level Rolldown plugin configuration.
    pub fn with_module_root(mut self, module_root: Option<PathBuf>) -> Self {
        self.module_root = module_root.map(|root| {
            AbsPath::canonicalize(&root)
                .map(AbsPath::into_path_buf)
                .unwrap_or_else(|_| lexical_normalize(&root))
        });
        self
    }

    /// Limit a programmatically injected legacy `compilePluginFile` to the
    /// target whose merged build option selected it. Latest-hvigor
    /// `loadCompilePlugin` entries and explicit Rolldown plugins leave this
    /// unset and apply to every target.
    pub fn with_target_name(mut self, target_name: Option<String>) -> Self {
        self.target_name = target_name;
        self
    }

    /// Whether this plugin belongs to the requested module build.
    pub fn applies_to_module_root(&self, module_root: &std::path::Path) -> bool {
        let Some(scope) = &self.module_root else {
            return true;
        };
        let candidate = AbsPath::canonicalize(module_root)
            .map(AbsPath::into_path_buf)
            .unwrap_or_else(|_| lexical_normalize(module_root));
        scope == &candidate
    }

    /// Whether this plugin belongs to the requested module/target pipeline.
    pub fn applies_to_module_target(
        &self,
        module_root: &std::path::Path,
        target_name: &str,
    ) -> bool {
        self.applies_to_module_root(module_root)
            && self
                .target_name
                .as_deref()
                .is_none_or(|scope| scope == target_name)
    }

    /// Stable identity used by upstream build-stage cache fingerprints.
    ///
    /// The JavaScript function body is owned by the package runtime and is not
    /// serializable in Rust; the runtime must invalidate by passing a changed
    /// plugin name/order or by implementing `shouldInvalidCache`.
    pub fn cache_identity(&self) -> String {
        let order = match self.order {
            HvigorPluginOrder::First => "first".to_string(),
            HvigorPluginOrder::Last => "last".to_string(),
            HvigorPluginOrder::Anchor { rank, sequence } => {
                format!("anchor-{rank}-{sequence}")
            }
            HvigorPluginOrder::Position(position) => format!("position-{position}"),
        };
        let identity = match self
            .gap_hooks
            .as_ref()
            .and_then(|hooks| hooks.runtime_cache_identity())
        {
            Some(runtime_identity) => {
                format!("{order}:{}:{runtime_identity}", self.plugin.call_name())
            }
            None => format!("{order}:{}", self.plugin.call_name()),
        };
        match self.target_name.as_deref() {
            Some(target_name) => format!("target={target_name}:{identity}"),
            None => identity,
        }
    }

    /// Run latest-hvigor `shouldInvalidCache` for one retained transformed
    /// module from the task cache.
    pub async fn should_invalid_cache(&self, module: &PluginCacheModule) -> anyhow::Result<bool> {
        let Some(gap_hooks) = &self.gap_hooks else {
            return Ok(false);
        };
        gap_hooks.should_invalid_cache(module).await
    }

    pub fn has_should_invalid_cache(&self) -> bool {
        self.gap_hooks
            .as_ref()
            .is_some_and(|hooks| hooks.has_should_invalid_cache())
    }

    /// Seed the compile-plugin `this.share` bridge before Rolldown buildStart.
    ///
    /// This is initialization data, not a second lifecycle hook runner. Every
    /// supported hook still executes through Rolldown except the two explicit
    /// gaps in [`HvigorPluginGapHooks`].
    pub async fn seed_hvigor_context(
        &self,
        share: &HvigorCompilePluginShare,
    ) -> anyhow::Result<()> {
        let Some(gap_hooks) = &self.gap_hooks else {
            return Ok(());
        };
        gap_hooks.seed_hvigor_context(share).await
    }

    /// Serialize the shared/plugin cache after a successful build. The bytes
    /// remain opaque to Rust and are persisted by `arkdown-cache`.
    pub async fn snapshot_hvigor_context_cache(&self) -> anyhow::Result<Option<Vec<u8>>> {
        let Some(gap_hooks) = &self.gap_hooks else {
            return Ok(None);
        };
        gap_hooks.snapshot_hvigor_context_cache().await
    }
}

/// Thin adapter for compile-plugin behavior absent from upstream Rolldown.
///
/// Standard hooks are deliberately not represented here: the wrapped plugin
/// implements [`rolldown_plugin::Plugin`] and Rolldown owns their scheduling,
/// context, diagnostics, and errors. Latest hvigor also adds lifecycle/cache
/// hooks that Rolldown lacks. `seed_hvigor_context` supplies the latest-hvigor
/// `share` object before Rolldown starts and is not user-visible lifecycle
/// behavior. The watch methods bridge the external Rust watcher into the live
/// Rolldown context before the next `beforeBuild` hook.
/// Future returned by [`HvigorPluginGapHooks`] async methods.
///
/// Boxed by hand — matching rolldown's internal `Pluginable` trait — because
/// this trait is consumed as a trait object (`Arc<dyn HvigorPluginGapHooks>`)
/// and RPITIT methods are not dyn-compatible. Implementors wrap their bodies
/// in `Box::pin(async { .. })`.
pub type HvigorHookFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HvigorLifecycleHook {
    BeforeBuild,
    AfterBuildStart,
    BeforeBuildEnd,
    AfterBuildEnd,
    BundleEnd,
    CleanUp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HvigorHookOrder {
    Pre,
    Normal,
    Post,
}

impl HvigorLifecycleHook {
    pub const ALL: [Self; 6] = [
        Self::BeforeBuild,
        Self::AfterBuildStart,
        Self::BeforeBuildEnd,
        Self::AfterBuildEnd,
        Self::BundleEnd,
        Self::CleanUp,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BeforeBuild => "beforeBuild",
            Self::AfterBuildStart => "afterBuildStart",
            Self::BeforeBuildEnd => "beforeBuildEnd",
            Self::AfterBuildEnd => "afterBuildEnd",
            Self::BundleEnd => "bundleEnd",
            Self::CleanUp => "cleanUp",
        }
    }
}

pub trait HvigorPluginGapHooks: Send + Sync {
    fn seed_hvigor_context<'a>(
        &'a self,
        _share: &'a HvigorCompilePluginShare,
    ) -> HvigorHookFuture<'a, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }

    fn should_invalid_cache<'a>(
        &'a self,
        _module: &'a PluginCacheModule,
    ) -> HvigorHookFuture<'a, anyhow::Result<bool>> {
        Box::pin(async { Ok(false) })
    }

    fn has_should_invalid_cache(&self) -> bool {
        false
    }

    fn snapshot_hvigor_context_cache(
        &self,
    ) -> HvigorHookFuture<'_, anyhow::Result<Option<Vec<u8>>>> {
        Box::pin(async { Ok(None) })
    }

    fn take_watch_changes(&self) -> anyhow::Result<Arc<[HvigorWatchChange]>> {
        Ok(Arc::from([]))
    }

    fn watch_change_with_context(
        &self,
        _change: HvigorWatchChange,
        _context: PluginContext,
    ) -> HvigorHookFuture<'_, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }

    fn has_watch_change(&self) -> bool {
        false
    }

    fn watch_change_order(&self) -> HvigorHookOrder {
        HvigorHookOrder::Normal
    }

    /// A latest-hvigor lifecycle hook with Rolldown's live plugin context.
    fn lifecycle_hook_with_context(
        &self,
        _hook: HvigorLifecycleHook,
        _context: PluginContext,
    ) -> HvigorHookFuture<'_, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }

    fn has_lifecycle_hook(&self, _hook: HvigorLifecycleHook) -> bool {
        false
    }

    fn lifecycle_hook_order(&self, _hook: HvigorLifecycleHook) -> HvigorHookOrder {
        HvigorHookOrder::Normal
    }

    fn runtime_cache_identity(&self) -> Option<String> {
        None
    }
}

#[derive(Clone)]
struct HvigorPluginGapEntry {
    name: String,
    hooks: Arc<dyn HvigorPluginGapHooks>,
}

#[derive(Clone)]
struct PluginFactoryEntry {
    plugin: SharedPluginable,
    gap_entry: Option<HvigorPluginGapEntry>,
    native_rank: Option<u16>,
}

impl PluginFactoryEntry {
    fn native(plugin: SharedPluginable, slot: HvigorNativePluginSlot) -> Self {
        Self {
            plugin,
            gap_entry: None,
            native_rank: Some(slot.rank()),
        }
    }

    fn package(plugin: SharedPluginable, gap_entry: Option<HvigorPluginGapEntry>) -> Self {
        Self {
            plugin,
            gap_entry,
            native_rank: None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HvigorLifecycleBoundary {
    WatchChange,
    BeforeBuild,
    AfterBuildStart,
    BeforeBuildEnd,
    AfterBuildEnd,
}

impl HvigorLifecycleBoundary {
    fn hooks(self) -> &'static [HvigorLifecycleHook] {
        match self {
            Self::WatchChange => &[],
            Self::BeforeBuild => &[HvigorLifecycleHook::BeforeBuild],
            Self::AfterBuildStart => &[HvigorLifecycleHook::AfterBuildStart],
            Self::BeforeBuildEnd => &[HvigorLifecycleHook::BeforeBuildEnd],
            Self::AfterBuildEnd => &[
                HvigorLifecycleHook::AfterBuildEnd,
                HvigorLifecycleHook::BundleEnd,
                HvigorLifecycleHook::CleanUp,
            ],
        }
    }
}

struct HvigorLifecyclePlugin {
    entries: Vec<HvigorPluginGapEntry>,
    boundary: HvigorLifecycleBoundary,
}

impl HvigorLifecyclePlugin {
    fn new(entries: Vec<HvigorPluginGapEntry>, boundary: HvigorLifecycleBoundary) -> Self {
        Self { entries, boundary }
    }

    async fn dispatch(
        entries: Vec<HvigorPluginGapEntry>,
        hooks: &[HvigorLifecycleHook],
        context: PluginContext,
    ) -> anyhow::Result<()> {
        for hook in hooks {
            for order in [
                HvigorHookOrder::Pre,
                HvigorHookOrder::Normal,
                HvigorHookOrder::Post,
            ] {
                for entry in &entries {
                    if !entry.hooks.has_lifecycle_hook(*hook)
                        || entry.hooks.lifecycle_hook_order(*hook) != order
                    {
                        continue;
                    }
                    entry
                        .hooks
                        .lifecycle_hook_with_context(*hook, context.clone())
                        .await
                        .map_err(|error| {
                            anyhow::anyhow!(
                                "{} hook for plugin '{}' failed: {error}",
                                hook.as_str(),
                                entry.name
                            )
                        })?;
                }
            }
        }
        Ok(())
    }

    async fn dispatch_watch_changes(
        entries: Vec<HvigorPluginGapEntry>,
        context: PluginContext,
    ) -> anyhow::Result<()> {
        let changes = entries
            .first()
            .map(|entry| entry.hooks.take_watch_changes())
            .transpose()?
            .unwrap_or_else(|| Arc::from([]));
        for change in changes.iter().cloned() {
            for order in [
                HvigorHookOrder::Pre,
                HvigorHookOrder::Normal,
                HvigorHookOrder::Post,
            ] {
                for entry in &entries {
                    if !entry.hooks.has_watch_change() || entry.hooks.watch_change_order() != order
                    {
                        continue;
                    }
                    entry
                        .hooks
                        .watch_change_with_context(change.clone(), context.clone())
                        .await
                        .map_err(|error| {
                            anyhow::anyhow!(
                                "watchChange hook for plugin '{}' failed for '{}': {error}",
                                entry.name,
                                change.path.display()
                            )
                        })?;
                }
            }
        }
        Ok(())
    }
}

impl std::fmt::Debug for HvigorLifecyclePlugin {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HvigorLifecyclePlugin")
            .field(
                "plugins",
                &self
                    .entries
                    .iter()
                    .map(|entry| entry.name.as_str())
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl Plugin for HvigorLifecyclePlugin {
    fn name(&self) -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed(match self.boundary {
            HvigorLifecycleBoundary::WatchChange => "arkdown:hvigor-watch-change",
            HvigorLifecycleBoundary::BeforeBuild => "arkdown:hvigor-before-build",
            HvigorLifecycleBoundary::AfterBuildStart => "arkdown:hvigor-after-build-start",
            HvigorLifecycleBoundary::BeforeBuildEnd => "arkdown:hvigor-before-build-end",
            HvigorLifecycleBoundary::AfterBuildEnd => "arkdown:hvigor-after-build-end",
        })
    }

    fn register_hook_usage(&self) -> HookUsage {
        match self.boundary {
            HvigorLifecycleBoundary::WatchChange
            | HvigorLifecycleBoundary::BeforeBuild
            | HvigorLifecycleBoundary::AfterBuildStart => HookUsage::BuildStart,
            HvigorLifecycleBoundary::BeforeBuildEnd | HvigorLifecycleBoundary::AfterBuildEnd => {
                HookUsage::BuildEnd
            }
        }
    }

    fn build_start(
        &self,
        context: &PluginContext,
        _args: &HookBuildStartArgs<'_>,
    ) -> impl std::future::Future<Output = HookNoopReturn> + Send {
        let boundary = self.boundary;
        let hook = match boundary {
            HvigorLifecycleBoundary::BeforeBuild => Some(HvigorLifecycleHook::BeforeBuild),
            HvigorLifecycleBoundary::AfterBuildStart => Some(HvigorLifecycleHook::AfterBuildStart),
            _ => None,
        };
        let entries = self.entries.clone();
        let context = context.clone();
        async move {
            if matches!(boundary, HvigorLifecycleBoundary::WatchChange) {
                return Self::dispatch_watch_changes(entries, context).await;
            }
            let Some(hook) = hook else {
                return Ok(());
            };
            Self::dispatch(entries, &[hook], context).await
        }
    }

    fn build_start_meta(&self) -> Option<PluginHookMeta> {
        match self.boundary {
            HvigorLifecycleBoundary::WatchChange => Some(PluginHookMeta {
                order: Some(RolldownPluginOrder::Pre),
            }),
            HvigorLifecycleBoundary::BeforeBuild => Some(PluginHookMeta {
                order: Some(RolldownPluginOrder::Pre),
            }),
            HvigorLifecycleBoundary::AfterBuildStart => Some(PluginHookMeta {
                order: Some(RolldownPluginOrder::Post),
            }),
            _ => None,
        }
    }

    fn build_end(
        &self,
        context: &PluginContext,
        args: Option<&HookBuildEndArgs<'_>>,
    ) -> impl std::future::Future<Output = HookNoopReturn> + Send {
        let succeeded = args.is_none();
        let plugin_context = context.clone();
        let entries = self.entries.clone();
        let boundary = self.boundary;
        async move {
            match boundary {
                HvigorLifecycleBoundary::BeforeBuildEnd if succeeded => {
                    Self::dispatch(entries, boundary.hooks(), plugin_context).await
                }
                HvigorLifecycleBoundary::AfterBuildEnd if succeeded => {
                    Self::dispatch(entries, boundary.hooks(), plugin_context).await
                }
                HvigorLifecycleBoundary::AfterBuildEnd => {
                    Self::dispatch(entries, &[HvigorLifecycleHook::CleanUp], plugin_context).await
                }
                _ => Ok(()),
            }
        }
    }

    fn build_end_meta(&self) -> Option<PluginHookMeta> {
        match self.boundary {
            HvigorLifecycleBoundary::BeforeBuildEnd => Some(PluginHookMeta {
                order: Some(RolldownPluginOrder::Pre),
            }),
            HvigorLifecycleBoundary::AfterBuildEnd => Some(PluginHookMeta {
                order: Some(RolldownPluginOrder::PinPost),
            }),
            _ => None,
        }
    }
}

/// Builds the fixed built-in plugin list for a module bundle.
struct TransformEntrySources<'a> {
    plan: &'a BundlePlan,
}

impl<'a> TransformEntrySources<'a> {
    fn new(plan: &'a BundlePlan) -> Self {
        Self { plan }
    }

    fn form_source_ids(&self) -> FxHashSet<String> {
        self.source_ids_for(&self.plan.input.form_entry_sources)
    }

    fn insight_intent_form_bindings(&self) -> FxHashMap<String, InsightIntentFormBinding> {
        let mut bindings = FxHashMap::default();
        for binding in &self.plan.input.insight_intent_form_bindings {
            bindings.insert(binding.source.clone(), binding.clone());
            if let Some(resolved) = self.resolved_entry_id(&binding.source) {
                bindings.insert(
                    TransformEntrySourceId::new(&resolved).normalized(),
                    binding.clone(),
                );
            }
        }
        bindings
    }

    fn source_ids_for(&self, entries: &[String]) -> FxHashSet<String> {
        let mut source_ids = FxHashSet::default();
        for entry in entries {
            source_ids.insert(entry.clone());
            if let Some(resolved) = self.resolved_entry_id(entry) {
                source_ids.insert(TransformEntrySourceId::new(&resolved).normalized());
            }
        }
        source_ids
    }

    fn resolved_entry_id(&self, entry: &str) -> Option<String> {
        let specifier = ModuleSpecifier::new(entry);
        let path = match specifier.kind() {
            SpecifierKind::AbsolutePath => PathBuf::from(entry),
            SpecifierKind::OhmUrl | SpecifierKind::ScopedPackage => return Some(entry.to_string()),
            _ => self
                .plan
                .input
                .module_root
                .join(strip_relative_prefix(entry)),
        };
        AbsPath::canonicalize(&path)
            .map(AbsPath::into_path_buf)
            .unwrap_or_else(|_| lexical_normalize(&path))
            .into_os_string()
            .into_string()
            .ok()
    }
}

pub struct PluginFactory<'a> {
    plan: &'a BundlePlan,
    input: PluginFactoryInput,
}

pub struct PluginFactoryOutput {
    pub plugins: Vec<SharedPluginable>,
    pub ets_transform_plan: Arc<RolldownEtsTransformPlan>,
}

impl<'a> PluginFactory<'a> {
    /// Create a new factory from a bundle plan and supplemental session input.
    pub fn new(plan: &'a BundlePlan, input: PluginFactoryInput) -> Self {
        Self { plan, input }
    }

    fn merge_anchored_plugins(
        built_ins: Vec<PluginFactoryEntry>,
        mut anchored: Vec<(u16, u32, PluginFactoryEntry)>,
    ) -> Vec<PluginFactoryEntry> {
        anchored.sort_by_key(|(rank, sequence, _)| (*rank, *sequence));
        let mut anchored = anchored.into_iter().peekable();
        let mut merged = Vec::with_capacity(built_ins.len() + anchored.len());
        for built_in in built_ins {
            let native_rank = built_in
                .native_rank
                .expect("built-in plugin entries must have a native rank");
            while anchored
                .peek()
                .is_some_and(|(anchor_rank, _, _)| *anchor_rank <= native_rank)
            {
                if let Some((_, _, plugin)) = anchored.next() {
                    merged.push(plugin);
                }
            }
            merged.push(built_in);
        }
        merged.extend(anchored.map(|(_, _, plugin)| plugin));
        merged
    }

    fn merge_positioned_plugins(
        built_ins: Vec<PluginFactoryEntry>,
        mut positioned: Vec<(usize, PluginFactoryEntry)>,
    ) -> anyhow::Result<Vec<PluginFactoryEntry>> {
        if positioned.is_empty() {
            return Ok(built_ins);
        }
        positioned.sort_by_key(|(position, _)| *position);
        let final_len = built_ins.len() + positioned.len();
        if let Some((position, plugin)) = positioned
            .iter()
            .find(|(position, _)| *position >= final_len)
        {
            anyhow::bail!(
                "resolved plugin position {position} for '{}' exceeds pipeline length {final_len}",
                plugin.plugin.call_name()
            );
        }

        let mut built_ins = built_ins.into_iter();
        let mut positioned = positioned.into_iter().peekable();
        let mut merged = Vec::with_capacity(final_len);
        for slot in 0..final_len {
            if positioned
                .peek()
                .is_some_and(|(position, _)| *position == slot)
            {
                if let Some((_, plugin)) = positioned.next() {
                    merged.push(plugin);
                }
            } else if let Some(plugin) = built_ins.next() {
                merged.push(plugin);
            }
        }
        if positioned.peek().is_some() || built_ins.next().is_some() {
            anyhow::bail!("resolved plugin positions do not form a valid pipeline")
        }
        Ok(merged)
    }

    /// Build the ordered plugin list.
    ///
    /// Returns an `anyhow::Result` so that construction errors (e.g. a missing
    /// ETS loader path) are surfaced cleanly.
    pub fn build(self) -> anyhow::Result<Vec<SharedPluginable>> {
        Ok(self.build_output()?.plugins)
    }

    pub fn build_output(self) -> anyhow::Result<PluginFactoryOutput> {
        let project_root = self.plan.input.project_root.clone();
        let project_path = self.plan.input.project_path.clone();
        let module_root = self.plan.input.module_root.clone();
        let module_name = self.plan.input.module_name.clone();
        let target_name = self.input.target_name.clone();
        let package_module_root = self
            .input
            .all_module_roots
            .iter()
            .find_map(|(name, root)| (name == &module_name).then(|| root.clone()))
            .unwrap_or_else(|| module_root.clone());
        let import_check_module_roots = self
            .input
            .all_module_roots
            .iter()
            .map(|(_, root)| root.clone())
            .collect();

        let package_plugins = self
            .input
            .package_plugins
            .into_iter()
            .filter(|plugin| plugin.applies_to_module_target(&package_module_root, &target_name))
            .collect::<Vec<_>>();

        // ── 1. DuplicateCheckPlugin (hvigor: checkDuplicatedFileNames) ──────────
        let duplicate_check_plugin: SharedPluginable = Arc::new(DuplicateCheckPlugin::new());

        // ── 3. OhosResolvePlugin (hvigor: resolve + newOhmUrl) ───────────────
        let resolver_config = ResolverConfig {
            project_root: project_root.clone(),
            sdk_api_dirs: self.input.sdk_api_dirs,
            sdk_kits_dirs: self.input.sdk_kits_dirs,
            sdk_arkts_dirs: self.input.sdk_arkts_dirs,
            source_roots: self.input.resolve_source_roots,
            module_paths: vec![
                package_module_root.join("oh_modules"),
                project_root.join("oh_modules"),
            ],
            package_resolution: self.input.package_resolution,
            use_normalized_ohm_url: self.plan.use_normalized_ohm_url,
            case_sensitive_check: self.input.case_sensitive_check,
            source_set_overlays: self.input.source_set_overlays,
        };
        let ohos_resolver = Arc::new(OhosResolver::new(resolver_config));
        let ohos_resolve_plugin: SharedPluginable = Arc::new(OhosResolvePlugin::with_resolver(
            ohos_resolver,
            project_root.clone(),
            self.plan.use_normalized_ohm_url,
        ));

        // ── 4. ImportCheckPlugin (hvigor: sensitiveCheck) ────────────────────
        let import_check_plugin: SharedPluginable = Arc::new(ImportCheckPlugin::with_policy(
            // `import-check-plugin.ts::getBelongModulePath` compares the
            // importer with `allModulePaths`, whose entries are OHOS package
            // roots rather than active `src/main` source roots.
            import_check_module_roots,
            self.input.no_external_import_by_path,
        ));

        // ── 5. Json5Plugin (hvigor: json) ────────────────────────────────────
        let json5_plugin: SharedPluginable =
            Arc::new(Json5Plugin::new(self.input.transform_sources.clone()));

        // ── 6. wrapperChunk ─────────────────────────────────────────────────
        // No native adapter: `render_chunk` is unreachable on the scan-only
        // production path. `HvigorNativePluginSlot::WrapperChunk` remains a
        // valid anchor rank for package-supplied JS plugins.

        // ── 7. EtsTransformPlugin (hvigor: sdkPlugins / etsTransform) ────────
        //
        // Placed after resolve, json, and wrapper to match hvigor's sdkPlugins
        // position (position 15+ in getBuiltInPlugins). Rolldown calls each
        // plugin's transform hook in array order, so EtsTransformPlugin.transform
        // now runs after OhosResolvePlugin's other hooks — matching hvigor's
        // compile stage ordering.
        let transform_entry_sources = TransformEntrySources::new(self.plan);
        let form_entry_source_ids = transform_entry_sources.form_source_ids();
        let insight_intent_form_bindings = transform_entry_sources.insight_intent_form_bindings();
        let entry_source_ids =
            transform_entry_sources.source_ids_for(&self.plan.input.page_entry_sources);
        let ets_transform_plan = RolldownEtsTransformPlan::new(
            self.plan.bundle_name.clone(),
            self.plan.input.module_name.clone(),
            project_root.clone(),
            project_path,
            self.input.all_module_roots,
            self.plan.target_api_version,
            self.input.resource_refs,
            self.input.component_metadata,
            self.input.transform_sources,
        )
        .with_form_entry_source_ids(form_entry_source_ids)
        .with_insight_intent_form_bindings(insight_intent_form_bindings)
        .with_insight_intent_schema_catalog(self.input.insight_intent_schema_catalog.clone())
        .with_entry_source_ids(entry_source_ids)
        .with_preview_mode(self.plan.preview_mode)
        .with_check_entry(self.plan.check_entry)
        .with_navigation_routes(self.input.navigation_routes)
        .with_checker_mode(self.input.checker_mode)
        .with_should_emit_js(self.plan.should_emit_js)
        .with_arkts_language_version(self.plan.arkts_language_version)
        .with_api_available_context(&self.plan.compatible_sdk_version, &self.plan.runtime_os)
        .with_system_api_context(self.input.system_api)
        .with_resource_context(self.input.resource_context)
        .with_tsc_target(self.input.tsc_target)
        .with_module_analyzer(self.plan.module_analyzer)
        .with_auto_lazy_import(
            self.input.auto_lazy_import,
            self.input.auto_lazy_filter.clone(),
            self.input.module_pkg_name.clone(),
        )
        .with_expand_import_path(self.input.expand_import_path.clone())
        .with_hsp_har_package_names(self.input.hsp_har_package_names.clone())
        .with_disable_mock_setup_decorator(!self.plan.preview_mode);
        let ets_transform_plan = Arc::new(ets_transform_plan);
        let ets_transform_plugin: SharedPluginable = Arc::new(EtsTransformPlugin::from_plan(
            Arc::clone(&ets_transform_plan),
        ));

        // ── 8. ApiTransformPlugin (hvigor SDK: apiTransform) ───────────────
        //
        // Structured API rewrites belong to arkdown-ets-transform. This
        // adapter retains the SDK's distinct post-emit source-map boundary so
        // the following genAbc-equivalent collector observes Rollup-compatible
        // combined mappings.
        let api_transform_plugin: SharedPluginable = Arc::new(ApiTransformPlugin);

        let mut native_plugins = vec![
            PluginFactoryEntry::native(
                duplicate_check_plugin,
                HvigorNativePluginSlot::DuplicateCheck,
            ),
            PluginFactoryEntry::native(ohos_resolve_plugin, HvigorNativePluginSlot::OhosResolve),
            PluginFactoryEntry::native(import_check_plugin, HvigorNativePluginSlot::ImportCheck),
            PluginFactoryEntry::native(json5_plugin, HvigorNativePluginSlot::Json),
        ];

        // HarDedup is an optional phase between wrapperChunk and the SDK
        // transforms. Native anchors remain correct whether this slot exists.
        if matches!(self.plan.output_target, OutputTarget::Hsp) {
            let har_plugin: Option<SharedPluginable> = match self.input.har_dedup {
                HarDedupFactoryMode::Disabled => None,
                HarDedupFactoryMode::Collect {
                    collect_file,
                    source_har_roots,
                } => Some(Arc::new(HarDedupPlugin::collect_pass(
                    collect_file,
                    source_har_roots,
                ))),
                HarDedupFactoryMode::RealBuild { collect_file } => {
                    Some(Arc::new(HarDedupPlugin::real_build(collect_file)?))
                }
            };
            if let Some(har_plugin) = har_plugin {
                native_plugins.push(PluginFactoryEntry::native(
                    har_plugin,
                    HvigorNativePluginSlot::HarDedup,
                ));
            }
        }

        native_plugins.extend([
            PluginFactoryEntry::native(ets_transform_plugin, HvigorNativePluginSlot::EtsTransform),
            PluginFactoryEntry::native(api_transform_plugin, HvigorNativePluginSlot::ApiTransform),
        ]);

        // ── 10. SDK terser (release) ────────────────────────────────────────
        // No native adapter: JSBUNDLE-only minifier (CLAUDE.md rule 14), and
        // `render_chunk` is unreachable on the scan-only production path.
        // ESMODULE release minification/obfuscation is the obfuscation stage
        // (ArkGuard). `HvigorNativePluginSlot::Minify` stays an anchor rank.

        let disabled_plugins = self
            .input
            .disabled_plugins
            .into_iter()
            .collect::<FxHashSet<_>>();
        native_plugins
            .retain(|entry| !disabled_plugins.contains(entry.plugin.call_name().as_ref()));

        // ── Package API plugins (same-process Node.js NAPI bridge) ──────────
        let mut package_names = native_plugins
            .iter()
            .map(|entry| entry.plugin.call_name().to_string())
            .collect::<FxHashSet<_>>();
        for package_plugin in &package_plugins {
            let name = package_plugin.plugin.call_name().to_string();
            if !package_names.insert(name.clone()) {
                anyhow::bail!("duplicated Rolldown plugin name '{name}'");
            }
        }
        let mut ordered_plugins = native_plugins;
        let mut package_first_plugins = Vec::new();
        let mut package_last_plugins = Vec::new();
        let mut package_anchored_plugins = Vec::new();
        let mut package_positioned_plugins = Vec::new();
        for package_plugin in package_plugins {
            let PackagePlugin {
                plugin,
                order,
                gap_hooks,
                module_root: _,
                target_name: _,
            } = package_plugin;
            let gap_entry = gap_hooks
                .as_ref()
                .filter(|hooks| {
                    hooks.has_watch_change()
                        || HvigorLifecycleHook::ALL
                            .iter()
                            .any(|hook| hooks.has_lifecycle_hook(*hook))
                })
                .map(|hooks| HvigorPluginGapEntry {
                    name: plugin.call_name().to_string(),
                    hooks: Arc::clone(hooks),
                });
            let entry = PluginFactoryEntry::package(plugin, gap_entry);
            match order {
                HvigorPluginOrder::First => {
                    package_first_plugins.push(entry);
                }
                HvigorPluginOrder::Last => {
                    package_last_plugins.push(entry);
                }
                HvigorPluginOrder::Anchor { rank, sequence } => {
                    package_anchored_plugins.push((rank, sequence, entry));
                }
                HvigorPluginOrder::Position(position) => {
                    package_positioned_plugins.push((position, entry));
                }
            }
        }
        ordered_plugins = Self::merge_anchored_plugins(ordered_plugins, package_anchored_plugins);
        ordered_plugins =
            Self::merge_positioned_plugins(ordered_plugins, package_positioned_plugins)?;
        if !package_first_plugins.is_empty() || !package_last_plugins.is_empty() {
            let built_in_plugins = ordered_plugins;
            ordered_plugins = package_first_plugins;
            ordered_plugins.extend(built_in_plugins);
            ordered_plugins.extend(package_last_plugins);
        }

        let gap_entries = ordered_plugins
            .iter()
            .filter_map(|entry| entry.gap_entry.clone())
            .collect::<Vec<_>>();
        let mut plugins = ordered_plugins
            .into_iter()
            .map(|entry| entry.plugin)
            .collect::<Vec<_>>();
        if !gap_entries.is_empty() {
            // Ordinary Rolldown plugins supply the four fork lifecycle
            // boundaries. Standard hooks remain owned by Rolldown itself.
            let boundaries = [
                HvigorLifecycleBoundary::WatchChange,
                HvigorLifecycleBoundary::BeforeBuild,
                HvigorLifecycleBoundary::AfterBuildStart,
                HvigorLifecycleBoundary::BeforeBuildEnd,
                HvigorLifecycleBoundary::AfterBuildEnd,
            ];
            let mut lifecycle_plugins = Vec::new();
            for boundary in boundaries {
                let entries = gap_entries
                    .iter()
                    .filter(|entry| {
                        if matches!(boundary, HvigorLifecycleBoundary::WatchChange) {
                            entry.hooks.has_watch_change()
                        } else {
                            boundary
                                .hooks()
                                .iter()
                                .any(|hook| entry.hooks.has_lifecycle_hook(*hook))
                        }
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                if !entries.is_empty() {
                    lifecycle_plugins.push((boundary, entries));
                }
            }
            for (boundary, entries) in lifecycle_plugins.iter().rev() {
                if !matches!(boundary, HvigorLifecycleBoundary::AfterBuildEnd) {
                    plugins.insert(
                        0,
                        Arc::new(HvigorLifecyclePlugin::new(entries.clone(), *boundary)),
                    );
                }
            }
            if let Some((boundary, entries)) = lifecycle_plugins
                .into_iter()
                .find(|(boundary, _)| matches!(boundary, HvigorLifecycleBoundary::AfterBuildEnd))
            {
                plugins.push(Arc::new(HvigorLifecyclePlugin::new(entries, boundary)));
            }
        }

        Ok(PluginFactoryOutput {
            plugins,
            ets_transform_plan,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::path::Path;

    use arkdown_core::BuildMode;
    use rolldown_plugin::{HookUsage, Plugin};

    use super::HvigorPluginOrder;
    use super::*;
    use crate::plan::{
        BundleCachePolicy, BundleInput, BundleOutputOptions, BundlePlan, BundlePluginSet,
        LoaderResourceJoin, SourceMapPolicy,
    };

    #[derive(Debug)]
    struct NamedTestPlugin(&'static str);

    impl Plugin for NamedTestPlugin {
        fn name(&self) -> Cow<'static, str> {
            Cow::Borrowed(self.0)
        }

        fn register_hook_usage(&self) -> HookUsage {
            HookUsage::empty()
        }
    }

    struct EmptyGapHooks;

    impl HvigorPluginGapHooks for EmptyGapHooks {
        fn has_lifecycle_hook(&self, hook: HvigorLifecycleHook) -> bool {
            hook == HvigorLifecycleHook::BeforeBuildEnd
        }
    }

    struct WatchGapHooks;

    impl HvigorPluginGapHooks for WatchGapHooks {
        fn has_watch_change(&self) -> bool {
            true
        }

        fn has_lifecycle_hook(&self, hook: HvigorLifecycleHook) -> bool {
            hook == HvigorLifecycleHook::BeforeBuild
        }
    }

    fn make_plan(root: &Path) -> BundlePlan {
        let module_root = root.join("entry");
        BundlePlan {
            input: BundleInput {
                loader_json_path: module_root
                    .join("build/default/intermediates/loader/loader.json"),
                project_root: root.to_path_buf(),
                project_path: module_root.join("src/main/ets"),
                module_root,
                module_name: "entry".to_string(),
                module_path_map: Vec::new(),
                module_record_roots: Vec::new(),
                module_meta_info_files: Vec::new(),
                entries: vec!["./ets/pages/Index.ets".to_string()],
                form_entry_sources: Vec::new(),
                insight_intent_form_bindings: Vec::new(),
                runtime_entry_sources: Vec::new(),
                page_entry_sources: Vec::new(),
                support_chunks: false,
                source_map_packages: crate::plan::BundleSourceMapPackageInfo {
                    entry_package_name: "entry".to_string(),
                    entry_package_version: "1.0.0".to_string(),
                    dependency_package_versions: rustc_hash::FxHashMap::default(),
                },
                runtime_packages: crate::plan::BundleRuntimePackageInfo::default(),
                byte_code_har_packages: Vec::new(),
                byte_code_har: false,
                preload_system_so_entries: Vec::new(),
                mock_config: None,
            },
            output: BundleOutputOptions::default(),
            source_map: SourceMapPolicy::None,
            cache: BundleCachePolicy::default(),
            loader_resource: LoaderResourceJoin {
                loader_json_path: root.join("entry/build/default/intermediates/loader/loader.json"),
                resources_index_path: None,
                resource_table_path: None,
            },
            plugins: BundlePluginSet::default(),
            build_mode: BuildMode::Debug,
            arkts_language_version: arkdown_core::ArktsLanguageVersion::default(),
            target_api_version: 12,
            compatible_sdk_version: "12".to_string(),
            runtime_os: "OpenHarmony".to_string(),
            bundle_name: "com.example.test".to_string(),
            use_normalized_ohm_url: true,
            should_emit_js: true,
            module_analyzer: arkdown_ets_transform::ModuleAnalyzerKind::default(),
            output_target: OutputTarget::Hap,
            preview_mode: false,
            check_entry: false,
            shim_missing_exports_override: None,
            treeshake_override: None,
            context_override: None,
        }
    }

    #[test]
    fn package_plugins_keep_first_and_last_order_around_builtins() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().to_path_buf();
        let plan = make_plan(&root);
        let mut input = PluginFactoryInput::from_plan(&plan);
        input.package_plugins = vec![
            PackagePlugin::new(
                Arc::new(NamedTestPlugin("first-a")),
                HvigorPluginOrder::First,
            ),
            PackagePlugin::new(
                Arc::new(NamedTestPlugin("first-b")),
                HvigorPluginOrder::First,
            ),
            PackagePlugin::new(Arc::new(NamedTestPlugin("last-a")), HvigorPluginOrder::Last),
            PackagePlugin::new(Arc::new(NamedTestPlugin("last-b")), HvigorPluginOrder::Last),
        ];

        let plugins = PluginFactory::new(&plan, input)
            .build()
            .expect("plugin factory");
        let names: Vec<_> = plugins
            .iter()
            .map(|plugin| plugin.call_name().to_string())
            .collect();

        assert_eq!(&names[0..2], &["first-a", "first-b"]);
        assert_eq!(&names[names.len() - 2..], &["last-a", "last-b"]);
    }

    #[test]
    fn package_plugins_use_absolute_positions_resolved_by_js_runtime() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().to_path_buf();
        let plan = make_plan(&root);
        let mut input = PluginFactoryInput::from_plan(&plan);
        input.package_plugins = vec![
            PackagePlugin::new(
                Arc::new(NamedTestPlugin("before-all")),
                HvigorPluginOrder::Position(0),
            ),
            PackagePlugin::new(
                Arc::new(NamedTestPlugin("after-duplicate-check")),
                HvigorPluginOrder::Position(2),
            ),
        ];

        let plugins = PluginFactory::new(&plan, input)
            .build()
            .expect("plugin factory");
        let names: Vec<_> = plugins
            .iter()
            .map(|plugin| plugin.call_name().to_string())
            .collect();

        assert_eq!(names[0], "before-all");
        assert_eq!(names[2], "after-duplicate-check");
    }

    #[test]
    fn native_anchor_tracks_the_optional_har_dedup_slot() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().to_path_buf();

        let hap_plan = make_plan(&root);
        let mut hap_input = PluginFactoryInput::from_plan(&hap_plan);
        hap_input.package_plugins = vec![PackagePlugin::new(
            Arc::new(NamedTestPlugin("before-ets")),
            HvigorPluginOrder::Anchor {
                rank: HvigorNativePluginSlot::EtsTransform.rank(),
                sequence: 0,
            },
        )];
        let hap_names = PluginFactory::new(&hap_plan, hap_input)
            .build()
            .expect("HAP plugin factory")
            .into_iter()
            .map(|plugin| plugin.call_name().to_string())
            .collect::<Vec<_>>();

        let mut hsp_plan = make_plan(&root);
        hsp_plan.output_target = OutputTarget::Hsp;
        let mut hsp_input = PluginFactoryInput::from_plan(&hsp_plan);
        hsp_input.har_dedup = HarDedupFactoryMode::Collect {
            collect_file: root.join("har-dedup.json"),
            source_har_roots: Vec::new(),
        };
        hsp_input.package_plugins = vec![PackagePlugin::new(
            Arc::new(NamedTestPlugin("before-ets")),
            HvigorPluginOrder::Anchor {
                rank: HvigorNativePluginSlot::EtsTransform.rank(),
                sequence: 0,
            },
        )];
        let hsp_names = PluginFactory::new(&hsp_plan, hsp_input)
            .build()
            .expect("HSP plugin factory")
            .into_iter()
            .map(|plugin| plugin.call_name().to_string())
            .collect::<Vec<_>>();

        for names in [&hap_names, &hsp_names] {
            let custom = names
                .iter()
                .position(|name| name == "before-ets")
                .expect("custom plugin");
            let ets = names
                .iter()
                .position(|name| name == "ets-transform")
                .expect("ets transform");
            assert_eq!(custom + 1, ets);
        }
        let har = hsp_names
            .iter()
            .position(|name| name == "har-dedup")
            .expect("HAR dedup plugin");
        let custom = hsp_names
            .iter()
            .position(|name| name == "before-ets")
            .expect("custom plugin");
        assert!(har < custom);
    }

    #[test]
    fn native_anchor_falls_forward_when_adapter_behavior_is_owned_by_scan() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().to_path_buf();
        let plan = make_plan(&root);
        let mut input = PluginFactoryInput::from_plan(&plan);
        input.package_plugins = vec![PackagePlugin::new(
            Arc::new(NamedTestPlugin("module-meta-anchor")),
            HvigorPluginOrder::Anchor {
                rank: HvigorNativePluginSlot::ModuleMeta.rank(),
                sequence: 0,
            },
        )];

        let names = PluginFactory::new(&plan, input)
            .build()
            .expect("plugin factory")
            .into_iter()
            .map(|plugin| plugin.call_name().to_string())
            .collect::<Vec<_>>();
        let custom = names
            .iter()
            .position(|name| name == "module-meta-anchor")
            .expect("custom plugin");
        let resolve = names
            .iter()
            .position(|name| name == "ohos-resolve")
            .expect("OHOS resolver");
        assert_eq!(custom + 1, resolve);
    }

    #[test]
    fn end_anchor_stays_last_in_both_build_modes() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().to_path_buf();
        for mode in [BuildMode::Debug, BuildMode::Release] {
            let mut plan = make_plan(&root);
            plan.build_mode = mode;
            let mut input = PluginFactoryInput::from_plan(&plan);
            input.package_plugins = vec![PackagePlugin::new(
                Arc::new(NamedTestPlugin("pipeline-end")),
                HvigorPluginOrder::Anchor {
                    rank: HvigorNativePluginSlot::END_RANK,
                    sequence: 0,
                },
            )];

            let names = PluginFactory::new(&plan, input)
                .build()
                .expect("plugin factory")
                .into_iter()
                .map(|plugin| plugin.call_name().to_string())
                .collect::<Vec<_>>();
            assert_eq!(names.last().map(String::as_str), Some("pipeline-end"));
            // The minify and wrapperChunk native adapters were deleted
            // (render_chunk is unreachable on the scan-only path); no built-in
            // plugin may carry those call names in either mode.
            assert!(!names.iter().any(|name| name == "minify"));
            assert!(!names.iter().any(|name| name == "wrapperChunk"));
        }
    }

    #[test]
    fn hvigor_gap_is_an_internal_rolldown_plugin() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().to_path_buf();
        let plan = make_plan(&root);
        let mut input = PluginFactoryInput::from_plan(&plan);
        input.package_plugins = vec![PackagePlugin::with_gap_hooks(
            Arc::new(NamedTestPlugin("custom")),
            HvigorPluginOrder::Last,
            Arc::new(EmptyGapHooks),
        )];

        let plugins = PluginFactory::new(&plan, input)
            .build()
            .expect("plugin factory");
        assert_eq!(
            plugins[0].call_name().as_ref(),
            "arkdown:hvigor-before-build-end"
        );
        assert_eq!(
            plugins[0].call_build_end_meta().and_then(|meta| meta.order),
            Some(RolldownPluginOrder::Pre)
        );
        assert_eq!(plugins.last().expect("custom plugin").call_name(), "custom");
    }

    #[test]
    fn external_watch_events_run_before_hvigor_before_build() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().to_path_buf();
        let plan = make_plan(&root);
        let mut input = PluginFactoryInput::from_plan(&plan);
        input.package_plugins = vec![PackagePlugin::with_gap_hooks(
            Arc::new(NamedTestPlugin("watch-custom")),
            HvigorPluginOrder::Last,
            Arc::new(WatchGapHooks),
        )];

        let plugins = PluginFactory::new(&plan, input)
            .build()
            .expect("plugin factory");
        let names = plugins
            .iter()
            .map(|plugin| plugin.call_name().to_string())
            .collect::<Vec<_>>();
        let watch = names
            .iter()
            .position(|name| name == "arkdown:hvigor-watch-change")
            .expect("watch lifecycle plugin");
        let before = names
            .iter()
            .position(|name| name == "arkdown:hvigor-before-build")
            .expect("beforeBuild lifecycle plugin");
        assert!(watch < before);
        assert_eq!(
            plugins[watch]
                .call_build_start_meta()
                .and_then(|meta| meta.order),
            Some(RolldownPluginOrder::Pre)
        );
    }

    #[test]
    fn duplicate_package_plugin_names_are_rejected() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().to_path_buf();
        let plan = make_plan(&root);
        let mut input = PluginFactoryInput::from_plan(&plan);
        input.package_plugins = vec![
            PackagePlugin::new(Arc::new(NamedTestPlugin("same")), HvigorPluginOrder::First),
            PackagePlugin::new(Arc::new(NamedTestPlugin("same")), HvigorPluginOrder::Last),
        ];

        let error = PluginFactory::new(&plan, input)
            .build()
            .expect_err("duplicated plugin names must be rejected");
        assert!(error
            .to_string()
            .contains("duplicated Rolldown plugin name 'same'"));
    }

    #[test]
    fn module_scoped_plugins_form_independent_rolldown_pipelines() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().to_path_buf();
        let plan = make_plan(&root);
        let mut input = PluginFactoryInput::from_plan(&plan);
        input.package_plugins = vec![
            PackagePlugin::new(
                Arc::new(NamedTestPlugin("same-module-name")),
                HvigorPluginOrder::Last,
            )
            .with_module_root(Some(root.join("entry"))),
            PackagePlugin::new(
                Arc::new(NamedTestPlugin("same-module-name")),
                HvigorPluginOrder::Last,
            )
            .with_module_root(Some(root.join("feature"))),
        ];

        let plugins = PluginFactory::new(&plan, input)
            .build()
            .expect("scoped plugin pipeline");
        assert_eq!(
            plugins
                .iter()
                .filter(|plugin| plugin.call_name() == "same-module-name")
                .count(),
            1
        );
    }

    #[test]
    fn target_scoped_plugins_only_enter_the_effective_target_pipeline() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().to_path_buf();
        let mut plan = make_plan(&root);
        // Real loader.json calls its `src/main` directory moduleRootPath;
        // package-plugin scope must use the session's actual module root.
        plan.input.module_root = root.join("entry/src/main");
        let mut input = PluginFactoryInput::from_plan(&plan);
        input.all_module_roots = vec![("entry".to_string(), root.join("entry"))];
        input.target_name = "phone".to_string();
        input.package_plugins = vec![
            PackagePlugin::new(
                Arc::new(NamedTestPlugin("phone-plugin")),
                HvigorPluginOrder::Last,
            )
            .with_module_root(Some(root.join("entry")))
            .with_target_name(Some("phone".to_string())),
            PackagePlugin::new(
                Arc::new(NamedTestPlugin("tablet-plugin")),
                HvigorPluginOrder::Last,
            )
            .with_module_root(Some(root.join("entry")))
            .with_target_name(Some("tablet".to_string())),
        ];

        let plugins = PluginFactory::new(&plan, input)
            .build()
            .expect("target-scoped plugin pipeline");
        assert!(plugins
            .iter()
            .any(|plugin| plugin.call_name() == "phone-plugin"));
        assert!(!plugins
            .iter()
            .any(|plugin| plugin.call_name() == "tablet-plugin"));
    }
}
