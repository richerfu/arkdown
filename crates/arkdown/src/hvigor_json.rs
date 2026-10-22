//! OXC-backed evaluator for hvigor's public JSON5 utilities.
//!
//! DevEco Studio 6.1.1 / hvigor 6.24.3 parses JSON5-shaped files through a
//! TypeScript expression AST so it can optionally attach source locations.
//! Arkdown keeps the same restricted value grammar while replacing the parser
//! and evaluator with Rust/OXC. The bridge representation preserves values
//! JSON itself cannot encode (`Infinity`, `-Infinity`, and `NaN`).

use oxc_allocator::Allocator;
use oxc_ast::ast::{Expression, ObjectPropertyKind, PropertyKey, Statement};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType, Span};
use oxc_syntax::operator::UnaryOperator;
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum HvigorJsonParseError {
    #[error("JSON5: invalid end of input at 1:0")]
    Empty,
    #[error("JSON5: {message} at {line}:{column}")]
    Parse {
        message: String,
        line: u32,
        column: u32,
    },
    #[error("JSON5: invalid property '{snippet}'")]
    InvalidProperty { snippet: String },
    #[error("JSON5: unsupported property name '{snippet}'")]
    UnsupportedPropertyName { snippet: String },
    #[error("JSON5: invalid value '{snippet}'")]
    InvalidValue { snippet: String },
    #[error("JSON5: failed to serialize OXC value: {0}")]
    Serialize(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
struct HvigorJsonLocation {
    line: u32,
    column: u32,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum HvigorJsonBridgeValue {
    Null {
        #[serde(skip_serializing_if = "Option::is_none")]
        location: Option<HvigorJsonLocation>,
    },
    Boolean {
        value: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        location: Option<HvigorJsonLocation>,
    },
    Number {
        value: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        location: Option<HvigorJsonLocation>,
    },
    String {
        value: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        location: Option<HvigorJsonLocation>,
    },
    Array {
        values: Vec<HvigorJsonBridgeValue>,
        #[serde(skip_serializing_if = "Option::is_none")]
        location: Option<HvigorJsonLocation>,
    },
    Object {
        properties: Vec<HvigorJsonBridgeProperty>,
        #[serde(skip_serializing_if = "Option::is_none")]
        location: Option<HvigorJsonLocation>,
    },
}

impl HvigorJsonBridgeValue {
    fn numeric_value(&self) -> Option<f64> {
        let Self::Number { value, .. } = self else {
            return None;
        };
        match value.as_str() {
            "Infinity" => Some(f64::INFINITY),
            "-Infinity" => Some(f64::NEG_INFINITY),
            "NaN" | "-NaN" => Some(f64::NAN),
            value => value.parse().ok(),
        }
    }
}

#[derive(Debug, Serialize)]
struct HvigorJsonBridgeProperty {
    key: String,
    value: HvigorJsonBridgeValue,
}

/// Single-use OXC JSON5 expression evaluator.
pub struct HvigorJsonParser<'a> {
    source: &'a str,
    locations: bool,
}

impl<'a> HvigorJsonParser<'a> {
    pub fn new(source: &'a str, locations: bool) -> Self {
        Self { source, locations }
    }

    pub fn bridge_json(self) -> Result<String, HvigorJsonParseError> {
        if self.source.trim().is_empty() {
            return Err(HvigorJsonParseError::Empty);
        }
        let wrapped = format!("(\n{}\n);", self.source);
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &wrapped, SourceType::ts().with_module(true)).parse();
        let line_index = HvigorJsonLineIndex::new(&wrapped);
        if parsed.fatal_error || !parsed.diagnostics.is_empty() {
            let (message, offset) = parsed.diagnostics.first().map_or_else(
                || ("OXC parser stopped unexpectedly".to_string(), 0),
                |diagnostic| {
                    (
                        diagnostic.to_string(),
                        diagnostic.labels.first().map_or(0, |label| label.offset()),
                    )
                },
            );
            let location = line_index.at(offset);
            return Err(HvigorJsonParseError::Parse {
                message,
                line: location.line,
                column: location.column,
            });
        }
        let expression = match parsed.program.body.first() {
            Some(Statement::ExpressionStatement(statement)) => &statement.expression,
            _ => return Err(HvigorJsonParseError::Empty),
        };
        let evaluator = HvigorJsonEvaluator {
            source: &wrapped,
            locations: self.locations,
            line_index,
        };
        let value = evaluator.evaluate(expression)?;
        serde_json::to_string(&value).map_err(Into::into)
    }
}

struct HvigorJsonEvaluator<'a> {
    source: &'a str,
    locations: bool,
    line_index: HvigorJsonLineIndex<'a>,
}

