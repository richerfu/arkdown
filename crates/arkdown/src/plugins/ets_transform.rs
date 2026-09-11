//! ArkTS transform rolldown adapter plugin.
//!
//! Converts `.ets` source files through the `arkdown-ets-transform` domain
//! crate before rolldown processes them. Handles struct decorator rules,
//! `@Component`/`@Entry` annotations, resource reference rewriting, and
//! OHM URL module specifier normalization.
//!
//! Previously the standalone `arkdown-plugin-ets-transform` crate.

#[cfg(feature = "memory-diagnostics")]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{borrow::Cow, future::ready, sync::Arc};

use arc_swap::ArcSwap;
use arkdown_cache::CacheFingerprint;
#[cfg(feature = "memory-diagnostics")]
use arkdown_ets_transform::RolldownEtsTransformOutput;
use arkdown_ets_transform::{
    CrossplatformSourceCollection, InsightIntentSourceMetadata, RolldownEtsTransformer,
};
use rolldown_common::LogWithoutPlugin;
use rolldown_plugin::{
    HookTransformArgs, HookTransformOutput, HookTransformOutputMap, HookTransformReturn, HookUsage,
    Plugin, SharedTransformPluginContext,
};
use rolldown_utils::dashmap::FxDashMap;

use super::ets_transform_cache::EtsTransformCache;
use crate::plan::BundleCachePolicy;

pub use arkdown_ets_transform::error::{DiagnosticSeverity, EtsTransformError};
pub use arkdown_ets_transform::transform;
pub use arkdown_ets_transform::transform::component_meta::ComponentMetadata;
pub use arkdown_ets_transform::{
    ArktsSourceChecker, CheckerMode, InsightIntentSchemaCatalog, NavigationRouteMap,
    ResourceModuleReference, ResourceReference, ResourceTransformContext, RolldownEtsTransformPlan,
    StaticModuleSpecifierRewrite, SystemApiCheckOptions, SystemResourceReferenceLoader,
    TransformEntrySourceId, TransformResolvedStaticModule, TransformSourceSnapshot,
};

/// ArkTS transform plugin adapter.
pub struct EtsTransformPlugin {
    plan: ArcSwap<RolldownEtsTransformPlan>,
    cache: EtsTransformCache,
    #[cfg(feature = "memory-diagnostics")]
    memory_trace: EtsTransformMemoryTrace,
}

#[derive(Debug)]
pub(crate) struct EtsTransformModuleMetadataEntry {
    pub(crate) module_source_code: Option<Arc<str>>,
    pub(crate) transform_source_map: Option<Arc<str>>,
    pub(crate) transform_code_hash: String,
    pub(crate) module_source_emits_javascript: bool,
    pub(crate) emits_javascript: bool,
    pub(crate) has_source_map: bool,
    pub(crate) is_comment_only_javascript: bool,
    pub(crate) is_shared_module: bool,
    pub(crate) insight_intent: InsightIntentSourceMetadata,
    pub(crate) crossplatform_collection: CrossplatformSourceCollection,
}

#[derive(Debug, Default)]
pub(crate) struct EtsTransformModuleMetadata {
    entries: FxDashMap<String, EtsTransformModuleMetadataEntry>,
}

impl EtsTransformModuleMetadata {
    fn record(&self, module_id: String, entry: EtsTransformModuleMetadataEntry) {
        self.entries.insert(module_id, entry);
    }

    pub(crate) fn javascript_source_map(&self, module_id: &str) -> Option<bool> {
        self.entries
            .get(module_id)
            .and_then(|entry| entry.emits_javascript.then_some(entry.has_source_map))
    }

    pub(crate) fn take(&self, module_id: &str) -> Option<EtsTransformModuleMetadataEntry> {
        self.entries.remove(module_id).map(|(_, entry)| entry)
    }
}

impl std::fmt::Debug for EtsTransformPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EtsTransformPlugin").finish_non_exhaustive()
    }
}

impl EtsTransformPlugin {
    pub fn from_plan(
        plan: Arc<RolldownEtsTransformPlan>,
        cache_policy: &BundleCachePolicy,
    ) -> Self {
        Self {
            cache: EtsTransformCache::from_policy(cache_policy),
            plan: ArcSwap::from(plan),
            #[cfg(feature = "memory-diagnostics")]
            memory_trace: EtsTransformMemoryTrace::from_env(),
        }
    }

