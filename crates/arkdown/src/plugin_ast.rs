//! OXC-backed ESTree parsing for JavaScript compile plugins.
//!
//! Latest hvigor's `arkts-pack.js::getCompilePluginContext` forwards
//! `PluginContext.parse` directly to Rollup 4.23.0. Rollup's source-owned
//! `ParseAst` contract accepts only `allowReturnOutsideFunction`; it always
//! parses a module and silently ignores unrelated object properties. Arkdown
//! preserves exactly that surface. JSON-inexpressible literal values are
//! revived by the thin JavaScript bridge after N-API transport.

use oxc_allocator::Allocator;
use oxc_ast_visit::utf8_to_utf16::Utf8ToUtf16;
use oxc_parser::{ParseOptions, Parser};
use oxc_span::SourceType;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Rollup 4.23.0 `ParseAst` options exposed by `PluginContext.parse`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PluginAstParseOptions {
    pub allow_return_outside_function: bool,
}

impl PluginAstParseOptions {
    pub fn from_json(options_json: Option<&str>) -> Result<Self, PluginAstParseError> {
        options_json.map_or_else(
            || Ok(Self::default()),
            |json| {
                serde_json::from_str(json).map_err(|error| PluginAstParseError::Options {
                    message: error.to_string(),
                })
            },
        )
    }
}

/// Typed failures produced by the OXC plugin parser.
#[derive(Debug, thiserror::Error)]
pub enum PluginAstParseError {
    #[error("PLUGIN_PARSE_OPTIONS_ERROR: {message}")]
    Options { message: String },
    #[error("PLUGIN_PARSE_ERROR: {message} ({line}:{column}) [pos={position}]")]
    Parse {
        message: String,
        position: u32,
        line: u32,
        column: u32,
    },
    #[error("PLUGIN_AST_SERIALIZE_ERROR: {0}")]
    Serialize(#[from] serde_json::Error),
}

/// Parsed ESTree envelope used by the JavaScript bridge and cache hooks.
#[derive(Debug, Clone)]
pub struct PluginAstParseResult {
    ast: Value,
}

impl PluginAstParseResult {
    pub fn bridge_json(&self) -> Result<String, PluginAstParseError> {
        serde_json::to_string(&PluginAstBridgePayload { ast: &self.ast }).map_err(Into::into)
    }

    pub fn into_bridge_value(self) -> Value {
        Value::Object(Map::from_iter([("ast".to_string(), self.ast)]))
    }

    pub(crate) fn into_ast(self) -> Value {
        self.ast
    }
}

#[derive(Serialize)]
struct PluginAstBridgePayload<'a> {
    ast: &'a Value,
}

/// Single-use OXC parser for the plugin-facing synchronous `this.parse` API.
pub struct PluginAstParser<'a> {
    source: &'a str,
    options: PluginAstParseOptions,
}

impl<'a> PluginAstParser<'a> {
    pub fn new(source: &'a str, options: PluginAstParseOptions) -> Self {
        Self { source, options }
    }

    pub fn parse(self) -> Result<PluginAstParseResult, PluginAstParseError> {
        let allocator = Allocator::default();
        let mut parsed = Parser::new(&allocator, self.source, SourceType::mjs())
            .with_options(ParseOptions {
                allow_return_outside_function: self.options.allow_return_outside_function,
                preserve_parens: false,
                enable_ident_hashes: false,
                ..ParseOptions::default()
            })
            .parse();

        if parsed.fatal_error || !parsed.diagnostics.is_empty() {
            let (message, byte_offset) = parsed.diagnostics.first().map_or_else(
                || ("OXC parser stopped unexpectedly".to_string(), 0),
                |diagnostic| {
                    let offset = diagnostic.labels.first().map_or(0, |label| label.offset());
                    (diagnostic.to_string(), offset)
                },
            );
            let position = PluginLineIndex::utf16_offset(self.source, byte_offset);
            let location = PluginLineIndex::new(self.source).position(position);
            return Err(PluginAstParseError::Parse {
                message,
                position,
                line: location.line,
                column: location.column,
            });
        }

        Utf8ToUtf16::new(self.source).convert_program(&mut parsed.program);
        let json = parsed.program.to_estree_json_with_fixes(false, false);
        let mut ast: Value = serde_json::from_str(&json)?;
        let program = ast
            .get_mut("node")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| PluginAstParseError::Options {
                message: "OXC ESTree envelope is missing Program node".to_string(),
            })?;
        // Rollup's native AST converter consumes the hashbang but does not
        // expose an extra Program property.
        program.remove("hashbang");

        Ok(PluginAstParseResult { ast })
    }
}

struct PluginLineIndex {
    line_starts: Vec<u32>,
}

impl PluginLineIndex {
    fn new(source: &str) -> Self {
        let mut line_starts = vec![0];
        let mut offset = 0_u32;
        for character in source.chars() {
            offset += character.len_utf16() as u32;
            if character == '\n' {
                line_starts.push(offset);
            }
        }
        Self { line_starts }
    }

    fn utf16_offset(source: &str, byte_offset: u32) -> u32 {
        let mut end = (byte_offset as usize).min(source.len());
        while end > 0 && !source.is_char_boundary(end) {
            end -= 1;
        }
        source[..end].encode_utf16().count() as u32
    }

    fn position(&self, offset: u32) -> PluginSourcePosition {
        let line_index = match self.line_starts.binary_search(&offset) {
            Ok(index) => index,
            Err(index) => index.saturating_sub(1),
        };
        PluginSourcePosition {
            line: line_index as u32 + 1,
            column: offset - self.line_starts[line_index],
        }
    }
}

struct PluginSourcePosition {
    line: u32,
    column: u32,
}
