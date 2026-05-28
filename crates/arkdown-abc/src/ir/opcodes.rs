/// Panda ISA instruction set — opcode bytes verified against
/// `arkcompiler_runtime_core/isa/isa.yaml` (API-24 branch, fetched 2026-05).
///
/// Every opcode value, operand width, and byte layout below was cross-checked
/// against the real `isa.yaml` from
/// `https://raw.githubusercontent.com/openharmony/arkcompiler_runtime_core/master/isa/isa.yaml`
/// and spot-verified against a real `modules.abc` produced by DevEco Studio /
/// es2abc for HarmonyOS API 12.
///
/// ## Instruction encoding
///
/// All multi-byte operands are **little-endian**.  The total byte length of each
/// encoded instruction is given in the doc comment for the corresponding variant.
///
/// ### Prefix bytes
///
/// Certain instruction groups use a one-byte prefix before the opcode:
/// - `0xFE` – `throw` family
/// - `0xFD` – `wide` forms (for operands that exceed 8-bit registers)
/// - `0xFC` – `deprecated` forms
/// - `0xFB` – `callruntime` forms
///
/// ### Inline-cache (IC) slot operand
///
/// Many call/access instructions carry an `imm` (IC slot) operand that the JIT
/// uses for inline caching.  On first call this should be 0; the runtime fills
/// it in dynamically.  Our compiler always emits `0` for this field.
#[derive(Debug, Clone, PartialEq)]
pub enum Insn {
    // ── Constant loaders (1 byte) ────────────────────────────────────────────
    /// `0x00` `op_none` — load `undefined` into accumulator.
    LdUndefined,
    /// `0x01` `op_none` — load `null` into accumulator.
    LdNull,
    /// `0x02` `op_none` — load `true` into accumulator.
    LdTrue,
    /// `0x03` `op_none` — load `false` into accumulator.
    LdFalse,

    // ── Integer / float immediates ────────────────────────────────────────────
    /// `0x62` `op_imm_32` — load signed 32-bit integer into accumulator.
    /// Encoding: `[0x62] [i32 LE]` = 5 bytes.
    LdAi(i32),

    /// `0x63` `op_imm_64` — load 64-bit float into accumulator.
    /// Encoding: `[0x63] [f64 LE]` = 9 bytes.
    FLdAi(f64),

    // ── String load ──────────────────────────────────────────────────────────
    /// `0x3E` `op_id_16` — load string (by 16-bit string index) into accumulator.
    /// Encoding: `[0x3E] [string_id: u16 LE]` = 3 bytes.
    LdaStr(u16),

    // ── Register moves ───────────────────────────────────────────────────────
    /// `0x60` `op_v_8` — load virtual register `v` into accumulator.
    /// Encoding: `[0x60] [vreg: u8]` = 2 bytes.
    Lda(u8),

    /// `0x61` `op_v_8` — store accumulator into virtual register `v`.
    /// Encoding: `[0x61] [vreg: u8]` = 2 bytes.
    Sta(u8),

    /// `0x44` `op_v1_4_v2_4` — move `v2` into `v1`; both registers fit in 4 bits (0..=15).
    /// Encoding: `[0x44] [(v2 << 4) | v1]` = 2 bytes.
    MovV4(u8, u8),

    /// `0x45` `op_v1_8_v2_8` — move `v2` into `v1`; 8-bit register numbers.
    /// Encoding: `[0x45] [v1: u8] [v2: u8]` = 3 bytes.
    MovV8(u8, u8),

    /// `0x8F` `op_v1_16_v2_16` — move `v2` into `v1`; 16-bit register numbers.
    /// Encoding: `[0x8F] [v1: u16 LE] [v2: u16 LE]` = 5 bytes.
    MovV16(u16, u16),

