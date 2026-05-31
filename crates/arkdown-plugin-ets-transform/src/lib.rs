//! Pure Rust port of the OpenHarmony SDK `ets-loader` ArkTS transform plugin.
//!
//! This crate replaces the `ets-loader` subprocess invocation described in SPEC-004
//! with a fully in-process Rust implementation. It implements three components as
//! a single [`rolldown_plugin::Plugin`]:
//!
//! | Component      | SDK equivalent        | What it does                                      |
//! | -------------- | --------------------- | ------------------------------------------------- |
//! | `etsChecker`   | `etsChecker` plugin   | Validates decorator usage rules (diagnostics)     |
//! | `etsTransform` | `etsTransform` plugin | Desugars ArkUI decorators and struct components   |
//! | `apiTransform` | `apiTransform` plugin | Rewrites OHOS API imports by target SDK version   |

pub mod api_transform;
pub mod checker;
pub mod error;
pub mod kit_map;
pub mod source_map;
pub mod transform;

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use arkdown_core::{ArkdownConfig, ModuleSource, ModuleType, ResolvedId};
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    BindingPattern, Declaration, ExportNamedDeclaration, ExportSpecifier, ImportDeclaration,
    ImportDeclarationSpecifier, ModuleExportName, Statement, StringLiteral,
};
#[cfg(test)]
use oxc_codegen::{Codegen, CodegenOptions};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::SourceType;
use rolldown_plugin::{
    HookLoadArgs, HookLoadOutput, HookLoadReturn, HookTransformArgs, HookTransformOutput,
    HookTransformOutputMap, HookTransformReturn, HookUsage, Plugin, SharedLoadPluginContext,
    SharedTransformPluginContext,
};
use rolldown_sourcemap::SourceMap as RolldownSourceMap;

/// ArkTS transform plugin — desugars `.ets` files into TypeScript.
///
/// Register this plugin in the plugin list before any TypeScript-only plugins
/// so that `.ets` modules are converted to `.ts` before downstream processing begins.
pub struct EtsTransformPlugin {
    pub config: Arc<ArkdownConfig>,
}

impl std::fmt::Debug for EtsTransformPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EtsTransformPlugin").finish_non_exhaustive()
    }
}

impl EtsTransformPlugin {
    pub fn new(config: Arc<ArkdownConfig>) -> Self {
        EtsTransformPlugin { config }
    }

    pub fn do_transform(
        &self,
        code: &str,
        id: &str,
    ) -> anyhow::Result<Option<HookTransformOutput>> {
        let is_ets = id.ends_with(".ets");
        let is_ts = !is_ets && id.ends_with(".ts");

        if !is_ets && !is_ts {
            return Ok(None);
        }

        let original_source = code;
        let code =
            rewrite_type_only_reexports(&api_transform::rewrite_kit_imports(original_source), id);
        let api_version = self.config.sdk_config.compile_api_version;

        if !is_ets {
            // .ts files: kit rewrite + version-gated rewrites only.
            let final_code = api_transform::rewrite_imports_for_version(&code, api_version);
            let final_code = split_type_only_named_imports(&final_code, id);
            let source_map_json =
                source_map::build_hvigor_standard_source_map(&final_code, original_source, id)
                    .or_else(|| source_map::build_token_source_map(&final_code, id));
            let map = source_map_json
                .as_deref()
                .and_then(|j| RolldownSourceMap::from_json_string(j).ok())
                .map(|sm| HookTransformOutputMap::Sourcemap(Box::new(sm)))
                .unwrap_or(HookTransformOutputMap::Null);
            return Ok(Some(HookTransformOutput {
                code: Some(final_code),
                map,
                ..Default::default()
            }));
        }

        let source = ModuleSource {
            id: ResolvedId {
                id: id.to_string(),
                external: false,
                module_type: ModuleType::Ets,
                ..Default::default()
            },
            content: code.clone(),
            source_map: None,
        };

        let options = transform::TransformOptions {
            bundle_name: Some(self.config.product.bundle_name.clone()),
            module_name: module_name_from_id(id),
        };

        let transformed = transform::transform_ets_with_options(&source, &options)
            .map_err(|e| anyhow::anyhow!("ets-transform: {e}"))?;

        let final_source = api_transform::rewrite_imports_for_version(&transformed, api_version);
        let final_source = split_type_only_named_imports(&final_source, id);

        // For .ets files the struct transform may rewrite the entire file
        // (e.g. Index.ets → ViewPU TypeScript).  Split into two cases:
        //
        // Case A — no struct transform (EntryAbility-style files with no
        // @Component / @Entry):  `transformed` is identical to `code`.  Use
        // the same per-token approach as .ts files: return the oxc-regenerated
        // code so the source map coordinates match the code handed to rolldown.
        //
        // Case B — struct-transformed files (Index.ets etc.): the generated
        // ViewPU TypeScript has no structural relationship to the kit-rewritten
        // ETS source, so a position-accurate cross-file source map is not
        // feasible here.  Fall back to a source map generated from the
        // TRANSFORMED TypeScript output so the generated-side positions at
        // least match the code rolldown receives, giving rolldown a valid chain
        // even if the "original" side of the map references ViewPU positions
        // rather than ETS positions.  This is still more useful than the
        // previous kit-rewritten-ETS map which had completely mismatched
        // coordinate spaces and prevented any chaining.
        if transformed == code {
            // Case A: no struct transform. hvigor preserves the TypeScript
            // source shape after kit import rewriting instead of re-emitting it
            // through a general TS code generator, so keep literals, type
            // annotations, and indentation intact here.
            let final_source = format_hvigor_standard_ts(&final_source);
            let source_map_json =
                source_map::build_hvigor_standard_source_map(&final_source, original_source, id)
                    .or_else(|| source_map::build_token_source_map(&final_source, id));
            let map = source_map_json
                .as_deref()
                .and_then(|j| RolldownSourceMap::from_json_string(j).ok())
                .map(|sm| HookTransformOutputMap::Sourcemap(Box::new(sm)))
                .unwrap_or(HookTransformOutputMap::Null);
            Ok(Some(HookTransformOutput {
                code: Some(final_source),
                map,
                ..Default::default()
            }))
        } else {
            // Case B: struct transform applied.  Re-run the transform via the
            // span-aware variant to obtain a source map that maps ViewPU output
            // lines back to the original `.ets` source positions.  The oxc
            // parser operates on `source.content` (kit-rewritten), so span byte
            // offsets reference positions in that string — pass it as `ets_source`
            // so `offset_to_line_col` converts them correctly.
            let (gen_code, source_map) =
                transform::transform_ets_viewpu_with_source_map(&source, &options, &source.content)
                    .map(|(code, map)| {
                        let final_code =
                            api_transform::rewrite_imports_for_version(&code, api_version);
                        let final_code = split_type_only_named_imports(&final_code, id);
                        (final_code, map)
                    })
                    .unwrap_or_else(|_| (final_source, None));
            let map = source_map
                .as_deref()
                .and_then(|j| RolldownSourceMap::from_json_string(j).ok())
                .map(|sm| HookTransformOutputMap::Sourcemap(Box::new(sm)))
                .unwrap_or(HookTransformOutputMap::Null);
            Ok(Some(HookTransformOutput {
                code: Some(gen_code),
                map,
                ..Default::default()
            }))
        }
    }
}

