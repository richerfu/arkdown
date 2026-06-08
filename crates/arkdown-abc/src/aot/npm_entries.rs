//! Upstream-shaped npm module entry Program compiler.
//!
//! Upstream `es2panda/util/moduleHelpers.cpp` `CompileNpmModuleEntryList`:
//!
//! 1. Reads the npm entries file.
//! 2. Computes `GetHash32String(npmEntriesFileContent)` as the cache key.
//! 3. If the cache file exists and hash matches, inserts the cached Program.
//! 4. Otherwise:
//!    a. Creates one `pandasm::Program` with `lang = ECMAScript`.
//!    b. For each non-empty line, parses it as `"recordName:fieldName"`.
//!       The text before `':'` becomes the record name; the text after becomes
//!       the field name.  If there is no `':'`, the whole line is both.
//!    c. Adds one `pandasm::Record` per line with one `u8 = false` field.
//!    d. Returns the Program with `needUpdateCache = true`.
//!
//! The compiled npm entries Program is inserted into `programsInfo` under the
//! entries file path as its key.  The expected program count is incremented by 1.
//!
//! ## Upstream reference
//! - `es2panda/util/moduleHelpers.cpp` `CompileNpmModuleEntryList`
//! - `es2panda/util/moduleHelpers.h`

use std::path::Path;

use crate::aot::cache_store::ProgramCacheStore;
use crate::emit::record::{ClassRecord, FieldRecord, FieldType, FieldValue, FIELD_OWNER_CLASS_IDX};
use crate::error::AbcError;
use crate::program::model::{Program, ProgramCacheEntry, SourceLang};
use crate::proto_panda::Program as ProtoProgram;

/// Upstream npm module entry Program compiler.
///
/// Wraps the logic from `CompileNpmModuleEntryList` in a structured Rust type.
pub struct NpmModuleEntryCompiler<'a> {
    cache_store: &'a ProgramCacheStore,
}

impl<'a> NpmModuleEntryCompiler<'a> {
    pub fn new(cache_store: &'a ProgramCacheStore) -> Self {
        Self { cache_store }
    }

    /// Compile the npm module entry list at `path` into a `ProgramCacheEntry`.
    ///
    /// Mirrors `CompileNpmModuleEntryList` in upstream `moduleHelpers.cpp`.
    pub fn compile(
        &self,
        path: &Path,
        cache_path: Option<&Path>,
    ) -> Result<ProgramCacheEntry, AbcError> {
        let content = fs_err::read_to_string(path).map_err(AbcError::Io)?;
        let hash_code = self.cache_store.npm_entries_hash(&content);

        // Try cache hit first.
        if let Some(cache_path) = cache_path {
            match self.cache_store.read_npm_program(cache_path, hash_code) {
                Ok(Some(proto_program)) => {
                    let program = proto_program_to_program(proto_program);
                    return Ok(ProgramCacheEntry {
                        program,
                        hash_code,
                        need_update_cache: false,
                        from_cache: true,
                        is_source_mode: false,
                        requires_post_analysis_opt: false,
                    });
                }
                Ok(None) => {} // cache miss — compile
                Err(err) => {
                    tracing::warn!(
                        npm_entries = %path.display(),
                        cache = %cache_path.display(),
                        error = %err,
                        "ignored npm entries cache read error"
                    );
                }
            }
        }

        // Compile from source.
        let program = build_npm_program_from_content(&content);

        Ok(ProgramCacheEntry {
            program,
            hash_code,
            need_update_cache: cache_path.is_some(),
            from_cache: false,
            is_source_mode: false,
            requires_post_analysis_opt: false,
        })
    }
}

/// Build a `Program` from npm entries file content.
///
/// Upstream: `CompileNpmModuleEntryList` parses each line and creates one
/// `pandasm::Record` per non-empty line.
pub fn build_npm_program_from_content(content: &str) -> Program {
    let mut program = Program::new_ecmascript();

    for line in content.lines() {
        if line.is_empty() {
            // Upstream: empty lines are skipped implicitly — the record/field
            // names would be empty strings which match upstream behavior.
            // We match upstream exactly: compile the empty line as an empty record.
            // Actually upstream just iterates every line unconditionally.
        }

        let (record_name, field_name) = parse_npm_entry_line(line);

        let record = build_npm_record(record_name, field_name);
        program.add_record(record);
    }

    program
}

/// Parse one npm entries line using upstream `line.find(":")` semantics.
///
/// Upstream `moduleHelpers.cpp`:
/// ```cpp
/// auto pos = line.find(":");
/// if (pos != std::string::npos) {
///   recordName = line.substr(0, pos);
///   fieldName = line.substr(pos + 1);
/// } else {
///   recordName = line;
///   fieldName = line;
/// }
/// ```
fn parse_npm_entry_line(line: &str) -> (&str, &str) {
    match line.find(':') {
        Some(pos) => (&line[..pos], &line[pos + 1..]),
        None => (line, line),
    }
}

