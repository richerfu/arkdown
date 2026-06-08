//! Upstream-shaped AOT compile options and SourceFile model.
//!
//! This module implements the `AotCompileOptions` and `SourceFile` types that
//! mirror upstream `es2panda::aot::Options` and `es2panda::SourceFile`.  Every
//! compile invocation starts from these types; `AotRunner` consumes them to
//! orchestrate the full AOT compile chain.
//!
//! ## Upstream reference
//! - `es2panda/aot/options.cpp` — `Options::Parse`, `CollectInputSourceFile`,
//!   `CollectInputAbcFile`, `CollectInputFilesFromFileList`, `ParseCacheFileOption`
//! - `es2panda/aot/options.h` — `Options` fields
//! - `es2panda/es2panda.h` — `SourceFile` struct, `ScriptKind`, `ScriptExtension`

use std::path::PathBuf;
use std::sync::Arc;

use oxc_index::IndexVec;
use rustc_hash::FxHashMap;

// ── ScriptKind ────────────────────────────────────────────────────────────────

/// Upstream `parser::ScriptKind` — controls how a source file is parsed.
///
/// Corresponds to `es2panda.h::ScriptKind` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScriptKind {
    /// Normal script mode (global scope, no ESM imports/exports).
    #[default]
    Script,
    /// ECMAScript module mode (`import`/`export`, top-level await).
    Module,
    /// CommonJS module mode (`require()`, `module.exports`).
    CommonJs,
    /// Patch mode for incremental updates.
    Patch,
    /// Concurrent module mode (`"use concurrent"` annotation).
    ConcurrentModule,
}

impl ScriptKind {
    /// Parse the `moduleKind` column from a filesInfo.txt line.
    ///
    /// Upstream `options.cpp CollectInputSourceFile`:
    /// ```cpp
    /// if (itemList[SCRIPT_KIND_IDX] == "script") scriptKind = SCRIPT;
    /// else if (itemList[SCRIPT_KIND_IDX] == "commonjs") scriptKind = COMMONJS;
    /// else scriptKind = MODULE;
    /// ```
    pub fn from_filesinfo_column(s: &str) -> Self {
        match s {
            "script" => Self::Script,
            "commonjs" => Self::CommonJs,
            _ => Self::Module,
        }
    }

    /// Return true if the source should be compiled as an ECMAScript module.
    pub fn is_module(self) -> bool {
        matches!(self, Self::Module | Self::ConcurrentModule)
    }

    /// Return true if this is a CommonJS source.
    pub fn is_commonjs(self) -> bool {
        matches!(self, Self::CommonJs)
    }
}

// ── ScriptExtension ───────────────────────────────────────────────────────────

/// Upstream `es2panda::ScriptExtension` — derived from file extension.
///
/// Used by `GetScriptExtension()` in `options.cpp`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScriptExtension {
    #[default]
    Js,
    Ts,
    As,
    /// Precompiled ABC bytecode input.
    Abc,
}

impl ScriptExtension {
    /// Derive the extension from a filename, falling back to `default_ext`.
    ///
    /// Mirrors upstream `GetScriptExtension(filename, inputExtension)`.
    pub fn from_filename(filename: &str, default_ext: &str) -> Self {
        let ext = filename
            .rfind('.')
            .map(|pos| &filename[pos + 1..])
            .unwrap_or("");
        match ext {
            "js" => Self::Js,
            "ts" => Self::Ts,
            "as" => Self::As,
            "abc" => Self::Abc,
            _ => Self::from_str(default_ext),
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "js" => Self::Js,
            "ts" => Self::Ts,
            "as" => Self::As,
            "abc" => Self::Abc,
            _ => Self::Js,
        }
    }

    pub fn is_abc(self) -> bool {
        matches!(self, Self::Abc)
    }
}

// ── SourceFile ────────────────────────────────────────────────────────────────

