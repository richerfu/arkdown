use oxc_allocator::Allocator;
use oxc_ast::ast::{ClassElement, Decorator, Expression, PropertyKey, Statement};
use oxc_parser::{Parser, ParserReturn};
use oxc_span::SourceType;

use crate::error::{DiagnosticSeverity, EtsTransformError};

/// Extract the simple name string from a decorator expression.
/// Handles both `@Foo` (identifier) and `@Foo(...)` (call expression) forms.
fn decorator_name<'a>(d: &'a Decorator<'a>) -> &'a str {
    match &d.expression {
        Expression::Identifier(id) => id.name.as_str(),
        Expression::CallExpression(call) => {
            if let Expression::Identifier(id) = &call.callee {
                id.name.as_str()
            } else {
                ""
            }
        }
        _ => "",
    }
}

/// Run all etsChecker validation rules against the given source text.
///
/// Returns a list of diagnostics (errors and warnings). The list may be empty.
/// This function never panics — parse errors are silently skipped so that
/// downstream transforms can still attempt to proceed.
pub fn check(source: &str, file: &str) -> Vec<EtsTransformError> {
    let allocator = Allocator::default();
    let source_type = SourceType::default().with_typescript(true).with_module(true);
    let ParserReturn { program, .. } =
        Parser::new(&allocator, source, source_type).parse();

    let mut errors: Vec<EtsTransformError> = Vec::new();
    let mut seen_component_names: std::collections::HashSet<String> = Default::default();

    for stmt in &program.body {
        if let Statement::ClassDeclaration(class) = stmt {
            let has_component =
                class.decorators.iter().any(|d| decorator_name(d) == "Component");
            let has_entry =
                class.decorators.iter().any(|d| decorator_name(d) == "Entry");

            // Rule: @Entry without @Component
            if has_entry && !has_component {
                errors.push(EtsTransformError::Diagnostic {
                    file: file.to_string(),
                    message: "@Entry applied without @Component".to_string(),
                    severity: DiagnosticSeverity::Error,
                });
            }

            if has_component {
                // Rule: duplicate @Component name in one file
                let class_name = class
                    .id
                    .as_ref()
                    .map(|id| id.name.as_str().to_string())
                    .unwrap_or_default();

                if !class_name.is_empty() {
                    if seen_component_names.contains(&class_name) {
                        errors.push(EtsTransformError::Diagnostic {
                            file: file.to_string(),
                            message: format!(
                                "Duplicate @Component name '{}' in file",
                                class_name
                            ),
                            severity: DiagnosticSeverity::Error,
                        });
                    } else {
                        seen_component_names.insert(class_name);
                    }
                }

                // Rule: missing build() method
                let has_build = class.body.body.iter().any(|m| {
                    if let ClassElement::MethodDefinition(method) = m {
                        match &method.key {
                            PropertyKey::StaticIdentifier(id) => {
                                id.name.as_str() == "build"
                            }
                            _ => false,
                        }
                    } else {
                        false
                    }
                });

                if !has_build {
                    let name = class
                        .id
                        .as_ref()
                        .map(|id| id.name.as_str())
                        .unwrap_or("<anonymous>");
                    errors.push(EtsTransformError::Diagnostic {
                        file: file.to_string(),
                        message: format!(
                            "@Component struct '{}' is missing a build() method",
                            name
                        ),
                        severity: DiagnosticSeverity::Error,
                    });
                }

                // Rule: build() must not have a non-void return type
                for elem in &class.body.body {
                    if let ClassElement::MethodDefinition(method) = elem {
                        let is_build = matches!(&method.key,
                            PropertyKey::StaticIdentifier(id) if id.name.as_str() == "build");
                        if is_build {
                            if let Some(ret_ty) = method
                                .value
                                .return_type
                                .as_ref()
                            {
                                // Only void is allowed — check via span text is simpler
                                // than deep-inspecting the TSTypeAnnotation tree.
                                // We flag any explicit non-void annotation as an error.
                                let ty_span = ret_ty.span;
                                if ty_span.size() > 0 {
                                    // Accept "void" silently; anything else is an error.
                                    let annotation_text = &source[ty_span.start as usize..ty_span.end as usize];
                                    if !annotation_text.contains("void") {
                                        errors.push(EtsTransformError::Diagnostic {
                                            file: file.to_string(),
                                            message: "build() must have a void return type"
                                                .to_string(),
                                            severity: DiagnosticSeverity::Error,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }

                // Rule: worker file must not use UI decorators
                if file.contains("workers/") || file.contains("/worker/") {
                    errors.push(EtsTransformError::Diagnostic {
                        file: file.to_string(),
                        message:
                            "@Component/@Entry must not be used in worker files".to_string(),
                        severity: DiagnosticSeverity::Error,
                    });
                }

                // Rule: @BuilderParam fields without type annotations (warning)
                for elem in &class.body.body {
                    if let ClassElement::PropertyDefinition(prop) = elem {
                        let is_builder_param = prop
                            .decorators
                            .iter()
                            .any(|d| decorator_name(d) == "BuilderParam");
                        if is_builder_param && prop.type_annotation.is_none() {
                            let field_name = match &prop.key {
                                PropertyKey::StaticIdentifier(id) => id.name.as_str().to_string(),
                                _ => "<unknown>".to_string(),
                            };
                            errors.push(EtsTransformError::Diagnostic {
                                file: file.to_string(),
                                message: format!(
                                    "@BuilderParam field '{}' has no type annotation",
                                    field_name
                                ),
                                severity: DiagnosticSeverity::Warning,
                            });
                        }
                    }
                }

                // Rule: @State/@Prop/@Link on a struct without @Component already guarded
                // by the outer has_component check — but check individual fields for
                // @State on non-component classes would be a second pass.
                // Here we also check for unsupported @State types (warning).
                for elem in &class.body.body {
                    if let ClassElement::PropertyDefinition(prop) = elem {
                        let is_state = prop
                            .decorators
                            .iter()
                            .any(|d| decorator_name(d) == "State");
                        if is_state && prop.type_annotation.is_none() {
                            // No type annotation on @State — allowed but may produce
                            // unexpected observable behavior; emit a warning.
                            let field_name = match &prop.key {
                                PropertyKey::StaticIdentifier(id) => id.name.as_str().to_string(),
                                _ => "<unknown>".to_string(),
                            };
                            errors.push(EtsTransformError::Diagnostic {
                                file: file.to_string(),
                                message: format!(
                                    "@State field '{}' has no type annotation (may be unsupported observable type)",
                                    field_name
                                ),
                                severity: DiagnosticSeverity::Warning,
                            });
                        }
                    }
                }
            }

            // Rule: @State/@Prop/@Link on non-@Component struct
            if !has_component {
                for elem in &class.body.body {
                    if let ClassElement::PropertyDefinition(prop) = elem {
                        let has_state_dec = prop.decorators.iter().any(|d| {
                            matches!(
                                decorator_name(d),
                                "State" | "Prop" | "Link"
                            )
                        });
                        if has_state_dec {
                            let field_name = match &prop.key {
                                PropertyKey::StaticIdentifier(id) => id.name.as_str().to_string(),
                                _ => "<unknown>".to_string(),
                            };
                            errors.push(EtsTransformError::Diagnostic {
                                file: file.to_string(),
                                message: format!(
                                    "State decorator on '{}' in non-@Component struct",
                                    field_name
                                ),
                                severity: DiagnosticSeverity::Error,
                            });
                        }
                    }
                }
            }
        }
    }

    errors
}
