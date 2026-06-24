//! ABC bytecode compilation stage.
//!
//! This crate wraps the SDK `es2abc` command surface. It provides the typed
//! [`Ts2abcPlan`] and [`Ts2abcCompiler`]
//! entry points for the CLI's `BuildPlanBuilder` and `ModulePipeline`.
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
//! | `backend.rs` | `BytecodeBackend` trait, `SdkEs2abcBackend`, `BytecodeBackendRunner` |
//! | `file_info.rs` | `FileInfoManifestWriter`, `FileInfoLine`, `CacheFileManifestWriter` |
//! | `records.rs` | `RecordNameDeriver`, `PackageRecordName`, `WorkerRecordName` |
//! | `module_source.rs` | `ModuleSourceCollector`, `ModuleSourceInputKind`, `TransformFactInput` |
//! | `driver.rs` | legacy `Ts2abcOptions` bridge (migration source; delete after migration) |
//! | `version.rs` | `BytecodeVersionPolicy`, `BytecodeVersion` |
//! | `error.rs` | `Ts2abcError`, `Ts2abcDiagnostic`, `Ts2abcUnsupportedFeature` |

pub mod args;
pub mod backend;
pub mod compile_context;
pub mod compiler;
pub mod driver;
pub mod error;
pub mod file_info;
pub mod module_source;
pub mod plan;
pub mod records;
pub mod version;

// ---------------------------------------------------------------------------
// Narrow re-exports (SPEC-048: no public free functions)
// ---------------------------------------------------------------------------

// args — CLI argument types
pub use args::{Ts2abcCommand, VersionQuery};

// backend — bytecode backend types (SPEC-048 production API)
pub use backend::{
    BytecodeBackend, BytecodeBackendRunner, SdkCommandBuilder, SdkEs2abcBackend,
    Ts2abcDriverMigrationAdapter,
};

// compile_context
pub use compile_context::{CompileContextInfo, PkgInfo};

// compiler
pub use compiler::{
    BytecodeAbcInput, RolldownBytecodeBackendChoice, RolldownBytecodeCompileInput,
    RolldownBytecodeCompiler, Ts2abcCompiler,
};

// error
pub use error::{Ts2abcDiagnostic, Ts2abcError, Ts2abcUnsupportedFeature};

// file_info
pub use file_info::{
    CacheFileManifestWriter, FileInfoColumnPolicy, FileInfoLine, FileInfoManifestWriter,
};

// module_source
pub use module_source::{
    DeclarationOnlyPlan, JsonModulePlan, LazyImportPlan, ModuleSourceCollector,
    ModuleSourceFilePlan, ModuleSourceInputKind, TransformFactInput,
};

// plan — typed plan (SPEC-048 primary API)
pub use plan::{
    BundleJsInput, BytecodeBackendKind, CompileUnitPlan, LoaderMetadataInput, ModuleSourceInput,
    RecordNamePlan, Ts2abcOutputPlan, Ts2abcParitySnapshot, Ts2abcPlan, Ts2abcPlanBuilder,
};

// records
pub use records::{PackageRecordName, RecordNameDeriver, RecordNameInput, WorkerRecordName};

// version — bytecode version policy (SPEC-048: free functions migrated to type)
pub use version::{BytecodeVersion, BytecodeVersionPolicy, DEFAULT_TARGET_API_VERSION};

// ---------------------------------------------------------------------------
// Legacy re-exports (migration sources — DELETE after callers migrate)
// ---------------------------------------------------------------------------

// driver.rs types are still re-exported for backward compatibility during
// Phase 3 migration.  Once all callers use Ts2abcPlan + Ts2abcCompiler,
// remove these re-exports and delete driver.rs.
//
// SPEC-048: compile_to_abc and compile_to_abc_with_backend are PUBLIC FREE
// FUNCTIONS and must NOT be re-exported per SPEC-048 rules.  Callers must
// migrate to:
//   - compile_to_abc → Ts2abcDriverMigrationAdapter::compile_options
//   - compile_to_abc_with_backend → Ts2abcDriverMigrationAdapter::compile_with_backend
pub use driver::{Ts2abcInput, Ts2abcOptions, Ts2abcOutput};
