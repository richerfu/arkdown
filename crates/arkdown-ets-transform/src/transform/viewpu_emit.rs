//! ViewPU AST emission helpers.
//!
//! This module is the transition boundary between ArkUI lowering decisions and
//! generated code. The lowering code should call these helpers for generated
//! ViewPU statements so the final text is printed by OXC codegen.

use std::borrow::Cow;

use oxc_allocator::{Allocator, CloneIn, Vec as OxcVec};
use oxc_ast::ast::{
    Argument, ClassElement, ClassType, Declaration, Decorator, Expression, FormalParameter,
    FormalParameterKind, FormalParameters, FunctionType, ImportOrExportKind, MemberExpression,
    MethodDefinitionKind, MethodDefinitionType, NumberBase, PropertyDefinitionType, PropertyKey,
    Statement, TSAccessibility, TSSignature, TSType, TSTypeAnnotation,
};
use oxc_ast::{AstBuilder, NONE};
use oxc_span::SPAN;
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, LogicalOperator, UnaryOperator};

use crate::transform::component_meta::ArkuiRuntimeNames;

pub(crate) struct GeneratedViewpuName;

impl GeneratedViewpuName {
    pub(crate) const VIEWPU_BASE: &'static str = "ViewPU";
    pub(crate) const VIEW_V2_BASE: &'static str = "ViewV2";
    pub(crate) const REUSABLE_V2_INNER_DECORATOR: &'static str = "__ReusableV2_Inner_Decorator__";
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
        component_name: &str,
        runtime_names: &ArkuiRuntimeNames,
        body_stmts: OxcVec<'b, Statement<'b>>,
    ) -> Statement<'b> {
        let object = ViewpuAstEmitter::view_context_expression(ast, use_parent_context);
        let property =
            ast.identifier_name(SPAN, ast.str(&runtime_names.observe_component_creation2));
        let member = ast.member_expression_static(SPAN, object, property, false);
        let callee = ViewpuAstEmitter::member_expression_to_expression(member);
        let arrow = ViewpuAstEmitter::observe_arrow_function_with_statements(
            ast,
            runtime_names,
            body_stmts,
        );
        let component = ast.expression_identifier(SPAN, ast.str(component_name));
        let arguments = ast.vec_from_iter([
            ViewpuAstEmitter::argument_from_expression(arrow),
            ViewpuAstEmitter::argument_from_expression(component),
        ]);
        let call = ast.expression_call(SPAN, callee, NONE, arguments, false);
        ast.statement_expression(SPAN, call)
    }
}

struct ViewpuAstEmitter;

