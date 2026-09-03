use crate::error::{DiagnosticSeverity, EtsTransformError};
use crate::transform::decorator::{ArkuiDecorator, ArkuiDecoratorName};
use crate::transform::{ClassObservation, ImportedClassObservations};

use super::arkts11_monitor::Arkts11MonitorValidator;
use oxc_ast::ast::{Class, ClassElement, MethodDefinitionKind};
use rustc_hash::FxHashSet;

/// Rejects struct-only ArkUI decorators on ordinary classes.
///
/// The source owner is `validate_ui_syntax.ts::validateStructDecorator` and
/// `validateMethodDecorator`. Data-model decorators such as `@Observed`,
/// `@ObservedV2`, `@Track`, and `@Trace` deliberately remain valid on classes.
pub(super) struct Arkts11ClassValidator<'class, 'context, 'errors> {
    class: &'class Class<'class>,
    file: &'errors str,
    errors: &'errors mut Vec<EtsTransformError>,
    class_observations: Option<&'class ImportedClassObservations>,
    sdk_type_bindings: Option<&'context FxHashSet<String>>,
}

impl<'class, 'context, 'errors> Arkts11ClassValidator<'class, 'context, 'errors> {
    pub(super) fn new(
        class: &'class Class<'class>,
        file: &'errors str,
        errors: &'errors mut Vec<EtsTransformError>,
    ) -> Self {
        Self {
            class,
            file,
            errors,
            class_observations: None,
            sdk_type_bindings: None,
        }
    }

    pub(super) fn with_class_observation_inventory(
        mut self,
        class_observations: &'class ImportedClassObservations,
    ) -> Self {
        self.class_observations = Some(class_observations);
        self
    }

    pub(super) fn with_sdk_type_bindings(mut self, bindings: &'context FxHashSet<String>) -> Self {
        self.sdk_type_bindings = Some(bindings);
        self
    }

