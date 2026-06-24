use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arkdown_path::ModuleSpecifierText;
use oxc_allocator::Allocator;
use oxc_codegen::{Codegen, CodegenOptions, IndentChar};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::SourceType;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::api_transform;
use crate::api_transform::event_emitter::EventEmitterImportRewriter;
use crate::error::EtsTransformError;
use crate::hvigor_format::HvigorStandardTs;
use crate::module_rewrite::{
    DefaultImportReexportRewriter, TypeOnlyNamedImportSplitter, TypeOnlyReexportSplitter,
};
use crate::resource_transform::{ResourceReference, ResourceReferenceRewriter};
use crate::source::{SourceClassifier, SourceFileKind, TransformSourceSnapshot};
use crate::source_map;
use crate::transform::source_emit::CodegenSourceText;
use crate::transform::{self, CheckerMode, ModuleExportIndex, TransformOptions};
use crate::typescript_emit::ImportedConstEnumRewriter;
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
    pub module_roots: Vec<(String, PathBuf)>,
    pub api_version: u32,
    pub resource_refs: Arc<FxHashMap<String, ResourceReference>>,
    pub component_metadata: Arc<transform::component_meta::ComponentMetadata>,
    pub sources: TransformSourceSnapshot,
    pub(crate) module_exports: ModuleExportIndex,
    entry_source_ids: Option<Arc<FxHashSet<String>>>,
    pub checker_mode: CheckerMode,
    pub should_emit_js: bool,
}

