use rustc_hash::FxHashSet;

use oxc_allocator::Allocator;
use oxc_allocator::CloneIn;
use oxc_ast::ast::{
    AccessorProperty, Argument, AssignmentExpression, AssignmentTarget, ClassElement, Expression,
    FormalParameterKind, FormalParameters, MemberExpression, MethodDefinition, NumberBase,
    ObjectPropertyKind, Program, PropertyDefinition, PropertyKind, Statement, StaticBlock,
    TSIndexSignature, TSType, TSTypeAnnotation, TSTypeName, VariableDeclarationKind,
    VariableDeclarator,
};
use oxc_ast::AstBuilder;
use oxc_ast::NONE;
use oxc_ast_visit::{walk_mut, VisitMut};
use oxc_span::{Span, SPAN};
use oxc_syntax::operator::AssignmentOperator;

use crate::transform::build_fn::{
    BuildBodyInfo, BuildLoweringError, BuildSourceMetadata, CustomComponentUpdateProps,
};
use crate::transform::component_meta::ArkuiRuntimeNames;
use crate::transform::decorator::{ArkuiDecoratorName, ArkuiStateDecorator, ConstructorInitKind};
use crate::transform::entry::EntryRegistrationInfo;
use crate::transform::global_rewrite::GlobalContextCallRewriter;
use crate::transform::viewpu_emit::{
    GeneratedViewpuName, ParamsInterfaceAstEmitter, ParamsInterfaceField, ParamsInterfaceFieldType,
    ViewpuAstBuilder, ViewpuMemberAstEmitter,
};

/// A `@Builder` method extracted from a `@Component` struct.
///
/// In ArkTS, `@Builder` methods contain the same ArkUI component DSL as `build()`.
/// They remain as class methods on the ViewPU subclass but their bodies are desugared
/// to `observeComponentCreation2` calls (identical to `build()` desugaring).
#[derive(Debug, Clone)]
pub struct BuilderMethodInfo<'a> {
    /// Method name as declared (not renamed).
    pub name: String,
    /// Original parameter list. A `parent = null` parameter is appended when emitted.
    pub params: &'a FormalParameters<'a>,
    /// Original return type annotation, if present.
    pub return_annotation: Option<&'a TSTypeAnnotation<'a>>,
    /// ArkUI DSL body plan. The final class emitter lowers it into OXC
    /// statements with the allocator that owns the generated class.
    pub body: BuildBodyInfo<'a>,
}

/// A state/prop/link field extracted from a `@Component` struct.
#[derive(Debug, Clone)]
pub struct StateField<'a> {
    /// Original property/decorator range used by the TypeScript source-map emitter.
    pub source_span: Span,
    /// Original property key range.
    pub name_span: Span,
    /// Field name (without leading `__`).
    pub name: String,
    /// Original TypeScript annotation AST when the field came from parsed source.
    pub type_annotation_ast: Option<&'a TSTypeAnnotation<'a>>,
    /// Whether the source property used `?`.
    pub optional: bool,
    /// Decorator name: `"State"`, `"Prop"`, `"Link"`, etc.
    pub decorator: String,
    /// Initializer expression from the source AST.
    pub initializer: Option<FieldExpression<'a>>,
    /// Handler name from `@Watch("handlerName")`, if present on this field.
    /// When set, generates `this.declareWatch("fieldName", this.handlerName)` in the
    /// constructor (after `setInitiallyProvidedValue`).
    pub watch_handler: Option<String>,
    /// Alias key from `@Provide("alias")` / `@Consume("alias")`.
    /// For `@Provide`, this is the key exposed to descendants.
    /// For `@Consume`, this is the key looked up from an ancestor `@Provide`.
    /// When absent the field name itself is used as the key.
    pub alias: Option<String>,
    /// First decorator argument expression for decorators that pass the
    /// original expression through to runtime APIs, such as `@StorageProp`.
    pub decorator_arg: Option<FieldExpression<'a>>,
    /// Result of hvigor's `isSimpleType` equivalent for this field, derived
    /// from the OXC type annotation AST.
    pub is_simple_type: bool,
}

#[derive(Debug, Clone)]
pub struct ParamField<'a> {
    pub name: String,
    pub source_span: Span,
    pub name_span: Span,
    pub type_annotation_ast: Option<&'a TSTypeAnnotation<'a>>,
}

#[derive(Debug, Clone)]
pub struct RegularField<'a> {
    pub name: String,
    pub source_span: Span,
    pub name_span: Span,
    pub type_annotation_ast: Option<&'a TSTypeAnnotation<'a>>,
    pub initializer: Option<FieldExpression<'a>>,
    pub optional: bool,
}

impl<'a> RegularField<'a> {
    fn is_custom_dialog_controller(&self) -> bool {
        self.type_annotation_ast.is_some_and(|annotation| {
            matches!(
                &annotation.type_annotation,
                TSType::TSTypeReference(reference)
                    if matches!(
                        &reference.type_name,
                        TSTypeName::IdentifierReference(identifier)
                            if identifier.name == ArkuiRuntimeTypeName::CUSTOM_DIALOG_CONTROLLER
                    )
            )
        })
    }
}

struct ArkuiRuntimeTypeName;

impl ArkuiRuntimeTypeName {
    const CUSTOM_DIALOG_CONTROLLER: &'static str = "CustomDialogController";
}

#[derive(Debug, Clone)]
pub enum FieldExpression<'a> {
    Ast(&'a Expression<'a>),
}

impl<'a> FieldExpression<'a> {
    pub fn from_expression(expression: &'a Expression<'a>) -> Self {
        Self::Ast(expression)
    }

    fn to_expression<'b>(&self, builder: &ViewpuAstBuilder<'b>) -> Expression<'b> {
        match self {
            Self::Ast(expression) => builder.clone_expression(expression),
        }
    }
}

/// Program-level lowering for SDK
/// `process_ui_syntax.js::createCustomDialogController`.
///
/// The SDK visits every `new CustomDialogController(...)` before component
/// classes are generated, so this pass intentionally owns V1/V2 component
/// fields, component methods, and ordinary class/function bodies together.
pub(super) struct CustomDialogControllerLowerer<'a, 'b> {
    allocator: &'a Allocator,
    ast: AstBuilder<'a>,
    builder: ViewpuAstBuilder<'a>,
    runtime_names: &'b ArkuiRuntimeNames,
    custom_dialog_components: &'b FxHashSet<String>,
    custom_component_update_props: &'b CustomComponentUpdateProps,
    source_metadata: BuildSourceMetadata<'b>,
    changed: bool,
}

impl<'a, 'b> CustomDialogControllerLowerer<'a, 'b> {
    pub(super) fn new(
        allocator: &'a Allocator,
        runtime_names: &'b ArkuiRuntimeNames,
        custom_dialog_components: &'b FxHashSet<String>,
        custom_component_update_props: &'b CustomComponentUpdateProps,
        source_text: &'b str,
        source_id: &'b str,
    ) -> Self {
        Self {
            allocator,
            ast: AstBuilder::new(allocator),
            builder: ViewpuAstBuilder::new(allocator),
            runtime_names,
            custom_dialog_components,
            custom_component_update_props,
            source_metadata: BuildSourceMetadata::new(source_text, source_id),
            changed: false,
        }
    }

    pub(super) fn program(mut self, program: &mut Program<'a>) -> bool {
        self.visit_program(program);
        self.changed
    }

