use oxc_allocator::Allocator;
use oxc_ast::ast::{
    ArrayExpressionElement, Declaration, ExportDefaultDeclarationKind, Expression,
    ImportOrExportKind, MemberExpression, NumberBase, ObjectPropertyKind, Program, PropertyKey,
    PropertyKind, Statement, VariableDeclarationKind,
};
use oxc_ast::{AstBuilder, NONE};
use oxc_codegen::Codegen;
use oxc_span::{SourceType, SPAN};
use oxc_syntax::{identifier::is_identifier_name, keyword::is_reserved_keyword};

const DEFAULT_BINDING: &str = "__json_default";

struct JsonExportName<'a> {
    name: &'a str,
}

impl<'a> JsonExportName<'a> {
    fn new(name: &'a str) -> Self {
        Self { name }
    }

    fn is_binding_identifier(&self) -> bool {
        is_identifier_name(self.name) && !is_reserved_keyword(self.name)
    }
}

pub(crate) struct JsonModuleEmitter<'a> {
    value: &'a serde_json::Value,
}

impl<'a> JsonModuleEmitter<'a> {
    pub(crate) fn new(value: &'a serde_json::Value) -> Self {
        Self { value }
    }

    /// Generate a synthetic ES module from a JSON value.
    ///
    /// Top-level object keys that are valid JS identifiers become named
    /// exports. The whole value is always the default export.
    pub(crate) fn emit(&self) -> String {
        let allocator = Allocator::default();
        let ast = AstBuilder::new(&allocator);
        let program = self.program(ast);
        Codegen::new().build(&program).code
    }

