//! ABC bytecode compilation services shared by arkdown build applications.
//!
//! The npm JavaScript CLI remains arkdown's only executable entrypoint. This
//! crate owns typed bytecode plans, SDK handoff manifests, and compiler
//! execution. It deliberately has no argv parser, process lifecycle, terminal
//! reporting, or N-API dependency.

pub mod aot;
pub mod bytecode;

#[cfg(test)]
mod aot_tests;
