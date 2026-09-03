use oxc_ast::ast::{
    Argument, Expression, MethodDefinitionKind, ObjectPropertyKind, PropertyKey, Statement,
    StructElement, StructStatement, TSType, TSTypeName,
};
use oxc_span::{GetSpan, Span};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::error::{DiagnosticSeverity, EtsTransformError};
use crate::transform::component_meta::ComponentMetadata;
use crate::transform::decorator::{ArkuiDecorator, ArkuiDecoratorName};
use crate::transform::viewpu_emit::GeneratedViewpuName;

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
            self.check_ui_statement(statement);
        }
    }

    fn check_ui_statement(&mut self, statement: &Statement<'_>) {
        match statement {
            Statement::ExpressionStatement(expression) => {
                if let Expression::ArkUIComponentExpression(component) = &expression.expression {
                    self.check_ui_component_children(component);
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
                self.check_ui_statement(&statement.consequent);
                if let Some(alternate) = statement.alternate.as_ref() {
                    self.check_ui_statement(alternate);
                }
            }
            Statement::BlockStatement(block) => {
                for statement in &block.body {
                    self.check_ui_statement(statement);
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
    ) {
        for child in &component.children {
            match child {
                oxc_ast::ast::ArkUIChild::Component(component) => {
                    self.check_ui_component_children(component);
                }
                oxc_ast::ast::ArkUIChild::Statement(statement) => {
                    self.check_ui_statement(statement);
                }
                oxc_ast::ast::ArkUIChild::Expression(_) => {}
            }
        }
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

            if !component_v2 {
                self.check_custom_decorator_conflict(property, &decorators);
                self.check_forbidden_state_type(property, name, &decorators);
            }

            for decorator in &property.decorators {
                let decorator_name = ArkuiDecorator::new(decorator).name();
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

            if property.type_annotation.is_none() {
                self.error(
                    "10905305",
                    format!("The property '{name}' must specify a type."),
                    property.span,
                    &[],
                );
            }

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

            let has_require = decorators.contains(&ArkuiDecoratorName::REQUIRE);
            for decorator in &decorators {
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
                if Self::forbids_default_value(decorator) && property.value.is_some() {
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
