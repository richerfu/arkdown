//! Hvigor release minification plugin.
//!
//! Replaces the SDK JS `@rollup/plugin-terser` worker with an in-process OXC
//! minifier for release Rollup/Rolldown chunks. Debug builds skip the hook.
//!
//! # Upstream correspondence
//!
//! The SDK owner is
//! `ets-loader/lib/fast_build/ark_compiler/terser-plugin.js`. It passes Terser
//! `compress.defaults=false`, enables dead-code/collapse/reduce/unused,
//! disables `join_vars` and `sequences`, and relies on
//! `@rollup/plugin-terser` to set `module=true` for ESM output. Terser's
//! default `mangle={}` is therefore active for module chunks. OXC exposes the
//! matching behavior as compressor plus module top-level mangling.

use std::borrow::Cow;
use std::future::Future;

use oxc_allocator::Allocator;
use oxc_codegen::{Codegen, CodegenOptions, CommentOptions};
use oxc_minifier::{
    CompressOptions, CompressOptionsUnused, MangleOptions, MangleOptionsKeepNames, Minifier,
    MinifierOptions,
};
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
/// Mirrors hvigor's `terser-plugin.js` release options for the Rolldown chunk
/// path. The mangle default stays enabled because `@rollup/plugin-terser`
/// enables Terser's default `mangle={}` and sets `module=true` for ESM chunks.
#[derive(Debug, Clone)]
pub struct MinifyPolicy {
    /// Run the OXC compressor using hvigor's Terser-compatible release shape.
    pub compress: bool,
    /// Run the OXC symbol mangler as Terser does for ESM chunks.
    pub mangle: bool,
}

impl Default for MinifyPolicy {
    fn default() -> Self {
        Self {
            compress: true,
            mangle: true,
        }
    }
}

/// In-process Rust implementation of hvigor's release chunk minifier.
pub struct HvigorReleaseMinifier {
    mode: MinifyMode,
    policy: MinifyPolicy,
}

impl std::fmt::Debug for HvigorReleaseMinifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HvigorReleaseMinifier")
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}

impl HvigorReleaseMinifier {
    /// Create a release minifier with the given policy.
    pub fn release(policy: MinifyPolicy) -> Self {
        Self {
            mode: MinifyMode::Release,
            policy,
        }
    }

    /// Create a no-op minifier for debug builds.
    pub fn debug() -> Self {
        Self {
            mode: MinifyMode::Debug,
            policy: MinifyPolicy::default(),
        }
    }

    /// Create a minifier from a build-mode flag.
    pub fn for_mode(is_release: bool) -> Self {
        if is_release {
            Self::release(MinifyPolicy::default())
        } else {
            Self::debug()
        }
    }

    /// Apply hvigor's release minifier to one ESM chunk.
    pub fn apply(&self, source: &str, filename: &str) -> anyhow::Result<Option<String>> {
        if matches!(self.mode, MinifyMode::Debug) {
            return Ok(None);
        }

        let allocator = Allocator::default();
        // `@rollup/plugin-terser` sets `module=true` when output format is `es`.
        let source_type = SourceType::mjs();
        let parse_result = Parser::new(&allocator, source, source_type).parse();
        if !parse_result.diagnostics.is_empty() {
            return Err(anyhow::anyhow!(
                "hvigor release minify failed to parse {filename}: {} diagnostic(s)",
                parse_result.diagnostics.len()
            ));
        }

        let mut program = parse_result.program;
        let minifier_output =
            Minifier::new(self.minifier_options()).minify(&allocator, &mut program);
        let codegen_options = CodegenOptions {
            minify: true,
            comments: CommentOptions::disabled(),
            ..Default::default()
        };
        let output = Codegen::new()
            .with_options(codegen_options)
            .with_scoping(minifier_output.scoping)
            .build(&program);
        let minified = output.code;

        if minified == source {
            return Ok(None);
        }
        Ok(Some(minified))
    }

