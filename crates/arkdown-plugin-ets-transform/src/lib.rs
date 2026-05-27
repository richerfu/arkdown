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

    /// Transform an ArkTS `.ets` module into TypeScript.
    ///
    /// The pipeline:
    /// 1. Bail out immediately for non-`.ets` modules (`Ok(None)` → pass through).
    /// 2. Run `etsChecker` — validation diagnostics, never fatal on warnings.
    /// 3. Run `etsTransform` — struct/decorator desugaring.
    /// 4. Run `apiTransform` — version-gated API import rewrites.
    /// 5. Return a new `HookTransformOutput` with the desugared TypeScript source.
    async fn transform(
        &self,
        _ctx: &PluginContext,
        code: &str,
        id: &str,
    ) -> PluginResult<Option<HookTransformOutput>> {
        if !id.ends_with(".ets") {
            return Ok(None);
        }

        // Build a temporary ModuleSource so the existing transform logic can be reused.
        let source = ModuleSource {
            id: ResolvedId {
                id: id.to_string(),
                external: false,
                module_type: ModuleType::Ets,
                ..Default::default()
            },
            content: code.to_string(),
            source_map: None,
        };

        let transformed = transform::transform_ets(&source).map_err(|e| {
            ArkdownError::PluginError {
                plugin: "ets-transform".to_string(),
                source: Box::new(e),
            }
        })?;

        let api_version = self.config.sdk_config.compile_api_version;
        let final_source = api_transform::apply(&transformed, api_version);

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
    async fn test_plugin_passthrough_non_ets() {
        let plugin = EtsTransformPlugin::new(make_config(12));
        let result = plugin
            .transform(&PluginContext::default(), "const x = 1;", "test.ts")
            .await
            .unwrap();
        assert!(result.is_none(), "non-ETS modules must pass through as None");
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
}
