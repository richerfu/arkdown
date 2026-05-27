/// Encode `value` as an unsigned LEB-128 byte sequence.
fn uleb128(mut value: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(2);
    loop {
        let byte = (value & 0x7F) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            break;
        } else {
            out.push(byte | 0x80);
        }
    }
    out
}

/// A compiled method (function) within a class record.
///
/// In Panda bytecode, every function — including a module's top-level "main"
/// function — is a method belonging to a class record.
#[derive(Debug)]
pub struct MethodRecord {
    /// Index into the string table for this method's name.
    pub name_idx: u32,
    /// Encoded bytecode for the method body.
    pub bytecode: Vec<u8>,
    /// Number of virtual registers required by this method.
    pub num_vregs: u16,
    /// Number of formal parameters (not counting `this`).
    pub num_args: u16,
}

/// A compiled module, represented as a Panda class record.
///
/// One `ClassRecord` is produced per JavaScript chunk file.
#[derive(Debug)]
pub struct ClassRecord {
    /// Index into the string table for this record's name (the OHM URL or
    /// record name that identifies this chunk within the bundle).
    pub name_idx: u32,
    /// All methods belonging to this record.
    pub methods: Vec<MethodRecord>,
}

impl ClassRecord {
    /// Serialise this record to bytes for embedding in an `.abc` file.
    ///
    /// ## Simplified binary layout
    ///
    /// This implementation emits a **simplified** layout that is structurally
    /// distinct from the real Panda `ClassItem` / `MethodItem` / `CodeItem`
    /// format.  It is sufficient for unit-testing the opcode encoder and
    /// header assembly, but **ArkRuntime will not accept this format**.
    ///
    /// The real Panda `ClassItem` binary format (from `file_items.h`) is:
    /// ```text
    /// ── ClassItem ──────────────────────────────────────────────────────────
    /// [class_name: inline ULEB128-prefixed null-terminated string]
    /// [super_class_off: u32 LE]   (file offset; 0 = no super)
    /// [access_flags: u32 LE]      (header_idx<<16 | func_kind<<8 | flags)
    /// [ClassTag::SOURCE_LANG=0x02] [lang: u8 = 0 (ECMAScript)]
    /// ... (method entries, annotation entries) ...
    /// [ClassTag::NOTHING=0x00]
    ///
    /// ── Per-MethodItem (tagged: MethodTag::CODE=0x01) ──────────────────────
    /// [method_name: inline string]
    /// [proto_off: u16 LE]         (index into proto table)
    /// [access_flags: u32 LE]      (header_idx<<16 | func_kind<<8 | flags)
    /// [MethodTag::SOURCE_LANG=0x02] [lang: u8]
    /// [MethodTag::CODE=0x01]
    ///   [code_off: u32 LE]        (file offset of CodeItem)
    /// [MethodTag::NOTHING=0x00]
    ///
    /// ── CodeItem (at code_off) ─────────────────────────────────────────────
    /// ULEB128(num_vregs)
    /// ULEB128(num_args)
    /// ULEB128(num_insns)          (byte count of bytecode)
    /// [bytecode: u8 * num_insns]
    /// [try blocks (empty = 0 bytes)]
    /// ```
    ///
    /// Full reimplementation is tracked in SPEC-009.
    ///
    /// ## Current simplified layout
    ///
    /// ```text
    /// [name_idx: u32 LE]
    /// [method_count: u32 LE]
    /// For each method:
    ///   [name_idx: u32 LE]
    ///   ULEB128(num_vregs)
    ///   ULEB128(num_args)
    ///   ULEB128(bytecode_len)
    ///   [bytecode: u8 * bytecode_len]
    /// ```
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.name_idx.to_le_bytes());
        buf.extend_from_slice(&(self.methods.len() as u32).to_le_bytes());
        for method in &self.methods {
            buf.extend_from_slice(&method.name_idx.to_le_bytes());
            buf.extend_from_slice(&uleb128(method.num_vregs as u64));
            buf.extend_from_slice(&uleb128(method.num_args as u64));
            buf.extend_from_slice(&uleb128(method.bytecode.len() as u64));
            buf.extend_from_slice(&method.bytecode);
        }
        buf
    }
}
