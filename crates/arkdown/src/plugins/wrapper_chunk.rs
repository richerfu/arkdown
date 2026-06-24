//! Wrapper chunk emission plugin.
//!
//! Emits the `globalThis.__arkts_module_registry__` wrapper chunk that
//! ArkRuntime uses to locate and execute lazily-loaded modules. Each entry
//! chunk gets a corresponding wrapper that registers the module graph under
//! its OHM URL key.
//!
//! # Upstream correspondence
//!
//! Replaces `wrapper-chunk-plugin.ts` from `hvigor-arkts-compose`.
//!
//! ## Activation
//!
//! The plugin is only active when `supportChunks = true`. When
//! `supportChunks = false` (the default for most HAP/HAR builds) every
//! `render_chunk` call returns `None` immediately so rolldown applies no
//! modification to the emitted code.
//!
//! ## ESM mode (current pipeline)
//!
//! The current arkdown pipeline emits ESM chunks (`OutputFormat::Esm`) and
//! compiles them with `es2abc --module`. The legacy IIFE/CJS `require()`
//! rewriting described in `chunk-model.md` is **not** applied here because
//! rolldown does not emit CJS `require()` calls in ESM output mode.
//!
//! For `supportChunks = true`, this plugin appends a `globalThis` registry
//! assignment to each chunk that exports symbols so that the ArkRuntime
//! dynamic-import mechanism can locate them at runtime:
//!
//! ```js
//! // appended to the end of a producing chunk
//! globalThis["commons_a1b2c3d4"] = globalThis["commons_a1b2c3d4"] || {
//!   MyComponent: MyComponent,
//!   helperFn: helperFn,
//! };
//! ```
//!
//! Hvigor uses `chunk.fileName + "_" + chunk.chunkHash` for the registry key.
//! Rolldown 1.1 computes the final content hash after `render_chunk`, and the
//! public `RollupRenderedChunk` passed to this hook does not expose that value.
//! Until Rolldown exposes the final hash to render hooks, this adapter uses the
//! chunk stem plus a deterministic filename-derived suffix. This is a known
//! semantic gap from hvigor rather than a complete `chunkHash` match.
//!
//! ## Source-map fidelity
//!
//! The appended `globalThis` snippet is new synthetic code with no
//! correspondence to original source positions. The plugin returns `None` for
//! `map` in `HookRenderChunkOutput`, which signals rolldown to keep the
//! existing source map unchanged (the appended bytes are outside the mapped
//! range).

use std::borrow::Cow;
use std::path::Path;

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    AssignmentOperator, AssignmentTarget, Expression, LogicalOperator, ObjectPropertyKind,
    PropertyKind, Statement,
};
use oxc_ast::AstBuilder;
use oxc_codegen::{Codegen, CodegenOptions, IndentChar};
use oxc_parser::Parser;
use oxc_span::{SourceType, SPAN};
use rolldown_plugin::{
    HookRenderChunkArgs, HookRenderChunkOutput, HookRenderChunkReturn, HookTransformOutputMap,
    HookUsage, Plugin, PluginContext,
};

/// Rolldown adapter for hvigor's `wrapper-chunk-plugin.ts`.
///
/// Constructed by [`crate::plugins::factory::PluginFactory`] when
/// `BundlePlan::input::support_chunks` is `true`.
///
/// When `support_chunks` is `false`, every `render_chunk` call is a no-op
/// and the plugin imposes zero overhead.
#[derive(Debug)]
pub struct WrapperChunkPlugin {
    support_chunks: bool,
}

impl WrapperChunkPlugin {
    /// Create a new `WrapperChunkPlugin`.
    ///
    /// Pass `support_chunks = true` to enable globalThis registry emission.
    /// Pass `support_chunks = false` for a pure no-op plugin.
    pub fn new(support_chunks: bool) -> Self {
        Self { support_chunks }
    }
}

impl Plugin for WrapperChunkPlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("wrapper-chunk")
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::RenderChunk
    }

    fn render_chunk(
        &self,
        _ctx: &PluginContext,
        args: &HookRenderChunkArgs<'_>,
    ) -> impl std::future::Future<Output = HookRenderChunkReturn> + Send {
        // Capture what we need before moving into the async block.
        let support_chunks = self.support_chunks;
        let exports: Vec<String> = args.chunk.exports.iter().map(|e| e.to_string()).collect();
        let filename = args.chunk.filename.to_string();
        let code = args.code.as_ref().clone();

        async move {
            if !support_chunks {
                return Ok(None);
            }

            // Only producing chunks (chunks that export symbols) need a
            // globalThis registry entry. Entry chunks with no exports and
            // side-effect-only chunks are left unchanged.
            if exports.is_empty() {
                return Ok(None);
            }

            let registry_key = RegistryKeyDeriver::new(&filename).derive();
            let new_code = RegistryChunkAppend::new(code, registry_key, exports).apply()?;

            Ok(Some(HookRenderChunkOutput {
                code: new_code,
                // No source-map update: the appended snippet is synthetic code
                // with no original-source positions.
                map: HookTransformOutputMap::Null,
            }))
        }
    }
}

// ---------------------------------------------------------------------------
// Registry key derivation
// ---------------------------------------------------------------------------

/// Derives the `globalThis` registry key for a chunk.
struct RegistryKeyDeriver<'a> {
    filename: &'a str,
}

