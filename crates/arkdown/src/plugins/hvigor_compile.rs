//! Latest-hvigor compile-plugin pipeline planning.
//!
//! The installed DevEco Studio 6.1.1 implementation lives in
//! `@ohos/hvigor-arkts-compose/dist/src/arkts-pack.js::getPlugins`. Custom
//! plugin ordering is resolved against the complete hvigor plugin-name list,
//! while Arkdown executes only the equivalent native Rolldown adapters. This
//! module owns both views so the Node layer only has to invoke a user-authored
//! function-valued `order` callback.

use oxc_index::{Idx, IndexVec};
use rustc_hash::{FxHashMap, FxHashSet};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

static BUILD_INSTANCE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Module-scoped `this.share.projectConfig` snapshot supplied to JavaScript
/// compile plugins. Rust owns the build model and passes this typed snapshot
/// once before Rolldown starts; the JavaScript bridge only preserves object
/// identity while user hooks execute.
#[derive(Debug, Clone)]
pub struct HvigorCompilePluginShare {
    /// Process-unique identity for one module build. Every JavaScript plugin
    /// receives this same value, allowing the compatibility layer to preserve
    /// the single shared Rollup object used by hvigor.
    pub build_instance_id: u64,
    /// Single Rust-owned effective compile configuration. The JavaScript
    /// compatibility layer receives this object directly and never derives or
    /// merges build fields.
    pub project_config: Arc<serde_json::Map<String, serde_json::Value>>,
    /// Immutable source metadata captured by `BuildSession` before hooks run.
    pub source_files: Arc<FxHashMap<PathBuf, HvigorCompilePluginSourceFile>>,
    /// Declared package aliases used by Hvigor's synchronous `importResolver`.
    pub import_resolver_entries: Arc<[HvigorCompilePluginImportEntry]>,
    /// Canonical package root to installed symbolic-link paths, matching the
    /// custom node-resolve `share.symlinkMap` consumed by ModuleMode.
    pub symlink_map: Arc<BTreeMap<PathBuf, Vec<PathBuf>>>,
    /// Dependency changes that can safely reuse the checker service. An empty
    /// or mixed invalidation batch disables incremental dependency reuse.
    pub dependency_info: HvigorCompilePluginDependencyInfo,
    /// Debounced filesystem events delivered by the Rust watch coordinator.
    ///
    /// Latest ets2bundle's `rollup-plugin-watch-change.ts` receives these
    /// through Rollup's `watchChange` hook before the next `beforeBuild`.
    /// Keeping the batch on the immutable module share lets the internal
    /// Rolldown lifecycle adapter preserve that ordering without moving watch
    /// policy into JavaScript.
    pub watch_changes: Arc<[HvigorWatchChange]>,
    /// Opaque V8 structured-clone payload loaded by Rust before hooks start.
    pub plugin_cache_snapshot: Option<Arc<[u8]>>,
}

impl HvigorCompilePluginShare {
    pub fn next_build_instance_id() -> u64 {
        BUILD_INSTANCE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    }
}

