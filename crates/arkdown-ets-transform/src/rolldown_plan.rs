use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arkdown_path::{AbsPath, ModuleSpecifierText};
use oxc_allocator::Allocator;
use oxc_ast_visit::VisitMut;
use oxc_codegen::{CodegenOptions, IndentChar};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::SourceType;
use rolldown_sourcemap::SourceMapBuilder;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::api_transform;
use crate::api_transform::event_emitter::EventEmitterImportRewriter;
use crate::error::EtsTransformError;
use crate::lazy_import::LazyImportRolldownAdapter;
use crate::module_rewrite::{
    ModuleAnalyzerKind, ModuleSourceFilePrinter, ModuleSourceFileSpanEraser,
    TypeOnlyNamedImportSplitter, TypeOnlyReexportSplitter,
};
use crate::resource_transform::{
    ResourceReference, ResourceReferenceIndex, ResourceReferenceRewriter, ResourceTransformContext,
};
use crate::source::{
    CompileArktsSourceType, SourceClassifier, SourceFileKind, TransformSourceSnapshot,
};
use crate::source_map;
use crate::transform::api_available::ApiAvailableTransformOptions;
use crate::transform::source_emit::HvigorTypescriptPrinter;
use crate::transform::{
    self, CheckerMode, InsightIntentSchemaCatalog, ModuleExportIndex, NavigationRouteMap,
    SystemApiCheckOptions, TransformOptions,
};
use crate::typescript_emit::{
    ConstEnumExportCache, ImportedConstEnumRewriter, TypeScriptDirectiveFlags,
    TypeScriptEnumValueRewriter, TypeScriptJavascriptEmitOutput, TypeScriptJavascriptEmitter,
};
use crate::visual::VisualSourceInjector;
use arkdown_core::{
    ArkTscTarget, ArktsLanguageVersion, InsightIntentFormBinding, ModuleSource, ModuleType,
    ResolvedId,
};

/// Narrow transform plan consumed by the Rolldown ETS transform adapter.
///
/// The plan is built before Rolldown starts. Hook execution receives source
/// text from Rolldown and must not read project, SDK, profile, or resource
/// metadata.
#[derive(Debug, Clone)]
pub struct RolldownEtsTransformPlan {
    pub arkts_language_version: ArktsLanguageVersion,
    api_available: ApiAvailableTransformOptions,
    system_api: SystemApiCheckOptions,
    pub bundle_name: String,
    pub module_name: String,
    pub project_root: PathBuf,
    pub project_path: PathBuf,
    pub module_roots: Vec<(String, PathBuf)>,
    pub api_version: u32,
    pub resource_refs: Arc<FxHashMap<String, ResourceReference>>,
    resource_reference_index: Arc<ResourceReferenceIndex>,
    resource_context: ResourceTransformContext,
    pub component_metadata: Arc<transform::component_meta::ComponentMetadata>,
    pub sources: TransformSourceSnapshot,
    type_facts: transform::ArktsTypeFactsSnapshot,
    const_enum_exports: Arc<ConstEnumExportCache>,
    pub(crate) module_exports: ModuleExportIndex,
    form_entry_source_ids: Arc<FxHashSet<String>>,
    insight_intent_form_bindings: Arc<FxHashMap<String, InsightIntentFormBinding>>,
    insight_intent_schema_catalog: InsightIntentSchemaCatalog,
    entry_source_ids: Arc<FxHashSet<String>>,
    navigation_routes: NavigationRouteMap,
    preview_mode: bool,
    check_entry: bool,
    pub checker_mode: CheckerMode,
    pub should_emit_js: bool,
    /// `arkOptions.tscConfig.targetESVersion` selected by the latest SDK
    /// checker before `etsTransform` emits JavaScript for suppression-pragmas.
    pub tsc_target: ArkTscTarget,
    pub module_analyzer: ModuleAnalyzerKind,
    /// `arkOptions.autoLazyImport` + `autoLazyFilter` + module package name,
    /// forwarded to `TransformOptions` for the SDK `process_lazy_import.js`
    /// rewrite.
    pub auto_lazy_import: bool,
    pub auto_lazy_filter: Option<arkdown_core::AutoLazyFilter>,
    pub pkg_name: Option<String>,
    /// `arkOptions.expandImportPath` (SDK `import_path_expand.js`).
    pub expand_import_path: Option<arkdown_core::ExpandImportPath>,
    /// Remote HSP/HAR package names (OHM maps) that never expand.
    pub hsp_har_package_names: FxHashSet<String>,
    /// Hvigor configures `mockParams` for every Ark compile; ets-loader removes
    /// `@MockSetup` in every non-Preview transform.
    pub disable_mock_setup_decorator: bool,
}

