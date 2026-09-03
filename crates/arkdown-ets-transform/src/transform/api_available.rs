//! `deviceInfo.apiAvailable` compatibility lowering.
//!
//! Mirrors `compiler/src/process_available_statement.ts` from the reference
//! compiler. The helper remains source-generated there, so this pass parses
//! the same fixed helper shape into OXC AST rather than assembling JavaScript
//! with ad-hoc output strings.

use oxc_allocator::{Allocator, ArenaBox, CloneIn, GetAllocator, Vec as OxcVec};
use oxc_ast::ast::{
    Argument, ArrowFunctionBody, BindingIdentifier, BindingPattern, CallExpression, Expression,
    FormalParameter, FormalParameterKind, FormalParameters, IdentifierName, IdentifierReference,
    ImportDeclarationSpecifier, ModuleExportName, Program, Statement,
};
use oxc_ast::builder::AstBuilder;
use oxc_ast_visit::{walk, walk_mut, Visit, VisitMut};
use oxc_parser::Parser;
use oxc_span::{SourceType, SPAN};
use oxc_syntax::operator::UnaryOperator;
use rustc_hash::FxHashSet;

use crate::error::{DiagnosticSeverity, EtsTransformError};

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

/// Validates calls to the SDK `deviceInfo.apiAvailable` binding before the
/// compatibility lowerer rewrites them.
///
/// The source checker resolves the callee through TypeScript's type checker to
/// `@ohos.deviceInfo.d.ts::apiAvailable`. Arkdown's transform snapshot has
/// already resolved imports, so this pass records only bindings imported from
/// that SDK module (including the BasicServicesKit re-export) and leaves user
/// objects with an `apiAvailable` member alone.
pub(crate) struct ApiAvailableValidator<'a> {
    source_id: &'a str,
    options: ApiAvailableTransformOptions,
}

impl<'a> ApiAvailableValidator<'a> {
    pub(crate) fn new(source_id: &'a str, options: ApiAvailableTransformOptions) -> Self {
        Self { source_id, options }
    }

    pub(crate) fn validate(&self, program: &Program<'_>) -> Result<(), EtsTransformError> {
        match self.diagnostics(program).into_iter().next() {
            Some(diagnostic) => Err(diagnostic),
            None => Ok(()),
        }
    }

    pub(crate) fn diagnostics(&self, program: &Program<'_>) -> Vec<EtsTransformError> {
        if !self.options.enabled {
            return Vec::new();
        }
        let bindings = ApiAvailableSdkBindings::from_program(program);
        if bindings.is_empty() {
            return Vec::new();
        }
        let mut visitor = ApiAvailableCallValidator {
            bindings: &bindings,
            source_id: self.source_id,
            open_harmony_runtime: self.options.open_harmony_runtime,
            issues: Vec::new(),
        };
        visitor.visit_program(program);
        visitor
            .issues
            .into_iter()
            .map(|issue| EtsTransformError::CodedDiagnostic {
                file: self.source_id.to_string(),
                code: issue.code.to_string(),
                message: issue.message,
                severity: DiagnosticSeverity::Error,
                span: Some(issue.span),
                solutions: issue.solutions,
            })
            .collect()
    }
}

#[derive(Default)]
struct ApiAvailableSdkBindings {
    names: FxHashSet<String>,
}

impl ApiAvailableSdkBindings {
    fn from_program(program: &Program<'_>) -> Self {
        let mut bindings = Self::default();
        for statement in &program.body {
            let Statement::ImportDeclaration(import) = statement else {
                continue;
            };
            let source = import.source.value.as_str();
            let Some(specifiers) = import.specifiers.as_ref() else {
                continue;
            };
            if matches!(source, "@ohos.deviceInfo" | "@ohos:deviceInfo") {
                for specifier in specifiers {
                    match specifier {
                        ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                            bindings.names.insert(specifier.local.name.to_string());
                        }
                        ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                            bindings.names.insert(specifier.local.name.to_string());
                        }
                        ImportDeclarationSpecifier::ImportSpecifier(specifier)
                            if ModuleExportNameText::new(&specifier.imported).as_str()
                                == "default" =>
                        {
                            bindings.names.insert(specifier.local.name.to_string());
                        }
                        _ => {}
                    }
                }
            } else if source == "@kit.BasicServicesKit" {
                for specifier in specifiers {
                    let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else {
                        continue;
                    };
                    if ModuleExportNameText::new(&specifier.imported).as_str() == "deviceInfo" {
                        bindings.names.insert(specifier.local.name.to_string());
                    }
                }
            }
        }
        bindings
    }

    fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    fn contains(&self, name: &str) -> bool {
        self.names.contains(name)
    }
}

struct ModuleExportNameText<'a> {
    name: &'a ModuleExportName<'a>,
}

impl<'a> ModuleExportNameText<'a> {
    fn new(name: &'a ModuleExportName<'a>) -> Self {
        Self { name }
    }

    fn as_str(&self) -> &str {
        match self.name {
            ModuleExportName::IdentifierName(identifier) => identifier.name.as_str(),
            ModuleExportName::IdentifierReference(identifier) => identifier.name.as_str(),
            ModuleExportName::StringLiteral(literal) => literal.value.as_str(),
        }
    }
}

