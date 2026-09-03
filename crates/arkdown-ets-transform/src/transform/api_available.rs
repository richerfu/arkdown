//! `deviceInfo.apiAvailable` compatibility lowering.
//!
//! Mirrors `compiler/src/process_available_statement.ts` from the reference
//! compiler. The helper remains source-generated there, so this pass parses
//! the same fixed helper shape into OXC AST rather than assembling JavaScript
//! with ad-hoc output strings.

use oxc_allocator::{Allocator, ArenaBox, CloneIn, GetAllocator, Vec as OxcVec};
use oxc_ast::ast::{
    Argument, ArrowFunctionBody, BindingIdentifier, BindingPattern, Expression, FormalParameter,
    FormalParameterKind, FormalParameters, IdentifierName, IdentifierReference, Program,
};
use oxc_ast::builder::AstBuilder;
use oxc_ast_visit::{walk_mut, Visit, VisitMut};
use oxc_parser::Parser;
use oxc_span::{SourceType, SPAN};
use oxc_syntax::operator::UnaryOperator;

#[derive(Debug, Clone, Copy)]
pub(crate) struct ApiAvailableTransformOptions {
    enabled: bool,
    open_harmony_runtime: bool,
}

impl Default for ApiAvailableTransformOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            open_harmony_runtime: true,
        }
    }
}

impl ApiAvailableTransformOptions {
    pub(crate) fn from_build_context(compatible_sdk_version: &str, runtime_os: &str) -> Self {
        Self {
            enabled: !CompatibleSdkVersion::new(compatible_sdk_version).is_point_version(),
            open_harmony_runtime: !arkdown_core::is_harmony_os(runtime_os),
        }
    }

    pub(crate) fn should_transform_source(self, source: &str) -> bool {
        self.enabled && source.contains(".apiAvailable")
    }
}

struct CompatibleSdkVersion<'a> {
    value: &'a str,
}

impl<'a> CompatibleSdkVersion<'a> {
    fn new(value: &'a str) -> Self {
        Self { value }
    }

    /// Equivalent to `isPointVersion` in the reference compiler.
    fn is_point_version(&self) -> bool {
        let value = self
            .value
            .strip_prefix('\'')
            .unwrap_or(self.value)
            .strip_suffix('\'')
            .unwrap_or_else(|| self.value.strip_prefix('\'').unwrap_or(self.value));
        let (version, build) = match value.split_once('(') {
            Some((version, build)) => {
                let Some(build) = build.strip_suffix(')') else {
                    return false;
                };
                if build.is_empty() || !build.bytes().all(|byte| byte.is_ascii_digit()) {
                    return false;
                }
                (version, Some(build))
            }
            None => (value, None),
        };
        let mut components = version.split('.');
        let Some(major) = components.next().and_then(Self::decimal_component) else {
            return false;
        };
        let Some(minor) = components.next().and_then(Self::decimal_component) else {
            return false;
        };
        let Some(patch) = components.next().and_then(Self::decimal_component) else {
            return false;
        };
        let _ = build;
        components.next().is_none() && (26..=99).contains(&major) && minor <= 99 && patch <= 99
    }

    fn decimal_component(value: &str) -> Option<u32> {
        if value.is_empty()
            || value.len() > 2
            || (value.len() > 1 && value.starts_with('0'))
            || !value.bytes().all(|byte| byte.is_ascii_digit())
        {
            return None;
        }
        value.parse().ok()
    }
}

pub(crate) struct ApiAvailableLowerer<'a> {
    allocator: &'a Allocator,
    options: ApiAvailableTransformOptions,
}

impl<'a> ApiAvailableLowerer<'a> {
    pub(crate) fn new(allocator: &'a Allocator, options: ApiAvailableTransformOptions) -> Self {
        Self { allocator, options }
    }

    pub(crate) fn program(&self, program: &mut Program<'a>) -> bool {
        if !self.options.enabled {
            return false;
        }
        let mut mutator = ApiAvailableMutator {
            allocator: self.allocator,
            suppress_reference: false,
            changed: false,
        };
        mutator.visit_program(program);
        if !mutator.changed {
            return false;
        }
        if let Some(helper) = self.helper_statement() {
            program.body.push(helper);
        }
        true
    }

