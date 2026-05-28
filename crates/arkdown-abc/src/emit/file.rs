use crate::emit::record::{encode_code_item, inline_string, ClassRecord};
use crate::emit::strings::StringTable;

// ── LiteralArray types (Task 1) ───────────────────────────────────────────────

/// A single entry in a Panda `LiteralArray`.
///
/// Wire format per entry: `[tag: u8] [value: variable]`
///
/// | Tag  | Type    | Value width |
/// |------|---------|-------------|
/// | 0x00 | integer | u32 LE      |
/// | 0x02 | string  | u16 LE (string-table index) |
/// | 0x05 | method  | u32 LE (file offset of CodeItem) |
/// | 0x0F | null    | none (tag only) |
#[derive(Debug)]
pub enum LiteralEntry {
    Integer(u32),
    String(u16),
    Method(u32),
    Null,
}

/// A literal array referenced by `defineclasswithbuffer`.
///
/// The 0-based position of this value in `AbcFile::literal_arrays` is used as
/// the `buf_id` operand in the `DefineClassWithBuffer` instruction.
#[derive(Debug)]
pub struct LiteralArray {
    /// Entries serialised into the literal-array section.
    pub entries: Vec<LiteralEntry>,
}

/// Magic bytes at offset 0x00 of every `.abc` file.
///
/// Source: `arkcompiler_runtime_core/libpandafile/file_items.h` — the literal
/// ASCII string `"PANDA"` followed by three `\x00` bytes (8 bytes total).
pub const MAGIC: [u8; 8] = *b"PANDA\x00\x00\x00";

/// File format version at offset 0x0C.
///
/// DevEco Studio 6.0 / es2abc v13.0.1.0 (API 13+) produces version
/// `[0x0D, 0x00, 0x01, 0x00]` = `0x0001_000D` in little-endian.
pub const VERSION: u32 = 0x0001_000D;

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
    /// Literal arrays referenced by `defineclasswithbuffer`.
    ///
    /// Each entry's 0-based index in this vector is the `buf_id` operand
    /// used in the `DefineClassWithBuffer` opcode.  The section is appended
    /// to the file after all CodeItems.
    pub literal_arrays: Vec<LiteralArray>,
}

impl AbcFile {
    /// Create an empty `AbcFile`.
    pub fn new() -> Self {
        AbcFile {
            strings: StringTable::default(),
            records: Vec::new(),
            literal_arrays: Vec::new(),
        }
    }

    /// Append a `ClassRecord` to this file.
    pub fn add_record(&mut self, record: ClassRecord) {
        self.records.push(record);
    }