    fn minifier_options(&self) -> MinifierOptions {
        MinifierOptions {
            compress: self.policy.compress.then(Self::compress_options),
            mangle: self.policy.mangle.then(Self::mangle_options),
        }
    }

    fn compress_options() -> CompressOptions {
        let mut options = CompressOptions {
            drop_debugger: true,
            drop_console: false,
            join_vars: false,
            sequences: false,
            unused: CompressOptionsUnused::Remove,
            ..CompressOptions::smallest()
        };
        // Terser keeps unused import specifiers under `module=true`; removing
        // them can hide module-linkage errors for missing exports.
        options.treeshake.invalid_import_side_effects = true;
        options
    }

    fn mangle_options() -> MangleOptions {
        MangleOptions {
            top_level: Some(true),
            keep_names: MangleOptionsKeepNames::all_false(),
            reserved: Default::default(),
            debug: false,
        }
    }
}

/// Rolldown plugin adapter for the OXC minification pass.
pub struct MinifyPlugin {
    minifier: HvigorReleaseMinifier,
}

impl std::fmt::Debug for MinifyPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MinifyPlugin")
            .field("minifier", &self.minifier)
            .finish()
    }
}

impl MinifyPlugin {
    /// Create a plugin that runs the minifier in release mode with the given
    /// policy.
    pub fn release(policy: MinifyPolicy) -> Self {
        Self {
            minifier: HvigorReleaseMinifier::release(policy),
        }
    }

    /// Create a no-op plugin for debug builds.
    pub fn debug() -> Self {
        Self {
            minifier: HvigorReleaseMinifier::debug(),
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
}

impl Plugin for MinifyPlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("minify")
    }

    fn register_hook_usage(&self) -> HookUsage {
        if matches!(self.minifier.mode, MinifyMode::Debug) {
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
        if matches!(self.minifier.mode, MinifyMode::Debug) {
            return std::future::ready(Ok(None));
        }

        let code = args.code.to_owned();
        let filename = args.chunk.filename.clone();
        let minifier = HvigorReleaseMinifier::release(self.minifier.policy.clone());
        std::future::ready(
            minifier
                .apply(&code, &filename)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_minifier_matches_hvigor_terser_module_mangle_contract() {
        let source = concat!(
            "export function demo(input){",
            "debugger;",
            "const unused=1;",
            "let longLocalName=input+1;",
            "console.log(longLocalName);",
            "return longLocalName;",
            "}",
            "const topLevelUnused=2;"
        );

        let minified = HvigorReleaseMinifier::release(MinifyPolicy::default())
            .apply(source, "entry.js")
            .expect("minify")
            .expect("changed");

        assert_eq!(
            minified,
            "export function demo(e){let t=e+1;console.log(t);return t}"
        );
    }

    #[test]
    fn debug_minifier_is_noop() {
        let output = HvigorReleaseMinifier::debug()
            .apply("export const value = 1;", "entry.js")
            .expect("debug apply");

        assert!(output.is_none());
    }

    #[test]
    fn release_minifier_rejects_parse_errors_without_fallback() {
        let error = HvigorReleaseMinifier::release(MinifyPolicy::default())
            .apply("export function {", "entry.js")
            .expect_err("parse error");

        assert!(
            error
                .to_string()
                .contains("hvigor release minify failed to parse entry.js"),
            "{error}"
        );
    }

    #[test]
    fn release_minifier_keeps_unused_import_specifiers_like_terser_module_mode() {
        let source = "import { unused, used } from 'pkg'; console.log(used);";

        let minified = HvigorReleaseMinifier::release(MinifyPolicy::default())
            .apply(source, "entry.js")
            .expect("minify")
            .expect("changed");

        assert!(minified.contains("unused"), "{minified}");
        assert!(minified.contains("used"), "{minified}");
        assert!(minified.contains("console.log"), "{minified}");
    }
}
