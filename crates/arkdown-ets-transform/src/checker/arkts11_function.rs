use oxc_ast::ast::{Decorator, Expression, Function};

use crate::error::{DiagnosticSeverity, EtsTransformError};
use crate::transform::decorator::{ArkuiDecorator, ArkuiDecoratorName};

/// Checks source-level ArkTS 1.1 function decorator contracts from
/// `validate_ui_syntax.ts::validateDifferentFunction` and
/// `process_ui_syntax.ts`.
pub(super) struct Arkts11FunctionValidator<'function, 'errors> {
    function: &'function Function<'function>,
    file: &'errors str,
    errors: &'errors mut Vec<EtsTransformError>,
}

impl<'function, 'errors> Arkts11FunctionValidator<'function, 'errors> {
    pub(super) fn new(
        function: &'function Function<'function>,
        file: &'errors str,
        errors: &'errors mut Vec<EtsTransformError>,
    ) -> Self {
        Self {
            function,
            file,
            errors,
        }
    }

    pub(super) fn check(self) {
        self.check_with_extra_decorators(&[]);
    }

    pub(super) fn check_with_extra_decorators(self, extra_decorators: &[Decorator<'_>]) {
        let decorators = self
            .function
            .decorators
            .iter()
            .chain(extra_decorators)
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

        for decorator in &self.function.decorators {
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
            }
        }
    }
}
