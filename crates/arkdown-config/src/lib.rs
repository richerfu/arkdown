//! `arkdown-config` — parse OHOS project configuration files into [`arkdown_core::ArkdownConfig`].
//!
//! Entry point: [`load_config`].

pub mod build_profile;
pub mod build_script_plugin;
pub mod build_script_runtime;
pub mod build_script_schema;
pub mod custom_config;
pub mod error;
pub mod hvigor_overrides;
pub mod hvigor_runtime;
pub mod module_json;
pub mod oh_package;
pub mod resolve;
pub mod sdk;

pub use build_script_plugin::BuildScriptPluginPathResolver;
pub use build_script_runtime::{
    BuildScriptCompilePluginRecord, BuildScriptModuleSelection, BuildScriptRuntimeModulePlan,
    BuildScriptRuntimeModulePlanRequest, BuildScriptRuntimePlan, BuildScriptRuntimePlanDiagnostic,
    BuildScriptRuntimePlanRequest, BuildScriptRuntimePlanner,
};
pub use build_script_schema::{
    BuildScriptBuildOptionValidationRequest, BuildScriptSchemaDiagnostic,
    BuildScriptSchemaValidator,
};
pub use custom_config::{ArkdownCustomConfig, ArkdownCustomConfigLoader, ArkdownPluginsConfig};
pub use error::ConfigError;
pub use hvigor_overrides::{
    HvigorAppOptionOverride, HvigorSigningConfigOverride, HvigorSigningMaterialOverride,
    ModuleHvigorOverrides, ProjectHvigorOverrides, RuntimeContextOverrides,
    RuntimeModuleContextOverrides, RuntimeProjectContextOverrides,
};
pub use hvigor_runtime::{
    HvigorNativePropertyPlan, HvigorRuntimeParameterPlan, HvigorRuntimeParameterPlanRequest,
    HvigorRuntimeParameterPlanner, HvigorRuntimePropertyDiagnostic, HvigorRuntimePropertyPlan,
    HvigorRuntimePropertyPlanRequest, HvigorRuntimePropertyPlanner,
};
pub use resolve::{load_config, ArkdownConfigLoader};
