/// Conservatively estimate the vreg count from bytecode length.
/// Real register allocation is a SPEC-009 follow-up.
pub(super) fn estimate_vregs(bytecode: &[u8]) -> u16 {
    max_vreg_count(bytecode).clamp(4, 64)
}

fn max_vreg_count(bytecode: &[u8]) -> u16 {
    let mut max_reg = None::<u16>;
    let mut pos = 0usize;
    while pos < bytecode.len() {
        let opcode = bytecode[pos];
        let mut note = |reg: u16| {
            max_reg = Some(max_reg.map_or(reg, |prev| prev.max(reg)));
        };

        match opcode {
            0x0a..=0x1a | 0x25 | 0x27 | 0x28 | 0x2a => {
                if let Some(&reg) = bytecode.get(pos + 2) {
                    note(reg as u16);
                }
            }
            0x2b | 0x2e => {
                for off in 2..=3 {
                    if let Some(&reg) = bytecode.get(pos + off) {
                        note(reg as u16);
                    }
                }
            }
            0x2c | 0x2f => {
                for off in 2..=4 {
                    if let Some(&reg) = bytecode.get(pos + off) {
                        note(reg as u16);
                    }
                }
            }
            0x30 => {
                for off in 2..=5 {
                    if let Some(&reg) = bytecode.get(pos + off) {
                        note(reg as u16);
                    }
                }
            }
            0x08 | 0x31 | 0x32 | 0x73 => {
                if let (Some(&count), Some(&start)) = (bytecode.get(pos + 2), bytecode.get(pos + 3))
                {
                    note(start as u16 + count.saturating_sub(1) as u16);
                }
            }
            0x2d => {
                if let Some(&reg) = bytecode.get(pos + 2) {
                    note(reg as u16);
                }
            }
            0xbc => {
                for off in 1..=4 {
                    if let Some(&reg) = bytecode.get(pos + off) {
                        note(reg as u16);
                    }
                }
            }
            0x35 => {
                if let Some(&reg) = bytecode.get(pos + 8) {
                    note(reg as u16);
                }
            }
            0x36 | 0x60 | 0x61 | 0x68 | 0xc0 | 0xc2 | 0xcf => {
                if let Some(&reg) = bytecode.get(pos + 1) {
                    note(reg as u16);
                }
            }
            0x43 | 0x7a => {
                if let Some(&reg) = bytecode.get(pos + 4) {
                    note(reg as u16);
                }
            }
            0x44 => {
                if let Some(&packed) = bytecode.get(pos + 1) {
                    note((packed & 0x0f) as u16);
                    note((packed >> 4) as u16);
                }
            }
            0x45 => {
                for off in 1..=2 {
                    if let Some(&reg) = bytecode.get(pos + off) {
                        note(reg as u16);
                    }
                }
            }
            0x8f => {
                if bytecode.len() >= pos + 5 {
                    note(u16::from_le_bytes([bytecode[pos + 1], bytecode[pos + 2]]));
                    note(u16::from_le_bytes([bytecode[pos + 3], bytecode[pos + 4]]));
                }
            }
            0xb9 => {
                if let Some(&reg) = bytecode.get(pos + 2) {
                    note(reg as u16);
                }
            }
            _ => {}
        }

        pos += bytecode_instruction_len(opcode, bytecode.get(pos + 1).copied()).max(1);
    }
    max_reg.map_or(0, |reg| reg + 1)
}

fn bytecode_instruction_len(opcode: u8, next: Option<u8>) -> usize {
    match opcode {
        0x00..=0x03 | 0x64 | 0x65 | 0x66 | 0xbb => 1,
        0x29 | 0x36 | 0x3c | 0x3d | 0x44 | 0x4d | 0x4f | 0x51 | 0x60 | 0x61 | 0x67 | 0x7c
        | 0x7e | 0xc0 | 0xc2 | 0xcf => 2,
        0x0a..=0x1a
        | 0x25
        | 0x27
        | 0x28
        | 0x2a
        | 0x2d
        | 0x3e
        | 0x45
        | 0x4e
        | 0x50
        | 0x68
        | 0x9b
        | 0xbd => 3,
        0x07 | 0x08 | 0x2b | 0x2e | 0x31 | 0x32 | 0x3f | 0x40 | 0x42 | 0x73 | 0xb6 | 0xc1 => 4,
        0x2c | 0x33 | 0x34 | 0x43 | 0x62 | 0x7a | 0x8f | 0x98 | 0x9a | 0x9c | 0xbc => 5,
        0x2f => 5,
        0x30 => 6,
        0x35 | 0x63 => 9,
        0xb9 => 3,
        0xfe if next == Some(0x00) => 2,
        0xfe if next == Some(0x07) => 3,
        0xfe if next == Some(0x09) => 4,
        0xfb if matches!(next, Some(0x13 | 0x14)) => 3,
        _ => 1,
    }
}