#[derive(Debug, Clone)]
pub struct HvigorCompilePluginSourceFile {
    pub content_hash: String,
    pub size: u64,
    pub modified_millis: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct HvigorCompilePluginImportEntry {
    pub alias: String,
    pub package_name: String,
    pub package_root: PathBuf,
    pub entry_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct HvigorCompilePluginDependencyInfo {
    pub enable_incremental: bool,
    pub changed_dependencies: Arc<[String]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HvigorWatchChangeEvent {
    Create,
    Update,
    Delete,
}

impl HvigorWatchChangeEvent {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::Delete => "delete",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HvigorWatchChange {
    pub path: PathBuf,
    pub event: HvigorWatchChangeEvent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct HvigorCompilePluginId(u32);

impl Idx for HvigorCompilePluginId {
    const MAX: usize = u32::MAX as usize;

    fn from_usize(index: usize) -> Self {
        Self(index as u32)
    }

    unsafe fn from_usize_unchecked(index: usize) -> Self {
        Self(index as u32)
    }

    fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct HvigorCompileScopeId(u32);

impl Idx for HvigorCompileScopeId {
    const MAX: usize = u32::MAX as usize;

    fn from_usize(index: usize) -> Self {
        Self(index as u32)
    }

    unsafe fn from_usize_unchecked(index: usize) -> Self {
        Self(index as u32)
    }

    fn index(self) -> usize {
        self.0 as usize
    }
}

/// Build facts that affect the latest IDE's built-in plugin list.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HvigorCompilePluginPlanConfig {
    #[serde(default)]
    pub compile_mode: Option<String>,
    #[serde(default)]
    pub preview: bool,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
}

impl HvigorCompilePluginPlanConfig {
    fn is_esmodule(&self) -> bool {
        self.compile_mode
            .as_deref()
            .unwrap_or("esmodule")
            .eq_ignore_ascii_case("esmodule")
    }

    fn is_release(&self) -> bool {
        self.mode
            .as_deref()
            .is_some_and(|mode| mode.eq_ignore_ascii_case("release"))
    }

    fn is_hsp(&self) -> bool {
        self.target
            .as_deref()
            .is_some_and(|target| target.eq_ignore_ascii_case("hsp"))
    }
}

/// Canonical order of Arkdown's native Rolldown adapters in the latest-hvigor
/// plugin pipeline. Optional adapters may be absent for a concrete build; an
/// anchored custom plugin then falls forward to the next enabled rank.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum HvigorNativePluginSlot {
    DuplicateCheck = 0,
    ModuleMeta = 1,
    OhosResolve = 2,
    ImportCheck = 3,
    Json = 4,
    /// Anchor-only slot: the native `wrapperChunk` adapter was deleted
    /// (`render_chunk` is unreachable on the scan-only production path).
    /// The rank stays so package-supplied JS plugins keep a stable anchor.
    WrapperChunk = 5,
    HarDedup = 6,
    EtsTransform = 7,
    ApiTransform = 8,
    /// Anchor-only slot: the native `minify` adapter was deleted (JSBUNDLE-only
    /// per CLAUDE.md rule 14; ESMODULE release minification is the
    /// obfuscation stage / ArkGuard). The rank stays as a JS-plugin anchor.
    Minify = 9,
}

impl HvigorNativePluginSlot {
    pub const END_RANK: u16 = Self::Minify as u16 + 1;

    pub const fn rank(self) -> u16 {
        self as u16
    }
}

/// Normalized order shape. JavaScript values are classified at the N-API
/// boundary; the pipeline state machine itself stays entirely in Rust.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HvigorCompilePluginOrder {
    First,
    Last,
    Callback,
    /// Latest hvigor validates falsy non-null orders but does not insert them.
    Omit,
}

/// One custom compile plugin supplied by the runtime glue.
#[derive(Debug, Clone)]
pub struct HvigorCompilePluginDescriptor {
    pub source_index: usize,
    pub name: String,
    pub module_root: Option<String>,
    pub target_name: Option<String>,
    pub order: HvigorCompilePluginOrder,
}

/// Final custom-plugin position in Arkdown's reduced native Rolldown pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HvigorCompilePluginPlacement {
    pub source_index: usize,
    pub order_index: usize,
    pub anchor_rank: u16,
    pub anchor_sequence: u32,
}

/// Failure raised while the JS glue invokes a plugin-authored order callback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HvigorCompileOrderCallbackError {
    Threw(String),
    InvalidReturn(String),
}

/// Structured diagnostic returned to the JavaScript error-object glue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HvigorCompilePluginDiagnostic {
    pub code: &'static str,
    pub message: String,
    pub plugin_name: Option<String>,
}

