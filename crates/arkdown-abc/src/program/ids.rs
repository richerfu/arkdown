//! Typed index types for the upstream Program model.
//!
//! Upstream es2abc stores all Program-level data in `std::map<std::string, …>` tables.
//! We use typed integer IDs backed by `oxc_index::IndexVec` for zero-cost typed access
//! and deterministic ordering.
//!
//! ## Upstream reference
//! - `es2panda/aot/main.cpp`: `programsInfo` is `std::map<std::string, ProgramCache*>`,
//!   iterated in lexicographic key order for deterministic output.

use oxc_index::define_index_type;

define_index_type! {
    /// Identifies one `SourceFile` in the `AotCompileOptions::source_files` table.
    ///
    /// Used by `CompileFileScheduler` to associate compile jobs with their inputs.
    pub struct SourceFileId = u32;
    DEFAULT = SourceFileId(0);
}

define_index_type! {
    /// Identifies one `pandasm::Program` within a compile run.
    ///
    /// Programs are keyed by `SourceFile::fileName` for source inputs,
    /// `abcFileName|recordName` for ABC inputs, and the npm entries list path
    /// for the npm entry program.
    pub struct ProgramId = u32;
    DEFAULT = ProgramId(0);
}

define_index_type! {
    /// Identifies one `pandasm::Record` within a `Program`.
    pub struct RecordId = u32;
    DEFAULT = RecordId(0);
}

define_index_type! {
    /// Identifies one `pandasm::Function` within a `Program`.
    pub struct FunctionId = u32;
    DEFAULT = FunctionId(0);
}

define_index_type! {
    /// Identifies one `pandasm::LiteralArray` within a `Program`.
    pub struct LiteralArrayId = u32;
    DEFAULT = LiteralArrayId(0);
}
