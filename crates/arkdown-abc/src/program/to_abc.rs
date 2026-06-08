//! Convert a `ProgramRegistry` to a final `AbcFile`.
//!
//! This module implements the `Program → AbcFile` conversion that happens at
//! the end of the AOT compile chain, after all Programs have been compiled,
//! optimized, and had their dependency relations resolved.
//!
//! ## String table merging
//!
//! When source-compiled Programs are built by `CompileFileJob`, each Program
//! carries its own local `StringTable` (from the scratch `AbcFile` used during
//! compilation).  Bytecode string indices in the Program's method records are
//! relative to this local StringTable.
//!
//! When merging Programs into a single output `AbcFile`, the local string
//! indices must be remapped to the global string indices in the output file's
//! `StringTable`.  This remapping is performed by:
//! 1. Re-interning each local string into the output file's StringTable.
//! 2. Building a `local_idx → global_idx` mapping.
//! 3. Patching all method bytecodes using `emit::bytecode::patch_bytecode_index_operands`.
//!
//! ## Upstream reference
//! - `assembler/assembly_emitter.cpp` — `AsmEmitter::EmitPrograms`
//! - `es2panda/aot/main.cpp` — `GenerateProgram`, `EmitMergedAbcJob`

use rustc_hash::FxHashMap;

use crate::aot::program_registry::ProgramRegistry;
use crate::emit::bytecode::instruction_len;
use crate::emit::file::AbcFile;
use crate::emit::record::{ClassRecord, FieldValue};
use crate::emit::strings::StringTable;
use crate::error::AbcError;
use crate::program::model::Program;

// ── ProgramRegistry → AbcFile ─────────────────────────────────────────────────

/// Convert a `ProgramRegistry` into a final `AbcFile` for binary serialization.
///
/// Programs are emitted in upstream key order (lexicographic, matching
/// `std::map<std::string, ProgramCache*>`).  Each Program's records and
/// literal arrays are inserted into the `AbcFile` accumulator, with string
/// table indices remapped from each Program's local table to the global table.
///
/// ## Upstream reference
/// `assembler/assembly_emitter.cpp` `AsmEmitter::EmitPrograms`.
pub fn registry_to_abc_file(registry: &ProgramRegistry) -> Result<AbcFile, AbcError> {
    let mut file = AbcFile::new();

    for (key, _, entry) in registry.iter_in_key_order() {
        let program = &entry.program;
        tracing::debug!(
            program_key = key,
            record_count = program.record_table.len(),
            literal_count = program.literal_array_table.len(),
            "emitting program"
        );

        merge_program_into_file(program, &mut file);
    }

    Ok(file)
}

/// Convert a single `Program` into a standalone `AbcFile`.
///
/// Used by the non-merge output path and `--outputProto` single-program mode.
pub fn program_to_abc_file(program: &Program) -> Result<AbcFile, AbcError> {
    let mut file = AbcFile::new();
    merge_program_into_file(program, &mut file);
    Ok(file)
}

/// Merge a Program's records, literal arrays, and strings into the output AbcFile.
///
/// Handles the critical string table and literal array index remapping needed
/// when combining multiple Programs that each have their own local string tables.
fn merge_program_into_file(program: &Program, file: &mut AbcFile) {
    // If this Program has an empty string table, its records' string indices
    // are already relative to the output file (e.g., for ABC input programs).
    if program.strings.is_empty() {
        // Simple merge: no remapping needed.
        for literal in program.literal_array_table.iter() {
            file.literal_arrays.push(literal.clone());
        }
        for record in program.record_table.iter() {
            file.add_record(record.clone());
        }
        return;
    }

    // Source-compiled Program: remap string and literal indices.

    // 1. Re-intern all local strings into the output file's global string table.
    //    Build string_remap: local_idx → global_idx.
    let string_remap: FxHashMap<u16, u16> = build_string_remap(&program.strings, &mut file.strings);

    // 2. Compute literal array offset (how many literals are already in output file).
    let literal_offset = file.literal_arrays.len() as u16;
    let literal_remap: FxHashMap<u16, u16> = if program.literal_array_table.is_empty() {
        FxHashMap::default()
    } else {
        // Build literal_remap: local_idx → global_idx.
        // The local literal array indices 0..N map to global indices (offset)..(offset+N).
        (0..program.literal_array_table.len() as u16)
            .map(|local_idx| (local_idx, local_idx + literal_offset))
            .collect()
    };

    // 3. Add literal arrays to output file.
    for literal in program.literal_array_table.iter() {
        file.literal_arrays.push(literal.clone());
    }

    // 4. Add records with remapped indices.
    for record in program.record_table.iter() {
        let remapped = remap_record_indices(record, &string_remap, &literal_remap);
        file.add_record(remapped);
    }
}