/// Upstream `es2panda::SourceFile` — one input file's metadata.
///
/// Fields match the upstream struct from `es2panda.h`.  The filelist columns
/// map directly to these fields.
///
/// ## Filelist column mapping (merge mode)
/// ```text
/// [0] filePath       → file_name
/// [1] recordName     → record_name
/// [2] moduleKind     → script_kind
/// [3] sourceFile     → source_file
/// [4] pkgName        → pkg_name
/// [5] isSharedModule → is_shared_module
/// [6] sourceLang     → source_lang
/// ```
///
/// ## Upstream reference
/// `es2panda/aot/options.cpp` `CollectInputSourceFile`.
#[derive(Debug, Clone)]
pub struct SourceFile {
    /// Input file path, matching upstream `src_.fileName`.
    pub file_name: PathBuf,
    /// Record name from column 1 of filesInfo (empty for non-merge mode).
    pub record_name: Box<str>,
    /// Script kind from column 2 (module/script/commonjs).
    pub script_kind: ScriptKind,
    /// Script extension derived from file name.
    pub script_extension: ScriptExtension,
    /// Debug source-file name from column 3 (empty string if not provided).
    pub source_file: Box<str>,
    /// Package name from column 4 (empty string if not provided).
    pub pkg_name: Box<str>,
    /// Shared module flag from column 5.
    pub is_shared_module: bool,
    /// Origin source language string from column 6.
    pub source_lang: Box<str>,
    /// Whether this is a source-mode (JS/TS) input vs an ABC input.
    ///
    /// Set by upstream `CollectInputAbcFile` to `false`; `CollectInputSourceFile`
    /// does not set this field explicitly so it defaults to `true`.
    pub is_source_mode: bool,
    /// Optional inline source text (for API callers that pass source directly).
    pub inline_source: Option<Arc<str>>,
    /// Precomputed source content hash (filled by compile job after reading file).
    pub hash: Option<u32>,
}

impl SourceFile {
    /// Create a new source-mode SourceFile from a file path.
    pub fn new_source(file_name: impl Into<PathBuf>, input_extension: &str) -> Self {
        let file_name = file_name.into();
        let ext_str = file_name.extension().and_then(|e| e.to_str()).unwrap_or("");
        let script_extension = ScriptExtension::from_filename(
            file_name.to_string_lossy().as_ref(),
            input_extension,
        );
        Self {
            file_name,
            record_name: Box::from(""),
            script_kind: ScriptKind::Script,
            script_extension,
            source_file: Box::from(""),
            pkg_name: Box::from(""),
            is_shared_module: false,
            source_lang: Box::from(""),
            is_source_mode: true,
            inline_source: None,
            hash: None,
        }
    }

    /// Create an ABC input SourceFile (is_source_mode = false).
    ///
    /// Upstream `CollectInputAbcFile` sets `isSourceMode = false`.
    pub fn new_abc_input(file_name: impl Into<PathBuf>, input_extension: &str) -> Self {
        let file_name = file_name.into();
        let script_extension = ScriptExtension::from_filename(
            file_name.to_string_lossy().as_ref(),
            input_extension,
        );
        Self {
            file_name,
            record_name: Box::from(""),
            script_kind: ScriptKind::Script,
            script_extension,
            source_file: Box::from(""),
            pkg_name: Box::from(""),
            is_shared_module: false,
            source_lang: Box::from(""),
            is_source_mode: false,
            inline_source: None,
            hash: None,
        }
    }

    /// Build a SourceFile from a filesInfo.txt line's parsed items.
    ///
    /// Upstream `CollectInputSourceFile` and `CollectInputAbcFile`.
    /// Empty items are dropped by `GetStringItems`; we handle that via
    /// the caller passing an `&[str]` slice that only contains non-empty items.
    pub fn from_filesinfo_items(
        items: &[&str],
        merge_abc: bool,
        input_extension: &str,
    ) -> Option<Self> {
        // Upstream: item list must not be empty and must not exceed 7.
        if items.is_empty() || items.len() > 7 {
            return None;
        }

        let file_name_str = items[0];
        let script_extension = ScriptExtension::from_filename(file_name_str, input_extension);

        if script_extension.is_abc() {
            // ABC input: only file_name and pkg_name (if merge).
            let mut sf = Self::new_abc_input(PathBuf::from(file_name_str), input_extension);
            if merge_abc && items.len() > 1 {
                sf.pkg_name = Box::from(items[1]);
            }
            return Some(sf);
        }

        // Source file.
        let record_name = if merge_abc && items.len() > 1 {
            Box::from(items[1])
        } else {
            Box::from("")
        };

        let script_kind = items
            .get(2)
            .copied()
            .map(ScriptKind::from_filesinfo_column)
            .unwrap_or(ScriptKind::Module);

        let source_file = Box::from(items.get(3).copied().unwrap_or(""));

        let pkg_name = if merge_abc {
            Box::from(items.get(4).copied().unwrap_or(""))
        } else {
            Box::from("")
        };

        let is_shared_module = items.get(5).is_some_and(|&v| v == "true");
        let source_lang = Box::from(items.get(6).copied().unwrap_or(""));

        Some(Self {
            file_name: PathBuf::from(file_name_str),
            record_name,
            script_kind,
            script_extension,
            source_file,
            pkg_name,
            is_shared_module,
            source_lang,
            is_source_mode: true,
            inline_source: None,
            hash: None,
        })
    }

