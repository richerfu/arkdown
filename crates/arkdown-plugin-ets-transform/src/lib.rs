//! Pure Rust port of the OpenHarmony SDK `ets-loader` ArkTS transform plugin.
//!
//! This crate replaces the `ets-loader` subprocess invocation described in SPEC-004
//! with a fully in-process Rust implementation. It implements three components as
//! a single [`ArkdownPlugin`][arkdown_core::ArkdownPlugin]:
//!
//! | Component      | SDK equivalent        | What it does                                      |
//! | -------------- | --------------------- | ------------------------------------------------- |
//! | `etsChecker`   | `etsChecker` plugin   | Validates decorator usage rules (diagnostics)     |
//! | `etsTransform` | `etsTransform` plugin | Desugars ArkUI decorators and struct components   |
//! | `apiTransform` | `apiTransform` plugin | Rewrites OHOS API imports by target SDK version   |
//!
//! # Usage
//!
//! ```rust,ignore
//! use std::sync::Arc;
//! use arkdown_plugin_ets_transform::EtsTransformPlugin;
//!
//! let plugin = EtsTransformPlugin::new(Arc::clone(&config));
//! loader.register_plugin(Box::new(plugin));
//! ```

pub mod api_transform;
pub mod checker;
pub mod error;
pub mod kit_map;
pub mod transform;

use std::sync::Arc;

use arkdown_core::{
    ArkdownConfig, ArkdownError, ArkdownPlugin, HookTransformOutput, HookUsage, ModuleSource,
    ModuleType, PluginContext, PluginResult, ResolvedId,
};
use async_trait::async_trait;
use oxc_allocator::Allocator;
use oxc_codegen::{Codegen, CodegenOptions};
use oxc_parser::Parser;
use oxc_span::SourceType;

/// ArkTS transform plugin — desugars `.ets` files into TypeScript.
///
/// Register this plugin in the `arkdown-loader` plugin list before any
/// TypeScript-only plugins so that `.ets` modules are converted to `.ts`
/// before downstream processing begins.
pub struct EtsTransformPlugin {
    pub config: Arc<ArkdownConfig>,
}

impl EtsTransformPlugin {
    pub fn new(config: Arc<ArkdownConfig>) -> Self {
        EtsTransformPlugin { config }
    }
}

#[async_trait]
impl ArkdownPlugin for EtsTransformPlugin {
    fn name(&self) -> &str {
        "ets-transform"
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::Transform
    }

    /// Transform an ArkTS `.ets` or `.ts` module.
    ///
    /// The pipeline:
    /// 1. Bail out immediately for files that are neither `.ets` nor `.ts`.
    /// 2. **Kit import rewrite** — `@kit.XxxKit` → individual `@ohos:xxx` imports
    ///    (both `.ets` and `.ts`; matches hvigorw ets-loader behaviour).
    /// 3. `.ets` only: run `etsChecker` diagnostics and `etsTransform`
    ///    struct/decorator/ArkUI build DSL desugaring with the ohos-rs/oxc ArkUI AST.
    /// 5. Apply version-gated API import rewrites (`@ohos/* → @system.*` for API < 9).
    /// 6. Return a new `HookTransformOutput`.
    async fn transform(
        &self,
        _ctx: &PluginContext,
        code: &str,
        id: &str,
    ) -> PluginResult<Option<HookTransformOutput>> {
        let is_ets = id.ends_with(".ets");
        let is_ts = !is_ets && id.ends_with(".ts");

        if !is_ets && !is_ts {
            return Ok(None);
        }

        // Preserve the original source before any rewriting so that source map
        // positions generated below reference the original file positions (not
        // the expanded kit-rewritten intermediate form).
        let original_source = code;

        // Step 1: Rewrite @kit.XxxKit imports to individual @ohos:xxx imports.
        // Must happen before struct desugaring so the oxc parser sees the final
        // import specifiers.
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
            let (gen_code, source_map) =
                generate_code_and_source_map(&code, id).unwrap_or((code, None));
            let final_code = api_transform::rewrite_imports_for_version(&gen_code, api_version);
            return Ok(Some(HookTransformOutput {
                code: final_code,
                source_map,
            }));
        }