impl RolldownEtsTransformPlan {
    #[expect(
        clippy::too_many_arguments,
        reason = "stable crate boundary; optional behavior is configured through typed with_* methods"
    )]
    pub fn new(
        bundle_name: impl Into<String>,
        module_name: impl Into<String>,
        project_root: PathBuf,
        project_path: PathBuf,
        module_roots: Vec<(String, PathBuf)>,
        api_version: u32,
        resource_refs: FxHashMap<String, ResourceReference>,
        component_metadata: transform::component_meta::ComponentMetadata,
        sources: TransformSourceSnapshot,
    ) -> Self {
        let module_name = module_name.into();
        let module_exports = ModuleExportIndex::build(&sources, &module_roots, &component_metadata);
        let const_enum_exports = Arc::new(ConstEnumExportCache::new(sources.clone()));
        let resource_reference_index = Arc::new(ResourceReferenceIndex::new(&resource_refs));
        Self {
            arkts_language_version: ArktsLanguageVersion::default(),
            api_available: ApiAvailableTransformOptions::default(),
            system_api: SystemApiCheckOptions::default(),
            bundle_name: bundle_name.into(),
            module_name,
            project_root,
            project_path,
            module_roots,
            api_version,
            resource_refs: Arc::new(resource_refs),
            resource_reference_index,
            resource_context: ResourceTransformContext::default(),
            component_metadata: Arc::new(component_metadata),
            sources,
            type_facts: transform::ArktsTypeFactsSnapshot::default(),
            const_enum_exports,
            module_exports,
            form_entry_source_ids: Arc::default(),
            insight_intent_form_bindings: Arc::default(),
            insight_intent_schema_catalog: InsightIntentSchemaCatalog::default(),
            entry_source_ids: Arc::default(),
            navigation_routes: NavigationRouteMap::default(),
            preview_mode: false,
            check_entry: false,
            checker_mode: CheckerMode::Oxc,
            should_emit_js: false,
            tsc_target: ArkTscTarget::default(),
            module_analyzer: ModuleAnalyzerKind::default(),
            auto_lazy_import: false,
            auto_lazy_filter: None,
            pkg_name: None,
            expand_import_path: None,
            hsp_har_package_names: FxHashSet::default(),
            disable_mock_setup_decorator: false,
        }
    }

    /// Set `arkOptions.expandImportPath` (SDK `import_path_expand.js`).
    pub fn with_expand_import_path(
        mut self,
        expand: Option<arkdown_core::ExpandImportPath>,
    ) -> Self {
        self.expand_import_path = expand;
        self
    }

    pub fn with_hsp_har_package_names(mut self, names: FxHashSet<String>) -> Self {
        self.hsp_har_package_names = names;
        self
    }

    pub fn with_disable_mock_setup_decorator(mut self, disable: bool) -> Self {
        self.disable_mock_setup_decorator = disable;
        self
    }

    /// Enable `arkOptions.autoLazyImport` with its include/exclude filter and
    /// the owning module's package name (SDK `meta.pkgName`).
    pub fn with_auto_lazy_import(
        mut self,
        enabled: bool,
        filter: Option<arkdown_core::AutoLazyFilter>,
        pkg_name: Option<String>,
    ) -> Self {
        self.auto_lazy_import = enabled;
        self.auto_lazy_filter = filter;
        self.pkg_name = pkg_name;
        self
    }

    pub fn with_checker_mode(mut self, checker_mode: CheckerMode) -> Self {
        self.checker_mode = checker_mode;
        self
    }

    pub fn with_type_facts(mut self, type_facts: transform::ArktsTypeFactsSnapshot) -> Self {
        self.type_facts = type_facts;
        self
    }

    /// Select the explicitly requested ArkTS compiler language and rebuild the
    /// cross-file export index with the matching OXC parser mode.
    pub fn with_arkts_language_version(mut self, version: ArktsLanguageVersion) -> Self {
        if self.arkts_language_version == version {
            return self;
        }
        self.arkts_language_version = version;
        self.module_exports = ModuleExportIndex::build_with_version(
            &self.sources,
            &self.module_roots,
            &self.component_metadata,
            version,
        );
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

    pub fn with_system_api_context(mut self, context: SystemApiCheckOptions) -> Self {
        self.system_api = context;
        self
    }

    pub fn with_resource_context(mut self, context: ResourceTransformContext) -> Self {
        self.resource_context = context;
        self
    }

    pub fn with_should_emit_js(mut self, should_emit_js: bool) -> Self {
        self.should_emit_js = should_emit_js;
        self
    }

    /// Set the target used by the SDK's TypeScript JavaScript-emission path.
    pub fn with_tsc_target(mut self, tsc_target: ArkTscTarget) -> Self {
        self.tsc_target = tsc_target;
        self
    }

    pub fn with_module_analyzer(mut self, module_analyzer: ModuleAnalyzerKind) -> Self {
        self.module_analyzer = module_analyzer;
        self
    }

    pub fn with_form_entry_source_ids(mut self, form_entry_source_ids: FxHashSet<String>) -> Self {
        self.form_entry_source_ids = Arc::new(form_entry_source_ids);
        self
    }

    pub fn with_insight_intent_form_bindings(
        mut self,
        bindings: FxHashMap<String, InsightIntentFormBinding>,
    ) -> Self {
        self.insight_intent_form_bindings = Arc::new(bindings);
        self
    }

    pub fn with_insight_intent_schema_catalog(
        mut self,
        catalog: InsightIntentSchemaCatalog,
    ) -> Self {
        self.insight_intent_schema_catalog = catalog;
        self
    }

    pub fn with_entry_source_ids(mut self, entry_source_ids: FxHashSet<String>) -> Self {
        self.entry_source_ids = Arc::new(entry_source_ids);
        self
    }

    pub fn with_preview_mode(mut self, preview_mode: bool) -> Self {
        self.preview_mode = preview_mode;
        self
    }

    pub fn with_check_entry(mut self, check_entry: bool) -> Self {
        self.check_entry = check_entry;
        self
    }

    /// Attach router-map metadata prepared by the coordinator before Rolldown
    /// starts. This preserves the SDK `createNavigationInit` lifecycle without
    /// allowing transform hooks to read profile artifacts.
    pub fn with_navigation_routes(mut self, navigation_routes: NavigationRouteMap) -> Self {
        self.navigation_routes = navigation_routes;
        self
    }

    pub fn transform_options_for(&self, id: &str) -> TransformOptions {
        let form_entry = self.is_form_entry(id);
        TransformOptions {
            arkts_language_version: self.arkts_language_version,
            api_version: self.api_version,
            api_available: self.api_available,
            system_api: self.system_api.clone(),
            bundle_name: Some(self.bundle_name.clone()),
            module_name: Some(self.module_name.clone()),
            project_root: Some(self.project_root.clone()),
            project_path: Some(self.project_path.clone()),
            module_roots: self.module_roots.clone(),
            component_metadata: Arc::clone(&self.component_metadata),
            sources: self.sources.clone(),
            type_facts: self.type_facts.clone(),
            module_exports: self.module_exports.clone(),
            navigation_routes: self.navigation_routes.clone(),
            form_entry,
            insight_intent_form_binding: self.insight_intent_form_binding(id),
            insight_intent_schema_catalog: self.insight_intent_schema_catalog.clone(),
            entry_source: self.is_entry_source(id),
            preview_mode: self.preview_mode,
            check_entry: self.check_entry,
            partial_update: true,
            checker_mode: self.checker_mode,
            auto_lazy_import: self.auto_lazy_import,
            auto_lazy_filter: self.auto_lazy_filter.clone(),
            pkg_name: self.pkg_name.clone(),
            expand_import_path: self.expand_import_path.clone(),
            hsp_har_package_names: self.hsp_har_package_names.clone(),
            module_source_file_stage: true,
            disable_mock_setup_decorator: self.disable_mock_setup_decorator,
        }
    }

    fn is_form_entry(&self, id: &str) -> bool {
        if self.form_entry_source_ids.contains(id) {
            return true;
        }
        let normalized = TransformEntrySourceId::new(id).normalized();
        self.form_entry_source_ids.contains(normalized.as_str())
    }

    fn insight_intent_form_binding(&self, id: &str) -> Option<InsightIntentFormBinding> {
        if let Some(binding) = self.insight_intent_form_bindings.get(id) {
            return Some(binding.clone());
        }
        let normalized = TransformEntrySourceId::new(id).normalized();
        self.insight_intent_form_bindings
            .get(normalized.as_str())
            .cloned()
    }

    fn is_entry_source(&self, id: &str) -> bool {
        if self.entry_source_ids.contains(id) {
            return true;
        }
        let normalized = TransformEntrySourceId::new(id).normalized();
        self.entry_source_ids.contains(normalized.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct RolldownEtsTransformOutput {
    pub code: String,
    /// Non-fatal source-compiler diagnostics emitted by transform-time checks.
    pub warnings: Vec<EtsTransformError>,
    /// Hvigor-compatible CompileArkTS source snapshot. When Rolldown needs a
    /// parser-only adaptation such as lowering `import lazy` to a standard
    /// import, this retains the pre-adaptation ArkTS form. It is shared with
    /// the ModuleMode collector instead of copying it again after Rolldown's
    /// internal TypeScript scan.
    pub module_source_code: Option<Arc<str>>,
    pub source_map: Option<String>,
    pub record_module_source_code: bool,
    /// Whether the source retained for `ModuleSourceFile` is JavaScript.
    /// This is distinct from [`Self::emits_javascript`]: when
    /// `projectConfig.shouldEmitJs` is true, Rolldown receives emitted JS but
    /// `processKitImport` still records the transformed TypeScript AST for
    /// genAbc. Only JS inputs and the SDK's ts-noCheck/ts-ignore path retain
    /// JavaScript as the bytecode source.
    pub module_source_emits_javascript: bool,
    pub emits_javascript: bool,
    /// The SDK TypeScript emitter gives a `//@ts-noCheck` source containing
    /// only comments the syntactic map `"mappings":";"`. Rolldown's map
    /// composition currently drops that empty-line sentinel, so the
    /// genAbc-equivalent collector restores it from this typed transform fact.
    pub is_comment_only_javascript: bool,
    pub is_shared_module: bool,
    pub insight_intent: transform::InsightIntentSourceMetadata,
    pub crossplatform_collection: transform::CrossplatformSourceCollection,
}

/// Production ETS transform engine used by the Rolldown plugin adapter.
pub struct RolldownEtsTransformer {
    plan: Arc<RolldownEtsTransformPlan>,
}

impl RolldownEtsTransformer {
    pub fn new(plan: Arc<RolldownEtsTransformPlan>) -> Self {
        Self { plan }
    }

    pub fn transform(
        &self,
        code: &str,
        id: &str,
    ) -> Result<Option<RolldownEtsTransformOutput>, EtsTransformError> {
        let source_kind = SourceClassifier::classify(id);
        if !source_kind.is_ets_transform_input() {
            let rolldown_source = if source_kind.is_javascript_runtime() {
                LazyImportRolldownAdapter::new(code, id, ArktsLanguageVersion::V1_1).lower()
            } else {
                Cow::Borrowed(code)
            };
            let code_for_rolldown = rolldown_source.as_ref();
            let javascript_source_map = (source_kind.is_javascript_runtime()
                && matches!(rolldown_source, Cow::Borrowed(_)))
            .then(|| source_map::JavaScriptIdentitySourceMapBuilder::new(code, id).build());
            let crossplatform_collection = if source_kind.is_javascript_runtime()
                && Path::new(id)
                    .extension()
                    .and_then(|extension| extension.to_str())
                    == Some("js")
                && self.plan.system_api.collects_crossplatform()
            {
                let allocator = Allocator::default();
                let source_type = SourceType::from_path(id)
                    .unwrap_or_else(|_| SourceType::mjs().with_module(true));
                let parsed = Parser::new(&allocator, code_for_rolldown, source_type).parse();
                if parsed.diagnostics.is_empty() {
                    transform::CrossplatformSourceCollection::modules_from_program(
                        &parsed.program,
                        self.plan.system_api.module_prefixes(),
                    )
                } else {
                    transform::CrossplatformSourceCollection::default()
                }
            } else {
                transform::CrossplatformSourceCollection::default()
            };
            let pipeline = HvigorSourcePipeline::new(self, id);
            if pipeline.should_finalize_unclassified_source(code_for_rolldown) {
                let finalization = pipeline.finalize_source(code_for_rolldown)?;
                if finalization.code != code_for_rolldown || !finalization.warnings.is_empty() {
                    return Ok(Some(RolldownEtsTransformOutput {
                        code: finalization.code.into_owned(),
                        warnings: finalization.warnings,
                        module_source_code: None,
                        source_map: javascript_source_map,
                        record_module_source_code: false,
                        module_source_emits_javascript: source_kind.is_javascript_runtime(),
                        emits_javascript: false,
                        is_comment_only_javascript: false,
                        is_shared_module: false,
                        insight_intent: transform::InsightIntentSourceMetadata::default(),
                        crossplatform_collection,
                    }));
                }
            }
            if !crossplatform_collection.is_empty() {
                return Ok(Some(RolldownEtsTransformOutput {
                    code: code_for_rolldown.to_string(),
                    warnings: Vec::new(),
                    module_source_code: None,
                    source_map: javascript_source_map,
                    record_module_source_code: false,
                    module_source_emits_javascript: source_kind.is_javascript_runtime(),
                    emits_javascript: false,
                    is_comment_only_javascript: false,
                    is_shared_module: false,
                    insight_intent: transform::InsightIntentSourceMetadata::default(),
                    crossplatform_collection,
                }));
            }
            if let Some(source_map) = javascript_source_map {
                return Ok(Some(RolldownEtsTransformOutput {
                    code: code.to_string(),
                    warnings: Vec::new(),
                    module_source_code: None,
                    source_map: Some(source_map),
                    record_module_source_code: false,
                    module_source_emits_javascript: source_kind.is_javascript_runtime(),
                    emits_javascript: false,
                    is_comment_only_javascript: false,
                    is_shared_module: false,
                    insight_intent: transform::InsightIntentSourceMetadata::default(),
                    crossplatform_collection,
                }));
            }
            if matches!(rolldown_source, Cow::Owned(_)) {
                return Ok(Some(RolldownEtsTransformOutput {
                    code: code_for_rolldown.to_string(),
                    warnings: Vec::new(),
                    module_source_code: None,
                    source_map: None,
                    record_module_source_code: false,
                    module_source_emits_javascript: source_kind.is_javascript_runtime(),
                    emits_javascript: false,
                    is_comment_only_javascript: false,
                    is_shared_module: false,
                    insight_intent: transform::InsightIntentSourceMetadata::default(),
                    crossplatform_collection,
                }));
            }
            return Ok(None);
        }
        let is_ets = matches!(source_kind, SourceFileKind::Ets);

        let original_source = code;
        let directive_flags = TypeScriptDirectiveFlags::from_source(original_source, id);
        // SDK rollup-plugin-ets-typescript.ts::getShouldEmitJs selects emit
        // whenever projectConfig.shouldEmitJs is true. When it is false, a
        // source suppression directive still selects emit unless @keepTs is
        // also present.
        let emits_javascript = self.plan.should_emit_js || directive_flags.should_emit_javascript();
        let is_comment_only_javascript = emits_javascript && directive_flags.is_comment_only();
        let is_shared_module = directive_flags.is_shared_module();
        let pipeline = HvigorSourcePipeline::new(self, id);
        let visual_source = if is_ets && !self.plan.arkts_language_version.is_static() {
            self.plan
                .sources
                .visual_source(Path::new(id))
                .map(|generated| {
                    VisualSourceInjector::new(original_source, id, generated).transform()
                })
                .transpose()?
                .flatten()
        } else {
            None
        };
        let code = pipeline.prepare_source(
            visual_source
                .as_ref()
                .map_or(original_source, |source| source.code.as_str()),
        );

        if !is_ets && !self.plan.arkts_language_version.is_static() {
            let crossplatform_collection =
                self.dynamic_typescript_system_api_collection(code.as_ref(), id)?;
            let finalization = pipeline.finalize_source(code.as_ref())?;
            // SDK genAbc lazy transform runs on every compile input after the
            // regular transform; TS/JS files are covered too.
            let final_code = crate::transform::apply_auto_lazy_import(
                finalization.code.as_ref(),
                id,
                &self.plan.transform_options_for(id),
            );
            let module_source_code = (!directive_flags.should_emit_javascript())
                .then(|| Arc::<str>::from(final_code.as_ref()));
            let emitted =
                self.emit_javascript_if_required(final_code.as_ref(), id, directive_flags)?;
            let source_map = if is_comment_only_javascript {
                Some(source_map::TypeScriptCommentOnlySourceMap::new(id).build())
            } else {
                emitted.source_map.or_else(|| {
                    source_map::HvigorStandardSourceMapBuilder::new(
                        &emitted.code,
                        original_source,
                        id,
                    )
                    .build()
                    .or_else(|| source_map::TokenSourceMapBuilder::new(&emitted.code, id).build())
                })
            };
            return Ok(Some(RolldownEtsTransformOutput {
                code: emitted.code,
                warnings: finalization.warnings,
                module_source_code,
                source_map,
                record_module_source_code: false,
                module_source_emits_javascript: directive_flags.should_emit_javascript(),
                emits_javascript,
                is_comment_only_javascript,
                is_shared_module,
                insight_intent: transform::InsightIntentSourceMetadata::default(),
                crossplatform_collection,
            }));
        }

        let source = ModuleSource {
            id: ResolvedId {
                id: id.to_string(),
                external: false,
                module_type: ModuleType::Ets,
                ..Default::default()
            },
            content: code.as_ref().to_string(),
            source_map: None,
        };

        let options = self.plan.transform_options_for(id);

        let transformed = transform::ArktsTransformer::with_options(&source, &options)
            .transform_with_source_map()?;

        if let Some(projection) = transformed.rolldown_projection.as_deref() {
            let projection = pipeline.finalize_source(projection)?;
            let mut warnings = transformed.warnings.clone();
            warnings.extend(projection.warnings);
            return Ok(Some(RolldownEtsTransformOutput {
                code: projection.code.into_owned(),
                warnings,
                module_source_code: Some(Arc::from(transformed.code)),
                source_map: None,
                record_module_source_code: false,
                module_source_emits_javascript: false,
                emits_javascript: false,
                is_comment_only_javascript: false,
                is_shared_module,
                insight_intent: transformed.insight_intent,
                crossplatform_collection: transformed.crossplatform_collection,
            }));
        }

        // `ModuleSourceFile` runs after ArkTS lowering and can replace one
        // source statement with several statements (notably `@kit.*` imports).
        // Keep its map as a separate edge in the map chain. Passing the
        // pre-finalization map through unchanged makes every following line
        // point at the wrong generated location in the aggregated hvigor map.
        // SDK ordering: `expandAllImportPaths` runs inside the TS emit
        // transformer chain before `processKitImport` (the expander skips
        // `@kit.*` sources), then `process_lazy_import` rewrites value
        // imports into `import lazy` in genAbc. The expander now runs inside
        // the transform pipeline (before codegen) so the source map covers
        // the expanded imports.
        let finalization =
            pipeline.finalize_transformed_source_with_source_map(&transformed.code)?;
        let HvigorSourceFinalization {
            code: finalized_code,
            source_map: finalization_source_map,
            warnings: finalization_warnings,
        } = finalization;
        let mut warnings = transformed.warnings.clone();
        warnings.extend(finalization_warnings);
        let final_source = match crate::transform::apply_auto_lazy_import(
            finalized_code.as_ref(),
            id,
            &self.plan.transform_options_for(id),
        ) {
            Cow::Owned(lowered) => Cow::Owned(lowered),
            Cow::Borrowed(_) => finalized_code,
        };
        let final_source = final_source.as_ref();

        if transformed.code == code.as_ref() {
            // The SDK etsTransform printer still formats a plain `.ets` file
            // even when no ArkUI syntax was lowered. Its TypeScript output uses
            // four-space indentation before ModuleMode writes the CompileArkTS
            // cache source consumed by es2abc.
            // For a resolvable module request, ModuleSourceFile creates a fresh
            // declaration around the retained import/export clause before the
            // TypeScript printer emits both text and mappings. Preserve that
            // source-node boundary while the original spans are still
            // available; reparsing later in the cache writer loses them.
            let module_source_print = (!directive_flags.should_emit_javascript()
                && finalization_source_map.is_none())
            .then(|| {
                ModuleSourceFilePrinter::new(
                    id,
                    final_source,
                    CompileArktsSourceType::from_id_with_version(
                        id,
                        self.plan.arkts_language_version,
                    ),
                )
                .print()
            })
            .flatten();
            let (formatted_source, module_source_map) = match module_source_print {
                Some(printed) => (printed.code, printed.source_map),
                None => (pipeline.format_source(final_source), None),
            };
            let module_source_code = (!directive_flags.should_emit_javascript())
                .then(|| Arc::<str>::from(formatted_source.as_str()));
            let emitted =
                self.emit_javascript_if_required(formatted_source.as_str(), id, directive_flags)?;
            let TypeScriptJavascriptEmitOutput {
                code: emitted_code,
                source_map: emitted_map,
            } = emitted;
            let source_map = if is_comment_only_javascript {
                Some(source_map::TypeScriptCommentOnlySourceMap::new(id).build())
            } else {
                let source_map_before_emit = finalization_source_map.or(module_source_map);
                match (source_map_before_emit, emitted_map) {
                    (Some(transform_map), Some(emit_map)) => {
                        Some(self.collapse_source_maps(id, &transform_map, &emit_map)?)
                    }
                    (Some(transform_map), None) => Some(transform_map),
                    (None, Some(emit_map)) => Some(emit_map),
                    (None, None) => source_map::HvigorStandardSourceMapBuilder::new(
                        &emitted_code,
                        original_source,
                        id,
                    )
                    .build()
                    .or_else(|| source_map::TokenSourceMapBuilder::new(&emitted_code, id).build()),
                }
            };
            return Ok(Some(RolldownEtsTransformOutput {
                code: emitted_code,
                warnings,
                module_source_code,
                source_map,
                record_module_source_code: false,
                module_source_emits_javascript: directive_flags.should_emit_javascript(),
                emits_javascript,
                is_comment_only_javascript,
                is_shared_module,
                insight_intent: transformed.insight_intent,
                crossplatform_collection: transformed.crossplatform_collection,
            }));
        }

        let module_source_code =
            (!directive_flags.should_emit_javascript()).then(|| Arc::<str>::from(final_source));
        let emitted = self.emit_javascript_if_required(final_source, id, directive_flags)?;
        let transformed_source_map = match (
            visual_source
                .as_ref()
                .map(|source| source.source_map.as_str()),
            transformed.source_map,
        ) {
            (Some(visual_map), Some(transform_map)) => {
                Some(self.collapse_source_maps(id, visual_map, &transform_map)?)
            }
            (None, transform_map) => transform_map,
            (Some(visual_map), None) => Some(visual_map.to_string()),
        };
        let transformed_source_map = match (transformed_source_map, finalization_source_map) {
            (Some(transform_map), Some(finalization_map)) => Some(
                self.collapse_module_source_reprint_map(id, &transform_map, &finalization_map)?,
            ),
            (Some(transform_map), None) => Some(transform_map),
            (None, Some(finalization_map)) => Some(finalization_map),
            (None, None) => None,
        };
        let source_map = if is_comment_only_javascript {
            Some(source_map::TypeScriptCommentOnlySourceMap::new(id).build())
        } else {
            match (transformed_source_map, emitted.source_map) {
                (Some(transform_map), Some(emit_map)) => {
                    Some(self.collapse_source_maps(id, &transform_map, &emit_map)?)
                }
                (Some(transform_map), None) => Some(transform_map),
                (None, Some(emit_map)) => Some(emit_map),
                (None, None) => source_map::HvigorStandardSourceMapBuilder::new(
                    &emitted.code,
                    original_source,
                    id,
                )
                .build()
                .or_else(|| source_map::TokenSourceMapBuilder::new(&emitted.code, id).build()),
            }
        };
        Ok(Some(RolldownEtsTransformOutput {
            code: emitted.code,
            warnings,
            module_source_code,
            source_map,
            record_module_source_code: false,
            module_source_emits_javascript: directive_flags.should_emit_javascript(),
            emits_javascript,
            is_comment_only_javascript,
            is_shared_module,
            insight_intent: transformed.insight_intent,
            crossplatform_collection: transformed.crossplatform_collection,
        }))
    }

    fn dynamic_typescript_system_api_collection(
        &self,
        code: &str,
        id: &str,
    ) -> Result<transform::CrossplatformSourceCollection, EtsTransformError> {
        let allocator = Allocator::default();
        let options = self.plan.transform_options_for(id);
        let parsed = options.component_metadata.parse_ets(
            &allocator,
            code,
            CompileArktsSourceType::from_id_with_version(id, self.plan.arkts_language_version),
        );
        if !parsed.diagnostics.is_empty() {
            return Err(EtsTransformError::Fatal {
                file: id.to_string(),
                message: format!("ArkTS parse failed: {:?}", parsed.diagnostics),
            });
        }
        let mut collection = if options.system_api.collects_crossplatform() {
            transform::CrossplatformSourceCollection::from_program(
                &parsed.program,
                options.system_api.module_prefixes(),
                &options.component_metadata,
            )
        } else {
            transform::CrossplatformSourceCollection::default()
        };
        if options.system_api.collects_crossplatform() {
            if let Some(facts) = options.type_facts.file_facts(id) {
                collection.merge(&facts.crossplatform_dependencies);
            }
        }
        Ok(collection)
    }

    fn emit_javascript_if_required(
        &self,
        source: &str,
        id: &str,
        directive_flags: TypeScriptDirectiveFlags,
    ) -> Result<TypeScriptJavascriptEmitOutput, EtsTransformError> {
        if !self.plan.should_emit_js && !directive_flags.should_emit_javascript() {
            return Ok(TypeScriptJavascriptEmitOutput {
                code: source.to_string(),
                source_map: None,
            });
        }
        TypeScriptJavascriptEmitter::new(source, id, &self.plan.sources)
            .with_const_enum_export_cache(&self.plan.const_enum_exports)
            .with_tsc_target(self.plan.tsc_target)
            .with_hvigor_emit_wrapper()
            .emit_with_source_map()
            .ok_or_else(|| EtsTransformError::Fatal {
                file: id.to_string(),
                message: "failed to emit JavaScript for a TypeScript suppression pragma file"
                    .to_string(),
            })
    }

    fn collapse_source_maps(
        &self,
        id: &str,
        transform_map: &str,
        emit_map: &str,
    ) -> Result<String, EtsTransformError> {
        let transform_map = rolldown_sourcemap::OwnedSourceMap::from_json_string(transform_map)
            .map_err(|error| EtsTransformError::Fatal {
                file: id.to_string(),
                message: format!("failed to decode ArkTS transform source map: {error}"),
            })?;
        let emit_map =
            rolldown_sourcemap::OwnedSourceMap::from_json_string(emit_map).map_err(|error| {
                EtsTransformError::Fatal {
                    file: id.to_string(),
                    message: format!("failed to decode TypeScript emit source map: {error}"),
                }
            })?;
        Ok(rolldown_sourcemap::collapse_sourcemaps(&[
            transform_map.as_source_map(),
            emit_map.as_source_map(),
        ])
        .to_json_string())
    }

    /// Compose a Rust reprint with the preceding ArkUI transform without
    /// assigning source ownership to generated nodes.
    ///
    /// ets2bundle's `ModuleSourceFile::processTransformedTsModuleRequest`
    /// updates the same transformed `ts.SourceFile` later written by
    /// `process_module_files.ts::genContentAndSourceMapInfo`. Factory-created
    /// class/constructor/ArkUI nodes therefore keep synthesized positions and
    /// never gain mappings during module-request or type-import processing.
    /// Rust must reparse between those owning stages, so the reprint map is
    /// restricted to source positions that the first transform actually
    /// exposed. An approximate sourcemap lookup would otherwise project new
    /// mappings onto the closest retained identifier.
    fn collapse_module_source_reprint_map(
        &self,
        id: &str,
        transform_map: &str,
        reprint_map: &str,
    ) -> Result<String, EtsTransformError> {
        let transform = rolldown_sourcemap::OwnedSourceMap::from_json_string(transform_map)
            .map_err(|error| EtsTransformError::Fatal {
                file: id.to_string(),
                message: format!("failed to decode ArkTS transform source map: {error}"),
            })?;
        let reprint =
            rolldown_sourcemap::OwnedSourceMap::from_json_string(reprint_map).map_err(|error| {
                EtsTransformError::Fatal {
                    file: id.to_string(),
                    message: format!("failed to decode module-source reprint map: {error}"),
                }
            })?;
        let mapped_intermediate_positions = transform
            .get_tokens()
            .map(|token| (token.get_dst_line(), token.get_dst_col()))
            .collect::<FxHashSet<_>>();

        let mut builder = SourceMapBuilder::default();
        if let Some(file) = reprint.get_file() {
            builder.set_file(file);
        }
        for name in reprint.get_names() {
            builder.add_name(name);
        }
        for source in reprint.get_sources() {
            builder.set_source_and_content(source, "");
        }
        for token in reprint.get_tokens() {
            let Some(source_id) = token.get_source_id() else {
                continue;
            };
            if !mapped_intermediate_positions.contains(&(token.get_src_line(), token.get_src_col()))
            {
                continue;
            }
            builder.add_token(
                token.get_dst_line(),
                token.get_dst_col(),
                token.get_src_line(),
                token.get_src_col(),
                Some(source_id),
                token.get_name_id(),
            );
        }
        let filtered_reprint = builder.into_sourcemap();
        Ok(
            rolldown_sourcemap::collapse_sourcemaps(&[
                transform.as_source_map(),
                &filtered_reprint,
            ])
            .to_json_string(),
        )
    }

    pub fn compile_transform(
        &self,
        code: &str,
        id: &str,
    ) -> Result<Option<RolldownEtsTransformOutput>, EtsTransformError> {
        let Some(mut output) = self.transform(code, id)? else {
            return Ok(None);
        };
        output.record_module_source_code = SourceClassifier::classify(id).is_ets_transform_input();
        if output.record_module_source_code {
            let rolldown_source =
                LazyImportRolldownAdapter::new(&output.code, id, self.plan.arkts_language_version)
                    .lower();
            // ModuleMode writes the code produced by etsTransform, then
            // `ModuleSourceFile` rewrites its static requests. Rolldown's
            // internal TypeScript scan is allowed to normalize syntax for its
            // graph, but it must not become the CompileArkTS cache source:
            // it loses raw literal spelling on replacement AST nodes. Keep a
            // source snapshot for every ETS/TS input; for `import lazy`, pass
            // the lowered copy to Rolldown while retaining the ArkTS form for
            // the ModuleMode writer.
            // A transform may already have retained its source-facing text
            // (for example the non-emit TypeScript path). That snapshot does
            // not remove the separate Rolldown scanner adaptation: current
            // ets2bundle keeps `ModuleSourceFile`'s ArkTS AST while Rollup
            // receives a scanner-compatible standard import.
            if output.module_source_code.is_none() {
                output.module_source_code = Some(Arc::from(output.code.clone()));
            }
            if let Cow::Owned(rolldown_source) = rolldown_source {
                output.code = rolldown_source;
            }
        }
        // Hvigor normally writes transformed TypeScript for es2abc, but its
        // `hasTsNoCheckOrTsIgnoreFiles` path writes JavaScript instead. Both
        // shapes remain graph inputs and are materialized after Rolldown scan.
        Ok(Some(output))
    }
}

pub struct TransformEntrySourceId<'a> {
    id: &'a str,
}

impl<'a> TransformEntrySourceId<'a> {
    pub fn new(id: &'a str) -> Self {
        Self { id }
    }

    pub fn normalized(&self) -> String {
        let path = Path::new(self.id);
        if !path.has_root() {
            return self.id.to_string();
        }
        AbsPath::canonicalize(path)
            .map(AbsPath::into_path_buf)
            .unwrap_or_else(|_| arkdown_path::lexical_normalize(path))
            .into_os_string()
            .into_string()
            .unwrap_or_else(|_| self.id.to_string())
    }
}

struct HvigorSourcePipeline<'a> {
    transformer: &'a RolldownEtsTransformer,
    id: &'a str,
}

impl<'a> HvigorSourcePipeline<'a> {
    fn new(transformer: &'a RolldownEtsTransformer, id: &'a str) -> Self {
        Self { transformer, id }
    }

    fn prepare_source<'s>(&self, source: &'s str) -> Cow<'s, str> {
        HvigorPrepareSourcePass::new(self).run(source)
    }

    fn finalize_source<'s>(
        &self,
        source: &'s str,
    ) -> Result<HvigorSourceFinalization<'s>, EtsTransformError> {
        HvigorFinalizeSourcePass::new(self).run(source)
    }

    fn finalize_transformed_source_with_source_map<'s>(
        &self,
        source: &'s str,
    ) -> Result<HvigorSourceFinalization<'s>, EtsTransformError> {
        HvigorFinalizeSourcePass::new(self).run_transformed(source)
    }

    fn format_source(&self, source: &str) -> String {
        let allocator = Allocator::default();
        let source_type = CompileArktsSourceType::from_id_with_version(
            self.id,
            self.transformer.plan.arkts_language_version,
        );
        let parsed = Parser::new(&allocator, source, source_type).parse();
        if parsed.panicked || !parsed.diagnostics.is_empty() {
            return source.to_string();
        }
        HvigorTypescriptPrinter::print(
            &parsed.program,
            CodegenOptions {
                indent_char: IndentChar::Space,
                indent_width: 4,
                ..CodegenOptions::default()
            },
        )
    }

    fn should_finalize_unclassified_source(&self, source: &str) -> bool {
        source.contains("@kit.")
            || source.contains("@ohos:")
            || source.contains("@ohos.util.emitter")
            || source.contains("@system.emitter")
            || source.contains("$r")
            || source.contains("$rawfile")
            || (self.transformer.plan.api_version < 9 && source.contains("@ohos/"))
    }
}

