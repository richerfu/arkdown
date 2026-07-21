//! @ComponentV2 lowering per SDK process_struct_componentV2.js.
//!
//! The SDK keeps V2 member decorators on generated ViewV2 class fields, moves
//! non-static field initializers into the constructor/reuse reset paths, and
//! wires `@Param` state through `initParam`/`updateParam`/`resetParam`.

use rustc_hash::FxHashSet;
use std::sync::Arc;

use oxc_allocator::{Allocator, CloneIn, Vec as OxcVec};
use oxc_ast::ast::{
    AccessorProperty, Argument, ClassElement, Decorator, Expression, FormalParameterKind,
    FormalParameters, FunctionType, MethodDefinition, MethodDefinitionKind, PropertyDefinition,
    PropertyKind, Statement, StaticBlock, StructElement, StructStatement, TSAccessibility,
    TSIndexSignature, TSType, TSTypeAnnotation,
};
use oxc_ast::{AstBuilder, NONE};
use oxc_span::SPAN;

use crate::transform::build_fn::{
    BuildBodyInfo, BuildBodyPlan, CustomComponentUpdateProps, StylesBodyLowerer,
};
use crate::transform::component_analysis::ExtendAttributeIndex;
use crate::transform::component_meta::ArkuiRuntimeNames;
use crate::transform::component_meta::ComponentMetadata;
use crate::transform::decorator::{ArkuiDecorator, ArkuiDecoratorName};
use crate::transform::global_rewrite::GlobalContextCallRewriter;
use crate::transform::viewpu_emit::{
    GeneratedViewpuName, ViewpuAstBuilder, ViewpuMemberAstEmitter,
};

#[derive(Debug, thiserror::Error)]
pub enum ComponentV2Error {
    #[error(
        "@ComponentV2 struct '{name}': unsupported decorator '@{decorator}' on field '{field}'"
    )]
    UnsupportedDecorator {
        name: String,
        decorator: String,
        field: String,
    },

    #[error("@ComponentV2 struct '{name}': missing build() method")]
    MissingBuildMethod { name: String },

    #[error("@ComponentV2 struct '{name}': unsupported ArkUI build DSL: {detail}")]
    UnsupportedBuildDsl { name: String, detail: String },

    #[error("@ComponentV2 struct '{name}': invalid decorators on field '{field}': {message}")]
    InvalidFieldDecorators {
        name: String,
        field: String,
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum V2FieldKind {
    Local,
    Param,
    Once,
    Event,
    Provider,
    Consumer,
    BuilderParam,
    Regular,
}

#[derive(Debug, Clone, Copy)]
enum V2RegularMember<'a> {
    Method(&'a MethodDefinition<'a>),
    StaticBlock(&'a StaticBlock<'a>),
    TSIndexSignature(&'a TSIndexSignature<'a>),
    AccessorProperty(&'a AccessorProperty<'a>),
}

impl<'a> V2RegularMember<'a> {
    fn to_class_element<'b>(&self, allocator: &'b Allocator) -> ClassElement<'b> {
        let ast = AstBuilder::new(allocator);
        match self {
            Self::Method(method) => {
                GlobalContextCallRewriter::new(allocator).method_definition(method)
            }
            Self::StaticBlock(block) => {
                ClassElement::StaticBlock(ast.alloc(block.clone_in(allocator)))
            }
            Self::TSIndexSignature(signature) => {
                ClassElement::TSIndexSignature(ast.alloc(signature.clone_in(allocator)))
            }
            Self::AccessorProperty(property) => {
                let mut property = property.clone_in(allocator);
                property.decorators.clear();
                ClassElement::AccessorProperty(ast.alloc(property))
            }
        }
    }
}

impl V2FieldKind {
    fn from_decorator_names(names: &[&str]) -> Self {
        if names.contains(&ArkuiDecoratorName::BUILDER_PARAM) {
            Self::BuilderParam
        } else if names.contains(&ArkuiDecoratorName::EVENT) {
            Self::Event
        } else if names.contains(&ArkuiDecoratorName::PARAM)
            || names.contains(&ArkuiDecoratorName::REQUIRE)
        {
            Self::Param
        } else if names.contains(&ArkuiDecoratorName::ONCE) {
            Self::Once
        } else if names.contains(&ArkuiDecoratorName::LOCAL) {
            Self::Local
        } else if names.contains(&ArkuiDecoratorName::PROVIDER) {
            Self::Provider
        } else if names.contains(&ArkuiDecoratorName::CONSUMER) {
            Self::Consumer
        } else {
            Self::Regular
        }
    }

