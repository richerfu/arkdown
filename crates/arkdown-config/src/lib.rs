//! `arkdown-config` — parse OHOS project configuration files into [`arkdown_core::ArkdownConfig`].
//!
//! Entry point: [`load_config`].

pub mod build_profile;
pub mod custom_config;
pub mod error;
pub mod module_json;
pub mod resolve;
pub mod sdk;

pub use custom_config::{ArkdownCustomConfig, ArkdownCustomConfigLoader, ArkdownPluginsConfig};
pub use error::ConfigError;
pub use resolve::{load_config, ArkdownConfigLoader};