struct ApiAvailableCallValidator<'a> {
    bindings: &'a ApiAvailableSdkBindings,
    source_id: &'a str,
    open_harmony_runtime: bool,
    issues: Vec<ApiAvailableIssue>,
}

impl<'a> ApiAvailableCallValidator<'a> {
    const BASE_ERROR: &'static str = "Invalid parameters for apiAvailable.";
    const NULLISH_ERROR: &'static str = "Null and undefined are not allowed for parameters.";
    const NUMBER_FORMAT_ERROR: &'static str = "Only decimal digits are allowed.";
    const OPEN_HARMONY_FORMAT_ERROR: &'static str = "Only digits and dots are allowed.";
    const DISTRIBUTION_FORMAT_ERROR: &'static str =
        "Only digits, dots, and left and right parentheses are allowed.";
    const OPEN_HARMONY_CONTENT_ERROR: &'static str = "The api version must be a decimal integer between 1 and 25. The M.S.F format must meet the following requirements: The value must be in the three decimal format, M must be greater than or equal to 26, and S and F must be decimal integers between 0 and 99.";
    const DISTRIBUTION_CONTENT_ERROR: &'static str = "The api version must be a decimal integer between 1 and 25. The M.S.F format must meet the following requirements: The value must be in the three decimal format, M must be decimal integers between 1 and 99, and S and F must be decimal integers between 0 and 99.";
    const TS_FILE_ERROR: &'static str =
        "apiAvailable cannot be used in .ts files. Please use .ets files instead.";

    fn validate_call(&self, call: &CallExpression<'_>) -> Option<ApiAvailableIssue> {
        let receiver = ApiAvailableCallee::new(&call.callee).receiver_identifier()?;
        if !self.bindings.contains(receiver) {
            return None;
        }
        if std::path::Path::new(self.source_id)
            .extension()
            .is_some_and(|extension| extension == "ts")
        {
            return Some(ApiAvailableIssue::new(
                "11706015",
                Self::TS_FILE_ERROR,
                call.span,
                ["Use .ets files instead of .ts files for apiAvailable calls."],
            ));
        }
        if call.arguments.len() != 1 {
            return Some(self.issue(call.span, None));
        }
        let argument = call.arguments.first()?.as_expression()?;
        self.validate_argument(argument)
            .map(|suffix| self.issue(call.span, Some(suffix)))
    }

    fn validate_argument(&self, argument: &Expression<'_>) -> Option<&'static str> {
        match argument {
            Expression::NullLiteral(_) => Some(Self::NULLISH_ERROR),
            Expression::Identifier(identifier) if identifier.name == "undefined" => {
                Some(Self::NULLISH_ERROR)
            }
            Expression::NumericLiteral(number) => self.validate_number(
                number
                    .raw
                    .as_ref()
                    .map_or_else(|| number.value.to_string(), |raw| raw.as_str().to_string()),
                number.value,
            ),
            Expression::UnaryExpression(unary) if unary.operator.is_arithmetic() => {
                let Expression::NumericLiteral(number) = &unary.argument else {
                    return None;
                };
                let sign = if unary.operator == UnaryOperator::UnaryNegation {
                    "-"
                } else {
                    "+"
                };
                let raw = number
                    .raw
                    .as_ref()
                    .map_or_else(|| number.value.to_string(), |raw| raw.as_str().to_string());
                let value = if sign == "-" {
                    -number.value
                } else {
                    number.value
                };
                self.validate_number(format!("{sign}{raw}"), value)
            }
            Expression::StringLiteral(string) => self.validate_string(string.value.as_str()),
            Expression::TemplateLiteral(template) if template.expressions.is_empty() => template
                .quasis
                .first()
                .map(|quasi| {
                    quasi
                        .value
                        .cooked
                        .as_ref()
                        .unwrap_or(&quasi.value.raw)
                        .as_str()
                })
                .and_then(|value| self.validate_string(value)),
            _ => None,
        }
    }

    fn validate_number(&self, raw: String, value: f64) -> Option<&'static str> {
        if !CanonicalDecimalInteger::new(&raw).is_valid() {
            return Some(Self::NUMBER_FORMAT_ERROR);
        }
        if !(1.0..26.0).contains(&value) {
            return Some(self.content_error());
        }
        None
    }

    fn validate_string(&self, value: &str) -> Option<&'static str> {
        if self.open_harmony_runtime {
            if !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || byte == b'.')
            {
                return Some(Self::OPEN_HARMONY_FORMAT_ERROR);
            }
            if !MsfVersion::new(value).is_valid_open_harmony() {
                return Some(Self::OPEN_HARMONY_CONTENT_ERROR);
            }
        } else {
            if !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'.' | b'(' | b')'))
            {
                return Some(Self::DISTRIBUTION_FORMAT_ERROR);
            }
            if !MsfVersion::new(value).is_valid_distribution() {
                return Some(Self::DISTRIBUTION_CONTENT_ERROR);
            }
        }
        None
    }

    fn content_error(&self) -> &'static str {
        if self.open_harmony_runtime {
            Self::OPEN_HARMONY_CONTENT_ERROR
        } else {
            Self::DISTRIBUTION_CONTENT_ERROR
        }
    }

    fn issue(&self, span: oxc_span::Span, suffix: Option<&str>) -> ApiAvailableIssue {
        ApiAvailableIssue::new(
            "11706013",
            format!("{}{}", Self::BASE_ERROR, suffix.unwrap_or_default()),
            span,
            ["Check the official API reference documentation, and switch to the supported interfaces."],
        )
    }
}