impl ViewpuAstEmitter {
    fn view_context_expression<'b>(
        ast: AstBuilder<'b>,
        use_parent_context: bool,
    ) -> Expression<'b> {
        if !use_parent_context {
            return ast.expression_this(SPAN);
        }

        let test = ast.expression_identifier(SPAN, GeneratedViewpuName::PARENT);
        let consequent = ast.expression_identifier(SPAN, GeneratedViewpuName::PARENT);
        let alternate = ast.expression_this(SPAN);
        let conditional = ast.expression_conditional(SPAN, test, consequent, alternate);
        ast.expression_parenthesized(SPAN, conditional)
    }

    fn observe_arrow_function_with_statements<'b>(
        ast: AstBuilder<'b>,
        runtime_names: &ArkuiRuntimeNames,
        statements: OxcVec<'b, Statement<'b>>,
    ) -> Expression<'b> {
        let mut params = ast.formal_parameters(
            SPAN,
            FormalParameterKind::ArrowFormalParameters,
            ast.vec_with_capacity(2),
            NONE,
        );
        params
            .items
            .push(Self::formal_parameter(ast, &runtime_names.elmt_id));
        params.items.push(Self::formal_parameter(
            ast,
            &runtime_names.is_initial_render,
        ));
        let body = ast.function_body(SPAN, ast.vec(), statements);
        ast.expression_arrow_function(SPAN, false, false, NONE, params, NONE, body)
    }

    fn formal_parameter<'b>(ast: AstBuilder<'b>, name: &str) -> oxc_ast::ast::FormalParameter<'b> {
        ast.formal_parameter(
            SPAN,
            ast.vec(),
            ast.binding_pattern_binding_identifier(SPAN, ast.str(name)),
            NONE,
            NONE,
            false,
            None,
            false,
            false,
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
        ast: AstBuilder<'b>,
        allocator: &'b Allocator,
    ) -> Statement<'b> {
        let declaration = self.declaration(ast, allocator);
        if self.exported {
            let export = ast.alloc_export_named_declaration(
                SPAN,
                Some(Declaration::TSInterfaceDeclaration(declaration)),
                ast.vec(),
                None,
                ImportOrExportKind::Value,
                NONE,
            );
            Statement::ExportNamedDeclaration(export)
        } else {
            Statement::TSInterfaceDeclaration(declaration)
        }
    }

    fn declaration<'b>(
        &self,
        ast: AstBuilder<'b>,
        allocator: &'b Allocator,
    ) -> oxc_allocator::Box<'b, oxc_ast::ast::TSInterfaceDeclaration<'b>> {
        let body = ast.ts_interface_body(
            SPAN,
            ast.vec_from_iter(
                self.fields
                    .iter()
                    .map(|field| self.property_signature(field, ast, allocator)),
            ),
        );
        ast.alloc_ts_interface_declaration(
            SPAN,
            ast.binding_identifier(SPAN, ast.str(self.name)),
            NONE,
            ast.vec(),
            body,
            false,
        )
    }

    fn property_signature<'b>(
        &self,
        field: &ParamsInterfaceField<'_>,
        ast: AstBuilder<'b>,
        allocator: &'b Allocator,
    ) -> TSSignature<'b> {
        ast.ts_signature_property_signature(
            SPAN,
            false,
            field.optional,
            false,
            ast.property_key_static_identifier(SPAN, ast.str(field.name.as_ref())),
            field
                .type_annotation
                .as_ref()
                .map(|annotation| self.type_annotation(annotation, allocator)),
        )
    }

    fn type_annotation<'b>(
        &self,
        annotation: &ParamsInterfaceFieldType<'_>,
        allocator: &'b Allocator,
    ) -> oxc_allocator::Box<'b, TSTypeAnnotation<'b>> {
        match annotation {
            ParamsInterfaceFieldType::Annotation(annotation) => {
                AstBuilder::new(allocator).alloc((**annotation).clone_in(allocator))
            }
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
            ast,
            formal_parameters(allocator),
            return_type(allocator),
            body_statements(allocator),
        )
    }

    fn statement<'b>(
        &self,
        ast: AstBuilder<'b>,
        formal_params: oxc_allocator::Box<'b, FormalParameters<'b>>,
        return_type: Option<oxc_allocator::Box<'b, TSTypeAnnotation<'b>>>,
        body_statements: OxcVec<'b, Statement<'b>>,
    ) -> Statement<'b> {
        let declaration = self.declaration(ast, formal_params, return_type, body_statements);
        if self.exported {
            let export = ast.alloc_export_named_declaration(
                SPAN,
                Some(Declaration::FunctionDeclaration(declaration)),
                ast.vec(),
                None,
                ImportOrExportKind::Value,
                NONE,
            );
            Statement::ExportNamedDeclaration(export)
        } else {
            Statement::FunctionDeclaration(declaration)
        }
    }

    fn declaration<'b>(
        &self,
        ast: AstBuilder<'b>,
        formal_params: oxc_allocator::Box<'b, FormalParameters<'b>>,
        return_type: Option<oxc_allocator::Box<'b, TSTypeAnnotation<'b>>>,
        body_statements: OxcVec<'b, Statement<'b>>,
    ) -> oxc_allocator::Box<'b, oxc_ast::ast::Function<'b>> {
        let body = ast.alloc_function_body(SPAN, ast.vec(), body_statements);
        ast.alloc_function(
            SPAN,
            FunctionType::FunctionDeclaration,
            Some(ast.binding_identifier(SPAN, ast.str(self.name))),
            false,
            false,
            false,
            NONE,
            NONE,
            formal_params,
            return_type,
            Some(body),
        )
    }
}