    // ── Arithmetic (accumulator op register) ─────────────────────────────────
    // Format: `op_imm_8_v_8` — opcode(1) + IC-slot(1) + vreg(1) = 3 bytes.
    /// `0x0A` — `acc = acc + v[vreg]`.  `slot` is the IC slot (pass 0).
    Add2(u8, u8),
    /// `0x0B` — `acc = acc - v[vreg]`.
    Sub2(u8, u8),
    /// `0x0C` — `acc = acc * v[vreg]`.
    Mul2(u8, u8),
    /// `0x0D` — `acc = acc / v[vreg]`.
    Div2(u8, u8),
    /// `0x0E` — `acc = acc % v[vreg]`.
    Mod2(u8, u8),

    // ── Equality / comparison (accumulator op register) ───────────────────────
    // Format: `op_imm_8_v_8` — opcode(1) + slot(1) + vreg(1) = 3 bytes.
    // Semantics: `acc = acc OP v[vreg]`.
    /// `0x0F` — `acc = acc == v[vreg]` (abstract equality).
    Eq(u8, u8),
    /// `0x10` — `acc = acc != v[vreg]` (abstract inequality).
    NotEq(u8, u8),
    /// `0x11` — `acc = acc < v[vreg]`.
    Less(u8, u8),
    /// `0x12` — `acc = acc <= v[vreg]`.
    LessEq(u8, u8),
    /// `0x13` — `acc = acc > v[vreg]`.
    Greater(u8, u8),
    /// `0x14` — `acc = acc >= v[vreg]`.
    GreaterEq(u8, u8),

    // ── Bitwise binary ops ────────────────────────────────────────────────────
    /// `0x15` — `acc = acc << v[vreg]`.
    Shl2(u8, u8),
    /// `0x16` — `acc = acc >> v[vreg]`.
    Shr2(u8, u8),
    /// `0x17` — `acc = acc >>> v[vreg]`.
    Ashr2(u8, u8),
    /// `0x18` — `acc = acc & v[vreg]`.
    And2(u8, u8),
    /// `0x19` — `acc = acc | v[vreg]`.
    Or2(u8, u8),
    /// `0x1A` — `acc = acc ^ v[vreg]`.
    Xor2(u8, u8),

    // ── Unary operators ───────────────────────────────────────────────────────
    // Format: `op_imm_8` — opcode(1) + slot(1) = 2 bytes. Operand is acc.
    /// `0x1C` — `acc = typeof acc` (returns a string like `"function"`).
    TypeOf(u8),
    /// `0x1F` — `acc = -acc`.
    Neg(u8),
    /// `0x20` — `acc = ~acc` (bitwise NOT).
    BitNot(u8),
    /// `0x21` — `acc = acc + 1`.
    Inc(u8),
    /// `0x22` — `acc = acc - 1`.
    Dec(u8),

    // ── `in` operator ─────────────────────────────────────────────────────────
    /// `0x25` `op_imm_8_v_8` — `acc = acc in v[vreg]`.
    /// Semantics: acc holds the property key; v[vreg] holds the object.
    /// Encoding: `[0x25] [slot: u8] [vreg: u8]` = 3 bytes.
    IsIn(u8, u8),

    // ── Strict equality / inequality ──────────────────────────────────────────
    /// `0x27` — `acc = acc !== v[vreg]`.
    StrictNotEq(u8, u8),
    /// `0x28` — `acc = acc === v[vreg]`.
    StrictEq(u8, u8),

    // ── Function calls (IC-slot first, then register operands) ───────────────
    /// `0x29` `op_imm_8` — call function in accumulator with 0 args.
    /// Encoding: `[0x29] [slot: u8]` = 2 bytes.
    CallArg0(u8),

    /// `0x2A` `op_imm_8_v_8` — call with 1 arg.
    /// Encoding: `[0x2A] [slot: u8] [v0: u8]` = 3 bytes.
    CallArg1(u8, u8),

    /// `0x2B` `op_imm_8_v1_8_v2_8` — call with 2 args.
    /// Encoding: `[0x2B] [slot: u8] [v0: u8] [v1: u8]` = 4 bytes.
    CallArgs2(u8, u8, u8),

    /// `0x2C` `op_imm_8_v1_8_v2_8_v3_8` — call with 3 args.
    /// Encoding: `[0x2C] [slot: u8] [v0: u8] [v1: u8] [v2: u8]` = 5 bytes.
    CallArgs3(u8, u8, u8, u8),