/// Remap raw string indices in method bytecodes.
///
/// The bytecodes contain raw LOCAL string table indices (relative to the
/// Program's local StringTable).  This function rewrites them to the GLOBAL
/// indices in the output AbcFile's StringTable using the provided mapping.
///
/// ## Opcodes with string operands (per isa.yaml)
///
/// | Opcode | Format        | String operand offset |
/// |--------|---------------|-----------------------|
/// | 0x3e   | op str_id:u16 | bytes 1-2             |
/// | 0x3f   | op imm str:u16| bytes 2-3             |
/// | 0x40   | same          | bytes 2-3             |
/// | 0x42   | same          | bytes 2-3             |
/// | 0x43   | op acc str:u16| bytes 2-3             |
/// | 0x7a   | same          | bytes 2-3             |
/// | 0x8e   | same          | bytes 2-3             |
/// | 0xdb   | same          | bytes 2-3             |
/// | 0xdc   | same          | bytes 2-3             |
/// | 0xfe09 | wide lda.str  | bytes 2-3             |
fn remap_bytecode_string_indices(bytecode: &[u8], string_remap: &FxHashMap<u16, u16>) -> Vec<u8> {
    if string_remap.is_empty() {
        return bytecode.to_vec();
    }

    let mut out = bytecode.to_vec();
    let mut pos = 0;

    while pos < out.len() {
        let Some(opcode) = out.get(pos).copied() else {
            break;
        };

        // Opcodes that carry a u16 string index at offset 1 (after opcode).
        if opcode == 0x3e && pos + 2 < out.len() {
            let old = u16::from_le_bytes([out[pos + 1], out[pos + 2]]);
            if let Some(&new) = string_remap.get(&old) {
                out[pos + 1] = (new & 0xff) as u8;
                out[pos + 2] = (new >> 8) as u8;
            }
            pos += 3;
            continue;
        }

        // Opcodes that carry a u16 string index at offset 2.
        if matches!(opcode, 0x3f | 0x40 | 0x42) && pos + 3 < out.len() {
            let old = u16::from_le_bytes([out[pos + 2], out[pos + 3]]);
            if let Some(&new) = string_remap.get(&old) {
                out[pos + 2] = (new & 0xff) as u8;
                out[pos + 3] = (new >> 8) as u8;
            }
            pos += 4;
            continue;
        }

        if matches!(opcode, 0x43 | 0x7a | 0x8e | 0xdb | 0xdc) && pos + 4 < out.len() {
            let old = u16::from_le_bytes([out[pos + 2], out[pos + 3]]);
            if let Some(&new) = string_remap.get(&old) {
                out[pos + 2] = (new & 0xff) as u8;
                out[pos + 3] = (new >> 8) as u8;
            }
            pos += 5;
            continue;
        }

        // Wide lda.str: prefix 0xfe 0x09, string id at offset 2.
        if opcode == 0xfe && out.get(pos + 1) == Some(&0x09) && pos + 3 < out.len() {
            let old = u16::from_le_bytes([out[pos + 2], out[pos + 3]]);
            if let Some(&new) = string_remap.get(&old) {
                out[pos + 2] = (new & 0xff) as u8;
                out[pos + 3] = (new >> 8) as u8;
            }
            pos += 4;
            continue;
        }

        pos += instruction_len(opcode, out.get(pos + 1).copied()).max(1);
    }

    out
}

/// Build a string index remapping: local_idx → global_idx.
///
/// Re-interns each string from the local StringTable into the global StringTable.
fn build_string_remap(
    local_strings: &StringTable,
    global_strings: &mut StringTable,
) -> FxHashMap<u16, u16> {
    local_strings
        .iter()
        .enumerate()
        .map(|(local_idx, s)| {
            let global_idx = global_strings.intern(s);
            (local_idx as u16, global_idx as u16)
        })
        .collect()
}

/// Apply string and literal index remapping to a ClassRecord.
///
/// Returns a new ClassRecord with all string/literal indices remapped to
/// the output AbcFile's global pools.
fn remap_record_indices(
    record: &ClassRecord,
    string_remap: &FxHashMap<u16, u16>,
    literal_remap: &FxHashMap<u16, u16>,
) -> ClassRecord {
    let mut remapped = record.clone();

    // Remap string_indices (used for the method index section string pool).
    remapped.string_indices = record
        .string_indices
        .iter()
        .map(|&idx| {
            string_remap
                .get(&(idx as u16))
                .map(|&new_idx| new_idx as u32)
                .unwrap_or(idx)
        })
        .collect();

    // Remap literal_indices and module_literal_indices.
    remapped.literal_indices = record
        .literal_indices
        .iter()
        .map(|&idx| remap_literal_idx(idx, literal_remap))
        .collect();
    remapped.module_literal_indices = record
        .module_literal_indices
        .iter()
        .map(|&idx| remap_literal_idx(idx, literal_remap))
        .collect();

    // Remap field values that reference literal arrays.
    for field in &mut remapped.fields {
        if let Some(FieldValue::LiteralArray(idx)) = &field.value {
            let new_idx = remap_literal_idx(*idx, literal_remap);
            field.value = Some(FieldValue::LiteralArray(new_idx));
        }
    }

    // Remap raw string indices in method bytecodes.
    // IMPORTANT: This remaps RAW LOCAL STRING INDICES to GLOBAL string indices.
    // This is different from patch_bytecode_index_operands, which maps raw string
    // indices to METHOD POOL INDICES (that's done later during ABC serialization).
    if !string_remap.is_empty() {
        for method in &mut remapped.methods {
            method.bytecode = remap_bytecode_string_indices(&method.bytecode, string_remap);
        }
    }

    remapped
}