    pub(super) fn check(mut self) {
        let observed_v2 = self.class.decorators.iter().any(|decorator| {
            ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::OBSERVED_V2
        });
        let observed_v1 =
            self.class.decorators.iter().any(|decorator| {
                ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::OBSERVED
            });
        if observed_v1 && observed_v2 {
            self.errors.push(Self::diagnostic(
                self.file,
                "10905226",
                "A class can not be decorated by '@Observed' and '@ObservedV2' at the same time."
                    .to_string(),
                self.class.span,
            ));
        }
        self.check_observed_inheritance(observed_v1, observed_v2);

        for decorator in &self.class.decorators {
            let name = ArkuiDecorator::new(decorator).name();
            if self.is_sdk_type_decorator(name) {
                self.errors.push(Self::diagnostic(
                    self.file,
                    "10905345",
                    format!(
                        "The '@{name}' decorator can decorate only member variables of a class."
                    ),
                    decorator.span,
                ));
            } else if Self::is_struct_decorator(name) {
                self.errors.push(Self::diagnostic(
                    self.file,
                    "10905337",
                    format!("The '@{name}' decorator can only be used with 'struct'."),
                    decorator.span,
                ));
            }
        }

        for element in &self.class.body.body {
            match element {
                ClassElement::PropertyDefinition(property) => {
                    for decorator in &property.decorators {
                        let name = ArkuiDecorator::new(decorator).name();
                        if self.is_sdk_type_decorator(name) {
                            if observed_v1 {
                                self.errors.push(Self::type_owner_diagnostic(
                                    self.file,
                                    name,
                                    "Observed",
                                    decorator.span,
                                    "10905341",
                                ));
                            }
                            if self.is_sendable() {
                                self.errors.push(Self::type_owner_diagnostic(
                                    self.file,
                                    name,
                                    "Sendable",
                                    decorator.span,
                                    "10905342",
                                ));
                            }
                        } else if Self::is_struct_member_decorator(name) {
                            self.errors.push(Self::diagnostic(
                                self.file,
                                "10905337",
                                format!("The '@{name}' decorator can only be used with 'struct'."),
                                decorator.span,
                            ));
                        } else if name == ArkuiDecoratorName::TRACK && observed_v2 {
                            self.errors.push(Self::diagnostic(
                                self.file,
                                "10905344",
                                "'@Track' cannot be used with classes decorated by '@ObservedV2.' Use the '@Trace' decorator instead."
                                    .to_string(),
                                decorator.span,
                            ));
                        } else if name == ArkuiDecoratorName::TRACE && !observed_v2 {
                            self.errors.push(Self::class_member_diagnostic(
                                self.file,
                                name,
                                "variables",
                                decorator.span,
                            ));
                        } else if matches!(
                            name,
                            ArkuiDecoratorName::MONITOR
                                | ArkuiDecoratorName::COMPUTED
                                | ArkuiDecoratorName::SYNC_MONITOR
                        ) {
                            self.errors.push(Self::class_member_diagnostic(
                                self.file,
                                name,
                                "method",
                                decorator.span,
                            ));
                        }
                    }
                }
                ClassElement::MethodDefinition(method) => {
                    for decorator in &method.decorators {
                        let name = ArkuiDecorator::new(decorator).name();
                        if self.is_sdk_type_decorator(name) {
                            if observed_v1 {
                                self.errors.push(Self::type_owner_diagnostic(
                                    self.file,
                                    name,
                                    "Observed",
                                    decorator.span,
                                    "10905341",
                                ));
                            }
                            if self.is_sendable() {
                                self.errors.push(Self::type_owner_diagnostic(
                                    self.file,
                                    name,
                                    "Sendable",
                                    decorator.span,
                                    "10905342",
                                ));
                            }
                            self.errors.push(Self::diagnostic(
                                self.file,
                                "10905340",
                                format!(
                                    "The '@{name}' can decorate only member variables in a 'class'."
                                ),
                                decorator.span,
                            ));
                            continue;
                        }
                        if observed_v2
                            && matches!(
                                name,
                                ArkuiDecoratorName::MONITOR | ArkuiDecoratorName::SYNC_MONITOR
                            )
                        {
                            Arkts11MonitorValidator::new(decorator, self.file, self.errors).check();
                        }
                        let (code, message) = if matches!(
                            name,
                            ArkuiDecoratorName::BUILDER
                                | ArkuiDecoratorName::LOCAL_BUILDER
                                | ArkuiDecoratorName::STYLES
                        ) {
                            (
                                "10905114",
                                format!(
                                    "Use the '@{name}' decorator only in the global scope or in a struct."
                                ),
                            )
                        } else if matches!(
                            name,
                            ArkuiDecoratorName::EXTEND | ArkuiDecoratorName::ANIMATABLE_EXTEND
                        ) {
                            (
                                "10905113",
                                format!("Use the '@{name}' decorator only in the global scope."),
                            )
                        } else if Self::is_struct_member_decorator(name) {
                            (
                                "10905337",
                                format!("The '@{name}' decorator can only be used with 'struct'."),
                            )
                        } else if name == ArkuiDecoratorName::TRACE
                            || (matches!(
                                name,
                                ArkuiDecoratorName::MONITOR
                                    | ArkuiDecoratorName::COMPUTED
                                    | ArkuiDecoratorName::SYNC_MONITOR
                            ) && !observed_v2)
                        {
                            self.errors.push(Self::class_member_diagnostic(
                                self.file,
                                name,
                                if name == ArkuiDecoratorName::TRACE {
                                    "variables"
                                } else {
                                    "method"
                                },
                                decorator.span,
                            ));
                            continue;
                        } else if name == ArkuiDecoratorName::COMPUTED
                            && method.kind != MethodDefinitionKind::Get
                        {
                            (
                                "10905116",
                                "'@Computed' can only decorate 'GetAccessor'.".to_string(),
                            )
                        } else {
                            continue;
                        };
                        self.errors.push(Self::diagnostic(
                            self.file,
                            code,
                            message,
                            decorator.span,
                        ));
                    }
                }
                _ => {}
            }
        }
    }

    fn check_observed_inheritance(&mut self, observed_v1: bool, observed_v2: bool) {
        let Some(parent) = self
            .class
            .heritage
            .as_ref()
            .and_then(|heritage| self.parent_observation(&heritage.expression))
        else {
            return;
        };
        if observed_v2 && parent.observed_v1 {
            self.errors.push(Self::diagnostic(
                self.file,
                "10905225",
                "A class decorated by '@ObservedV2' cannot inherit from a class decorated by '@Observed'."
                    .to_string(),
                self.class.span,
            ));
        } else if observed_v1 && parent.observed_v2 {
            self.errors.push(Self::diagnostic(
                self.file,
                "10905224",
                "A class decorated by '@Observed' cannot inherit from a class decorated by '@ObservedV2'."
                    .to_string(),
                self.class.span,
            ));
        }
    }

