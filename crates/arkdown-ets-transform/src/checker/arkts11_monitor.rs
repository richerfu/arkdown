use oxc_ast::ast::{Argument, Decorator, Expression, ObjectPropertyKind, PropertyKey};
use oxc_span::GetSpan;

use crate::error::{DiagnosticSeverity, EtsTransformError};
use crate::transform::decorator::ArkuiDecorator;
use rustc_hash::FxHashSet;

/// Checks the syntax-only monitor contracts from
/// `validate_ui_syntax.ts::checkMonitorDecorators`.
///
/// Root state-property existence and the complete argument-shape contract run
/// directly on the OXC AST. Nested property-chain existence remains project
/// type-analysis behavior.
pub(super) struct Arkts11MonitorValidator<'decorator, 'context, 'errors> {
    decorator: &'decorator Decorator<'decorator>,
    file: &'errors str,
    errors: &'errors mut Vec<EtsTransformError>,
    observable_roots: Option<&'context FxHashSet<String>>,
}

impl<'decorator, 'context, 'errors> Arkts11MonitorValidator<'decorator, 'context, 'errors> {
    pub(super) fn new(
        decorator: &'decorator Decorator<'decorator>,
        file: &'errors str,
        errors: &'errors mut Vec<EtsTransformError>,
    ) -> Self {
        Self {
            decorator,
            file,
            errors,
            observable_roots: None,
        }
    }

    pub(super) fn with_observable_roots(mut self, roots: &'context FxHashSet<String>) -> Self {
        self.observable_roots = Some(roots);
        self
    }

    pub(super) fn check(mut self) {
        let name = ArkuiDecorator::new(self.decorator).name();
        let is_monitor = name == "Monitor";
        let Expression::CallExpression(call) = &self.decorator.expression else {
            return;
        };
        if call.arguments.is_empty() {
            return;
        }

        let wildcard = if is_monitor {
            self.monitor_wildcard_mode(&call.arguments[0])
        } else {
            MonitorWildcardMode::Disabled
        };
        for (index, argument) in call.arguments.iter().enumerate() {
            if is_monitor && index == 0 && wildcard.consumes_options_argument() {
                continue;
            }
            let Argument::StringLiteral(literal) = argument else {
                let message = format!(
                    "Only constant expressions are supported as parameters in '@{name}'. Variables are not allowed."
                );
                if is_monitor {
                    self.warning(message);
                } else {
                    self.error("10905365", message, argument.span());
                }
                continue;
            };
            self.check_observable_root(name, literal.value.as_str(), literal.span);
            if (!is_monitor || (wildcard.is_enabled() && index > 0))
                && !MonitorWildcardPattern::new(literal.value.as_str()).is_valid()
            {
                self.error(
                    "10905367",
                    format!(
                        "In wildcard-based monitoring scenarios with '@{name}', the .* pattern must be placed at the end of the string."
                    ),
                    literal.span,
                );
            }
        }
    }

    fn check_observable_root(&mut self, name: &str, path: &str, span: oxc_span::Span) {
        let Some(roots) = self.observable_roots else {
            return;
        };
        let root = path.trim().split('.').next().unwrap_or_default();
        if !root.is_empty() && roots.contains(root) {
            return;
        }
        if name == "SyncMonitor" {
            self.error(
                "10905366",
                "'@SyncMonitor' cannot observe non-existent variables or non-state variables, except in wildcard-based monitoring scenarios.",
                span,
            );
        } else {
            self.warning(
                "The '@Monitor' decorator needs to monitor the state variables that exist."
                    .to_string(),
            );
        }
    }

    fn monitor_wildcard_mode(&mut self, argument: &Argument<'_>) -> MonitorWildcardMode {
        let Some(Expression::ObjectExpression(options)) = argument.as_expression() else {
            return MonitorWildcardMode::Disabled;
        };
        if options.properties.is_empty() {
            return MonitorWildcardMode::EnabledWithoutConsumedOptions;
        }
        if options.properties.len() != 1 {
            return MonitorWildcardMode::Disabled;
        }
        let ObjectPropertyKind::ObjectProperty(property) = &options.properties[0] else {
            return MonitorWildcardMode::Disabled;
        };
        let PropertyKey::StaticIdentifier(name) = &property.key else {
            return MonitorWildcardMode::Disabled;
        };
        if name.name != "enableWildcard" {
            return MonitorWildcardMode::Disabled;
        }
        let enabled = match &property.value {
            Expression::BooleanLiteral(value) => value.value,
            value => {
                self.error(
                    "10905372",
                    "The value of 'enableWildcard' must be a Boolean keyword.",
                    value.span(),
                );
                false
            }
        };
        if enabled {
            MonitorWildcardMode::Enabled
        } else {
            MonitorWildcardMode::ConsumedOptions
        }
    }

    fn error(&mut self, code: &str, message: impl Into<String>, span: oxc_span::Span) {
        self.errors.push(EtsTransformError::CodedDiagnostic {
            file: self.file.to_string(),
            code: code.to_string(),
            message: message.into(),
            severity: DiagnosticSeverity::Error,
            span: Some(span),
            solutions: Vec::new(),
        });
    }

    fn warning(&mut self, message: impl Into<String>) {
        self.errors.push(EtsTransformError::Diagnostic {
            file: self.file.to_string(),
            message: message.into(),
            severity: DiagnosticSeverity::Warning,
        });
    }
}

#[derive(Clone, Copy)]
enum MonitorWildcardMode {
    Disabled,
    ConsumedOptions,
    Enabled,
    EnabledWithoutConsumedOptions,
}

impl MonitorWildcardMode {
    fn consumes_options_argument(self) -> bool {
        matches!(self, Self::ConsumedOptions | Self::Enabled)
    }

    fn is_enabled(self) -> bool {
        matches!(self, Self::Enabled | Self::EnabledWithoutConsumedOptions)
    }
}

struct MonitorWildcardPattern<'a> {
    value: &'a str,
}

impl<'a> MonitorWildcardPattern<'a> {
    fn new(value: &'a str) -> Self {
        Self { value }
    }

    fn is_valid(&self) -> bool {
        let wildcard_count = self.value.bytes().filter(|byte| *byte == b'*').count();
        wildcard_count == 0
            || (wildcard_count == 1
                && self.value.ends_with(".*")
                && !matches!(self.value, "*" | ".*")
                && !self.value[..self.value.len() - 2].ends_with('.'))
    }
}
