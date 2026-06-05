use oxc_ast::ast::{Argument, Expression};
use oxc_span::GetSpan;

/// Encode `value` as an unsigned LEB-128 byte sequence (appended to `buf`).
fn encode_uleb128(mut value: u64, buf: &mut Vec<u8>) {
    loop {
        let byte = (value & 0x7F) as u8;
        value >>= 7;
        if value == 0 {
            buf.push(byte);
            break;
        } else {
            buf.push(byte | 0x80);
        }
    }
}

/// Encode `value` as a signed LEB-128 byte sequence (appended to `buf`).
fn encode_sleb128(mut value: i32, buf: &mut Vec<u8>) {
    let mut more = true;
    while more {
        let byte = (value & 0x7F) as u8;
        value >>= 7;
        more = !(((value == 0) && (byte & 0x40 == 0)) || ((value == -1) && (byte & 0x40 != 0)));
        buf.push(if more { byte | 0x80 } else { byte });
    }
}

/// Return the 0-based line number corresponding to `byte_offset` in `source`.
///
/// es2panda stores `lexer::SourcePosition::line` directly in
/// `pandaIns->ins_debug.line_number`; that value is 0-based. The assembler
/// then computes LNP deltas from the first instruction's debug line.
pub(super) fn line_at(source: &str, byte_offset: u32) -> u32 {
    let end = (byte_offset as usize).min(source.len());
    source[..end].bytes().filter(|&b| b == b'\n').count() as u32
}

/// Return the 0-based column number corresponding to `byte_offset` in `source`.
pub(super) fn col_at(source: &str, byte_offset: u32) -> u32 {
    let end = (byte_offset as usize).min(source.len());
    let last_nl = source[..end].rfind('\n').map_or(0, |p| p + 1);
    (end - last_nl) as u32
}

pub(super) fn argument_span_start(arg: &Argument<'_>) -> u32 {
    if let Some(expr) = arg.as_expression() {
        if let Expression::CallExpression(call) = expr {
            return call.callee.span().start;
        }
        return expr.span().start;
    }
    arg.span().start
}

#[derive(Debug, Clone)]
struct SourceMapSegment {
    generated_col: u32,
    source_line: u32,
    source_col: u32,
}

#[derive(Debug, Clone, Default)]
pub(super) struct SourceMapper {
    lines: Vec<Vec<SourceMapSegment>>,
}

impl SourceMapper {
    pub(super) fn from_json(map: Option<&str>) -> Option<Self> {
        let value: serde_json::Value = serde_json::from_str(map?).ok()?;
        let mappings = value.get("mappings")?.as_str()?;
        let mut lines = Vec::new();
        let mut prev_source_idx = 0i32;
        let mut prev_source_line = 0i32;
        let mut prev_source_col = 0i32;
        for line in mappings.split(';') {
            let mut segments = Vec::new();
            let mut prev_generated_col = 0i32;
            for raw_segment in line.split(',').filter(|segment| !segment.is_empty()) {
                let fields = decode_vlq_segment(raw_segment)?;
                if fields.len() < 4 {
                    continue;
                }
                prev_generated_col += fields[0];
                prev_source_idx += fields[1];
                prev_source_line += fields[2];
                prev_source_col += fields[3];
                if prev_source_idx < 0 || prev_source_line < 0 || prev_source_col < 0 {
                    continue;
                }
                segments.push(SourceMapSegment {
                    generated_col: prev_generated_col as u32,
                    source_line: prev_source_line as u32,
                    source_col: prev_source_col as u32,
                });
            }
            lines.push(segments);
        }
        Some(Self { lines })
    }

    pub(super) fn map_offset(&self, source: &str, byte_offset: u32) -> Option<(u32, u32)> {
        let gen_line = line_at(source, byte_offset);
        let gen_col = col_at(source, byte_offset);
        let segments = self.lines.get(gen_line as usize)?;
        let segment = segments
            .iter()
            .take_while(|segment| segment.generated_col <= gen_col)
            .last()?;
        let col_delta = gen_col.saturating_sub(segment.generated_col);
        Some((segment.source_line, segment.source_col + col_delta))
    }
}

fn decode_vlq_segment(segment: &str) -> Option<Vec<i32>> {
    let mut values = Vec::new();
    let mut value = 0u32;
    let mut shift = 0u32;
    for ch in segment.chars() {
        let mut digit = source_map_base64_value(ch)?;
        let continuation = digit & 0x20 != 0;
        digit &= 0x1f;
        value += digit << shift;
        if continuation {
            shift += 5;
            continue;
        }
        let negative = value & 1 != 0;
        let magnitude = (value >> 1) as i32;
        values.push(if negative { -magnitude } else { magnitude });
        value = 0;
        shift = 0;
    }
    if shift == 0 {
        Some(values)
    } else {
        None
    }
}

fn source_map_base64_value(ch: char) -> Option<u32> {
    match ch {
        'A'..='Z' => Some(ch as u32 - 'A' as u32),
        'a'..='z' => Some(ch as u32 - 'a' as u32 + 26),
        '0'..='9' => Some(ch as u32 - '0' as u32 + 52),
        '+' => Some(62),
        '/' => Some(63),
        _ => None,
    }
}