impl Plugin for EtsTransformPlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("ets-transform")
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::Load | HookUsage::Transform
    }

    fn load(
        &self,
        _ctx: SharedLoadPluginContext,
        args: &HookLoadArgs<'_>,
    ) -> impl std::future::Future<Output = HookLoadReturn> + Send {
        let id = args.id.to_owned();
        let config = Arc::clone(&self.config);
        async move {
            if !id.ends_with(".ets") {
                return Ok(None);
            }
            let code = match std::fs::read_to_string(&id) {
                Ok(code) => code,
                Err(_) => return Ok(None),
            };
            if !transform::has_arkts_transform_syntax(&code) {
                return Ok(None);
            }
            let output = EtsTransformPlugin::new(config).do_transform(&code, &id)?;
            Ok(output.map(|output| HookLoadOutput {
                code: output.code.unwrap_or(code).into(),
                ..Default::default()
            }))
        }
    }

    fn transform(
        &self,
        _ctx: SharedTransformPluginContext,
        args: &HookTransformArgs<'_>,
    ) -> impl std::future::Future<Output = HookTransformReturn> + Send {
        let code = args.code.as_str().to_owned();
        let id = args.id.to_owned();
        let config = Arc::clone(&self.config);
        async move { EtsTransformPlugin::new(config).do_transform(&code, &id) }
    }
}

#[derive(Debug)]
pub(crate) struct SourcePatch {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) replacement: String,
}

#[derive(Debug)]
struct ImportSpecText {
    text: String,
    is_type_only: bool,
}

fn split_type_only_named_imports(source: &str, id: &str) -> String {
    let allocator = Allocator::default();
    let source_type =
        SourceType::from_path(id).unwrap_or_else(|_| SourceType::ts().with_module(true));
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.errors.is_empty() {
        return source.to_string();
    }

    let semantic_ret = SemanticBuilder::new().build(&parsed.program);
    if !semantic_ret.errors.is_empty() {
        return source.to_string();
    }
    let scoping = semantic_ret.semantic.scoping();
    let mut patches = Vec::new();

    for statement in &parsed.program.body {
        let Statement::ImportDeclaration(import) = statement else {
            continue;
        };
        let Some(replacement) = split_import_declaration(import, scoping) else {
            continue;
        };
        patches.push(SourcePatch {
            start: import.span.start as usize,
            end: import.span.end as usize,
            replacement,
        });
    }

    apply_source_patches(source, patches)
}

