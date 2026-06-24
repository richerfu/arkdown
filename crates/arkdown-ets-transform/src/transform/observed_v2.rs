//! `@ObservedV2` class lowering.
//!
//! Lowers `@ObservedV2` class declarations by:
//! 1. Stripping the `@ObservedV2` class decorator.
//! 2. Replacing `@Trace` field decorators with `@Track` (the runtime equivalent).
//! 3. Stripping `@Monitor`, `@SyncMonitor`, and `@Computed` decorators from
//!    methods/getters (runtime wires them without further decorator syntax).
//!
//! # Upstream correspondence
//!
//! Corresponds to SDK `process_observed_v2.js` in `ets-loader`:
//! - `@ObservedV2` class decorator → stripped
//! - `@Trace` field decorator → replaced by `@Track` (same runtime role, different name)
//! - `@Monitor` / `@SyncMonitor` / `@Computed` → stripped from method definitions
//!
//! # Implementation note
//!
//! The transform uses OXC's `AstBuilder` alloc methods exclusively so that
//! `node_id` assignments are handled correctly — no raw struct literals.

use oxc_allocator::{Allocator, CloneIn, Vec as OxcVec};
use oxc_ast::ast::{ClassElement, ClassType, Decorator, Statement, TSClassImplements};
use oxc_ast::{AstBuilder, NONE};
use oxc_span::{Span, SPAN};

use crate::transform::decorator::{ArkuiDecorator, ArkuiDecoratorName};

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Lower all `@ObservedV2` class declarations in a statement list.
///
/// # Upstream correspondence
///
/// Mirrors `process_observed_v2.js::processObservedV2Class` in `ets-loader`.
pub fn lower_observed_v2_classes<'a>(
    stmts: &[Statement<'a>],
    allocator: &'a Allocator,
) -> OxcVec<'a, Statement<'a>> {
    let ast = AstBuilder::new(allocator);
    let mut out = ast.vec();
    for stmt in stmts {
        out.push(maybe_lower(stmt, allocator, &ast));
    }
    out
}

// ---------------------------------------------------------------------------
// Statement-level dispatch
// ---------------------------------------------------------------------------

fn maybe_lower<'a>(
    stmt: &Statement<'a>,
    allocator: &'a Allocator,
    ast: &AstBuilder<'a>,
) -> Statement<'a> {
    match stmt {
        Statement::ClassDeclaration(class) => {
            if !class
                .decorators
                .iter()
                .any(|d| ArkuiDecorator::new(d).name() == ArkuiDecoratorName::OBSERVED_V2)
            {
                return stmt.clone_in(allocator);
            }
            lower_class(class.span, class, allocator, ast, false)
        }
        Statement::ExportNamedDeclaration(export) => {
            // Propagate into inline `export class Foo { ... }`.
            if let Some(decl) = &export.declaration {
                if let oxc_ast::ast::Declaration::ClassDeclaration(class) = decl {
                    if class
                        .decorators
                        .iter()
                        .any(|d| ArkuiDecorator::new(d).name() == ArkuiDecoratorName::OBSERVED_V2)
                    {
                        let lowered_stmt = lower_class(class.span, class, allocator, ast, true);
                        return lowered_stmt;
                    }
                }
            }
            stmt.clone_in(allocator)
        }
        _ => stmt.clone_in(allocator),
    }
}

// ---------------------------------------------------------------------------
// Class lowering
// ---------------------------------------------------------------------------

fn lower_class<'a>(
    span: Span,
    class: &oxc_ast::ast::Class<'a>,
    allocator: &'a Allocator,
    ast: &AstBuilder<'a>,
    is_exported: bool,
) -> Statement<'a> {
    // 1. Strip @ObservedV2 from class decorators.
    let new_decorators: OxcVec<'a, Decorator<'a>> = class
        .decorators
        .iter()
        .filter(|d| ArkuiDecorator::new(d).name() != ArkuiDecoratorName::OBSERVED_V2)
        .map(|d| d.clone_in(allocator))
        .fold(ast.vec(), |mut v, d| {
            v.push(d);
            v
        });

    // 2. Lower each class element.
    let new_elements: OxcVec<'a, ClassElement<'a>> = class
        .body
        .body
        .iter()
        .map(|elem| lower_element(elem, allocator, ast))
        .fold(ast.vec(), |mut v, e| {
            v.push(e);
            v
        });

    let body = ast.alloc_class_body(class.body.span, new_elements);
    let super_class = class.super_class.as_ref().map(|sc| sc.clone_in(allocator));
    let id = class
        .id
        .as_ref()
        .map(|i| ast.binding_identifier(i.span, i.name.as_str()));

    let new_class = ast.alloc_class(
        span,
        ClassType::ClassDeclaration,
        new_decorators,
        id,
        NONE,
        super_class,
        NONE,
        ast.vec::<TSClassImplements<'a>>(),
        body,
        class.r#abstract,
        class.declare,
    );

    if is_exported {
        let decl = oxc_ast::ast::Declaration::ClassDeclaration(new_class);
        let export_node = ast.alloc_export_named_declaration(
            span,
            Some(decl),
            ast.vec(),
            None,
            oxc_ast::ast::ImportOrExportKind::Value,
            NONE,
        );
        Statement::ExportNamedDeclaration(export_node)
    } else {
        Statement::ClassDeclaration(new_class)
    }
}

// ---------------------------------------------------------------------------
// Class element lowering
// ---------------------------------------------------------------------------

/// Decorators that are stripped from method/getter definitions.
const STRIP_METHOD_DECORATORS: &[&str] = &[
    ArkuiDecoratorName::MONITOR,
    ArkuiDecoratorName::SYNC_MONITOR,
    ArkuiDecoratorName::COMPUTED,
];