struct HvigorPrepareSourcePass<'a, 'pipeline> {
    pipeline: &'pipeline HvigorSourcePipeline<'a>,
}

impl<'a, 'pipeline> HvigorPrepareSourcePass<'a, 'pipeline> {
    fn new(pipeline: &'pipeline HvigorSourcePipeline<'a>) -> Self {
        Self { pipeline }
    }

    fn run<'s>(&self, source: &'s str) -> Cow<'s, str> {
        let reexports = TypeOnlyReexportSplitter::new(
            self.pipeline.id,
            &self.pipeline.transformer.plan.sources,
        );
        let reexports =
            reexports.with_module_analyzer(self.pipeline.transformer.plan.module_analyzer);
        let allocator = Allocator::default();
        let source_type = self.source_type();
        let mut parsed = Parser::new(&allocator, source, source_type).parse();
        if !parsed.diagnostics.is_empty() {
            return Cow::Borrowed(source);
        }

        let changed = reexports.split_program(&allocator, &mut parsed.program);

        self.finish(source, &parsed.program, changed)
    }

    fn source_type(&self) -> SourceType {
        CompileArktsSourceType::from_id_with_version(
            self.pipeline.id,
            self.pipeline.transformer.plan.arkts_language_version,
        )
    }

    fn finish<'s>(
        &self,
        source: &'s str,
        program: &oxc_ast::ast::Program<'_>,
        changed: bool,
    ) -> Cow<'s, str> {
        if !changed {
            return Cow::Borrowed(source);
        }
        Cow::Owned(hvigor_stage_codegen(program))
    }
}

