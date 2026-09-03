use oxc_ast::ast::{
    Argument, ArkUIChild, ArkUIComponentExpression, ArrowFunctionBody, Expression, FunctionBody,
    MethodDefinitionKind, ObjectPropertyKind, PropertyKey, Statement, StructElement,
    StructStatement, TSType, TSTypeName,
};
use oxc_span::{GetSpan, Span};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::error::{DiagnosticSeverity, EtsTransformError};
use crate::transform::component_meta::ComponentMetadata;
use crate::transform::decorator::{ArkuiDecorator, ArkuiDecoratorName};
use crate::transform::viewpu_emit::GeneratedViewpuName;
use crate::transform::ImportedClassObservations;

use super::arkts11_monitor::Arkts11MonitorValidator;

/// Checks the ArkTS 1.1 struct/member contracts owned by
/// `process_component_class.ts` and `process_component_member.ts`.
///
/// This validator only uses the already parsed OXC program. Cross-file type
/// compatibility remains in the project semantic checker; keeping those two
/// responsibilities separate prevents per-module transform hooks from reading
/// declarations or package metadata.
pub(super) struct Arkts11StructValidator<'statement, 'errors> {
    statement: &'statement StructStatement<'statement>,
    file: &'errors str,
    errors: &'errors mut Vec<EtsTransformError>,
    exported: bool,
    component_metadata: Option<&'statement ComponentMetadata>,
    component_reuse: Option<&'statement FxHashMap<String, bool>>,
    class_observations: Option<&'statement ImportedClassObservations>,
}

impl<'statement, 'errors> Arkts11StructValidator<'statement, 'errors> {
    pub(super) fn new(
        statement: &'statement StructStatement<'statement>,
        file: &'errors str,
        errors: &'errors mut Vec<EtsTransformError>,
    ) -> Self {
        Self {
            statement,
            file,
            errors,
            exported: false,
            component_metadata: None,
            component_reuse: None,
            class_observations: None,
        }
    }

    pub(super) fn with_component_reuse_inventory(
        mut self,
        component_reuse: &'statement FxHashMap<String, bool>,
    ) -> Self {
        self.component_reuse = Some(component_reuse);
        self
    }

    pub(super) fn with_component_metadata(
        mut self,
        component_metadata: Option<&'statement ComponentMetadata>,
    ) -> Self {
        self.component_metadata = component_metadata;
        self
    }

    pub(super) fn with_class_observation_inventory(
        mut self,
        class_observations: &'statement ImportedClassObservations,
    ) -> Self {
        self.class_observations = Some(class_observations);
        self
    }

    pub(super) fn with_exported(mut self, exported: bool) -> Self {
        self.exported = exported;
        self
    }

    pub(super) fn check(mut self) {
        self.check_decorators();
        self.check_global_reuse_pool();
        self.check_heritage();
        self.check_methods();
        self.check_properties();
        self.check_custom_dialog_controller();
    }

