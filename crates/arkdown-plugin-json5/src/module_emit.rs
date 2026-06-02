use oxc_allocator::Allocator;
use oxc_ast::ast::{
    ArrayExpressionElement, Declaration, ExportDefaultDeclarationKind, Expression,
    ImportOrExportKind, MemberExpression, NumberBase, ObjectPropertyKind, Program, PropertyKey,
    PropertyKind, Statement, VariableDeclarationKind,
};
use oxc_ast::{AstBuilder, NONE};
use oxc_codegen::Codegen;
use oxc_span::{SourceType, SPAN};

const DEFAULT_BINDING: &str = "__json_default";

/// Generate a synthetic ES module from a JSON value.
/// Top-level object keys that are valid JS identifiers become named exports.
/// The whole value is always the default export.
pub fn generate_esm_from_json(value: &serde_json::Value) -> String {
    JsonModuleEmitter::emit_for(value)
}

pub(crate) struct JsonModuleEmitter<'a> {
    value: &'a serde_json::Value,
}

impl<'a> JsonModuleEmitter<'a> {
    fn new(value: &'a serde_json::Value) -> Self {
        Self { value }
    }

    pub fn emit_for(value: &'a serde_json::Value) -> String {
        Self::new(value).emit()
    }

    fn emit(&self) -> String {
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
            ast.vec(),
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
        self.inline_default_export_statement(
            ast,
            &serde_json::Value::String(DEFAULT_BINDING.to_owned()),
        )
    }

    fn inline_default_export_statement<'b>(
        &self,
        ast: AstBuilder<'b>,
        value: &serde_json::Value,
    ) -> Statement<'b> {
        let expression = if matches!(value, serde_json::Value::String(value) if value == DEFAULT_BINDING)
        {
            ast.expression_identifier(SPAN, DEFAULT_BINDING)
        } else {
            Self::json_value_expression(value, ast)
        };
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
        if s.is_empty() {
            return false;
        }
        let mut chars = s.chars();
        let first = chars.next().unwrap();
        if !first.is_alphabetic() && first != '_' && first != '$' {
            return false;
        }
        !matches!(
            s,
            "import"
                | "export"
                | "default"
                | "class"
                | "function"
                | "return"
                | "var"
                | "let"
                | "const"
                | "if"
                | "else"
                | "for"
                | "while"
                | "do"
                | "switch"
                | "case"
                | "break"
                | "continue"
                | "new"
                | "delete"
                | "typeof"
                | "void"
                | "in"
                | "of"
                | "this"
                | "super"
                | "null"
                | "undefined"
                | "true"
                | "false"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_esm_object() {
        let val: serde_json::Value = serde_json::json!({"name": "foo", "version": 2});
        let code = generate_esm_from_json(&val);
        assert!(code.contains("export var name"));
        assert!(code.contains("export var version"));
        assert!(code.contains("export default"));
    }

    #[test]
    fn generate_esm_array() {
        let val: serde_json::Value = serde_json::json!([1, 2, 3]);
        let code = generate_esm_from_json(&val);
        assert!(code.contains("export default"));
        assert!(!code.contains("export var"));
    }

    #[test]
    fn invalid_identifier_skipped() {
        let val: serde_json::Value = serde_json::json!({"123invalid": true, "valid_key": false});
        let code = generate_esm_from_json(&val);
        assert!(code.contains("export var valid_key"));
        assert!(!code.contains("export var 123invalid"));
    }
}
