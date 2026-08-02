use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arkdown_path::{AbsPath, ModuleSpecifierText};
use oxc_allocator::Allocator;
use oxc_codegen::{CodegenOptions, IndentChar};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::SourceType;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::api_transform;
use crate::api_transform::event_emitter::EventEmitterImportRewriter;
use crate::error::EtsTransformError;
use crate::lazy_import::LazyImportRolldownAdapter;
use crate::module_rewrite::{
    ModuleAnalyzerKind, TypeOnlyNamedImportSplitter, TypeOnlyReexportSplitter,
};
use crate::resource_transform::{ResourceReference, ResourceReferenceRewriter};
use crate::source::{
    CompileArktsSourceType, SourceClassifier, SourceFileKind, TransformSourceSnapshot,
};
use crate::source_map;
use crate::transform::source_emit::HvigorTypescriptPrinter;
use crate::transform::{self, CheckerMode, ModuleExportIndex, TransformOptions};
use crate::typescript_emit::{
    ConstEnumExportCache, ImportedConstEnumRewriter, TypeScriptDirectiveFlags,
    TypeScriptJavascriptEmitOutput, TypeScriptJavascriptEmitter,
};
use arkdown_core::{ModuleSource, ModuleType, ResolvedId};

/// Narrow transform plan consumed by the Rolldown ETS transform adapter.
///
/// The plan is built before Rolldown starts. Hook execution receives source
/// text from Rolldown and must not read project, SDK, profile, or resource
/// metadata.
#[derive(Debug, Clone)]
pub struct RolldownEtsTransformPlan {
    pub bundle_name: String,
    pub module_name: String,
    pub project_root: PathBuf,
    pub project_path: PathBuf,
    pub module_roots: Vec<(String, PathBuf)>,
    pub api_version: u32,
    pub resource_refs: Arc<FxHashMap<String, ResourceReference>>,
    pub component_metadata: Arc<transform::component_meta::ComponentMetadata>,
    pub sources: TransformSourceSnapshot,
    const_enum_exports: Arc<ConstEnumExportCache>,
    pub(crate) module_exports: ModuleExportIndex,
    form_entry_source_ids: Arc<FxHashSet<String>>,
    pub checker_mode: CheckerMode,
    pub should_emit_js: bool,
    pub module_analyzer: ModuleAnalyzerKind,
}

