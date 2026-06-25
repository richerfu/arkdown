#[cfg(test)]
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    ClassElement, Declaration, ExportDefaultDeclarationKind, Expression, Program, PropertyKey,
    Statement, StructElement, StructStatement, TSType,
};
#[cfg(test)]
use oxc_parser::Parser;
#[cfg(test)]
use oxc_span::SourceType;
use std::path::Path;

use crate::error::{DiagnosticSeverity, EtsTransformError};
use crate::transform::decorator::{ArkuiDecorator, ArkuiDecoratorName};
use crate::transform::viewpu_emit::GeneratedViewpuName;
use rustc_hash::FxHashSet;

/// Run all etsChecker validation rules against the given source text.
///
/// Returns a list of diagnostics (errors and warnings). The list may be empty.
/// This function never panics — parse errors are silently skipped so that
/// downstream transforms can still attempt to proceed.
#[cfg(test)]
fn check(source: &str, file: &str) -> Vec<EtsTransformError> {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(file).unwrap_or_else(|_| SourceType::ets());
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.diagnostics.is_empty() {
        return Vec::new();
    }
    EtsChecker::new(&parsed.program, file).check()
}

pub struct EtsChecker<'program, 'alloc, 'file> {
    program: &'program Program<'alloc>,
    file: &'file str,
}

impl<'program, 'alloc, 'file> EtsChecker<'program, 'alloc, 'file> {
    pub fn new(program: &'program Program<'alloc>, file: &'file str) -> Self {
        Self { program, file }
    }

