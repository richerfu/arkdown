use crate::emit::record::{
    encode_code_item, inline_string, uleb128, ClassRecord, FieldRecord, FieldValue,
    FIELD_OWNER_CLASS_IDX,
};
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
/// hvigorw (DevEco Studio 6.0, target SDK 5.0.5(17)) produces version
/// `[0x0C, 0x00, 0x06, 0x00]` = `0x0006_000C` in little-endian.
/// This matches what the ArkRuntime at API 17 expects.
///
/// Note: the standalone DevEco 6.0 es2abc binary produces `0x0001_000D`,
/// but hvigorw's internal es2abc uses `0x0006_000C`.  We match hvigorw.
pub const VERSION: u32 = 0x0006_000C;

/// Byte length of the fixed `.abc` file header.
///
/// The Panda header is **60 bytes** (0x3C).  The class index begins
/// immediately after the header at offset `class_idx_off = 0x3C`.
const HEADER_SIZE: usize = 0x3C;

const SOURCE_LANG_ECMASCRIPT: u8 = 0x00;

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

fn sdk_native_field(name: &str) -> FieldRecord {
    FieldRecord {
        name: name.to_string(),
        class_idx: FIELD_OWNER_CLASS_IDX,
        type_idx: 0,
        access_flags: 0,
        value: Some(FieldValue::U8(0)),
    }
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
    /// 0x0C         4    version          = VERSION (0x0006_000C)
    /// 0x10         4    file_size        = total serialized length      (patched)
    /// 0x14         4    foreign_off      = 0   (no foreign section in API 13+)
    /// 0x18         4    foreign_size     = 0
    /// 0x1C         4    num_classes      = total classes (all regular ClassItems)
    /// 0x20         4    class_idx_off    = 0x3C (immediately after header)
    /// 0x24         4    num_lnp          = 0   (no line-number programs)
    /// 0x28         4    lnp_idx_off      = 0
    /// 0x2C         4    num_literalarrays
    /// 0x30         4    literalarray_idx_off
    /// 0x34         4    num_idx_regions  = 1
    /// 0x38         4    idx_section_off  = after class and literal-array indexes
    ///
    /// ── class index (at 0x3C) ─────────────────────────────────────────────
    ///      4*N    class_idx  [u32 file-offsets, sorted alphabetically by name]
    ///
    /// ── literal-array index ──────────────────────────────────────────────
    ///      4*M    literal array file offsets
    ///
    /// ── IndexSection ─────────────────────────────────────────────────────
    ///       40    libpandafile `IndexHeader`
    ///      4*N    class index table used by FieldItem/MethodItem class_idx
    ///      4*K    method/constant index table used by bytecode id operands
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
    /// API 13+ replaces old API 9 stubs. hvigor emits two regular ClassItems
    /// with 0 fields, 0 methods, and access_flags = 0x2001 (ACC_PUBLIC |
    /// ACC_ANNOTATION):
    ///
    /// * `L_ESConcurrentModuleRequestsAnnotation;`
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
        // ── SDK framework namespace stubs ────────────────────────────────────
        //
        // hvigorw always emits these 7 ClassItems as placeholder stubs for the
        // OHOS SDK runtime namespaces, each with one `@native.*` u8 field.
        // They are NOT derived from the user's imports — they are a fixed set
        // present in every merged ABC file.
        //
        // Class names start with 'L@' (0x40) so they sort before 'L_' annotation
        // stubs (0x5F) and before user class names that start with 'L' followed by
        // a bundle letter.
        //
        // access_flags = 0x0001 (ACC_PUBLIC): regular class, not an annotation.
        let sdk_stubs = [
            ClassRecord {
                name: "L@ohos.app;".to_string(),
                methods: vec![],
                access_flags: 0x0001,
                source_lang: SOURCE_LANG_ECMASCRIPT,
                source_file: None,
                module_requests: vec![],
                fields: vec![sdk_native_field("@native.ohos.app")],
            },
            ClassRecord {
                name: "L@ohos.curves;".to_string(),
                methods: vec![],
                access_flags: 0x0001,
                source_lang: SOURCE_LANG_ECMASCRIPT,
                source_file: None,
                module_requests: vec![],
                fields: vec![sdk_native_field("@native.ohos.curves")],
            },
            ClassRecord {
                name: "L@ohos.matrix4;".to_string(),
                methods: vec![],
                access_flags: 0x0001,
                source_lang: SOURCE_LANG_ECMASCRIPT,
                source_file: None,
                module_requests: vec![],
                fields: vec![sdk_native_field("@native.ohos.matrix4")],
            },
            ClassRecord {
                name: "L@system.app;".to_string(),
                methods: vec![],
                access_flags: 0x0001,
                source_lang: SOURCE_LANG_ECMASCRIPT,
                source_file: None,
                module_requests: vec![],
                fields: vec![sdk_native_field("@native.system.app")],
            },
            ClassRecord {
                name: "L@system.curves;".to_string(),
                methods: vec![],
                access_flags: 0x0001,
                source_lang: SOURCE_LANG_ECMASCRIPT,
                source_file: None,
                module_requests: vec![],
                fields: vec![sdk_native_field("@native.system.curves")],
            },
            ClassRecord {
                name: "L@system.matrix4;".to_string(),
                methods: vec![],
                access_flags: 0x0001,
                source_lang: SOURCE_LANG_ECMASCRIPT,
                source_file: None,
                module_requests: vec![],
                fields: vec![sdk_native_field("@native.system.matrix4")],
            },
            ClassRecord {
                name: "L@system.router;".to_string(),
                methods: vec![],
                access_flags: 0x0001,
                source_lang: SOURCE_LANG_ECMASCRIPT,
                source_file: None,
                module_requests: vec![],
                fields: vec![sdk_native_field("@native.system.router")],
            },
        ];

        // ── Annotation stub classes (API 13+ / hvigorw) ──────────────────────────
        // hvigorw emits exactly 2 annotation stubs (acc=0x2001 = ACC_PUBLIC|ACC_ANNOTATION).
        // L_ESExpectedPropertyCountAnnotation is NOT emitted by hvigorw.
        let annotation_stubs = [
            ClassRecord {
                name: "L_ESConcurrentModuleRequestsAnnotation;".to_string(),
                methods: vec![],
                access_flags: 0x2001,
                source_lang: SOURCE_LANG_ECMASCRIPT,
                source_file: None,
                module_requests: vec![],
                fields: vec![],
            },
            ClassRecord {
                name: "L_ESSlotNumberAnnotation;".to_string(),
                methods: vec![],
                access_flags: 0x2001,
                source_lang: SOURCE_LANG_ECMASCRIPT,
                source_file: None,
                module_requests: vec![],
                fields: vec![],
            },
        ];

        // ── Sort ALL classes (user classes + sdk stubs + annotation stubs) by name ──
        // Sort order by leading byte after 'L':
        //   '@' (0x40) → SDK stubs   (L@ohos.*, L@system.*)
        //   '_' (0x5F) → annotation stubs (L_ES*)
        //   lowercase   → user bundle classes (Lcom.*, L&bundle&;)
        let mut all_classes: Vec<&ClassRecord> = self
            .records
            .iter()
            .chain(sdk_stubs.iter())
            .chain(annotation_stubs.iter())
            .collect();
        all_classes.sort_by(|a, b| a.name.cmp(&b.name));

        let total_classes = all_classes.len() as u32;
        let class_idx_off = HEADER_SIZE as u32; // 0x3C
        let total_methods = all_classes
            .iter()
            .map(|class| class.methods.len() as u32)
            .sum::<u32>();
        let method_index_count = self.strings.len() as u32 + total_methods;

        let num_lnp = all_classes
            .iter()
            .flat_map(|class| class.methods.iter())
            .filter(|method| method.debug_info.is_some())
            .count() as u32;
        let lnp_idx_off = if num_lnp == 0 {
            0
        } else {
            class_idx_off + total_classes * 4
        };

        let num_literal_arrays = self.literal_arrays.len() as u32;
        let literalarray_idx_off = if num_literal_arrays == 0 {
            0
        } else {
            class_idx_off + total_classes * 4 + num_lnp * 4
        };

        // IndexSection is placed after the class index and literal-array index.
        let idx_sect_off = class_idx_off + total_classes * 4 + num_lnp * 4 + num_literal_arrays * 4;
        let index_class_count = 2 + 1 + self.records.len() as u32 + sdk_stubs.len() as u32;
        let index_header_size: u32 = 40;
        let index_class_idx_off = idx_sect_off + index_header_size;
        let index_method_idx_off = index_class_idx_off + index_class_count * 4;
        let idx_sect_size = index_header_size + index_class_count * 4 + method_index_count * 4;

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

        // ── 1b. Compute module_request StringItem offsets ─────────────────────
        //
        // Module specifier strings (e.g. "@ohos:app.ability.UIAbility") are
        // emitted as standalone StringItems immediately after the method name
        // strings.  Their file offsets are tracked here for future use in the
        // full ModuleRecord annotation encoding.
        let mut module_req_string_offsets: Vec<Vec<u32>> = Vec::new();
        let mut all_module_req_strings: Vec<Vec<Vec<u8>>> = Vec::new();

        for class in &all_classes {
            let mut req_offsets = Vec::new();
            let mut req_strings = Vec::new();
            for spec in &class.module_requests {
                req_offsets.push(cur);
                let s = inline_string(spec);
                cur += s.len() as u32;
                req_strings.push(s);
            }
            module_req_string_offsets.push(req_offsets);
            all_module_req_strings.push(req_strings);
        }
        // Suppress unused-variable warnings; offsets will be used when full
        // ModuleRecord annotation emission is implemented.
        let _ = &module_req_string_offsets;

        // ── 1c. Compute field StringItem offsets ──────────────────────────────
        //
        // Field name strings (e.g. "pkgName@", "isCommonjs", …) are emitted as
        // standalone StringItems immediately after the module-request strings.
        // Their file offsets are passed to `serialize_class_item()` so the
        // FieldItem `name_off` fields point to the correct bytes.
        let mut field_name_string_offsets: Vec<Vec<u32>> = Vec::new();
        let mut all_field_name_strings: Vec<Vec<Vec<u8>>> = Vec::new();

        for class in &all_classes {
            let mut f_offsets = Vec::new();
            let mut f_strings = Vec::new();
            for field in &class.fields {
                f_offsets.push(cur);
                let s = inline_string(&field.name);
                cur += s.len() as u32;
                f_strings.push(s);
            }
            field_name_string_offsets.push(f_offsets);
            all_field_name_strings.push(f_strings);
        }

        // ── 1d. Compute source-file StringItem offsets ───────────────────────
        //
        // ClassTag::SOURCE_FILE and method DebugInfo/LNP SET_FILE both point to
        // a standalone StringItem containing the original source path.
        let mut source_file_string_offsets: Vec<Option<u32>> = Vec::new();
        let mut all_source_file_strings: Vec<Option<Vec<u8>>> = Vec::new();

        for class in &all_classes {
            if let Some(source_file) = &class.source_file {
                source_file_string_offsets.push(Some(cur));
                let s = inline_string(source_file);
                cur += s.len() as u32;
                all_source_file_strings.push(Some(s));
            } else {
                source_file_string_offsets.push(None);
                all_source_file_strings.push(None);
            }
        }

        // ── 1e. Compute bytecode StringItem offsets ──────────────────────────
        //
        // Bytecode id operands use the IndexHeader method/constant index.  The
        // string table indices allocated during lowering therefore need real
        // file offsets in the same stable order.
        let mut bytecode_string_offsets = Vec::with_capacity(self.strings.len());
        let mut all_bytecode_strings = Vec::with_capacity(self.strings.len());
        for string in self.strings.iter() {
            bytecode_string_offsets.push(cur);
            let s = inline_string(string);
            cur += s.len() as u32;
            all_bytecode_strings.push(s);
        }

        // ── 2. Compute ClassItem sizes & start offsets ────────────────────────
        let class_section_start = cur;

        let class_item_sizes: Vec<usize> = all_classes
            .iter()
            .map(|c| c.class_item_byte_size())
            .collect();

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

        // Build the IndexHeader class table used by FieldItem/MethodItem
        // `class_idx` values. hvigor keeps two non-EntityId scalar entries at
        // the front, then `_ESSlotNumberAnnotation`, user records, and SDK
        // namespace stubs. `_ESConcurrentModuleRequestsAnnotation` is present
        // in the top-level class index but not in this typed class table.
        let mut index_class_offsets = Vec::with_capacity(index_class_count as usize);
        index_class_offsets.extend_from_slice(&[2, 6]);
        let mut name_to_index_class_idx = std::collections::HashMap::new();
        let mut next_index_class_idx = 2u16;
        for class in std::iter::once(&annotation_stubs[1])
            .chain(self.records.iter())
            .chain(sdk_stubs.iter())
        {
            let off = *name_to_class_offset
                .get(class.name.as_str())
                .expect("indexed class name must have a ClassItem offset");
            index_class_offsets.push(off);
            name_to_index_class_idx.insert(class.name.as_str(), next_index_class_idx);
            next_index_class_idx += 1;
        }
        debug_assert_eq!(index_class_offsets.len() as u32, index_class_count);

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

        let mut method_index_offsets = Vec::with_capacity(method_index_count as usize);
        method_index_offsets.extend(bytecode_string_offsets.iter().copied());
        for (class, &class_off) in all_classes.iter().zip(class_item_offsets.iter()) {
            method_index_offsets.extend(class.method_item_offsets(class_off));
        }
        debug_assert_eq!(method_index_offsets.len() as u32, method_index_count);

        // ── Pre-compute DebugInfo and LineNumberProgram sections ─────────────
        // Each debug method gets a minimal LNP: SET_FILE, END_SEQUENCE. The
        // source-file offset is stored in the DebugInfo constant pool, matching
        // libpandafile's LineNumberProgramItem::EmitSetFile convention.
        let (debug_info_offsets, debug_info_section, lnp_offsets, lnp_section): (
            Vec<Vec<Option<u32>>>,
            Vec<u8>,
            Vec<u32>,
            Vec<u8>,
        ) = {
            let mut per_class_offsets = Vec::with_capacity(all_classes.len());
            let mut debug_buf = Vec::new();
            let mut lnp_buf = Vec::new();
            let mut debug_cur = code_cur;
            let mut lnp_index = 0u32;

            for (class_idx, class) in all_classes.iter().enumerate() {
                let mut class_offsets = Vec::with_capacity(class.methods.len());
                let source_file_off = source_file_string_offsets[class_idx];
                for method in &class.methods {
                    if let (Some(debug), Some(source_file_off)) =
                        (&method.debug_info, source_file_off)
                    {
                        class_offsets.push(Some(debug_cur));
                        let bytes = encode_debug_info(debug.line_start, source_file_off, lnp_index);
                        debug_cur += bytes.len() as u32;
                        debug_buf.extend_from_slice(&bytes);
                        lnp_index += 1;
                    } else {
                        class_offsets.push(None);
                    }
                }
                per_class_offsets.push(class_offsets);
            }

            let mut lnp_offsets = Vec::with_capacity(num_lnp as usize);
            let mut lnp_cur = debug_cur;
            for _ in 0..num_lnp {
                lnp_offsets.push(lnp_cur);
                // LineNumberProgramItem opcodes: SET_FILE, END_SEQUENCE.
                lnp_buf.extend_from_slice(&[0x09, 0x00]);
                lnp_cur += 2;
            }

            debug_assert_eq!(lnp_index, num_lnp);
            (per_class_offsets, debug_buf, lnp_offsets, lnp_buf)
        };

        let post_lnp_offset = code_cur + debug_info_section.len() as u32 + lnp_section.len() as u32;

        // ── Pre-compute LiteralArray section bytes ────────────────────────────
        // The section is appended after all CodeItems and debug sections.
        // Format per LiteralArray:
        //   [num_entries: u32 LE]
        //   Per entry: [tag: u8] [value: variable]
        let (literal_array_offsets, lit_arr_section): (Vec<u32>, Vec<u8>) = {
            let mut buf = Vec::new();
            let mut offsets = Vec::with_capacity(self.literal_arrays.len());
            let mut offset_cur = post_lnp_offset;
            for lit_arr in &self.literal_arrays {
                offsets.push(offset_cur);
                let before = buf.len();
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
                offset_cur += (buf.len() - before) as u32;
            }
            (offsets, buf)
        };

        let total_size = post_lnp_offset + lit_arr_section.len() as u32;

        // ── IndexSection values (computed before emission) ────────────────────
        // class_item_offsets is sorted by class name (same order as all_classes).
        let first_class_off = *class_item_offsets.first().expect("at least one class");

        // ── 4. Serialize ClassItems with actual offsets ───────────────────────
        let class_item_bytes: Vec<Vec<u8>> = all_classes
            .iter()
            .enumerate()
            .map(|(ci, class)| {
                let class_idx = *name_to_index_class_idx
                    .get(class.name.as_str())
                    .unwrap_or(&0);
                class.serialize_class_item(
                    class_idx,
                    &field_name_string_offsets[ci],
                    source_file_string_offsets[ci],
                    &method_string_offsets[ci],
                    &code_offsets[ci],
                    &debug_info_offsets[ci],
                )
            })
            .collect();

        // Verify sizes are consistent
        for (i, (expected, actual)) in class_item_sizes
            .iter()
            .zip(class_item_bytes.iter())
            .enumerate()
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
        file.extend_from_slice(&MAGIC); // 0x00 magic[8]
        file.extend_from_slice(&0u32.to_le_bytes()); // 0x08 checksum (patched)
        file.extend_from_slice(&VERSION.to_le_bytes()); // 0x0C version
        file.extend_from_slice(&0u32.to_le_bytes()); // 0x10 file_size (patched)
        file.extend_from_slice(&0u32.to_le_bytes()); // 0x14 foreign_off = 0
        file.extend_from_slice(&0u32.to_le_bytes()); // 0x18 foreign_size = 0
        file.extend_from_slice(&total_classes.to_le_bytes()); // 0x1C num_classes
        file.extend_from_slice(&class_idx_off.to_le_bytes()); // 0x20 class_idx_off = 0x3C
        file.extend_from_slice(&num_lnp.to_le_bytes()); // 0x24 num_lnp
        file.extend_from_slice(&lnp_idx_off.to_le_bytes()); // 0x28 lnp_idx_off
        file.extend_from_slice(&num_literal_arrays.to_le_bytes()); // 0x2C num_literalarrays
        file.extend_from_slice(&literalarray_idx_off.to_le_bytes()); // 0x30 literalarray_idx_off
        file.extend_from_slice(&1u32.to_le_bytes()); // 0x34 num_idx_regions = 1
        file.extend_from_slice(&idx_sect_off.to_le_bytes()); // 0x38 idx_section_off
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
            class_idx_off + total_classes * 4,
            "class index must end exactly before the optional LNP index"
        );

        // ── LineNumberProgram index ─────────────────────────────────────────
        for offset in &lnp_offsets {
            file.extend_from_slice(&offset.to_le_bytes());
        }
        debug_assert_eq!(
            file.len() as u32,
            class_idx_off + total_classes * 4 + num_lnp * 4,
            "LNP index must end exactly before the optional literal-array index"
        );

        // ── LiteralArray index ───────────────────────────────────────────────
        //
        // API 17 hvigor output stores a u32 file offset for each LiteralArray
        // between the class index and IndexSection.  Leaving the old
        // 0xFFFF_FFFF markers makes SDK ark_disasm reject the file.
        for offset in &literal_array_offsets {
            file.extend_from_slice(&offset.to_le_bytes());
        }
        debug_assert_eq!(
            file.len() as u32,
            idx_sect_off,
            "literal-array index must end exactly at idx_sect_off"
        );

        // ── IndexSection ─────────────────────────────────────────────────────
        //
        // libpandafile expects a 40-byte IndexHeader followed by EntityId
        // arrays.  The old 60-byte placeholder made class_idx/proto_idx
        // resolution read into arbitrary bytes after the header.
        file.extend_from_slice(&first_class_off.to_le_bytes()); // start
        file.extend_from_slice(&total_size.to_le_bytes()); // end
        file.extend_from_slice(&index_class_count.to_le_bytes()); // class_idx_size
        file.extend_from_slice(&index_class_idx_off.to_le_bytes()); // class_idx_off
        file.extend_from_slice(&method_index_count.to_le_bytes()); // method_idx_size
        file.extend_from_slice(&index_method_idx_off.to_le_bytes()); // method_idx_off
        file.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes()); // field_idx_size
        file.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes()); // field_idx_off
        file.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes()); // proto_idx_size
        file.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes()); // proto_idx_off

        for offset in &index_class_offsets {
            file.extend_from_slice(&offset.to_le_bytes());
        }
        for offset in &method_index_offsets {
            file.extend_from_slice(&offset.to_le_bytes());
        }
        debug_assert_eq!(
            file.len() as u32,
            idx_sect_off + idx_sect_size,
            "IndexSection must match its computed header and array size"
        );

        // ── Method StringItems ────────────────────────────────────────────────
        for class_strings in &all_method_strings {
            for s in class_strings {
                file.extend_from_slice(s);
            }
        }

        // ── Module Request StringItems ────────────────────────────────────────
        //
        // Emit each module specifier (e.g. "@ohos:app.ability.UIAbility") as a
        // standalone StringItem accessible by its file offset.  This lets
        // ArkRuntime discover the imported module specifiers; the full
        // ModuleRecord annotation encoding is a follow-up task.
        for class_req_strings in &all_module_req_strings {
            for s in class_req_strings {
                file.extend_from_slice(s);
            }
        }

        // ── Field Name StringItems ────────────────────────────────────────────
        //
        // Emit the field name string for each FieldRecord (e.g. "pkgName@",
        // "isCommonjs", etc.) so that each FieldItem's `name_off` field can
        // point to the correct byte offset in the file.
        for class_field_strings in &all_field_name_strings {
            for s in class_field_strings {
                file.extend_from_slice(s);
            }
        }

        // ── Source File StringItems ─────────────────────────────────────────
        for s in all_source_file_strings.iter().flatten() {
            file.extend_from_slice(s);
        }

        // ── Bytecode StringItems ─────────────────────────────────────────────
        for s in &all_bytecode_strings {
            file.extend_from_slice(s);
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

        // ── DebugInfo and LineNumberProgram sections ────────────────────────
        file.extend_from_slice(&debug_info_section);
        file.extend_from_slice(&lnp_section);
        debug_assert_eq!(file.len() as u32, post_lnp_offset);

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

fn encode_debug_info(line_start: u32, source_file_offset: u32, lnp_index: u32) -> Vec<u8> {
    let mut constant_pool = uleb128(source_file_offset as u64);
    let mut out = Vec::with_capacity(8 + constant_pool.len());
    out.extend_from_slice(&uleb128(line_start as u64));
    out.extend_from_slice(&uleb128(0)); // num_parameters
    out.extend_from_slice(&uleb128(constant_pool.len() as u64));
    out.append(&mut constant_pool);
    out.extend_from_slice(&uleb128(lnp_index as u64));
    out
}

impl Default for AbcFile {
    fn default() -> Self {
        Self::new()
    }
}
