//! ViewPU AST emission helpers.
//!
//! This module is the transition boundary between ArkUI lowering decisions and
//! generated code. The lowering code should call these helpers for generated
//! ViewPU statements so the final text is printed by OXC codegen.

use std::borrow::Cow;

use oxc_allocator::{Allocator, ArenaBox, CloneIn, GetAllocator, Vec as OxcVec};
use oxc_ast::ast::{
    Argument, ArrowFunctionBody, BindingIdentifier, BindingPattern, Class, ClassBody, ClassElement,
    ClassHeritage, ClassType, Declaration, Decorator, ExportDeclaration, ExportDefaultDeclaration,
    Expression, FormalParameter, FormalParameterKind, FormalParameters, Function, FunctionBody,
    FunctionType, IdentifierName, MemberExpression, MethodDefinitionKind, MethodDefinitionType,
    NumberBase, PropertyDefinitionType, PropertyKey, Statement, TSAccessibility, TSInterfaceBody,
    TSInterfaceDeclaration, TSSignature, TSType, TSTypeAnnotation, TSTypeName,
    TSTypeParameterInstantiation,
};
use oxc_ast::builder::AstBuilder;
use oxc_span::{Span, SPAN};
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, LogicalOperator, UnaryOperator};

use crate::transform::component_meta::ArkuiRuntimeNames;

pub(crate) struct GeneratedViewpuName;

impl GeneratedViewpuName {
    pub(crate) const VIEWPU_BASE: &'static str = "ViewPU";
    pub(crate) const VIEW_V2_BASE: &'static str = "ViewV2";
    pub(crate) const REUSABLE_V2_INNER_DECORATOR: &'static str = "__ReusableV2_Inner_Decorator__";
    pub(crate) const REUSE_OR_CREATE_NEW_COMPONENT: &'static str = "reuseOrCreateNewComponent";
    pub(crate) const COMPONENT_CLASS: &'static str = "componentClass";
    pub(crate) const GET_PARAMS: &'static str = "getParams";
    pub(crate) const GET_REUSE_ID: &'static str = "getReuseId";
    pub(crate) const ENABLE_V2_COMPATIBILITY: &'static str = "enableV2Compatibility";
    pub(crate) const MAKE_V1_OBSERVED: &'static str = "makeV1Observed";
    pub(crate) const CAN_BE_OBSERVED: &'static str = "canBeObserved";
    pub(crate) const BASE_CLASS: &'static str = "BaseClass";
    pub(crate) const PARENT: &'static str = "parent";
    pub(crate) const PARAMS: &'static str = "params";
    pub(crate) const EXTRA_INFO: &'static str = "extraInfo";
    pub(crate) const LOCAL_STORAGE_INTERNAL: &'static str = "__localStorage";
    pub(crate) const INITIAL_RENDER: &'static str = "initialRender";
    pub(crate) const RENDER: &'static str = "render";
    pub(crate) const BUILD: &'static str = "build";
    pub(crate) const CONSTRUCTOR: &'static str = "constructor";
    pub(crate) const RERENDER: &'static str = "rerender";
    pub(crate) const UPDATE_DIRTY_ELEMENTS: &'static str = "updateDirtyElements";
    pub(crate) const GET_ENTRY_NAME: &'static str = "getEntryName";
    pub(crate) const FINALIZE_CONSTRUCTION: &'static str = "finalizeConstruction";
    pub(crate) const INIT_ALLOW_COMPONENT_FREEZE: &'static str = "initAllowComponentFreeze";
    pub(crate) const SET_REUSABLE_MEMORY_OPTIMIZATION: &'static str =
        "__setReusableMemOptStrategy__Internal";
    pub(crate) const SET_INITIALLY_PROVIDED_VALUE: &'static str = "setInitiallyProvidedValue";
    pub(crate) const UPDATE_STATE_VARS: &'static str = "updateStateVars";
    pub(crate) const INIT_PARAM: &'static str = "initParam";
    pub(crate) const UPDATE_PARAM: &'static str = "updateParam";
    pub(crate) const RESET_PARAM: &'static str = "resetParam";
    pub(crate) const RESET_STATE_VARS_ON_REUSE: &'static str = "resetStateVarsOnReuse";
    pub(crate) const RESET_CONSUMER: &'static str = "resetConsumer";
    pub(crate) const RESET_COMPUTED: &'static str = "resetComputed";
    pub(crate) const RESET_MONITORS_ON_REUSE: &'static str = "resetMonitorsOnReuse";
    pub(crate) const PURGE_VARIABLE_DEPENDENCIES_ON_ELMT_ID: &'static str =
        "purgeVariableDependenciesOnElmtId";
    pub(crate) const PURGE_DEPENDENCY_ON_ELMT_ID: &'static str = "purgeDependencyOnElmtId";
    pub(crate) const ABOUT_TO_BE_DELETED: &'static str = "aboutToBeDeleted";
    pub(crate) const ABOUT_TO_BE_DELETED_INTERNAL: &'static str = "aboutToBeDeletedInternal";
    pub(crate) const UPDATE_RECYCLE_ELMT_ID: &'static str = "updateRecycleElmtId";
    pub(crate) const UPDATE_ELMT_ID: &'static str = "updateElmtId";
    pub(crate) const DECLARE_WATCH: &'static str = "declareWatch";
    pub(crate) const INITIALIZE_CONSUME: &'static str = "initializeConsume";
    pub(crate) const ADD_PROVIDED_VAR: &'static str = "addProvidedVar";
    pub(crate) const CREATE_STORAGE_LINK: &'static str = "createStorageLink";
    pub(crate) const CREATE_STORAGE_PROP: &'static str = "createStorageProp";
    pub(crate) const CREATE_LOCAL_STORAGE_LINK: &'static str = "createLocalStorageLink";
    pub(crate) const CREATE_LOCAL_STORAGE_PROP: &'static str = "createLocalStorageProp";
    pub(crate) const PROTOTYPE: &'static str = "prototype";
    pub(crate) const REFLECT: &'static str = "Reflect";
    pub(crate) const DEFINE_PROPERTY: &'static str = "defineProperty";
    pub(crate) const IS_REUSABLE: &'static str = "isReusable_";
    pub(crate) const GET: &'static str = "get";
    pub(crate) const SET: &'static str = "set";
    pub(crate) const RESET: &'static str = "reset";
    pub(crate) const DELETE: &'static str = "delete";
    pub(crate) const NEW_VALUE: &'static str = "newValue";
    pub(crate) const UNDEFINED: &'static str = "undefined";
    pub(crate) const SOURCE_PAGE: &'static str = "page";
    pub(crate) const SOURCE_LINE: &'static str = "line";
    pub(crate) const SOURCE_COL: &'static str = "col";
    pub(crate) const SUBSCRIBER_MANAGER: &'static str = "SubscriberManager";
    pub(crate) const SUBSCRIBER_MANAGER_GET: &'static str = "Get";
    pub(crate) const ID_ACCESSOR: &'static str = "id__";
    pub(crate) const SET_CONTROLLER: &'static str = "setController";
    pub(crate) const RM_ELMT_ID: &'static str = "rmElmtId";
    pub(crate) const OLD_ELMT_ID: &'static str = "oldElmtId";
    pub(crate) const NEW_ELMT_ID: &'static str = "newElmtId";
    pub(crate) const CONTROLLER: &'static str = "ctr";

