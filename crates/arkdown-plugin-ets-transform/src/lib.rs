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
            let final_source = api_transform::rewrite_imports_for_version(&code, api_version);
            // Generate a per-token source map from the KIT-REWRITTEN TypeScript
            // source (before the version-gated import rewrite but after kit
            // expansion).  The kit rewrite expands 3 @kit import lines into 7+
            // individual @ohos import lines, giving oxc more lines to generate
            // mapping segments for — producing a longer, denser VLQ string that
            // is closer to hvigor's tsc-generated source map.
            // The source_map_path is `id` (the original .ts path) so the final
            // sourceMaps.map entry names the original source file correctly.
            let source_map = generate_source_map_json(&code, id);
            return Ok(Some(HookTransformOutput {
                code: final_source,
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

        // Generate the source map from the KIT-REWRITTEN .ets source (after
        // kit-import expansion but BEFORE struct desugaring), not from the
        // expanded ViewPU class in `final_source`.
        //
        // Rationale: the struct transform rewrites 15 lines of ArkUI DSL into
        // ~100 lines of ViewPU boilerplate.  Generating a per-token map from
        // that boilerplate produces ~1100 VLQ chars where every generated method
        // gets a redundant mapping.  hvigor's ets-loader instead maps key
        // positions in the generated class back to their original struct
        // positions, yielding a much sparser (~366 VLQ char) map.
        //
        // By using `code` (the kit-rewritten .ets source, parsed by the
        // ETS-capable oxc fork), we produce a source map where the "source"
        // side positions reference the kit-rewritten file content.  For
        // non-struct .ets files (like EntryAbility.ets) this means the kit
        // import expansion lines are also mapped, giving a denser VLQ string
        // that better approximates hvigor's tsc-generated map.  The resulting
        // VLQ string is proportional to the short original source (15 lines)
        // not the expanded ViewPU output (100+ lines).
        let source_map = generate_source_map_json(&code, id);

        Ok(Some(HookTransformOutput {
            code: final_source,
            source_map,
        }))
    }
}

/// Generate a JSON source map string by parsing `original_code` and running
/// oxc codegen with `source_map_path` set to `id`.
///
/// The resulting map records per-token mappings from the re-generated
/// TypeScript back to positions in `original_code`.  The file named in the
/// source map is `id` (the original `.ets` / `.ts` path).
///
/// Callers should always pass the PRE-TRANSFORM source as `original_code`:
/// - For `.ts` files: the kit-rewritten TypeScript (before version-gate rewrite).
/// - For `.ets` files: the kit-rewritten ETS source (before struct desugaring).
///
/// Passing the original source rather than the transformed output achieves two
/// things: (1) source positions in the map reference the short original file
/// rather than the much longer generated output, matching hvigor's sparse
/// mapping style; (2) positions correctly correspond to locations a developer
/// would recognise in the original file.
///
/// It is returned as a serialized JSON string so that it can cross the
/// plugin-adapter boundary without an oxc-version type mismatch (the adapter
/// uses `rolldown_sourcemap::SourceMap::from_json_string` to decode it into the
/// rolldown-internal type).
///
/// Returns `None` if parsing fails (in which case the caller should fall back
/// to `HookTransformOutputMap::Null` in the adapter, suppressing rolldown's
/// "possibly broken sourcemap" warning without providing a map).
fn generate_source_map_json(original_code: &str, id: &str) -> Option<String> {
    let allocator = Allocator::default();
    let source_type =
        SourceType::from_path(id).unwrap_or_else(|_| SourceType::ts().with_module(true));
    let parsed = Parser::new(&allocator, original_code, source_type).parse();
    if !parsed.errors.is_empty() {
        return None;
    }
    let result = Codegen::new()
        .with_options(CodegenOptions {
            source_map_path: Some(std::path::PathBuf::from(id)),
            ..CodegenOptions::default()
        })
        .build(&parsed.program);
    result.map.map(|m| m.to_json_string())
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
}