    pub fn check(&self) -> Vec<EtsTransformError> {
        let file = self.file;
        let mut errors: Vec<EtsTransformError> = Vec::new();
        let mut seen_component_names: FxHashSet<String> = Default::default();
        let mut entry_count = 0usize;

        for stmt in &self.program.body {
            match stmt {
                Statement::StructStatement(statement) => {
                    check_struct_component(statement, file, &mut seen_component_names, &mut errors);
                    // Count @Entry decorators — at most one per file.
                    if statement
                        .decorators
                        .iter()
                        .any(|d| ArkuiDecorator::new(d).name() == ArkuiDecoratorName::ENTRY)
                    {
                        entry_count += 1;
                    }
                    // Rule: @Sendable cannot be applied to structs — only to class declarations.
                    // hvigor's etsChecker rejects @Sendable on @Component structs.
                    if statement
                        .decorators
                        .iter()
                        .any(|d| ArkuiDecorator::new(d).name() == ArkuiDecoratorName::SENDABLE)
                    {
                        errors.push(EtsTransformError::Diagnostic {
                            file: file.to_string(),
                            message: format!(
                                "'@Sendable' cannot be applied to struct '{}'; \
                                 @Sendable is only valid on class declarations",
                                statement.id.name.as_str()
                            ),
                            severity: DiagnosticSeverity::Error,
                        });
                    }
                }
                Statement::ExportNamedDeclaration(export) => {
                    if let Some(Declaration::StructStatement(statement)) =
                        export.declaration.as_ref()
                    {
                        check_struct_component(
                            statement,
                            file,
                            &mut seen_component_names,
                            &mut errors,
                        );
                    }
                }
                Statement::ExportDefaultDeclaration(export) => {
                    if let ExportDefaultDeclarationKind::StructStatement(statement) =
                        &export.declaration
                    {
                        check_struct_component(
                            statement,
                            file,
                            &mut seen_component_names,
                            &mut errors,
                        );
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
                // Rule: @Concurrent can only be on a top-level function.
                // hvigor validates that @Concurrent is only used on standalone functions,
                // not on methods or arrow functions.
                let is_concurrent = func
                    .decorators
                    .iter()
                    .any(|d| ArkuiDecorator::new(d).name() == ArkuiDecoratorName::CONCURRENT);
                if is_concurrent {
                    // Basic validation: @Concurrent function must not have any non-function-level
                    // issues. The decorator itself on a function declaration is valid.
                    // Warn if it's on a generator or async function (not supported for Concurrent).
                    if func.generator {
                        let name = func
                            .id
                            .as_ref()
                            .map(|id| id.name.as_str())
                            .unwrap_or("<anonymous>");
                        errors.push(EtsTransformError::Diagnostic {
                            file: file.to_string(),
                            message: format!(
                                "@Concurrent function '{}' cannot be a generator function",
                                name
                            ),
                            severity: DiagnosticSeverity::Error,
                        });
                    }
                }

                let is_builder = func
                    .decorators
                    .iter()
                    .any(|d| ArkuiDecorator::new(d).name() == ArkuiDecoratorName::BUILDER);
                let is_styles = func
                    .decorators
                    .iter()
                    .any(|d| ArkuiDecorator::new(d).name() == ArkuiDecoratorName::STYLES);
                // @AnimatableExtend also requires void return type (matches hvigor validation).
                let is_animatable_extend = func.decorators.iter().any(|d| {
                    ArkuiDecorator::new(d).name() == ArkuiDecoratorName::ANIMATABLE_EXTEND
                });
                // @Extend(Component) must have void return type — it declares extra style
                // methods for an existing component.  Matches hvigor etsChecker's
                // validateExtendDecorator() rule.
                let is_extend = func
                    .decorators
                    .iter()
                    .any(|d| ArkuiDecorator::new(d).name() == ArkuiDecoratorName::EXTEND);
                if is_builder || is_styles || is_animatable_extend || is_extend {
                    if let Some(ret_ty) = func.return_type.as_ref() {
                        if !is_void_type(&ret_ty.type_annotation) {
                            let decorator = if is_builder {
                                "@Builder"
                            } else if is_styles {
                                "@Styles"
                            } else if is_extend {
                                "@Extend"
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
                    .any(|d| ArkuiDecorator::new(d).name() == ArkuiDecoratorName::COMPONENT);
                let has_component_v2 = class
                    .decorators
                    .iter()
                    .any(|d| ArkuiDecorator::new(d).name() == ArkuiDecoratorName::COMPONENT_V2);
                let has_observed = class
                    .decorators
                    .iter()
                    .any(|d| ArkuiDecorator::new(d).name() == ArkuiDecoratorName::OBSERVED);
                let has_observed_v2 = class
                    .decorators
                    .iter()
                    .any(|d| ArkuiDecorator::new(d).name() == ArkuiDecoratorName::OBSERVED_V2);
                let has_sendable = class
                    .decorators
                    .iter()
                    .any(|d| ArkuiDecorator::new(d).name() == ArkuiDecoratorName::SENDABLE);
                let has_entry = class
                    .decorators
                    .iter()
                    .any(|d| ArkuiDecorator::new(d).name() == ArkuiDecoratorName::ENTRY);

                // Rule: @Sendable is for inter-thread data transfer; it cannot be mixed
                // with UI-component decorators (@Component, @ComponentV2).
                // Matches hvigor's validateSendableClass() rule.
                if has_sendable && (has_component || has_component_v2) {
                    let comp_name = if has_component_v2 {
                        "@ComponentV2"
                    } else {
                        "@Component"
                    };
                    let class_name = class
                        .id
                        .as_ref()
                        .map(|id| id.name.as_str().to_string())
                        .unwrap_or_else(|| "<anonymous>".to_string());
                    errors.push(EtsTransformError::Diagnostic {
                        file: file.to_string(),
                        message: format!(
                            "Class '{}' cannot be decorated with both '@Sendable' and '{}'",
                            class_name, comp_name
                        ),
                        severity: DiagnosticSeverity::Error,
                    });
                }

                // Rule: @Observed and @ObservedV2 are data model decorators — cannot
                // be mixed with @Component, @ComponentV2, or @Entry decorators.
                // Matches hvigor etsChecker rule for data model classes.
                if (has_observed || has_observed_v2) && (has_component || has_component_v2) {
                    let obs_name = if has_observed_v2 {
                        "@ObservedV2"
                    } else {
                        "@Observed"
                    };
                    let comp_name = if has_component_v2 {
                        "@ComponentV2"
                    } else {
                        "@Component"
                    };
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

                // Rule: source-level @Track is rejected inside @ObservedV2.
                // hvigor accepts @Trace in @ObservedV2 source and lowers it to
                // runtime @Track later.
                if has_observed_v2 {
                    let class_name = class
                        .id
                        .as_ref()
                        .map(|id| id.name.as_str().to_string())
                        .unwrap_or_else(|| "<anonymous>".to_string());
                    for element in &class.body.body {
                        if let ClassElement::PropertyDefinition(prop) = element {
                            let has_track = prop.decorators.iter().any(|d| {
                                ArkuiDecorator::new(d).name() == ArkuiDecoratorName::TRACK
                            });
                            if has_track {
                                errors.push(EtsTransformError::Diagnostic {
                                    file: file.to_string(),
                                    message: format!(
                                        "'@Track' cannot be used in '@ObservedV2' class '{}'; \
                                     use '@Trace' in source so the compiler can lower it",
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
                                    id.name.as_str() == GeneratedViewpuName::BUILD
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
                            PropertyKey::StaticIdentifier(id) if id.name.as_str() == GeneratedViewpuName::BUILD);
                            if is_build {
                                if let Some(ret_ty) = method.value.return_type.as_ref() {
                                    if !is_void_type(&ret_ty.type_annotation) {
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

                    // Rule: @LocalBuilder methods must have void return type.
                    // @LocalBuilder declares a builder function scoped to the class; like @Builder
                    // it produces UI elements and must not return a value.
                    // Matches hvigor etsChecker's validateLocalBuilderReturnType() rule.
                    for elem in &class.body.body {
                        if let ClassElement::MethodDefinition(method) = elem {
                            let is_local_builder = method.decorators.iter().any(|d| {
                                ArkuiDecorator::new(d).name() == ArkuiDecoratorName::LOCAL_BUILDER
                            });
                            if is_local_builder {
                                if let Some(ret_ty) = method.value.return_type.as_ref() {
                                    if !is_void_type(&ret_ty.type_annotation) {
                                        let method_name = match &method.key {
                                            PropertyKey::StaticIdentifier(id) => {
                                                id.name.as_str().to_string()
                                            }
                                            _ => "<anonymous>".to_string(),
                                        };
                                        errors.push(EtsTransformError::Diagnostic {
                                            file: file.to_string(),
                                            message: format!(
                                            "@LocalBuilder method '{}' must have void return type",
                                            method_name
                                        ),
                                            severity: DiagnosticSeverity::Error,
                                        });
                                    }
                                }
                            }
                        }
                    }

                    // Rule: worker file must not use UI decorators
                    if CheckerSourcePath::new(file).is_worker_file() {
                        errors.push(EtsTransformError::Diagnostic {
                            file: file.to_string(),
                            message: "@Component/@Entry must not be used in worker files"
                                .to_string(),
                            severity: DiagnosticSeverity::Error,
                        });
                    }

                    // Rule: @BuilderParam fields without type annotations (warning)
                    for elem in &class.body.body {
                        if let ClassElement::PropertyDefinition(prop) = elem {
                            let is_builder_param = prop.decorators.iter().any(|d| {
                                ArkuiDecorator::new(d).name() == ArkuiDecoratorName::BUILDER_PARAM
                            });
                            if is_builder_param && prop.type_annotation.is_none() {
                                let field_name = match &prop.key {
                                    PropertyKey::StaticIdentifier(id) => {
                                        id.name.as_str().to_string()
                                    }
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
                            let is_state = prop.decorators.iter().any(|d| {
                                ArkuiDecorator::new(d).name() == ArkuiDecoratorName::STATE
                            });
                            if is_state && prop.type_annotation.is_none() {
                                // No type annotation on @State — allowed but may produce
                                // unexpected observable behavior; emit a warning.
                                let field_name = match &prop.key {
                                    PropertyKey::StaticIdentifier(id) => {
                                        id.name.as_str().to_string()
                                    }
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
                                    ArkuiDecorator::new(d).name(),
                                    ArkuiDecoratorName::STATE
                                        | ArkuiDecoratorName::PROP
                                        | ArkuiDecoratorName::LINK
                                )
                            });
                            if has_state_dec {
                                let field_name = match &prop.key {
                                    PropertyKey::StaticIdentifier(id) => {
                                        id.name.as_str().to_string()
                                    }
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
}

fn check_struct_component(
    statement: &StructStatement<'_>,
    file: &str,
    seen_component_names: &mut FxHashSet<String>,
    errors: &mut Vec<EtsTransformError>,
) {
    let component_name = statement.id.name.as_str();
    let decorator_names: Vec<&str> = statement
        .decorators
        .iter()
        .map(|decorator| ArkuiDecorator::new(decorator).name())
        .collect();
    let has_component_v1 = decorator_names.contains(&ArkuiDecoratorName::COMPONENT);
    let has_component_v2 = decorator_names.contains(&ArkuiDecoratorName::COMPONENT_V2);
    let has_custom_dialog = decorator_names.contains(&ArkuiDecoratorName::CUSTOM_DIALOG);
    let has_reusable = decorator_names.contains(&ArkuiDecoratorName::REUSABLE);
    let has_reusable_v2 = decorator_names.contains(&ArkuiDecoratorName::REUSABLE_V2);
    let has_entry = decorator_names.contains(&ArkuiDecoratorName::ENTRY);
    let has_preview = decorator_names.contains(&ArkuiDecoratorName::PREVIEW);
    let has_component = decorator_names
        .iter()
        .any(|name| ArkuiDecoratorName::is_component_struct_or_dialog(name));

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
        let reusable_dec = if has_reusable_v2 {
            "@ReusableV2"
        } else {
            "@Reusable"
        };
        errors.push(EtsTransformError::Diagnostic {
            file: file.to_string(),
            message: format!(
                "The struct '{}' cannot be decorated with both '@Entry' and '{}'",
                component_name, reusable_dec
            ),
            severity: DiagnosticSeverity::Error,
        });
    }

    // Rule: @CustomDialog cannot be combined with @Entry.
    // @CustomDialog defines a dialog component shown as an overlay; it cannot
    // serve as an application page entry point.
    // Matches hvigor etsChecker's validateCustomDialogNotEntry() rule.
    if has_entry && has_custom_dialog {
        errors.push(EtsTransformError::Diagnostic {
            file: file.to_string(),
            message: format!(
                "The struct '{}' cannot be decorated with both '@Entry' and '@CustomDialog'",
                component_name
            ),
            severity: DiagnosticSeverity::Error,
        });
    }

    // Rule: @Preview can only be used on @Component structs.
    // Matches hvigor etsChecker's validatePreviewDecorator() rule.
    if has_preview && !has_component {
        errors.push(EtsTransformError::Diagnostic {
            file: file.to_string(),
            message: format!(
                "'@Preview' can only be applied to '@Component' structs; \
                 struct '{}' is missing '@Component'",
                component_name
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
                    if matches!(&method.key, PropertyKey::StaticIdentifier(id) if id.name.as_str() == GeneratedViewpuName::BUILD)
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

        if CheckerSourcePath::new(file).is_worker_file() {
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
            let name = ArkuiDecorator::new(d).name();
            ArkuiDecoratorName::is_checker_state_property(name).then_some(name)
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
            .any(|d| ArkuiDecorator::new(d).name() == ArkuiDecoratorName::BUILDER_PARAM);
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
        for storage_dec in &[
            ArkuiDecoratorName::STORAGE_LINK,
            ArkuiDecoratorName::STORAGE_PROP,
            ArkuiDecoratorName::APP_STORAGE_LINK,
            ArkuiDecoratorName::APP_STORAGE_PROP,
        ] {
            let found_without_key = prop.decorators.iter().any(|d| {
                let name = ArkuiDecorator::new(d).name();
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

        let is_state = prop
            .decorators
            .iter()
            .any(|d| ArkuiDecorator::new(d).name() == ArkuiDecoratorName::STATE);
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

        // Rule: @Require can only be combined with the required-state decorators
        // for the component version. V1 allows @Prop/@State/@Provide/@BuilderParam;
        // V2 allows @Param/@BuilderParam per SDK process_struct_componentV2.js.
        let has_require = prop
            .decorators
            .iter()
            .any(|d| ArkuiDecorator::new(d).name() == ArkuiDecoratorName::REQUIRE);
        if has_require {
            let has_allowed = prop.decorators.iter().any(|d| {
                let name = ArkuiDecorator::new(d).name();
                if has_component_v2 {
                    matches!(
                        name,
                        ArkuiDecoratorName::PARAM | ArkuiDecoratorName::BUILDER_PARAM
                    )
                } else {
                    ArkuiDecoratorName::can_combine_with_require(name)
                }
            });
            if !has_allowed {
                let allowed = if has_component_v2 {
                    "'@Param' or '@BuilderParam'"
                } else {
                    "'@Prop', '@State', '@Provide', or '@BuilderParam'"
                };
                errors.push(EtsTransformError::Diagnostic {
                    file: file.to_string(),
                    message: if has_component_v2 {
                        "In a struct decorated with '@ComponentV2', '@Require' can only be used with '@Param' and '@BuilderParam'.".to_string()
                    } else {
                        format!(
                            "'@Require' on field '{}' can only be combined with {}",
                            field_name, allowed
                        )
                    },
                    severity: DiagnosticSeverity::Error,
                });
            }
        }

        // Rule: @Once is a modifier for @Param in ComponentV2. It must not
        // create a standalone param-like field.
        if has_component_v2 {
            let has_once = prop
                .decorators
                .iter()
                .any(|d| ArkuiDecorator::new(d).name() == ArkuiDecoratorName::ONCE);
            let has_param = prop
                .decorators
                .iter()
                .any(|d| ArkuiDecorator::new(d).name() == ArkuiDecoratorName::PARAM);
            if has_once && !has_param {
                errors.push(EtsTransformError::Diagnostic {
                    file: file.to_string(),
                    message:
                        "When a variable decorated with '@Once', it must also be decorated with '@Param'."
                            .to_string(),
                    severity: DiagnosticSeverity::Error,
                });
            }
            if has_param && prop.value.is_none() && !has_require {
                errors.push(EtsTransformError::Diagnostic {
                    file: file.to_string(),
                    message: "When a variable decorated with '@Param' is not assigned a default value, it must also be decorated with '@Require'.".to_string(),
                    severity: DiagnosticSeverity::Error,
                });
            }
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
                if ArkuiDecorator::new(decorator).name() != ArkuiDecoratorName::WATCH {
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
        let is_build = matches!(&method.key, PropertyKey::StaticIdentifier(id) if id.name.as_str() == GeneratedViewpuName::BUILD);
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

        // Rule: @LocalBuilder method in a struct must have void return type.
        // Matches hvigor etsChecker's validateLocalBuilderReturnType() rule.
        let is_local_builder = method
            .decorators
            .iter()
            .any(|d| ArkuiDecorator::new(d).name() == ArkuiDecoratorName::LOCAL_BUILDER);
        if is_local_builder {
            if let Some(ret_ty) = method.value.return_type.as_ref() {
                if !is_void_type(&ret_ty.type_annotation) {
                    let method_name = match &method.key {
                        PropertyKey::StaticIdentifier(id) => id.name.as_str().to_string(),
                        _ => "<anonymous>".to_string(),
                    };
                    errors.push(EtsTransformError::Diagnostic {
                        file: file.to_string(),
                        message: format!(
                            "@LocalBuilder method '{}' must have void return type",
                            method_name
                        ),
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

struct CheckerSourcePath<'a> {
    file: &'a str,
}

impl<'a> CheckerSourcePath<'a> {
    fn new(file: &'a str) -> Self {
        Self { file }
    }

    fn is_worker_file(&self) -> bool {
        Path::new(self.file).components().any(|component| {
            matches!(
                component,
                std::path::Component::Normal(segment)
                    if segment == "worker" || segment == "workers"
            )
        })
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
    fn test_track_on_plain_class_matches_hvigor_class_decorator_rule() {
        let src = "class MyData {\n  @Track count: number = 0;\n}";
        let errs = check(src, "test.ets");
        assert!(
            errs.iter()
                .all(|err| !matches!(err, EtsTransformError::Diagnostic { message, .. } if message.contains("@Track"))),
            "plain class @Track must not produce checker errors; got: {:?}",
            errs
        );
    }

    #[test]
    fn test_track_on_observed_v1_class_matches_hvigor_class_decorator_rule() {
        let src = "@Observed\nclass MyData {\n  @Track count: number = 0;\n}";
        let errs = check(src, "test.ets");
        assert!(
            errs.iter()
                .all(|err| !matches!(err, EtsTransformError::Diagnostic { message, .. } if message.contains("@Track"))),
            "@Observed class @Track must not produce checker errors; got: {:?}",
            errs
        );
    }

    #[test]
    fn test_track_in_observed_v2_class_is_error() {
        let src = "@ObservedV2\nclass MyData {\n  @Track count: number = 0;\n}";
        assert!(
            has_error_containing(src, "@Track"),
            "@ObservedV2 source must use @Trace, not direct @Track"
        );
    }

    #[test]
    fn test_observed_alone_is_valid() {
        let src = "@Observed\nclass MyData {\n  count: number = 0;\n}";
        let errs = check(src, "test.ets");
        assert!(
            !errs
                .iter()
                .any(|e| matches!(e, EtsTransformError::Diagnostic { .. })),
            "@Observed alone must not produce errors; got: {:?}",
            errs
        );
    }

    #[test]
    fn test_observed_v2_alone_is_valid() {
        let src = "@ObservedV2\nclass MyData {\n  count: number = 0;\n}";
        let errs = check(src, "test.ets");
        assert!(
            !errs.iter().any(|e| matches!(
                e,
                EtsTransformError::Diagnostic {
                    severity: crate::error::DiagnosticSeverity::Error,
                    ..
                }
            )),
            "@ObservedV2 alone must not produce errors; got: {:?}",
            errs
        );
    }

    // ── @Entry + @CustomDialog ────────────────────────────────────────────────

    #[test]
    fn test_entry_with_custom_dialog_is_error() {
        let src = r#"
@Entry
@CustomDialog
@Component
struct MyDialog {
  controller: CustomDialogController;
  build() {}
}
"#;
        assert!(
            has_error_containing(src, "@Entry"),
            "@Entry + @CustomDialog must be an error"
        );
    }

    #[test]
    fn test_custom_dialog_without_entry_is_valid() {
        let src = r#"
@CustomDialog
@Component
struct MyDialog {
  controller: CustomDialogController;
  build() {}
}
"#;
        let errs = check(src, "test.ets");
        let entry_errors: Vec<_> = errs
            .iter()
            .filter(|e| {
                matches!(e, EtsTransformError::Diagnostic { message, severity, .. }
                    if message.contains("@CustomDialog") && message.contains("@Entry")
                    && matches!(severity, crate::error::DiagnosticSeverity::Error))
            })
            .collect();
        assert!(
            entry_errors.is_empty(),
            "@CustomDialog without @Entry must not produce conflict error; got: {:?}",
            errs
        );
    }

    // ── @Require ──────────────────────────────────────────────────────────────

    #[test]
    fn test_require_on_prop_is_valid() {
        let src = r#"
@Component
struct MyComp {
  @Require @Prop title: string = '';
  build() {}
}
"#;
        let errs = check(src, "test.ets");
        let require_errors: Vec<_> = errs
            .iter()
            .filter(|e| {
                matches!(e, EtsTransformError::Diagnostic { message, severity, .. }
                    if message.contains("@Require")
                    && matches!(severity, crate::error::DiagnosticSeverity::Error))
            })
            .collect();
        assert!(
            require_errors.is_empty(),
            "@Require + @Prop must be valid; got: {:?}",
            errs
        );
    }

    #[test]
    fn test_require_on_link_is_error() {
        let src = r#"
@Component
struct MyComp {
  @Require @Link count: number;
  build() {}
}
"#;
        assert!(
            has_error_containing(src, "@Require"),
            "@Require on @Link must be an error"
        );
    }

    #[test]
    fn test_require_alone_is_error() {
        let src = r#"
@Component
struct MyComp {
  @Require name: string = '';
  build() {}
}
"#;
        assert!(
            has_error_containing(src, "@Require"),
            "@Require without @Prop/@State/@Provide/@BuilderParam must be an error"
        );
    }

    #[test]
    fn test_require_on_state_is_valid() {
        let src = r#"
@Component
struct MyComp {
  @Require @State count: number = 0;
  build() {}
}
"#;
        let errs = check(src, "test.ets");
        let require_errors: Vec<_> = errs
            .iter()
            .filter(|e| {
                matches!(e, EtsTransformError::Diagnostic { message, severity, .. }
                    if message.contains("@Require")
                    && matches!(severity, crate::error::DiagnosticSeverity::Error))
            })
            .collect();
        assert!(
            require_errors.is_empty(),
            "@Require + @State must be valid; got: {:?}",
            errs
        );
    }

    #[test]
    fn test_require_on_component_v2_param_is_valid() {
        let src = r#"
@ComponentV2
struct MyComp {
  @Require @Param title: string = '';
  build() {}
}
"#;
        let errs = check(src, "test.ets");
        let require_errors: Vec<_> = errs
            .iter()
            .filter(|e| {
                matches!(e, EtsTransformError::Diagnostic { message, severity, .. }
                    if message.contains("@Require")
                    && matches!(severity, crate::error::DiagnosticSeverity::Error))
            })
            .collect();
        assert!(
            require_errors.is_empty(),
            "@ComponentV2 @Require + @Param must be valid; got: {:?}",
            errs
        );
    }

    #[test]
    fn test_once_without_param_on_component_v2_is_error() {
        let src = r#"
@ComponentV2
struct MyComp {
  @Once title: string = '';
  build() {}
}
"#;
        assert!(
            has_error_containing(src, "@Once"),
            "@ComponentV2 @Once without @Param must be an error"
        );
    }

    // ── @Extend ───────────────────────────────────────────────────────────────

    #[test]
    fn test_extend_with_non_void_return_is_error() {
        let src = r#"@Extend(Button) function myButton(): string {}"#;
        assert!(
            has_error_containing(src, "@Extend"),
            "@Extend function with non-void return must be an error"
        );
    }

    #[test]
    fn test_extend_with_void_return_is_valid() {
        let src = r#"@Extend(Button) function myButton(): void {}"#;
        let errs = check(src, "test.ets");
        let extend_errors: Vec<_> = errs
            .iter()
            .filter(|e| {
                matches!(e, EtsTransformError::Diagnostic { message, severity, .. }
                    if message.contains("@Extend")
                    && matches!(severity, crate::error::DiagnosticSeverity::Error))
            })
            .collect();
        assert!(
            extend_errors.is_empty(),
            "@Extend with void return must be valid; got: {:?}",
            errs
        );
    }

    // ── @LocalBuilder ─────────────────────────────────────────────────────────

    #[test]
    fn test_local_builder_with_non_void_return_in_struct_is_error() {
        let src = r#"
@Component
struct MyComp {
  @LocalBuilder myBuilder(): string { return ''; }
  build() {}
}
"#;
        assert!(
            has_error_containing(src, "@LocalBuilder"),
            "@LocalBuilder method with non-void return in struct must be an error"
        );
    }

    #[test]
    fn test_local_builder_with_void_return_in_struct_is_valid() {
        let src = r#"
@Component
struct MyComp {
  @LocalBuilder myBuilder(): void {}
  build() {}
}
"#;
        let errs = check(src, "test.ets");
        let lb_errors: Vec<_> = errs
            .iter()
            .filter(|e| {
                matches!(e, EtsTransformError::Diagnostic { message, severity, .. }
                    if message.contains("@LocalBuilder")
                    && matches!(severity, crate::error::DiagnosticSeverity::Error))
            })
            .collect();
        assert!(
            lb_errors.is_empty(),
            "@LocalBuilder with void return in struct must be valid; got: {:?}",
            errs
        );
    }

    #[test]
    fn test_local_builder_with_non_void_return_in_class_is_error() {
        let src = r#"
@Component
class MyComp {
  @LocalBuilder myBuilder(): string { return ''; }
  build(): void {}
}
"#;
        assert!(
            has_error_containing(src, "@LocalBuilder"),
            "@LocalBuilder method with non-void return in class must be an error"
        );
    }
}