impl<'a> HvigorJsonEvaluator<'a> {
    fn evaluate(
        &self,
        expression: &Expression<'_>,
    ) -> Result<HvigorJsonBridgeValue, HvigorJsonParseError> {
        let expression = self.unwrap_parentheses(expression);
        match expression {
            Expression::ObjectExpression(object) => {
                let mut properties = Vec::with_capacity(object.properties.len());
                for property in &object.properties {
                    let ObjectPropertyKind::ObjectProperty(property) = property else {
                        return Err(self.invalid_property(property.span()));
                    };
                    if property.computed || property.method || property.shorthand {
                        return Err(self.invalid_property(property.span));
                    }
                    let key = self.property_name(&property.key)?;
                    properties.push(HvigorJsonBridgeProperty {
                        key,
                        value: self.evaluate(&property.value)?,
                    });
                }
                Ok(HvigorJsonBridgeValue::Object {
                    properties,
                    location: self.container_location(object.span),
                })
            }
            Expression::ArrayExpression(array) => {
                let mut values = Vec::with_capacity(array.elements.len());
                for element in &array.elements {
                    let Some(expression) = element.as_expression() else {
                        return Err(self.invalid_value(element.span()));
                    };
                    values.push(self.evaluate(expression)?);
                }
                Ok(HvigorJsonBridgeValue::Array {
                    values,
                    location: self.container_location(array.span),
                })
            }
            Expression::StringLiteral(literal) => Ok(HvigorJsonBridgeValue::String {
                value: literal.value.as_str().unwrap_or_default().to_string(),
                location: self.primitive_location(literal.span),
            }),
            Expression::NumericLiteral(literal) => Ok(HvigorJsonBridgeValue::Number {
                value: Self::number_text(literal.value),
                location: self.primitive_location(literal.span),
            }),
            Expression::BooleanLiteral(literal) => Ok(HvigorJsonBridgeValue::Boolean {
                value: literal.value,
                location: self.primitive_location(literal.span),
            }),
            Expression::NullLiteral(literal) => Ok(HvigorJsonBridgeValue::Null {
                location: self.primitive_location(literal.span),
            }),
            Expression::Identifier(identifier) if identifier.name == "Infinity" => {
                Ok(HvigorJsonBridgeValue::Number {
                    value: "Infinity".to_string(),
                    location: self.primitive_location(identifier.span),
                })
            }
            Expression::Identifier(identifier) if identifier.name == "NaN" => {
                Ok(HvigorJsonBridgeValue::Number {
                    value: "NaN".to_string(),
                    location: self.primitive_location(identifier.span),
                })
            }
            Expression::UnaryExpression(unary)
                if matches!(
                    unary.operator,
                    UnaryOperator::UnaryPlus | UnaryOperator::UnaryNegation
                ) =>
            {
                let operand = self.evaluate(&unary.argument)?;
                let Some(mut value) = operand.numeric_value() else {
                    return Err(self.invalid_value(unary.span));
                };
                if unary.operator == UnaryOperator::UnaryNegation {
                    value = -value;
                }
                Ok(HvigorJsonBridgeValue::Number {
                    value: Self::number_text(value),
                    location: self.primitive_location(unary.span),
                })
            }
            _ => Err(self.invalid_value(expression.span())),
        }
    }

    fn unwrap_parentheses<'b>(&self, mut expression: &'b Expression<'b>) -> &'b Expression<'b> {
        while let Expression::ParenthesizedExpression(parenthesized) = expression {
            expression = &parenthesized.expression;
        }
        expression
    }

    fn property_name(&self, key: &PropertyKey<'_>) -> Result<String, HvigorJsonParseError> {
        match key {
            PropertyKey::StaticIdentifier(identifier) => Ok(identifier.name.to_string()),
            PropertyKey::StringLiteral(literal) => {
                Ok(literal.value.as_str().unwrap_or_default().to_string())
            }
            PropertyKey::NumericLiteral(literal) => Ok(Self::number_text(literal.value)),
            _ => Err(HvigorJsonParseError::UnsupportedPropertyName {
                snippet: self.source_text(key.span()),
            }),
        }
    }

    fn number_text(value: f64) -> String {
        if value.is_nan() {
            "NaN".to_string()
        } else if value == f64::INFINITY {
            "Infinity".to_string()
        } else if value == f64::NEG_INFINITY {
            "-Infinity".to_string()
        } else if value == 0.0 && value.is_sign_negative() {
            "-0".to_string()
        } else {
            value.to_string()
        }
    }

    fn container_location(&self, span: Span) -> Option<HvigorJsonLocation> {
        self.locations.then(|| {
            let mut location = self.line_index.at(span.start);
            location.column += 1;
            location
        })
    }

    fn primitive_location(&self, span: Span) -> Option<HvigorJsonLocation> {
        self.locations.then(|| self.line_index.at(span.end))
    }

    fn invalid_property(&self, span: Span) -> HvigorJsonParseError {
        HvigorJsonParseError::InvalidProperty {
            snippet: self.source_text(span),
        }
    }

    fn invalid_value(&self, span: Span) -> HvigorJsonParseError {
        HvigorJsonParseError::InvalidValue {
            snippet: self.source_text(span),
        }
    }

    fn source_text(&self, span: Span) -> String {
        span.source_text(self.source).to_string()
    }
}

struct HvigorJsonLineIndex<'a> {
    source: &'a str,
    line_starts: Vec<usize>,
}

impl<'a> HvigorJsonLineIndex<'a> {
    fn new(source: &'a str) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(
            source
                .bytes()
                .enumerate()
                .filter_map(|(index, byte)| (byte == b'\n').then_some(index + 1)),
        );
        Self {
            source,
            line_starts,
        }
    }

    fn at(&self, offset: u32) -> HvigorJsonLocation {
        let offset = (offset as usize).min(self.source.len());
        let line_index = self.line_starts.partition_point(|start| *start <= offset) - 1;
        let line_start = self.line_starts[line_index];
        let column = self.source[line_start..offset].encode_utf16().count() as u32;
        HvigorJsonLocation {
            // The wrapper adds one complete line, while TypeScript's source
            // API is zero-based. These cancel exactly for original JSON lines.
            line: (line_index as u32).max(1),
            column,
        }
    }
}
