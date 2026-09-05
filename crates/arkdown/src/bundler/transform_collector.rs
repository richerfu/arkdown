//! Rolldown transform plugin collecting hvigor module sources and source maps.

use std::borrow::Cow;
#[cfg(feature = "memory-diagnostics")]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use rustc_hash::{FxHashMap, FxHashSet};

use arkdown_cache::CacheKeyMaterial;
use arkdown_ets_transform::{CrossplatformSourceCollection, InsightIntentSourceMetadata};
use rolldown_plugin::{
    HookTransformArgs, HookTransformReturn, HookUsage, Plugin, PluginHookMeta,
    PluginOrder as RolldownPluginOrder, SharedTransformPluginContext,
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
    pub(crate) insight_intent_sources: Arc<FxDashMap<String, InsightIntentSourceMetadata>>,
    pub(crate) crossplatform_collections: Arc<FxDashMap<String, CrossplatformSourceCollection>>,
}

pub(crate) struct HvigorModuleTransformRecord {
    pub(crate) module_id: String,
    pub(crate) module_source_code: Option<Arc<str>>,
    pub(crate) source_map: Option<Arc<str>>,
    pub(crate) emits_javascript: bool,
    pub(crate) is_shared_module: bool,
    pub(crate) insight_intent: InsightIntentSourceMetadata,
    pub(crate) crossplatform_collection: CrossplatformSourceCollection,
}

impl HvigorModuleTransformStore {
    pub(crate) fn record(&self, record: HvigorModuleTransformRecord) {
        let HvigorModuleTransformRecord {
            module_id,
            module_source_code,
            source_map,
            emits_javascript,
            is_shared_module,
            insight_intent,
            crossplatform_collection,
        } = record;
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
            self.shared_module_ids.insert(module_id.clone(), ());
        }
        // Keep every transformed ETS/TS source, including an empty fact set.
        // Incremental InsightIntent processing needs to distinguish an
        // untouched source from a recompiled source whose decorators were
        // removed; dropping empty facts would retain stale cache entries.
        self.insight_intent_sources
            .insert(module_id.clone(), insight_intent);
        if !crossplatform_collection.is_empty() {
            self.crossplatform_collections
                .insert(module_id, crossplatform_collection);
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
        let mut insight_intent_sources = FxHashMap::default();
        for item in self.insight_intent_sources.iter() {
            insight_intent_sources.insert(item.key().clone(), item.value().clone());
        }
        self.insight_intent_sources.clear();
        let mut crossplatform_collections = FxHashMap::default();
        for item in self.crossplatform_collections.iter() {
            crossplatform_collections.insert(item.key().clone(), item.value().clone());
        }
        self.crossplatform_collections.clear();
        HvigorModuleTransformSnapshot {
            module_source_overrides,
            source_maps,
            javascript_module_ids,
            shared_module_ids,
            insight_intent_sources,
            crossplatform_collections,
        }
    }
}

