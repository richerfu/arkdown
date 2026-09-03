use std::sync::Arc;

use arkdown_core::ArktsLanguageVersion;
use oxc_allocator::Allocator;

use crate::error::EtsTransformError;
use crate::source::CompileArktsSourceType;
use crate::transform::api_available::{ApiAvailableTransformOptions, ApiAvailableValidator};
use crate::transform::component_meta::ComponentMetadata;

use super::{EtsCheckReport, EtsChecker};

/// Standalone native source-check entry used outside Rolldown hooks.
///
/// It shares the SDK-derived OXC parser configuration with the production
/// transform and returns every source diagnostic in traversal order. Project
/// I/O and declaration inventory construction remain coordinator-owned.
pub struct ArktsSourceChecker {
    component_metadata: Arc<ComponentMetadata>,
    arkts_language_version: ArktsLanguageVersion,
    api_available: ApiAvailableTransformOptions,
    page_context: Option<ArktsSourcePageContext>,
}

impl ArktsSourceChecker {
    pub fn new(component_metadata: Arc<ComponentMetadata>) -> Self {
        Self {
            component_metadata,
            arkts_language_version: ArktsLanguageVersion::default(),
            api_available: ApiAvailableTransformOptions::default(),
            page_context: None,
        }
    }

    pub fn with_arkts_language_version(mut self, version: ArktsLanguageVersion) -> Self {
        self.arkts_language_version = version;
        self
    }

    pub fn with_api_available_context(
        mut self,
        compatible_sdk_version: &str,
        runtime_os: &str,
    ) -> Self {
        self.api_available =
            ApiAvailableTransformOptions::from_build_context(compatible_sdk_version, runtime_os);
        self
    }

    pub fn with_page_context(
        mut self,
        entry_source: bool,
        preview_mode: bool,
        check_entry: bool,
    ) -> Self {
        self.page_context = Some(ArktsSourcePageContext {
            entry_source,
            preview_mode,
            check_entry,
        });
        self
    }

    pub fn check(&self, source_id: &str, content: &str) -> EtsCheckReport {
        let allocator = Allocator::default();
        let source_type =
            CompileArktsSourceType::from_id_with_version(source_id, self.arkts_language_version);
        let parsed = self
            .component_metadata
            .parse_ets(&allocator, content, source_type);
        if !parsed.diagnostics.is_empty() {
            let mut report = EtsCheckReport::default();
            report.extend([EtsTransformError::Fatal {
                file: source_id.to_string(),
                message: format!("ArkUI parse failed: {:?}", parsed.diagnostics),
            }]);
            return report;
        }

        // ArkTS 1.2 is selected explicitly and is owned by the static OXC
        // frontend. Dynamic ArkTS 1.1 UI diagnostics must never leak into it.
        if self.arkts_language_version.is_static() {
            return EtsCheckReport::default();
        }

        let mut checker = EtsChecker::new(&parsed.program, source_id)
            .with_component_metadata(&self.component_metadata);
        if let Some(context) = self.page_context {
            checker = checker.with_page_context(
                context.entry_source,
                context.preview_mode,
                context.check_entry,
            );
        }
        let mut report = checker.check();
        if self.api_available.should_transform_source(content) {
            report.extend(
                ApiAvailableValidator::new(source_id, self.api_available)
                    .diagnostics(&parsed.program),
            );
        }
        report
    }
}

#[derive(Clone, Copy)]
struct ArktsSourcePageContext {
    entry_source: bool,
    preview_mode: bool,
    check_entry: bool,
}
