use std::sync::Arc;

use arkdown_core::ArktsLanguageVersion;
use oxc_allocator::Allocator;

use crate::error::EtsTransformError;
use crate::source::CompileArktsSourceType;
use crate::transform::api_available::ApiAvailableTransformOptions;
use crate::transform::component_meta::ComponentMetadata;
use crate::transform::legacy_extend::LegacyExtendSource;
use crate::transform::system_api::Arkts11SystemApiValidator;
use crate::transform::{
    CustomComponentSemanticInventory, ModuleExportIndex, SystemApiCheckOptions, TransformOptions,
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
    api_available: ApiAvailableTransformOptions,
    system_api: SystemApiCheckOptions,
    page_context: Option<ArktsSourcePageContext>,
    project_semantics: Option<ArktsProjectSemantics>,
}

impl ArktsSourceChecker {
    pub fn new(component_metadata: Arc<ComponentMetadata>) -> Self {
        Self {
            component_metadata,
            arkts_language_version: ArktsLanguageVersion::default(),
            api_available: ApiAvailableTransformOptions::default(),
            system_api: SystemApiCheckOptions::default(),
            page_context: None,
            project_semantics: None,
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

    pub fn with_api_available_context(
        mut self,
        compatible_sdk_version: &str,
        runtime_os: &str,
    ) -> Self {
        self.api_available =
            ApiAvailableTransformOptions::from_build_context(compatible_sdk_version, runtime_os);
        self.system_api = self
            .system_api
            .clone()
            .with_compatible_sdk_version(compatible_sdk_version)
            .with_runtime_os(runtime_os);
        self
    }

    pub fn with_system_api_context(mut self, context: SystemApiCheckOptions) -> Self {
        self.system_api = context;
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
        let parser_source = LegacyExtendSource::new(content, self.arkts_language_version);
        let source_type =
            CompileArktsSourceType::from_id_with_version(source_id, self.arkts_language_version);
        let parsed =
            self.component_metadata
                .parse_ets(&allocator, parser_source.as_str(), source_type);
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

        let custom_component_options = self.custom_component_options();
        let custom_component_inventory = CustomComponentSemanticInventory::collect(
            &parsed.program,
            source_id,
            &custom_component_options,
        );
        let mut checker = EtsChecker::new(&parsed.program, source_id)
            .with_component_metadata(&self.component_metadata)
            .with_custom_component_info(&custom_component_inventory.info);
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
        let mut report = checker.check();
        let (module_exports, sources, module_roots) = if let Some(project) = &self.project_semantics
        {
            (
                &project.module_exports,
                &project.sources,
                project.module_roots.as_slice(),
            )
        } else {
            static EMPTY_EXPORTS: std::sync::LazyLock<ModuleExportIndex> =
                std::sync::LazyLock::new(ModuleExportIndex::default);
            static EMPTY_SOURCES: std::sync::LazyLock<TransformSourceSnapshot> =
                std::sync::LazyLock::new(TransformSourceSnapshot::default);
            (
                &*EMPTY_EXPORTS,
                &*EMPTY_SOURCES,
                &[] as &[(String, std::path::PathBuf)],
            )
        };
        report.extend(
            Arkts11SystemApiValidator::new(
                source_id,
                content,
                self.api_available,
                &self.system_api.clone().with_ohos_test_source(
                    ArktsSourceCheckPath::new(std::path::Path::new(source_id)).is_ohos_test(),
                ),
                module_exports,
                sources,
                module_roots,
            )
            .diagnostics(&parsed.program),
        );
        report
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

struct ArktsSourceCheckPath<'a> {
    path: &'a std::path::Path,
}

impl<'a> ArktsSourceCheckPath<'a> {
    fn new(path: &'a std::path::Path) -> Self {
        Self { path }
    }

    fn is_ohos_test(&self) -> bool {
        self.path
            .components()
            .any(|component| component.as_os_str() == "ohosTest")
    }
}