    fn is_supported_decorator(name: &str) -> bool {
        matches!(
            name,
            ArkuiDecoratorName::LOCAL
                | ArkuiDecoratorName::PARAM
                | ArkuiDecoratorName::ONCE
                | ArkuiDecoratorName::EVENT
                | ArkuiDecoratorName::PROVIDER
                | ArkuiDecoratorName::CONSUMER
                | ArkuiDecoratorName::BUILDER_PARAM
        )
    }

    fn contributes_param_update(self) -> bool {
        matches!(self, Self::Param | Self::Once)
    }

    fn strips_require(self) -> bool {
        matches!(self, Self::Param | Self::Once | Self::BuilderParam)
    }
}

struct V2Field<'a> {
    name: String,
    property: &'a PropertyDefinition<'a>,
    type_annotation: Option<&'a TSTypeAnnotation<'a>>,
    initializer: Option<&'a Expression<'a>>,
    kind: V2FieldKind,
    is_static: bool,
}

pub(super) struct ComponentV2Lowerer<'a, 'b> {
    stmt: &'a StructStatement<'a>,
    allocator: &'b Allocator,
    runtime_names: &'a ArkuiRuntimeNames,
    source_text: &'a str,
    source_id: &'a str,
    custom_components: &'a FxHashSet<String>,
    builder_functions: &'a FxHashSet<String>,
    custom_component_update_props: &'a CustomComponentUpdateProps,
    component_metadata: &'a ComponentMetadata,
    extend_attributes: Arc<ExtendAttributeIndex>,
    module_name: Option<&'a str>,
    is_exported: bool,
    is_default: bool,
    is_entry: bool,
    emit_reusable_v2_helper: bool,
}

