//! Rolldown transform plugin collecting hvigor module sources and source maps.

use std::borrow::Cow;
#[cfg(feature = "memory-diagnostics")]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use rustc_hash::{FxHashMap, FxHashSet};

use rolldown_plugin::{
    HookTransformArgs, HookTransformReturn, HookUsage, Plugin, SharedTransformPluginContext,
};
use rolldown_utils::dashmap::FxDashMap;

use crate::plugins::ets_transform::EtsTransformModuleMetadata;

use super::*;

#[derive(Debug, Clone, Default)]
pub(crate) struct HvigorModuleTransformStore {
    pub(crate) module_source_overrides: Arc<FxDashMap<String, Arc<str>>>,
    pub(crate) source_maps: Arc<FxDashMap<String, Arc<str>>>,
    pub(crate) javascript_module_ids: Arc<FxDashMap<String, ()>>,
    pub(crate) shared_module_ids: Arc<FxDashMap<String, ()>>,
}

impl HvigorModuleTransformStore {
    pub(crate) fn record(
        &self,
        module_id: String,
        module_source_code: Option<Arc<str>>,
        source_map: Option<Arc<str>>,
        emits_javascript: bool,
        is_shared_module: bool,
    ) {
        if let Some(module_source_code) = module_source_code {
            self.module_source_overrides
                .insert(module_id.clone(), module_source_code);
        }
        if let Some(source_map) = source_map {
            self.source_maps.insert(module_id.clone(), source_map);
        }
        if emits_javascript {
            self.javascript_module_ids.insert(module_id.clone(), ());
        }
        if is_shared_module {
            self.shared_module_ids.insert(module_id, ());
        }
    }

    pub(crate) fn finish(&self) -> HvigorModuleTransformSnapshot {
        let mut module_source_overrides = FxHashMap::default();
        for item in self.module_source_overrides.iter() {
            module_source_overrides.insert(item.key().clone(), Arc::clone(item.value()));
        }
        self.module_source_overrides.clear();
        let mut source_maps = FxHashMap::default();
        for item in self.source_maps.iter() {
            source_maps.insert(item.key().clone(), Arc::clone(item.value()));
        }
        self.source_maps.clear();
        let javascript_module_ids = self
            .javascript_module_ids
            .iter()
            .map(|item| item.key().clone())
            .collect();
        self.javascript_module_ids.clear();
        let shared_module_ids = self
            .shared_module_ids
            .iter()
            .map(|item| item.key().clone())
            .collect();
        self.shared_module_ids.clear();
        HvigorModuleTransformSnapshot {
            module_source_overrides,
            source_maps,
            javascript_module_ids,
            shared_module_ids,
        }
    }
}

pub(crate) struct HvigorModuleTransformSnapshot {
    pub(crate) module_source_overrides: FxHashMap<String, Arc<str>>,
    pub(crate) source_maps: FxHashMap<String, Arc<str>>,
    pub(crate) javascript_module_ids: FxHashSet<String>,
    pub(crate) shared_module_ids: FxHashSet<String>,
}

#[derive(Debug)]
pub(crate) struct HvigorModuleTransformCollector {
    pub(crate) store: HvigorModuleTransformStore,
}

impl HvigorModuleTransformCollector {
    pub(crate) fn new(store: HvigorModuleTransformStore) -> Self {
        Self { store }
    }

    /// `transform.js::preserveSourceMap` ignores Rollup/CommonJS virtual
    /// modules. Only a materializable runtime JavaScript module can own a
    /// ModuleMode source-map entry.
    pub(crate) fn is_runtime_javascript_source(module_id: &str) -> bool {
        HvigorModuleSourceId::new(module_id).should_compile()
            && arkdown_ets_transform::SourceClassifier::classify(module_id).is_javascript_runtime()
    }

