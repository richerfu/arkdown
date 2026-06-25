//! Plugin factory — assembles the ordered plugin set for a module build.
//!
//! `PluginFactory` owns the construction logic that was previously in
//! `packages/arkdown/src/cmd/build/plugin_factory.rs`. Moving it here makes the
//! ordered plugin set an internal concern of the bundler crate rather than
//! a CLI concern, and allows the CLI to depend only on `arkdown` for plugin
//! instantiation.
//!
//! # Plugin order
//!
//! The order matches hvigor's `arkts-pack.js::getBuiltInPlugins` sequence:
//!
//! | # | hvigor name | Rust plugin | Always active |
//! |---|-------------|-------------|---------------|
//! | 1 | `checkDuplicatedFileNames` | `DuplicateCheckPlugin` | yes |
//! | 2 | `moduleMetaInfo` (ESMODULE) | `ModuleMetaPlugin` | yes |
//! | 3 | `resolve` | `OhosResolvePlugin` | yes |
//! | 4 | `sensitiveCheck` | `ImportCheckPlugin` | yes |
//! | 5 | `json` | `Json5Plugin` | yes |
//! | 6 | `wrapperChunk` | `WrapperChunkPlugin` | yes (no-op when `support_chunks = false`) |
//! | 7 | `sdkPlugins` (etsTransform) | `EtsTransformPlugin` | yes |
//! | 8 | `HarDedup` (HSP) | `HarDedupPlugin` | HSP builds only |
//! | 9 | — | `MinifyPlugin` | yes (no-op in debug) |
//!
//! hvigor entries without a Rust equivalent (`commonPlugin`, `invalidCachePlugin`,
//! `routerMap`, `newOhmUrl` absorbed by resolve, `commonjs` Rolldown built-in,
//! `gc` no-op, `waitAsync` no-op, `collectImporters` post-bundle coordinator)
//! are omitted from this list.
//!
//! JavaScript compile plugins are supplied by the package runtime as normalized
//! Rolldown plugin objects. The factory does not parse or execute hvigorfile
//! sources.
//!
//! # Upstream correspondence
//!
//! Replaces `packages/arkdown/src/cmd/build/plugin_factory.rs`. The CLI shim
//! delegates to this factory once the migration is complete.

use std::path::PathBuf;
use std::sync::Arc;

use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;

use arkdown_core::OutputTarget;
use arkdown_path::{lexical_normalize, strip_relative_prefix, ModuleSpecifier, SpecifierKind};
use arkdown_resolver::{OhosResolver, PkgBriefInfo, ResolverConfig};
use rolldown_plugin::__inner::SharedPluginable;

use crate::plan::BundlePlan;
use crate::plugins::duplicate_check::DuplicateCheckPlugin;
use crate::plugins::ets_transform::{
    CheckerMode, ComponentMetadata, EtsTransformPlugin, RolldownEtsTransformPlan,
    TransformEntrySourceId, TransformSourceSnapshot,
};
use crate::plugins::har_dedup::{CollectFilePath, HarDedupPlugin};
use crate::plugins::import_check::ImportCheckPlugin;
use crate::plugins::json5::Json5Plugin;
use crate::plugins::minify::MinifyPlugin;
use crate::plugins::module_meta::ModuleMetaPlugin;
use crate::plugins::ohos_resolve::OhosResolvePlugin;
use crate::plugins::wrapper_chunk::WrapperChunkPlugin;
use crate::ShouldInvalidCacheRequest;

