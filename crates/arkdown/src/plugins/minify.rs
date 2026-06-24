//! Minification pass plugin (release builds).
//!
//! Applies the OXC codegen minify pass to each output chunk in release mode,
//! reducing bundle size before the ABC compilation step. Skipped entirely
//! in debug builds to preserve source-level identifiers for debugging.
//!
//! # Upstream correspondence
//!
//! Replaces `terser-plugin.ts` in the SDK JS pipeline.  The upstream plugin
//! invoked Terser on each Rollup output chunk.  This adapter runs the OXC
//! codegen minifier instead, which accepts the same ESM source that rolldown
//! emits.
//!
//! # Policy
//!
//! - In debug builds the plugin is a no-op: `render_chunk` returns `None` so
//!   rolldown keeps the original chunk code unchanged.
//! - In release builds the plugin runs the OXC codegen with `minify: true`.
//!   Identifier mangling is disabled by default because ArkRuntime uses
//!   string-keyed module registries that must not be renamed.

use std::borrow::Cow;
use std::future::Future;

use oxc_allocator::Allocator;
use oxc_codegen::{Codegen, CodegenOptions};
use oxc_parser::Parser;
use oxc_span::SourceType;

use rolldown_plugin::{
    HookRenderChunkArgs, HookRenderChunkOutput, HookRenderChunkReturn, HookTransformOutputMap,
    HookUsage, Plugin, PluginContext,
};

/// Build mode selector for the minifier plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MinifyMode {
    /// Debug builds: minification is disabled.
    Debug,
    /// Release builds: minification is enabled.
    Release,
}

/// Minification policy applied in release mode.
///
/// Mirrors the subset of Terser options that hvigor's `terser-plugin.ts`
/// used.  Identifier mangling is off by default for ArkRuntime
/// compatibility.
#[derive(Debug, Clone)]
pub struct MinifyPolicy {
    /// Compress whitespace and remove unreachable code via OXC codegen minify.
    pub compress: bool,
}

impl Default for MinifyPolicy {
    fn default() -> Self {
        Self { compress: true }
    }
}

/// Rolldown plugin adapter for the OXC minification pass.
pub struct MinifyPlugin {
    mode: MinifyMode,
    policy: MinifyPolicy,
}

impl std::fmt::Debug for MinifyPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MinifyPlugin")
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}

impl MinifyPlugin {
    /// Create a plugin that runs the minifier in release mode with the given
    /// policy.
    pub fn release(policy: MinifyPolicy) -> Self {
        Self {
            mode: MinifyMode::Release,
            policy,
        }
    }

    /// Create a no-op plugin for debug builds.
    pub fn debug() -> Self {
        Self {
            mode: MinifyMode::Debug,
            policy: MinifyPolicy::default(),
        }
    }

    /// Create a plugin from a build-mode flag.
    pub fn for_mode(is_release: bool) -> Self {
        if is_release {
            Self::release(MinifyPolicy::default())
        } else {
            Self::debug()
        }
    }

    /// Apply the OXC minifier to a single chunk's source text.
    ///
    /// Returns `None` if the source does not change or if the chunk cannot be
    /// parsed (in which case the original source is preserved).
    fn minify_chunk(source: &str, _policy: &MinifyPolicy) -> anyhow::Result<Option<String>> {
        let allocator = Allocator::default();
        let source_type = SourceType::mjs();

        let parse_result = Parser::new(&allocator, source, source_type).parse();
        if !parse_result.diagnostics.is_empty() {
            // Do not fail the build for minifier parse errors; return None so
            // the original source is kept and ABC compilation can still proceed.
            tracing::warn!(
                errors = parse_result.diagnostics.len(),
                "minify: parse errors in chunk; skipping minification"
            );
            return Ok(None);
        }

        let codegen_options = CodegenOptions {
            minify: true,
            ..Default::default()
        };
        let output = Codegen::new()
            .with_options(codegen_options)
            .build(&parse_result.program);
        let minified = output.code;

        if minified == source {
            return Ok(None);
        }
        Ok(Some(minified))
    }
}

impl Plugin for MinifyPlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("minify")
    }

    fn register_hook_usage(&self) -> HookUsage {
        if matches!(self.mode, MinifyMode::Debug) {
            HookUsage::empty()
        } else {
            HookUsage::RenderChunk
        }
    }

    fn render_chunk(
        &self,
        _ctx: &PluginContext,
        args: &HookRenderChunkArgs<'_>,
    ) -> impl Future<Output = HookRenderChunkReturn> + Send {
        // In debug mode register_hook_usage returns empty so rolldown will not
        // call this hook, but guard here for safety.
        if matches!(self.mode, MinifyMode::Debug) {
            return std::future::ready(Ok(None));
        }
        let code = args.code.to_owned();
        let policy = self.policy.clone();
        std::future::ready(
            MinifyPlugin::minify_chunk(&code, &policy)
                .map(|maybe_code| {
                    maybe_code.map(|minified| HookRenderChunkOutput {
                        code: minified,
                        map: HookTransformOutputMap::Null,
                    })
                })
                .map_err(|e| anyhow::anyhow!("minify: {e}")),
        )
    }
}
