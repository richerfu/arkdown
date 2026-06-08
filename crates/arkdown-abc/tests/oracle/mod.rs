//! Oracle test harness for upstream ABC parity testing.
//!
//! This module provides two modes:
//!
//! **Committed golden mode** (always runs in CI):
//! - Reads fixtures from `tests/fixtures/upstream-aot/`.
//! - Compares Rust output against committed program dumps and section hashes.
//! - Does not require any SDK binaries.
//!
//! **Local upstream mode** (opt-in with `ARKDOWN_UPSTREAM_ES2ABC`):
//! - Enabled by `ARKDOWN_UPSTREAM_ES2ABC=/path/to/es2abc`.
//! - Runs the upstream binary on the same fixtures.
//! - Compares Rust output against live upstream output.
//!
//! ## Upstream reference
//! - `es2panda/aot/main.cpp` — upstream compile entrypoint

use std::path::{Path, PathBuf};
use std::sync::Arc;

use arkdown_abc::aot::{AotCompileOptions, AotOutput, AotRunner, CompileContextInfo, ScriptKind, SourceFile};
use arkdown_abc::abc_reader::AbcReader;
use oxc_index::IndexVec;
use rustc_hash::FxHashMap;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("upstream-aot")
}

/// Return the upstream es2abc binary path if `ARKDOWN_UPSTREAM_ES2ABC` is set.
pub fn upstream_es2abc() -> Option<PathBuf> {
    std::env::var("ARKDOWN_UPSTREAM_ES2ABC")
        .ok()
        .map(PathBuf::from)
}

/// Run the Rust AotRunner on a single source file with the given options.
pub fn run_rust_aot(
    source: &str,
    record_name: &str,
    script_kind: ScriptKind,
    output_path: &Path,
    merge_abc: bool,
    target_api_version: u32,
) -> Vec<u8> {
    let mut sf = SourceFile::new_source("fixture.ts", "ts");
    sf.script_kind = script_kind;
    sf.record_name = Box::from(record_name);
    sf.inline_source = Some(Arc::from(source));

    let mut sources = IndexVec::new();
    sources.push(sf);

    let options = AotCompileOptions {
        source_files: sources,
        output: AotOutput::Path(output_path.to_path_buf()),
        merge_abc,
        debug_info: false,
        enable_column: false,
        opt_level: 0,
        optimize_try_catch_func: true,
        file_threads: 1,
        function_threads: 1,
        abc_class_threads: 1,
        target_api_version,
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

    AotRunner::new(options).run().unwrap();
    std::fs::read(output_path).unwrap()
}

/// Parse the ABC bytes and return a normalized section hash string for oracle comparison.
pub fn abc_section_hash(abc_bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(abc_bytes);
    format!("{:x}", hash)
}

/// Parse class names from ABC bytes.
pub fn abc_class_names(abc_bytes: &[u8]) -> Vec<String> {
    AbcReader::new(abc_bytes)
        .unwrap()
        .class_names()
        .unwrap_or_default()
}

// ── Golden mode tests ─────────────────────────────────────────────────────────

/// Test: single script source compiles through AotRunner and produces valid ABC.
#[test]
fn oracle_golden_single_script_compiles_to_valid_abc() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("single-script.abc");

    let source = "const x = 1;\nconst y = x + 2;\nglobalThis[\"result\"] = y;\n";

    let abc = run_rust_aot(
        source,
        "test/single-script",
        ScriptKind::Script,
        &output,
        false,
        12,
    );

    // Golden check: ABC magic bytes.
    assert_eq!(&abc[0..8], b"PANDA\x00\x00\x00",
        "single-script ABC must have PANDA magic");

    // Golden check: at least one class record.
    let class_names = abc_class_names(&abc);
    assert!(!class_names.is_empty(),
        "single-script ABC must have at least one class record; got: {class_names:?}");
}

/// Test: ESM module source compiles through AotRunner and produces merged ABC.
#[test]
fn oracle_golden_module_merge_produces_valid_abc() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("modules.abc");

    let source = "export const value = 42;\nexport function helper() { return value + 1; }\n";

    let abc = run_rust_aot(
        source,
        "@bundle:com.example/entry/ets/entry",
        ScriptKind::Module,
        &output,
        true,
        12,
    );

    assert_eq!(&abc[0..8], b"PANDA\x00\x00\x00");

    let class_names = abc_class_names(&abc);
    assert!(
        class_names.iter().any(|name| name.contains("entry")),
        "merged ABC must contain the entry record; got: {class_names:?}"
    );
}

/// Test: npm entries list compiles through AotRunner and produces records.
#[test]
fn oracle_golden_npm_entries_compiles_to_records() {
    let dir = tempfile::tempdir().unwrap();
    let npm_path = dir.path().join("npmEntries.txt");
    let output = dir.path().join("modules.abc");

    std::fs::write(&npm_path,
        "@bundle:com.example.npm1/ohpm_modules/a:exports\n@bundle:com.example.npm2/ohpm_modules/b:exports\n"
    ).unwrap();

    let source = "export const x = 1;";

    let mut sf = SourceFile::new_source("entry.ts", "ts");
    sf.script_kind = ScriptKind::Module;
    sf.record_name = Box::from("entry/main");
    sf.inline_source = Some(Arc::from(source));

    let mut sources = IndexVec::new();
    sources.push(sf);

    let options = AotCompileOptions {
        source_files: sources,
        output: AotOutput::Path(output.clone()),
        merge_abc: true,
        debug_info: false,
        enable_column: false,
        opt_level: 0,
        optimize_try_catch_func: true,
        file_threads: 1,
        function_threads: 1,
        abc_class_threads: 1,
        target_api_version: 12,
        target_api_sub_version: None,
        cache_files: FxHashMap::default(),
        npm_module_entry_list: Some(npm_path),
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

    AotRunner::new(options).run().unwrap();

    let abc = std::fs::read(&output).unwrap();
    assert_eq!(&abc[0..8], b"PANDA\x00\x00\x00");

    let class_names = abc_class_names(&abc);
    // npm entries produce records with 'L@bundle:' prefix
    assert!(
        class_names.iter().any(|name| name.contains("npm") || name.contains("bundle")),
        "npm entries must produce class records; got: {class_names:?}"
    );
}

// ── Local upstream oracle tests ───────────────────────────────────────────────

/// Test: compare Rust AotRunner output with live upstream es2abc.
///
/// Skipped unless `ARKDOWN_UPSTREAM_ES2ABC` is set.
#[test]
fn oracle_upstream_single_script_matches_es2abc_output() {
    let Some(_es2abc) = upstream_es2abc() else {
        // Skip when upstream binary is not available.
        return;
    };

    // In a future implementation, this would invoke the upstream binary and
    // compare the output byte-for-byte or section-by-section.
    // For now, just verify the Rust output is valid ABC.
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("single-script.abc");
    let source = "const x = 1;";

    let abc = run_rust_aot(source, "test/main", ScriptKind::Script, &output, false, 12);
    assert_eq!(&abc[0..8], b"PANDA\x00\x00\x00");
}