struct HvigorFinalizeSourcePass<'a, 'pipeline> {
    pipeline: &'pipeline HvigorSourcePipeline<'a>,
}

struct HvigorSourceFinalization<'s> {
    code: Cow<'s, str>,
    source_map: Option<String>,
    warnings: Vec<EtsTransformError>,
}

impl<'s> HvigorSourceFinalization<'s> {
    fn unchanged(source: &'s str) -> Self {
        Self {
            code: Cow::Borrowed(source),
            source_map: None,
            warnings: Vec::new(),
        }
    }

    fn changed(code: String, source_map: Option<String>, warnings: Vec<EtsTransformError>) -> Self {
        Self {
            code: Cow::Owned(code),
            source_map,
            warnings,
        }
    }

    fn unchanged_with_warnings(source: &'s str, warnings: Vec<EtsTransformError>) -> Self {
        Self {
            code: Cow::Borrowed(source),
            source_map: None,
            warnings,
        }
    }
}

impl<'a, 'pipeline> HvigorFinalizeSourcePass<'a, 'pipeline> {
    fn new(pipeline: &'pipeline HvigorSourcePipeline<'a>) -> Self {
        Self { pipeline }
    }

    fn run<'s>(&self, source: &'s str) -> Result<HvigorSourceFinalization<'s>, EtsTransformError> {
        self.run_with_source_map(source, ModuleSourceAstOwnership::Pending)
    }

    fn run_transformed<'s>(
        &self,
        source: &'s str,
    ) -> Result<HvigorSourceFinalization<'s>, EtsTransformError> {
        self.run_with_source_map(source, ModuleSourceAstOwnership::Applied)
    }

    fn run_with_source_map<'s>(
        &self,
        source: &'s str,
        module_source_ownership: ModuleSourceAstOwnership,
    ) -> Result<HvigorSourceFinalization<'s>, EtsTransformError> {
        let allocator = Allocator::default();
        let source_type = self.source_type();
        let mut parsed = Parser::new(&allocator, source, source_type).parse();
        if !parsed.diagnostics.is_empty() {
            return Ok(HvigorSourceFinalization::unchanged(source));
        }

        // The SDK transformer resolves local enum member values through the
        // TypeScript emit resolver before it writes ModuleSourceFile. Enable
        // OXC's matching enum evaluation so the Rust pass can make the same
        // substitution entirely inside the owning OXC transform.
        let semantic_ret = SemanticBuilder::new()
            .with_enum_eval(true)
            .build(&parsed.program);
        if !semantic_ret.diagnostics.is_empty() {
            return Ok(HvigorSourceFinalization::unchanged(source));
        }
        let scoping = semantic_ret.semantic.into_scoping();

        let mut changed = false;

        let imported_const_enum_rewrite = ImportedConstEnumRewriter::new(
            self.pipeline.id,
            &self.pipeline.transformer.plan.sources,
        )
        .with_const_enum_export_cache(&self.pipeline.transformer.plan.const_enum_exports)
        .rewrite_program_with_inlined_imports(&allocator, &mut parsed.program);
        let imported_const_enum_changed = imported_const_enum_rewrite.changed;
        changed |= imported_const_enum_changed;

        let enum_value_changed =
            TypeScriptEnumValueRewriter::rewrite_program(&allocator, &mut parsed.program, &scoping);
        changed |= enum_value_changed;

        // The preceding enum passes replace value references with literals.
        // `ModuleSourceFile` decides whether an import is type-only after that
        // substitution, so its binding usage must come from the mutated AST
        // rather than the scope built from the original source. Rebuild only
        // for the modules whose enum substitutions changed a reference.
        let rewritten_scoping = (imported_const_enum_changed || enum_value_changed)
            .then(|| {
                let semantic_ret = SemanticBuilder::new()
                    .with_enum_eval(true)
                    .build(&parsed.program);
                semantic_ret
                    .diagnostics
                    .is_empty()
                    .then(|| semantic_ret.semantic.into_scoping())
            })
            .flatten();
        let scoping_for_type_imports = rewritten_scoping.as_ref().unwrap_or(&scoping);

        changed |= TypeOnlyNamedImportSplitter::new(
            self.pipeline.id,
            &self.pipeline.transformer.plan.sources,
        )
        .with_module_analyzer(self.pipeline.transformer.plan.module_analyzer)
        .split_program_with_inlined_const_enum_imports(
            &allocator,
            &mut parsed.program,
            scoping_for_type_imports,
            &imported_const_enum_rewrite.inlined_import_locals,
        );

        // Latest SDK `process_kit_import.js::processKitImport` invokes
        // `getTypeExportImportAndConstEnumTransformer` before it replaces a
        // `@kit.*` declaration with factory-created system imports. Preserve
        // that ordering: the semantic scope still sees the original kit alias,
        // but it must be rebuilt after enum literals replace the alias's only
        // value references. Otherwise an SDK enum folded to `0`/`1` leaves a
        // spurious runtime `@ohos.*` import in the CompileArkTS handoff.
        changed |= api_transform::KitImportRewriter::new(source).rewrite_program(
            &allocator,
            &mut parsed.program,
            scoping_for_type_imports,
        );

        // SDK `ModuleSourceFile::processTransformedTsModuleRequest` revisits
        // every static import/export after kit lowering. For extend-SDK APIs,
        // `api_check_utils::moduleRequestCallback` resolves the inventoried
        // declaration header's `@bundle` target when its API requirement is
        // compatible with `compatibleSdkVersion`.
        let compatible_api_version = self.pipeline.transformer.plan.api_version;
        changed |= crate::StaticModuleSpecifierRewrite::new(
            source,
            self.pipeline.id,
            |specifier: &str| {
                self.pipeline
                    .transformer
                    .plan
                    .sources
                    .sdk_bundle_ohm_url(specifier, compatible_api_version)
            },
        )
        .rewrite_program(&allocator, &mut parsed.program);

        changed |= crate::StaticModuleSpecifierRewrite::new(
            source,
            self.pipeline.id,
            |specifier: &str| {
                EventEmitterImportRewriter::replacement_for(specifier).map(str::to_string)
            },
        )
        .rewrite_program(&allocator, &mut parsed.program);

        let api_version = self.pipeline.transformer.plan.api_version;
        changed |= crate::StaticModuleSpecifierRewrite::new(
            source,
            self.pipeline.id,
            |specifier: &str| {
                (api_version < 9)
                    .then(|| ModuleSpecifierText::new(specifier).legacy_system_specifier())
                    .flatten()
            },
        )
        .rewrite_program(&allocator, &mut parsed.program);

        let resource_rewrite = ResourceReferenceRewriter::new(
            source,
            self.pipeline.id,
            self.pipeline.transformer.plan.resource_refs.as_ref(),
            self.pipeline
                .transformer
                .plan
                .resource_reference_index
                .as_ref(),
            &self.pipeline.transformer.plan.bundle_name,
            Some(&self.pipeline.transformer.plan.module_name),
        )
        .with_context(self.pipeline.transformer.plan.resource_context.clone())
        .rewrite_program(&allocator, &mut parsed.program)?;
        changed |= resource_rewrite.changed;

        let changed_before_module_source = changed;

        // ModuleSourceFile rebuilds every declaration whose request will be
        // resolved to an OHM URL. The declaration shell and request literal
        // are factory-created nodes; only the retained import/export clause
        // owns source mappings. Apply that ownership before the printer even
        // when an earlier finalization pass (for example Kit expansion) has
        // already made this stage non-identity.
        let mut module_source_spans = ModuleSourceFileSpanEraser::default();
        module_source_spans.visit_program(&mut parsed.program);
        changed |= module_source_spans.changed;

        // `etsTransform` hands ModuleSourceFile the transformed SourceFile,
        // not printed text. ArktsTransformer has therefore already applied
        // the declaration ownership above on that same AST. If reparsing was
        // needed only to run this ownership pass again, retain the first
        // printer result and its source map. This preserves TypeScript's
        // synthesized-node identity instead of composing a second printer map.
        if module_source_ownership == ModuleSourceAstOwnership::Applied
            && !changed_before_module_source
        {
            return Ok(HvigorSourceFinalization::unchanged_with_warnings(
                source,
                resource_rewrite.warnings,
            ));
        }

        if !changed {
            return Ok(HvigorSourceFinalization::unchanged_with_warnings(
                source,
                resource_rewrite.warnings,
            ));
        }
        let generated = HvigorTypescriptPrinter::print_with_source_map(
            &parsed.program,
            CodegenOptions {
                source_map_path: Some(self.pipeline.id.into()),
                indent_char: IndentChar::Space,
                indent_width: 4,
                ..CodegenOptions::default()
            },
            source,
        );
        Ok(HvigorSourceFinalization::changed(
            generated.code,
            generated.source_map,
            resource_rewrite.warnings,
        ))
    }

    fn source_type(&self) -> SourceType {
        CompileArktsSourceType::from_id_with_version(
            self.pipeline.id,
            self.pipeline.transformer.plan.arkts_language_version,
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ModuleSourceAstOwnership {
    Pending,
    Applied,
}

fn hvigor_stage_codegen(program: &oxc_ast::ast::Program<'_>) -> String {
    HvigorTypescriptPrinter::print(
        program,
        CodegenOptions {
            indent_char: IndentChar::Space,
            indent_width: 4,
            ..CodegenOptions::default()
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watch_visual_fragments_enter_the_default_arkts_transform_before_arkui_lowering() {
        let id = PathBuf::from("/tmp/project/entry/src/main/ets/pages/Index.ets");
        let mut visual_sources = FxHashMap::default();
        visual_sources.insert(
            id.clone(),
            crate::VisualGeneratedSource {
                build: "    Column()".to_string(),
                ..crate::VisualGeneratedSource::default()
            },
        );
        let sources = TransformSourceSnapshot::default().with_visual_sources(visual_sources);
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/entry/src/main/ets"),
            Vec::new(),
            20,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            sources,
        )
        .with_checker_mode(CheckerMode::Off);
        let output = RolldownEtsTransformer::new(Arc::new(plan))
            .transform(
                "@Entry\n@Component\nstruct Index {\n  build() {}\n}\n",
                id.to_str().unwrap(),
            )
            .expect("visual transform")
            .expect("ETS output");

        assert!(output.code.contains("Column.create()"), "{}", output.code);
    }

    #[test]
    fn unclassified_ohm_source_finalizes_kit_imports() {
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/entry/src/main/ets"),
            Vec::new(),
            12,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        );
        let transformer = RolldownEtsTransformer::new(Arc::new(plan));
        let output = transformer
            .transform(
                r#"import { preferences } from "@kit.ArkData";
preferences.getPreferencesSync(undefined, { name: "x" });"#,
                "@bundle:com.example/datastore/Index",
            )
            .unwrap()
            .expect("unclassified source with kit import must be finalized");

        assert!(
            output
                .code
                .contains("import preferences from \"@ohos.data.preferences\""),
            "got: {}",
            output.code
        );
        assert!(
            !output.code.contains("@kit.ArkData"),
            "got: {}",
            output.code
        );
    }

    #[test]
    fn api22_resource_modules_and_dynamic_resources_use_runtime_id_getters() {
        let context = ResourceTransformContext {
            min_api_version: 22,
            bundle_name: "com.example".to_string(),
            module_name: "entry".to_string(),
            ..ResourceTransformContext::default()
        };
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/entry/src/main/ets"),
            Vec::new(),
            22,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        )
        .with_resource_context(context);
        let output = RolldownEtsTransformer::new(Arc::new(plan))
            .transform(
                concat!(
                    "export const shared = $r('[feature].string.title', 'fallback');\n",
                    "export const dynamic = $r(resourceName);\n",
                    "export const templateFallback = $r(`app.unsupported.missing`);\n"
                ),
                "/tmp/project/entry/src/main/ets/Resources.ets",
            )
            .expect("resource transform")
            .expect("resource output");

        assert_eq!(
            output.code.matches("get id()").count(),
            3,
            "{}",
            output.code
        );
        assert!(output
            .code
            .contains("typeof __getResourceId__ === \"function\""));
        assert!(output.code.contains("\"moduleName\": \"feature\""));
        assert!(output
            .code
            .contains("params: ['[feature].string.title', 'fallback']"));
    }

    #[test]
    fn api23_har_uses_resource_id_har_for_app_but_preserves_system_ids() {
        let mut references = FxHashMap::default();
        references.insert(
            "app.string.title".to_string(),
            ResourceReference {
                id: 0x0100_0001,
                runtime_type: 10003,
                bundle_name: "com.example".to_string(),
                module_name: "library".to_string(),
            },
        );
        references.insert(
            "sys.string.ok".to_string(),
            ResourceReference {
                id: 0x0700_0001,
                runtime_type: 10003,
                bundle_name: "com.example".to_string(),
                module_name: "library".to_string(),
            },
        );
        let context = ResourceTransformContext {
            min_api_version: 23,
            compile_har: true,
            bundle_name: "com.example".to_string(),
            module_name: "library".to_string(),
            ..ResourceTransformContext::default()
        };
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "library",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/library/src/main/ets"),
            Vec::new(),
            23,
            references,
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        )
        .with_resource_context(context);
        let output = RolldownEtsTransformer::new(Arc::new(plan))
            .transform(
                concat!(
                    "export const appValue = $r('app.string.title', 'fallback');\n",
                    "export const sysValue = $r('sys.string.ok');\n"
                ),
                "/tmp/project/library/src/main/ets/Resources.ets",
            )
            .expect("HAR resource transform")
            .expect("HAR output");

        assert!(output.code.contains("get id()"), "{}", output.code);
        assert!(
            output.code.contains("__resourceIdHar__(this)"),
            "{}",
            output.code
        );
        assert!(output.code.contains("id: 117440513") || output.code.contains("\"id\": 117440513"));
        assert!(output.code.contains("'app.string.title', 'fallback'"));
        assert!(output
            .code
            .contains("\"bundleName\": \"__harDefaultBundleName__\""));
        assert!(output
            .code
            .contains("\"moduleName\": \"__harDefaultModuleName__\""));
    }

    #[test]
    fn dynamic_resource_names_import_generated_module_info_once() {
        let context = ResourceTransformContext {
            min_api_version: 22,
            ui_transform_optimization: true,
            bundle_name: "com.example".to_string(),
            module_name: "entry".to_string(),
            ..ResourceTransformContext::default()
        };
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/entry/src/main/ets"),
            Vec::new(),
            22,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        )
        .with_resource_context(context);
        let output = RolldownEtsTransformer::new(Arc::new(plan))
            .transform(
                "export const value = $r(resourceName);\n",
                "/tmp/project/entry/src/main/ets/Resources.ets",
            )
            .expect("dynamic resource transform")
            .expect("dynamic resource output");

        assert_eq!(output.code.matches("from \"ModuleInfo\"").count(), 1);
        assert!(output.code.contains("\"bundleName\": __BUNDLE_NAME__"));
        assert!(output.code.contains("\"moduleName\": __MODULE_NAME__"));
    }

    #[test]
    fn hms_sdk_bundle_annotations_rewrite_static_value_and_type_requests_like_hvigor() {
        let tmp = tempfile::tempdir().unwrap();
        let hms_api_root = tmp.path().join("hms/ets/api");
        std::fs::create_dir_all(&hms_api_root).unwrap();
        let declarations = [
            (
                "@hms.core.map.MapComponent.d.ets",
                r#"/**
 * @bundle com.huawei.hms.mapservice.kit/mapLibrary/ets/MapComponent 5.0.0(12)
 */
export struct MapComponent {}"#,
            ),
            (
                "@hms.core.map.map.d.ts",
                r#"/**
 * @bundle com.huawei.hms.mapservice.kit/mapLibrary/ets/map 5.0.0(12)
 */
declare const map: object;
export default map;"#,
            ),
            (
                "@hms.core.map.mapCommon.d.ts",
                r#"/**
 * @bundle com.huawei.hms.mapservice.kit/mapLibrary/ets/mapCommon 5.0.0(12)
 */
declare const mapCommon: object;
export default mapCommon;"#,
            ),
            (
                "@hms.core.map.navi.d.ts",
                "declare const navi: object;\nexport default navi;",
            ),
        ];
        let mut files = FxHashMap::default();
        for (file_name, source) in declarations {
            let path = hms_api_root.join(file_name);
            std::fs::write(&path, source).unwrap();
            files.insert(path, Arc::<str>::from(source));
        }
        let sources =
            TransformSourceSnapshot::new(files).with_sdk_declaration_roots(vec![hms_api_root]);
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            tmp.path().to_path_buf(),
            tmp.path().join("entry/src/main/ets"),
            Vec::new(),
            12,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            sources,
        );
        let transformer = RolldownEtsTransformer::new(Arc::new(plan));
        let output = transformer
            .transform(
                r#"import { MapComponent, map, mapCommon, navi } from "@kit.MapKit";
export type { MapComponent as ExternalMapComponent } from "@hms.core.map.MapComponent";
const allMapApis = [MapComponent, map, mapCommon, navi];"#,
                "@bundle:com.example/entry/ets/MapView",
            )
            .unwrap()
            .expect("MapKit source must be finalized");

        for record in ["MapComponent", "map", "mapCommon"] {
            assert!(
                output.code.contains(&format!(
                    "@bundle:com.huawei.hms.mapservice.kit/mapLibrary/ets/{record}"
                )),
                "SDK declaration annotation must produce the external bundle record for {record}:\n{}",
                output.code
            );
        }
        assert!(
            output.code.contains("@hms.core.map.navi"),
            "an SDK declaration without @bundle metadata remains a system API request:\n{}",
            output.code
        );
    }

    #[test]
    fn unclassified_js_source_finalizes_legacy_emitter_imports() {
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/entry/src/main/ets"),
            Vec::new(),
            12,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        );
        let transformer = RolldownEtsTransformer::new(Arc::new(plan));
        let source = r#"import emitter from '@system.emitter';
