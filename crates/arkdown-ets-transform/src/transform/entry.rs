use oxc_allocator::{Allocator, ArenaBox, CloneIn, GetAllocator, Vec as OxcVec};
use oxc_ast::ast::{
    Argument, ArrowFunctionBody, BindingPattern, Expression, FormalParameterKind, FormalParameters,
    IdentifierName, MemberExpression, NewExpression, ObjectPropertyKind, PropertyKey, PropertyKind,
    Statement, VariableDeclarationKind, VariableDeclarator,
};
use oxc_ast::builder::AstBuilder;
use oxc_span::SPAN;
use oxc_syntax::operator::{BinaryOperator, LogicalOperator};
use std::path::PathBuf;

use crate::transform::component_meta::ArkuiRuntimeNames;
use arkdown_path::ArtifactPathSerializer;

/// Route metadata used by hvigor's generated `registerNamedRoute` call.
#[derive(Debug, Clone)]
pub struct EntryRegistrationInfo<'a> {
    pub bundle_name: String,
    pub module_name: String,
    pub page_path: String,
    pub page_full_path: String,
    pub integrated_hsp: String,
    pub module_type: String,
    /// SDK `projectConfig.cardObj[resourceFileName]` classification.
    pub form_entry: bool,
    pub options: EntryOptions<'a>,
}

/// Parsed `@Entry(...)` option value.
///
/// hvigor's ets-loader models object-literal entry arguments as
/// `EntryOptionValue { routeName, storage, useSharedStorage }`. Keep the same
/// semantic shape here so codegen can decide route and LocalStorage behavior
/// from AST facts instead of treating the whole decorator argument as storage.
#[derive(Debug, Clone, Default)]
pub struct EntryOptions<'a> {
    argument: Option<&'a Expression<'a>>,
    route_name: Option<&'a Expression<'a>>,
    storage: Option<&'a Expression<'a>>,
    use_shared_storage: Option<&'a Expression<'a>>,
    object_literal: bool,
}

impl<'a> EntryOptions<'a> {
    pub fn from_argument(argument: Option<&'a Expression<'a>>) -> Self {
        let Some(argument) = argument else {
            return Self::default();
        };

        let Expression::ObjectExpression(object) = argument else {
            return Self {
                argument: Some(argument),
                ..Self::default()
            };
        };

        let mut options = Self {
            object_literal: true,
            ..Self::default()
        };
        for property in &object.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                continue;
            };
            let Some(name) = Self::property_name(&property.key) else {
                continue;
            };
            match name.as_str() {
                EntryRuntimeName::ROUTE_NAME => options.route_name = Some(&property.value),
                EntryRuntimeName::STORAGE => options.storage = Some(&property.value),
                EntryRuntimeName::USE_SHARED_STORAGE => {
                    options.use_shared_storage = Some(&property.value);
                }
                _ => {}
            }
        }
        options
    }

    fn property_name(key: &PropertyKey<'_>) -> Option<String> {
        match key {
            PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.to_string()),
            PropertyKey::StringLiteral(literal) => Some(literal.value.to_string()),
            _ => None,
        }
    }
}

#[derive(Debug, Default)]
pub struct EntryRegistrationBlock<'a> {
    registrations: Vec<EntryRegistrationItem<'a>>,
}

impl<'a> EntryRegistrationBlock<'a> {
    pub fn push(&mut self, component_name: &str, info: Option<&EntryRegistrationInfo<'a>>) {
        self.registrations.push(EntryRegistrationItem {
            component_name: component_name.to_string(),
            info: info.cloned(),
        });
    }

    pub(super) fn statements<'b>(
        &self,
        ast: AstBuilder<'b>,
        allocator: &'b Allocator,
        runtime_names: &ArkuiRuntimeNames,
    ) -> OxcVec<'b, Statement<'b>> {
        let mut statements = OxcVec::new_in(&ast);
        for registration in &self.registrations {
            statements.extend(
                EntryRegistrationAstEmitter::new(
                    &registration.component_name,
                    registration.info.as_ref(),
                    runtime_names,
                )
                .statements(&ast, allocator),
            );
        }
        statements
    }
}

struct EntryRuntimeName;