impl<'a, 'b> ComponentV2Lowerer<'a, 'b> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        stmt: &'a StructStatement<'a>,
        allocator: &'b Allocator,
        runtime_names: &'a ArkuiRuntimeNames,
        source_text: &'a str,
        source_id: &'a str,
        custom_components: &'a FxHashSet<String>,
        builder_functions: &'a FxHashSet<String>,
        custom_component_update_props: &'a CustomComponentUpdateProps,
        component_metadata: &'a ComponentMetadata,
        extend_attributes: Arc<ExtendAttributeIndex>,
        module_name: Option<&'a str>,
        is_exported: bool,
        is_default: bool,
        is_entry: bool,
        emit_reusable_v2_helper: bool,
    ) -> Self {
        Self {
            stmt,
            allocator,
            runtime_names,
            source_text,
            source_id,
            custom_components,
            builder_functions,
            custom_component_update_props,
            component_metadata,
            extend_attributes,
            module_name,
            is_exported,
            is_default,
            is_entry,
            emit_reusable_v2_helper,
        }
    }

    pub(super) fn lower(&self) -> Result<OxcVec<'b, Statement<'b>>, ComponentV2Error> {
        let name = self.stmt.id.name.as_str();
        let raw_object_fields = FxHashSet::default();
        let mut visible_builder_methods = self.visible_builder_methods();
        let mut builder_param_methods = FxHashSet::default();
        self.collect_builder_param_methods(
            &mut visible_builder_methods,
            &mut builder_param_methods,
        );

        let mut fields = Vec::new();
        let mut build_body = None;
        let mut has_build = false;
        let mut builder_methods = Vec::new();
        let mut styles_methods: Vec<&MethodDefinition<'_>> = Vec::new();
        let mut regular_members: Vec<V2RegularMember<'_>> = Vec::new();
        let mut constructor_method: Option<&MethodDefinition<'_>> = None;
        let mut computed_names = Vec::new();
        let mut has_monitor_method = false;

        for element in &self.stmt.body.body {
            match element {
                StructElement::PropertyDefinition(property) => {
                    if let Some(field) =
                        self.field_from_property(name, property, &visible_builder_methods)?
                    {
                        fields.push(field);
                    }
                }
                StructElement::MethodDefinition(method)
                    if self.is_method_named(method, GeneratedViewpuName::BUILD) =>
                {
                    has_build = true;
                    let plan = BuildBodyPlan::component_v2(
                        method.value.body.as_deref(),
                        self.source_text,
                        self.source_id,
                        self.custom_components,
                        &visible_builder_methods,
                        &builder_param_methods,
                        self.custom_component_update_props,
                        self.component_metadata,
                        &raw_object_fields,
                        self.module_name,
                        false,
                    )
                    .with_shared_extend_attributes(Arc::clone(&self.extend_attributes));
                    build_body = Some(plan);
                }
                StructElement::MethodDefinition(method) if self.is_builder_method(method) => {
                    let Some(method_name) = method.key.static_name() else {
                        continue;
                    };
                    let plan = BuildBodyPlan::builder_v2(
                        method.value.body.as_deref(),
                        self.source_text,
                        self.source_id,
                        self.custom_components,
                        &visible_builder_methods,
                        &builder_param_methods,
                        self.custom_component_update_props,
                        self.component_metadata,
                        &raw_object_fields,
                        self.module_name,
                    )
                    .with_shared_extend_attributes(Arc::clone(&self.extend_attributes));
                    builder_methods.push((method_name.into_owned(), method.as_ref(), plan));
                }
                StructElement::MethodDefinition(method) if self.is_styles_method(method) => {
                    styles_methods.push(method.as_ref());
                }
                StructElement::MethodDefinition(method) if self.is_constructor(method) => {
                    constructor_method = Some(method.as_ref());
                }
                StructElement::MethodDefinition(method) => {
                    if self.has_method_decorator(method, ArkuiDecoratorName::COMPUTED) {
                        if let Some(method_name) = method.key.static_name() {
                            computed_names.push(method_name.into_owned());
                        }
                    }
                    self.validate_lifecycle_method(name, method)?;
                    has_monitor_method |=
                        self.has_method_decorator(method, ArkuiDecoratorName::MONITOR);
                    regular_members.push(V2RegularMember::Method(method));
                }
                StructElement::StaticBlock(block) => {
                    regular_members.push(V2RegularMember::StaticBlock(block));
                }
                StructElement::TSIndexSignature(signature) => {
                    regular_members.push(V2RegularMember::TSIndexSignature(signature));
                }
                StructElement::AccessorProperty(property) => {
                    regular_members.push(V2RegularMember::AccessorProperty(property));
                }
            }
        }

        if !has_build {
            return Err(ComponentV2Error::MissingBuildMethod {
                name: name.to_string(),
            });
        }

        let ast = AstBuilder::new(self.allocator);
        let mut elements = ast.vec();

        elements.push(self.constructor_element(constructor_method, &fields));
        elements.push(self.reset_state_vars_on_reuse_element(
            &fields,
            &computed_names,
            has_monitor_method,
        ));

        for field in &fields {
            elements.push(self.field_property_element(field));
        }

        if fields
            .iter()
            .any(|field| !field.is_static && field.kind.contributes_param_update())
        {
            elements.push(self.update_state_vars_element(&fields));
        }

        elements.push(self.initial_render_element(name, build_body.as_ref())?);
        for (method_name, method, body) in &builder_methods {
            elements.push(self.builder_method_element(name, method_name, method, body)?);
        }
        for method in &styles_methods {
            if let Some(element) = self.styles_method_element(method) {
                elements.push(element);
            }
        }
        elements.push(self.rerender_element());
        if self.is_entry {
            elements.push(self.entry_name_element(name));
        }

        for member in &regular_members {
            elements.push(member.to_class_element(self.allocator));
        }

        let mut statements = ast.vec();
        if self.emit_reusable_v2_helper {
            statements.push(self.reusable_v2_helper_statement());
        }
        statements.push(
            ViewpuMemberAstEmitter::new(self.allocator)
                .class_declaration_statement_with_decorators(
                    self.allocator.alloc_str(name),
                    GeneratedViewpuName::VIEW_V2_BASE,
                    elements,
                    self.class_decorators(),
                    self.is_exported,
                    self.is_default,
                ),
        );
        Ok(statements)
    }

    fn class_decorators(&self) -> OxcVec<'b, Decorator<'b>> {
        let ast = AstBuilder::new(self.allocator);
        if !self.is_reusable_v2() {
            return ast.vec();
        }
        let decorator = ast.decorator(
            SPAN,
            ast.expression_identifier(SPAN, GeneratedViewpuName::REUSABLE_V2_INNER_DECORATOR),
        );
        ast.vec1(decorator)
    }

    fn reusable_v2_helper_statement(&self) -> Statement<'b> {
        let ast = AstBuilder::new(self.allocator);
        let builder = ViewpuAstBuilder::new(self.allocator);
        let base_class = ast.expression_identifier(SPAN, GeneratedViewpuName::BASE_CLASS);
        let base_class_prototype = ast.member_expression_static(
            SPAN,
            base_class,
            ast.identifier_name(SPAN, GeneratedViewpuName::PROTOTYPE),
            false,
        );
        let getter_body = ast.function_body(
            SPAN,
            ast.vec(),
            ast.vec1(ast.statement_expression(SPAN, ast.expression_boolean_literal(SPAN, true))),
        );
        let getter = ast.expression_arrow_function(
            SPAN,
            true,
            false,
            NONE,
            ast.formal_parameters(
                SPAN,
                FormalParameterKind::ArrowFormalParameters,
                ast.vec(),
                NONE,
            ),
            NONE,
            getter_body,
        );
        let descriptor = ast.expression_object(
            SPAN,
            ast.vec1(ast.object_property_kind_object_property(
                SPAN,
                PropertyKind::Init,
                ast.property_key_static_identifier(SPAN, ast.str(GeneratedViewpuName::GET)),
                getter,
                false,
                false,
                false,
            )),
        );
        let reflect_define_property = ast.member_expression_static(
            SPAN,
            ast.expression_identifier(SPAN, GeneratedViewpuName::REFLECT),
            ast.identifier_name(SPAN, GeneratedViewpuName::DEFINE_PROPERTY),
            false,
        );
        let call = ast.expression_call(
            SPAN,
            Expression::from(reflect_define_property),
            NONE,
            ast.vec_from_iter([
                Argument::from(Expression::from(base_class_prototype)),
                Argument::from(ast.expression_string_literal(
                    SPAN,
                    ast.str(GeneratedViewpuName::IS_REUSABLE),
                    None,
                )),
                Argument::from(descriptor),
            ]),
            false,
        );
        let body = ast.alloc_function_body(
            SPAN,
            ast.vec(),
            ast.vec1(ast.statement_expression(SPAN, call)),
        );
        let function = ast.alloc_function(
            SPAN,
            FunctionType::FunctionDeclaration,
            Some(ast.binding_identifier(
                SPAN,
                ast.str(GeneratedViewpuName::REUSABLE_V2_INNER_DECORATOR),
            )),
            false,
            false,
            false,
            NONE,
            NONE,
            ast.alloc_formal_parameters(
                SPAN,
                FormalParameterKind::FormalParameter,
                ast.vec1(builder.formal_parameter(GeneratedViewpuName::BASE_CLASS)),
                NONE,
            ),
            NONE,
            Some(body),
        );
        Statement::FunctionDeclaration(function)
    }

    fn is_reusable_v2(&self) -> bool {
        self.stmt.decorators.iter().any(|decorator| {
            ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::REUSABLE_V2
        })
    }

    fn visible_builder_methods(&self) -> FxHashSet<String> {
        let mut methods = self.builder_functions.clone();
        for element in &self.stmt.body.body {
            let StructElement::MethodDefinition(method) = element else {
                continue;
            };
            if self.is_builder_method(method) {
                if let Some(name) = method.key.static_name() {
                    methods.insert(name.into_owned());
                }
            }
        }
        methods
    }

    fn collect_builder_param_methods(
        &self,
        visible_builder_methods: &mut FxHashSet<String>,
        builder_param_methods: &mut FxHashSet<String>,
    ) {
        for element in &self.stmt.body.body {
            let StructElement::PropertyDefinition(property) = element else {
                continue;
            };
            if self.property_has_decorator(property, ArkuiDecoratorName::BUILDER_PARAM) {
                if let Some(name) = property.key.static_name() {
                    let name = name.into_owned();
                    visible_builder_methods.insert(name.clone());
                    builder_param_methods.insert(name);
                }
            }
        }
    }

    fn field_from_property(
        &self,
        component_name: &str,
        property: &'a PropertyDefinition<'a>,
        visible_builder_methods: &FxHashSet<String>,
    ) -> Result<Option<V2Field<'a>>, ComponentV2Error> {
        let Some(field_name) = property.key.static_name() else {
            return Ok(None);
        };
        let field_name = field_name.into_owned();
        let decorator_names = self.property_decorator_names(property);
        self.validate_field_decorators(
            component_name,
            &field_name,
            property,
            &decorator_names,
            visible_builder_methods,
        )?;
        Ok(Some(V2Field {
            name: field_name,
            property,
            type_annotation: property.type_annotation.as_deref(),
            initializer: property.value.as_ref(),
            kind: V2FieldKind::from_decorator_names(&decorator_names),
            is_static: property.r#static,
        }))
    }

    fn validate_field_decorators(
        &self,
        component_name: &str,
        field_name: &str,
        property: &PropertyDefinition<'_>,
        decorator_names: &[&str],
        visible_builder_methods: &FxHashSet<String>,
    ) -> Result<(), ComponentV2Error> {
        for decorator_name in decorator_names {
            if ArkuiDecoratorName::is_component_v2_meta(decorator_name)
                || V2FieldKind::is_supported_decorator(decorator_name)
            {
                continue;
            }
            return Err(ComponentV2Error::UnsupportedDecorator {
                name: component_name.to_string(),
                decorator: (*decorator_name).to_string(),
                field: field_name.to_string(),
            });
        }
        let has_param = decorator_names.contains(&ArkuiDecoratorName::PARAM);
        let has_require = decorator_names.contains(&ArkuiDecoratorName::REQUIRE);
        let has_builder_param = decorator_names.contains(&ArkuiDecoratorName::BUILDER_PARAM);
        let kind = V2FieldKind::from_decorator_names(decorator_names);
        if !matches!(kind, V2FieldKind::Regular) && property.type_annotation.is_none() {
            return Err(ComponentV2Error::InvalidFieldDecorators {
                name: component_name.to_string(),
                field: field_name.to_string(),
                message: format!("The property '{field_name}' must specify a type."),
            });
        }
        if has_param && property.value.is_none() && !has_require {
            return Err(ComponentV2Error::InvalidFieldDecorators {
                name: component_name.to_string(),
                field: field_name.to_string(),
                message: "When a variable decorated with '@Param' is not assigned a default value, it must also be decorated with '@Require'.".to_string(),
            });
        }
        if decorator_names.contains(&ArkuiDecoratorName::ONCE)
            && !decorator_names.contains(&ArkuiDecoratorName::PARAM)
        {
            return Err(ComponentV2Error::InvalidFieldDecorators {
                name: component_name.to_string(),
                field: field_name.to_string(),
                message:
                    "When a variable decorated with '@Once', it must also be decorated with '@Param'."
                        .to_string(),
            });
        }
        if has_require && !has_param && !has_builder_param {
            return Err(ComponentV2Error::InvalidFieldDecorators {
                name: component_name.to_string(),
                field: field_name.to_string(),
                message: "In a struct decorated with '@ComponentV2', '@Require' can only be used with '@Param' and '@BuilderParam'.".to_string(),
            });
        }
        if has_builder_param
            && property.value.as_ref().is_some_and(|initializer| {
                !self.builder_param_initializer_is_builder(initializer, visible_builder_methods)
            })
        {
            return Err(ComponentV2Error::InvalidFieldDecorators {
                name: component_name.to_string(),
                field: field_name.to_string(),
                message: "'@BuilderParam' property can only initialized by '@Builder' function."
                    .to_string(),
            });
        }
        Ok(())
    }

    fn builder_param_initializer_is_builder(
        &self,
        initializer: &Expression<'_>,
        visible_builder_methods: &FxHashSet<String>,
    ) -> bool {
        match initializer {
            Expression::Identifier(identifier) => {
                visible_builder_methods.contains(identifier.name.as_str())
            }
            Expression::StaticMemberExpression(member) => {
                visible_builder_methods.contains(member.property.name.as_str())
                    || self.is_wrapped_builder_property_access(initializer)
            }
            _ => false,
        }
    }

    fn is_wrapped_builder_property_access(&self, initializer: &Expression<'_>) -> bool {
        let Expression::StaticMemberExpression(member) = initializer else {
            return false;
        };
        if member.property.name != "builder" {
            return false;
        }
        let Expression::CallExpression(call) = &member.object else {
            return false;
        };
        matches!(&call.callee, Expression::Identifier(identifier) if identifier.name == "wrapBuilder")
    }

    fn validate_lifecycle_method(
        &self,
        component_name: &str,
        method: &MethodDefinition<'_>,
    ) -> Result<(), ComponentV2Error> {
        let Some(decorator_name) = method.decorators.iter().find_map(|decorator| {
            let name = ArkuiDecorator::new(decorator).name();
            Self::component_lifecycle_member_decorator(name).then_some(name)
        }) else {
            return Ok(());
        };
        if method.value.params.items.is_empty() {
            return Ok(());
        }
        let method_name = method
            .key
            .static_name()
            .map(|name| name.into_owned())
            .unwrap_or_else(|| "<anonymous>".to_string());
        let message = if decorator_name == ArkuiDecoratorName::COMPONENT_REUSE {
            "Methods decorated with '@ComponentReuse' in '@ComponentV2' cannot have input parameters."
                .to_string()
        } else {
            format!("Methods decorated with '@{decorator_name}' cannot have input parameters.")
        };
        Err(ComponentV2Error::InvalidFieldDecorators {
            name: component_name.to_string(),
            field: method_name,
            message,
        })
    }

    fn component_lifecycle_member_decorator(name: &str) -> bool {
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
        )
    }

    fn constructor_element(
        &self,
        constructor_method: Option<&MethodDefinition<'_>>,
        fields: &[V2Field<'_>],
    ) -> ClassElement<'b> {
        let builder = ViewpuAstBuilder::new(self.allocator);
        let mut stmts = builder.statements();
        stmts.push(builder.super_call_statement(&[
            GeneratedViewpuName::PARENT,
            &self.runtime_names.elmt_id,
            GeneratedViewpuName::EXTRA_INFO,
        ]));
        if let Some(body) = constructor_method.and_then(|method| method.value.body.as_deref()) {
            for statement in &body.statements {
                stmts.push(statement.clone_in(self.allocator));
            }
        }
        for field in fields {
            if let Some(statement) = self.constructor_field_statement(field) {
                stmts.push(statement);
            }
        }
        stmts.push(builder.this_call_statement(
            GeneratedViewpuName::FINALIZE_CONSTRUCTION,
            builder.expressions(),
        ));

        let params = builder.formal_parameters_from_iter([
            builder.formal_parameter(GeneratedViewpuName::PARENT),
            builder.formal_parameter(GeneratedViewpuName::PARAMS),
            builder.formal_parameter(GeneratedViewpuName::LOCAL_STORAGE_INTERNAL),
            builder.formal_parameter_with_initializer(
                &self.runtime_names.elmt_id,
                None,
                Some(builder.minus_one_expression()),
            ),
            builder.formal_parameter(&self.runtime_names.params_lambda),
            builder.formal_parameter(GeneratedViewpuName::EXTRA_INFO),
        ]);
        ViewpuMemberAstEmitter::new(self.allocator).method_with_params(
            GeneratedViewpuName::CONSTRUCTOR,
            params,
            stmts,
            false,
            None,
        )
    }

    fn reset_state_vars_on_reuse_element(
        &self,
        fields: &[V2Field<'_>],
        computed_names: &[String],
        has_monitor_method: bool,
    ) -> ClassElement<'b> {
        let builder = ViewpuAstBuilder::new(self.allocator);
        let mut stmts = builder.statements();
        for field in fields {
            if let Some(statement) = self.reset_field_statement(field) {
                stmts.push(statement);
            }
        }
        for name in computed_names {
            stmts.push(builder.this_call_statement(
                GeneratedViewpuName::RESET_COMPUTED,
                builder.expressions_from_iter([builder.string_expression(name)]),
            ));
        }
        if has_monitor_method {
            stmts.push(builder.this_call_statement(
                GeneratedViewpuName::RESET_MONITORS_ON_REUSE,
                builder.expressions(),
            ));
        }
        let object_type = builder.type_reference_annotation("Object");
        let params = builder.formal_parameters_from_iter([builder
            .formal_parameter_with_type_annotation(
                GeneratedViewpuName::PARAMS,
                Some(object_type.as_ref()),
            )]);
        ViewpuMemberAstEmitter::new(self.allocator).method_with_type_annotation(
            GeneratedViewpuName::RESET_STATE_VARS_ON_REUSE,
            params,
            Some(builder.void_type_annotation()),
            stmts,
            false,
            Some(TSAccessibility::Public),
        )
    }

    fn field_property_element(&self, field: &V2Field<'_>) -> ClassElement<'b> {
        let ast = AstBuilder::new(self.allocator);
        let mut property = field.property.clone_in(self.allocator);
        if !field.is_static {
            property.value = None;
        }
        property.decorators = self.filtered_field_decorators(field);
        ClassElement::PropertyDefinition(ast.alloc(property))
    }

    fn filtered_field_decorators(&self, field: &V2Field<'_>) -> OxcVec<'b, Decorator<'b>> {
        let ast = AstBuilder::new(self.allocator);
        field
            .property
            .decorators
            .iter()
            .filter(|decorator| {
                let name = ArkuiDecorator::new(decorator).name();
                if name == ArkuiDecoratorName::BUILDER_PARAM
                    && field.kind == V2FieldKind::BuilderParam
                {
                    return false;
                }
                if name == ArkuiDecoratorName::REQUIRE && field.kind.strips_require() {
                    return false;
                }
                true
            })
            .map(|decorator| decorator.clone_in(self.allocator))
            .fold(ast.vec(), |mut decorators, decorator| {
                decorators.push(decorator);
                decorators
            })
    }

    fn update_state_vars_element(&self, fields: &[V2Field<'_>]) -> ClassElement<'b> {
        let builder = ViewpuAstBuilder::new(self.allocator);
        let mut stmts = builder.statements();
        let mut return_block = builder.statements();
        return_block.push(builder.return_statement());
        stmts.push(builder.if_statement(builder.params_is_undefined(), return_block));
        for field in fields
            .iter()
            .filter(|field| !field.is_static && field.kind.contributes_param_update())
        {
            let mut consequent = builder.statements();
            consequent.push(builder.this_call_statement(
                GeneratedViewpuName::UPDATE_PARAM,
                builder.expressions_from_iter([
                    builder.string_expression(&field.name),
                    builder.params_field_expression(&field.name),
                ]),
            ));
            stmts.push(builder.if_statement(builder.params_has_field(&field.name), consequent));
        }
        let params = builder
            .formal_parameters_from_iter([builder.formal_parameter(GeneratedViewpuName::PARAMS)]);
        ViewpuMemberAstEmitter::new(self.allocator).method_with_params(
            GeneratedViewpuName::UPDATE_STATE_VARS,
            params,
            stmts,
            false,
            Some(TSAccessibility::Public),
        )
    }

    fn initial_render_element(
        &self,
        component_name: &str,
        build_body: Option<&BuildBodyPlan<'_>>,
    ) -> Result<ClassElement<'b>, ComponentV2Error> {
        let stmts = build_body
            .map(|body| BuildBodyInfo::Plan(body.clone()).statements(self.allocator))
            .transpose()
            .map_err(|err| ComponentV2Error::UnsupportedBuildDsl {
                name: component_name.to_string(),
                detail: err.to_string(),
            })?
            .unwrap_or_else(|| OxcVec::new_in(&self.allocator));
        let builder = ViewpuAstBuilder::new(self.allocator);
        Ok(
            ViewpuMemberAstEmitter::new(self.allocator).method_with_params(
                GeneratedViewpuName::INITIAL_RENDER,
                builder.formal_parameters(),
                stmts,
                false,
                None,
            ),
        )
    }

    fn builder_method_element(
        &self,
        component_name: &str,
        method_name: &str,
        method: &MethodDefinition<'_>,
        body: &BuildBodyPlan<'_>,
    ) -> Result<ClassElement<'b>, ComponentV2Error> {
        let stmts = BuildBodyInfo::Plan(body.clone())
            .statements(self.allocator)
            .map_err(|err| ComponentV2Error::UnsupportedBuildDsl {
                name: component_name.to_string(),
                detail: err.to_string(),
            })?;
        let builder = ViewpuAstBuilder::new(self.allocator);
        let return_type = method
            .value
            .return_type
            .as_deref()
            .map(|annotation| builder.clone_type_annotation(annotation));
        Ok(
            ViewpuMemberAstEmitter::new(self.allocator).method_with_formal_parameters(
                self.allocator.alloc_str(method_name),
                builder.clone_formal_parameters_with_parent(&method.value.params),
                return_type,
                stmts,
                false,
                None,
            ),
        )
    }

    fn styles_method_element(&self, method: &MethodDefinition<'_>) -> Option<ClassElement<'b>> {
        let method_name = method.key.static_name()?;
        let builder = ViewpuAstBuilder::new(self.allocator);
        let return_type = method
            .value
            .return_type
            .as_deref()
            .map(|annotation| builder.clone_type_annotation(annotation));
        let stmts = StylesBodyLowerer::new(method.value.body.as_deref()).statements(self.allocator);
        Some(
            ViewpuMemberAstEmitter::new(self.allocator).method_with_formal_parameters(
                self.allocator.alloc_str(&method_name),
                builder.clone_formal_parameters(&method.value.params),
                return_type,
                stmts,
                false,
                None,
            ),
        )
    }

    fn rerender_element(&self) -> ClassElement<'b> {
        let builder = ViewpuAstBuilder::new(self.allocator);
        let mut stmts = builder.statements();
        stmts.push(builder.this_call_statement(
            GeneratedViewpuName::UPDATE_DIRTY_ELEMENTS,
            builder.expressions(),
        ));
        ViewpuMemberAstEmitter::new(self.allocator).method_with_params(
            GeneratedViewpuName::RERENDER,
            builder.formal_parameters(),
            stmts,
            false,
            None,
        )
    }

    fn entry_name_element(&self, name: &str) -> ClassElement<'b> {
        let builder = ViewpuAstBuilder::new(self.allocator);
        let mut stmts = builder.statements();
        stmts.push(builder.return_string_statement(name));
        ViewpuMemberAstEmitter::new(self.allocator).method_with_type_annotation(
            GeneratedViewpuName::GET_ENTRY_NAME,
            builder.formal_parameters(),
            Some(builder.string_type_annotation()),
            stmts,
            true,
            None,
        )
    }

    fn constructor_field_statement(&self, field: &V2Field<'_>) -> Option<Statement<'b>> {
        if field.is_static {
            return None;
        }
        let builder = ViewpuAstBuilder::new(self.allocator);
        match field.kind {
            V2FieldKind::Param | V2FieldKind::Once => Some(builder.this_call_statement(
                GeneratedViewpuName::INIT_PARAM,
                builder.expressions_from_iter([
                    builder.string_expression(&field.name),
                    self.param_init_value(field),
                ]),
            )),
            V2FieldKind::Event => Some(builder.this_assign_statement(
                &field.name,
                self.params_field_or(field, self.event_default_expression(field, false), false),
            )),
            V2FieldKind::BuilderParam => Some(builder.this_assign_statement(
                &field.name,
                self.params_field_or(field, self.initializer_or_undefined(field), false),
            )),
            V2FieldKind::Local
            | V2FieldKind::Provider
            | V2FieldKind::Consumer
            | V2FieldKind::Regular => Some(
                builder.this_assign_statement(&field.name, self.initializer_or_undefined(field)),
            ),
        }
    }

    fn reset_field_statement(&self, field: &V2Field<'_>) -> Option<Statement<'b>> {
        if field.is_static {
            return None;
        }
        let builder = ViewpuAstBuilder::new(self.allocator);
        match field.kind {
            V2FieldKind::Param | V2FieldKind::Once => Some(builder.this_call_statement(
                GeneratedViewpuName::RESET_PARAM,
                builder.expressions_from_iter([
                    builder.string_expression(&field.name),
                    self.param_init_value(field),
                ]),
            )),
            V2FieldKind::Event => Some(builder.this_assign_statement(
                &field.name,
                self.params_field_or(field, self.event_default_expression(field, true), false),
            )),
            V2FieldKind::BuilderParam => Some(builder.this_assign_statement(
                &field.name,
                self.params_field_or(field, self.initializer_or_undefined(field), false),
            )),
            V2FieldKind::Consumer => Some(builder.this_call_statement(
                GeneratedViewpuName::RESET_CONSUMER,
                builder.expressions_from_iter([
                    builder.string_expression(&field.name),
                    self.initializer_or_undefined(field),
                ]),
            )),
            V2FieldKind::Local | V2FieldKind::Provider => Some(
                builder.this_assign_statement(&field.name, self.initializer_or_undefined(field)),
            ),
            V2FieldKind::Regular => None,
        }
    }

    fn param_init_value(&self, field: &V2Field<'_>) -> Expression<'b> {
        self.params_field_or(field, self.initializer_or_undefined(field), true)
    }

    fn params_field_or(
        &self,
        field: &V2Field<'_>,
        alternate: Expression<'b>,
        require_params_presence: bool,
    ) -> Expression<'b> {
        let builder = ViewpuAstBuilder::new(self.allocator);
        let test = if require_params_presence {
            builder.params_present_and_has_field(&field.name)
        } else {
            builder.params_has_field(&field.name)
        };
        builder.conditional_expression(
            test,
            builder.params_field_expression(&field.name),
            alternate,
        )
    }

    fn initializer_or_undefined(&self, field: &V2Field<'_>) -> Expression<'b> {
        let builder = ViewpuAstBuilder::new(self.allocator);
        field
            .initializer
            .map(|initializer| initializer.clone_in(self.allocator))
            .unwrap_or_else(|| builder.undefined_expression())
    }

    fn event_default_expression(
        &self,
        field: &V2Field<'_>,
        include_typed_params: bool,
    ) -> Expression<'b> {
        let builder = ViewpuAstBuilder::new(self.allocator);
        if let Some(initializer) = field.initializer {
            return initializer.clone_in(self.allocator);
        }
        let params = include_typed_params
            .then(|| self.event_function_type_params(field.type_annotation))
            .flatten();
        builder.empty_arrow_function_expression(params)
    }

    fn event_function_type_params(
        &self,
        type_annotation: Option<&'a TSTypeAnnotation<'a>>,
    ) -> Option<&'a FormalParameters<'a>> {
        let TSType::TSFunctionType(function_type) = &type_annotation?.type_annotation else {
            return None;
        };
        Some(function_type.params.as_ref())
    }

    fn property_decorator_names<'c>(&self, property: &'c PropertyDefinition<'c>) -> Vec<&'c str> {
        property
            .decorators
            .iter()
            .map(|decorator| ArkuiDecorator::new(decorator).name())
            .collect()
    }

    fn property_has_decorator(&self, property: &PropertyDefinition<'_>, name: &str) -> bool {
        property
            .decorators
            .iter()
            .any(|decorator| ArkuiDecorator::new(decorator).name() == name)
    }

    fn is_builder_method(&self, method: &MethodDefinition<'_>) -> bool {
        self.has_method_decorator(method, ArkuiDecoratorName::BUILDER)
            || self.has_method_decorator(method, ArkuiDecoratorName::LOCAL_BUILDER)
    }

    fn is_styles_method(&self, method: &MethodDefinition<'_>) -> bool {
        self.has_method_decorator(method, ArkuiDecoratorName::STYLES)
    }

    fn has_method_decorator(&self, method: &MethodDefinition<'_>, name: &str) -> bool {
        method
            .decorators
            .iter()
            .any(|decorator| ArkuiDecorator::new(decorator).name() == name)
    }

    fn is_method_named(&self, method: &MethodDefinition<'_>, name: &str) -> bool {
        method
            .key
            .static_name()
            .is_some_and(|method_name| method_name == name)
    }

    fn is_constructor(&self, method: &MethodDefinition<'_>) -> bool {
        method.kind == MethodDefinitionKind::Constructor
            || self.is_method_named(method, GeneratedViewpuName::CONSTRUCTOR)
    }
}