fn remap_literal_idx(idx: usize, literal_remap: &FxHashMap<u16, u16>) -> usize {
    literal_remap
        .get(&(idx as u16))
        .map(|&new_idx| new_idx as usize)
        .unwrap_or(idx)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aot::program_registry::ProgramRegistry;
    use crate::emit::record::ClassRecord;
    use crate::emit::strings::StringTable;
    use crate::program::model::{Program, ProgramCacheEntry};

    fn make_empty_record(name: &str) -> ClassRecord {
        ClassRecord {
            name: name.to_string(),
            methods: vec![],
            fields: vec![],
            string_indices: vec![],
            literal_indices: vec![],
            module_literal_indices: vec![],
            source_code_strings: vec![],
            access_flags: 1,
            source_lang: 0,
            metadata_attrs: Default::default(),
            conflict: false,
            params_num: 0,
            body_presence: false,
            file_location: None,
            standalone_ecmascript_input: false,
            source_file: None,
            annotations: vec![],
            module_requests: vec![],
        }
    }

    fn make_program_with_record(name: &str) -> Program {
        let mut program = Program::new_ecmascript();
        program.add_record(make_empty_record(name));
        program
    }

    fn make_program_with_strings(record_name: &str, strings: &[&str]) -> Program {
        let mut program = Program::new_ecmascript();
        for s in strings {
            program.strings.intern(s);
        }
        program.add_record(make_empty_record(record_name));
        program
    }

    #[test]
    fn registry_emits_records_in_lexicographic_key_order() {
        // Upstream: programs emitted in std::map key order.
        let mut registry = ProgramRegistry::new(3);
        registry
            .insert_source_program("c.ts", ProgramCacheEntry::new_compiled(make_program_with_record("c"), false))
            .unwrap();
        registry
            .insert_source_program("a.ts", ProgramCacheEntry::new_compiled(make_program_with_record("a"), false))
            .unwrap();
        registry
            .insert_source_program("b.ts", ProgramCacheEntry::new_compiled(make_program_with_record("b"), false))
            .unwrap();

        let abc = registry_to_abc_file(&registry).unwrap();
        let record_names: Vec<&str> = abc.records.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(
            record_names,
            ["a", "b", "c"],
            "records must be emitted in key order"
        );
    }

    #[test]
    fn empty_registry_produces_empty_abc_with_magic() {
        let registry = ProgramRegistry::new(0);
        let abc = registry_to_abc_file(&registry).unwrap();
        let bytes = abc.serialize();
        assert_eq!(&bytes[0..8], b"PANDA\x00\x00\x00");
        assert!(abc.records.is_empty());
    }

    #[test]
    fn merged_abc_contains_all_records_from_all_programs() {
        let mut registry = ProgramRegistry::new(2);

        let prog1 = make_program_with_record("entry/main");
        let prog2 = make_program_with_record("entry/utils");

        registry
            .insert_source_program("a.ts", ProgramCacheEntry::new_compiled(prog1, false))
            .unwrap();
        registry
            .insert_source_program("b.ts", ProgramCacheEntry::new_compiled(prog2, false))
            .unwrap();

        let abc = registry_to_abc_file(&registry).unwrap();
        assert_eq!(
            abc.records.len(),
            2,
            "merged ABC must contain all records from all Programs"
        );
    }

    #[test]
    fn strings_from_program_are_merged_into_output_string_table() {
        // Programs with local string tables must have their strings merged.
        let mut registry = ProgramRegistry::new(2);

        let prog1 = make_program_with_strings("record1", &["hello", "world"]);
        let prog2 = make_program_with_strings("record2", &["foo", "bar"]);

        registry
            .insert_source_program("a.ts", ProgramCacheEntry::new_compiled(prog1, false))
            .unwrap();
        registry
            .insert_source_program("b.ts", ProgramCacheEntry::new_compiled(prog2, false))
            .unwrap();

        let abc = registry_to_abc_file(&registry).unwrap();
        // Output file should have all 4 strings interned.
        assert!(abc.strings.id_of("hello").is_some(), "must have 'hello'");
        assert!(abc.strings.id_of("world").is_some(), "must have 'world'");
        assert!(abc.strings.id_of("foo").is_some(), "must have 'foo'");
        assert!(abc.strings.id_of("bar").is_some(), "must have 'bar'");
    }
}
