//! Cross-process cache adapter for native OXC transform outputs.
//!
//! Disk access is confined to construction and successful bundle commit. The
//! Rolldown transform hook performs immutable lookups and records misses in a
//! lock-free per-build map.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use arc_swap::ArcSwap;
use arkdown_cache::{
    CacheFingerprint, ModuleTransformCacheCommit, ModuleTransformCacheRecord,
    ModuleTransformCacheStore,
};
use arkdown_ets_transform::{
    CrossplatformSourceCollection, InsightIntentSourceMetadata, RolldownEtsTransformOutput,
    RolldownEtsTransformPlan,
};
use rolldown_utils::dashmap::FxDashMap;
use serde::{Deserialize, Serialize};

use crate::plan::BundleCachePolicy;

use super::ets_transform::{DiagnosticSeverity, EtsTransformError};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CachedEtsTransformOutput {
    code: String,
    warnings: Vec<CachedTransformWarning>,
    module_source_code: Option<String>,
    source_map: Option<String>,
    record_module_source_code: bool,
    emits_javascript: bool,
    is_comment_only_javascript: bool,
    is_shared_module: bool,
    insight_intent: InsightIntentSourceMetadata,
    crossplatform_collection: CrossplatformSourceCollection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CachedTransformWarning {
    file: String,
    message: String,
    code: Option<String>,
    span: Option<CachedTransformSpan>,
    solutions: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CachedTransformSpan {
    start: u32,
    end: u32,
}

impl CachedEtsTransformOutput {
    fn capture(output: &RolldownEtsTransformOutput) -> Self {
        Self {
            code: output.code.clone(),
            warnings: output
                .warnings
                .iter()
                .filter_map(CachedTransformWarning::capture)
                .collect(),
            module_source_code: output.module_source_code.as_deref().map(str::to_string),
            source_map: output.source_map.clone(),
            record_module_source_code: output.record_module_source_code,
            emits_javascript: output.emits_javascript,
            is_comment_only_javascript: output.is_comment_only_javascript,
            is_shared_module: output.is_shared_module,
            insight_intent: output.insight_intent.clone(),
            crossplatform_collection: output.crossplatform_collection.clone(),
        }
    }

    fn restore(self) -> RolldownEtsTransformOutput {
        RolldownEtsTransformOutput {
            code: self.code,
            warnings: self
                .warnings
                .into_iter()
                .map(CachedTransformWarning::restore)
                .collect(),
            module_source_code: self.module_source_code.map(Arc::<str>::from),
            source_map: self.source_map,
            record_module_source_code: self.record_module_source_code,
            emits_javascript: self.emits_javascript,
            is_comment_only_javascript: self.is_comment_only_javascript,
            is_shared_module: self.is_shared_module,
            insight_intent: self.insight_intent,
            crossplatform_collection: self.crossplatform_collection,
        }
    }
}

impl CachedTransformWarning {
    fn capture(error: &EtsTransformError) -> Option<Self> {
        match error {
            EtsTransformError::Diagnostic {
                file,
                message,
                severity: DiagnosticSeverity::Warning,
            } => Some(Self {
                file: file.clone(),
                message: message.clone(),
                code: None,
                span: None,
                solutions: Vec::new(),
            }),
            EtsTransformError::CodedDiagnostic {
                file,
                code,
                message,
                severity: DiagnosticSeverity::Warning,
                span,
                solutions,
            } => Some(Self {
                file: file.clone(),
                message: message.clone(),
                code: Some(code.clone()),
                span: span.map(|span| CachedTransformSpan {
                    start: span.start,
                    end: span.end,
                }),
                solutions: solutions.clone(),
            }),
            _ => None,
        }
    }

    fn restore(self) -> EtsTransformError {
        if let Some(code) = self.code {
            EtsTransformError::CodedDiagnostic {
                file: self.file,
                code,
                message: self.message,
                severity: DiagnosticSeverity::Warning,
                span: self
                    .span
                    .map(|span| oxc_span::Span::new(span.start, span.end)),
                solutions: self.solutions,
            }
        } else {
            EtsTransformError::Diagnostic {
                file: self.file,
                message: self.message,
                severity: DiagnosticSeverity::Warning,
            }
        }
    }
}

pub(super) struct EtsTransformCache {
    store: Option<ModuleTransformCacheStore>,
    read_enabled: bool,
    write_enabled: bool,
    hits: ArcSwap<BTreeMap<String, ModuleTransformCacheRecord<CachedEtsTransformOutput>>>,
    pending: FxDashMap<String, ModuleTransformCacheRecord<CachedEtsTransformOutput>>,
    hit_count: AtomicUsize,
    miss_count: AtomicUsize,
}

impl std::fmt::Debug for EtsTransformCache {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EtsTransformCache")
            .field("read_enabled", &self.read_enabled)
            .field("write_enabled", &self.write_enabled)
            .field("loaded_entries", &self.hits.load().len())
            .finish_non_exhaustive()
    }
}

impl EtsTransformCache {
    pub(super) fn from_policy(policy: &BundleCachePolicy) -> Self {
        let store = policy.cache_directory.as_ref().map(|root| {
            ModuleTransformCacheStore::new(root.clone(), Self::environment_key(policy))
        });
        Self {
            store,
            read_enabled: policy.read_cache,
            write_enabled: policy.write_cache,
            hits: ArcSwap::from_pointee(BTreeMap::new()),
            pending: FxDashMap::default(),
            hit_count: AtomicUsize::new(0),
            miss_count: AtomicUsize::new(0),
        }
    }

    pub(super) fn begin_build(&self, plan: &RolldownEtsTransformPlan) {
        self.pending.clear();
        self.hit_count.store(0, Ordering::Relaxed);
        self.miss_count.store(0, Ordering::Relaxed);
        let hits = if self.read_enabled {
            let source_hashes = plan.sources.transform_cache_source_hashes();
            self.store
                .as_ref()
                .and_then(|store| match store.load(&source_hashes) {
                    Ok(cache) => Some(cache.into_records()),
                    Err(error) => {
                        tracing::warn!(error = %error, "OXC transform cache read disabled for this build");
                        None
                    }
                })
                .unwrap_or_default()
        } else {
            BTreeMap::new()
        };
        self.hits.store(Arc::new(hits));
    }

    pub(super) fn restore(
        &self,
        plan: &RolldownEtsTransformPlan,
        module_id: &str,
        code: &str,
    ) -> Option<RolldownEtsTransformOutput> {
        if !self.read_enabled {
            self.miss_count.fetch_add(1, Ordering::Relaxed);
            return None;
        }
        let input_hash = CacheFingerprint::content_hash(code);
        let source_snapshot_hash = plan
            .sources
            .transform_cache_source_hash_for(module_id, code);
        let hits = self.hits.load();
        let Some(record) = hits.get(module_id) else {
            self.miss_count.fetch_add(1, Ordering::Relaxed);
            return None;
        };
        if record.input_hash != input_hash || record.source_snapshot_hash != source_snapshot_hash {
            self.miss_count.fetch_add(1, Ordering::Relaxed);
            return None;
        }
        self.hit_count.fetch_add(1, Ordering::Relaxed);
        Some(record.payload.clone().restore())
    }

    pub(super) fn record(
        &self,
        plan: &RolldownEtsTransformPlan,
        module_id: &str,
        input: &str,
        output: &RolldownEtsTransformOutput,
    ) {
        if !self.write_enabled {
            return;
        }
        self.pending.insert(
            module_id.to_string(),
            ModuleTransformCacheRecord {
                module_id: module_id.to_string(),
                input_hash: CacheFingerprint::content_hash(input),
                source_snapshot_hash: plan
                    .sources
                    .transform_cache_source_hash_for(module_id, input),
                payload: CachedEtsTransformOutput::capture(output),
            },
        );
    }

    pub(super) fn commit(
        &self,
        plan: &RolldownEtsTransformPlan,
        module_imports: &rustc_hash::FxHashMap<String, Vec<String>>,
        active_module_ids: Vec<String>,
    ) {
        if !self.write_enabled {
            return;
        }
        let Some(store) = self.store.as_ref() else {
            return;
        };
        let active_module_ids = active_module_ids.into_iter().collect::<BTreeSet<_>>();
        let hits = self.hits.load();
        let mut records = hits
            .values()
            .filter(|record| active_module_ids.contains(&record.module_id))
            .map(|record| (record.module_id.clone(), record.clone()))
            .collect::<BTreeMap<_, _>>();
        for entry in self.pending.iter() {
            records.insert(entry.key().clone(), entry.value().clone());
        }
        let module_imports = module_imports
            .iter()
            .map(|(module_id, imports)| (module_id.clone(), imports.clone()))
            .collect::<BTreeMap<_, _>>();
        let source_hashes = plan.sources.transform_cache_source_hashes();
        let active_module_ids = active_module_ids.into_iter().collect::<Vec<_>>();
        let result = store.commit(ModuleTransformCacheCommit {
            records: records.into_values().collect(),
            module_imports: &module_imports,
            active_module_ids: &active_module_ids,
            source_snapshot_hashes: &source_hashes,
        });
        if let Err(error) = result {
            tracing::warn!(error = %error, "failed to commit OXC transform cache");
        }
        self.pending.clear();
        tracing::debug!(
            hits = self.hit_count.load(Ordering::Relaxed),
            misses = self.miss_count.load(Ordering::Relaxed),
            "OXC transform cache committed"
        );
    }

    fn environment_key(policy: &BundleCachePolicy) -> String {
        format!(
            "{}#{}",
            policy.compile_env_key, policy.transform_environment_key
        )
    }
}
