/// Panda ISA instruction set representation.
///
/// Each variant maps to one logical Panda bytecode instruction.  The `encode`
/// method serialises a single instruction into the byte sequence used in an
/// `.abc` method body.
///
/// ## Opcode bytes
///
/// The byte values below are *approximations* derived from the Panda ISA
/// specification (`arkcompiler_runtime_core/isa/isa.yaml`).  They must be
/// verified against real `es2abc` output before the compiler is considered
/// production-ready.  Where verification is pending, a `// VERIFY` comment
/// marks the line.
#[derive(Debug, Clone, PartialEq)]
pub enum Insn {
    /// Load `undefined` into the accumulator.
    LdUndefined,
    /// Load `null` into the accumulator.
    LdNull,
    /// Load `true` into the accumulator.
    LdTrue,
    /// Load `false` into the accumulator.
    LdFalse,
    /// Load a small integer immediate into the accumulator.
    LdAi(i64),
    /// Load a double-precision float immediate into the accumulator.
    FLdAi(f64),
    /// Load a string (referenced by its string-table index) into the accumulator.
    LdaStr(u32),
    /// Load a value from virtual register `vreg` into the accumulator.
    Lda(u16),
    /// Store the accumulator into virtual register `vreg`.
    Sta(u16),
    /// `acc = acc + vreg`
    Add2(u16),
    /// `acc = acc - vreg`
    Sub2(u16),
    /// `acc = acc * vreg`
    Mul2(u16),
    /// `acc = acc / vreg`
    Div2(u16),
    /// `acc = (acc == vreg)`
    Eq(u16),
    /// `acc = (acc === vreg)` (strict equality)
    Stricteq(u16),
    /// Load `globalThis[name_str_idx]` into the accumulator.
    LdGlobalVar(u32),
    /// Store the accumulator into `globalThis[name_str_idx]`.
    StGlobalVar(u32),
    /// Call a method with a short call form (≤ 4 args).
    CallShort { method: u32, args: Vec<u16> },
    /// Call a method with an explicit `this` receiver.
    CallThis { method: u32, this: u16, args: Vec<u16> },
    /// Return the accumulator value.
    Return,
    /// Return `undefined` (pop frame without returning a value).
    ReturnUndefined,
    /// Unconditional jump by a signed 32-bit byte offset.
    JmpImm(i32),
    /// Jump if accumulator equals zero (or is falsy).
    Jeqz(i32),
    /// Jump if accumulator is not zero (or is truthy).
    Jnez(i32),
    /// Throw the value in the accumulator.
    Throw,
    /// Construct a new object instance.
    NewObj { class: u32, args: Vec<u16> },
    /// Load `obj.prop` by name string index into the accumulator; `obj` is in `obj_vreg`.
    GetPropByName(u16, u32),
    /// Store the accumulator into `obj.prop`; `obj` is in `obj_vreg`.
    SetPropByName(u16, u32),
    // TODO: add further opcodes as the code generator grows:
    //   - typeof, instanceof, in
    //   - array/object literals
    //   - try/catch/finally
    //   - class definition opcodes
    //   - async/await
}

impl Insn {
    /// Encode this instruction into its binary representation.
    ///
    /// `_strings` is passed for instructions that encode string-table indices
    /// inline; most instructions ignore it.
    ///
    /// The byte layout of multi-byte operands is little-endian throughout.
    ///
    /// VERIFY: Every opcode byte below must be confirmed against
    /// `arkcompiler_runtime_core/isa/isa.yaml` once integration tests with
    /// real ArkRuntime are available.
    pub fn encode(&self, _strings: &mut crate::emit::strings::StringTable) -> Vec<u8> {
        match self {
            // --- accumulatore loads ---
            Insn::LdUndefined  => vec![0x02],                          // VERIFY
            Insn::LdNull       => vec![0x03],                          // VERIFY
            Insn::LdTrue       => vec![0x04],                          // VERIFY
            Insn::LdFalse      => vec![0x05],                          // VERIFY

            // --- integer immediate ---
            Insn::LdAi(n) => {
                let mut v = vec![0x08];                                // VERIFY
                v.extend_from_slice(&(*n as i32).to_le_bytes());
                v
            }

            // --- float immediate ---
            // TODO: encode f64 immediate properly once opcode byte is verified.
            Insn::FLdAi(_f) => vec![0x02], // stub: ldundefined as no-op placeholder

            // --- string literal ---
            Insn::LdaStr(idx) => {
                let mut v = vec![0x0E];                                // VERIFY
                v.extend_from_slice(&idx.to_le_bytes());
                v
            }

            // --- vreg moves ---
            Insn::Lda(vreg) => {
                let mut v = vec![0x09];                                // VERIFY
                v.extend_from_slice(&vreg.to_le_bytes());
                v
            }
            Insn::Sta(vreg) => {
                let mut v = vec![0x0A];                                // VERIFY
                v.extend_from_slice(&vreg.to_le_bytes());
                v
            }

            // --- arithmetic ---
            // TODO: encode arithmetic opcodes once byte values are verified.
            Insn::Add2(_) | Insn::Sub2(_) | Insn::Mul2(_) | Insn::Div2(_) => {
                vec![0x02] // stub
            }

            // --- comparisons ---
            Insn::Eq(_) | Insn::Stricteq(_) => vec![0x02], // stub

            // --- global variable access ---
            Insn::LdGlobalVar(idx) => {
                let mut v = vec![0x40];                                // VERIFY
                v.extend_from_slice(&idx.to_le_bytes());
                v
            }
            Insn::StGlobalVar(idx) => {
                let mut v = vec![0x41];                                // VERIFY
                v.extend_from_slice(&idx.to_le_bytes());
                v
            }

            // --- returns ---
            Insn::Return          => vec![0x24],                       // VERIFY
            Insn::ReturnUndefined => vec![0x25],                       // VERIFY

            // --- calls ---
            // TODO: encode call opcodes once byte values are verified.
            Insn::CallShort { .. } | Insn::CallThis { .. } => vec![0x02], // stub

            // --- jumps ---
            // TODO: encode jump opcodes once byte values are verified.
            Insn::JmpImm(_) | Insn::Jeqz(_) | Insn::Jnez(_) => vec![0x02], // stub

            // --- misc ---
            Insn::Throw => vec![0x02], // stub
            Insn::NewObj { .. } => vec![0x02], // stub
            Insn::GetPropByName(_, _) | Insn::SetPropByName(_, _) => vec![0x02], // stub
        }
    }
}
