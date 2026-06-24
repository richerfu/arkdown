use oxc_allocator::{Allocator, CloneIn, Vec as OxcVec};
use oxc_ast::ast::{Expression, FunctionBody, Statement};
use oxc_ast::AstBuilder;
use oxc_span::SPAN;

pub(crate) struct StylesBodyLowerer<'a> {
    body: Option<&'a FunctionBody<'a>>,
}

impl<'a> StylesBodyLowerer<'a> {
    pub(crate) fn new(body: Option<&'a FunctionBody<'a>>) -> Self {
        Self { body }
    }

    pub(crate) fn statements<'b>(&self, allocator: &'b Allocator) -> OxcVec<'b, Statement<'b>> {
        let mut statements = OxcVec::new_in(allocator);
        let Some(body) = self.body else {
            return statements;
        };

        for statement in &body.statements {
            statements.push(self.statement(statement, allocator));
        }
        statements
    }

    fn statement<'b>(&self, statement: &Statement<'_>, allocator: &'b Allocator) -> Statement<'b> {
        match statement {
            Statement::ExpressionStatement(statement) => {
                let expression = self.expression(&statement.expression, allocator);
                AstBuilder::new(allocator).statement_expression(statement.span, expression)
            }
            _ => statement.clone_in(allocator),
        }
    }

    fn expression<'b>(
        &self,
        expression: &Expression<'_>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        match expression {
            Expression::LeadingDotExpression(expression) => {
                let mut expression = expression.expression.clone_in(allocator);
                StylesLeadingDotPrefixer::new(allocator).prefix(&mut expression);
                expression
            }
            _ => expression.clone_in(allocator),
        }
    }
}

struct StylesLeadingDotPrefixer<'a> {
    allocator: &'a Allocator,
    ast: AstBuilder<'a>,
}

impl<'a> StylesLeadingDotPrefixer<'a> {
    fn new(allocator: &'a Allocator) -> Self {
        Self {
            allocator,
            ast: AstBuilder::new(allocator),
        }
    }

    fn prefix(&self, expression: &mut Expression<'a>) -> bool {
        if let Expression::CallExpression(call) = expression {
            self.prefix_callee(&mut call.callee)
        } else {
            false
        }
    }

    fn prefix_callee(&self, callee: &mut Expression<'a>) -> bool {
        match callee {
            Expression::Identifier(identifier) => {
                let property_name = self.allocator.alloc_str(identifier.name.as_str());
                let member = self.ast.member_expression_static(
                    identifier.span,
                    self.ast.expression_this(SPAN),
                    self.ast.identifier_name(identifier.span, property_name),
                    false,
                );
                *callee = Expression::from(member);
                true
            }
            Expression::StaticMemberExpression(member) => self.prefix(&mut member.object),
            Expression::CallExpression(call) => self.prefix_callee(&mut call.callee),
            _ => false,
        }
    }
}