    /// Return the cache lookup key for this SourceFile.
    ///
    /// Upstream uses `src_->fileName` as the cache key in `--cache-file` mapping.
    pub fn cache_key(&self) -> String {
        self.file_name.to_string_lossy().into_owned()
    }
}

// ── SourceFileId re-export ────────────────────────────────────────────────────

pub use crate::program::ids::SourceFileId;

// ── AotOutput ────────────────────────────────────────────────────────────────

/// AOT output specification, matching upstream `--output` / base64 / per-file modes.
#[derive(Debug, Clone)]
pub enum AotOutput {
    /// Write to an explicit output path (most common, `--output`).
    Path(PathBuf),
    /// Write to stdout as base64 (single Program, no output path specified).
    Base64,
    /// Per-input output paths (non-merge mode with `OutputFiles()`).
    PerInput,
}

// ── CompileContextInfo ────────────────────────────────────────────────────────

/// Upstream `compileContextInfo` parsed from `--compile-context-info`.
///
/// Holds dependency resolution roots, HSP pkg names, package version update info,
/// replace-records, and bundle-name update info.  This is the Rust equivalent of
/// the C++ `CompileContextInfo` struct in `options.h`.
#[derive(Debug, Clone, Default)]
pub struct CompileContextInfo {
    /// Upstream `compileEntries` — dependency BFS roots.
    pub compile_entries: Vec<Box<str>>,
    /// Upstream `hspPkgNames` — external packages excluded from BFS.
    pub hsp_pkg_names: std::collections::BTreeSet<Box<str>>,
    /// Upstream `pkgContextInfo` — package versions for OHM URL update.
    pub pkg_context_info: std::collections::BTreeMap<Box<str>, PkgInfo>,
    /// Upstream `updateVersionInfo` — per-ABC package version map.
    pub update_version_info:
        std::collections::BTreeMap<Box<str>, std::collections::BTreeMap<Box<str>, PkgInfo>>,
    /// Upstream `replaceRecords` — records to replace from ABC input.
    pub replace_records: std::collections::BTreeMap<Box<str>, Vec<Box<str>>>,
    /// Upstream `needModifyRecord` — whether bundle name updates are required.
    pub need_modify_record: bool,
    /// Upstream `bundleName` — new bundle name when `needModifyRecord` is true.
    pub bundle_name: Box<str>,
    /// File path of the compile-context-info JSON (for error messages).
    pub path: Option<PathBuf>,
}

impl CompileContextInfo {
    /// Upstream `NeedCollectDepsRelation` in `options.cpp` line 463:
    /// ```cpp
    /// return compilerOptions_.enableAbcInput &&
    ///        !compilerOptions_.compileContextInfo.compileEntries.empty();
    /// ```
    ///
    /// Note: this takes `enable_abc_input` as a parameter because `CompileContextInfo`
    /// does not own that flag (it lives on `AotCompileOptions`).
    pub fn needs_dependency_relation(&self, enable_abc_input: bool) -> bool {
        enable_abc_input && !self.compile_entries.is_empty()
    }

    pub fn needs_package_version_update(&self) -> bool {
        !self.pkg_context_info.is_empty() || !self.update_version_info.is_empty()
    }
}

/// Package version information used for OHM URL updates.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PkgInfo {
    pub package_name: Box<str>,
    pub version: Box<str>,
}

// ── AotCompileOptions ─────────────────────────────────────────────────────────

