use std::collections::HashMap;

pub(super) fn patch_bytecode_index_operands(
    bytecode: &[u8],
    string_pool_indices: &HashMap<u16, u16>,
    literal_pool_indices: &HashMap<u16, u16>,
    ctor_pool_index: u16,
    definefunc_pool_indices: &[u16],
    definemethod_pool_indices_by_name: &HashMap<u16, u16>,
    definefunc_cursor: &mut usize,
) -> Vec<u8> {
    let mut patched = bytecode.to_vec();
    let mut pos = 0;
    while pos < patched.len() {
        let Some(opcode) = patched.get(pos).copied() else {
            break;
        };
        match opcode {
            0x3e if pos + 2 < patched.len() => {
                rewrite_u16_operand(&mut patched, pos + 1, string_pool_indices);
                pos += 3;
                continue;
            }
            0x3f | 0x40 | 0x42 if pos + 3 < patched.len() => {
                rewrite_u16_operand(&mut patched, pos + 2, string_pool_indices);
                pos += 4;
                continue;
            }
            0x43 | 0x7a if pos + 4 < patched.len() => {
                rewrite_u16_operand(&mut patched, pos + 2, string_pool_indices);
                pos += 5;
                continue;
            }
            0xfe if patched.get(pos + 1) == Some(&0x09) && pos + 3 < patched.len() => {
                rewrite_u16_operand(&mut patched, pos + 2, string_pool_indices);
                pos += 4;
                continue;
            }
            _ => {}
        }
        if opcode == 0x33 && pos + 4 < patched.len() {
            let method_pos = pos + 2;
            let method_index = if let Some(&index) = definefunc_pool_indices.get(*definefunc_cursor)
            {
                *definefunc_cursor += 1;
                index
            } else {
                ctor_pool_index
            };
            patched[method_pos..method_pos + 2].copy_from_slice(&method_index.to_le_bytes());
            pos += 5;
            continue;
        }
        if opcode == 0x34 && pos + 4 < patched.len() {
            let method_pos = pos + 2;
            let method_name_id = u16::from_le_bytes([patched[method_pos], patched[method_pos + 1]]);
            if let Some(&method_index) = definemethod_pool_indices_by_name.get(&method_name_id) {
                patched[method_pos..method_pos + 2].copy_from_slice(&method_index.to_le_bytes());
            }
            pos += 5;
            continue;
        }
        if opcode == 0x35 && pos + 8 < patched.len() {
            let ctor_pos = pos + 2;
            patched[ctor_pos..ctor_pos + 2].copy_from_slice(&ctor_pool_index.to_le_bytes());
            let buf_pos = pos + 4;
            let old_buf = u16::from_le_bytes([patched[buf_pos], patched[buf_pos + 1]]);
            let new_buf = literal_pool_indices
                .get(&old_buf)
                .copied()
                .unwrap_or(old_buf);
            patched[buf_pos..buf_pos + 2].copy_from_slice(&new_buf.to_le_bytes());
            pos += 9;
            continue;
        }
        if opcode == 0xb6 && pos + 3 < patched.len() {
            let literal_pos = pos + 2;
            let old_literal = u16::from_le_bytes([patched[literal_pos], patched[literal_pos + 1]]);
            let new_literal = literal_pool_indices
                .get(&old_literal)
                .copied()
                .unwrap_or(old_literal);
            patched[literal_pos..literal_pos + 2].copy_from_slice(&new_literal.to_le_bytes());
            pos += 4;
            continue;
        }
        if opcode == 0x07 && pos + 3 < patched.len() {
            let literal_pos = pos + 2;
            let old_literal = u16::from_le_bytes([patched[literal_pos], patched[literal_pos + 1]]);
            let new_literal = literal_pool_indices
                .get(&old_literal)
                .copied()
                .unwrap_or(old_literal);
            patched[literal_pos..literal_pos + 2].copy_from_slice(&new_literal.to_le_bytes());
            pos += 4;
            continue;
        }
        pos += instruction_len(opcode, patched.get(pos + 1).copied());
    }
    patched
}

pub(super) fn assign_inline_cache_slots(bytecode: &mut [u8]) -> u32 {
    let mut current_slot = 0u32;
    let mut pos = 0usize;
    while pos < bytecode.len() {
        let Some(opcode) = bytecode.get(pos).copied() else {
            break;
        };
        if let Some((operand_pos, inc)) = inline_cache_slot_operand(bytecode, pos, opcode) {
            if let Some(slot) = bytecode.get_mut(operand_pos) {
                *slot = current_slot.min(u8::MAX as u32) as u8;
            }
            current_slot += inc as u32;
        }
        pos += instruction_len(opcode, bytecode.get(pos + 1).copied()).max(1);
    }
    current_slot
}