    pub(crate) fn restore_hvigor_javascript_mapping_lines(
        raw_map: String,
        is_comment_only_javascript: bool,
    ) -> String {
        let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&raw_map) else {
            return raw_map;
        };
        let Some(mappings) = value.get("mappings").and_then(|value| value.as_str()) else {
            return raw_map;
        };
        let mappings = if is_comment_only_javascript {
            mappings.is_empty().then_some(";".to_string())
        } else {
            (!mappings.ends_with(';')).then(|| format!("{mappings};"))
        };
        if let Some(mappings) = mappings {
            value["mappings"] = serde_json::Value::String(mappings);
            return serde_json::to_string(&value).unwrap_or(raw_map);
        }
        raw_map
    }
}

impl Plugin for HvigorModuleTransformCollector {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("arkdown-module-source-transform-collector")
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::Transform
    }

    fn transform(
        &self,
        ctx: SharedTransformPluginContext,
        args: &HookTransformArgs<'_>,
    ) -> impl Future<Output = HookTransformReturn> + Send {
        let store = self.store.clone();
        let module_id = args.id.to_string();
        let metadata = ctx
            .meta()
            .get::<EtsTransformModuleMetadata>()
            .and_then(|metadata| metadata.take(&module_id));
        let is_runtime_javascript = Self::is_runtime_javascript_source(&module_id);
        let source_map = if let Some(metadata) = metadata.as_ref() {
            // Latest IDE ets-loader `transform.js::preserveSourceMap` retains
            // Rollup's combined mappings verbatim, then changes only source
            // attribution/package metadata in its SourceMapGenerator. Rolldown
            // drops the final unmapped line that TypeScript's source-map URL
            // comment contributes to an emitted JavaScript module. Restore
            // that boundary for SDK `Program.emit` output; comment-only input
            // is its one-line `"mappings":";"` sentinel form.
            let raw_map = ctx.get_combined_sourcemap().to_json_string();
            let raw_map = if metadata.emits_javascript {
                Self::restore_hvigor_javascript_mapping_lines(
                    raw_map,
                    metadata.is_comment_only_javascript,
                )
            } else {
                raw_map
            };
            Some(Arc::<str>::from(raw_map))
        } else if is_runtime_javascript {
            // ets-loader's SourceMapGenerator records runtime JavaScript as
            // well as ETS/TS transform inputs. JS bypasses EtsTransformPlugin,
            // so retain Rolldown's identity/combined map here instead of
            // silently losing every package JS source from sourceMaps.json.
            Some(Arc::<str>::from(
                ctx.get_combined_sourcemap().to_json_string(),
            ))
        } else {
            None
        };
        async move {
            if let Some(metadata) = metadata {
                store.record(
                    module_id,
                    metadata.module_source_code,
                    source_map,
                    metadata.emits_javascript,
                    metadata.is_shared_module,
                );
            } else if let Some(source_map) = source_map {
                store.record(module_id, None, Some(source_map), true, false);
            }
            Ok(None)
        }
    }
}

#[cfg(feature = "memory-diagnostics")]
#[derive(Debug, Default)]
pub(crate) struct MemoryTransformDiagnosticPlugin {
    pub(crate) transformed_modules: AtomicUsize,
    pub(crate) transformed_code_bytes: AtomicUsize,
}

#[cfg(feature = "memory-diagnostics")]
impl MemoryTransformDiagnosticPlugin {
    pub(crate) fn new() -> Self {
        Self::default()
    }
}

#[cfg(feature = "memory-diagnostics")]
impl Plugin for MemoryTransformDiagnosticPlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("arkdown-memory-transform-diagnostics")
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::Transform
    }

    fn transform(
        &self,
        _ctx: SharedTransformPluginContext,
        args: &HookTransformArgs<'_>,
    ) -> impl Future<Output = HookTransformReturn> + Send {
        let count = self.transformed_modules.fetch_add(1, Ordering::Relaxed) + 1;
        let bytes = self
            .transformed_code_bytes
            .fetch_add(args.code.len(), Ordering::Relaxed)
            + args.code.len();
        if count == 1 || count % 500 == 0 {
            crate::memory_diagnostics::dump_with(
                "bundle:transform-progress",
                format_args!(
                    "modules={} observed_code_bytes={} last_module={} last_code_bytes={}",
                    count,
                    bytes,
                    args.id,
                    args.code.len()
                ),
            );
        }
        async { Ok(None) }
    }
}