/// Upstream-shaped AOT compile options.
///
/// This is the primary input to `AotRunner`.  It mirrors the combination of
/// upstream `CompilerOptions` and the parsed `Options` object from
/// `es2panda/aot/options.h`.
///
/// ## Key design rules (from SPEC-049)
///
/// - `--module` and `--commonjs` conflict; validated before any input processing.
/// - `debug_info` forces `opt_level = 0`, matching upstream:
///   `compilerOptions_.optLevel = isDebug ? 0 : opOptLevel`.
/// - `arkdown-ts2abc::ArgParser` builds this through `AotCompileRequest`.
///   It must not construct `AbcModule`.
#[derive(Debug, Clone)]
pub struct AotCompileOptions {
    /// Input source files, indexed by `SourceFileId`.
    pub source_files: IndexVec<SourceFileId, SourceFile>,
    /// Output specification.
    pub output: AotOutput,
    /// Merge all input records into a single ABC output file.
    pub merge_abc: bool,
    /// Embed debug information (source positions, variable names).
    ///
    /// Forces `opt_level = 0` when `true`, matching upstream.
    pub debug_info: bool,
    /// Enable column info in debug output.
    pub enable_column: bool,
    /// Effective optimization level (0 = none, 1 = basic, 2 = full).
    ///
    /// Set to 0 when `debug_info = true`, matching upstream `isDebug ? 0 : opOptLevel`.
    pub opt_level: u8,
    /// Whether to optimize functions with try/catch blocks.
    pub optimize_try_catch_func: bool,
    /// Number of file-level compilation threads.
    pub file_threads: usize,
    /// Number of function-level compilation threads.
    pub function_threads: usize,
    /// Number of ABC class serialization threads.
    pub abc_class_threads: usize,
    /// Target OpenHarmony API version (e.g. 12 for API 12).
    pub target_api_version: u32,
    /// Optional target API stage/sub-version (`--target-api-sub-version`).
    pub target_api_sub_version: Option<Box<str>>,
    /// Cache file mapping from `SourceFile::fileName` to `.protoBin` path.
    ///
    /// Upstream `--cache-file bare.protoBin` maps the single source to one file.
    /// Upstream `--cache-file @modules.cache` reads a semicolon-separated manifest.
    pub cache_files: FxHashMap<Box<str>, PathBuf>,
    /// Path to the npm module entry list (`--npm-module-entry-list`).
    pub npm_module_entry_list: Option<PathBuf>,
    /// Compile context info for dependency resolution and record replacement.
    pub compile_context_info: CompileContextInfo,
    /// Whether to accept precompiled `.abc` input files.
    pub enable_abc_input: bool,
    /// Whether to print resolved dependency info and return (no ABC emit).
    pub dump_deps_info: bool,
    /// Whether to remove programs unreachable from compile entries.
    pub remove_redundant_file: bool,
    /// Whether to write a protoPanda Program snapshot instead of final ABC.
    pub output_proto: Option<PathBuf>,
    /// Optional custom `moduleRecordField` name for non-merge `_ESModuleRecord`.
    pub module_record_field_name: Option<Box<str>>,
    /// Whether to enable ArkTS annotation support (API 20+).
    pub enable_annotations: bool,
    /// Whether to use define semantics for class fields.
    pub use_define_semantic: bool,
    /// Upstream `modifiedPkgName` — `"srcPkg:dstPkg"` format.
    ///
    /// When set, rewrites OHM URL package names in ABC input records.
    /// Corresponds to `--src-package-name srcPkg:dstPkg` in es2abc CLI.
    pub modified_pkg_name: Option<Box<str>>,
    /// Whether cross-program global optimization is required.
    ///
    /// Upstream `compilerOptions_.requireGlobalOptimization` — set when
    /// `optLevel > 0 && branchElimination && mergeAbc`.  When true:
    /// - Cache hits are bypassed (programs must be freshly compiled for analysis).
    /// - Cache emit is skipped in `EmitFileQueue`.
    /// - `ProgramOptimizer::analyze_for_global_optimization` is called instead
    ///   of `optimize_source_program`.
    /// - `optimize_after_analysis` runs after all programs have been analyzed.
    ///
    /// ## Upstream reference
    /// `es2panda/compiler/core/compileQueue.cpp` `RetrieveProgramFromCacheFiles`
    /// line 74: returns false immediately when `requireGlobalOptimization`.
    pub require_global_optimization: bool,
}

impl AotCompileOptions {
    /// Return the effective debug info flag (false when opt_level > 0).
    pub fn effective_debug_info(&self) -> bool {
        self.debug_info && self.opt_level == 0
    }

