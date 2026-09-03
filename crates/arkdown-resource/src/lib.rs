//! `arkdown-resource` — resource compilation planning and SDK `restool` backend.
//!
//! hvigor parity requires the SDK `restool` backend.  Production builds always
//! use [`ResourceBackendRunner::default_sdk`] with the SDK-provided `restool`
//! binary.
//!
//! # Production usage
//!
//! ```rust,no_run
//! use arkdown_resource::{ResourceBackendRunner, ResourceCompileConfig, ResourceCompileInput};
//! use std::path::PathBuf;
//!
//! let runner = ResourceBackendRunner::default_sdk(PathBuf::from("/sdk/restool"));
//! let output = runner.compile(
//!     &ResourceCompileConfig {
//!         package_name: "com.example.myapp".into(),
//!         module_name: "entry".into(),
//!         module_names: vec![],
//!         device_types: vec!["phone".into()],
//!         is_debug: false,
//!     },
//!     ResourceCompileInput {
//!         resources_dirs: vec!["src/main/resources".into()],
//!         output_dir: "build/generated/resources".into(),
//!     },
//! ).expect("resource compilation failed");
//! ```

pub mod backend;
pub mod error;
pub mod id_defined;
pub mod plan;

mod model;
mod reference_inventory;
mod runtime_key;

// Re-export the primary public API at crate root.
pub use backend::{ResourceBackend, ResourceBackendRunner, SdkRestoolBackend};
pub use error::ResourceError;
pub use id_defined::IdDefinedFileProcessor;
pub use model::{ResourceCompileConfig, ResourceCompileInput, ResourceCompileOutput, ResourceInfo};
pub use plan::{
    ResourceCompileOptions, ResourceCompilePlan, ResourceDependencyInput, ResourceInputRoot,
    ResourceOutputPaths, RestoolConfigInput, RestoolHarEntry,
};
pub use reference_inventory::ResourceReferenceInventory;
