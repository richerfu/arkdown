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
            // @AnimatableExtend also requires void return type (matches hvigor validation).
            let is_animatable_extend = func
                .decorators
                .iter()
                .any(|d| decorator_name(d) == "AnimatableExtend");
            if is_builder || is_styles || is_animatable_extend {
                if let Some(ret_ty) = func.return_type.as_ref() {
                    if !is_void_type(&ret_ty.type_annotation) {
                        let decorator = if is_builder {
                            "@Builder"
                        } else if is_styles {
                            "@Styles"
                        } else {
                            "@AnimatableExtend"
                        };
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
            let has_component_v2 = class
                .decorators
                .iter()
                .any(|d| decorator_name(d) == "ComponentV2");
            let has_observed = class
                .decorators
                .iter()
                .any(|d| decorator_name(d) == "Observed");
            let has_observed_v2 = class
                .decorators
                .iter()
                .any(|d| decorator_name(d) == "ObservedV2");
            let has_entry = class
                .decorators
                .iter()
                .any(|d| decorator_name(d) == "Entry");

            // Rule: @Observed and @ObservedV2 are data model decorators — cannot
            // be mixed with @Component, @ComponentV2, or @Entry decorators.
            // Matches hvigor etsChecker rule for data model classes.
            if (has_observed || has_observed_v2) && (has_component || has_component_v2) {
                let obs_name = if has_observed_v2 { "@ObservedV2" } else { "@Observed" };
                let comp_name = if has_component_v2 { "@ComponentV2" } else { "@Component" };
                let class_name = class
                    .id
                    .as_ref()
                    .map(|id| id.name.as_str().to_string())
                    .unwrap_or_else(|| "<anonymous>".to_string());
                errors.push(EtsTransformError::Diagnostic {
                    file: file.to_string(),
                    message: format!(
                        "Class '{}' cannot be decorated with both '{}' and '{}'",
                        class_name, obs_name, comp_name
                    ),
                    severity: DiagnosticSeverity::Error,
                });
            }

            // Rule: @ObservedV2 and @Observed cannot be mixed.
            if has_observed && has_observed_v2 {
                let class_name = class
                    .id
                    .as_ref()
                    .map(|id| id.name.as_str().to_string())
                    .unwrap_or_else(|| "<anonymous>".to_string());
                errors.push(EtsTransformError::Diagnostic {
                    file: file.to_string(),
                    message: format!(
                        "Class '{}' cannot use both '@Observed' and '@ObservedV2' decorators",
                        class_name
                    ),
                    severity: DiagnosticSeverity::Error,
                });
            }

            // Rule: @Track is only valid in @ObservedV2 classes; using it in
            // non-@ObservedV2 classes (including @Observed or plain classes) is an error.
            // Matches hvigor etsChecker's validateTrackDecorator() rule.
            if !has_observed_v2 {
                let class_name = class
                    .id
                    .as_ref()
                    .map(|id| id.name.as_str().to_string())
                    .unwrap_or_else(|| "<anonymous>".to_string());
                for element in &class.body.body {
                    if let ClassElement::PropertyDefinition(prop) = element {
                        let has_track =
                            prop.decorators.iter().any(|d| decorator_name(d) == "Track");
                        if has_track {
                            errors.push(EtsTransformError::Diagnostic {
                                file: file.to_string(),
                                message: format!(
                                    "'@Track' can only be used in '@ObservedV2' classes; \
                                     class '{}' is not decorated with '@ObservedV2'",
                                    class_name
                                ),
                                severity: DiagnosticSeverity::Error,
                            });
                        }
                    }
                }
            }

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

    // Rule: @Reusable cannot be combined with @Entry.
    // @Reusable components are designed for reuse, not as page entry points.
    // Matches hvigor etsChecker's validateReusableNotEntry() rule.
    if has_entry && (has_reusable || has_reusable_v2) {
        let reusable_dec = if has_reusable_v2 { "@ReusableV2" } else { "@Reusable" };
        errors.push(EtsTransformError::Diagnostic {
            file: file.to_string(),
            message: format!(
                "The struct '{}' cannot be decorated with both '@Entry' and '{}'",
                component_name, reusable_dec
            ),
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
        let state_dec_name = prop.decorators.iter().find_map(|d| {
            let name = decorator_name(d);
            matches!(
                name,
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
            .then_some(name)
        });
        if let Some(dec_name) = state_dec_name {
            if !has_component {
                errors.push(EtsTransformError::Diagnostic {
                    file: file.to_string(),
                    message: format!(
                        "State decorator '@{}' on '{}' in non-@Component struct",
                        dec_name, field_name
                    ),
                    severity: DiagnosticSeverity::Error,
                });
            }
            // State decorators are not allowed on static struct properties.
            // Matches hvigor etsChecker's validateStateDecoratorNotStatic() rule.
            if prop.r#static {
                errors.push(EtsTransformError::Diagnostic {
                    file: file.to_string(),
                    message: format!(
                        "'@{}' is not allowed on static property '{}' in struct '{}'",
                        dec_name, field_name, component_name
                    ),
                    severity: DiagnosticSeverity::Error,
                });
            }
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

        // Rule: @StorageLink/@StorageProp/@AppStorageLink/@AppStorageProp require
        // a string key argument — e.g. @StorageLink("myKey").
        // Matches hvigor etsChecker's validateStorageDecoratorHasKey() rule.
        for storage_dec in &["StorageLink", "StorageProp", "AppStorageLink", "AppStorageProp"] {
            let found_without_key = prop.decorators.iter().any(|d| {
                let name = decorator_name(d);
                if name != *storage_dec {
                    return false;
                }
                // Check if it's a bare identifier (no argument) — which is invalid.
                matches!(&d.expression, Expression::Identifier(_))
            });
            if found_without_key {
                errors.push(EtsTransformError::Diagnostic {
                    file: file.to_string(),
                    message: format!(
                        "'@{}' on field '{}' requires a storage key argument, e.g. @{}(\"key\")",
                        storage_dec, field_name, storage_dec
                    ),
                    severity: DiagnosticSeverity::Error,
                });
            }
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

    // Validate @Watch decorator arguments reference valid methods in this struct.
    // Collect method names first, then check @Watch("name") properties.
    // Matches hvigor etsChecker's validateWatchDecoratorReference() rule.
    if has_component {
        let method_names: FxHashSet<&str> = statement
            .body
            .body
            .iter()
            .filter_map(|elem| {
                if let StructElement::MethodDefinition(method) = elem {
                    if let PropertyKey::StaticIdentifier(id) = &method.key {
                        return Some(id.name.as_str());
                    }
                }
                None
            })
            .collect();

        for element in &statement.body.body {
            let StructElement::PropertyDefinition(prop) = element else {
                continue;
            };
            for decorator in &prop.decorators {
                if decorator_name(decorator) != "Watch" {
                    continue;
                }
                // Extract the string argument: @Watch("methodName")
                let method_ref = if let Expression::CallExpression(call) = &decorator.expression {
                    call.arguments.first().and_then(|arg| {
                        if let oxc_ast::ast::Argument::StringLiteral(s) = arg {
                            Some(s.value.as_str())
                        } else {
                            None
                        }
                    })
                } else {
                    None
                };
                if let Some(method_name) = method_ref {
                    if !method_name.is_empty() && !method_names.contains(method_name) {
                        let field_name = match &prop.key {
                            PropertyKey::StaticIdentifier(id) => id.name.as_str().to_string(),
                            _ => "<unknown>".to_string(),
                        };
                        errors.push(EtsTransformError::Diagnostic {
                            file: file.to_string(),
                            message: format!(
                                "@Watch on field '{}' references method '{}' which does not exist in struct '{}'",
                                field_name, method_name, component_name
                            ),
                            severity: DiagnosticSeverity::Error,
                        });
                    }
                }
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn has_error_containing(source: &str, fragment: &str) -> bool {
        let errs = check(source, "test.ets");
        errs.iter().any(|e| match e {
            EtsTransformError::Diagnostic { message, .. } => message.contains(fragment),
            _ => false,
        })
    }

    #[test]
    fn test_observed_with_component_is_error() {
        let src = "@Observed\n@Component\nclass MyData {\n  count: number = 0;\n}";
        assert!(
            has_error_containing(src, "@Observed"),
            "@Observed with @Component must be an error"
        );
    }

    #[test]
    fn test_observed_v2_with_component_v2_is_error() {
        let src = "@ObservedV2\n@ComponentV2\nclass MyData {\n  count: number = 0;\n}";
        assert!(
            has_error_containing(src, "@ObservedV2"),
            "@ObservedV2 with @ComponentV2 must be an error"
        );
    }

    #[test]
    fn test_observed_and_observed_v2_mixed_is_error() {
        let src = "@Observed\n@ObservedV2\nclass MyData {\n  count: number = 0;\n}";
        assert!(
            has_error_containing(src, "@Observed"),
            "@Observed with @ObservedV2 must be an error"
        );
    }

    #[test]
    fn test_track_outside_observed_v2_is_error() {
        // @Track on a plain class should error.
        let src = "class MyData {\n  @Track count: number = 0;\n}";
        assert!(
            has_error_containing(src, "@Track"),
            "@Track outside @ObservedV2 must be an error"
        );
    }

    #[test]
    fn test_track_on_observed_v1_class_is_error() {
        // @Track is V2 only; cannot use in @Observed (V1) class.
        let src = "@Observed\nclass MyData {\n  @Track count: number = 0;\n}";
        assert!(
            has_error_containing(src, "@Track"),
            "@Track in @Observed class must be an error (V2 decorator in V1 class)"
        );
    }

    #[test]
    fn test_track_in_observed_v2_class_is_valid() {
        // @Track is valid inside @ObservedV2 class.
        let src = "@ObservedV2\nclass MyData {\n  @Track count: number = 0;\n}";
        let errs = check(src, "test.ets");
        let track_errors: Vec<_> = errs
            .iter()
            .filter(|e| {
                matches!(e, EtsTransformError::Diagnostic { message, severity, .. }
                    if message.contains("@Track") && matches!(severity, crate::error::DiagnosticSeverity::Error))
            })
            .collect();
        assert!(
            track_errors.is_empty(),
            "@Track in @ObservedV2 class must not produce errors; got: {:?}",
            errs
        );
    }

    #[test]
    fn test_observed_alone_is_valid() {
        let src = "@Observed\nclass MyData {\n  count: number = 0;\n}";
        let errs = check(src, "test.ets");
        assert!(
            !errs.iter().any(|e| matches!(e, EtsTransformError::Diagnostic { .. })),
            "@Observed alone must not produce errors; got: {:?}",
            errs
        );
    }

    #[test]
    fn test_observed_v2_alone_is_valid() {
        let src = "@ObservedV2\nclass MyData {\n  count: number = 0;\n}";
        let errs = check(src, "test.ets");
        assert!(
            !errs.iter().any(|e| matches!(e, EtsTransformError::Diagnostic {
                severity: crate::error::DiagnosticSeverity::Error, ..
            })),
            "@ObservedV2 alone must not produce errors; got: {:?}",
            errs
        );
    }
}