// ─── Public helpers for class-level code generation ──────────────────────────

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
        OxcVec::new_in(self.allocator)
    }

    pub(crate) fn expressions(&self) -> OxcVec<'a, Expression<'a>> {
        OxcVec::new_in(self.allocator)
    }

    pub(crate) fn clone_expression(&self, expression: &Expression<'_>) -> Expression<'a> {
        expression.clone_in(self.allocator)
    }

    pub(crate) fn super_call_statement(&self, args: &[&str]) -> Statement<'a> {
        let callee = self.ast.expression_super(SPAN);
        self.call_expression_statement(callee, self.expressions_from_identifiers(args))
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
        self.ast.statement_expression(SPAN, call)
    }

    pub(crate) fn this_field_call_expression(
        &self,
        field: &str,
        method: &str,
        args: OxcVec<'a, Expression<'a>>,
    ) -> Expression<'a> {
        let field_expression = self.member_expression_to_expression(self.this_member(field));
        let property = self
            .ast
            .identifier_name(SPAN, self.allocator.alloc_str(method));
        let member = self
            .ast
            .member_expression_static(SPAN, field_expression, property, false);
        let callee = self.member_expression_to_expression(member);
        let arguments = self
            .ast
            .vec_from_iter(args.into_iter().map(Self::argument_from_expression));
        self.ast
            .expression_call(SPAN, callee, NONE, arguments, false)
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
        let object = self
            .ast
            .expression_identifier(SPAN, GeneratedViewpuName::PARAMS);
        let property = self
            .ast
            .identifier_name(SPAN, self.allocator.alloc_str(field));
        self.member_expression_to_expression(
            self.ast
                .member_expression_static(SPAN, object, property, false),
        )
    }

    pub(crate) fn string_expression(&self, value: &str) -> Expression<'a> {
        self.ast
            .expression_string_literal(SPAN, self.allocator.alloc_str(value), None)
    }

    pub(crate) fn undefined_expression(&self) -> Expression<'a> {
        self.ast
            .expression_identifier(SPAN, GeneratedViewpuName::UNDEFINED)
    }

    pub(crate) fn null_expression(&self) -> Expression<'a> {
        self.ast.expression_null_literal(SPAN)
    }

    pub(crate) fn minus_one_expression(&self) -> Expression<'a> {
        let value =
            self.ast
                .expression_numeric_literal(SPAN, 1.0, Some("1".into()), NumberBase::Decimal);
        self.ast
            .expression_unary(SPAN, UnaryOperator::UnaryNegation, value)
    }

    pub(crate) fn identifier_expression(&self, name: &str) -> Expression<'a> {
        self.ast
            .expression_identifier(SPAN, self.allocator.alloc_str(name))
    }

    pub(crate) fn boolean_expression(&self, value: bool) -> Expression<'a> {
        self.ast.expression_boolean_literal(SPAN, value)
    }

    pub(crate) fn this_member_expression(&self, property: &str) -> Expression<'a> {
        self.member_expression_to_expression(self.this_member(property))
    }

    pub(crate) fn this_expression(&self) -> Expression<'a> {
        self.ast.expression_this(SPAN)
    }

    pub(crate) fn subscriber_manager_delete_statement(&self) -> Statement<'a> {
        let subscriber_manager = self
            .ast
            .expression_identifier(SPAN, GeneratedViewpuName::SUBSCRIBER_MANAGER);
        let get_member = self.ast.member_expression_static(
            SPAN,
            subscriber_manager,
            self.ast
                .identifier_name(SPAN, GeneratedViewpuName::SUBSCRIBER_MANAGER_GET),
            false,
        );
        let get_call = self.ast.expression_call(
            SPAN,
            Expression::from(get_member),
            NONE,
            self.ast.vec(),
            false,
        );
        let delete_member = self.ast.member_expression_static(
            SPAN,
            get_call,
            self.ast.identifier_name(SPAN, GeneratedViewpuName::DELETE),
            false,
        );
        let id_member = self.this_member_expression(GeneratedViewpuName::ID_ACCESSOR);
        let id_call = self
            .ast
            .expression_call(SPAN, id_member, NONE, self.ast.vec(), false);
        let delete_call = self.ast.expression_call(
            SPAN,
            Expression::from(delete_member),
            NONE,
            self.ast.vec1(Self::argument_from_expression(id_call)),
            false,
        );
        self.ast.statement_expression(SPAN, delete_call)
    }

    pub(crate) fn new_expression(
        &self,
        callee_name: &str,
        args: OxcVec<'a, Expression<'a>>,
    ) -> Expression<'a> {
        let callee = self
            .ast
            .expression_identifier(SPAN, self.allocator.alloc_str(callee_name));
        let arguments = self
            .ast
            .vec_from_iter(args.into_iter().map(Self::argument_from_expression));
        self.ast.expression_new(SPAN, callee, NONE, arguments)
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
        let arguments = self
            .ast
            .vec_from_iter(args.into_iter().map(Self::argument_from_expression));
        self.ast
            .expression_call(SPAN, callee, type_arguments, arguments, false)
    }

    fn type_parameter_instantiation(
        &self,
        type_annotation: TSType<'a>,
    ) -> oxc_allocator::Box<'a, oxc_ast::ast::TSTypeParameterInstantiation<'a>> {
        self.ast
            .alloc_ts_type_parameter_instantiation(SPAN, self.ast.vec1(type_annotation))
    }

    pub(crate) fn typeof_equals_function(&self, name: &str, function_type: &str) -> Expression<'a> {
        let argument = self
            .ast
            .expression_identifier(SPAN, self.allocator.alloc_str(name));
        let left = self
            .ast
            .expression_unary(SPAN, UnaryOperator::Typeof, argument);
        let right = self.string_expression(function_type);
        self.ast
            .expression_binary(SPAN, left, BinaryOperator::StrictEquality, right)
    }

    pub(crate) fn params_field_not_undefined(&self, field: &str) -> Expression<'a> {
        let left = self.params_field_expression(field);
        let right = self.undefined_expression();
        self.ast
            .expression_binary(SPAN, left, BinaryOperator::StrictInequality, right)
    }

    pub(crate) fn params_field_equals_undefined(&self, field: &str) -> Expression<'a> {
        let left = self.params_field_expression(field);
        let right = self.undefined_expression();
        self.ast
            .expression_binary(SPAN, left, BinaryOperator::StrictEquality, right)
    }

    pub(crate) fn params_field_truthy(&self, field: &str) -> Expression<'a> {
        self.params_field_expression(field)
    }

    pub(crate) fn params_is_undefined(&self) -> Expression<'a> {
        let left = self.identifier_expression(GeneratedViewpuName::PARAMS);
        let right = self.undefined_expression();
        self.ast
            .expression_binary(SPAN, left, BinaryOperator::StrictEquality, right)
    }

    pub(crate) fn params_has_field(&self, field: &str) -> Expression<'a> {
        let left = self.string_expression(field);
        let right = self.identifier_expression(GeneratedViewpuName::PARAMS);
        self.ast
            .expression_binary(SPAN, left, BinaryOperator::In, right)
    }

    pub(crate) fn params_present_and_has_field(&self, field: &str) -> Expression<'a> {
        let left = self.identifier_expression(GeneratedViewpuName::PARAMS);
        let right = self.params_has_field(field);
        self.ast
            .expression_logical(SPAN, left, LogicalOperator::And, right)
    }

    pub(crate) fn conditional_expression(
        &self,
        test: Expression<'a>,
        consequent: Expression<'a>,
        alternate: Expression<'a>,
    ) -> Expression<'a> {
        self.ast
            .expression_conditional(SPAN, test, consequent, alternate)
    }

    pub(crate) fn if_statement(
        &self,
        test: Expression<'a>,
        statements: OxcVec<'a, Statement<'a>>,
    ) -> Statement<'a> {
        let consequent = self.ast.statement_block(SPAN, statements);
        self.ast.statement_if(SPAN, test, consequent, None)
    }

    pub(crate) fn return_string_statement(&self, value: &str) -> Statement<'a> {
        self.ast
            .statement_return(SPAN, Some(self.string_expression(value)))
    }

    pub(crate) fn return_expression_statement(&self, value: Expression<'a>) -> Statement<'a> {
        self.ast.statement_return(SPAN, Some(value))
    }

    pub(crate) fn return_statement(&self) -> Statement<'a> {
        self.ast.statement_return(SPAN, None)
    }

    pub(crate) fn empty_arrow_function_expression(
        &self,
        params: Option<&FormalParameters<'_>>,
    ) -> Expression<'a> {
        let params = params
            .map(|params| {
                let mut params = params.clone_in(self.allocator);
                params.kind = FormalParameterKind::ArrowFormalParameters;
                self.ast.alloc(params)
            })
            .unwrap_or_else(|| {
                self.ast.alloc_formal_parameters(
                    SPAN,
                    FormalParameterKind::ArrowFormalParameters,
                    self.ast.vec(),
                    NONE,
                )
            });
        let body = self.ast.function_body(SPAN, self.ast.vec(), self.ast.vec());
        self.ast
            .expression_arrow_function(SPAN, false, false, NONE, params, NONE, body)
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
        self.ast.formal_parameter(
            SPAN,
            self.ast.vec(),
            self.ast
                .binding_pattern_binding_identifier(SPAN, self.allocator.alloc_str(name)),
            type_annotation,
            initializer,
            false,
            None,
            false,
            false,
        )
    }

    pub(crate) fn formal_parameters_from_iter<I>(
        &self,
        params: I,
    ) -> OxcVec<'a, FormalParameter<'a>>
    where
        I: IntoIterator<Item = FormalParameter<'a>>,
    {
        self.ast.vec_from_iter(params)
    }

    pub(crate) fn formal_parameters(&self) -> OxcVec<'a, FormalParameter<'a>> {
        OxcVec::new_in(self.allocator)
    }

    pub(crate) fn clone_formal_parameters(
        &self,
        params: &FormalParameters<'_>,
    ) -> oxc_allocator::Box<'a, FormalParameters<'a>> {
        self.ast.alloc(params.clone_in(self.allocator))
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
        self.ast.alloc(params)
    }

    pub(crate) fn clone_type_annotation(
        &self,
        annotation: &TSTypeAnnotation<'_>,
    ) -> oxc_allocator::Box<'a, TSTypeAnnotation<'a>> {
        self.ast.alloc(annotation.clone_in(self.allocator))
    }

    pub(crate) fn string_type_annotation(&self) -> oxc_allocator::Box<'a, TSTypeAnnotation<'a>> {
        self.ast
            .alloc_ts_type_annotation(SPAN, self.ast.ts_type_string_keyword(SPAN))
    }

    pub(crate) fn unknown_type_annotation(&self) -> oxc_allocator::Box<'a, TSTypeAnnotation<'a>> {
        self.ast
            .alloc_ts_type_annotation(SPAN, self.ast.ts_type_unknown_keyword(SPAN))
    }

    pub(crate) fn type_reference_annotation(
        &self,
        name: &str,
    ) -> oxc_allocator::Box<'a, TSTypeAnnotation<'a>> {
        self.ast
            .alloc_ts_type_annotation(SPAN, self.type_reference_type(name))
    }

    fn type_reference_type(&self, name: &str) -> TSType<'a> {
        let type_reference = self.ast.ts_type_type_reference(
            SPAN,
            self.ast
                .ts_type_name_identifier_reference(SPAN, self.allocator.alloc_str(name)),
            Option::<oxc_allocator::Box<'a, oxc_ast::ast::TSTypeParameterInstantiation<'a>>>::None,
        );
        type_reference
    }

    pub(crate) fn state_property_type_annotation(
        &self,
        wrapper_name: &str,
        annotation: Option<&TSTypeAnnotation<'_>>,
    ) -> oxc_allocator::Box<'a, TSTypeAnnotation<'a>> {
        let type_arguments = annotation.map(|annotation| {
            self.ast.alloc_ts_type_parameter_instantiation(
                SPAN,
                self.ast
                    .vec1(annotation.type_annotation.clone_in(self.allocator)),
            )
        });
        let type_reference = self.ast.ts_type_type_reference(
            SPAN,
            self.ast
                .ts_type_name_identifier_reference(SPAN, self.allocator.alloc_str(wrapper_name)),
            type_arguments,
        );
        self.ast.alloc_ts_type_annotation(SPAN, type_reference)
    }

    pub(crate) fn void_type_annotation(&self) -> oxc_allocator::Box<'a, TSTypeAnnotation<'a>> {
        self.ast.alloc(
            self.ast
                .ts_type_annotation(SPAN, self.ast.ts_type_void_keyword(SPAN)),
        )
    }

    pub(crate) fn expressions_from_identifiers(
        &self,
        names: &[&str],
    ) -> OxcVec<'a, Expression<'a>> {
        self.ast.vec_from_iter(names.iter().map(|name| {
            self.ast
                .expression_identifier(SPAN, self.allocator.alloc_str(name))
        }))
    }

    pub(crate) fn expressions_from_iter<I>(&self, expressions: I) -> OxcVec<'a, Expression<'a>>
    where
        I: IntoIterator<Item = Expression<'a>>,
    {
        self.ast.vec_from_iter(expressions)
    }

    fn assignment_statement(
        &self,
        left: oxc_ast::ast::AssignmentTarget<'a>,
        right: Expression<'a>,
    ) -> Statement<'a> {
        let assignment =
            self.ast
                .expression_assignment(SPAN, AssignmentOperator::Assign, left, right);
        self.ast.statement_expression(SPAN, assignment)
    }

    fn call_expression_statement(
        &self,
        callee: Expression<'a>,
        args: OxcVec<'a, Expression<'a>>,
    ) -> Statement<'a> {
        let arguments = self
            .ast
            .vec_from_iter(args.into_iter().map(Self::argument_from_expression));
        let call = self
            .ast
            .expression_call(SPAN, callee, NONE, arguments, false);
        self.ast.statement_expression(SPAN, call)
    }

    fn this_member(&self, property: &str) -> MemberExpression<'a> {
        let object = self.ast.expression_this(SPAN);
        let property = self
            .ast
            .identifier_name(SPAN, self.allocator.alloc_str(property));
        self.ast
            .member_expression_static(SPAN, object, property, false)
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
        self.method_with_type_annotation(
            name,
            params_items,
            None,
            body_stmts,
            is_static,
            accessibility,
        )
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
        let formal_params = self.ast.alloc_formal_parameters(
            SPAN,
            FormalParameterKind::FormalParameter,
            params_items,
            NONE,
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
        let body = self.ast.function_body(SPAN, self.ast.vec(), body_stmts);
        let func = self.ast.alloc_function(
            SPAN,
            oxc_ast::ast::FunctionType::FunctionExpression,
            None,
            false,
            false,
            false,
            NONE,
            NONE,
            formal_params,
            return_type,
            Some(body),
        );
        let key =
            PropertyKey::StaticIdentifier(self.ast.alloc(self.ast.identifier_name(SPAN, name)));
        self.ast.class_element_method_definition(
            SPAN,
            MethodDefinitionType::MethodDefinition,
            self.ast.vec(),
            key,
            func,
            MethodDefinitionKind::Method,
            false,
            is_static,
            false,
            false,
            accessibility,
        )
    }

    pub(crate) fn getter_with_type_annotation(
        &self,
        name: &'a str,
        return_type: Option<oxc_allocator::Box<'a, TSTypeAnnotation<'a>>>,
        body_stmts: OxcVec<'a, Statement<'a>>,
        accessibility: Option<TSAccessibility>,
    ) -> ClassElement<'a> {
        let formal_params = self.ast.alloc_formal_parameters(
            SPAN,
            FormalParameterKind::FormalParameter,
            self.ast.vec(),
            NONE,
        );
        let body = self.ast.function_body(SPAN, self.ast.vec(), body_stmts);
        let func = self.ast.alloc_function(
            SPAN,
            oxc_ast::ast::FunctionType::FunctionExpression,
            None,
            false,
            false,
            false,
            NONE,
            NONE,
            formal_params,
            return_type,
            Some(body),
        );
        let key =
            PropertyKey::StaticIdentifier(self.ast.alloc(self.ast.identifier_name(SPAN, name)));
        self.ast.class_element_method_definition(
            SPAN,
            MethodDefinitionType::MethodDefinition,
            self.ast.vec(),
            key,
            func,
            MethodDefinitionKind::Get,
            false,
            false,
            false,
            false,
            accessibility,
        )
    }

    pub(crate) fn setter_with_param(
        &self,
        name: &'a str,
        param: FormalParameter<'a>,
        body_stmts: OxcVec<'a, Statement<'a>>,
        accessibility: Option<TSAccessibility>,
    ) -> ClassElement<'a> {
        let formal_params = self.ast.alloc_formal_parameters(
            SPAN,
            FormalParameterKind::FormalParameter,
            self.ast.vec1(param),
            NONE,
        );
        let body = self.ast.function_body(SPAN, self.ast.vec(), body_stmts);
        let no_return_type: Option<oxc_allocator::Box<'a, oxc_ast::ast::TSTypeAnnotation<'a>>> =
            None;
        let func = self.ast.alloc_function(
            SPAN,
            oxc_ast::ast::FunctionType::FunctionExpression,
            None,
            false,
            false,
            false,
            NONE,
            NONE,
            formal_params,
            no_return_type,
            Some(body),
        );
        let key =
            PropertyKey::StaticIdentifier(self.ast.alloc(self.ast.identifier_name(SPAN, name)));
        self.ast.class_element_method_definition(
            SPAN,
            MethodDefinitionType::MethodDefinition,
            self.ast.vec(),
            key,
            func,
            MethodDefinitionKind::Set,
            false,
            false,
            false,
            false,
            accessibility,
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
        let key =
            PropertyKey::StaticIdentifier(self.ast.alloc(self.ast.identifier_name(SPAN, name)));
        let accessibility = if is_private {
            Some(TSAccessibility::Private)
        } else {
            None
        };

        self.ast.class_element_property_definition(
            SPAN,
            PropertyDefinitionType::PropertyDefinition,
            self.ast.vec(),
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
        )
    }

    pub(crate) fn class_declaration_statement(
        &self,
        name: &'a str,
        super_class_name: &'a str,
        elements: OxcVec<'a, ClassElement<'a>>,
        is_exported: bool,
        is_default_export: bool,
    ) -> Statement<'a> {
        self.class_declaration_statement_with_decorators(
            name,
            super_class_name,
            elements,
            self.ast.vec(),
            is_exported,
            is_default_export,
        )
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
        let super_class = Some(self.ast.expression_identifier(SPAN, super_class_name));
        let body = self.ast.alloc_class_body(SPAN, elements);
        let class = self.ast.alloc_class(
            SPAN,
            ClassType::ClassDeclaration,
            decorators,
            Some(self.ast.binding_identifier(SPAN, name)),
            NONE,
            super_class,
            NONE,
            self.ast.vec(),
            body,
            false,
            false,
        );

        if is_default_export {
            let decl = oxc_ast::ast::ExportDefaultDeclarationKind::ClassDeclaration(class);
            let export = self.ast.alloc_export_default_declaration(SPAN, decl);
            Statement::ExportDefaultDeclaration(export)
        } else if is_exported {
            let decl = oxc_ast::ast::Declaration::ClassDeclaration(class);
            let export = self.ast.alloc_export_named_declaration(
                SPAN,
                Some(decl),
                self.ast.vec(),
                None,
                oxc_ast::ast::ImportOrExportKind::Value,
                NONE,
            );
            Statement::ExportNamedDeclaration(export)
        } else {
            Statement::ClassDeclaration(class)
        }
    }
}
