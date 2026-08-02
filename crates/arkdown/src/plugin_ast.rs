//! OXC-backed ESTree parsing for JavaScript compile plugins.
//!
//! Latest hvigor's `arkts-pack.js::getCompilePluginContext` forwards
//! `PluginContext.parse` directly to Rollup. Arkdown keeps that developer API
//! in JavaScript, but owns parsing, span conversion, and ESTree construction in
//! Rust. The JavaScript bridge only revives values JSON cannot represent
//! (`BigInt` and `RegExp`) and dispatches `onComment` callbacks.

use oxc_allocator::Allocator;
use oxc_ast::CommentKind;
use oxc_ast_visit::utf8_to_utf16::Utf8ToUtf16;
use oxc_parser::{ParseOptions, Parser};
use oxc_span::SourceType;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Acorn/Rollup parse options supported by Arkdown's OXC bridge.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PluginAstParseOptions {
    /// `module` by default, matching Rollup's plugin context.
    pub source_type: Option<String>,
    pub ranges: bool,
    pub locations: bool,
    pub source_file: Option<String>,
    pub direct_source_file: Option<String>,
    pub allow_return_outside_function: bool,
    pub preserve_parens: bool,
    pub capture_comments: bool,
    pub ecma_version: Option<Value>,
    pub allow_await_outside_function: bool,
    pub allow_super_outside_method: bool,
    pub allow_hash_bang: Option<bool>,
    pub has_on_token: bool,
    pub has_program: bool,
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

    fn source_type(&self) -> Result<SourceType, PluginAstParseError> {
        match self.source_type.as_deref().unwrap_or("module") {
            "module" => Ok(SourceType::mjs()),
            "script" => Ok(SourceType::script()),
            value => Err(PluginAstParseError::UnsupportedOption {
                option: "sourceType",
                value: value.to_string(),
            }),
        }
    }

    fn validate(&self, source: &str) -> Result<(), PluginAstParseError> {
        if let Some(version) = self.ecma_version.as_ref() {
            let is_latest = version.as_str() == Some("latest")
                || version
                    .as_u64()
                    .is_some_and(|version| version >= 100_000_000);
            if !is_latest {
                return Err(PluginAstParseError::UnsupportedOption {
                    option: "ecmaVersion",
                    value: version.to_string(),
                });
            }
        }
        if self.allow_await_outside_function
            && self.source_type.as_deref().unwrap_or("module") == "script"
        {
            return Err(PluginAstParseError::UnsupportedOption {
                option: "allowAwaitOutsideFunction",
                value: "true with sourceType=script".to_string(),
            });
        }
        if self.allow_super_outside_method {
            return Err(PluginAstParseError::UnsupportedOption {
                option: "allowSuperOutsideMethod",
                value: "true".to_string(),
            });
        }
        if self.has_on_token {
            return Err(PluginAstParseError::UnsupportedOption {
                option: "onToken",
                value: "callback or array".to_string(),
            });
        }
        if self.has_program {
            return Err(PluginAstParseError::UnsupportedOption {
                option: "program",
                value: "existing AST".to_string(),
            });
        }
        if self.allow_hash_bang == Some(false) && source.starts_with("#!") {
            return Err(PluginAstParseError::Parse {
                message: "Unexpected character '#'".to_string(),
                position: 0,
                line: 1,
                column: 0,
            });
        }
        Ok(())
    }
}

