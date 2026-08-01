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

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;

use arkdown_core::OutputTarget;
use arkdown_path::{
    lexical_normalize, strip_relative_prefix, AbsPath, ModuleSpecifier, SpecifierKind,
};
use arkdown_resolver::{OhosResolver, PkgBriefInfo, ResolverConfig};
use rolldown_common::ModuleInfo;
use rolldown_plugin::{
    __inner::SharedPluginable, HookBuildEndArgs, HookNoopReturn, HookUsage, Plugin, PluginContext,
    PluginHookMeta, PluginOrder as RolldownPluginOrder,
};

use crate::plan::BundlePlan;
use crate::plugins::duplicate_check::DuplicateCheckPlugin;
use crate::plugins::ets_transform::{
    CheckerMode, ComponentMetadata, EtsTransformPlugin, RolldownEtsTransformPlan,
    TransformEntrySourceId, TransformSourceSnapshot,
};
use crate::plugins::har_dedup::HarDedupPlugin;
use crate::plugins::import_check::ImportCheckPlugin;
use crate::plugins::json5::Json5Plugin;
use crate::plugins::minify::MinifyPlugin;
use crate::plugins::module_meta::ModuleMetaPlugin;
use crate::plugins::ohos_resolve::OhosResolvePlugin;
use crate::plugins::wrapper_chunk::WrapperChunkPlugin;
use crate::PluginCacheModule;

/// Placement of a latest-hvigor compile plugin in the Rolldown plugin list.
///
/// String orders map directly to the two outer positions. Function-valued
/// orders are evaluated by the Node runtime against hvigor's plugin-name map
/// and arrive here as one absolute slot in Arkdown's Rust-native list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HvigorPluginOrder {
    First,
    #[default]
    Last,
    Position(usize),
}

/// Read-only module graph exposed as `this.getModuleIds()` and
/// `this.getModuleInfo(id)` to latest-hvigor compile plugins.
///
/// The latest DevEco Studio 6.24.3 Rollup fork invokes `beforeBuildEnd` after
/// the graph has been built and before any `buildEnd` hook. Keeping this as a
/// typed snapshot lets the NAPI runtime expose that graph without retaining a
/// non-`Send` Rolldown [`PluginContext`] across an async JavaScript call.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackagePluginGraphContext {
    module_ids: Vec<String>,
    module_info_map: BTreeMap<String, PackagePluginModuleInfo>,
}

impl PackagePluginGraphContext {
    fn from_rolldown(context: &PluginContext) -> Self {
        let mut module_ids = Vec::new();
        let mut module_info_map = BTreeMap::new();
        for module_id in context.get_module_ids() {
            let id = module_id.as_str().to_string();
            module_ids.push(id.clone());
            if let Some(module_info) = context.get_module_info(&id) {
                module_info_map.insert(id, PackagePluginModuleInfo::from_rolldown(&module_info));
            }
        }
        Self {
            module_ids,
            module_info_map,
        }
    }

    pub fn to_json(&self) -> anyhow::Result<String> {
        serde_json::to_string(self).map_err(Into::into)
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PackagePluginModuleInfo {
    id: String,
    code: Option<String>,
    ast: Option<()>,
    original_code: Option<()>,
    imported_ids: Vec<String>,
    importers: Vec<String>,
    is_entry: bool,
    is_external: bool,
    is_included: Option<bool>,
    has_default_export: bool,
    exports: Vec<String>,
    dynamically_imported_ids: Vec<String>,
    exported_bindings: Option<()>,
    dynamic_importers: Vec<String>,
}

impl PackagePluginModuleInfo {
    fn from_rolldown(module_info: &ModuleInfo) -> Self {
        let exports = module_info
            .exports
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        Self {
            id: module_info.id.to_string(),
            code: module_info.code.as_ref().map(ToString::to_string),
            ast: None,
            original_code: None,
            imported_ids: module_info
                .imported_ids
                .iter()
                .map(ToString::to_string)
                .collect(),
            importers: module_info
                .importers
                .iter()
                .map(ToString::to_string)
                .collect(),
            is_entry: module_info.is_entry,
            is_external: false,
            is_included: None,
            has_default_export: exports.iter().any(|name| name == "default"),
            exports,
            dynamically_imported_ids: module_info
                .dynamically_imported_ids
                .iter()
                .map(ToString::to_string)
                .collect(),
            exported_bindings: None,
            dynamic_importers: module_info
                .dynamic_importers
                .iter()
                .map(ToString::to_string)
                .collect(),
        }
    }
}

/// Input data for `PluginFactory` that supplements `BundlePlan` with
/// session-level information not yet captured in the plan itself.
///
/// These fields come from `BuildSession` / `ModuleCompileContext` at the CLI
/// layer and are passed down to the factory. Once `BundlePlan` is extended to
/// carry full session data (SPEC-048 Phase 5+), this struct can be collapsed
/// into `BundlePlan`.
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
    /// Whether import path casing must match the filesystem exactly.
    pub case_sensitive_check: bool,
    /// Explicit phase for hvigor-compatible HSP HAR deduplication.
    pub har_dedup: HarDedupFactoryMode,
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
            target_name: "default".to_string(),
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
            case_sensitive_check: false,
            har_dedup: HarDedupFactoryMode::Disabled,
            disabled_plugins: vec![],
            package_plugins: vec![],
        }
    }
}