    fn check_global_reuse_pool(&mut self) {
        let Some(options) = self
            .statement
            .decorators
            .iter()
            .find(|decorator| {
                matches!(
                    ArkuiDecorator::new(decorator).name(),
                    ArkuiDecoratorName::COMPONENT | ArkuiDecoratorName::COMPONENT_V2
                )
            })
            .and_then(|decorator| ArkuiDecorator::new(decorator).arg_expression())
            .and_then(|argument| match argument {
                Expression::ObjectExpression(options) if options.properties.len() >= 2 => {
                    Some(options)
                }
                _ => None,
            })
        else {
            return;
        };

        let mut reuse_pool = None;
        let mut pool_accepts = None;
        for property in &options.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                return;
            };
            let PropertyKey::StaticIdentifier(name) = &property.key else {
                return;
            };
            match name.name.as_str() {
                "reusePool" => reuse_pool = Some(&property.value),
                "poolAccepts" => pool_accepts = Some(&property.value),
                _ => {}
            }
        }
        let (Some(reuse_pool), Some(pool_accepts)) = (reuse_pool, pool_accepts) else {
            self.error(
                "10905375",
                format!(
                    "'{}' must provide both reusePool and poolAccepts. Neither can be omitted when using the global reuse pool.",
                    self.statement.id.name
                ),
                options.span,
                &[],
            );
            return;
        };
        let Expression::ArrayExpression(pool_accepts) = pool_accepts else {
            return;
        };
        let Expression::StringLiteral(reuse_pool) = reuse_pool else {
            self.error(
                "10905378",
                "ReusePool can only accept string literal.",
                reuse_pool.span(),
                &[],
            );
            return;
        };
        if !matches!(reuse_pool.value.as_str(), "shared" | "perInstance") {
            self.error(
                "10905377",
                format!(
                    "ReusePool must be either 'shared' or 'perInstance'. The value '{}' is not valid.",
                    reuse_pool.value
                ),
                reuse_pool.span,
                &[],
            );
            return;
        }
        if pool_accepts.elements.is_empty() {
            self.error(
                "10905376",
                "PoolAccepts cannot be an empty array. Provide at least one '@Reusable' or '@ReusableV2' component.",
                pool_accepts.span,
                &[],
            );
            return;
        }
        for accepted in &pool_accepts.elements {
            let Some(Expression::Identifier(component)) = accepted.as_expression() else {
                continue;
            };
            if component.name == self.statement.id.name {
                self.error(
                    "10905374",
                    format!(
                        "'{}' cannot list itself in poolAccepts. The pool is not yet ready when '{}' is being constructed.",
                        component.name, component.name
                    ),
                    component.span,
                    &[],
                );
                continue;
            }
            if self
                .component_reuse
                .and_then(|inventory| inventory.get(component.name.as_str()))
                .is_some_and(|reusable| !reusable)
            {
                self.error(
                    "10905373",
                    format!(
                        "'{}' is not a '@Reusable' or '@ReusableV2' component and cannot be added to poolAccepts.",
                        component.name
                    ),
                    component.span,
                    &[],
                );
            }
        }
    }

    fn check_decorators(&mut self) {
        let decorators = self
            .statement
            .decorators
            .iter()
            .map(|decorator| (ArkuiDecorator::new(decorator).name(), decorator.span))
            .collect::<Vec<_>>();
        if decorators.is_empty() {
            self.error(
                "10905233",
                format!(
                    "Decorator '@Component', '@ComponentV2', or '@CustomDialog' is missing for struct '{}'.",
                    self.statement.id.name
                ),
                self.statement.span,
                &[],
            );
            return;
        }

        for (name, span) in &decorators {
            if !Self::is_inner_struct_decorator(name) {
                self.warning(
                    format!(
                        "The struct '{}' use invalid decorator.",
                        self.statement.id.name
                    ),
                    *span,
                );
            }
        }

        let has_inner = decorators
            .iter()
            .any(|(name, _)| Self::is_inner_struct_decorator(name));
        let has_main = decorators
            .iter()
            .any(|(name, _)| Self::is_main_struct_decorator(name));
        let has_component_v1 = decorators
            .iter()
            .any(|(name, _)| *name == ArkuiDecoratorName::COMPONENT);
        let has_component_v2 = decorators
            .iter()
            .any(|(name, _)| *name == ArkuiDecoratorName::COMPONENT_V2);
        let has_custom_dialog = decorators
            .iter()
            .any(|(name, _)| *name == ArkuiDecoratorName::CUSTOM_DIALOG);
        let has_reusable = decorators
            .iter()
            .any(|(name, _)| *name == ArkuiDecoratorName::REUSABLE);
        let has_reusable_v2 = decorators
            .iter()
            .any(|(name, _)| *name == ArkuiDecoratorName::REUSABLE_V2);
        let has_entry = decorators
            .iter()
            .any(|(name, _)| *name == ArkuiDecoratorName::ENTRY);

        if !has_inner {
            self.error(
                "10905230",
                format!(
                    "Decorator '@Component', '@ComponentV2', or '@CustomDialog' is missing for struct '{}'.",
                    self.statement.id.name
                ),
                self.statement.id.span,
                &[],
            );
        } else if has_component_v2 && (has_component_v1 || has_reusable || has_custom_dialog) {
            self.error(
                "10905229",
                format!(
                    "The struct '{}' can not be decorated with '@ComponentV2' and '@Component', '@Reusable', '@CustomDialog' at the same time.",
                    self.statement.id.name
                ),
                self.statement.id.span,
                &[],
            );
        } else if has_reusable_v2 && !has_component_v2 {
            self.error(
                "10905242",
                "'@ReusableV2' is only applicable to custom components decorated by '@ComponentV2'.",
                self.statement.id.span,
                &[],
            );
        }
        if has_reusable && has_reusable_v2 {
            self.error(
                "10905241",
                "The '@Reusable' and '@ReusableV2' decoraotrs cannot be applied simultaneously.",
                self.statement.id.span,
                &[],
            );
        }
        if self.component_metadata.is_some_and(|metadata| {
            metadata.is_builtin_style_name(self.statement.id.name.as_str())
                && !metadata.is_system_api_style_name(self.statement.id.name.as_str())
        }) {
            self.error(
                "10905228",
                format!(
                    "The struct '{}' cannot have the same name as the built-in attribute '{}'.",
                    self.statement.id.name, self.statement.id.name
                ),
                self.statement.id.span,
                &[],
            );
        }
        if self.component_metadata.is_some_and(|metadata| {
            metadata.is_builtin_component_name(self.statement.id.name.as_str())
        }) {
            self.error(
                "10905227",
                format!(
                    "The struct '{}' cannot have the same name as the built-in component '{}'.",
                    self.statement.id.name, self.statement.id.name
                ),
                self.statement.id.span,
                &[],
            );
        }
        if !has_main {
            self.warning(
                format!(
                    "Decorator '@Component', '@ComponentV2', or '@CustomDialog' is missing for struct '{}'.",
                    self.statement.id.name
                ),
                self.statement.id.span,
            );
        }
        if self.exported && has_entry {
            self.warning(
                "It's not a recommended way to export struct with '@Entry' decorator, which may cause ACE Engine error in component preview mode.",
                self.statement.span,
            );
        }
    }

    fn check_heritage(&mut self) {
        let valid_custom_component = self
            .statement
            .super_class
            .as_ref()
            .is_some_and(|expression| {
                matches!(expression, Expression::Identifier(identifier) if identifier.name == "CustomComponent")
            }) && self.statement.implements.is_empty();
        if (self.statement.super_class.is_some() || !self.statement.implements.is_empty())
            && !valid_custom_component
        {
            self.error(
                "10905212",
                "Structs are not allowed to inherit from classes or implement interfaces.",
                self.statement.span,
                &[],
            );
        }
    }

    fn check_methods(&mut self) {
        let mut build_count = 0usize;
        let observable_roots = self.observable_roots();
        for element in &self.statement.body.body {
            let StructElement::MethodDefinition(method) = element else {
                continue;
            };
            let method_name = Self::property_name(&method.key);
            let decorators = method
                .decorators
                .iter()
                .map(|decorator| ArkuiDecorator::new(decorator).name())
                .filter(|name| Self::is_method_decorator(name))
                .collect::<Vec<_>>();
            self.check_lifecycle_method(method);
            for decorator in &method.decorators {
                let name = ArkuiDecorator::new(decorator).name();
                if self.has_struct_decorator(ArkuiDecoratorName::COMPONENT_V2)
                    && matches!(
                        name,
                        ArkuiDecoratorName::MONITOR | ArkuiDecoratorName::SYNC_MONITOR
                    )
                {
                    Arkts11MonitorValidator::new(decorator, self.file, self.errors)
                        .with_observable_roots(&observable_roots)
                        .check();
                }
                if Self::is_v1_component_member_decorator(name)
                    || matches!(
                        name,
                        ArkuiDecoratorName::BUILDER_PARAM
                            | ArkuiDecoratorName::REQUIRE
                            | ArkuiDecoratorName::ENV
                            | ArkuiDecoratorName::CUSTOM_ENV
                    )
                {
                    self.error(
                        "10905112",
                        format!("'@{name}' can not decorate the method."),
                        decorator.span,
                        &[],
                    );
                }
                if Self::is_v2_component_member_decorator(name) {
                    self.error(
                        "10905346",
                        format!("'@{name}' can only decorate member property."),
                        decorator.span,
                        &[],
                    );
                }
                if name == ArkuiDecoratorName::CONCURRENT {
                    self.error(
                        "10905123",
                        "'@Concurrent' can not be used on method, please use it on function declaration.",
                        decorator.span,
                        &[],
                    );
                }
            }
            let distinct = decorators.iter().copied().collect::<FxHashSet<_>>();
            if distinct.len() > 1 {
                self.error(
                    "10905121",
                    "The member property or method can not be decorated by multiple built-in decorators.",
                    method.span,
                    &[],
                );
            }
            for decorator in distinct {
                if decorators
                    .iter()
                    .filter(|candidate| **candidate == decorator)
                    .count()
                    <= 1
                {
                    continue;
                }
                if matches!(
                    decorator,
                    ArkuiDecoratorName::MONITOR | ArkuiDecoratorName::COMPUTED
                ) {
                    self.error(
                        "10905119",
                        format!("Duplicate '@{decorator}' decorators for method are not allowed."),
                        method.span,
                        &[],
                    );
                } else {
                    self.warning(
                        format!("Duplicate '@{decorator}' decorators for method are not allowed."),
                        method.span,
                    );
                }
            }
            if decorators.contains(&ArkuiDecoratorName::COMPUTED)
                && method.kind != MethodDefinitionKind::Get
            {
                self.error(
                    "10905116",
                    "'@Computed' can only decorate 'GetAccessor'.",
                    method.span,
                    &[],
                );
            }
            if method_name == Some(GeneratedViewpuName::BUILD) {
                build_count += 1;
                if !method.value.params.items.is_empty() || method.value.params.rest.is_some() {
                    self.error(
                        "10905106",
                        "The 'build' method can not have arguments.",
                        method.span,
                        &[],
                    );
                }
                if self.check_build_root(method) {
                    self.check_ui_body_syntax(method);
                }
            }
            if method.decorators.iter().any(|decorator| {
                ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::STYLES
            }) && (!method.value.params.items.is_empty() || method.value.params.rest.is_some())
            {
                self.error(
                    "10905105",
                    "'@Styles' decorated functions and methods cannot have arguments.",
                    method.span,
                    &[],
                );
            }
            if method.decorators.iter().any(|decorator| {
                matches!(
                    ArkuiDecorator::new(decorator).name(),
                    ArkuiDecoratorName::BUILDER | ArkuiDecoratorName::LOCAL_BUILDER
                )
            }) {
                self.check_ui_body_syntax(method);
            }
            if method.r#static
                && method.decorators.iter().any(|decorator| {
                    ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::LOCAL_BUILDER
                })
            {
                self.error(
                    "10905104",
                    "Static methods in custom components cannot be decorated by '@LocalBuilder'.",
                    method.span,
                    &[],
                );
            }
        }
        if build_count != 1 {
            self.error(
                "10905103",
                format!(
                    "The struct '{}' must have at least and at most one 'build' method.",
                    self.statement.id.name
                ),
                self.statement.id.span,
                &["A structurally modified page must have at least one and no more than one 'build' method."],
            );
        }
    }

    fn check_lifecycle_method(&mut self, method: &oxc_ast::ast::MethodDefinition<'_>) {
        let names = method
            .decorators
            .iter()
            .map(|decorator| ArkuiDecorator::new(decorator).name())
            .collect::<Vec<_>>();
        let builder_styles = names
            .iter()
            .copied()
            .filter(|name| {
                matches!(
                    *name,
                    ArkuiDecoratorName::BUILDER | ArkuiDecoratorName::STYLES
                )
            })
            .collect::<Vec<_>>();
        let warning_lifecycle = names
            .iter()
            .copied()
            .filter(|name| Self::is_warning_lifecycle_decorator(name))
            .collect::<Vec<_>>();
        let error_lifecycle = names
            .iter()
            .copied()
            .filter(|name| {
                matches!(
                    *name,
                    ArkuiDecoratorName::COMPONENT_ACTIVE | ArkuiDecoratorName::COMPONENT_INACTIVE
                )
            })
            .collect::<Vec<_>>();

        if !builder_styles.is_empty() && !warning_lifecycle.is_empty() {
            self.warning(
                format!(
                    "Lifecycle decorators {} cannot be used together with {}.",
                    Self::decorator_list(&warning_lifecycle),
                    Self::decorator_list(&builder_styles)
                ),
                method.span,
            );
        }
        if !builder_styles.is_empty() && !error_lifecycle.is_empty() {
            self.error(
                "10905383",
                format!(
                    "Lifecycle decorators {} cannot be used together with {}.",
                    Self::decorator_list(&error_lifecycle),
                    Self::decorator_list(&builder_styles)
                ),
                method.span,
                &[],
            );
        }

        for name in names
            .iter()
            .copied()
            .filter(|name| Self::is_lifecycle_decorator(name))
        {
            if name != ArkuiDecoratorName::COMPONENT_REUSE {
                if !method.value.params.items.is_empty() || method.value.params.rest.is_some() {
                    self.error(
                        "10905371",
                        format!("Methods decorated with '@{name}' cannot have input parameters."),
                        method.span,
                        &[],
                    );
                }
                continue;
            }
            if self.has_struct_decorator(ArkuiDecoratorName::COMPONENT_V2) {
                if !method.value.params.items.is_empty() || method.value.params.rest.is_some() {
                    self.error(
                        "10905370",
                        "Methods decorated with '@ComponentReuse' in '@ComponentV2' cannot have input parameters.",
                        method.span,
                        &[],
                    );
                }
            } else if !Self::valid_component_v1_reuse_parameters(method) {
                self.error(
                    "10905369",
                    "In a struct decorated with '@Component', the function decorated with '@ComponentReuse' should have only one input parameter of the following type: Record<string, Object | null | undefined>.",
                    method.span,
                    &[],
                );
            }
        }
    }

    fn valid_component_v1_reuse_parameters(method: &oxc_ast::ast::MethodDefinition<'_>) -> bool {
        if method.value.params.items.is_empty() && method.value.params.rest.is_none() {
            return true;
        }
        if method.value.params.items.len() != 1 || method.value.params.rest.is_some() {
            return false;
        }
        let Some(annotation) = method.value.params.items[0].type_annotation.as_ref() else {
            return false;
        };
        match &annotation.type_annotation {
            TSType::TSObjectKeyword(_) => true,
            TSType::TSTypeReference(reference) => matches!(
                &reference.type_name,
                TSTypeName::IdentifierReference(identifier)
                    if matches!(identifier.name.as_str(), "Record" | "Object" | "ESObject" | "object")
            ),
            _ => false,
        }
    }

    fn decorator_list(names: &[&str]) -> String {
        names
            .iter()
            .map(|name| format!("@{name}"))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn check_build_root(&mut self, method: &oxc_ast::ast::MethodDefinition<'_>) -> bool {
        let Some(body) = method.value.body.as_ref() else {
            return true;
        };
        if body.statements.is_empty() {
            return true;
        }
        let valid_root = body.statements.len() == 1
            && match &body.statements[0] {
                Statement::IfStatement(_) => true,
                _ if !self.has_struct_decorator(ArkuiDecoratorName::ENTRY) => true,
                statement => self
                    .component_metadata
                    .and_then(|metadata| {
                        Self::root_component_name(statement).map(|name| {
                            !metadata.has_builtin_component_inventory()
                                || metadata.is_builtin_container_component(name)
                        })
                    })
                    .unwrap_or(true),
            };
        if !valid_root {
            self.error(
                "10905210",
                "In an '@Entry' decorated component, the 'build' method can have only one root node, which must be a container component.",
                body.span,
                &[],
            );
        }
        valid_root
    }

    fn check_ui_body_syntax(&mut self, method: &oxc_ast::ast::MethodDefinition<'_>) {
        let Some(body) = method.value.body.as_ref() else {
            return;
        };
        for statement in &body.statements {
            self.check_ui_statement(statement, None);
        }
    }

    fn check_ui_statement(&mut self, statement: &Statement<'_>, parent: Option<&str>) {
        match statement {
            Statement::ExpressionStatement(expression) => {
                if let Expression::ArkUIComponentExpression(component) = &expression.expression {
                    self.check_ui_component_children(component, parent);
                } else if !matches!(
                    expression.expression,
                    Expression::CallExpression(_)
                        | Expression::ChainExpression(_)
                        | Expression::TSNonNullExpression(_)
                ) {
                    self.error(
                        "10905204",
                        "The expression does not meet UI component syntax.",
                        expression.span,
                        &[],
                    );
                }
            }
            Statement::IfStatement(statement) => {
                self.check_ui_statement(&statement.consequent, parent);
                if let Some(alternate) = statement.alternate.as_ref() {
                    self.check_ui_statement(alternate, parent);
                }
            }
            Statement::BlockStatement(block) => {
                for statement in &block.body {
                    self.check_ui_statement(statement, parent);
                }
            }
            _ => self.error(
                "10905209",
                "Only UI component syntax can be written here.",
                statement.span(),
                &[],
            ),
        }
    }

    fn check_ui_component_children(
        &mut self,
        component: &oxc_ast::ast::ArkUIComponentExpression<'_>,
        parent: Option<&str>,
    ) {
        self.check_component_parent_contract(component, parent);
        self.check_component_child_contract(component);
        let component_name = Self::component_expression_name(component);
        if Self::is_iteration_component(component) {
            for argument in &component.arguments {
                self.check_iteration_argument(argument, parent);
            }
        }
        for child in &component.children {
            match child {
                oxc_ast::ast::ArkUIChild::Component(component) => {
                    self.check_ui_component_children(component, component_name);
                }
                oxc_ast::ast::ArkUIChild::Statement(statement) => {
                    self.check_ui_statement(statement, component_name);
                }
                oxc_ast::ast::ArkUIChild::Expression(expression) => {
                    self.check_ui_expression(expression, component_name);
                }
            }
        }
    }

    fn check_ui_expression(&mut self, expression: &Expression<'_>, parent: Option<&str>) {
        match expression {
            Expression::ArkUIComponentExpression(component) => {
                self.check_ui_component_children(component, parent);
            }
            Expression::ConditionalExpression(expression) => {
                self.check_ui_expression(&expression.consequent, parent);
                self.check_ui_expression(&expression.alternate, parent);
            }
            Expression::ParenthesizedExpression(expression) => {
                self.check_ui_expression(&expression.expression, parent);
            }
            _ => {}
        }
    }

    fn check_iteration_argument(&mut self, argument: &Argument<'_>, parent: Option<&str>) {
        let Some(Expression::ArrowFunctionExpression(arrow)) = argument.as_expression() else {
            return;
        };
        match &arrow.body {
            ArrowFunctionBody::FunctionBody(body) => {
                for statement in &body.statements {
                    self.check_ui_statement(statement, parent);
                }
            }
            body => {
                if let Some(expression) = body.as_expression() {
                    self.check_ui_expression(expression, parent);
                }
            }
        }
    }

    fn check_component_parent_contract(
        &mut self,
        component: &ArkUIComponentExpression<'_>,
        parent: Option<&str>,
    ) {
        let Some(component_name) = Self::component_expression_name(component) else {
            return;
        };
        let Some(parent) = parent.filter(|parent| {
            self.component_metadata
                .is_some_and(|metadata| metadata.is_builtin_component_name(parent))
        }) else {
            return;
        };
        let allowed = self
            .component_metadata
            .and_then(|metadata| metadata.specific_parent_components(component_name));
        if let Some(allowed) = allowed.filter(|allowed| !allowed.iter().any(|name| name == parent))
        {
            self.error(
                "10905201",
                format!(
                    "The '{component_name}' component can only be nested in the '{}' parent component.",
                    allowed.join(",")
                ),
                component.span,
                &[],
            );
        }
    }

    fn check_component_child_contract(&mut self, component: &ArkUIComponentExpression<'_>) {
        if !component.has_children {
            return;
        }
        let Expression::Identifier(callee) = &component.callee else {
            return;
        };
        let component_name = callee.name.as_str();
        let toggle_type = (component_name == "Toggle")
            .then(|| Self::toggle_component_type(component))
            .flatten();
        let atomic = self
            .component_metadata
            .is_some_and(|metadata| metadata.is_atomic_component(component_name));
        if atomic || matches!(toggle_type, Some("Checkbox" | "Switch")) {
            if let Some(toggle_type) = toggle_type {
                self.error(
                    "10905223",
                    format!(
                        "When the component '{component_name}' set 'type' as '{toggle_type}', it can't have any child."
                    ),
                    callee.span,
                    &[],
                );
            } else {
                self.error(
                    "10905222",
                    format!("The component '{component_name}' can't have any child."),
                    callee.span,
                    &[],
                );
            }
        }

        let single = self
            .component_metadata
            .is_some_and(|metadata| metadata.is_single_child_component(component_name))
            || toggle_type == Some("Button");
        if single && (component.children.len() > 3 || self.component_children_count(component) > 1)
        {
            if let Some(toggle_type) = toggle_type {
                self.error(
                    "10905221",
                    format!(
                        "When the component '{component_name}' set 'type' as '{toggle_type}', it can only have a single child component."
                    ),
                    callee.span,
                    &[],
                );
            } else {
                self.error(
                    "10905220",
                    format!("The '{component_name}' component can have only one child component."),
                    callee.span,
                    &[],
                );
            }
        }

        let allowed = self
            .component_metadata
            .and_then(|metadata| metadata.specific_child_components(component_name))
            .map(<[String]>::to_vec);
        if let Some(allowed) = allowed
            .filter(|allowed| self.component_has_disallowed_child(component, allowed.as_slice()))
        {
            self.error(
                "10905219",
                format!(
                    "The component '{component_name}' can only have the child component '{}'.",
                    allowed.join(",")
                ),
                callee.span,
                &[],
            );
        }

        if component_name == "Button"
            && component
                .arguments
                .iter()
                .any(Self::is_button_label_argument)
        {
            self.error(
                "10905202",
                "The Button component with a label parameter can not have any child.",
                callee.span,
                &[],
            );
        }
    }

    fn toggle_component_type<'node>(
        component: &'node ArkUIComponentExpression<'_>,
    ) -> Option<&'node str> {
        let argument = component.arguments.first()?.as_expression()?;
        let Expression::ObjectExpression(object) = argument else {
            return None;
        };
        object.properties.iter().find_map(|property| {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                return None;
            };
            let PropertyKey::StaticIdentifier(name) = &property.key else {
                return None;
            };
            if name.name != "type" {
                return None;
            }
            match &property.value {
                Expression::Identifier(identifier) => Some(identifier.name.as_str()),
                Expression::StaticMemberExpression(member) => Some(member.property.name.as_str()),
                Expression::StringLiteral(literal) => Some(literal.value.as_str()),
                _ => None,
            }
        })
    }

    fn is_button_label_argument(argument: &Argument<'_>) -> bool {
        match argument.as_expression() {
            Some(Expression::StringLiteral(_)) => true,
            Some(Expression::CallExpression(call)) => {
                matches!(&call.callee, Expression::Identifier(identifier) if identifier.name == "$r")
            }
            _ => false,
        }
    }

    fn component_expression_name<'node>(
        component: &'node ArkUIComponentExpression<'_>,
    ) -> Option<&'node str> {
        match &component.callee {
            Expression::Identifier(identifier) => Some(identifier.name.as_str()),
            _ => None,
        }
    }

    fn component_children_count(&self, component: &ArkUIComponentExpression<'_>) -> usize {
        component
            .children
            .iter()
            .map(|child| self.arkui_child_count(child))
            .sum::<usize>()
            .min(2)
    }

    fn arkui_child_count(&self, child: &ArkUIChild<'_>) -> usize {
        match child {
            ArkUIChild::Component(component) => {
                if Self::is_iteration_component(component) {
                    2
                } else {
                    1
                }
            }
            ArkUIChild::Statement(statement) => self.statement_child_count(statement),
            ArkUIChild::Expression(expression) => self.expression_child_count(expression),
        }
    }

    fn statement_child_count(&self, statement: &Statement<'_>) -> usize {
        match statement {
            Statement::ExpressionStatement(statement) => {
                self.expression_child_count(&statement.expression)
            }
            Statement::BlockStatement(block) => block
                .body
                .iter()
                .map(|statement| self.statement_child_count(statement))
                .sum::<usize>()
                .min(2),
            Statement::IfStatement(statement) => {
                self.statement_child_count(&statement.consequent).max(
                    statement
                        .alternate
                        .as_ref()
                        .map_or(0, |alternate| self.statement_child_count(alternate)),
                )
            }
            _ => 0,
        }
    }

    fn expression_child_count(&self, expression: &Expression<'_>) -> usize {
        match expression {
            Expression::ArkUIComponentExpression(component) => {
                if Self::is_iteration_component(component) {
                    2
                } else {
                    1
                }
            }
            Expression::ConditionalExpression(expression) => self
                .expression_child_count(&expression.consequent)
                .max(self.expression_child_count(&expression.alternate)),
            Expression::ParenthesizedExpression(expression) => {
                self.expression_child_count(&expression.expression)
            }
            _ => 0,
        }
    }

    fn component_has_disallowed_child(
        &self,
        component: &ArkUIComponentExpression<'_>,
        allowed: &[String],
    ) -> bool {
        component
            .children
            .iter()
            .any(|child| self.arkui_child_is_disallowed(child, allowed))
    }

    fn arkui_child_is_disallowed(&self, child: &ArkUIChild<'_>, allowed: &[String]) -> bool {
        match child {
            ArkUIChild::Component(component) => {
                self.component_expression_is_disallowed(component, allowed)
            }
            ArkUIChild::Statement(statement) => {
                self.statement_has_disallowed_child(statement, allowed)
            }
            ArkUIChild::Expression(expression) => {
                self.expression_has_disallowed_child(expression, allowed)
            }
        }
    }

    fn component_expression_is_disallowed(
        &self,
        component: &ArkUIComponentExpression<'_>,
        allowed: &[String],
    ) -> bool {
        if Self::is_iteration_component(component) {
            return component
                .arguments
                .iter()
                .any(|argument| self.argument_has_disallowed_child(argument, allowed));
        }
        matches!(
            &component.callee,
            Expression::Identifier(identifier)
                if !allowed.iter().any(|name| name == identifier.name.as_str())
        )
    }

    fn argument_has_disallowed_child(&self, argument: &Argument<'_>, allowed: &[String]) -> bool {
        let Some(Expression::ArrowFunctionExpression(arrow)) = argument.as_expression() else {
            return false;
        };
        if let ArrowFunctionBody::FunctionBody(body) = &arrow.body {
            return self.function_body_has_disallowed_child(body, allowed);
        }
        arrow
            .body
            .as_expression()
            .is_some_and(|expression| self.expression_has_disallowed_child(expression, allowed))
    }

    fn function_body_has_disallowed_child(
        &self,
        body: &FunctionBody<'_>,
        allowed: &[String],
    ) -> bool {
        body.statements
            .iter()
            .any(|statement| self.statement_has_disallowed_child(statement, allowed))
    }

    fn statement_has_disallowed_child(
        &self,
        statement: &Statement<'_>,
        allowed: &[String],
    ) -> bool {
        match statement {
            Statement::ExpressionStatement(statement) => {
                self.expression_has_disallowed_child(&statement.expression, allowed)
            }
            Statement::BlockStatement(block) => block
                .body
                .iter()
                .any(|statement| self.statement_has_disallowed_child(statement, allowed)),
            Statement::IfStatement(statement) => {
                self.statement_has_disallowed_child(&statement.consequent, allowed)
                    || statement.alternate.as_ref().is_some_and(|alternate| {
                        self.statement_has_disallowed_child(alternate, allowed)
                    })
            }
            _ => false,
        }
    }

    fn expression_has_disallowed_child(
        &self,
        expression: &Expression<'_>,
        allowed: &[String],
    ) -> bool {
        match expression {
            Expression::ArkUIComponentExpression(component) => {
                self.component_expression_is_disallowed(component, allowed)
            }
            Expression::ConditionalExpression(expression) => {
                self.expression_has_disallowed_child(&expression.consequent, allowed)
                    || self.expression_has_disallowed_child(&expression.alternate, allowed)
            }
            Expression::ParenthesizedExpression(expression) => {
                self.expression_has_disallowed_child(&expression.expression, allowed)
            }
            _ => false,
        }
    }

    fn is_iteration_component(component: &ArkUIComponentExpression<'_>) -> bool {
        matches!(
            &component.callee,
            Expression::Identifier(identifier)
                if matches!(identifier.name.as_str(), "ForEach" | "LazyForEach")
        )
    }

    fn root_component_name<'node>(statement: &'node Statement<'_>) -> Option<&'node str> {
        let Statement::ExpressionStatement(statement) = statement else {
            return None;
        };
        Self::expression_root_name(&statement.expression)
    }

    fn expression_root_name<'node>(expression: &'node Expression<'_>) -> Option<&'node str> {
        match expression {
            Expression::ArkUIComponentExpression(component) => {
                Self::expression_root_name(&component.callee)
            }
            Expression::CallExpression(call) => Self::expression_root_name(&call.callee),
            Expression::StaticMemberExpression(member) => {
                Self::expression_root_name(&member.object)
            }
            Expression::Identifier(identifier) => Some(identifier.name.as_str()),
            Expression::ParenthesizedExpression(parenthesized) => {
                Self::expression_root_name(&parenthesized.expression)
            }
            _ => None,
        }
    }

    fn check_properties(&mut self) {
        let method_names = self.method_names();
        let entry = self.has_struct_decorator(ArkuiDecoratorName::ENTRY);
        let component_v2 = self.has_struct_decorator(ArkuiDecoratorName::COMPONENT_V2);
        for element in &self.statement.body.body {
            let StructElement::PropertyDefinition(property) = element else {
                continue;
            };
            if property.decorators.is_empty() {
                continue;
            }
            let name = Self::property_name(&property.key).unwrap_or("<unknown>");
            let decorators = property
                .decorators
                .iter()
                .map(|decorator| ArkuiDecorator::new(decorator).name())
                .collect::<Vec<_>>();

            if decorators.contains(&ArkuiDecoratorName::ENV)
                && !self.has_struct_decorator(ArkuiDecoratorName::COMPONENT)
                && !self.has_struct_decorator(ArkuiDecoratorName::COMPONENT_V2)
            {
                self.error(
                    "10905250",
                    "The '@Env' decorator can only be used in structs decorated with either '@Component' or '@ComponentV2'.",
                    property.span,
                    &[],
                );
            }

            if !component_v2 {
                self.check_custom_decorator_conflict(property, &decorators);
                self.check_forbidden_state_type(property, name, &decorators);
                self.check_observed_v2_property_type(property, &decorators);
            }

            for decorator in &property.decorators {
                let decorator_name = ArkuiDecorator::new(decorator).name();
                if self
                    .component_metadata
                    .is_some_and(|metadata| metadata.is_builtin_style_name(decorator_name))
                {
                    self.error(
                        "10905309",
                        format!(
                            "The decorator '@{decorator_name}' cannot have the same name as the built-in style attribute '{decorator_name}'."
                        ),
                        decorator.span,
                        &[],
                    );
                }
                if matches!(
                    decorator_name,
                    ArkuiDecoratorName::MONITOR
                        | ArkuiDecoratorName::LOCAL_BUILDER
                        | ArkuiDecoratorName::SYNC_MONITOR
                ) {
                    self.error(
                        "10905115",
                        format!("'@{decorator_name}' can only decorate method."),
                        decorator.span,
                        &[],
                    );
                } else if decorator_name == ArkuiDecoratorName::COMPUTED {
                    self.error(
                        "10905116",
                        "'@Computed' can only decorate 'GetAccessor'.",
                        decorator.span,
                        &[],
                    );
                }
                if component_v2 && Self::is_v1_component_member_decorator(decorator_name) {
                    self.error(
                        "10905339",
                        format!(
                            "The '@{decorator_name}' decorator can only be used in a 'struct' decorated with '@Component'."
                        ),
                        decorator.span,
                        &[],
                    );
                } else if !component_v2 && Self::is_v2_decorator(decorator_name) {
                    self.error(
                        "10905338",
                        format!(
                            "The '@{decorator_name}' decorator can only be used in a 'struct' decorated with '@ComponentV2'."
                        ),
                        decorator.span,
                        &[],
                    );
                }
            }

            if component_v2 {
                self.check_component_v2_property_contract(property, name, &decorators);
            }

            if property.type_annotation.is_none() {
                if component_v2 {
                    if decorators
                        .iter()
                        .any(|decorator| Self::requires_component_v2_member_type(decorator))
                    {
                        self.error(
                            "10905328",
                            format!("The property '{name}' must specify a type."),
                            property.span,
                            &[],
                        );
                    }
                } else {
                    self.error(
                        "10905305",
                        format!("The property '{name}' must specify a type."),
                        property.span,
                        &[],
                    );
                }
            }

            if !component_v2 {
                let member_decorators = decorators
                    .iter()
                    .copied()
                    .filter(|decorator| Self::counts_as_state_management_decorator(decorator))
                    .collect::<Vec<_>>();
                if Self::has_invalid_multiple_decorators(&member_decorators) {
                    self.error(
                        "10905302",
                        format!(
                            "The property '{name}' cannot have multiple state management decorators."
                        ),
                        property.span,
                        &[],
                    );
                }
            }

            let has_require = decorators.contains(&ArkuiDecoratorName::REQUIRE);
            for decorator in &decorators {
                if *decorator == ArkuiDecoratorName::BUILDER_PARAM
                    && property
                        .value
                        .as_ref()
                        .is_some_and(Self::is_non_builder_literal)
                {
                    self.error(
                        "10905101",
                        "'@BuilderParam' property can only initialized by '@Builder' function or '@LocalBuilder' method in struct.",
                        property.span,
                        &[],
                    );
                }
                if *decorator == ArkuiDecoratorName::OBJECT_LINK
                    && property.type_annotation.as_ref().is_some_and(|annotation| {
                        Self::is_object_link_forbidden_type(&annotation.type_annotation)
                    })
                {
                    self.error(
                        "10905307",
                        "'@ObjectLink' cannot be used with this type. Apply it only to classes decorated by '@ObservedV2' or initialized using the return value of 'makeV1Observed'.",
                        property.span,
                        &[],
                    );
                }
                if Self::forbids_function_type(decorator)
                    && property.type_annotation.as_ref().is_some_and(|annotation| {
                        Self::is_function_type(&annotation.type_annotation)
                    })
                {
                    self.error(
                        "10905363",
                        format!(
                            "The V1 decorator '@{decorator}' cannot be applied to a Function-type variable '{name}'.(From Api Version 23)"
                        ),
                        property.span,
                        &[],
                    );
                }
                if Self::requires_default_value(decorator)
                    && property.value.is_none()
                    && !(has_require && Self::require_releases_default(decorator))
                {
                    self.error(
                        "10905303",
                        format!(
                            "The '@{decorator}' property '{name}' must be specified a default value."
                        ),
                        property.span,
                        &[],
                    );
                }
                if Self::forbids_default_value(decorator)
                    && property.value.is_some()
                    && !(component_v2 && *decorator == ArkuiDecoratorName::ENV)
                {
                    self.error(
                        "10905304",
                        format!("The '@{decorator}' property cannot be specified a default value."),
                        property.span,
                        &["Please initialize the rules according to the decorator."],
                    );
                }
                if property.r#static && Self::is_member_decorator(decorator) {
                    self.error(
                        "10905313",
                        "The static variable of struct cannot be used together with built-in decorators.",
                        property.span,
                        &[],
                    );
                }
                if entry && Self::must_initialize_from_parent(decorator) {
                    self.warning(
                        format!(
                            "The '@Entry' component '{}' cannot have the '@{decorator}' property '{name}'.",
                            self.statement.id.name
                        ),
                        property.span,
                    );
                }
                if property.optional && Self::must_initialize_from_parent(decorator) {
                    self.warning(
                        format!(
                            "The @{decorator} property '{name}' cannot be an optional parameter."
                        ),
                        property.span,
                    );
                }
            }

            if decorators.contains(&ArkuiDecoratorName::WATCH) {
                self.check_watch(property, name, &decorators, &method_names);
            }
        }
    }

    fn check_component_v2_property_contract(
        &mut self,
        property: &oxc_ast::ast::PropertyDefinition<'_>,
        _name: &str,
        decorators: &[&str],
    ) {
        let counted = decorators
            .iter()
            .copied()
            .filter(|decorator| Self::counts_for_component_v2_conflict(decorator))
            .collect::<Vec<_>>();
        let mut seen = FxHashSet::default();
        let mut distinct = Vec::new();
        let mut duplicate_seen = FxHashSet::default();
        let mut duplicates = Vec::new();
        for decorator in &counted {
            if seen.insert(*decorator) {
                distinct.push(*decorator);
            } else if duplicate_seen.insert(*decorator) {
                duplicates.push(*decorator);
            }
        }
        if distinct.len() > 1 {
            self.error(
                "10905121",
                "The member property or method can not be decorated by multiple built-in decorators.",
                property.span,
                &[],
            );
        }
        if !duplicates.is_empty() {
            self.error(
                "10905119",
                format!(
                    "Duplicate '{}' decorators for method are not allowed.",
                    duplicates
                        .iter()
                        .map(|decorator| format!("@{decorator}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                property.span,
                &[],
            );
        }

        let has_param = decorators.contains(&ArkuiDecoratorName::PARAM);
        let has_require = decorators.contains(&ArkuiDecoratorName::REQUIRE);
        let has_builder_param = decorators.contains(&ArkuiDecoratorName::BUILDER_PARAM);
        if has_param && property.value.is_none() && !has_require {
            self.error(
                "10905327",
                "When a variable decorated with '@Param' is not assigned a default value, it must also be decorated with '@Require'.",
                property.span,
                &[],
            );
        }
        if decorators.contains(&ArkuiDecoratorName::ONCE) && !has_param {
            self.error(
                "10905326",
                "When a variable decorated with '@Once', it must also be decorated with '@Param'.",
                property.span,
                &[],
            );
        }
        if has_require && !has_param && !has_builder_param {
            self.error(
                "10905325",
                "In a struct decorated with '@ComponentV2', '@Require' can only be used with '@Param' and '@BuilderParam'.",
                property.span,
                &[],
            );
        }
        if decorators.contains(&ArkuiDecoratorName::ENV) && property.value.is_some() {
            self.error(
                "10905362",
                "The '@Env' property cannot be specified a default value.",
                property.span,
                &[],
            );
        }
    }

    fn check_watch(
        &mut self,
        property: &oxc_ast::ast::PropertyDefinition<'_>,
        name: &str,
        decorators: &[&str],
        method_names: &FxHashSet<String>,
    ) {
        let non_meta_decorators = decorators
            .iter()
            .filter(|decorator| {
                !matches!(
                    **decorator,
                    ArkuiDecoratorName::WATCH | ArkuiDecoratorName::REQUIRE
                )
            })
            .count();
        if non_meta_decorators == 0 {
            self.error(
                "10905310",
                format!("Regular variable '{name}' can not be decorated with '@Watch'."),
                property.span,
                &[],
            );
            return;
        }

        let watch = property
            .decorators
            .iter()
            .find(|decorator| ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::WATCH)
            .expect("the caller established a @Watch decorator");
        let Expression::CallExpression(call) = &watch.expression else {
            return;
        };
        if call.arguments.len() != 1 {
            return;
        }
        let Some(argument) = call.arguments.first() else {
            return;
        };
        match argument {
            Argument::StringLiteral(literal) => {
                if !method_names.contains(literal.value.as_str()) {
                    self.error(
                        "10905301",
                        format!(
                            "'@Watch' cannot be used with '{}'. Apply it only to parameters that correspond to existing methods.",
                            literal.raw.as_ref().map_or(literal.value.as_str(), |raw| raw.as_str())
                        ),
                        literal.span,
                        &[],
                    );
                }
            }
            Argument::Identifier(identifier) => self.warning_with_code(
                "10905311",
                format!(
                    "'@Watch' cannot be used with '{}'. Apply it only to 'string' parameters.",
                    identifier.name
                ),
                identifier.span,
            ),
            Argument::StaticMemberExpression(member) => self.warning_with_code(
                "10905311",
                "'@Watch' cannot be used with a property access. Apply it only to 'string' parameters.",
                member.span,
            ),
            _ => self.error(
                "10905311",
                "'@Watch' cannot be used with this argument. Apply it only to 'string' parameters.",
                argument.span(),
                &[],
            ),
        }
    }

    fn check_custom_decorator_conflict(
        &mut self,
        property: &oxc_ast::ast::PropertyDefinition<'_>,
        decorators: &[&str],
    ) {
        let inner = decorators
            .iter()
            .copied()
            .find(|name| Self::is_v1_inner_member_decorator(name));
        let has_custom = decorators
            .iter()
            .any(|name| !Self::is_v1_inner_member_decorator(name));
        if let Some(inner) = inner.filter(|_| has_custom) {
            self.error(
                "10905312",
                format!(
                    "The inner decorator '@{inner}' cannot be used together with custom decorator."
                ),
                property.span,
                &[],
            );
        }
    }

    fn check_forbidden_state_type(
        &mut self,
        property: &oxc_ast::ast::PropertyDefinition<'_>,
        property_name: &str,
        decorators: &[&str],
    ) {
        let Some(decorator) = decorators.iter().copied().find(|name| {
            Self::is_v1_inner_member_decorator(name)
                && !matches!(
                    *name,
                    ArkuiDecoratorName::WATCH | ArkuiDecoratorName::REQUIRE
                )
        }) else {
            return;
        };
        let Some(type_name) = property.type_annotation.as_ref().and_then(|annotation| {
            match &annotation.type_annotation {
                TSType::TSTypeReference(reference) => match &reference.type_name {
                    TSTypeName::IdentifierReference(identifier) => Some(identifier.name.as_str()),
                    _ => None,
                },
                _ => None,
            }
        }) else {
            return;
        };
        if self
            .component_metadata
            .is_some_and(|metadata| metadata.is_forbidden_state_type(type_name))
        {
            self.error(
                "10905308",
                format!(
                    "The '@{decorator}' property '{property_name}' cannot be a '{type_name}' object."
                ),
                property.span,
                &[],
            );
        }
    }

    fn check_observed_v2_property_type(
        &mut self,
        property: &oxc_ast::ast::PropertyDefinition<'_>,
        decorators: &[&str],
    ) {
        let Some(decorator) = decorators
            .iter()
            .copied()
            .find(|name| Self::is_v1_component_member_decorator(name))
        else {
            return;
        };
        let Some(annotation) = property.type_annotation.as_ref() else {
            return;
        };
        if self.type_contains_observed_v2(&annotation.type_annotation) {
            self.error(
                "10905348",
                format!(
                    "The type of the '@{decorator}' property can not be a class decorated with '@ObservedV2'."
                ),
                property.span,
                &[],
            );
        }
    }

    fn type_contains_observed_v2(&self, ty: &TSType<'_>) -> bool {
        match ty {
            TSType::TSUnionType(union) => union
                .types
                .iter()
                .any(|member| self.type_contains_observed_v2(member)),
            TSType::TSTypeReference(reference) => self
                .class_observations
                .and_then(|observations| match &reference.type_name {
                    TSTypeName::IdentifierReference(identifier) => {
                        observations.bindings.get(identifier.name.as_str()).copied()
                    }
                    TSTypeName::QualifiedName(qualified) => {
                        let TSTypeName::IdentifierReference(namespace) = &qualified.left else {
                            return None;
                        };
                        observations
                            .namespaces
                            .get(namespace.name.as_str())
                            .and_then(|classes| classes.get(qualified.right.name.as_str()))
                            .copied()
                    }
                    TSTypeName::ThisExpression(_) => None,
                })
                .is_some_and(|observation| observation.observed_v2),
            _ => false,
        }
    }

    fn check_custom_dialog_controller(&mut self) {
        if !self.has_struct_decorator(ArkuiDecoratorName::CUSTOM_DIALOG) {
            return;
        }
        let controller_count = self
            .statement
            .body
            .body
            .iter()
            .filter_map(|element| match element {
                StructElement::PropertyDefinition(property) => Some(property),
                _ => None,
            })
            .filter(|property| {
                property.type_annotation.as_ref().is_some_and(|annotation| {
                    matches!(
                        &annotation.type_annotation,
                        TSType::TSTypeReference(reference)
                            if matches!(&reference.type_name, TSTypeName::IdentifierReference(identifier) if identifier.name == "CustomDialogController")
                    )
                })
            })
            .count();
        if controller_count == 0 {
            self.error(
                "10905211",
                "The '@CustomDialog' decorated custom component must contain a property of the CustomDialogController type.",
                self.statement.id.span,
                &[],
            );
        }
        if controller_count >= 2 {
            self.warning(
                "A @CustomDialog component can only have one uninitialized CustomDialogController.",
                self.statement.id.span,
            );
        }
    }

    fn method_names(&self) -> FxHashSet<String> {
        self.statement
            .body
            .body
            .iter()
            .filter_map(|element| match element {
                StructElement::MethodDefinition(method) => {
                    Self::property_name(&method.key).map(str::to_string)
                }
                _ => None,
            })
            .collect()
    }

    fn observable_roots(&self) -> FxHashSet<String> {
        let mut roots = FxHashSet::default();
        for element in &self.statement.body.body {
            match element {
                StructElement::PropertyDefinition(property)
                    if property.decorators.iter().any(|decorator| {
                        matches!(
                            ArkuiDecorator::new(decorator).name(),
                            ArkuiDecoratorName::LOCAL
                                | ArkuiDecoratorName::PARAM
                                | ArkuiDecoratorName::PROVIDER
                                | ArkuiDecoratorName::CONSUMER
                                | ArkuiDecoratorName::ONCE
                        )
                    }) =>
                {
                    if let Some(name) = Self::property_name(&property.key) {
                        roots.insert(name.to_string());
                    }
                }
                StructElement::MethodDefinition(method)
                    if method.kind == MethodDefinitionKind::Get
                        && method.decorators.iter().any(|decorator| {
                            ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::COMPUTED
                        }) =>
                {
                    if let Some(name) = Self::property_name(&method.key) {
                        roots.insert(name.to_string());
                    }
                }
                _ => {}
            }
        }
        roots
    }

    fn has_struct_decorator(&self, name: &str) -> bool {
        self.statement
            .decorators
            .iter()
            .any(|decorator| ArkuiDecorator::new(decorator).name() == name)
    }

    fn property_name<'key>(key: &'key PropertyKey<'_>) -> Option<&'key str> {
        match key {
            PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.as_str()),
            _ => None,
        }
    }

    fn is_member_decorator(name: &str) -> bool {
        matches!(
            name,
            ArkuiDecoratorName::STATE
                | ArkuiDecoratorName::PROP
                | ArkuiDecoratorName::LINK
                | ArkuiDecoratorName::STORAGE_PROP
                | ArkuiDecoratorName::STORAGE_LINK
                | ArkuiDecoratorName::PROVIDE
                | ArkuiDecoratorName::CONSUME
                | ArkuiDecoratorName::OBJECT_LINK
                | ArkuiDecoratorName::WATCH
                | ArkuiDecoratorName::BUILDER_PARAM
                | ArkuiDecoratorName::LOCAL_STORAGE_LINK
                | ArkuiDecoratorName::LOCAL_STORAGE_PROP
                | ArkuiDecoratorName::REQUIRE
                | ArkuiDecoratorName::ENV
                | ArkuiDecoratorName::CUSTOM_ENV
        )
    }

    fn is_main_struct_decorator(name: &str) -> bool {
        matches!(
            name,
            ArkuiDecoratorName::COMPONENT
                | ArkuiDecoratorName::COMPONENT_V2
                | ArkuiDecoratorName::CUSTOM_DIALOG
        )
    }

    fn is_inner_struct_decorator(name: &str) -> bool {
        matches!(
            name,
            ArkuiDecoratorName::ENTRY
                | ArkuiDecoratorName::PREVIEW
                | ArkuiDecoratorName::COMPONENT
                | ArkuiDecoratorName::CUSTOM_DIALOG
                | ArkuiDecoratorName::REUSABLE
                | ArkuiDecoratorName::COMPONENT_V2
                | ArkuiDecoratorName::REUSABLE_V2
                | "InsightIntentPage"
        )
    }

    fn is_v1_component_member_decorator(name: &str) -> bool {
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
        )
    }

    fn is_v1_inner_member_decorator(name: &str) -> bool {
        Self::is_v1_component_member_decorator(name)
            || matches!(
                name,
                ArkuiDecoratorName::BUILDER_PARAM
                    | ArkuiDecoratorName::REQUIRE
                    | ArkuiDecoratorName::ENV
                    | ArkuiDecoratorName::CUSTOM_ENV
            )
    }

    fn is_v2_component_member_decorator(name: &str) -> bool {
        matches!(
            name,
            ArkuiDecoratorName::LOCAL
                | ArkuiDecoratorName::PARAM
                | ArkuiDecoratorName::ONCE
                | ArkuiDecoratorName::EVENT
                | ArkuiDecoratorName::PROVIDER
                | ArkuiDecoratorName::CONSUMER
        )
    }

    fn is_v2_decorator(name: &str) -> bool {
        Self::is_v2_component_member_decorator(name)
            || matches!(
                name,
                ArkuiDecoratorName::MONITOR
                    | ArkuiDecoratorName::COMPUTED
                    | ArkuiDecoratorName::SYNC_MONITOR
            )
    }

    fn counts_for_component_v2_conflict(name: &str) -> bool {
        name != ArkuiDecoratorName::ONCE
            && (Self::is_v2_decorator(name)
                || matches!(
                    name,
                    ArkuiDecoratorName::BUILDER_PARAM
                        | ArkuiDecoratorName::ENV
                        | ArkuiDecoratorName::CUSTOM_ENV
                ))
    }

    fn requires_component_v2_member_type(name: &str) -> bool {
        Self::is_v2_component_member_decorator(name)
            || matches!(
                name,
                ArkuiDecoratorName::BUILDER_PARAM | ArkuiDecoratorName::ENV
            )
    }

    fn is_method_decorator(name: &str) -> bool {
        matches!(
            name,
            ArkuiDecoratorName::BUILDER
                | ArkuiDecoratorName::LOCAL_BUILDER
                | ArkuiDecoratorName::STYLES
                | ArkuiDecoratorName::MONITOR
                | ArkuiDecoratorName::COMPUTED
                | ArkuiDecoratorName::SYNC_MONITOR
        )
    }

    fn is_lifecycle_decorator(name: &str) -> bool {
        matches!(
            name,
            ArkuiDecoratorName::COMPONENT_INIT
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

    fn is_warning_lifecycle_decorator(name: &str) -> bool {
        matches!(
            name,
            ArkuiDecoratorName::COMPONENT_INIT
                | ArkuiDecoratorName::COMPONENT_APPEAR
                | ArkuiDecoratorName::COMPONENT_BUILT
                | ArkuiDecoratorName::COMPONENT_REUSE
                | ArkuiDecoratorName::COMPONENT_RECYCLE
                | ArkuiDecoratorName::COMPONENT_DISAPPEAR
        )
    }

    fn has_invalid_multiple_decorators(decorators: &[&str]) -> bool {
        if decorators.len() <= 1 {
            return false;
        }
        decorators.len() != 2
            || !decorators.contains(&ArkuiDecoratorName::REQUIRE)
            || !decorators.iter().any(|decorator| {
                matches!(
                    *decorator,
                    ArkuiDecoratorName::PROP
                        | ArkuiDecoratorName::BUILDER_PARAM
                        | ArkuiDecoratorName::STATE
                        | ArkuiDecoratorName::PROVIDE
                        | ArkuiDecoratorName::WATCH
                )
            })
    }

    fn counts_as_state_management_decorator(name: &str) -> bool {
        Self::is_member_decorator(name) && name != ArkuiDecoratorName::WATCH
    }

    fn requires_default_value(name: &str) -> bool {
        matches!(
            name,
            ArkuiDecoratorName::STATE
                | ArkuiDecoratorName::PROVIDE
                | ArkuiDecoratorName::STORAGE_PROP
                | ArkuiDecoratorName::STORAGE_LINK
                | ArkuiDecoratorName::LOCAL_STORAGE_LINK
                | ArkuiDecoratorName::LOCAL_STORAGE_PROP
        )
    }

    fn require_releases_default(name: &str) -> bool {
        matches!(
            name,
            ArkuiDecoratorName::PROP
                | ArkuiDecoratorName::BUILDER_PARAM
                | ArkuiDecoratorName::STATE
                | ArkuiDecoratorName::PROVIDE
        )
    }

    fn forbids_default_value(name: &str) -> bool {
        matches!(
            name,
            ArkuiDecoratorName::LINK | ArkuiDecoratorName::OBJECT_LINK | ArkuiDecoratorName::ENV
        )
    }

    fn must_initialize_from_parent(name: &str) -> bool {
        matches!(
            name,
            ArkuiDecoratorName::PROP | ArkuiDecoratorName::LINK | ArkuiDecoratorName::OBJECT_LINK
        )
    }

    fn forbids_function_type(name: &str) -> bool {
        matches!(
            name,
            ArkuiDecoratorName::STATE
                | ArkuiDecoratorName::PROP
                | ArkuiDecoratorName::LINK
                | ArkuiDecoratorName::STORAGE_PROP
                | ArkuiDecoratorName::STORAGE_LINK
                | ArkuiDecoratorName::PROVIDE
                | ArkuiDecoratorName::CONSUME
                | ArkuiDecoratorName::LOCAL_STORAGE_LINK
                | ArkuiDecoratorName::LOCAL_STORAGE_PROP
        )
    }

    fn is_function_type(ty: &TSType<'_>) -> bool {
        match ty {
            TSType::TSParenthesizedType(parenthesized) => {
                Self::is_function_type(&parenthesized.type_annotation)
            }
            TSType::TSFunctionType(_) => true,
            TSType::TSTypeReference(reference) => matches!(
                &reference.type_name,
                TSTypeName::IdentifierReference(identifier) if identifier.name == "Function"
            ),
            _ => false,
        }
    }

    fn is_non_builder_literal(expression: &Expression<'_>) -> bool {
        matches!(
            expression,
            Expression::StringLiteral(_)
                | Expression::NumericLiteral(_)
                | Expression::BooleanLiteral(_)
                | Expression::NullLiteral(_)
                | Expression::ObjectExpression(_)
                | Expression::ArrayExpression(_)
        )
    }

    fn is_object_link_forbidden_type(ty: &TSType<'_>) -> bool {
        match ty {
            TSType::TSUnionType(union) => union
                .types
                .iter()
                .any(|member| Self::is_object_link_forbidden_type(member)),
            TSType::TSParenthesizedType(parenthesized) => {
                Self::is_object_link_forbidden_type(&parenthesized.type_annotation)
            }
            TSType::TSFunctionType(_) => true,
            TSType::TSTypeReference(reference) => matches!(
                &reference.type_name,
                TSTypeName::IdentifierReference(identifier)
                    if identifier.name == "Function"
            ),
            TSType::TSAnyKeyword(_)
            | TSType::TSBigIntKeyword(_)
            | TSType::TSBooleanKeyword(_)
            | TSType::TSIntrinsicKeyword(_)
            | TSType::TSNeverKeyword(_)
            | TSType::TSNullKeyword(_)
            | TSType::TSNumberKeyword(_)
            | TSType::TSStringKeyword(_)
            | TSType::TSSymbolKeyword(_)
            | TSType::TSUndefinedKeyword(_)
            | TSType::TSUnknownKeyword(_)
            | TSType::TSVoidKeyword(_) => true,
            _ => false,
        }
    }

    fn error(&mut self, code: &str, message: impl Into<String>, span: Span, solutions: &[&str]) {
        self.coded_diagnostic(code, message, DiagnosticSeverity::Error, span, solutions);
    }

    fn warning(&mut self, message: impl Into<String>, span: Span) {
        self.errors.push(EtsTransformError::Diagnostic {
            file: self.file.to_string(),
            message: message.into(),
            severity: DiagnosticSeverity::Warning,
        });
        let _ = span;
    }

    fn warning_with_code(&mut self, code: &str, message: impl Into<String>, span: Span) {
        self.coded_diagnostic(code, message, DiagnosticSeverity::Warning, span, &[]);
    }

    fn coded_diagnostic(
        &mut self,
        code: &str,
        message: impl Into<String>,
        severity: DiagnosticSeverity,
        span: Span,
        solutions: &[&str],
    ) {
        self.errors.push(EtsTransformError::CodedDiagnostic {
            file: self.file.to_string(),
            code: code.to_string(),
            message: message.into(),
            severity,
            span: Some(span),
            solutions: solutions
                .iter()
                .map(|solution| (*solution).to_string())
                .collect(),
        });
    }
}