    /// `0x73` `op_imm1_8_imm2_8_v_8` — call with range of args.
    /// `argc` = number of args; `v_start` = first arg register.
    /// Encoding: `[0x73] [slot: u8] [argc: u8] [v_start: u8]` = 4 bytes.
    CallRange(u8, u8, u8),

    /// `0x2D` `op_imm_8_v_8` — call method on `this` with 0 args.
    /// Accumulator holds the function; `this_vreg` holds the receiver.
    /// Encoding: `[0x2D] [slot: u8] [this_vreg: u8]` = 3 bytes.
    CallThis0(u8, u8),

    /// `0x2E` `op_imm_8_v1_8_v2_8` — call method with 1 arg.
    /// Encoding: `[0x2E] [slot: u8] [this: u8] [v0: u8]` = 4 bytes.
    CallThis1(u8, u8, u8),

    /// `0x2F` `op_imm_8_v1_8_v2_8_v3_8` — call method with 2 args.
    /// Encoding: `[0x2F] [slot: u8] [this: u8] [v0: u8] [v1: u8]` = 5 bytes.
    CallThis2(u8, u8, u8, u8),

    /// `0x30` — call method with 3 args.
    /// Encoding: `[0x30] [slot: u8] [this: u8] [v0: u8] [v1: u8] [v2: u8]` = 6 bytes.
    CallThis3(u8, u8, u8, u8, u8),

    /// `0x31` `op_imm1_8_imm2_8_v_8` — call method with range of args.
    /// `this_vreg` is the first register in the range (receiver); subsequent
    /// registers are the arguments.
    /// Encoding: `[0x31] [slot: u8] [argc: u8] [this_vreg: u8]` = 4 bytes.
    CallThisRange(u8, u8, u8),

    // ── Global variable access ────────────────────────────────────────────────
    // Short form (imm=u8): opcode(1) + slot(1) + name_id(2) = 4 bytes.
    /// `0x3F` — load `globalThis.<name>` into accumulator.
    /// `name_id` is a 16-bit index into the string table.
    TryLdGlobalByName(u8, u16),

    /// `0x40` — store accumulator into `globalThis.<name>`.
    TryStGlobalByName(u8, u16),

    /// `0x7E` — load an imported module binding by module variable index.
    LdExternalModuleVar(u8),

    /// `0x7C` — store accumulator into an exported module binding.
    StModuleVar(u8),

    /// `0x3C` — load a lexical environment binding.
    LdLexVar(u8, u8),

    /// `0x3D` — store accumulator into a lexical environment binding.
    StLexVar(u8, u8),

    /// `0xB6` — create a named lexical environment.
    NewLexEnvWithName(u8, u16),

    /// `0x09` — create an anonymous lexical environment.
    NewLexEnv(u8),

    /// `0xFE 0x09` — throw if accumulator is hole, using name string id.
    ThrowUndefinedIfHoleWithName(u16),

    // ── Object property access ────────────────────────────────────────────────
    /// `0x42` — load `acc.<name>` into accumulator (acc is the object).
    /// Short form: `[0x42] [slot: u8] [name_id: u16 LE]` = 4 bytes.
    LdObjByName(u8, u16),

    /// `0x43` — store accumulator into `obj_vreg.<name>`.
    /// Short form: `[0x43] [slot: u8] [name_id: u16 LE] [obj_vreg: u8]` = 5 bytes.
    StObjByName(u8, u16, u8),

    // ── Definition instructions ───────────────────────────────────────────────
    /// `0x33` — define function literal.
    /// `slot` = IC slot; `method_id` = 16-bit method literal index;
    /// `length` = formal parameter count.
    /// Encoding: `[0x33] [slot: u8] [method_id: u16 LE] [length: u8]` = 5 bytes.
    DefineFuncShort(u8, u16, u8),

