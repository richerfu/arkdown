use oxc_allocator::{Allocator, ArenaVec, CloneIn, FromIn};
use oxc_ast::{ast::*, builder::AstBuilder};
use oxc_ast_visit::{walk_mut, VisitMut};
use oxc_parser::Parser;
use oxc_span::{SourceType, SPAN};
use oxc_str::{Ident, Str};

use super::{
    AnnotationConstant, AnnotationImportDisposition, AnnotationProperty, AnnotationUse,
    ArktsFileTypeFacts,
};
use crate::error::EtsTransformError;

const ANNOTATION_PREFIX: &str = "__$$ETS_ANNOTATION$$__";

/// OXC code generation for OH `ohApi.ts::transformAnnotation`.
///
/// Type identity, retention, evaluated constants and inferred type text come
/// exclusively from the native TypeScript checker snapshot. This pass owns the
/// two source transformations from upstream: declaration lowering first, then
/// import/use lowering.
pub(crate) struct AnnotationLowerer<'a, 'facts> {
    source_id: &'facts str,
    allocator: &'a Allocator,
    ast: AstBuilder<'a>,
    facts: &'facts ArktsFileTypeFacts,
    failure: Option<String>,
}

impl<'a, 'facts> AnnotationLowerer<'a, 'facts> {
    pub(crate) fn requires_facts(program: &Program<'_>) -> bool {
        program.body.iter().any(|statement| match statement {
            Statement::AnnotationDeclaration(_) => true,
            Statement::ExportDeclaration(export) => {
                matches!(export.declaration, Declaration::AnnotationDeclaration(_))
            }
            _ => false,
        })
    }

    pub(crate) fn new(
        allocator: &'a Allocator,
        source_id: &'facts str,
        facts: &'facts ArktsFileTypeFacts,
    ) -> Self {
        Self {
            source_id,
            allocator,
            ast: AstBuilder::new(allocator),
            facts,
            failure: None,
        }
    }

    pub(crate) fn program(mut self, program: &mut Program<'a>) -> Result<(), EtsTransformError> {
        // Upstream visits declarations before annotation imports and uses.
        program
            .body
            .retain_mut(|statement| self.annotation_declaration(statement));
        program
            .body
            .retain_mut(|statement| self.annotation_import(statement));
        self.visit_program(program);
        self.failure.map_or(Ok(()), |message| {
            Err(EtsTransformError::Fatal {
                file: self.source_id.to_string(),
                message,
            })
        })
    }

    fn annotation_declaration(&mut self, statement: &mut Statement<'a>) -> bool {
        let declaration = match statement {
            Statement::AnnotationDeclaration(declaration) => Some(declaration.as_mut()),
            Statement::ExportDeclaration(export) => match &mut export.declaration {
                Declaration::AnnotationDeclaration(declaration) => Some(declaration.as_mut()),
                _ => None,
            },
            _ => None,
        };
        let Some(declaration) = declaration else {
            return true;
        };
        let Some(info) = self.facts.declaration(declaration.span).cloned() else {
            self.fail("native TypeScript returned no facts for an annotation declaration");
            return true;
        };
        if info.source_retention {
            return false;
        }
        self.prefix_binding(&mut declaration.id);
        if declaration.body.body.len() != info.properties.len() {
            self.fail("annotation declaration member count differs from native TypeScript facts");
            return true;
        }
        for (element, property) in declaration.body.body.iter_mut().zip(&info.properties) {
            let AnnotationElement::PropertyDefinition(definition) = element;
            definition.type_annotation = self.type_annotation(&property.type_text);
            definition.value = self.declaration_initializer(property);
        }
        true
    }

    fn annotation_import(&mut self, statement: &mut Statement<'a>) -> bool {
        let Statement::ImportDeclaration(import) = statement else {
            return true;
        };
        let Some(specifiers) = &mut import.specifiers else {
            return true;
        };
        let had_bindings = !specifiers.is_empty();
        specifiers.retain_mut(|specifier| {
            let ImportDeclarationSpecifier::ImportSpecifier(named) = specifier else {
                return true;
            };
            match self.facts.annotation_import(named.span) {
                Some(AnnotationImportDisposition::Remove) => false,
                Some(AnnotationImportDisposition::Rename) => {
                    let unaliased = named.imported.name() == named.local.name;
                    self.prefix_binding(&mut named.local);
                    if unaliased {
                        if let ModuleExportName::IdentifierName(imported) = &mut named.imported {
                            self.prefix_identifier_name(imported);
                        }
                    }
                    true
                }
                None => true,
            }
        });
        // OH drops an import whose only clause was removed. Existing side-effect
        // imports have no bindings and remain unchanged.
        !had_bindings || !specifiers.is_empty()
    }