/// HAR deduplication phase selected by the CLI coordinator.
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
    pub async fn seed_hvigor_context(&self, share_json: &str) -> anyhow::Result<()> {
        let Some(gap_hooks) = &self.gap_hooks else {
            return Ok(());
        };
        gap_hooks.seed_hvigor_context(share_json).await
    }
}

/// Thin adapter for compile-plugin behavior absent from upstream Rolldown.
///
/// Standard hooks are deliberately not represented here: the wrapped plugin
/// implements [`rolldown_plugin::Plugin`] and Rolldown owns their scheduling,
/// context, diagnostics, and errors. Latest hvigor adds only two public custom
/// compile-plugin hooks that Rolldown lacks: `shouldInvalidCache` and
/// `beforeBuildEnd`. `seed_hvigor_context` supplies the latest-hvigor `share`
/// object before Rolldown starts and is not user-visible lifecycle behavior.
#[async_trait::async_trait]
pub trait HvigorPluginGapHooks: Send + Sync {
    async fn seed_hvigor_context(&self, _share_json: &str) -> anyhow::Result<()> {
        Ok(())
    }

    async fn should_invalid_cache(&self, _module: &PluginCacheModule) -> anyhow::Result<bool> {
        Ok(false)
    }

    fn has_should_invalid_cache(&self) -> bool {
        false
    }