    /// Publish the immutable source/config snapshot for the next incremental
    /// Rolldown scan. `ArcSwap` keeps transform hooks lock-free while allowing
    /// the long-lived graph owner to survive watch rebuilds.
    pub fn update_plan(&self, plan: Arc<RolldownEtsTransformPlan>) {
        self.plan.store(plan);
    }

    pub(crate) fn begin_build(&self) {
        let plan = self.plan.load_full();
        self.cache.begin_build(&plan);
    }

    pub(crate) fn commit_cache(
        &self,
        module_imports: &rustc_hash::FxHashMap<String, Vec<String>>,
        active_module_ids: Vec<String>,
    ) {
        let plan = self.plan.load_full();
        self.cache.commit(&plan, module_imports, active_module_ids);
    }

    fn transform_sync(
        &self,
        ctx: SharedTransformPluginContext,
        args: &HookTransformArgs<'_>,
    ) -> HookTransformReturn {
        #[cfg(feature = "memory-diagnostics")]
        let trace_seq = self.memory_trace.begin(args);

        let plan = self.plan.load_full();
        let result = if let Some(output) = self.cache.restore(&plan, args.id, args.code.as_str()) {
            Ok(Some(output))
        } else {
            let result = RolldownEtsTransformer::new(Arc::clone(&plan))
                .compile_transform(args.code.as_str(), args.id)
                .map_err(|error| anyhow::anyhow!("ets-transform: {error}"));
            if let Ok(Some(output)) = &result {
                self.cache
                    .record(&plan, args.id, args.code.as_str(), output);
            }
            result
        };

        #[cfg(feature = "memory-diagnostics")]
        self.memory_trace.end(trace_seq, args, &result);

        let Some(mut output) = result? else {
            return Ok(None);
        };
        for warning in &output.warnings {
            Self::emit_warning(&ctx, warning);
        }
        let transform_code_hash = CacheFingerprint::content_hash(&output.code);
        let transform_source_map = output.source_map.take().map(Arc::<str>::from);
        let has_source_map = transform_source_map.is_some();
        let map = transform_source_map
            .as_deref()
            .map(rolldown_sourcemap::OwnedSourceMap::from_json_string)
            .transpose()
            .map_err(|error| {
                anyhow::anyhow!(
                    "ets-transform: invalid source map for '{}': {error}",
                    args.id
                )
            })?
            .map_or(HookTransformOutputMap::Null, |map| {
                HookTransformOutputMap::Sourcemap(Box::new(map.into_inner()))
            });
        if output.record_module_source_code || !output.crossplatform_collection.is_empty() {
            ctx.meta()
                .get_or_insert_default::<EtsTransformModuleMetadata>()
                .record(
                    args.id.to_string(),
                    EtsTransformModuleMetadataEntry {
                        module_source_code: output.module_source_code.clone(),
                        transform_source_map,
                        transform_code_hash,
                        module_source_emits_javascript: output.module_source_emits_javascript,
                        emits_javascript: output.emits_javascript,
                        has_source_map,
                        is_comment_only_javascript: output.is_comment_only_javascript,
                        is_shared_module: output.is_shared_module,
                        insight_intent: output.insight_intent,
                        crossplatform_collection: output.crossplatform_collection,
                    },
                );
        }
        Ok(Some(HookTransformOutput {
            code: Some(output.code),
            map,
            ..Default::default()
        }))
    }

    fn emit_warning(ctx: &SharedTransformPluginContext, warning: &EtsTransformError) {
        let (message, code, file) = match warning {
            EtsTransformError::CodedDiagnostic {
                file,
                code,
                message,
                severity: DiagnosticSeverity::Warning,
                ..
            } => (message.clone(), Some(code.clone()), file.clone()),
            EtsTransformError::Diagnostic {
                file,
                message,
                severity: DiagnosticSeverity::Warning,
            } => (message.clone(), None, file.clone()),
            _ => return,
        };
        ctx.warn(LogWithoutPlugin {
            message,
            code,
            ids: Some(vec![file]),
            ..Default::default()
        });
    }
}