    fn parent_observation(
        &self,
        expression: &oxc_ast::ast::Expression<'_>,
    ) -> Option<ClassObservation> {
        let inventory = self.class_observations?;
        match expression {
            oxc_ast::ast::Expression::Identifier(identifier) => {
                inventory.bindings.get(identifier.name.as_str()).copied()
            }
            oxc_ast::ast::Expression::StaticMemberExpression(member) => {
                let oxc_ast::ast::Expression::Identifier(namespace) = &member.object else {
                    return None;
                };
                inventory
                    .namespaces
                    .get(namespace.name.as_str())
                    .and_then(|classes| classes.get(member.property.name.as_str()))
                    .copied()
            }
            _ => None,
        }
    }

    fn diagnostic(
        file: &str,
        code: &str,
        message: String,
        span: oxc_span::Span,
    ) -> EtsTransformError {
        EtsTransformError::CodedDiagnostic {
            file: file.to_string(),
            code: code.to_string(),
            message,
            severity: DiagnosticSeverity::Error,
            span: Some(span),
            solutions: Vec::new(),
        }
    }

    fn class_member_diagnostic(
        file: &str,
        name: &str,
        member_kind: &str,
        span: oxc_span::Span,
    ) -> EtsTransformError {
        Self::diagnostic(
            file,
            "10905343",
            format!(
                "The '@{name}' can decorate only member '{member_kind}' within a 'class' decorated with '@ObservedV2'."
            ),
            span,
        )
    }

    fn type_owner_diagnostic(
        file: &str,
        name: &str,
        owner: &str,
        span: oxc_span::Span,
        code: &str,
    ) -> EtsTransformError {
        Self::diagnostic(
            file,
            code,
            format!(
                "The '@{name}' decorator can not be used in a 'class' decorated with '@{owner}'."
            ),
            span,
        )
    }

    fn is_sendable(&self) -> bool {
        self.class
            .decorators
            .iter()
            .any(|decorator| ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::SENDABLE)
    }

    fn is_sdk_type_decorator(&self, name: &str) -> bool {
        self.sdk_type_bindings
            .is_some_and(|bindings| bindings.contains(name))
    }

    fn is_struct_decorator(name: &str) -> bool {
        matches!(
            name,
            ArkuiDecoratorName::COMPONENT
                | ArkuiDecoratorName::COMPONENT_V2
                | ArkuiDecoratorName::ENTRY
                | ArkuiDecoratorName::PREVIEW
                | ArkuiDecoratorName::CUSTOM_DIALOG
                | ArkuiDecoratorName::REUSABLE
                | ArkuiDecoratorName::REUSABLE_V2
        )
    }

    fn is_struct_member_decorator(name: &str) -> bool {
        matches!(
            name,
            ArkuiDecoratorName::STATE
                | ArkuiDecoratorName::PROP
                | ArkuiDecoratorName::LINK
                | ArkuiDecoratorName::PROVIDE
                | ArkuiDecoratorName::CONSUME
                | ArkuiDecoratorName::WATCH
                | ArkuiDecoratorName::STORAGE_LINK
                | ArkuiDecoratorName::STORAGE_PROP
                | ArkuiDecoratorName::LOCAL_STORAGE_LINK
                | ArkuiDecoratorName::LOCAL_STORAGE_PROP
                | ArkuiDecoratorName::OBJECT_LINK
                | ArkuiDecoratorName::BUILDER_PARAM
                | ArkuiDecoratorName::REQUIRE
                | ArkuiDecoratorName::ENV
                | ArkuiDecoratorName::CUSTOM_ENV
                | ArkuiDecoratorName::LOCAL
                | ArkuiDecoratorName::PARAM
                | ArkuiDecoratorName::ONCE
                | ArkuiDecoratorName::EVENT
                | ArkuiDecoratorName::PROVIDER
                | ArkuiDecoratorName::CONSUMER
                | ArkuiDecoratorName::COMPONENT_INIT
                | ArkuiDecoratorName::COMPONENT_APPEAR
                | ArkuiDecoratorName::COMPONENT_BUILT
                | ArkuiDecoratorName::COMPONENT_ATTACH
                | ArkuiDecoratorName::COMPONENT_DETACH
                | ArkuiDecoratorName::COMPONENT_REUSE
                | ArkuiDecoratorName::COMPONENT_RECYCLE
                | ArkuiDecoratorName::COMPONENT_DISAPPEAR
                | ArkuiDecoratorName::COMPONENT_ACTIVE
                | ArkuiDecoratorName::COMPONENT_INACTIVE
        )
    }
}