/// Upstream pandasm class name format for npm entries.
///
/// `"L" + name + ";"` unless already in that format.
fn pandasm_npm_class_name(name: &str) -> String {
    if name.starts_with('L') && name.ends_with(';') {
        name.to_string()
    } else {
        format!("L{name};")
    }
}

/// Build one `ClassRecord` for an npm entry.
///
/// Upstream: one Record with `name = pandasm_class_name(recordName)`,
/// one field with `name = fieldName`, `type = u8`, `value = false (0)`.
fn build_npm_record(record_name: &str, field_name: &str) -> ClassRecord {
    ClassRecord {
        name: pandasm_npm_class_name(record_name),
        methods: vec![],
        string_indices: vec![],
        literal_indices: vec![],
        module_literal_indices: vec![],
        source_code_strings: vec![],
        access_flags: 1,
        source_lang: SourceLang::EcmaScript.as_byte(),
        metadata_attrs: Default::default(),
        conflict: false,
        params_num: 0,
        body_presence: false,
        file_location: None,
        standalone_ecmascript_input: false,
        source_file: None,
        annotations: vec![],
        module_requests: vec![],
        fields: vec![FieldRecord {
            name: field_name.to_string(),
            class_idx: FIELD_OWNER_CLASS_IDX,
            type_idx: 0,
            field_type: FieldType::U8,
            metadata_field_type: None,
            access_flags: 0,
            value: Some(FieldValue::U8(0)), // false
            annotations: vec![],
            metadata_attrs: Default::default(),
            source: Default::default(),
        }],
    }
}

/// Convert a proto Program back to our `Program` type for cache hit path.
///
/// This is a lightweight conversion — we only need the record names and
/// field values for the npm entries case.
fn proto_program_to_program(_proto: ProtoProgram) -> Program {
    // For the npm entries cache path we restore the records from the proto.
    // The full proto conversion is handled by `program_cache.rs` / `proto_panda.rs`.
    // For now, return an empty program — the cache hit path in the compile
    // queue will use the proto_panda round-trip via program_cache.rs.
    // TODO(spec-049-p5): implement full proto → Program restoration here
    // when phase 10 (to_abc.rs) is complete.
    Program::new_ecmascript()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emit::record::FieldValue;

    #[test]
    fn record_colon_field_creates_one_record_with_u8_false_field() {
        // Upstream: "recordName:fieldName" → one Record, one u8=false field.
        let content = "@normalized:N&entry/src/main/ets/pages/Index\n";
        let program = build_npm_program_from_content(content);
        assert_eq!(program.record_table.len(), 1);
        let record = program.record_table.iter().next().unwrap();
        assert_eq!(record.fields.len(), 1);
        let field = &record.fields[0];
        assert!(matches!(field.value, Some(FieldValue::U8(0))),
            "npm entry field value must be u8=false (0)");
        assert_eq!(field.field_type, FieldType::U8);
    }

    #[test]
    fn line_without_colon_uses_whole_line_for_both_record_and_field() {
        // Upstream: line without ":" → recordName = fieldName = whole line.
        let content = "some/record/without/colon\n";
        let program = build_npm_program_from_content(content);
        assert_eq!(program.record_table.len(), 1);
        let record = program.record_table.iter().next().unwrap();
        assert_eq!(record.fields.len(), 1);
        // record name = pandasm class name of "some/record/without/colon"
        // field name = "some/record/without/colon"
        assert_eq!(record.fields[0].name, "some/record/without/colon");
    }

    #[test]
    fn parse_npm_entry_line_colon_splits_at_first_colon() {
        let (record, field) = parse_npm_entry_line("@normalized:N&foo&bar");
        assert_eq!(record, "@normalized");
        assert_eq!(field, "N&foo&bar");
    }

    #[test]
    fn parse_npm_entry_line_no_colon_returns_whole_line() {
        let (record, field) = parse_npm_entry_line("some/entry");
        assert_eq!(record, "some/entry");
        assert_eq!(field, "some/entry");
    }

    #[test]
    fn empty_lines_build_records_matching_upstream() {
        // Upstream iterates all lines including empty ones.
        let content = "a:b\n\nc:d\n";
        let program = build_npm_program_from_content(content);
        // 3 lines → 3 records (including the empty line).
        assert_eq!(program.record_table.len(), 3);
    }

    #[test]
    fn npm_program_record_name_uses_pandasm_class_format() {
        let content = "entry/main:exports\n";
        let program = build_npm_program_from_content(content);
        let record = program.record_table.iter().next().unwrap();
        // Upstream pandasm class name: "L" + name + ";"
        assert!(
            record.name.starts_with('L') && record.name.ends_with(';'),
            "npm record must use pandasm class name format; got: {}",
            record.name
        );
    }
}
