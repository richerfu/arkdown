use oxc_ast::ast::Program;
use oxc_semantic::SemanticBuilder;

use crate::error::EtsTransformError;

use super::api_available::{ApiAvailableTransformOptions, ApiAvailableValidator};
use super::available_annotation::AvailableAnnotationValidator;

/// Single semantic entry for dynamic ArkTS 1.1 system-API checks.
///
/// Symbol resolution is constructed at most once per source and shared by all
/// system-API rules. Future `@since`, permission and syscap validators attach
/// here instead of building private semantic snapshots in transform hooks.
pub(crate) struct Arkts11SystemApiValidator<'a> {
    source_id: &'a str,
    source: &'a str,
    options: ApiAvailableTransformOptions,
}

impl<'a> Arkts11SystemApiValidator<'a> {
    pub(crate) fn new(
        source_id: &'a str,
        source: &'a str,
        options: ApiAvailableTransformOptions,
    ) -> Self {
        Self {
            source_id,
            source,
            options,
        }
    }

    pub(crate) fn is_required(&self) -> bool {
        self.options.should_validate_source(self.source)
    }

    pub(crate) fn diagnostics<'program>(
        &self,
        program: &'program Program<'program>,
    ) -> Vec<EtsTransformError> {
        if !self.is_required() {
            return Vec::new();
        }
        let semantic = SemanticBuilder::new().build(program).semantic;
        let scoping = semantic.into_scoping();
        let mut diagnostics = Vec::new();
        if self.options.should_validate_annotation(self.source) {
            diagnostics.extend(
                AvailableAnnotationValidator::new(self.source_id, self.options)
                    .diagnostics_with_scoping(program, &scoping),
            );
        }
        if self.options.should_transform_source(self.source) {
            diagnostics.extend(
                ApiAvailableValidator::new(self.source_id, self.options)
                    .diagnostics_with_scoping(program, &scoping),
            );
        }
        diagnostics
    }
}