fn split_import_declaration(
    import: &ImportDeclaration<'_>,
    scoping: &oxc_semantic::Scoping,
) -> Option<String> {
    if import.import_kind.is_type() {
        return None;
    }
    let specifiers = import.specifiers.as_ref()?;
    let source_text = import.source.value.as_str();
    let mut default_or_namespace = Vec::new();
    let mut named = Vec::new();
    let mut has_type_only_named = false;

    for specifier in specifiers {
        match specifier {
            ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                let text = import_specifier_text(specifier);
                let is_type_only = specifier.import_kind.is_type()
                    || specifier.local.symbol_id.get().is_some_and(|symbol_id| {
                        !scoping
                            .get_resolved_references(symbol_id)
                            .any(|reference| reference.flags().is_value())
                    });
                has_type_only_named |= is_type_only;
                named.push(ImportSpecText { text, is_type_only });
            }
            ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                default_or_namespace.push(specifier.local.name.to_string());
            }
            ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                default_or_namespace.push(format!("* as {}", specifier.local.name));
            }
        }
    }

    if !has_type_only_named {
        return None;
    }

    let value_named = named
        .iter()
        .filter(|specifier| !specifier.is_type_only)
        .map(|specifier| specifier.text.as_str())
        .collect::<Vec<_>>();
    let type_named = named
        .iter()
        .filter(|specifier| specifier.is_type_only)
        .map(|specifier| specifier.text.as_str())
        .collect::<Vec<_>>();

    let mut out = String::new();
    if !default_or_namespace.is_empty() || !value_named.is_empty() {
        out.push_str("import ");
        out.push_str(&default_or_namespace.join(", "));
        if !default_or_namespace.is_empty() && !value_named.is_empty() {
            out.push_str(", ");
        }
        if !value_named.is_empty() {
            out.push_str("{ ");
            out.push_str(&value_named.join(", "));
            out.push_str(" }");
        }
        out.push_str(" from \"");
        out.push_str(source_text);
        out.push_str("\";\n");
    }
    out.push_str("import type { ");
    out.push_str(&type_named.join(", "));
    out.push_str(" } from \"");
    out.push_str(source_text);
    out.push_str("\";");
    Some(out)
}

fn import_specifier_text(specifier: &oxc_ast::ast::ImportSpecifier<'_>) -> String {
    let imported = module_export_name_text(&specifier.imported);
    let local = specifier.local.name.to_string();
    if imported == local {
        local
    } else {
        format!("{imported} as {local}")
    }
}

pub(crate) fn module_export_name_text(name: &ModuleExportName<'_>) -> String {
    match name {
        ModuleExportName::IdentifierName(identifier) => identifier.name.to_string(),
        ModuleExportName::IdentifierReference(identifier) => identifier.name.to_string(),
        ModuleExportName::StringLiteral(literal) => literal.raw.as_ref().map_or_else(
            || format!("{:?}", literal.value.as_str()),
            ToString::to_string,
        ),
    }
}

pub(crate) fn apply_source_patches(source: &str, mut patches: Vec<SourcePatch>) -> String {
    if patches.is_empty() {
        return source.to_string();
    }
    patches.sort_by_key(|patch| patch.start);
    let mut out = String::with_capacity(source.len());
    let mut last_end = 0usize;
    for patch in patches {
        if patch.start < last_end {
            continue;
        }
        out.push_str(&source[last_end..patch.start]);
        out.push_str(&patch.replacement);
        last_end = patch.end;
    }
    out.push_str(&source[last_end..]);
    out
}

/// Rewrites static `import` / `export ... from` module specifiers using OXC AST
/// spans.  This mirrors the TypeScript visitor shape used by hvigor's
/// `process_import` / `process_kit_import`: only module specifier nodes are
/// rewritten, while the surrounding declaration syntax is preserved.
pub fn rewrite_static_module_specifiers_with(
    source: &str,
    id: &str,
    mut rewrite: impl FnMut(&str) -> Option<String>,
) -> String {
    let allocator = Allocator::default();
    let source_type =
        SourceType::from_path(id).unwrap_or_else(|_| SourceType::ts().with_module(true));
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.errors.is_empty() {
        return source.to_string();
    }

    let mut patches = Vec::new();
    for statement in &parsed.program.body {
        match statement {
            Statement::ImportDeclaration(import) => {
                push_module_specifier_patch(&import.source, &mut rewrite, &mut patches);
            }
            Statement::ExportNamedDeclaration(export) => {
                if let Some(source) = &export.source {
                    push_module_specifier_patch(source, &mut rewrite, &mut patches);
                }
            }
            Statement::ExportAllDeclaration(export) => {
                push_module_specifier_patch(&export.source, &mut rewrite, &mut patches);
            }
            _ => {}
        }
    }

    apply_source_patches(source, patches)
}

