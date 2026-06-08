//! Rust equivalent of `pandasm::Program`.
//!
//! This module defines the intermediate representation that sits between source
//! compilation and final `AbcFile` binary serialization.  Every compile path —
//! source, ABC input, npm entries — must produce a `Program` before final ABC
//! emission.  Direct source-to-`AbcFile` paths are forbidden per SPEC-049.
//!
//! ## Design
//!
//! Rather than duplicating all field definitions from the existing `ClassRecord`
//! and `MethodRecord` types, `Program` wraps them directly.  The existing types
//! already implement the full upstream `pandasm::Record` and `pandasm::Function`
//! field sets.  `program/to_abc.rs` converts a `ProgramRegistry` to `AbcFile`
//! by inserting each Program's records into the file, exactly matching upstream
//! `AsmEmitter::EmitPrograms` behavior.
//!
//! ## Upstream reference
//! - `assembler/assembly_program.h` — `pandasm::Program`
//! - `es2panda/aot/main.cpp` — `programsInfo` insertion/key semantics
//! - `es2panda/compiler/core/emitter/emitter.cpp` — `EmitPrograms` order

use oxc_index::IndexVec;

use crate::emit::file::LiteralArray;
use crate::emit::record::ClassRecord;
use crate::emit::strings::StringTable;
use crate::program::ids::{LiteralArrayId, RecordId};

// ── Source language ────────────────────────────────────────────────────────────

/// ArkTS/es2abc source language byte stored in pandasm Program and Record.
///
/// Matches upstream `panda::panda_file::SourceLang` enum values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SourceLang {
    #[default]
    EcmaScript = 0,
    Panda = 1,
}

impl SourceLang {
    /// Map an `origin_source_lang` string from filesInfo column 7 to a SourceLang
    /// byte, matching upstream `Program::SetSourceLang()` behavior.
    pub fn from_origin_str(s: &str) -> Self {
        match s {
            "ets" | "ts" | "js" => Self::EcmaScript,
            "panda" => Self::Panda,
            _ => Self::EcmaScript,
        }
    }

    pub fn as_byte(self) -> u8 {
        self as u8
    }
}

// ── Program metadata ──────────────────────────────────────────────────────────

/// Upstream `pandasm::Program` metadata fields beyond the record/function tables.
///
/// These flags affect how the Program is merged into the final `AbcFile`.
#[derive(Debug, Clone, Default)]
pub struct ProgramMetadata {
    /// Source mode flag from `SourceFile::isSourceMode`.
    ///
    /// `true` for normal JS/TS/ETS source inputs; `false` for ABC inputs.
    pub is_source_mode: bool,
    /// Set when the Program was decoded from a merged ABC input file.
    ///
    /// Corresponds to upstream `Program::SetGeneratedFromMergedAbc(true)`.
    pub is_generated_from_merged_abc: bool,
    /// Shared module flag from `SourceFile::isSharedModule`.
    pub is_shared_module: bool,
    /// Origin source language for this program.
    pub source_lang: SourceLang,
}

// ── Program ───────────────────────────────────────────────────────────────────

/// Rust equivalent of `pandasm::Program`.
///
/// This is the mandatory intermediate representation between source compilation
/// and final `AbcFile` binary serialization.  Both source compiles and ABC input
/// decodes must produce a `Program` value.  The `AotOutputGenerator` converts a
/// `ProgramRegistry` (collection of Programs) into a final `AbcFile`.
///
/// ## Record/function model
///
/// The existing `ClassRecord` type implements the full upstream `pandasm::Record`
/// field set (records, methods, fields, literal arrays, debug info, annotations).
/// `Program.record_table` stores them by typed `RecordId`.  Functions (methods)
/// live inside their owning `ClassRecord.methods` — they are not indexed globally
/// with a `FunctionId` because the existing emit pipeline resolves them through
/// the record → method tree.  `FunctionId` is retained as a type for future
/// cross-record function reference use cases.
///
/// ## Upstream reference
/// - `assembler/assembly_program.h` — `pandasm::Program::record_table`,
///   `pandasm::Program::function_table`, `pandasm::Program::literalarray_table`.
/// - `es2panda/aot/main.cpp` — `programsInfo` stores one Program per key.
#[derive(Debug, Clone, Default)]
pub struct Program {
    /// Language of this Program (EcmaScript or Panda).
    pub lang: SourceLang,
    /// All records in this Program, indexed by `RecordId`.
    ///
    /// Corresponds to upstream `pandasm::Program::record_table`.  Insertion order
    /// is preserved; the final ABC emitter serializes records in this order.
    pub record_table: IndexVec<RecordId, ClassRecord>,
    /// All literal arrays in this Program.
    ///
    /// Corresponds to upstream `pandasm::Program::literalarray_table`.
    pub literal_array_table: IndexVec<LiteralArrayId, LiteralArray>,
    /// String table for this Program.
    ///
    /// Carries the local StringTable built during source compilation so that
    /// `program_to_abc_file` can re-intern strings into the output AbcFile's
    /// StringTable and remap bytecode string indices correctly.
    ///
    /// For Programs built from ABC input (via abc2program), this is empty since
    /// string indices are correctly relative to the parsed ABC file.
    pub strings: StringTable,
    /// Program-level metadata (source mode, merged-abc flag, shared flag).
    pub metadata: ProgramMetadata,
}