    fn helper_statement(&self) -> Option<oxc_ast::ast::Statement<'a>> {
        let source = if self.options.open_harmony_runtime {
            OPEN_HARMONY_HELPER
        } else {
            HARMONY_HELPER
        };
        let mut parsed =
            Parser::new(self.allocator, source, SourceType::ts().with_module(true)).parse();
        if parsed.panicked || !parsed.diagnostics.is_empty() {
            return None;
        }
        GeneratedSpanClearer.visit_program(&mut parsed.program);
        parsed.program.body.into_iter().next()
    }
}

struct GeneratedSpanClearer;

impl<'a> VisitMut<'a> for GeneratedSpanClearer {
    fn visit_span(&mut self, span: &mut oxc_span::Span) {
        *span = SPAN;
    }
}

struct ApiAvailableMutator<'a> {
    allocator: &'a Allocator,
    suppress_reference: bool,
    changed: bool,
}

impl<'a> VisitMut<'a> for ApiAvailableMutator<'a> {
    fn visit_expression(&mut self, expression: &mut Expression<'a>) {
        if let Expression::UnaryExpression(unary) = expression {
            if unary.operator == UnaryOperator::Typeof {
                let previous = self.suppress_reference;
                self.suppress_reference = true;
                self.visit_expression(&mut unary.argument);
                self.suppress_reference = previous;
                return;
            }
        }

        if let Some(replacement) = self.call_replacement(expression) {
            *expression = replacement;
            self.changed = true;
            walk_mut::walk_expression(self, expression);
            return;
        }

        if !self.suppress_reference {
            if let Some(replacement) = self.reference_replacement(expression) {
                *expression = replacement;
                self.changed = true;
                walk_mut::walk_expression(self, expression);
                return;
            }
        }

        walk_mut::walk_expression(self, expression);
    }
}