impl RolldownEtsTransformPlan {
    pub fn new(
        bundle_name: impl Into<String>,
        module_name: impl Into<String>,
        project_root: PathBuf,
        module_roots: Vec<(String, PathBuf)>,
        api_version: u32,
        resource_refs: FxHashMap<String, ResourceReference>,
        component_metadata: transform::component_meta::ComponentMetadata,
        sources: TransformSourceSnapshot,
    ) -> Self {
        let module_exports = ModuleExportIndex::build(&sources, &module_roots);
        Self {
            bundle_name: bundle_name.into(),
            module_name: module_name.into(),
            project_root,
            module_roots,
            api_version,
            resource_refs: Arc::new(resource_refs),
            component_metadata: Arc::new(component_metadata),
            sources,
            module_exports,
            entry_source_ids: None,
            checker_mode: CheckerMode::Oxc,
            should_emit_js: true,
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

    pub fn with_entry_source_ids(mut self, entry_source_ids: FxHashSet<String>) -> Self {
        self.entry_source_ids = Some(Arc::new(entry_source_ids));
        self
    }

    fn should_emit_entry_registration(&self, id: &str) -> bool {
        let Some(entry_source_ids) = &self.entry_source_ids else {
            return true;
        };

        if entry_source_ids.contains(id) {
            return true;
        }

        let normalized = TransformEntrySourceId::new(id).normalized();
        entry_source_ids.contains(normalized.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct RolldownEtsTransformOutput {
    pub code: String,
    pub source_map: Option<String>,
    pub record_module_source_code: bool,
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
                        source_map: None,
                        record_module_source_code: false,
                    }));
                }
            }
            return Ok(None);
        }
        let is_ets = matches!(source_kind, SourceFileKind::Ets);

        let original_source = code;
        let pipeline = HvigorSourcePipeline::new(self, id);
        let code = pipeline.prepare_source(original_source);

        if !is_ets {
            let final_code = pipeline.finalize_source(code.as_ref());
            let source_map = source_map::HvigorStandardSourceMapBuilder::new(
                final_code.as_ref(),
                original_source,
                id,
            )
            .build()
            .or_else(|| source_map::TokenSourceMapBuilder::new(final_code.as_ref(), id).build());
            return Ok(Some(RolldownEtsTransformOutput {
                code: final_code.into_owned(),
                source_map,
                record_module_source_code: false,
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

        let options = TransformOptions {
            bundle_name: Some(self.plan.bundle_name.clone()),
            module_name: Some(self.plan.module_name.clone()),
            project_root: Some(self.plan.project_root.clone()),
            module_roots: self.plan.module_roots.clone(),
            component_metadata: Arc::clone(&self.plan.component_metadata),
            sources: self.plan.sources.clone(),
            module_exports: self.plan.module_exports.clone(),
            emit_entry_registration: self.plan.should_emit_entry_registration(id),
            partial_update: true,
            checker_mode: self.plan.checker_mode,
        };

        let transformed =
            transform::ArktsTransformer::with_options(&source, &options).transform()?;

        let final_source = pipeline.finalize_source(&transformed);

        if transformed == code.as_ref() {
            let final_source = HvigorStandardTs::new(final_source.as_ref()).format();
            let source_map =
                source_map::HvigorStandardSourceMapBuilder::new(&final_source, original_source, id)
                    .build()
                    .or_else(|| source_map::TokenSourceMapBuilder::new(&final_source, id).build());
            return Ok(Some(RolldownEtsTransformOutput {
                code: final_source,
                source_map,
                record_module_source_code: false,
            }));
        }

        let gen_code = final_source.into_owned();
        let source_map =
            source_map::HvigorStandardSourceMapBuilder::new(&gen_code, original_source, id)
                .build()
                .or_else(|| source_map::TokenSourceMapBuilder::new(&gen_code, id).build());
        Ok(Some(RolldownEtsTransformOutput {
            code: gen_code,
            source_map,
            record_module_source_code: false,
        }))
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
        // hvigor ESMODULE/CompileArkTS writes transformed TypeScript cache files
        // and lets es2abc compile the filesInfo/modules.cache handoff. Rolldown
        // only needs this TS shape for graph parsing.
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
        if !path.is_absolute() {
            return self.id.to_string();
        }
        path.canonicalize()
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
            source,
            self.pipeline.id,
            &self.pipeline.transformer.plan.sources,
        );
        let default_reexport = DefaultImportReexportRewriter::new(source, self.pipeline.id);
        if !reexports.may_split() && !default_reexport.may_rewrite() {
            return Cow::Borrowed(source);
        }

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
        changed |= reexports.split_program(&allocator, &mut parsed.program);
        changed |= default_reexport.rewrite_program(&allocator, &mut parsed.program, &scoping);

        self.finish(source, &parsed.program, changed)
    }

    fn source_type(&self) -> SourceType {
        SourceType::from_path(self.pipeline.id)
            .unwrap_or_else(|_| SourceType::ts().with_module(true))
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
        if !self.may_rewrite(source) {
            return Cow::Borrowed(source);
        }

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
            source,
            self.pipeline.id,
            &self.pipeline.transformer.plan.sources,
        )
        .rewrite_program(&allocator, &mut parsed.program);

        changed |= TypeOnlyNamedImportSplitter::new(
            source,
            self.pipeline.id,
            &self.pipeline.transformer.plan.sources,
        )
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

    fn may_rewrite(&self, source: &str) -> bool {
        api_transform::KitImportRewriter::new(source).may_rewrite()
            || source.contains("@ohos:")
            || source.contains("@ohos.util.emitter")
            || source.contains("@system.emitter")
            || (self.pipeline.transformer.plan.api_version < 9 && source.contains("@ohos/"))
            || ImportedConstEnumRewriter::new(
                source,
                self.pipeline.id,
                &self.pipeline.transformer.plan.sources,
            )
            .may_rewrite()
            || TypeOnlyNamedImportSplitter::new(
                source,
                self.pipeline.id,
                &self.pipeline.transformer.plan.sources,
            )
            .may_split()
            || ResourceReferenceRewriter::new(
                source,
                self.pipeline.id,
                self.pipeline.transformer.plan.resource_refs.as_ref(),
                &self.pipeline.transformer.plan.bundle_name,
                Some(&self.pipeline.transformer.plan.module_name),
            )
            .may_rewrite()
    }

    fn source_type(&self) -> SourceType {
        SourceType::from_path(self.pipeline.id)
            .unwrap_or_else(|_| SourceType::ts().with_module(true))
    }
}

fn hvigor_stage_codegen(program: &oxc_ast::ast::Program<'_>) -> String {
    CodegenSourceText::new(
        Codegen::new()
            .with_options(CodegenOptions {
                indent_char: IndentChar::Space,
                indent_width: 4,
                ..CodegenOptions::default()
            })
            .build(program)
            .code,
    )
    .without_trailing_newline()
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
                "/tmp/project/oh_modules/@example/lx/src/main/ets/channel/ChannelManager.ts",
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
    fn compile_transform_records_arkui_lowered_module_source() {
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
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
    fn rolldown_transform_skips_entry_registration_for_non_entry_imports() {
        let mut entry_sources = FxHashSet::default();
        entry_sources.insert("/tmp/project/entry/src/main/ets/pages/Index.ets".to_string());
        let plan = RolldownEtsTransformPlan::new(
            "com.example",
            "entry",
            PathBuf::from("/tmp/project"),
            Vec::new(),
            12,
            FxHashMap::default(),
            transform::component_meta::ComponentMetadata::default(),
            TransformSourceSnapshot::default(),
        )
        .with_entry_source_ids(entry_sources);
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
            !output.code.contains("registerNamedRoute"),
            "non-entry import context must not emit named-route registration:\n{}",
            output.code
        );
    }

    #[test]
    fn package_nav_destination_uses_host_module_name() {
        let plan = RolldownEtsTransformPlan::new(
            "com.example.hexample",
            "example",
            PathBuf::from("/tmp/project"),
            vec![("example".to_string(), PathBuf::from("/tmp/project/example"))],
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
                "oh_modules/.ohpm/@example+privacypage@hash=/oh_modules/@example/privacypage/src/main/ets/pages/PrivacyPage.ets",
            )
            .unwrap()
            .expect("package ETS source must be transformed");

        assert!(
            output.code.contains("moduleName: \"example\""),
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
                "pagePath: \"oh_modules/.ohpm/@example+privacypage@hash=/oh_modules/@example/privacypage/src/main/ets/pages/PrivacyPage\""
            ),
            "pagePath must continue to point at the package source; got:\n{}",
            output.code
        );
    }
}