    fn rewrite_custom_dialog_controller_builder(
        &mut self,
        expression: &mut Expression<'a>,
        controller_expression: Expression<'a>,
    ) {
        let Expression::NewExpression(new_expression) = expression else {
            return;
        };
        if !Self::is_identifier(
            &new_expression.callee,
            ArkuiRuntimeTypeName::CUSTOM_DIALOG_CONTROLLER,
        ) {
            return;
        }
        if new_expression.arguments.len() != 1 {
            return;
        }
        let Some(Argument::ObjectExpression(config)) = new_expression.arguments.first_mut() else {
            return;
        };

        for property in &mut config.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                continue;
            };
            if !property
                .key
                .static_name()
                .is_some_and(|name| name == "builder")
            {
                continue;
            }
            let Some((dialog_name, mut params, source_span)) =
                self.custom_dialog_builder_call(&property.value)
            else {
                continue;
            };
            self.rewrite_custom_dialog_params(&dialog_name, &mut params);
            property.value = self.custom_dialog_builder_arrow(
                &dialog_name,
                params,
                controller_expression.clone_in(self.allocator),
                source_span,
            );
        }

        new_expression
            .arguments
            .push(Argument::from(self.builder.this_expression()));
        self.changed = true;
    }

    fn custom_dialog_builder_call(
        &self,
        expression: &Expression<'a>,
    ) -> Option<(String, Expression<'a>, Span)> {
        let Expression::CallExpression(call) = expression else {
            return None;
        };
        let dialog_name = Self::identifier_name(&call.callee)?;
        if !self.custom_dialog_components.contains(dialog_name) {
            return None;
        }
        let params = match call.arguments.first() {
            Some(Argument::ObjectExpression(object)) => {
                Expression::ObjectExpression(object.clone_in(self.allocator))
            }
            None => self.ast.expression_object(SPAN, self.ast.vec()),
            _ => return None,
        };
        Some((dialog_name.to_string(), params, call.span))
    }

    /// SDK `createCustomComponentBuilderArrowFunction` materializes a custom
    /// dialog with the same constructor-parameter rules as an ordinary custom
    /// component.  The dialog controller pass owns this earlier lowering, so
    /// it must not bypass `@Link` backing properties or legacy `$prop`
    /// references while synthesizing the lazy builder.
    fn rewrite_custom_dialog_params(&self, dialog_name: &str, params: &mut Expression<'a>) {
        let Some(component_info) = self.custom_component_update_props.get(dialog_name) else {
            return;
        };
        let Expression::ObjectExpression(params) = params else {
            return;
        };
        for property in &mut params.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                continue;
            };
            let property_name = property.key.static_name();
            let backing_name = Self::dollar_reference_name(&property.value).or_else(|| {
                property_name
                    .as_deref()
                    .filter(|name| component_info.link_props.contains(*name))
                    .and_then(|_| Self::this_member_name(&property.value))
            });
            if let Some(backing_name) = backing_name {
                property.value = self.builder.this_backing_member_expression(&backing_name);
            }
        }
    }

    fn this_member_name(value: &Expression<'_>) -> Option<String> {
        let Expression::StaticMemberExpression(member) = value else {
            return None;
        };
        matches!(&member.object, Expression::ThisExpression(_))
            .then(|| member.property.name.to_string())
    }

    fn dollar_reference_name(value: &Expression<'_>) -> Option<String> {
        match value {
            Expression::Identifier(identifier) => {
                identifier.name.strip_prefix('$').map(str::to_string)
            }
            Expression::StaticMemberExpression(member)
                if matches!(&member.object, Expression::ThisExpression(_)) =>
            {
                member.property.name.strip_prefix('$').map(str::to_string)
            }
            _ => None,
        }
    }

    fn custom_dialog_builder_arrow(
        &self,
        dialog_name: &str,
        params: Expression<'a>,
        controller_expression: Expression<'a>,
        source_span: Span,
    ) -> Expression<'a> {
        let mut statements = self.ast.vec();
        let params_for_generator = params.clone_in(self.allocator);
        statements.push(self.variable_declaration(
            CustomDialogBuilderName::JS_DIALOG,
            self.custom_dialog_new_expression(dialog_name, params, source_span),
        ));
        statements.push(self.js_dialog_call_statement(
            GeneratedViewpuName::SET_CONTROLLER,
            self.ast.vec1(Argument::from(controller_expression)),
        ));
        statements.push(self.viewpu_create_statement());
        statements.push(self.variable_declaration(
            &self.runtime_names.params_lambda,
            self.params_lambda_expression(params_for_generator),
        ));
        statements.push(self.js_dialog_params_generator_assignment());

        self.ast.expression_arrow_function(
            SPAN,
            false,
            false,
            NONE,
            self.empty_arrow_parameters(),
            NONE,
            self.ast.function_body(SPAN, self.ast.vec(), statements),
        )
    }

    fn custom_dialog_new_expression(
        &self,
        dialog_name: &str,
        params: Expression<'a>,
        source_span: Span,
    ) -> Expression<'a> {
        self.ast.expression_new(
            SPAN,
            self.ast
                .expression_identifier(SPAN, self.ast.str(dialog_name)),
            NONE,
            self.ast.vec_from_iter([
                Argument::from(self.builder.this_expression()),
                Argument::from(params),
                Argument::from(self.builder.undefined_expression()),
                Argument::from(self.builder.minus_one_expression()),
                Argument::from(self.builder.empty_arrow_function_expression(None)),
                Argument::from(self.source_info_expression(source_span)),
            ]),
        )
    }

    /// Latest SDK `process_component_member.js::addCustomComponentId` appends
    /// the original custom-dialog call location to the generated ViewPU
    /// constructor, including for the lazy builder synthesized by
    /// `process_ui_syntax.js::createCustomComponentBuilderArrowFunction`.
    fn source_info_expression(&self, source_span: Span) -> Expression<'a> {
        let (line, col) = self.source_metadata.source_position(source_span.start);
        let page = self.source_metadata.component_page_path();
        self.ast.expression_object(
            SPAN,
            self.ast.vec_from_iter([
                self.object_property(
                    GeneratedViewpuName::SOURCE_PAGE,
                    self.ast
                        .expression_string_literal(SPAN, self.ast.str(&page), None),
                ),
                self.object_property(
                    GeneratedViewpuName::SOURCE_LINE,
                    self.ast.expression_numeric_literal(
                        SPAN,
                        line as f64,
                        None,
                        NumberBase::Decimal,
                    ),
                ),
                self.object_property(
                    GeneratedViewpuName::SOURCE_COL,
                    self.ast.expression_numeric_literal(
                        SPAN,
                        col as f64,
                        None,
                        NumberBase::Decimal,
                    ),
                ),
            ]),
        )
    }

    fn object_property(&self, name: &str, value: Expression<'a>) -> ObjectPropertyKind<'a> {
        self.ast.object_property_kind_object_property(
            SPAN,
            PropertyKind::Init,
            self.ast
                .property_key_static_identifier(SPAN, self.ast.str(name)),
            value,
            false,
            false,
            false,
        )
    }

    fn params_lambda_expression(&self, params: Expression<'a>) -> Expression<'a> {
        let body = self.ast.function_body(
            SPAN,
            self.ast.vec(),
            self.ast.vec1(self.ast.statement_return(SPAN, Some(params))),
        );
        self.ast.expression_arrow_function(
            SPAN,
            false,
            false,
            NONE,
            self.empty_arrow_parameters(),
            NONE,
            body,
        )
    }

    fn viewpu_create_statement(&self) -> Statement<'a> {
        self.call_statement(
            self.static_member_expression(
                self.ast
                    .expression_identifier(SPAN, GeneratedViewpuName::VIEWPU_BASE),
                &self.runtime_names.method_create,
            ),
            self.ast.vec1(Argument::from(
                self.ast
                    .expression_identifier(SPAN, CustomDialogBuilderName::JS_DIALOG),
            )),
        )
    }

    fn js_dialog_call_statement(
        &self,
        method: &str,
        arguments: oxc_allocator::Vec<'a, Argument<'a>>,
    ) -> Statement<'a> {
        self.call_statement(
            self.static_member_expression(
                self.ast
                    .expression_identifier(SPAN, CustomDialogBuilderName::JS_DIALOG),
                method,
            ),
            arguments,
        )
    }

    fn js_dialog_params_generator_assignment(&self) -> Statement<'a> {
        let left = AssignmentTarget::from(
            self.static_member(
                self.ast
                    .expression_identifier(SPAN, CustomDialogBuilderName::JS_DIALOG),
                &self.runtime_names.params_generator_prefix,
            ),
        );
        let assignment = self.ast.expression_assignment(
            SPAN,
            AssignmentOperator::Assign,
            left,
            self.ast
                .expression_identifier(SPAN, self.ast.str(&self.runtime_names.params_lambda)),
        );
        self.ast.statement_expression(SPAN, assignment)
    }

    fn variable_declaration(&self, name: &str, init: Expression<'a>) -> Statement<'a> {
        let binding = self
            .ast
            .binding_pattern_binding_identifier(SPAN, self.ast.str(name));
        let declarator = self.ast.variable_declarator(
            SPAN,
            VariableDeclarationKind::Let,
            binding,
            NONE,
            Some(init),
            false,
        );
        Statement::from(self.ast.declaration_variable(
            SPAN,
            VariableDeclarationKind::Let,
            self.ast.vec1(declarator),
            false,
        ))
    }

    fn call_statement(
        &self,
        callee: Expression<'a>,
        arguments: oxc_allocator::Vec<'a, Argument<'a>>,
    ) -> Statement<'a> {
        self.ast.statement_expression(
            SPAN,
            self.ast
                .expression_call(SPAN, callee, NONE, arguments, false),
        )
    }

    fn static_member_expression(&self, object: Expression<'a>, property: &str) -> Expression<'a> {
        Expression::from(self.static_member(object, property))
    }

    fn static_member(&self, object: Expression<'a>, property: &str) -> MemberExpression<'a> {
        self.ast.member_expression_static(
            SPAN,
            object,
            self.ast.identifier_name(SPAN, self.ast.str(property)),
            false,
        )
    }

    fn empty_arrow_parameters(&self) -> oxc_allocator::Box<'a, FormalParameters<'a>> {
        self.ast.alloc_formal_parameters(
            SPAN,
            FormalParameterKind::ArrowFormalParameters,
            self.ast.vec(),
            NONE,
        )
    }

    fn identifier_name<'c>(expression: &'c Expression<'_>) -> Option<&'c str> {
        match expression {
            Expression::Identifier(identifier) => Some(identifier.name.as_str()),
            _ => None,
        }
    }

    fn is_identifier(expression: &Expression<'_>, expected: &str) -> bool {
        Self::identifier_name(expression).is_some_and(|name| name == expected)
    }

    fn assignment_target_expression(
        &self,
        target: &AssignmentTarget<'a>,
    ) -> Option<Expression<'a>> {
        match target {
            AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
                Some(Expression::Identifier(identifier.clone_in(self.allocator)))
            }
            AssignmentTarget::StaticMemberExpression(member) => Some(
                Expression::StaticMemberExpression(member.clone_in(self.allocator)),
            ),
            AssignmentTarget::ComputedMemberExpression(member) => Some(
                Expression::ComputedMemberExpression(member.clone_in(self.allocator)),
            ),
            AssignmentTarget::PrivateFieldExpression(member) => Some(
                Expression::PrivateFieldExpression(member.clone_in(self.allocator)),
            ),
            _ => None,
        }
    }
}

