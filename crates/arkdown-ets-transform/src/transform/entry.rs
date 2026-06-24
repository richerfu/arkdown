use oxc_allocator::{Allocator, CloneIn, Vec as OxcVec};
use oxc_ast::ast::{
    Argument, Expression, FormalParameterKind, MemberExpression, ObjectPropertyKind, PropertyKey,
    PropertyKind, Statement, VariableDeclarationKind,
};
use oxc_ast::{AstBuilder, NONE};
use oxc_span::SPAN;
use oxc_syntax::operator::BinaryOperator;
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
        let mut statements = ast.vec();
        for registration in &self.registrations {
            statements.extend(
                EntryRegistrationAstEmitter::new(
                    &registration.component_name,
                    registration.info.as_ref(),
                    runtime_names,
                )
                .statements(ast, allocator),
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
        ast: AstBuilder<'b>,
        allocator: &'b Allocator,
    ) -> OxcVec<'b, Statement<'b>> {
        let Some(info) = self.info else {
            return ast.vec1(self.named_route_statement(ast, allocator));
        };

        if info.options.object_literal {
            return self.object_entry_statements(ast, info, allocator);
        }

        if info.options.argument.is_some() {
            return self.card_statements(ast, info, allocator);
        }

        ast.vec1(self.named_route_statement(ast, allocator))
    }

    fn named_route_statement<'b>(
        &self,
        ast: AstBuilder<'b>,
        allocator: &'b Allocator,
    ) -> Statement<'b> {
        self.named_route_statement_with(ast, allocator, None, None)
    }

    fn named_route_statement_with<'b>(
        &self,
        ast: AstBuilder<'b>,
        allocator: &'b Allocator,
        route_name: Option<Expression<'b>>,
        storage: Option<Expression<'b>>,
    ) -> Statement<'b> {
        let arguments = ast.vec_from_iter([
            Self::argument_from_expression(self.component_factory_arrow(ast, storage)),
            Self::argument_from_expression(route_name.unwrap_or_else(|| {
                ast.expression_string_literal(SPAN, EntryRuntimeName::EMPTY_ROUTE, None)
            })),
            Self::argument_from_expression(self.metadata_expression(ast)),
        ]);
        let _ = allocator;
        self.call_statement(ast, EntryRuntimeName::REGISTER_NAMED_ROUTE, arguments)
    }

    fn object_entry_statements<'b>(
        &self,
        ast: AstBuilder<'b>,
        info: &EntryRegistrationInfo<'_>,
        allocator: &'b Allocator,
    ) -> OxcVec<'b, Statement<'b>> {
        let route_name = info
            .options
            .route_name
            .map(|expression| expression.clone_in(allocator));
        let storage = self.entry_storage_expression(ast, allocator, &info.options);

        match (route_name, storage) {
            (Some(route_name), Some(storage)) => {
                ast.vec1(self.route_and_storage_block(ast, allocator, route_name, storage))
            }
            (Some(route_name), None) => ast.vec1(self.route_block(ast, allocator, route_name)),
            (None, Some(storage)) => {
                ast.vec1(self.named_route_statement_with(ast, allocator, None, Some(storage)))
            }
            (None, None) => ast.vec1(self.named_route_statement(ast, allocator)),
        }
    }

    fn route_and_storage_block<'b>(
        &self,
        ast: AstBuilder<'b>,
        allocator: &'b Allocator,
        route_name: Expression<'b>,
        storage: Expression<'b>,
    ) -> Statement<'b> {
        let mut statements = ast.vec();
        statements.push(self.route_name_declaration(ast, route_name));
        let consequent = ast.statement_block(
            SPAN,
            ast.vec1(self.named_route_statement_with(
                ast,
                allocator,
                Some(ast.expression_identifier(SPAN, EntryRuntimeName::ROUTENAME_NODE)),
                Some(storage),
            )),
        );
        let alternate =
            ast.statement_block(SPAN, ast.vec1(self.named_route_statement(ast, allocator)));
        statements.push(ast.statement_if(
            SPAN,
            self.route_name_node_not_undefined(ast),
            consequent,
            Some(alternate),
        ));
        ast.statement_block(SPAN, statements)
    }

    fn route_block<'b>(
        &self,
        ast: AstBuilder<'b>,
        allocator: &'b Allocator,
        route_name: Expression<'b>,
    ) -> Statement<'b> {
        let mut statements = ast.vec();
        statements.push(self.route_name_declaration(ast, route_name));
        statements.push(self.named_route_statement_with(
            ast,
            allocator,
            Some(ast.expression_identifier(SPAN, EntryRuntimeName::ROUTENAME_NODE)),
            None,
        ));
        ast.statement_block(SPAN, statements)
    }

    fn route_name_declaration<'b>(
        &self,
        ast: AstBuilder<'b>,
        init: Expression<'b>,
    ) -> Statement<'b> {
        let binding =
            ast.binding_pattern_binding_identifier(SPAN, EntryRuntimeName::ROUTENAME_NODE);
        let declarator = ast.variable_declarator(
            SPAN,
            VariableDeclarationKind::Let,
            binding,
            NONE,
            Some(init),
            false,
        );
        Statement::from(ast.declaration_variable(
            SPAN,
            VariableDeclarationKind::Let,
            ast.vec1(declarator),
            false,
        ))
    }

    fn route_name_node_not_undefined<'b>(&self, ast: AstBuilder<'b>) -> Expression<'b> {
        ast.expression_binary(
            SPAN,
            ast.expression_identifier(SPAN, EntryRuntimeName::ROUTENAME_NODE),
            BinaryOperator::StrictInequality,
            ast.expression_identifier(SPAN, ast.str(self.runtime_names.undefined.as_str())),
        )
    }

    fn entry_storage_expression<'b>(
        &self,
        ast: AstBuilder<'b>,
        allocator: &'b Allocator,
        options: &EntryOptions<'_>,
    ) -> Option<Expression<'b>> {
        if let Some(use_shared_storage) = options.use_shared_storage {
            return Some(
                ast.expression_conditional(
                    SPAN,
                    use_shared_storage.clone_in(allocator),
                    self.local_storage_get_shared_call(ast),
                    options
                        .storage
                        .map(|storage| storage.clone_in(allocator))
                        .unwrap_or_else(|| {
                            ast.expression_identifier(
                                SPAN,
                                ast.str(self.runtime_names.undefined.as_str()),
                            )
                        }),
                ),
            );
        }

        options.storage.map(|storage| storage.clone_in(allocator))
    }

    fn local_storage_get_shared_call<'b>(&self, ast: AstBuilder<'b>) -> Expression<'b> {
        let member = ast.member_expression_static(
            SPAN,
            ast.expression_identifier(SPAN, EntryRuntimeName::LOCAL_STORAGE),
            ast.identifier_name(SPAN, EntryRuntimeName::GET_SHARED),
            false,
        );
        ast.expression_call(
            SPAN,
            Self::member_expression_to_expression(member),
            NONE,
            ast.vec(),
            false,
        )
    }

    fn card_statements<'b>(
        &self,
        ast: AstBuilder<'b>,
        info: &EntryRegistrationInfo<'_>,
        allocator: &'b Allocator,
    ) -> OxcVec<'b, Statement<'b>> {
        let allocate_call = self.member_call_expression(
            ast,
            &self.runtime_names.view_stack_processor,
            EntryRuntimeName::ALLOCATE_NEW_ELMET_ID_FOR_NEXT_COMPONENT,
            ast.vec(),
        );
        let start_call = self.member_call_expression(
            ast,
            &self.runtime_names.view_stack_processor,
            &self.runtime_names.start_get_access_recording_for,
            ast.vec1(Self::argument_from_expression(allocate_call)),
        );
        let storage = info
            .options
            .argument
            .map(|expression| expression.clone_in(allocator))
            .unwrap_or_else(|| {
                ast.expression_identifier(SPAN, ast.str(&self.runtime_names.undefined))
            });
        let load_call = ast.expression_call(
            SPAN,
            ast.expression_identifier(SPAN, EntryRuntimeName::LOAD_ETS_CARD),
            NONE,
            ast.vec_from_iter([
                Self::argument_from_expression(self.component_new_expression(ast, Some(storage))),
                Self::argument_from_expression(ast.expression_string_literal(
                    SPAN,
                    ast.str(&self.record_name(info)),
                    None,
                )),
            ]),
            false,
        );
        let stop_call = self.member_call_expression(
            ast,
            &self.runtime_names.view_stack_processor,
            &self.runtime_names.stop_get_access_recording,
            ast.vec(),
        );

        ast.vec_from_iter([
            ast.statement_expression(SPAN, start_call),
            ast.statement_expression(SPAN, load_call),
            ast.statement_expression(SPAN, stop_call),
        ])
    }

    fn component_factory_arrow<'b>(
        &self,
        ast: AstBuilder<'b>,
        storage: Option<Expression<'b>>,
    ) -> Expression<'b> {
        let expression = self.component_new_expression(ast, storage);
        let body = ast.function_body(
            SPAN,
            ast.vec(),
            ast.vec1(ast.statement_expression(SPAN, expression)),
        );
        ast.expression_arrow_function(
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
            body,
        )
    }

    fn component_new_expression<'b>(
        &self,
        ast: AstBuilder<'b>,
        storage: Option<Expression<'b>>,
    ) -> Expression<'b> {
        let mut arguments = ast.vec_from_iter([
            Self::argument_from_expression(
                ast.expression_identifier(SPAN, ast.str(&self.runtime_names.undefined)),
            ),
            Self::argument_from_expression(ast.expression_object(SPAN, ast.vec())),
        ]);
        if let Some(storage) = storage {
            arguments.push(Self::argument_from_expression(storage));
        }
        ast.expression_new(
            SPAN,
            ast.expression_identifier(SPAN, ast.str(self.component_name)),
            NONE,
            arguments,
        )
    }

    fn metadata_expression<'b>(&self, ast: AstBuilder<'b>) -> Expression<'b> {
        let Some(info) = self.info else {
            return ast.expression_object(SPAN, ast.vec());
        };
        ast.expression_object(
            SPAN,
            ast.vec_from_iter([
                self.string_property(ast, EntryRuntimeName::BUNDLE_NAME, &info.bundle_name),
                self.string_property(ast, EntryRuntimeName::MODULE_NAME, &info.module_name),
                self.string_property(ast, EntryRuntimeName::PAGE_PATH, &info.page_path),
                self.string_property(ast, EntryRuntimeName::PAGE_FULL_PATH, &info.page_full_path),
                self.string_property(ast, EntryRuntimeName::INTEGRATED_HSP, &info.integrated_hsp),
                self.string_property(ast, EntryRuntimeName::MODULE_TYPE, &info.module_type),
            ]),
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
        ast: AstBuilder<'b>,
        name: &'static str,
        value: &str,
    ) -> ObjectPropertyKind<'b> {
        ast.object_property_kind_object_property(
            SPAN,
            PropertyKind::Init,
            ast.property_key_static_identifier(SPAN, name),
            ast.expression_string_literal(SPAN, ast.str(value), None),
            false,
            false,
            false,
        )
    }

    fn call_statement<'b>(
        &self,
        ast: AstBuilder<'b>,
        callee_name: &str,
        arguments: OxcVec<'b, Argument<'b>>,
    ) -> Statement<'b> {
        ast.statement_expression(
            SPAN,
            ast.expression_call(
                SPAN,
                ast.expression_identifier(SPAN, ast.str(callee_name)),
                NONE,
                arguments,
                false,
            ),
        )
    }

    fn member_call_expression<'b>(
        &self,
        ast: AstBuilder<'b>,
        object: &str,
        property: &str,
        arguments: OxcVec<'b, Argument<'b>>,
    ) -> Expression<'b> {
        let member = ast.member_expression_static(
            SPAN,
            ast.expression_identifier(SPAN, ast.str(object)),
            ast.identifier_name(SPAN, ast.str(property)),
            false,
        );
        ast.expression_call(
            SPAN,
            Self::member_expression_to_expression(member),
            NONE,
            arguments,
            false,
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