impl EntryRuntimeName {
    const REGISTER_NAMED_ROUTE: &'static str = "registerNamedRoute";
    const LOAD_ETS_CARD: &'static str = "loadEtsCard";
    const ALLOCATE_NEW_ELMET_ID_FOR_NEXT_COMPONENT: &'static str =
        "AllocateNewElmetIdForNextComponent";
    const ETS_SEGMENT: &'static str = "ets";
    const EMPTY_ROUTE: &'static str = "";
    const BUNDLE_NAME: &'static str = "bundleName";
    const MODULE_NAME: &'static str = "moduleName";
    const PAGE_PATH: &'static str = "pagePath";
    const PAGE_FULL_PATH: &'static str = "pageFullPath";
    const INTEGRATED_HSP: &'static str = "integratedHsp";
    const MODULE_TYPE: &'static str = "moduleType";
    const ROUTE_NAME: &'static str = "routeName";
    const ROUTENAME_NODE: &'static str = "routeNameNode";
    const STORAGE: &'static str = "storage";
    const USE_SHARED_STORAGE: &'static str = "useSharedStorage";
    const LOCAL_STORAGE: &'static str = "LocalStorage";
    const GET_SHARED: &'static str = "getShared";
}

#[derive(Debug)]
struct EntryRegistrationItem<'a> {
    component_name: String,
    info: Option<EntryRegistrationInfo<'a>>,
}

struct EntryRegistrationAstEmitter<'a, 'runtime> {
    component_name: &'a str,
    info: Option<&'a EntryRegistrationInfo<'a>>,
    runtime_names: &'runtime ArkuiRuntimeNames,
}

impl<'a, 'runtime> EntryRegistrationAstEmitter<'a, 'runtime> {
    fn new(
        component_name: &'a str,
        info: Option<&'a EntryRegistrationInfo<'a>>,
        runtime_names: &'runtime ArkuiRuntimeNames,
    ) -> Self {
        Self {
            component_name,
            info,
            runtime_names,
        }
    }