export { emitter };"#;
        let output = transformer
            .transform(source, "entry/src/main/ets/pages/emitter.js")
            .unwrap()
            .expect("unclassified JS legacy emitter import must be finalized");

        assert!(
            output.code.contains("@ohos.events.emitter"),
            "got: {}",
            output.code
        );
        assert!(
            !output.code.contains("@system.emitter"),
            "got: {}",
            output.code
        );
        assert!(
            output
                .code
                .contains("import emitter from \"@ohos.events.emitter\";"),
            "latest ets-loader prints a rewritten module request as a generated double-quoted literal: {}",
            output.code
        );
        let source_map = rolldown_sourcemap::OwnedSourceMap::from_json_string(
            output
                .source_map
                .as_deref()
                .expect("JavaScript combined map"),
        )
        .expect("decode JavaScript combined map");
        let expected_tokens = source.encode_utf16().count();
        assert_eq!(source_map.get_tokens().count(), expected_tokens);
    }

    #[test]
    fn crossplatform_collection_covers_typescript_and_javascript_inputs() {
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/entry/src/main/ets"),
            Vec::new(),
            12,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        )
        .with_system_api_context(
            SystemApiCheckOptions::default()
                .with_crossplatform_collection(true)
                .with_module_prefixes(["ohos", "system", "kit", "arkts", "arkui-x"]),
        );
        let transformer = RolldownEtsTransformer::new(Arc::new(plan));

        let typescript = transformer
            .compile_transform(
                "import call from '@arkui-x.telephony';\nForEach();\nif (call) {}\n",
                "/tmp/project/entry/src/main/ets/model/Call.ts",
            )
            .expect("TypeScript transform")
            .expect("TypeScript collection output");
        assert_eq!(
            typescript.crossplatform_collection.modules,
            ["arkui-x.telephony".to_string()]
        );
        assert_eq!(
            typescript.crossplatform_collection.components,
            ["ForEach".to_string(), "If".to_string()]
        );
        assert!(typescript.record_module_source_code);

        let javascript = transformer
            .compile_transform(
                "import router from '@ohos.router';\n\
                 import '@ohos.sideEffect';\n\
                 import colon from '@ohos:colon';\n\
                 void router; void colon;\n",
                "/tmp/project/entry/src/main/ets/model/Router.js",
            )
            .expect("JavaScript transform")
            .expect("JavaScript collection output");
        assert_eq!(
            javascript.crossplatform_collection.modules,
            ["ohos.router".to_string()]
        );
        assert!(javascript.crossplatform_collection.components.is_empty());
        assert!(!javascript.record_module_source_code);
    }

    #[test]
    fn unclassified_js_source_finalizes_dynamic_resources_with_plan_names() {
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/entry/src/main/ets"),
            Vec::new(),
            12,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        );
        let transformer = RolldownEtsTransformer::new(Arc::new(plan));
        let output = transformer
            .transform(
                r#"export const value = $r (resourceName);
export const raw = $rawfile("data/config.json");"#,
                "entry/src/main/ets/pages/resources.js",
            )
            .unwrap()
            .expect("unclassified JS resource calls must be finalized");

        assert!(output.code.contains("\"id\": -1"), "got: {}", output.code);
        assert!(output.code.contains("\"type\": -1"), "got: {}", output.code);
        assert!(output.code.contains("\"id\": 0"), "got: {}", output.code);
        assert!(
            output.code.contains("\"type\": 30000") || output.code.contains("\"type\": 3e4"),
            "got: {}",
            output.code
        );
        assert!(
            output.code.contains("\"bundleName\": \"com.example\""),
            "got: {}",
            output.code
        );
        assert!(
            output.code.contains("\"moduleName\": \"entry\""),
            "got: {}",
            output.code
        );
    }

    #[test]
    fn compile_transform_keeps_hvigor_module_source_as_rolldown_input() {
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/entry/src/main/ets"),
            Vec::new(),
            12,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        );
        let transformer = RolldownEtsTransformer::new(Arc::new(plan));
        let output = transformer
            .compile_transform(
                r#"import type { PageInfo } from "./PageInfo";
export default class ChannelManager {
  private defaultChannelName: string;
  constructor(defaultChannelName: string) {
    this.defaultChannelName = defaultChannelName;
  }
}"#,
                "/tmp/project/oh_modules/@meituan/lx/src/main/ets/channel/ChannelManager.ts",
            )
            .unwrap()
            .expect("TS source must produce transform output");

        assert!(
            output.code.contains("private defaultChannelName: string"),
            "Rollup-facing code must keep hvigor's CompileArkTS TypeScript shape:\n{}",
            output.code
        );
        assert!(
            output.code.contains("import type { PageInfo }"),
            "Rollup-facing code should preserve type-only imports for Rolldown TS parsing:\n{}",
            output.code
        );
        assert!(
            output.record_module_source_code,
            "CompileArkTS module source sidecar must be recorded"
        );
        assert!(
            output.code.contains("import type { PageInfo }"),
            "hvigor ModuleSourceFile sidecar must preserve type imports:\n{}",
            output.code
        );
        assert!(
            output.code.contains("private defaultChannelName: string"),
            "hvigor ModuleSourceFile sidecar must preserve TS field syntax:\n{}",
            output.code
        );
    }

    #[test]
    fn compile_transform_preserves_hvigor_default_import_export_shape() {
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/entry/src/main/ets"),
            Vec::new(),
            12,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        );
        let transformer = RolldownEtsTransformer::new(Arc::new(plan));
        let output = transformer
            .compile_transform(
                "import LX from './src/main/ets/LX';\nexport default LX;\n",
                "/tmp/project/oh_modules/@example/lx/Index.ets",
            )
            .unwrap()
            .expect("default import/export barrel must produce transform output");

        assert!(
            output
                .code
                .contains("import LX from \"./src/main/ets/LX\";"),
            "hvigor keeps the default import binding:\n{}",
            output.code
        );
        assert!(
            output.code.contains("export default LX;"),
            "hvigor keeps the local default export:\n{}",
            output.code
        );
        assert!(
            !output.code.contains("export { default }"),
            "the prepare pass must not invent a live re-export:\n{}",
            output.code
        );
    }

    #[test]
    fn compile_transform_preserves_lazy_import_for_compile_arkts_only() {
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/entry/src/main/ets"),
            Vec::new(),
            12,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        );
        let transformer = RolldownEtsTransformer::new(Arc::new(plan));
        let output = transformer
            .compile_transform(
                "import lazy { textToSpeech } from '@kit.CoreSpeechKit';\ntextToSpeech.createEngine({});\n",
                "/tmp/project/oh_modules/@scope/speech/FaceSpeech.ets",
            )
            .unwrap()
            .expect("lazy ArkTS source must produce transform output");

        assert!(
            output
                .code
                .contains("import textToSpeech from \"@hms.ai.textToSpeech\";"),
            "the current Rolldown scanner receives a standard import:\n{}",
            output.code
        );
        assert!(!output.code.contains("import lazy"), "{}", output.code);
        let module_source = output
            .module_source_code
            .expect("CompileArkTS must retain the hvigor lazy import");
        assert!(
            module_source.contains("import lazy textToSpeech from \"@hms.ai.textToSpeech\";"),
            "es2abc source must retain lazy module semantics:\n{module_source}"
        );
    }

    #[test]
    fn compile_transform_keeps_unrenamed_kit_named_import_without_alias() {
        // `process_kit_import.ts::ImportSpecifierKitInfo.transform` supplies
        // `undefined` as propertyName when the public kit name and system API
        // binding are equal. TypeScript therefore prints `{ BusinessError }`,
        // not the explicit-alias shape `{ BusinessError as BusinessError }`.
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/entry/src/main/ets"),
            Vec::new(),
            12,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        );
        let transformer = RolldownEtsTransformer::new(Arc::new(plan));
        let output = transformer
            .compile_transform(
                "import { BusinessError } from '@kit.BasicServicesKit';\nexport type Failure = BusinessError;\n",
                "/tmp/project/entry/src/ohosTest/ets/OpenHarmonyTestRunner.ets",
            )
            .unwrap()
            .expect("kit import transform");
        let module_source = output
            .module_source_code
            .expect("CompileArkTS must retain transformed TypeScript");

        assert!(
            module_source.contains("import type { BusinessError } from"),
            "{module_source}"
        );
        assert!(!module_source.contains("BusinessError as BusinessError"));
    }

    #[test]
    fn compile_transform_adapts_lazy_import_in_ts_dependency_for_rolldown() {
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/entry/src/main/ets"),
            Vec::new(),
            12,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        );
        let transformer = RolldownEtsTransformer::new(Arc::new(plan));
        let output = transformer
            .compile_transform(
                "import lazy nativePlayer from 'libaemonplayer.so';\nnativePlayer.create();\n",
                "/tmp/project/oh_modules/mtmediaplayer/src/main/ts/MTMediaPlayer.ts",
            )
            .unwrap()
            .expect("ArkTS lazy import in a .ts dependency must produce transform output");

        assert!(
            output
                .code
                .contains("import nativePlayer from \"libaemonplayer.so\";"),
            "Rolldown must receive a standard import from a .ts dependency:\n{}",
            output.code
        );
        assert!(!output.code.contains("import lazy"), "{}", output.code);
        assert_eq!(
            output.module_source_code.as_deref(),
            Some("import lazy nativePlayer from \"libaemonplayer.so\";\nnativePlayer.create();\n"),
            "CompileArkTS must retain lazy-import semantics through the source printer"
        );
    }

    #[test]
    fn compile_transform_folds_sdk_enum_through_kit_import_like_hvigor() {
        let sdk_root = PathBuf::from("/sdk/ets/api");
        let kit_root = PathBuf::from("/sdk/ets/kits");
        let mut files = FxHashMap::default();
        files.insert(
            sdk_root.join("@ohos.multimedia.media.d.ts"),
            Arc::<str>::from(
                r#"declare namespace media {
    declare interface OutputSize {}
    enum VideoScaleType {
        VIDEO_SCALE_TYPE_FIT = 0,
        VIDEO_SCALE_TYPE_FIT_CROP = 1
    }
}
export default media;"#,
            ),
        );
        files.insert(
            kit_root.join("@kit.MediaKit.d.ts"),
            Arc::<str>::from(
                r#"import media from "@ohos.multimedia.media";
export { media };"#,
            ),
        );
        let sources = TransformSourceSnapshot::new(files)
            .with_sdk_declaration_roots(vec![sdk_root, kit_root]);
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/entry/src/main/ets"),
            Vec::new(),
            12,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            sources,
        );
        let transformer = RolldownEtsTransformer::new(Arc::new(plan));
        let output = transformer
            .compile_transform(
                r#"import { media } from "@kit.MediaKit";
export enum VideoScaleMode {
    FIT = media.VideoScaleType.VIDEO_SCALE_TYPE_FIT,
    CROP = media.VideoScaleType.VIDEO_SCALE_TYPE_FIT_CROP
}"#,
                "/tmp/project/PlayerInterface.ets",
            )
            .unwrap()
            .expect("SDK enum source must produce transform output");

        assert!(
            output.code.contains("FIT = 0") && output.code.contains("CROP = 1"),
            "hvigor's TypeScript checker folds SDK enum members before CompileArkTS:\n{}",
            output.code
        );
        assert!(
            output
                .code
                .contains("import type media from \"@ohos.multimedia.media\";"),
            "SDK process_kit_import preserves the type-only fact produced by its prior const-enum transformer:\n{}",
            output.code
        );
    }

    #[test]
    fn compile_transform_inlines_local_const_enum_members_like_hvigor_module_source_file() {
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/entry/src/main/ets"),
            Vec::new(),
            12,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        );
        let transformer = RolldownEtsTransformer::new(Arc::new(plan));
        let output = transformer
            .compile_transform(
                r#"export const enum ConditionType {
    EqualTo = 'EqualTo',
    Limit = 'Limit',
}
export enum Sequence {
    First,
    Second,
    Third = Second + 1,
}
export const values = [ConditionType.EqualTo, ConditionType['Limit']];
export const runtimeValue = Sequence.Second;
function shadow(ConditionType: { EqualTo: string }) {
    return ConditionType.EqualTo;
}"#,
                "/tmp/project/oh_modules/@scope/cloud/DatabaseZoneQuery.ts",
            )
            .unwrap()
            .expect("local const enum source must produce CompileArkTS output");

        assert!(
            output.code.contains("export const enum ConditionType"),
            "latest TypeScript keeps the declaration in ModuleSourceFile:\n{}",
            output.code
        );
        assert!(
            output.code.contains("[\"EqualTo\", \"Limit\"]"),
            "latest TypeScript's getTypeExportImportAndConstEnumTransformer must inline local const enum members before es2abc:\n{}",
            output.code
        );
        assert!(
            output.code.contains("return ConditionType.EqualTo;"),
            "semantic binding identity must protect shadowed values:\n{}",
            output.code
        );
        assert!(
            output.code.contains("First = 0")
                && output.code.contains("Second = 1")
                && output.code.contains("Third = 2")
                && output.code.contains("runtimeValue = Sequence.Second"),
            "the source cache must normalize enum initializers but preserve regular enum runtime access:\n{}",
            output.code
        );
    }

    #[test]
    fn compile_transform_elides_imports_after_const_enum_substitution() {
        let project_root = PathBuf::from("/tmp/project");
        let source_path = project_root.join("oh_modules/@scope/cloud/SignInResponse.ts");
        let constants_path = project_root.join("oh_modules/@scope/cloud/Constants.ts");
        let mut files = FxHashMap::default();
        files.insert(
            constants_path,
            Arc::<str>::from(
                "export namespace Constant { export const enum Key { MapUidKey = 'uid' } }\n",
            ),
        );
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            project_root.clone(),
            project_root.join("entry/src/main/ets"),
            Vec::new(),
            12,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::new(files),
        );
        let transformer = RolldownEtsTransformer::new(Arc::new(plan));
        let output = transformer
            .compile_transform(
                r#"import { Constant } from "./Constants";
export const uid = Constant.Key.MapUidKey;"#,
                source_path.to_str().expect("test path is UTF-8"),
            )
            .unwrap()
            .expect("const enum source must produce CompileArkTS output");

        assert!(
            output
                .code
                .contains("import type { Constant } from \"./Constants\";"),
            "hvigor turns an import whose values were all inlined into a type import:\n{}",
            output.code
        );
        assert!(
            output.code.contains("uid = \"uid\""),
            "hvigor's enum pass substitutes the imported const enum member:\n{}",
            output.code
        );
        assert!(
            !output.code.contains("import { Constant }"),
            "the runtime import must not survive after const enum substitution:\n{}",
            output.code
        );
    }

    #[test]
    fn compile_transform_formats_plain_ets_with_sdk_indentation() {
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/entry/src/main/ets"),
            Vec::new(),
            12,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        );
        let transformer = RolldownEtsTransformer::new(Arc::new(plan));
        let output = transformer
            .compile_transform(
                "export function sharedGreet(name: string): string {\n  return `Hello from HSP, ${name}!`;\n}\n",
                "/tmp/project/shared/src/main/ets/Index.ets",
            )
            .unwrap()
            .expect("plain ETS must produce ModuleMode source");

        assert_eq!(
            output.code,
            "export function sharedGreet(name: string): string {\n    return `Hello from HSP, ${name}!`;\n}\n"
        );
        let source_map: serde_json::Value =
            serde_json::from_str(output.source_map.as_deref().expect("plain ETS source map"))
                .expect("valid source map");
        assert_eq!(
            source_map["mappings"],
            "AAAA,MAAM,UAAU,WAAW,CAAC,IAAI,EAAE,MAAM,GAAG,MAAM;IAC/C,OAAO,mBAAmB,IAAI,GAAG,CAAC;AACpC,CAAC"
        );
    }

    #[test]
    fn compile_transform_preserves_latest_typescript_printer_literal_spelling() {
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/entry/src/main/ets"),
            Vec::new(),
            12,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        );
        let transformer = RolldownEtsTransformer::new(Arc::new(plan));
        let source = "export const a: string = 'single';\nexport const b = \"double\";\nexport const c = 'a\\n\\'b';\n";
        let output = transformer
            .compile_transform(source, "/tmp/project/entry/src/main/ets/Index.ets")
            .unwrap()
            .expect("plain ETS must produce ModuleMode source");

        // Latest SDK process_module_files.genContentAndSourceMapInfo delegates
        // unchanged nodes to TypeScript createPrinter, which retains each
        // source literal's original quote and escape spelling.
        assert_eq!(output.code, source);
    }

    #[test]
    fn compile_transform_prints_fresh_module_declarations_after_module_rewrite() {
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/entry/src/main/ets"),
            Vec::new(),
            12,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        );
        let transformer = RolldownEtsTransformer::new(Arc::new(plan));
        let output = transformer
            .compile_transform(
                "import type { Local } from './Local';\nimport emitter from '@ohos.util.emitter';\nemitter.on('event', () => {});\nexport const value = 1;\n",
                "/tmp/project/entry/src/main/ets/Index.ets",
            )
            .unwrap()
            .expect("ETS source must produce ModuleMode source");

        // ModuleSourceFile rebuilds every import/export declaration around its
        // retained clause. TypeScript therefore prints fresh request literals
        // with double quotes, including declarations whose request text did
        // not change.
        assert!(
            output
                .code
                .contains("import type { Local } from \"./Local\";"),
            "{output:?}"
        );
        assert!(
            output
                .code
                .contains("import emitter from \"@ohos.events.emitter\";"),
            "{output:?}"
        );
    }

    #[test]
    fn expanded_import_and_arkui_lowering_keep_module_source_ast_for_single_print() {
        // Source chain:
        // rollup-plugin-ets-typescript.ts::transformNodes keeps the transformed
        // SourceFile through processUISyntax -> expandAllImportPaths ->
        // processKitImport; process_module_files.ts then prints the SourceFile
        // only after ModuleSourceFile::processTransformedTsModuleRequest.
        // The retained import clauses map to their original bindings, while
        // factory-created declaration shells and ArkUI runtime nodes stay
        // synthesized. This is the observed multi-module fixture mapping.
        let fixture_root = arkdown_path::lexical_normalize(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/multi-module-hap"),
        );
        let library_root = fixture_root.join("library");
        let page_path = fixture_root.join("entry/src/main/ets/pages/Index.ets");
        let mut source_map = FxHashMap::default();
        source_map.insert(
            library_root.join("Index.ets"),
            Arc::<str>::from(include_str!(
                "../../../tests/fixtures/multi-module-hap/library/Index.ets"
            )),
        );
        source_map.insert(
            library_root.join("src/main/ets/lib/greeting.ets"),
            Arc::<str>::from(include_str!(
                "../../../tests/fixtures/multi-module-hap/library/src/main/ets/lib/greeting.ets"
            )),
        );
        let source = include_str!(
            "../../../tests/fixtures/multi-module-hap/entry/src/main/ets/pages/Index.ets"
        );
        source_map.insert(page_path.clone(), Arc::<str>::from(source));
        let sources = TransformSourceSnapshot::new(source_map).with_package_roots(
            FxHashMap::from_iter([("library".to_string(), library_root.clone())]),
        );
        let plan = RolldownEtsTransformPlan::new(
            "com.example.multimodule",
            "entry",
            fixture_root.clone(),
            fixture_root.join("entry/src/main/ets"),
            vec![("library".to_string(), library_root)],
            17,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            sources,
        )
        .with_expand_import_path(Some(arkdown_core::ExpandImportPath {
            enable: true,
            exclude: Vec::new(),
        }))
        .with_checker_mode(CheckerMode::Off);
        let output = RolldownEtsTransformer::new(Arc::new(plan))
            .compile_transform(source, page_path.to_str().expect("UTF-8 fixture path"))
            .expect("multi-module transform")
            .expect("multi-module output");
        let source_map: serde_json::Value = serde_json::from_str(
            output
                .source_map
                .as_deref()
                .expect("multi-module TypeScript source map"),
        )
        .expect("source map JSON");

        assert!(
            output
                .code
                .contains("import { greet } from \"library/src/main/ets/lib/greeting\";"),
            "{}",
            output.code
        );
        assert!(
            output
                .code
                .contains("import { LIB_VERSION } from \"library/Index\";"),
            "{}",
            output.code
        );
        assert_eq!(
            source_map["mappings"],
            ";;;;IAKS,OAAO,GAAE,MAAM;;SALf,KAAK;SAAE,WAAW;MAIpB,KAAK;IAFZ;;;;;sDAG2B,aAAa;;;KALK;;;;;;;;;;;;;;;;IAK3C,4CAAgB,MAAM,EAAiB;QAAhC,OAAO;;;QAAP,OAAO,WAAE,MAAM;;;IAEtB;;YACE,GAAG;YAAH,GAAG,CAMD,MAAM,CAAC,MAAM;;;YALb,MAAM;YAAN,MAAM,CAIJ,KAAK,CAAC,MAAM;;;YAHZ,IAAI,QAAC,IAAI,CAAC,OAAO,GAAG,KAAK,CAAC,SAAS,CAAC,GAAG,WAAW;YAAlD,IAAI,CACD,QAAQ,CAAC,EAAE;YADd,IAAI,CAED,UAAU,CAAC,UAAU,CAAC,IAAI;;QAF7B,IAAI;QADN,MAAM;QADR,GAAG;KAOJ"
        );
    }

    #[test]
    fn compile_transform_prints_fresh_quote_when_type_import_split_rebuilds_declaration() {
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/entry/src/main/ets"),
            Vec::new(),
            12,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        );
        let transformer = RolldownEtsTransformer::new(Arc::new(plan));
        let output = transformer
            .compile_transform(
                "import { Local } from './Local';\nexport const value: Local = {} as Local;\n",
                "/tmp/project/entry/src/main/ets/Index.ets",
            )
            .unwrap()
            .expect("ETS source must produce ModuleMode source");

        // `TypeOnlyNamedImportSplitter` creates a replacement import AST node;
        // ModuleSourceFile then owns the declaration shell and TypeScript
        // prints the fresh source literal with double quotes.
        assert!(
            output
                .code
                .contains("import type { Local } from \"./Local\";"),
            "{output:?}"
        );
        assert_eq!(
            output.module_source_code.as_deref(),
            Some(output.code.as_str()),
            "CompileArkTS must retain etsTransform output before Rolldown's internal TypeScript scan"
        );
    }

    #[test]
    fn compile_transform_preserves_typescript_block_comment_text_after_codegen() {
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/entry/src/main/ets"),
            Vec::new(),
            12,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        );
        let transformer = RolldownEtsTransformer::new(Arc::new(plan));
        let source = "export class CommentedValue {\n  /**\n   * Retain the source spacing inside a block comment.\n   */\n  static readonly opacity = 0.5;\n}\n";
        let output = transformer
            .compile_transform(source, "/tmp/project/entry/src/main/ets/Index.ets")
            .unwrap()
            .expect("plain ETS must produce ModuleMode source");

        // Latest SDK TypeScript `createPrinter` leaves retained comments as
        // source text. OXC otherwise emits the same comment body without the
        // leading space after each newline.
        assert_eq!(
            output.code,
            "export class CommentedValue {\n    /**\n     * Retain the source spacing inside a block comment.\n     */\n    static readonly opacity = 0.5;\n}\n"
        );
    }

    #[test]
    fn compile_transform_preserves_typescript_source_literal_layout_after_codegen() {
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/entry/src/main/ets"),
            Vec::new(),
            12,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        );
        let transformer = RolldownEtsTransformer::new(Arc::new(plan));
        let output = transformer
            .compile_transform(
                "export const compact = { left: make('x'), right: make('y') };\nexport const multiline = {\n  colors: [[make('a'), 0],\n    [make('b'), 1]]\n};\nexport const trailing = {\n  span: { xs: 1 },\n};\n",
                "/tmp/project/entry/src/main/ets/Index.ets",
            )
            .unwrap()
            .expect("plain ETS must produce ModuleMode source");

        // The installed SDK's TypeScript printer keeps `ObjectLiteralExpression`
        // and `ArrayLiteralExpression` `multiLine` plus trailing-comma metadata
        // after etsTransform replaces child nodes. The same layout determines
        // es2abc's generated method-name hashes for transformed ViewPU files.
        assert_eq!(
            output.code,
            "export const compact = { left: make('x'), right: make('y') };\nexport const multiline = {\n    colors: [[make('a'), 0],\n        [make('b'), 1]]\n};\nexport const trailing = {\n    span: { xs: 1 },\n};\n"
        );
    }

    #[test]
    fn compile_transform_emits_javascript_for_ts_ignore_file_like_hvigor() {
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/entry/src/main/ets"),
            Vec::new(),
            12,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        );
        let transformer = RolldownEtsTransformer::new(Arc::new(plan));
        let output = transformer
            .compile_transform(
                r#"// @ts-ignore
export namespace Zip {
  export function gzip(data: ArrayBuffer | string): Uint8Array {
    return runtime.gzip(data)
  }
}"#,
                "/tmp/project/oh_modules/@scope/archive/src/main/ets/Zip.ts",
            )
            .unwrap()
            .expect("TypeScript suppression pragma must select hvigor's JS emitter path");

        assert!(
            output.code.contains("export let Zip") || output.code.contains("export var Zip"),
            "{}",
            output.code
        );
        assert!(
            !output.code.contains("data: ArrayBuffer"),
            "{}",
            output.code
        );
        assert!(!output.code.contains("namespace Zip"), "{}", output.code);
        assert!(output.emits_javascript);
        assert!(output.module_source_emits_javascript);
        assert_eq!(output.module_source_code.as_deref(), Some(&*output.code));
    }

    #[test]
    fn compile_transform_emits_ordinary_typescript_when_project_switch_is_enabled() {
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/entry/src/main/ets"),
            Vec::new(),
            12,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        )
        .with_should_emit_js(true);
        let transformer = RolldownEtsTransformer::new(Arc::new(plan));
        let output = transformer
            .compile_transform(
                "export const value: string = 'arkdown';\n",
                "/tmp/project/entry/src/main/ets/Value.ts",
            )
            .unwrap()
            .expect("project shouldEmitJs must select the JavaScript emitter");

        assert_eq!(
            output.code,
            "export const value = \"arkdown\";\r\n//# sourceMappingURL=Value.js.map"
        );
        assert!(output.emits_javascript);
        assert!(!output.module_source_emits_javascript);
        assert_eq!(
            output.module_source_code.as_deref(),
            Some("export const value: string = 'arkdown';\n")
        );
        assert!(output.source_map.is_some());
    }

    #[test]
    fn compile_transform_matches_latest_resource_table_javascript_contract() {
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/entry/src/main/ets"),
            Vec::new(),
            24,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        );
        let transformer = RolldownEtsTransformer::new(Arc::new(plan));
        let output = transformer
            .compile_transform(
                "/* generated resource table */\n//@ts-noCheck\n",
                "/tmp/project/entry/build/default/generated/r/default/ResourceTable.ts",
            )
            .unwrap()
            .expect("ResourceTable.ts participates in CompileArkTS");

        assert_eq!(
            output.code,
            concat!(
                "\"use strict\";\r\n",
                "//# sourceMappingURL=ResourceTable.js.map"
            )
        );
        let source_map: serde_json::Value = serde_json::from_str(
            output
                .source_map
                .as_deref()
                .expect("ResourceTable source map"),
        )
        .expect("valid source map");
        assert_eq!(source_map["mappings"], ";");
        assert!(output.emits_javascript);
    }

    #[test]
    fn compile_transform_matches_latest_module_info_javascript_contract() {
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/entry/src/main/ets"),
            Vec::new(),
            24,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        );
        let transformer = RolldownEtsTransformer::new(Arc::new(plan));
        let output = transformer
            .compile_transform(
                "\n//@ts-noCheck\nexport const __MODULE_NAME__: string = __getCurrentModuleName__()\nexport const __BUNDLE_NAME__: string = __getCurrentBundleName__()\n",
                "/tmp/project/entry/build/default/generated/profile/default/ModuleInfo.ts",
            )
            .unwrap()
            .expect("ModuleInfo.ts participates in CompileArkTS");

        assert_eq!(
            output.code,
            concat!(
                "//@ts-noCheck\r\n",
                "export const __MODULE_NAME__ = __getCurrentModuleName__();\r\n",
                "export const __BUNDLE_NAME__ = __getCurrentBundleName__();\r\n",
                "//# sourceMappingURL=ModuleInfo.js.map"
            )
        );
        let source_map: serde_json::Value = serde_json::from_str(
            output
                .source_map
                .as_deref()
                .expect("ModuleInfo TypeScript emit source map"),
        )
        .expect("valid ModuleInfo source map");
        assert_eq!(
            source_map["mappings"],
            "AACA,aAAa;AACb,MAAM,CAAC,MAAM,eAAe,GAAW,wBAAwB,EAAE,CAAA;AACjE,MAAM,CAAC,MAAM,eAAe,GAAW,wBAAwB,EAAE,CAAA"
        );
        assert!(output.emits_javascript);
    }

    #[test]
    fn compile_transform_propagates_sdk_shared_module_directive_fact() {
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/entry/src/main/ets"),
            Vec::new(),
            12,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        );
        let transformer = RolldownEtsTransformer::new(Arc::new(plan));
        let output = transformer
            .compile_transform(
                "import collections from '@ohos:arkts.collections';\n\"use shared\";\nexport class SharedState {}",
                "/tmp/project/oh_modules/shared-state/SharedState.ts",
            )
            .unwrap()
            .expect("TypeScript input participates in CompileArkTS");

        assert!(output.is_shared_module);
    }

    #[test]
    fn compile_transform_records_arkui_lowered_module_source() {
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/entry/src/main/ets"),
            Vec::new(),
            12,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        )
        .with_checker_mode(CheckerMode::Off);
        let transformer = RolldownEtsTransformer::new(Arc::new(plan));
        let output = transformer
            .compile_transform(
                r#"@Entry
@Component
struct Index {
  build() {
    Text("home")
  }
}"#,
                "/tmp/project/entry/src/main/ets/pages/Index.ets",
            )
            .unwrap()
            .expect("ArkUI source must produce transform output");
        assert!(
            output.record_module_source_code,
            "CompileArkTS module source sidecar must be recorded"
        );

        assert!(
            output.code.contains("class Index extends ViewPU"),
            "sidecar must use hvigor's lowered ArkUI source shape:\n{}",
            output.code
        );
        assert!(
            !output.code.contains("@Component"),
            "sidecar must not pass raw ArkUI decorators to es2abc:\n{}",
            output.code
        );
    }

    #[test]
    fn static_arkts_gives_rolldown_only_a_graph_projection() {
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/entry/src/main/ets"),
            Vec::new(),
            20,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        )
        .with_arkts_language_version(ArktsLanguageVersion::V1_2)
        .with_checker_mode(CheckerMode::Off);
        let transformer = RolldownEtsTransformer::new(Arc::new(plan));
        let source = r#"'use static';
