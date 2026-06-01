use oxc_allocator::Allocator;
use oxc_ast::ast::{Argument, CallExpression, Expression, ObjectPropertyKind, Program, Statement};
use oxc_parser::Parser;
use oxc_span::SourceType;

use crate::source_edit::{apply_source_patches, SourcePatch};

/// Replace local `require("./chunk")` calls with `globalThis["chunk_<hash>"]`.
/// External requires (e.g. node builtins) are left unchanged.
pub fn replace_require_calls(code: &str, _current_chunk_hash: &str) -> String {
    let allocator = Allocator::default();
    let parse_result = Parser::new(&allocator, code, SourceType::cjs()).parse();
    if parse_result.panicked || !parse_result.errors.is_empty() {
        return code.to_owned();
    }

    let mut collector = RequireRewriteCollector::default();
    collector.collect_program(&parse_result.program);
    apply_source_patches(code, collector.into_patches())
}

#[derive(Default)]
struct RequireRewriteCollector {
    patches: Vec<SourcePatch>,
}

impl RequireRewriteCollector {
    fn collect_program(&mut self, program: &Program<'_>) {
        for statement in &program.body {
            self.collect_statement(statement);
        }
    }

    fn into_patches(self) -> Vec<SourcePatch> {
        self.patches
    }

    fn collect_statement(&mut self, statement: &Statement<'_>) {
        match statement {
            Statement::BlockStatement(block) => {
                for statement in &block.body {
                    self.collect_statement(statement);
                }
            }
            Statement::ExpressionStatement(statement) => {
                self.collect_expression(&statement.expression);
            }
            Statement::VariableDeclaration(declaration) => {
                for declarator in &declaration.declarations {
                    if let Some(init) = &declarator.init {
                        self.collect_expression(init);
                    }
                }
            }
            Statement::ReturnStatement(statement) => {
                if let Some(argument) = &statement.argument {
                    self.collect_expression(argument);
                }
            }
            Statement::IfStatement(statement) => {
                self.collect_expression(&statement.test);
                self.collect_statement(&statement.consequent);
                if let Some(alternate) = &statement.alternate {
                    self.collect_statement(alternate);
                }
            }
            Statement::ForStatement(statement) => {
                if let Some(test) = &statement.test {
                    self.collect_expression(test);
                }
                if let Some(update) = &statement.update {
                    self.collect_expression(update);
                }
                self.collect_statement(&statement.body);
            }
            Statement::WhileStatement(statement) => {
                self.collect_expression(&statement.test);
                self.collect_statement(&statement.body);
            }
            Statement::DoWhileStatement(statement) => {
                self.collect_statement(&statement.body);
                self.collect_expression(&statement.test);
            }
            _ => {}
        }
    }

    fn collect_expression(&mut self, expression: &Expression<'_>) {
        match expression {
            Expression::CallExpression(call) => self.collect_call(call),
            Expression::NewExpression(new_expression) => {
                self.collect_expression(&new_expression.callee);
                for argument in &new_expression.arguments {
                    self.collect_argument(argument);
                }
            }
            Expression::StaticMemberExpression(member) => {
                self.collect_expression(&member.object);
            }
            Expression::ComputedMemberExpression(member) => {
                self.collect_expression(&member.object);
                self.collect_expression(&member.expression);
            }
            Expression::AssignmentExpression(assignment) => {
                self.collect_expression(&assignment.right);
            }
            Expression::LogicalExpression(logical) => {
                self.collect_expression(&logical.left);
                self.collect_expression(&logical.right);
            }
            Expression::BinaryExpression(binary) => {
                self.collect_expression(&binary.left);
                self.collect_expression(&binary.right);
            }
            Expression::UnaryExpression(unary) => {
                self.collect_expression(&unary.argument);
            }
            Expression::SequenceExpression(sequence) => {
                for expression in &sequence.expressions {
                    self.collect_expression(expression);
                }
            }
            Expression::ConditionalExpression(conditional) => {
                self.collect_expression(&conditional.test);
                self.collect_expression(&conditional.consequent);
                self.collect_expression(&conditional.alternate);
            }
            Expression::ObjectExpression(object) => {
                for property in &object.properties {
                    match property {
                        ObjectPropertyKind::ObjectProperty(property) => {
                            self.collect_expression(&property.value);
                        }
                        ObjectPropertyKind::SpreadProperty(spread) => {
                            self.collect_expression(&spread.argument);
                        }
                    }
                }
            }
            Expression::ArrayExpression(array) => {
                for element in &array.elements {
                    if let Some(expression) = element.as_expression() {
                        self.collect_expression(expression);
                    }
                }
            }
            Expression::ParenthesizedExpression(parenthesized) => {
                self.collect_expression(&parenthesized.expression);
            }
            Expression::TSAsExpression(expression) => {
                self.collect_expression(&expression.expression);
            }
            Expression::TSSatisfiesExpression(expression) => {
                self.collect_expression(&expression.expression);
            }
            Expression::TSNonNullExpression(expression) => {
                self.collect_expression(&expression.expression);
            }
            Expression::TSTypeAssertion(expression) => {
                self.collect_expression(&expression.expression);
            }
            _ => {}
        }
    }

