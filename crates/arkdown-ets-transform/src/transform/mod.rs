pub(crate) mod api_available;
mod auto_lazy_import;
pub(crate) mod available_annotation;
pub(crate) use auto_lazy_import::apply as apply_auto_lazy_import;
mod build_fn;
mod builder_factory;
mod component;
mod component_analysis;
mod component_collection;
pub mod component_meta;
mod crossplatform_collection;
mod declaration_emit;
pub(crate) mod decorator;
mod entry;
mod expand_import_path;
mod global_rewrite;
mod insight_intent;
pub(crate) mod legacy_extend;
mod local_builder;
mod navigation;
pub mod source_emit;
mod source_text;
pub(crate) mod static_directive;
mod static_graph;
mod syntax;
pub(crate) mod system_api;
pub(crate) mod viewpu_emit;
mod viewpu_finalize;
mod viewv2;
pub(crate) use syntax::ArktsTransformSyntaxDetector;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rustc_hash::FxHashSet;

use arkdown_core::{
    ArktsLanguageVersion, AutoLazyFilter, ExpandImportPath, InsightIntentFormBinding, ModuleSource,
};
use arkdown_path::{classify::is_inside_oh_modules, ArtifactPathSerializer, HvigorModulePath};
use oxc_allocator::{Allocator, ArenaBox, ArenaVec, CloneIn, Vec as OxcVec};
use oxc_ast::ast::{
    Argument, BindingPattern, BindingRestElement, Class, ClassElement, Declaration, Decorator,
    Directive, ExportDefaultDeclarationKind, Expression, FormalParameter, FormalParameterKind,
    FormalParameterRest, FormalParameters, Function, FunctionBody, FunctionType, MethodDefinition,
    MethodDefinitionKind, MethodDefinitionType, ObjectPropertyKind, Program, PropertyDefinition,
    PropertyKey, Statement, StringLiteral, StructElement, StructStatement,
    TSNamespaceDeclarationBody, TSType,
};
use oxc_ast_visit::{walk_mut, VisitMut};
use oxc_codegen::{CodegenOptions, CommentOptions, IndentChar};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SPAN};

use crate::error::EtsTransformError;
use crate::source::{ArktsSourceLanguageDetector, CompileArktsSourceType, TransformSourceSnapshot};
use crate::transform::api_available::{ApiAvailableLowerer, ApiAvailableTransformOptions};
use crate::transform::build_fn::{
    BuildBodyInfo, BuildBodyPlan, BuildBodySharedInputs, ExtendBodyPlan, StylesMethodInfo,
};
pub(crate) use crate::transform::build_fn::{
    CustomComponentGeneration, CustomComponentInfo, CustomComponentPropertyKind,
    CustomComponentUpdateProps,
};
use crate::transform::builder_factory::BuilderFactoryValidator;
use crate::transform::component::{
    BuilderMethodInfo, BuilderMethodKind, ComponentInfo, ComponentRuntimeOptions,
    ComponentSourceMember, CustomDialogControllerLowerer, FieldExpression, GlobalReusePoolConfig,
    PageTransitionMethodInfo, ParamField, RegularField, RegularMember, StateField,
    ViewPuClassEmitter,
};
use crate::transform::component_analysis::{
    BuilderFunctionNameCollector, CustomComponentNameCollector, ExtendAttributeIndex,
    ImportedCustomComponentNamesCollector, LocalBuilderFileInventory,
    PreviewComponentNameCollector, SimpleTypeContext, StructBuilderMethodNames,
    StructBuilderParamNames, StructMutableBuilderProperties, StructRegularAttributeProperties,
};
use crate::transform::component_collection::ComponentNameCollector;
use crate::transform::component_meta::ComponentMetadata;
use crate::transform::declaration_emit::{ParamsInterfaceEmitter, TopLevelFunctionEmitter};
use crate::transform::decorator::{ArkuiDecorator, ArkuiDecoratorName};
use crate::transform::entry::{
    EntryOptions, EntryRegistrationBlock, EntryRegistrationInfo, PreviewRegistrationBlock,
};
use crate::transform::global_rewrite::{
    EsObjectTypeRewriter, GlobalContextCallRewriter, WorkerPathRewriter,
};
pub use crate::transform::insight_intent::{
    InsightIntentDeclaration, InsightIntentEntityDeclaration, InsightIntentSchemaCatalog,
    InsightIntentSourceMetadata,
};
use crate::transform::insight_intent::{
    InsightIntentDecoratorProcessor, InsightIntentProcessorOptions,
};
use crate::transform::navigation::NavigationBuilderRegistrationBlock;
use crate::transform::source_emit::HvigorTypescriptPrinter;
use crate::transform::source_text::ViewpuOutputText;
use crate::transform::static_directive::StaticDirectiveSource;
use crate::transform::static_graph::StaticArktsGraphProjection;
use crate::transform::system_api::Arkts11SystemApiValidator;
pub use crate::transform::system_api::SystemApiCheckOptions;
use crate::transform::viewpu_emit::GeneratedViewpuName;
use crate::transform::viewpu_finalize::ViewpuProgramFinalizer;
use crate::transform::viewv2::ComponentV2Lowerer;
pub(crate) use component_analysis::{
    ClassObservation, CustomComponentSemanticInventory, ImportedClassObservations,
    ImportedInsightIntentEntities, ModuleExportIndex,
};
pub use crossplatform_collection::CrossplatformSourceCollection;
pub use navigation::{NavigationRoute, NavigationRouteMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckerMode {
    Oxc,
    Off,
}

#[derive(Debug, Clone)]
pub struct TransformOptions {
    /// Explicit build-wide ArkTS language. The default remains ArkTS 1.1.
    pub arkts_language_version: ArktsLanguageVersion,
    pub(crate) api_available: ApiAvailableTransformOptions,
    pub system_api: SystemApiCheckOptions,
    pub bundle_name: Option<String>,
    pub module_name: Option<String>,
    pub project_root: Option<PathBuf>,
    /// Hvigor `projectConfig.projectPath` / `aceModuleRoot`.
    ///
    /// For ETS compilation this is `<module>/src/main/ets`. Entry route
    /// metadata is relative to this path, not to each source package's own
    /// `src/main/ets` directory.
    pub project_path: Option<PathBuf>,
    pub module_roots: Vec<(String, PathBuf)>,
    pub component_metadata: Arc<ComponentMetadata>,
    pub sources: TransformSourceSnapshot,
    pub(crate) module_exports: ModuleExportIndex,
    /// Source-indexed `routerMap` data supplied by the build coordinator.
    ///
    /// The ETS hook only looks this up; profile files are never read during a
    /// Rolldown transform callback.
    pub navigation_routes: NavigationRouteMap,
    /// Whether this source was declared in a referenced form profile.
    ///
    /// Mirrors SDK `projectConfig.cardObj[resourceFileName]`.
    pub form_entry: bool,
    /// Form-extension ownership for `@InsightIntentForm`. This is distinct
    /// from `form_entry`, which identifies ArkUI widget/card sources.
    pub insight_intent_form_binding: Option<InsightIntentFormBinding>,
    /// Built-in schema catalog from `<ets-loader>/insight_intents/schema`.
    pub insight_intent_schema_catalog: InsightIntentSchemaCatalog,
    /// Whether Rolldown loaded this source through an entry query. This is the
    /// Rust equivalent of the source compiler's `fileQuery === '?entry'`.
    pub entry_source: bool,
    /// Source compiler preview mode (`projectConfig.isPreview`).
    pub preview_mode: bool,
    /// Preview's `projectConfig.checkEntry` override. Kept explicit because it
    /// changes entry/preview validation independently of preview mode.
    pub check_entry: bool,
    /// Whether Stage model partial update is enabled (API >= 9).
    ///
    /// Defaults to `true`; set to `false` only for FA model / JSBUNDLE builds,
    /// which `validate_compile_mode()` in `arkdown-profile` rejects anyway.
    pub partial_update: bool,
    pub checker_mode: CheckerMode,
    /// `arkOptions.autoLazyImport`: rewrite value imports into
    /// `import lazy` declarations (SDK `process_lazy_import.js::transformLazyImport`).
    pub auto_lazy_import: bool,
    /// `arkOptions.autoLazyFilter` include/exclude lists keyed on the current
    /// module's package name (`isNotAutoLazyImport`).
    pub auto_lazy_filter: Option<AutoLazyFilter>,
    /// Package name of the module owning the transformed file (SDK
    /// `meta.pkgName` used by `isNotAutoLazyImport`).
    pub pkg_name: Option<String>,
    /// `arkOptions.expandImportPath` — package-level import expansion
    /// (SDK `import_path_expand.js`). Only effective for non-HAR modules.
    pub expand_import_path: Option<ExpandImportPath>,
    /// Package names present in `hspNameOhmMap`/`harNameOhmMap` — hvigor's
    /// `import_path_expand.js` never expands remote HSP/HAR packages
    /// (`a2[i]` guard); local `file:` dependencies are not in the maps and
    /// do expand.
    pub hsp_har_package_names: FxHashSet<String>,
    /// The transformed AST is owned by the ESMODULE `ModuleSourceFile` stage.
    /// That stage rebuilds resolved module-request declaration shells before
    /// the single TypeScript printer pass; standalone transform callers do not.
    pub(crate) module_source_file_stage: bool,
    /// SDK `process_mock.disableMockDecorator`: `mockParams` is present for
    /// every Ark compile, so only Preview builds retain `@MockSetup`.
    pub disable_mock_setup_decorator: bool,
}

impl Default for TransformOptions {
    fn default() -> Self {
        Self {
            arkts_language_version: ArktsLanguageVersion::default(),
            api_available: ApiAvailableTransformOptions::default(),
            system_api: SystemApiCheckOptions::default(),
            bundle_name: None,
            module_name: None,
            project_root: None,
            project_path: None,
            module_roots: Vec::new(),
            component_metadata: Arc::new(ComponentMetadata::default()),
            sources: TransformSourceSnapshot::default(),
            module_exports: ModuleExportIndex::default(),
            navigation_routes: NavigationRouteMap::default(),
            form_entry: false,
            insight_intent_form_binding: None,
            insight_intent_schema_catalog: InsightIntentSchemaCatalog::default(),
            entry_source: false,
            preview_mode: false,
            check_entry: false,
            partial_update: true,
            checker_mode: CheckerMode::Oxc,
            auto_lazy_import: false,
            auto_lazy_filter: None,
            pkg_name: None,
            expand_import_path: None,
            hsp_har_package_names: FxHashSet::default(),
            module_source_file_stage: false,
            disable_mock_setup_decorator: false,
        }
    }
}

impl TransformOptions {
    pub fn with_arkts_language_version(mut self, version: ArktsLanguageVersion) -> Self {
        self.arkts_language_version = version;
        self
    }

    pub fn with_component_metadata(mut self, metadata: Arc<ComponentMetadata>) -> Self {
        self.component_metadata = metadata;
        self
    }

    /// Whether Stage model partial update is enabled (API >= 9).
    pub fn supports_partial_update(&self) -> bool {
        self.partial_update
    }

    fn source_language_version(&self, source_id: &str, source: &str) -> ArktsLanguageVersion {
        if !self.arkts_language_version.is_static() {
            return self.arkts_language_version;
        }
        let path = Path::new(source_id);
        ArktsSourceLanguageDetector::new(source).version_for_path(self.arkts_language_version, path)
    }
}

pub struct ArkuiComponentNameCollector<'a> {
    content: &'a str,
    id: &'a str,
    options: Option<&'a TransformOptions>,
}

