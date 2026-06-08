//! `ts2abc`/`es2abc` driver layer.
//!
//! The upstream SDK script is only a wrapper around the native `es2abc`
//! backend. This crate models that command surface in Rust and can route
//! compile work either into arkdown's Rust bytecode backend or the SDK
//! `es2abc` executable.

pub mod args;
pub mod compile_context;
pub mod driver;
pub mod error;
pub mod version;

pub use args::{Ts2abcCommand, VersionQuery};
pub use compile_context::{CompileContextInfo, PkgInfo};
pub use driver::{
    compile_to_abc, compile_to_abc_with_backend, ArkdownAbcBackend, BytecodeBackend,
    SdkEs2abcBackend, Ts2abcInput, Ts2abcOptions, Ts2abcOutput,
};
pub use error::Ts2abcError;
pub use version::{
    es2abc_target_bc_version, ts2abc_script_target_bc_version, BytecodeVersion,
    DEFAULT_TARGET_API_VERSION,
};
