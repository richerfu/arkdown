//! Latest SDK `apiTransform` Rolldown adapter.
//!
//! The owning ArkTS transform already performs the structured system API,
//! kit, and native-library rewrites. This adapter preserves the separate
//! Rollup hook boundary used by the SDK: JavaScript-emitting `.ts`/`.ets`
//! files receive a high-resolution identity map before `genAbc` calls
//! `getCombinedSourcemap()`.

use std::{borrow::Cow, future::ready};

use rolldown_plugin::{
    HookTransformArgs, HookTransformOutput, HookTransformOutputMap, HookTransformReturn, HookUsage,
    Plugin, SharedTransformPluginContext,
};
use rolldown_sourcemap::{SourceMap, SourceMapBuilder};

use super::ets_transform::EtsTransformModuleMetadata;

#[derive(Debug, Default)]
pub(crate) struct ApiTransformPlugin;

impl ApiTransformPlugin {
    fn transform_sync(
        &self,
        ctx: SharedTransformPluginContext,
        args: &HookTransformArgs<'_>,
    ) -> HookTransformReturn {
        let javascript_source_map = ctx
            .meta()
            .get::<EtsTransformModuleMetadata>()
            .and_then(|metadata| metadata.javascript_source_map(args.id));
        let Some(has_prior_source_map) = javascript_source_map else {
            return Ok(None);
        };

        // Installed SDK 6.1.1:
        // rollup-plugin-system-api.js::apiTransform.transform returns
        // `new MagicString(code).generateMap({ hires: true })` when
        // `shouldEmitJsFlagById(id)` is true. Keeping this as its own Rolldown
        // hook lets Rolldown own map-chain collapse just as Rollup does.
        let prior_source_map = has_prior_source_map.then(|| ctx.get_combined_sourcemap());
        let map = RollupHiresSourceMap::new(args.code.as_str(), args.id, prior_source_map.as_ref())
            .build();
        Ok(Some(HookTransformOutput {
            code: Some(args.code.to_string()),
            map: HookTransformOutputMap::Sourcemap(Box::new(map)),
            ..Default::default()
        }))
    }
}

/// Produces the high-resolution identity boundary emitted by SDK
/// `apiTransform`, adjusted for Rollup 3's source-map lookup semantics.
///
/// Rolldown normally composes maps with a greatest-lower-bound lookup. The
/// latest IDE ships Rollup 3.19.1-htrunk1, whose `Link.traceSegment` binary
/// search can select the next segment for columns between sparse mappings.
/// Pointing each identity token at the segment Rollup would select lets
/// Rolldown retain ownership of the hook chain while producing the same final
/// combined map.
#[derive(Debug)]
pub(super) struct RollupHiresSourceMap<'a> {
    code: &'a str,
    module_id: &'a str,
    prior_source_map: Option<&'a SourceMap>,
}

impl<'a> RollupHiresSourceMap<'a> {
    pub(super) fn new(
        code: &'a str,
        module_id: &'a str,
        prior_source_map: Option<&'a SourceMap>,
    ) -> Self {
        Self {
            code,
            module_id,
            prior_source_map,
        }
    }

    pub(super) fn build(&self) -> SourceMap {
        let prior_lines = self.prior_source_map.map(Self::index_prior_lines);
        let mut builder = SourceMapBuilder::default();
        let source_id = builder.set_source_and_content(self.module_id, self.code);
        let mut generated_line = 0_u32;
        let mut generated_column = 0_u32;

        // MagicString 0.27 walks JavaScript string indices, so both halves of
        // a surrogate pair and the newline code unit receive a mapping.
        for code_unit in self.code.encode_utf16() {
            let source_column = prior_lines
                .as_ref()
                .and_then(|lines| lines.get(generated_line as usize))
                .and_then(|segments| Self::rollup_trace_column(segments, generated_column))
                .or_else(|| self.prior_source_map.is_none().then_some(generated_column));
            if let Some(source_column) = source_column {
                builder.add_token(
                    generated_line,
                    generated_column,
                    generated_line,
                    source_column,
                    Some(source_id),
                    None,
                );
            }
            if code_unit == u16::from(b'\n') {
                generated_line += 1;
                generated_column = 0;
            } else {
                generated_column += 1;
            }
        }

        let mut source_map = builder.into_owned_sourcemap().into_inner();
        source_map.set_source_contents(vec![None]);
        source_map
    }

    fn index_prior_lines(source_map: &SourceMap) -> Vec<Vec<RollupTraceSegment>> {
        let mut lines = Vec::<Vec<RollupTraceSegment>>::new();
        for token in source_map.get_tokens() {
            let line = token.get_dst_line() as usize;
            if lines.len() <= line {
                lines.resize_with(line + 1, Vec::new);
            }
            lines[line].push(RollupTraceSegment {
                column: token.get_dst_col(),
                has_source: token.get_source_id().is_some(),
            });
        }
        lines
    }

    fn rollup_trace_column(segments: &[RollupTraceSegment], column: u32) -> Option<u32> {
        let mut search_start = 0_isize;
        let mut search_end = segments.len() as isize - 1;
        while search_start <= search_end {
            let middle = (search_start + search_end) >> 1;
            let segment = segments[middle as usize];
            if segment.column == column || search_start == search_end {
                return segment.has_source.then_some(segment.column);
            }
            if segment.column > column {
                search_end = middle - 1;
            } else {
                search_start = middle + 1;
            }
        }
        None
    }
}

#[derive(Clone, Copy, Debug)]
struct RollupTraceSegment {
    column: u32,
    has_source: bool,
}

impl Plugin for ApiTransformPlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("apiTransform")
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