impl<'a> ArkuiComponentNameCollector<'a> {
    pub fn new(content: &'a str, id: &'a str) -> Self {
        Self {
            content,
            id,
            options: None,
        }
    }

    pub fn with_options(content: &'a str, id: &'a str, options: &'a TransformOptions) -> Self {
        Self {
            content,
            id,
            options: Some(options),
        }
    }

    /// Collect ArkUI component names from an ETS file using OXC's ArkUI AST nodes.
    ///
    /// Hvigor writes this information to `component_collection.json` during the
    /// widget CompileArkTS pass. The collection is based on parsed ArkUI
    /// component expressions rather than source-text matching.
    pub fn collect(&self) -> Result<Vec<String>, EtsTransformError> {
        let allocator = Allocator::default();
        let arkts_language_version = self
            .options
            .map(|options| options.arkts_language_version)
            .unwrap_or_default();
        let parser_source =
            legacy_extend::LegacyExtendSource::new(self.content, arkts_language_version);
        let source_type =
            CompileArktsSourceType::from_id_with_version(self.id, arkts_language_version);
        let parsed = match self.options {
            Some(options) => options.component_metadata.parse_ets(
                &allocator,
                parser_source.as_str(),
                source_type,
            ),
            None => Parser::new(&allocator, parser_source.as_str(), source_type).parse(),
        };
        if !parsed.diagnostics.is_empty() {
            return Err(EtsTransformError::Fatal {
                file: self.id.to_string(),
                message: format!("ArkUI parse failed: {:?}", parsed.diagnostics),
            });
        }

        let mut custom_components = FxHashSet::default();
        let mut component_collector = CustomComponentNameCollector::new(&mut custom_components);
        for statement in &parsed.program.body {
            component_collector.collect_statement(statement);
        }
        if let Some(options) = self.options {
            ImportedCustomComponentNamesCollector::new(
                &parsed.program,
                self.id,
                options,
                &mut custom_components,
            )
            .collect();
        }
        Ok(ComponentNameCollector::new(custom_components).collect_program(&parsed.program))
    }
}

pub struct ArktsTransformer<'a> {
    source: &'a ModuleSource,
    options: ArktsTransformerOptions<'a>,
}

pub struct ArktsTransformOutput {
    pub code: String,
    pub source_map: Option<String>,
    pub(crate) rolldown_projection: Option<String>,
    /// Per-source InsightIntent facts consumed by the build-end profile
    /// processor. Transform hooks never write profile or cache files.
    pub insight_intent: InsightIntentSourceMetadata,
    /// ArkUI-X module/component facts collected from the original OXC AST and
    /// used-system-API declarations.
    pub crossplatform_collection: CrossplatformSourceCollection,
}

enum ArktsTransformerOptions<'a> {
    Owned(Box<TransformOptions>),
    Borrowed(&'a TransformOptions),
}

impl ArktsTransformerOptions<'_> {
    fn as_ref(&self) -> &TransformOptions {
        match self {
            Self::Owned(options) => options.as_ref(),
            Self::Borrowed(options) => options,
        }
    }
}

impl<'a> ArktsTransformer<'a> {
    /// Transform an ArkTS `.ets` source file into TypeScript.
    ///
    /// ArkUI parsing is handled by the workspace-pinned ohos-rs/oxc fork using
    /// `SourceType::ets()` for the default language and
    /// `SourceType::ets_static()` for explicit ArkTS 1.2 plans.
    pub fn new(source: &'a ModuleSource) -> Self {
        Self {
            source,
            options: ArktsTransformerOptions::Owned(Box::default()),
        }
    }

    pub fn with_options(source: &'a ModuleSource, options: &'a TransformOptions) -> Self {
        Self {
            source,
            options: ArktsTransformerOptions::Borrowed(options),
        }
    }

    pub fn transform(&self) -> Result<String, EtsTransformError> {
        self.transform_with_source_map().map(|output| output.code)
    }

