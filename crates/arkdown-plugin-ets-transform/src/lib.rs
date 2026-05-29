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
use std::sync::Arc;

use arkdown_core::{ArkdownConfig, ModuleSource, ModuleType, ResolvedId};
use oxc_allocator::Allocator;
use oxc_codegen::{Codegen, CodegenOptions};
use oxc_parser::Parser;
use oxc_span::SourceType;
use rolldown_plugin::{
    HookTransformArgs, HookTransformOutput, HookTransformOutputMap, HookTransformReturn, HookUsage,
    Plugin, SharedTransformPluginContext,
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
        let code = api_transform::rewrite_kit_imports(original_source);
        let api_version = self.config.sdk_config.compile_api_version;

        if !is_ets {
            // .ts files: kit rewrite + version-gated rewrites only.
            //
            // Return the oxc-regenerated code together with its source map so
            // that the map's generated-side coordinates exactly match the code
            // we hand to rolldown.  rolldown can then chain:
            //   bundled-output → regenerated-code → kit-rewritten source
            // producing a per-token map in the final chunk.source_map.
            let (gen_code, source_map_json) =
                generate_code_and_source_map(&code, id).unwrap_or((code, None));
            let final_code = api_transform::rewrite_imports_for_version(&gen_code, api_version);
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

        let transformed =
            transform::transform_ets_with_options(&source, &options).map_err(|e| {
                anyhow::anyhow!("ets-transform: {e}")
            })?;

        let final_source = api_transform::rewrite_imports_for_version(&transformed, api_version);

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
            // Case A: no struct transform — use per-token regenerated code.
            let (gen_code, source_map_json) =
                generate_code_and_source_map(&code, id).unwrap_or((final_source, None));
            let final_code = api_transform::rewrite_imports_for_version(&gen_code, api_version);
            let map = source_map_json
                .as_deref()
                .and_then(|j| RolldownSourceMap::from_json_string(j).ok())
                .map(|sm| HookTransformOutputMap::Sourcemap(Box::new(sm)))
                .unwrap_or(HookTransformOutputMap::Null);
            Ok(Some(HookTransformOutput {
                code: Some(final_code),
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
            let (gen_code, source_map) = transform::transform_ets_viewpu_with_source_map(
                &source,
                &options,
                &source.content,
            )
            .map(|(code, map)| {
                let final_code =
                    api_transform::rewrite_imports_for_version(&code, api_version);
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
        HookUsage::Transform
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
        let out = plugin.do_transform(code, id).unwrap()
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
        let code = "import { UIAbility } from \"@ohos:app.ability.UIAbility\";\nconst TAG = 'test';";
        let id = "entry/src/main/ets/entryability/EntryAbility.ets";
        let out = plugin.do_transform(code, id).unwrap()
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
}
