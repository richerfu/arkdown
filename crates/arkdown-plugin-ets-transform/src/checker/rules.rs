use oxc_allocator::Allocator;
use oxc_ast::ast::{
    ClassElement, Declaration, Decorator, ExportDefaultDeclarationKind, Expression, PropertyKey,
    Statement, StructElement, StructStatement, TSType,
};
use oxc_parser::{Parser, ParserReturn};
use oxc_span::SourceType;

use crate::error::{DiagnosticSeverity, EtsTransformError};
use rustc_hash::FxHashSet;

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
    let source_type = SourceType::from_path(file).unwrap_or_else(|_| SourceType::ets());
    let ParserReturn { program, .. } = Parser::new(&allocator, source, source_type).parse();

    let mut errors: Vec<EtsTransformError> = Vec::new();
    let mut seen_component_names: FxHashSet<String> = Default::default();
    let mut entry_count = 0usize;

    for stmt in &program.body {
        match stmt {
            Statement::StructStatement(statement) => {
                check_struct_component(statement, file, &mut seen_component_names, &mut errors);
                // Count @Entry decorators — at most one per file.
                if statement
                    .decorators
                    .iter()
                    .any(|d| decorator_name(d) == "Entry")
                {
                    entry_count += 1;
                }
            }
            Statement::ExportNamedDeclaration(export) => {
                if let Some(Declaration::StructStatement(statement)) = export.declaration.as_ref() {
                    check_struct_component(statement, file, &mut seen_component_names, &mut errors);
                }
            }
            Statement::ExportDefaultDeclaration(export) => {
                if let ExportDefaultDeclarationKind::StructStatement(statement) =
                    &export.declaration
                {
                    check_struct_component(statement, file, &mut seen_component_names, &mut errors);
                }
            }
            _ => {}
        }

        // Rule: @Builder functions must have void return type.
        // Rule: @Styles functions must have void return type.
        // (Top-level @Builder/@Styles function declarations)
        let func_decl = match stmt {
            Statement::FunctionDeclaration(f) => Some(f.as_ref()),
            Statement::ExportNamedDeclaration(e) => {
                if let Some(Declaration::FunctionDeclaration(f)) = e.declaration.as_ref() {
                    Some(f.as_ref())
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(func) = func_decl {
            let is_builder = func
                .decorators
                .iter()
                .any(|d| decorator_name(d) == "Builder");
            let is_styles = func
                .decorators
                .iter()
                .any(|d| decorator_name(d) == "Styles");
            if is_builder || is_styles {
                if let Some(ret_ty) = func.return_type.as_ref() {
                    if !is_void_type(&ret_ty.type_annotation) {
                        let decorator = if is_builder { "@Builder" } else { "@Styles" };
                        let name = func
                            .id
                            .as_ref()
                            .map(|id| id.name.as_str())
                            .unwrap_or("<anonymous>");
                        errors.push(EtsTransformError::Diagnostic {
                            file: file.to_string(),
                            message: format!(
                                "{} function '{}' must have void return type",
                                decorator, name
                            ),
                            severity: DiagnosticSeverity::Error,
                        });
                    }
                }
            }
        }

        if let Statement::ClassDeclaration(class) = stmt {
            let has_component = class
                .decorators
                .iter()
                .any(|d| decorator_name(d) == "Component");
            let has_entry = class
                .decorators
                .iter()
                .any(|d| decorator_name(d) == "Entry");

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
                            message: format!("Duplicate @Component name '{}' in file", class_name),
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
                            PropertyKey::StaticIdentifier(id) => id.name.as_str() == "build",
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
                            if let Some(ret_ty) = method.value.return_type.as_ref() {
                                if !is_void_type(&ret_ty.type_annotation) {
                                    errors.push(EtsTransformError::Diagnostic {
                                        file: file.to_string(),
                                        message: "build() must have a void return type".to_string(),
                                        severity: DiagnosticSeverity::Error,
                                    });
                                }
                            }
                        }
                    }
                }

                // Rule: worker file must not use UI decorators
                if file.contains("workers/") || file.contains("/worker/") {
                    errors.push(EtsTransformError::Diagnostic {
                        file: file.to_string(),
                        message: "@Component/@Entry must not be used in worker files".to_string(),
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
                        let is_state = prop.decorators.iter().any(|d| decorator_name(d) == "State");
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
                        let has_state_dec = prop
                            .decorators
                            .iter()
                            .any(|d| matches!(decorator_name(d), "State" | "Prop" | "Link"));
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

    // Rule: at most one @Entry struct per file.
    if entry_count > 1 {
        errors.push(EtsTransformError::Diagnostic {
            file: file.to_string(),
            message: format!(
                "At most one '@Entry' decorated struct is allowed per file, but {} were found",
                entry_count
            ),
            severity: DiagnosticSeverity::Error,
        });
    }

    errors
}

fn check_struct_component(
    statement: &StructStatement<'_>,
    file: &str,
    seen_component_names: &mut FxHashSet<String>,
    errors: &mut Vec<EtsTransformError>,
) {
    let component_name = statement.id.name.as_str();
    let decorator_names: Vec<&str> = statement.decorators.iter().map(decorator_name).collect();
    let has_component_v1 = decorator_names.iter().any(|name| *name == "Component");
    let has_component_v2 = decorator_names.iter().any(|name| *name == "ComponentV2");
    let has_custom_dialog = decorator_names.iter().any(|name| *name == "CustomDialog");
    let has_reusable = decorator_names.iter().any(|name| *name == "Reusable");
    let has_reusable_v2 = decorator_names.iter().any(|name| *name == "ReusableV2");
    let has_entry = decorator_names.iter().any(|name| *name == "Entry");
    let has_component = has_component_v1 || has_component_v2 || has_custom_dialog;

    if !has_component {
        errors.push(EtsTransformError::Diagnostic {
            file: file.to_string(),
            message: format!(
                "Decorator '@Component', '@ComponentV2', or '@CustomDialog' is missing for struct '{}'",
                component_name
            ),
            severity: DiagnosticSeverity::Error,
        });
    }

    if has_component_v2 && (has_component_v1 || has_reusable || has_custom_dialog) {
        errors.push(EtsTransformError::Diagnostic {
            file: file.to_string(),
            message: format!(
                "The struct '{}' can not be decorated with '@ComponentV2' and '@Component', '@Reusable', '@CustomDialog' at the same time",
                component_name
            ),
            severity: DiagnosticSeverity::Error,
        });
    }

    if has_reusable_v2 && !has_component_v2 {
        errors.push(EtsTransformError::Diagnostic {
            file: file.to_string(),
            message:
                "'@ReusableV2' is only applicable to custom components decorated by '@ComponentV2'"
                    .to_string(),
            severity: DiagnosticSeverity::Error,
        });
    }

    if has_reusable && has_reusable_v2 {
        errors.push(EtsTransformError::Diagnostic {
            file: file.to_string(),
            message:
                "The '@Reusable' and '@ReusableV2' decorators cannot be applied simultaneously"
                    .to_string(),
            severity: DiagnosticSeverity::Error,
        });
    }

    if has_entry && !has_component {
        errors.push(EtsTransformError::Diagnostic {
            file: file.to_string(),
            message: "@Entry applied without @Component/@ComponentV2/@CustomDialog".to_string(),
            severity: DiagnosticSeverity::Error,
        });
    }

    if has_component {
        if !seen_component_names.insert(component_name.to_string()) {
            errors.push(EtsTransformError::Diagnostic {
                file: file.to_string(),
                message: format!("Duplicate @Component name '{}' in file", component_name),
                severity: DiagnosticSeverity::Error,
            });
        }

        let has_build = statement.body.body.iter().any(|element| {
            matches!(
                element,
                StructElement::MethodDefinition(method)
                    if matches!(&method.key, PropertyKey::StaticIdentifier(id) if id.name.as_str() == "build")
            )
        });
        if !has_build {
            errors.push(EtsTransformError::Diagnostic {
                file: file.to_string(),
                message: format!(
                    "@Component struct '{}' is missing a build() method",
                    component_name
                ),
                severity: DiagnosticSeverity::Error,
            });
        }

        if file.contains("workers/") || file.contains("/worker/") {
            errors.push(EtsTransformError::Diagnostic {
                file: file.to_string(),
                message: "@Component/@Entry must not be used in worker files".to_string(),
                severity: DiagnosticSeverity::Error,
            });
        }
    }

    for element in &statement.body.body {
        let StructElement::PropertyDefinition(prop) = element else {
            continue;
        };
        let field_name = match &prop.key {
            PropertyKey::StaticIdentifier(id) => id.name.as_str().to_string(),
            _ => "<unknown>".to_string(),
        };
        let has_state_dec = prop.decorators.iter().any(|d| {
            matches!(
                decorator_name(d),
                "State"
                    | "Prop"
                    | "Link"
                    | "ObjectLink"
                    | "Provide"
                    | "Consume"
                    | "StorageLink"
                    | "StorageProp"
                    | "LocalStorageLink"
                    | "LocalStorageProp"
            )
        });
        if has_state_dec && !has_component {
            errors.push(EtsTransformError::Diagnostic {
                file: file.to_string(),
                message: format!(
                    "State decorator on '{}' in non-@Component struct",
                    field_name
                ),
                severity: DiagnosticSeverity::Error,
            });
        }

        let is_builder_param = prop
            .decorators
            .iter()
            .any(|d| decorator_name(d) == "BuilderParam");
        if is_builder_param && prop.type_annotation.is_none() {
            errors.push(EtsTransformError::Diagnostic {
                file: file.to_string(),
                message: format!(
                    "@BuilderParam field '{}' has no type annotation",
                    field_name
                ),
                severity: DiagnosticSeverity::Warning,
            });
        }

        let is_state = prop.decorators.iter().any(|d| decorator_name(d) == "State");
        if is_state && prop.type_annotation.is_none() {
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

    for element in &statement.body.body {
        let StructElement::MethodDefinition(method) = element else {
            continue;
        };
        let is_build =
            matches!(&method.key, PropertyKey::StaticIdentifier(id) if id.name.as_str() == "build");
        if is_build {
            if let Some(ret_ty) = method.value.return_type.as_ref() {
                if !is_void_type(&ret_ty.type_annotation) {
                    errors.push(EtsTransformError::Diagnostic {
                        file: file.to_string(),
                        message: "build() must have a void return type".to_string(),
                        severity: DiagnosticSeverity::Error,
                    });
                }
            }
        }
    }
}

fn is_void_type(ty: &TSType<'_>) -> bool {
    match ty {
        TSType::TSVoidKeyword(_) => true,
        TSType::TSParenthesizedType(parenthesized) => is_void_type(&parenthesized.type_annotation),
        _ => false,
    }
}
