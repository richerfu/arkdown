use oxc_ast::ast::{
    Class, ClassElement, Declaration, Decorator, ExportDefaultDeclarationKind, Program, Statement,
    TSModuleDeclarationBody,
};

use crate::transform::decorator::{ArkuiDecorator, ArkuiDecoratorName};

const KNOWN_ARKUI_DECORATORS: &[&str] = &[
    ArkuiDecoratorName::COMPONENT,
    ArkuiDecoratorName::ENTRY,
    ArkuiDecoratorName::CUSTOM_DIALOG,
    ArkuiDecoratorName::REUSABLE,
    ArkuiDecoratorName::COMPONENT_V2,
    ArkuiDecoratorName::REUSABLE_V2,
    ArkuiDecoratorName::BUILDER,
    ArkuiDecoratorName::LOCAL_BUILDER,
    ArkuiDecoratorName::STYLES,
    ArkuiDecoratorName::EXTEND,
    ArkuiDecoratorName::ANIMATABLE_EXTEND,
    ArkuiDecoratorName::STATE,
    ArkuiDecoratorName::PROP,
    ArkuiDecoratorName::LINK,
    ArkuiDecoratorName::OBJECT_LINK,
    ArkuiDecoratorName::PROVIDE,
    ArkuiDecoratorName::CONSUME,
    ArkuiDecoratorName::BUILDER_PARAM,
    ArkuiDecoratorName::STORAGE_LINK,
    ArkuiDecoratorName::STORAGE_PROP,
    ArkuiDecoratorName::STORAGE_PROP_REF,
    ArkuiDecoratorName::LOCAL_STORAGE_LINK,
    ArkuiDecoratorName::LOCAL_STORAGE_PROP,
    ArkuiDecoratorName::LOCAL_STORAGE_PROP_REF,
    ArkuiDecoratorName::LOCAL,
    ArkuiDecoratorName::PARAM,
    ArkuiDecoratorName::ONCE,
    ArkuiDecoratorName::PROVIDER,
    ArkuiDecoratorName::CONSUMER,
    ArkuiDecoratorName::EVENT,
    ArkuiDecoratorName::ENV,
    ArkuiDecoratorName::REQUIRE,
    ArkuiDecoratorName::WATCH,
    ArkuiDecoratorName::TRACE,
    ArkuiDecoratorName::COMPUTED,
    ArkuiDecoratorName::MONITOR,
    ArkuiDecoratorName::SYNC_MONITOR,
    ArkuiDecoratorName::OBSERVED,
    ArkuiDecoratorName::OBSERVED_V2,
    ArkuiDecoratorName::COMPONENT_INIT,
    ArkuiDecoratorName::COMPONENT_APPEAR,
    ArkuiDecoratorName::COMPONENT_BUILT,
    ArkuiDecoratorName::COMPONENT_ATTACH,
    ArkuiDecoratorName::COMPONENT_DETACH,
    ArkuiDecoratorName::COMPONENT_REUSE,
    ArkuiDecoratorName::COMPONENT_RECYCLE,
    ArkuiDecoratorName::COMPONENT_DISAPPEAR,
    ArkuiDecoratorName::COMPONENT_ACTIVE,
    ArkuiDecoratorName::COMPONENT_INACTIVE,
    ArkuiDecoratorName::CONCURRENT,
];

pub struct ArktsTransformSyntaxDetector;

impl ArktsTransformSyntaxDetector {
    pub fn new() -> Self {
        Self
    }

    pub(super) fn program(&self, program: &Program<'_>) -> bool {
        program
            .body
            .iter()
            .any(|statement| self.statement(statement))
    }

    pub(super) fn is_known_decorator_name(&self, name: &str) -> bool {
        KNOWN_ARKUI_DECORATORS.contains(&name)
    }

    fn statement(&self, statement: &Statement<'_>) -> bool {
        match statement {
            Statement::StructStatement(_)
            | Statement::AnnotationDeclaration(_)
            | Statement::LazyImportDeclaration(_) => true,
            Statement::ClassDeclaration(class) => self.class_has_decorators(class),
            Statement::FunctionDeclaration(function) => {
                self.decorators_have_arkui_syntax(&function.decorators)
            }
            Statement::ExportNamedDeclaration(export) => {
                self.decorators_have_arkui_syntax(&export.decorators)
                    || matches!(
                        export.declaration.as_ref(),
                        Some(Declaration::StructStatement(_))
                            | Some(Declaration::AnnotationDeclaration(_))
                    )
                    || matches!(
                        export.declaration.as_ref(),
                        Some(Declaration::FunctionDeclaration(function))
                            if self.decorators_have_arkui_syntax(&function.decorators)
                    )
                    || matches!(
                        export.declaration.as_ref(),
                        Some(Declaration::ClassDeclaration(class))
                            if self.class_has_decorators(class)
                    )
                    || matches!(
                        export.declaration.as_ref(),
                        Some(Declaration::TSModuleDeclaration(module))
                            if self.ts_module_body(&module.body)
                    )
            }
            Statement::ExportDefaultDeclaration(export) => {
                matches!(
                    &export.declaration,
                    ExportDefaultDeclarationKind::StructStatement(_)
                ) || matches!(
                    &export.declaration,
                    ExportDefaultDeclarationKind::ClassDeclaration(class)
                        if self.class_has_decorators(class)
                )
            }
            Statement::TSModuleDeclaration(module) => self.ts_module_body(&module.body),
            _ => false,
        }
    }

    fn ts_module_body(&self, body: &Option<TSModuleDeclarationBody<'_>>) -> bool {
        match body {
            Some(TSModuleDeclarationBody::TSModuleDeclaration(module)) => {
                self.ts_module_body(&module.body)
            }
            Some(TSModuleDeclarationBody::TSModuleBlock(block)) => {
                block.body.iter().any(|statement| self.statement(statement))
            }
            None => false,
        }
    }

    fn decorators_have_arkui_syntax(&self, decorators: &[Decorator<'_>]) -> bool {
        decorators
            .iter()
            .map(|decorator| ArkuiDecorator::new(decorator).name())
            .any(|name| self.is_known_decorator_name(name))
    }

    fn class_has_decorators(&self, class: &Class<'_>) -> bool {
        !class.decorators.is_empty()
            || class.body.body.iter().any(|element| match element {
                ClassElement::PropertyDefinition(property) => !property.decorators.is_empty(),
                ClassElement::AccessorProperty(accessor) => !accessor.decorators.is_empty(),
                ClassElement::MethodDefinition(method) => !method.decorators.is_empty(),
                ClassElement::StaticBlock(_) | ClassElement::TSIndexSignature(_) => false,
            })
    }
}
