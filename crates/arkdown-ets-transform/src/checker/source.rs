use std::path::Path;
use std::sync::Arc;

use arkdown_core::ArktsLanguageVersion;
use oxc_allocator::Allocator;

use crate::error::EtsTransformError;
use crate::source::{ArktsSourceLanguageDetector, CompileArktsSourceType};
use crate::transform::component_meta::ComponentMetadata;
use crate::transform::legacy_extend::ArktsExtendSource;
use crate::transform::static_directive::StaticDirectiveSource;
use crate::transform::{
    ArktsTypeFactsSnapshot, CustomComponentSemanticInventory, ModuleExportIndex, TransformOptions,
};
use crate::TransformSourceSnapshot;

use super::{EtsCheckReport, EtsChecker};

/// Standalone native source-check entry used outside Rolldown hooks.
///
/// It shares the SDK-derived OXC parser configuration with the production
/// transform and returns every source diagnostic in traversal order. Project
/// I/O and declaration inventory construction remain coordinator-owned.
pub struct ArktsSourceChecker {
    component_metadata: Arc<ComponentMetadata>,
    arkts_language_version: ArktsLanguageVersion,
    page_context: Option<ArktsSourcePageContext>,
    project_semantics: Option<ArktsProjectSemantics>,
    type_facts: ArktsTypeFactsSnapshot,
}

impl ArktsSourceChecker {
    pub fn new(component_metadata: Arc<ComponentMetadata>) -> Self {
        Self {
            component_metadata,
            arkts_language_version: ArktsLanguageVersion::default(),
            page_context: None,
            project_semantics: None,
            type_facts: ArktsTypeFactsSnapshot::default(),
        }
    }

    pub fn with_arkts_language_version(mut self, version: ArktsLanguageVersion) -> Self {
        self.arkts_language_version = version;
        if let Some(project) = &mut self.project_semantics {
            project.module_exports = ModuleExportIndex::build_with_version(
                &project.sources,
                &project.module_roots,
                &self.component_metadata,
                version,
            );
        }
        self
    }

    /// Attach the immutable source graph used for cross-file ArkTS checks.
    ///
    /// The export closure is built once here and shared by every subsequent
    /// `check` call. The checker never reads files or reparses sibling modules
    /// from an individual source callback.
    pub fn with_project_sources(
        mut self,
        sources: TransformSourceSnapshot,
        module_roots: Vec<(String, std::path::PathBuf)>,
    ) -> Self {
        let module_exports = ModuleExportIndex::build_with_version(
            &sources,
            &module_roots,
            &self.component_metadata,
            self.arkts_language_version,
        );
        self.project_semantics = Some(ArktsProjectSemantics {
            sources,
            module_roots,
            module_exports,
        });
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

    pub fn with_type_facts(mut self, type_facts: ArktsTypeFactsSnapshot) -> Self {
        self.type_facts = type_facts;
        self
    }

    pub fn check(&self, source_id: &str, content: &str) -> EtsCheckReport {
        let allocator = Allocator::default();
        let parser_source = ArktsExtendSource::new(content, self.arkts_language_version);
        let path = Path::new(source_id);
        let source_language_version = ArktsSourceLanguageDetector::new(parser_source.as_str())
            .version_for_path(self.arkts_language_version, path);
        let static_directive_source = source_language_version
            .is_static()
            .then(|| StaticDirectiveSource::new(parser_source.as_str()));
        let parser_content = static_directive_source
            .as_ref()
            .map_or_else(|| parser_source.as_str(), StaticDirectiveSource::as_str);
        let source_type =
            CompileArktsSourceType::from_id_with_version(source_id, source_language_version);
        let mut parsed = self
            .component_metadata
            .parse_ets(&allocator, parser_content, source_type);
        if !parsed.diagnostics.is_empty() {
            let mut report = EtsCheckReport::default();
            report.extend([EtsTransformError::Fatal {
                file: source_id.to_string(),
                message: format!("ArkUI parse failed: {:?}", parsed.diagnostics),
            }]);
            return report;
        }
        if let Some(static_directive_source) = &static_directive_source {
            static_directive_source.restore(&mut parsed.program, &allocator);
        }

        // Static source files are owned by the static OXC/frontend path.
        // Dynamic files in the same 1.2 product still require the normal
        // ArkUI checks and lowering diagnostics.
        if source_language_version.is_static() {
            return EtsCheckReport::default();
        }

        let custom_component_options = self.custom_component_options();
        let custom_component_inventory = CustomComponentSemanticInventory::collect(
            &parsed.program,
            source_id,
            &custom_component_options,
        );
        let mut checker = EtsChecker::new(&parsed.program, source_id)
            .with_component_metadata(&self.component_metadata)
            .with_custom_component_info(&custom_component_inventory.info)
            .with_type_facts(self.type_facts.file_facts(source_id));
        if let Some(project) = &self.project_semantics {
            checker = checker.with_imported_class_observations(
                project.module_exports.imported_class_observations(
                    &parsed.program,
                    std::path::Path::new(source_id),
                    &project.sources,
                    &project.module_roots,
                ),
            );
        }
        if let Some(context) = self.page_context {
            checker = checker.with_page_context(
                context.entry_source,
                context.preview_mode,
                context.check_entry,
            );
        }
        checker.check()
    }

    fn custom_component_options(&self) -> TransformOptions {
        let mut options = TransformOptions::default()
            .with_component_metadata(Arc::clone(&self.component_metadata));
        options.arkts_language_version = self.arkts_language_version;
        if let Some(project) = &self.project_semantics {
            options.sources = project.sources.clone();
            options.module_roots.clone_from(&project.module_roots);
            options.module_exports = project.module_exports.clone();
        }
        options
    }
}

#[derive(Clone, Copy)]
struct ArktsSourcePageContext {
    entry_source: bool,
    preview_mode: bool,
    check_entry: bool,
}

struct ArktsProjectSemantics {
    sources: TransformSourceSnapshot,
    module_roots: Vec<(String, std::path::PathBuf)>,
    module_exports: ModuleExportIndex,
}