    fn collect_call(&mut self, call: &CallExpression<'_>) {
        if let Some(patch) = require_call_patch(call) {
            self.patches.push(patch);
            return;
        }

        self.collect_expression(&call.callee);
        for argument in &call.arguments {
            self.collect_argument(argument);
        }
    }

    fn collect_argument(&mut self, argument: &Argument<'_>) {
        if let Some(expression) = argument.as_expression() {
            self.collect_expression(expression);
        }
    }
}

fn require_call_patch(call: &CallExpression<'_>) -> Option<SourcePatch> {
    if !matches!(
        &call.callee,
        Expression::Identifier(identifier) if identifier.name.as_str() == "require"
    ) {
        return None;
    }

    let Some(Argument::StringLiteral(literal)) = call.arguments.first() else {
        return None;
    };

    let chunk_id = literal.value.as_str();
    if !(chunk_id.starts_with("./") || chunk_id.starts_with("../")) {
        return None;
    }

    let chunk_name = normalize_chunk_name(chunk_id);
    Some(SourcePatch {
        start: call.span.start as usize,
        end: call.span.end as usize,
        replacement: format!("globalThis[\"{}\"]", chunk_key(chunk_name)),
    })
}

fn normalize_chunk_name(chunk_id: &str) -> &str {
    chunk_id
        .trim_start_matches("./")
        .trim_start_matches("../")
        .trim_end_matches(".js")
}

fn chunk_key(name: &str) -> String {
    format!("{}_{:x}", name, chunk_name_hash(name))
}

fn chunk_name_hash(name: &str) -> u32 {
    let mut h: u32 = 0x811c9dc5;
    for b in name.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x01000193);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_require_unchanged() {
        let code = "var x = 1;";
        assert_eq!(replace_require_calls(code, ""), code);
    }

    #[test]
    fn test_external_require_unchanged() {
        let code = r#"var fs = require("fs");"#;
        let result = replace_require_calls(code, "");
        assert!(result.contains(r#"require("fs")"#));
    }

    #[test]
    fn test_local_require_double_quote() {
        let code = r#"var _h = require("./helper");"#;
        let result = replace_require_calls(code, "");
        assert!(result.contains("globalThis[\"helper_"));
        assert!(!result.contains("require("));
    }

    #[test]
    fn test_local_require_single_quote() {
        let code = "var _h = require('./helper');";
        let result = replace_require_calls(code, "");
        assert!(result.contains("globalThis[\"helper_"));
        assert!(!result.contains("require("));
    }

    #[test]
    fn test_local_require_with_js_extension() {
        let code = r#"var _h = require("./helper.js");"#;
        let result = replace_require_calls(code, "");
        // .js suffix is stripped from the key name
        assert!(result.contains("globalThis[\"helper_"));
    }

    #[test]
    fn test_multiline_code() {
        let code = "var a = 1;\nvar _h = require(\"./shared\");\nvar b = 2;";
        let result = replace_require_calls(code, "");
        assert!(result.contains("var a = 1;"));
        assert!(result.contains("globalThis[\"shared_"));
        assert!(result.contains("var b = 2;"));
    }

    #[test]
    fn rewrites_nested_local_require_call() {
        let code = r#"var value = load(require("./shared"));"#;
        let result = replace_require_calls(code, "");
        assert!(result.contains("load(globalThis[\"shared_"));
        assert!(!result.contains("require("));
    }
}
