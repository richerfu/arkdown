//! Typed build-application services behind arkdown's N-API boundary.
//!
//! The npm JavaScript CLI remains the only command-line entrypoint. This crate
//! owns Rust build planning and orchestration without depending on N-API or
//! exposing argv/process lifecycle behavior.

pub mod app_coordinator;
#[cfg(test)]
mod application_tests;
pub mod bundle_plan;
#[cfg(test)]
mod bundle_plan_tests;
pub mod bytecode_har_declaration;
pub mod check;
pub mod clean;
#[cfg(test)]
mod clean_tests;
pub mod code_resource;
pub mod compile_context;
mod compile_project_config;
pub mod coordinator;
pub mod device_test;
pub mod har_dedup;
pub mod hot_reload;
#[cfg(test)]
mod hot_reload_tests;
pub mod intermediates;
pub mod layout;
pub mod module_plan;
pub mod native_dependency;
mod native_stage;
pub mod obfuscation;
pub mod package_resolution;
mod pkg_sdk_info;
pub mod planning;
pub mod plugin_share;
pub mod pre_build;
pub mod preview;
mod progress;
mod project_checker;
pub mod quickfix;
#[cfg(test)]
mod quickfix_tests;
pub mod remote_hsp_sign;
pub mod request;
pub mod source_snapshot;
pub mod static_arkts;
pub mod task_cache;
pub mod unit_test;
#[cfg(test)]
mod unit_test_tests;
pub mod visual_codegen;
mod watch_stage;

mod service;
pub use service::BuildService;

#[cfg(test)]
mod code_resource_tests;
#[cfg(test)]
mod layout_tests;