struct ApiAvailableIssue {
    code: &'static str,
    message: String,
    span: oxc_span::Span,
    solutions: Vec<String>,
}

impl ApiAvailableIssue {
    fn new<I, S>(
        code: &'static str,
        message: impl Into<String>,
        span: oxc_span::Span,
        solutions: I,
    ) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            code,
            message: message.into(),
            span,
            solutions: solutions.into_iter().map(Into::into).collect(),
        }
    }
}

impl<'program> Visit<'program> for ApiAvailableCallValidator<'_> {
    fn visit_call_expression(&mut self, call: &CallExpression<'program>) {
        if let Some(issue) = self.validate_call(call) {
            self.issues.push(issue);
        }
        walk::walk_call_expression(self, call);
    }
}

struct ApiAvailableCallee<'a> {
    expression: &'a Expression<'a>,
}

impl<'a> ApiAvailableCallee<'a> {
    fn new(expression: &'a Expression<'a>) -> Self {
        Self { expression }
    }

    fn receiver_identifier(&self) -> Option<&'a str> {
        let receiver = match self.unwrap(self.expression) {
            Expression::StaticMemberExpression(member)
                if member.property.name == "apiAvailable" =>
            {
                self.unwrap(&member.object)
            }
            Expression::ComputedMemberExpression(member) if matches!(&member.expression, Expression::StringLiteral(value) if value.value == "apiAvailable") => {
                self.unwrap(&member.object)
            }
            _ => return None,
        };
        match receiver {
            Expression::Identifier(identifier) => Some(identifier.name.as_str()),
            _ => None,
        }
    }

    fn unwrap<'expression>(
        &self,
        expression: &'expression Expression<'a>,
    ) -> &'expression Expression<'a> {
        match expression {
            Expression::TSNonNullExpression(expression) => self.unwrap(&expression.expression),
            Expression::ParenthesizedExpression(expression) => self.unwrap(&expression.expression),
            Expression::TSAsExpression(expression) => self.unwrap(&expression.expression),
            Expression::TSTypeAssertion(expression) => self.unwrap(&expression.expression),
            _ => expression,
        }
    }
}

struct CanonicalDecimalInteger<'a> {
    value: &'a str,
}

impl<'a> CanonicalDecimalInteger<'a> {
    fn new(value: &'a str) -> Self {
        Self { value }
    }

    fn is_valid(&self) -> bool {
        let digits = self.value.strip_prefix(['+', '-']).unwrap_or(self.value);
        !digits.is_empty()
            && digits.bytes().all(|byte| byte.is_ascii_digit())
            && (digits == "0" || !digits.starts_with('0'))
    }
}

struct MsfVersion<'a> {
    value: &'a str,
}

impl<'a> MsfVersion<'a> {
    fn new(value: &'a str) -> Self {
        Self { value }
    }

    fn is_valid_open_harmony(&self) -> bool {
        self.parts()
            .is_some_and(|parts| parts.major >= 26 && parts.build.is_none())
    }

    fn is_valid_distribution(&self) -> bool {
        self.parts()
            .is_some_and(|parts| parts.major < 26 || parts.build.is_none())
    }

    fn parts(&self) -> Option<MsfVersionParts<'_>> {
        let (version, build) = match self.value.split_once('(') {
            Some((version, build)) => {
                let build = build.strip_suffix(')')?;
                if build.is_empty()
                    || build.len() > 2
                    || !build.bytes().all(|byte| byte.is_ascii_digit())
                {
                    return None;
                }
                (version, Some(build))
            }
            None => (self.value, None),
        };
        let mut components = version.split('.');
        let major = Self::component(components.next()?, false)?;
        let minor = Self::component(components.next()?, true)?;
        let patch = Self::component(components.next()?, true)?;
        if components.next().is_some() {
            return None;
        }
        Some(MsfVersionParts {
            major,
            _minor: minor,
            _patch: patch,
            build,
        })
    }

    fn component(value: &str, allow_zero: bool) -> Option<u32> {
        if value.is_empty()
            || value.len() > 2
            || !value.bytes().all(|byte| byte.is_ascii_digit())
            || (value.len() > 1 && value.starts_with('0'))
        {
            return None;
        }
        let value = value.parse::<u32>().ok()?;
        (allow_zero || value > 0).then_some(value)
    }
}

struct MsfVersionParts<'a> {
    major: u32,
    _minor: u32,
    _patch: u32,
    build: Option<&'a str>,
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