fn push_module_specifier_patch(
    literal: &StringLiteral<'_>,
    rewrite: &mut impl FnMut(&str) -> Option<String>,
    patches: &mut Vec<SourcePatch>,
) {
    let specifier = literal.value.as_str();
    let Some(replacement) = rewrite(specifier) else {
        return;
    };
    if replacement == specifier {
        return;
    }
    patches.push(SourcePatch {
        start: literal.span.start as usize,
        end: literal.span.end as usize,
        replacement: format!("\"{replacement}\""),
    });
}

/// Parse `source` and re-emit it with oxc codegen, returning both the
/// regenerated source text and a per-token VLQ source map.
///
/// The source map maps generated-side positions (in the returned code string)
/// back to original positions in `source`.  The `sources[0]` entry in the map
/// is set to `id` so the final `sourceMaps.map` names the right source file.
///
/// Callers **must** return the first element of the tuple as the `code` field
/// of `HookTransformOutput`.  Using the regenerated code (rather than the
/// original text) ensures the map's generated-side coordinates exactly match
/// the code rolldown receives, which in turn allows rolldown to chain:
///
/// ```text
/// bundled-output positions
///   → regenerated-code positions  (rolldown's own codegen map)
///   → original source positions   (this function's per-token map)
/// ```
///
/// Public entry point for generating a per-token source map JSON for `source`.
/// Used by `arkdown-cli` to bypass rolldown's composed map in favour of a
/// direct per-token map from the original `.ets`/`.ts` source.
pub fn generate_source_map_for_file(source: &str, id: &str) -> Option<String> {
    source_map::build_token_source_map(source, id)
}

/// Returns `None` if parsing fails; callers should fall back to the raw source
/// text with no source map in that case.
#[cfg(test)]
fn generate_code_and_source_map(source: &str, id: &str) -> Option<(String, Option<String>)> {
    let allocator = Allocator::default();
    let source_type =
        SourceType::from_path(id).unwrap_or_else(|_| SourceType::ts().with_module(true));
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.errors.is_empty() {
        return None;
    }
    let result = Codegen::new()
        .with_options(CodegenOptions {
            source_map_path: Some(std::path::PathBuf::from(id)),
            ..CodegenOptions::default()
        })
        .build(&parsed.program);
    let map_json = result.map.map(|m| ensure_names_field(&m.to_json_string()));
    Some((result.code, map_json))
}

fn rewrite_type_only_reexports(source: &str, id: &str) -> String {
    let allocator = Allocator::default();
    let source_type =
        SourceType::from_path(id).unwrap_or_else(|_| SourceType::ts().with_module(true));
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.errors.is_empty() {
        return source.to_string();
    }

    let mut patches = Vec::new();
    for statement in &parsed.program.body {
        let Statement::ExportNamedDeclaration(export) = statement else {
            continue;
        };
        let Some(replacement) = rewrite_type_only_reexport(export, id) else {
            continue;
        };
        patches.push(SourcePatch {
            start: export.span.start as usize,
            end: export.span.end as usize,
            replacement,
        });
    }

    apply_source_patches(source, patches)
}

fn rewrite_type_only_reexport(export: &ExportNamedDeclaration<'_>, id: &str) -> Option<String> {
    if export.export_kind.is_type() || export.specifiers.is_empty() {
        return None;
    }
    let source = export.source.as_ref()?;
    let specifier = source.value.as_str();
    let target = resolve_relative_source(id, specifier)?;
    let target_exports = exported_binding_kinds(&target)?;

    let mut value_specifiers = Vec::new();
    let mut type_specifiers = Vec::new();

    for specifier in &export.specifiers {
        let local_name = module_export_name_text(&specifier.local);
        let specifier_text = export_specifier_text(specifier);
        let is_type = specifier.export_kind.is_type()
            || target_exports
                .get(&local_name)
                .is_some_and(|kind| *kind == ExportBindingKind::Type);
        if is_type {
            type_specifiers.push(specifier_text);
        } else {
            value_specifiers.push(specifier_text);
        }
    }

    if type_specifiers.is_empty() {
        return None;
    }

    let module_source = module_source_text(source);
    let mut replacement = String::new();
    if !value_specifiers.is_empty() {
        replacement.push_str("export { ");
        replacement.push_str(&value_specifiers.join(", "));
        replacement.push_str(" } from ");
        replacement.push_str(&module_source);
        replacement.push_str(";\n");
    }
    replacement.push_str("export type { ");
    replacement.push_str(&type_specifiers.join(", "));
    replacement.push_str(" } from ");
    replacement.push_str(&module_source);
    replacement.push(';');
    Some(replacement)
}

