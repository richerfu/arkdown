use crate::text::StaticPropertyNameText;
use oxc_ast::ast::{
    Argument, ArkUIChild, ArkUIComponentExpression, ArrowFunctionBody, ChainElement, Expression,
    FunctionBody, MethodDefinitionKind, ObjectPropertyKind, PropertyKey, Statement, StructElement,
    StructStatement, TSNonNullExpression, TSType, TSTypeName,
};
use oxc_span::{GetSpan, Span};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::error::{DiagnosticSeverity, EtsTransformError};
use crate::transform::component_meta::ComponentMetadata;
use crate::transform::decorator::{ArkuiDecorator, ArkuiDecoratorName};
use crate::transform::viewpu_emit::GeneratedViewpuName;
use crate::transform::{
    ArktsFileTypeFacts, CustomComponentGeneration, CustomComponentInfo,
    CustomComponentPropertyKind, CustomComponentUpdateProps, ImportedClassObservations,
};

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
    custom_component_info: Option<&'statement CustomComponentUpdateProps>,
    type_facts: Option<&'statement ArktsFileTypeFacts>,
    parent_v1_property_kinds: FxHashMap<String, CustomComponentPropertyKind>,
    parent_env_props: FxHashSet<String>,
    parent_computed_props: FxHashSet<String>,
}

impl<'statement, 'errors> Arkts11StructValidator<'statement, 'errors> {
    pub(super) fn new(
        statement: &'statement StructStatement<'statement>,
        file: &'errors str,
        errors: &'errors mut Vec<EtsTransformError>,
    ) -> Self {
        let (parent_v1_property_kinds, parent_env_props, parent_computed_props) =
            Self::parent_member_facts(statement);
        Self {
            statement,
            file,
            errors,
            exported: false,
            component_metadata: None,
            component_reuse: None,
            class_observations: None,
            custom_component_info: None,
            type_facts: None,
            parent_v1_property_kinds,
            parent_env_props,
            parent_computed_props,
        }
    }

    fn parent_member_facts(
        statement: &StructStatement<'_>,
    ) -> (
        FxHashMap<String, CustomComponentPropertyKind>,
        FxHashSet<String>,
        FxHashSet<String>,
    ) {
        let mut property_kinds = FxHashMap::default();
        let mut env_props = FxHashSet::default();
        let mut computed_props = FxHashSet::default();
        for element in &statement.body.body {
            match element {
                StructElement::PropertyDefinition(property) => {
                    let Some(name) = property.key.static_name() else {
                        continue;
                    };
                    let mut kind = None;
                    for decorator in &property.decorators {
                        let decorator = ArkuiDecorator::new(decorator).name();
                        if decorator == ArkuiDecoratorName::ENV {
                            env_props.insert(name.to_owned_text());
                        }
                        kind = kind.or_else(|| Self::component_property_kind(decorator));
                    }
                    property_kinds.insert(
                        name.to_owned_text(),
                        kind.unwrap_or(CustomComponentPropertyKind::Regular),
                    );
                }
                StructElement::MethodDefinition(method)
                    if method.kind == MethodDefinitionKind::Get
                        && method.decorators.iter().any(|decorator| {
                            ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::COMPUTED
                        }) =>
                {
                    if let Some(name) = method.key.static_name() {
                        computed_props.insert(name.to_owned_text());
                    }
                }
                _ => {}
            }
        }
        (property_kinds, env_props, computed_props)
    }