/// Input data for `PluginFactory` that supplements `BundlePlan` with
/// session-level information not yet captured in the plan itself.
///
/// These fields come from `BuildSession` / `ModuleCompileContext` at the CLI
/// layer and are passed down to the factory. Once `BundlePlan` is extended to
/// carry full session data (SPEC-048 Phase 5+), this struct can be collapsed
/// into `BundlePlan`.
pub struct PluginFactoryInput {
    /// SDK `api/` directories for `@ohos/*` resolution.
    pub sdk_api_dirs: Vec<PathBuf>,
    /// SDK `kits/` directories for `@kit.*` resolution.
    pub sdk_kits_dirs: Vec<PathBuf>,
    /// SDK `arkts/` directories for `@arkts.*` resolution.
    pub sdk_arkts_dirs: Vec<PathBuf>,
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
    /// SDK ArkUI component metadata loaded before Rolldown hooks run.
    pub component_metadata: ComponentMetadata,
    /// Source files loaded before Rolldown hooks run for cross-file AST analysis.
    pub transform_sources: TransformSourceSnapshot,
    /// Package roots resolved from the current module's declared dependency graph.
    ///
    /// ImportCheck treats these roots as legal package-interface targets even
    /// when the resolver maps a package import to a workspace `file:` HAR path
    /// instead of an `oh_modules` path.
    pub allowed_package_roots: Vec<PathBuf>,
    /// Package index resolved from the current module's declared dependency
    /// graph.
    ///
    /// OhosResolver consumes this instead of scanning arbitrary `oh_modules`
    /// entries so package visibility matches hvigor's ModuleDependencyInfo.
    pub pkg_name_to_pkg_info: FxHashMap<String, PkgBriefInfo>,
    /// Source-set overlays for build-target overrides.
    ///
    /// Each entry is `(src_main_ets_root, target_ets_root)`.
    pub source_set_overlays: Vec<(PathBuf, PathBuf)>,
    /// ArkTS checker mode forwarded from package/CLI config.
    pub checker_mode: CheckerMode,
    /// HAR dedup collect-file path (used for HSP builds).
    ///
    /// If `None`, the factory falls back to
    /// `<module_root>/build/default/intermediates/har_files/default/har_files.json`.
    pub har_dedup_collect_file: Option<PathBuf>,
    /// Built-in plugin names to disable for this build.
    ///
    /// Populated from `arkdown.config.json` `plugins.disabled` via the coordinator.
    pub disabled_plugins: Vec<String>,
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
            sdk_api_dirs: vec![],
            sdk_kits_dirs: vec![],
            sdk_arkts_dirs: vec![],
            all_module_roots: vec![(
                plan.input.module_name.clone(),
                plan.input.module_root.clone(),
            )],
            resource_refs: FxHashMap::default(),
            component_metadata: ComponentMetadata::default(),
            transform_sources: TransformSourceSnapshot::default(),
            allowed_package_roots: vec![],
            pkg_name_to_pkg_info: FxHashMap::default(),
            source_set_overlays: vec![],
            checker_mode: CheckerMode::Oxc,
            har_dedup_collect_file: None,
            disabled_plugins: vec![],
            package_plugins: vec![],
        }
    }
}

/// A package-supplied plugin plus its hvigor insertion order.
#[derive(Clone)]
pub struct PackagePlugin {
    plugin: SharedPluginable,
    order: crate::plugins::external::PluginOrder,
    runtime_hooks: Option<Arc<dyn PackagePluginRuntimeHooks>>,
}

impl PackagePlugin {
    /// Create a package plugin entry.
    pub fn new(plugin: SharedPluginable, order: crate::plugins::external::PluginOrder) -> Self {
        Self {
            plugin,
            order,
            runtime_hooks: None,
        }
    }

    /// Create a package plugin with package-runtime-only hooks.
    pub fn with_runtime_hooks(
        plugin: SharedPluginable,
        order: crate::plugins::external::PluginOrder,
        runtime_hooks: Arc<dyn PackagePluginRuntimeHooks>,
    ) -> Self {
        Self {
            plugin,
            order,
            runtime_hooks: Some(runtime_hooks),
        }
    }

    /// Stable identity used by upstream build-stage cache fingerprints.
    ///
    /// The JavaScript function body is owned by the package runtime and is not
    /// serializable in Rust; the runtime must invalidate by passing a changed
    /// plugin name/order or by implementing `shouldInvalidCache`.
    pub fn cache_identity(&self) -> String {
        let order = match self.order {
            crate::plugins::external::PluginOrder::First => "first",
            crate::plugins::external::PluginOrder::Last => "last",
        };
        match self
            .runtime_hooks
            .as_ref()
            .and_then(|hooks| hooks.runtime_cache_identity())
        {
            Some(runtime_identity) => {
                format!("{order}:{}:{runtime_identity}", self.plugin.call_name())
            }
            None => format!("{order}:{}", self.plugin.call_name()),
        }
    }

    /// Run the package-runtime `shouldInvalidCache` hook when present.
    pub async fn should_invalid_cache(
        &self,
        request: &ShouldInvalidCacheRequest,
    ) -> anyhow::Result<bool> {
        let Some(runtime_hooks) = &self.runtime_hooks else {
            return Ok(false);
        };
        runtime_hooks.should_invalid_cache(request).await
    }