/// Accumulates Panda LineNumberProgram opcodes for a single method.
///
/// Implements the exact algorithm from libpandafile `BuildLineNumberProgram`:
/// - `EmitLineNumber`: try SPECIAL; else ADV_LINE + SP(0,0)
/// - `EmitColumnNumber`: flush pending ADV_PC, then SET_COLUMN
/// - Per-instruction: EmitLineNumber + EmitColumnNumber + pc_inc += insn_size
///
/// LNP encoding constants (from libpandafile/file_items.h):
/// - `OPCODE_BASE = 0x0C` (12)
/// - `LINE_BASE = -4`
/// - `LINE_RANGE = 15`
///
/// Architecture: opcodes go to `data_` (ops), operands go to `constant_pool` (cp).
/// Format of `finish()` output: [ops][0x00=END][cp_len:u32LE][cp]
pub(crate) struct LineTracker {
    start_line: u32,
    current_line: u32,
    current_pc: u32,
    /// Pending PC increment (accumulated since last column/line emit).
    pending_pc: u32,
    /// Previous column number (u32::MAX = not yet set).
    prev_col: u32,
    /// LNP data_ stream: opcode bytes + inline reg SLEB128s.
    ops: Vec<u8>,
    /// DebugInfo constant_pool: ULEB128/SLEB128 operands for opcodes.
    cp: Vec<u8>,
}

const LNP_OPCODE_BASE: u32 = 0x0C;
const LNP_LINE_BASE: i32 = -4;
const LNP_LINE_RANGE: u32 = 15;

impl LineTracker {
    pub(crate) fn new(start_line: u32) -> Self {
        Self {
            start_line,
            current_line: start_line,
            current_pc: 0,
            pending_pc: 0,
            prev_col: u32::MAX,
            ops: Vec::new(),
            cp: Vec::new(),
        }
    }

    pub(crate) fn start_line(&self) -> u32 {
        self.start_line
    }

    /// Emit SPECIAL(pc_inc, line_inc) if encodable; returns true on success.
    fn try_special(&mut self, pc_inc: u32, line_inc: i32) -> bool {
        let adj = line_inc - LNP_LINE_BASE;
        if adj >= 0 && (adj as u32) < LNP_LINE_RANGE {
            let op = pc_inc * LNP_LINE_RANGE + adj as u32 + LNP_OPCODE_BASE;
            if op <= 255 {
                self.ops.push(op as u8);
                self.current_pc += pc_inc;
                self.current_line = (self.current_line as i32 + line_inc) as u32;
                return true;
            }
        }
        false
    }

    /// Emit ADV_PC(n): opcode to data_, operand to cp.
    fn emit_adv_pc(&mut self, n: u32) {
        if n == 0 {
            return;
        }
        self.ops.push(0x01);
        encode_uleb128(n as u64, &mut self.cp);
        self.current_pc += n;
    }

    /// Emit ADV_LINE(n): opcode to data_, operand to cp.
    fn emit_adv_line(&mut self, n: i32) {
        self.ops.push(0x02);
        encode_sleb128(n, &mut self.cp);
        self.current_line = (self.current_line as i32 + n) as u32;
    }

    /// EmitNumber from libpandafile: try SPECIAL; else ADV_PC+SPECIAL or ADV_LINE+SP(0,0).
    fn emit_number(&mut self, pc_inc: u32, line_inc: i32) {
        if !self.try_special(pc_inc, line_inc) {
            if pc_inc > 0 {
                self.emit_adv_pc(pc_inc);
                if !self.try_special(0, line_inc) {
                    self.emit_adv_line(line_inc);
                    self.try_special(0, 0);
                }
            } else {
                self.emit_adv_line(line_inc);
                self.try_special(0, 0);
            }
        }
    }

    /// EmitColumn: flush pending ADV_PC, then emit SET_COLUMN if column changed.
    fn emit_column(&mut self, col: u32) {
        if col == self.prev_col {
            return;
        }
        if self.pending_pc > 0 {
            self.emit_adv_pc(self.pending_pc);
            self.pending_pc = 0;
        }
        self.ops.push(0x0b);
        encode_uleb128(col as u64, &mut self.cp);
        self.prev_col = col;
    }

    pub(crate) fn emit_epilogue_column_reset(&mut self) {
        self.prev_col = 0;
        self.emit_column(u32::MAX);
    }

    /// Per-instruction advance: EmitLineNumber + EmitColumnNumber + accumulate pc.
    /// Matches libpandafile BuildLineNumberProgram inner loop exactly.
    pub(crate) fn advance_with_col(&mut self, new_line: u32, col: u32, insn_size: u32) {
        let line_inc = new_line as i32 - self.current_line as i32;
        if line_inc != 0 {
            self.emit_number(self.pending_pc, line_inc);
            self.pending_pc = 0;
        }
        self.emit_column(col);
        self.pending_pc += insn_size;
    }

    /// Per-statement advance (no column). Kept for compatibility.
    #[allow(dead_code)]
    fn advance(&mut self, new_pc: u32, new_line: u32) {
        let pc_delta = new_pc.saturating_sub(self.current_pc);
        let line_delta = new_line as i32 - self.current_line as i32;
        if pc_delta == 0 && line_delta == 0 {
            return;
        }
        self.pending_pc += pc_delta;
        if line_delta != 0 {
            self.emit_number(self.pending_pc, line_delta);
            self.pending_pc = 0;
        }
    }

    /// Finish and return encoded bytes: [ops][0x00=END][cp_len:u32LE][cp].
    ///
    /// Mirrors libpandafile `BuildLineNumberProgram`: after the last
    /// instruction it emits only END_SEQUENCE; it does not flush the final
    /// accumulated pc_inc.
    pub(crate) fn finish(self) -> Vec<u8> {
        let mut self_ = self;
        self_.ops.push(0x00);
        let cp_len = self_.cp.len() as u32;
        self_.ops.extend_from_slice(&cp_len.to_le_bytes());
        self_.ops.extend_from_slice(&self_.cp);
        self_.ops
    }

    #[allow(dead_code)]
    pub(super) fn finish_without_final_pc(self) -> Vec<u8> {
        self.finish()
    }
}