impl<'a> RegistryKeyDeriver<'a> {
    fn new(filename: &'a str) -> Self {
        Self { filename }
    }

    /// Format: `"<stem>_<hash8>"`.
    ///
    /// This is a deterministic fallback for hvigor's
    /// `chunk.fileName + "_" + chunk.chunkHash`; Rolldown does not expose the
    /// final `chunkHash` at `render_chunk` time.
    fn derive(&self) -> String {
        let stem = Path::new(self.filename)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(self.filename);
        let hash = Self::fnv1a_64(self.filename.as_bytes());
        let hash8 = RegistryHashPrefix::new(hash as u32);
        RegistryKeyText::from_parts([stem, "_", hash8.as_str()])
    }

    /// FNV-1a 64-bit hash of a byte slice.
    ///
    /// Matches the algorithm used elsewhere in the workspace, for example
    /// other artifact hashing paths.
    fn fnv1a_64(bytes: &[u8]) -> u64 {
        let mut h: u64 = 0xcbf29ce484222325;
        for &b in bytes {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h
    }
}

struct RegistryHashPrefix {
    value: String,
}

impl RegistryHashPrefix {
    fn new(hash: u32) -> Self {
        const HEX: &[u8; 16] = b"0123456789abcdef";

        let mut bytes = [0_u8; 8];
        for index in 0..bytes.len() {
            let shift = (bytes.len() - index - 1) * 4;
            let nibble = ((hash >> shift) & 0x0f) as usize;
            bytes[index] = HEX[nibble];
        }

        Self {
            value: bytes.iter().map(|byte| char::from(*byte)).collect(),
        }
    }

    fn as_str(&self) -> &str {
        &self.value
    }
}

struct RegistryKeyText;

impl RegistryKeyText {
    fn from_parts<const N: usize>(parts: [&str; N]) -> String {
        parts.into_iter().collect()
    }
}

struct RegistryChunkAppend {
    code: String,
    registry_key: String,
    exports: Vec<String>,
}

impl RegistryChunkAppend {
    fn new(code: String, registry_key: String, exports: Vec<String>) -> Self {
        Self {
            code,
            registry_key,
            exports,
        }
    }

    fn apply(self) -> anyhow::Result<String> {
        let allocator = Allocator::default();
        let source_type = SourceType::default().with_module(true);
        let mut parsed = Parser::new(&allocator, &self.code, source_type).parse();
        if parsed.panicked || !parsed.diagnostics.is_empty() {
            anyhow::bail!("wrapper-chunk failed to parse rolldown output before registry append");
        }

        let ast = AstBuilder::new(&allocator);
        parsed
            .program
            .body
            .push(RegistrySnippetEmitter::new(&self.registry_key, &self.exports).statement(ast));

        Ok(Codegen::new()
            .with_options(CodegenOptions {
                indent_char: IndentChar::Space,
                indent_width: 2,
                ..CodegenOptions::default()
            })
            .build(&parsed.program)
            .code)
    }
}

struct RegistrySnippetEmitter<'a> {
    registry_key: &'a str,
    exports: &'a [String],
}

impl<'a> RegistrySnippetEmitter<'a> {
    const GLOBAL_THIS: &'static str = "globalThis";

    fn new(registry_key: &'a str, exports: &'a [String]) -> Self {
        Self {
            registry_key,
            exports,
        }
    }

    fn statement<'b>(&self, ast: AstBuilder<'b>) -> Statement<'b> {
        ast.statement_expression(SPAN, self.assignment(ast))
    }

    fn assignment<'b>(&self, ast: AstBuilder<'b>) -> Expression<'b> {
        ast.expression_assignment(
            SPAN,
            AssignmentOperator::Assign,
            self.registry_assignment_target(ast),
            ast.expression_logical(
                SPAN,
                self.registry_member_expression(ast),
                LogicalOperator::Or,
                self.exports_object_expression(ast),
            ),
        )
    }

    fn registry_assignment_target<'b>(&self, ast: AstBuilder<'b>) -> AssignmentTarget<'b> {
        AssignmentTarget::ComputedMemberExpression(ast.alloc_computed_member_expression(
            SPAN,
            ast.expression_identifier(SPAN, Self::GLOBAL_THIS),
            ast.expression_string_literal(SPAN, ast.str(self.registry_key), None),
            false,
        ))
    }

    fn registry_member_expression<'b>(&self, ast: AstBuilder<'b>) -> Expression<'b> {
        Expression::ComputedMemberExpression(ast.alloc_computed_member_expression(
            SPAN,
            ast.expression_identifier(SPAN, Self::GLOBAL_THIS),
            ast.expression_string_literal(SPAN, ast.str(self.registry_key), None),
            false,
        ))
    }

    fn exports_object_expression<'b>(&self, ast: AstBuilder<'b>) -> Expression<'b> {
        ast.expression_object(
            SPAN,
            ast.vec_from_iter(self.exports.iter().map(|export| {
                ObjectPropertyKind::from(ast.object_property_kind_object_property(
                    SPAN,
                    PropertyKind::Init,
                    ast.property_key_static_identifier(SPAN, ast.str(export)),
                    ast.expression_identifier(SPAN, ast.str(export)),
                    false,
                    true,
                    false,
                ))
            })),
        )
    }
}