    const PARAMS_INTERFACE_SUFFIX: &'static str = "_Params";

    pub(crate) fn backing_field(name: &str) -> String {
        GeneratedViewpuIdentifier::from_parts(["__", name])
    }

    pub(crate) fn backing_field_alloc<'a>(allocator: &'a Allocator, name: &str) -> &'a str {
        allocator.alloc_str(&Self::backing_field(name))
    }

    pub(crate) fn extend_function(component: &str, name: &str) -> String {
        GeneratedViewpuIdentifier::from_parts(["__", component, "__", name])
    }

    pub(crate) fn params_interface(component: &str) -> String {
        GeneratedViewpuIdentifier::from_parts([component, Self::PARAMS_INTERFACE_SUFFIX])
    }

    pub(crate) fn params_interface_component(name: &str) -> Option<&str> {
        name.strip_suffix(Self::PARAMS_INTERFACE_SUFFIX)
            .filter(|component| !component.is_empty())
    }
}

struct GeneratedViewpuIdentifier;

impl GeneratedViewpuIdentifier {
    fn from_parts<const N: usize>(parts: [&str; N]) -> String {
        parts.into_iter().collect()
    }
}

pub(crate) struct ViewpuStatementAstEmitter;

impl ViewpuStatementAstEmitter {
    pub(crate) fn observe_component_creation_ast_statement<'b>(
        ast: AstBuilder<'b>,
        use_parent_context: bool,
        global_builder_parameter: Option<&str>,
        component_name: &str,
        runtime_names: &ArkuiRuntimeNames,
        body_stmts: OxcVec<'b, Statement<'b>>,
    ) -> Statement<'b> {
        let object = ViewpuAstEmitter::view_context_expression(&ast, use_parent_context);
        let property = IdentifierName::new(
            SPAN,
            ast.allocator()
                .alloc_str(&runtime_names.observe_component_creation2),
            &ast,
        );
        let member =
            MemberExpression::new_static_member_expression(SPAN, object, property, false, &ast);
        let callee = ViewpuAstEmitter::member_expression_to_expression(member);
        let arrow = ViewpuAstEmitter::observe_arrow_function_with_statements(
            &ast,
            runtime_names,
            global_builder_parameter,
            body_stmts,
        );
        let component =
            Expression::new_identifier(SPAN, ast.allocator().alloc_str(component_name), &ast);
        let arguments = OxcVec::from_iter_in(
            [
                ViewpuAstEmitter::argument_from_expression(arrow),
                ViewpuAstEmitter::argument_from_expression(component),
            ],
            &ast,
        );
        let call = Expression::new_call_expression(SPAN, callee, None, arguments, false, &ast);
        Statement::new_expression_statement(SPAN, call, &ast)
    }
}

struct ViewpuAstEmitter;

impl ViewpuAstEmitter {
    fn view_context_expression<'b>(
        ast: &AstBuilder<'b>,
        use_parent_context: bool,
    ) -> Expression<'b> {
        if !use_parent_context {
            return Expression::new_this_expression(SPAN, ast);
        }

        let test = Expression::new_identifier(SPAN, GeneratedViewpuName::PARENT, ast);
        let consequent = Expression::new_identifier(SPAN, GeneratedViewpuName::PARENT, ast);
        let alternate = Expression::new_this_expression(SPAN, ast);
        let conditional =
            Expression::new_conditional_expression(SPAN, test, consequent, alternate, ast);
        Expression::new_parenthesized_expression(SPAN, conditional, ast)
    }

    fn observe_arrow_function_with_statements<'b>(
        ast: &AstBuilder<'b>,
        runtime_names: &ArkuiRuntimeNames,
        global_builder_parameter: Option<&str>,
        statements: OxcVec<'b, Statement<'b>>,
    ) -> Expression<'b> {
        let mut params = FormalParameters::new(
            SPAN,
            FormalParameterKind::ArrowFormalParameters,
            OxcVec::with_capacity_in(2, ast),
            None,
            ast,
        );
        params
            .items
            .push(Self::formal_parameter(ast, &runtime_names.elmt_id));
        params.items.push(Self::formal_parameter(
            ast,
            &runtime_names.is_initial_render,
        ));
        if let Some(parameter) = global_builder_parameter {
            let capture_name = format!("__{parameter}__");
            params.items.push(FormalParameter::new(
                SPAN,
                OxcVec::new_in(ast),
                BindingPattern::new_binding_identifier(
                    SPAN,
                    ast.allocator().alloc_str(parameter),
                    ast,
                ),
                None,
                Some(ArenaBox::new_in(
                    Expression::new_identifier(SPAN, ast.allocator().alloc_str(&capture_name), ast),
                    ast,
                )),
                false,
                None,
                false,
                false,
                ast,
            ));
        }
        let body = ArrowFunctionBody::new_function_body(SPAN, OxcVec::new_in(ast), statements, ast);
        Expression::new_arrow_function_expression(
            SPAN,
            false,
            None,
            ArenaBox::new_in(params, ast),
            None,
            body,
            ast,
        )
    }

    fn formal_parameter<'b>(ast: &AstBuilder<'b>, name: &str) -> oxc_ast::ast::FormalParameter<'b> {
        FormalParameter::new(
            SPAN,
            OxcVec::new_in(ast),
            BindingPattern::new_binding_identifier(SPAN, ast.allocator().alloc_str(name), ast),
            None,
            None,
            false,
            None,
            false,
            false,
            ast,
        )
    }

    fn argument_from_expression<'b>(expression: Expression<'b>) -> Argument<'b> {
        match expression {
            Expression::Identifier(value) => Argument::Identifier(value),
            Expression::ArrowFunctionExpression(value) => Argument::ArrowFunctionExpression(value),
            _ => panic!("generated ViewPU observe arguments must be identifiers or arrows"),
        }
    }

    fn member_expression_to_expression<'b>(member: MemberExpression<'b>) -> Expression<'b> {
        match member {
            MemberExpression::ComputedMemberExpression(member) => {
                Expression::ComputedMemberExpression(member)
            }
            MemberExpression::StaticMemberExpression(member) => {
                Expression::StaticMemberExpression(member)
            }
            MemberExpression::PrivateFieldExpression(member) => {
                Expression::PrivateFieldExpression(member)
            }
        }
    }
}

