/// A compiled method (function) within a class record.
///
/// In Panda bytecode, every function — including a module's top-level "main"
/// function — is represented as a method inside a class record.
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
    /// Serialize this record to bytes suitable for embedding in an `.abc` file.
    ///
    /// Format:
    /// ```text
    /// [name_idx: u32 LE]
    /// [method_count: u16 LE]
    /// For each method:
    ///   [name_idx: u32 LE]
    ///   [num_vregs: u16 LE]
    ///   [num_args: u16 LE]
    ///   [bytecode_len: u32 LE]
    ///   [bytecode: u8 * bytecode_len]
    /// ```
    ///
    /// NOTE: The actual Panda `file_items.h` class record layout differs from
    /// this simplified format.  This is a development placeholder.  Once the
    /// format is verified against `arkcompiler_runtime_core/libpandafile/`, this
    /// method must be updated to emit the correct binary layout.
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.name_idx.to_le_bytes());
        buf.extend_from_slice(&(self.methods.len() as u16).to_le_bytes());
        for method in &self.methods {
            buf.extend_from_slice(&method.name_idx.to_le_bytes());
            buf.extend_from_slice(&method.num_vregs.to_le_bytes());
            buf.extend_from_slice(&method.num_args.to_le_bytes());
            buf.extend_from_slice(&(method.bytecode.len() as u32).to_le_bytes());
            buf.extend_from_slice(&method.bytecode);
        }
        buf
    }
}