impl HvigorCompilePluginDiagnostic {
    fn invalid_callback(plugin_name: &str, failure: HvigorCompileOrderCallbackError) -> Self {
        let detail = match failure {
            HvigorCompileOrderCallbackError::Threw(message) => {
                format!("order function threw: {message}")
            }
            HvigorCompileOrderCallbackError::InvalidReturn(value) => {
                format!("order function must return a number, got {value}")
            }
        };
        Self {
            code: "INVALID_PLUGIN_EXPORT",
            message: format!("Plugin \"{plugin_name}\": {detail}"),
            plugin_name: Some(plugin_name.to_string()),
        }
    }

    fn duplicate(plugin_name: String) -> Self {
        Self {
            code: "DUPLICATED_PLUGIN_NAME_IN_ROLLUP",
            message: format!("duplicated Rolldown plugin name '{plugin_name}'"),
            plugin_name: Some(plugin_name),
        }
    }

    fn invalid_package_callback(
        plugin_name: &str,
        failure: HvigorCompileOrderCallbackError,
    ) -> Self {
        let detail = match failure {
            HvigorCompileOrderCallbackError::Threw(message) => {
                format!("order function threw: {message}")
            }
            HvigorCompileOrderCallbackError::InvalidReturn(value) => {
                format!("order function must return a number, got {value}")
            }
        };
        Self {
            code: "INVALID_PLUGIN_ORDER_FN",
            message: format!("Plugin \"{plugin_name}\": {detail}"),
            plugin_name: Some(plugin_name.to_string()),
        }
    }
}

