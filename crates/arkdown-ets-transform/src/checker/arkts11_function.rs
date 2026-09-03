use oxc_ast::ast::{Decorator, Expression, Function, Statement};

use crate::error::{DiagnosticSeverity, EtsTransformError};
use crate::transform::component_meta::ComponentMetadata;
use crate::transform::decorator::{ArkuiDecorator, ArkuiDecoratorName};

/// Checks source-level ArkTS 1.1 function decorator contracts from
/// `validate_ui_syntax.ts::validateDifferentFunction` and
/// `process_ui_syntax.ts`.
pub(super) struct Arkts11FunctionValidator<'function, 'context, 'errors> {
    function: &'function Function<'function>,
    file: &'errors str,
    errors: &'errors mut Vec<EtsTransformError>,
    component_metadata: Option<&'context ComponentMetadata>,
}

impl<'function, 'context, 'errors> Arkts11FunctionValidator<'function, 'context, 'errors> {
    pub(super) fn new(
        function: &'function Function<'function>,
        file: &'errors str,
        errors: &'errors mut Vec<EtsTransformError>,
    ) -> Self {
        Self {
            function,
            file,
            errors,
            component_metadata: None,
        }
    }

    pub(super) fn with_component_metadata(
        mut self,
        metadata: Option<&'context ComponentMetadata>,
    ) -> Self {
        self.component_metadata = metadata;
        self
    }

    pub(super) fn check(self) {
        self.check_with_extra_decorators(&[]);
    }

    pub(super) fn check_with_extra_decorators(mut self, extra_decorators: &[Decorator<'_>]) {
        let all_decorators = self
            .function
            .decorators
            .iter()
            .chain(extra_decorators)
            .collect::<Vec<_>>();
        let decorators = all_decorators
            .iter()
            .map(|decorator| ArkuiDecorator::new(decorator).name())
            .collect::<Vec<_>>();

        let distinct = decorators
            .iter()
            .copied()
            .collect::<rustc_hash::FxHashSet<_>>();
        if distinct.len() > 1 || distinct.contains(ArkuiDecoratorName::LOCAL_BUILDER) {
            self.errors.push(EtsTransformError::CodedDiagnostic {
                file: self.file.to_string(),
                code: "10905117".to_string(),
                message: "A function can only be decorated by one of the '@AnimatableExtend', '@Builder', '@Extend', '@Styles', '@Concurrent' and '@Sendable'.".to_string(),
                severity: DiagnosticSeverity::Error,
                span: Some(self.function.span),
                solutions: Vec::new(),
            });
        }

        for decorator in distinct {
            let count = decorators
                .iter()
                .filter(|candidate| **candidate == decorator)
                .count();
            if count > 1 {
                self.errors.push(EtsTransformError::Diagnostic {
                    file: self.file.to_string(),
                    message: format!(
                        "Duplicate '@{decorator}' decorators for function are not allowed."
                    ),
                    severity: DiagnosticSeverity::Warning,
                });
            }
        }

        if decorators.contains(&ArkuiDecoratorName::STYLES)
            && (!self.function.params.items.is_empty() || self.function.params.rest.is_some())
        {
            self.errors.push(EtsTransformError::CodedDiagnostic {
                file: self.file.to_string(),
                code: "10905110".to_string(),
                message: "'@Styles' decorated functions and methods cannot have arguments."
                    .to_string(),
                severity: DiagnosticSeverity::Error,
                span: Some(self.function.span),
                solutions: Vec::new(),
            });
        }

        if decorators.contains(&ArkuiDecoratorName::CONCURRENT) && self.function.generator {
            let function_kind = if self.function.r#async {
                "Async generator"
            } else {
                "Generator"
            };
            self.errors.push(EtsTransformError::CodedDiagnostic {
                file: self.file.to_string(),
                code: "10905122".to_string(),
                message: format!(
                    "'@Concurrent' can not be used on '{function_kind}' function declaration."
                ),
                severity: DiagnosticSeverity::Error,
                span: Some(self.function.span),
                solutions: Vec::new(),
            });
        }

        for decorator in all_decorators {
            let name = ArkuiDecorator::new(decorator).name();
            if !matches!(
                name,
                ArkuiDecoratorName::EXTEND | ArkuiDecoratorName::ANIMATABLE_EXTEND
            ) {
                continue;
            }
            let Expression::CallExpression(call) = &decorator.expression else {
                continue;
            };
            if call.arguments.len() != 1 {
                self.errors.push(EtsTransformError::CodedDiagnostic {
                    file: self.file.to_string(),
                    code: "10905108".to_string(),
                    message: format!("'@{name}' should have one and only one parameter."),
                    severity: DiagnosticSeverity::Error,
                    span: Some(call.span),
                    solutions: Vec::new(),
                });
                continue;
            }
            if name == ArkuiDecoratorName::EXTEND {
                self.check_extend_name_collision(decorator, call);
            }
        }
    }

    fn check_extend_name_collision(
        &mut self,
        decorator: &Decorator<'_>,
        call: &oxc_ast::ast::CallExpression<'_>,
    ) {
        let Some(function_name) = self.function.id.as_ref().map(|id| id.name.as_str()) else {
            return;
        };
        let Some(Expression::Identifier(component)) = call
            .arguments
            .first()
            .and_then(|argument| argument.as_expression())
        else {
            return;
        };
        if !self.component_metadata.is_some_and(|metadata| {
            metadata.is_component_style_name(component.name.as_str(), function_name)
        }) {
            return;
        }
        let message = format!(
            "The '@Extend' function cannot have the same name as the built-in style attribute '{function_name}' of the component '{}'.",
            component.name
        );
        let self_call = self
            .function
            .body
            .as_ref()
            .and_then(|body| body.statements.first())
            .and_then(|statement| match statement {
                Statement::ExpressionStatement(statement) => Some(&statement.expression),
                _ => None,
            })
            .and_then(|expression| match expression {
                Expression::LeadingDotExpression(expression) => Some(&expression.expression),
                _ => None,
            })
            .and_then(Self::outer_call_name)
            == Some(function_name);
        if self_call {
            self.errors.push(EtsTransformError::CodedDiagnostic {
                file: self.file.to_string(),
                code: "10905360".to_string(),
                message,
                severity: DiagnosticSeverity::Error,
                span: Some(decorator.span),
                solutions: Vec::new(),
            });
        } else {
            self.errors.push(EtsTransformError::Diagnostic {
                file: self.file.to_string(),
                message,
                severity: DiagnosticSeverity::Warning,
            });
        }
    }

    fn outer_call_name<'node>(expression: &'node Expression<'_>) -> Option<&'node str> {
        let Expression::CallExpression(call) = expression else {
            return None;
        };
        match &call.callee {
            Expression::Identifier(identifier) => Some(identifier.name.as_str()),
            Expression::StaticMemberExpression(member) => Some(member.property.name.as_str()),
            _ => None,
        }
    }
}