fn inline_cache_slot_operand(bytecode: &[u8], pos: usize, opcode: u8) -> Option<(usize, u8)> {
    if opcode == 0xfb {
        let sub_opcode = bytecode.get(pos + 1).copied()?;
        let operand_pos = pos + 2;
        if operand_pos >= bytecode.len() {
            return None;
        }
        let inc = match sub_opcode {
            0x13 | 0x14 => 1,
            _ => return None,
        };
        return Some((operand_pos, inc));
    }

    let operand_pos = pos + 1;
    if operand_pos >= bytecode.len() {
        return None;
    }
    let inc = match opcode {
        0x05
        | 0x07
        | 0x0a..=0x1a
        | 0x1f..=0x22
        | 0x25
        | 0x27
        | 0x28
        | 0x34
        | 0x33
        | 0x35
        | 0x3f
        | 0x40 => 1,
        0x08 | 0x1c | 0x29..=0x32 | 0x42 | 0x43 | 0x73 | 0x79 | 0x7a | 0xb9 => 2,
        _ => return None,
    };
    Some((operand_pos, inc))
}

fn rewrite_u16_operand(patched: &mut [u8], operand_pos: usize, pool_indices: &HashMap<u16, u16>) {
    let old = u16::from_le_bytes([patched[operand_pos], patched[operand_pos + 1]]);
    if let Some(&new) = pool_indices.get(&old) {
        patched[operand_pos..operand_pos + 2].copy_from_slice(&new.to_le_bytes());
    }
}

pub(super) fn collect_bytecode_string_indices(bytecode: &[u8], indices: &mut Vec<u32>) {
    let mut pos = 0usize;
    while pos < bytecode.len() {
        let Some(opcode) = bytecode.get(pos).copied() else {
            break;
        };
        match opcode {
            0x3e if pos + 2 < bytecode.len() => {
                indices.push(u16::from_le_bytes([bytecode[pos + 1], bytecode[pos + 2]]) as u32);
                pos += 3;
                continue;
            }
            0x3f | 0x40 | 0x42 if pos + 3 < bytecode.len() => {
                indices.push(u16::from_le_bytes([bytecode[pos + 2], bytecode[pos + 3]]) as u32);
                pos += 4;
                continue;
            }
            0x43 | 0x7a if pos + 4 < bytecode.len() => {
                indices.push(u16::from_le_bytes([bytecode[pos + 2], bytecode[pos + 3]]) as u32);
                pos += 5;
                continue;
            }
            0xfe if bytecode.get(pos + 1) == Some(&0x09) && pos + 3 < bytecode.len() => {
                indices.push(u16::from_le_bytes([bytecode[pos + 2], bytecode[pos + 3]]) as u32);
                pos += 4;
                continue;
            }
            _ => {}
        }

        let next = bytecode.get(pos + 1).copied();
        pos += instruction_len(opcode, next).max(1);
    }
}

fn instruction_len(opcode: u8, next: Option<u8>) -> usize {
    match opcode {
        0x00..=0x03 | 0x64 | 0x65 => 1,
        0x1c | 0x29 | 0x3c | 0x3d | 0x44 | 0x4d | 0x4f | 0x51 | 0x60 | 0x61 | 0x7c | 0x7e => 2,
        0x2a | 0x2d | 0x3e | 0x45 | 0x50 | 0x9b | 0x4e | 0xb9 => 3,
        0x0a..=0x1a | 0x25 | 0x27 | 0x28 => 3,
        0x1f..=0x22 => 2,
        0x07 | 0x08 | 0x2b | 0x2e | 0x31 | 0x32 | 0x3f | 0x40 | 0x42 | 0x73 | 0xb6 => 4,
        0x2c | 0x33 | 0x34 | 0x43 | 0x62 | 0x7a | 0x8f | 0x98 | 0x9a | 0x9c | 0xbc => 5,
        0x2f => 5,
        0x30 => 6,
        0x35 | 0x63 => 9,
        0xcf => 2,
        0xfe if next == Some(0x00) => 2,
        0xfe if next == Some(0x07) => 3,
        0xfe if next == Some(0x09) => 4,
        0xfb if matches!(next, Some(0x13 | 0x14)) => 3,
        _ => 1,
    }
}
