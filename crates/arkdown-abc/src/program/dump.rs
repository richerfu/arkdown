//! Normalized Program dump for oracle tests.
//!
//! This module provides a human-readable, normalized text representation of a
//! compiled `Program`.  The dump format is designed to be stable across
//! compilation runs and comparable against upstream `--dump-normalized-asm-program`
//! output from `es2abc`.
//!
//! ## Upstream reference
//! - `assembler/assembly_program_dump.cpp` — upstream PandasmProgramDumper
//! - `es2panda/aot/main.cpp` DumpProgramInfos — calls `PandasmProgramDumper::Dump`

use std::fmt;
use std::fmt::Write as FmtWrite;

use crate::program::model::Program;

/// Write a normalized text dump of a `Program` to the given `String`.
///
/// The dump is sorted by record name, then by method name within each record.
/// This normalization ensures the same source compiles to the same dump even
/// when internal insertion order varies.
pub fn dump_program(program: &Program, out: &mut String) {
    // Sort record names for a stable dump.
    let mut sorted_records: Vec<_> = program.record_table.iter_enumerated().collect();
    sorted_records.sort_by(|(_, a), (_, b)| a.name.cmp(&b.name));

    for (_, record) in &sorted_records {
        dump_record(record, out);
    }

    writeln!(out).ok();
    writeln!(out, ".language ECMAScript").ok();
}

fn dump_record(record: &crate::emit::record::ClassRecord, out: &mut String) {
    writeln!(out, ".record {} {{", record.name).ok();

    // Fields sorted by name.
    let mut sorted_fields = record.fields.clone();
    sorted_fields.sort_by(|a, b| a.name.cmp(&b.name));
    for field in &sorted_fields {
        let type_str = field_type_str(&field.field_type);
        if let Some(value) = &field.value {
            writeln!(out, "\t{type_str} {} = {}", field.name, field_value_str(value)).ok();
        } else {
            writeln!(out, "\t{type_str} {}", field.name).ok();
        }
    }

    writeln!(out, "}}").ok();
    writeln!(out).ok();

    // Methods (functions) sorted by name.
    let mut sorted_methods = record.methods.clone();
    sorted_methods.sort_by(|a, b| a.name.cmp(&b.name));

    for method in &sorted_methods {
        dump_function(method, out);
    }
}

fn dump_function(method: &crate::emit::record::MethodRecord, out: &mut String) {
    writeln!(out, ".function any {}(any a0, any a1, any a2) {{", method.name).ok();
    writeln!(out, "\tregs: {}", method.num_vregs).ok();
    writeln!(out, "\targs: {}", method.num_args).ok();
    writeln!(out, "\tbytecode_len: {}", method.bytecode.len()).ok();
    writeln!(out, "}}").ok();
    writeln!(out).ok();
}

fn field_type_str(field_type: &crate::emit::record::FieldType) -> &'static str {
    use crate::emit::record::FieldType;
    match field_type {
        FieldType::U1 => "u1",
        FieldType::I8 => "i8",
        FieldType::U8 => "u8",
        FieldType::I16 => "i16",
        FieldType::U16 => "u16",
        FieldType::I32 => "i32",
        FieldType::U32 => "u32",
        FieldType::F32 => "f32",
        FieldType::F64 => "f64",
        FieldType::I64 => "i64",
        FieldType::U64 => "u64",
        FieldType::PandaString => "panda.String",
        FieldType::Record { .. } => "record",
        FieldType::Array { .. } => "array",
        FieldType::Any => "any",
    }
}

fn field_value_str(value: &crate::emit::record::FieldValue) -> String {
    use crate::emit::record::FieldValue;
    match value {
        FieldValue::U8(v) => v.to_string(),
        FieldValue::Bool(v) => if *v { "1" } else { "0" }.to_string(),
        FieldValue::U32(v) => v.to_string(),
        FieldValue::F64(v) => format!("{v:.6}"),
        FieldValue::String(v) => format!("\"{}\"", v.escape_default()),
        FieldValue::LiteralArray(idx) => format!("literalarray_#{idx}"),
        FieldValue::RawScalar(v) => format!("scalar#{}", v.value_type),
    }
}

/// Format a complete Program dump as a `String`.
pub fn format_program_dump(program: &Program) -> String {
    let mut out = String::new();
    dump_program(program, &mut out);
    out
}

/// Format a normalized (sorted, stable) Program dump for oracle comparisons.
///
/// This is the primary API used by oracle tests to compare Rust output against
/// upstream `--dump-normalized-asm-program` output.
pub fn format_normalized_program_dump(program: &Program) -> String {
    // Currently the same as format_program_dump since we always normalize.
    format_program_dump(program)
}