    /// Serialize the file to a byte vector.
    ///
    /// ## Binary layout (API 13+ / DevEco Studio 6.0 / es2abc v13.0.1.0)
    ///
    /// ```text
    /// Offset     Size   Field
    /// 0x00         8    magic            = b"PANDA\x00\x00\x00"
    /// 0x08         4    checksum         = Adler-32 over file[0x0C..]  (patched last)
    /// 0x0C         4    version          = VERSION (0x0001_000D)
    /// 0x10         4    file_size        = total serialized length      (patched)
    /// 0x14         4    foreign_off      = 0   (no foreign section in API 13+)
    /// 0x18         4    foreign_size     = 0
    /// 0x1C         4    num_classes      = total classes (all regular ClassItems)
    /// 0x20         4    class_idx_off    = 0x3C (immediately after header)
    /// 0x24         4    num_lnp          = 0   (no line-number programs)
    /// 0x28         4    lnp_idx_off      = 0
    /// 0x2C         4                     = 0xFFFFFFFF  (unused marker)
    /// 0x30         4                     = 0xFFFFFFFF  (unused marker)
    /// 0x34         4    num_idx_regions  = 1
    /// 0x38         4    idx_section_off  = 0x3C + num_classes * 4
    ///                                      (immediately after class index)
    ///
    /// ── class index (at 0x3C) ─────────────────────────────────────────────
    ///      4*N    class_idx  [u32 file-offsets, sorted alphabetically by name]
    ///
    /// ── IndexSection (60 bytes, immediately after class index) ───────────
    ///        4    region_start      = first_class_off
    ///        4    region_end        = file_size
    ///        4    num_entries       = num_classes
    ///        4    entity_id_0       = idx_sect_off + 40
    ///        4    tag_0             = 1
    ///        4    entity_id_1       = idx_sect_off + 56
    ///     4*4    padding            = 0xFFFFFFFF
    ///        4    2                 (static sub-table header)
    ///        4    6                 (num standard module fields, static)
    ///        4    first_class_off   (= class_offsets[0])
    ///        4    last_class_off    (= class_offsets[N-1])
    ///        4    class_section_end (= code_section_start)
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
    /// ## Annotation stub classes (API 13+)
    ///
    /// API 13+ replaces old API 9 stubs.  All three are regular ClassItems
    /// with 0 fields, 0 methods, and access_flags = 0x2001 (ACC_PUBLIC |
    /// ACC_ANNOTATION):
    ///
    /// * `L_ESConcurrentModuleRequestsAnnotation;`
    /// * `L_ESExpectedPropertyCountAnnotation;`
    /// * `L_ESSlotNumberAnnotation;`
    ///
    /// There is **no** ForeignClassItem or foreign section.
    ///
    /// ## Two-pass offset computation
    ///
    /// ClassItems reference CodeItems by file offset, and CodeItems follow
    /// all ClassItems.  We compute ClassItem byte sizes first (they do not
    /// depend on the offset values themselves, only on method counts), derive
    /// CodeItem start offsets, then serialize with correct absolute offsets.
    pub fn serialize(&self) -> Vec<u8> {
        // ── Annotation stub classes (API 13+, acc=0x2001 = ACC_PUBLIC|ACC_ANNOTATION) ──
        let annotation_stubs = [
            ClassRecord {
                name: "L_ESConcurrentModuleRequestsAnnotation;".to_string(),
                methods: vec![],
                access_flags: 0x2001,
            },
            ClassRecord {
                name: "L_ESExpectedPropertyCountAnnotation;".to_string(),
                methods: vec![],
                access_flags: 0x2001,
            },
            ClassRecord {
                name: "L_ESSlotNumberAnnotation;".to_string(),
                methods: vec![],
                access_flags: 0x2001,
            },
        ];

        // ── Sort ALL classes (user classes + annotation stubs) by name ────────
        // User module names use '&' (0x26) which sorts before '_' (0x5F) used by
        // annotation stubs, so user classes always appear first in the index.
        let mut all_classes: Vec<&ClassRecord> =
            self.records.iter().chain(annotation_stubs.iter()).collect();
        all_classes.sort_by(|a, b| a.name.cmp(&b.name));

        let total_classes = all_classes.len() as u32;
        let class_idx_off = HEADER_SIZE as u32; // 0x3C

        // IndexSection is placed immediately after the class index (no foreign section).
        let idx_sect_off = class_idx_off + total_classes * 4;
        let idx_sect_size: u32 = 60; // fixed 60 bytes = 15 × u32

        // Method StringItems start immediately after the IndexSection.
        let string_section_start = idx_sect_off + idx_sect_size;

        // ── 1. Compute method StringItem offsets ──────────────────────────────
        let mut method_string_offsets: Vec<Vec<u32>> = Vec::new();
        let mut all_method_strings: Vec<Vec<Vec<u8>>> = Vec::new();

        let mut cur = string_section_start;
        for class in &all_classes {
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

        let class_item_sizes: Vec<usize> =
            all_classes.iter().map(|c| c.class_item_byte_size()).collect();

        let class_item_offsets: Vec<u32> = {
            let mut offsets = Vec::with_capacity(all_classes.len());
            let mut pos = class_section_start;
            for &sz in &class_item_sizes {
                offsets.push(pos);
                pos += sz as u32;
            }
            offsets
        };

        // Build a map from class name → ClassItem file offset (for the class index).
        let name_to_class_offset: std::collections::HashMap<&str, u32> = all_classes
            .iter()
            .zip(class_item_offsets.iter())
            .map(|(c, &off)| (c.name.as_str(), off))
            .collect();

        // Build a map from class name → sorted position (for class_idx fields in MethodItems).
        let name_to_sorted_idx: std::collections::HashMap<&str, u16> = all_classes
            .iter()
            .enumerate()
            .map(|(i, c)| (c.name.as_str(), i as u16))
            .collect();

        // ── 3. Compute CodeItem start offsets ─────────────────────────────────
        let code_section_start: u32 =
            class_section_start + class_item_sizes.iter().sum::<usize>() as u32;

        let mut code_offsets: Vec<Vec<u32>> = Vec::new();
        let mut all_code_items: Vec<Vec<Vec<u8>>> = Vec::new();

        let mut code_cur = code_section_start;
        for class in &all_classes {
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

        // ── Pre-compute LiteralArray section bytes ────────────────────────────
        // The section is appended after all CodeItems.
        // Format per LiteralArray:
        //   [num_entries: u32 LE]
        //   Per entry: [tag: u8] [value: variable]
        let lit_arr_section: Vec<u8> = {
            let mut buf = Vec::new();
            for lit_arr in &self.literal_arrays {
                buf.extend_from_slice(&(lit_arr.entries.len() as u32).to_le_bytes());
                for entry in &lit_arr.entries {
                    match entry {
                        LiteralEntry::Integer(v) => {
                            buf.push(0x00);
                            buf.extend_from_slice(&v.to_le_bytes());
                        }
                        LiteralEntry::String(idx) => {
                            buf.push(0x02);
                            buf.extend_from_slice(&idx.to_le_bytes());
                        }
                        LiteralEntry::Method(off) => {
                            buf.push(0x05);
                            buf.extend_from_slice(&off.to_le_bytes());
                        }
                        LiteralEntry::Null => {
                            buf.push(0x0F);
                        }
                    }
                }
            }
            buf
        };

        let total_size = code_cur + lit_arr_section.len() as u32;

        // ── IndexSection values (computed before emission) ────────────────────
        // class_item_offsets is sorted by class name (same order as all_classes).
        let first_class_off = *class_item_offsets.first().expect("at least one class");
        let last_class_off = *class_item_offsets.last().expect("at least one class");
        let class_section_end = code_section_start; // = last_class_off + last_class_size

        // Entity IDs: file offsets pointing to specific words WITHIN the IndexSection.
        //   entity_id_0 → IndexSection[+40] (the static "2" sub-table header)
        //   entity_id_1 → IndexSection[+56] (the class_section_end field)
        let entity_id_0 = idx_sect_off + 40;
        let entity_id_1 = idx_sect_off + 56;

        // ── 4. Serialize ClassItems with actual offsets ───────────────────────
        let class_item_bytes: Vec<Vec<u8>> = all_classes
            .iter()
            .enumerate()
            .map(|(ci, class)| {
                let class_idx = *name_to_sorted_idx
                    .get(class.name.as_str())
                    .expect("class name must be in sorted list");
                class.serialize_class_item(
                    class_idx,
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
        let mut file: Vec<u8> = Vec::with_capacity(total_size as usize + 4);

        // ── Header (60 bytes = 0x3C) ──────────────────────────────────────────
        file.extend_from_slice(&MAGIC);                                    // 0x00 magic[8]
        file.extend_from_slice(&0u32.to_le_bytes());                       // 0x08 checksum (patched)
        file.extend_from_slice(&VERSION.to_le_bytes());                    // 0x0C version
        file.extend_from_slice(&0u32.to_le_bytes());                       // 0x10 file_size (patched)
        file.extend_from_slice(&0u32.to_le_bytes());                       // 0x14 foreign_off = 0
        file.extend_from_slice(&0u32.to_le_bytes());                       // 0x18 foreign_size = 0
        file.extend_from_slice(&total_classes.to_le_bytes());              // 0x1C num_classes
        file.extend_from_slice(&class_idx_off.to_le_bytes());              // 0x20 class_idx_off = 0x3C
        file.extend_from_slice(&0u32.to_le_bytes());                       // 0x24 num_lnp = 0
        file.extend_from_slice(&0u32.to_le_bytes());                       // 0x28 lnp_idx_off = 0
        file.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes());             // 0x2C = -1 (unused)
        file.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes());             // 0x30 = -1 (unused)
        file.extend_from_slice(&1u32.to_le_bytes());                       // 0x34 num_idx_regions = 1
        file.extend_from_slice(&idx_sect_off.to_le_bytes());               // 0x38 idx_section_off
        debug_assert_eq!(file.len(), HEADER_SIZE, "header must be exactly 0x3C bytes");

        // ── Class index (at 0x3C, sorted by class name) ───────────────────────
        for class in &all_classes {
            let off = *name_to_class_offset
                .get(class.name.as_str())
                .expect("class name must be in class index");
            file.extend_from_slice(&off.to_le_bytes());
        }
        debug_assert_eq!(
            file.len() as u32,
            idx_sect_off,
            "class index must end exactly at idx_sect_off"
        );

        // ── IndexSection (60 bytes, immediately after class index) ────────────
        //
        // Structure (15 × u32):
        //   [00] region_start = first_class_off
        //   [04] region_end   = file_size (total_size)
        //   [08] num_entries  = num_classes
        //   [0c] entity_id_0  = idx_sect_off + 40  → points to [+40] below
        //   [10] tag_0        = 1
        //   [14] entity_id_1  = idx_sect_off + 56  → points to [+56] below
        //   [18..27]          = 0xFFFFFFFF × 4 (padding)
        //   [28] 2            = sub-table count (static)
        //   [2c] 6            = num standard module fields (static)
        //   [30] first_class_off
        //   [34] last_class_off
        //   [38] class_section_end  ← entity_id_1 points here
        file.extend_from_slice(&first_class_off.to_le_bytes());         // [+00] region_start
        file.extend_from_slice(&total_size.to_le_bytes());              // [+04] region_end
        file.extend_from_slice(&total_classes.to_le_bytes());           // [+08] num_entries
        file.extend_from_slice(&entity_id_0.to_le_bytes());             // [+0c] entity_id_0
        file.extend_from_slice(&1u32.to_le_bytes());                    // [+10] tag_0 = 1
        file.extend_from_slice(&entity_id_1.to_le_bytes());             // [+14] entity_id_1
        file.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes());          // [+18] padding
        file.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes());          // [+1c] padding
        file.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes());          // [+20] padding
        file.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes());          // [+24] padding
        file.extend_from_slice(&2u32.to_le_bytes());                    // [+28] sub-table header
        file.extend_from_slice(&6u32.to_le_bytes());                    // [+2c] num std fields
        file.extend_from_slice(&first_class_off.to_le_bytes());         // [+30] first_class_off
        file.extend_from_slice(&last_class_off.to_le_bytes());          // [+34] last_class_off
        file.extend_from_slice(&class_section_end.to_le_bytes());       // [+38] class_section_end
        debug_assert_eq!(
            file.len() as u32,
            idx_sect_off + idx_sect_size,
            "IndexSection must be exactly 60 bytes"
        );

