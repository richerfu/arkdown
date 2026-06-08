//! Program-level intermediate representation for the upstream AOT chain.
//!
//! Every compile path (source, ABC input, npm entries) must produce a `Program`
//! before final `AbcFile` emission.  The `ProgramRegistry` stores all programs
//! compiled in one run and provides the deterministic ordering required by
//! upstream `programsInfo` (`std::map<std::string, ProgramCache*>`).
//!
//! ## Upstream reference
//! - `assembler/assembly_program.h` — `pandasm::Program`
//! - `es2panda/aot/main.cpp` — `programsInfo`

pub mod dump;
pub mod ids;
pub mod model;
pub mod to_abc;

pub use ids::{FunctionId, LiteralArrayId, ProgramId, RecordId};
pub use model::{Program, ProgramCacheEntry, ProgramMetadata, SourceLang};