    /// Run the package-runtime `beforeBuildEnd` hook when present.
    pub async fn before_build_end(&self) -> anyhow::Result<()> {
        self.before_build_end_with_context(&[]).await
    }

    /// Run the package-runtime `beforeBuildEnd` hook, passing module IDs as context.
    ///
    /// `module_ids` contains the OHM-URL module identifiers produced by the bundle
    /// stage.  They are serialised as `{ moduleIds: [...] }` and forwarded to the
    /// JS hook argument, making `getModuleIds()` available inside `beforeBuildEnd`.
    ///
    /// # Upstream correspondence
    ///
    /// Mirrors hvigor's `getCompilePluginContext(this).getModuleIds()` surface,
    /// which is most commonly used inside `beforeBuildEnd`.
    pub async fn before_build_end_with_context(&self, module_ids: &[String]) -> anyhow::Result<()> {
        let Some(runtime_hooks) = &self.runtime_hooks else {
            return Ok(());
        };
        runtime_hooks
            .before_build_end_with_context(module_ids)
            .await
    }

    /// Run the package-runtime `shouldTransformCachedModule` hook when present.
    pub async fn should_transform_cached_module(
        &self,
        request: &ShouldInvalidCacheRequest,
    ) -> anyhow::Result<bool> {
        let Some(runtime_hooks) = &self.runtime_hooks else {
            return Ok(false);
        };
        runtime_hooks.should_transform_cached_module(request).await
    }

    /// Run the package-runtime `beforeBuild` hook when present.
    pub async fn before_build(&self) -> anyhow::Result<()> {
        self.before_build_with_share("").await
    }

    /// Run the `beforeBuild` hook with a serialized share context.
    ///
    /// The `share_json` string is forwarded to the JS TSFN as a JSON share
    /// context so that plugin hooks can access build metadata as `this.share`.
    /// Matches hvigor's `getCompilePluginContext(this).share` surface.
    pub async fn before_build_with_share(&self, share_json: &str) -> anyhow::Result<()> {
        let Some(runtime_hooks) = &self.runtime_hooks else {
            return Ok(());
        };
        runtime_hooks.before_build_with_share(share_json).await
    }

    /// Run the package-runtime `afterBuildStart` hook when present.
    pub async fn after_build_start(&self) -> anyhow::Result<()> {
        self.after_build_start_with_share("").await
    }

    /// Run the `afterBuildStart` hook with a serialized share context.
    pub async fn after_build_start_with_share(&self, share_json: &str) -> anyhow::Result<()> {
        let Some(runtime_hooks) = &self.runtime_hooks else {
            return Ok(());
        };
        runtime_hooks.after_build_start_with_share(share_json).await
    }

    /// Run the package-runtime `afterBuildEnd` hook when present.
    ///
    /// Always called after the bundle stage, even on error.
    pub async fn after_build_end(&self, success: bool) -> anyhow::Result<()> {
        let Some(runtime_hooks) = &self.runtime_hooks else {
            return Ok(());
        };
        runtime_hooks.after_build_end(success).await
    }

    /// Run the package-runtime `cleanUp` hook when present.
    ///
    /// Always called at the end of the build session, even on error.
    pub async fn clean_up(&self) -> anyhow::Result<()> {
        let Some(runtime_hooks) = &self.runtime_hooks else {
            return Ok(());
        };
        runtime_hooks.clean_up().await
    }

    /// Forward a warning message to the plugin's `warn` handler, if present.
    pub async fn warn(&self, message: &str) -> anyhow::Result<()> {
        let Some(runtime_hooks) = &self.runtime_hooks else {
            return Ok(());
        };
        runtime_hooks.warn(message).await
    }

    /// Forward an error message to the plugin's `error` handler, if present.
    pub async fn error(&self, message: &str) -> anyhow::Result<()> {
        let Some(runtime_hooks) = &self.runtime_hooks else {
            return Ok(());
        };
        runtime_hooks.error(message).await
    }
}