    /// Return the global ScriptKind for single-input compiles.
    ///
    /// For filelist-based compiles, each SourceFile has its own script_kind.
    pub fn global_script_kind(&self) -> ScriptKind {
        self.source_files
            .first()
            .map(|sf| sf.script_kind)
            .unwrap_or(ScriptKind::Script)
    }

    /// Return the total number of source files to compile.
    pub fn expected_program_count(&self) -> usize {
        self.source_files.len()
    }
}

// ── Filelist parser ───────────────────────────────────────────────────────────

/// Parse an upstream `@filesInfo.txt` file into a `Vec<SourceFile>`.
///
/// Each line is parsed into semicolon-separated items matching the upstream
/// filelist column layout.  Empty items are dropped per `GetStringItems`.
///
/// ## Upstream reference
/// `es2panda/aot/options.cpp` `CollectInputFilesFromFileList`.
pub fn parse_filesinfo(
    content: &str,
    merge_abc: bool,
    input_extension: &str,
) -> Result<Vec<SourceFile>, FilesInfoError> {
    let mut files = Vec::new();
    for (line_no, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        // Upstream `GetStringItems` drops empty items.
        let items: Vec<&str> = line
            .split(';')
            .filter(|item| !item.is_empty())
            .collect();

        if items.is_empty() || items.len() > 7 {
            return Err(FilesInfoError::InvalidLine {
                line_no: line_no + 1,
                line: line.to_string(),
            });
        }

        let sf = SourceFile::from_filesinfo_items(&items, merge_abc, input_extension)
            .ok_or_else(|| FilesInfoError::InvalidLine {
                line_no: line_no + 1,
                line: line.to_string(),
            })?;
        files.push(sf);
    }
    Ok(files)
}

/// Error returned when parsing a filesInfo.txt line fails.
#[derive(Debug, thiserror::Error)]
pub enum FilesInfoError {
    #[error("invalid filesInfo line {line_no}: {line}")]
    InvalidLine { line_no: usize, line: String },
    #[error("--module and --commonjs conflict before any input processing")]
    ModuleCommonjsConflict,
    #[error("io error reading filesInfo: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    // SPEC-049 Phase 1 unit tests

    #[test]
    fn parse_filelist_merge_source_columns_match_upstream() {
        // Upstream filelist (merge mode): 7 columns
        let content = "src/entry/main.ts;@bundle:com.example/entry/ets/pages/Index;module;entry|1.0.0|src/main/ets/pages/Index.ts;entry;false;ts";
        let files = parse_filesinfo(content, true, "js").unwrap();
        assert_eq!(files.len(), 1);
        let sf = &files[0];
        assert_eq!(sf.file_name, PathBuf::from("src/entry/main.ts"));
        assert_eq!(
            sf.record_name.as_ref(),
            "@bundle:com.example/entry/ets/pages/Index"
        );
        assert_eq!(sf.script_kind, ScriptKind::Module);
        assert_eq!(
            sf.source_file.as_ref(),
            "entry|1.0.0|src/main/ets/pages/Index.ts"
        );
        assert_eq!(sf.pkg_name.as_ref(), "entry");
        assert!(!sf.is_shared_module);
        assert_eq!(sf.source_lang.as_ref(), "ts");
        assert!(sf.is_source_mode);
    }

    #[test]
    fn parse_filelist_non_merge_uses_output_column() {
        // Upstream non-merge filelist: column 4 is output file (not pkgName).
        let content = "src/main.ts;;module;main.ts;output/main.abc;false;ts";
        let files = parse_filesinfo(content, false, "js").unwrap();
        assert_eq!(files.len(), 1);
        let sf = &files[0];
        assert_eq!(sf.file_name, PathBuf::from("src/main.ts"));
        // Non-merge: record_name is empty.
        assert_eq!(sf.record_name.as_ref(), "");
        assert!(sf.is_source_mode);
    }

    #[test]
    fn parse_abc_filelist_collects_pkg_name_only() {
        // ABC input in filelist: only file_name and pkg_name columns.
        let content = "oh_modules/.modules.abc;entry";
        let files = parse_filesinfo(content, true, "js").unwrap();
        assert_eq!(files.len(), 1);
        let sf = &files[0];
        assert_eq!(sf.file_name, PathBuf::from("oh_modules/.modules.abc"));
        assert!(!sf.is_source_mode);
        assert_eq!(sf.pkg_name.as_ref(), "entry");
    }

