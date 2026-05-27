use crate::emit::record::ClassRecord;
use crate::emit::strings::StringTable;

/// Magic bytes at offset 0x00 of every `.abc` file.
///
/// Source: `arkcompiler_runtime_core/libpandafile/file_items.h`
/// The real OHOS ABC format uses the ASCII string "PANDA" followed by three
/// null bytes, for a total of 8 bytes.
pub const MAGIC: [u8; 8] = *b"PANDA\x00\x00\x00";

/// File format version encoded in the header at offset 0x08.
///
/// `0x000C0000` corresponds to version 12.0.0.0 (ArkCompiler API 12).
/// VERIFY: Cross-check against the version field in real `.abc` output from
/// `es2abc --target-api-version 12`.
pub const VERSION: u32 = 0x000C_0000;

/// Byte length of the fixed `.abc` file header.
const HEADER_SIZE: usize = 0x40; // 64 bytes

/// Assembled Panda `.abc` file ready for serialization.
///
/// Holds the interned string table and all class records compiled from the
/// input JavaScript chunks.
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
    /// Layout:
    /// ```text
    /// [header: HEADER_SIZE bytes]
    /// [string section]
    /// [class record sections]
    /// ```
    ///
    /// Header fields (all little-endian u32 unless noted):
    /// ```text
    /// 0x00  magic        [u8; 8]  = b"PANDA\x00\x00\x00"
    /// 0x08  checksum     u32      = 0 (unverified; CRC32 patch is TODO)
    /// 0x0C  version      u32      = VERSION
    /// 0x10  file_size    u32      = total byte length (patched after assembly)
    /// 0x14  foreign_off  u32      = 0 (no foreign items)
    /// 0x18  foreign_size u32      = 0
    /// 0x1C  num_classes  u32      = number of ClassRecords
    /// 0x20  class_idx_off u32     = offset of class index section
    /// 0x24  num_lnp      u32      = 0 (no line-number programs)
    /// 0x28  lnp_idx_off  u32      = 0
    /// 0x2C  reserved     [u8; 8]
    /// 0x34  num_idx_rgns u32      = 0
    /// 0x38  idx_sec_off  u32      = 0
    /// 0x3C  (pad to 0x40)
    /// ```
    ///
    /// VERIFY: Cross-check all field offsets and sizes against
    /// `arkcompiler_runtime_core/libpandafile/file_items.h` before shipping.
    pub fn serialize(&self) -> Vec<u8> {
        // Serialize variable-size sections first so we can compute offsets.
        let string_section = self.strings.serialize();
        let class_sections: Vec<Vec<u8>> =
            self.records.iter().map(|r| r.serialize()).collect();

        let string_off = HEADER_SIZE as u32;
        let classes_off = string_off + string_section.len() as u32;

        let mut file: Vec<u8> = Vec::with_capacity(
            HEADER_SIZE + string_section.len() + class_sections.iter().map(|s| s.len()).sum::<usize>(),
        );

        // --- Header ---
        file.extend_from_slice(&MAGIC);                                          // 0x00 magic [u8; 8]
        file.extend_from_slice(&0u32.to_le_bytes());                             // 0x08 checksum placeholder
        file.extend_from_slice(&VERSION.to_le_bytes());                          // 0x0C version
        file.extend_from_slice(&0u32.to_le_bytes());                             // 0x10 file_size placeholder
        file.extend_from_slice(&0u32.to_le_bytes());                             // 0x14 foreign_off
        file.extend_from_slice(&0u32.to_le_bytes());                             // 0x18 foreign_size
        file.extend_from_slice(&(self.records.len() as u32).to_le_bytes());      // 0x1C num_classes
        file.extend_from_slice(&classes_off.to_le_bytes());                      // 0x20 class_idx_off
        file.extend_from_slice(&0u32.to_le_bytes());                             // 0x24 num_lnp
        file.extend_from_slice(&0u32.to_le_bytes());                             // 0x28 lnp_idx_off
        file.extend_from_slice(&[0u8; 8]);                                       // 0x2C reserved [u8; 8]
        file.extend_from_slice(&0u32.to_le_bytes());                             // 0x34 num_idx_regions
        file.extend_from_slice(&0u32.to_le_bytes());                             // 0x38 idx_section_off
        // Pad to HEADER_SIZE (0x38 → 0x40 = 8 bytes padding)
        while file.len() < HEADER_SIZE {
            file.push(0);
        }

        // --- String section ---
        file.extend_from_slice(&string_section);

        // --- Class record sections ---
        for class_bytes in &class_sections {
            file.extend_from_slice(class_bytes);
        }

        // Patch file_size field at 0x10 (magic is 8 bytes, so checksum is at
        // 0x08, version at 0x0C, file_size at 0x10).
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
