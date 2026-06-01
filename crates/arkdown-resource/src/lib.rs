//! `arkdown-resource` — pure-Rust port of OpenHarmony's `restool` resource compiler.
//!
//! Transforms `src/main/resources/` into a `resources.index` binary and a
//! `ResourceTable.txt` text file, and copies media and profile files into the
//! output directory.
//!
//! # Usage
//!
//! ```rust,no_run
//! use arkdown_resource::{
//!     ResourceCompiler, ResourceCompileConfig, ResourceCompileInput,
//! };
//!
//! let compiler = ResourceCompiler::new(ResourceCompileConfig {
//!     package_name: "com.example.myapp".into(),
//!     module_name: "entry".into(),
//!     device_types: vec!["phone".into()],
//!     is_debug: false,
//! });
//!
//! let _output = compiler.compile(ResourceCompileInput {
//!     resources_dirs: vec!["src/main/resources".into()],
//!     output_dir: "build/generated/resources".into(),
//! }).expect("resource compilation failed");
//! ```

pub mod compiler;
pub mod element;
pub mod error;
pub mod index;
pub mod media;
pub mod profile;
pub mod scan;
pub mod table;

// Re-export the primary public API at crate root.
pub use compiler::{
    CompiledEntry, ResourceCompileConfig, ResourceCompileInput, ResourceCompileOutput,
    ResourceCompiler, ResourceInfo,
};
pub use error::ResourceError;