#[cfg(feature = "memory-diagnostics")]
#[derive(Debug)]
struct EtsTransformMemoryTrace {
    trace_all: bool,
    high_water_enabled: bool,
    high_water_step_bytes: usize,
    next_high_water_bytes: AtomicUsize,
    sequence: AtomicUsize,
}

#[cfg(feature = "memory-diagnostics")]
impl EtsTransformMemoryTrace {
    fn from_env() -> Self {
        let trace_all = std::env::var_os("ARKDOWN_TRANSFORM_TRACE").is_some();
        let high_water_enabled =
            trace_all || std::env::var_os("ARKDOWN_TRANSFORM_HIGH_WATER").is_some();
        let step_mib = std::env::var("ARKDOWN_TRANSFORM_HIGH_WATER_STEP_MIB")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(256);
        Self {
            trace_all,
            high_water_enabled,
            high_water_step_bytes: step_mib.saturating_mul(1024 * 1024),
            next_high_water_bytes: AtomicUsize::new(step_mib.saturating_mul(1024 * 1024)),
            sequence: AtomicUsize::new(0),
        }
    }

    fn begin(&self, args: &HookTransformArgs<'_>) -> usize {
        let seq = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        if self.trace_all {
            crate::memory_diagnostics::dump_with(
                "ets-transform:begin",
                format_args!("seq={} id={} code_bytes={}", seq, args.id, args.code.len()),
            );
        }
        self.dump_high_water("begin", seq, args, None);
        seq
    }

    fn end(
        &self,
        seq: usize,
        args: &HookTransformArgs<'_>,
        result: &anyhow::Result<Option<RolldownEtsTransformOutput>>,
    ) {
        if self.trace_all {
            let details = match result {
                Ok(Some(output)) => format!(
                    "seq={} id={} code_bytes={} output_bytes={} record_module_source_code={}",
                    seq,
                    args.id,
                    args.code.len(),
                    output.code.len(),
                    output.record_module_source_code
                ),
                Ok(None) => format!(
                    "seq={} id={} code_bytes={} output=none",
                    seq,
                    args.id,
                    args.code.len()
                ),
                Err(error) => format!(
                    "seq={} id={} code_bytes={} error={}",
                    seq,
                    args.id,
                    args.code.len(),
                    error
                ),
            };
            crate::memory_diagnostics::dump_with("ets-transform:end", format_args!("{details}"));
        }
        self.dump_high_water(
            "end",
            seq,
            args,
            result.as_ref().ok().and_then(Option::as_ref),
        );
    }

    fn dump_high_water(
        &self,
        phase: &str,
        seq: usize,
        args: &HookTransformArgs<'_>,
        output: Option<&RolldownEtsTransformOutput>,
    ) {
        if !self.high_water_enabled {
            return;
        }
        let Some(snapshot) = crate::memory_diagnostics::snapshot() else {
            return;
        };
        let rss = snapshot.physical_bytes;
        let mut threshold = self.next_high_water_bytes.load(Ordering::Relaxed);
        while rss >= threshold {
            match self.next_high_water_bytes.compare_exchange(
                threshold,
                threshold.saturating_add(self.high_water_step_bytes),
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    let output_bytes = output.map_or(0, |output| output.code.len());
                    crate::memory_diagnostics::dump_with(
                        "ets-transform:high-water",
                        format_args!(
                            "phase={} seq={} rss_bytes={} threshold_bytes={} id={} code_bytes={} output_bytes={}",
                            phase,
                            seq,
                            rss,
                            threshold,
                            args.id,
                            args.code.len(),
                            output_bytes
                        ),
                    );
                    return;
                }
                Err(current) => threshold = current,
            }
        }
    }
}

impl Plugin for EtsTransformPlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("ets-transform")
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::Transform
    }

    fn transform(
        &self,
        ctx: SharedTransformPluginContext,
        args: &HookTransformArgs<'_>,
    ) -> impl std::future::Future<Output = HookTransformReturn> + Send {
        ready(self.transform_sync(ctx, args))
    }
}