impl<'a, 'b> VisitMut<'a> for CustomDialogControllerLowerer<'a, 'b> {
    fn visit_property_definition(&mut self, property: &mut PropertyDefinition<'a>) {
        let property_name = property.key.static_name().map(|name| name.to_string());
        if let (Some(property_name), Some(initializer)) = (property_name, property.value.as_mut()) {
            self.rewrite_custom_dialog_controller_builder(
                initializer,
                self.builder.this_member_expression(&property_name),
            );
        }
        walk_mut::walk_property_definition(self, property);
    }

    fn visit_variable_declarator(&mut self, declarator: &mut VariableDeclarator<'a>) {
        let variable_name = declarator
            .id
            .get_binding_identifier()
            .map(|identifier| identifier.name.to_string());
        if let (Some(variable_name), Some(initializer)) = (variable_name, declarator.init.as_mut())
        {
            // SDK createCustomComponentBuilderArrowFunction intentionally
            // addresses variable declarations as `this.<name>`.
            self.rewrite_custom_dialog_controller_builder(
                initializer,
                self.builder.this_member_expression(&variable_name),
            );
        }
        walk_mut::walk_variable_declarator(self, declarator);
    }

    fn visit_assignment_expression(&mut self, assignment: &mut AssignmentExpression<'a>) {
        if let Some(controller_expression) = self.assignment_target_expression(&assignment.left) {
            self.rewrite_custom_dialog_controller_builder(
                &mut assignment.right,
                controller_expression,
            );
        }
        walk_mut::walk_assignment_expression(self, assignment);
    }
}

struct CustomDialogBuilderName;

impl CustomDialogBuilderName {
    const JS_DIALOG: &'static str = "jsDialog";
}

/// Structural description of a parsed `@Component` struct, ready for code generation.
#[derive(Debug, Clone)]
pub struct ComponentInfo<'a> {
    /// Whether this component uses Stage model partial update (ESMODULE, API >= 9).
    ///
    /// `true`  → generates `initialRender()` method (Stage model, default).
    /// `false` → generates `render()` method (FA model, JSBUNDLE, deprecated).
    ///
    /// FA model (`partial_update = false`) is not supported by arkdown;
    /// `validate_compile_mode()` in `arkdown-profile` rejects such builds.
    pub partial_update: bool,
    pub name: String,
    pub name_span: Span,
    pub entry_span: Span,
    pub build_method_span: Span,
    pub is_entry: bool,
    pub is_exported: bool,
    pub is_default_export: bool,
    pub is_reusable: bool,
    /// True when the struct is decorated with `@CustomDialog`.
    ///
    /// When set, the emitter generates a `setController(ctr: CustomDialogController)` method
    /// on the class (after field declarations, before `initialRender()`), for every regular
    /// field whose type annotation is exactly `"CustomDialogController"`.
    pub is_custom_dialog: bool,
    pub state_fields: Vec<StateField<'a>>,
    /// All struct properties in source order. hvigor adds every property to
    /// the generated `*_Params` interface from `createVariableInitStatement`,
    /// including regular/private fields and callback fields.
    pub params_fields: Vec<ParamField<'a>>,
    /// Struct property names in source order. Constructor initialization,
    /// `setInitiallyProvidedValue`, and field declarations follow this order.
    pub property_order: Vec<String>,
    /// Non-decorated struct fields. hvigor still routes them through params and
    /// emits private class fields initialized in the constructor.
    pub regular_fields: Vec<RegularField<'a>>,
    /// Struct members that are not ArkUI state/decorator fields and are not
    /// build/@Builder/@Styles methods. hvigor keeps these as class members.
    pub regular_members: Vec<RegularMember<'a>>,
    /// Placement of source-derived class members after the synthetic ViewPU
    /// lifecycle prefix. This preserves the in-place member transformation in
    /// SDK `process_component_class.js::processMembers`.
    pub source_members: Vec<ComponentSourceMember>,
    /// Body plan for the hvigor-style `initialRender()` method.
    pub initial_render_body: BuildBodyInfo<'a>,
    pub entry_registration: Option<EntryRegistrationInfo<'a>>,
    /// `@Builder` methods: desugared ArkUI DSL, emitted as regular class methods.
    pub builder_methods: Vec<BuilderMethodInfo<'a>>,
}

