//! ABC bytecode compilation stage.
//!
//! This module wraps the SDK `es2abc` command surface. It provides the typed
//! [`Ts2abcPlan`] and [`Ts2abcCompiler`]
//! entry points for the build coordinator.
//!
//! # Architecture
//!
//! ```text
//! Ts2abcPlan           ← build coordinator assembles this
//!   -> Ts2abcCompiler::compile()
//!      -> SdkEs2abcBackend::compile()         → es2abc subprocess
//!      -> Ts2abcOutput
//! ```
//!
//! # File structure
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
pub use compiler::{
    BytecodeAbcInput, RolldownBytecodeBackendChoice, RolldownBytecodeCompileInput,
    RolldownBytecodeCompiler,
};

// SDK driver output
pub use driver::Ts2abcOutput;
pub use plan::{Ts2abcReloadMode, Ts2abcReloadPlan};

// Plan and manifest helpers remain private until another build-domain caller
// needs their typed API.