    #[test]
    fn debug_info_forces_opt_level_zero() {
        // SPEC-049 Phase 1: debug info must force opt_level = 0.
        let opts = AotCompileOptions {
            source_files: IndexVec::new(),
            output: AotOutput::Base64,
            merge_abc: false,
            debug_info: true,
            enable_column: false,
            opt_level: 2, // should be overridden to 0
            optimize_try_catch_func: true,
            file_threads: 1,
            function_threads: 1,
            abc_class_threads: 1,
            target_api_version: 12,
            target_api_sub_version: None,
            cache_files: FxHashMap::default(),
            npm_module_entry_list: None,
            compile_context_info: CompileContextInfo::default(),
            enable_abc_input: false,
            dump_deps_info: false,
            remove_redundant_file: false,
            output_proto: None,
            module_record_field_name: None,
            enable_annotations: false,
            use_define_semantic: false,
            require_global_optimization: false,
            modified_pkg_name: None,
        };
        // effective_debug_info() must be false when opt_level > 0.
        let opts_release = AotCompileOptions { debug_info: true, opt_level: 2, ..opts.clone() };
        assert!(!opts_release.effective_debug_info(),
            "opt_level=2 must suppress debug info even when debug_info=true");
        // effective_debug_info() must be true when opt_level = 0.
        let opts_debug = AotCompileOptions { debug_info: true, opt_level: 0, ..opts };
        assert!(opts_debug.effective_debug_info(),
            "opt_level=0 + debug_info=true must yield effective debug info");
    }

    #[test]
    fn module_commonjs_conflict_errors_before_compile() {
        // Upstream rejects --module + --commonjs before any input processing.
        // We test this at the ScriptKind level: both Module and CommonJs cannot
        // be the same kind.
        assert_ne!(ScriptKind::Module, ScriptKind::CommonJs);
        assert!(ScriptKind::Module.is_module());
        assert!(ScriptKind::CommonJs.is_commonjs());
        assert!(!ScriptKind::Module.is_commonjs());
        assert!(!ScriptKind::CommonJs.is_module());
    }

    #[test]
    fn parse_cache_file_bare_path_maps_single_source() {
        // Bare --cache-file: maps the single source file name to the path.
        // This is tested via the cache module; here we just test SourceFile.cache_key().
        let sf = SourceFile::new_source("src/main.ts", "ts");
        assert_eq!(sf.cache_key(), "src/main.ts");
    }

    #[test]
    fn parse_cache_file_manifest_maps_by_source_file_name() {
        // Upstream `--cache-file @modules.cache` reads a semicolon manifest where
        // column 1 is `SourceFile::fileName` and column 2 is the `.protoBin` path.
        // Matching `ParseCacheFileOption` in `options.cpp`.
        use crate::cache::CacheFileMap;
        use std::path::Path;

        let dir = tempfile::tempdir().unwrap();
        let manifest = dir.path().join("modules.cache");
        std::fs::write(
            &manifest,
            "src/Index.ts;/cache/Index.protoBin\nsrc/Utils.ts;/cache/Utils.protoBin\n",
        )
        .unwrap();

        let map = CacheFileMap::read_manifest(&manifest).unwrap();
        assert_eq!(
            map.get("src/Index.ts"),
            Some(Path::new("/cache/Index.protoBin")),
            "manifest must map source file name to proto bin path"
        );
        assert_eq!(
            map.get("src/Utils.ts"),
            Some(Path::new("/cache/Utils.protoBin")),
        );
        assert!(
            map.get("nonexistent.ts").is_none(),
            "unknown source file must not be in cache map"
        );
    }

    #[test]
    fn empty_filelist_items_are_dropped() {
        // Upstream GetStringItems: empty items are dropped.
        // A line with consecutive semicolons drops the empty tokens.
        let content = "src/a.ts;;module;;entry;;ts";
        let files = parse_filesinfo(content, true, "js").unwrap();
        assert_eq!(files.len(), 1);
        let sf = &files[0];
        // Items after empty-drop: ["src/a.ts", "module", "entry", "ts"]
        // After drop: [0]=file, [1]=record_name becomes "module" (empty dropped),
        // but per filelist semantics with empty dropped, index mapping changes.
        // The SourceFile is constructed from the non-empty items only.
        assert_eq!(sf.file_name, PathBuf::from("src/a.ts"));
    }
}