impl<'a> ApiAvailableMutator<'a> {
    fn call_replacement(&self, expression: &Expression<'a>) -> Option<Expression<'a>> {
        let Expression::CallExpression(call) = expression else {
            return None;
        };
        let receiver = self.receiver(&call.callee)?;
        let ast = AstBuilder::new(self.allocator);
        let mut arguments = OxcVec::with_capacity_in(call.arguments.len() + 1, &ast);
        arguments.push(Argument::from(receiver));
        arguments.extend(
            call.arguments
                .iter()
                .map(|argument| argument.clone_in(self.allocator)),
        );
        Some(Expression::new_call_expression(
            SPAN,
            Expression::new_identifier(SPAN, "__mockApiAvailable", &ast),
            None,
            arguments,
            false,
            &ast,
        ))
    }

    fn reference_replacement(&self, expression: &Expression<'a>) -> Option<Expression<'a>> {
        let receiver = self.direct_receiver(expression)?;
        let ast = AstBuilder::new(self.allocator);
        let parameter_name = ApiAvailableParameterName::new(&receiver).into_name();
        let parameter =
            Expression::new_identifier(SPAN, ast.allocator().alloc_str(&parameter_name), &ast);
        let call = Expression::new_call_expression(
            SPAN,
            Expression::new_identifier(SPAN, "__mockApiAvailable", &ast),
            None,
            OxcVec::from_iter_in([Argument::from(receiver), Argument::from(parameter)], &ast),
            false,
            &ast,
        );
        let mut params = FormalParameters::new(
            SPAN,
            FormalParameterKind::ArrowFormalParameters,
            OxcVec::with_capacity_in(1, &ast),
            None,
            &ast,
        );
        params.items.push(FormalParameter::new(
            SPAN,
            OxcVec::new_in(&ast),
            BindingPattern::new_binding_identifier(
                SPAN,
                ast.allocator().alloc_str(&parameter_name),
                &ast,
            ),
            None,
            None,
            false,
            None,
            false,
            false,
            &ast,
        ));
        let arrow = Expression::new_arrow_function_expression(
            SPAN,
            false,
            None,
            ArenaBox::new_in(params, &ast),
            None,
            ArrowFunctionBody::from(call),
            &ast,
        );
        Some(Expression::new_parenthesized_expression(SPAN, arrow, &ast))
    }

    fn direct_receiver(&self, expression: &Expression<'a>) -> Option<Expression<'a>> {
        match expression {
            Expression::StaticMemberExpression(member)
                if member.property.name == "apiAvailable" =>
            {
                Some(member.object.clone_in(self.allocator))
            }
            Expression::ComputedMemberExpression(member)
                if matches!(
                    &member.expression,
                    Expression::StringLiteral(literal) if literal.value == "apiAvailable"
                ) =>
            {
                Some(member.object.clone_in(self.allocator))
            }
            _ => None,
        }
    }

    fn receiver(&self, expression: &Expression<'a>) -> Option<Expression<'a>> {
        if let Some(receiver) = self.direct_receiver(expression) {
            return Some(receiver);
        }
        let ast = AstBuilder::new(self.allocator);
        match expression {
            Expression::TSNonNullExpression(non_null) => self.receiver(&non_null.expression),
            Expression::ParenthesizedExpression(parenthesized) => {
                self.receiver(&parenthesized.expression).map(|receiver| {
                    Expression::new_parenthesized_expression(parenthesized.span, receiver, &ast)
                })
            }
            Expression::TSAsExpression(as_expression) => {
                self.receiver(&as_expression.expression).map(|receiver| {
                    Expression::new_ts_as_expression(
                        as_expression.span,
                        receiver,
                        as_expression.type_annotation.clone_in(self.allocator),
                        &ast,
                    )
                })
            }
            Expression::TSTypeAssertion(assertion) => {
                self.receiver(&assertion.expression).map(|receiver| {
                    Expression::new_ts_type_assertion(
                        assertion.span,
                        assertion.type_annotation.clone_in(self.allocator),
                        receiver,
                        &ast,
                    )
                })
            }
            _ => None,
        }
    }
}

struct ApiAvailableParameterName {
    name: String,
}

impl ApiAvailableParameterName {
    fn new(expression: &Expression<'_>) -> Self {
        let mut identifiers = ApiAvailableIdentifierCollector::default();
        identifiers.visit_expression(expression);
        if !identifiers.names.contains("v") {
            return Self {
                name: "v".to_string(),
            };
        }
        for suffix in 0usize.. {
            let candidate = format!("v{suffix}");
            if !identifiers.names.contains(candidate.as_str()) {
                return Self { name: candidate };
            }
        }
        unreachable!("an unbounded identifier suffix search must find a free name")
    }

    fn into_name(self) -> String {
        self.name
    }
}

#[derive(Default)]
struct ApiAvailableIdentifierCollector {
    names: rustc_hash::FxHashSet<String>,
}

impl<'a> Visit<'a> for ApiAvailableIdentifierCollector {
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        self.names.insert(identifier.name.to_string());
    }

    fn visit_identifier_name(&mut self, identifier: &IdentifierName<'a>) {
        self.names.insert(identifier.name.to_string());
    }

    fn visit_binding_identifier(&mut self, identifier: &BindingIdentifier<'a>) {
        self.names.insert(identifier.name.to_string());
    }
}

const OPEN_HARMONY_HELPER: &str = r#"function __mockApiAvailable(e, t) {
    var r, s;
    return (!!e && ('number' == typeof t ? !(t < 1 || 25 < t || !Number.isInteger(t)) && e.sdkApiVersion >= t
        : 'string' == typeof t && !!(t = t.match(/^(2[6-9]|[3-9]\d)\.(0|[1-9]\d{0,1})\.(0|[1-9]\d{0,1})$/)) && ((r = Number(t[1])), (s = Number(t[2])), (t = Number(t[3])), e.distributionOSApiVersion >= 1e4 * r + 100 * s + t)));
}"#;

const HARMONY_HELPER: &str = r#"function __mockApiAvailable(e, t) {
    var r, s, i;
    return (!!e && ('number' == typeof t ? !(t < 1 || 25 < t || !Number.isInteger(t)) && e.sdkApiVersion >= t
        : 'string' == typeof t && !!(t = t.match(/^([1-9]\d{0,1})\.(0|[1-9]\d{0,1})\.(0|[1-9]\d{0,1})(\(([1-9]\d{0,1})\))?$/)) && ((r = Number(t[1])), (s = Number(t[2])), (i = Number(t[3])), (r < 26 || !t[4])) && e.distributionOSApiVersion >= 1e4 * r + 100 * s + i));
}"#;