    fn component_property_kind(decorator: &str) -> Option<CustomComponentPropertyKind> {
        match decorator {
            ArkuiDecoratorName::STATE => Some(CustomComponentPropertyKind::State),
            ArkuiDecoratorName::PROP => Some(CustomComponentPropertyKind::Prop),
            ArkuiDecoratorName::LINK => Some(CustomComponentPropertyKind::Link),
            ArkuiDecoratorName::PROVIDE => Some(CustomComponentPropertyKind::Provide),
            ArkuiDecoratorName::OBJECT_LINK => Some(CustomComponentPropertyKind::ObjectLink),
            ArkuiDecoratorName::STORAGE_LINK => Some(CustomComponentPropertyKind::StorageLink),
            ArkuiDecoratorName::STORAGE_PROP => Some(CustomComponentPropertyKind::StorageProp),
            ArkuiDecoratorName::LOCAL_STORAGE_LINK => {
                Some(CustomComponentPropertyKind::LocalStorageLink)
            }
            ArkuiDecoratorName::LOCAL_STORAGE_PROP => {
                Some(CustomComponentPropertyKind::LocalStorageProp)
            }
            ArkuiDecoratorName::CONSUME => Some(CustomComponentPropertyKind::Consume),
            ArkuiDecoratorName::BUILDER_PARAM => Some(CustomComponentPropertyKind::BuilderParam),
            ArkuiDecoratorName::ENV => Some(CustomComponentPropertyKind::Env),
            _ => None,
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

    pub(super) fn with_custom_component_info(
        mut self,
        custom_component_info: Option<&'statement CustomComponentUpdateProps>,
    ) -> Self {
        self.custom_component_info = custom_component_info;
        self
    }

    pub(super) fn with_type_facts(
        mut self,
        type_facts: Option<&'statement ArktsFileTypeFacts>,
    ) -> Self {
        self.type_facts = type_facts;
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
        self.check_pre_transform_property_types();
        self.check_methods();
        self.check_properties();
        self.check_custom_dialog_controller();
    }

    fn check_pre_transform_property_types(&mut self) {
        if self.has_struct_decorator(ArkuiDecoratorName::COMPONENT_V2) {
            return;
        }
        for element in &self.statement.body.body {
            let StructElement::PropertyDefinition(property) = element else {
                continue;
            };
            let decorators = property
                .decorators
                .iter()
                .map(|decorator| ArkuiDecorator::new(decorator).name())
                .collect::<Vec<_>>();
            self.check_observed_v2_property_type(property, &decorators);
        }
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
        if !matches!(
            reuse_pool.value.as_str(),
            Some("shared") | Some("perInstance")
        ) {
            self.error(
                "10905377",
                format!(
                    "ReusePool must be either 'shared' or 'perInstance'. The value '{}' is not valid.",
                    reuse_pool.value.display()
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
            if method.kind == MethodDefinitionKind::Set
                && method_name.is_some_and(|name| self.parent_computed_props.contains(name))
            {
                self.error(
                    "10905130",
                    "A property decorated by '@Computed' cannot define a set method.",
                    method.span,
                    &[],
                );
            }
            let decorators = method
                .decorators
                .iter()
                .map(|decorator| ArkuiDecorator::new(decorator).name())
                .filter(|name| Self::is_method_decorator(name))
                .collect::<Vec<_>>();
            if decorators.contains(&ArkuiDecoratorName::LOCAL_BUILDER)
                && method.decorators.len() > 1
            {
                self.error(
                    "10905125",
                    "The member property or method can not be decorated by multiple decorators.",
                    method.span,
                    &[],
                );
            }
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
            let distinct = decorators
                .iter()
                .copied()
                .filter(|decorator| *decorator != ArkuiDecoratorName::LOCAL_BUILDER)
                .collect::<FxHashSet<_>>();
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
                } else if let Expression::CallExpression(call) = &expression.expression {
                    self.check_custom_component_call(call);
                } else if let Expression::StaticMemberExpression(member) = &expression.expression {
                    self.error(
                        "10905206",
                        format!(
                            "'{}' does not meet UI component syntax.",
                            Self::static_member_source_name(member)
                        ),
                        expression.span,
                        &[],
                    );
                } else if !matches!(
                    expression.expression,
                    Expression::ChainExpression(_) | Expression::TSNonNullExpression(_)
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

    fn static_member_source_name(member: &oxc_ast::ast::StaticMemberExpression<'_>) -> String {
        let object = match &member.object {
            Expression::ThisExpression(_) => "this".to_string(),
            Expression::Identifier(identifier) => identifier.name.to_string(),
            Expression::StaticMemberExpression(parent) => Self::static_member_source_name(parent),
            _ => "<expression>".to_string(),
        };
        format!("{object}.{}", member.property.name)
    }

    fn check_ui_component_children(
        &mut self,
        component: &oxc_ast::ast::ArkUIComponentExpression<'_>,
        parent: Option<&str>,
    ) {
        self.check_component_parent_contract(component, parent);
        self.check_component_child_contract(component);
        let component_name = Self::component_expression_name(component);
        if let Some(component_name) = component_name {
            self.check_custom_component_arguments(
                component_name,
                &component.arguments,
                component.span,
            );
            self.check_custom_component_reuse_usage(component_name, component);
        }
        if component_name == Some("Repeat") {
            self.check_repeat_template_reusable_v2(component);
        }
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

    fn check_repeat_template_reusable_v2(&mut self, component: &ArkUIComponentExpression<'_>) {
        for chain in &component.chain_expressions {
            let Expression::StaticMemberExpression(member) = &chain.callee else {
                continue;
            };
            if member.property.name != "template" {
                continue;
            }
            for argument in &chain.arguments {
                if let Some(expression) = argument.as_expression() {
                    self.check_repeat_template_expression(expression);
                }
            }
        }
    }

    fn check_repeat_template_statement(&mut self, statement: &Statement<'_>) {
        match statement {
            Statement::ExpressionStatement(statement) => {
                self.check_repeat_template_expression(&statement.expression);
            }
            Statement::BlockStatement(block) => {
                for statement in &block.body {
                    self.check_repeat_template_statement(statement);
                }
            }
            Statement::IfStatement(statement) => {
                self.check_repeat_template_statement(&statement.consequent);
                if let Some(alternate) = statement.alternate.as_ref() {
                    self.check_repeat_template_statement(alternate);
                }
            }
            _ => {}
        }
    }

    fn check_repeat_template_expression(&mut self, expression: &Expression<'_>) {
        match expression {
            Expression::ArrowFunctionExpression(arrow) => match &arrow.body {
                ArrowFunctionBody::FunctionBody(body) => {
                    for statement in &body.statements {
                        self.check_repeat_template_statement(statement);
                    }
                }
                body => {
                    if let Some(expression) = body.as_expression() {
                        self.check_repeat_template_expression(expression);
                    }
                }
            },
            Expression::ArkUIComponentExpression(component) => {
                self.check_repeat_template_component(component);
            }
            Expression::CallExpression(call) => {
                if let Some((name, _, span)) = Self::custom_component_base_call(call) {
                    self.report_reusable_v2_in_repeat_template(name, span);
                }
                for argument in &call.arguments {
                    if let Some(expression) = argument.as_expression() {
                        self.check_repeat_template_expression(expression);
                    }
                }
            }
            Expression::ParenthesizedExpression(parenthesized) => {
                self.check_repeat_template_expression(&parenthesized.expression);
            }
            _ => {}
        }
    }

    fn check_repeat_template_component(&mut self, component: &ArkUIComponentExpression<'_>) {
        if let Some(name) = Self::component_expression_name(component) {
            self.report_reusable_v2_in_repeat_template(name, component.span);
        }
        for argument in &component.arguments {
            if let Some(expression) = argument.as_expression() {
                self.check_repeat_template_expression(expression);
            }
        }
        for chain in &component.chain_expressions {
            for argument in &chain.arguments {
                if let Some(expression) = argument.as_expression() {
                    self.check_repeat_template_expression(expression);
                }
            }
        }
        for child in &component.children {
            match child {
                ArkUIChild::Component(component) => self.check_repeat_template_component(component),
                ArkUIChild::Statement(statement) => {
                    self.check_repeat_template_statement(statement);
                }
                ArkUIChild::Expression(expression) => {
                    self.check_repeat_template_expression(expression);
                }
            }
        }
    }

    fn report_reusable_v2_in_repeat_template(&mut self, component_name: &str, span: Span) {
        if !self
            .custom_component_info
            .and_then(|components| components.get(component_name))
            .is_some_and(|info| info.reusable_v2)
        {
            return;
        }
        self.error(
            "10905247",
            "The template attribute of the Repeat component cannot contain any custom component decorated with '@ReusableV2'.",
            span,
            &[],
        );
    }

    fn check_custom_component_reuse_usage(
        &mut self,
        component_name: &str,
        component: &ArkUIComponentExpression<'_>,
    ) {
        let attributes = component
            .chain_expressions
            .iter()
            .filter_map(|chain| {
                let Expression::StaticMemberExpression(member) = &chain.callee else {
                    return None;
                };
                Some((member.property.name.as_str(), chain.span))
            })
            .collect::<Vec<_>>();
        self.check_custom_component_reuse(component_name, component.span, attributes.as_slice());
    }

    fn check_custom_component_reuse(
        &mut self,
        component_name: &str,
        span: Span,
        attributes: &[(&str, Span)],
    ) {
        let Some(info) = self
            .custom_component_info
            .and_then(|components| components.get(component_name))
            .cloned()
        else {
            return;
        };
        let parent_v2 = self.has_struct_decorator(ArkuiDecoratorName::COMPONENT_V2);
        let parent_reusable = self.has_struct_decorator(ArkuiDecoratorName::REUSABLE);
        let parent_reusable_v2 = self.has_struct_decorator(ArkuiDecoratorName::REUSABLE_V2);
        if !parent_v2 && !parent_reusable && info.reusable_v2 {
            self.error(
                "10905244",
                "A custom component decorated with '@Component' cannot contain child components decorated with '@ReusableV2'.",
                span,
                &[],
            );
        } else if !parent_v2 && parent_reusable && info.reusable_v2 {
            self.error(
                "10905245",
                "A custom component decorated with '@Reusable' cannot contain child components decorated with '@ReusableV2'.",
                span,
                &[],
            );
        } else if parent_reusable_v2 && info.reusable {
            self.error(
                "10905246",
                "A custom component decorated with '@ReusableV2' cannot contain child components decorated with '@Reusable'.",
                span,
                &[],
            );
        } else if parent_v2 && !parent_reusable_v2 && info.reusable {
            self.warning(
                "When a custom component is decorated with '@ComponentV2' and contains a child decorated with '@Reusable', the child component will not create.",
                span,
            );
        }

        let reusable_v2 = info.generation == CustomComponentGeneration::V2 && info.reusable_v2;
        for (attribute, span) in attributes {
            match *attribute {
                "reuse" if !reusable_v2 => self.error(
                    "10905248",
                    "The reuse attribute is only applicable to custom components decorated with both '@ComponentV2' and '@ReusableV2'.",
                    *span,
                    &[],
                ),
                "reuseId" if reusable_v2 => self.error(
                    "10905249",
                    "The reuseId attribute is not applicable to custom components decorated with both '@ComponentV2' and '@ReusableV2'.",
                    *span,
                    &[],
                ),
                _ => {}
            }
        }
    }

    fn check_custom_component_call(&mut self, call: &oxc_ast::ast::CallExpression<'_>) {
        let Some((name, arguments, span)) = Self::custom_component_base_call(call) else {
            return;
        };
        if name == "Repeat" {
            self.check_repeat_template_call(call);
        }
        self.check_custom_component_arguments(name, arguments, span);
        let attributes = Self::custom_component_call_attributes(call);
        self.check_custom_component_reuse(name, span, attributes.as_slice());
    }

    fn check_repeat_template_call(&mut self, mut call: &oxc_ast::ast::CallExpression<'_>) {
        while let Expression::StaticMemberExpression(member) = &call.callee {
            if member.property.name == "template" {
                for argument in &call.arguments {
                    if let Some(expression) = argument.as_expression() {
                        self.check_repeat_template_expression(expression);
                    }
                }
            }
            let Expression::CallExpression(inner) = &member.object else {
                break;
            };
            call = inner;
        }
    }

    fn custom_component_call_attributes<'node>(
        mut call: &'node oxc_ast::ast::CallExpression<'node>,
    ) -> Vec<(&'node str, Span)> {
        let mut attributes = Vec::new();
        while let Expression::StaticMemberExpression(member) = &call.callee {
            attributes.push((member.property.name.as_str(), call.span));
            let Expression::CallExpression(inner) = &member.object else {
                break;
            };
            call = inner;
        }
        attributes
    }

    fn custom_component_base_call<'node>(
        call: &'node oxc_ast::ast::CallExpression<'node>,
    ) -> Option<(&'node str, &'node [Argument<'node>], Span)> {
        match &call.callee {
            Expression::Identifier(identifier) => Some((
                identifier.name.as_str(),
                call.arguments.as_slice(),
                call.span,
            )),
            Expression::StaticMemberExpression(member) => {
                let Expression::CallExpression(inner) = &member.object else {
                    return None;
                };
                Self::custom_component_base_call(inner)
            }
            _ => None,
        }
    }

    fn check_custom_component_arguments(
        &mut self,
        component_name: &str,
        arguments: &[Argument<'_>],
        span: Span,
    ) {
        let Some(info) = self
            .custom_component_info
            .and_then(|components| components.get(component_name))
            .cloned()
        else {
            return;
        };
        let mut supplied = FxHashSet::default();
        let mut two_way_props = Vec::new();
        if let Some(Argument::ObjectExpression(object)) = arguments.first() {
            for property in &object.properties {
                let ObjectPropertyKind::ObjectProperty(property) = property else {
                    continue;
                };
                let PropertyKey::StaticIdentifier(name) = &property.key else {
                    continue;
                };
                let name = name.name.as_str();
                supplied.insert(name.to_string());
                if self.has_struct_decorator(ArkuiDecoratorName::COMPONENT_V2)
                    && Self::double_non_null_value(&property.value).is_some_and(|value| {
                        matches!(
                            value,
                            Expression::Identifier(_) | Expression::StaticMemberExpression(_)
                        )
                    })
                {
                    two_way_props.push(name.to_string());
                }
                self.check_custom_component_property(
                    component_name,
                    name,
                    &property.value,
                    property.span,
                    &info,
                );
            }
        }

        for property_name in two_way_props {
            let mut event_name = String::with_capacity(property_name.len() + 1);
            event_name.push('$');
            event_name.push_str(&property_name);
            if supplied.contains(&event_name) {
                self.error(
                    "10905358",
                    format!(
                        "When the two-way binding syntax is used, do not assign a value to '@Event' variable '{event_name}' because the framework generates the default assignment."
                    ),
                    span,
                    &[],
                );
            }
        }

        if info.generation == CustomComponentGeneration::V2 {
            for required in &info.required_props {
                if supplied.contains(required) {
                    continue;
                }
                self.error(
                    "10905321",
                    format!(
                        "'@Require' decorated '{required}' must be initialized through the component constructor."
                    ),
                    span,
                    &[],
                );
            }
        } else {
            for required in &info.v1_required_props {
                if supplied.contains(required) {
                    continue;
                }
                self.error(
                    "10905359",
                    format!(
                        "'@Require' decorated '{required}' must be initialized through the component constructor."
                    ),
                    span,
                    &[],
                );
            }
            let parent_v2 = self.has_struct_decorator(ArkuiDecoratorName::COMPONENT_V2);
            for required in &info.v1_mandatory_props {
                if parent_v2 && info.link_props.contains(required) {
                    continue;
                }
                if supplied.contains(required) {
                    continue;
                }
                self.error(
                    "10905316",
                    format!(
                        "The property '{required}' in the custom component '{component_name}' is missing (mandatory to specify)."
                    ),
                    span,
                    &[],
                );
            }
            if parent_v2 && !info.link_props.is_empty() {
                self.error(
                    "10905213",
                    "A V2 component cannot be used with any member property decorated by '@Link' in a V1 component.",
                    span,
                    &[],
                );
            }
        }
    }

    fn check_custom_component_property(
        &mut self,
        component_name: &str,
        property_name: &str,
        initializer: &Expression<'_>,
        span: Span,
        info: &CustomComponentInfo,
    ) {
        self.check_component_env_assignment(property_name, initializer, span, info);
        if info.generation == CustomComponentGeneration::V1 {
            self.check_component_v1_property(
                component_name,
                property_name,
                initializer,
                span,
                info,
            );
            return;
        }
        if info.custom_env_props.contains(property_name) {
            self.error(
                "10905375",
                format!(
                    "The '@CustomEnv' property '{property_name}' in the custom component '{component_name}' cannot be initialized by parent component."
                ),
                span,
                &[],
            );
            return;
        }
        let Some(kind) = info.forbidden_init_props.get(property_name).copied() else {
            self.check_component_v2_assignment(
                component_name,
                property_name,
                initializer,
                span,
                info,
            );
            self.check_component_v2_two_way_binding(
                component_name,
                property_name,
                initializer,
                span,
                info,
            );
            return;
        };
        self.error(
            "10905324",
            format!(
                "The '{}' property '{property_name}' in the custom component '{component_name}' cannot be initialized here (forbidden to specify).",
                kind.source_name()
            ),
            span,
            &[],
        );
    }

    fn check_component_env_assignment(
        &mut self,
        property_name: &str,
        initializer: &Expression<'_>,
        span: Span,
        info: &CustomComponentInfo,
    ) {
        let Some(initializer_name) = Self::parent_initializer_name(initializer) else {
            return;
        };
        if !self.parent_env_props.contains(initializer_name) {
            return;
        }
        if info.generation == CustomComponentGeneration::V2 {
            if !info.param_props.contains(property_name)
                || info.builder_params.contains(property_name)
            {
                self.error(
                    "10905252",
                    "Within structs decorated with '@ComponentV2', '@Env' can only initialize variables decorated with '@Param'.",
                    span,
                    &[],
                );
            }
        } else if !info.regular_props.contains(property_name) {
            self.error(
                "10905253",
                "Within structs decorated with '@Component', '@Env' can only initialize regular(non-decorated) variables.",
                span,
                &[],
            );
        }
    }

    fn parent_initializer_name<'node>(initializer: &'node Expression<'_>) -> Option<&'node str> {
        match initializer {
            Expression::StaticMemberExpression(member)
                if matches!(&member.object, Expression::ThisExpression(_)) =>
            {
                Some(member.property.name.as_str())
            }
            Expression::Identifier(identifier) => identifier.name.strip_prefix('$'),
            _ => None,
        }
    }

    fn check_component_v1_property(
        &mut self,
        component_name: &str,
        property_name: &str,
        initializer: &Expression<'_>,
        span: Span,
        info: &CustomComponentInfo,
    ) {
        if info.custom_env_props.contains(property_name) {
            self.error(
                "10905375",
                format!(
                    "The '@CustomEnv' property '{property_name}' in the custom component '{component_name}' cannot be initialized by parent component."
                ),
                span,
                &[],
            );
            return;
        }
        if let Some(kind) = info.v1_forbidden_init_props.get(property_name).copied() {
            self.error(
                "10905317",
                format!(
                    "The '{}' property '{property_name}' in the custom component '{component_name}' cannot be initialized here (forbidden to specify).",
                    kind.source_name()
                ),
                span,
                &[],
            );
            return;
        }
        let Some(child_kind) = info.v1_property_kinds.get(property_name).copied() else {
            return;
        };
        if child_kind != CustomComponentPropertyKind::Link
            && matches!(initializer, Expression::Identifier(identifier) if identifier.name.starts_with('$'))
        {
            self.error(
                "10905314",
                format!(
                    "Property '{property_name}' cannot initialize using '$' to create a reference to a variable."
                ),
                initializer.span(),
                &[],
            );
        }
        if !self.has_struct_decorator(ArkuiDecoratorName::COMPONENT_V2) {
            // process_custom_component.ts::checkFromParentToChild accepts a
            // local identifier for @ObjectLink before applying the ordinary
            // non-decorator assignment matrix. Resolved type validity is
            // checked separately from tsrs facts on the child declaration.
            let accepted_local_object_link = child_kind == CustomComponentPropertyKind::ObjectLink
                && matches!(initializer, Expression::Identifier(identifier) if !identifier.name.starts_with('$'));
            if !accepted_local_object_link {
                if let Some((parent_name, parent_kind)) =
                    self.parent_property_reference(initializer)
                {
                    if !Self::v1_assignment_is_allowed(parent_kind, child_kind) {
                        self.error(
                            "10905315",
                            format!(
                                "The '{}' property '{parent_name}' cannot be assigned to the '{}' property '{property_name}'.",
                                parent_kind.source_name(),
                                child_kind.source_name()
                            ),
                            initializer.span(),
                            &[],
                        );
                    }
                }
            }
        }
        if child_kind == CustomComponentPropertyKind::Link
            && Self::link_source_nesting(initializer) >= 2
        {
            self.error(
                "10905364",
                format!(
                    "The type of the parent component's state variable initializing the '@Link' variable '{property_name}' must match the '@Link' variable's declared type."
                ),
                span,
                &[],
            );
        }
    }

    fn parent_property_reference(
        &self,
        initializer: &Expression<'_>,
    ) -> Option<(String, CustomComponentPropertyKind)> {
        match initializer {
            Expression::StaticMemberExpression(member)
                if matches!(&member.object, Expression::ThisExpression(_)) =>
            {
                let name = member.property.name.as_str();
                self.parent_v1_property_kinds
                    .get(name)
                    .copied()
                    .map(|kind| (name.to_string(), kind))
            }
            Expression::Identifier(identifier) => {
                if let Some(name) = identifier.name.strip_prefix('$') {
                    self.parent_v1_property_kinds
                        .get(name)
                        .copied()
                        .map(|kind| (name.to_string(), kind))
                } else {
                    Some((
                        identifier.name.to_string(),
                        CustomComponentPropertyKind::Regular,
                    ))
                }
            }
            _ => None,
        }
    }

    fn v1_assignment_is_allowed(
        parent: CustomComponentPropertyKind,
        child: CustomComponentPropertyKind,
    ) -> bool {
        match child {
            CustomComponentPropertyKind::State
            | CustomComponentPropertyKind::Prop
            | CustomComponentPropertyKind::Provide => true,
            CustomComponentPropertyKind::Regular => matches!(
                parent,
                CustomComponentPropertyKind::Regular
                    | CustomComponentPropertyKind::State
                    | CustomComponentPropertyKind::Link
                    | CustomComponentPropertyKind::Prop
                    | CustomComponentPropertyKind::ObjectLink
                    | CustomComponentPropertyKind::StorageLink
                    | CustomComponentPropertyKind::Env
            ),
            CustomComponentPropertyKind::Link | CustomComponentPropertyKind::ObjectLink => {
                parent != CustomComponentPropertyKind::Regular
            }
            _ => true,
        }
    }

    fn link_source_nesting(initializer: &Expression<'_>) -> usize {
        let mut current = initializer;
        let mut nesting = 0;
        loop {
            match current {
                Expression::StaticMemberExpression(member) => {
                    nesting += 1;
                    current = &member.object;
                }
                Expression::ComputedMemberExpression(member) => {
                    nesting += 1;
                    current = &member.object;
                }
                Expression::TSNonNullExpression(non_null) => current = &non_null.expression,
                Expression::ThisExpression(_) => return nesting,
                _ => return 0,
            }
        }
    }

    fn check_component_v2_two_way_binding(
        &mut self,
        component_name: &str,
        property_name: &str,
        initializer: &Expression<'_>,
        span: Span,
        info: &CustomComponentInfo,
    ) {
        if info.generation != CustomComponentGeneration::V2 {
            return;
        }
        let Some(value) = Self::double_non_null_value(initializer) else {
            return;
        };
        if !matches!(
            value,
            Expression::Identifier(_) | Expression::StaticMemberExpression(_)
        ) {
            self.error(
                "10905318",
                format!(
                    "When the two-way binding syntax is used, the initial value of property '{property_name}' must be a variable."
                ),
                span,
                &[],
            );
            return;
        }
        if Self::contains_optional_chain(value) {
            self.error(
                "10905320",
                format!(
                    "The optional character can not be used in the initial value of property '{property_name}'."
                ),
                span,
                &[],
            );
            return;
        }
        if self.has_struct_decorator(ArkuiDecoratorName::COMPONENT_V2)
            && matches!(
                value,
                Expression::StaticMemberExpression(member)
                    if matches!(&member.object, Expression::ThisExpression(_))
                        && self.parent_computed_props.contains(member.property.name.as_str())
            )
        {
            self.error(
                "10905129",
                "A property decorated by '@Computed' cannot be used with two-bind syntax.",
                value.span(),
                &[],
            );
        }
        let mut event_name = String::with_capacity(property_name.len() + 1);
        event_name.push('$');
        event_name.push_str(property_name);
        if info.param_props.contains(property_name) && info.event_props.contains(&event_name) {
            return;
        }
        self.error(
            "10905319",
            format!(
                "When the two-way binding syntax is used, the variable '{property_name}' must be decorated with '@Param', and the '@Event' variable '${property_name}' must be defined in the {component_name}."
            ),
            span,
            &[],
        );
    }

    fn double_non_null_value<'node, 'ast>(
        expression: &'node Expression<'ast>,
    ) -> Option<&'node Expression<'ast>> {
        match expression {
            Expression::TSNonNullExpression(outer) => Self::double_non_null_inner(outer),
            Expression::ChainExpression(chain) => match &chain.expression {
                ChainElement::TSNonNullExpression(outer) => Self::double_non_null_inner(outer),
                _ => None,
            },
            _ => None,
        }
    }

    fn double_non_null_inner<'node, 'ast>(
        outer: &'node TSNonNullExpression<'ast>,
    ) -> Option<&'node Expression<'ast>> {
        let Expression::TSNonNullExpression(inner) = &outer.expression else {
            return None;
        };
        (!matches!(inner.expression, Expression::TSNonNullExpression(_)))
            .then_some(&inner.expression)
    }

    fn contains_optional_chain(expression: &Expression<'_>) -> bool {
        match expression {
            Expression::ChainExpression(_) => true,
            Expression::StaticMemberExpression(member) => {
                member.optional || Self::contains_optional_chain(&member.object)
            }
            Expression::ComputedMemberExpression(member) => {
                member.optional
                    || Self::contains_optional_chain(&member.object)
                    || Self::contains_optional_chain(&member.expression)
            }
            Expression::CallExpression(call) => {
                call.optional || Self::contains_optional_chain(&call.callee)
            }
            Expression::ParenthesizedExpression(parenthesized) => {
                Self::contains_optional_chain(&parenthesized.expression)
            }
            _ => false,
        }
    }

    fn check_component_v2_assignment(
        &mut self,
        component_name: &str,
        property_name: &str,
        initializer: &Expression<'_>,
        span: Span,
        info: &CustomComponentInfo,
    ) {
        if self.has_struct_decorator(ArkuiDecoratorName::COMPONENT_V2)
            || info.generation != CustomComponentGeneration::V2
            || !info.update_props.contains(property_name)
        {
            return;
        }
        let Expression::StaticMemberExpression(member) = initializer else {
            return;
        };
        if !matches!(&member.object, Expression::ThisExpression(_)) {
            return;
        }
        let parent_property_name = member.property.name.as_str();
        let Some(parent_property) = self.parent_update_property(parent_property_name) else {
            return;
        };
        let Some(annotation) = parent_property.type_annotation.as_ref() else {
            return;
        };
        if !self.is_forbidden_component_v2_assignment_type(&annotation.type_annotation) {
            return;
        }
        self.error(
            "10905323",
            format!(
                "Property '{property_name}' in the '@ComponentV2' component '{component_name}' is not allowed to be assigned value here."
            ),
            span,
            &[],
        );
    }

    fn parent_update_property(&self, name: &str) -> Option<&oxc_ast::ast::PropertyDefinition<'_>> {
        self.statement.body.body.iter().find_map(|element| {
            let StructElement::PropertyDefinition(property) = element else {
                return None;
            };
            if !property
                .key
                .static_name()
                .is_some_and(|key| key.as_str() == Some(name))
                || !property.decorators.iter().any(|decorator| {
                    Self::is_v1_update_source_decorator(ArkuiDecorator::new(decorator).name())
                })
            {
                return None;
            }
            Some(property.as_ref())
        })
    }

    fn is_forbidden_component_v2_assignment_type(&self, ty: &TSType<'_>) -> bool {
        match ty {
            TSType::TSParenthesizedType(parenthesized) => {
                self.is_forbidden_component_v2_assignment_type(&parenthesized.type_annotation)
            }
            TSType::TSUnionType(union) => {
                if union.types.iter().any(Self::is_basic_assignment_type) {
                    return false;
                }
                union.types.iter().all(Self::is_function_assignment_type)
            }
            _ if Self::is_basic_assignment_type(ty) => false,
            TSType::TSTypeReference(reference)
                if matches!(
                    &reference.type_name,
                    TSTypeName::IdentifierReference(identifier)
                        if matches!(identifier.name.as_str(), "Set" | "Map" | "Date" | "Array")
                ) =>
            {
                false
            }
            _ => Self::is_function_assignment_type(ty) || self.type_contains_observed_v2(ty),
        }
    }

    fn is_basic_assignment_type(ty: &TSType<'_>) -> bool {
        matches!(
            ty,
            TSType::TSStringKeyword(_)
                | TSType::TSNumberKeyword(_)
                | TSType::TSBooleanKeyword(_)
                | TSType::TSBigIntKeyword(_)
                | TSType::TSNullKeyword(_)
                | TSType::TSUndefinedKeyword(_)
                | TSType::TSLiteralType(_)
        )
    }

    fn is_function_assignment_type(ty: &TSType<'_>) -> bool {
        match ty {
            TSType::TSParenthesizedType(parenthesized) => {
                Self::is_function_assignment_type(&parenthesized.type_annotation)
            }
            TSType::TSUnionType(union) => union.types.iter().all(Self::is_function_assignment_type),
            TSType::TSFunctionType(_) => true,
            TSType::TSTypeReference(reference) => matches!(
                &reference.type_name,
                TSTypeName::IdentifierReference(identifier) if identifier.name == "Function"
            ),
            _ => false,
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
                Expression::StringLiteral(literal) => {
                    Some((literal.value.as_str()).unwrap_or_default())
                }
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

            if decorators.contains(&ArkuiDecoratorName::LOCAL_BUILDER)
                && property.decorators.len() > 1
            {
                self.error(
                    "10905125",
                    "The member property or method can not be decorated by multiple decorators.",
                    property.span,
                    &[],
                );
            }

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
            self.check_env_decorator_contract(property, &decorators);

            if !component_v2 {
                self.check_custom_decorator_conflict(property, &decorators);
                self.check_forbidden_state_type(property, name, &decorators);
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
                    let (code, message) = if component_v2 {
                        (
                            "10905107",
                            "'@BuilderParam' property can only initialized by '@Builder' function.",
                        )
                    } else {
                        (
                            "10905101",
                            "'@BuilderParam' property can only initialized by '@Builder' function or '@LocalBuilder' method in struct.",
                        )
                    };
                    self.error(code, message, property.span, &[]);
                }
                if *decorator == ArkuiDecoratorName::OBJECT_LINK
                    && property.type_annotation.is_some()
                {
                    match self.object_link_type_is_valid(property.key.span()) {
                        Some(true) => {}
                        Some(false) => self.error(
                            "10905307",
                            "'@ObjectLink' cannot be used with this type. Apply it only to classes decorated by '@ObservedV2' or initialized using the return value of 'makeV1Observed'.",
                            property.span,
                            &[],
                        ),
                        None => self.errors.push(EtsTransformError::MissingTypeFacts {
                            file: self.file.to_string(),
                            capability: "@ObjectLink resolved declaration type",
                        }),
                    }
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

    fn check_env_decorator_contract(
        &mut self,
        property: &oxc_ast::ast::PropertyDefinition<'_>,
        decorators: &[&str],
    ) {
        let Some(annotation) = property.type_annotation.as_ref() else {
            return;
        };
        if decorators.contains(&ArkuiDecoratorName::ENV) {
            let Some(type_name) = Self::single_type_name(&annotation.type_annotation) else {
                self.invalid_env_type(property.span);
                return;
            };
            let Some(expected_arguments) = Self::env_expected_arguments(type_name.as_str()) else {
                self.invalid_env_type(property.span);
                return;
            };
            let Some(decorator) = property
                .decorators
                .iter()
                .find(|decorator| ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::ENV)
            else {
                return;
            };
            let Some(argument) = ArkuiDecorator::new(decorator).arg_expression() else {
                return;
            };
            if Self::non_system_env_argument(argument) {
                return;
            }
            let actual = Self::static_member_argument_name(argument);
            if !actual.is_some_and(|actual| expected_arguments.contains(&actual.as_str())) {
                self.error(
                    "10905368",
                    format!(
                        "Invalid parameter. State variables decorated with '@Env' of type '{type_name}' can only accept {}.",
                        expected_arguments.join(", ")
                    ),
                    argument.span(),
                    &[],
                );
            }
        }
        if decorators.contains(&ArkuiDecoratorName::CUSTOM_ENV) {
            self.check_custom_env_key(property, &annotation.type_annotation);
        }
    }

    fn check_custom_env_key(
        &mut self,
        property: &oxc_ast::ast::PropertyDefinition<'_>,
        property_type: &TSType<'_>,
    ) {
        let Some(decorator) = property.decorators.iter().find(|decorator| {
            ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::CUSTOM_ENV
        }) else {
            return;
        };
        let Some(argument) = ArkuiDecorator::new(decorator).arg_expression() else {
            return;
        };
        if matches!(argument, Expression::Identifier(_)) {
            return;
        }
        let Expression::CallExpression(call) = argument else {
            self.invalid_custom_env_key(argument.span());
            return;
        };
        let Expression::StaticMemberExpression(member) = &call.callee else {
            self.invalid_custom_env_key(argument.span());
            return;
        };
        if !matches!(&member.object, Expression::Identifier(identifier) if identifier.name == "CustomEnvKey")
            || member.property.name != "create"
        {
            self.invalid_custom_env_key(argument.span());
            return;
        }
        let Some(type_arguments) = call.type_arguments.as_ref() else {
            self.invalid_custom_env_key(argument.span());
            return;
        };
        if type_arguments.params.is_empty() {
            self.invalid_custom_env_key(argument.span());
            return;
        }
        let property_types = Self::flatten_type_names(property_type);
        let key_types = Self::flatten_type_names(&type_arguments.params[0]);
        if property_types.len() != key_types.len()
            || property_types.iter().any(|name| !key_types.contains(name))
        {
            self.error(
                "10905381",
                "The type of the property decorated with '@CustomEnv' must be consistent with the generic type of the key.",
                argument.span(),
                &[],
            );
        }
    }

    fn invalid_env_type(&mut self, span: Span) {
        self.error(
            "10905251",
            "The '@Env' decorator can only decorate 'WindowSizeLayoutBreakpointInfo', 'SizeInVP', 'Size', 'UIEnvWindowAvoidAreaInfoPX', 'UIEnvWindowAvoidAreaInfoVP' classes or their child classes.",
            span,
            &[],
        );
    }

    fn invalid_custom_env_key(&mut self, span: Span) {
        self.error(
            "10905382",
            "Invalid key for '@CustomEnv', '@CustomEnv' key must be global const and created from CustomEnvKey.create<T>().",
            span,
            &[],
        );
    }

    fn env_expected_arguments(type_name: &str) -> Option<&'static [&'static str]> {
        match type_name {
            "WindowSizeLayoutBreakpointInfo" => Some(&["SystemProperties.BREAK_POINT"]),
            "UIEnvWindowAvoidAreaInfoVP" => Some(&["SystemProperties.WINDOW_AVOID_AREA"]),
            "UIEnvWindowAvoidAreaInfoPX" => Some(&["SystemProperties.WINDOW_AVOID_AREA_PX"]),
            "SizeInVP" => Some(&["SystemProperties.WINDOW_SIZE"]),
            "Size" => Some(&["SystemProperties.WINDOW_SIZE_PX"]),
            "number" => Some(&[
                "SystemProperties.WINDOW_DISPLAY_ID",
                "SystemProperties.WINDOW_SYSTEM_DENSITY",
            ]),
            "boolean" => Some(&[
                "SystemProperties.WINDOW_IS_FOCUSED",
                "SystemProperties.WINDOW_IS_HIGHLIGHTED",
            ]),
            _ => None,
        }
    }

    fn non_system_env_argument(argument: &Expression<'_>) -> bool {
        matches!(
            argument,
            Expression::StaticMemberExpression(member)
                if matches!(
                    &member.object,
                    Expression::Identifier(identifier)
                        if matches!(identifier.name.as_str(), "WritableEnvKey" | "ReadonlyEnvKey")
                )
        )
    }

    fn static_member_argument_name(argument: &Expression<'_>) -> Option<String> {
        let Expression::StaticMemberExpression(member) = argument else {
            return None;
        };
        let Expression::Identifier(object) = &member.object else {
            return None;
        };
        Some(format!("{}.{}", object.name, member.property.name))
    }

    fn single_type_name(ty: &TSType<'_>) -> Option<String> {
        let names = Self::flatten_type_names(ty);
        (names.len() == 1).then(|| names[0].clone())
    }

    fn flatten_type_names(ty: &TSType<'_>) -> Vec<String> {
        match ty {
            TSType::TSUnionType(union) => union
                .types
                .iter()
                .filter_map(Self::single_type_name)
                .collect(),
            TSType::TSParenthesizedType(parenthesized) => {
                Self::flatten_type_names(&parenthesized.type_annotation)
            }
            TSType::TSTypeReference(reference) => match &reference.type_name {
                TSTypeName::IdentifierReference(identifier) => vec![identifier.name.to_string()],
                _ => Vec::new(),
            },
            TSType::TSNumberKeyword(_) => vec!["number".to_string()],
            TSType::TSBooleanKeyword(_) => vec!["boolean".to_string()],
            TSType::TSStringKeyword(_) => vec!["string".to_string()],
            _ => Vec::new(),
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
                if !literal.value.as_str().is_some_and(|value| method_names.contains(value)) {
                    self.error(
                        "10905301",
                        format!(
                            "'@Watch' cannot be used with '{}'. Apply it only to parameters that correspond to existing methods.",
                            literal.raw.as_ref().map_or(literal.value.as_str().unwrap_or_default(), |raw| raw.as_str())
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

    fn is_v1_update_source_decorator(name: &str) -> bool {
        matches!(
            name,
            ArkuiDecoratorName::STATE
                | ArkuiDecoratorName::PROP
                | ArkuiDecoratorName::LINK
                | ArkuiDecoratorName::PROVIDE
                | ArkuiDecoratorName::CONSUME
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
                | Expression::ArrowFunctionExpression(_)
        )
    }

    fn object_link_type_is_valid(&self, name_span: Span) -> Option<bool> {
        self.type_facts?
            .property(name_span)?
            .declared_type
            .as_ref()
            .map(|identity| identity.is_valid_object_link_type())
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