/// Lifecycle hook contract for package-supplied plugins that operate outside
/// the Rolldown build pipeline.
///
/// These hooks are called by [`BuildCoordinator`] at specific points in the
/// build lifecycle.  They correspond to hvigor's `beforeBuild`, `buildStart`,
/// `afterBuildStart`, `beforeBuildEnd`, `buildEnd`, `afterBuildEnd`, `cleanUp`
/// hook sequence.  Rolldown-native hooks (`resolveId`, `load`, `transform`,
/// etc.) are handled separately by the Rolldown plugin machinery.
///
/// All methods have default no-op implementations so that implementors only
/// need to override the hooks they care about.
#[async_trait::async_trait]
pub trait PackagePluginRuntimeHooks: Send + Sync {
    /// Called before the build starts (hvigor: `beforeBuild`).
    async fn before_build(&self) -> anyhow::Result<()> {
        self.before_build_with_share("").await
    }

    /// `beforeBuild` with a JSON share context.
    ///
    /// `share_json` is serialized build metadata passed to the JS hook as a
    /// JSON string.  runtime.js normalizeHookArguments parses it and binds
    /// it as `this` so plugins can access `this.share.bundleName` etc.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `getCompilePluginContext(this).share` in hvigor.
    async fn before_build_with_share(&self, _share_json: &str) -> anyhow::Result<()> {
        Ok(())
    }

    /// Called after `buildStart` completes (hvigor: `afterBuildStart`).
    async fn after_build_start(&self) -> anyhow::Result<()> {
        self.after_build_start_with_share("").await
    }

    /// `afterBuildStart` with a JSON share context.
    async fn after_build_start_with_share(&self, _share_json: &str) -> anyhow::Result<()> {
        Ok(())
    }

    async fn should_invalid_cache(
        &self,
        _request: &ShouldInvalidCacheRequest,
    ) -> anyhow::Result<bool> {
        Ok(false)
    }

    /// Called per-module to decide if a cached transformation should be
    /// re-run (hvigor: `shouldTransformCachedModule`).
    async fn should_transform_cached_module(
        &self,
        _request: &ShouldInvalidCacheRequest,
    ) -> anyhow::Result<bool> {
        Ok(false)
    }

    async fn before_build_end(&self) -> anyhow::Result<()> {
        self.before_build_end_with_context(&[]).await
    }

    /// `beforeBuildEnd` with module context.
    ///
    /// Default implementation serialises `module_ids` as
    /// `{ "moduleIds": [...] }` and calls `before_build_end`; implementations
    /// can override directly to receive the context.
    async fn before_build_end_with_context(&self, _module_ids: &[String]) -> anyhow::Result<()> {
        Ok(())
    }

    /// Called after `buildEnd` completes (hvigor: `afterBuildEnd`).
    ///
    /// `success` is `true` when the build completed without errors.
    /// Always called, even when the build failed.
    async fn after_build_end(&self, _success: bool) -> anyhow::Result<()> {
        Ok(())
    }

    /// Called for resource cleanup after the build session ends
    /// (hvigor: `cleanUp`).  Always called, even on error.
    async fn clean_up(&self) -> anyhow::Result<()> {
        Ok(())
    }

    /// Forward a warning diagnostic from the Rust pipeline to the plugin.
    ///
    /// Corresponds to hvigor's `context.warn(msg)` from `getCompilePluginContext`.
    /// When the Rust build pipeline emits a warning, it calls this on all registered
    /// plugins that provide a `warn` handler.  Default: no-op.
    async fn warn(&self, _message: &str) -> anyhow::Result<()> {
        Ok(())
    }

    /// Forward an error diagnostic from the Rust pipeline to the plugin.
    ///
    /// Corresponds to hvigor's `context.error(msg)` from `getCompilePluginContext`.
    /// Default: no-op.
    async fn error(&self, _message: &str) -> anyhow::Result<()> {
        Ok(())
    }