        // Step 2: ArkTS struct/decorator and ArkUI build DSL desugaring (.ets only).
        // The workspace-pinned ohos-rs/oxc fork parses `.ets` with ArkUI nodes,
        // so the build body is lowered before downstream TypeScript parsing.
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
                ArkdownError::PluginError {
                    plugin: "ets-transform".to_string(),
                    source: Box::new(e),
                }
            })?;

        // Step 4: Apply version-gated API import rewrites.
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
            let (gen_code, source_map) =
                generate_code_and_source_map(&code, id).unwrap_or((final_source, None));
            let final_code = api_transform::rewrite_imports_for_version(&gen_code, api_version);
            Ok(Some(HookTransformOutput {
                code: final_code,
                source_map,
            }))
        } else {
            // Case B: struct transform applied — map from the transformed TS
            // output so coordinates are at least self-consistent for rolldown.
            // Two statements: the borrow of `final_source` in the function call
            // ends after the first statement so the second can move it into the
            // fallback tuple when needed.
            let gen_result = generate_code_and_source_map(&final_source, id);
            let (gen_code, source_map) = gen_result.unwrap_or((final_source, None));
            Ok(Some(HookTransformOutput {
                code: gen_code,
                source_map,
            }))
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
    let map_json = result.map.map(|m| m.to_json_string());
    Some((result.code, map_json))
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

    #[tokio::test]
    async fn test_plugin_passthrough_non_ets_non_ts() {
        let plugin = EtsTransformPlugin::new(make_config(12));
        // .js files are not handled by this plugin
        let result = plugin
            .transform(&PluginContext::default(), "const x = 1;", "test.js")
            .await
            .unwrap();
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
            .transform(&PluginContext::default(), code, "app/src/main/ets/util.ts")
            .await
            .unwrap();
        let out = result.expect("TS file with kit import must return Some");
        assert!(
            out.code.contains("@ohos:hilog"),
            "@kit.PerformanceAnalysisKit/hilog must be rewritten to @ohos:hilog; got:\n{}",
            out.code
        );
        assert!(
            !out.code.contains("@kit."),
            "@kit.* specifier must be gone; got:\n{}",
            out.code
        );
    }

    #[tokio::test]
    async fn test_plugin_kit_rewrite_in_ets_file() {
        let plugin = EtsTransformPlugin::new(make_config(12));
        let code = "import { UIAbility } from \"@kit.AbilityKit\";\n@Entry\n@Component\nstruct Index {\n  build() {}\n}";
        let id = "entry/src/main/ets/entryability/EntryAbility.ets";
        let result = plugin
            .transform(&PluginContext::default(), code, id)
            .await
            .unwrap();
        let out = result.expect("ETS transform must return Some");
        assert!(
            out.code.contains("@ohos:app.ability.UIAbility"),
            "UIAbility must be rewritten to @ohos:app.ability.UIAbility; got:\n{}",
            out.code
        );
        assert!(
            !out.code.contains("@kit.AbilityKit"),
            "@kit.AbilityKit must be gone; got:\n{}",
            out.code
        );
    }

    #[tokio::test]
    async fn test_plugin_transforms_ets_to_ts() {
        let plugin = EtsTransformPlugin::new(make_config(12));
        let code = "@Entry\n@Component\nstruct Index {\n  build() {}\n}";
        let id = "entry/src/main/ets/pages/Index.ets";
        let result = plugin
            .transform(&PluginContext::default(), code, id)
            .await
            .unwrap();
        let out = result.expect("ETS transform must return Some(HookTransformOutput)");
        assert!(
            out.code.contains("class Index extends ViewPU"),
            "struct must be desugared to ViewPU class; got:\n{}",
            out.code
        );
    }

    #[tokio::test]
    async fn test_plugin_lowers_minimal_arkui_build_body() {
        let plugin = EtsTransformPlugin::new(make_config(12));
        // Typical Index.ets page with ArkUI declarative DSL in build().
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
        let result = plugin
            .transform(&PluginContext::default(), code, id)
            .await
            .unwrap();
        let out = result.expect("ETS transform must return Some(HookTransformOutput)");
        assert!(
            out.code.contains("class Index extends ViewPU"),
            "struct must be desugared to ViewPU class; got:\n{}",
            out.code
        );
        assert!(
            out.code.contains("this.observeComponentCreation2"),
            "ArkUI build() DSL must be lowered; got:\n{}",
            out.code
        );
        assert!(
            out.code.contains("Row.create();"),
            "Row.create missing; got:\n{}",
            out.code
        );
        assert!(
            out.code.contains("Column.create();"),
            "Column.create missing; got:\n{}",
            out.code
        );
        assert!(
            out.code.contains("Text.create(this.message);"),
            "Text.create missing; got:\n{}",
            out.code
        );
        assert!(
            !out.code.contains("build() { return; }"),
            "build body must not be stubbed; got:\n{}",
            out.code
        );
    }

    /// Verify per-token VLQ source maps: for a .ts file the plugin must return
    /// `source_map = Some(...)` and the `mappings` field must contain at least
    /// one comma-separated segment within a single source line (i.e. more than
    /// one mapping per line), proving that the codegen emits per-token entries
    /// rather than per-statement entries.
    #[tokio::test]
    async fn test_ts_file_emits_per_token_source_map() {
        let plugin = EtsTransformPlugin::new(make_config(12));
        let code = "const x: number = 1;\nconst y: string = 'hi';";
        let id = "entry/src/main/ets/util.ts";
        let result = plugin
            .transform(&PluginContext::default(), code, id)
            .await
            .unwrap();
        let out = result.expect("TS transform must return Some");
        let map_json = out.source_map.expect("source_map must be Some for .ts files");
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
    #[tokio::test]
    async fn test_non_struct_ets_emits_per_token_source_map() {
        let plugin = EtsTransformPlugin::new(make_config(12));
        let code = "import { UIAbility } from \"@ohos:app.ability.UIAbility\";\nconst TAG = 'test';";
        let id = "entry/src/main/ets/entryability/EntryAbility.ets";
        let result = plugin
            .transform(&PluginContext::default(), code, id)
            .await
            .unwrap();
        let out = result.expect("ETS non-struct transform must return Some");
        let map_json = out
            .source_map
            .expect("source_map must be Some for non-struct .ets files");
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