    fn program<'b>(&self, ast: AstBuilder<'b>) -> Program<'b> {
        let statements = match self.value {
            serde_json::Value::Object(map) => {
                let mut statements = ast.vec();
                statements.push(self.default_binding_statement(ast));
                for key in map.keys().filter(|key| Self::is_valid_js_identifier(key)) {
                    statements.push(self.named_export_statement(ast, key));
                }
                statements.push(self.default_export_statement(ast));
                statements
            }
            value => ast.vec1(self.inline_default_export_statement(ast, value)),
        };

        ast.program(
            SPAN,
            SourceType::default().with_module(true),
            "",
            ast.vec(),
            None,
            ast.vec(),
            statements,
        )
    }

    fn default_binding_statement<'b>(&self, ast: AstBuilder<'b>) -> Statement<'b> {
        Statement::from(ast.declaration_variable(
            SPAN,
            VariableDeclarationKind::Var,
            ast.vec1(ast.variable_declarator(
                SPAN,
                VariableDeclarationKind::Var,
                ast.binding_pattern_binding_identifier(SPAN, DEFAULT_BINDING),
                NONE,
                Some(Self::json_value_expression(self.value, ast)),
                false,
            )),
            false,
        ))
    }

    fn named_export_statement<'b>(&self, ast: AstBuilder<'b>, key: &str) -> Statement<'b> {
        Statement::from(ast.module_declaration_export_named_declaration(
            SPAN,
            Some(Declaration::VariableDeclaration(
                ast.alloc_variable_declaration(
                    SPAN,
                    VariableDeclarationKind::Var,
                    ast.vec1(ast.variable_declarator(
                        SPAN,
                        VariableDeclarationKind::Var,
                        ast.binding_pattern_binding_identifier(SPAN, ast.allocator.alloc_str(key)),
                        NONE,
                        Some(self.default_member_expression(ast, key)),
                        false,
                    )),
                    false,
                ),
            )),
            ast.vec(),
            None,
            ImportOrExportKind::Value,
            NONE,
        ))
    }

    fn default_member_expression<'b>(&self, ast: AstBuilder<'b>, key: &str) -> Expression<'b> {
        Self::member_expression_to_expression(ast.member_expression_computed(
            SPAN,
            ast.expression_identifier(SPAN, DEFAULT_BINDING),
            ast.expression_string_literal(SPAN, ast.str(key), None),
            false,
        ))
    }

    fn default_export_statement<'b>(&self, ast: AstBuilder<'b>) -> Statement<'b> {
        self.export_default_expression_statement(
            ast,
            ast.expression_identifier(SPAN, DEFAULT_BINDING),
        )
    }

    fn inline_default_export_statement<'b>(
        &self,
        ast: AstBuilder<'b>,
        value: &serde_json::Value,
    ) -> Statement<'b> {
        self.export_default_expression_statement(ast, Self::json_value_expression(value, ast))
    }

    fn export_default_expression_statement<'b>(
        &self,
        ast: AstBuilder<'b>,
        expression: Expression<'b>,
    ) -> Statement<'b> {
        Statement::from(ast.module_declaration_export_default_declaration(
            SPAN,
            ExportDefaultDeclarationKind::from(expression),
        ))
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

    fn json_value_expression<'b>(value: &serde_json::Value, ast: AstBuilder<'b>) -> Expression<'b> {
        match value {
            serde_json::Value::Null => ast.expression_null_literal(SPAN),
            serde_json::Value::Bool(value) => ast.expression_boolean_literal(SPAN, *value),
            serde_json::Value::Number(value) => {
                let raw = value.to_string();
                ast.expression_numeric_literal(
                    SPAN,
                    value
                        .as_f64()
                        .expect("JSON numbers are representable as f64"),
                    Some(ast.str(&raw)),
                    NumberBase::Decimal,
                )
            }
            serde_json::Value::String(value) => {
                ast.expression_string_literal(SPAN, ast.str(value), None)
            }
            serde_json::Value::Array(values) => ast.expression_array(
                SPAN,
                ast.vec_from_iter(values.iter().map(|value| {
                    ArrayExpressionElement::from(Self::json_value_expression(value, ast))
                })),
            ),
            serde_json::Value::Object(values) => ast.expression_object(
                SPAN,
                ast.vec_from_iter(values.iter().map(|(key, value)| {
                    let property_key =
                        PropertyKey::from(ast.expression_string_literal(SPAN, ast.str(key), None));
                    let property_value = Self::json_value_expression(value, ast);
                    ObjectPropertyKind::from(ast.object_property_kind_object_property(
                        SPAN,
                        PropertyKind::Init,
                        property_key,
                        property_value,
                        false,
                        false,
                        false,
                    ))
                })),
            ),
        }
    }

    pub(crate) fn is_valid_js_identifier(s: &str) -> bool {
        JsonExportName::new(s).is_binding_identifier()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxc_allocator::Allocator;
    use oxc_ast::ast::{Declaration, Statement};
    use oxc_parser::Parser;
    use oxc_span::SourceType;

    struct JsonModuleAssertion<'a> {
        code: &'a str,
    }

    impl<'a> JsonModuleAssertion<'a> {
        fn new(code: &'a str) -> Self {
            Self { code }
        }

        fn has_named_var_export(&self, expected: &str) -> bool {
            let allocator = Allocator::default();
            let parsed =
                Parser::new(&allocator, self.code, SourceType::ts().with_module(true)).parse();
            parsed.program.body.iter().any(|statement| {
                let Statement::ExportNamedDeclaration(export) = statement else {
                    return false;
                };
                let Some(Declaration::VariableDeclaration(declaration)) = &export.declaration
                else {
                    return false;
                };
                declaration.declarations.iter().any(|declarator| {
                    declarator.id.get_identifier_name().as_deref() == Some(expected)
                })
            })
        }

        fn has_default_export(&self) -> bool {
            let allocator = Allocator::default();
            let parsed =
                Parser::new(&allocator, self.code, SourceType::ts().with_module(true)).parse();
            parsed
                .program
                .body
                .iter()
                .any(|statement| matches!(statement, Statement::ExportDefaultDeclaration(_)))
        }
    }

    #[test]
    fn generate_esm_object() {
        let val: serde_json::Value = serde_json::json!({"name": "foo", "version": 2});
        let code = JsonModuleEmitter::new(&val).emit();
        let module = JsonModuleAssertion::new(&code);
        assert!(module.has_named_var_export("name"));
        assert!(module.has_named_var_export("version"));
        assert!(module.has_default_export());
    }

    #[test]
    fn generate_esm_array() {
        let val: serde_json::Value = serde_json::json!([1, 2, 3]);
        let code = JsonModuleEmitter::new(&val).emit();
        let module = JsonModuleAssertion::new(&code);
        assert!(module.has_default_export());
        assert!(!module.has_named_var_export("0"));
    }

    #[test]
    fn invalid_identifier_skipped() {
        let val: serde_json::Value = serde_json::json!({"123invalid": true, "valid_key": false});
        let code = JsonModuleEmitter::new(&val).emit();
        let module = JsonModuleAssertion::new(&code);
        assert!(module.has_named_var_export("valid_key"));
        assert!(!module.has_named_var_export("123invalid"));
    }
}