pub(crate) enum ParamsInterfaceFieldType<'a> {
    Annotation(&'a TSTypeAnnotation<'a>),
    Unknown,
}

pub(crate) struct ParamsInterfaceField<'a> {
    name: Cow<'a, str>,
    optional: bool,
    type_annotation: Option<ParamsInterfaceFieldType<'a>>,
    source_span: Span,
    name_span: Span,
}

impl<'a> ParamsInterfaceField<'a> {
    pub(crate) fn new<T>(
        name: T,
        optional: bool,
        type_annotation: Option<ParamsInterfaceFieldType<'a>>,
    ) -> Self
    where
        T: Into<Cow<'a, str>>,
    {
        Self {
            name: name.into(),
            optional,
            type_annotation,
            source_span: SPAN,
            name_span: SPAN,
        }
    }

    pub(crate) fn with_source<T>(
        name: T,
        optional: bool,
        type_annotation: Option<ParamsInterfaceFieldType<'a>>,
        source_span: Span,
        name_span: Span,
    ) -> Self
    where
        T: Into<Cow<'a, str>>,
    {
        Self {
            name: name.into(),
            optional,
            type_annotation,
            source_span,
            name_span,
        }
    }
}

pub(crate) struct ParamsInterfaceAstEmitter<'a> {
    name: &'a str,
    fields: Vec<ParamsInterfaceField<'a>>,
    exported: bool,
}

impl<'a> ParamsInterfaceAstEmitter<'a> {
    pub(crate) fn new(
        name: &'a str,
        fields: Vec<ParamsInterfaceField<'a>>,
        exported: bool,
    ) -> Self {
        Self {
            name,
            fields,
            exported,
        }
    }

    pub(crate) fn statement<'b>(
        &self,
        ast: &AstBuilder<'b>,
        allocator: &'b Allocator,
    ) -> Statement<'b> {
        let declaration = self.declaration(ast, allocator);
        if self.exported {
            let export = ArenaBox::new_in(
                ExportDeclaration::new(SPAN, Declaration::TSInterfaceDeclaration(declaration), ast),
                ast,
            );
            Statement::ExportDeclaration(export)
        } else {
            Statement::TSInterfaceDeclaration(declaration)
        }
    }

    fn declaration<'b>(
        &self,
        ast: &AstBuilder<'b>,
        allocator: &'b Allocator,
    ) -> oxc_allocator::Box<'b, oxc_ast::ast::TSInterfaceDeclaration<'b>> {
        let body = TSInterfaceBody::boxed(
            SPAN,
            OxcVec::from_iter_in(
                self.fields
                    .iter()
                    .map(|field| self.property_signature(field, ast, allocator)),
                ast,
            ),
            ast,
        );
        TSInterfaceDeclaration::boxed(
            SPAN,
            BindingIdentifier::new(SPAN, ast.allocator().alloc_str(self.name), ast),
            None,
            OxcVec::new_in(ast),
            body,
            false,
            ast,
        )
    }

    fn property_signature<'b>(
        &self,
        field: &ParamsInterfaceField<'_>,
        ast: &AstBuilder<'b>,
        allocator: &'b Allocator,
    ) -> TSSignature<'b> {
        TSSignature::new_ts_property_signature(
            field.source_span,
            false,
            field.optional,
            false,
            PropertyKey::new_static_identifier(
                field.name_span,
                ast.allocator().alloc_str(field.name.as_ref()),
                ast,
            ),
            field
                .type_annotation
                .as_ref()
                .map(|annotation| self.type_annotation(annotation, allocator)),
            ast,
        )
    }

    fn type_annotation<'b>(
        &self,
        annotation: &ParamsInterfaceFieldType<'_>,
        allocator: &'b Allocator,
    ) -> oxc_allocator::Box<'b, TSTypeAnnotation<'b>> {
        match annotation {
            ParamsInterfaceFieldType::Annotation(annotation) => ArenaBox::new_in(
                (**annotation).clone_in(allocator),
                &AstBuilder::new(allocator),
            ),
            ParamsInterfaceFieldType::Unknown => {
                ViewpuAstBuilder::new(allocator).unknown_type_annotation()
            }
        }
    }
}

pub(crate) struct TopLevelFunctionAstEmitter<'a> {
    exported: bool,
    name: &'a str,
}

impl<'a> TopLevelFunctionAstEmitter<'a> {
    pub(crate) fn new(exported: bool, name: &'a str) -> Self {
        Self { exported, name }
    }

    pub(crate) fn statement_with_formal_parameters_and_body_statements<'b, F, G, H>(
        &self,
        allocator: &'b Allocator,
        formal_parameters: F,
        return_type: G,
        body_statements: H,
    ) -> Statement<'b>
    where
        F: FnOnce(&'b Allocator) -> oxc_allocator::Box<'b, FormalParameters<'b>>,
        G: FnOnce(&'b Allocator) -> Option<oxc_allocator::Box<'b, TSTypeAnnotation<'b>>>,
        H: FnOnce(&'b Allocator) -> OxcVec<'b, Statement<'b>>,
    {
        let ast = AstBuilder::new(allocator);
        self.statement(
            &ast,
            formal_parameters(allocator),
            return_type(allocator),
            body_statements(allocator),
        )
    }

    fn statement<'b>(
        &self,
        ast: &AstBuilder<'b>,
        formal_params: oxc_allocator::Box<'b, FormalParameters<'b>>,
        return_type: Option<oxc_allocator::Box<'b, TSTypeAnnotation<'b>>>,
        body_statements: OxcVec<'b, Statement<'b>>,
    ) -> Statement<'b> {
        let declaration = self.declaration(ast, formal_params, return_type, body_statements);
        if self.exported {
            let export = ArenaBox::new_in(
                ExportDeclaration::new(SPAN, Declaration::FunctionDeclaration(declaration), ast),
                ast,
            );
            Statement::ExportDeclaration(export)
        } else {
            Statement::FunctionDeclaration(declaration)
        }
    }

    fn declaration<'b>(
        &self,
        ast: &AstBuilder<'b>,
        formal_params: oxc_allocator::Box<'b, FormalParameters<'b>>,
        return_type: Option<oxc_allocator::Box<'b, TSTypeAnnotation<'b>>>,
        body_statements: OxcVec<'b, Statement<'b>>,
    ) -> oxc_allocator::Box<'b, oxc_ast::ast::Function<'b>> {
        let body = FunctionBody::boxed(SPAN, OxcVec::new_in(ast), body_statements, ast);
        ArenaBox::new_in(
            Function::new(
                SPAN,
                FunctionType::FunctionDeclaration,
                Some(BindingIdentifier::new(
                    SPAN,
                    ast.allocator().alloc_str(self.name),
                    ast,
                )),
                false,
                false,
                false,
                None,
                None,
                formal_params,
                return_type,
                Some(body),
                ast,
            ),
            ast,
        )
    }
}

