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

#[cfg(feature = "memory-diagnostics")]
use arkdown_ets_transform::RolldownEtsTransformOutput;
use arkdown_ets_transform::RolldownEtsTransformer;
use rolldown_plugin::{
    HookTransformArgs, HookTransformOutput, HookTransformOutputMap, HookTransformReturn, HookUsage,
    Plugin, SharedTransformPluginContext,
};
use rolldown_utils::dashmap::FxDashMap;

pub use arkdown_ets_transform::error::{DiagnosticSeverity, EtsTransformError};
pub use arkdown_ets_transform::transform;
pub use arkdown_ets_transform::transform::component_meta::ComponentMetadata;
pub use arkdown_ets_transform::{
    ArktsSourceChecker, CheckerMode, ModuleAnalyzerKind, NavigationRouteMap, ResourceReference,
    ResourceTransformContext, RolldownEtsTransformPlan, StaticModuleSpecifierRewrite,
    SystemResourceReferenceLoader, TransformEntrySourceId, TransformResolvedStaticModule,
    TransformSourceSnapshot,
};

/// ArkTS transform plugin adapter.
pub struct EtsTransformPlugin {
    transformer: Arc<RolldownEtsTransformer>,
    #[cfg(feature = "memory-diagnostics")]
    memory_trace: EtsTransformMemoryTrace,
}

#[derive(Debug)]
pub(crate) struct EtsTransformModuleMetadataEntry {
    pub(crate) module_source_code: Option<Arc<str>>,
    pub(crate) emits_javascript: bool,
    pub(crate) has_source_map: bool,
    pub(crate) is_comment_only_javascript: bool,
    pub(crate) is_shared_module: bool,
}

#[derive(Debug, Default)]
pub(crate) struct EtsTransformModuleMetadata {
    entries: FxDashMap<String, EtsTransformModuleMetadataEntry>,
}

impl EtsTransformModuleMetadata {
    fn record(
        &self,
        module_id: String,
        module_source_code: Option<Arc<str>>,
        emits_javascript: bool,
        has_source_map: bool,
        is_comment_only_javascript: bool,
        is_shared_module: bool,
    ) {
        self.entries.insert(
            module_id,
            EtsTransformModuleMetadataEntry {
                module_source_code,
                emits_javascript,
                has_source_map,
                is_comment_only_javascript,
                is_shared_module,
            },
        );
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
    pub fn from_plan(plan: Arc<RolldownEtsTransformPlan>) -> Self {
        Self {
            transformer: Arc::new(RolldownEtsTransformer::new(plan)),
            #[cfg(feature = "memory-diagnostics")]
            memory_trace: EtsTransformMemoryTrace::from_env(),
        }
    }

    fn transform_sync(
        &self,
        ctx: SharedTransformPluginContext,
        args: &HookTransformArgs<'_>,
    ) -> HookTransformReturn {
        #[cfg(feature = "memory-diagnostics")]
        let trace_seq = self.memory_trace.begin(args);

        let result = self
            .transformer
            .compile_transform(args.code.as_str(), args.id)
            .map_err(|error| anyhow::anyhow!("ets-transform: {error}"));

        #[cfg(feature = "memory-diagnostics")]
        self.memory_trace.end(trace_seq, args, &result);

        let Some(output) = result? else {
            return Ok(None);
        };
        let has_source_map = output.source_map.is_some();
        let map = output
            .source_map
            .as_deref()
            .map(rolldown_sourcemap::OwnedSourceMap::from_json_string)
            .transpose()
            .map_err(|error| anyhow::anyhow!("ets-transform: invalid source map: {error}"))?
            .map_or(HookTransformOutputMap::Null, |map| {
                HookTransformOutputMap::Sourcemap(Box::new(map.into_inner()))
            });
        if output.record_module_source_code {
            ctx.meta()
                .get_or_insert_default::<EtsTransformModuleMetadata>()
                .record(
                    args.id.to_string(),
                    output.module_source_code.clone(),
                    output.emits_javascript,
                    has_source_map,
                    output.is_comment_only_javascript,
                    output.is_shared_module,
                );
        }
        Ok(Some(HookTransformOutput {
            code: Some(output.code),
            map,
            ..Default::default()
        }))
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
