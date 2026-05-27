use crate::emit::record::ClassRecord;
use crate::emit::strings::StringTable;

/// Magic bytes at offset 0x00 of every `.abc` file.
///
/// Source: `arkcompiler_runtime_core/libpandafile/file_items.h` — the literal
/// ASCII string `"PANDA"` followed by three `\x00` bytes (8 bytes total).
pub const MAGIC: [u8; 8] = *b"PANDA\x00\x00\x00";

/// File format version at offset 0x0C, stored as a raw `[u8; 4]` tuple.
///
/// The real `modules.abc` produced by DevEco Studio / es2abc for HarmonyOS
/// API 12 contains the byte sequence `[0x0C, 0x00, 0x06, 0x00]`, which
/// decodes as version **12.0.6.0** (API 12, ArkCompiler 3.3.x).  Stored as a
/// little-endian `u32` this is `0x0006_000C`.
///
/// Verified by hexdumping a real `modules.abc`:
/// ```text
/// 0x0C: 0c 00 06 00   ← [12, 0, 6, 0] LE u32 = 0x0006000C
/// ```
pub const VERSION: u32 = 0x0006_000C;

/// Byte length of the fixed `.abc` file header.
///
/// The real header is **60 bytes** (0x3C).  The class index begins immediately
/// after the header, which is confirmed by `class_idx_off = 0x3C` in every
/// real file examined.
const HEADER_SIZE: usize = 0x3C;

/// Assembled Panda `.abc` file ready for serialization.
pub struct AbcFile {
    /// Interned string table — all string references in the bytecode are
    /// resolved through this table.
    pub strings: StringTable,
    /// Class records, one per compiled JavaScript chunk.
    pub records: Vec<ClassRecord>,
}

impl AbcFile {
    /// Create an empty `AbcFile`.
    pub fn new() -> Self {
        AbcFile {
            strings: StringTable::default(),
            records: Vec::new(),
        }
    }

    /// Append a `ClassRecord` to this file.
    pub fn add_record(&mut self, record: ClassRecord) {
        self.records.push(record);
    }

