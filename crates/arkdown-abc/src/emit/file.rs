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
    /// 0x08         4    checksum         = Adler-32 over file[0x0C..]  (patched last)
    /// 0x0C         4    version          = VERSION constant
    /// 0x10         4    file_size        = total serialized length      (patched)
    /// 0x14         4    foreign_off      = offset of ForeignClassItem section
    /// 0x18         4    foreign_size     = byte length of foreign section
    /// 0x1C         4    num_classes      = total classes (regular + foreign)
    /// 0x20         4    class_idx_off    = 0x3C (immediately after header)
    /// 0x24         4    num_lnp          = 0   (no line-number programs)
    /// 0x28         4    lnp_idx_off      = 0
    /// 0x2C         4                     = 1   (es2abc always sets this)
    /// 0x30         4                     = class_idx_end (= foreign_off)
    /// 0x34         4    num_idx_regions  = 1
    /// 0x38         4    idx_section_off  = offset of IndexSection
    ///
    /// ── class index (at 0x3C) ─────────────────────────────────────────────
    ///       4*N    class_idx  [u32 file-offsets, sorted alphabetically by name;
    ///                          includes the foreign class entry]
    ///
    /// ── ForeignClassItem section (at foreign_off) ─────────────────────────
    ///        21    inline_string("L_ESTypeAnnotation;")
    ///              = ULEB128((19<<1)|1) + b"L_ESTypeAnnotation;" + 0x00
    ///
    /// ── method StringItems ────────────────────────────────────────────────
    ///          ?    one ULEB128-prefixed StringItem per method per class
    ///
    /// ── ClassItems ────────────────────────────────────────────────────────
    ///          ?    one ClassItem per class (inline name + tagged entries)
    ///
    /// ── CodeItems ─────────────────────────────────────────────────────────
    ///          ?    one CodeItem per method (ULEB128 header + bytecode)
    ///
    /// ── IndexSection (1 region) ───────────────────────────────────────────
    ///        16    region_start(u32) + region_end(u32) + 0(u32) + 0(u32)
    /// ```
    ///
    /// ## Annotation stub classes
    ///
    /// es2abc always emits 4 annotation-related classes for every JS module.
    /// ArkRuntime will reject the `.abc` file with "Load current file's panda
    /// file failed" if any of them are absent.  We always inject them here:
    ///
    /// * `L_ESTypeAnnotation;`     — ForeignClassItem (only an `inline_string`)
    /// * `L_ESModuleRecord;`       — empty ClassItem (0 methods)
    /// * `L_ESSlotNumberAnnotation;` — empty ClassItem (0 methods)
    /// * `L_ESTypeInfoRecord;`     — empty ClassItem (0 methods)
    ///
    /// ## Two-pass offset computation
    ///
    /// ClassItems reference CodeItems by file offset, and CodeItems follow
    /// all ClassItems.  We compute ClassItem byte sizes first (they do not
    /// depend on the offset values themselves, only on method counts), derive
    /// CodeItem start offsets, then serialize with correct absolute offsets.
    pub fn serialize(&self) -> Vec<u8> {
        // ── Annotation stub classes (ArkRuntime requires these for every JS module) ──
        // Three empty regular ClassItems.  L_ESTypeAnnotation; is handled separately
        // as a ForeignClassItem (it is never a full ClassItem in es2abc output).
        let annotation_stubs = [
            ClassRecord { name: "L_ESModuleRecord;".to_string(), methods: vec![] },
            ClassRecord { name: "L_ESSlotNumberAnnotation;".to_string(), methods: vec![] },
            ClassRecord { name: "L_ESTypeInfoRecord;".to_string(), methods: vec![] },
        ];

        // ── Sort ALL regular classes (user classes + annotation stubs) by name ─
        let mut regular_classes: Vec<&ClassRecord> =
            self.records.iter().chain(annotation_stubs.iter()).collect();
        regular_classes.sort_by(|a, b| a.name.cmp(&b.name));

        // ── Build full sorted class list including the foreign entry ──────────
        // The ForeignClassItem is "L_ESTypeAnnotation;" – it appears in the class
        // index at the correct alphabetical position but has no ClassItem body.
        let foreign_name = "L_ESTypeAnnotation;";
        let foreign_item_bytes = inline_string(foreign_name); // 21 bytes
        let foreign_size = foreign_item_bytes.len() as u32;   // 21

        // Merge regular names with the foreign name, sort alphabetically.
        let mut all_class_names: Vec<(&str, bool)> = regular_classes
            .iter()
            .map(|c| (c.name.as_str(), false))
            .chain(std::iter::once((foreign_name, true)))
            .collect();
        all_class_names.sort_by(|a, b| a.0.cmp(b.0));

        let total_classes = all_class_names.len() as u32;
        let class_idx_off = HEADER_SIZE as u32; // 0x3C

        // Foreign section sits immediately after the class index array.
        let foreign_off = class_idx_off + total_classes * 4;

        // Method StringItems follow the foreign section.
        let string_section_start = foreign_off + foreign_size;

        // ── 1. Compute method StringItem offsets ──────────────────────────────
        // Method StringItems are placed immediately after the foreign section.
        // Each StringItem is `inline_string(method.name)` bytes.
        let mut method_string_offsets: Vec<Vec<u32>> = Vec::new(); // [class_i][method_j] -> file offset
        let mut all_method_strings: Vec<Vec<Vec<u8>>> = Vec::new(); // [class_i][method_j] -> bytes

        let mut cur = string_section_start;
        for class in &regular_classes {
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
            regular_classes.iter().map(|c| c.class_item_byte_size()).collect();

        let class_item_offsets: Vec<u32> = {
            let mut offsets = Vec::with_capacity(regular_classes.len());
            let mut pos = class_section_start;
            for &sz in &class_item_sizes {
                offsets.push(pos);
                pos += sz as u32;
            }
            offsets
        };

        // Build a map from class name → ClassItem file offset (for the class index).
        let name_to_class_offset: std::collections::HashMap<&str, u32> = regular_classes
            .iter()
            .zip(class_item_offsets.iter())
            .map(|(c, &off)| (c.name.as_str(), off))
            .collect();

        // Build a map from class name → position in all_class_names (for class_idx
        // fields inside MethodItems — the MethodItem.class_idx must point to the
        // correct row in the sorted class index, which now includes the foreign entry).
        let name_to_sorted_idx: std::collections::HashMap<&str, u16> = all_class_names
            .iter()
            .enumerate()
            .map(|(i, (name, _))| (*name, i as u16))
            .collect();

        // ── 3. Compute CodeItem start offsets ─────────────────────────────────
        let code_section_start: u32 =
            class_section_start + class_item_sizes.iter().sum::<usize>() as u32;

        let mut code_offsets: Vec<Vec<u32>> = Vec::new(); // [class_i][method_j] -> file offset
        let mut all_code_items: Vec<Vec<Vec<u8>>> = Vec::new(); // [class_i][method_j] -> bytes

        let mut code_cur = code_section_start;
        for class in &regular_classes {
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
        let class_item_bytes: Vec<Vec<u8>> = regular_classes
            .iter()
            .enumerate()
            .map(|(ci, class)| {
                // class_idx must be the position in the full sorted class list
                // (including the foreign entry), not just the regular list.
                let class_idx = *name_to_sorted_idx
                    .get(class.name.as_str())
                    .expect("class name must be in all_class_names");
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
        // IndexSection is appended after all CodeItems.
        let idx_section_off = total_size;
        // The IndexSection region covers from foreign_off to the end of file.
        let idx_region_start = foreign_off;
        let idx_region_end = total_size + 16; // 4 × u32 fields in IndexSection

        let mut file: Vec<u8> = Vec::with_capacity(idx_region_end as usize);

        // ── Header (60 bytes = 0x3C) ──────────────────────────────────────────
        file.extend_from_slice(&MAGIC);                          // 0x00 magic[8]
        file.extend_from_slice(&0u32.to_le_bytes());             // 0x08 checksum (patched below)
        file.extend_from_slice(&VERSION.to_le_bytes());          // 0x0C version
        file.extend_from_slice(&0u32.to_le_bytes());             // 0x10 file_size (patched below)
        file.extend_from_slice(&foreign_off.to_le_bytes());      // 0x14 foreign_off
        file.extend_from_slice(&foreign_size.to_le_bytes());     // 0x18 foreign_size
        file.extend_from_slice(&total_classes.to_le_bytes());    // 0x1C num_classes
        file.extend_from_slice(&class_idx_off.to_le_bytes());    // 0x20 class_idx_off = 0x3C
        file.extend_from_slice(&0u32.to_le_bytes());             // 0x24 num_lnp
        file.extend_from_slice(&0u32.to_le_bytes());             // 0x28 lnp_idx_off
        // 0x2C: es2abc always sets this field to 1
        file.extend_from_slice(&1u32.to_le_bytes());             // 0x2C = 1
        // 0x30: offset of first byte after the class index = foreign_off
        file.extend_from_slice(&foreign_off.to_le_bytes());      // 0x30 = foreign_off
        // 0x34: num_idx_regions = 1 (one IndexSection region)
        file.extend_from_slice(&1u32.to_le_bytes());             // 0x34 num_idx_regions = 1
        // 0x38: offset of IndexSection (written after CodeItems)
        file.extend_from_slice(&idx_section_off.to_le_bytes());  // 0x38 idx_section_off
        debug_assert_eq!(file.len(), HEADER_SIZE, "header must be exactly 0x3C bytes");

        // ── Class index (at 0x3C, sorted by class name) ───────────────────────
        // Includes the foreign class entry at its alphabetical position.
        for (name, is_foreign) in &all_class_names {
            let off = if *is_foreign {
                foreign_off // ForeignClassItem offset
            } else {
                *name_to_class_offset.get(name).expect("name in class index must be in regular classes")
            };
            file.extend_from_slice(&off.to_le_bytes());
        }
        debug_assert_eq!(
            file.len() as u32,
            foreign_off,
            "class index must end exactly at foreign_off"
        );

        // ── ForeignClassItem section ──────────────────────────────────────────
        // A ForeignClassItem is ONLY an inline_string — no ClassItem header.
        // The class index entry for L_ESTypeAnnotation; points directly here.
        file.extend_from_slice(&foreign_item_bytes);
        debug_assert_eq!(
            file.len() as u32,
            string_section_start,
            "foreign section must end at string_section_start"
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

        // ── IndexSection (minimal, 1 region) ─────────────────────────────────
        // ArkRuntime requires an IndexSection (idx_section_off ≠ 0, num_idx_regions = 1).
        file.extend_from_slice(&idx_region_start.to_le_bytes()); // region file-offset start
        file.extend_from_slice(&idx_region_end.to_le_bytes());   // region file-offset end
        file.extend_from_slice(&0u32.to_le_bytes());             // num_class_id_entries = 0
        file.extend_from_slice(&0u32.to_le_bytes());             // num_method_id_entries = 0

        // ── Patch file_size at offset 0x10 ────────────────────────────────────
        let file_size = file.len() as u32;
        file[0x10..0x14].copy_from_slice(&file_size.to_le_bytes());

        // ── Compute Adler-32 checksum over file[12..] and patch offset 0x08 ───
        // Adler-32 covers all bytes after the checksum field itself (offset 12 = 0x0C).
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