    /// `0x35` — define class with a literal-array initialiser buffer.
    /// `slot` = IC slot; `ctor_id` = 16-bit method index for the constructor;
    /// `buf_id` = 16-bit literal-array ID; `proto_argc` = number of prototype
    /// methods; `proto_vreg` = register holding the prototype object.
    /// Encoding: `[0x35] [slot:u8] [ctor_id:u16 LE] [buf_id:u16 LE] [proto_argc:u16 LE] [proto_vreg:u8]` = 9 bytes.
    DefineClassWithBuffer(u8, u16, u16, u16, u8),

    /// `0x05` / `0x80` — create an empty array, carrying an IC slot.
    CreateEmptyArray(u16),

    /// `0x79` / `0xCB` — store accumulator into own array index.
    StOwnByIndex(u16, u8, u16),

    // ── Object construction ──────────────────────────────────────────────────
    /// `0x08` `op_imm1_8_imm2_8_v_8` — construct new object.
    /// `slot` = IC slot; `argc` = arg count (includes constructor arg in v_start);
    /// `v_start` = first register (holds the constructor function).
    /// Encoding: `[0x08] [slot: u8] [argc: u8] [v_start: u8]` = 4 bytes.
    NewObjRange(u8, u8, u8),

    // ── Returns ──────────────────────────────────────────────────────────────
    /// `0x64` `op_none` — return accumulator value.
    Return,

    /// `0x65` `op_none` — return `undefined` (void return).
    ReturnUndefined,

    // ── Unconditional branch ─────────────────────────────────────────────────
    /// `0x4E` `op_imm_16` — jump by signed 16-bit byte offset.
    /// Encoding: `[0x4E] [offset: i16 LE]` = 3 bytes.
    Jmp(i16),

    /// `0x98` `op_imm_32` — jump by signed 32-bit byte offset.
    /// Encoding: `[0x98] [offset: i32 LE]` = 5 bytes.
    JmpWide(i32),

    // ── Conditional branches ─────────────────────────────────────────────────
    /// `0x50` `op_imm_16` — jump if accumulator equals zero / is falsy.
    /// Encoding: `[0x50] [offset: i16 LE]` = 3 bytes.
    Jeqz(i16),

    /// `0x9A` `op_imm_32` — wide form of `jeqz`.
    JeqzWide(i32),

    /// `0x9B` `op_imm_16` — jump if accumulator is not zero / is truthy.
    /// Encoding: `[0x9B] [offset: i16 LE]` = 3 bytes.
    Jnez(i16),

    /// `0x9C` `op_imm_32` — wide form of `jnez`.
    JnezWide(i32),

    // ── Exception ────────────────────────────────────────────────────────────
    /// `0xFE 0x00` — throw the value in the accumulator.
    /// Encoding: `[0xFE] [0x00]` = 2 bytes (prefixed instruction).
    Throw,
}