import { value } from "./value"
@Entry @Component struct StaticPage {
  build() { Text(value); Text($r(resourceName)) }
}
"#;
        let output = transformer
            .compile_transform(
                source,
                "/tmp/project/entry/src/main/ets/pages/StaticPage.ets",
            )
            .unwrap()
            .expect("static ArkTS must produce a Rolldown graph projection");

        assert!(
            output.code.contains("import \"./value\";"),
            "{}",
            output.code
        );
        assert!(
            !output.code.contains("struct StaticPage"),
            "{}",
            output.code
        );
        let module_source = output
            .module_source_code
            .as_deref()
            .expect("the static compiler source must be retained separately");
        assert!(
            module_source.contains("struct StaticPage"),
            "{module_source}"
        );
        assert!(
            module_source.contains("$r(resourceName)"),
            "{module_source}"
        );
        assert!(!module_source.contains("\"id\": -1"), "{module_source}");
        assert!(!module_source.contains("ViewPU"), "{module_source}");
    }

    #[test]
    fn static_product_keeps_typescript_sources_in_the_dynamic_pipeline() {
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/entry/src/main/ets"),
            Vec::new(),
            20,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        )
        .with_arkts_language_version(ArktsLanguageVersion::V1_2)
        .with_checker_mode(CheckerMode::Off);
        let transformer = RolldownEtsTransformer::new(Arc::new(plan));
        let source =
            "import { value } from './value'\nexport function read(): number { return value }\n";
        let output = transformer
            .compile_transform(source, "/tmp/project/entry/src/main/ets/value.ts")
            .unwrap()
            .expect("TypeScript in an ArkTS 1.2 product stays dynamic");

        assert!(output.code.contains("import { value } from \"./value\";"));
        assert!(output.code.contains("export function read(): number"));
        assert_eq!(output.module_source_code.as_deref(), Some(&*output.code));
        assert!(!output.emits_javascript);
        assert!(output.record_module_source_code);
    }

    #[test]
    fn static_product_projects_dynamic_import_from_hybrid_dependency() {
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/entry/src/main/ets"),
            vec![
                ("entry".to_string(), PathBuf::from("/tmp/project/entry")),
                ("library".to_string(), PathBuf::from("/tmp/project/library")),
            ],
            20,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        )
        .with_arkts_language_version(ArktsLanguageVersion::V1_2)
        .with_checker_mode(CheckerMode::Off);
        let transformer = RolldownEtsTransformer::new(Arc::new(plan));
        // OpenHarmony ArkTSStaticSample/library uses this shape. The current
        // frontend classifies the dependency module as hybrid and this file as
        // 1.1 because its first effective line is not `use static`.
        let source = concat!(
            "\u{feff}/* OpenHarmony sample license */\n",
            "import('./sta_bridge/InitStatic');\n",
            "const factory = function () { return 1; };\n",
            "export { DataParser } from './dispatch/DataParser';\n",
        );
        let output = transformer
            .compile_transform(
                source,
                "/tmp/project/library/src/main/ets/components/Index.ets",
            )
            .unwrap()
            .expect("hybrid dependency source must produce a graph projection");

        assert!(
            output.code.contains("import('./sta_bridge/InitStatic')"),
            "{}",
            output.code
        );
        assert_eq!(output.module_source_code.as_deref(), Some(source));
    }

    #[test]
    fn javascript_lazy_import_is_lowered_only_for_rolldown() {
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/entry/src/main/ets"),
            Vec::new(),
            20,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        )
        .with_arkts_language_version(ArktsLanguageVersion::V1_2)
        .with_checker_mode(CheckerMode::Off);
        let transformer = RolldownEtsTransformer::new(Arc::new(plan));
        let source = concat!(
            "import lazy { DataParser as DataParserDyn } from '../dyn/DataParser';\n",
            "export { DataParserDyn };\n",
        );
        let output = transformer
            .compile_transform(source, "/tmp/project/library/dispatch/DataParser.js")
            .unwrap()
            .expect("JavaScript lazy import needs a standard ESM graph projection");

        assert!(!output.code.contains("import lazy"), "{}", output.code);
        assert!(
            output.code.contains("from \"../dyn/DataParser\""),
            "{}",
            output.code
        );
        assert!(output.module_source_code.is_none());
        assert!(!output.record_module_source_code);
    }

    #[test]
    fn rolldown_transform_registers_entry_decorated_dependency_sources() {
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/entry/src/main/ets"),
            Vec::new(),
            12,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        );
        let transformer = RolldownEtsTransformer::new(Arc::new(plan));
        let output = transformer
            .transform(
                r#"export const TOAST_PAGE_ENTRY_NAME = "ToastSubWindowPage";
@Entry({ routeName: TOAST_PAGE_ENTRY_NAME })
@Component
export struct ToastSubWindowPage {
  build() {}
}"#,
                "/tmp/project/oh_modules/@msi/runtime/src/main/ets/toast/pages/ToastSubWindowPage.ets",
            )
            .unwrap()
            .expect("package @Entry source must still lower as ArkUI");

        assert!(
            output.code.contains("export const TOAST_PAGE_ENTRY_NAME"),
            "constant export must remain importable:\n{}",
            output.code
        );
        assert!(
            output
                .code
                .contains("export class ToastSubWindowPage extends ViewPU"),
            "component must still lower to ViewPU:\n{}",
            output.code
        );
        assert!(
            output
                .code
                .contains("registerNamedRoute(() => new ToastSubWindowPage(undefined, {})"),
            "SDK processUISyntax/createEntryNode registers dependency @Entry sources:\n{}",
            output.code
        );
        assert!(
            output.code.contains(
                "pagePath: \"../../../../oh_modules/@msi/runtime/src/main/ets/toast/pages/ToastSubWindowPage\""
            ),
            "SDK process_module_package.pathMessage computes pagePath relative to projectPath:\n{}",
            output.code
        );
    }

    #[test]
    fn dependency_entry_named_index_does_not_collide_with_host_page() {
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/entry/src/main/ets"),
            Vec::new(),
            12,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        )
        .with_checker_mode(CheckerMode::Off);
        let transformer = RolldownEtsTransformer::new(Arc::new(plan));
        let source = r#"@Entry
