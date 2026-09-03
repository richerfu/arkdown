use rustc_hash::FxHashSet;

use arkdown_core::ArktsLanguageVersion;
use oxc_ast::ast::{CallExpression, Expression, Program};
use oxc_ast_visit::{walk, Visit};

use crate::error::EtsTransformError;

/// Validates `wrapBuilder` and `mutableBuilder` exactly where the source
/// `process_ui_syntax.ts` does: each factory takes a single identifier that
/// resolves to a visible `@Builder` function.
pub(super) struct BuilderFactoryValidator<'a> {
    builder_functions: &'a FxHashSet<String>,
    source_id: &'a str,
    language_version: ArktsLanguageVersion,
}

impl<'a> BuilderFactoryValidator<'a> {
    pub(super) fn new(
        builder_functions: &'a FxHashSet<String>,
        source_id: &'a str,
        language_version: ArktsLanguageVersion,
    ) -> Self {
        Self {
            builder_functions,
            source_id,
            language_version,
        }
    }

    pub(super) fn validate<'program>(
        &self,
        program: &Program<'program>,
    ) -> Result<(), EtsTransformError> {
        let mut visitor = BuilderFactoryCallVisitor {
            builder_functions: self.builder_functions,
            language_version: self.language_version,
            invalid_factory: None,
        };
        visitor.visit_program(program);
        match visitor.invalid_factory {
            Some(factory) => Err(EtsTransformError::Fatal {
                file: self.source_id.to_string(),
                message: match self.language_version {
                    ArktsLanguageVersion::V1_1 => {
                        format!("The {factory}'s parameter should be '@Builder' function.")
                    }
                    ArktsLanguageVersion::V1_2 => {
                        "The wrapBuilder's parameter should be @Builder function.".to_string()
                    }
                },
            }),
            None => Ok(()),
        }
    }
}

struct BuilderFactoryCallVisitor<'a> {
    builder_functions: &'a FxHashSet<String>,
    language_version: ArktsLanguageVersion,
    invalid_factory: Option<&'static str>,
}

impl BuilderFactoryCallVisitor<'_> {
    fn factory(&self, call: &CallExpression<'_>) -> Option<&'static str> {
        let Expression::Identifier(identifier) = &call.callee else {
            return None;
        };
        match identifier.name.as_str() {
            "wrapBuilder" => Some("wrapBuilder"),
            "mutableBuilder" if self.language_version == ArktsLanguageVersion::V1_1 => {
                Some("mutableBuilder")
            }
            _ => None,
        }
    }

    fn has_valid_argument(&self, call: &CallExpression<'_>) -> bool {
        if self.language_version == ArktsLanguageVersion::V1_2 {
            // `ui-syntax-plugins/rules/wrap-builder-check.ts` scans all
            // arguments and retains the last identifier name.
            return call
                .arguments
                .iter()
                .filter_map(ArgumentIdentifier::from_argument)
                .next_back()
                .is_some_and(|name| self.builder_functions.contains(name));
        }
        let Some(argument) = call.arguments.first() else {
            // The source only diagnoses an existing first argument.
            return true;
        };
        matches!(
            argument.as_expression(),
            Some(Expression::Identifier(identifier))
                if self.builder_functions.contains(identifier.name.as_str())
        )
    }
}

impl<'program> Visit<'program> for BuilderFactoryCallVisitor<'_> {
    fn visit_call_expression(&mut self, call: &CallExpression<'program>) {
        if self.invalid_factory.is_some() {
            return;
        }
        if let Some(factory) = self.factory(call) {
            if !self.has_valid_argument(call) {
                self.invalid_factory = Some(factory);
                return;
            }
        }
        walk::walk_call_expression(self, call);
    }
}

struct ArgumentIdentifier;

impl ArgumentIdentifier {
    fn from_argument<'a>(argument: &'a oxc_ast::ast::Argument<'_>) -> Option<&'a str> {
        match argument.as_expression() {
            Some(Expression::Identifier(identifier)) => Some(identifier.name.as_str()),
            _ => None,
        }
    }
}