fn lower_element<'a>(
    element: &ClassElement<'a>,
    allocator: &'a Allocator,
    ast: &AstBuilder<'a>,
) -> ClassElement<'a> {
    match element {
        ClassElement::PropertyDefinition(prop) => {
            let needs_update = prop.decorators.iter().any(|d| {
                matches!(
                    ArkuiDecorator::new(d).name(),
                    ArkuiDecoratorName::TRACE | ArkuiDecoratorName::TRACK
                )
            });
            if !needs_update {
                return element.clone_in(allocator);
            }

            let new_decorators: OxcVec<'a, Decorator<'a>> = prop
                .decorators
                .iter()
                .filter_map(|d| match ArkuiDecorator::new(d).name() {
                    ArkuiDecoratorName::TRACE => {
                        // Replace @Trace with @Track identifier expression.
                        let track_expr = ast.expression_identifier(
                            SPAN,
                            allocator.alloc_str(ArkuiDecoratorName::TRACK),
                        );
                        Some(ast.decorator(SPAN, track_expr))
                    }
                    ArkuiDecoratorName::TRACK => None,
                    _ => Some(d.clone_in(allocator)),
                })
                .fold(ast.vec(), |mut v, d| {
                    v.push(d);
                    v
                });

            let mut cloned = prop.clone_in(allocator);
            cloned.decorators = new_decorators;
            ClassElement::PropertyDefinition(cloned)
        }

        ClassElement::MethodDefinition(method) => {
            // Strip @Monitor, @SyncMonitor, @Computed from method decorators.
            let needs_strip = method
                .decorators
                .iter()
                .any(|d| STRIP_METHOD_DECORATORS.contains(&ArkuiDecorator::new(d).name()));
            if !needs_strip {
                return element.clone_in(allocator);
            }

            let new_decorators: OxcVec<'a, Decorator<'a>> = method
                .decorators
                .iter()
                .filter(|d| !STRIP_METHOD_DECORATORS.contains(&ArkuiDecorator::new(d).name()))
                .map(|d| d.clone_in(allocator))
                .fold(ast.vec(), |mut v, d| {
                    v.push(d);
                    v
                });

            let mut cloned = method.clone_in(allocator);
            cloned.decorators = new_decorators;
            ClassElement::MethodDefinition(cloned)
        }

        _ => element.clone_in(allocator),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use oxc_allocator::Allocator;
    use oxc_codegen::{Codegen, CodegenOptions};
    use oxc_parser::Parser;
    use oxc_span::SourceType;

    fn transform_and_emit(source: &str) -> String {
        let allocator = Allocator::default();
        let source_type = SourceType::ets().with_module(true);
        let mut parsed = Parser::new(&allocator, source, source_type).parse();
        if !parsed.diagnostics.is_empty() {
            panic!("parse errors: {:?}", parsed.diagnostics);
        }
        parsed.program.body = lower_observed_v2_classes(&parsed.program.body, &allocator);
        Codegen::new()
            .with_options(CodegenOptions::default())
            .build(&parsed.program)
            .code
    }

    #[test]
    fn observed_v2_decorator_stripped_from_class() {
        let src = "@ObservedV2\nclass Counter {\n  count: number = 0;\n}";
        let out = transform_and_emit(src);
        assert!(
            !out.contains("@ObservedV2"),
            "@ObservedV2 decorator should be stripped; got: {out}"
        );
        assert!(
            out.contains("class Counter"),
            "class name must be preserved; got: {out}"
        );
    }

    #[test]
    fn trace_decorator_replaced_by_track() {
        let src = "@ObservedV2\nclass Counter {\n  @Trace count: number = 0;\n}";
        let out = transform_and_emit(src);
        assert!(
            !out.contains("@Trace"),
            "@Trace should be stripped; got: {out}"
        );
        assert!(
            out.contains("@Track"),
            "@Track should replace @Trace; got: {out}"
        );
    }

    #[test]
    fn source_track_decorator_is_not_retained() {
        let src = "@ObservedV2\nclass Counter {\n  @Track count: number = 0;\n}";
        let out = transform_and_emit(src);
        assert!(
            !out.contains("@Track"),
            "source-level @Track must not be retained; got: {out}"
        );
    }

    #[test]
    fn monitor_decorator_stripped_from_method() {
        let src = r#"@ObservedV2
class Counter {
  @Trace count: number = 0;
  @Monitor("count")
  onCountChange(m) {}
}"#;
        let out = transform_and_emit(src);
        assert!(
            !out.contains("@Monitor"),
            "@Monitor must be stripped; got: {out}"
        );
        assert!(
            out.contains("onCountChange"),
            "method body must be preserved; got: {out}"
        );
    }

    #[test]
    fn computed_decorator_stripped_from_getter() {
        let src = "@ObservedV2\nclass Foo {\n  @Trace x: number = 1;\n  @Computed get doubled(): number { return this.x * 2; }\n}";
        let out = transform_and_emit(src);
        assert!(
            !out.contains("@Computed"),
            "@Computed must be stripped; got: {out}"
        );
        assert!(
            out.contains("doubled"),
            "getter must be preserved; got: {out}"
        );
    }

    #[test]
    fn non_observed_v2_class_unchanged() {
        // A plain class without @ObservedV2 must not be touched.
        let src = "class Foo {\n  x: number = 0;\n}";
        let out = transform_and_emit(src);
        assert!(
            out.contains("class Foo"),
            "plain class must be preserved; got: {out}"
        );
        // No @Track injection for non-observed classes.
        assert!(
            !out.contains("@Track"),
            "no @Track for plain class; got: {out}"
        );
    }
}