    /// Serialize the file to a byte vector.
    ///
    /// ## Binary layout (all multi-byte fields little-endian)
    ///
    /// ```text
    /// Offset  Size  Field
    /// 0x00    8     magic            = b"PANDA\x00\x00\x00"
    /// 0x08    4     checksum         = 0 (CRC32 patch TODO)
    /// 0x0C    4     version          = VERSION (0x0006_000C for API 12)
    /// 0x10    4     file_size        = total serialised length (patched)
    /// 0x14    4     foreign_off      = 0 (no foreign items)
    /// 0x18    4     foreign_size     = 0
    /// 0x1C    4     num_classes      = number of ClassRecords
    /// 0x20    4     class_idx_off    = HEADER_SIZE (0x3C)
    /// 0x24    4     num_lnp          = 0 (no line-number programs)
    /// 0x28    4     lnp_idx_off      = 0
    /// 0x2C    4     num_idx_regions  = 0
    /// 0x30    4     idx_section_off  = 0
    /// 0x34    4     reserved_0       = 0 (literalarray_count in newer SDK)
    /// 0x38    4     reserved_1       = 0 (literalarray_section_off)
    ///
    /// ── class index (at 0x3C, immediately after header) ───────────────────
    /// 0x3C    4*N   class_idx        = [u32 file-offset per ClassRecord]
    ///
    /// ── string section ────────────────────────────────────────────────────
    ///         ?     string items (ULEB128-prefixed, null-terminated)
    ///
    /// ── class record sections ─────────────────────────────────────────────
    ///         ?     ClassRecord binary (simplified placeholder)
    /// ```
    ///
    /// ## Known limitations (TODO: SPEC-009 follow-up)
    ///
    /// The class and method record binary layout emitted by `ClassRecord::serialize`
    /// is a **simplified placeholder**.  It does not match the exact Panda
    /// `ClassItem` / `MethodItem` / `CodeItem` binary format expected by
    /// ArkRuntime.  Specifically:
    ///
    /// - The real `ClassItem` name is an inline ULEB128-prefixed null-terminated
    ///   string followed by super-class offset, access_flags, and tagged entries
    ///   (`ClassTag::SOURCE_LANG`, `ClassTag::NOTHING`, etc.).
    /// - The real `MethodItem` embeds its name string, packed `access_flags`
    ///   (header_idx << 16 | function_kind << 8 | flags), and tagged entries
    ///   (`MethodTag::CODE`, `MethodTag::SOURCE_LANG`, etc.).
    /// - The real `CodeItem` stores `num_vregs`, `num_args`, `num_insns` as
    ///   ULEB128, then raw bytecode, then try-catch blocks.
    /// - Index regions (`num_idx_regions`, `idx_section_off`) are required for
    ///   ArkRuntime class-linker lookups and are not yet emitted.
    ///
    /// The opcode bytes, instruction encoding, string ULEB128 format, header
    /// fields, and VERSION constant are all correct based on cross-checking
    /// against real `modules.abc` output and `isa/isa.yaml`.
    pub fn serialize(&self) -> Vec<u8> {
        // ── Compute offsets for all sections ─────────────────────────────────

        let num_classes = self.records.len() as u32;
        let class_idx_off = HEADER_SIZE as u32;           // 0x3C
        let class_idx_size = num_classes * 4;             // 4 bytes per entry
        let string_section_off = class_idx_off + class_idx_size;

        let string_section = self.strings.serialize();
        let class_sections: Vec<Vec<u8>> =
            self.records.iter().map(|r| r.serialize()).collect();

        // Class record offsets within the file (needed for class_idx entries).
        let mut class_offsets: Vec<u32> = Vec::with_capacity(self.records.len());
        let mut cur = string_section_off + string_section.len() as u32;
        for cs in &class_sections {
            class_offsets.push(cur);
            cur += cs.len() as u32;
        }
        let total_size = cur;

        let mut file: Vec<u8> =
            Vec::with_capacity(total_size as usize);

        // ── Header (60 bytes = 0x3C) ──────────────────────────────────────────
        file.extend_from_slice(&MAGIC);                                         // 0x00 magic [u8;8]
        file.extend_from_slice(&0u32.to_le_bytes());                            // 0x08 checksum (TODO: CRC32)
        file.extend_from_slice(&VERSION.to_le_bytes());                         // 0x0C version
        file.extend_from_slice(&0u32.to_le_bytes());                            // 0x10 file_size (patched below)
        file.extend_from_slice(&0u32.to_le_bytes());                            // 0x14 foreign_off
        file.extend_from_slice(&0u32.to_le_bytes());                            // 0x18 foreign_size
        file.extend_from_slice(&num_classes.to_le_bytes());                     // 0x1C num_classes
        file.extend_from_slice(&class_idx_off.to_le_bytes());                   // 0x20 class_idx_off = 0x3C
        file.extend_from_slice(&0u32.to_le_bytes());                            // 0x24 num_lnp
        file.extend_from_slice(&0u32.to_le_bytes());                            // 0x28 lnp_idx_off
        file.extend_from_slice(&0u32.to_le_bytes());                            // 0x2C num_idx_regions
        file.extend_from_slice(&0u32.to_le_bytes());                            // 0x30 idx_section_off
        file.extend_from_slice(&0u32.to_le_bytes());                            // 0x34 reserved (literalarray_count)
        file.extend_from_slice(&0u32.to_le_bytes());                            // 0x38 reserved (literalarray_off)
        debug_assert_eq!(file.len(), HEADER_SIZE, "header must be exactly 60 bytes");

        // ── Class index (at class_idx_off = 0x3C) ────────────────────────────
        for &off in &class_offsets {
            file.extend_from_slice(&off.to_le_bytes());
        }

        // ── String section ────────────────────────────────────────────────────
        file.extend_from_slice(&string_section);

        // ── Class record sections ─────────────────────────────────────────────
        for class_bytes in &class_sections {
            file.extend_from_slice(class_bytes);
        }

        debug_assert_eq!(file.len(), total_size as usize);

        // Patch file_size at offset 0x10.
        let file_size = file.len() as u32;
        file[0x10..0x14].copy_from_slice(&file_size.to_le_bytes());

        file
    }
}

impl Default for AbcFile {
    fn default() -> Self {
        Self::new()
    }
}