    pub fn transform_with_source_map(&self) -> Result<ArktsTransformOutput, EtsTransformError> {
        let source = self.source;
        let options = self.options.as_ref();
        let parser_source = legacy_extend::LegacyExtendSource::new(
            source.content.as_str(),
            options.arkts_language_version,
        );
        let source_language_version =
            options.source_language_version(&source.id.id, parser_source.as_str());
        let static_directive_source = source_language_version
            .is_static()
            .then(|| StaticDirectiveSource::new(parser_source.as_str()));
        let content = static_directive_source
            .as_ref()
            .map_or_else(|| parser_source.as_str(), StaticDirectiveSource::as_str);

        let allocator = Allocator::default();
        let source_type =
            CompileArktsSourceType::from_id_with_version(&source.id.id, source_language_version);
        let mut parsed = options
            .component_metadata
            .parse_ets(&allocator, content, source_type);
        if !parsed.diagnostics.is_empty() {
            return Err(EtsTransformError::Fatal {
                file: source.id.id.clone(),
                message: format!("ArkUI parse failed: {:?}", parsed.diagnostics),
            });
        }
        if let Some(static_directive_source) = &static_directive_source {
            static_directive_source.restore(&mut parsed.program, &allocator);
        }

        if source_language_version.is_static() {
            return self.transform_static(&allocator, &mut parsed.program);
        }

        let mock_setup_decorators_lowered = if options.disable_mock_setup_decorator {
            MockSetupDecoratorLowerer::new().program(&mut parsed.program)
        } else {
            false
        };
        let needs_api_available = options.api_available.should_transform_source(content);
        let system_api_validator = Arkts11SystemApiValidator::new(
            &source.id.id,
            content,
            options.api_available,
            &options.system_api,
            &options.module_exports,
            &options.sources,
            &options.module_roots,
        );
        let needs_system_api_validation = system_api_validator.is_required();
        let mut crossplatform_collection = if options.system_api.collects_crossplatform() {
            CrossplatformSourceCollection::from_program(
                &parsed.program,
                options.system_api.module_prefixes(),
                &options.component_metadata,
            )
        } else {
            CrossplatformSourceCollection::default()
        };
        let syntax_detector = ArktsTransformSyntaxDetector::new();
        if !mock_setup_decorators_lowered
            && !needs_api_available
            && !needs_system_api_validation
            && !options.system_api.collects_crossplatform()
            && !syntax_detector.program(&parsed.program)
        {
            return Ok(ArktsTransformOutput {
                code: source.content.clone(),
                source_map: None,
                rolldown_projection: None,
                insight_intent: InsightIntentSourceMetadata::default(),
                crossplatform_collection,
            });
        }

        let custom_component_inventory =
            CustomComponentSemanticInventory::collect(&parsed.program, &source.id.id, options);
        let mut custom_components = custom_component_inventory.names;
        let mut custom_component_update_props = custom_component_inventory.info;

        let system_api_validation =
            needs_system_api_validation.then(|| system_api_validator.validate(&parsed.program));
        if let Some(validation) = &system_api_validation {
            crossplatform_collection.merge(&validation.crossplatform_dependencies);
        }

        if matches!(options.checker_mode, CheckerMode::Oxc) {
            if let Some(validation) = &system_api_validation {
                let diagnostics = &validation.diagnostics;
                for diagnostic in diagnostics {
                    tracing::debug!(file = %source.id.id, diagnostic = ?diagnostic, "system API diagnostic");
                }
                if let Some(error) = diagnostics.iter().find(|diagnostic| diagnostic.is_error()) {
                    return Err(error.clone());
                }
            }
            let report = crate::checker::EtsChecker::new(&parsed.program, &source.id.id)
                .with_component_metadata(&options.component_metadata)
                .with_custom_component_info(&custom_component_update_props)
                .with_imported_class_observations(
                    options.module_exports.imported_class_observations(
                        &parsed.program,
                        Path::new(&source.id.id),
                        &options.sources,
                        &options.module_roots,
                    ),
                )
                .with_page_context(
                    options.entry_source,
                    options.preview_mode,
                    options.check_entry,
                )
                .check();
            for diag in report.diagnostics() {
                tracing::debug!(file = %source.id.id, diagnostic = ?diag, "ets checker diagnostic");
            }
            let demote_linter_errors = is_inside_oh_modules(Path::new(&source.id.id));
            if !demote_linter_errors {
                if let Some(error) = report.first_error() {
                    return Err(error.clone());
                }
            }
        }

        let imported_insight_intent_entities =
            options.module_exports.imported_insight_intent_entities(
                &parsed.program,
                Path::new(&source.id.id),
                &options.sources,
                &options.module_roots,
            );
        let insight_intent = InsightIntentDecoratorProcessor::new(
            &parsed.program,
            InsightIntentProcessorOptions {
                source_id: &source.id.id,
                source_text: content,
                bundle_name: options.bundle_name.as_deref().unwrap_or_default(),
                module_name: options.module_name.as_deref().unwrap_or_default(),
                form_binding: options.insight_intent_form_binding.clone(),
                schema_catalog: options.insight_intent_schema_catalog.clone(),
                imported_entity_types: imported_insight_intent_entities,
            },
        )
        .program(&mut parsed.program)?;

        let simple_type_context =
            SimpleTypeContext::from_program(&parsed.program, &source.id.id, options);
        let hvigor_source_id = TransformSourcePath::new(&source.id.id).hvigor_source_path(options);
        let builder_function_names =
            BuilderFunctionNameCollector::new(&parsed.program, &source.id.id, options).collect();
        let local_builder_functions = builder_function_names.local;
        let builder_functions = builder_function_names.all;
        let validator_builder_functions = match options.arkts_language_version {
            ArktsLanguageVersion::V1_1 => &builder_functions,
            ArktsLanguageVersion::V1_2 => &local_builder_functions,
        };
        BuilderFactoryValidator::new(
            validator_builder_functions,
            &source.id.id,
            options.arkts_language_version,
        )
        .validate(&parsed.program)?;
        let navigation_registrations = NavigationBuilderRegistrationBlock::for_source(
            options.navigation_routes.routes_for_source(&source.id.id),
            &local_builder_functions,
            &source.id.id,
        )?;
        // SDK process_component_build.js::getComponentType only reaches the
        // custom-component branch when the symbol is present in
        // validate_ui_syntax.customComponents. Arkdown's unresolved-package
        // usage fallback is deliberately broader, so exclude symbols whose
        // source/re-export chain has already proven them to be @Builder
        // functions. Struct @Builder methods are collected later and are not
        // removed here; an unqualified call still resolves to a real component
        // when a local builder method has the same name.
        custom_components.retain(|name| !builder_functions.contains(name));
        custom_component_update_props.retain(|name, _| !builder_functions.contains(name));
        let custom_dialog_components = custom_component_update_props
            .iter()
            .filter(|(_, info)| info.is_custom_dialog)
            .map(|(name, _)| name.clone())
            .collect::<FxHashSet<_>>();
        let extend_attributes = Arc::new(ExtendAttributeIndex::from_program(&parsed.program));
        let global_styles =
            TopLevelStylesInventory::new(&parsed.program, &source.id.id).collect(&allocator)?;
        let preview_components = PreviewComponentNameCollector::new(&parsed.program).collect();
        let has_local_builder_in_file =
            LocalBuilderFileInventory::new(&parsed.program).has_local_builder();
        let preview_runtime = options.preview_mode
            && options.arkts_language_version == ArktsLanguageVersion::V1_1
            && !preview_components.is_empty();

        let statement_transformer =
            ArktsStatementTransformer::new(ArktsStatementTransformerInput {
                allocator: &allocator,
                source_text: content,
                id: &source.id.id,
                options,
                simple_type_context: &simple_type_context,
                custom_components: &custom_components,
                custom_component_update_props: &custom_component_update_props,
                builder_functions: &builder_functions,
                hvigor_source_id: &hvigor_source_id,
                extend_attributes,
                global_styles,
                has_local_builder_in_file,
            });
        let concurrent_directives_lowered =
            ConcurrentDirectiveLowerer::new(&allocator).program(&mut parsed.program);
        let global_context_calls_lowered =
            GlobalContextCallRewriter::new(&allocator).program(&mut parsed.program);
        let worker_paths_lowered = WorkerPathRewriter::new(&allocator).program(&mut parsed.program);
        let es_object_types_lowered =
            EsObjectTypeRewriter::new(&allocator).program(&mut parsed.program);
        let custom_dialog_controllers_lowered = CustomDialogControllerLowerer::new(
            &allocator,
            options.component_metadata.runtime_names(),
            &custom_dialog_components,
            &custom_component_update_props,
            content,
            &hvigor_source_id,
        )
        .program(&mut parsed.program);
        let api_available_lowered = needs_api_available
            && ApiAvailableLowerer::new(&allocator, options.api_available)
                .program(&mut parsed.program);
        let mut entry_registrations = EntryRegistrationBlock::default();
        let mut preview_registrations =
            preview_runtime.then(|| PreviewRegistrationBlock::new(preview_components));
        let mut reusable_v2_helper_emitted = false;
        let replacement_plans = parsed
            .program
            .body
            .iter()
            .map(|statement| {
                // V2 dispatch: @ComponentV2 structs are lowered through ComponentV2Lowerer,
                // not the V1 ViewPU path.
                let emit_reusable_v2_helper = !reusable_v2_helper_emitted
                    && statement_transformer.is_reusable_v2_component(statement);
                if let Some(replacement) =
                    statement_transformer.component_v2(statement, emit_reusable_v2_helper)?
                {
                    if let Some((name, info)) =
                        statement_transformer.component_v2_entry_registration(statement)?
                    {
                        if let Some(preview) = &mut preview_registrations {
                            preview.set_entry(&name, Some(&info));
                        } else {
                            entry_registrations.push(&name, Some(&info));
                        }
                    }
                    reusable_v2_helper_emitted |= emit_reusable_v2_helper;
                    Ok(Some(StatementReplacementPlan::AstStatements(replacement)))
                } else if let Some(component) = statement_transformer.component(statement)? {
                    if component.is_entry {
                        if let Some(preview) = &mut preview_registrations {
                            preview
                                .set_entry(&component.name, component.entry_registration.as_ref());
                        } else {
                            entry_registrations
                                .push(&component.name, component.entry_registration.as_ref());
                        }
                    }
                    let statements = ViewPuClassEmitter::new(
                        &component,
                        options.component_metadata.runtime_names(),
                    )
                    .statements(&allocator)
                    .map_err(|error| {
                        error.into_transform_error(&source.id.id, "unsupported ArkUI build() DSL")
                    })?;
                    Ok(Some(StatementReplacementPlan::AstStatements(statements)))
                } else {
                    statement_transformer.non_component_plan(statement)
                }
            })
            .collect::<Result<Vec<_>, EtsTransformError>>()?;

        if mock_setup_decorators_lowered
            || concurrent_directives_lowered
            || global_context_calls_lowered
            || worker_paths_lowered
            || es_object_types_lowered
            || custom_dialog_controllers_lowered
            || api_available_lowered
            || insight_intent.changed
            || !navigation_registrations.is_empty()
            || replacement_plans.iter().any(Option::is_some)
        {
            let entry_registration_statements = entry_registrations.statements(
                oxc_ast::builder::AstBuilder::new(&allocator),
                &allocator,
                options.component_metadata.runtime_names(),
            );
            drop(entry_registrations);
            let preview_registration_statement = preview_registrations.map(|preview| {
                preview.statement(
                    oxc_ast::builder::AstBuilder::new(&allocator),
                    &allocator,
                    options.component_metadata.runtime_names(),
                )
            });

            let mut body = OxcVec::new_in(&&allocator);
            let old_body = std::mem::replace(&mut parsed.program.body, OxcVec::new_in(&&allocator));
            for (statement, replacement_plan) in old_body.into_iter().zip(replacement_plans) {
                match replacement_plan {
                    Some(StatementReplacementPlan::AstStatements(statements)) => {
                        body.extend(statements);
                    }
                    Some(StatementReplacementPlan::LowerSendableClass) => {
                        let statements =
                            SendableClassLowerer::new(&allocator).statements(statement);
                        body.extend(statements);
                    }
                    None => body.push(statement),
                }
            }
            body.extend(entry_registration_statements);
            if let Some(statement) = preview_registration_statement {
                body.push(statement);
            }
            body.extend(
                navigation_registrations
                    .statements(&oxc_ast::builder::AstBuilder::new(&allocator), &allocator),
            );
            parsed.program.body = body;
            ViewpuProgramFinalizer::new(&allocator, &mut parsed.program)
                .with_local_builder_context(has_local_builder_in_file)
                .finalize();
            // hvigor runs `expandAllImportPaths` inside the TS emit
            // transformer chain; expanding before codegen keeps the emitted
            // source map in sync with the expanded imports.
            crate::transform::expand_import_path::apply_to_program(
                &mut parsed.program,
                &allocator,
                &source.id.id,
                options,
            );

            // Source pipeline ownership follows the current ets-loader AST
            // chain exactly: processUISyntax -> expandAllImportPaths ->
            // processKitImport -> ModuleSourceFile. ModuleSourceFile receives
            // the transformed SourceFile itself, then rebuilds resolvable
            // import/export declarations before the only TypeScript printer
            // pass (`module_source_file.ts::processTransformedTsModuleRequest`).
            // Mark those declaration shells synthetic while their retained
            // clauses still carry the original source spans. Printing and
            // reparsing first would lose TypeScript's `pos === -1` ownership
            // and cannot be repaired from generated VLQ tokens.
            if options.module_source_file_stage {
                let mut module_source_spans =
                    crate::module_rewrite::ModuleSourceFileSpanEraser::default();
                module_source_spans.visit_program(&mut parsed.program);
            }

            let generated = HvigorTypescriptPrinter::print_with_source_map(
                &parsed.program,
                CodegenOptions {
                    comments: CommentOptions::disabled(),
                    source_map_path: Some(source.id.id.clone().into()),
                    indent_char: IndentChar::Space,
                    indent_width: 4,
                    ..CodegenOptions::default()
                },
                content,
            );
            let out = ViewpuOutputText::new(generated.code).without_extra_trailing_blank_lines();
            Ok(ArktsTransformOutput {
                code: out,
                source_map: generated.source_map,
                rolldown_projection: None,
                insight_intent: insight_intent.metadata,
                crossplatform_collection,
            })
        } else {
            Ok(ArktsTransformOutput {
                code: content.to_string(),
                source_map: None,
                rolldown_projection: None,
                insight_intent: insight_intent.metadata,
                crossplatform_collection,
            })
        }
    }

    /// Execute the ArkTS 1.2 frontend boundary without routing its AST through
    /// the legacy ViewPU/ViewV2 emitter.
    ///
    /// The source compiler runs UI, interop, syntax and memo plugins inside
    /// the static compiler's parsed/checked states. OXC owns parsing and the
    /// dependency projection here; the resulting static source remains the
    /// input of that backend while Rolldown receives standard ESM only.
    fn transform_static<'alloc>(
        &self,
        allocator: &'alloc Allocator,
        program: &mut Program<'alloc>,
    ) -> Result<ArktsTransformOutput, EtsTransformError> {
        let options = self.options.as_ref();
        let builder_functions =
            BuilderFunctionNameCollector::new(program, &self.source.id.id, options).collect();
        BuilderFactoryValidator::new(
            &builder_functions.local,
            &self.source.id.id,
            ArktsLanguageVersion::V1_2,
        )
        .validate(program)?;

        let projection = StaticArktsGraphProjection::new(program).project(allocator);
        Ok(ArktsTransformOutput {
            code: self.source.content.clone(),
            source_map: None,
            rolldown_projection: Some(projection),
            insight_intent: InsightIntentSourceMetadata::default(),
            crossplatform_collection: CrossplatformSourceCollection::default(),
        })
    }
}

/// AST equivalent of ets-loader `process_mock.disableMockDecorator`.
///
/// The SDK removes only the configured `@MockSetup` decorator from classes,
/// class methods/accessors/constructors, properties, and parameters. Other
/// decorators retain their original order and spelling.
struct MockSetupDecoratorLowerer {
    changed: bool,
}

impl MockSetupDecoratorLowerer {
    fn new() -> Self {
        Self { changed: false }
    }

    fn program<'a>(mut self, program: &mut Program<'a>) -> bool {
        self.visit_program(program);
        self.changed
    }

    fn strip<'a>(&mut self, decorators: &mut OxcVec<'a, Decorator<'a>>) {
        let previous_len = decorators.len();
        decorators.retain(|decorator| {
            !matches!(
                &decorator.expression,
                Expression::Identifier(identifier) if identifier.name == "MockSetup"
            )
        });
        self.changed |= decorators.len() != previous_len;
    }
}

impl<'a> VisitMut<'a> for MockSetupDecoratorLowerer {
    fn visit_class(&mut self, class: &mut Class<'a>) {
        self.strip(&mut class.decorators);
        walk_mut::walk_class(self, class);
    }

    fn visit_method_definition(&mut self, method: &mut MethodDefinition<'a>) {
        self.strip(&mut method.decorators);
        walk_mut::walk_method_definition(self, method);
    }

    fn visit_property_definition(&mut self, property: &mut PropertyDefinition<'a>) {
        self.strip(&mut property.decorators);
        walk_mut::walk_property_definition(self, property);
    }