pub(crate) struct HvigorModuleTransformSnapshot {
    pub(crate) module_source_overrides: FxHashMap<String, Arc<str>>,
    pub(crate) source_maps: FxHashMap<String, Arc<str>>,
    pub(crate) javascript_module_ids: FxHashSet<String>,
    pub(crate) shared_module_ids: FxHashSet<String>,
    pub(crate) insight_intent_sources: FxHashMap<String, InsightIntentSourceMetadata>,
    pub(crate) crossplatform_collections: FxHashMap<String, CrossplatformSourceCollection>,
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
        }
        serde_json::to_string(&value).unwrap_or(raw_map)
    }

    pub(crate) fn normalize_runtime_javascript_source_map(
        raw_map: String,
        has_trailing_newline: bool,
    ) -> String {
        let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&raw_map) else {
            return raw_map;
        };
        if has_trailing_newline {
            if let Some(mappings) = value
                .get("mappings")
                .and_then(serde_json::Value::as_str)
                .filter(|mappings| !mappings.ends_with(';'))
            {
                value["mappings"] = serde_json::Value::String(format!("{mappings};"));
            }
        }
        if value.get("sourceRoot").and_then(serde_json::Value::as_str) == Some("") {
            value
                .as_object_mut()
                .expect("source map object")
                .remove("sourceRoot");
        }
        serde_json::to_string(&value).unwrap_or(raw_map)
    }

    pub(crate) fn select_transform_source_map(
        code: &str,
        transform_code_hash: &str,
        transform_source_map: Option<&str>,
        combined_source_map: String,
    ) -> String {
        let Some(transform_source_map) = transform_source_map else {
            return combined_source_map;
        };
        if CacheKeyMaterial::compute_content_hash(code) != transform_code_hash {
            return combined_source_map;
        }
        if Self::combined_map_is_projection(transform_source_map, &combined_source_map) {
            transform_source_map.to_string()
        } else {
            combined_source_map
        }
    }

    fn combined_map_is_projection(transform_source_map: &str, combined_source_map: &str) -> bool {
        let (Ok(transform), Ok(combined)) = (
            rolldown_sourcemap::OwnedSourceMap::from_json_string(transform_source_map),
            rolldown_sourcemap::OwnedSourceMap::from_json_string(combined_source_map),
        ) else {
            return false;
        };
        let lookup = transform.generate_lookup_table();
        let mut compared_source_token = false;
        for combined_token in combined.get_tokens() {
            let Some(source_id) = combined_token.get_source_id() else {
                continue;
            };
            compared_source_token = true;
            let Some(transform_token) = transform.lookup_token(
                &lookup,
                combined_token.get_dst_line(),
                combined_token.get_dst_col(),
            ) else {
                return false;
            };
            if transform_token.get_source_id().is_none()
                || transform_token.get_src_line() != combined_token.get_src_line()
                || transform_token.get_src_col() != combined_token.get_src_col()
                || transform.get_source(transform_token.get_source_id().unwrap_or_default())
                    != combined.get_source(source_id)
            {
                return false;
            }
        }
        compared_source_token
    }
}

impl Plugin for HvigorModuleTransformCollector {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("arkdown-module-source-transform-collector")
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::Transform
    }

    fn transform_meta(&self) -> Option<PluginHookMeta> {
        Some(PluginHookMeta {
            order: Some(RolldownPluginOrder::Post),
        })
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
            let combined_source_map = ctx.get_combined_sourcemap().to_json_string();
            let raw_map = if !metadata.emits_javascript {
                Self::select_transform_source_map(
                    args.code.as_str(),
                    &metadata.transform_code_hash,
                    metadata.transform_source_map.as_deref(),
                    combined_source_map,
                )
            } else {
                combined_source_map
            };
            let raw_map = if metadata.emits_javascript {
                Self::restore_hvigor_javascript_mapping_lines(
                    raw_map,
                    metadata.is_comment_only_javascript,
                )
            } else if is_runtime_javascript {
                Self::normalize_runtime_javascript_source_map(raw_map, args.code.ends_with('\n'))
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
                Self::normalize_runtime_javascript_source_map(
                    ctx.get_combined_sourcemap().to_json_string(),
                    args.code.ends_with('\n'),
                ),
            ))
        } else {
            None
        };
        async move {
            if let Some(metadata) = metadata {
                store.record(HvigorModuleTransformRecord {
                    module_id,
                    module_source_code: metadata.module_source_code,
                    source_map,
                    emits_javascript: metadata.emits_javascript,
                    is_shared_module: metadata.is_shared_module,
                    insight_intent: metadata.insight_intent,
                    crossplatform_collection: metadata.crossplatform_collection,
                });
            } else if let Some(source_map) = source_map {
                store.record(HvigorModuleTransformRecord {
                    module_id,
                    module_source_code: None,
                    source_map: Some(source_map),
                    emits_javascript: true,
                    is_shared_module: false,
                    insight_intent: InsightIntentSourceMetadata::default(),
                    crossplatform_collection: CrossplatformSourceCollection::default(),
                });
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