#[derive(Debug, Clone, Copy)]
pub enum RegularMember<'a> {
    Method(&'a MethodDefinition<'a>),
    Property(&'a PropertyDefinition<'a>),
    StaticBlock(&'a StaticBlock<'a>),
    TSIndexSignature(&'a TSIndexSignature<'a>),
    AccessorProperty(&'a AccessorProperty<'a>),
}

#[derive(Debug, Clone)]
pub enum ComponentSourceMember {
    Property(String),
    Regular(usize),
    InitialRender,
    Builder(usize),
}

impl<'a> RegularMember<'a> {
    fn to_class_element<'b>(&self, allocator: &'b Allocator) -> ClassElement<'b> {
        match self {
            Self::Method(method) => {
                GlobalContextCallRewriter::new(allocator).method_definition(method)
            }
            Self::Property(property) => {
                let ast = AstBuilder::new(allocator);
                let mut property = property.clone_in(allocator);
                property.decorators.clear();
                ClassElement::PropertyDefinition(ast.alloc(property))
            }
            Self::StaticBlock(block) => {
                let ast = AstBuilder::new(allocator);
                ClassElement::StaticBlock(ast.alloc(block.clone_in(allocator)))
            }
            Self::TSIndexSignature(signature) => {
                let ast = AstBuilder::new(allocator);
                ClassElement::TSIndexSignature(ast.alloc(signature.clone_in(allocator)))
            }
            Self::AccessorProperty(property) => {
                let ast = AstBuilder::new(allocator);
                let mut property = property.clone_in(allocator);
                property.decorators.clear();
                ClassElement::AccessorProperty(ast.alloc(property))
            }
        }
    }
}

impl Default for ComponentInfo<'_> {
    fn default() -> Self {
        Self {
            partial_update: true,
            name: Default::default(),
            name_span: SPAN,
            entry_span: SPAN,
            build_method_span: SPAN,
            is_entry: Default::default(),
            is_exported: Default::default(),
            is_default_export: Default::default(),
            is_reusable: Default::default(),
            is_custom_dialog: Default::default(),
            state_fields: Default::default(),
            params_fields: Default::default(),
            property_order: Default::default(),
            regular_fields: Default::default(),
            regular_members: Default::default(),
            source_members: Default::default(),
            initial_render_body: Default::default(),
            entry_registration: Default::default(),
            builder_methods: Default::default(),
        }
    }
}

impl ComponentInfo<'_> {
    /// Resolved property order: uses `property_order` if set, otherwise derives
    /// from state_fields + regular_fields.
    pub fn resolved_property_order(&self) -> Vec<String> {
        if self.property_order.is_empty() {
            self.state_fields
                .iter()
                .map(|f| f.name.clone())
                .chain(self.regular_fields.iter().map(|f| f.name.clone()))
                .collect()
        } else {
            self.property_order.clone()
        }
    }

    /// Resolved params fields: uses `params_fields` if set, otherwise derives
    /// from state_fields.
    pub fn resolved_params_fields(&self) -> Vec<ParamField<'_>> {
        if self.params_fields.is_empty() {
            self.state_fields
                .iter()
                .map(|f| ParamField {
                    name: f.name.clone(),
                    source_span: f.source_span,
                    name_span: f.name_span,
                    type_annotation_ast: f.type_annotation_ast,
                })
                .collect()
        } else {
            self.params_fields.clone()
        }
    }

    pub fn find_regular_field(&self, name: &str) -> Option<&RegularField<'_>> {
        self.regular_fields.iter().find(|f| f.name == name)
    }

    pub fn find_state_field(&self, name: &str) -> Option<&StateField<'_>> {
        self.state_fields.iter().find(|f| f.name == name)
    }
}

pub(crate) struct ViewPuClassEmitter<'a> {
    info: &'a ComponentInfo<'a>,
    runtime_names: &'a ArkuiRuntimeNames,
    params_type: String,
    property_order: Vec<String>,
    params_fields: Vec<ParamField<'a>>,
}

impl<'a> ViewPuClassEmitter<'a> {
    pub(crate) fn new(info: &'a ComponentInfo<'a>, runtime_names: &'a ArkuiRuntimeNames) -> Self {
        Self {
            info,
            runtime_names,
            params_type: GeneratedViewpuName::params_interface(&info.name),
            property_order: info.resolved_property_order(),
            params_fields: info.resolved_params_fields(),
        }
    }