impl Insn {
    /// Encode this instruction into its binary byte representation.
    ///
    /// `_strings` is accepted for API compatibility but is currently unused;
    /// all string indices are expected to be resolved before instruction
    /// construction.
    ///
    /// All multi-byte operands are little-endian.
    pub fn encode(&self, _strings: &mut crate::emit::strings::StringTable) -> Vec<u8> {
        match self {
            // ── constant loaders ─────────────────────────────────────────────
            Insn::LdUndefined => vec![0x00],
            Insn::LdNull => vec![0x01],
            Insn::LdTrue => vec![0x02],
            Insn::LdFalse => vec![0x03],

            // ── immediates ───────────────────────────────────────────────────
            Insn::LdAi(n) => {
                let mut v = vec![0x62];
                v.extend_from_slice(&n.to_le_bytes());
                v
            }
            Insn::FLdAi(f) => {
                let mut v = vec![0x63];
                v.extend_from_slice(&f.to_le_bytes());
                v
            }

            // ── string load ──────────────────────────────────────────────────
            Insn::LdaStr(id) => {
                let mut v = vec![0x3E];
                v.extend_from_slice(&id.to_le_bytes());
                v
            }

            // ── register moves ───────────────────────────────────────────────
            Insn::Lda(vreg) => vec![0x60, *vreg],
            Insn::Sta(vreg) => vec![0x61, *vreg],

            Insn::MovV4(dst, src) => {
                // 4-bit register pair: bits[3:0] = dst, bits[7:4] = src
                let packed = (*src << 4) | (*dst & 0x0F);
                vec![0x44, packed]
            }
            Insn::MovV8(dst, src) => vec![0x45, *dst, *src],
            Insn::MovV16(dst, src) => {
                let mut v = vec![0x8F];
                v.extend_from_slice(&dst.to_le_bytes());
                v.extend_from_slice(&src.to_le_bytes());
                v
            }

            // ── arithmetic ───────────────────────────────────────────────────
            Insn::Add2(slot, vreg) => vec![0x0A, *slot, *vreg],
            Insn::Sub2(slot, vreg) => vec![0x0B, *slot, *vreg],
            Insn::Mul2(slot, vreg) => vec![0x0C, *slot, *vreg],
            Insn::Div2(slot, vreg) => vec![0x0D, *slot, *vreg],
            Insn::Mod2(slot, vreg) => vec![0x0E, *slot, *vreg],

            // ── equality / comparison ─────────────────────────────────────────
            Insn::Eq(slot, vreg) => vec![0x0F, *slot, *vreg],
            Insn::NotEq(slot, vreg) => vec![0x10, *slot, *vreg],
            Insn::Less(slot, vreg) => vec![0x11, *slot, *vreg],
            Insn::LessEq(slot, vreg) => vec![0x12, *slot, *vreg],
            Insn::Greater(slot, vreg) => vec![0x13, *slot, *vreg],
            Insn::GreaterEq(slot, vreg) => vec![0x14, *slot, *vreg],

            // ── bitwise binary ops ────────────────────────────────────────────
            Insn::Shl2(slot, vreg) => vec![0x15, *slot, *vreg],
            Insn::Shr2(slot, vreg) => vec![0x16, *slot, *vreg],
            Insn::Ashr2(slot, vreg) => vec![0x17, *slot, *vreg],
            Insn::And2(slot, vreg) => vec![0x18, *slot, *vreg],
            Insn::Or2(slot, vreg) => vec![0x19, *slot, *vreg],
            Insn::Xor2(slot, vreg) => vec![0x1A, *slot, *vreg],

            // ── unary operators ───────────────────────────────────────────────
            Insn::TypeOf(slot) => vec![0x1C, *slot],
            Insn::Neg(slot) => vec![0x1F, *slot],
            Insn::BitNot(slot) => vec![0x20, *slot],
            Insn::Inc(slot) => vec![0x21, *slot],
            Insn::Dec(slot) => vec![0x22, *slot],

            // ── in operator ───────────────────────────────────────────────────
            Insn::IsIn(slot, vreg) => vec![0x25, *slot, *vreg],

            // ── strict equality ───────────────────────────────────────────────
            Insn::StrictNotEq(slot, vreg) => vec![0x27, *slot, *vreg],
            Insn::StrictEq(slot, vreg) => vec![0x28, *slot, *vreg],

            // ── calls ────────────────────────────────────────────────────────
            Insn::CallArg0(slot) => vec![0x29, *slot],
            Insn::CallArg1(slot, v0) => vec![0x2A, *slot, *v0],
            Insn::CallArgs2(slot, v0, v1) => vec![0x2B, *slot, *v0, *v1],
            Insn::CallArgs3(slot, v0, v1, v2) => vec![0x2C, *slot, *v0, *v1, *v2],
            Insn::CallRange(slot, argc, v_start) => vec![0x73, *slot, *argc, *v_start],

            Insn::CallThis0(slot, this) => vec![0x2D, *slot, *this],
            Insn::CallThis1(slot, this, v0) => vec![0x2E, *slot, *this, *v0],
            Insn::CallThis2(slot, this, v0, v1) => vec![0x2F, *slot, *this, *v0, *v1],
            Insn::CallThis3(slot, this, v0, v1, v2) => vec![0x30, *slot, *this, *v0, *v1, *v2],
            Insn::CallThisRange(slot, argc, this_vreg) => vec![0x31, *slot, *argc, *this_vreg],

            // ── global access ────────────────────────────────────────────────
            Insn::TryLdGlobalByName(slot, name_id) => {
                let mut v = vec![0x3F, *slot];
                v.extend_from_slice(&name_id.to_le_bytes());
                v
            }
            Insn::TryStGlobalByName(slot, name_id) => {
                let mut v = vec![0x40, *slot];
                v.extend_from_slice(&name_id.to_le_bytes());
                v
            }
            Insn::LdExternalModuleVar(index) => vec![0x7e, *index],
            Insn::StModuleVar(index) => vec![0x7c, *index],
            Insn::LdLexVar(level, slot) => vec![0x3c, (level << 4) | slot],
            Insn::StLexVar(level, slot) => vec![0x3d, (level << 4) | slot],
            Insn::NewLexEnvWithName(count, literal_id) => {
                let mut v = vec![0xb6, *count];
                v.extend_from_slice(&literal_id.to_le_bytes());
                v
            }
            Insn::NewLexEnv(count) => vec![0x09, *count],
            Insn::ThrowUndefinedIfHoleWithName(name_id) => {
                let mut v = vec![0xfe, 0x09];
                v.extend_from_slice(&name_id.to_le_bytes());
                v
            }

            // ── object property ──────────────────────────────────────────────
            Insn::LdObjByName(slot, name_id) => {
                let mut v = vec![0x42, *slot];
                v.extend_from_slice(&name_id.to_le_bytes());
                v
            }
            Insn::StObjByName(slot, name_id, obj) => {
                let mut v = vec![0x43, *slot];
                v.extend_from_slice(&name_id.to_le_bytes());
                v.push(*obj);
                v
            }

            // ── definition ───────────────────────────────────────────────────
            Insn::DefineFuncShort(slot, method_id, length) => {
                let mut v = vec![0x33, *slot];
                v.extend_from_slice(&method_id.to_le_bytes());
                v.push(*length);
                v
            }
            Insn::DefineClassWithBuffer(slot, ctor_id, buf_id, proto_argc, proto_vreg) => {
                let mut v = vec![0x35, *slot];
                v.extend_from_slice(&ctor_id.to_le_bytes());
                v.extend_from_slice(&buf_id.to_le_bytes());
                v.extend_from_slice(&proto_argc.to_le_bytes());
                v.push(*proto_vreg);
                v
            }
            Insn::CreateEmptyArray(slot) if *slot <= u8::MAX as u16 => vec![0x05, *slot as u8],
            Insn::CreateEmptyArray(slot) => {
                let mut v = vec![0x80];
                v.extend_from_slice(&slot.to_le_bytes());
                v
            }
            Insn::StOwnByIndex(slot, array, index) if *slot <= u8::MAX as u16 => {
                let mut v = vec![0x79, *slot as u8, *array];
                v.extend_from_slice(&index.to_le_bytes());
                v
            }
            Insn::StOwnByIndex(slot, array, index) => {
                let mut v = vec![0xcb];
                v.extend_from_slice(&slot.to_le_bytes());
                v.push(*array);
                v.extend_from_slice(&index.to_le_bytes());
                v
            }

            // ── object construction ──────────────────────────────────────────
            Insn::NewObjRange(slot, argc, v_start) => vec![0x08, *slot, *argc, *v_start],

            // ── returns ──────────────────────────────────────────────────────
            Insn::Return => vec![0x64],
            Insn::ReturnUndefined => vec![0x65],

            // ── branches ─────────────────────────────────────────────────────
            Insn::Jmp(off) => {
                let mut v = vec![0x4E];
                v.extend_from_slice(&off.to_le_bytes());
                v
            }
            Insn::JmpWide(off) => {
                let mut v = vec![0x98];
                v.extend_from_slice(&off.to_le_bytes());
                v
            }
            Insn::Jeqz(off) => {
                let mut v = vec![0x50];
                v.extend_from_slice(&off.to_le_bytes());
                v
            }
            Insn::JeqzWide(off) => {
                let mut v = vec![0x9A];
                v.extend_from_slice(&off.to_le_bytes());
                v
            }
            Insn::Jnez(off) => {
                let mut v = vec![0x9B];
                v.extend_from_slice(&off.to_le_bytes());
                v
            }
            Insn::JnezWide(off) => {
                let mut v = vec![0x9C];
                v.extend_from_slice(&off.to_le_bytes());
                v
            }

            // ── exception ────────────────────────────────────────────────────
            Insn::Throw => vec![0xFE, 0x00],
        }
    }