    fn declaration_initializer(&mut self, property: &AnnotationProperty) -> Option<Expression<'a>> {
        if let Some(initializer) = &property.initializer {
            return self.constant_expression(initializer, property, property.array_depth, true);
        }
        self.enum_default(property, property.array_depth)
    }

    fn annotation_object(&mut self, annotation: &AnnotationUse) -> Option<Expression<'a>> {
        let mut properties =
            ArenaVec::with_capacity_in(annotation.info.properties.len(), &self.ast);
        for property in &annotation.info.properties {
            let value = if let Some(argument) = &property.argument {
                self.constant_expression(argument, property, property.array_depth, true)
            } else if let Some(initializer) = &property.initializer {
                self.constant_expression(initializer, property, property.array_depth, true)
            } else {
                self.enum_default(property, property.array_depth)
            };
            let Some(value) = value else {
                self.fail(format!(
                    "annotation property '{}' has neither a checked argument nor an upstream default",
                    property.name
                ));
                return None;
            };
            properties.push(ObjectPropertyKind::new_object_property(
                SPAN,
                PropertyKind::Init,
                self.property_key(property),
                value,
                false,
                false,
                false,
                &self.ast,
            ));
        }
        Some(Expression::new_object_expression(
            SPAN, properties, &self.ast,
        ))
    }

    fn constant_expression(
        &mut self,
        constant: &AnnotationConstant,
        property: &AnnotationProperty,
        array_depth: usize,
        evaluated_empty_array: bool,
    ) -> Option<Expression<'a>> {
        match constant {
            AnnotationConstant::Number(text) => Some(self.number_expression(text)),
            AnnotationConstant::String(value) => Some(Expression::new_string_literal(
                SPAN,
                Str::from_in(value.as_str(), self.allocator),
                None,
                &self.ast,
            )),
            AnnotationConstant::Boolean(value) => {
                Some(Expression::new_boolean_literal(SPAN, *value, &self.ast))
            }
            AnnotationConstant::Array(items) if items.is_empty() => {
                self.empty_array_expression(property, array_depth, evaluated_empty_array)
            }
            AnnotationConstant::Array(items) => {
                let next_depth = array_depth.saturating_sub(1);
                let mut elements = ArenaVec::with_capacity_in(items.len(), &self.ast);
                for item in items {
                    elements.push(ArrayExpressionElement::from(self.constant_expression(
                        item,
                        property,
                        next_depth,
                        evaluated_empty_array,
                    )?));
                }
                Some(Expression::new_array_expression(SPAN, elements, &self.ast))
            }
        }
    }

    fn empty_array_expression(
        &mut self,
        property: &AnnotationProperty,
        array_depth: usize,
        evaluated: bool,
    ) -> Option<Expression<'a>> {
        if array_depth > 1 {
            let inner = self.empty_array_expression(property, array_depth - 1, evaluated)?;
            return Some(Expression::new_array_expression(
                SPAN,
                [ArrayExpressionElement::from(inner)],
                &self.ast,
            ));
        }
        let type_argument = self.type_node(&property.element_type_text)?;
        let type_arguments = TSTypeParameterInstantiation::boxed(
            SPAN,
            ArenaVec::from_value_in(type_argument, &self.ast),
            &self.ast,
        );
        let mut arguments = ArenaVec::new_in(&self.ast);
        if let Some(first) = &property.enum_first_value {
            let indicator = match first {
                AnnotationConstant::Number(_) => {
                    if evaluated {
                        1.0
                    } else {
                        0.0
                    }
                }
                AnnotationConstant::String(_) => {
                    if evaluated {
                        3.0
                    } else {
                        2.0
                    }
                }
                _ => {
                    self.fail("native TypeScript returned a non-scalar first enum value");
                    return None;
                }
            };
            arguments.push(Argument::from(Expression::new_numeric_literal(
                SPAN,
                indicator,
                None,
                NumberBase::Decimal,
                &self.ast,
            )));
        }
        Some(Expression::new_new_expression(
            SPAN,
            Expression::new_identifier(SPAN, "Array", &self.ast),
            Some(type_arguments),
            arguments,
            &self.ast,
        ))
    }

    fn enum_default(
        &mut self,
        property: &AnnotationProperty,
        array_depth: usize,
    ) -> Option<Expression<'a>> {
        let first = property.enum_first_value.as_ref()?;
        if array_depth != 0 {
            return self.empty_array_expression(property, array_depth, false);
        }
        let AnnotationConstant::Number(value) = first else {
            return None;
        };
        let number = Expression::new_new_expression(
            SPAN,
            Expression::new_identifier(SPAN, "Number", &self.ast),
            None,
            [Argument::from(self.number_expression(value))],
            &self.ast,
        );
        let number_type = self.type_node("number")?;
        Some(Expression::new_ts_as_expression(
            SPAN,
            number,
            number_type,
            &self.ast,
        ))
    }

    fn number_expression(&self, text: &str) -> Expression<'a> {
        match text {
            "NaN" | "Infinity" => {
                Expression::new_identifier(SPAN, Ident::from_in(text, self.allocator), &self.ast)
            }
            "-Infinity" => Expression::new_unary_expression(
                SPAN,
                UnaryOperator::UnaryNegation,
                Expression::new_identifier(SPAN, "Infinity", &self.ast),
                &self.ast,
            ),
            _ => Expression::new_numeric_literal(
                SPAN,
                text.parse::<f64>().unwrap_or(f64::NAN),
                Some(Str::from_in(text, self.allocator)),
                NumberBase::Decimal,
                &self.ast,
            ),
        }
    }

    fn property_key(&self, property: &AnnotationProperty) -> PropertyKey<'a> {
        let source = property.name_text.as_str();
        if source.starts_with('\'') || source.starts_with('"') {
            return PropertyKey::new_string_literal(
                SPAN,
                Str::from_in(property.name.as_str(), self.allocator),
                Some(Str::from_in(source, self.allocator)),
                &self.ast,
            );
        }
        PropertyKey::new_static_identifier(
            SPAN,
            Ident::from_in(property.name.as_str(), self.allocator),
            &self.ast,
        )
    }

    fn type_annotation(
        &mut self,
        text: &str,
    ) -> Option<oxc_allocator::Box<'a, TSTypeAnnotation<'a>>> {
        self.type_node(text)
            .map(|ty| TSTypeAnnotation::boxed(SPAN, ty, &self.ast))
    }

    fn type_node(&mut self, text: &str) -> Option<TSType<'a>> {
        let source = format!("type __ArkdownAnnotationType = {text};");
        let parsed = Parser::new(self.allocator, &source, SourceType::ts()).parse();
        if !parsed.diagnostics.is_empty() {
            self.fail(format!(
                "native TypeScript returned an unparseable annotation type '{text}'"
            ));
            return None;
        }
        let Some(Statement::TSTypeAliasDeclaration(alias)) = parsed.program.body.first() else {
            self.fail(format!(
                "native TypeScript returned an invalid annotation type '{text}'"
            ));
            return None;
        };
        Some(alias.type_annotation.clone_in(self.allocator))
    }

    fn prefix_expression(&self, expression: &mut Expression<'a>) {
        match expression {
            Expression::Identifier(identifier) => self.prefix_identifier_reference(identifier),
            Expression::StaticMemberExpression(member) => {
                self.prefix_identifier_name(&mut member.property)
            }
            Expression::CallExpression(call) => self.prefix_expression(&mut call.callee),
            _ => {}
        }
    }

    fn prefix_binding(&self, identifier: &mut BindingIdentifier<'a>) {
        identifier.name = self.prefixed(identifier.name.as_str());
    }

    fn prefix_identifier_reference(&self, identifier: &mut IdentifierReference<'a>) {
        identifier.name = self.prefixed(identifier.name.as_str());
    }

    fn prefix_identifier_name(&self, identifier: &mut IdentifierName<'a>) {
        identifier.name = self.prefixed(identifier.name.as_str());
    }

    fn prefixed(&self, name: &str) -> Ident<'a> {
        Ident::from_in(format!("{ANNOTATION_PREFIX}{name}"), self.allocator)
    }

    fn fail(&mut self, message: impl Into<String>) {
        if self.failure.is_none() {
            self.failure = Some(message.into());
        }
    }
}

impl<'a> VisitMut<'a> for AnnotationLowerer<'a, '_> {
    fn visit_decorators(&mut self, decorators: &mut ArenaVec<'a, Decorator<'a>>) {
        decorators.retain_mut(|decorator| {
            let Some(annotation) = self.facts.annotation_use(decorator.span).cloned() else {
                return true;
            };
            if annotation.info.source_retention || !annotation.runtime_retained {
                return false;
            }
            let Some(object) = self.annotation_object(&annotation) else {
                return true;
            };
            let (mut callee, type_arguments) = match &decorator.expression {
                Expression::CallExpression(call) => (
                    call.callee.clone_in(self.allocator),
                    call.type_arguments.clone_in(self.allocator),
                ),
                expression => (expression.clone_in(self.allocator), None),
            };
            self.prefix_expression(&mut callee);
            decorator.expression = Expression::new_call_expression(
                decorator.span,
                callee,
                type_arguments,
                [Argument::from(object)],
                false,
                &self.ast,
            );
            true
        });
        walk_mut::walk_decorators(self, decorators);
    }
}
