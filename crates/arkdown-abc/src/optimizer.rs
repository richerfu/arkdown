//! Bytecode optimizer entry aligned with upstream `bytecode_optimizer`.
//!
//! Upstream `OptimizeFunction()` emits pandasm to ABC, rebuilds a compiler graph
//! from bytecode, runs the optimizer pipeline, then writes the optimized frame
//! size back to `function.regs_num`.  arkdown does not yet carry a pandasm graph,
//! so this module owns the encoded-bytecode side of that contract and is where
//! the Rust pass stack is being ported.

use crate::emit::bytecode::instruction_len;
use crate::emit::file::AbcFile;
use crate::emit::record::MethodRecord;

#[derive(Debug, Clone, Copy)]
pub(crate) struct BytecodeOptimizerOptions {
    pub(crate) opt_level: u8,
    pub(crate) skip_methods_with_eh: bool,
}

impl BytecodeOptimizerOptions {
    pub(crate) fn from_compile_options(opt_level: u8, optimize_try_catch_func: bool) -> Self {
        Self {
            opt_level,
            // es2panda/aot/options.cpp wires bytecodeopt::options.SetSkipMethodsWithEh
            // to !--opt-try-catch-func.
            skip_methods_with_eh: !optimize_try_catch_func,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct RegOperand {
    offset: usize,
    width: RegWidth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegWidth {
    U4Low,
    U4High,
    U8,
    U16,
}

#[derive(Debug, Clone, Copy)]
struct RegUse {
    operand: RegOperand,
    value: u16,
}

#[derive(Debug, Clone, Copy)]
struct RangeUse {
    start_operand: RegOperand,
    start: u16,
    count: u16,
}

#[derive(Debug, Default)]
struct MethodFrameUse {
    register_uses: Vec<RegUse>,
    range_uses: Vec<RangeUse>,
    frame_values: Vec<u16>,
    fully_decoded: bool,
}

#[cfg(test)]
pub(crate) fn optimize_file(file: &mut AbcFile, options: BytecodeOptimizerOptions) {
    let should_optimize = vec![true; file.records.len()];
    optimize_records(file, options, &should_optimize);
}

pub(crate) fn optimize_records(
    file: &mut AbcFile,
    options: BytecodeOptimizerOptions,
    should_optimize: &[bool],
) {
    if options.opt_level == 0 {
        return;
    }

    debug_assert_eq!(file.records.len(), should_optimize.len());
    for (record, optimize_record) in file.records.iter_mut().zip(should_optimize) {
        if !*optimize_record {
            continue;
        }
        for method in &mut record.methods {
            optimize_method(method, options);
        }
    }
}

fn optimize_method(method: &mut MethodRecord, options: BytecodeOptimizerOptions) {
    if options.skip_methods_with_eh && !method.catch_entries.is_empty() {
        return;
    }

    compact_register_frame(method);
}

fn compact_register_frame(method: &mut MethodRecord) {
    if method.num_vregs == 0 {
        return;
    }

    let frame_use = analyze_method_frame(method);
    if !frame_use.fully_decoded {
        return;
    }

    let old_num_vregs = method.num_vregs;
    let arg_end = old_num_vregs.saturating_add(method.num_args);
    if frame_use.frame_values.iter().any(|value| *value >= arg_end) {
        return;
    }

    let mut used_locals: Vec<u16> = frame_use
        .frame_values
        .iter()
        .filter_map(|value| (*value < old_num_vregs).then_some(*value))
        .collect();
    used_locals.sort_unstable();
    used_locals.dedup();

    let Ok(new_num_vregs) = u16::try_from(used_locals.len()) else {
        return;
    };
    if new_num_vregs >= old_num_vregs {
        return;
    }

    if !register_frame_rewrite_is_encodable(
        &frame_use,
        &used_locals,
        old_num_vregs,
        new_num_vregs,
        method.num_args,
    ) {
        return;
    }

    rewrite_register_frame(
        &mut method.bytecode,
        &frame_use,
        &used_locals,
        old_num_vregs,
        new_num_vregs,
        method.num_args,
    );
    rewrite_register_frame_insns(
        &mut method.pandasm_insns,
        &used_locals,
        old_num_vregs,
        new_num_vregs,
        method.num_args,
    );
    method.num_vregs = new_num_vregs;
}

fn rewrite_register_frame_insns(
    insns: &mut [crate::ir::opcodes::Insn],
    used_locals: &[u16],
    old_num_vregs: u16,
    new_num_vregs: u16,
    num_args: u16,
) {
    for insn in insns {
        insn.remap_register_operands(|value| {
            map_frame_value(value, used_locals, old_num_vregs, new_num_vregs, num_args)
        });
    }
}

fn register_frame_rewrite_is_encodable(
    frame_use: &MethodFrameUse,
    used_locals: &[u16],
    old_num_vregs: u16,
    new_num_vregs: u16,
    num_args: u16,
) -> bool {
    frame_use.register_uses.iter().all(|reg_use| {
        map_frame_value(
            reg_use.value,
            used_locals,
            old_num_vregs,
            new_num_vregs,
            num_args,
        )
        .is_some_and(|new_value| register_operand_can_encode(reg_use.operand, new_value))
    }) && frame_use.range_uses.iter().all(|range| {
        mapped_range_start(range, used_locals, old_num_vregs, new_num_vregs, num_args)
            .is_some_and(|new_start| register_operand_can_encode(range.start_operand, new_start))
    })
}

fn rewrite_register_frame(
    bytecode: &mut [u8],
    frame_use: &MethodFrameUse,
    used_locals: &[u16],
    old_num_vregs: u16,
    new_num_vregs: u16,
    num_args: u16,
) {
    for reg_use in &frame_use.register_uses {
        let Some(new_value) = map_frame_value(
            reg_use.value,
            used_locals,
            old_num_vregs,
            new_num_vregs,
            num_args,
        ) else {
            continue;
        };
        if new_value != reg_use.value {
            write_register_operand(bytecode, reg_use.operand, new_value);
        }
    }
    for range in &frame_use.range_uses {
        let Some(new_start) =
            mapped_range_start(range, used_locals, old_num_vregs, new_num_vregs, num_args)
        else {
            continue;
        };
        if new_start != range.start {
            write_register_operand(bytecode, range.start_operand, new_start);
        }
    }
}

fn map_frame_value(
    value: u16,
    used_locals: &[u16],
    old_num_vregs: u16,
    new_num_vregs: u16,
    num_args: u16,
) -> Option<u16> {
    let arg_end = old_num_vregs.saturating_add(num_args);
    if value < old_num_vregs {
        used_locals
            .binary_search(&value)
            .ok()
            .and_then(|index| u16::try_from(index).ok())
    } else if value < arg_end {
        Some(new_num_vregs + (value - old_num_vregs))
    } else {
        None
    }
}

fn mapped_range_start(
    range: &RangeUse,
    used_locals: &[u16],
    old_num_vregs: u16,
    new_num_vregs: u16,
    num_args: u16,
) -> Option<u16> {
    let mut mapped_start = None;
    for index in 0..range.count {
        let value = range.start.checked_add(index)?;
        let mapped = map_frame_value(value, used_locals, old_num_vregs, new_num_vregs, num_args)?;
        match mapped_start {
            None => mapped_start = Some(mapped),
            Some(start) if mapped == start.checked_add(index)? => {}
            Some(_) => return None,
        }
    }
    mapped_start.or_else(|| {
        map_frame_value(
            range.start,
            used_locals,
            old_num_vregs,
            new_num_vregs,
            num_args,
        )
    })
}

fn analyze_method_frame(method: &MethodRecord) -> MethodFrameUse {
    let mut out = MethodFrameUse {
        fully_decoded: true,
        ..MethodFrameUse::default()
    };
    let mut pos = 0usize;
    while pos < method.bytecode.len() {
        let opcode = method.bytecode[pos];
        let next = method.bytecode.get(pos + 1).copied();
        if !is_known_instruction(opcode, next) {
            out.fully_decoded = false;
            break;
        }
        let len = instruction_len(opcode, next).max(1);
        if pos + len > method.bytecode.len() {
            out.fully_decoded = false;
            break;
        }

        for operand in register_operands(&method.bytecode, pos, opcode, next) {
            if let Some(value) = read_register_operand(&method.bytecode, operand) {
                out.register_uses.push(RegUse { operand, value });
                out.frame_values.push(value);
            } else {
                out.fully_decoded = false;
            }
        }
        if let Some(range_use) = range_operand(&method.bytecode, pos, opcode, next) {
            out.range_uses.push(range_use);
            for index in 0..range_use.count {
                if let Some(value) = range_use.start.checked_add(index) {
                    out.frame_values.push(value);
                } else {
                    out.fully_decoded = false;
                }
            }
        }

        pos += len;
    }
    out
}

fn is_known_instruction(opcode: u8, next: Option<u8>) -> bool {
    matches!(
        opcode,
        0x00..=0x09
            | 0x0a..=0x1b
            | 0x1c..=0x1e
            | 0x1f..=0x22
            | 0x23..=0x24
            | 0x25..=0x28
            | 0x29..=0x32
            | 0x33..=0x40
            | 0x42..=0x46
            | 0x4d..=0x51
            | 0x60..=0x65
            | 0x66..=0x70
            | 0x73
            | 0x76..=0x7a
            | 0x7c..=0x80
            | 0x8a..=0x8b
            | 0x8e..=0x8f
            | 0x98..=0x9c
            | 0xa1
            | 0xb0..=0xb1
            | 0xb3..=0xb6
            | 0xb9..=0xbd
            | 0xbf..=0xc6
            | 0xc9
            | 0xcb
            | 0xcd..=0xce
            | 0xcf
            | 0xd0
            | 0xd3
            | 0xd5
            | 0xd8..=0xdc
    ) || matches!(
        (opcode, next),
        (0xfd, Some(0x00))
            | (0xfe, Some(0x00 | 0x02 | 0x05 | 0x07 | 0x09))
            | (0xfb, Some(0x01 | 0x04 | 0x05 | 0x06 | 0x13 | 0x14))
    )
}

fn register_operands(bytecode: &[u8], pos: usize, opcode: u8, next: Option<u8>) -> Vec<RegOperand> {
    let mut operands = match opcode {
        0x44 => vec![
            RegOperand {
                offset: pos + 1,
                width: RegWidth::U4Low,
            },
            RegOperand {
                offset: pos + 1,
                width: RegWidth::U4High,
            },
        ],
        0x60 | 0x61 | 0x36 | 0xb1 | 0xc2 | 0xc3 | 0xc4 | 0xc5 | 0xcd | 0xce => reg8(pos, &[1]),
        0x45 => reg8(pos, &[1, 2]),
        0x8f => vec![
            RegOperand {
                offset: pos + 1,
                width: RegWidth::U16,
            },
            RegOperand {
                offset: pos + 3,
                width: RegWidth::U16,
            },
        ],
        0x0a..=0x1b | 0x25..=0x28 | 0x2a | 0x2d | 0x37 | 0x39 | 0x68 | 0x77 | 0xb4 | 0xb9 => {
            reg8(pos, &[2])
        }
        0x2b | 0x2e | 0x38 | 0x78 | 0x99 | 0xba | 0xc9 => reg8(pos, &[2, 3]),
        0x2c | 0x2f => reg8(pos, &[2, 3, 4]),
        0x30 => reg8(pos, &[2, 3, 4, 5]),
        0x08 | 0x31 | 0x32 | 0x73 => reg8(pos, &[3]),
        0x35 => reg8(pos, &[8]),
        0x43 | 0x7a | 0x8e | 0xd0 | 0xdb | 0xdc => reg8(pos, &[4]),
        0xbc => reg8(pos, &[1, 2, 3, 4]),
        0x79 => reg8(pos, &[2]),
        0xcb => reg8(pos, &[3]),
        0xc6 => reg8(pos, &[1, 2]),
        0xd9 => reg8(pos, &[6]),
        0xfd if next == Some(0x00) => reg8(pos, &[4, 5]),
        0xfe if next == Some(0x05) => reg8(pos, &[2]),
        0xfb if next == Some(0x01) => reg8(pos, &[3, 4]),
        0xfb if next == Some(0x05) => reg8(pos, &[7]),
        0xfb if next == Some(0x06) => reg8(pos, &[3]),
        _ => Vec::new(),
    };
    operands.retain(|operand| operand.offset < bytecode.len());
    operands
}

fn reg8(pos: usize, offsets: &[usize]) -> Vec<RegOperand> {
    offsets
        .iter()
        .map(|offset| RegOperand {
            offset: pos + offset,
            width: RegWidth::U8,
        })
        .collect()
}

fn range_operand(bytecode: &[u8], pos: usize, opcode: u8, next: Option<u8>) -> Option<RangeUse> {
    let (count_offset, start_offset) = match opcode {
        0x08 | 0x31 | 0x32 | 0x73 => (2, 3),
        0xb3 => (1, 3),
        0xfd if next == Some(0x00) => {
            let count = read_u16(bytecode, pos + 2)?;
            let operand = RegOperand {
                offset: pos + 5,
                width: RegWidth::U8,
            };
            let start = read_register_operand(bytecode, operand)?;
            return Some(RangeUse {
                start_operand: operand,
                start,
                count,
            });
        }
        _ => return None,
    };
    let count = *bytecode.get(pos + count_offset)? as u16;
    let operand = RegOperand {
        offset: pos + start_offset,
        width: RegWidth::U8,
    };
    let start = read_register_operand(bytecode, operand)?;
    Some(RangeUse {
        start_operand: operand,
        start,
        count,
    })
}

fn read_register_operand(bytecode: &[u8], operand: RegOperand) -> Option<u16> {
    match operand.width {
        RegWidth::U4Low => bytecode
            .get(operand.offset)
            .map(|byte| (byte & 0x0f) as u16),
        RegWidth::U4High => bytecode.get(operand.offset).map(|byte| (byte >> 4) as u16),
        RegWidth::U8 => bytecode.get(operand.offset).map(|byte| *byte as u16),
        RegWidth::U16 => read_u16(bytecode, operand.offset),
    }
}

fn register_operand_can_encode(operand: RegOperand, value: u16) -> bool {
    match operand.width {
        RegWidth::U4Low | RegWidth::U4High => value <= 0x0f,
        RegWidth::U8 => value <= u8::MAX as u16,
        RegWidth::U16 => true,
    }
}

fn write_register_operand(bytecode: &mut [u8], operand: RegOperand, value: u16) {
    match operand.width {
        RegWidth::U4Low => {
            if let Some(byte) = bytecode.get_mut(operand.offset) {
                *byte = (*byte & 0xf0) | ((value as u8) & 0x0f);
            }
        }
        RegWidth::U4High => {
            if let Some(byte) = bytecode.get_mut(operand.offset) {
                *byte = (*byte & 0x0f) | (((value as u8) & 0x0f) << 4);
            }
        }
        RegWidth::U8 => {
            if let Some(byte) = bytecode.get_mut(operand.offset) {
                *byte = value as u8;
            }
        }
        RegWidth::U16 => {
            if operand.offset + 1 < bytecode.len() {
                bytecode[operand.offset..operand.offset + 2].copy_from_slice(&value.to_le_bytes());
            }
        }
    }
}

fn read_u16(bytecode: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes([
        *bytecode.get(offset)?,
        *bytecode.get(offset + 1)?,
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emit::record::{function_kind, MethodKind, MethodLabelStyle, MethodRecord};

    fn method(bytecode: Vec<u8>, num_vregs: u16, num_args: u16) -> MethodRecord {
        MethodRecord {
            name: "func_main_0".to_string(),
            bytecode,
            pandasm_insns: Vec::new(),
            pandasm_debug: Vec::new(),
            num_vregs,
            num_args,
            value_of_first_param: -1,
            slots_num: None,
            debug_info: None,
            line_ops: vec![],
            catch_entries: vec![],
            program_labels: vec![],
            label_table: vec![],
            kind: MethodKind::Module,
            function_kind: function_kind::FUNCTION,
            source_lang: None,
            concurrent_module_requests: Vec::new(),
            method_pool_entry: false,
            expected_property_count: None,
            annotations: Vec::new(),
            metadata_attrs: Default::default(),
            param_annotations: Vec::new(),
            definefunc_order: None,
            label_style: MethodLabelStyle::Source,
            param_types: Vec::new(),
            return_type: None,
            body_presence: None,
            body_location: None,
            file_location: None,
        }
    }

    #[test]
    fn compact_register_frame_rewrites_argument_registers() {
        let mut method = method(vec![0x60, 10, 0x61, 2, 0x45, 3, 11, 0x65], 10, 3);

        compact_register_frame(&mut method);

        assert_eq!(method.num_vregs, 2);
        assert_eq!(method.bytecode, vec![0x60, 2, 0x61, 0, 0x45, 1, 3, 0x65]);
    }

    #[test]
    fn compact_register_frame_rewrites_local_arg_crossing_range_start() {
        let mut method = method(vec![0x73, 0, 4, 8, 0x65], 10, 3);

        compact_register_frame(&mut method);

        assert_eq!(method.num_vregs, 2);
        assert_eq!(method.bytecode, vec![0x73, 0, 4, 0, 0x65]);
    }

    #[test]
    fn compact_register_frame_rewrites_pure_argument_range_start() {
        let mut method = method(vec![0x73, 0, 2, 10, 0x65], 10, 3);

        compact_register_frame(&mut method);

        assert_eq!(method.num_vregs, 0);
        assert_eq!(method.bytecode, vec![0x73, 0, 2, 0, 0x65]);
    }

    #[test]
    fn optimize_file_optimizes_eh_methods_by_default() {
        let mut method = method(vec![0x60, 10, 0x65], 10, 3);
        method.catch_entries.push(crate::emit::record::CatchEntry {
            try_begin: 0,
            try_end: 1,
            catch_begin: 1,
            catch_end: 2,
            exception_record: None,
        });
        let mut file = AbcFile::new();
        file.records.push(crate::emit::record::ClassRecord {
            name: "test".to_string(),
            methods: vec![method],
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
            fields: vec![],
        });

        optimize_file(
            &mut file,
            BytecodeOptimizerOptions::from_compile_options(2, true),
        );

        assert_eq!(file.records[0].methods[0].num_vregs, 0);
        assert_eq!(file.records[0].methods[0].bytecode, vec![0x60, 0, 0x65]);
    }

    #[test]
    fn optimize_file_honors_disabled_try_catch_optimization() {
        let mut method = method(vec![0x60, 10, 0x65], 10, 3);
        method.catch_entries.push(crate::emit::record::CatchEntry {
            try_begin: 0,
            try_end: 1,
            catch_begin: 1,
            catch_end: 2,
            exception_record: None,
        });
        let mut file = AbcFile::new();
        file.records.push(crate::emit::record::ClassRecord {
            name: "test".to_string(),
            methods: vec![method],
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
            fields: vec![],
        });

        optimize_file(
            &mut file,
            BytecodeOptimizerOptions::from_compile_options(2, false),
        );

        assert_eq!(file.records[0].methods[0].num_vregs, 10);
        assert_eq!(file.records[0].methods[0].bytecode, vec![0x60, 10, 0x65]);
    }
}