    /// Return the encoded byte length of this instruction without encoding it.
    pub fn byte_len(&self) -> usize {
        match self {
            Insn::LdUndefined | Insn::LdNull | Insn::LdTrue | Insn::LdFalse => 1,
            Insn::LdAi(_) => 5,
            Insn::FLdAi(_) => 9,
            Insn::LdaStr(_) => 3,
            Insn::Lda(_) | Insn::Sta(_) => 2,
            Insn::MovV4(_, _) => 2,
            Insn::MovV8(_, _) => 3,
            Insn::MovV16(_, _) => 5,
            Insn::Add2(_, _)
            | Insn::Sub2(_, _)
            | Insn::Mul2(_, _)
            | Insn::Div2(_, _)
            | Insn::Mod2(_, _)
            | Insn::Eq(_, _)
            | Insn::NotEq(_, _)
            | Insn::Less(_, _)
            | Insn::LessEq(_, _)
            | Insn::Greater(_, _)
            | Insn::GreaterEq(_, _)
            | Insn::Shl2(_, _)
            | Insn::Shr2(_, _)
            | Insn::Ashr2(_, _)
            | Insn::And2(_, _)
            | Insn::Or2(_, _)
            | Insn::Xor2(_, _)
            | Insn::IsIn(_, _)
            | Insn::StrictNotEq(_, _)
            | Insn::StrictEq(_, _) => 3,
            Insn::TypeOf(_) | Insn::Neg(_) | Insn::BitNot(_) | Insn::Inc(_) | Insn::Dec(_) => 2,
            Insn::CallArg0(_) => 2,
            Insn::CallArg1(_, _) => 3,
            Insn::CallArgs2(_, _, _) => 4,
            Insn::CallArgs3(_, _, _, _) => 5,
            Insn::CallRange(_, _, _) => 4,
            Insn::CallThis0(_, _) => 3,
            Insn::CallThis1(_, _, _) => 4,
            Insn::CallThis2(_, _, _, _) => 5,
            Insn::CallThis3(_, _, _, _, _) => 6,
            Insn::CallThisRange(_, _, _) => 4,
            Insn::TryLdGlobalByName(_, _)
            | Insn::TryStGlobalByName(_, _)
            | Insn::LdObjByName(_, _) => 4,
            Insn::LdExternalModuleVar(_) | Insn::StModuleVar(_) => 2,
            Insn::LdLexVar(_, _) | Insn::StLexVar(_, _) => 2,
            Insn::NewLexEnv(_) => 2,
            Insn::NewLexEnvWithName(_, _) => 4,
            Insn::ThrowUndefinedIfHoleWithName(_) => 4,
            Insn::StObjByName(_, _, _) => 5,
            Insn::DefineFuncShort(_, _, _) => 5,
            Insn::DefineClassWithBuffer(_, _, _, _, _) => 9,
            Insn::CreateEmptyArray(slot) => {
                if *slot <= u8::MAX as u16 {
                    2
                } else {
                    3
                }
            }
            Insn::StOwnByIndex(slot, _, _) => {
                if *slot <= u8::MAX as u16 {
                    5
                } else {
                    6
                }
            }
            Insn::NewObjRange(_, _, _) => 4,
            Insn::Return | Insn::ReturnUndefined => 1,
            Insn::Jmp(_) | Insn::Jeqz(_) | Insn::Jnez(_) => 3,
            Insn::JmpWide(_) | Insn::JeqzWide(_) | Insn::JnezWide(_) => 5,
            Insn::Throw => 2,
        }
    }
}
