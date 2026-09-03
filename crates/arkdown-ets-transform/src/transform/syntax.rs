use oxc_ast::ast::{
    CallExpression, Class, ClassElement, Declaration, Decorator, ExportDefaultDeclarationKind,
    Expression, Program, Statement, TSNamespaceDeclarationBody,
};
use oxc_ast_visit::{walk, Visit};

use crate::transform::decorator::{ArkuiDecorator, ArkuiDecoratorName};
use crate::transform::global_rewrite::{
    EsObjectTypeRewrite, GlobalContextCallRewrite, WorkerPathRewrite,
};

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
    ArkuiDecoratorName::CUSTOM_ENV,
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
        GlobalContextCallRewrite::new().program_needs_rewrite(program)
            || WorkerPathRewrite::new().program_needs_rewrite(program)
            || EsObjectTypeRewrite::new().program_needs_rewrite(program)
            || BuilderFactoryCallDetector::new().contains(program)
            || program
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
            Statement::ExportDeclaration(export) => {
                self.decorators_have_arkui_syntax(&export.decorators)
                    || matches!(
                        &export.declaration,
                        Declaration::StructStatement(_) | Declaration::AnnotationDeclaration(_)
                    )
                    || matches!(
                        &export.declaration,
                        Declaration::FunctionDeclaration(function)
                            if self.decorators_have_arkui_syntax(&function.decorators)
                    )
                    || matches!(
                        &export.declaration,
                        Declaration::ClassDeclaration(class)
                            if self.class_has_decorators(class)
                    )
                    || matches!(
                        &export.declaration,
                        Declaration::TSNamespaceDeclaration(module)
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
            Statement::TSNamespaceDeclaration(module) => self.ts_module_body(&module.body),
            Statement::TSExternalModuleDeclaration(module) => match module.body.as_deref() {
                Some(block) => block.body.iter().any(|s| self.statement(s)),
                None => false,
            },
            _ => false,
        }
    }

    fn ts_module_body(&self, body: &TSNamespaceDeclarationBody<'_>) -> bool {
        match body {
            TSNamespaceDeclarationBody::TSNamespaceDeclaration(module) => {
                self.ts_module_body(&module.body)
            }
            TSNamespaceDeclarationBody::TSModuleBlock(block) => {
                block.body.iter().any(|statement| self.statement(statement))
            }
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
                _ => false,
            })
    }
}

struct BuilderFactoryCallDetector {
    found: bool,
}

impl BuilderFactoryCallDetector {
    fn new() -> Self {
        Self { found: false }
    }

    fn contains<'a>(mut self, program: &Program<'a>) -> bool {
        self.visit_program(program);
        self.found
    }
}

impl<'a> Visit<'a> for BuilderFactoryCallDetector {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if self.found {
            return;
        }
        if matches!(
            &call.callee,
            Expression::Identifier(identifier)
                if matches!(identifier.name.as_str(), "wrapBuilder" | "mutableBuilder")
        ) {
            self.found = true;
            return;
        }
        walk::walk_call_expression(self, call);
    }
}