/// Typed failures produced by the OXC plugin parser.
#[derive(Debug, thiserror::Error)]
pub enum PluginAstParseError {
    #[error("PLUGIN_PARSE_OPTIONS_ERROR: {message}")]
    Options { message: String },
    #[error("UNSUPPORTED_PLUGIN_PARSE_OPTION: {option}={value}")]
    UnsupportedOption { option: &'static str, value: String },
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

/// Parsed ESTree envelope plus Acorn-compatible comments.
#[derive(Debug, Clone)]
pub struct PluginAstParseResult {
    ast: Value,
    comments: Vec<PluginAstComment>,
}

impl PluginAstParseResult {
    /// Serialize the compact Rust-to-JavaScript bridge payload.
    pub fn bridge_json(&self) -> Result<String, PluginAstParseError> {
        serde_json::to_string(&PluginAstBridgePayload {
            ast: &self.ast,
            comments: &self.comments,
        })
        .map_err(Into::into)
    }

    pub(crate) fn into_ast(self) -> Value {
        self.ast
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PluginAstBridgePayload<'a> {
    ast: &'a Value,
    comments: &'a [PluginAstComment],
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PluginAstComment {
    #[serde(rename = "type")]
    kind: &'static str,
    value: String,
    start: u32,
    end: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    loc: Option<PluginSourceLocation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    range: Option<[u32; 2]>,
}

#[derive(Debug, Clone, Serialize)]
struct PluginSourceLocation {
    start: PluginSourcePosition,
    end: PluginSourcePosition,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
struct PluginSourcePosition {
    line: u32,
    column: u32,
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
        self.options.validate(self.source)?;
        let source_type = self.options.source_type()?;
        let allocator = Allocator::default();
        let mut parsed = Parser::new(&allocator, self.source, source_type)
            .with_options(ParseOptions {
                allow_return_outside_function: self.options.allow_return_outside_function,
                preserve_parens: self.options.preserve_parens,
                enable_ident_hashes: false,
                ..ParseOptions::default()
            })
            .parse();

        if parsed.panicked || !parsed.diagnostics.is_empty() {
            let (message, byte_offset) = parsed.diagnostics.first().map_or_else(
                || ("OXC parser stopped unexpectedly".to_string(), 0),
                |diagnostic| {
                    let offset = diagnostic
                        .labels
                        .first()
                        .map_or(0, |label| label.offset() as u32);
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

        let line_index = PluginLineIndex::new(self.source);
        let comments = self.collect_comments(&parsed.program, &line_index);
        Utf8ToUtf16::new(self.source).convert_program(&mut parsed.program);

        let json = parsed
            .program
            .to_estree_json_with_fixes(false, self.options.ranges);
        let mut ast: Value = serde_json::from_str(&json)?;
        let node = ast
            .get_mut("node")
            .ok_or_else(|| PluginAstParseError::Options {
                message: "OXC ESTree envelope is missing node".to_string(),
            })?;
        PluginAstNormalizer::new(&self.options, &line_index).normalize(node);

        Ok(PluginAstParseResult { ast, comments })
    }

    fn collect_comments(
        &self,
        program: &oxc_ast::ast::Program<'_>,
        line_index: &PluginLineIndex,
    ) -> Vec<PluginAstComment> {
        if !self.options.capture_comments {
            return Vec::new();
        }
        let span_converter = Utf8ToUtf16::new(self.source);
        let mut offset_converter = span_converter.converter();
        program
            .comments
            .iter()
            .map(|comment| {
                let content_span = comment.content_span();
                let value = content_span.source_text(self.source).to_string();
                let mut span = comment.span;
                if let Some(converter) = offset_converter.as_mut() {
                    converter.convert_span(&mut span);
                }
                let loc = self.options.locations.then(|| PluginSourceLocation {
                    start: line_index.position(span.start),
                    end: line_index.position(span.end),
                    source: self.options.source_file.clone(),
                });
                PluginAstComment {
                    kind: match comment.kind {
                        CommentKind::Line => "Line",
                        CommentKind::SingleLineBlock | CommentKind::MultiLineBlock => "Block",
                    },
                    value,
                    start: span.start,
                    end: span.end,
                    loc,
                    range: self.options.ranges.then_some([span.start, span.end]),
                }
            })
            .collect()
    }
}

struct PluginAstNormalizer<'a> {
    options: &'a PluginAstParseOptions,
    line_index: &'a PluginLineIndex,
}

impl<'a> PluginAstNormalizer<'a> {
    fn new(options: &'a PluginAstParseOptions, line_index: &'a PluginLineIndex) -> Self {
        Self {
            options,
            line_index,
        }
    }

    fn normalize(&self, value: &mut Value) {
        match value {
            Value::Array(values) => {
                for value in values {
                    self.normalize(value);
                }
            }
            Value::Object(object) => self.normalize_object(object),
            _ => {}
        }
    }

    fn normalize_object(&self, object: &mut Map<String, Value>) {
        let is_program = object.get("type").and_then(Value::as_str) == Some("Program");
        if is_program {
            // OXC exposes hashbang separately; Acorn/Rollup's Program does not.
            object.remove("hashbang");
        }

        let span = object
            .get("type")
            .and_then(Value::as_str)
            .zip(object.get("start").and_then(Value::as_u64))
            .zip(object.get("end").and_then(Value::as_u64))
            .map(|((_, start), end)| (start as u32, end as u32));

        for value in object.values_mut() {
            self.normalize(value);
        }

        let Some((start, end)) = span else {
            return;
        };
        if self.options.locations {
            object.insert(
                "loc".to_string(),
                serde_json::to_value(PluginSourceLocation {
                    start: self.line_index.position(start),
                    end: self.line_index.position(end),
                    source: self.options.source_file.clone(),
                })
                .expect("source locations are infallible to serialize"),
            );
        }
        if let Some(source_file) = self.options.direct_source_file.as_ref() {
            object.insert("sourceFile".to_string(), Value::String(source_file.clone()));
        }
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