fn resolve_relative_source(id: &str, specifier: &str) -> Option<std::path::PathBuf> {
    let base = std::path::Path::new(id).parent()?;
    let candidate = base.join(specifier);
    let candidates = [
        candidate.clone(),
        candidate.with_extension("ets"),
        candidate.with_extension("ts"),
        candidate.join("Index.ets"),
        candidate.join("index.ets"),
        candidate.join("Index.ts"),
        candidate.join("index.ts"),
    ];
    candidates.into_iter().find(|path| path.is_file())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExportBindingKind {
    Type,
    Value,
}

fn exported_binding_kinds(path: &std::path::Path) -> Option<HashMap<String, ExportBindingKind>> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return None;
    };
    let allocator = Allocator::default();
    let source_type =
        SourceType::from_path(path).unwrap_or_else(|_| SourceType::ts().with_module(true));
    let parsed = Parser::new(&allocator, &content, source_type).parse();
    if !parsed.errors.is_empty() {
        return None;
    }

    let mut exports = HashMap::new();
    for statement in &parsed.program.body {
        let Statement::ExportNamedDeclaration(export) = statement else {
            continue;
        };
        if let Some(declaration) = &export.declaration {
            collect_exported_declaration(declaration, &mut exports);
        }
        for specifier in &export.specifiers {
            let exported_name = module_export_name_text(&specifier.exported);
            let local_name = module_export_name_text(&specifier.local);
            if specifier.export_kind.is_type() || export.export_kind.is_type() {
                exports.insert(exported_name, ExportBindingKind::Type);
            } else if let Some(kind) = exports.get(&local_name).copied() {
                exports.insert(exported_name, kind);
            } else {
                exports
                    .entry(exported_name)
                    .or_insert(ExportBindingKind::Value);
            }
        }
    }
    Some(exports)
}

fn collect_exported_declaration(
    declaration: &Declaration<'_>,
    exports: &mut HashMap<String, ExportBindingKind>,
) {
    match declaration {
        Declaration::TSInterfaceDeclaration(declaration) => {
            exports.insert(declaration.id.name.to_string(), ExportBindingKind::Type);
        }
        Declaration::TSTypeAliasDeclaration(declaration) => {
            exports.insert(declaration.id.name.to_string(), ExportBindingKind::Type);
        }
        Declaration::TSEnumDeclaration(declaration) if declaration.r#const => {
            exports.insert(declaration.id.name.to_string(), ExportBindingKind::Type);
        }
        Declaration::TSEnumDeclaration(declaration) => {
            exports.insert(declaration.id.name.to_string(), ExportBindingKind::Value);
        }
        Declaration::FunctionDeclaration(declaration) => {
            if let Some(id) = &declaration.id {
                exports.insert(id.name.to_string(), ExportBindingKind::Value);
            }
        }
        Declaration::ClassDeclaration(declaration) => {
            if let Some(id) = &declaration.id {
                exports.insert(id.name.to_string(), ExportBindingKind::Value);
            }
        }
        Declaration::VariableDeclaration(declaration) => {
            for declarator in &declaration.declarations {
                collect_binding_names(&declarator.id, exports);
            }
        }
        Declaration::TSModuleDeclaration(_)
        | Declaration::TSGlobalDeclaration(_)
        | Declaration::StructStatement(_)
        | Declaration::AnnotationDeclaration(_) => {}
        Declaration::TSImportEqualsDeclaration(declaration) => {
            exports.insert(declaration.id.name.to_string(), ExportBindingKind::Value);
        }
    }
}

fn collect_binding_names(
    pattern: &BindingPattern<'_>,
    exports: &mut HashMap<String, ExportBindingKind>,
) {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => {
            exports.insert(identifier.name.to_string(), ExportBindingKind::Value);
        }
        BindingPattern::ObjectPattern(pattern) => {
            for property in &pattern.properties {
                collect_binding_names(&property.value, exports);
            }
            if let Some(rest) = &pattern.rest {
                collect_binding_names(&rest.argument, exports);
            }
        }
        BindingPattern::ArrayPattern(pattern) => {
            for element in pattern.elements.iter().flatten() {
                collect_binding_names(element, exports);
            }
            if let Some(rest) = &pattern.rest {
                collect_binding_names(&rest.argument, exports);
            }
        }
        BindingPattern::AssignmentPattern(pattern) => {
            collect_binding_names(&pattern.left, exports);
        }
    }
}

fn export_specifier_text(specifier: &ExportSpecifier<'_>) -> String {
    let local = module_export_name_text(&specifier.local);
    let exported = module_export_name_text(&specifier.exported);
    if local == exported {
        local
    } else {
        format!("{local} as {exported}")
    }
}

fn module_source_text(source: &oxc_ast::ast::StringLiteral<'_>) -> String {
    source.raw.as_ref().map_or_else(
        || format!("{:?}", source.value.as_str()),
        ToString::to_string,
    )
}

/// Ensure the Source Map v3 JSON object contains a `"names"` array.
///
/// hvigor always emits `"names":[]` even when no name mappings are present.
/// oxc may omit the field when no names are tracked. This function parses the
/// JSON and inserts an empty array if the field is absent, then re-serializes.
/// If parsing fails the original string is returned unchanged.
#[cfg(test)]
fn ensure_names_field(json: &str) -> String {
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(json) else {
        return json.to_string();
    };
    if let Some(obj) = value.as_object_mut() {
        if !obj.contains_key("names") {
            obj.insert("names".to_string(), serde_json::json!([]));
        }
    }
    serde_json::to_string(&value).unwrap_or_else(|_| json.to_string())
}

