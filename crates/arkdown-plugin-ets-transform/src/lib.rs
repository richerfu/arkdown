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

use async_trait::async_trait;
use arkdown_core::{
    ArkdownConfig, ArkdownError, ArkdownPlugin, HookTransformOutput, HookUsage, ModuleSource,
    ModuleType, PluginContext, PluginResult, ResolvedId,
};

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
    /// 3. `.ets` only: **ArkUI build() stub** — replaces declarative DSL bodies with
    ///    `return;` so the OXC parser can accept the file as TypeScript.
    /// 4. `.ets` only: run `etsChecker` diagnostics and `etsTransform` struct/decorator
    ///    desugaring.
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

        // Step 1: Rewrite @kit.XxxKit imports to individual @ohos:xxx imports.
        // Must happen before struct desugaring so the oxc parser sees the final
        // import specifiers.
        let code = api_transform::rewrite_kit_imports(code);

        let api_version = self.config.sdk_config.compile_api_version;

        if !is_ets {
            // .ts files: kit rewrite + version-gated rewrites only.
            let final_source = api_transform::rewrite_imports_for_version(&code, api_version);
            return Ok(Some(HookTransformOutput {
                code: final_source,
                source_map: None,
            }));
        }

        // Step 2: Stub ArkUI build() DSL bodies so the source is valid TypeScript
        // for the downstream OXC parser.  The ArkUI declarative syntax
        // (e.g. `Row() { Column() { } }.width()`) is not legal TypeScript; replace
        // it with `return;` before struct desugaring.
        let code = api_transform::stub_arkui_build_bodies(&code);

        // Step 3: ArkTS struct/decorator desugaring (.ets only).
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

        let transformed = transform::transform_ets(&source).map_err(|e| {
            ArkdownError::PluginError {
                plugin: "ets-transform".to_string(),
                source: Box::new(e),
            }
        })?;

        // Step 4: Apply version-gated API import rewrites.
        let final_source = api_transform::rewrite_imports_for_version(&transformed, api_version);

        Ok(Some(HookTransformOutput {
            code: final_source,
            source_map: None,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkdown_core::{
        BuildMode, OutputTarget, ProductConfig, SdkConfig, StrictModeConfig, BuildOption,
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
        assert!(result.contains("@system.hilog"), "legacy FA rewrite must apply");
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
        assert!(result.is_none(), "non-ETS/non-TS modules must pass through as None");
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
    async fn test_plugin_stubs_arkui_build_body() {
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
            out.code.contains("build() { return; }"),
            "ArkUI build() DSL must be stubbed; got:\n{}",
            out.code
        );
        assert!(
            !out.code.contains("Row()"),
            "ArkUI Row() DSL must be gone; got:\n{}",
            out.code
        );
    }
}
