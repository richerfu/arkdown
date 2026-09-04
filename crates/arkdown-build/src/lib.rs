//! Typed build-application services behind arkdown's N-API boundary.
//!
//! The npm JavaScript CLI remains the only command-line entrypoint. This crate
//! owns Rust build planning and orchestration without depending on N-API or
//! exposing argv/process lifecycle behavior.

pub mod bundle_plan;
pub mod bytecode_har_declaration;
pub mod check;
pub mod clean;
#[cfg(test)]
mod clean_tests;
pub mod code_resource;
pub mod compile_context;
pub mod device_test;
pub mod har_dedup;
pub mod hot_reload;
#[cfg(test)]
mod hot_reload_tests;
pub mod intermediates;
pub mod layout;
pub mod module_plan;
pub mod native_dependency;
pub mod package_resolution;
pub mod planning;
pub mod pre_build;
pub mod preview;
pub mod quickfix;
#[cfg(test)]
mod quickfix_tests;
pub mod remote_hsp_sign;
pub mod request;
pub mod sdk_checker;
#[cfg(test)]
mod sdk_checker_tests;
pub mod source_snapshot;
pub mod unit_test;
#[cfg(test)]
mod unit_test_tests;
pub mod visual_codegen;

#[cfg(test)]
mod code_resource_tests;
#[cfg(test)]
mod layout_tests;
