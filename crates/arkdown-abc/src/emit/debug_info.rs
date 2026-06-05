use crate::emit::file::StringItemWriter;
use crate::emit::record::{uleb128, ClassRecord, MethodRecord};
use std::cmp::Ordering;

const MANDATORY_PARAM_THIS_REG: u8 = 2;
const LNP_LINE_BASE: i32 = -4;
const LNP_LINE_RANGE: u32 = 15;

/// Encode a SLEB128 value as a byte vector.
pub(super) fn encode_sleb128_bytes(mut value: i32) -> Vec<u8> {
    let mut out = Vec::new();
    let mut more = true;
    while more {
        let byte = (value & 0x7F) as u8;
        value >>= 7;
        more = !(((value == 0) && (byte & 0x40 == 0)) || ((value == -1) && (byte & 0x40 != 0)));
        out.push(if more { byte | 0x80 } else { byte });
    }
    out
}

#[derive(Debug, Clone)]
pub(super) struct MidLocalEvent {
    pc: u32,
    kind: MidLocalKind,
    order: usize,
    reg: i32,
    name_off: u32,
    type_off: u32,
    type_signature_off: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MidLocalKind {
    Start,
    End,
}

pub(super) fn infer_func_main_class_local_events(
    method: &MethodRecord,
    class: &ClassRecord,
    string_items: &StringItemWriter,
    any_type_off: u32,
) -> Vec<MidLocalEvent> {
    if method.name != "func_main_0" {
        return Vec::new();
    }
    let Some(short_name) = class_short_name(&class.name) else {
        return Vec::new();
    };
    let Some(name_off) = string_items.id_of(&short_name) else {
        return Vec::new();
    };

    let has_params_shape = class
        .source_code_strings
        .iter()
        .any(|name| name.starts_with('#') && name.ends_with("_Params"));
    let reg = if has_params_shape { 6 } else { 5 };
    let base_pc = debug_scope_start_pc(method);
    let start_pc = if has_params_shape {
        infer_first_large_line_change_pc(method.line_ops.as_slice(), base_pc)
    } else {
        infer_second_line_change_pc(method.line_ops.as_slice(), base_pc)
    };
    let Some(start_pc) = start_pc else {
        return Vec::new();
    };
    let Some(end_pc) = infer_class_local_end_pc(method.bytecode.as_slice(), has_params_shape)
    else {
        return Vec::new();
    };
    if end_pc <= start_pc {
        return Vec::new();
    }

    vec![
        MidLocalEvent {
            pc: start_pc,
            kind: MidLocalKind::Start,
            order: 0,
            reg,
            name_off,
            type_off: any_type_off,
            type_signature_off: Some(any_type_off),
        },
        MidLocalEvent {
            pc: end_pc,
            kind: MidLocalKind::End,
            order: 1,
            reg,
            name_off,
            type_off: any_type_off,
            type_signature_off: Some(any_type_off),
        },
    ]
}

pub(super) fn local_variable_debug_param_offsets(
    method: &MethodRecord,
    string_items: &StringItemWriter,
) -> Vec<u32> {
    method
        .debug_info
        .as_ref()
        .map(|debug| {
            debug
                .local_variables
                .iter()
                .filter(|local| local.is_parameter(method.num_vregs))
                .filter_map(|local| string_items.id_of(&local.name))
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn local_variable_debug_events(
    method: &MethodRecord,
    string_items: &StringItemWriter,
) -> Vec<MidLocalEvent> {
    let Some(debug) = method.debug_info.as_ref() else {
        return Vec::new();
    };
    if debug.local_variables.is_empty() {
        return Vec::new();
    }

    let mut pairs = Vec::new();
    for (idx, local) in debug.local_variables.iter().enumerate() {
        if local.is_parameter(method.num_vregs) {
            continue;
        }
        pairs.push(LocalVariablePair {
            insn_order: local.start,
            variable_index: idx,
            kind: MidLocalKind::Start,
        });
        pairs.push(LocalVariablePair {
            insn_order: local.start.saturating_add(local.length),
            variable_index: idx,
            kind: MidLocalKind::End,
        });
    }

    pairs.sort_by(|a, b| {
        let a_var = &debug.local_variables[a.variable_index];
        let b_var = &debug.local_variables[b.variable_index];
        match a.insn_order.cmp(&b.insn_order) {
            Ordering::Equal if a_var.reg == b_var.reg => a_var.start.cmp(&b_var.start),
            other => other,
        }
    });

    let instruction_offsets = instruction_pc_offsets(method);
    pairs
        .into_iter()
        .enumerate()
        .map(|(order, pair)| {
            let local = &debug.local_variables[pair.variable_index];
            let name_off = string_items
                .id_of(&local.name)
                .expect("local variable name must be interned before LNP emit");
            let type_off = string_items
                .id_of(&local.signature)
                .expect("local variable signature must be interned before LNP emit");
            let type_signature_off = (!local.signature_type.is_empty()).then(|| {
                string_items
                    .id_of(&local.signature_type)
                    .expect("local variable signature_type must be interned before LNP emit")
            });
            MidLocalEvent {
                pc: pc_for_instruction_order(&instruction_offsets, pair.insn_order),
                kind: pair.kind,
                order,
                reg: local.reg,
                name_off,
                type_off,
                type_signature_off,
            }
        })
        .collect()
}

#[derive(Debug, Clone)]
struct LocalVariablePair {
    insn_order: u32,
    variable_index: usize,
    kind: MidLocalKind,
}

fn instruction_pc_offsets(method: &MethodRecord) -> Vec<u32> {
    let mut offsets = Vec::with_capacity(method.pandasm_insns.len() + 1);
    let mut pc = 0u32;
    offsets.push(pc);
    for insn in &method.pandasm_insns {
        pc = pc.saturating_add(insn.byte_len() as u32);
        offsets.push(pc);
    }
    offsets
}

fn pc_for_instruction_order(offsets: &[u32], instruction_order: u32) -> u32 {
    offsets
        .get(instruction_order as usize)
        .copied()
        .unwrap_or_else(|| offsets.last().copied().unwrap_or_default())
}

fn class_short_name(class_name: &str) -> Option<String> {
    let trimmed = class_name.strip_prefix('L').unwrap_or(class_name);
    let trimmed = trimmed.strip_suffix(';').unwrap_or(trimmed);
    trimmed
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .map(str::to_string)
}

fn infer_second_line_change_pc(method_ops: &[u8], base_pc: u32) -> Option<u32> {
    infer_nth_line_change_pc(method_ops, base_pc, 2, 1)
}

fn infer_first_large_line_change_pc(method_ops: &[u8], base_pc: u32) -> Option<u32> {
    infer_nth_line_change_pc(method_ops, base_pc, 1, 2)
}

fn infer_nth_line_change_pc(
    method_ops: &[u8],
    base_pc: u32,
    target_change: u8,
    min_abs_delta: i32,
) -> Option<u32> {
    let (ops, cp) = split_line_ops(method_ops)?;
    let mut cp_pos = 0usize;
    let mut op_pos = 0usize;
    let mut pc = base_pc;
    let mut line_changes = 0u8;
    while op_pos < ops.len() {
        let op = ops[op_pos];
        op_pos += 1;
        match op {
            0x01 => {
                let (delta, next) = read_uleb128_from(cp, cp_pos)?;
                cp_pos = next;
                pc = pc.saturating_add(delta as u32);
            }
            0x02 => {
                let (line_delta, next) = read_sleb128_from(cp, cp_pos)?;
                cp_pos = next;
                if line_delta.unsigned_abs() >= min_abs_delta as u32 {
                    line_changes += 1;
                    if line_changes == target_change {
                        return Some(pc);
                    }
                }
            }
            0x03 | 0x04 => {
                let (_, next_op) = read_sleb128_from(ops, op_pos)?;
                op_pos = next_op;
                let cp_count = if op == 0x04 { 3 } else { 2 };
                for _ in 0..cp_count {
                    let (_, next_cp) = read_uleb128_from(cp, cp_pos)?;
                    cp_pos = next_cp;
                }
            }
            0x05 | 0x06 => {
                let (_, next_op) = read_sleb128_from(ops, op_pos)?;
                op_pos = next_op;
            }
            0x09 | 0x0a | 0x0b => {
                let (_, next) = read_uleb128_from(cp, cp_pos)?;
                cp_pos = next;
            }
            op if op >= 0x0c => {
                let adjusted = (op as u32).saturating_sub(0x0c);
                let pc_delta = adjusted / LNP_LINE_RANGE;
                let line_delta = (adjusted % LNP_LINE_RANGE) as i32 + LNP_LINE_BASE;
                pc = pc.saturating_add(pc_delta);
                if line_delta.unsigned_abs() >= min_abs_delta as u32 {
                    line_changes += 1;
                    if line_changes == target_change {
                        return Some(pc);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn infer_class_local_end_pc(bytecode: &[u8], has_params_shape: bool) -> Option<u32> {
    if has_params_shape {
        for (idx, window) in bytecode.windows(5).enumerate() {
            if window[0] == 0x61 && window[2] == 0x60 && window[4] == 0x3d {
                return Some(idx as u32);
            }
        }
    } else if let Some(idx) = bytecode.iter().position(|&b| b == 0x7c) {
        return Some(idx as u32);
    }
    None
}

pub(super) fn append_line_ops_with_mid_locals(
    method_ops: &[u8],
    base_pc: u32,
    mid_locals: &[MidLocalEvent],
    lnp_data: &mut Vec<u8>,
    cp: &mut Vec<u8>,
) {
    let Some((ops, method_cp)) = split_line_ops(method_ops) else {
        lnp_data.extend_from_slice(method_ops);
        let mut events = mid_locals.to_vec();
        events.sort_by_key(|event| (event.pc, event.order));
        let mut event_pos = 0usize;
        let mut pc = base_pc;
        while event_pos < events.len() {
            let event_pc = events[event_pos].pc;
            if event_pc > pc {
                emit_adv_pc(event_pc - pc, lnp_data, cp);
                pc = event_pc;
            }
            emit_mid_local_events_at_pc(event_pc, &events, &mut event_pos, lnp_data, cp);
        }
        return;
    };
    let mut events = mid_locals.to_vec();
    events.sort_by_key(|event| (event.pc, event.order));
    let mut event_pos = 0usize;
    let mut pc = base_pc;
    let mut op_pos = 0usize;
    let mut cp_pos = 0usize;

    while op_pos < ops.len() {
        emit_mid_local_events_at_pc(pc, &events, &mut event_pos, lnp_data, cp);

        let op = ops[op_pos];
        op_pos += 1;
        match op {
            0x01 => {
                let (delta, next) = read_uleb128_from(method_cp, cp_pos).unwrap_or((0, cp_pos));
                cp_pos = next;
                append_pc_advance_with_events(
                    delta as u32,
                    &mut pc,
                    &events,
                    &mut event_pos,
                    lnp_data,
                    cp,
                );
            }
            0x02 => {
                let (delta, next) = read_sleb128_from(method_cp, cp_pos).unwrap_or((0, cp_pos));
                cp_pos = next;
                lnp_data.push(0x02);
                cp.extend_from_slice(&encode_sleb128_bytes(delta));
            }
            0x03 | 0x04 => {
                let reg_start = op_pos;
                let (_, next_op) = read_sleb128_from(ops, op_pos).unwrap_or((0, op_pos));
                op_pos = next_op;
                lnp_data.push(op);
                lnp_data.extend_from_slice(&ops[reg_start..next_op]);
                let cp_count = if op == 0x04 { 3 } else { 2 };
                for _ in 0..cp_count {
                    let (value, next_cp) =
                        read_uleb128_from(method_cp, cp_pos).unwrap_or((0, cp_pos));
                    cp_pos = next_cp;
                    cp.extend_from_slice(&uleb128(value));
                }
            }
            0x05 | 0x06 => {
                let reg_start = op_pos;
                let (_, next_op) = read_sleb128_from(ops, op_pos).unwrap_or((0, op_pos));
                op_pos = next_op;
                lnp_data.push(op);
                lnp_data.extend_from_slice(&ops[reg_start..next_op]);
            }
            0x09 | 0x0a | 0x0b => {
                let (value, next) = read_uleb128_from(method_cp, cp_pos).unwrap_or((0, cp_pos));
                cp_pos = next;
                lnp_data.push(op);
                cp.extend_from_slice(&uleb128(value));
            }
            op if op >= 0x0c => {
                let adjusted = (op as u32).saturating_sub(0x0c);
                let pc_delta = adjusted / LNP_LINE_RANGE;
                let line_delta = (adjusted % LNP_LINE_RANGE) as i32 + LNP_LINE_BASE;
                if has_mid_local_event_between(pc, pc.saturating_add(pc_delta), &events, event_pos)
                {
                    append_pc_advance_with_events(
                        pc_delta,
                        &mut pc,
                        &events,
                        &mut event_pos,
                        lnp_data,
                        cp,
                    );
                    emit_special_opcode(0, line_delta, lnp_data, cp);
                } else {
                    lnp_data.push(op);
                    pc = pc.saturating_add(pc_delta);
                }
            }
            _ => lnp_data.push(op),
        }
    }
    emit_mid_local_events_at_pc(pc, &events, &mut event_pos, lnp_data, cp);
}

fn split_line_ops(method_ops: &[u8]) -> Option<(&[u8], &[u8])> {
    let end_pos = method_ops.iter().position(|&b| b == 0x00)?;
    let after = end_pos + 1;
    if after + 4 > method_ops.len() {
        return Some((&method_ops[..end_pos], &[]));
    }
    let cp_len = u32::from_le_bytes(method_ops[after..after + 4].try_into().ok()?) as usize;
    let cp_start = after + 4;
    let cp_end = cp_start.checked_add(cp_len)?;
    if cp_end > method_ops.len() {
        return Some((&method_ops[..end_pos], &[]));
    }
    Some((&method_ops[..end_pos], &method_ops[cp_start..cp_end]))
}

fn append_pc_advance_with_events(
    delta: u32,
    pc: &mut u32,
    events: &[MidLocalEvent],
    event_pos: &mut usize,
    lnp_data: &mut Vec<u8>,
    cp: &mut Vec<u8>,
) {
    let target = pc.saturating_add(delta);
    while *event_pos < events.len() && events[*event_pos].pc <= target {
        let event_pc = events[*event_pos].pc;
        if event_pc > *pc {
            emit_adv_pc(event_pc - *pc, lnp_data, cp);
            *pc = event_pc;
        }
        emit_mid_local_events_at_pc(event_pc, events, event_pos, lnp_data, cp);
    }
    if target > *pc {
        emit_adv_pc(target - *pc, lnp_data, cp);
        *pc = target;
    }
}

fn emit_adv_pc(delta: u32, lnp_data: &mut Vec<u8>, cp: &mut Vec<u8>) {
    if delta == 0 {
        return;
    }
    lnp_data.push(0x01);
    cp.extend_from_slice(&uleb128(delta as u64));
}

fn emit_special_opcode(pc_delta: u32, line_delta: i32, lnp_data: &mut Vec<u8>, cp: &mut Vec<u8>) {
    let adjusted_line = line_delta - LNP_LINE_BASE;
    if pc_delta < LNP_LINE_RANGE && adjusted_line >= 0 && (adjusted_line as u32) < LNP_LINE_RANGE {
        let op = pc_delta * LNP_LINE_RANGE + adjusted_line as u32 + 0x0c;
        if op <= u8::MAX as u32 {
            lnp_data.push(op as u8);
            return;
        }
    }
    if pc_delta > 0 {
        emit_adv_pc(pc_delta, lnp_data, cp);
    }
    lnp_data.push(0x02);
    cp.extend_from_slice(&encode_sleb128_bytes(line_delta));
    lnp_data.push(0x0c);
}

fn has_mid_local_event_between(
    start_pc: u32,
    end_pc: u32,
    events: &[MidLocalEvent],
    event_pos: usize,
) -> bool {
    events
        .get(event_pos)
        .is_some_and(|event| event.pc > start_pc && event.pc <= end_pc)
}

fn emit_mid_local_events_at_pc(
    pc: u32,
    events: &[MidLocalEvent],
    event_pos: &mut usize,
    lnp_data: &mut Vec<u8>,
    cp: &mut Vec<u8>,
) {
    while *event_pos < events.len() && events[*event_pos].pc == pc {
        let event = &events[*event_pos];
        match event.kind {
            MidLocalKind::Start => {
                if event.type_signature_off.is_some() {
                    lnp_data.push(0x04);
                } else {
                    lnp_data.push(0x03);
                }
                lnp_data.extend_from_slice(&encode_sleb128_bytes(event.reg));
                cp.extend_from_slice(&uleb128(event.name_off as u64));
                cp.extend_from_slice(&uleb128(event.type_off as u64));
                if let Some(type_signature_off) = event.type_signature_off {
                    cp.extend_from_slice(&uleb128(type_signature_off as u64));
                }
            }
            MidLocalKind::End => {
                lnp_data.push(0x05);
                lnp_data.extend_from_slice(&encode_sleb128_bytes(event.reg));
            }
        }
        *event_pos += 1;
    }
}

fn read_uleb128_from(bytes: &[u8], mut pos: usize) -> Option<(u64, usize)> {
    let mut result = 0u64;
    let mut shift = 0u32;
    loop {
        let byte = *bytes.get(pos)?;
        pos += 1;
        result |= ((byte & 0x7f) as u64) << shift;
        if byte & 0x80 == 0 {
            return Some((result, pos));
        }
        shift += 7;
    }
}

fn read_sleb128_from(bytes: &[u8], mut pos: usize) -> Option<(i32, usize)> {
    let mut result = 0i32;
    let mut shift = 0u32;
    let byte = loop {
        let byte = *bytes.get(pos)?;
        pos += 1;
        result |= ((byte & 0x7f) as i32) << shift;
        shift += 7;
        if byte & 0x80 == 0 {
            break byte;
        }
    };
    if shift < 32 && byte & 0x40 != 0 {
        result |= !0 << shift;
    }
    Some((result, pos))
}

/// Encode a Panda `DebugInfoItem` with a pre-built constant_pool.
///
/// libpandafile `DebugInfoItem::Write` format:
/// ```text
/// line_num:           ULEB128        (1-based source line, or 0xFFFFFFFF sentinel)
/// num_parameters:     ULEB128        (number of formal parameters)
/// [param_off: ULEB128] * num_params  (file offset of each parameter name StringItem)
/// constant_pool_size: ULEB128        (byte count of the constant pool)
/// constant_pool:      raw bytes      (operands for LNP opcodes in the LNP data_ stream)
/// lnp_index:          ULEB128        (index into the LNP index table)
/// ```
///
/// The `constant_pool` contains ULEB128-encoded operands for opcodes in the LNP
/// data_ stream that need them (SET_FILE src_off, START_LOCAL_EXTENDED operands,
/// ADVANCE_PC delta, ADVANCE_LINE delta, etc.).
pub(super) fn encode_debug_info_with_cp(
    line_start: u32,
    param_name_offsets: &[u32],
    constant_pool: &[u8],
    lnp_index: u32,
    no_line_number: bool,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + constant_pool.len());
    // es2abc leaves the source line in the LineNumberProgram and writes the
    // DebugInfo line_num field as NO_LINE_NUMBER.
    let line_num = if no_line_number { u32::MAX } else { line_start };
    out.extend_from_slice(&uleb128(line_num as u64));
    out.extend_from_slice(&uleb128(param_name_offsets.len() as u64));
    for &off in param_name_offsets {
        out.extend_from_slice(&uleb128(off as u64));
    }
    // constant_pool_size = byte count (not item count).
    out.extend_from_slice(&uleb128(constant_pool.len() as u64));
    out.extend_from_slice(constant_pool);
    out.extend_from_slice(&uleb128(lnp_index as u64));
    out
}

/// Encode a Panda `DebugInfoItem` with a minimal constant_pool containing only
/// the SET_FILE operand. Used as a fallback when no LNP tracking is available.
#[allow(dead_code)]
fn encode_debug_info(
    line_start: u32,
    param_name_offsets: &[u32],
    source_file_offset: u32,
    lnp_index: u32,
) -> Vec<u8> {
    let cp = uleb128(source_file_offset as u64);
    encode_debug_info_with_cp(line_start, param_name_offsets, &cp, lnp_index, true)
}

pub(super) fn debug_scope_start_pc(method: &MethodRecord) -> u32 {
    let mut pc = argument_copy_prologue_size(method);

    // `prepend_module_argument_copies` preserves a leading NewLexEnvWithName
    // before the copied parameters.  Upstream local-variable ranges start after
    // that hidden scope setup.
    if method.name == "func_main_0" && method.bytecode.first() == Some(&0xb6) {
        pc += 4;
    }

    // Function bodies that capture `this` in an arrow start with the function
    // `this` lexenv, then copied args, then a hidden `lda this; stlexvar`.
    if method.name != "func_main_0"
        && method.bytecode.first() == Some(&0xb6)
        && method.bytecode.get(1) == Some(&1)
    {
        pc += 4; // NewLexEnvWithName
        let this_src = method.num_vregs + MANDATORY_PARAM_THIS_REG as u16;
        if this_src <= u8::MAX as u16 {
            pc += 4; // lda v(this_src) + stlexvar 0, 0
        }
    }

    pc
}

fn argument_copy_prologue_size(method: &MethodRecord) -> u32 {
    let has_this_lexenv = method.name != "func_main_0"
        && method.bytecode.first() == Some(&0xb6)
        && method.bytecode.get(1) == Some(&1);
    let mut size = 0u32;
    for dst in 0..method.num_args {
        if has_this_lexenv && dst == MANDATORY_PARAM_THIS_REG as u16 {
            continue;
        }
        let src = method.num_vregs + dst;
        size += if dst <= 0x0f && src <= 0x0f {
            2
        } else if src <= u8::MAX as u16 {
            3
        } else {
            5
        };
    }
    size
}
