use oxc_ast::ast::{CallExpression, Expression, Function};
use oxc_ast_visit::{walk, Visit};
use oxc_span::{GetSpan, Span};
use oxc_syntax::scope::ScopeFlags;

use crate::{apply_source_patches, SourcePatch};

/// Rewrites global ArkUI runtime calls in a source expression using OXC AST spans.
pub(crate) fn rewrite_global_context_calls_in_expression(
    expression: &Expression<'_>,
    source_text: &str,
) -> String {
    let span = expression.span();
    let mut visitor = GlobalContextCallVisitor {
        patches: Vec::new(),
    };
    visitor.visit_expression(expression);
    apply_patches_in_span(source_text, span, visitor.patches)
}

/// Rewrites global ArkUI runtime calls in a class/function member text.
pub(crate) fn rewrite_global_context_calls_in_function_text(
    function: &Function<'_>,
    source_text: &str,
    text_start: usize,
    text: &str,
) -> String {
    let span = Span::new(text_start as u32, (text_start + text.len()) as u32);
    let mut visitor = GlobalContextCallVisitor {
        patches: Vec::new(),
    };
    visitor.visit_function(function, ScopeFlags::Function);
    apply_patches_in_span(source_text, span, visitor.patches)
}

struct GlobalContextCallVisitor {
    patches: Vec<SourcePatch>,
}

impl<'a> Visit<'a> for GlobalContextCallVisitor {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if let Expression::Identifier(callee) = &call.callee {
            if callee.name == "animateTo" {
                self.patches.push(SourcePatch {
                    start: callee.span.start as usize,
                    end: callee.span.end as usize,
                    replacement: "Context.animateTo".to_string(),
                });
            }
        }
        walk::walk_call_expression(self, call);
    }
}

fn apply_patches_in_span(source_text: &str, span: Span, patches: Vec<SourcePatch>) -> String {
    let start = span.start as usize;
    let end = span.end as usize;
    let local_patches = patches
        .into_iter()
        .filter(|patch| patch.start >= start && patch.end <= end)
        .map(|patch| SourcePatch {
            start: patch.start - start,
            end: patch.end - start,
            replacement: patch.replacement,
        })
        .collect();
    apply_source_patches(&source_text[start..end], local_patches)
}
