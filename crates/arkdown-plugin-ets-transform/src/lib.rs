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
mod hvigor_format;
pub mod kit_map;
mod module_emit;
mod module_rewrite;
mod resource_transform;
mod source_edit;
pub mod source_map;
mod sys_resource;
pub mod transform;
mod typescript_emit;

use std::borrow::Cow;
use rustc_hash::FxHashMap;
use std::sync::{Arc, Mutex};

use arkdown_core::{ArkdownConfig, ModuleSource, ModuleType, ResolvedId};
use hvigor_format::{format_hvigor_standard_ts, module_name_from_id};
use module_rewrite::{rewrite_type_only_reexports, split_type_only_named_imports};
#[cfg(test)]
use oxc_allocator::Allocator;
#[cfg(test)]
use oxc_codegen::{Codegen, CodegenOptions};
#[cfg(test)]
use oxc_parser::Parser;
#[cfg(test)]
use oxc_span::SourceType;
use resource_transform::rewrite_resource_references;
use rolldown_plugin::{
    HookLoadArgs, HookLoadOutput, HookLoadReturn, HookTransformArgs, HookTransformOutput,
    HookTransformOutputMap, HookTransformReturn, HookUsage, Plugin, SharedLoadPluginContext,
    SharedTransformPluginContext,
};
use rolldown_sourcemap::SourceMap as RolldownSourceMap;

pub use module_rewrite::rewrite_static_module_specifiers_with;
#[cfg(test)]
pub(crate) use module_rewrite::{exported_binding_kinds, ExportBindingKind};
pub use resource_transform::ResourceReference;
pub(crate) use source_edit::{
    apply_source_patches, apply_source_patches_with_offsets, module_export_name_text, SourcePatch,
};
pub use sys_resource::load_system_resource_references;

/// ArkTS transform plugin — desugars `.ets` files into TypeScript.
///
/// Register this plugin in the plugin list before any TypeScript-only plugins
/// so that `.ets` modules are converted to `.ts` before downstream processing begins.
pub struct EtsTransformPlugin {
    pub config: Arc<ArkdownConfig>,
    resource_refs: Arc<FxHashMap<String, ResourceReference>>,
    component_metadata: Arc<Mutex<Option<transform::component_meta::ComponentMetadata>>>,
}

impl std::fmt::Debug for EtsTransformPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EtsTransformPlugin").finish_non_exhaustive()
    }
}

impl EtsTransformPlugin {
    pub fn new(config: Arc<ArkdownConfig>) -> Self {
        EtsTransformPlugin {
            config,
            resource_refs: Arc::new(FxHashMap::default()),
            component_metadata: Arc::new(Mutex::new(None)),
        }
    }

    pub fn with_resource_references(
        config: Arc<ArkdownConfig>,
        resource_refs: FxHashMap<String, ResourceReference>,
    ) -> Self {
        EtsTransformPlugin {
            config,
            resource_refs: Arc::new(resource_refs),
            component_metadata: Arc::new(Mutex::new(None)),
        }
    }

    fn component_metadata(&self) -> anyhow::Result<transform::component_meta::ComponentMetadata> {
        let mut guard = self
            .component_metadata
            .lock()
            .map_err(|_| anyhow::anyhow!("component metadata cache poisoned"))?;
        if let Some(metadata) = guard.as_ref() {
            return Ok(metadata.clone());
        }
        let metadata = transform::component_meta::ComponentMetadata::from_ets_loader_path(
            &self.config.sdk_config.ets_loader_path,
        )?;
        *guard = Some(metadata.clone());
        Ok(metadata)
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
            let final_code = typescript_emit::rewrite_imported_const_enums(&final_code, id);
            let final_code = split_type_only_named_imports(&final_code, id);
            let final_code =
                rewrite_resource_references(&final_code, id, self.resource_refs.as_ref());
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
            project_root: Some(self.config.project_root.clone()),
            module_roots: self
                .config
                .modules
                .iter()
                .map(|module| (module.name.clone(), module.src_path.clone()))
                .collect(),
            component_metadata: self.component_metadata()?,
        };

        let transformed = transform::transform_ets_with_options(&source, &options).map_err(|e| {
            tracing::error!(target: "arkdown::ets_transform", file = %id, error = %e, "ETS transform failed");
            anyhow::anyhow!("ets-transform: {e}")
        })?;