    fn runtime_cache_identity(&self) -> Option<String> {
        None
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

    fn source_ids(&self) -> FxHashSet<String> {
        let mut source_ids = FxHashSet::default();
        for entry in &self.plan.input.entries {
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
        path.canonicalize()
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

    /// Build the ordered plugin list.
    ///
    /// Returns an `anyhow::Result` so that construction errors (e.g. a missing
    /// ETS loader path) are surfaced cleanly.
    pub fn build(self) -> anyhow::Result<Vec<SharedPluginable>> {
        Ok(self.build_output()?.plugins)
    }

    pub fn build_output(self) -> anyhow::Result<PluginFactoryOutput> {
        let project_root = self.plan.input.project_root.clone();
        let module_root = self.plan.input.module_root.clone();
        let module_name = self.plan.input.module_name.clone();

        let package_plugins = self.input.package_plugins;

        // ── 1. DuplicateCheckPlugin (hvigor: checkDuplicatedFileNames) ──────────
        let duplicate_check_plugin: SharedPluginable = Arc::new(DuplicateCheckPlugin::new());

        // ── 2. ModuleMetaPlugin (hvigor: moduleMetaInfo, ESMODULE only) ──────
        let module_meta_plugin: SharedPluginable =
            Arc::new(ModuleMetaPlugin::new(project_root.clone(), module_name));

        // ── 3. OhosResolvePlugin (hvigor: resolve + newOhmUrl) ───────────────
        let resolver_config = ResolverConfig {
            project_root: project_root.clone(),
            sdk_api_dirs: self.input.sdk_api_dirs,
            sdk_kits_dirs: self.input.sdk_kits_dirs,
            sdk_arkts_dirs: self.input.sdk_arkts_dirs,
            module_paths: vec![
                module_root.join("oh_modules"),
                project_root.join("oh_modules"),
            ],
            pkg_name_to_pkg_info: self.input.pkg_name_to_pkg_info,
            use_normalized_ohm_url: self.plan.use_normalized_ohm_url,
            case_sensitive_check: false,
            source_set_overlays: self.input.source_set_overlays,
        };
        let ohos_resolver = Arc::new(OhosResolver::new(resolver_config));
        let ohos_resolve_plugin: SharedPluginable = Arc::new(OhosResolvePlugin::with_resolver(
            ohos_resolver,
            project_root.clone(),
            self.plan.use_normalized_ohm_url,
            false,
        ));

        // ── 4. ImportCheckPlugin (hvigor: sensitiveCheck) ────────────────────
        let import_check_plugin: SharedPluginable =
            Arc::new(ImportCheckPlugin::with_allowed_package_roots(
                module_root.clone(),
                self.input.allowed_package_roots,
            ));

        // ── 5. Json5Plugin (hvigor: json) ────────────────────────────────────
        let json5_plugin: SharedPluginable = Arc::new(Json5Plugin);

        // ── 6. WrapperChunkPlugin (hvigor: wrapperChunk) ─────────────────────
        let wrapper_chunk_plugin: SharedPluginable =
            Arc::new(WrapperChunkPlugin::new(self.plan.input.support_chunks));

        // ── 7. EtsTransformPlugin (hvigor: sdkPlugins / etsTransform) ────────
        //
        // Placed after resolve, json, and wrapper to match hvigor's sdkPlugins
        // position (position 15+ in getBuiltInPlugins). Rolldown calls each
        // plugin's transform hook in array order, so EtsTransformPlugin.transform
        // now runs after OhosResolvePlugin's other hooks — matching hvigor's
        // compile stage ordering.
        let ets_transform_plan = RolldownEtsTransformPlan::new(
            self.plan.bundle_name.clone(),
            self.plan.input.module_name.clone(),
            project_root.clone(),
            self.input.all_module_roots,
            self.plan.target_api_version,
            self.input.resource_refs,
            self.input.component_metadata,
            self.input.transform_sources,
        )
        .with_entry_source_ids(TransformEntrySources::new(&self.plan).source_ids())
        .with_checker_mode(self.input.checker_mode)
        .with_should_emit_js(self.plan.should_emit_js)
        .with_module_analyzer(self.plan.module_analyzer);
        let ets_transform_plan = Arc::new(ets_transform_plan);
        let ets_transform_plugin: SharedPluginable = Arc::new(EtsTransformPlugin::from_plan(
            Arc::clone(&ets_transform_plan),
        ));

        let mut plugins: Vec<SharedPluginable> = vec![
            duplicate_check_plugin,
            module_meta_plugin,
            ohos_resolve_plugin,
            import_check_plugin,
            json5_plugin,
            wrapper_chunk_plugin,
            ets_transform_plugin,
        ];

        // ── 8. HarDedupPlugin (hvigor: HarDedup, HSP only) ───────────────────
        if matches!(self.plan.output_target, OutputTarget::Hsp) {
            let collect_file = self
                .input
                .har_dedup_collect_file
                .unwrap_or_else(|| CollectFilePath::default_for_module(&module_root));
            if let Some(parent) = collect_file.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            let har_plugin: SharedPluginable = if collect_file.exists() {
                match HarDedupPlugin::real_build(collect_file.clone()) {
                    Ok(plugin) => Arc::new(plugin),
                    Err(error) => {
                        tracing::warn!(
                            error = %error,
                            "plugin-factory: HAR dedup real_build failed, falling back to collect_pass"
                        );
                        Arc::new(HarDedupPlugin::collect_pass(collect_file))
                    }
                }
            } else {
                Arc::new(HarDedupPlugin::collect_pass(collect_file))
            };
            plugins.push(har_plugin);
        }

        // ── 9. MinifyPlugin ───────────────────────────────────────────────────
        let is_release = !self.plan.is_debug();
        let minify_plugin: SharedPluginable = Arc::new(MinifyPlugin::for_mode(is_release));
        plugins.push(minify_plugin);

        // ── Package API plugins (same-process Node.js NAPI bridge) ──────────
        let mut package_first_plugins = Vec::new();
        let mut package_last_plugins = Vec::new();
        for package_plugin in package_plugins {
            match package_plugin.order {
                crate::plugins::external::PluginOrder::First => {
                    package_first_plugins.push(package_plugin.plugin);
                }
                crate::plugins::external::PluginOrder::Last => {
                    package_last_plugins.push(package_plugin.plugin);
                }
            }
        }
        if !package_first_plugins.is_empty() || !package_last_plugins.is_empty() {
            let built_in_plugins = plugins;
            plugins = package_first_plugins;
            plugins.extend(built_in_plugins);
            plugins.extend(package_last_plugins);
        }

        let mut names = FxHashSet::default();
        for plugin in &plugins {
            let name = plugin.call_name().to_string();
            if !names.insert(name.clone()) {
                anyhow::bail!("duplicated Rolldown plugin name '{name}'");
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

    use arkdown_core::BuildMode;
    use rolldown_plugin::{HookUsage, Plugin};

    use super::*;
    use crate::plan::{
        BundleCachePolicy, BundleInput, BundleOutputOptions, BundlePlan, BundlePluginSet,
        LoaderResourceJoin, SourceMapPolicy,
    };
    use crate::plugins::external::PluginOrder;

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

    fn make_plan(root: &PathBuf) -> BundlePlan {
        let module_root = root.join("entry");
        BundlePlan {
            input: BundleInput {
                loader_json_path: module_root
                    .join("build/default/intermediates/loader/loader.json"),
                project_root: root.clone(),
                module_root,
                module_name: "entry".to_string(),
                module_path_map: Vec::new(),
                module_record_roots: Vec::new(),
                entries: vec!["./ets/pages/Index.ets".to_string()],
                runtime_entry_sources: Vec::new(),
                support_chunks: false,
                source_map_packages: crate::plan::BundleSourceMapPackageInfo {
                    entry_package_name: "entry".to_string(),
                    entry_package_version: "1.0.0".to_string(),
                    dependency_package_versions: rustc_hash::FxHashMap::default(),
                },
                runtime_packages: crate::plan::BundleRuntimePackageInfo::default(),
                byte_code_har_packages: Vec::new(),
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
            target_api_version: 12,
            bundle_name: "com.example.test".to_string(),
            use_normalized_ohm_url: true,
            should_emit_js: true,
            module_analyzer: arkdown_ets_transform::ModuleAnalyzerKind::default(),
            output_target: OutputTarget::Hap,
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
            PackagePlugin::new(Arc::new(NamedTestPlugin("first-a")), PluginOrder::First),
            PackagePlugin::new(Arc::new(NamedTestPlugin("first-b")), PluginOrder::First),
            PackagePlugin::new(Arc::new(NamedTestPlugin("last-a")), PluginOrder::Last),
            PackagePlugin::new(Arc::new(NamedTestPlugin("last-b")), PluginOrder::Last),
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
    fn duplicate_package_plugin_names_are_rejected() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().to_path_buf();
        let plan = make_plan(&root);
        let mut input = PluginFactoryInput::from_plan(&plan);
        input.package_plugins = vec![
            PackagePlugin::new(Arc::new(NamedTestPlugin("same")), PluginOrder::First),
            PackagePlugin::new(Arc::new(NamedTestPlugin("same")), PluginOrder::Last),
        ];

        let error = PluginFactory::new(&plan, input)
            .build()
            .expect_err("duplicated plugin names must be rejected");
        assert!(error
            .to_string()
            .contains("duplicated Rolldown plugin name 'same'"));
    }
}