    fn visit_formal_parameter(&mut self, parameter: &mut FormalParameter<'a>) {
        self.strip(&mut parameter.decorators);
        walk_mut::walk_formal_parameter(self, parameter);
    }
}

struct ConcurrentDirectiveLowerer<'a> {
    allocator: &'a Allocator,
}

impl<'a> ConcurrentDirectiveLowerer<'a> {
    fn new(allocator: &'a Allocator) -> Self {
        Self { allocator }
    }

    fn program(&self, program: &mut Program<'a>) -> bool {
        let mut changed = false;
        for statement in &mut program.body {
            changed |= self.statement(statement);
        }
        changed
    }

    fn statement(&self, statement: &mut Statement<'a>) -> bool {
        match statement {
            Statement::FunctionDeclaration(function) => self.function(function),
            Statement::ExportDeclaration(export) => match &mut export.declaration {
                Declaration::FunctionDeclaration(function) => self.function(function),
                Declaration::TSNamespaceDeclaration(module) => {
                    self.ts_module_body(&mut module.body)
                }
                _ => false,
            },
            Statement::TSNamespaceDeclaration(module) => self.ts_module_body(&mut module.body),
            Statement::TSExternalModuleDeclaration(module) => match module.body.as_deref_mut() {
                Some(block) => {
                    let mut changed = false;
                    for statement in &mut block.body {
                        changed |= self.statement(statement);
                    }
                    changed
                }
                None => false,
            },
            _ => false,
        }
    }

    fn ts_module_body(&self, body: &mut TSNamespaceDeclarationBody<'a>) -> bool {
        match body {
            TSNamespaceDeclarationBody::TSNamespaceDeclaration(module) => {
                self.ts_module_body(&mut module.body)
            }
            TSNamespaceDeclarationBody::TSModuleBlock(block) => {
                let mut changed = false;
                for statement in &mut block.body {
                    changed |= self.statement(statement);
                }
                changed
            }
        }
    }

    fn function(&self, function: &mut Function<'a>) -> bool {
        if !function.decorators.iter().any(|decorator| {
            ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::CONCURRENT
        }) {
            return false;
        }
        function.decorators.clear();
        if let Some(body) = function.body.as_mut() {
            let builder = oxc_ast::builder::AstBuilder::new(self.allocator);
            let directive = Directive::new(
                SPAN,
                oxc_ast::ast::StringLiteral::new(
                    SPAN,
                    ConcurrentFunctionEmitter::DIRECTIVE,
                    None,
                    &builder,
                ),
                ConcurrentFunctionEmitter::DIRECTIVE,
                &builder,
            );
            body.directives.insert(0, directive);
        }
        true
    }
}

enum StatementReplacementPlan<'a> {
    AstStatements(OxcVec<'a, Statement<'a>>),
    LowerSendableClass,
}

struct ArktsStatementTransformer<'a, 'b> {
    allocator: &'b Allocator,
    source_text: Arc<str>,
    id: &'a str,
    options: &'a TransformOptions,
    simple_type_context: &'a SimpleTypeContext,
    custom_components: &'a FxHashSet<String>,
    custom_component_update_props: &'a CustomComponentUpdateProps,
    builder_functions: &'a FxHashSet<String>,
    extend_attributes: Arc<ExtendAttributeIndex>,
    global_styles: Vec<StylesMethodInfo<'b>>,
    hvigor_source_id: Arc<str>,
    module_name: Option<Arc<str>>,
    has_local_builder_in_file: bool,
}

struct ArktsStatementTransformerInput<'a, 'b> {
    allocator: &'b Allocator,
    source_text: &'a str,
    id: &'a str,
    options: &'a TransformOptions,
    simple_type_context: &'a SimpleTypeContext,
    custom_components: &'a FxHashSet<String>,
    custom_component_update_props: &'a CustomComponentUpdateProps,
    builder_functions: &'a FxHashSet<String>,
    hvigor_source_id: &'a str,
    extend_attributes: Arc<ExtendAttributeIndex>,
    global_styles: Vec<StylesMethodInfo<'b>>,
    has_local_builder_in_file: bool,
}

impl<'a, 'b> ArktsStatementTransformer<'a, 'b> {
    fn new(input: ArktsStatementTransformerInput<'a, 'b>) -> Self {
        Self {
            allocator: input.allocator,
            source_text: Arc::from(input.source_text),
            id: input.id,
            options: input.options,
            simple_type_context: input.simple_type_context,
            custom_components: input.custom_components,
            custom_component_update_props: input.custom_component_update_props,
            builder_functions: input.builder_functions,
            extend_attributes: input.extend_attributes,
            global_styles: input.global_styles,
            hvigor_source_id: Arc::from(input.hvigor_source_id),
            module_name: input.options.module_name.as_deref().map(Arc::from),
            has_local_builder_in_file: input.has_local_builder_in_file,
        }
    }

    fn component_v2(
        &self,
        statement: &Statement<'_>,
        emit_reusable_v2_helper: bool,
    ) -> Result<Option<OxcVec<'b, Statement<'b>>>, EtsTransformError> {
        let (struct_stmt, is_exported, is_default) = match statement {
            Statement::StructStatement(s) => (s.as_ref(), false, false),
            Statement::ExportDeclaration(e) => match &e.declaration {
                Declaration::StructStatement(s) => (s.as_ref(), true, false),
                _ => return Ok(None),
            },
            Statement::ExportDefaultDeclaration(e) => match &e.declaration {
                ExportDefaultDeclarationKind::StructStatement(s) => (s.as_ref(), true, true),
                _ => return Ok(None),
            },
            _ => return Ok(None),
        };

        if !ArkuiDecoratorRules::is_component_v2_struct(struct_stmt) {
            return Ok(None);
        }
        let global_reuse_pool = self.global_reuse_pool(struct_stmt)?;
        let runtime_options = self.component_runtime_options(struct_stmt);

        match ComponentV2Lowerer::new(
            struct_stmt,
            self.allocator,
            self.options.component_metadata.runtime_names(),
            self.source_text.as_ref(),
            self.hvigor_source_id.as_ref(),
            self.custom_components,
            self.builder_functions,
            self.custom_component_update_props,
            &self.options.component_metadata,
            Arc::clone(&self.extend_attributes),
            self.options.bundle_name.as_deref(),
            self.options.module_name.as_deref(),
            is_exported,
            is_default,
            struct_stmt.decorators.iter().any(|decorator| {
                ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::ENTRY
            }),
            emit_reusable_v2_helper,
            global_reuse_pool,
            runtime_options,
            self.has_local_builder_in_file,
        )
        .lower()
        {
            Ok(replacement) => Ok(Some(replacement)),
            Err(err) => Err(EtsTransformError::Fatal {
                file: self.id.to_string(),
                message: format!("unsupported ArkUI @ComponentV2 DSL: {err}"),
            }),
        }
    }

    fn is_reusable_v2_component(&self, statement: &Statement<'_>) -> bool {
        let struct_stmt = match statement {
            Statement::StructStatement(s) => s.as_ref(),
            Statement::ExportDeclaration(e) => match &e.declaration {
                Declaration::StructStatement(s) => s.as_ref(),
                _ => return false,
            },
            Statement::ExportDefaultDeclaration(e) => match &e.declaration {
                ExportDefaultDeclarationKind::StructStatement(s) => s.as_ref(),
                _ => return false,
            },
            _ => return false,
        };
        ArkuiDecoratorRules::is_component_v2_struct(struct_stmt)
            && struct_stmt.decorators.iter().any(|decorator| {
                ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::REUSABLE_V2
            })
    }