fn format_hvigor_standard_ts(source: &str) -> String {
    let mut lines = Vec::new();
    let mut iter = source.lines().peekable();

    while let Some(line) = iter.next() {
        if line.trim().is_empty() {
            continue;
        }

        let trimmed = line.trim_start();
        if trimmed.ends_with('(') && trimmed.contains('.') {
            let indent = hvigor_indent(line);
            let mut args = Vec::new();
            let mut consumed = Vec::new();
            let mut closed = false;
            while let Some(arg_line) = iter.peek().copied() {
                let arg = arg_line.trim();
                if arg == ");" {
                    iter.next();
                    closed = true;
                    break;
                }
                if arg.is_empty() {
                    break;
                }
                consumed.push(arg_line);
                args.push(arg.trim_end_matches(',').to_string());
                iter.next();
            }
            if closed && !args.is_empty() {
                let prefix = trimmed.trim_end_matches('(');
                lines.push(format!("{indent}{prefix}({});", args.join(", ")));
                continue;
            }
            lines.push(format!("{}{}", hvigor_indent(line), trimmed));
            for consumed_line in consumed {
                lines.push(format!(
                    "{}{}",
                    hvigor_indent(consumed_line),
                    consumed_line.trim_start()
                ));
            }
            continue;
        }

        lines.push(format!("{}{}", hvigor_indent(line), trimmed));
    }

    let mut out = lines.join("\n");
    out.push('\n');
    out
}

fn hvigor_indent(line: &str) -> String {
    let spaces = line.bytes().take_while(|&b| b == b' ').count();
    " ".repeat(spaces * 2)
}

