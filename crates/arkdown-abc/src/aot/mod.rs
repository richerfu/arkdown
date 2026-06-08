//! Upstream AOT compile chain for the pure-Rust ABC backend.
//!
//! This module implements the full upstream AOT architecture from
//! `es2panda/aot/main.cpp`:
//!
//! ```text
//! AotCompileOptions / SourceFile
//!   -> CompileFileScheduler
//!   -> ProgramCacheStore lookup
//!   -> Source or ABC input Program compile
//!   -> ProgramRegistry
//!   -> NpmModuleEntryCompiler
//!   -> DepsRelationResolver
//!   -> ProgramOptimizer
//!   -> AotOutputGenerator
//!   -> AbcFile emit
//! ```
//!
//! ## Upstream reference
//! - `es2panda/aot/main.cpp` — top-level `Run()` function
//! - `es2panda/aot/options.cpp` — options parsing
//! - `es2panda/compiler/core/compileQueue.cpp` — compile job scheduler
//! - `es2panda/aot/emitFiles.cpp` — output generation
//! - `es2panda/util/moduleHelpers.cpp` — npm entry compilation
//! - `es2panda/aot/resolveDepsRelation.cpp` — dependency BFS

pub mod abc_input;
pub mod cache_store;
pub mod compile_queue;
pub mod deps;
pub mod npm_entries;
pub mod optimizer;
pub mod options;
pub mod output;
pub mod program_registry;
pub mod runner;
pub mod source_compiler;

pub use options::{
    AotCompileOptions, AotOutput, CompileContextInfo, FilesInfoError, PkgInfo, ScriptExtension,
    ScriptKind, SourceFile, SourceFileId,
};
pub use optimizer::ProgramOptimizer;
pub use output::AotOutputResult;
pub use program_registry::ProgramRegistry;
pub use runner::AotRunner;
pub use source_compiler::SourceProgramCompiler;