    fn component_v2_entry_registration<'s>(
        &self,
        statement: &'s Statement<'s>,
    ) -> Result<Option<(String, EntryRegistrationInfo<'s>)>, EtsTransformError> {
        let struct_stmt = match statement {
            Statement::StructStatement(s) => s.as_ref(),
            Statement::ExportDeclaration(e) => match &e.declaration {
                Declaration::StructStatement(s) => s.as_ref(),
                _ => return Ok(None),
            },
            Statement::ExportDefaultDeclaration(e) => match &e.declaration {
                ExportDefaultDeclarationKind::StructStatement(s) => s.as_ref(),
                _ => return Ok(None),
            },
            _ => return Ok(None),
        };
        if !ArkuiDecoratorRules::is_component_v2_struct(struct_stmt) {
            return Ok(None);
        }
        let Some(entry_decorator) = struct_stmt
            .decorators
            .iter()
            .find(|decorator| ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::ENTRY)
        else {
            return Ok(None);
        };
        let storage = ArkuiDecorator::new(entry_decorator).arg_expression();
        self.validate_form_entry_argument(storage)?;
        Ok(Some((
            struct_stmt.id.name.to_string(),
            EntryRegistrationFactory::new(self.id, self.hvigor_source_id.as_ref(), self.options)
                .with_entry_argument(storage)?,
        )))
    }

    fn component<'s>(
        &self,
        statement: &'s Statement<'s>,
    ) -> Result<Option<ComponentInfo<'s>>, EtsTransformError>
    where
        'b: 's,
    {
        match statement {
            Statement::StructStatement(statement) => {
                self.component_from_struct(statement, false, false)
            }
            Statement::ExportDeclaration(export) => match &export.declaration {
                Declaration::StructStatement(statement) => {
                    self.component_from_struct(statement.as_ref(), true, false)
                }
                _ => Ok(None),
            },
            Statement::ExportDefaultDeclaration(export) => match &export.declaration {
                ExportDefaultDeclarationKind::StructStatement(statement) => {
                    self.component_from_struct(statement, true, true)
                }
                _ => Ok(None),
            },
            _ => Ok(None),
        }
    }

    fn non_component_plan(
        &self,
        statement: &Statement<'_>,
    ) -> Result<Option<StatementReplacementPlan<'b>>, EtsTransformError> {
        let replacement = match statement {
            Statement::AnnotationDeclaration(annotation) => Some(
                StatementReplacementPlan::AstStatements(ArenaVec::from_value_in(
                    ParamsInterfaceEmitter::new(annotation).statement(self.allocator, false),
                    &self.allocator,
                )),
            ),
            Statement::ClassDeclaration(_)
                if SendableClassLowerer::statement_needs_lowering(statement) =>
            {
                Some(StatementReplacementPlan::LowerSendableClass)
            }
            Statement::FunctionDeclaration(function) if Self::is_styles_function(function, &[]) => {
                Some(StatementReplacementPlan::AstStatements(ArenaVec::new_in(
                    &self.allocator,
                )))
            }
            Statement::FunctionDeclaration(function) => self
                .decorated_function_statement(function, false, &[])?
                .map(|statement| {
                    StatementReplacementPlan::AstStatements(ArenaVec::from_value_in(
                        statement,
                        &self.allocator,
                    ))
                }),
            Statement::ExportDeclaration(export) => match &export.declaration {
                Declaration::FunctionDeclaration(function)
                    if Self::is_styles_function(function, &export.decorators) =>
                {
                    Some(StatementReplacementPlan::AstStatements(ArenaVec::new_in(
                        &self.allocator,
                    )))
                }
                Declaration::FunctionDeclaration(function) => self
                    .decorated_function_statement(function, true, &export.decorators)?
                    .map(|statement| {
                        StatementReplacementPlan::AstStatements(ArenaVec::from_value_in(
                            statement,
                            &self.allocator,
                        ))
                    }),
                Declaration::AnnotationDeclaration(annotation) => Some(
                    StatementReplacementPlan::AstStatements(ArenaVec::from_value_in(
                        ParamsInterfaceEmitter::new(annotation).statement(self.allocator, true),
                        &self.allocator,
                    )),
                ),
                Declaration::ClassDeclaration(_)
                    if SendableClassLowerer::statement_needs_lowering(statement) =>
                {
                    Some(StatementReplacementPlan::LowerSendableClass)
                }
                _ => None,
            },
            Statement::ExportDefaultDeclaration(export) => match &export.declaration {
                ExportDefaultDeclarationKind::ClassDeclaration(_)
                    if SendableClassLowerer::statement_needs_lowering(statement) =>
                {
                    Some(StatementReplacementPlan::LowerSendableClass)
                }
                _ => None,
            },
            _ => None,
        };
        Ok(replacement)
    }

    fn is_styles_function(function: &Function<'_>, extra_decorators: &[Decorator<'_>]) -> bool {
        function
            .decorators
            .iter()
            .chain(extra_decorators)
            .any(|decorator| ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::STYLES)
    }

    fn decorated_function_statement(
        &self,
        function: &Function<'_>,
        exported: bool,
        extra_decorators: &[Decorator<'_>],
    ) -> Result<Option<Statement<'b>>, EtsTransformError> {
        let decorators = function.decorators.iter().chain(extra_decorators.iter());
        let decorator_kind = decorators
            .clone()
            .map(|decorator| ArkuiDecorator::new(decorator).name())
            .find(|name| ArkuiDecoratorName::is_top_level_function(name))
            .map(str::to_string);
        let Some(decorator_kind) = decorator_kind else {
            return Ok(None);
        };
        let Some(function_id) = function.id.as_ref() else {
            return Ok(None);
        };
        let name = function_id.name.as_str();
        let append_parent = ArkuiDecoratorName::appends_parent_parameter(decorator_kind.as_str());
        let return_annotation = function.return_type.as_deref();
        let statement = match decorator_kind.as_str() {
            ArkuiDecoratorName::CONCURRENT => {
                ConcurrentFunctionEmitter::new(self.allocator).statement(function, exported)
            }
            decorator_name if ArkuiDecoratorName::is_extend_function(decorator_name) => {
                let component = function
                    .decorators
                    .iter()
                    .chain(extra_decorators.iter())
                    .find(|decorator| ArkuiDecorator::new(decorator).name() == decorator_kind)
                    .and_then(|decorator| ArkuiDecorator::new(decorator).identifier_arg())
                    .unwrap_or_else(|| "View".to_string());
                let body_statements = ExtendBodyPlan::new(
                    function.body.as_deref(),
                    &component,
                    Arc::clone(&self.options.component_metadata),
                    Arc::clone(&self.extend_attributes),
                )
                .with_styles_methods(&self.global_styles)
                .statements(self.allocator)
                .map_err(|error| EtsTransformError::Fatal {
                    file: self.id.to_string(),
                    message: error.hvigor_message().to_string(),
                })?;
                if decorator_name == ArkuiDecoratorName::ANIMATABLE_EXTEND {
                    TopLevelFunctionEmitter::animatable_extend_statement(
                        self.allocator,
                        exported,
                        &component,
                        name,
                        &function.params,
                        body_statements,
                        self.options.component_metadata.runtime_names(),
                    )
                } else {
                    TopLevelFunctionEmitter::extend_with_body_statement(
                        self.allocator,
                        exported,
                        &component,
                        name,
                        &function.params,
                        body_statements,
                    )
                }
            }
            _ => {
                let raw_object_fields = FxHashSet::default();
                let first_parameter = if function.params.items.len() == 1 {
                    function.params.items[0]
                        .pattern
                        .get_binding_identifier()
                        .map(|identifier| Arc::<str>::from(identifier.name.as_str()))
                } else {
                    None
                };
                let body = BuildBodyPlan::global_builder_with_shared_inputs(
                    function.body.as_deref(),
                    BuildBodySharedInputs {
                        source_text: Arc::clone(&self.source_text),
                        source_id: Arc::clone(&self.hvigor_source_id),
                        custom_components: self.custom_components,
                        builder_methods: self.builder_functions,
                        custom_component_update_props: self.custom_component_update_props,
                        component_metadata: Arc::clone(&self.options.component_metadata),
                        raw_object_fields: &raw_object_fields,
                        module_name: None,
                    },
                    GeneratedViewpuName::VIEWPU_BASE,
                    first_parameter,
                )
                .with_xcomponent_module_identity(
                    self.options.bundle_name.as_deref(),
                    self.module_name.as_deref(),
                )
                .with_shared_extend_attributes(Arc::clone(&self.extend_attributes))
                // Top-level @Builder functions share the same file-level
                // @Styles registry as component build methods in
                // process_component_build.js.
                .with_styles_methods(&self.global_styles);
                TopLevelFunctionEmitter::decorated_with_params_statement(
                    self.allocator,
                    exported,
                    name,
                    &function.params,
                    append_parent,
                    return_annotation,
                    &BuildBodyInfo::plan(body),
                )
                .map_err(|error| {
                    error.into_transform_error(self.id, "unsupported ArkUI builder DSL")
                })?
            }
        };

        Ok(Some(statement))
    }

    fn component_from_struct<'s>(
        &self,
        statement: &'s StructStatement<'s>,
        is_exported: bool,
        is_default_export: bool,
    ) -> Result<Option<ComponentInfo<'s>>, EtsTransformError>
    where
        'b: 's,
    {
        if !ArkuiDecoratorRules::is_component_struct(statement) {
            return Ok(None);
        }

        let entry_decorator = statement
            .decorators
            .iter()
            .find(|decorator| ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::ENTRY);
        let is_entry = entry_decorator.is_some();
        let entry_span = entry_decorator.map_or(SPAN, GetSpan::span);
        let entry_argument =
            entry_decorator.and_then(|decorator| ArkuiDecorator::new(decorator).arg_expression());
        self.validate_form_entry_argument(entry_argument)?;
        let is_reusable = statement
            .decorators
            .iter()
            .any(|decorator| ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::REUSABLE);
        let global_reuse_pool = self.global_reuse_pool(statement)?;
        let runtime_options = self.component_runtime_options(statement);
        let mut state_fields = Vec::new();
        let mut initial_render_body = BuildBodyInfo::default();
        let mut initial_render_method_span = SPAN;
        let mut builder_methods: Vec<BuilderMethodInfo> = Vec::new();
        let mut page_transition_methods: Vec<PageTransitionMethodInfo> = Vec::new();
        let mut styles_methods: Vec<StylesMethodInfo<'s>> = self.global_styles.to_vec();
        for element in &statement.body.body {
            let StructElement::MethodDefinition(method) = element else {
                continue;
            };
            if !method.decorators.iter().any(|decorator| {
                ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::STYLES
            }) {
                continue;
            }
            if !method.value.params.items.is_empty() {
                return Err(EtsTransformError::Fatal {
                    file: self.id.to_string(),
                    message: "'@Styles' decorated functions and methods cannot have arguments."
                        .to_string(),
                });
            }
            if let Some(name) = method.key.static_name() {
                styles_methods.push(StylesMethodInfo {
                    name: name.into_owned(),
                    body: method.value.body.as_deref(),
                });
            }
        }
        let builder_method_names = StructBuilderMethodNames::new(statement).collect();
        let builder_param_names = StructBuilderParamNames::new(statement).collect();
        let mutable_builder_properties =
            Arc::new(StructMutableBuilderProperties::new(statement).collect());
        let regular_attribute_properties =
            Arc::new(StructRegularAttributeProperties::new(statement, false).collect());
        let mut builder_initializer_names = self.builder_functions.clone();
        builder_initializer_names.extend(builder_method_names.iter().cloned());
        let mut visible_builder_methods = builder_initializer_names.clone();
        visible_builder_methods.extend(builder_param_names.iter().cloned());
        let raw_object_fields =
            ComponentRawObjectFields::new(statement, self.simple_type_context).collect();
        let mut regular_members: Vec<RegularMember<'_>> = Vec::new();
        let mut source_members = Vec::new();
        let mut params_fields: Vec<ParamField> = Vec::new();
        let mut property_order: Vec<String> = Vec::new();
        let mut regular_fields: Vec<RegularField> = Vec::new();
        let mut runtime_decorated_fields = Vec::new();

        for element in &statement.body.body {
            match element {
                StructElement::PropertyDefinition(property) => {
                    if property.r#static {
                        // SDK ets-loader parity:
                        // process_component_class.js::processMembers keeps static struct
                        // properties as class members with `d.push(e)` and does not route
                        // them through `createVariableInitStatement`, params, or constructor
                        // initialization.
                        regular_members.push(RegularMember::Property(property));
                        source_members
                            .push(ComponentSourceMember::Regular(regular_members.len() - 1));
                        continue;
                    }

                    let is_custom_env = property.decorators.iter().any(|decorator| {
                        ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::CUSTOM_ENV
                    });
                    if let Some(name) = property.key.static_name() {
                        let name = name.into_owned();
                        property_order.push(name.clone());
                        if !is_custom_env {
                            source_members.push(ComponentSourceMember::Property(name.clone()));
                        }
                        params_fields.push(ParamField {
                            name,
                            source_span: property.span,
                            name_span: property.key.span(),
                            type_annotation_ast: property.type_annotation.as_deref(),
                        });
                    }

                    if is_custom_env {
                        runtime_decorated_fields.push(property.as_ref());
                        source_members.push(ComponentSourceMember::RuntimeDecorated(
                            runtime_decorated_fields.len() - 1,
                        ));
                        continue;
                    }

                    let decorator_names: Vec<&str> = property
                        .decorators
                        .iter()
                        .map(|decorator| ArkuiDecorator::new(decorator).name())
                        .collect();
                    let Some(decorator) = ArkuiDecoratorRules::primary_state(&decorator_names)
                    else {
                        if let Some(name) = property.key.static_name() {
                            let initializer = property
                                .value
                                .as_ref()
                                .map(FieldExpression::from_expression);
                            regular_fields.push(RegularField {
                                name: name.into_owned(),
                                source: property,
                                initializer,
                            });
                        } else {
                            regular_members.push(RegularMember::Property(property));
                            source_members
                                .push(ComponentSourceMember::Regular(regular_members.len() - 1));
                        }
                        continue;
                    };
                    let Some(name) = property.key.static_name() else {
                        continue;
                    };
                    let type_node = property
                        .type_annotation
                        .as_ref()
                        .map(|annotation| &annotation.type_annotation);
                    let initializer = property
                        .value
                        .as_ref()
                        .map(FieldExpression::from_expression);
                    if decorator == ArkuiDecoratorName::BUILDER_PARAM
                        && property.value.as_ref().is_some_and(|initializer| {
                            !self.builder_param_initializer_is_builder(
                                initializer,
                                &builder_initializer_names,
                                &mutable_builder_properties,
                            )
                        })
                    {
                        return Err(EtsTransformError::Fatal {
                            file: self.id.to_string(),
                            message: "'@BuilderParam' property can only initialized by '@Builder' function or '@LocalBuilder' method in struct.".to_string(),
                        });
                    }
                    let watch_handler = property
                        .decorators
                        .iter()
                        .find(|d| ArkuiDecorator::new(d).name() == ArkuiDecoratorName::WATCH)
                        .and_then(|decorator| ArkuiDecorator::new(decorator).string_arg());
                    let provider_decorator = property.decorators.iter().find(|d| {
                        ArkuiDecoratorName::is_provider_alias(ArkuiDecorator::new(d).name())
                    });
                    let provider_alias = provider_decorator
                        .and_then(|decorator| ArkuiDecorator::new(decorator).provider_alias());
                    let (alias, provide_allow_override) = provider_alias
                        .map(|(alias, allow_override)| (Some(alias), allow_override))
                        .unwrap_or((None, false));
                    let provide_override_key = provider_decorator
                        .and_then(|decorator| {
                            ArkuiDecorator::new(decorator).provider_override_key()
                        })
                        .map(FieldExpression::from_expression);
                    let provide_allow_override =
                        provide_allow_override || provide_override_key.is_some();
                    let decorator_arg = property
                        .decorators
                        .iter()
                        .find(|d| ArkuiDecorator::new(d).name() == decorator)
                        .and_then(|decorator| ArkuiDecorator::new(decorator).arg_expression())
                        .map(FieldExpression::from_expression);
                    state_fields.push(StateField {
                        name: name.into_owned(),
                        source_span: property.span,
                        name_span: property.key.span(),
                        type_annotation_ast: property.type_annotation.as_deref(),
                        optional: property.optional,
                        decorator: decorator.to_string(),
                        initializer,
                        watch_handler,
                        alias,
                        provide_allow_override,
                        provide_override_key,
                        decorator_arg,
                        is_simple_type: type_node.is_none_or(|ty| {
                            self.simple_type_context.is_simple_type_annotation(ty)
                        }),
                    });
                }
                StructElement::MethodDefinition(method)
                    if method
                        .key
                        .static_name()
                        .is_some_and(|name| name == GeneratedViewpuName::BUILD) =>
                {
                    let build_method_span = method.span;
                    let plan = BuildBodyPlan::component_with_shared_inputs(
                        method.value.body.as_deref(),
                        BuildBodySharedInputs {
                            source_text: Arc::clone(&self.source_text),
                            source_id: Arc::clone(&self.hvigor_source_id),
                            custom_components: self.custom_components,
                            builder_methods: &visible_builder_methods,
                            custom_component_update_props: self.custom_component_update_props,
                            component_metadata: Arc::clone(&self.options.component_metadata),
                            raw_object_fields: &raw_object_fields,
                            module_name: self.module_name.as_ref().map(Arc::clone),
                        },
                        is_reusable,
                        GeneratedViewpuName::VIEWPU_BASE,
                    )
                    .with_xcomponent_module_identity(
                        self.options.bundle_name.as_deref(),
                        self.module_name.as_deref(),
                    )
                    .with_shared_extend_attributes(Arc::clone(&self.extend_attributes))
                    .with_shared_mutable_builder_properties(Arc::clone(&mutable_builder_properties))
                    .with_shared_regular_attribute_properties(Arc::clone(
                        &regular_attribute_properties,
                    ))
                    .with_styles_methods(&styles_methods);
                    initial_render_body = BuildBodyInfo::plan(plan);
                    // Preserve the original build method range on the generated
                    // initialRender method for TypeScript-printer source maps.
                    // There is only one valid build method per component.
                    initial_render_method_span = build_method_span;
                    source_members.push(ComponentSourceMember::InitialRender);
                }
                StructElement::MethodDefinition(method)
                    if method.decorators.iter().any(|d| {
                        matches!(
                            ArkuiDecorator::new(d).name(),
                            ArkuiDecoratorName::BUILDER | ArkuiDecoratorName::LOCAL_BUILDER
                        )
                    }) =>
                {
                    let Some(name) = method.key.static_name() else {
                        continue;
                    };
                    let kind = if method.decorators.iter().any(|decorator| {
                        ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::LOCAL_BUILDER
                    }) {
                        if method.r#static {
                            return Err(EtsTransformError::Fatal {
                                file: self.id.to_string(),
                                message: "Static methods in custom components cannot be decorated by '@LocalBuilder'.".to_string(),
                            });
                        }
                        BuilderMethodKind::LocalBuilder
                    } else {
                        BuilderMethodKind::Builder
                    };
                    let body_factory = match kind {
                        BuilderMethodKind::Builder => BuildBodyPlan::builder_with_shared_inputs,
                        BuilderMethodKind::LocalBuilder => {
                            BuildBodyPlan::local_builder_with_shared_inputs
                        }
                    };
                    let body = body_factory(
                        method.value.body.as_deref(),
                        BuildBodySharedInputs {
                            source_text: Arc::clone(&self.source_text),
                            source_id: Arc::clone(&self.hvigor_source_id),
                            custom_components: self.custom_components,
                            builder_methods: &visible_builder_methods,
                            custom_component_update_props: self.custom_component_update_props,
                            component_metadata: Arc::clone(&self.options.component_metadata),
                            raw_object_fields: &raw_object_fields,
                            module_name: self.module_name.as_ref().map(Arc::clone),
                        },
                        GeneratedViewpuName::VIEWPU_BASE,
                        is_reusable,
                    )
                    .with_xcomponent_module_identity(
                        self.options.bundle_name.as_deref(),
                        self.module_name.as_deref(),
                    )
                    .with_shared_extend_attributes(Arc::clone(&self.extend_attributes))
                    .with_shared_mutable_builder_properties(Arc::clone(&mutable_builder_properties))
                    .with_shared_regular_attribute_properties(Arc::clone(
                        &regular_attribute_properties,
                    ))
                    .with_styles_methods(&styles_methods);
                    builder_methods.push(BuilderMethodInfo {
                        name: name.into_owned(),
                        params: &method.value.params,
                        return_annotation: method.value.return_type.as_deref(),
                        body: BuildBodyInfo::plan(body),
                        kind,
                    });
                    source_members.push(ComponentSourceMember::Builder(builder_methods.len() - 1));
                }
                StructElement::MethodDefinition(method)
                    if method.key.static_name().is_some_and(|name| {
                        name == self
                            .options
                            .component_metadata
                            .runtime_names()
                            .page_transition_method
                    }) =>
                {
                    let body = BuildBodyPlan::page_transition_with_shared_inputs(
                        method.value.body.as_deref(),
                        BuildBodySharedInputs {
                            source_text: Arc::clone(&self.source_text),
                            source_id: Arc::clone(&self.hvigor_source_id),
                            custom_components: self.custom_components,
                            builder_methods: &visible_builder_methods,
                            custom_component_update_props: self.custom_component_update_props,
                            component_metadata: Arc::clone(&self.options.component_metadata),
                            raw_object_fields: &raw_object_fields,
                            module_name: self.module_name.as_ref().map(Arc::clone),
                        },
                        GeneratedViewpuName::VIEWPU_BASE,
                    )
                    .with_xcomponent_module_identity(
                        self.options.bundle_name.as_deref(),
                        self.module_name.as_deref(),
                    )
                    .with_shared_extend_attributes(Arc::clone(&self.extend_attributes))
                    .with_shared_mutable_builder_properties(Arc::clone(&mutable_builder_properties))
                    .with_styles_methods(&styles_methods);
                    page_transition_methods.push(PageTransitionMethodInfo {
                        method,
                        body: BuildBodyInfo::plan(body),
                    });
                    source_members.push(ComponentSourceMember::PageTransition(
                        page_transition_methods.len() - 1,
                    ));
                }
                StructElement::MethodDefinition(method)
                    if method
                        .decorators
                        .iter()
                        .any(|d| ArkuiDecorator::new(d).name() == ArkuiDecoratorName::STYLES) =>
                {
                    // SDK stores the body in INNER_STYLE_FUNCTION and removes
                    // the declaration from the generated class. Calls are
                    // expanded by BuildBodyPlan through styles_methods.
                }
                StructElement::MethodDefinition(method) => {
                    if method.kind != MethodDefinitionKind::Constructor
                        && method
                            .key
                            .static_name()
                            .is_none_or(|name| name != GeneratedViewpuName::CONSTRUCTOR)
                    {
                        regular_members.push(RegularMember::Method(method));
                        source_members
                            .push(ComponentSourceMember::Regular(regular_members.len() - 1));
                    }
                }
                StructElement::StaticBlock(block) => {
                    regular_members.push(RegularMember::StaticBlock(block));
                    source_members.push(ComponentSourceMember::Regular(regular_members.len() - 1));
                }
                StructElement::TSIndexSignature(signature) => {
                    regular_members.push(RegularMember::TSIndexSignature(signature));
                    source_members.push(ComponentSourceMember::Regular(regular_members.len() - 1));
                }
                StructElement::AccessorProperty(property) => {
                    regular_members.push(RegularMember::AccessorProperty(property));
                    source_members.push(ComponentSourceMember::Regular(regular_members.len() - 1));
                }
                // ETS overload declarations (`ETSOverloadDeclaration`) are
                // managed-signature declarations, not runtime members; the
                // ArkUI lowering below has no member category for them.
                StructElement::ETSOverloadDeclaration(_) => {}
            }
        }

        Ok(Some(ComponentInfo {
            partial_update: self.options.supports_partial_update(),
            name: statement.id.name.to_string(),
            name_span: statement.id.span,
            entry_span,
            build_method_span: initial_render_method_span,
            is_entry,
            is_exported,
            is_default_export,
            is_reusable,
            has_local_builder_in_file: self.has_local_builder_in_file,
            global_reuse_pool,
            runtime_options,
            is_custom_dialog: statement.decorators.iter().any(|decorator| {
                ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::CUSTOM_DIALOG
            }),
            state_fields,
            params_fields,
            property_order,
            regular_fields,
            runtime_decorated_fields,
            regular_members,
            source_members,
            initial_render_body,
            entry_registration: is_entry
                .then(|| {
                    EntryRegistrationFactory::new(
                        self.id,
                        self.hvigor_source_id.as_ref(),
                        self.options,
                    )
                    .with_entry_argument(entry_argument)
                })
                .transpose()?,
            builder_methods,
            page_transition_methods,
        }))
    }

    fn global_reuse_pool<'s>(
        &self,
        component: &'s StructStatement<'s>,
    ) -> Result<Option<GlobalReusePoolConfig<'s>>, EtsTransformError> {
        let Some(decorator) = component.decorators.iter().find(|decorator| {
            matches!(
                ArkuiDecorator::new(decorator).name(),
                ArkuiDecoratorName::COMPONENT | ArkuiDecoratorName::COMPONENT_V2
            )
        }) else {
            return Ok(None);
        };
        let Expression::CallExpression(call) = &decorator.expression else {
            return Ok(None);
        };
        let Some(Argument::ObjectExpression(options)) = call.arguments.first() else {
            return Ok(None);
        };
        if options.properties.len() < 2 {
            return Ok(None);
        }

        let mut ownership = None;
        let mut accepted_components = None;
        for property in &options.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                return Ok(None);
            };
            let PropertyKey::StaticIdentifier(name) = &property.key else {
                return Ok(None);
            };
            match name.name.as_str() {
                "reusePool" => ownership = Some(&property.value),
                "poolAccepts" => {
                    if let Expression::ArrayExpression(accepted) = &property.value {
                        accepted_components = Some(accepted.as_ref());
                    } else {
                        // `process_component_class.ts::processGlobalReusePool`
                        // silently skips non-array values. The UI checker owns
                        // any source diagnostic; this transform must not invent
                        // a stricter one.
                        return Ok(None);
                    }
                }
                _ => {}
            }
        }
        let (Some(ownership), Some(accepted_components)) = (ownership, accepted_components) else {
            return Err(self.global_reuse_pool_error(
                "10905375",
                &component.id.name,
                &format!(
                    "'{}' must provide both reusePool and poolAccepts. Neither can be omitted when using the global reuse pool.",
                    component.id.name
                ),
            ));
        };
        let Expression::StringLiteral(ownership_literal) = ownership else {
            return Err(self.global_reuse_pool_error(
                "10905378",
                &component.id.name,
                "ReusePool can only accept string literal.",
            ));
        };
        if !matches!(ownership_literal.value.as_str(), "shared" | "perInstance") {
            return Err(self.global_reuse_pool_error(
                "10905377",
                &component.id.name,
                &format!(
                    "ReusePool must be either 'shared' or 'perInstance'. The value '{}' is not valid.",
                    ownership_literal.value
                ),
            ));
        }
        if accepted_components.elements.is_empty() {
            return Err(self.global_reuse_pool_error(
                "10905376",
                &component.id.name,
                "PoolAccepts cannot be an empty array. Provide at least one '@Reusable' or '@ReusableV2' component.",
            ));
        }
        for accepted in &accepted_components.elements {
            let Some(Expression::Identifier(name)) = accepted.as_expression() else {
                continue;
            };
            if name.name == component.id.name {
                return Err(self.global_reuse_pool_error(
                    "10905374",
                    &component.id.name,
                    &format!(
                        "'{}' cannot list itself in poolAccepts. The pool is not yet ready when '{}' is being constructed.",
                        name.name, name.name
                    ),
                ));
            }
            if !self
                .custom_component_update_props
                .get(name.name.as_str())
                .is_some_and(|info| info.reusable || info.reusable_v2)
            {
                return Err(self.global_reuse_pool_error(
                    "10905373",
                    &component.id.name,
                    &format!(
                        "'{}' is not a '@Reusable' or '@ReusableV2' component and cannot be added to poolAccepts.",
                        name.name
                    ),
                ));
            }
        }

        Ok(Some(GlobalReusePoolConfig {
            ownership,
            accepted_components,
        }))
    }

    fn component_runtime_options<'s>(
        &self,
        component: &'s StructStatement<'s>,
    ) -> ComponentRuntimeOptions<'s> {
        let mut options = ComponentRuntimeOptions::default();
        for decorator in &component.decorators {
            let Expression::CallExpression(call) = &decorator.expression else {
                continue;
            };
            for argument in &call.arguments {
                let Argument::ObjectExpression(object) = argument else {
                    continue;
                };
                for property in &object.properties {
                    let ObjectPropertyKind::ObjectProperty(property) = property else {
                        continue;
                    };
                    if property.shorthand {
                        continue;
                    }
                    let Some(name) = property.key.static_name() else {
                        continue;
                    };
                    match name.as_ref() {
                        "freezeWhenInactive" if options.freeze_when_inactive.is_none() => {
                            options.freeze_when_inactive = Some(&property.value);
                        }
                        "memoryOptimizationStrategy"
                            if options.memory_optimization_strategy.is_none() =>
                        {
                            options.memory_optimization_strategy = Some(&property.value);
                        }
                        _ => {}
                    }
                }
            }
        }
        options
    }

    fn builder_param_initializer_is_builder(
        &self,
        initializer: &Expression<'_>,
        builder_names: &FxHashSet<String>,
        mutable_builder_properties: &FxHashSet<String>,
    ) -> bool {
        match initializer {
            Expression::Identifier(identifier) => builder_names.contains(identifier.name.as_str()),
            Expression::StaticMemberExpression(member) => {
                if builder_names.contains(member.property.name.as_str()) {
                    return true;
                }
                if member.property.name != "builder" {
                    return false;
                }
                match &member.object {
                    Expression::CallExpression(call) => matches!(
                        &call.callee,
                        Expression::Identifier(identifier)
                            if matches!(identifier.name.as_str(), "wrapBuilder" | "mutableBuilder")
                    ),
                    Expression::StaticMemberExpression(receiver) => {
                        matches!(&receiver.object, Expression::ThisExpression(_))
                            && mutable_builder_properties.contains(receiver.property.name.as_str())
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    fn global_reuse_pool_error(
        &self,
        code: &str,
        component: &str,
        message: &str,
    ) -> EtsTransformError {
        EtsTransformError::CodedDiagnostic {
            file: self.id.to_string(),
            message: format!("@Component '{component}': {message}"),
            code: code.to_string(),
            severity: crate::error::DiagnosticSeverity::Error,
            span: None,
            solutions: Vec::new(),
        }
    }

    fn validate_form_entry_argument(
        &self,
        argument: Option<&Expression<'_>>,
    ) -> Result<(), EtsTransformError> {
        if self.options.form_entry
            && argument.is_some_and(|argument| matches!(argument, Expression::ObjectExpression(_)))
        {
            return Err(EtsTransformError::Fatal {
                file: self.id.to_string(),
                message: "'@Entry' doesn't support {} parameter in card".to_string(),
            });
        }
        Ok(())
    }
}

struct ConcurrentFunctionEmitter<'a> {
    allocator: &'a Allocator,
}

impl<'a> ConcurrentFunctionEmitter<'a> {
    const DIRECTIVE: &'static str = "use concurrent";

    fn new(allocator: &'a Allocator) -> Self {
        Self { allocator }
    }

    fn statement(&self, function: &Function<'_>, exported: bool) -> Statement<'a> {
        let ast = oxc_ast::builder::AstBuilder::new(self.allocator);
        let mut function = function.clone_in(self.allocator);
        function.decorators.clear();
        if let Some(body) = function.body.as_mut() {
            let directive = Directive::new(
                SPAN,
                StringLiteral::new(SPAN, Self::DIRECTIVE, None, &ast),
                Self::DIRECTIVE,
                &ast,
            );
            body.directives.insert(0, directive);
        }
        let function = ArenaBox::new_in(function, &ast);
        if !exported {
            return Statement::FunctionDeclaration(function);
        }
        Statement::new_export_declaration(SPAN, Declaration::FunctionDeclaration(function), &ast)
    }
}

/// Implements SDK `process_sendable.js::processSendableClass`.
///
/// `process_ui_syntax.js` leaves every ordinary class decorator intact and
/// dispatches only classes carrying `@Sendable` to this transform. Keeping
/// that distinction is important: ArkUI and application decorators are part
/// of the CompileArkTS input consumed by es2abc, not TypeScript decorators for
/// arkdown to erase or lower.
struct SendableClassLowerer<'a> {
    allocator: &'a Allocator,
    ast: oxc_ast::builder::AstBuilder<'a>,
}

impl<'a> SendableClassLowerer<'a> {
    const DIRECTIVE: &'static str = "use sendable";
    const SUPER_ARGS: &'static str = "args";

    fn new(allocator: &'a Allocator) -> Self {
        Self {
            allocator,
            ast: oxc_ast::builder::AstBuilder::new(allocator),
        }
    }

    fn statement_needs_lowering(statement: &Statement<'_>) -> bool {
        Self::statement_has_class_decorator(statement, ArkuiDecoratorName::SENDABLE)
    }

    fn statements(&self, mut statement: Statement<'a>) -> OxcVec<'a, Statement<'a>> {
        match &mut statement {
            Statement::ClassDeclaration(class) => self.class(class),
            Statement::ExportDeclaration(export) => {
                Self::remove_sendable_decorator(&mut export.decorators);
                if let Declaration::ClassDeclaration(class) = &mut export.declaration {
                    self.class(class);
                }
            }
            Statement::ExportDefaultDeclaration(export) => {
                if let ExportDefaultDeclarationKind::ClassDeclaration(class) =
                    &mut export.declaration
                {
                    self.class(class);
                }
            }
            _ => {}
        }
        ArenaVec::from_value_in(statement, &self.ast)
    }

    fn statement_has_class_decorator(statement: &Statement<'_>, name: &str) -> bool {
        match statement {
            Statement::ClassDeclaration(class) => Self::class_has_decorator(class, name),
            Statement::ExportDeclaration(export) => {
                export
                    .decorators
                    .iter()
                    .any(|decorator| ArkuiDecorator::new(decorator).name() == name)
                    || matches!(
                        &export.declaration,
                        Declaration::ClassDeclaration(class) if Self::class_has_decorator(class, name)
                    )
            }
            Statement::ExportDefaultDeclaration(export) => {
                if let ExportDefaultDeclarationKind::ClassDeclaration(class) = &export.declaration {
                    Self::class_has_decorator(class, name)
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn class_has_decorator(class: &Class<'_>, name: &str) -> bool {
        class
            .decorators
            .iter()
            .any(|decorator| ArkuiDecorator::new(decorator).name() == name)
    }

    fn class(&self, class: &mut Class<'a>) {
        Self::remove_sendable_decorator(&mut class.decorators);
        let mut has_constructor = false;
        for element in &mut class.body.body {
            match element {
                ClassElement::PropertyDefinition(property) if property.optional => {
                    self.optional_property(property);
                }
                ClassElement::MethodDefinition(method)
                    if method.kind == MethodDefinitionKind::Constructor =>
                {
                    has_constructor = true;
                    if let Some(body) = method.value.body.as_mut() {
                        body.directives.insert(0, self.directive());
                    }
                }
                _ => {}
            }
        }
        if !has_constructor {
            let constructor = self.constructor(class.heritage.is_some());
            class.body.body.insert(0, constructor);
        }
    }

    fn optional_property(&self, property: &mut oxc_ast::ast::PropertyDefinition<'a>) {
        property.optional = false;
        if let Some(annotation) = property.type_annotation.as_mut() {
            let original = annotation.type_annotation.clone_in(self.allocator);
            annotation.type_annotation = match original {
                TSType::TSUnionType(mut union) => {
                    if !union
                        .types
                        .iter()
                        .any(|ty| matches!(ty, TSType::TSUndefinedKeyword(_)))
                    {
                        union
                            .types
                            .push(TSType::new_ts_undefined_keyword(SPAN, &self.ast));
                    }
                    TSType::TSUnionType(union)
                }
                ty => TSType::new_ts_union_type(
                    SPAN,
                    ArenaVec::from_iter_in(
                        [ty, TSType::new_ts_undefined_keyword(SPAN, &self.ast)],
                        &self.ast,
                    ),
                    &self.ast,
                ),
            };
        }
        if property.value.is_none() {
            property.value = Some(Expression::new_identifier(SPAN, "undefined", &self.ast));
        }
    }

    fn constructor(&self, has_super_class: bool) -> ClassElement<'a> {
        let rest = has_super_class.then(|| {
            FormalParameterRest::boxed(
                SPAN,
                ArenaVec::new_in(&self.ast),
                BindingRestElement::new(
                    SPAN,
                    BindingPattern::new_binding_identifier(SPAN, Self::SUPER_ARGS, &self.ast),
                    &self.ast,
                ),
                None,
                &self.ast,
            )
        });
        let params = FormalParameters::boxed(
            SPAN,
            FormalParameterKind::FormalParameter,
            ArenaVec::new_in(&self.ast),
            rest,
            &self.ast,
        );
        let mut statements = ArenaVec::new_in(&self.ast);
        if has_super_class {
            let arguments = ArenaVec::from_value_in(
                Argument::new_spread_element(
                    SPAN,
                    Expression::new_identifier(SPAN, Self::SUPER_ARGS, &self.ast),
                    &self.ast,
                ),
                &self.ast,
            );
            let call = Expression::new_call_expression(
                SPAN,
                Expression::new_super(SPAN, &self.ast),
                None,
                arguments,
                false,
                &self.ast,
            );
            statements.push(Statement::new_expression_statement(SPAN, call, &self.ast));
        }
        let body = FunctionBody::boxed(
            SPAN,
            ArenaVec::from_value_in(self.directive(), &self.ast),
            statements,
            &self.ast,
        );
        let function = Function::boxed(
            SPAN,
            FunctionType::FunctionExpression,
            None,
            false,
            false,
            false,
            None,
            None,
            params,
            None,
            Some(body),
            &self.ast,
        );
        ClassElement::new_method_definition(
            SPAN,
            MethodDefinitionType::MethodDefinition,
            ArenaVec::new_in(&self.ast),
            PropertyKey::new_static_identifier(SPAN, "constructor", &self.ast),
            function,
            MethodDefinitionKind::Constructor,
            false,
            false,
            false,
            false,
            None,
            &self.ast,
        )
    }

    fn directive(&self) -> oxc_ast::ast::Directive<'a> {
        Directive::new(
            SPAN,
            StringLiteral::new(SPAN, Self::DIRECTIVE, None, &self.ast),
            Self::DIRECTIVE,
            &self.ast,
        )
    }

    fn remove_sendable_decorator(decorators: &mut OxcVec<'a, Decorator<'a>>) {
        decorators.retain(|decorator| {
            ArkuiDecorator::new(decorator).name() != ArkuiDecoratorName::SENDABLE
        });
    }
}

struct ArkuiDecoratorRules;

impl ArkuiDecoratorRules {
    /// Returns true when the struct is a V1 component: `@Component` or `@CustomDialog`.
    ///
    /// `@ComponentV2` is intentionally excluded and lowered through `ComponentV2Lowerer`.
    fn is_component_struct(statement: &StructStatement<'_>) -> bool {
        statement.decorators.iter().any(|decorator| {
            ArkuiDecoratorName::is_component_struct(ArkuiDecorator::new(decorator).name())
        })
    }

    fn is_component_v2_struct(statement: &StructStatement<'_>) -> bool {
        statement.decorators.iter().any(|decorator| {
            ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::COMPONENT_V2
        })
    }

    fn primary_state(names: &[&str]) -> Option<&'static str> {
        ArkuiDecoratorName::primary_state(names)
    }
}

struct TopLevelStylesInventory<'program, 'ast, 'source> {
    program: &'program Program<'ast>,
    source_id: &'source str,
}

impl<'program, 'ast, 'source> TopLevelStylesInventory<'program, 'ast, 'source> {
    fn new(program: &'program Program<'ast>, source_id: &'source str) -> Self {
        Self { program, source_id }
    }

    fn collect(
        &self,
        allocator: &'ast Allocator,
    ) -> Result<Vec<StylesMethodInfo<'ast>>, EtsTransformError> {
        let mut styles = Vec::new();
        for statement in &self.program.body {
            match statement {
                Statement::FunctionDeclaration(function) => {
                    self.push_function(&mut styles, function, &[], allocator)?;
                }
                Statement::ExportDeclaration(export) => {
                    if let Declaration::FunctionDeclaration(function) = &export.declaration {
                        self.push_function(
                            &mut styles,
                            function.as_ref(),
                            &export.decorators,
                            allocator,
                        )?;
                    }
                }
                _ => {}
            }
        }
        Ok(styles)
    }

    fn push_function(
        &self,
        styles: &mut Vec<StylesMethodInfo<'ast>>,
        function: &Function<'ast>,
        extra_decorators: &[Decorator<'ast>],
        allocator: &'ast Allocator,
    ) -> Result<(), EtsTransformError> {
        if !function
            .decorators
            .iter()
            .chain(extra_decorators)
            .any(|decorator| ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::STYLES)
        {
            return Ok(());
        }
        if !function.params.items.is_empty() {
            return Err(EtsTransformError::Fatal {
                file: self.source_id.to_string(),
                message: "'@Styles' decorated functions and methods cannot have arguments."
                    .to_string(),
            });
        }
        let Some(name) = function.id.as_ref() else {
            return Ok(());
        };
        styles.push(StylesMethodInfo {
            name: name.name.to_string(),
            body: function
                .body
                .as_deref()
                .map(|body| allocator.alloc(body.clone_in(allocator)) as &FunctionBody<'ast>),
        });
        Ok(())
    }
}

struct ComponentRawObjectFields<'a, 'b> {
    statement: &'a StructStatement<'a>,
    simple_type_context: &'b SimpleTypeContext,
}

impl<'a, 'b> ComponentRawObjectFields<'a, 'b> {
    fn new(statement: &'a StructStatement<'a>, simple_type_context: &'b SimpleTypeContext) -> Self {
        Self {
            statement,
            simple_type_context,
        }
    }

    fn collect(&self) -> FxHashSet<String> {
        // SDK process_component_class.js::isNeedGetRawObject returns false for
        // the entire ComponentV2 context.
        if ArkuiDecoratorRules::is_component_v2_struct(self.statement) {
            return FxHashSet::default();
        }
        let mut names = FxHashSet::default();
        for element in &self.statement.body.body {
            let StructElement::PropertyDefinition(property) = element else {
                continue;
            };
            let decorator_names: Vec<&str> = property
                .decorators
                .iter()
                .map(|decorator| ArkuiDecorator::new(decorator).name())
                .collect();
            let Some(decorator) = ArkuiDecoratorRules::primary_state(&decorator_names) else {
                continue;
            };
            if decorator == ArkuiDecoratorName::BUILDER_PARAM {
                continue;
            }
            let Some(name) = property.key.static_name() else {
                continue;
            };
            let Some(type_node) = property
                .type_annotation
                .as_ref()
                .map(|annotation| &annotation.type_annotation)
            else {
                continue;
            };
            if !self
                .simple_type_context
                .is_simple_type_annotation(type_node)
            {
                names.insert(name.into_owned());
            }
        }
        names
    }
}

struct EntryRegistrationFactory<'a> {
    source_id: &'a str,
    hvigor_source_id: &'a str,
    options: &'a TransformOptions,
}

impl<'a> EntryRegistrationFactory<'a> {
    fn new(source_id: &'a str, hvigor_source_id: &'a str, options: &'a TransformOptions) -> Self {
        Self {
            source_id,
            hvigor_source_id,
            options,
        }
    }

    fn with_entry_argument<'b>(
        &self,
        argument: Option<&'b Expression<'b>>,
    ) -> Result<EntryRegistrationInfo<'b>, EtsTransformError> {
        let source_path = TransformSourcePath::new(self.source_id);
        let hvigor_source_path = TransformSourcePath::new(self.hvigor_source_id);
        let project_path =
            self.options
                .project_path
                .as_deref()
                .ok_or_else(|| EtsTransformError::Fatal {
                    file: self.source_id.to_string(),
                    message: "missing hvigor projectPath for @Entry route metadata".to_string(),
                })?;
        let project_root =
            self.options
                .project_root
                .as_deref()
                .ok_or_else(|| EtsTransformError::Fatal {
                    file: self.source_id.to_string(),
                    message: "missing hvigor projectRootPath for @Entry route metadata".to_string(),
                })?;
        let module_name = self
            .options
            .module_name
            .clone()
            .or_else(|| hvigor_source_path.module_name())
            .unwrap_or_else(|| "entry".to_string());
        let page_path = source_path.entry_page_path(project_path, project_root);
        let page_full_path = hvigor_source_path.extensionless_path("ets");

        Ok(EntryRegistrationInfo {
            bundle_name: self.options.bundle_name.clone().unwrap_or_default(),
            module_name,
            page_path,
            page_full_path,
            integrated_hsp: "false".to_string(),
            module_type: "followWithHap".to_string(),
            form_entry: self.options.form_entry,
            options: EntryOptions::from_argument(argument),
        })
    }
}

struct TransformSourcePath<'a> {
    id: &'a str,
}

impl<'a> TransformSourcePath<'a> {
    fn new(id: &'a str) -> Self {
        Self { id }
    }

    fn module_name(&self) -> Option<String> {
        HvigorModulePath::new(self.id).module_name()
    }

    fn entry_page_path(&self, project_path: &Path, project_root: &Path) -> String {
        HvigorModulePath::new(self.id).entry_page_path(project_path, project_root)
    }

    fn extensionless_path(&self, extension: &str) -> String {
        HvigorModulePath::new(self.id).extensionless_path(extension)
    }

    fn hvigor_source_path(&self, options: &TransformOptions) -> String {
        if let Some(path) = HvigorModulePath::new(self.id).hvigor_segment_path() {
            return path;
        }

        let source_path = Path::new(self.id);
        if let Some(project_root) = &options.project_root {
            if let Ok(relative) = source_path.strip_prefix(project_root) {
                return ArtifactPathSerializer::rel_to_slash(relative);
            }
        }

        self.id.to_string()
    }
}

#[cfg(test)]
mod tests;