fn module_name_from_id(id: &str) -> Option<String> {
    id.split("/src/main/ets/").next().and_then(|prefix| {
        if prefix == id || prefix.is_empty() {
            None
        } else {
            prefix.rsplit('/').next().map(str::to_string)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkdown_core::{
        BuildMode, BuildOption, OutputTarget, ProductConfig, SdkConfig, StrictModeConfig,
    };
    use std::path::PathBuf;

    fn make_config(api_version: u32) -> Arc<ArkdownConfig> {
        Arc::new(ArkdownConfig {
            project_root: PathBuf::from("/tmp/test"),
            build_mode: BuildMode::Debug,
            output_target: OutputTarget::Hap,
            product: ProductConfig {
                name: "test".to_string(),
                bundle_name: "com.example.test".to_string(),
                compatible_sdk_version: "12".to_string(),
                runtime_os: "OpenHarmony".to_string(),
                signing_config: None,
                build_option: BuildOption {
                    strict_mode: StrictModeConfig {
                        case_sensitive_check: false,
                        use_normalized_ohm_url: true,
                    },
                    ark_options: None,
                    res_options: None,
                    source_option: None,
                    preload_so: vec![],
                },
            },
            modules: vec![],
            sdk_config: SdkConfig {
                ets_loader_path: PathBuf::from("/unused"),
                es2abc_path: PathBuf::from("/unused"),
                restool_path: PathBuf::from("/unused"),
                compile_api_version: api_version,
                has_rollup_plugin: true,
                compile_sdk_version: "6.0.0.47".to_string(),
            },
        })
    }

    #[test]
    fn test_source_map_has_names_field() {
        let json = generate_code_and_source_map("const x = 1;", "/tmp/test.ts")
            .and_then(|(_, map)| map)
            .expect("source map must be generated");
        let v: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert!(
            v.get("names").is_some(),
            "source map must contain 'names' field; got: {}",
            json
        );
        assert!(
            v["names"].is_array(),
            "'names' must be an array; got: {}",
            json
        );
    }

    #[test]
    fn test_ensure_names_field_adds_when_missing() {
        let input = r#"{"version":3,"mappings":"AAAA","sources":["test.ts"]}"#;
        let output = ensure_names_field(input);
        let v: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(v.get("names").is_some(), "names must be inserted");
        assert!(v["names"].is_array(), "names must be array");
    }

    #[test]
    fn test_ensure_names_field_preserves_existing() {
        let input = r#"{"version":3,"mappings":"AAAA","names":["foo","bar"]}"#;
        let output = ensure_names_field(input);
        let v: serde_json::Value = serde_json::from_str(&output).unwrap();
        let names = v["names"].as_array().unwrap();
        assert_eq!(names.len(), 2, "existing names must be preserved");
    }

    #[test]
    fn test_hvigor_standard_format_preserves_unmerged_multiline_call() {
        let input = "class ConfirmView {\n  checkBackAction() {\n    AlertDialog.show(\n      {\n        value: true\n      }\n    )\n  }\n}\n";
        let output = format_hvigor_standard_ts(input);
        assert!(
            output.contains("value: true"),
            "formatter must not drop call body when the close line is not `);`: {output}"
        );
        assert!(
            output.contains("    )"),
            "formatter must preserve the unmatched close line: {output}"
        );
    }

    #[test]
    fn test_exported_binding_kinds_use_ast_nodes() {
        let tmp = tempfile::Builder::new()
            .suffix(".ets")
            .tempfile()
            .expect("create temp file");
        std::fs::write(
            tmp.path(),
            "export interface BreakpointTypes<T> {}\nexport class BreakpointType<T> {}\nexport interface ResponseData {}\n",
        )
        .unwrap();
        let exports = exported_binding_kinds(tmp.path()).expect("target file must parse");
        assert!(
            exports.get("BreakpointType") == Some(&ExportBindingKind::Value),
            "BreakpointType must not match BreakpointTypes"
        );
        assert_eq!(exports.get("ResponseData"), Some(&ExportBindingKind::Type));
    }

    #[test]
    fn test_api_rewrite_legacy() {
        let result = api_transform::apply("import x from '@ohos/hilog'", 8);
        assert!(
            result.contains("@system.hilog"),
            "legacy FA rewrite must apply"
        );
    }

    #[test]
    fn test_api_rewrite_modern_unchanged() {
        let result = api_transform::apply("import x from '@ohos/hilog'", 12);
        assert!(
            result.contains("@ohos/hilog"),
            "modern import must not be rewritten"
        );
    }

    #[test]
    fn test_plugin_passthrough_non_ets_non_ts() {
        let plugin = EtsTransformPlugin::new(make_config(12));
        let result = plugin.do_transform("const x = 1;", "test.js").unwrap();
        assert!(
            result.is_none(),
            "non-ETS/non-TS modules must pass through as None"
        );
    }

    #[tokio::test]
    async fn test_plugin_kit_rewrite_in_ts_file() {
        let plugin = EtsTransformPlugin::new(make_config(12));
        let code = r#"import { hilog } from "@kit.PerformanceAnalysisKit";"#;
        let result = plugin
            .do_transform(code, "app/src/main/ets/util.ts")
            .unwrap();
        let output = result.expect("TS file with kit import must return Some");
        let code = output.code.unwrap_or_default();
        assert!(
            code.contains("@ohos:hilog"),
            "@kit.PerformanceAnalysisKit/hilog must be rewritten to @ohos:hilog; got:\n{}",
            code
        );
        assert!(
            !code.contains("@kit."),
            "@kit.* specifier must be gone; got:\n{}",
            code
        );
    }

    #[tokio::test]
    async fn test_plugin_kit_rewrite_in_ets_file() {
        let plugin = EtsTransformPlugin::new(make_config(12));
        let code = "import { UIAbility } from \"@kit.AbilityKit\";\n@Entry\n@Component\nstruct Index {\n  build() {}\n}";
        let id = "entry/src/main/ets/entryability/EntryAbility.ets";
        let result = plugin.do_transform(code, id).unwrap();
        let output = result.expect("ETS transform must return Some");
        let code = output.code.unwrap_or_default();
        assert!(
            code.contains("@ohos:app.ability.UIAbility"),
            "UIAbility must be rewritten to @ohos:app.ability.UIAbility; got:\n{}",
            code
        );
        assert!(
            !code.contains("@kit.AbilityKit"),
            "@kit.AbilityKit must be gone; got:\n{}",
            code
        );
    }

    #[tokio::test]
    async fn test_plugin_transforms_ets_to_ts() {
        let plugin = EtsTransformPlugin::new(make_config(12));
        let code = "@Entry\n@Component\nstruct Index {\n  build() {}\n}";
        let id = "entry/src/main/ets/pages/Index.ets";
        let result = plugin.do_transform(code, id).unwrap();
        let output = result.expect("ETS transform must return Some");
        let code = output.code.unwrap_or_default();
        assert!(
            code.contains("class Index extends ViewPU"),
            "struct must be desugared to ViewPU class; got:\n{}",
            code
        );
    }

    #[tokio::test]
    async fn test_plugin_lowers_minimal_arkui_build_body() {
        let plugin = EtsTransformPlugin::new(make_config(12));
        let code = "\
@Entry\n\
@Component\n\
struct Index {\n\
  @State message: string = 'Hello World';\n\
\n\
  build() {\n\
    Row() {\n\
      Column() {\n\
        Text(this.message)\n\
          .fontSize(50)\n\
          .fontWeight(FontWeight.Bold)\n\
      }\n\
      .width('100%')\n\
    }\n\
    .height('100%')\n\
  }\n\
}";
        let id = "entry/src/main/ets/pages/Index.ets";
        let result = plugin.do_transform(code, id).unwrap();
        let output = result.expect("ETS transform must return Some");
        let code = output.code.unwrap_or_default();
        assert!(
            code.contains("class Index extends ViewPU"),
            "struct must be desugared to ViewPU class; got:\n{}",
            code
        );
        assert!(
            code.contains("this.observeComponentCreation2"),
            "ArkUI build() DSL must be lowered; got:\n{}",
            code
        );
        assert!(
            code.contains("Row.create();"),
            "Row.create missing; got:\n{}",
            code
        );
        assert!(
            code.contains("Column.create();"),
            "Column.create missing; got:\n{}",
            code
        );
        assert!(
            code.contains("Text.create(this.message);"),
            "Text.create missing; got:\n{}",
            code
        );
        assert!(
            !code.contains("build() { return; }"),
            "build body must not be stubbed; got:\n{}",
            code
        );
    }

    /// Verify per-token VLQ source maps: for a .ts file the plugin must return
    /// `source_map = Some(...)` and the `mappings` field must contain at least
    /// one comma-separated segment within a single source line (i.e. more than
    /// one mapping per line), proving that the codegen emits per-token entries
    /// rather than per-statement entries.
    #[test]
    fn test_ts_file_emits_per_token_source_map() {
        let plugin = EtsTransformPlugin::new(make_config(12));
        let code = "const x: number = 1;\nconst y: string = 'hi';";
        let id = "entry/src/main/ets/util.ts";
        let out = plugin
            .do_transform(code, id)
            .unwrap()
            .expect("TS transform must return Some");
        let map_json = match &out.map {
            rolldown_plugin::HookTransformOutputMap::Sourcemap(sm) => sm.to_json_string(),
            _ => panic!("source_map must be Some for .ts files"),
        };
        let map: serde_json::Value =
            serde_json::from_str(&map_json).expect("source_map must be valid JSON");
        let mappings = map
            .get("mappings")
            .and_then(|v| v.as_str())
            .expect("source_map must have a mappings field");
        let has_per_token = mappings.lines().any(|line| line.contains(','));
        assert!(
            has_per_token,
            "mappings must have comma-separated (per-token) segments; got: {mappings}"
        );
    }

    /// For a non-struct .ets file (no @Component/@Entry) the source map must
    /// be per-token (generated coordinates match the returned code) so that
    /// rolldown can chain it into a dense final chunk source map.
    #[test]
    fn test_non_struct_ets_emits_per_token_source_map() {
        let plugin = EtsTransformPlugin::new(make_config(12));
        let code =
            "import { UIAbility } from \"@ohos:app.ability.UIAbility\";\nconst TAG = 'test';";
        let id = "entry/src/main/ets/entryability/EntryAbility.ets";
        let out = plugin
            .do_transform(code, id)
            .unwrap()
            .expect("ETS non-struct transform must return Some");
        let map_json = match &out.map {
            rolldown_plugin::HookTransformOutputMap::Sourcemap(sm) => sm.to_json_string(),
            _ => panic!("source_map must be Some for non-struct .ets files"),
        };
        let map: serde_json::Value =
            serde_json::from_str(&map_json).expect("source_map must be valid JSON");
        let mappings = map
            .get("mappings")
            .and_then(|v| v.as_str())
            .expect("source_map must have a mappings field");
        let has_per_token = mappings.lines().any(|line| line.contains(','));
        assert!(
            has_per_token,
            "mappings must have comma-separated (per-token) segments; got: {mappings}"
        );
        let sources = map
            .get("sources")
            .and_then(|v| v.as_array())
            .expect("source_map must have sources");
        assert!(
            sources
                .iter()
                .any(|s| s.as_str().is_some_and(|p| p.contains("EntryAbility"))),
            "sources must reference the original .ets file; got: {sources:?}"
        );
    }

    #[test]
    fn test_rewrites_interface_reexport_to_type_export() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let root = tmp.path();
        let model_dir = root.join("src/main/ets/model");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(
            model_dir.join("ActionEventData.ets"),
            "export interface ActionEventData { value: boolean; }",
        )
        .unwrap();
        let index = root.join("Index.ets");
        std::fs::write(&index, "export {}").unwrap();
        let source = "export { ActionEventData } from './src/main/ets/model/ActionEventData';";

        let rewritten = rewrite_type_only_reexports(source, index.to_str().unwrap());
        assert_eq!(
            rewritten.trim(),
            "export type { ActionEventData } from './src/main/ets/model/ActionEventData';"
        );
    }

    #[tokio::test]
    async fn test_splits_type_only_named_imports_with_oxc_semantic() {
        let plugin = EtsTransformPlugin::new(make_config(12));
        let code = "\
import { Runtime, Shape, Alias as AliasType } from './model';

interface Props {
  shape: Shape;
  alias: AliasType;
}

Runtime.start();
";
        let out = plugin
            .do_transform(code, "entry/src/main/ets/model/use-model.ts")
            .unwrap()
            .expect("TS transform must return Some");
        let output = out.code.unwrap_or_default();
        assert!(
            output.contains("import { Runtime } from \"./model\";"),
            "value import must remain a value import; got:\n{output}"
        );
        assert!(
            output.contains("import type { Shape, Alias as AliasType } from \"./model\";"),
            "type-only imports must be split using semantic reference flags; got:\n{output}"
        );
    }
}