// ─── Public helpers for class-level code generation ──────────────────────────

struct ViewpuClassDeclaration<'a> {
    name: &'a str,
    name_span: Span,
    super_class_name: &'a str,
    elements: OxcVec<'a, ClassElement<'a>>,
    decorators: OxcVec<'a, Decorator<'a>>,
    is_exported: bool,
    is_default_export: bool,
}

pub(crate) struct ViewpuAstBuilder<'a> {
    allocator: &'a Allocator,
    ast: AstBuilder<'a>,
}

impl<'a> ViewpuAstBuilder<'a> {
    pub(crate) fn new(allocator: &'a Allocator) -> Self {
        Self {
            allocator,
            ast: AstBuilder::new(allocator),
        }
    }

    pub(crate) fn statements(&self) -> OxcVec<'a, Statement<'a>> {
        OxcVec::new_in(&self.allocator)
    }

    pub(crate) fn expressions(&self) -> OxcVec<'a, Expression<'a>> {
        OxcVec::new_in(&self.allocator)
    }

    pub(crate) fn clone_expression(&self, expression: &Expression<'_>) -> Expression<'a> {
        expression.clone_in(self.allocator)
    }

    pub(crate) fn super_call_statement(&self, args: &[&str]) -> Statement<'a> {
        let callee = Expression::new_super(SPAN, &self.ast);
        self.call_expression_statement(callee, self.expressions_from_identifiers(args))
    }