    fn statements<'b>(
        &self,
        ast: &AstBuilder<'b>,
        allocator: &'b Allocator,
    ) -> OxcVec<'b, Statement<'b>> {
        let Some(info) = self.info else {
            return OxcVec::from_value_in(self.named_route_statement(ast, allocator), ast);
        };

        if info.form_entry {
            return self.card_statements(ast, info, allocator);
        }

        if info.options.object_literal {
            return self.object_entry_statements(ast, info, allocator);
        }

        if info.options.argument.is_some() {
            return OxcVec::from_value_in(
                self.identifier_entry_statement(
                    ast,
                    allocator,
                    info.options.argument.expect("checked above"),
                ),
                ast,
            );
        }

        OxcVec::from_value_in(self.named_route_statement(ast, allocator), ast)
    }

    fn named_route_statement<'b>(
        &self,
        ast: &AstBuilder<'b>,
        allocator: &'b Allocator,
    ) -> Statement<'b> {
        self.named_route_statement_with(ast, allocator, None, None)
    }

    fn named_route_statement_with<'b>(
        &self,
        ast: &AstBuilder<'b>,
        allocator: &'b Allocator,
        route_name: Option<Expression<'b>>,
        storage: Option<Expression<'b>>,
    ) -> Statement<'b> {
        let arguments = OxcVec::from_iter_in(
            [
                Self::argument_from_expression(self.component_factory_arrow(ast, storage)),
                Self::argument_from_expression(route_name.unwrap_or_else(|| {
                    Expression::new_string_literal(SPAN, EntryRuntimeName::EMPTY_ROUTE, None, ast)
                })),
                Self::argument_from_expression(self.metadata_expression(ast)),
            ],
            ast,
        );
        let _ = allocator;
        self.call_statement(ast, EntryRuntimeName::REGISTER_NAMED_ROUTE, arguments)
    }

    fn object_entry_statements<'b>(
        &self,
        ast: &AstBuilder<'b>,
        info: &EntryRegistrationInfo<'_>,
        allocator: &'b Allocator,
    ) -> OxcVec<'b, Statement<'b>> {
        let route_name = info
            .options
            .route_name
            .map(|expression| expression.clone_in(allocator));
        let storage = self.entry_storage_expression(ast, allocator, &info.options);

        match (route_name, storage) {
            (Some(route_name), Some(storage)) => OxcVec::from_value_in(
                self.route_and_storage_block(ast, allocator, route_name, storage),
                ast,
            ),
            (Some(route_name), None) => {
                OxcVec::from_value_in(self.route_block(ast, allocator, route_name), ast)
            }
            (None, Some(storage)) => OxcVec::from_value_in(
                self.named_route_statement_with(ast, allocator, None, Some(storage)),
                ast,
            ),
            (None, None) => OxcVec::from_value_in(self.named_route_statement(ast, allocator), ast),
        }
    }

    fn route_and_storage_block<'b>(
        &self,
        ast: &AstBuilder<'b>,
        allocator: &'b Allocator,
        route_name: Expression<'b>,
        storage: Expression<'b>,
    ) -> Statement<'b> {
        let mut statements = OxcVec::new_in(ast);
        statements.push(self.route_name_declaration(ast, route_name));
        let alternate_storage = storage.clone_in(allocator);
        let consequent = Statement::new_block_statement(
            SPAN,
            OxcVec::from_value_in(
                self.named_route_statement_with(
                    ast,
                    allocator,
                    Some(Expression::new_identifier(
                        SPAN,
                        EntryRuntimeName::ROUTENAME_NODE,
                        ast,
                    )),
                    Some(storage),
                ),
                ast,
            ),
            ast,
        );
        let alternate = Statement::new_block_statement(
            SPAN,
            OxcVec::from_value_in(
                self.named_route_statement_with(ast, allocator, None, Some(alternate_storage)),
                ast,
            ),
            ast,
        );
        statements.push(Statement::new_if_statement(
            SPAN,
            self.route_name_node_not_undefined(ast),
            consequent,
            Some(alternate),
            ast,
        ));
        Statement::new_block_statement(SPAN, statements, ast)
    }

    fn route_block<'b>(
        &self,
        ast: &AstBuilder<'b>,
        allocator: &'b Allocator,
        route_name: Expression<'b>,
    ) -> Statement<'b> {
        let mut statements = OxcVec::new_in(ast);
        statements.push(self.route_name_declaration(ast, route_name));
        statements.push(self.named_route_statement_with(
            ast,
            allocator,
            Some(Expression::new_identifier(
                SPAN,
                EntryRuntimeName::ROUTENAME_NODE,
                ast,
            )),
            None,
        ));
        Statement::new_block_statement(SPAN, statements, ast)
    }

    fn route_name_declaration<'b>(
        &self,
        ast: &AstBuilder<'b>,
        init: Expression<'b>,
    ) -> Statement<'b> {
        let binding =
            BindingPattern::new_binding_identifier(SPAN, EntryRuntimeName::ROUTENAME_NODE, ast);
        let declarator = VariableDeclarator::new(SPAN, binding, None, Some(init), false, ast);
        Statement::new_variable_declaration(
            SPAN,
            VariableDeclarationKind::Let,
            OxcVec::from_value_in(declarator, ast),
            false,
            ast,
        )
    }

    fn route_name_node_not_undefined<'b>(&self, ast: &AstBuilder<'b>) -> Expression<'b> {
        Expression::new_binary_expression(
            SPAN,
            Expression::new_identifier(SPAN, EntryRuntimeName::ROUTENAME_NODE, ast),
            BinaryOperator::Inequality,
            Expression::new_identifier(
                SPAN,
                ast.allocator()
                    .alloc_str(self.runtime_names.undefined.as_str()),
                ast,
            ),
            ast,
        )
    }

    fn identifier_entry_statement<'b>(
        &self,
        ast: &AstBuilder<'b>,
        allocator: &'b Allocator,
        argument: &Expression<'_>,
    ) -> Statement<'b> {
        let route_and_storage = self.identifier_route_and_storage_condition(
            ast,
            allocator,
            argument,
            BinaryOperator::Inequality,
            BinaryOperator::Inequality,
        );
        let route_only = self.identifier_route_and_storage_condition(
            ast,
            allocator,
            argument,
            BinaryOperator::Inequality,
            BinaryOperator::Equality,
        );
        let storage_only = self.identifier_route_and_storage_condition(
            ast,
            allocator,
            argument,
            BinaryOperator::Equality,
            BinaryOperator::Inequality,
        );
        let shared_storage = self.identifier_property_condition(
            ast,
            allocator,
            argument,
            EntryRuntimeName::USE_SHARED_STORAGE,
            BinaryOperator::Inequality,
        );

        let route_and_storage_statement = self.named_route_statement_with(
            ast,
            allocator,
            Some(self.identifier_property_expression(
                ast,
                allocator,
                argument,
                EntryRuntimeName::ROUTE_NAME,
            )),
            Some(self.identifier_storage_expression(ast, allocator, argument)),
        );
        let route_only_statement = self.named_route_statement_with(
            ast,
            allocator,
            Some(self.identifier_property_expression(
                ast,
                allocator,
                argument,
                EntryRuntimeName::ROUTE_NAME,
            )),
            Some(self.identifier_storage_expression(ast, allocator, argument)),
        );
        let storage_only_statement = self.named_route_statement_with(
            ast,
            allocator,
            None,
            Some(self.identifier_storage_expression(ast, allocator, argument)),
        );
        let shared_storage_statement = self.named_route_statement_with(
            ast,
            allocator,
            None,
            Some(self.local_storage_get_shared_call(ast)),
        );
        let fallback_statement = self.named_route_statement_with(
            ast,
            allocator,
            None,
            Some(argument.clone_in(allocator)),
        );

        let shared_or_fallback = Statement::new_if_statement(
            SPAN,
            shared_storage,
            Statement::new_block_statement(
                SPAN,
                OxcVec::from_value_in(shared_storage_statement, ast),
                ast,
            ),
            Some(Statement::new_block_statement(
                SPAN,
                OxcVec::from_value_in(fallback_statement, ast),
                ast,
            )),
            ast,
        );
        let storage_or_shared = Statement::new_if_statement(
            SPAN,
            storage_only,
            Statement::new_block_statement(
                SPAN,
                OxcVec::from_value_in(storage_only_statement, ast),
                ast,
            ),
            Some(shared_or_fallback),
            ast,
        );
        let route_or_storage = Statement::new_if_statement(
            SPAN,
            route_only,
            Statement::new_block_statement(
                SPAN,
                OxcVec::from_value_in(route_only_statement, ast),
                ast,
            ),
            Some(storage_or_shared),
            ast,
        );
        Statement::new_if_statement(
            SPAN,
            route_and_storage,
            Statement::new_block_statement(
                SPAN,
                OxcVec::from_value_in(route_and_storage_statement, ast),
                ast,
            ),
            Some(route_or_storage),
            ast,
        )
    }

    fn identifier_route_and_storage_condition<'b>(
        &self,
        ast: &AstBuilder<'b>,
        allocator: &'b Allocator,
        argument: &Expression<'_>,
        route_operator: BinaryOperator,
        storage_operator: BinaryOperator,
    ) -> Expression<'b> {
        let argument_exists = argument.clone_in(allocator);
        let route_condition = self.identifier_property_comparison(
            ast,
            allocator,
            argument,
            EntryRuntimeName::ROUTE_NAME,
            route_operator,
        );
        let storage_condition = self.identifier_property_comparison(
            ast,
            allocator,
            argument,
            EntryRuntimeName::STORAGE,
            storage_operator,
        );
        Expression::new_logical_expression(
            SPAN,
            Expression::new_logical_expression(
                SPAN,
                argument_exists,
                LogicalOperator::And,
                route_condition,
                ast,
            ),
            LogicalOperator::And,
            storage_condition,
            ast,
        )
    }

    fn identifier_property_condition<'b>(
        &self,
        ast: &AstBuilder<'b>,
        allocator: &'b Allocator,
        argument: &Expression<'_>,
        property: &'static str,
        operator: BinaryOperator,
    ) -> Expression<'b> {
        Expression::new_logical_expression(
            SPAN,
            argument.clone_in(allocator),
            LogicalOperator::And,
            self.identifier_property_comparison(ast, allocator, argument, property, operator),
            ast,
        )
    }

    fn identifier_property_comparison<'b>(
        &self,
        ast: &AstBuilder<'b>,
        allocator: &'b Allocator,
        argument: &Expression<'_>,
        property: &'static str,
        operator: BinaryOperator,
    ) -> Expression<'b> {
        Expression::new_binary_expression(
            SPAN,
            self.identifier_property_expression(ast, allocator, argument, property),
            operator,
            Expression::new_identifier(
                SPAN,
                ast.allocator()
                    .alloc_str(self.runtime_names.undefined.as_str()),
                ast,
            ),
            ast,
        )
    }

    fn identifier_property_expression<'b>(
        &self,
        ast: &AstBuilder<'b>,
        allocator: &'b Allocator,
        argument: &Expression<'_>,
        property: &'static str,
    ) -> Expression<'b> {
        Self::member_expression_to_expression(MemberExpression::new_static_member_expression(
            SPAN,
            argument.clone_in(allocator),
            IdentifierName::new(SPAN, property, ast),
            false,
            ast,
        ))
    }

    fn identifier_storage_expression<'b>(
        &self,
        ast: &AstBuilder<'b>,
        allocator: &'b Allocator,
        argument: &Expression<'_>,
    ) -> Expression<'b> {
        Expression::new_conditional_expression(
            SPAN,
            self.identifier_property_expression(
                ast,
                allocator,
                argument,
                EntryRuntimeName::USE_SHARED_STORAGE,
            ),
            self.local_storage_get_shared_call(ast),
            self.identifier_property_expression(
                ast,
                allocator,
                argument,
                EntryRuntimeName::STORAGE,
            ),
            ast,
        )
    }

    fn entry_storage_expression<'b>(
        &self,
        ast: &AstBuilder<'b>,
        allocator: &'b Allocator,
        options: &EntryOptions<'_>,
    ) -> Option<Expression<'b>> {
        if let Some(use_shared_storage) = options.use_shared_storage {
            return Some(Expression::new_conditional_expression(
                SPAN,
                use_shared_storage.clone_in(allocator),
                self.local_storage_get_shared_call(ast),
                options
                    .storage
                    .map(|storage| storage.clone_in(allocator))
                    .unwrap_or_else(|| {
                        Expression::new_identifier(
                            SPAN,
                            ast.allocator()
                                .alloc_str(self.runtime_names.undefined.as_str()),
                            ast,
                        )
                    }),
                ast,
            ));
        }

        options.storage.map(|storage| storage.clone_in(allocator))
    }

    fn local_storage_get_shared_call<'b>(&self, ast: &AstBuilder<'b>) -> Expression<'b> {
        let member = MemberExpression::new_static_member_expression(
            SPAN,
            Expression::new_identifier(SPAN, EntryRuntimeName::LOCAL_STORAGE, ast),
            IdentifierName::new(SPAN, EntryRuntimeName::GET_SHARED, ast),
            false,
            ast,
        );
        Expression::new_call_expression(
            SPAN,
            Self::member_expression_to_expression(member),
            None,
            OxcVec::new_in(ast),
            false,
            ast,
        )
    }

    fn card_statements<'b>(
        &self,
        ast: &AstBuilder<'b>,
        info: &EntryRegistrationInfo<'_>,
        allocator: &'b Allocator,
    ) -> OxcVec<'b, Statement<'b>> {
        let allocate_call = self.member_call_expression(
            ast,
            &self.runtime_names.view_stack_processor,
            EntryRuntimeName::ALLOCATE_NEW_ELMET_ID_FOR_NEXT_COMPONENT,
            OxcVec::new_in(ast),
        );
        let start_call = self.member_call_expression(
            ast,
            &self.runtime_names.view_stack_processor,
            &self.runtime_names.start_get_access_recording_for,
            OxcVec::from_value_in(Self::argument_from_expression(allocate_call), ast),
        );
        let storage = info
            .options
            .argument
            .map(|expression| expression.clone_in(allocator));
        let load_call = Expression::new_call_expression(
            SPAN,
            Expression::new_identifier(SPAN, EntryRuntimeName::LOAD_ETS_CARD, ast),
            None,
            OxcVec::from_iter_in(
                [
                    Self::argument_from_expression(Expression::NewExpression(
                        self.component_new_expression(ast, storage),
                    )),
                    Self::argument_from_expression(Expression::new_string_literal(
                        SPAN,
                        ast.allocator().alloc_str(&self.record_name(info)),
                        None,
                        ast,
                    )),
                ],
                ast,
            ),
            false,
            ast,
        );
        let stop_call = self.member_call_expression(
            ast,
            &self.runtime_names.view_stack_processor,
            &self.runtime_names.stop_get_access_recording,
            OxcVec::new_in(ast),
        );

        OxcVec::from_iter_in(
            [
                Statement::new_expression_statement(SPAN, start_call, ast),
                Statement::new_expression_statement(SPAN, load_call, ast),
                Statement::new_expression_statement(SPAN, stop_call, ast),
            ],
            ast,
        )
    }

    fn component_factory_arrow<'b>(
        &self,
        ast: &AstBuilder<'b>,
        storage: Option<Expression<'b>>,
    ) -> Expression<'b> {
        let expression = self.component_new_expression(ast, storage);
        let body = ArrowFunctionBody::NewExpression(expression);
        Expression::new_arrow_function_expression(
            SPAN,
            false,
            None,
            ArenaBox::new_in(
                FormalParameters::new(
                    SPAN,
                    FormalParameterKind::ArrowFormalParameters,
                    OxcVec::new_in(ast),
                    None,
                    ast,
                ),
                ast,
            ),
            None,
            body,
            ast,
        )
    }

    fn component_new_expression<'b>(
        &self,
        ast: &AstBuilder<'b>,
        storage: Option<Expression<'b>>,
    ) -> ArenaBox<'b, NewExpression<'b>> {
        let mut arguments = OxcVec::from_iter_in(
            [
                Self::argument_from_expression(Expression::new_identifier(
                    SPAN,
                    ast.allocator().alloc_str(&self.runtime_names.undefined),
                    ast,
                )),
                Self::argument_from_expression(Expression::new_object_expression(
                    SPAN,
                    OxcVec::new_in(ast),
                    ast,
                )),
            ],
            ast,
        );
        if let Some(storage) = storage {
            arguments.push(Self::argument_from_expression(storage));
        }
        NewExpression::boxed(
            SPAN,
            Expression::new_identifier(SPAN, ast.allocator().alloc_str(self.component_name), ast),
            None,
            arguments,
            ast,
        )
    }

    fn metadata_expression<'b>(&self, ast: &AstBuilder<'b>) -> Expression<'b> {
        let Some(info) = self.info else {
            return Expression::new_object_expression(SPAN, OxcVec::new_in(ast), ast);
        };
        Expression::new_object_expression(
            SPAN,
            OxcVec::from_iter_in(
                [
                    self.string_property(ast, EntryRuntimeName::BUNDLE_NAME, &info.bundle_name),
                    self.string_property(ast, EntryRuntimeName::MODULE_NAME, &info.module_name),
                    self.string_property(ast, EntryRuntimeName::PAGE_PATH, &info.page_path),
                    self.string_property(
                        ast,
                        EntryRuntimeName::PAGE_FULL_PATH,
                        &info.page_full_path,
                    ),
                    self.string_property(
                        ast,
                        EntryRuntimeName::INTEGRATED_HSP,
                        &info.integrated_hsp,
                    ),
                    self.string_property(ast, EntryRuntimeName::MODULE_TYPE, &info.module_type),
                ],
                ast,
            ),
            ast,
        )
    }

    fn record_name(&self, info: &EntryRegistrationInfo<'_>) -> String {
        let mut record_path = PathBuf::from(&info.bundle_name);
        record_path.push(&info.module_name);
        record_path.push(EntryRuntimeName::ETS_SEGMENT);
        record_path.push(&info.page_path);
        ArtifactPathSerializer::rel_to_slash(&record_path)
    }

    fn string_property<'b>(
        &self,
        ast: &AstBuilder<'b>,
        name: &'static str,
        value: &str,
    ) -> ObjectPropertyKind<'b> {
        ObjectPropertyKind::new_object_property(
            SPAN,
            PropertyKind::Init,
            PropertyKey::new_static_identifier(SPAN, name, ast),
            Expression::new_string_literal(SPAN, ast.allocator().alloc_str(value), None, ast),
            false,
            false,
            false,
            ast,
        )
    }

    fn call_statement<'b>(
        &self,
        ast: &AstBuilder<'b>,
        callee_name: &str,
        arguments: OxcVec<'b, Argument<'b>>,
    ) -> Statement<'b> {
        Statement::new_expression_statement(
            SPAN,
            Expression::new_call_expression(
                SPAN,
                Expression::new_identifier(SPAN, ast.allocator().alloc_str(callee_name), ast),
                None,
                arguments,
                false,
                ast,
            ),
            ast,
        )
    }

    fn member_call_expression<'b>(
        &self,
        ast: &AstBuilder<'b>,
        object: &str,
        property: &str,
        arguments: OxcVec<'b, Argument<'b>>,
    ) -> Expression<'b> {
        let member = MemberExpression::new_static_member_expression(
            SPAN,
            Expression::new_identifier(SPAN, ast.allocator().alloc_str(object), ast),
            IdentifierName::new(SPAN, ast.allocator().alloc_str(property), ast),
            false,
            ast,
        );
        Expression::new_call_expression(
            SPAN,
            Self::member_expression_to_expression(member),
            None,
            arguments,
            false,
            ast,
        )
    }

    fn argument_from_expression<'b>(expression: Expression<'b>) -> Argument<'b> {
        Argument::from(expression)
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