impl Program {
    /// Create an empty Program with EcmaScript source language.
    pub fn new_ecmascript() -> Self {
        Self {
            lang: SourceLang::EcmaScript,
            ..Self::default()
        }
    }

    /// Add a record to the Program and return its typed ID.
    pub fn add_record(&mut self, record: ClassRecord) -> RecordId {
        self.record_table.push(record)
    }

    /// Add a literal array to the Program and return its typed ID.
    pub fn add_literal_array(&mut self, array: LiteralArray) -> LiteralArrayId {
        self.literal_array_table.push(array)
    }

    /// Look up a record by pandasm name.
    pub fn find_record(&self, name: &str) -> Option<(RecordId, &ClassRecord)> {
        self.record_table
            .iter_enumerated()
            .find(|(_, r)| r.name == name)
    }

    /// Return the number of records in this Program.
    pub fn record_count(&self) -> usize {
        self.record_table.len()
    }

    /// Return the number of literal arrays in this Program.
    pub fn literal_array_count(&self) -> usize {
        self.literal_array_table.len()
    }
}

// ── ProgramCacheEntry ─────────────────────────────────────────────────────────

/// A compiled Program together with its cache state.
///
/// Equivalent to upstream `util::ProgramCache` (which stores `pandasm::Program`
/// plus cache-write metadata flagging).
#[derive(Debug, Clone)]
pub struct ProgramCacheEntry {
    /// The compiled Program.
    pub program: Program,
    /// Cache hash code for this program (0 if no cache is requested).
    pub hash_code: u32,
    /// Whether upstream would write this program to a `.protoBin` cache file.
    ///
    /// `false` for cache hits (no re-write needed) and ABC input programs where
    /// the cache write happens immediately after compile in `build_abc_input_file`.
    pub need_update_cache: bool,
    /// Whether this program was restored from a cache hit.
    pub from_cache: bool,
    /// Whether this program was compiled from a source file (vs ABC input).
    pub is_source_mode: bool,
    /// Whether global optimization analysis needs to run on this program.
    ///
    /// Set by upstream `CompileFileJob::Run` → `OptimizeAndCacheProgram` when the
    /// optimizer flags a program for global analysis.
    pub requires_post_analysis_opt: bool,
}

impl ProgramCacheEntry {
    /// Create a new entry for a freshly compiled source program.
    pub fn new_compiled(program: Program, need_update_cache: bool) -> Self {
        Self {
            program,
            hash_code: 0,
            need_update_cache,
            from_cache: false,
            is_source_mode: true,
            requires_post_analysis_opt: false,
        }
    }

    /// Create a new entry for a cache hit (no re-compile needed).
    pub fn new_cache_hit(program: Program, hash_code: u32) -> Self {
        Self {
            program,
            hash_code,
            need_update_cache: false,
            from_cache: true,
            is_source_mode: true,
            requires_post_analysis_opt: false,
        }
    }

    /// Create a new entry for an ABC input program.
    pub fn new_abc_input(program: Program) -> Self {
        Self {
            program,
            hash_code: 0,
            need_update_cache: false,
            from_cache: false,
            is_source_mode: false,
            requires_post_analysis_opt: false,
        }
    }
}