    pub(crate) fn guarded_super_method_call_statement(
        &self,
        method: &str,
        argument: &Expression<'_>,
    ) -> Statement<'a> {
        let available = self.super_computed_member_expression(method);
        let callable = Expression::new_binary_expression(
            SPAN,
            Expression::new_unary_expression(
                SPAN,
                UnaryOperator::Typeof,
                self.super_computed_member_expression(method),
                &self.ast,
            ),
            BinaryOperator::StrictEquality,
            self.string_expression("function"),
            &self.ast,
        );
        let condition = Expression::new_logical_expression(
            SPAN,
            available,
            LogicalOperator::And,
            callable,
            &self.ast,
        );
        let call = self.call_expression_statement(
            self.super_computed_member_expression(method),
            OxcVec::from_value_in(argument.clone_in(self.allocator), &self.ast),
        );
        self.if_statement(condition, OxcVec::from_value_in(call, &self.ast))
    }

    pub(crate) fn this_assign_statement(
        &self,
        property: &str,
        value: Expression<'a>,
    ) -> Statement<'a> {
        self.assignment_statement(self.this_member_assignment_target(property), value)
    }

    pub(crate) fn this_call_statement(
        &self,
        method: &str,
        args: OxcVec<'a, Expression<'a>>,
    ) -> Statement<'a> {
        let callee = self.member_expression_to_expression(self.this_member(method));
        self.call_expression_statement(callee, args)
    }

    pub(crate) fn this_field_call_statement(
        &self,
        field: &str,
        method: &str,
        args: OxcVec<'a, Expression<'a>>,
    ) -> Statement<'a> {
        let call = self.this_field_call_expression(field, method, args);
        Statement::new_expression_statement(SPAN, call, &self.ast)
    }

    pub(crate) fn this_field_call_expression(
        &self,
        field: &str,
        method: &str,
        args: OxcVec<'a, Expression<'a>>,
    ) -> Expression<'a> {
        let field_expression = self.member_expression_to_expression(self.this_member(field));
        let property = IdentifierName::new(SPAN, self.allocator.alloc_str(method), &self.ast);
        let member = MemberExpression::new_static_member_expression(
            SPAN,
            field_expression,
            property,
            false,
            &self.ast,
        );
        let callee = self.member_expression_to_expression(member);
        let arguments = OxcVec::from_iter_in(
            args.into_iter().map(Self::argument_from_expression),
            &self.ast,
        );
        Expression::new_call_expression(SPAN, callee, None, arguments, false, &self.ast)
    }

    pub(crate) fn backing_field_name(&self, field: &str) -> &'a str {
        GeneratedViewpuName::backing_field_alloc(self.allocator, field)
    }

    pub(crate) fn this_backing_member_expression(&self, field: &str) -> Expression<'a> {
        self.this_member_expression(self.backing_field_name(field))
    }

    pub(crate) fn this_backing_field_call_statement(
        &self,
        field: &str,
        method: &str,
        args: OxcVec<'a, Expression<'a>>,
    ) -> Statement<'a> {
        self.this_field_call_statement(self.backing_field_name(field), method, args)
    }

    pub(crate) fn this_backing_field_call_expression(
        &self,
        field: &str,
        method: &str,
        args: OxcVec<'a, Expression<'a>>,
    ) -> Expression<'a> {
        self.this_field_call_expression(self.backing_field_name(field), method, args)
    }

    pub(crate) fn params_field_expression(&self, field: &str) -> Expression<'a> {
        let object = Expression::new_identifier(SPAN, GeneratedViewpuName::PARAMS, &self.ast);
        let property = IdentifierName::new(SPAN, self.allocator.alloc_str(field), &self.ast);
        self.member_expression_to_expression(MemberExpression::new_static_member_expression(
            SPAN, object, property, false, &self.ast,
        ))
    }

    pub(crate) fn string_expression(&self, value: &str) -> Expression<'a> {
        Expression::new_string_literal(SPAN, self.allocator.alloc_str(value), None, &self.ast)
    }

    pub(crate) fn undefined_expression(&self) -> Expression<'a> {
        Expression::new_identifier(SPAN, GeneratedViewpuName::UNDEFINED, &self.ast)
    }

    pub(crate) fn null_expression(&self) -> Expression<'a> {
        Expression::new_null_literal(SPAN, &self.ast)
    }

    pub(crate) fn minus_one_expression(&self) -> Expression<'a> {
        let value = Expression::new_numeric_literal(
            SPAN,
            1.0,
            Some("1".into()),
            NumberBase::Decimal,
            &self.ast,
        );
        Expression::new_unary_expression(SPAN, UnaryOperator::UnaryNegation, value, &self.ast)
    }

    pub(crate) fn identifier_expression(&self, name: &str) -> Expression<'a> {
        Expression::new_identifier(SPAN, self.allocator.alloc_str(name), &self.ast)
    }

    pub(crate) fn boolean_expression(&self, value: bool) -> Expression<'a> {
        Expression::new_boolean_literal(SPAN, value, &self.ast)
    }

    pub(crate) fn this_member_expression(&self, property: &str) -> Expression<'a> {
        self.member_expression_to_expression(self.this_member(property))
    }

    pub(crate) fn this_expression(&self) -> Expression<'a> {
        Expression::new_this_expression(SPAN, &self.ast)
    }

    pub(crate) fn subscriber_manager_delete_statement(&self) -> Statement<'a> {
        let subscriber_manager =
            Expression::new_identifier(SPAN, GeneratedViewpuName::SUBSCRIBER_MANAGER, &self.ast);
        let get_member = MemberExpression::new_static_member_expression(
            SPAN,
            subscriber_manager,
            IdentifierName::new(SPAN, GeneratedViewpuName::SUBSCRIBER_MANAGER_GET, &self.ast),
            false,
            &self.ast,
        );
        let get_call = Expression::new_call_expression(
            SPAN,
            Expression::from(get_member),
            None,
            OxcVec::new_in(&self.ast),
            false,
            &self.ast,
        );
        let delete_member = MemberExpression::new_static_member_expression(
            SPAN,
            get_call,
            IdentifierName::new(SPAN, GeneratedViewpuName::DELETE, &self.ast),
            false,
            &self.ast,
        );
        let id_member = self.this_member_expression(GeneratedViewpuName::ID_ACCESSOR);
        let id_call = Expression::new_call_expression(
            SPAN,
            id_member,
            None,
            OxcVec::new_in(&self.ast),
            false,
            &self.ast,
        );
        let delete_call = Expression::new_call_expression(
            SPAN,
            Expression::from(delete_member),
            None,
            OxcVec::from_value_in(Self::argument_from_expression(id_call), &self.ast),
            false,
            &self.ast,
        );
        Statement::new_expression_statement(SPAN, delete_call, &self.ast)
    }

    pub(crate) fn new_expression(
        &self,
        callee_name: &str,
        args: OxcVec<'a, Expression<'a>>,
    ) -> Expression<'a> {
        let callee =
            Expression::new_identifier(SPAN, self.allocator.alloc_str(callee_name), &self.ast);
        let arguments = OxcVec::from_iter_in(
            args.into_iter().map(Self::argument_from_expression),
            &self.ast,
        );
        Expression::new_new_expression(SPAN, callee, None, arguments, &self.ast)
    }

    pub(crate) fn this_call_expression(
        &self,
        method: &str,
        args: OxcVec<'a, Expression<'a>>,
    ) -> Expression<'a> {
        self.this_call_expression_with_type_arguments(method, None, args)
    }

    pub(crate) fn this_call_expression_with_type_annotation_arg(
        &self,
        method: &str,
        type_arg: Option<&TSTypeAnnotation<'_>>,
        args: OxcVec<'a, Expression<'a>>,
    ) -> Expression<'a> {
        let type_arguments = type_arg.map(|annotation| {
            self.type_parameter_instantiation(annotation.type_annotation.clone_in(self.allocator))
        });
        self.this_call_expression_with_type_arguments(method, type_arguments, args)
    }

    fn this_call_expression_with_type_arguments(
        &self,
        method: &str,
        type_arguments: Option<
            oxc_allocator::Box<'a, oxc_ast::ast::TSTypeParameterInstantiation<'a>>,
        >,
        args: OxcVec<'a, Expression<'a>>,
    ) -> Expression<'a> {
        let callee = self.member_expression_to_expression(self.this_member(method));
        let arguments = OxcVec::from_iter_in(
            args.into_iter().map(Self::argument_from_expression),
            &self.ast,
        );
        Expression::new_call_expression(SPAN, callee, type_arguments, arguments, false, &self.ast)
    }

    fn type_parameter_instantiation(
        &self,
        type_annotation: TSType<'a>,
    ) -> oxc_allocator::Box<'a, oxc_ast::ast::TSTypeParameterInstantiation<'a>> {
        TSTypeParameterInstantiation::boxed(
            SPAN,
            OxcVec::from_value_in(type_annotation, &self.ast),
            &self.ast,
        )
    }

    pub(crate) fn typeof_equals_function(&self, name: &str, function_type: &str) -> Expression<'a> {
        let argument = Expression::new_identifier(SPAN, self.allocator.alloc_str(name), &self.ast);
        let left =
            Expression::new_unary_expression(SPAN, UnaryOperator::Typeof, argument, &self.ast);
        let right = self.string_expression(function_type);
        Expression::new_binary_expression(
            SPAN,
            left,
            BinaryOperator::StrictEquality,
            right,
            &self.ast,
        )
    }

    pub(crate) fn params_field_not_undefined(&self, field: &str) -> Expression<'a> {
        let left = self.params_field_expression(field);
        let right = self.undefined_expression();
        Expression::new_binary_expression(
            SPAN,
            left,
            BinaryOperator::StrictInequality,
            right,
            &self.ast,
        )
    }

    pub(crate) fn params_field_equals_undefined(&self, field: &str) -> Expression<'a> {
        let left = self.params_field_expression(field);
        let right = self.undefined_expression();
        Expression::new_binary_expression(
            SPAN,
            left,
            BinaryOperator::StrictEquality,
            right,
            &self.ast,
        )
    }

    pub(crate) fn params_field_truthy(&self, field: &str) -> Expression<'a> {
        self.params_field_expression(field)
    }

    pub(crate) fn params_is_undefined(&self) -> Expression<'a> {
        let left = self.identifier_expression(GeneratedViewpuName::PARAMS);
        let right = self.undefined_expression();
        Expression::new_binary_expression(
            SPAN,
            left,
            BinaryOperator::StrictEquality,
            right,
            &self.ast,
        )
    }

    pub(crate) fn params_has_field(&self, field: &str) -> Expression<'a> {
        let left = self.string_expression(field);
        let right = self.identifier_expression(GeneratedViewpuName::PARAMS);
        Expression::new_binary_expression(SPAN, left, BinaryOperator::In, right, &self.ast)
    }

    pub(crate) fn params_present_and_has_field(&self, field: &str) -> Expression<'a> {
        let left = self.identifier_expression(GeneratedViewpuName::PARAMS);
        let right = self.params_has_field(field);
        Expression::new_logical_expression(SPAN, left, LogicalOperator::And, right, &self.ast)
    }

    pub(crate) fn conditional_expression(
        &self,
        test: Expression<'a>,
        consequent: Expression<'a>,
        alternate: Expression<'a>,
    ) -> Expression<'a> {
        Expression::new_conditional_expression(SPAN, test, consequent, alternate, &self.ast)
    }

    pub(crate) fn if_statement(
        &self,
        test: Expression<'a>,
        statements: OxcVec<'a, Statement<'a>>,
    ) -> Statement<'a> {
        let consequent = Statement::new_block_statement(SPAN, statements, &self.ast);
        Statement::new_if_statement(SPAN, test, consequent, None, &self.ast)
    }

    pub(crate) fn return_string_statement(&self, value: &str) -> Statement<'a> {
        Statement::new_return_statement(SPAN, Some(self.string_expression(value)), &self.ast)
    }

    pub(crate) fn return_expression_statement(&self, value: Expression<'a>) -> Statement<'a> {
        Statement::new_return_statement(SPAN, Some(value), &self.ast)
    }

    pub(crate) fn return_statement(&self) -> Statement<'a> {
        Statement::new_return_statement(SPAN, None, &self.ast)
    }

    pub(crate) fn empty_arrow_function_expression(
        &self,
        params: Option<&FormalParameters<'_>>,
    ) -> Expression<'a> {
        let params = params
            .map(|params| {
                let mut params = params.clone_in(self.allocator);
                params.kind = FormalParameterKind::ArrowFormalParameters;
                ArenaBox::new_in(params, &self.ast)
            })
            .unwrap_or_else(|| {
                ArenaBox::new_in(
                    FormalParameters::new(
                        SPAN,
                        FormalParameterKind::ArrowFormalParameters,
                        OxcVec::new_in(&self.ast),
                        None,
                        &self.ast,
                    ),
                    &self.ast,
                )
            });
        let body = ArrowFunctionBody::new_function_body(
            SPAN,
            OxcVec::new_in(&self.ast),
            OxcVec::new_in(&self.ast),
            &self.ast,
        );
        Expression::new_arrow_function_expression(SPAN, false, None, params, None, body, &self.ast)
    }

    pub(crate) fn formal_parameter(&self, name: &str) -> FormalParameter<'a> {
        self.formal_parameter_with_initializer(name, None, None)
    }

    pub(crate) fn formal_parameter_with_type_annotation(
        &self,
        name: &str,
        type_annotation: Option<&TSTypeAnnotation<'_>>,
    ) -> FormalParameter<'a> {
        let type_annotation =
            type_annotation.map(|annotation| self.clone_type_annotation(annotation));
        self.formal_parameter_with_initializer(name, type_annotation, None)
    }

    pub(crate) fn formal_parameter_with_initializer(
        &self,
        name: &str,
        type_annotation: Option<oxc_allocator::Box<'a, TSTypeAnnotation<'a>>>,
        initializer: Option<Expression<'a>>,
    ) -> FormalParameter<'a> {
        FormalParameter::new(
            SPAN,
            OxcVec::new_in(&self.ast),
            BindingPattern::new_binding_identifier(SPAN, self.allocator.alloc_str(name), &self.ast),
            type_annotation,
            initializer.map(|initializer| ArenaBox::new_in(initializer, &self.ast)),
            false,
            None,
            false,
            false,
            &self.ast,
        )
    }

    pub(crate) fn formal_parameters_from_iter<I>(
        &self,
        params: I,
    ) -> OxcVec<'a, FormalParameter<'a>>
    where
        I: IntoIterator<Item = FormalParameter<'a>>,
    {
        OxcVec::from_iter_in(params, &self.ast)
    }

    pub(crate) fn formal_parameters(&self) -> OxcVec<'a, FormalParameter<'a>> {
        OxcVec::new_in(&self.allocator)
    }

    pub(crate) fn clone_formal_parameters(
        &self,
        params: &FormalParameters<'_>,
    ) -> oxc_allocator::Box<'a, FormalParameters<'a>> {
        ArenaBox::new_in(params.clone_in(self.allocator), &self.ast)
    }

    pub(crate) fn clone_formal_parameters_with_parent(
        &self,
        params: &FormalParameters<'_>,
    ) -> oxc_allocator::Box<'a, FormalParameters<'a>> {
        let mut params = params.clone_in(self.allocator);
        params.items.push(self.formal_parameter_with_initializer(
            GeneratedViewpuName::PARENT,
            None,
            Some(self.null_expression()),
        ));
        ArenaBox::new_in(params, &self.ast)
    }

    pub(crate) fn clone_type_annotation(
        &self,
        annotation: &TSTypeAnnotation<'_>,
    ) -> oxc_allocator::Box<'a, TSTypeAnnotation<'a>> {
        ArenaBox::new_in(annotation.clone_in(self.allocator), &self.ast)
    }

    pub(crate) fn string_type_annotation(&self) -> oxc_allocator::Box<'a, TSTypeAnnotation<'a>> {
        TSTypeAnnotation::boxed(
            SPAN,
            TSType::new_ts_string_keyword(SPAN, &self.ast),
            &self.ast,
        )
    }

    pub(crate) fn unknown_type_annotation(&self) -> oxc_allocator::Box<'a, TSTypeAnnotation<'a>> {
        TSTypeAnnotation::boxed(
            SPAN,
            TSType::new_ts_unknown_keyword(SPAN, &self.ast),
            &self.ast,
        )
    }

    pub(crate) fn type_reference_annotation(
        &self,
        name: &str,
    ) -> oxc_allocator::Box<'a, TSTypeAnnotation<'a>> {
        TSTypeAnnotation::boxed(SPAN, self.type_reference_type(name), &self.ast)
    }

    fn type_reference_type(&self, name: &str) -> TSType<'a> {
        let type_reference = TSType::new_ts_type_reference(
            SPAN,
            TSTypeName::new_identifier_reference(SPAN, self.allocator.alloc_str(name), &self.ast),
            Option::<oxc_allocator::Box<'a, oxc_ast::ast::TSTypeParameterInstantiation<'a>>>::None,
            &self.ast,
        );
        type_reference
    }

    pub(crate) fn state_property_type_annotation(
        &self,
        wrapper_name: &str,
        annotation: Option<&TSTypeAnnotation<'_>>,
    ) -> oxc_allocator::Box<'a, TSTypeAnnotation<'a>> {
        let type_arguments = annotation.map(|annotation| {
            TSTypeParameterInstantiation::boxed(
                SPAN,
                OxcVec::from_value_in(
                    annotation.type_annotation.clone_in(self.allocator),
                    &self.ast,
                ),
                &self.ast,
            )
        });
        let type_reference = TSType::new_ts_type_reference(
            SPAN,
            TSTypeName::new_identifier_reference(
                SPAN,
                self.allocator.alloc_str(wrapper_name),
                &self.ast,
            ),
            type_arguments,
            &self.ast,
        );
        TSTypeAnnotation::boxed(SPAN, type_reference, &self.ast)
    }

    pub(crate) fn void_type_annotation(&self) -> oxc_allocator::Box<'a, TSTypeAnnotation<'a>> {
        TSTypeAnnotation::boxed(
            SPAN,
            TSType::new_ts_void_keyword(SPAN, &self.ast),
            &self.ast,
        )
    }

    pub(crate) fn expressions_from_identifiers(
        &self,
        names: &[&str],
    ) -> OxcVec<'a, Expression<'a>> {
        OxcVec::from_iter_in(
            names.iter().map(|name| {
                Expression::new_identifier(SPAN, self.allocator.alloc_str(name), &self.ast)
            }),
            &self.ast,
        )
    }

    pub(crate) fn expressions_from_iter<I>(&self, expressions: I) -> OxcVec<'a, Expression<'a>>
    where
        I: IntoIterator<Item = Expression<'a>>,
    {
        OxcVec::from_iter_in(expressions, &self.ast)
    }

    fn assignment_statement(
        &self,
        left: oxc_ast::ast::AssignmentTarget<'a>,
        right: Expression<'a>,
    ) -> Statement<'a> {
        let assignment = Expression::new_assignment_expression(
            SPAN,
            AssignmentOperator::Assign,
            left,
            right,
            &self.ast,
        );
        Statement::new_expression_statement(SPAN, assignment, &self.ast)
    }

    fn call_expression_statement(
        &self,
        callee: Expression<'a>,
        args: OxcVec<'a, Expression<'a>>,
    ) -> Statement<'a> {
        let arguments = OxcVec::from_iter_in(
            args.into_iter().map(Self::argument_from_expression),
            &self.ast,
        );
        let call = Expression::new_call_expression(SPAN, callee, None, arguments, false, &self.ast);
        Statement::new_expression_statement(SPAN, call, &self.ast)
    }

    fn this_member(&self, property: &str) -> MemberExpression<'a> {
        let object = Expression::new_this_expression(SPAN, &self.ast);
        let property = IdentifierName::new(SPAN, self.allocator.alloc_str(property), &self.ast);
        MemberExpression::new_static_member_expression(SPAN, object, property, false, &self.ast)
    }

    fn super_computed_member_expression(&self, property: &str) -> Expression<'a> {
        Expression::from(MemberExpression::new_computed_member_expression(
            SPAN,
            Expression::new_super(SPAN, &self.ast),
            self.string_expression(property),
            false,
            &self.ast,
        ))
    }

    fn this_member_assignment_target(&self, property: &str) -> oxc_ast::ast::AssignmentTarget<'a> {
        match self.this_member(property) {
            MemberExpression::ComputedMemberExpression(member) => {
                oxc_ast::ast::AssignmentTarget::ComputedMemberExpression(member)
            }
            MemberExpression::StaticMemberExpression(member) => {
                oxc_ast::ast::AssignmentTarget::StaticMemberExpression(member)
            }
            MemberExpression::PrivateFieldExpression(member) => {
                oxc_ast::ast::AssignmentTarget::PrivateFieldExpression(member)
            }
        }
    }

    fn member_expression_to_expression(&self, member: MemberExpression<'a>) -> Expression<'a> {
        ViewpuAstEmitter::member_expression_to_expression(member)
    }

    fn argument_from_expression(expression: Expression<'a>) -> Argument<'a> {
        Argument::from(expression)
    }
}

pub(crate) struct ViewpuMemberAstEmitter<'a> {
    ast: AstBuilder<'a>,
}

