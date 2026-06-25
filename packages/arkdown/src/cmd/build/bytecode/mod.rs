//! ABC bytecode compilation stage.
//!
//! This module wraps the SDK `es2abc` command surface. It provides the typed
//! [`Ts2abcPlan`] and [`Ts2abcCompiler`]
//! entry points for the build coordinator.
//!
//! # SPEC-048 Architecture
//!
//! ```text
//! Ts2abcPlan           ← CLI BuildPlanBuilder assembles this
//!   -> Ts2abcCompiler::compile()
//!      -> SdkEs2abcBackend::compile()         → es2abc subprocess
//!      -> Ts2abcOutput
//! ```
//!
//! # File structure (SPEC-048)
//!
//! | File | Responsibility |
//! |------|---------------|
//! | `plan.rs` | `Ts2abcPlan`, `Ts2abcPlanBuilder`, `CompileUnitPlan` |
//! | `backend.rs` | `BytecodeBackend` trait, `SdkEs2abcBackend` |
//! | `file_info.rs` | `FileInfoLine`, `CacheFileManifestWriter` |
//! | `driver.rs` | SDK `es2abc` command runner |
//! | `error.rs` | `Ts2abcError` |

mod backend;
mod compile_context;
mod compiler;
mod driver;
mod error;
mod file_info;
mod plan;

// ---------------------------------------------------------------------------
// Narrow re-exports
// ---------------------------------------------------------------------------

// compiler
pub(crate) use compiler::{
  BytecodeAbcInput, RolldownBytecodeBackendChoice, RolldownBytecodeCompileInput,
  RolldownBytecodeCompiler,
};

// SDK driver output
pub(crate) use driver::Ts2abcOutput;

// Plan and manifest helpers are consumed inside this module; keep them private
// to avoid recreating the old standalone crate boundary.
