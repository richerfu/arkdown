//! `arkdown-config` — parse OHOS project configuration files into [`arkdown_core::ArkdownConfig`].
//!
//! Entry point: [`load_config`].

pub mod build_profile;
pub mod error;
pub mod hvigor_overrides;
pub mod module_json;
pub mod oh_package;
pub mod resolve;
pub mod sdk;

pub use error::ConfigError;
pub use hvigor_overrides::{
    HvigorAppOptionOverride, HvigorSigningConfigOverride, HvigorSigningMaterialOverride,
    ModuleHvigorOverrides, ProjectHvigorOverrides,
};
pub use resolve::load_config;