        let final_source = api_transform::rewrite_imports_for_version(&transformed, api_version);
        let final_source = typescript_emit::rewrite_imported_const_enums(&final_source, id);
        let final_source = split_type_only_named_imports(&final_source, id);
        let final_source =
            rewrite_resource_references(&final_source, id, self.resource_refs.as_ref());

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
                    .map_err(|e| {
                        tracing::error!(target: "arkdown::ets_transform", file = %id, error = %e, "ETS source-map transform failed");
                        anyhow::anyhow!("ets-transform source-map: {e}")
                    })
                    .map(|(code, map)| {
                        let final_code =
                            api_transform::rewrite_imports_for_version(&code, api_version);
                        let final_code =
                            typescript_emit::rewrite_imported_const_enums(&final_code, id);
                        let final_code = split_type_only_named_imports(&final_code, id);
                        let final_code = rewrite_resource_references(
                            &final_code,
                            id,
                            self.resource_refs.as_ref(),
                        );
                        (final_code, map)
                    })?;
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

    pub fn do_compile_transform(
        &self,
        code: &str,
        id: &str,
        emit_js: bool,
    ) -> anyhow::Result<Option<HookTransformOutput>> {
        let Some(mut output) = self.do_transform(code, id)? else {
            return Ok(None);
        };
        if emit_js {
            if let Some(current_code) = output.code.as_deref() {
                if let Some(js_code) = typescript_emit::emit_javascript(current_code, id) {
                    let source_map_json = source_map::build_token_source_map(&js_code, id);
                    output.map = source_map_json
                        .as_deref()
                        .and_then(|j| RolldownSourceMap::from_json_string(j).ok())
                        .map(|sm| HookTransformOutputMap::Sourcemap(Box::new(sm)))
                        .unwrap_or(HookTransformOutputMap::Null);
                    output.code = Some(js_code);
                }
            }
        }
        Ok(Some(output))
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
        let resource_refs = Arc::clone(&self.resource_refs);
        let component_metadata = Arc::clone(&self.component_metadata);
        async move {
            if !id.ends_with(".ets") {
                return Ok(None);
            }
            let code = match std::fs::read_to_string(&id) {
                Ok(code) => code,
                Err(_) => return Ok(None),
            };
            if !transform::has_arkts_transform_syntax(&code, &id) {
                return Ok(None);
            }
            let output = EtsTransformPlugin {
                config,
                resource_refs,
                component_metadata,
            }
            .do_transform(&code, &id)?;
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
        let resource_refs = Arc::clone(&self.resource_refs);
        let component_metadata = Arc::clone(&self.component_metadata);
        async move {
            EtsTransformPlugin {
                config,
                resource_refs,
                component_metadata,
            }
            .do_transform(&code, &id)
        }
    }
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

/// Returns `None` if parsing fails; tests use that to assert source-map
/// generation only when OXC parsed the source successfully.
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
                ets_loader_path: PathBuf::from(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/tests/fixtures/ets-loader"
                )),
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