    pub(crate) fn statements<'b>(
        self,
        allocator: &'b Allocator,
    ) -> Result<oxc_allocator::Vec<'b, oxc_ast::ast::Statement<'b>>, BuildLoweringError> {
        let ast = oxc_ast::AstBuilder::new(allocator);
        let interface_statement = self.build_params_interface_statement(ast, allocator);
        let elements = self.build_class_elements(allocator)?;
        let class_statement = ViewpuMemberAstEmitter::new(allocator)
            .class_declaration_statement_with_name_span(
                allocator.alloc_str(&self.info.name),
                self.info.name_span,
                GeneratedViewpuName::VIEWPU_BASE,
                elements,
                self.info.is_exported,
                self.info.is_default_export,
            );
        Ok(ast.vec_from_iter([interface_statement, class_statement]))
    }

    /// Build the `interface Foo_Params { ... }` statement.
    fn build_params_interface_statement<'b>(
        &self,
        ast: oxc_ast::AstBuilder<'b>,
        allocator: &'b Allocator,
    ) -> oxc_ast::ast::Statement<'b> {
        let fields = self
            .params_fields
            .iter()
            .map(|field| {
                let type_annotation = field
                    .type_annotation_ast
                    .map(ParamsInterfaceFieldType::Annotation);
                ParamsInterfaceField::with_source(
                    field.name.as_str(),
                    true,
                    type_annotation,
                    field.source_span,
                    field.name_span,
                )
            })
            .collect();
        ParamsInterfaceAstEmitter::new(&self.params_type, fields, false).statement(ast, allocator)
    }

    /// Build all class body elements in source order.
    fn build_class_elements<'b>(
        &self,
        allocator: &'b Allocator,
    ) -> Result<oxc_allocator::Vec<'b, oxc_ast::ast::ClassElement<'b>>, BuildLoweringError> {
        let ast = AstBuilder::new(allocator);
        let mut elements = ast.vec();

        // constructor
        elements.push(self.build_constructor(allocator));
        // setInitiallyProvidedValue
        elements.push(self.build_set_initially_provided_value(allocator));
        // updateStateVars
        elements.push(self.build_update_state_vars(allocator));
        // purgeVariableDependenciesOnElmtId
        elements.push(self.build_purge_variable_dependencies(allocator));
        // aboutToBeDeleted
        elements.push(self.build_about_to_be_deleted(allocator));
        // updateRecycleElmtId (conditional)
        if self.info.is_reusable {
            elements.push(self.build_update_recycle_elmt_id(allocator));
        }
        if self.info.source_members.is_empty() {
            // Compatibility path for callers that construct ComponentInfo
            // directly. Parsed components always carry source placement.
            for elem in self.build_field_declarations(allocator) {
                elements.push(elem);
            }
            for elem in self.build_set_controller_methods(allocator) {
                elements.push(elem);
            }
            for elem in self.build_regular_members(allocator) {
                elements.push(elem);
            }
            elements.push(self.build_initial_render(allocator)?);
            for elem in self.build_builder_methods(allocator)? {
                elements.push(elem);
            }
        } else {
            self.push_source_members(allocator, &mut elements)?;
        }
        // rerender
        elements.push(self.build_rerender(allocator));
        // static getEntryName (conditional)
        if self.info.is_entry {
            elements.push(self.build_entry_name(allocator));
        }

        Ok(elements)
    }

    fn push_source_members<'b>(
        &self,
        allocator: &'b Allocator,
        elements: &mut oxc_allocator::Vec<'b, ClassElement<'b>>,
    ) -> Result<(), BuildLoweringError> {
        let mut emitted_controller = false;
        for member in &self.info.source_members {
            match member {
                ComponentSourceMember::Property(name) => {
                    elements.extend(self.build_field_declaration(allocator, name));
                    if !emitted_controller
                        && self.info.is_custom_dialog
                        && self
                            .info
                            .find_regular_field(name)
                            .is_some_and(RegularField::is_custom_dialog_controller)
                    {
                        elements.extend(self.build_set_controller_methods(allocator));
                        emitted_controller = true;
                    }
                }
                ComponentSourceMember::Regular(index) => {
                    if let Some(member) = self.info.regular_members.get(*index) {
                        elements.push(member.to_class_element(allocator));
                    }
                }
                ComponentSourceMember::InitialRender => {
                    elements.push(self.build_initial_render(allocator)?);
                }
                ComponentSourceMember::Builder(index) => {
                    if let Some(method) = self.info.builder_methods.get(*index) {
                        elements.push(self.build_builder_method(allocator, method)?);
                    }
                }
            }
        }
        Ok(())
    }

    fn build_constructor<'b>(&self, allocator: &'b Allocator) -> oxc_ast::ast::ClassElement<'b> {
        let builder = ViewpuAstBuilder::new(allocator);
        let mut stmts = builder.statements();
        stmts.push(builder.super_call_statement(&[
            GeneratedViewpuName::PARENT,
            GeneratedViewpuName::LOCAL_STORAGE_INTERNAL,
            &self.runtime_names.elmt_id,
            GeneratedViewpuName::EXTRA_INFO,
        ]));

        let mut params_lambda_stmts = builder.statements();
        params_lambda_stmts.push(builder.this_assign_statement(
            &self.runtime_names.params_generator_prefix,
            builder.identifier_expression(&self.runtime_names.params_lambda),
        ));
        stmts.push(builder.if_statement(
            builder.typeof_equals_function(
                &self.runtime_names.params_lambda,
                &self.runtime_names.function_type,
            ),
            params_lambda_stmts,
        ));

        for name in &self.property_order {
            if let Some(field) = self.info.find_regular_field(name) {
                let init = field
                    .initializer
                    .as_ref()
                    .map(|initializer| {
                        self.regular_field_initializer_expression(&builder, initializer)
                    })
                    .unwrap_or_else(|| builder.undefined_expression());
                stmts.push(builder.this_assign_statement(name, init));
            } else if let Some(field) = self.info.find_state_field(name) {
                self.push_state_constructor_init(&builder, &mut stmts, field);
                self.push_provide_var_registration(&builder, &mut stmts, field);
            }
        }
        stmts.push(builder.this_call_statement(
            GeneratedViewpuName::SET_INITIALLY_PROVIDED_VALUE,
            builder.expressions_from_identifiers(&[GeneratedViewpuName::PARAMS]),
        ));
        for field in &self.info.state_fields {
            if let Some(handler) = &field.watch_handler {
                stmts.push(builder.this_call_statement(
                    GeneratedViewpuName::DECLARE_WATCH,
                    builder.expressions_from_iter([
                        builder.string_expression(&field.name),
                        builder.this_member_expression(handler),
                    ]),
                ));
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
            builder.formal_parameter_with_initializer(
                &self.runtime_names.params_lambda,
                None,
                Some(builder.undefined_expression()),
            ),
            builder.formal_parameter(GeneratedViewpuName::EXTRA_INFO),
        ]);
        ViewpuMemberAstEmitter::new(allocator).method_with_params_and_source_span(
            GeneratedViewpuName::CONSTRUCTOR,
            params,
            stmts,
            false,
            None,
            self.info.entry_span,
            SPAN,
        )
    }

    fn build_set_initially_provided_value<'b>(
        &self,
        allocator: &'b Allocator,
    ) -> oxc_ast::ast::ClassElement<'b> {
        let builder = ViewpuAstBuilder::new(allocator);
        let mut stmts = builder.statements();
        for name in &self.property_order {
            if self
                .info
                .find_state_field(name)
                .is_some_and(|field| field.decorator == ArkuiDecoratorName::OBJECT_LINK)
            {
                // SDK process_component_member.js::createUpdateParams selects
                // createUpdateParamsWithSet for @ObjectLink. The backing
                // SynchedPropertyNesedObjectPU must receive every parent value,
                // including `undefined`, without going through a field setter.
                stmts.push(builder.this_backing_field_call_statement(
                    name,
                    GeneratedViewpuName::SET,
                    builder.expressions_from_iter([builder.params_field_expression(name)]),
                ));
                continue;
            }
            if self.has_initially_provided_value_slot(name) {
                let mut consequent = builder.statements();
                consequent.push(
                    builder.this_assign_statement(name, builder.params_field_expression(name)),
                );
                stmts.push(
                    builder.if_statement(builder.params_field_not_undefined(name), consequent),
                );
            }
            if let Some(field) = self.info.find_state_field(name) {
                self.push_prop_default_fallback(&builder, &mut stmts, field);
            }
        }

        let params = builder.formal_parameters_from_iter([builder
            .formal_parameter_with_initializer(
                GeneratedViewpuName::PARAMS,
                Some(builder.type_reference_annotation(&self.params_type)),
                None,
            )]);
        ViewpuMemberAstEmitter::new(allocator).method_with_params(
            GeneratedViewpuName::SET_INITIALLY_PROVIDED_VALUE,
            params,
            stmts,
            false,
            None,
        )
    }

    fn build_update_state_vars<'b>(
        &self,
        allocator: &'b Allocator,
    ) -> oxc_ast::ast::ClassElement<'b> {
        let builder = ViewpuAstBuilder::new(allocator);
        let mut stmts = builder.statements();
        for field in
            self.info.state_fields.iter().filter(|field| {
                ArkuiStateDecorator::new(&field.decorator).has_update_state_vars_slot()
            })
        {
            if field.decorator == ArkuiDecoratorName::STATE {
                let mut consequent = builder.statements();
                consequent.push(builder.this_backing_field_call_statement(
                    &field.name,
                    GeneratedViewpuName::SET,
                    builder.expressions_from_iter([builder.params_field_expression(&field.name)]),
                ));
                stmts.push(
                    builder.if_statement(builder.params_field_truthy(&field.name), consequent),
                );
            } else {
                stmts.push(builder.this_backing_field_call_statement(
                    &field.name,
                    Self::update_state_vars_api(field),
                    builder.expressions_from_iter([builder.params_field_expression(&field.name)]),
                ));
            }
        }

        let params = builder.formal_parameters_from_iter([builder
            .formal_parameter_with_initializer(
                GeneratedViewpuName::PARAMS,
                Some(builder.type_reference_annotation(&self.params_type)),
                None,
            )]);
        ViewpuMemberAstEmitter::new(allocator).method_with_params(
            GeneratedViewpuName::UPDATE_STATE_VARS,
            params,
            stmts,
            false,
            None,
        )
    }

    fn build_purge_variable_dependencies<'b>(
        &self,
        allocator: &'b Allocator,
    ) -> oxc_ast::ast::ClassElement<'b> {
        let builder = ViewpuAstBuilder::new(allocator);
        let mut stmts = builder.statements();
        for name in self.backing_field_names() {
            stmts.push(builder.this_backing_field_call_statement(
                &name,
                GeneratedViewpuName::PURGE_DEPENDENCY_ON_ELMT_ID,
                builder.expressions_from_identifiers(&[GeneratedViewpuName::RM_ELMT_ID]),
            ));
        }
        let params = builder.formal_parameters_from_iter([
            builder.formal_parameter(GeneratedViewpuName::RM_ELMT_ID)
        ]);
        ViewpuMemberAstEmitter::new(allocator).method_with_params(
            GeneratedViewpuName::PURGE_VARIABLE_DEPENDENCIES_ON_ELMT_ID,
            params,
            stmts,
            false,
            None,
        )
    }

    fn build_about_to_be_deleted<'b>(
        &self,
        allocator: &'b Allocator,
    ) -> oxc_ast::ast::ClassElement<'b> {
        let builder = ViewpuAstBuilder::new(allocator);
        let mut stmts = builder.statements();
        for name in self.backing_field_names() {
            stmts.push(builder.this_backing_field_call_statement(
                &name,
                GeneratedViewpuName::ABOUT_TO_BE_DELETED,
                builder.expressions(),
            ));
        }
        stmts.push(builder.subscriber_manager_delete_statement());
        stmts.push(builder.this_call_statement(
            GeneratedViewpuName::ABOUT_TO_BE_DELETED_INTERNAL,
            builder.expressions(),
        ));
        ViewpuMemberAstEmitter::new(allocator).method_with_params(
            GeneratedViewpuName::ABOUT_TO_BE_DELETED,
            builder.formal_parameters(),
            stmts,
            false,
            None,
        )
    }

    fn build_update_recycle_elmt_id<'b>(
        &self,
        allocator: &'b Allocator,
    ) -> oxc_ast::ast::ClassElement<'b> {
        let builder = ViewpuAstBuilder::new(allocator);
        let mut stmts = builder.statements();
        for name in self.backing_field_names() {
            stmts.push(builder.this_backing_field_call_statement(
                &name,
                GeneratedViewpuName::UPDATE_ELMT_ID,
                builder.expressions_from_identifiers(&[
                    GeneratedViewpuName::OLD_ELMT_ID,
                    GeneratedViewpuName::NEW_ELMT_ID,
                ]),
            ));
        }
        let params = builder.formal_parameters_from_iter([
            builder.formal_parameter(GeneratedViewpuName::OLD_ELMT_ID),
            builder.formal_parameter(GeneratedViewpuName::NEW_ELMT_ID),
        ]);
        ViewpuMemberAstEmitter::new(allocator).method_with_params(
            GeneratedViewpuName::UPDATE_RECYCLE_ELMT_ID,
            params,
            stmts,
            false,
            None,
        )
    }

    fn build_field_declarations<'b>(
        &self,
        allocator: &'b Allocator,
    ) -> Vec<oxc_ast::ast::ClassElement<'b>> {
        let mut elements = Vec::new();
        for name in &self.property_order {
            elements.extend(self.build_field_declaration(allocator, name));
        }
        elements
    }

    fn build_field_declaration<'b>(
        &self,
        allocator: &'b Allocator,
        property_name: &str,
    ) -> Vec<ClassElement<'b>> {
        let mut elements = Vec::new();
        if let Some(field) = self.info.find_regular_field(property_name) {
            let name = allocator.alloc_str(&field.name);
            let type_annotation = field.type_annotation_ast.map(|type_annotation| {
                ViewpuAstBuilder::new(allocator).clone_type_annotation(type_annotation)
            });
            // SDK ets-loader parity:
            // process_component_class.js::addPropertyMember/createPropertyDeclaration
            // emits regular field declarations without initializers. The initializer is
            // emitted by process_component_member.js::createVariableInitStatement into
            // the generated constructor, after state-backed fields exist.
            elements.push(
                ViewpuMemberAstEmitter::new(allocator).property_with_type_annotation(
                    name,
                    type_annotation,
                    None,
                    true,
                    field.optional,
                ),
            );
        } else if let Some(field) = self.info.find_state_field(property_name) {
            self.push_state_field_declaration(allocator, &mut elements, field);
        }
        elements
    }

    fn build_set_controller_methods<'b>(
        &self,
        allocator: &'b Allocator,
    ) -> Vec<oxc_ast::ast::ClassElement<'b>> {
        if !self.info.is_custom_dialog {
            return Vec::new();
        }
        let mut elements = Vec::new();
        for field in &self.info.regular_fields {
            if field.is_custom_dialog_controller() {
                let builder = ViewpuAstBuilder::new(allocator);
                let mut stmts = builder.statements();
                stmts.push(builder.this_assign_statement(
                    &field.name,
                    builder.identifier_expression(GeneratedViewpuName::CONTROLLER),
                ));
                let params = builder.formal_parameters_from_iter([builder
                    .formal_parameter_with_initializer(
                        GeneratedViewpuName::CONTROLLER,
                        Some(builder.type_reference_annotation(
                            ArkuiRuntimeTypeName::CUSTOM_DIALOG_CONTROLLER,
                        )),
                        None,
                    )]);
                elements.push(ViewpuMemberAstEmitter::new(allocator).method_with_params(
                    GeneratedViewpuName::SET_CONTROLLER,
                    params,
                    stmts,
                    false,
                    None,
                ));
                break;
            }
        }
        elements
    }

    fn build_regular_members<'b>(
        &self,
        allocator: &'b Allocator,
    ) -> Vec<oxc_ast::ast::ClassElement<'b>> {
        let mut elements = Vec::new();
        for member in &self.info.regular_members {
            elements.push(member.to_class_element(allocator));
        }
        elements
    }

    fn build_initial_render<'b>(
        &self,
        allocator: &'b Allocator,
    ) -> Result<oxc_ast::ast::ClassElement<'b>, BuildLoweringError> {
        let stmts = self.info.initial_render_body.statements(allocator)?;
        let method_name = if self.info.partial_update {
            GeneratedViewpuName::INITIAL_RENDER
        } else {
            GeneratedViewpuName::RENDER
        };
        let builder = ViewpuAstBuilder::new(allocator);
        Ok(
            ViewpuMemberAstEmitter::new(allocator).method_with_params_and_source_span(
                method_name,
                builder.formal_parameters(),
                stmts,
                false,
                None,
                self.info.build_method_span,
                SPAN,
            ),
        )
    }

    fn build_builder_methods<'b>(
        &self,
        allocator: &'b Allocator,
    ) -> Result<Vec<oxc_ast::ast::ClassElement<'b>>, BuildLoweringError> {
        let mut elements = Vec::new();
        for method in &self.info.builder_methods {
            elements.push(self.build_builder_method(allocator, method)?);
        }
        Ok(elements)
    }

    fn build_builder_method<'b>(
        &self,
        allocator: &'b Allocator,
        method: &BuilderMethodInfo<'_>,
    ) -> Result<oxc_ast::ast::ClassElement<'b>, BuildLoweringError> {
        let name = allocator.alloc_str(&method.name);
        let stmts = method.body.statements(allocator)?;
        let builder = ViewpuAstBuilder::new(allocator);
        let params = builder.clone_formal_parameters_with_parent(method.params);
        let return_type = method
            .return_annotation
            .map(|annotation| builder.clone_type_annotation(annotation));
        Ok(
            ViewpuMemberAstEmitter::new(allocator).method_with_formal_parameters(
                name,
                params,
                return_type,
                stmts,
                false,
                None,
            ),
        )
    }

    fn build_rerender<'b>(&self, allocator: &'b Allocator) -> oxc_ast::ast::ClassElement<'b> {
        let builder = ViewpuAstBuilder::new(allocator);
        let mut stmts = builder.statements();
        stmts.push(builder.this_call_statement(
            GeneratedViewpuName::UPDATE_DIRTY_ELEMENTS,
            builder.expressions(),
        ));
        ViewpuMemberAstEmitter::new(allocator).method_with_params(
            GeneratedViewpuName::RERENDER,
            builder.formal_parameters(),
            stmts,
            false,
            None,
        )
    }

    fn build_entry_name<'b>(&self, allocator: &'b Allocator) -> oxc_ast::ast::ClassElement<'b> {
        let builder = ViewpuAstBuilder::new(allocator);
        let mut stmts = builder.statements();
        stmts.push(builder.return_string_statement(&self.info.name));
        ViewpuMemberAstEmitter::new(allocator).method_with_type_annotation(
            GeneratedViewpuName::GET_ENTRY_NAME,
            builder.formal_parameters(),
            Some(builder.string_type_annotation()),
            stmts,
            true,
            None,
        )
    }

    fn push_state_field_declaration<'b>(
        &self,
        allocator: &'b Allocator,
        elements: &mut Vec<oxc_ast::ast::ClassElement<'b>>,
        field: &StateField,
    ) {
        if field.decorator == ArkuiDecoratorName::BUILDER_PARAM {
            let builder = ViewpuAstBuilder::new(allocator);
            let backing_name = builder.backing_field_name(&field.name);
            elements.push(
                ViewpuMemberAstEmitter::new(allocator).property_with_type_annotation(
                    backing_name,
                    None,
                    None,
                    true,
                    field.optional,
                ),
            );
            return;
        }

        let backing =
            ArkuiStateDecorator::new(&field.decorator).declaration_class_for(field.is_simple_type);
        let builder = ViewpuAstBuilder::new(allocator);
        let backing_name = builder.backing_field_name(&field.name);
        let initializer = if Self::uses_local_storage(field) {
            Some(self.local_storage_initializer_expression(&builder, field))
        } else {
            None
        };
        let type_annotation = field
            .type_annotation_ast
            .map(|annotation| builder.state_property_type_annotation(backing, Some(annotation)))
            .unwrap_or_else(|| builder.state_property_type_annotation(backing, None));
        elements.push(
            ViewpuMemberAstEmitter::new(allocator).property_with_type_annotation_and_source_span(
                backing_name,
                Some(type_annotation),
                initializer,
                true,
                field.optional,
                field.source_span,
                SPAN,
            ),
        );

        if ArkuiStateDecorator::new(&field.decorator).is_state() {
            self.push_state_accessors(allocator, elements, field);
        }
    }

    fn push_state_accessors<'b>(
        &self,
        allocator: &'b Allocator,
        elements: &mut Vec<oxc_ast::ast::ClassElement<'b>>,
        field: &StateField,
    ) {
        let builder = ViewpuAstBuilder::new(allocator);
        let mut getter_stmts = builder.statements();
        getter_stmts.push(builder.return_expression_statement(
            builder.this_backing_field_call_expression(
                &field.name,
                GeneratedViewpuName::GET,
                builder.expressions(),
            ),
        ));
        elements.push(
            ViewpuMemberAstEmitter::new(allocator).getter_with_source_span(
                allocator.alloc_str(&field.name),
                getter_stmts,
                None,
                field.name_span,
                field.name_span,
            ),
        );

        // SDK process_component_member.js::immutableDecorators contains
        // @ObjectLink. Its SynchedPropertyNesedObjectPU is updated through
        // backing `.set(...)` calls and exposes no generated field setter.
        if field.decorator == ArkuiDecoratorName::OBJECT_LINK {
            return;
        }

        let mut setter_stmts = builder.statements();
        setter_stmts.push(builder.this_backing_field_call_statement(
            &field.name,
            GeneratedViewpuName::SET,
            builder.expressions_from_identifiers(&[GeneratedViewpuName::NEW_VALUE]),
        ));
        let param = if field.type_annotation_ast.is_some() {
            builder.formal_parameter_with_type_annotation(
                GeneratedViewpuName::NEW_VALUE,
                field.type_annotation_ast,
            )
        } else {
            builder.formal_parameter_with_initializer(
                GeneratedViewpuName::NEW_VALUE,
                Some(builder.unknown_type_annotation()),
                None,
            )
        };
        elements.push(
            ViewpuMemberAstEmitter::new(allocator).setter_with_param_and_source_span(
                allocator.alloc_str(&field.name),
                param,
                setter_stmts,
                None,
                field.source_span,
                field.name_span,
            ),
        );
    }

    fn push_state_constructor_init<'b>(
        &self,
        builder: &ViewpuAstBuilder<'b>,
        stmts: &mut oxc_allocator::Vec<'b, oxc_ast::ast::Statement<'b>>,
        field: &StateField,
    ) {
        let state_decorator = ArkuiStateDecorator::new(&field.decorator);
        let backing = state_decorator.class_for(field.is_simple_type);
        match state_decorator.constructor_init_kind() {
            ConstructorInitKind::NewWithLocalInit => {
                let value = builder.new_expression(
                    backing,
                    builder.expressions_from_iter([
                        self.field_initializer_expression(builder, field),
                        builder.this_expression(),
                        builder.string_expression(&field.name),
                    ]),
                );
                stmts.push(
                    builder.this_assign_statement(builder.backing_field_name(&field.name), value),
                );
            }
            ConstructorInitKind::NewWithParamInit => {
                let value = builder.new_expression(
                    backing,
                    builder.expressions_from_iter([
                        builder.params_field_expression(&field.name),
                        builder.this_expression(),
                        builder.string_expression(&field.name),
                    ]),
                );
                stmts.push(
                    builder.this_assign_statement(builder.backing_field_name(&field.name), value),
                );
            }
            ConstructorInitKind::ConsumeInit => {
                let key = field.alias.as_deref().unwrap_or(field.name.as_str());
                let mut args = builder.expressions_from_iter([
                    builder.string_expression(key),
                    builder.string_expression(&field.name),
                ]);
                if field.initializer.is_some() {
                    args.push(self.field_initializer_expression(builder, field));
                }
                let value =
                    builder.this_call_expression(GeneratedViewpuName::INITIALIZE_CONSUME, args);
                stmts.push(
                    builder.this_assign_statement(builder.backing_field_name(&field.name), value),
                );
            }
            ConstructorInitKind::BuilderParamInit => {
                stmts.push(builder.this_assign_statement(
                    &field.name,
                    self.field_initializer_expression(builder, field),
                ));
            }
            ConstructorInitKind::StorageInit => {
                if Self::uses_local_storage(field) {
                    return;
                }
                let value = builder.this_call_expression(
                    Self::storage_api(field),
                    builder.expressions_from_iter([
                        self.storage_key_expression(builder, field),
                        self.field_initializer_expression(builder, field),
                        builder.string_expression(&field.name),
                    ]),
                );
                stmts.push(
                    builder.this_assign_statement(builder.backing_field_name(&field.name), value),
                );
            }
        }
    }

    fn push_provide_var_registration<'b>(
        &self,
        builder: &ViewpuAstBuilder<'b>,
        stmts: &mut oxc_allocator::Vec<'b, oxc_ast::ast::Statement<'b>>,
        field: &StateField,
    ) {
        if field.decorator != ArkuiDecoratorName::PROVIDE {
            return;
        }
        if let Some(alias) = &field.alias {
            if alias != &field.name {
                stmts.push(self.add_provided_var_statement(builder, alias, &field.name));
            }
        }
        stmts.push(self.add_provided_var_statement(builder, &field.name, &field.name));
    }

    fn add_provided_var_statement<'b>(
        &self,
        builder: &ViewpuAstBuilder<'b>,
        provided_name: &str,
        field_name: &str,
    ) -> oxc_ast::ast::Statement<'b> {
        builder.this_call_statement(
            GeneratedViewpuName::ADD_PROVIDED_VAR,
            builder.expressions_from_iter([
                builder.string_expression(provided_name),
                builder.this_backing_member_expression(field_name),
                builder.boolean_expression(false),
            ]),
        )
    }

    fn push_prop_default_fallback<'b>(
        &self,
        builder: &ViewpuAstBuilder<'b>,
        stmts: &mut oxc_allocator::Vec<'b, oxc_ast::ast::Statement<'b>>,
        field: &StateField,
    ) {
        if field.decorator != ArkuiDecoratorName::PROP {
            return;
        }
        let Some(init) = field.initializer.as_ref() else {
            return;
        };
        let mut consequent = builder.statements();
        consequent.push(builder.this_backing_field_call_statement(
            &field.name,
            GeneratedViewpuName::SET,
            builder.expressions_from_iter([init.to_expression(builder)]),
        ));
        stmts.push(builder.if_statement(
            builder.params_field_equals_undefined(&field.name),
            consequent,
        ));
    }

    fn storage_key_expression<'b>(
        &self,
        builder: &ViewpuAstBuilder<'b>,
        field: &StateField,
    ) -> oxc_ast::ast::Expression<'b> {
        field
            .decorator_arg
            .as_ref()
            .map(|arg| arg.to_expression(builder))
            .unwrap_or_else(|| builder.string_expression(&field.name))
    }

    fn storage_api(field: &StateField) -> &'static str {
        match field.decorator.as_str() {
            ArkuiDecoratorName::STORAGE_LINK | ArkuiDecoratorName::LOCAL_STORAGE_LINK => {
                GeneratedViewpuName::CREATE_STORAGE_LINK
            }
            _ => GeneratedViewpuName::CREATE_STORAGE_PROP,
        }
    }

    fn local_storage_initializer_expression<'b>(
        &self,
        builder: &ViewpuAstBuilder<'b>,
        field: &StateField,
    ) -> oxc_ast::ast::Expression<'b> {
        let local_init = field
            .initializer
            .as_ref()
            .map(|init| init.to_expression(builder))
            .unwrap_or_else(|| builder.undefined_expression());
        let args = builder.expressions_from_iter([
            self.storage_key_expression(builder, field),
            local_init,
            builder.string_expression(&field.name),
        ]);
        if field.type_annotation_ast.is_some() {
            builder.this_call_expression_with_type_annotation_arg(
                Self::local_storage_api(field),
                field.type_annotation_ast,
                args,
            )
        } else {
            builder.this_call_expression(Self::local_storage_api(field), args)
        }
    }

    fn field_initializer_expression<'b>(
        &self,
        builder: &ViewpuAstBuilder<'b>,
        field: &StateField<'_>,
    ) -> Expression<'b> {
        field
            .initializer
            .as_ref()
            .map(|initializer| initializer.to_expression(builder))
            .unwrap_or_else(|| builder.undefined_expression())
    }

    fn regular_field_initializer_expression<'b>(
        &self,
        builder: &ViewpuAstBuilder<'b>,
        initializer: &FieldExpression<'_>,
    ) -> Expression<'b> {
        initializer.to_expression(builder)
    }

    fn local_storage_api(field: &StateField) -> &'static str {
        match field.decorator.as_str() {
            ArkuiDecoratorName::LOCAL_STORAGE_LINK => {
                GeneratedViewpuName::CREATE_LOCAL_STORAGE_LINK
            }
            _ => GeneratedViewpuName::CREATE_LOCAL_STORAGE_PROP,
        }
    }

    fn uses_local_storage(field: &StateField) -> bool {
        matches!(
            field.decorator.as_str(),
            ArkuiDecoratorName::LOCAL_STORAGE_LINK
                | ArkuiDecoratorName::LOCAL_STORAGE_PROP
                | ArkuiDecoratorName::LOCAL_STORAGE_PROP_REF
        )
    }

    fn has_initially_provided_value_slot(&self, name: &str) -> bool {
        self.info.find_regular_field(name).is_some()
            || self.info.find_state_field(name).is_some_and(|field| {
                ArkuiStateDecorator::new(&field.decorator).has_initially_provided_value_slot()
            })
    }

    fn backing_field_names(&self) -> Vec<String> {
        self.info
            .state_fields
            .iter()
            .filter(|field| field.decorator != ArkuiDecoratorName::BUILDER_PARAM)
            .map(|field| field.name.clone())
            .collect()
    }

    fn update_state_vars_api(field: &StateField) -> &'static str {
        if field.decorator == ArkuiDecoratorName::OBJECT_LINK {
            // Hvigor's generated ViewPU updateStateVars body uses `.set(...)`
            // for SynchedPropertyNesedObjectPU. Prop/Param-style one-way
            // wrappers use `.reset(...)` instead.
            GeneratedViewpuName::SET
        } else if ArkuiDecoratorName::contributes_update_state_vars(&field.decorator) {
            GeneratedViewpuName::RESET
        } else {
            GeneratedViewpuName::SET
        }
    }
}