    /// `beforeBuildEnd` with the complete post-scan module graph.
    async fn before_build_end_with_graph_context(
        &self,
        _context: &PackagePluginGraphContext,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn has_before_build_end(&self) -> bool {
        false
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

struct HvigorBeforeBuildEndPlugin {
    entries: Vec<HvigorPluginGapEntry>,
}

impl HvigorBeforeBuildEndPlugin {
    fn new(entries: Vec<HvigorPluginGapEntry>) -> Self {
        Self { entries }
    }
}

impl std::fmt::Debug for HvigorBeforeBuildEndPlugin {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HvigorBeforeBuildEndPlugin")
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

impl Plugin for HvigorBeforeBuildEndPlugin {
    fn name(&self) -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("arkdown:hvigor-before-build-end")
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::BuildEnd
    }

    fn build_end(
        &self,
        context: &PluginContext,
        args: Option<&HookBuildEndArgs<'_>>,
    ) -> impl std::future::Future<Output = HookNoopReturn> + Send {
        // Latest hvigor does not enter beforeBuildEnd/buildEnd/afterBuildEnd
        // when graph.build() throws. Rolldown reports that path by supplying
        // HookBuildEndArgs, so do not manufacture a success lifecycle here.
        let graph_context = args
            .is_none()
            .then(|| PackagePluginGraphContext::from_rolldown(context));
        let entries = self.entries.clone();
        async move {
            let Some(graph_context) = graph_context else {
                return Ok(());
            };
            // `beforeBuildEnd` is hookSeq in the latest IDE fork: preserve the
            // resolved compile-plugin pipeline order and await each hook.
            for entry in entries {
                entry
                    .hooks
                    .before_build_end_with_graph_context(&graph_context)
                    .await
                    .map_err(|error| {
                        anyhow::anyhow!(
                            "beforeBuildEnd hook for plugin '{}' failed: {error}",
                            entry.name
                        )
                    })?;
            }
            Ok(())
        }
    }

    fn build_end_meta(&self) -> Option<PluginHookMeta> {
        Some(PluginHookMeta {
            order: Some(RolldownPluginOrder::Pre),
        })
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

        let package_plugins = self
            .input
            .package_plugins
            .into_iter()
            .filter(|plugin| plugin.applies_to_module_target(&package_module_root, &target_name))
            .collect::<Vec<_>>();

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
            case_sensitive_check: self.input.case_sensitive_check,
            source_set_overlays: self.input.source_set_overlays,
        };
        let ohos_resolver = Arc::new(OhosResolver::new(resolver_config));
        let ohos_resolve_plugin: SharedPluginable = Arc::new(OhosResolvePlugin::with_resolver(
            ohos_resolver,
            project_root.clone(),
            self.plan.use_normalized_ohm_url,
            self.input.case_sensitive_check,
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
            project_path,
            self.input.all_module_roots,
            self.plan.target_api_version,
            self.input.resource_refs,
            self.input.component_metadata,
            self.input.transform_sources,
        )
        .with_form_entry_source_ids(TransformEntrySources::new(&self.plan).form_source_ids())
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
                plugins.push(har_plugin);
            }
        }

        // ── 9. MinifyPlugin ───────────────────────────────────────────────────
        let is_release = !self.plan.is_debug();
        let minify_plugin: SharedPluginable = Arc::new(MinifyPlugin::for_mode(is_release));
        plugins.push(minify_plugin);

        // ── Package API plugins (same-process Node.js NAPI bridge) ──────────
        let mut package_names = FxHashSet::default();
        for package_plugin in &package_plugins {
            let name = package_plugin.plugin.call_name().to_string();
            if !package_names.insert(name.clone()) {
                anyhow::bail!("duplicated Rolldown plugin name '{name}'");
            }
        }
        let mut ordered_plugins = plugins
            .into_iter()
            .map(|plugin| (plugin, None))
            .collect::<Vec<_>>();
        let mut package_first_plugins = Vec::new();
        let mut package_last_plugins = Vec::new();
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
                .filter(|hooks| hooks.has_before_build_end())
                .map(|hooks| HvigorPluginGapEntry {
                    name: plugin.call_name().to_string(),
                    hooks: Arc::clone(hooks),
                });
            let entry = (plugin, gap_entry);
            match order {
                HvigorPluginOrder::First => {
                    package_first_plugins.push(entry);
                }
                HvigorPluginOrder::Last => {
                    package_last_plugins.push(entry);
                }
                HvigorPluginOrder::Position(position) => {
                    package_positioned_plugins.push((position, entry));
                }
            }
        }
        if !package_positioned_plugins.is_empty() {
            package_positioned_plugins.sort_by_key(|(position, _)| *position);
            let final_len = ordered_plugins.len() + package_positioned_plugins.len();
            if let Some((position, plugin)) = package_positioned_plugins
                .iter()
                .find(|(position, _)| *position >= final_len)
            {
                anyhow::bail!(
                    "resolved plugin position {position} for '{}' exceeds pipeline length {final_len}",
                    plugin.0.call_name()
                );
            }

            let mut built_ins = ordered_plugins.into_iter();
            let mut positioned = package_positioned_plugins.into_iter().peekable();
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
            ordered_plugins = merged;
        }
        if !package_first_plugins.is_empty() || !package_last_plugins.is_empty() {
            let built_in_plugins = ordered_plugins;
            ordered_plugins = package_first_plugins;
            ordered_plugins.extend(built_in_plugins);
            ordered_plugins.extend(package_last_plugins);
        }

        let gap_entries = ordered_plugins
            .iter()
            .filter_map(|(_, gap_entry)| gap_entry.clone())
            .collect::<Vec<_>>();
        let mut plugins = ordered_plugins
            .into_iter()
            .map(|(plugin, _)| plugin)
            .collect::<Vec<_>>();
        if !gap_entries.is_empty() {
            // This is an ordinary Rolldown plugin. Its `pre` buildEnd metadata
            // supplies only hvigor's missing beforeBuildEnd boundary while all
            // standard compile hooks stay in Rolldown's own driver.
            plugins.insert(0, Arc::new(HvigorBeforeBuildEndPlugin::new(gap_entries)));
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

    #[async_trait::async_trait]
    impl HvigorPluginGapHooks for EmptyGapHooks {
        fn has_before_build_end(&self) -> bool {
            true
        }
    }

    fn make_plan(root: &PathBuf) -> BundlePlan {
        let module_root = root.join("entry");
        BundlePlan {
            input: BundleInput {
                loader_json_path: module_root
                    .join("build/default/intermediates/loader/loader.json"),
                project_root: root.clone(),
                project_path: module_root.join("src/main/ets"),
                module_root,
                module_name: "entry".to_string(),
                module_path_map: Vec::new(),
                module_record_roots: Vec::new(),
                module_meta_info_files: Vec::new(),
                entries: vec!["./ets/pages/Index.ets".to_string()],
                form_entry_sources: Vec::new(),
                runtime_entry_sources: Vec::new(),
                support_chunks: false,
                source_map_packages: crate::plan::BundleSourceMapPackageInfo {
                    entry_package_name: "entry".to_string(),
                    entry_package_version: "1.0.0".to_string(),
                    dependency_package_versions: rustc_hash::FxHashMap::default(),
                },
                runtime_packages: crate::plan::BundleRuntimePackageInfo::default(),
                byte_code_har_packages: Vec::new(),
                preload_system_so_entries: Vec::new(),
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
                Arc::new(NamedTestPlugin("after-module-meta")),
                HvigorPluginOrder::Position(3),
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
        assert_eq!(names[2], "module-meta");
        assert_eq!(names[3], "after-module-meta");
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