        // ── Method StringItems ────────────────────────────────────────────────
        for class_strings in &all_method_strings {
            for s in class_strings {
                file.extend_from_slice(s);
            }
        }
        debug_assert_eq!(file.len() as u32, class_section_start);

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
        debug_assert_eq!(file.len() as u32, code_cur);

        // ── LiteralArray section (after all CodeItems) ────────────────────────
        file.extend_from_slice(&lit_arr_section);
        debug_assert_eq!(file.len() as u32, total_size);

        // ── Patch file_size at offset 0x10 ────────────────────────────────────
        let file_size = file.len() as u32;
        file[0x10..0x14].copy_from_slice(&file_size.to_le_bytes());

        // ── Compute Adler-32 checksum over file[12..] and patch offset 0x08 ───
        let checksum = adler32(&file[12..]);
        file[0x08..0x0C].copy_from_slice(&checksum.to_le_bytes());

        file
    }
}

/// Compute Adler-32 checksum of `data`.
///
/// Used for the ABC file checksum field at offset 0x08.
/// Verified against: `es2abc` output for API 9 SDK.
fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65521;
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in data {
        a = (a + byte as u32) % MOD;
        b = (b + a) % MOD;
    }
    (b << 16) | a
}

impl Default for AbcFile {
    fn default() -> Self {
        Self::new()
    }
}
