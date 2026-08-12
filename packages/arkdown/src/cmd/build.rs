pub mod app_coordinator;
pub mod args;
mod packing_check;
mod pre_build;
mod remote_hsp_sign;
mod bundle_plan;
pub(crate) mod bytecode;
mod bytecode_har_declaration;
mod bytecode_stage;
mod code_resource;
#[cfg(test)]
mod code_resource_tests;
pub mod coordinator;
mod coverage_instrument;
mod generate_har_declarations;
#[cfg(test)]
mod generate_har_declarations_tests;
mod har_dedup;
#[cfg(test)]
mod har_dedup_tests;
mod har_importee_cache;
#[cfg(test)]
mod har_importee_cache_tests;
mod intermediates;
mod layout;
mod module_plan;
mod preload_system_so;
mod syscap;
mod unit_test;
#[cfg(test)]
mod tests;
mod watch_stage;

pub use args::{BuildArgs, BuildCommand};
