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

    /// Core transform logic: returns `(transformed_code, source_map_json)` or `None` to skip.
    pub fn do_transform(
        &self,
        code: &str,
        id: &str,
    ) -> anyhow::Result<Option<(String, Option<String>)>> {
        let is_ets = id.ends_with(".ets");
        let is_ts = !is_ets && id.ends_with(".ts");

        if !is_ets && !is_ts {
            return Ok(None);
        }

        let original_source = code;
        let code = api_transform::rewrite_kit_imports(original_source);
        let api_version = self.config.sdk_config.compile_api_version;

        if !is_ets {
            let final_source = api_transform::rewrite_imports_for_version(&code, api_version);
            let source_map = generate_source_map_json(&code, id);
            return Ok(Some((final_source, source_map)));
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
        let source_map = generate_source_map_json(&code, id);

        Ok(Some((final_source, source_map)))
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
        async move {
            let plugin = EtsTransformPlugin::new(config);
            let result = plugin.do_transform(&code, &id)?;
            Ok(result.map(|(transformed, source_map_json)| {
                let map = match source_map_json {
                    Some(ref json) => match RolldownSourceMap::from_json_string(json) {
                        Ok(sm) => HookTransformOutputMap::Sourcemap(Box::new(sm)),
                        Err(_) => HookTransformOutputMap::Null,
                    },
                    None => HookTransformOutputMap::Null,
                };
                HookTransformOutput {
                    code: Some(transformed),
                    map,
                    ..Default::default()
                }
            }))
        }
    }
}

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
        let (code, _map) = result.expect("TS file with kit import must return Some");
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
        let (code, _map) = result.expect("ETS transform must return Some");
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
        let (code, _map) = result.expect("ETS transform must return Some");
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
        let (code, _map) = result.expect("ETS transform must return Some");
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
}