@Component
struct Index {
  build() {}
}"#;
        let host = transformer
            .transform(source, "/tmp/project/entry/src/main/ets/pages/Index.ets")
            .unwrap()
            .expect("host @Entry source must be transformed");
        let dependency = transformer
            .transform(
                source,
                "/tmp/project/oh_modules/.ohpm/appmock@1.0.0/oh_modules/appmock/src/main/ets/pages/Index.ets",
            )
            .unwrap()
            .expect("dependency @Entry source must be transformed");

        assert!(
            host.code.contains("pagePath: \"pages/Index\""),
            "host pagePath must remain relative to projectPath:\n{}",
            host.code
        );
        assert!(
            dependency.code.contains(
                "pagePath: \"../../../../oh_modules/.ohpm/appmock@1.0.0/oh_modules/appmock/src/main/ets/pages/Index\""
            ),
            "dependency pagePath must retain package identity like SDK path.relative:\n{}",
            dependency.code
        );
        assert!(
            !dependency.code.contains("pagePath: \"pages/Index\""),
            "dependency @Entry must not overwrite the host pages/Index registration:\n{}",
            dependency.code
        );
    }

    #[test]
    fn rolldown_transform_preserves_form_entry_classification_after_path_normalization() {
        let normalized_id = "/tmp/project/entry/src/main/ets/forms/Card.ets".to_string();
        let mut form_entry_sources = FxHashSet::default();
        form_entry_sources.insert(normalized_id);
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/entry/src/main/ets"),
            Vec::new(),
            12,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        )
        .with_form_entry_source_ids(form_entry_sources)
        .with_checker_mode(CheckerMode::Off);
        let transformer = RolldownEtsTransformer::new(Arc::new(plan));
        let output = transformer
            .transform(
                r#"@Entry
@Component
struct Card {
  build() {
    Text("card")
  }
}"#,
                "/tmp/project/entry/src/main/ets/pages/../forms/Card.ets",
            )
            .unwrap()
            .expect("form entry source must produce transform output");

        assert!(
            output.code.contains("loadEtsCard(new Card(undefined, {})"),
            "form entry must use hvigor's card registration:\n{}",
            output.code
        );
        assert!(
            !output.code.contains("registerNamedRoute"),
            "form entry must not be registered as a page route:\n{}",
            output.code
        );
    }

    #[test]
    fn package_nav_destination_uses_host_module_name() {
        let plan = RolldownEtsTransformPlan::new(
            "com.sankuai.hmeituan",
            "meituan",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/meituan/src/main/ets"),
            vec![("meituan".to_string(), PathBuf::from("/tmp/project/meituan"))],
            12,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        )
        .with_checker_mode(CheckerMode::Off);
        let transformer = RolldownEtsTransformer::new(Arc::new(plan));
        let output = transformer
            .transform(
                r#"@Component
struct PrivacyPageComponent {
  build() {
    NavDestination() {
      Text("privacy")
    }
  }
}"#,
                "oh_modules/.ohpm/@meituan+privacypage@hash=/oh_modules/@meituan/privacypage/src/main/ets/pages/PrivacyPage.ets",
            )
            .unwrap()
            .expect("package ETS source must be transformed");

        assert!(
            output.code.contains("moduleName: \"meituan\""),
            "package NavDestination metadata must use host module name; got:\n{}",
            output.code
        );
        assert!(
            !output.code.contains("moduleName: \"privacypage\""),
            "package module self-name must not leak into NavDestination metadata; got:\n{}",
            output.code
        );
        assert!(
            output.code.contains(
                "pagePath: \"oh_modules/.ohpm/@meituan+privacypage@hash=/oh_modules/@meituan/privacypage/src/main/ets/pages/PrivacyPage\""
            ),
            "pagePath must continue to point at the package source; got:\n{}",
            output.code
        );
    }

    #[test]
    fn local_test_transform_removes_only_mock_setup_decorators() {
        let project_root = PathBuf::from("virtual").join("project");
        let module_root = project_root.join("entry");
        let source_root = module_root.join("src").join("main").join("ets");
        let mock_source = module_root.join("src").join("mock").join("ServiceMock.ets");
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            project_root,
            source_root,
            Vec::new(),
            12,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        )
        .with_checker_mode(CheckerMode::Off)
        .with_disable_mock_setup_decorator(true);
        let transformer = RolldownEtsTransformer::new(Arc::new(plan));
        let output = transformer
            .transform(
                r#"@MockSetup
@Keep
class Service {
  @MockSetup
  value: number = 1;

  @Keep
  @MockSetup
  load(@MockSetup input: number): number {
    return input + this.value;
  }
}"#,
                mock_source.to_string_lossy().as_ref(),
            )
            .unwrap()
            .expect("mock decorator lowering must produce output");

        assert!(!output.code.contains("MockSetup"), "{}", output.code);
        assert_eq!(output.code.matches("@Keep").count(), 2, "{}", output.code);
    }

    #[test]
    fn module_source_reprint_preserves_synthetic_arkui_source_map_ownership() {
        // ets2bundle ModuleSourceFile keeps the transformed ts.SourceFile when
        // getTypeExportImportAndConstEnumTransformer removes this type-only
        // import. The later printer must not give the synthetic class shell or
        // generated ArkUI statement terminators a source range.
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/project/entry/src/main/ets"),
            Vec::new(),
            12,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        )
        .with_checker_mode(CheckerMode::Off);
        let output = RolldownEtsTransformer::new(Arc::new(plan))
            .transform(
                "import unused from '@ohos.hilog';\n@Component\nstruct Page {\n  build() {\n    Row() {}.height('100%')\n  }\n}\n",
                "/tmp/project/entry/src/main/ets/pages/Page.ets",
            )
            .expect("module source transform")
            .expect("ArkUI source output");
        assert!(!output.code.contains("import unused"), "{}", output.code);

        let source_map = rolldown_sourcemap::OwnedSourceMap::from_json_string(
            output.source_map.as_deref().expect("ArkUI source map"),
        )
        .expect("valid ArkUI source map");
        let positions = source_map
            .get_tokens()
            .map(|token| (token.get_dst_line(), token.get_dst_col()))
            .collect::<FxHashSet<_>>();

        let class_start = output.code.find("class Page").expect("generated class") as u32;
        let class_keyword = crate::source_map::offset_to_line_col(&output.code, class_start);
        let class_name = crate::source_map::offset_to_line_col(&output.code, class_start + 6);
        assert!(!positions.contains(&class_keyword), "{}", output.code);
        assert!(positions.contains(&class_name), "{}", output.code);

        for statement in ["Row.create();", "Row.height('100%');"] {
            let start = output
                .code
                .find(statement)
                .expect("generated ArkUI statement") as u32;
            let semicolon = crate::source_map::offset_to_line_col(
                &output.code,
                start + statement.len() as u32 - 1,
            );
            let statement_end =
                crate::source_map::offset_to_line_col(&output.code, start + statement.len() as u32);
            assert!(
                !positions.contains(&semicolon),
                "{statement}:\n{}",
                output.code
            );
            assert!(
                !positions.contains(&statement_end),
                "{statement}:\n{}",
                output.code
            );
        }
    }
}