impl<'a> ViewpuMemberAstEmitter<'a> {
    pub(crate) fn new(allocator: &'a Allocator) -> Self {
        Self {
            ast: AstBuilder::new(allocator),
        }
    }

    pub(crate) fn method_with_params(
        &self,
        name: &'a str,
        params_items: OxcVec<'a, FormalParameter<'a>>,
        body_stmts: OxcVec<'a, Statement<'a>>,
        is_static: bool,
        accessibility: Option<TSAccessibility>,
    ) -> ClassElement<'a> {
        self.method_with_params_and_source_span(
            name,
            params_items,
            body_stmts,
            is_static,
            accessibility,
            SPAN,
            SPAN,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn method_with_params_and_source_span(
        &self,
        name: &'a str,
        params_items: OxcVec<'a, FormalParameter<'a>>,
        body_stmts: OxcVec<'a, Statement<'a>>,
        is_static: bool,
        accessibility: Option<TSAccessibility>,
        source_span: Span,
        name_span: Span,
    ) -> ClassElement<'a> {
        let element = self.method_with_type_annotation(
            name,
            params_items,
            None,
            body_stmts,
            is_static,
            accessibility,
        );
        self.method_source_span(element, source_span, name_span)
    }

    pub(crate) fn method_with_type_annotation(
        &self,
        name: &'a str,
        params_items: OxcVec<'a, FormalParameter<'a>>,
        return_type: Option<oxc_allocator::Box<'a, TSTypeAnnotation<'a>>>,
        body_stmts: OxcVec<'a, Statement<'a>>,
        is_static: bool,
        accessibility: Option<TSAccessibility>,
    ) -> ClassElement<'a> {
        let formal_params = ArenaBox::new_in(
            FormalParameters::new(
                SPAN,
                FormalParameterKind::FormalParameter,
                params_items,
                None,
                &self.ast,
            ),
            &self.ast,
        );
        self.method_with_formal_parameters(
            name,
            formal_params,
            return_type,
            body_stmts,
            is_static,
            accessibility,
        )
    }

    pub(crate) fn method_with_formal_parameters(
        &self,
        name: &'a str,
        formal_params: oxc_allocator::Box<'a, FormalParameters<'a>>,
        return_type: Option<oxc_allocator::Box<'a, TSTypeAnnotation<'a>>>,
        body_stmts: OxcVec<'a, Statement<'a>>,
        is_static: bool,
        accessibility: Option<TSAccessibility>,
    ) -> ClassElement<'a> {
        let body = FunctionBody::boxed(SPAN, OxcVec::new_in(&self.ast), body_stmts, &self.ast);
        let func = ArenaBox::new_in(
            Function::new(
                SPAN,
                oxc_ast::ast::FunctionType::FunctionExpression,
                None,
                false,
                false,
                false,
                None,
                None,
                formal_params,
                return_type,
                Some(body),
                &self.ast,
            ),
            &self.ast,
        );
        let key = PropertyKey::new_static_identifier(SPAN, name, &self.ast);
        let method_kind = if name == GeneratedViewpuName::CONSTRUCTOR {
            MethodDefinitionKind::Constructor
        } else {
            MethodDefinitionKind::Method
        };
        ClassElement::new_method_definition(
            SPAN,
            MethodDefinitionType::MethodDefinition,
            OxcVec::new_in(&self.ast),
            key,
            func,
            method_kind,
            false,
            is_static,
            false,
            false,
            accessibility,
            &self.ast,
        )
    }

    fn method_source_span(
        &self,
        mut element: ClassElement<'a>,
        source_span: Span,
        name_span: Span,
    ) -> ClassElement<'a> {
        let ClassElement::MethodDefinition(method) = &mut element else {
            return element;
        };
        method.span = source_span;
        if let PropertyKey::StaticIdentifier(identifier) = &mut method.key {
            identifier.span = name_span;
        }
        element
    }

    pub(crate) fn getter_with_source_span(
        &self,
        name: &'a str,
        body_stmts: OxcVec<'a, Statement<'a>>,
        accessibility: Option<TSAccessibility>,
        source_span: Span,
        name_span: Span,
    ) -> ClassElement<'a> {
        let formal_params = ArenaBox::new_in(
            FormalParameters::new(
                SPAN,
                FormalParameterKind::FormalParameter,
                OxcVec::new_in(&self.ast),
                None,
                &self.ast,
            ),
            &self.ast,
        );
        let body = FunctionBody::boxed(SPAN, OxcVec::new_in(&self.ast), body_stmts, &self.ast);
        let no_return_type: Option<oxc_allocator::Box<'a, TSTypeAnnotation<'a>>> = None;
        let func = ArenaBox::new_in(
            Function::new(
                SPAN,
                oxc_ast::ast::FunctionType::FunctionExpression,
                None,
                false,
                false,
                false,
                None,
                None,
                formal_params,
                no_return_type,
                Some(body),
                &self.ast,
            ),
            &self.ast,
        );
        let key = PropertyKey::new_static_identifier(name_span, name, &self.ast);
        ClassElement::new_method_definition(
            source_span,
            MethodDefinitionType::MethodDefinition,
            OxcVec::new_in(&self.ast),
            key,
            func,
            MethodDefinitionKind::Get,
            false,
            false,
            false,
            false,
            accessibility,
            &self.ast,
        )
    }

    pub(crate) fn setter_with_param_and_source_span(
        &self,
        name: &'a str,
        param: FormalParameter<'a>,
        body_stmts: OxcVec<'a, Statement<'a>>,
        accessibility: Option<TSAccessibility>,
        source_span: Span,
        name_span: Span,
    ) -> ClassElement<'a> {
        let formal_params = ArenaBox::new_in(
            FormalParameters::new(
                SPAN,
                FormalParameterKind::FormalParameter,
                OxcVec::from_value_in(param, &self.ast),
                None,
                &self.ast,
            ),
            &self.ast,
        );
        let body = FunctionBody::boxed(SPAN, OxcVec::new_in(&self.ast), body_stmts, &self.ast);
        let no_return_type: Option<oxc_allocator::Box<'a, oxc_ast::ast::TSTypeAnnotation<'a>>> =
            None;
        let func = ArenaBox::new_in(
            Function::new(
                SPAN,
                oxc_ast::ast::FunctionType::FunctionExpression,
                None,
                false,
                false,
                false,
                None,
                None,
                formal_params,
                no_return_type,
                Some(body),
                &self.ast,
            ),
            &self.ast,
        );
        let key = PropertyKey::new_static_identifier(name_span, name, &self.ast);
        ClassElement::new_method_definition(
            source_span,
            MethodDefinitionType::MethodDefinition,
            OxcVec::new_in(&self.ast),
            key,
            func,
            MethodDefinitionKind::Set,
            false,
            false,
            false,
            false,
            accessibility,
            &self.ast,
        )
    }

    pub(crate) fn property_with_type_annotation(
        &self,
        name: &'a str,
        type_annotation: Option<oxc_allocator::Box<'a, TSTypeAnnotation<'a>>>,
        value: Option<Expression<'a>>,
        is_private: bool,
        optional: bool,
    ) -> ClassElement<'a> {
        self.property_with_type_annotation_and_source_span(
            name,
            type_annotation,
            value,
            is_private,
            optional,
            SPAN,
            SPAN,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn property_with_type_annotation_and_source_span(
        &self,
        name: &'a str,
        type_annotation: Option<oxc_allocator::Box<'a, TSTypeAnnotation<'a>>>,
        value: Option<Expression<'a>>,
        is_private: bool,
        optional: bool,
        source_span: Span,
        name_span: Span,
    ) -> ClassElement<'a> {
        let key = PropertyKey::new_static_identifier(name_span, name, &self.ast);
        let accessibility = if is_private {
            Some(TSAccessibility::Private)
        } else {
            None
        };

        ClassElement::new_property_definition(
            source_span,
            PropertyDefinitionType::PropertyDefinition,
            OxcVec::new_in(&self.ast),
            key,
            type_annotation,
            value,
            false,
            false,
            false,
            false,
            optional,
            false,
            false,
            accessibility,
            &self.ast,
        )
    }

    pub(crate) fn class_declaration_statement_with_name_span(
        &self,
        name: &'a str,
        name_span: Span,
        super_class_name: &'a str,
        elements: OxcVec<'a, ClassElement<'a>>,
        is_exported: bool,
        is_default_export: bool,
    ) -> Statement<'a> {
        self.class_declaration_statement(ViewpuClassDeclaration {
            name,
            name_span,
            super_class_name,
            elements,
            decorators: OxcVec::new_in(&self.ast),
            is_exported,
            is_default_export,
        })
    }

    pub(crate) fn class_declaration_statement_with_decorators(
        &self,
        name: &'a str,
        super_class_name: &'a str,
        elements: OxcVec<'a, ClassElement<'a>>,
        decorators: OxcVec<'a, Decorator<'a>>,
        is_exported: bool,
        is_default_export: bool,
    ) -> Statement<'a> {
        self.class_declaration_statement(ViewpuClassDeclaration {
            name,
            name_span: SPAN,
            super_class_name,
            elements,
            decorators,
            is_exported,
            is_default_export,
        })
    }

    fn class_declaration_statement(
        &self,
        declaration: ViewpuClassDeclaration<'a>,
    ) -> Statement<'a> {
        let heritage = Some(ClassHeritage::new(
            Expression::new_identifier(SPAN, declaration.super_class_name, &self.ast),
            None,
            &self.ast,
        ));
        let body = ClassBody::boxed(SPAN, declaration.elements, &self.ast);
        let class = Class::boxed(
            SPAN,
            ClassType::ClassDeclaration,
            declaration.decorators,
            Some(BindingIdentifier::new(
                declaration.name_span,
                declaration.name,
                &self.ast,
            )),
            None,
            heritage,
            OxcVec::new_in(&self.ast),
            body,
            false,
            false,
            &self.ast,
        );

        if declaration.is_default_export {
            let decl = oxc_ast::ast::ExportDefaultDeclarationKind::ClassDeclaration(class);
            let export = ArenaBox::new_in(
                ExportDefaultDeclaration::new(SPAN, decl, &self.ast),
                &self.ast,
            );
            Statement::ExportDefaultDeclaration(export)
        } else if declaration.is_exported {
            let decl = oxc_ast::ast::Declaration::ClassDeclaration(class);
            let export = ArenaBox::new_in(ExportDeclaration::new(SPAN, decl, &self.ast), &self.ast);
            Statement::ExportDeclaration(export)
        } else {
            Statement::ClassDeclaration(class)
        }
    }
}
