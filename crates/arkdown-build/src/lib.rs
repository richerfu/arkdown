//! Typed build-application services behind arkdown's N-API boundary.
//!
//! The npm JavaScript CLI remains the only command-line entrypoint. This crate
//! owns Rust build planning and orchestration without depending on N-API or
//! exposing argv/process lifecycle behavior.

pub mod intermediates;
pub mod layout;
pub mod pre_build;

#[cfg(test)]
mod layout_tests;
