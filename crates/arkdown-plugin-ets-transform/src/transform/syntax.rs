use oxc_allocator::Allocator;
use oxc_ast::ast::{Declaration, Decorator, ExportDefaultDeclarationKind, Program, Statement};
use oxc_parser::Parser;
use oxc_span::SourceType;

use crate::transform::decorator::decorator_name;

const KNOWN_ARKUI_DECORATORS: &[&str] = &[
    "Component",
    "Entry",
    "CustomDialog",
    "Reusable",
    "ComponentV2",
    "ReusableV2",
    "Builder",
    "LocalBuilder",
    "Styles",
    "Extend",
    "AnimatableExtend",
    "State",
    "Prop",
    "Link",
    "ObjectLink",
    "Provide",
    "Consume",
    "BuilderParam",
    "StorageLink",
    "StorageProp",
    "StoragePropRef",
    "LocalStorageLink",
    "LocalStorageProp",
    "LocalStoragePropRef",
    "Local",
    "Param",
    "Once",
    "Provider",
    "Consumer",
    "Event",
    "Env",
    "Require",
    "Watch",
    "Trace",
    "Computed",
    "Monitor",
    "SyncMonitor",
    "ComponentInit",
    "ComponentAppear",
    "ComponentBuilt",
    "ComponentAttach",
    "ComponentDetach",
    "ComponentReuse",
    "ComponentRecycle",
    "ComponentDisappear",
    "ComponentActive",
    "ComponentInactive",
];

pub(crate) fn has_arkts_transform_syntax(content: &str, id: &str) -> bool {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(id).unwrap_or_else(|_| SourceType::ets());
    let parsed = Parser::new(&allocator, content, source_type).parse();
    !parsed.errors.is_empty() || program_has_arkts_transform_syntax(&parsed.program)
}

pub(super) fn program_has_arkts_transform_syntax(program: &Program<'_>) -> bool {
    program
        .body
        .iter()
        .any(statement_has_arkts_transform_syntax)
}

fn statement_has_arkts_transform_syntax(statement: &Statement<'_>) -> bool {
    match statement {
        Statement::StructStatement(_)
        | Statement::AnnotationDeclaration(_)
        | Statement::LazyImportDeclaration(_) => true,
        Statement::ClassDeclaration(class) => decorators_have_arkui_syntax(&class.decorators),
        Statement::FunctionDeclaration(function) => {
            decorators_have_arkui_syntax(&function.decorators)
        }
        Statement::ExportNamedDeclaration(export) => {
            decorators_have_arkui_syntax(&export.decorators)
                || matches!(
                    export.declaration.as_ref(),
                    Some(Declaration::StructStatement(_))
                        | Some(Declaration::AnnotationDeclaration(_))
                )
                || matches!(
                    export.declaration.as_ref(),
                    Some(Declaration::FunctionDeclaration(function))
                        if decorators_have_arkui_syntax(&function.decorators)
                )
        }
        Statement::ExportDefaultDeclaration(export) => {
            matches!(
                &export.declaration,
                ExportDefaultDeclarationKind::StructStatement(_)
            )
        }
        _ => false,
    }
}

fn decorators_have_arkui_syntax(decorators: &[Decorator<'_>]) -> bool {
    decorators
        .iter()
        .map(decorator_name)
        .any(|name| KNOWN_ARKUI_DECORATORS.contains(&name))
}
