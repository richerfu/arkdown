use std::sync::OnceLock;

use serde::Deserialize;

use crate::ir::opcodes::{PandasmImm, PandasmInsn};

const ISA_YAML: &str = include_str!("isa.yaml");

#[derive(Debug, Deserialize)]
struct IsaYaml {
    groups: Vec<GroupYaml>,
}

#[derive(Debug, Deserialize)]
struct GroupYaml {
    #[serde(default)]
    properties: Vec<String>,
    #[serde(default)]
    instructions: Vec<InstructionYaml>,
}

#[derive(Debug, Deserialize)]
struct InstructionYaml {
    sig: String,
    opcode_idx: Vec<u8>,
    format: Vec<String>,
    #[serde(default)]
    prefix: Option<String>,
    #[serde(default)]
    properties: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct InstructionSpec {
    pub(crate) pandasm_opcode: u32,
    prefix: Option<u8>,
    opcode: u8,
    format: String,
    operands: Vec<OperandKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OperandKind {
    Reg,
    Imm {
        signed: bool,
        float: bool,
        jump: bool,
    },
    Id(IdKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IdKind {
    Method,
    String,
    LiteralArray,
    Other,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct IdOperand {
    pub(crate) kind: IdKind,
    pub(crate) offset: usize,
    pub(crate) width: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FormatKind {
    Reg,
    Imm,
    Id,
}

#[derive(Debug, Clone, Copy)]
struct FormatOperand {
    kind: FormatKind,
    width: u8,
    offset: usize,
    high_nibble: bool,
}

static SPECS: OnceLock<Vec<InstructionSpec>> = OnceLock::new();

pub(crate) fn pandasm_spec(prefix: Option<u8>, opcode: u8) -> Option<&'static InstructionSpec> {
    specs()
        .iter()
        .find(|spec| spec.prefix == prefix && spec.opcode == opcode)
}

pub(crate) fn decode_pandasm_instruction<F>(
    bytecode: &[u8],
    pos: usize,
    mut resolve_id: F,
) -> Option<PandasmInsn>
where
    F: FnMut(IdKind, u32) -> Option<Vec<u8>>,
{
    let first = *bytecode.get(pos)?;
    let (prefix, opcode, header_len) = if is_prefix(first) {
        (Some(first), *bytecode.get(pos + 1)?, 2usize)
    } else {
        (None, first, 1usize)
    };
    let spec = pandasm_spec(prefix, opcode)?;
    let format = parse_format(&spec.format, header_len)?;
    let len = format_len(&format, header_len);
    let end = pos.checked_add(len)?;
    let bytes = bytecode.get(pos..end)?;

    let mut regs = Vec::new();
    let mut ids = Vec::new();
    let mut imms = Vec::new();
    for (operand, layout) in spec.operands.iter().zip(format.iter()) {
        match (*operand, layout.kind) {
            (OperandKind::Reg, FormatKind::Reg) => {
                regs.push(u32::try_from(read_unsigned(bytes, layout)?).ok()?);
            }
            (OperandKind::Imm { float: true, .. }, FormatKind::Imm) => {
                let bits = read_unsigned(bytes, layout)?;
                imms.push(PandasmImm::Double(f64::from_bits(bits)));
            }
            (OperandKind::Imm { jump: true, .. }, FormatKind::Imm) => {
                let value = read_signed(bytes, layout)?;
                ids.push(value.to_string().into_bytes());
            }
            (OperandKind::Imm { signed: true, .. }, FormatKind::Imm) => {
                imms.push(PandasmImm::Int(read_signed(bytes, layout)?));
            }
            (OperandKind::Imm { .. }, FormatKind::Imm) => {
                imms.push(PandasmImm::Int(read_unsigned(bytes, layout)? as i64));
            }
            (OperandKind::Id(kind), FormatKind::Id) => {
                let index = u32::try_from(read_unsigned(bytes, layout)?).ok()?;
                ids.push(resolve_id(kind, index)?);
            }
            _ => return None,
        }
    }

    Some(PandasmInsn {
        opcode: spec.pandasm_opcode,
        regs,
        ids,
        imms,
        bytes: bytes.to_vec(),
    })
}

pub(crate) fn instruction_len(bytecode: &[u8], pos: usize) -> Option<usize> {
    let first = *bytecode.get(pos)?;
    let (prefix, opcode, header_len) = if is_prefix(first) {
        (Some(first), *bytecode.get(pos + 1)?, 2usize)
    } else {
        (None, first, 1usize)
    };
    let spec = pandasm_spec(prefix, opcode)?;
    let format = parse_format(&spec.format, header_len)?;
    Some(format_len(&format, header_len))
}

pub(crate) fn id_operands(bytecode: &[u8], pos: usize) -> Option<Vec<IdOperand>> {
    let first = *bytecode.get(pos)?;
    let (prefix, opcode, header_len) = if is_prefix(first) {
        (Some(first), *bytecode.get(pos + 1)?, 2usize)
    } else {
        (None, first, 1usize)
    };
    let spec = pandasm_spec(prefix, opcode)?;
    let format = parse_format(&spec.format, header_len)?;
    let mut ids = Vec::new();
    for (operand, layout) in spec.operands.iter().zip(format.iter()) {
        if let (OperandKind::Id(kind), FormatKind::Id) = (*operand, layout.kind) {
            ids.push(IdOperand {
                kind,
                offset: layout.offset,
                width: layout.width,
            });
        }
    }
    Some(ids)
}

pub(crate) fn encode_pandasm_instruction<F>(
    opcode: u32,
    regs: &[u32],
    ids: &[Vec<u8>],
    imms: &[PandasmImm],
    mut resolve_id: F,
) -> Option<Vec<u8>>
where
    F: FnMut(IdKind, &[u8]) -> Option<u32>,
{
    for spec in specs().iter().filter(|spec| spec.pandasm_opcode == opcode) {
        let header_len = usize::from(spec.prefix.is_some()) + 1;
        let format = parse_format(&spec.format, header_len)?;
        if format.len() != spec.operands.len() {
            continue;
        }
        let len = format_len(&format, header_len);
        let mut bytes = vec![0; len];
        let mut cursor = 0usize;
        if let Some(prefix) = spec.prefix {
            bytes[cursor] = prefix;
            cursor += 1;
        }
        bytes[cursor] = spec.opcode;

        let mut reg_index = 0usize;
        let mut id_index = 0usize;
        let mut imm_index = 0usize;
        let mut failed = false;
        for (operand, layout) in spec.operands.iter().zip(format.iter()) {
            let value = match (*operand, layout.kind) {
                (OperandKind::Reg, FormatKind::Reg) => {
                    let value = *regs.get(reg_index)?;
                    reg_index += 1;
                    i64::from(value)
                }
                (OperandKind::Imm { jump: true, .. }, FormatKind::Imm) => {
                    let id = ids.get(id_index)?;
                    id_index += 1;
                    std::str::from_utf8(id).ok()?.parse::<i64>().ok()?
                }
                (OperandKind::Imm { float: true, .. }, FormatKind::Imm) => {
                    let value = match imms.get(imm_index)? {
                        PandasmImm::Double(value) => value.to_bits(),
                        PandasmImm::Int(_) => {
                            failed = true;
                            0
                        }
                    };
                    imm_index += 1;
                    if failed || !write_unsigned(&mut bytes, layout, value) {
                        failed = true;
                    }
                    continue;
                }
                (OperandKind::Imm { .. }, FormatKind::Imm) => {
                    let value = match imms.get(imm_index)? {
                        PandasmImm::Int(value) => *value,
                        PandasmImm::Double(_) => {
                            failed = true;
                            0
                        }
                    };
                    imm_index += 1;
                    value
                }
                (OperandKind::Id(kind), FormatKind::Id) => {
                    let id = ids.get(id_index)?;
                    id_index += 1;
                    i64::from(resolve_id(kind, id)?)
                }
                _ => {
                    failed = true;
                    0
                }
            };
            if failed {
                break;
            }
            let ok = match operand {
                OperandKind::Imm { signed: true, .. } => write_signed(&mut bytes, layout, value),
                _ => u64::try_from(value)
                    .ok()
                    .is_some_and(|value| write_unsigned(&mut bytes, layout, value)),
            };
            if !ok {
                failed = true;
                break;
            }
        }
        if !failed && reg_index == regs.len() && id_index == ids.len() && imm_index == imms.len() {
            return Some(bytes);
        }
    }
    None
}

fn specs() -> &'static [InstructionSpec] {
    SPECS.get_or_init(|| {
        let isa: IsaYaml =
            serde_yaml::from_str(ISA_YAML).expect("embedded Panda ISA yaml is valid");
        let mut specs = Vec::new();
        let mut pandasm_opcode = 0u32;
        for group in isa.groups {
            let group_jump = group.properties.iter().any(|property| property == "jump");
            for instruction in group.instructions {
                let instruction_jump = group_jump
                    || instruction
                        .properties
                        .iter()
                        .any(|property| property == "jump");
                let operands = parse_operands(&instruction.sig, instruction_jump);
                let prefix = instruction.prefix.as_deref().map(prefix_byte);
                for (opcode, format) in instruction
                    .opcode_idx
                    .iter()
                    .copied()
                    .zip(instruction.format.iter().cloned())
                {
                    specs.push(InstructionSpec {
                        pandasm_opcode,
                        prefix,
                        opcode,
                        format,
                        operands: operands.clone(),
                    });
                }
                pandasm_opcode = pandasm_opcode.saturating_add(1);
            }
        }
        specs
    })
}

fn parse_operands(sig: &str, jump: bool) -> Vec<OperandKind> {
    let Some((_, operands)) = sig.split_once(' ') else {
        return Vec::new();
    };
    operands
        .split(',')
        .filter_map(|operand| {
            let operand = operand.trim();
            let name = operand.split(':').next().unwrap_or_default();
            let normalized = name.trim_end_matches(|ch: char| ch.is_ascii_digit());
            if normalized == "v" {
                Some(OperandKind::Reg)
            } else if normalized == "imm" {
                Some(OperandKind::Imm {
                    signed: jump || operand.contains(":i"),
                    float: operand.contains(":f64"),
                    jump,
                })
            } else if matches!(normalized, "method_id") {
                Some(OperandKind::Id(IdKind::Method))
            } else if matches!(normalized, "string_id" | "stringId") {
                Some(OperandKind::Id(IdKind::String))
            } else if matches!(normalized, "literalarray_id") {
                Some(OperandKind::Id(IdKind::LiteralArray))
            } else if matches!(normalized, "type_id" | "field_id" | "callsite_id") {
                Some(OperandKind::Id(IdKind::Other))
            } else {
                None
            }
        })
        .collect()
}

fn parse_format(format: &str, header_len: usize) -> Option<Vec<FormatOperand>> {
    let format = format.strip_prefix("pref_").unwrap_or(format);
    let mut parts = format.split('_');
    if parts.next()? != "op" {
        return None;
    }

    let mut operands = Vec::new();
    let mut offset = header_len;
    let mut low_nibble_used = false;
    while let Some(kind) = parts.next() {
        if kind == "none" {
            break;
        }
        let width = parts.next()?.parse::<u8>().ok()?;
        let format_kind = if kind.starts_with('v') {
            FormatKind::Reg
        } else if kind.starts_with("imm") {
            FormatKind::Imm
        } else if kind.starts_with("id") {
            FormatKind::Id
        } else {
            return None;
        };

        if width == 4 {
            operands.push(FormatOperand {
                kind: format_kind,
                width,
                offset,
                high_nibble: low_nibble_used,
            });
            if low_nibble_used {
                offset = offset.saturating_add(1);
            }
            low_nibble_used = !low_nibble_used;
        } else {
            if low_nibble_used {
                return None;
            }
            operands.push(FormatOperand {
                kind: format_kind,
                width,
                offset,
                high_nibble: false,
            });
            offset = offset.saturating_add((width as usize) / 8);
        }
    }

    if low_nibble_used {
        offset = offset.saturating_add(1);
    }
    let _ = offset;
    Some(operands)
}

fn format_len(format: &[FormatOperand], header_len: usize) -> usize {
    format
        .iter()
        .map(|operand| operand.offset + operand_width_bytes(operand.width))
        .max()
        .unwrap_or(header_len)
}

fn operand_width_bytes(width: u8) -> usize {
    if width == 4 {
        1
    } else {
        (width as usize) / 8
    }
}

fn read_unsigned(bytes: &[u8], operand: &FormatOperand) -> Option<u64> {
    let value = match operand.width {
        4 => {
            let byte = *bytes.get(operand.offset)?;
            if operand.high_nibble {
                (byte >> 4) as u64
            } else {
                (byte & 0x0f) as u64
            }
        }
        8 => *bytes.get(operand.offset)? as u64,
        16 => u16::from_le_bytes([*bytes.get(operand.offset)?, *bytes.get(operand.offset + 1)?])
            as u64,
        32 => u32::from_le_bytes([
            *bytes.get(operand.offset)?,
            *bytes.get(operand.offset + 1)?,
            *bytes.get(operand.offset + 2)?,
            *bytes.get(operand.offset + 3)?,
        ]) as u64,
        64 => u64::from_le_bytes([
            *bytes.get(operand.offset)?,
            *bytes.get(operand.offset + 1)?,
            *bytes.get(operand.offset + 2)?,
            *bytes.get(operand.offset + 3)?,
            *bytes.get(operand.offset + 4)?,
            *bytes.get(operand.offset + 5)?,
            *bytes.get(operand.offset + 6)?,
            *bytes.get(operand.offset + 7)?,
        ]),
        _ => return None,
    };
    Some(value)
}

fn read_signed(bytes: &[u8], operand: &FormatOperand) -> Option<i64> {
    let value = match operand.width {
        4 => read_unsigned(bytes, operand)? as i64,
        8 => i8::from_le_bytes([*bytes.get(operand.offset)?]) as i64,
        16 => i16::from_le_bytes([*bytes.get(operand.offset)?, *bytes.get(operand.offset + 1)?])
            as i64,
        32 => i32::from_le_bytes([
            *bytes.get(operand.offset)?,
            *bytes.get(operand.offset + 1)?,
            *bytes.get(operand.offset + 2)?,
            *bytes.get(operand.offset + 3)?,
        ]) as i64,
        64 => i64::from_le_bytes([
            *bytes.get(operand.offset)?,
            *bytes.get(operand.offset + 1)?,
            *bytes.get(operand.offset + 2)?,
            *bytes.get(operand.offset + 3)?,
            *bytes.get(operand.offset + 4)?,
            *bytes.get(operand.offset + 5)?,
            *bytes.get(operand.offset + 6)?,
            *bytes.get(operand.offset + 7)?,
        ]),
        _ => return None,
    };
    Some(value)
}

fn write_unsigned(bytes: &mut [u8], operand: &FormatOperand, value: u64) -> bool {
    match operand.width {
        4 if value <= 0x0f => {
            let Some(byte) = bytes.get_mut(operand.offset) else {
                return false;
            };
            if operand.high_nibble {
                *byte = (*byte & 0x0f) | ((value as u8) << 4);
            } else {
                *byte = (*byte & 0xf0) | (value as u8);
            }
            true
        }
        8 if value <= u8::MAX as u64 => {
            let Some(slot) = bytes.get_mut(operand.offset) else {
                return false;
            };
            *slot = value as u8;
            true
        }
        16 if value <= u16::MAX as u64 => {
            let end = operand.offset + 2;
            let Some(slot) = bytes.get_mut(operand.offset..end) else {
                return false;
            };
            slot.copy_from_slice(&(value as u16).to_le_bytes());
            true
        }
        32 if value <= u32::MAX as u64 => {
            let end = operand.offset + 4;
            let Some(slot) = bytes.get_mut(operand.offset..end) else {
                return false;
            };
            slot.copy_from_slice(&(value as u32).to_le_bytes());
            true
        }
        64 => {
            let end = operand.offset + 8;
            let Some(slot) = bytes.get_mut(operand.offset..end) else {
                return false;
            };
            slot.copy_from_slice(&value.to_le_bytes());
            true
        }
        _ => false,
    }
}

fn write_signed(bytes: &mut [u8], operand: &FormatOperand, value: i64) -> bool {
    match operand.width {
        4 if (0..=0x0f).contains(&value) => write_unsigned(bytes, operand, value as u64),
        8 if i8::try_from(value).is_ok() => {
            let Some(slot) = bytes.get_mut(operand.offset) else {
                return false;
            };
            *slot = value as i8 as u8;
            true
        }
        16 if i16::try_from(value).is_ok() => {
            let end = operand.offset + 2;
            let Some(slot) = bytes.get_mut(operand.offset..end) else {
                return false;
            };
            slot.copy_from_slice(&(value as i16).to_le_bytes());
            true
        }
        32 if i32::try_from(value).is_ok() => {
            let end = operand.offset + 4;
            let Some(slot) = bytes.get_mut(operand.offset..end) else {
                return false;
            };
            slot.copy_from_slice(&(value as i32).to_le_bytes());
            true
        }
        64 => {
            let end = operand.offset + 8;
            let Some(slot) = bytes.get_mut(operand.offset..end) else {
                return false;
            };
            slot.copy_from_slice(&value.to_le_bytes());
            true
        }
        _ => false,
    }
}

fn is_prefix(byte: u8) -> bool {
    matches!(byte, 0xfb..=0xfe)
}

fn prefix_byte(prefix: &str) -> u8 {
    match prefix {
        "callruntime" => 0xfb,
        "deprecated" => 0xfc,
        "wide" => 0xfd,
        "throw" => 0xfe,
        _ => 0xff,
    }
}

#[cfg(test)]
mod tests {
    use super::decode_pandasm_instruction;

    #[test]
    fn decodes_untyped_no_operand_instruction_from_upstream_isa() {
        let insn =
            decode_pandasm_instruction(&[0x6a], 0, |_, _| None).expect("ldnan decodes from ISA");
        assert_eq!(insn.opcode, 0);
        assert!(insn.regs.is_empty());
        assert!(insn.ids.is_empty());
        assert!(insn.imms.is_empty());
        assert_eq!(insn.bytes, vec![0x6a]);
    }

    #[test]
    fn decodes_prefixed_instruction_from_upstream_isa() {
        let insn = decode_pandasm_instruction(&[0xfc, 0x00], 0, |_, _| None)
            .expect("deprecated.ldlexenv decodes from ISA");
        assert_eq!(insn.opcode, 9);
        assert_eq!(insn.bytes, vec![0xfc, 0x00]);
    }

    #[test]
    fn encodes_untyped_instruction_with_shortest_matching_format() {
        let bytes = super::encode_pandasm_instruction(
            19,
            &[],
            &[],
            &[super::PandasmImm::Int(7)],
            |_, _| None,
        )
        .expect("getiterator encodes");
        assert_eq!(bytes, vec![0x67, 7]);
    }
}
