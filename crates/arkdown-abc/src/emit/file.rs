use crate::emit::record::{encode_code_item, inline_string, ClassRecord};
use crate::emit::strings::StringTable;

/// Magic bytes at offset 0x00 of every `.abc` file.
///
/// Source: `arkcompiler_runtime_core/libpandafile/file_items.h` — the literal
/// ASCII string `"PANDA"` followed by three `\x00` bytes (8 bytes total).
pub const MAGIC: [u8; 8] = *b"PANDA\x00\x00\x00";

/// File format version at offset 0x0C.
///
/// The real `modules.abc` produced by DevEco Studio / es2abc for HarmonyOS
/// API 12 uses `[0x0C, 0x00, 0x06, 0x00]` = version `12.0.6.0`.  Stored as
/// a little-endian `u32` this is `0x0006_000C`.
///
/// The test files produced by the local es2abc (API 9) use version `0x09`.
/// For runtime compatibility we keep API 12 here; update as needed.
pub const VERSION: u32 = 0x0006_000C;

/// Byte length of the fixed `.abc` file header.
///
/// The Panda header is **60 bytes** (0x3C).  The class index begins
/// immediately after the header at offset `class_idx_off = 0x3C`.
const HEADER_SIZE: usize = 0x3C;

/// Assembled Panda `.abc` file ready for serialization.
pub struct AbcFile {
    /// Interned string table for bytecode string operands (`lda.str`,
    /// `tryldglobalbyname`, etc.).  Referenced by 16-bit index in opcodes.
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
    /// Offset     Size   Field
    /// 0x00         8    magic            = b"PANDA\x00\x00\x00"
    /// 0x08         4    checksum         = CRC32 (currently 0, TODO)
    /// 0x0C         4    version          = VERSION constant
    /// 0x10         4    file_size        = total serialized length (patched)
    /// 0x14         4    foreign_off      = 0  (no foreign class items)
    /// 0x18         4    foreign_size     = 0
    /// 0x1C         4    num_classes      = number of ClassRecords
    /// 0x20         4    class_idx_off    = 0x3C (immediately after header)
    /// 0x24         4    num_lnp          = 0   (no line-number programs)
    /// 0x28         4    lnp_idx_off      = 0
    /// 0x2C         4    num_idx_regions  = 0   (no index region)
    /// 0x30         4    idx_section_off  = 0
    /// 0x34         4    literalarray_cnt = 0
    /// 0x38         4    literalarray_off = 0
    ///
    /// ── class index (at 0x3C) ─────────────────────────────────────────────
    ///         4*N   class_idx  [u32 file-offsets, sorted by class name]
    ///
    /// ── method StringItems ────────────────────────────────────────────────
    ///          ?    one ULEB128-prefixed StringItem per method per class
    ///
    /// ── ClassItems ────────────────────────────────────────────────────────
    ///          ?    one ClassItem per class (inline name + tagged entries)
    ///
    /// ── CodeItems ─────────────────────────────────────────────────────────
    ///          ?    one CodeItem per method (ULEB128 header + bytecode)
    /// ```
    ///
    /// ## Two-pass offset computation
    ///
    /// ClassItems reference CodeItems by file offset, and CodeItems follow
    /// all ClassItems.  We compute ClassItem byte sizes first (they do not
    /// depend on the offset values themselves, only on method counts), derive
    /// CodeItem start offsets, then serialize with correct absolute offsets.
    pub fn serialize(&self) -> Vec<u8> {
        // ── Sort classes by name (Panda class index must be sorted) ───────────
        let mut classes: Vec<&ClassRecord> = self.records.iter().collect();
        classes.sort_by(|a, b| a.name.cmp(&b.name));

        let num_classes = classes.len() as u32;
        let class_idx_off = HEADER_SIZE as u32; // 0x3C

        // ── 1. Compute method StringItem offsets ──────────────────────────────
        // Method StringItems are placed immediately after the class index.
        // Each StringItem is `inline_string(method.name)` bytes.

        let string_section_start = class_idx_off + num_classes * 4;
        let mut method_string_offsets: Vec<Vec<u32>> = Vec::new(); // [class_i][method_j] -> file offset
        let mut all_method_strings: Vec<Vec<Vec<u8>>> = Vec::new(); // [class_i][method_j] -> bytes

        let mut cur = string_section_start;
        for class in &classes {
            let mut class_offsets = Vec::new();
            let mut class_strings = Vec::new();
            for method in &class.methods {
                class_offsets.push(cur);
                let s = inline_string(&method.name);
                cur += s.len() as u32;
                class_strings.push(s);
            }
            method_string_offsets.push(class_offsets);
            all_method_strings.push(class_strings);
        }

        // ── 2. Compute ClassItem sizes & start offsets ────────────────────────
        let class_section_start = cur;

        let class_item_sizes: Vec<usize> = classes.iter().map(|c| c.class_item_byte_size()).collect();

        let class_item_offsets: Vec<u32> = {
            let mut offsets = Vec::with_capacity(classes.len());
            let mut pos = class_section_start;
            for &sz in &class_item_sizes {
                offsets.push(pos);
                pos += sz as u32;
            }
            offsets
        };

        // ── 3. Compute CodeItem start offsets ─────────────────────────────────
        let code_section_start: u32 = class_section_start
            + class_item_sizes.iter().sum::<usize>() as u32;

        let mut code_offsets: Vec<Vec<u32>> = Vec::new(); // [class_i][method_j] -> file offset
        let mut all_code_items: Vec<Vec<Vec<u8>>> = Vec::new(); // [class_i][method_j] -> bytes

        let mut code_cur = code_section_start;
        for class in &classes {
            let mut class_code_offs = Vec::new();
            let mut class_code_items = Vec::new();
            for method in &class.methods {
                class_code_offs.push(code_cur);
                let ci = encode_code_item(method.num_vregs, method.num_args, &method.bytecode);
                code_cur += ci.len() as u32;
                class_code_items.push(ci);
            }
            code_offsets.push(class_code_offs);
            all_code_items.push(class_code_items);
        }

        let total_size = code_cur;

        // ── 4. Serialize ClassItems with actual offsets ───────────────────────
        let class_item_bytes: Vec<Vec<u8>> = classes
            .iter()
            .enumerate()
            .map(|(ci, class)| {
                class.serialize_class_item(
                    ci as u16,
                    &method_string_offsets[ci],
                    &code_offsets[ci],
                )
            })
            .collect();

        // Verify sizes are consistent
        for (i, (expected, actual)) in
            class_item_sizes.iter().zip(class_item_bytes.iter()).enumerate()
        {
            debug_assert_eq!(
                *expected,
                actual.len(),
                "ClassItem[{i}] size mismatch: expected {expected}, got {}",
                actual.len()
            );
        }

        // ── 5. Assemble the file ──────────────────────────────────────────────
        let mut file: Vec<u8> = Vec::with_capacity(total_size as usize);

        // ── Header (60 bytes = 0x3C) ──────────────────────────────────────────
        file.extend_from_slice(&MAGIC);                          // 0x00 magic[8]
        file.extend_from_slice(&0u32.to_le_bytes());             // 0x08 checksum (TODO: CRC32)
        file.extend_from_slice(&VERSION.to_le_bytes());          // 0x0C version
        file.extend_from_slice(&0u32.to_le_bytes());             // 0x10 file_size (patched below)
        file.extend_from_slice(&0u32.to_le_bytes());             // 0x14 foreign_off
        file.extend_from_slice(&0u32.to_le_bytes());             // 0x18 foreign_size
        file.extend_from_slice(&num_classes.to_le_bytes());      // 0x1C num_classes
        file.extend_from_slice(&class_idx_off.to_le_bytes());    // 0x20 class_idx_off = 0x3C
        file.extend_from_slice(&0u32.to_le_bytes());             // 0x24 num_lnp
        file.extend_from_slice(&0u32.to_le_bytes());             // 0x28 lnp_idx_off
        file.extend_from_slice(&0u32.to_le_bytes());             // 0x2C num_idx_regions = 0
        file.extend_from_slice(&0u32.to_le_bytes());             // 0x30 idx_section_off = 0
        file.extend_from_slice(&0u32.to_le_bytes());             // 0x34 literalarray_count
        file.extend_from_slice(&0u32.to_le_bytes());             // 0x38 literalarray_off
        debug_assert_eq!(file.len(), HEADER_SIZE, "header must be exactly 0x3C bytes");

        // ── Class index (at 0x3C, sorted by class name) ───────────────────────
        for &off in &class_item_offsets {
            file.extend_from_slice(&off.to_le_bytes());
        }
        debug_assert_eq!(
            file.len(),
            (HEADER_SIZE + classes.len() * 4),
            "class index size mismatch"
        );

        // ── Method StringItems ────────────────────────────────────────────────
        for class_strings in &all_method_strings {
            for s in class_strings {
                file.extend_from_slice(s);
            }
        }

        // ── ClassItems ────────────────────────────────────────────────────────
        for class_bytes in &class_item_bytes {
            file.extend_from_slice(class_bytes);
        }
        debug_assert_eq!(file.len() as u32, code_section_start);

        // ── CodeItems ─────────────────────────────────────────────────────────
        for class_codes in &all_code_items {
            for code in class_codes {
                file.extend_from_slice(code);
            }
        }

        debug_assert_eq!(file.len() as u32, total_size);

        // ── Patch file_size at offset 0x10 ────────────────────────────────────
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
