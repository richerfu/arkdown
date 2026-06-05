use std::collections::{BTreeMap, BTreeSet};

use rustc_hash::FxHashMap;

use crate::abc_reader::{AbcFieldValue, AbcLiteralEntry, AbcReader};
use crate::emit::bytecode::instruction_len;
use crate::emit::file::{AbcFile, LiteralArray, LiteralEntry};
use crate::emit::record::{
    method_access, CatchEntry, ClassRecord, FieldRecord, FieldValue, MethodKind, MethodRecord,
    FIELD_OWNER_CLASS_IDX,
};
use crate::error::AbcError;
use crate::ir::opcodes::Insn;
use crate::isa::{
    decode_pandasm_instruction, id_operands as isa_id_operands,
    instruction_len as isa_instruction_len, IdKind,
};

pub(crate) fn append_abc_file(file: &mut AbcFile, data: &[u8]) -> Result<Vec<usize>, AbcError> {
    let reader = AbcReader::new(data).map_err(read_error("<abc input>"))?;
    let classes = reader.class_items().map_err(read_error("<abc input>"))?;

    let module_literal_offsets = classes
        .iter()
        .flat_map(|class| {
            class.fields.iter().filter_map(|field| {
                (field.name == "moduleRecordIdx")
                    .then_some(field.value)
                    .flatten()
            })
        })
        .filter_map(|value| match value {
            AbcFieldValue::U32(offset) => Some(offset),
            AbcFieldValue::U8(_) => None,
        })
        .collect::<BTreeSet<_>>();

    let literal_offsets = reader
        .literal_array_offsets()
        .map_err(read_error("<abc input>"))?;
    let mut literal_offset_to_index = BTreeMap::new();
    for offset in literal_offsets {
        let literal = if module_literal_offsets.contains(&offset) {
            read_module_record_literal_array(data, &reader, offset)?
        } else {
            read_tagged_literal_array(&reader, offset)?
        };
        let index = file.literal_arrays.len();
        file.literal_arrays.push(literal);
        literal_offset_to_index.insert(offset, index);
    }

    let method_names = classes
        .iter()
        .flat_map(|class| {
            class
                .methods
                .iter()
                .map(|method| (method.offset, method.name.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    let method_pool_offsets = reader
        .index_sections()
        .map_err(read_error("<abc input>"))?
        .into_iter()
        .flat_map(|section| section.method_offsets)
        .filter(|offset| method_names.contains_key(offset))
        .collect::<BTreeSet<_>>();

    let mut appended = Vec::new();
    for class in classes {
        if is_ark_annotation_stub(&class.name) {
            continue;
        }

        let mut record_string_indices = BTreeSet::new();
        let mut record_literal_indices = BTreeSet::new();
        let fields = class
            .fields
            .iter()
            .map(|field| {
                Ok(FieldRecord {
                    name: field.name.clone(),
                    class_idx: FIELD_OWNER_CLASS_IDX,
                    type_idx: field.type_idx,
                    access_flags: field.access_flags,
                    value: field_value_from_abc(
                        field.name.as_str(),
                        field.value,
                        &literal_offset_to_index,
                    )?,
                })
            })
            .collect::<Result<Vec<_>, AbcError>>()?;

        let methods = class
            .methods
            .iter()
            .map(|method| {
                let code = method
                    .code
                    .as_ref()
                    .ok_or_else(|| AbcError::UnsupportedConstruct {
                        construct: format!("ABC method {} has no CodeItem", method.name),
                        file: class.name.clone(),
                    })?;
                let bytecode = unpatch_bytecode_index_operands(
                    &reader,
                    file,
                    method.offset,
                    &code.bytecode,
                    &literal_offset_to_index,
                    &method_names,
                    &mut record_string_indices,
                    &mut record_literal_indices,
                )?;
                let pandasm_insns = decode_bytecode(&bytecode, file, class.name.as_str())?;
                Ok(MethodRecord {
                    name: method.name.clone(),
                    bytecode,
                    pandasm_insns,
                    pandasm_debug: Vec::new(),
                    num_vregs: code.num_vregs as u16,
                    num_args: code.num_args as u16,
                    debug_info: None,
                    line_ops: Vec::new(),
                    catch_entries: code
                        .catches
                        .iter()
                        .map(|catch| CatchEntry {
                            try_begin: catch.try_begin,
                            try_end: catch.try_end,
                            catch_begin: catch.handler_pc,
                            catch_end: catch.handler_pc.saturating_add(catch.code_size),
                        })
                        .collect(),
                    kind: method_kind_from_access_flags(method.name.as_str(), method.access_flags),
                    method_pool_entry: method_pool_offsets.contains(&method.offset),
                    expected_property_count: None,
                    definefunc_order: None,
                })
            })
            .collect::<Result<Vec<_>, AbcError>>()?;

        let record_index = file.records.len();
        file.add_record(ClassRecord {
            name: class.name,
            methods,
            string_indices: record_string_indices.into_iter().collect(),
            literal_indices: record_literal_indices.into_iter().collect(),
            module_literal_indices: class
                .fields
                .iter()
                .filter_map(|field| match (field.name.as_str(), field.value) {
                    ("moduleRecordIdx" | "scopeNames", Some(AbcFieldValue::U32(offset))) => {
                        literal_offset_to_index.get(&offset).copied()
                    }
                    _ => None,
                })
                .collect(),
            source_code_strings: Vec::new(),
            access_flags: class.access_flags,
            source_lang: class.source_lang.unwrap_or(0),
            standalone_ecmascript_input: false,
            source_file: None,
            module_requests: Vec::new(),
            fields,
        });
        appended.push(record_index);
    }

    Ok(appended)
}

fn read_error(file: &'static str) -> impl Fn(crate::abc_reader::AbcReadError) -> AbcError {
    move |error| AbcError::UnsupportedConstruct {
        construct: format!("ABC input parse failed: {error}"),
        file: file.to_string(),
    }
}

fn is_ark_annotation_stub(name: &str) -> bool {
    matches!(
        name,
        "L_ESConcurrentModuleRequestsAnnotation;"
            | "L_ESExpectedPropertyCountAnnotation;"
            | "L_ESSlotNumberAnnotation;"
    )
}

fn field_value_from_abc(
    field_name: &str,
    value: Option<AbcFieldValue>,
    literal_offset_to_index: &BTreeMap<u32, usize>,
) -> Result<Option<FieldValue>, AbcError> {
    let Some(value) = value else {
        return Ok(None);
    };
    match value {
        AbcFieldValue::U8(value) => Ok(Some(FieldValue::U8(value))),
        AbcFieldValue::U32(offset)
            if matches!(
                field_name,
                "moduleRecordIdx" | "scopeNames" | "moduleRequestPhaseIdx"
            ) =>
        {
            let Some(index) = literal_offset_to_index.get(&offset).copied() else {
                return Err(AbcError::UnsupportedConstruct {
                    construct: format!(
                        "missing literal array for field {field_name} at {offset:#x}"
                    ),
                    file: "<abc input>".to_string(),
                });
            };
            Ok(Some(FieldValue::LiteralArray(index)))
        }
        AbcFieldValue::U32(value) => Ok(Some(FieldValue::U32(value))),
    }
}

fn read_tagged_literal_array(
    reader: &AbcReader<'_>,
    offset: u32,
) -> Result<LiteralArray, AbcError> {
    let array = reader
        .read_literal_array_item(offset)
        .map_err(read_error("<abc input>"))?;
    let mut entries = Vec::with_capacity(array.entries.len());
    for entry in array.entries {
        entries.push(match entry {
            AbcLiteralEntry::U32(value) => LiteralEntry::TaggedInteger(value),
            AbcLiteralEntry::String { value, .. } => LiteralEntry::TaggedString(value),
            AbcLiteralEntry::Method { offset, name } => name
                .map(LiteralEntry::TaggedMethodName)
                .unwrap_or(LiteralEntry::TaggedMethod(offset)),
            AbcLiteralEntry::U16(value) => LiteralEntry::TaggedMethodAffiliate(value),
            AbcLiteralEntry::Null => LiteralEntry::TaggedNull,
            other => {
                return Err(AbcError::UnsupportedConstruct {
                    construct: format!("literal entry {other:?} is not representable yet"),
                    file: format!("{offset:#x}"),
                })
            }
        });
    }
    Ok(LiteralArray { entries })
}

fn read_module_record_literal_array(
    data: &[u8],
    reader: &AbcReader<'_>,
    offset: u32,
) -> Result<LiteralArray, AbcError> {
    let mut cursor = offset as usize;
    let item_count = read_u32(data, &mut cursor)? as usize;
    let mut consumed = 0usize;
    let mut entries = Vec::with_capacity(item_count);

    read_raw_u32(data, &mut cursor, &mut consumed, &mut entries)?;
    let requests = match entries.last() {
        Some(LiteralEntry::RawU32(value)) => *value as usize,
        _ => 0,
    };
    for _ in 0..requests {
        read_raw_string(data, reader, &mut cursor, &mut consumed, &mut entries)?;
    }

    read_counted(
        data,
        reader,
        &mut cursor,
        &mut consumed,
        &mut entries,
        |data, reader, cursor, consumed, entries| {
            read_raw_string(data, reader, cursor, consumed, entries)?;
            read_raw_string(data, reader, cursor, consumed, entries)?;
            read_raw_u16(data, cursor, consumed, entries)
        },
    )?;
    read_counted(
        data,
        reader,
        &mut cursor,
        &mut consumed,
        &mut entries,
        |data, reader, cursor, consumed, entries| {
            read_raw_string(data, reader, cursor, consumed, entries)?;
            read_raw_u16(data, cursor, consumed, entries)
        },
    )?;
    read_counted(
        data,
        reader,
        &mut cursor,
        &mut consumed,
        &mut entries,
        |data, reader, cursor, consumed, entries| {
            read_raw_string(data, reader, cursor, consumed, entries)?;
            read_raw_string(data, reader, cursor, consumed, entries)
        },
    )?;
    read_counted(
        data,
        reader,
        &mut cursor,
        &mut consumed,
        &mut entries,
        |data, reader, cursor, consumed, entries| {
            read_raw_string(data, reader, cursor, consumed, entries)?;
            read_raw_string(data, reader, cursor, consumed, entries)?;
            read_raw_u16(data, cursor, consumed, entries)
        },
    )?;
    read_counted(
        data,
        reader,
        &mut cursor,
        &mut consumed,
        &mut entries,
        |data, _reader, cursor, consumed, entries| read_raw_u16(data, cursor, consumed, entries),
    )?;

    if consumed != item_count {
        return Err(AbcError::UnsupportedConstruct {
            construct: format!(
                "module literal array item count mismatch: header={item_count}, parsed={consumed}"
            ),
            file: format!("{offset:#x}"),
        });
    }

    Ok(LiteralArray { entries })
}

fn read_counted(
    data: &[u8],
    reader: &AbcReader<'_>,
    cursor: &mut usize,
    consumed: &mut usize,
    entries: &mut Vec<LiteralEntry>,
    mut read_item: impl FnMut(
        &[u8],
        &AbcReader<'_>,
        &mut usize,
        &mut usize,
        &mut Vec<LiteralEntry>,
    ) -> Result<(), AbcError>,
) -> Result<(), AbcError> {
    read_raw_u32(data, cursor, consumed, entries)?;
    let count = match entries.last() {
        Some(LiteralEntry::RawU32(value)) => *value as usize,
        _ => 0,
    };
    for _ in 0..count {
        read_item(data, reader, cursor, consumed, entries)?;
    }
    Ok(())
}

fn read_raw_u32(
    data: &[u8],
    cursor: &mut usize,
    consumed: &mut usize,
    entries: &mut Vec<LiteralEntry>,
) -> Result<(), AbcError> {
    let value = read_u32(data, cursor)?;
    entries.push(LiteralEntry::RawU32(value));
    *consumed += 1;
    Ok(())
}

fn read_raw_u16(
    data: &[u8],
    cursor: &mut usize,
    consumed: &mut usize,
    entries: &mut Vec<LiteralEntry>,
) -> Result<(), AbcError> {
    if cursor.saturating_add(2) > data.len() {
        return Err(AbcError::UnsupportedConstruct {
            construct: "truncated module literal u16".to_string(),
            file: "<abc input>".to_string(),
        });
    }
    let value = u16::from_le_bytes([data[*cursor], data[*cursor + 1]]);
    *cursor += 2;
    entries.push(LiteralEntry::RawU16(value));
    *consumed += 1;
    Ok(())
}

fn read_raw_string(
    data: &[u8],
    reader: &AbcReader<'_>,
    cursor: &mut usize,
    consumed: &mut usize,
    entries: &mut Vec<LiteralEntry>,
) -> Result<(), AbcError> {
    let offset = read_u32(data, cursor)?;
    let value = reader
        .string_at(offset)
        .map_err(read_error("<abc input>"))?;
    entries.push(LiteralEntry::RawString(value));
    *consumed += 1;
    Ok(())
}

fn read_u32(data: &[u8], cursor: &mut usize) -> Result<u32, AbcError> {
    if cursor.saturating_add(4) > data.len() {
        return Err(AbcError::UnsupportedConstruct {
            construct: "truncated ABC u32".to_string(),
            file: "<abc input>".to_string(),
        });
    }
    let value = u32::from_le_bytes([
        data[*cursor],
        data[*cursor + 1],
        data[*cursor + 2],
        data[*cursor + 3],
    ]);
    *cursor += 4;
    Ok(value)
}

fn method_kind_from_access_flags(name: &str, access_flags: u32) -> MethodKind {
    if name == "func_main_0" {
        MethodKind::Module
    } else if name == "constructor" {
        MethodKind::Constructor
    } else if access_flags == method_access::ANON {
        MethodKind::Anonymous
    } else {
        MethodKind::Function
    }
}

fn unpatch_bytecode_index_operands(
    reader: &AbcReader<'_>,
    file: &mut AbcFile,
    method_off: u32,
    bytecode: &[u8],
    literal_offset_to_index: &BTreeMap<u32, usize>,
    method_names: &BTreeMap<u32, String>,
    record_string_indices: &mut BTreeSet<u32>,
    record_literal_indices: &mut BTreeSet<usize>,
) -> Result<Vec<u8>, AbcError> {
    let pool = reader
        .method_index_for_method(method_off)
        .map_err(read_error("<abc input>"))?
        .unwrap_or_default();
    let mut string_pool = FxHashMap::<u32, u32>::default();
    let mut literal_pool = FxHashMap::<u32, u32>::default();
    let mut method_pool = FxHashMap::<u32, u32>::default();

    for (pool_index, offset) in pool.into_iter().enumerate() {
        let pool_index = pool_index as u32;
        if let Some(&literal_index) = literal_offset_to_index.get(&offset) {
            literal_pool.insert(pool_index, literal_index as u32);
        } else if let Some(name) = method_names.get(&offset) {
            let string_id = file.strings.intern(name);
            record_string_indices.insert(string_id);
            method_pool.insert(pool_index, string_id);
        } else if let Ok(value) = reader.string_at(offset) {
            let string_id = file.strings.intern(value.as_str());
            record_string_indices.insert(string_id);
            string_pool.insert(pool_index, string_id);
        }
    }

    let mut out = bytecode.to_vec();
    let mut pos = 0usize;
    while pos < out.len() {
        let opcode = out[pos];
        match opcode {
            0x3e if pos + 2 < out.len() => {
                rewrite_pool_u16(&mut out, pos + 1, &string_pool);
                pos += 3;
                continue;
            }
            0x3f | 0x40 | 0x42 if pos + 3 < out.len() => {
                rewrite_pool_u16(&mut out, pos + 2, &string_pool);
                pos += 4;
                continue;
            }
            0x43 | 0x7a | 0x8e | 0xdb | 0xdc if pos + 4 < out.len() => {
                rewrite_pool_u16(&mut out, pos + 2, &string_pool);
                pos += 5;
                continue;
            }
            0xfe if out.get(pos + 1) == Some(&0x09) && pos + 3 < out.len() => {
                rewrite_pool_u16(&mut out, pos + 2, &string_pool);
                pos += 4;
                continue;
            }
            0x33 | 0x34 if pos + 4 < out.len() => {
                rewrite_pool_u16(&mut out, pos + 2, &method_pool);
                pos += 5;
                continue;
            }
            0x35 if pos + 8 < out.len() => {
                rewrite_pool_u16(&mut out, pos + 2, &method_pool);
                rewrite_literal_pool_u16(&mut out, pos + 4, &literal_pool, record_literal_indices);
                pos += 9;
                continue;
            }
            0xb6 if pos + 3 < out.len() => {
                rewrite_literal_pool_u16(&mut out, pos + 2, &literal_pool, record_literal_indices);
                pos += 4;
                continue;
            }
            0x06 | 0x07 if pos + 3 < out.len() => {
                rewrite_literal_pool_u16(&mut out, pos + 2, &literal_pool, record_literal_indices);
                pos += 4;
                continue;
            }
            0xfb if out.get(pos + 1) == Some(&0x04) && pos + 5 < out.len() => {
                rewrite_literal_pool_u16(&mut out, pos + 4, &literal_pool, record_literal_indices);
                pos += 6;
                continue;
            }
            _ => {}
        }
        if let Some(len) = rewrite_isa_id_operands(
            &mut out,
            pos,
            &string_pool,
            &method_pool,
            &literal_pool,
            record_literal_indices,
        ) {
            pos += len;
            continue;
        }
        pos += instruction_len(opcode, out.get(pos + 1).copied()).max(1);
    }

    Ok(out)
}

fn rewrite_pool_u16(bytecode: &mut [u8], operand_pos: usize, pool_map: &FxHashMap<u32, u32>) {
    let old = u16::from_le_bytes([bytecode[operand_pos], bytecode[operand_pos + 1]]);
    if let Some(new) = pool_map
        .get(&(old as u32))
        .and_then(|new| u16::try_from(*new).ok())
    {
        bytecode[operand_pos..operand_pos + 2].copy_from_slice(&new.to_le_bytes());
    }
}

fn rewrite_literal_pool_u16(
    bytecode: &mut [u8],
    operand_pos: usize,
    pool_map: &FxHashMap<u32, u32>,
    record_literal_indices: &mut BTreeSet<usize>,
) {
    let old = u16::from_le_bytes([bytecode[operand_pos], bytecode[operand_pos + 1]]);
    if let Some(new) = pool_map
        .get(&(old as u32))
        .and_then(|new| u16::try_from(*new).ok())
    {
        bytecode[operand_pos..operand_pos + 2].copy_from_slice(&new.to_le_bytes());
        record_literal_indices.insert(new as usize);
    }
}

fn rewrite_isa_id_operands(
    bytecode: &mut [u8],
    pos: usize,
    string_pool: &FxHashMap<u32, u32>,
    method_pool: &FxHashMap<u32, u32>,
    literal_pool: &FxHashMap<u32, u32>,
    record_literal_indices: &mut BTreeSet<usize>,
) -> Option<usize> {
    let len = isa_instruction_len(bytecode, pos)?;
    if pos.checked_add(len)? > bytecode.len() {
        return None;
    }
    let operands = isa_id_operands(bytecode, pos)?;
    for operand in operands {
        let operand_pos = pos.checked_add(operand.offset)?;
        let old = read_id_operand(bytecode, operand_pos, operand.width)?;
        let new = match operand.kind {
            IdKind::String => string_pool.get(&old).copied(),
            IdKind::Method => method_pool.get(&old).copied(),
            IdKind::LiteralArray => literal_pool.get(&old).copied().inspect(|idx| {
                record_literal_indices.insert(*idx as usize);
            }),
            IdKind::Other => None,
        };
        if let Some(new) = new {
            write_id_operand(bytecode, operand_pos, operand.width, new)?;
        }
    }
    Some(len)
}

fn read_id_operand(bytecode: &[u8], operand_pos: usize, width: u8) -> Option<u32> {
    match width {
        16 => {
            Some(
                u16::from_le_bytes([*bytecode.get(operand_pos)?, *bytecode.get(operand_pos + 1)?])
                    as u32,
            )
        }
        32 => Some(u32::from_le_bytes([
            *bytecode.get(operand_pos)?,
            *bytecode.get(operand_pos + 1)?,
            *bytecode.get(operand_pos + 2)?,
            *bytecode.get(operand_pos + 3)?,
        ])),
        _ => None,
    }
}

fn write_id_operand(bytecode: &mut [u8], operand_pos: usize, width: u8, value: u32) -> Option<()> {
    match width {
        16 => {
            let value = u16::try_from(value).ok()?;
            bytecode[operand_pos..operand_pos + 2].copy_from_slice(&value.to_le_bytes());
        }
        32 => {
            bytecode[operand_pos..operand_pos + 4].copy_from_slice(&value.to_le_bytes());
        }
        _ => return None,
    }
    Some(())
}

fn decode_bytecode(
    bytecode: &[u8],
    file: &AbcFile,
    record_name: &str,
) -> Result<Vec<Insn>, AbcError> {
    let mut insns = Vec::new();
    let mut pos = 0usize;
    while pos < bytecode.len() {
        let insn = decode_one(bytecode, pos, file, record_name)?;
        let len = insn.byte_len().max(1);
        if pos.saturating_add(len) > bytecode.len() {
            return Err(AbcError::UnsupportedConstruct {
                construct: format!(
                    "truncated ABC bytecode instruction opcode {:#x} at pc {pos}",
                    bytecode[pos]
                ),
                file: "<abc input>".to_string(),
            });
        }
        insns.push(insn);
        pos += len;
    }
    Ok(insns)
}

fn decode_one(
    bytecode: &[u8],
    pos: usize,
    file: &AbcFile,
    record_name: &str,
) -> Result<Insn, AbcError> {
    let op = bytecode[pos];
    let at = |offset: usize| -> u8 { bytecode[pos + offset] };
    let u16_at = |offset: usize| -> u16 { u16::from_le_bytes([at(offset), at(offset + 1)]) };
    let i16_at = |offset: usize| -> i16 { i16::from_le_bytes([at(offset), at(offset + 1)]) };
    let i32_at = |offset: usize| -> i32 {
        i32::from_le_bytes([at(offset), at(offset + 1), at(offset + 2), at(offset + 3)])
    };
    let u64_at = |offset: usize| -> u64 {
        u64::from_le_bytes([
            at(offset),
            at(offset + 1),
            at(offset + 2),
            at(offset + 3),
            at(offset + 4),
            at(offset + 5),
            at(offset + 6),
            at(offset + 7),
        ])
    };

    let insn = match op {
        0x00 => Insn::LdUndefined,
        0x01 => Insn::LdNull,
        0x02 => Insn::LdTrue,
        0x03 => Insn::LdFalse,
        0x04 => Insn::CreateEmptyObject,
        0x05 => Insn::CreateEmptyArray(at(1) as u16),
        0x06 => Insn::CreateArrayWithBuffer(at(1), u16_at(2)),
        0x07 => Insn::CreateObjectWithBuffer(at(1), u16_at(2)),
        0x08 => Insn::NewObjRange(at(1), at(2), at(3)),
        0x09 => Insn::NewLexEnv(at(1)),
        0x0a => Insn::Add2(at(1), at(2)),
        0x0b => Insn::Sub2(at(1), at(2)),
        0x0c => Insn::Mul2(at(1), at(2)),
        0x0d => Insn::Div2(at(1), at(2)),
        0x0e => Insn::Mod2(at(1), at(2)),
        0x0f => Insn::Eq(at(1), at(2)),
        0x10 => Insn::NotEq(at(1), at(2)),
        0x11 => Insn::Less(at(1), at(2)),
        0x12 => Insn::LessEq(at(1), at(2)),
        0x13 => Insn::Greater(at(1), at(2)),
        0x14 => Insn::GreaterEq(at(1), at(2)),
        0x15 => Insn::Shl2(at(1), at(2)),
        0x16 => Insn::Shr2(at(1), at(2)),
        0x17 => Insn::Ashr2(at(1), at(2)),
        0x18 => Insn::And2(at(1), at(2)),
        0x19 => Insn::Or2(at(1), at(2)),
        0x1a => Insn::Xor2(at(1), at(2)),
        0x1b => Insn::Exp(at(1), at(2)),
        0x1c => Insn::TypeOf(at(1)),
        0x1d => Insn::ToNumber(at(1)),
        0x1e => Insn::ToNumeric(at(1)),
        0x1f => Insn::Neg(at(1)),
        0x20 => Insn::BitNot(at(1)),
        0x21 => Insn::Inc(at(1)),
        0x22 => Insn::Dec(at(1)),
        0x23 => Insn::IsTrue,
        0x24 => Insn::IsFalse,
        0x25 => Insn::IsIn(at(1), at(2)),
        0x26 => Insn::InstanceOf(at(1), at(2)),
        0x27 => Insn::StrictNotEq(at(1), at(2)),
        0x28 => Insn::StrictEq(at(1), at(2)),
        0x29 => Insn::CallArg0(at(1)),
        0x2a => Insn::CallArg1(at(1), at(2)),
        0x2b => Insn::CallArgs2(at(1), at(2), at(3)),
        0x2c => Insn::CallArgs3(at(1), at(2), at(3), at(4)),
        0x2d => Insn::CallThis0(at(1), at(2)),
        0x2e => Insn::CallThis1(at(1), at(2), at(3)),
        0x2f => Insn::CallThis2(at(1), at(2), at(3), at(4)),
        0x30 => Insn::CallThis3(at(1), at(2), at(3), at(4), at(5)),
        0x31 => Insn::CallThisRange(at(1), at(2), at(3)),
        0x32 => Insn::SuperCallThisRange(at(1), at(2), at(3)),
        0x33 => Insn::DefineFuncShort(at(1), u16_at(2), at(4)),
        0x34 => Insn::DefineMethodShort(at(1), u16_at(2), at(4)),
        0x35 => Insn::DefineClassWithBuffer(at(1), u16_at(2), u16_at(4), u16_at(6), at(8)),
        0x36 => Insn::GetNextPropName(at(1)),
        0x37 => Insn::LdObjByValue(at(1), at(2)),
        0x38 => Insn::StObjByValue(at(1), at(2), at(3)),
        0x39 => Insn::LdSuperByValue(at(1), at(2)),
        0x3c => {
            let packed = at(1);
            Insn::LdLexVar(packed & 0x0f, packed >> 4)
        }
        0x3d => {
            let packed = at(1);
            Insn::StLexVar(packed & 0x0f, packed >> 4)
        }
        0x3e => Insn::LdaStr(u16_at(1)),
        0x3f => Insn::TryLdGlobalByName(at(1), u16_at(2)),
        0x40 => Insn::TryStGlobalByName(at(1), u16_at(2)),
        0x42 => Insn::LdObjByName(at(1), u16_at(2)),
        0x43 => Insn::StObjByName(at(1), u16_at(2), at(4)),
        0x44 => {
            let packed = at(1);
            Insn::MovV4(packed & 0x0f, packed >> 4)
        }
        0x45 => Insn::MovV8(at(1), at(2)),
        0x46 => Insn::LdSuperByName(at(1), u16_at(2)),
        0x4d => Insn::JmpShort(at(1) as i8),
        0x4e => Insn::Jmp(i16_at(1)),
        0x4f => Insn::JeqzShort(at(1) as i8),
        0x50 => Insn::Jeqz(i16_at(1)),
        0x51 => Insn::JnezShort(at(1) as i8),
        0x60 => Insn::Lda(at(1)),
        0x61 => Insn::Sta(at(1)),
        0x62 => Insn::LdAi(i32_at(1)),
        0x63 => Insn::FLdAi(f64::from_bits(u64_at(1))),
        0x64 => Insn::Return,
        0x65 => Insn::ReturnUndefined,
        0x66 => Insn::GetPropIterator,
        0x67 => Insn::GetIterator(at(1)),
        0x68 => Insn::CloseIterator(at(1), at(2)),
        0x69 => Insn::PopLexEnv,
        0x6d => Insn::LdGlobal,
        0x6f => Insn::LdThis,
        0x70 => Insn::LdHole,
        0x73 => Insn::CallRange(at(1), at(2), at(3)),
        0x76 => Insn::GetTemplateObject(at(1) as u16),
        0x77 => Insn::SetObjectWithProto(at(1), at(2)),
        0x78 => Insn::StOwnByValue(at(1), at(2), at(3)),
        0x79 => Insn::StOwnByIndex(at(1) as u16, at(2), u16_at(3)),
        0x7a => Insn::StOwnByName(at(1), u16_at(2), at(4)),
        0x7c => Insn::StModuleVar(at(1)),
        0x7d => Insn::LdLocalModuleVar(at(1)),
        0x7e => Insn::LdExternalModuleVar(at(1)),
        0x80 => Insn::CreateEmptyArray(u16_at(1)),
        0x8a => Insn::LdLexVar(at(1), at(2)),
        0x8b => Insn::StLexVar(at(1), at(2)),
        0x8e => Insn::StOwnByNameWithNameSet(at(1), u16_at(2), at(4)),
        0x8f => Insn::MovV16(u16_at(1), u16_at(3)),
        0x98 => Insn::JmpWide(i32_at(1)),
        0x99 => Insn::StOwnByValueWithNameSet(at(1), at(2), at(3)),
        0x9a => Insn::JeqzWide(i32_at(1)),
        0x9b => Insn::Jnez(i16_at(1)),
        0x9c => Insn::JnezWide(i32_at(1)),
        0xa1 | 0xcf => Insn::CopyRestArgs(at(1)),
        0xb0 => Insn::Debugger,
        0xb1 => Insn::CreateGeneratorObj(at(1)),
        0xb3 => Insn::CreateObjectWithExcludedKeys(at(1) as u16, at(2), at(3)),
        0xb4 => Insn::NewObjApply(at(1), at(2)),
        0xb6 => Insn::NewLexEnvWithName(at(1), u16_at(2)),
        0xb9 => Insn::SuperCallSpread(at(1), at(2)),
        0xba => Insn::Apply(at(1), at(2), at(3)),
        0xbb => Insn::SuperCallArrowRange(at(1), at(2), at(3)),
        0xbc => Insn::DefineGetterSetterByValue(at(1), at(2), at(3), at(4)),
        0xbd => Insn::DynamicImport,
        0xbf => Insn::ResumeGenerator,
        0xc0 => Insn::GetResumeMode,
        0xc1 => Insn::GetTemplateObject(u16_at(1)),
        0xc2 => Insn::DelObjProp(at(1)),
        0xc3 => Insn::SuspendGenerator(at(1)),
        0xc4 => Insn::AsyncFunctionAwaitUncaught(at(1)),
        0xc5 => Insn::CopyDataProperties(at(1)),
        0xc6 => Insn::StArraySpread(at(1), at(2)),
        0xc9 => Insn::StSuperByValue(at(1), at(2), at(3)),
        0xcb => Insn::StOwnByIndex(u16_at(1), at(3), u16_at(4)),
        0xcd => Insn::AsyncFunctionResolve(at(1)),
        0xce => Insn::AsyncFunctionReject(at(1)),
        0xd0 => Insn::StSuperByName(at(1), u16_at(2), at(4)),
        0xd3 => Insn::LdBigInt(u16_at(1)),
        0xd5 => Insn::Nop,
        0xd8 => Insn::LdPrivateProperty(at(1), u16_at(2), u16_at(4)),
        0xd9 => Insn::StPrivateProperty(at(1), u16_at(2), u16_at(4), at(6)),
        0xda => Insn::TestIn(at(1), u16_at(2), u16_at(4)),
        0xdb => Insn::DefineFieldByName(at(1), u16_at(2), at(4)),
        0xdc => Insn::DefinePropertyByName(at(1), u16_at(2), at(4)),
        0xae => Insn::AsyncFunctionEnter,
        0xfd if at(1) == 0x00 => Insn::CreateObjectWithExcludedKeys(u16_at(2), at(4), at(5)),
        0xfe => match at(1) {
            0x00 => Insn::Throw,
            0x02 => Insn::ThrowPatternNonCoercible,
            0x05 => Insn::ThrowIfNotObject(at(2)),
            0x07 => Insn::ThrowIfSuperNotCorrectCall(at(2)),
            0x09 => Insn::ThrowUndefinedIfHoleWithName(u16_at(2)),
            sub => {
                return decode_pandasm_instruction(bytecode, pos, |kind, index| {
                    resolve_pandasm_id(file, record_name, kind, index)
                })
                .map(Insn::Pandasm)
                .ok_or_else(|| unsupported_prefixed_error("throw", sub, pos));
            }
        },
        0xfb => match at(1) {
            0x01 => Insn::CallRuntimeDefineFieldByValue(at(2), at(3), at(4)),
            0x04 => Insn::CallRuntimeCreatePrivateProperty(u16_at(2), u16_at(4)),
            0x05 => Insn::CallRuntimeDefinePrivateProperty(at(2), u16_at(3), u16_at(5), at(7)),
            0x06 => Insn::CallRuntimeCallInit(at(2), at(3)),
            0x13 => Insn::CallRuntimeIsTrue(at(2)),
            0x14 => Insn::CallRuntimeIsFalse(at(2)),
            sub => {
                return decode_pandasm_instruction(bytecode, pos, |kind, index| {
                    resolve_pandasm_id(file, record_name, kind, index)
                })
                .map(Insn::Pandasm)
                .ok_or_else(|| unsupported_prefixed_error("callruntime", sub, pos));
            }
        },
        _ => {
            if let Some(raw) = decode_pandasm_instruction(bytecode, pos, |kind, index| {
                resolve_pandasm_id(file, record_name, kind, index)
            }) {
                return Ok(Insn::Pandasm(raw));
            }
            return Err(AbcError::UnsupportedConstruct {
                construct: format!(
                    "unsupported ABC bytecode opcode {op:#x} at pc {pos} for Program materialization"
                ),
                file: "<abc input>".to_string(),
            });
        }
    };
    Ok(insn)
}

fn resolve_pandasm_id(
    file: &AbcFile,
    record_name: &str,
    kind: IdKind,
    index: u32,
) -> Option<Vec<u8>> {
    match kind {
        IdKind::String | IdKind::Method => file
            .strings
            .get(index)
            .map(|value| value.as_bytes().to_vec()),
        IdKind::LiteralArray => Some(format!("{record_name}_{index}").into_bytes()),
        IdKind::Other => None,
    }
}

fn unsupported_prefixed_error(prefix: &str, sub_opcode: u8, pos: usize) -> AbcError {
    AbcError::UnsupportedConstruct {
        construct: format!(
            "unsupported ABC bytecode {prefix} sub-opcode {sub_opcode:#x} at pc {pos} for Program materialization"
        ),
        file: "<abc input>".to_string(),
    }
}