    #[test]
    fn test_plugin_lowers_resource_calls_from_ast() {
        let plugin = EtsTransformPlugin::with_resource_references(
            make_config(12),
            FxHashMap::from_iter([
                (
                    "app.string.title".to_string(),
                    ResourceReference {
                        id: 0x0100_0001,
                        runtime_type: 10003,
                        bundle_name: "com.example.test".to_string(),
                        module_name: "entry".to_string(),
                    },
                ),
                (
                    "app.float.topic_icon_border".to_string(),
                    ResourceReference {
                        id: 0x0100_0002,
                        runtime_type: 10002,
                        bundle_name: "com.example.test".to_string(),
                        module_name: "entry".to_string(),
                    },
                ),
                (
                    "sys.color.ohos_id_color_emphasize".to_string(),
                    ResourceReference {
                        id: 125829135,
                        runtime_type: 10001,
                        bundle_name: "com.example.test".to_string(),
                        module_name: "entry".to_string(),
                    },
                ),
            ]),
        );
        let code = "\
const text = \"$r('app.string.title')\";\n\
const title = $r('app.string.title', 'fallback');\n\
const sysColor = $r('sys.color.ohos_id_color_emphasize');\n\
const raw = $rawfile('data/config.json');\n\
const dynamic = $r(resourceName);\n\
if (!(\"finalizeConstruction\" in ViewPU.prototype)) {\n\
  Reflect.set(ViewPU.prototype, \"finalizeConstruction\", () => { });\n\
}\n\
interface TopicItemView_Params { item?: Topic; }\n\
import type { Topic } from \"@bundle:com.example.test/entry@topic/ets/model/Topic\";\n\
class TopicItemView extends ViewPU {\n\
  private __isSelected: ObservedPropertySimplePU<boolean>;\n\
  initialRender() {\n\
    Button.borderWidth(this.isSelected ? $r('app.float.topic_icon_border') : 0);\n\
  }\n\
}\n\
function __Text__topicTitleStyle(): void {\n\
  Text.fontSize($r('app.float.topic_icon_border'));\n\
}\n";
        let result = plugin
            .do_transform(code, "app/src/main/ets/util.ets")
            .unwrap()
            .expect("ETS transform must return Some");
        let output = result.code.unwrap_or_default();
        assert!(
            output.contains("const text = \"$r('app.string.title')\";"),
            "resource-like text inside a string literal must not be rewritten; got:\n{output}"
        );
        assert!(
            output.contains(
                "const title = { \"id\": 16777217, \"type\": 10003, params: ['fallback'], \"bundleName\": \"com.example.test\", \"moduleName\": \"entry\" };"
            ),
            "static $r call must become the hvigor resource object; got:\n{output}"
        );
        assert!(
            output.contains(
                "const raw = { \"id\": 0, \"type\": 30000, params: ['data/config.json'], \"bundleName\": \"com.example.test\", \"moduleName\": \"entry\" };"
            ),
            "static $rawfile call must become the hvigor rawfile object; got:\n{output}"
        );
        assert!(
            output.contains(
                "const sysColor = { \"id\": 125829135, \"type\": 10001, params: [], \"bundleName\": \"com.example.test\", \"moduleName\": \"entry\" };"
            ),
            "static sys $r call must become the hvigor resource object; got:\n{output}"
        );
        assert!(
            output.contains(
                "const dynamic = { \"id\": -1, \"type\": -1, params: [resourceName], \"bundleName\": \"com.example.test\", \"moduleName\": \"entry\" };"
            ),
            "dynamic $r call must become the hvigor variable resource object; got:\n{output}"
        );
        assert!(
            output.contains(
                "Button.borderWidth(this.isSelected ? { \"id\": 16777218, \"type\": 10002, params: [], \"bundleName\": \"com.example.test\", \"moduleName\": \"entry\" } : 0);"
            ),
            "resource call inside generated component class must be lowered; got:\n{output}"
        );
        assert!(
            output.contains(
                "Text.fontSize({ \"id\": 16777218, \"type\": 10002, params: [], \"bundleName\": \"com.example.test\", \"moduleName\": \"entry\" });"
            ),
            "resource call inside generated style function must be lowered; got:\n{output}"
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
            output.contains("import { Runtime } from './model';"),
            "value import must remain a value import; got:\n{output}"
        );
        assert!(
            output.contains("import type { Shape, Alias as AliasType } from './model';"),
            "type-only imports must be split using semantic reference flags; got:\n{output}"
        );
    }

    #[test]
    fn test_static_rewrite_keeps_relative_type_only_imports() {
        let code = "\
import type { HomeResources } from './HomeResource';
import { Runtime } from './Runtime';
export type { Model } from './Model';
export { Value } from './Value';
";
        let out = rewrite_static_module_specifiers_with(
            code,
            "entry/src/main/ets/model/Use.ts",
            |spec| {
                spec.starts_with('.')
                    .then(|| format!("@bundle:com.example/entry/ets/model/{spec}"))
            },
        );
        assert!(
            out.contains("import type { HomeResources } from './HomeResource';"),
            "relative type-only imports must stay relative like upstream import_path_expand; got:\n{out}"
        );
        assert!(
            out.contains("export type { Model } from './Model';"),
            "relative type-only exports must stay relative; got:\n{out}"
        );
        assert!(
            out.contains(
                "import { Runtime } from \"@bundle:com.example/entry/ets/model/./Runtime\";"
            ) && out
                .contains("export { Value } from \"@bundle:com.example/entry/ets/model/./Value\";"),
            "value imports/exports should still rewrite; got:\n{out}"
        );
    }
}