/// JavaScript value facts captured once at the N-API boundary. Rust owns all
/// latest-hvigor shape rules; JavaScript keeps only the original functions and
/// object identity required to construct Rolldown hook adapters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HvigorPluginValueShape {
    pub kind: HvigorPluginValueKind,
    pub truthy: bool,
    pub string_value: Option<String>,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HvigorPluginValueKind {
    Undefined,
    Null,
    Boolean,
    Number,
    String,
    Function,
    Array,
    Object,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HvigorPluginAttributeShape {
    pub name: String,
    pub value: HvigorPluginValueShape,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HvigorPluginShape {
    pub source_index: usize,
    pub source_file: Option<String>,
    pub value: HvigorPluginValueShape,
    pub name: HvigorPluginValueShape,
    pub order: HvigorPluginValueShape,
    pub attributes: Vec<HvigorPluginAttributeShape>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HvigorPluginValidationOutcome {
    pub warnings: Vec<HvigorCompilePluginDiagnostic>,
    pub error: Option<HvigorCompilePluginDiagnostic>,
}

/// Latest-hvigor plugin surface validator shared by compile-plugin files and
/// Arkdown's explicit Rolldown package API.
pub struct HvigorPluginShapeValidator;

impl HvigorPluginShapeValidator {
    const COMPILE_HOOKS: [&'static str; 13] = [
        "beforeBuild",
        "buildStart",
        "afterBuildStart",
        "resolveId",
        "load",
        "transform",
        "shouldInvalidCache",
        "moduleParsed",
        "beforeBuildEnd",
        "buildEnd",
        "afterBuildEnd",
        "bundleEnd",
        "cleanUp",
    ];
    const PACKAGE_ATTRIBUTES: [&'static str; 16] = [
        "name",
        "order",
        "beforeBuild",
        "buildStart",
        "afterBuildStart",
        "resolveId",
        "load",
        "transform",
        "moduleParsed",
        "buildEnd",
        "resolveDynamicImport",
        "beforeBuildEnd",
        "afterBuildEnd",
        "bundleEnd",
        "cleanUp",
        "shouldInvalidCache",
    ];

    pub fn validate_compile(shape: &HvigorPluginShape) -> HvigorPluginValidationOutcome {
        let mut outcome = HvigorPluginValidationOutcome::default();
        if shape.value.kind != HvigorPluginValueKind::Object {
            outcome.error = Some(HvigorCompilePluginDiagnostic {
                code: "INVALID_PLUGIN_TYPE",
                message: format!(
                    "{} must export a compile-plugin object, got {}",
                    shape.source_file.as_deref().unwrap_or("<compile-plugin>"),
                    shape.value.description
                ),
                plugin_name: None,
            });
            return outcome;
        }
        if !shape.name.truthy {
            outcome.error = Some(HvigorCompilePluginDiagnostic {
                code: "PLUGIN_NAME_NOT_EXIST",
                message: format!(
                    "{} exports a compile plugin without a name",
                    shape.source_file.as_deref().unwrap_or("<compile-plugin>")
                ),
                plugin_name: None,
            });
            return outcome;
        }
        if shape.name.kind != HvigorPluginValueKind::String {
            outcome.error = Some(HvigorCompilePluginDiagnostic {
                code: "PLUGIN_NAME_NOT_STRING",
                message: format!(
                    "{} exports a compile plugin whose name is not a string",
                    shape.source_file.as_deref().unwrap_or("<compile-plugin>")
                ),
                plugin_name: None,
            });
            return outcome;
        }
        let plugin_name = shape.name.string_value.as_deref().unwrap_or_default();
        if let Some(error) = Self::validate_order(plugin_name, &shape.order, false) {
            outcome.error = Some(error);
            return outcome;
        }

        for attribute in &shape.attributes {
            if attribute.name == "name" || attribute.name == "order" {
                continue;
            }
            if Self::COMPILE_HOOKS.contains(&attribute.name.as_str()) {
                if attribute.value.truthy
                    && !matches!(
                        attribute.value.kind,
                        HvigorPluginValueKind::Function | HvigorPluginValueKind::Object
                    )
                {
                    outcome.error = Some(HvigorCompilePluginDiagnostic {
                        code: "PLUGIN_NAME_WITH_UNKNOWN_HOOK",
                        message: format!(
                            "Plugin \"{plugin_name}\": hook \"{}\" must be a function or ordered handler object",
                            attribute.name
                        ),
                        plugin_name: Some(plugin_name.to_string()),
                    });
                    return outcome;
                }
                continue;
            }
            outcome.warnings.push(HvigorCompilePluginDiagnostic {
                code: "UNKNOWN_PLUGIN_ATTR",
                message: format!(
                    "'{}' exports an invalid plugin '{plugin_name}' with unknown property: '{}'.",
                    shape.source_file.as_deref().unwrap_or("<compile-plugin>"),
                    attribute.name
                ),
                plugin_name: Some(plugin_name.to_string()),
            });
        }
        outcome
    }

    pub fn validate_package(shape: &HvigorPluginShape) -> HvigorPluginValidationOutcome {
        let mut outcome = HvigorPluginValidationOutcome::default();
        let plugin_name = shape.name.string_value.as_deref().unwrap_or_default();
        if !shape.name.truthy {
            outcome.warnings.push(HvigorCompilePluginDiagnostic {
                code: "PLUGIN_NAME_NOT_EXIST",
                message: "A compile plugin is missing a \"name\" field.".to_string(),
                plugin_name: None,
            });
        }
        if let Some(error) = Self::validate_order(plugin_name, &shape.order, true) {
            outcome.error = Some(error);
            return outcome;
        }
        for attribute in &shape.attributes {
            if !Self::PACKAGE_ATTRIBUTES.contains(&attribute.name.as_str()) {
                outcome.warnings.push(HvigorCompilePluginDiagnostic {
                    code: "UNKNOWN_PLUGIN_ATTR",
                    message: format!(
                        "Plugin \"{plugin_name}\": unknown attribute \"{}\" will be ignored.",
                        attribute.name
                    ),
                    plugin_name: Some(plugin_name.to_string()),
                });
            }
        }
        outcome
    }

    fn validate_order(
        plugin_name: &str,
        order: &HvigorPluginValueShape,
        package_plugin: bool,
    ) -> Option<HvigorCompilePluginDiagnostic> {
        let absent = if package_plugin {
            order.kind == HvigorPluginValueKind::Undefined
        } else {
            !order.truthy
        };
        if absent
            || order.kind == HvigorPluginValueKind::Function
            || order.string_value.as_deref() == Some("first")
            || order.string_value.as_deref() == Some("last")
        {
            return None;
        }
        let suffix = if package_plugin {
            ". Expected 'first', 'last', or a function."
        } else {
            ""
        };
        Some(HvigorCompilePluginDiagnostic {
            code: "PLUGIN_NAME_WITH_UNKNOWN_ORDER",
            message: format!(
                "Plugin \"{plugin_name}\": unknown order value {}{suffix}",
                order.description
            ),
            plugin_name: Some(plugin_name.to_string()),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct HvigorCompileScope {
    module_root: Option<String>,
    target_name: Option<String>,
}

#[derive(Debug, Clone)]
struct HvigorCompilePipelineEntry {
    name: String,
    plugin_id: Option<HvigorCompilePluginId>,
    native_slot: Option<HvigorNativePluginSlot>,
}

impl HvigorCompilePipelineEntry {
    fn glue(name: &str) -> Self {
        Self {
            name: name.to_string(),
            plugin_id: None,
            native_slot: None,
        }
    }

    fn native(name: &str, native_slot: HvigorNativePluginSlot) -> Self {
        Self {
            name: name.to_string(),
            plugin_id: None,
            native_slot: Some(native_slot),
        }
    }

    fn custom(name: String, plugin_id: HvigorCompilePluginId) -> Self {
        Self {
            name,
            plugin_id: Some(plugin_id),
            native_slot: None,
        }
    }

    fn enters_native_pipeline(&self) -> bool {
        self.native_slot.is_some() || self.plugin_id.is_some()
    }
}

struct HvigorCompilePipeline {
    entries: Vec<HvigorCompilePipelineEntry>,
}

impl HvigorCompilePipeline {
    fn new(entries: Vec<HvigorCompilePipelineEntry>) -> Self {
        Self { entries }
    }

    fn insert_first(&mut self, entry: HvigorCompilePipelineEntry) {
        self.entries.insert(0, entry);
    }

    fn insert_last(&mut self, entry: HvigorCompilePipelineEntry) {
        self.entries.push(entry);
    }

    fn insert_at_js_index(&mut self, index: f64, entry: HvigorCompilePipelineEntry) {
        let index = if index.is_sign_negative() {
            0
        } else if !index.is_finite() {
            self.entries.len()
        } else {
            (index.trunc() as usize).min(self.entries.len())
        };
        self.entries.insert(index, entry);
    }

    fn name_to_index(&self) -> Vec<(String, usize)> {
        let mut values = FxHashMap::<String, usize>::default();
        let mut ordered_names = Vec::<String>::with_capacity(self.entries.len());
        for (index, entry) in self.entries.iter().enumerate() {
            let mut name = entry.name.clone();
            let mut suffix = 1usize;
            // arkts-pack.js 6.24.3 uses `while (record[name])`, so an index
            // of zero is deliberately treated as unoccupied. A normal `{}`
            // also observes truthy Object.prototype properties.
            while Self::record_value_is_truthy(&values, &name) {
                name = format!("{}_{}", entry.name, suffix);
                suffix += 1;
            }
            if !values.contains_key(&name) {
                ordered_names.push(name.clone());
            }
            values.insert(name, index);
        }
        ordered_names
            .into_iter()
            .filter_map(|name| values.get(&name).copied().map(|index| (name, index)))
            .collect()
    }

    fn name_to_index_with_own_keys(&self) -> Vec<(String, usize)> {
        let mut values = FxHashMap::<String, usize>::default();
        let mut ordered_names = Vec::<String>::with_capacity(self.entries.len());
        for (index, entry) in self.entries.iter().enumerate() {
            let mut name = entry.name.clone();
            let mut suffix = 1usize;
            while values.contains_key(&name) {
                name = format!("{}_{}", entry.name, suffix);
                suffix += 1;
            }
            ordered_names.push(name.clone());
            values.insert(name, index);
        }
        ordered_names
            .into_iter()
            .filter_map(|name| values.get(&name).copied().map(|index| (name, index)))
            .collect()
    }

    fn record_value_is_truthy(values: &FxHashMap<String, usize>, name: &str) -> bool {
        values.get(name).map_or_else(
            || {
                matches!(
                    name,
                    "constructor"
                        | "__defineGetter__"
                        | "__defineSetter__"
                        | "hasOwnProperty"
                        | "__lookupGetter__"
                        | "__lookupSetter__"
                        | "isPrototypeOf"
                        | "propertyIsEnumerable"
                        | "toString"
                        | "valueOf"
                        | "__proto__"
                        | "toLocaleString"
                )
            },
            |index| *index != 0,
        )
    }

    fn duplicate_name(&self) -> Option<String> {
        let mut names = FxHashSet::default();
        self.entries
            .iter()
            .find_map(|entry| (!names.insert(entry.name.clone())).then(|| entry.name.clone()))
    }

    fn custom_placements(&self) -> Vec<(HvigorCompilePluginId, usize, u16, u32)> {
        let reduced = self
            .entries
            .iter()
            .filter(|entry| entry.enters_native_pipeline())
            .collect::<Vec<_>>();
        let mut anchor_sequences = FxHashMap::<u16, u32>::default();
        reduced
            .iter()
            .enumerate()
            .filter_map(|(order_index, entry)| {
                let plugin_id = entry.plugin_id?;
                let anchor_rank = reduced
                    .iter()
                    .skip(order_index + 1)
                    .find_map(|entry| entry.native_slot)
                    .map_or(
                        HvigorNativePluginSlot::END_RANK,
                        HvigorNativePluginSlot::rank,
                    );
                let anchor_sequence = anchor_sequences.entry(anchor_rank).or_default();
                let placement = (plugin_id, order_index, anchor_rank, *anchor_sequence);
                *anchor_sequence += 1;
                Some(placement)
            })
            .collect()
    }
}

/// One explicit package plugin participating in Arkdown's Rolldown-compatible
/// top-level function-order extension.
#[derive(Debug, Clone)]
pub struct RolldownPackagePluginDescriptor {
    pub source_index: usize,
    pub name: String,
    pub has_order_callback: bool,
}

/// Final explicit-plugin sequence. Callback plugins are marked so the JS glue
/// can remove the now-resolved function property before creating TSFNs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RolldownPackagePluginPlacement {
    pub source_index: usize,
    pub callback_resolved: bool,
}

/// Rust owner of explicit package-plugin function ordering. Non-function
/// `first`/`last` orders remain attached to the plugin and are interpreted by
/// `PluginFactory`; callback plugins are resolved into deterministic list order
/// before entering the Rolldown bridge.
pub struct RolldownPackagePluginPlanner;

impl RolldownPackagePluginPlanner {
    pub fn plan<F>(
        descriptors: Vec<RolldownPackagePluginDescriptor>,
        mut invoke_order: F,
    ) -> Result<Vec<RolldownPackagePluginPlacement>, HvigorCompilePluginDiagnostic>
    where
        F: FnMut(
            &RolldownPackagePluginDescriptor,
            &[(String, usize)],
        ) -> Result<f64, HvigorCompileOrderCallbackError>,
    {
        let plugins: IndexVec<HvigorCompilePluginId, RolldownPackagePluginDescriptor> =
            descriptors.into_iter().collect();
        let mut pipeline = HvigorCompilePipeline::new(
            plugins
                .iter_enumerated()
                .filter(|(_, plugin)| !plugin.has_order_callback)
                .map(|(plugin_id, plugin)| {
                    HvigorCompilePipelineEntry::custom(plugin.name.clone(), plugin_id)
                })
                .collect(),
        );
        let callback_ids = plugins
            .iter_enumerated()
            .filter_map(|(plugin_id, plugin)| plugin.has_order_callback.then_some(plugin_id))
            .collect::<Vec<_>>();
        for plugin_id in callback_ids.iter().copied() {
            let descriptor = &plugins[plugin_id];
            let name_to_index = pipeline.name_to_index_with_own_keys();
            let index = invoke_order(descriptor, &name_to_index).map_err(|failure| {
                HvigorCompilePluginDiagnostic::invalid_package_callback(&descriptor.name, failure)
            })?;
            if index.is_nan() {
                return Err(HvigorCompilePluginDiagnostic::invalid_package_callback(
                    &descriptor.name,
                    HvigorCompileOrderCallbackError::InvalidReturn("null".to_string()),
                ));
            }
            pipeline.insert_at_js_index(
                index,
                HvigorCompilePipelineEntry::custom(descriptor.name.clone(), plugin_id),
            );
        }
        let callback_ids = callback_ids.into_iter().collect::<FxHashSet<_>>();
        Ok(pipeline
            .entries
            .iter()
            .filter_map(|entry| entry.plugin_id)
            .map(|plugin_id| RolldownPackagePluginPlacement {
                source_index: plugins[plugin_id].source_index,
                callback_resolved: callback_ids.contains(&plugin_id),
            })
            .collect())
    }
}

/// Rust owner of latest-hvigor compile-plugin ordering and native-slot
/// reduction.
pub struct HvigorCompilePluginPlanner {
    config: HvigorCompilePluginPlanConfig,
}

impl HvigorCompilePluginPlanner {
    pub fn new(config: HvigorCompilePluginPlanConfig) -> Self {
        Self { config }
    }

    pub fn plan<F>(
        &self,
        descriptors: Vec<HvigorCompilePluginDescriptor>,
        mut invoke_order: F,
    ) -> Result<Vec<HvigorCompilePluginPlacement>, HvigorCompilePluginDiagnostic>
    where
        F: FnMut(
            &HvigorCompilePluginDescriptor,
            &[(String, usize)],
        ) -> Result<f64, HvigorCompileOrderCallbackError>,
    {
        let plugins: IndexVec<HvigorCompilePluginId, HvigorCompilePluginDescriptor> =
            descriptors.into_iter().collect();
        let scopes = self.scopes(&plugins);
        let mut placements = Vec::new();

        for plugin_ids in scopes {
            let mut pipeline = HvigorCompilePipeline::new(self.builtins());
            for plugin_id in plugin_ids {
                let descriptor = &plugins[plugin_id];
                let entry = HvigorCompilePipelineEntry::custom(descriptor.name.clone(), plugin_id);
                match descriptor.order {
                    HvigorCompilePluginOrder::First => pipeline.insert_first(entry),
                    HvigorCompilePluginOrder::Last => pipeline.insert_last(entry),
                    HvigorCompilePluginOrder::Callback => {
                        let name_to_index = pipeline.name_to_index();
                        let index =
                            invoke_order(descriptor, &name_to_index).map_err(|failure| {
                                HvigorCompilePluginDiagnostic::invalid_callback(
                                    &descriptor.name,
                                    failure,
                                )
                            })?;
                        if index.is_nan() {
                            return Err(HvigorCompilePluginDiagnostic::invalid_callback(
                                &descriptor.name,
                                HvigorCompileOrderCallbackError::InvalidReturn("null".to_string()),
                            ));
                        }
                        pipeline.insert_at_js_index(index, entry);
                    }
                    HvigorCompilePluginOrder::Omit => {}
                }
            }

            if let Some(name) = pipeline.duplicate_name() {
                return Err(HvigorCompilePluginDiagnostic::duplicate(name));
            }
            placements.extend(pipeline.custom_placements().into_iter().map(
                |(plugin_id, order_index, anchor_rank, anchor_sequence)| {
                    HvigorCompilePluginPlacement {
                        source_index: plugins[plugin_id].source_index,
                        order_index,
                        anchor_rank,
                        anchor_sequence,
                    }
                },
            ));
        }

        Ok(placements)
    }

    fn scopes(
        &self,
        plugins: &IndexVec<HvigorCompilePluginId, HvigorCompilePluginDescriptor>,
    ) -> IndexVec<HvigorCompileScopeId, Vec<HvigorCompilePluginId>> {
        let mut scope_ids = FxHashMap::<HvigorCompileScope, HvigorCompileScopeId>::default();
        let mut scopes = IndexVec::<HvigorCompileScopeId, Vec<HvigorCompilePluginId>>::new();
        for (plugin_id, descriptor) in plugins.iter_enumerated() {
            let scope = HvigorCompileScope {
                module_root: descriptor.module_root.clone(),
                target_name: descriptor.target_name.clone(),
            };
            let scope_id = *scope_ids
                .entry(scope)
                .or_insert_with(|| scopes.push(Vec::new()));
            scopes[scope_id].push(plugin_id);
        }
        scopes
    }

    fn builtins(&self) -> Vec<HvigorCompilePipelineEntry> {
        let mut entries = vec![
            HvigorCompilePipelineEntry::glue("commonPlugin"),
            HvigorCompilePipelineEntry::glue("invalidCachePlugin"),
            HvigorCompilePipelineEntry::native(
                "checkDuplicatedFileNamesPlugin",
                HvigorNativePluginSlot::DuplicateCheck,
            ),
        ];
        if self.config.is_esmodule() {
            entries.push(HvigorCompilePipelineEntry::native(
                "moduleInfoMetaPlugin",
                HvigorNativePluginSlot::ModuleMeta,
            ));
        }
        entries.extend([
            HvigorCompilePipelineEntry::native("oh-resolve", HvigorNativePluginSlot::OhosResolve),
            HvigorCompilePipelineEntry::glue("routerMapPlugin"),
            HvigorCompilePipelineEntry::glue("newOhmUrlPlugin"),
            HvigorCompilePipelineEntry::native(
                "sensitiveCheckPlugin",
                HvigorNativePluginSlot::ImportCheck,
            ),
            HvigorCompilePipelineEntry::glue("commonjs"),
            HvigorCompilePipelineEntry::glue("gcPlugin"),
            HvigorCompilePipelineEntry::native("json", HvigorNativePluginSlot::Json),
            HvigorCompilePipelineEntry::native(
                "wrapperChunkPlugin",
                HvigorNativePluginSlot::WrapperChunk,
            ),
            HvigorCompilePipelineEntry::glue("waitAsyncPlugin"),
            HvigorCompilePipelineEntry::glue("collectImportersPlugin"),
        ]);
        if self.config.is_hsp() {
            entries.push(HvigorCompilePipelineEntry::native(
                "hspCollectHarFiles",
                HvigorNativePluginSlot::HarDedup,
            ));
        }
        entries.extend([
            HvigorCompilePipelineEntry::glue("memoryMonitor"),
            HvigorCompilePipelineEntry::glue("watchChangedFiles"),
            HvigorCompilePipelineEntry::glue("etsChecker"),
            HvigorCompilePipelineEntry::native(
                "etsTransform",
                HvigorNativePluginSlot::EtsTransform,
            ),
            // Unlike the old JS translation, this adapter is a real Rust
            // Rolldown slot and must participate in absolute positions.
            HvigorCompilePipelineEntry::native(
                "apiTransform",
                HvigorNativePluginSlot::ApiTransform,
            ),
            HvigorCompilePipelineEntry::glue("buildInstrument"),
            HvigorCompilePipelineEntry::glue("genAbc"),
        ]);
        if self.config.is_release() {
            entries.push(HvigorCompilePipelineEntry::native(
                "terser",
                HvigorNativePluginSlot::Minify,
            ));
        }
        entries.push(HvigorCompilePipelineEntry::glue("createProgramPlugin"));
        if !self.config.preview {
            entries.push(HvigorCompilePipelineEntry::glue("delete"));
        }
        entries
    }
}