impl RolldownEtsTransformPlan {
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
        let module_exports = ModuleExportIndex::build(&sources, &module_roots, &component_metadata);
        let const_enum_exports = Arc::new(ConstEnumExportCache::new(sources.clone()));
        Self {
            bundle_name: bundle_name.into(),
            module_name: module_name.into(),
            project_root,
            project_path,
            module_roots,
            api_version,
            resource_refs: Arc::new(resource_refs),
            component_metadata: Arc::new(component_metadata),
            sources,
            const_enum_exports,
            module_exports,
            form_entry_source_ids: Arc::default(),
            checker_mode: CheckerMode::Oxc,
            should_emit_js: true,
            module_analyzer: ModuleAnalyzerKind::default(),
        }
    }

    pub fn with_checker_mode(mut self, checker_mode: CheckerMode) -> Self {
        self.checker_mode = checker_mode;
        self
    }

    pub fn with_should_emit_js(mut self, should_emit_js: bool) -> Self {
        self.should_emit_js = should_emit_js;
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

    pub fn transform_options_for(&self, id: &str) -> TransformOptions {
        TransformOptions {
            bundle_name: Some(self.bundle_name.clone()),
            module_name: Some(self.module_name.clone()),
            project_root: Some(self.project_root.clone()),
            project_path: Some(self.project_path.clone()),
            module_roots: self.module_roots.clone(),
            component_metadata: Arc::clone(&self.component_metadata),
            sources: self.sources.clone(),
            module_exports: self.module_exports.clone(),
            form_entry: self.is_form_entry(id),
            partial_update: true,
            checker_mode: self.checker_mode,
        }
    }

    fn is_form_entry(&self, id: &str) -> bool {
        if self.form_entry_source_ids.contains(id) {
            return true;
        }
        let normalized = TransformEntrySourceId::new(id).normalized();
        self.form_entry_source_ids.contains(normalized.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct RolldownEtsTransformOutput {
    pub code: String,
    /// Hvigor-compatible CompileArkTS source when Rolldown needs a parser-only
    /// adaptation such as lowering `import lazy` to a standard import.
    pub module_source_code: Option<String>,
    pub source_map: Option<String>,
    pub record_module_source_code: bool,
    pub emits_javascript: bool,
    pub is_shared_module: bool,
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
            let pipeline = HvigorSourcePipeline::new(self, id);
            if pipeline.should_finalize_unclassified_source(code) {
                let final_code = pipeline.finalize_source(code);
                if final_code != code {
                    return Ok(Some(RolldownEtsTransformOutput {
                        code: final_code.into_owned(),
                        module_source_code: None,
                        source_map: None,
                        record_module_source_code: false,
                        emits_javascript: false,
                        is_shared_module: false,
                    }));
                }
            }
            return Ok(None);
        }
        let is_ets = matches!(source_kind, SourceFileKind::Ets);

        let original_source = code;
        let directive_flags = TypeScriptDirectiveFlags::from_source(original_source, id);
        let emits_javascript = self.plan.should_emit_js && directive_flags.should_emit_javascript();
        let is_shared_module = directive_flags.is_shared_module();
        let pipeline = HvigorSourcePipeline::new(self, id);
        let code = pipeline.prepare_source(original_source);

        if !is_ets {
            let final_code = pipeline.finalize_source(code.as_ref());
            let emitted =
                self.emit_javascript_if_required(final_code.as_ref(), id, directive_flags)?;
            let source_map = if emits_javascript && directive_flags.is_comment_only() {
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
                module_source_code: None,
                source_map,
                record_module_source_code: false,
                emits_javascript,
                is_shared_module,
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

        let final_source = pipeline.finalize_source(&transformed.code);

        if transformed.code == code.as_ref() {
            // The SDK etsTransform printer still formats a plain `.ets` file
            // even when no ArkUI syntax was lowered. Its TypeScript output uses
            // four-space indentation before ModuleMode writes the CompileArkTS
            // cache source consumed by es2abc.
            let formatted_source = pipeline.format_source(final_source.as_ref());
            let emitted =
                self.emit_javascript_if_required(formatted_source.as_str(), id, directive_flags)?;
            let source_map = emitted.source_map.or_else(|| {
                source_map::HvigorStandardSourceMapBuilder::new(&emitted.code, original_source, id)
                    .build()
                    .or_else(|| source_map::TokenSourceMapBuilder::new(&emitted.code, id).build())
            });
            return Ok(Some(RolldownEtsTransformOutput {
                code: emitted.code,
                module_source_code: None,
                source_map,
                record_module_source_code: false,
                emits_javascript,
                is_shared_module,
            }));
        }

        let emitted =
            self.emit_javascript_if_required(final_source.as_ref(), id, directive_flags)?;
        let source_map = match (transformed.source_map, emitted.source_map) {
            (Some(transform_map), Some(emit_map)) => {
                Some(self.collapse_source_maps(id, &transform_map, &emit_map)?)
            }
            (Some(transform_map), None) => Some(transform_map),
            (None, Some(emit_map)) => Some(emit_map),
            (None, None) => {
                source_map::HvigorStandardSourceMapBuilder::new(&emitted.code, original_source, id)
                    .build()
                    .or_else(|| source_map::TokenSourceMapBuilder::new(&emitted.code, id).build())
            }
        };
        Ok(Some(RolldownEtsTransformOutput {
            code: emitted.code,
            module_source_code: None,
            source_map,
            record_module_source_code: false,
            emits_javascript,
            is_shared_module,
        }))
    }

    fn emit_javascript_if_required(
        &self,
        source: &str,
        id: &str,
        directive_flags: TypeScriptDirectiveFlags,
    ) -> Result<TypeScriptJavascriptEmitOutput, EtsTransformError> {
        if !self.plan.should_emit_js || !directive_flags.should_emit_javascript() {
            return Ok(TypeScriptJavascriptEmitOutput {
                code: source.to_string(),
                source_map: None,
            });
        }
        TypeScriptJavascriptEmitter::new(source, id, &self.plan.sources)
            .with_const_enum_export_cache(&self.plan.const_enum_exports)
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
            let rolldown_source = LazyImportRolldownAdapter::new(&output.code, id).lower();
            if let Cow::Owned(rolldown_source) = rolldown_source {
                output.module_source_code =
                    Some(std::mem::replace(&mut output.code, rolldown_source));
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

    fn finalize_source<'s>(&self, source: &'s str) -> Cow<'s, str> {
        HvigorFinalizeSourcePass::new(self).run(source)
    }

    fn format_source(&self, source: &str) -> String {
        let allocator = Allocator::default();
        let source_type = CompileArktsSourceType::from_id(self.id);
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
        CompileArktsSourceType::from_id(self.pipeline.id)
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

impl<'a, 'pipeline> HvigorFinalizeSourcePass<'a, 'pipeline> {
    fn new(pipeline: &'pipeline HvigorSourcePipeline<'a>) -> Self {
        Self { pipeline }
    }

    fn run<'s>(&self, source: &'s str) -> Cow<'s, str> {
        let allocator = Allocator::default();
        let source_type = self.source_type();
        let mut parsed = Parser::new(&allocator, source, source_type).parse();
        if !parsed.diagnostics.is_empty() {
            return Cow::Borrowed(source);
        }

        let semantic_ret = SemanticBuilder::new().build(&parsed.program);
        if !semantic_ret.diagnostics.is_empty() {
            return Cow::Borrowed(source);
        }
        let scoping = semantic_ret.semantic.into_scoping();

        let mut changed = false;

        // Hvigor runs SDK `etsTransform` before `apiTransform`. Kit import
        // lowering is part of SDK etsTransform after UI syntax lowering, while
        // legacy system API migration runs in the later apiTransform plugin.
        changed |= api_transform::KitImportRewriter::new(source).rewrite_program(
            &allocator,
            &mut parsed.program,
            &scoping,
        );

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

        changed |= ImportedConstEnumRewriter::new(
            self.pipeline.id,
            &self.pipeline.transformer.plan.sources,
        )
        .with_const_enum_export_cache(&self.pipeline.transformer.plan.const_enum_exports)
        .rewrite_program(&allocator, &mut parsed.program);

        changed |= TypeOnlyNamedImportSplitter::new(
            self.pipeline.id,
            &self.pipeline.transformer.plan.sources,
        )
        .with_module_analyzer(self.pipeline.transformer.plan.module_analyzer)
        .split_program(&allocator, &mut parsed.program, &scoping);

        changed |= ResourceReferenceRewriter::new(
            source,
            self.pipeline.id,
            self.pipeline.transformer.plan.resource_refs.as_ref(),
            &self.pipeline.transformer.plan.bundle_name,
            Some(&self.pipeline.transformer.plan.module_name),
        )
        .rewrite_program(&allocator, &mut parsed.program);

        if !changed {
            return Cow::Borrowed(source);
        }
        Cow::Owned(hvigor_stage_codegen(&parsed.program))
    }

    fn source_type(&self) -> SourceType {
        CompileArktsSourceType::from_id(self.pipeline.id)
    }
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
        let output = transformer
            .transform(
                r#"import emitter from "@system.emitter";
export { emitter };"#,
                "entry/src/main/ets/pages/emitter.js",
            )
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

        assert!(output.code.contains("id: -1"), "got: {}", output.code);
        assert!(output.code.contains("type: -1"), "got: {}", output.code);
        assert!(output.code.contains("id: 0"), "got: {}", output.code);
        assert!(
            output.code.contains("type: 30000") || output.code.contains("type: 3e4"),
            "got: {}",
            output.code
        );
        assert!(
            output.code.contains("bundleName: \"com.example\""),
            "got: {}",
            output.code
        );
        assert!(
            output.code.contains("moduleName: \"entry\""),
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
            output.code.contains("import LX from './src/main/ets/LX';"),
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
            Some("import lazy nativePlayer from 'libaemonplayer.so';\nnativePlayer.create();\n"),
            "CompileArkTS must retain the original lazy-import semantics"
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
                .contains("import media from \"@ohos.multimedia.media\";"),
            "Kit import lowering remains part of the same hvigor stage:\n{}",
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
                "/* generated resource table */\r\n",
                "//@ts-noCheck\r\n",
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
}
