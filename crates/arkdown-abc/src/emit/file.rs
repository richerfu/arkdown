use crate::emit::record::{
    encode_code_item, inline_string, uleb128, ClassRecord, FieldRecord, FieldValue, MethodRecord,
    FIELD_OWNER_CLASS_IDX,
};
use crate::emit::strings::StringTable;

const MANDATORY_PARAM_THIS_REG: u8 = 2;
const LNP_LINE_BASE: i32 = -4;
const LNP_LINE_RANGE: u32 = 15;

// ── LiteralArray types (Task 1) ───────────────────────────────────────────────

/// A single entry in a Panda `LiteralArray`.
///
/// Generic class/scope buffers are encoded as item pairs:
/// `[tag_value: u8] [value: variable]`. The array count is the raw item count,
/// so each tagged entry contributes two items. Module records are a special
/// compact libpandafile layout and store raw values without tag-value items.
#[derive(Debug)]
pub enum LiteralEntry {
    TaggedInteger(u32),
    TaggedString(String),
    TaggedMethod(u32),
    TaggedMethodName(String),
    TaggedMethodAffiliate(u16),
    TaggedNull,
    RawU32(u32),
    RawU16(u16),
    RawString(String),
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

impl LiteralEntry {
    fn item_count(&self) -> u32 {
        match self {
            LiteralEntry::TaggedInteger(_)
            | LiteralEntry::TaggedString(_)
            | LiteralEntry::TaggedMethod(_)
            | LiteralEntry::TaggedMethodName(_)
            | LiteralEntry::TaggedMethodAffiliate(_)
            | LiteralEntry::TaggedNull => 2,
            LiteralEntry::RawU32(_) | LiteralEntry::RawU16(_) | LiteralEntry::RawString(_) => 1,
        }
    }
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

fn align4(value: u32) -> u32 {
    (value + 3) & !3
}

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
                string_indices: vec![],
                literal_indices: vec![],
                module_literal_indices: vec![],
                source_code_strings: vec![],
                access_flags: 0x0001,
                source_lang: SOURCE_LANG_ECMASCRIPT,
                source_file: None,
                module_requests: vec![],
                fields: vec![sdk_native_field("@native.ohos.app")],
            },
            ClassRecord {
                name: "L@ohos.curves;".to_string(),
                methods: vec![],
                string_indices: vec![],
                literal_indices: vec![],
                module_literal_indices: vec![],
                source_code_strings: vec![],
                access_flags: 0x0001,
                source_lang: SOURCE_LANG_ECMASCRIPT,
                source_file: None,
                module_requests: vec![],
                fields: vec![sdk_native_field("@native.ohos.curves")],
            },
            ClassRecord {
                name: "L@ohos.matrix4;".to_string(),
                methods: vec![],
                string_indices: vec![],
                literal_indices: vec![],
                module_literal_indices: vec![],
                source_code_strings: vec![],
                access_flags: 0x0001,
                source_lang: SOURCE_LANG_ECMASCRIPT,
                source_file: None,
                module_requests: vec![],
                fields: vec![sdk_native_field("@native.ohos.matrix4")],
            },
            ClassRecord {
                name: "L@system.app;".to_string(),
                methods: vec![],
                string_indices: vec![],
                literal_indices: vec![],
                module_literal_indices: vec![],
                source_code_strings: vec![],
                access_flags: 0x0001,
                source_lang: SOURCE_LANG_ECMASCRIPT,
                source_file: None,
                module_requests: vec![],
                fields: vec![sdk_native_field("@native.system.app")],
            },
            ClassRecord {
                name: "L@system.curves;".to_string(),
                methods: vec![],
                string_indices: vec![],
                literal_indices: vec![],
                module_literal_indices: vec![],
                source_code_strings: vec![],
                access_flags: 0x0001,
                source_lang: SOURCE_LANG_ECMASCRIPT,
                source_file: None,
                module_requests: vec![],
                fields: vec![sdk_native_field("@native.system.curves")],
            },
            ClassRecord {
                name: "L@system.matrix4;".to_string(),
                methods: vec![],
                string_indices: vec![],
                literal_indices: vec![],
                module_literal_indices: vec![],
                source_code_strings: vec![],
                access_flags: 0x0001,
                source_lang: SOURCE_LANG_ECMASCRIPT,
                source_file: None,
                module_requests: vec![],
                fields: vec![sdk_native_field("@native.system.matrix4")],
            },
            ClassRecord {
                name: "L@system.router;".to_string(),
                methods: vec![],
                string_indices: vec![],
                literal_indices: vec![],
                module_literal_indices: vec![],
                source_code_strings: vec![],
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
                string_indices: vec![],
                literal_indices: vec![],
                module_literal_indices: vec![],
                source_code_strings: vec![],
                access_flags: 0x2001,
                source_lang: SOURCE_LANG_ECMASCRIPT,
                source_file: None,
                module_requests: vec![],
                fields: vec![],
            },
            ClassRecord {
                name: "L_ESSlotNumberAnnotation;".to_string(),
                methods: vec![],
                string_indices: vec![],
                literal_indices: vec![],
                module_literal_indices: vec![],
                source_code_strings: vec![],
                access_flags: 0x2001,
                source_lang: SOURCE_LANG_ECMASCRIPT,
                source_file: None,
                module_requests: vec![],
                fields: vec![],
            },
        ];

        // Physical ClassItem order follows libpandafile ItemContainer insertion:
        // annotation records, user records, then fixed SDK stubs.  The top-level
        // class index is sorted separately below.
        let all_classes_owned: Vec<ClassRecord> = annotation_stubs
            .iter()
            .cloned()
            .chain(self.records.iter().cloned())
            .chain(sdk_stubs.iter().cloned())
            .map(class_with_upstream_method_order)
            .collect();
        let all_classes: Vec<&ClassRecord> = all_classes_owned.iter().collect();
        let mut class_index_classes = all_classes.clone();
        class_index_classes.sort_by(|a, b| a.name.cmp(&b.name));

        let total_classes = all_classes.len() as u32;
        let class_idx_off = HEADER_SIZE as u32; // 0x3C
        let method_index_count = all_classes
            .iter()
            .map(|class| {
                method_index_string_refs(class, &self.strings).len()
                    + method_index_method_indices(class).len()
                    + class.literal_indices.len()
            })
            .sum::<usize>() as u32;

        let num_lnp = all_classes
            .iter()
            .flat_map(|class| class.methods.iter())
            .filter(|method| method.debug_info.is_some())
            .count() as u32;

        // hvigor places the LNP index at the very end of the file (after the literal-array
        // section), not between the class index and literal-array index.  We match that layout
        // so `lnp_idx_off` is patched into the header after `total_size` is known.
        let num_literal_arrays = self.literal_arrays.len() as u32;
        let literalarray_idx_off = if num_literal_arrays == 0 {
            0
        } else {
            // Literal-array index follows the class index immediately.
            class_idx_off + total_classes * 4
        };

        // IndexSection is placed after the class index and literal-array index (no LNP gap).
        let idx_sect_off = class_idx_off + total_classes * 4 + num_literal_arrays * 4;
        let index_class_count = 2 + 1 + self.records.len() as u32 + sdk_stubs.len() as u32;
        let index_header_size: u32 = 40;
        let index_class_idx_off = idx_sect_off + index_header_size;
        let index_method_idx_off = index_class_idx_off + index_class_count * 4;
        let idx_sect_size = index_header_size + index_class_count * 4 + method_index_count * 4;

        // libpandafile writes ClassItems immediately after the IndexSection.
        // The MethodItem/FieldItem string offsets inside those ClassItems may
        // point forward to StringItems emitted later.
        let class_section_start = idx_sect_off + idx_sect_size;
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

        let string_section_start =
            class_section_start + class_item_sizes.iter().sum::<usize>() as u32;

        // Build the class-OHM-URL prefix (strip "L" prefix and ";" suffix).
        // hvigor stores full-path method strings: "<OHM_URL>.<method_short_name>"
        // e.g. "com.example.app/entry/ets/pages/Index.#~@0>#initialRender"
        let class_ohm_urls: Vec<String> = all_classes
            .iter()
            .map(|c| {
                c.name
                    .strip_prefix('L')
                    .and_then(|s| s.strip_suffix(';'))
                    .unwrap_or(&c.name)
                    .to_string()
            })
            .collect();

        let mut string_items = StringItemWriter::new(string_section_start);

        let mut bytecode_string_offsets = vec![0u32; self.strings.len()];
        let mut emitted_bytecode_strings = vec![false; self.strings.len()];
        let mut supplemental_string_offsets =
            std::collections::HashMap::<(usize, String), u32>::new();
        let mut method_string_offsets: Vec<Vec<u32>> = all_classes
            .iter()
            .map(|class| vec![0; class.methods.len()])
            .collect();
        let mut field_name_string_offsets: Vec<Vec<u32>> = all_classes
            .iter()
            .map(|class| vec![0; class.fields.len()])
            .collect();
        let mut source_file_string_offsets: Vec<Option<u32>> = vec![None; all_classes.len()];
        let mut literal_string_offsets: Vec<Vec<Option<u32>>> = self
            .literal_arrays
            .iter()
            .map(|array| vec![None; array.entries.len()])
            .collect();
        let mut param_name_offsets_by_method: Vec<Vec<Option<Vec<(String, i32, u32)>>>> =
            all_classes
                .iter()
                .map(|class| vec![None; class.methods.len()])
                .collect();

        for (class_idx, class) in all_classes.iter().enumerate() {
            let mut class_pool_strings = Vec::new();
            for string_ref in method_index_string_refs(class, &self.strings) {
                match &string_ref {
                    MethodIndexStringRef::Bytecode(string_idx) => {
                        let string_idx = *string_idx as usize;
                        if emitted_bytecode_strings[string_idx] {
                            continue;
                        }
                        emitted_bytecode_strings[string_idx] = true;
                    }
                    MethodIndexStringRef::Supplemental(_) => {}
                }
                class_pool_strings.push(PoolStringItem::MethodIndex(string_ref));
            }
            let ohm_url = &class_ohm_urls[class_idx];
            for method_idx in method_index_method_indices(class) {
                class_pool_strings.push(PoolStringItem::FullMethodName(format!(
                    "{ohm_url}.{}",
                    class.methods[method_idx].name
                )));
            }
            class_pool_strings.sort_by(|a, b| {
                a.value(&self.strings)
                    .cmp(b.value(&self.strings))
                    .then_with(|| a.kind_order().cmp(&b.kind_order()))
            });
            for item in class_pool_strings {
                match item {
                    PoolStringItem::MethodIndex(MethodIndexStringRef::Bytecode(string_idx)) => {
                        let off =
                            string_items.get_or_create(self.strings.get(string_idx).unwrap_or(""));
                        bytecode_string_offsets[string_idx as usize] = off;
                    }
                    PoolStringItem::MethodIndex(MethodIndexStringRef::Supplemental(name)) => {
                        let off = string_items.get_or_create(&name);
                        supplemental_string_offsets.insert((class_idx, name.clone()), off);
                    }
                    PoolStringItem::FullMethodName(name) => {
                        string_items.get_or_create(&name);
                    }
                }
            }

            for (field_idx, field) in class.fields.iter().enumerate() {
                field_name_string_offsets[class_idx][field_idx] =
                    string_items.get_or_create(&field.name);
            }

            let mut function_order: Vec<usize> = (0..class.methods.len()).collect();
            function_order.sort_by(|&a, &b| class.methods[a].name.cmp(&class.methods[b].name));
            for method_idx in function_order {
                let method = &class.methods[method_idx];
                method_string_offsets[class_idx][method_idx] =
                    string_items.get_or_create(&method.name);

                if let Some(debug) = &method.debug_info {
                    let is_arrow = method.name.contains("@1*#")
                        || method.name == "#*#"
                        || method.name.starts_with("#*#^");
                    if is_arrow {
                        string_items.get_or_create("0newTarget");
                        string_items.get_or_create("0this");
                        string_items.get_or_create("4funcObj");
                    } else {
                        string_items.get_or_create("4funcObj");
                        string_items.get_or_create("any");
                        string_items.get_or_create("4newTarget");
                        if method.name.contains("#~@0=") {
                            string_items.get_or_create("args");
                        }
                    }
                    let mut param_entries: Vec<(String, i32)> = debug
                        .param_names
                        .iter()
                        .enumerate()
                        .map(|(param_idx, param_name)| (param_name.clone(), 3 + param_idx as i32))
                        .collect();
                    param_entries.sort_by(|a, b| a.0.cmp(&b.0));
                    let param_offsets = param_entries
                        .into_iter()
                        .map(|(name, reg)| {
                            let off = string_items.get_or_create(&name);
                            (name, reg, off)
                        })
                        .collect();
                    if !is_arrow {
                        string_items.get_or_create("this");
                    }
                    param_name_offsets_by_method[class_idx][method_idx] = Some(param_offsets);
                }

                if let Some(source_file) = &class.source_file {
                    let off = string_items.get_or_create(source_file);
                    source_file_string_offsets[class_idx].get_or_insert(off);
                }
            }

            for source_code in &class.source_code_strings {
                string_items.get_or_create(source_code);
            }

            for &array_idx in &class.module_literal_indices {
                let array = &self.literal_arrays[array_idx];
                for (entry_idx, entry) in array.entries.iter().enumerate() {
                    let value = match entry {
                        LiteralEntry::TaggedString(value) | LiteralEntry::RawString(value) => value,
                        _ => continue,
                    };
                    let off = string_items.get_or_create(value);
                    literal_string_offsets[array_idx][entry_idx] = Some(off);
                }
            }

            for &array_idx in &class.literal_indices {
                let array = &self.literal_arrays[array_idx];
                for (entry_idx, entry) in array.entries.iter().enumerate() {
                    let value = match entry {
                        LiteralEntry::TaggedString(value) | LiteralEntry::RawString(value) => value,
                        _ => continue,
                    };
                    let off = string_items.get_or_create(value);
                    literal_string_offsets[array_idx][entry_idx] = Some(off);
                }
            }
        }

        for (array_idx, array) in self.literal_arrays.iter().enumerate() {
            for (entry_idx, entry) in array.entries.iter().enumerate() {
                if literal_string_offsets[array_idx][entry_idx].is_some() {
                    continue;
                }
                let value = match entry {
                    LiteralEntry::TaggedString(value) | LiteralEntry::RawString(value) => value,
                    _ => continue,
                };
                let off = string_items.get_or_create(value);
                literal_string_offsets[array_idx][entry_idx] = Some(off);
            }
        }

        let mut per_lnp_param_name_offsets: Vec<Vec<(String, i32, u32)>> = Vec::new();
        for (class_idx, class) in all_classes.iter().enumerate() {
            if source_file_string_offsets[class_idx].is_none() {
                continue;
            }
            let mut method_order: Vec<usize> = (0..class.methods.len()).collect();
            method_order.sort_by(|&a, &b| class.methods[a].name.cmp(&class.methods[b].name));
            for method_idx in method_order {
                if class.methods[method_idx].debug_info.is_some() {
                    per_lnp_param_name_offsets.push(
                        param_name_offsets_by_method[class_idx][method_idx]
                            .clone()
                            .unwrap_or_default(),
                    );
                }
            }
        }

        let scope_offs_regular: [u32; 3] = [
            string_items.get_or_create("4funcObj"),
            string_items.get_or_create("4newTarget"),
            string_items.get_or_create("this"),
        ];
        let scope_offs_arrow: [u32; 3] = [
            string_items.get_or_create("0newTarget"),
            string_items.get_or_create("0this"),
            string_items.get_or_create("4funcObj"),
        ];
        let any_type_off = string_items.get_or_create("any");
        let not_supported_source_code_off = string_items.get_or_create("not supported");
        let slot_number_str_off = string_items.get_or_create("SlotNumber");
        let mut cur = string_items.cur();

        let mut method_name_offsets = std::collections::HashMap::new();
        for (class, &class_off) in all_classes.iter().zip(class_item_offsets.iter()) {
            for (method, method_off) in class
                .methods
                .iter()
                .zip(class.method_item_offsets(class_off))
            {
                method_name_offsets.insert(method.name.as_str(), method_off);
            }
        }

        // Build the IndexHeader class table used by FieldItem/MethodItem
        // `class_idx` values. hvigor keeps two non-EntityId scalar entries at
        // the front, then `_ESSlotNumberAnnotation`, user records, and SDK
        // namespace stubs. `_ESConcurrentModuleRequestsAnnotation` is present
        // in the top-level class index but not in this typed class table.
        let mut index_class_offsets = Vec::with_capacity(index_class_count as usize);
        index_class_offsets.extend_from_slice(&[2, 6]);
        let mut name_to_index_class_idx = std::collections::HashMap::new();
        for (i, class) in std::iter::once(&annotation_stubs[1])
            .chain(self.records.iter())
            .chain(sdk_stubs.iter())
            .enumerate()
        {
            let off = *name_to_class_offset
                .get(class.name.as_str())
                .expect("indexed class name must have a ClassItem offset");
            index_class_offsets.push(off);
            // Entries 0 and 1 are scalar placeholders; user classes start at index 2.
            name_to_index_class_idx.insert(class.name.as_str(), 2u16 + i as u16);
        }
        debug_assert_eq!(index_class_offsets.len() as u32, index_class_count);

        let method_pool_layouts = build_method_pool_layouts(&all_classes, &self.strings);
        let literal_emit_order = build_literal_emit_order(&all_classes, self.literal_arrays.len());
        let literal_header_order = build_literal_header_order(&all_classes, &self.literal_arrays);

        // ── 3. Pre-compute LiteralArray section bytes ─────────────────────────
        //
        // libpandafile places LiteralArray payloads before CodeItems.  The
        // literal-array index near the header points at these payload offsets.
        // Format per LiteralArray:
        //   [num_items: u32 LE]
        //   Tagged entries: [tag_value: u8] [value: variable]
        //   Raw module entries: [value: variable]
        let (literal_array_offsets, lit_arr_section): (Vec<u32>, Vec<u8>) = {
            let mut buf = Vec::new();
            let mut offsets = vec![0u32; self.literal_arrays.len()];
            let mut offset_cur = cur;
            for array_idx in literal_emit_order.iter().copied() {
                let lit_arr = &self.literal_arrays[array_idx];
                offsets[array_idx] = offset_cur;
                let before = buf.len();
                let item_count = lit_arr
                    .entries
                    .iter()
                    .map(LiteralEntry::item_count)
                    .sum::<u32>();
                buf.extend_from_slice(&item_count.to_le_bytes());
                for (entry_idx, entry) in lit_arr.entries.iter().enumerate() {
                    match entry {
                        LiteralEntry::TaggedInteger(v) => {
                            buf.push(0x02);
                            buf.extend_from_slice(&v.to_le_bytes());
                        }
                        LiteralEntry::TaggedString(_) => {
                            let offset = literal_string_offsets[array_idx][entry_idx]
                                .expect("tagged string literal offset must be resolved");
                            buf.push(0x05);
                            buf.extend_from_slice(&offset.to_le_bytes());
                        }
                        LiteralEntry::TaggedMethod(off) => {
                            buf.push(0x06);
                            buf.extend_from_slice(&off.to_le_bytes());
                        }
                        LiteralEntry::TaggedMethodName(name) => {
                            let off = method_name_offsets
                                .get(name.as_str())
                                .copied()
                                .expect("tagged method literal offset must be resolved");
                            buf.push(0x06);
                            buf.extend_from_slice(&off.to_le_bytes());
                        }
                        LiteralEntry::TaggedMethodAffiliate(value) => {
                            buf.push(0x09);
                            buf.extend_from_slice(&value.to_le_bytes());
                        }
                        LiteralEntry::TaggedNull => {
                            buf.push(0xff);
                            buf.push(0);
                        }
                        LiteralEntry::RawU32(value) => {
                            buf.extend_from_slice(&value.to_le_bytes());
                        }
                        LiteralEntry::RawU16(value) => {
                            buf.extend_from_slice(&value.to_le_bytes());
                        }
                        LiteralEntry::RawString(_) => {
                            let offset = literal_string_offsets[array_idx][entry_idx]
                                .expect("raw string literal offset must be resolved");
                            buf.extend_from_slice(&offset.to_le_bytes());
                        }
                    }
                }
                offset_cur += (buf.len() - before) as u32;
            }
            (offsets, buf)
        };
        cur += lit_arr_section.len() as u32;

        // ── 4. Pre-compute AnnotationItem section ────────────────────────────
        //
        // hvigor emits one `L_ESSlotNumberAnnotation` item per method and places
        // the AnnotationItems before CodeItems in the referenced item area.
        //
        // AnnotationItem format (libpandafile annotation_data_accessor.h):
        //   class_idx: u16 LE  (2 = L_ESSlotNumberAnnotation in IndexSection)
        //   count:     u16 LE  (1 element)
        //   elements:  [(name_off: u32 LE, value: u32 LE)] * count
        //   tags:      [tag: u8] * count  (0x37 = 'U32' type tag, after ALL elements)
        // = 2+2+8+1 = 13 bytes per annotation
        let annotation_section_start = cur;
        let total_annotations: u32 = all_classes.iter().map(|c| c.methods.len() as u32).sum();
        let annotation_section_size = total_annotations * 13; // 13 bytes per annotation
        let annotation_order_per_class: Vec<Vec<usize>> = all_classes
            .iter()
            .map(|class| {
                let mut order: Vec<usize> = (0..class.methods.len()).collect();
                order.sort_by(|&a, &b| class.methods[a].name.cmp(&class.methods[b].name));
                order
            })
            .collect();

        // Per-class annotation offsets (parallel to methods).
        let mut annotation_offsets_per_class: Vec<Vec<u32>> = Vec::with_capacity(all_classes.len());
        {
            let mut ann_cur = annotation_section_start;
            for (class_idx, class) in all_classes.iter().enumerate() {
                let mut class_ann_offs = Vec::with_capacity(class.methods.len());
                class_ann_offs.resize(class.methods.len(), 0);
                for &method_idx in &annotation_order_per_class[class_idx] {
                    class_ann_offs[method_idx] = ann_cur;
                    ann_cur += 13;
                }
                annotation_offsets_per_class.push(class_ann_offs);
            }
        }
        cur += annotation_section_size;

        // ── 5. Compute CodeItem start offsets ─────────────────────────────────
        let code_section_start: u32 = cur;

        let mut code_offsets: Vec<Vec<u32>> = Vec::new();
        let mut all_code_items: Vec<Vec<u8>> = Vec::new();
        let mut slot_counts_per_class: Vec<Vec<u32>> = all_classes
            .iter()
            .map(|class| vec![0; class.methods.len()])
            .collect();

        let mut code_cur = code_section_start;
        for (class_idx, class) in all_classes.iter().enumerate() {
            let mut class_code_offs = vec![0u32; class.methods.len()];
            let mut class_code_items = Vec::with_capacity(class.methods.len());
            let layout = &method_pool_layouts[class_idx];
            let mut definefunc_cursor = 0usize;
            for method in &class.methods {
                let bytecode = patch_bytecode_index_operands(
                    &method.bytecode,
                    &layout.string_pool_indices,
                    &layout.literal_pool_indices,
                    layout.ctor_pool_index,
                    &layout.definefunc_pool_indices,
                    &layout.definemethod_pool_indices_by_name,
                    &mut definefunc_cursor,
                );
                let mut bytecode = bytecode;
                let slot_count = assign_inline_cache_slots(&mut bytecode);
                slot_counts_per_class[class_idx][class_code_items.len()] = slot_count;
                let ci = encode_code_item(
                    method.num_vregs,
                    method.num_args,
                    &bytecode,
                    &method.catch_entries,
                );
                class_code_items.push(ci);
            }
            let mut code_order: Vec<usize> = (0..class.methods.len()).collect();
            code_order.sort_by(|&a, &b| class.methods[a].name.cmp(&class.methods[b].name));
            for method_idx in code_order {
                class_code_offs[method_idx] = code_cur;
                let ci = class_code_items[method_idx].clone();
                code_cur += ci.len() as u32;
                all_code_items.push(ci);
            }
            code_offsets.push(class_code_offs);
        }

        // ── Pre-compute DebugInfo and LineNumberProgram sections ─────────────
        // Each debug method gets an LNP: SET_FILE + optional advance sequences +
        // END_SEQUENCE.  The source-file offset is stored in the DebugInfo
        // constant pool, matching libpandafile's EmitSetFile convention.
        //
        // When the method carries pre-computed `line_ops` (ADVANCE_PC,
        // ADVANCE_LINE, SPECIAL_OPCODE, END_SEQUENCE) from the IR builder, those
        // are embedded verbatim after the SET_FILE preamble.  Otherwise, a bare
        // END_SEQUENCE is appended for a minimal but valid LNP.
        #[allow(clippy::type_complexity)]
        let (debug_info_offsets, debug_lnp_section, lnp_offsets): (
            Vec<Vec<Option<u32>>>,
            Vec<u8>,
            Vec<u32>,
        ) = {
            let mut per_class_offsets = Vec::with_capacity(all_classes.len());
            let mut section_buf = Vec::new();
            // DebugInfo/LNP section starts after AnnotationItems + CodeItems.
            let mut cur = code_cur;
            let mut lnp_index = 0u32;

            // ── Correct LNP format (libpandafile architecture) ─────────────────
            //
            // The LNP `data_` stream contains opcode bytes + inline SLEB128 register
            // numbers (for START_LOCAL*, END_LOCAL). All other operands (string offsets,
            // pc/line deltas) go in the DebugInfo `constant_pool`.
            //
            // libpandafile writes each LineNumberProgram immediately before the
            // DebugInfo that references it.  Methods follow the same name-sorted
            // order used for CodeItems, while ClassItem method offsets remain in
            // the original method vector order.
            let _epilogue_pc_delta: u32 = 4;
            let mut lnp_offsets = Vec::with_capacity(num_lnp as usize);

            for (class_idx, class) in all_classes.iter().enumerate() {
                let mut class_offsets = vec![None; class.methods.len()];
                let source_file_off = source_file_string_offsets[class_idx];
                let mut method_order: Vec<usize> = (0..class.methods.len()).collect();
                method_order.sort_by(|&a, &b| class.methods[a].name.cmp(&class.methods[b].name));
                for method_idx in method_order {
                    let method = &class.methods[method_idx];
                    if let (Some(debug), Some(src_off)) = (&method.debug_info, source_file_off) {
                        let param_offs = per_lnp_param_name_offsets
                            .get(lnp_index as usize)
                            .cloned()
                            .unwrap_or_default();
                        let method_ops = method.line_ops.as_slice();

                        // ── Build LNP data_ + constant_pool matching BuildLineNumberProgram ──
                        //
                        // Per libpandafile/assembly-emitter.cpp BuildLineNumberProgram:
                        //   SET_FILE → cp: src_off
                        //   ADV_PC(initial) → cp: initial_pc_delta
                        //   SLE(reg, name, type, typesig) for each scope var
                        //   [body: per-instruction ADV_LINE/SP + SET_COL + ADV_PC]
                        //   EPIL_BEGIN
                        //   ADV_PC(1) → cp: 1
                        //   END_L(reg) for each scope var → cp: closing_pc_offset
                        //   END
                        //
                        // Scope var register layout (from es2panda GenScopeVariableInfo):
                        //   Regular: r0=funcObj, r1=newTarget, r2=this
                        //   Arrow:   r1=newTarget, r2=this, r0=funcObj
                        let is_arrow = method.name.contains("@1*#")
                            || method.name == "#*#"
                            || method.name.starts_with("#*#^");
                        let mut param_offs = param_offs;
                        let is_generated_constructor = method.name.contains("#~@0=")
                            && param_offs.is_empty()
                            && method.num_args > 3;
                        if is_generated_constructor {
                            let args_off = string_items.id_of("args").expect(
                                "args string offset must be interned for constructor debug info",
                            );
                            param_offs.push(("args".to_string(), 3, args_off));
                        }

                        let mut lnp_data: Vec<u8> = Vec::new();
                        let mut cp: Vec<u8> = Vec::new();

                        // ── Build LNP matching libpandafile BuildLineNumberProgram exactly ──
                        //
                        // Pattern from golden binary analysis:
                        //   SF → cp: src_off
                        //   PC(num_args×2) → cp: prologue_size  [arg-copy prologue]
                        //   SLE(reg) × (3+params) → cp: name,type,typesig per var
                        //   [body from line_ops: LN+SP+SC+PC per instruction]
                        //   EL(reg) × (3+params) → cp: closing_pc=0
                        //   END

                        // SET_SOURCE_CODE is emitted for module func_main_0 by
                        // es2panda when record debug source is disabled.
                        if method.name == "func_main_0" {
                            lnp_data.push(0x0a);
                            cp.extend_from_slice(&uleb128(not_supported_source_code_off as u64));
                        }

                        // SET_FILE
                        lnp_data.push(0x09);
                        cp.extend_from_slice(&uleb128(src_off as u64));

                        // ADV_PC(prologue_size): skip hidden entry instructions
                        // before the source-visible function scope starts.
                        // es2panda emits the real mov.v4/mov.v8/mov.v16 argument
                        // copy width, and module entries keep a leading
                        // newlexenvwithname before those copies.
                        let prologue_size = debug_scope_start_pc(method);
                        lnp_data.push(0x01); // ADV_PC
                        cp.extend_from_slice(&uleb128(prologue_size as u64));

                        // START_LOCAL_EXTENDED for scope vars.  Non-arrow
                        // functions emit `this` after regular locals because
                        // es2panda records function-scope bindings first, then
                        // the mandatory `this` binding.
                        let pre_scope_locals: Vec<(String, i32, u32)> = if method.name
                            == "func_main_0"
                        {
                            class
                                .source_code_strings
                                .iter()
                                .filter(|name| name.starts_with('#') && name.ends_with("_Params"))
                                .filter_map(|name| {
                                    string_items.id_of(name).map(|off| (name.clone(), 3, off))
                                })
                                .collect()
                        } else {
                            Vec::new()
                        };
                        for &(_, reg, name_off) in &pre_scope_locals {
                            lnp_data.push(0x04);
                            lnp_data.extend_from_slice(&encode_sleb128_bytes(reg));
                            cp.extend_from_slice(&uleb128(name_off as u64));
                            cp.extend_from_slice(&uleb128(any_type_off as u64));
                            cp.extend_from_slice(&uleb128(any_type_off as u64));
                        }

                        let start_scope: Vec<(i32, u32)> = if is_arrow {
                            vec![
                                (1, scope_offs_arrow[0]),
                                (2, scope_offs_arrow[1]),
                                (0, scope_offs_arrow[2]),
                            ]
                        } else {
                            vec![(0, scope_offs_regular[0]), (1, scope_offs_regular[1])]
                        };
                        for (reg, name_off) in start_scope {
                            lnp_data.push(0x04);
                            lnp_data.extend_from_slice(&encode_sleb128_bytes(reg));
                            cp.extend_from_slice(&uleb128(name_off as u64)); // name
                            cp.extend_from_slice(&uleb128(any_type_off as u64)); // type="any"
                            cp.extend_from_slice(&uleb128(any_type_off as u64));
                            // typesig="any"
                        }
                        // START_LOCAL_EXTENDED for locals/params, sorted like
                        // es2panda's scope binding map but keeping original
                        // register assignments.
                        let mut local_offs = param_offs.clone();
                        if !is_arrow {
                            if !debug.lexical_this {
                                local_offs.push(("this".to_string(), 2, scope_offs_regular[2]));
                            }
                            local_offs.sort_by(|a, b| a.0.cmp(&b.0));
                        }
                        for &(_, reg, param_off) in &local_offs {
                            lnp_data.push(0x04);
                            lnp_data.extend_from_slice(&encode_sleb128_bytes(reg));
                            cp.extend_from_slice(&uleb128(param_off as u64));
                            cp.extend_from_slice(&uleb128(any_type_off as u64));
                            cp.extend_from_slice(&uleb128(any_type_off as u64));
                        }

                        // Body opcodes + cp from line_ops, interleaving local
                        // variable events that start/end inside the instruction
                        // stream. This matches libpandafile
                        // BuildLineNumberProgram, which flushes pending PC
                        // before local start/end events.
                        let mid_locals =
                            infer_func_main_class_local_events(method, class, &string_items);
                        append_line_ops_with_mid_locals(
                            method_ops,
                            prologue_size,
                            &mid_locals,
                            any_type_off,
                            &mut lnp_data,
                            &mut cp,
                        );

                        // END_LOCAL in the same order as START_LOCAL.
                        let end_regs: Vec<i32> = if is_arrow {
                            vec![1, 2, 0]
                        } else {
                            let mut regs: Vec<i32> =
                                pre_scope_locals.iter().map(|(_, reg, _)| *reg).collect();
                            regs.extend([0, 1]);
                            regs.extend(local_offs.iter().map(|(_, reg, _)| *reg));
                            regs
                        };
                        for reg in end_regs {
                            lnp_data.push(0x05); // END_LOCAL
                            lnp_data.extend_from_slice(&encode_sleb128_bytes(reg));
                        }
                        if is_arrow {
                            for (_, reg, _) in &param_offs {
                                lnp_data.push(0x05); // END_LOCAL
                                lnp_data.extend_from_slice(&encode_sleb128_bytes(*reg));
                            }
                        }

                        lnp_data.push(0x00); // END_SEQUENCE

                        lnp_offsets.push(cur);
                        section_buf.extend_from_slice(&lnp_data);
                        cur += lnp_data.len() as u32;

                        class_offsets[method_idx] = Some(cur);
                        let di_bytes =
                            encode_debug_info_with_cp(debug.line_start, &[], &cp, lnp_index);
                        section_buf.extend_from_slice(&di_bytes);
                        cur += di_bytes.len() as u32;
                        lnp_index += 1;
                    }
                }
                per_class_offsets.push(class_offsets);
            }

            debug_assert_eq!(lnp_index, num_lnp);
            (per_class_offsets, section_buf, lnp_offsets)
        };

        // post_lnp_offset = after LiteralArrays + CodeItems + AnnotationItems + interleaved DebugInfo/LNP data.
        let post_lnp_offset = code_cur + debug_lnp_section.len() as u32;

        let total_size = post_lnp_offset;

        let mut method_index_offsets = Vec::with_capacity(method_index_count as usize);
        // Method index ordering follows libpandafile's per-record region:
        // this record's bytecode strings, method literals referenced by bytecode,
        // then literal arrays referenced by bytecode.  It is not a single global
        // [all strings | all methods | all literals] block.
        for (class_idx, ((class, &class_off), layout)) in all_classes
            .iter()
            .zip(class_item_offsets.iter())
            .zip(method_pool_layouts.iter())
            .enumerate()
        {
            for string_ref in &layout.string_refs {
                match string_ref {
                    MethodIndexStringRef::Bytecode(string_idx) => {
                        method_index_offsets.push(bytecode_string_offsets[*string_idx as usize]);
                    }
                    MethodIndexStringRef::Supplemental(name) => {
                        let off = supplemental_string_offsets
                            .get(&(class_idx, name.clone()))
                            .copied()
                            .expect("supplemental method-index string offset must be resolved");
                        method_index_offsets.push(off);
                    }
                }
            }
            let method_item_offsets = class.method_item_offsets(class_off);
            for &method_idx in &layout.method_indices {
                method_index_offsets.push(method_item_offsets[method_idx]);
            }
            for &literal_idx in &class.literal_indices {
                method_index_offsets.push(literal_array_offsets[literal_idx]);
            }
        }
        debug_assert_eq!(method_index_offsets.len() as u32, method_index_count);

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
                    &annotation_offsets_per_class[ci],
                    &literal_array_offsets,
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
        // LNP index is placed at the end of the file and aligned to 4 bytes;
        // hvigor pads between the DebugInfo/LNP payload and the index when needed.
        let lnp_idx_off = if num_lnp == 0 { 0 } else { align4(total_size) };
        let final_size = if num_lnp == 0 {
            total_size
        } else {
            lnp_idx_off + num_lnp * 4
        };

        let mut file: Vec<u8> = Vec::with_capacity(final_size as usize + 4);

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
        file.extend_from_slice(&lnp_idx_off.to_le_bytes()); // 0x28 lnp_idx_off (at file end)
        file.extend_from_slice(&num_literal_arrays.to_le_bytes()); // 0x2C num_literalarrays
        file.extend_from_slice(&literalarray_idx_off.to_le_bytes()); // 0x30 literalarray_idx_off
        file.extend_from_slice(&1u32.to_le_bytes()); // 0x34 num_idx_regions = 1
        file.extend_from_slice(&idx_sect_off.to_le_bytes()); // 0x38 idx_section_off
        debug_assert_eq!(file.len(), HEADER_SIZE, "header must be exactly 0x3C bytes");

        // ── Class index (at 0x3C, sorted by class name) ───────────────────────
        for class in &class_index_classes {
            let off = *name_to_class_offset
                .get(class.name.as_str())
                .expect("class name must be in class index");
            file.extend_from_slice(&off.to_le_bytes());
        }
        debug_assert_eq!(
            file.len() as u32,
            class_idx_off + total_classes * 4,
            "class index must end exactly before the literal-array index"
        );

        // ── LiteralArray index ───────────────────────────────────────────────
        //
        // hvigor places the literal-array index immediately after the class index
        // (no LNP index in between).  API 17 output stores one u32 file offset per array.
        for &literal_idx in &literal_header_order {
            file.extend_from_slice(&literal_array_offsets[literal_idx].to_le_bytes());
        }
        debug_assert_eq!(
            file.len() as u32,
            idx_sect_off,
            "literal-array index must end exactly at idx_sect_off"
        );

        // ── IndexSection ─────────────────────────────────────────────────────
        //
        // libpandafile expects a 40-byte IndexHeader followed by EntityId
        // arrays.  The entity region spans from the first ClassItem to the end
        // of the complete file (including the trailing LNP index).
        file.extend_from_slice(&first_class_off.to_le_bytes()); // start
        file.extend_from_slice(&final_size.to_le_bytes()); // end (full file including LNP idx)
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

        debug_assert_eq!(file.len() as u32, class_section_start);

        // ── ClassItems ────────────────────────────────────────────────────────
        for class_bytes in &class_item_bytes {
            file.extend_from_slice(class_bytes);
        }
        debug_assert_eq!(file.len() as u32, string_section_start);

        // ── StringItems ──────────────────────────────────────────────────────
        for s in string_items.items() {
            file.extend_from_slice(s);
        }

        if let Some(first_literal_array_off) = literal_array_offsets.iter().min().copied() {
            debug_assert_eq!(file.len() as u32, first_literal_array_off);
        }

        // ── LiteralArray section ──────────────────────────────────────────────
        file.extend_from_slice(&lit_arr_section);

        debug_assert_eq!(file.len() as u32, annotation_section_start);

        // ── AnnotationItems (L_ESSlotNumberAnnotation, one per method) ────────
        //
        // Format per annotation (libpandafile AnnotationDataAccessor):
        //   class_idx: u16 LE  (2 = L_ESSlotNumberAnnotation in IndexSection)
        //   count:     u16 LE  (1 element)
        //   elements:  [(name_off: u32 LE, value: u32 LE)] * count
        //   tags:      [tag: u8] * count  (0x37 = 'U32' type tag)
        // = 2+2+8+1 = 13 bytes per annotation
        //
        for (class_idx, _class) in all_classes.iter().enumerate() {
            for &method_idx in &annotation_order_per_class[class_idx] {
                let slot_count = slot_counts_per_class[class_idx][method_idx];
                file.extend_from_slice(&2u16.to_le_bytes()); // class_idx = 2 (L_ESSlotNumberAnnotation)
                file.extend_from_slice(&1u16.to_le_bytes()); // count = 1
                file.extend_from_slice(&slot_number_str_off.to_le_bytes()); // name_off → "SlotNumber"
                file.extend_from_slice(&slot_count.to_le_bytes()); // value = IC slot count
                file.push(0x37); // tag = 'U32' (0x37 = ASCII '7')
            }
        }
        debug_assert_eq!(file.len() as u32, code_section_start);

        // ── CodeItems ─────────────────────────────────────────────────────────
        for code in &all_code_items {
            file.extend_from_slice(code);
        }
        debug_assert_eq!(file.len() as u32, code_cur);

        // ── DebugInfo and LineNumberProgram sections ────────────────────────
        file.extend_from_slice(&debug_lnp_section);
        debug_assert_eq!(file.len() as u32, total_size);

        // ── LineNumberProgram index (at file end, after literal arrays) ────────
        //
        // hvigor places the LNP index as the final section of the file.  The
        // `lnp_idx_off` header field points here.
        while (file.len() as u32) < lnp_idx_off {
            file.push(0);
        }
        for offset in &lnp_offsets {
            file.extend_from_slice(&offset.to_le_bytes());
        }
        debug_assert_eq!(file.len() as u32, final_size);

        // ── Patch file_size at offset 0x10 ────────────────────────────────────
        let file_size = file.len() as u32;
        file[0x10..0x14].copy_from_slice(&file_size.to_le_bytes());

        // ── Compute Adler-32 checksum over file[12..] and patch offset 0x08 ───
        let checksum = adler32(&file[12..]);
        file[0x08..0x0C].copy_from_slice(&checksum.to_le_bytes());

        file
    }
}

fn patch_bytecode_index_operands(
    bytecode: &[u8],
    string_pool_indices: &std::collections::HashMap<u16, u16>,
    literal_pool_indices: &std::collections::HashMap<u16, u16>,
    ctor_pool_index: u16,
    definefunc_pool_indices: &[u16],
    definemethod_pool_indices_by_name: &std::collections::HashMap<u16, u16>,
    definefunc_cursor: &mut usize,
) -> Vec<u8> {
    let mut patched = bytecode.to_vec();
    let mut pos = 0;
    while pos < patched.len() {
        let Some(opcode) = patched.get(pos).copied() else {
            break;
        };
        match opcode {
            0x3e if pos + 2 < patched.len() => {
                rewrite_u16_operand(&mut patched, pos + 1, string_pool_indices);
                pos += 3;
                continue;
            }
            0x3f | 0x40 | 0x42 if pos + 3 < patched.len() => {
                rewrite_u16_operand(&mut patched, pos + 2, string_pool_indices);
                pos += 4;
                continue;
            }
            0x43 | 0x7a if pos + 4 < patched.len() => {
                rewrite_u16_operand(&mut patched, pos + 2, string_pool_indices);
                pos += 5;
                continue;
            }
            0xfe if patched.get(pos + 1) == Some(&0x09) && pos + 3 < patched.len() => {
                rewrite_u16_operand(&mut patched, pos + 2, string_pool_indices);
                pos += 4;
                continue;
            }
            _ => {}
        }
        if opcode == 0x33 && pos + 4 < patched.len() {
            let method_pos = pos + 2;
            let method_index = if let Some(&index) = definefunc_pool_indices.get(*definefunc_cursor)
            {
                *definefunc_cursor += 1;
                index
            } else {
                ctor_pool_index
            };
            patched[method_pos..method_pos + 2].copy_from_slice(&method_index.to_le_bytes());
            pos += 5;
            continue;
        }
        if opcode == 0x34 && pos + 4 < patched.len() {
            let method_pos = pos + 2;
            let method_name_id = u16::from_le_bytes([patched[method_pos], patched[method_pos + 1]]);
            if let Some(&method_index) = definemethod_pool_indices_by_name.get(&method_name_id) {
                patched[method_pos..method_pos + 2].copy_from_slice(&method_index.to_le_bytes());
            }
            pos += 5;
            continue;
        }
        if opcode == 0x35 && pos + 8 < patched.len() {
            let ctor_pos = pos + 2;
            patched[ctor_pos..ctor_pos + 2].copy_from_slice(&ctor_pool_index.to_le_bytes());
            let buf_pos = pos + 4;
            let old_buf = u16::from_le_bytes([patched[buf_pos], patched[buf_pos + 1]]);
            let new_buf = literal_pool_indices
                .get(&old_buf)
                .copied()
                .unwrap_or(old_buf);
            patched[buf_pos..buf_pos + 2].copy_from_slice(&new_buf.to_le_bytes());
            pos += 9;
            continue;
        }
        if opcode == 0xb6 && pos + 3 < patched.len() {
            let literal_pos = pos + 2;
            let old_literal = u16::from_le_bytes([patched[literal_pos], patched[literal_pos + 1]]);
            let new_literal = literal_pool_indices
                .get(&old_literal)
                .copied()
                .unwrap_or(old_literal);
            patched[literal_pos..literal_pos + 2].copy_from_slice(&new_literal.to_le_bytes());
            pos += 4;
            continue;
        }
        if opcode == 0x07 && pos + 3 < patched.len() {
            let literal_pos = pos + 2;
            let old_literal = u16::from_le_bytes([patched[literal_pos], patched[literal_pos + 1]]);
            let new_literal = literal_pool_indices
                .get(&old_literal)
                .copied()
                .unwrap_or(old_literal);
            patched[literal_pos..literal_pos + 2].copy_from_slice(&new_literal.to_le_bytes());
            pos += 4;
            continue;
        }
        pos += instruction_len(opcode, patched.get(pos + 1).copied());
    }
    patched
}

fn assign_inline_cache_slots(bytecode: &mut [u8]) -> u32 {
    let mut current_slot = 0u32;
    let mut pos = 0usize;
    while pos < bytecode.len() {
        let Some(opcode) = bytecode.get(pos).copied() else {
            break;
        };
        if let Some((operand_pos, inc)) = inline_cache_slot_operand(bytecode, pos, opcode) {
            if let Some(slot) = bytecode.get_mut(operand_pos) {
                *slot = current_slot.min(u8::MAX as u32) as u8;
            }
            current_slot += inc as u32;
        }
        pos += instruction_len(opcode, bytecode.get(pos + 1).copied()).max(1);
    }
    current_slot
}

fn inline_cache_slot_operand(bytecode: &[u8], pos: usize, opcode: u8) -> Option<(usize, u8)> {
    if opcode == 0xfb {
        let sub_opcode = bytecode.get(pos + 1).copied()?;
        let operand_pos = pos + 2;
        if operand_pos >= bytecode.len() {
            return None;
        }
        let inc = match sub_opcode {
            // callruntime.istrue / callruntime.isfalse are one-slot IC users.
            0x13 | 0x14 => 1,
            _ => return None,
        };
        return Some((operand_pos, inc));
    }

    let operand_pos = pos + 1;
    if operand_pos >= bytecode.len() {
        return None;
    }
    let inc = match opcode {
        // one_slot IC instructions, from arkcompiler_runtime_core/isa/isa.yaml.
        0x05
        | 0x07
        | 0x0a..=0x1a
        | 0x1f..=0x22
        | 0x25
        | 0x27
        | 0x28
        | 0x34
        | 0x33
        | 0x35
        | 0x3f
        | 0x40 => 1,
        // two_slot IC instructions.
        0x08 | 0x1c | 0x29..=0x32 | 0x42 | 0x43 | 0x73 | 0x79 | 0x7a | 0xb9 => 2,
        _ => return None,
    };
    Some((operand_pos, inc))
}

fn class_with_upstream_method_order(mut class: ClassRecord) -> ClassRecord {
    class.methods.sort_by(|a, b| {
        let a_len = a.name.encode_utf16().count();
        let b_len = b.name.encode_utf16().count();
        a_len.cmp(&b_len).then_with(|| a.name.cmp(&b.name))
    });
    class
}

fn rewrite_u16_operand(
    patched: &mut [u8],
    operand_pos: usize,
    pool_indices: &std::collections::HashMap<u16, u16>,
) {
    let old = u16::from_le_bytes([patched[operand_pos], patched[operand_pos + 1]]);
    if let Some(&new) = pool_indices.get(&old) {
        patched[operand_pos..operand_pos + 2].copy_from_slice(&new.to_le_bytes());
    }
}

#[derive(Debug, Default)]
struct MethodPoolLayout {
    string_refs: Vec<MethodIndexStringRef>,
    string_pool_indices: std::collections::HashMap<u16, u16>,
    literal_pool_indices: std::collections::HashMap<u16, u16>,
    definemethod_pool_indices_by_name: std::collections::HashMap<u16, u16>,
    method_indices: Vec<usize>,
    definefunc_pool_indices: Vec<u16>,
    ctor_pool_index: u16,
}

struct StringItemWriter {
    cur: u32,
    offsets: std::collections::HashMap<String, u32>,
    items: Vec<Vec<u8>>,
}

impl StringItemWriter {
    fn new(start: u32) -> Self {
        Self {
            cur: start,
            offsets: std::collections::HashMap::new(),
            items: Vec::new(),
        }
    }

    fn get_or_create(&mut self, value: &str) -> u32 {
        if let Some(&off) = self.offsets.get(value) {
            return off;
        }
        let off = self.cur;
        let item = inline_string(value);
        self.cur += item.len() as u32;
        self.offsets.insert(value.to_string(), off);
        self.items.push(item);
        off
    }

    fn id_of(&self, value: &str) -> Option<u32> {
        self.offsets.get(value).copied()
    }

    fn cur(&self) -> u32 {
        self.cur
    }

    fn items(&self) -> &[Vec<u8>] {
        &self.items
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum MethodIndexStringRef {
    Bytecode(u32),
    Supplemental(String),
}

impl MethodIndexStringRef {
    fn value<'a>(&'a self, strings: &'a StringTable) -> &'a str {
        match self {
            MethodIndexStringRef::Bytecode(idx) => strings.get(*idx).unwrap_or(""),
            MethodIndexStringRef::Supplemental(value) => value.as_str(),
        }
    }
}

enum PoolStringItem {
    MethodIndex(MethodIndexStringRef),
    FullMethodName(String),
}

impl PoolStringItem {
    fn value<'a>(&'a self, strings: &'a StringTable) -> &'a str {
        match self {
            PoolStringItem::MethodIndex(string_ref) => string_ref.value(strings),
            PoolStringItem::FullMethodName(name) => name.as_str(),
        }
    }

    fn kind_order(&self) -> u8 {
        match self {
            PoolStringItem::MethodIndex(_) => 0,
            PoolStringItem::FullMethodName(_) => 1,
        }
    }
}

fn build_method_pool_layouts(
    classes: &[&ClassRecord],
    strings: &StringTable,
) -> Vec<MethodPoolLayout> {
    let mut next_pool_index = 0u16;
    let mut global_string_pool_indices = std::collections::HashMap::<u16, u16>::new();
    classes
        .iter()
        .map(|class| {
            let mut layout = MethodPoolLayout::default();
            let mut all_bytecode_string_indices = Vec::new();
            for method in &class.methods {
                collect_bytecode_string_indices(&method.bytecode, &mut all_bytecode_string_indices);
            }
            all_bytecode_string_indices.sort_unstable();
            all_bytecode_string_indices.dedup();

            layout.string_refs = method_index_string_refs(class, strings);
            for string_ref in &layout.string_refs {
                if let MethodIndexStringRef::Bytecode(string_idx) = string_ref {
                    let string_idx = *string_idx as u16;
                    layout
                        .string_pool_indices
                        .insert(string_idx, next_pool_index);
                    global_string_pool_indices
                        .entry(string_idx)
                        .or_insert(next_pool_index);
                }
                next_pool_index = next_pool_index.saturating_add(1);
            }
            for string_idx in all_bytecode_string_indices {
                let string_idx = string_idx as u16;
                if layout.string_pool_indices.contains_key(&string_idx) {
                    continue;
                }
                if let Some(&pool_index) = global_string_pool_indices.get(&string_idx) {
                    layout.string_pool_indices.insert(string_idx, pool_index);
                }
            }

            layout.method_indices = method_index_method_indices(class);
            for &method_idx in &layout.method_indices {
                let pool_index = next_pool_index;
                let method = &class.methods[method_idx];
                if method.name.contains("#~@0=") {
                    layout.ctor_pool_index = pool_index;
                } else if method.name.contains("*#") {
                    layout.definefunc_pool_indices.push(pool_index);
                }
                if let Some(name_id) = strings.id_of(&method.name) {
                    layout
                        .definemethod_pool_indices_by_name
                        .insert(name_id as u16, pool_index);
                }
                next_pool_index = next_pool_index.saturating_add(1);
            }

            for &literal_idx in &class.literal_indices {
                layout
                    .literal_pool_indices
                    .insert(literal_idx as u16, next_pool_index);
                next_pool_index = next_pool_index.saturating_add(1);
            }

            layout
        })
        .collect()
}

fn build_literal_emit_order(classes: &[&ClassRecord], literal_count: usize) -> Vec<usize> {
    let mut seen = vec![false; literal_count];
    let mut order = Vec::with_capacity(literal_count);
    for class in classes {
        for &literal_idx in class
            .module_literal_indices
            .iter()
            .chain(class.literal_indices.iter())
        {
            if literal_idx < literal_count && !seen[literal_idx] {
                seen[literal_idx] = true;
                order.push(literal_idx);
            }
        }
    }
    for (idx, was_seen) in seen.into_iter().enumerate() {
        if !was_seen {
            order.push(idx);
        }
    }
    order
}

fn build_literal_header_order(
    classes: &[&ClassRecord],
    literal_arrays: &[LiteralArray],
) -> Vec<usize> {
    let mut rank = vec![u16::MAX; literal_arrays.len()];

    for class in classes {
        let is_entry = class.name.contains("/entryability/");
        let is_page = class.name.contains("/ets/pages/");

        for (pos, &literal_idx) in class.module_literal_indices.iter().enumerate() {
            if literal_idx >= literal_arrays.len() {
                continue;
            }
            rank[literal_idx] = match (is_page, is_entry, pos) {
                // module_literal_indices are [scopeNames, moduleRecordIdx].
                (true, _, 0) => 7,
                (true, _, 1) => 2,
                (_, true, 0) => 9,
                (_, true, 1) => 5,
                _ => 80,
            };
        }

        for &literal_idx in &class.literal_indices {
            if literal_idx >= literal_arrays.len() {
                continue;
            }
            let array = &literal_arrays[literal_idx];
            rank[literal_idx] = if is_page && literal_array_has_method_affiliate_emit(array) {
                0
            } else if is_page && literal_array_is_page_route_metadata(array) {
                4
            } else if is_page && literal_array_is_this_object(array) {
                6
            } else if is_page && literal_array_is_page_index_stub(array) {
                1
            } else if is_entry && literal_array_has_method_affiliate_emit(array) {
                3
            } else if is_entry {
                8
            } else {
                80
            };
        }
    }

    let mut order: Vec<usize> = (0..literal_arrays.len()).collect();
    order.sort_by_key(|idx| (rank[*idx], *idx));
    order
}

fn literal_array_has_method_affiliate_emit(array: &LiteralArray) -> bool {
    array
        .entries
        .iter()
        .any(|entry| matches!(entry, LiteralEntry::TaggedMethodAffiliate(_)))
}

fn literal_array_is_page_route_metadata(array: &LiteralArray) -> bool {
    array.entries.iter().any(|entry| {
        matches!(
            entry,
            LiteralEntry::TaggedString(value) | LiteralEntry::RawString(value) if value == "pageFullPath"
        )
    })
}

fn literal_array_is_this_object(array: &LiteralArray) -> bool {
    matches!(
        array.entries.as_slice(),
        [
            LiteralEntry::TaggedInteger(1),
            LiteralEntry::TaggedString(name),
            ..
        ] if name == "this"
    )
}

fn literal_array_is_page_index_stub(array: &LiteralArray) -> bool {
    matches!(
        array.entries.as_slice(),
        [
            LiteralEntry::TaggedInteger(1),
            LiteralEntry::TaggedString(name),
            ..
        ] if name == "Index"
    )
}

fn method_index_string_refs(
    class: &ClassRecord,
    strings: &StringTable,
) -> Vec<MethodIndexStringRef> {
    let mut indices = Vec::new();
    for method in &class.methods {
        collect_bytecode_string_indices(&method.bytecode, &mut indices);
    }
    indices.retain(|idx| strings.get(*idx).is_some());
    indices.sort_unstable();
    indices.dedup();
    indices.sort_by(|a, b| {
        strings
            .get(*a)
            .unwrap_or("")
            .cmp(strings.get(*b).unwrap_or(""))
            .then_with(|| a.cmp(b))
    });
    if class.name.contains("/ets/pages/") {
        indices.retain(|idx| {
            !matches!(
                strings.get(*idx).unwrap_or(""),
                "__localStorage"
                    | "elmtId"
                    | "extraInfo"
                    | "params"
                    | "paramsLambda"
                    | "parent"
                    | "prototype"
            )
        });
    }
    let mut refs: Vec<MethodIndexStringRef> = indices
        .into_iter()
        .map(MethodIndexStringRef::Bytecode)
        .collect();
    if class.name.contains("/ets/pages/") {
        for required in ["getEntryName", "initialRender", "rerender"] {
            if !refs
                .iter()
                .any(|string_ref| string_ref.value(strings) == required)
            {
                refs.push(MethodIndexStringRef::Supplemental(required.to_string()));
            }
        }
    }
    refs.sort_by(|a, b| a.value(strings).cmp(b.value(strings)));
    refs
}

fn collect_bytecode_string_indices(bytecode: &[u8], indices: &mut Vec<u32>) {
    let mut pos = 0usize;
    while pos < bytecode.len() {
        let Some(opcode) = bytecode.get(pos).copied() else {
            break;
        };
        match opcode {
            0x3e if pos + 2 < bytecode.len() => {
                indices.push(u16::from_le_bytes([bytecode[pos + 1], bytecode[pos + 2]]) as u32);
                pos += 3;
                continue;
            }
            0x3f | 0x40 | 0x42 if pos + 3 < bytecode.len() => {
                indices.push(u16::from_le_bytes([bytecode[pos + 2], bytecode[pos + 3]]) as u32);
                pos += 4;
                continue;
            }
            0x43 | 0x7a if pos + 4 < bytecode.len() => {
                indices.push(u16::from_le_bytes([bytecode[pos + 2], bytecode[pos + 3]]) as u32);
                pos += 5;
                continue;
            }
            0xfe if bytecode.get(pos + 1) == Some(&0x09) && pos + 3 < bytecode.len() => {
                indices.push(u16::from_le_bytes([bytecode[pos + 2], bytecode[pos + 3]]) as u32);
                pos += 4;
                continue;
            }
            _ => {}
        }

        let next = bytecode.get(pos + 1).copied();
        pos += instruction_len(opcode, next).max(1);
    }
}

fn method_index_method_indices(class: &ClassRecord) -> Vec<usize> {
    let mut indices: Vec<usize> = class
        .methods
        .iter()
        .enumerate()
        .filter_map(|(idx, method)| method_is_method_pool_literal(&method.name).then_some(idx))
        .collect();
    indices.sort_by(|&a, &b| class.methods[a].name.cmp(&class.methods[b].name));
    indices
}

fn method_is_method_pool_literal(name: &str) -> bool {
    name.contains("#~@0=")
        || name.contains("#~@0<#")
        || name.contains("*#")
        || name.contains("#initialRender")
        || name.contains("#message")
        || name.contains("#rerender")
}

fn instruction_len(opcode: u8, next: Option<u8>) -> usize {
    match opcode {
        0x00..=0x03 | 0x64 | 0x65 => 1,
        0x1c | 0x29 | 0x3c | 0x3d | 0x44 | 0x4d | 0x4f | 0x51 | 0x60 | 0x61 | 0x7c | 0x7e => 2,
        0x2a | 0x2d | 0x3e | 0x45 | 0x50 | 0x9b | 0x4e | 0xb9 => 3,
        0x0a..=0x1a | 0x25 | 0x27 | 0x28 => 3,
        0x1f..=0x22 => 2,
        0x07 | 0x08 | 0x2b | 0x2e | 0x31 | 0x32 | 0x3f | 0x40 | 0x42 | 0x73 | 0xb6 => 4,
        0x2c | 0x33 | 0x34 | 0x43 | 0x62 | 0x7a | 0x8f | 0x98 | 0x9a | 0x9c | 0xbc => 5,
        0x2f => 5,
        0x30 => 6,
        0x35 | 0x63 => 9,
        0xcf => 2,
        0xfe if next == Some(0x00) => 2,
        0xfe if next == Some(0x07) => 3,
        0xfe if next == Some(0x09) => 4,
        0xfb if matches!(next, Some(0x13 | 0x14)) => 3,
        _ => 1,
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

/// Encode a SLEB128 value as a byte vector.
fn encode_sleb128_bytes(mut value: i32) -> Vec<u8> {
    let mut out = Vec::new();
    let mut more = true;
    while more {
        let byte = (value & 0x7F) as u8;
        value >>= 7;
        more = !(((value == 0) && (byte & 0x40 == 0)) || ((value == -1) && (byte & 0x40 != 0)));
        out.push(if more { byte | 0x80 } else { byte });
    }
    out
}

#[derive(Debug, Clone)]
struct MidLocalEvent {
    pc: u32,
    kind: MidLocalKind,
    reg: i32,
    name_off: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MidLocalKind {
    Start,
    End,
}

fn infer_func_main_class_local_events(
    method: &MethodRecord,
    class: &ClassRecord,
    string_items: &StringItemWriter,
) -> Vec<MidLocalEvent> {
    if method.name != "func_main_0" {
        return Vec::new();
    }
    let Some(short_name) = class_short_name(&class.name) else {
        return Vec::new();
    };
    let Some(name_off) = string_items.id_of(&short_name) else {
        return Vec::new();
    };

    let has_params_shape = class
        .source_code_strings
        .iter()
        .any(|name| name.starts_with('#') && name.ends_with("_Params"));
    let reg = if has_params_shape { 6 } else { 5 };
    let base_pc = debug_scope_start_pc(method);
    let start_pc = if has_params_shape {
        infer_first_large_line_change_pc(method.line_ops.as_slice(), base_pc)
    } else {
        infer_second_line_change_pc(method.line_ops.as_slice(), base_pc)
    };
    let Some(start_pc) = start_pc else {
        return Vec::new();
    };
    let Some(end_pc) = infer_class_local_end_pc(method.bytecode.as_slice(), has_params_shape)
    else {
        return Vec::new();
    };
    if end_pc <= start_pc {
        return Vec::new();
    }

    vec![
        MidLocalEvent {
            pc: start_pc,
            kind: MidLocalKind::Start,
            reg,
            name_off,
        },
        MidLocalEvent {
            pc: end_pc,
            kind: MidLocalKind::End,
            reg,
            name_off,
        },
    ]
}

fn class_short_name(class_name: &str) -> Option<String> {
    let trimmed = class_name.strip_prefix('L').unwrap_or(class_name);
    let trimmed = trimmed.strip_suffix(';').unwrap_or(trimmed);
    trimmed
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .map(str::to_string)
}

fn infer_second_line_change_pc(method_ops: &[u8], base_pc: u32) -> Option<u32> {
    infer_nth_line_change_pc(method_ops, base_pc, 2, 1)
}

fn infer_first_large_line_change_pc(method_ops: &[u8], base_pc: u32) -> Option<u32> {
    infer_nth_line_change_pc(method_ops, base_pc, 1, 2)
}

fn infer_nth_line_change_pc(
    method_ops: &[u8],
    base_pc: u32,
    target_change: u8,
    min_abs_delta: i32,
) -> Option<u32> {
    let (ops, cp) = split_line_ops(method_ops)?;
    let mut cp_pos = 0usize;
    let mut op_pos = 0usize;
    let mut pc = base_pc;
    let mut line_changes = 0u8;
    while op_pos < ops.len() {
        let op = ops[op_pos];
        op_pos += 1;
        match op {
            0x01 => {
                let (delta, next) = read_uleb128_from(cp, cp_pos)?;
                cp_pos = next;
                pc = pc.saturating_add(delta as u32);
            }
            0x02 => {
                let (line_delta, next) = read_sleb128_from(cp, cp_pos)?;
                cp_pos = next;
                if line_delta.unsigned_abs() >= min_abs_delta as u32 {
                    line_changes += 1;
                    if line_changes == target_change {
                        return Some(pc);
                    }
                }
            }
            0x04 => {
                let (_, next_op) = read_sleb128_from(ops, op_pos)?;
                op_pos = next_op;
                for _ in 0..3 {
                    let (_, next_cp) = read_uleb128_from(cp, cp_pos)?;
                    cp_pos = next_cp;
                }
            }
            0x05 | 0x06 => {
                let (_, next_op) = read_sleb128_from(ops, op_pos)?;
                op_pos = next_op;
            }
            0x09 | 0x0a | 0x0b => {
                let (_, next) = read_uleb128_from(cp, cp_pos)?;
                cp_pos = next;
            }
            op if op >= 0x0c => {
                let adjusted = (op as u32).saturating_sub(0x0c);
                let pc_delta = adjusted / LNP_LINE_RANGE;
                let line_delta = (adjusted % LNP_LINE_RANGE) as i32 + LNP_LINE_BASE;
                pc = pc.saturating_add(pc_delta);
                if line_delta.unsigned_abs() >= min_abs_delta as u32 {
                    line_changes += 1;
                    if line_changes == target_change {
                        return Some(pc);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn infer_class_local_end_pc(bytecode: &[u8], has_params_shape: bool) -> Option<u32> {
    if has_params_shape {
        for (idx, window) in bytecode.windows(5).enumerate() {
            if window[0] == 0x61 && window[2] == 0x60 && window[4] == 0x3d {
                return Some(idx as u32);
            }
        }
    } else {
        if let Some(idx) = bytecode.iter().position(|&b| b == 0x7c) {
            return Some(idx as u32);
        }
    }
    None
}

fn append_line_ops_with_mid_locals(
    method_ops: &[u8],
    base_pc: u32,
    mid_locals: &[MidLocalEvent],
    any_type_off: u32,
    lnp_data: &mut Vec<u8>,
    cp: &mut Vec<u8>,
) {
    let Some((ops, method_cp)) = split_line_ops(method_ops) else {
        lnp_data.extend_from_slice(method_ops);
        return;
    };
    let mut events = mid_locals.to_vec();
    events.sort_by_key(|event| {
        (
            event.pc,
            match event.kind {
                MidLocalKind::Start => 0u8,
                MidLocalKind::End => 1u8,
            },
        )
    });
    let mut event_pos = 0usize;
    let mut pc = base_pc;
    let mut op_pos = 0usize;
    let mut cp_pos = 0usize;

    while op_pos < ops.len() {
        emit_mid_local_events_at_pc(pc, &events, &mut event_pos, any_type_off, lnp_data, cp);

        let op = ops[op_pos];
        op_pos += 1;
        match op {
            0x01 => {
                let (delta, next) = read_uleb128_from(method_cp, cp_pos).unwrap_or((0, cp_pos));
                cp_pos = next;
                append_pc_advance_with_events(
                    delta as u32,
                    &mut pc,
                    &events,
                    &mut event_pos,
                    any_type_off,
                    lnp_data,
                    cp,
                );
            }
            0x02 => {
                let (delta, next) = read_sleb128_from(method_cp, cp_pos).unwrap_or((0, cp_pos));
                cp_pos = next;
                lnp_data.push(0x02);
                cp.extend_from_slice(&encode_sleb128_bytes(delta));
            }
            0x04 => {
                let reg_start = op_pos;
                let (_, next_op) = read_sleb128_from(ops, op_pos).unwrap_or((0, op_pos));
                op_pos = next_op;
                lnp_data.push(0x04);
                lnp_data.extend_from_slice(&ops[reg_start..next_op]);
                for _ in 0..3 {
                    let (value, next_cp) =
                        read_uleb128_from(method_cp, cp_pos).unwrap_or((0, cp_pos));
                    cp_pos = next_cp;
                    cp.extend_from_slice(&uleb128(value));
                }
            }
            0x05 | 0x06 => {
                let reg_start = op_pos;
                let (_, next_op) = read_sleb128_from(ops, op_pos).unwrap_or((0, op_pos));
                op_pos = next_op;
                lnp_data.push(op);
                lnp_data.extend_from_slice(&ops[reg_start..next_op]);
            }
            0x09 | 0x0a | 0x0b => {
                let (value, next) = read_uleb128_from(method_cp, cp_pos).unwrap_or((0, cp_pos));
                cp_pos = next;
                lnp_data.push(op);
                cp.extend_from_slice(&uleb128(value));
            }
            op if op >= 0x0c => {
                let adjusted = (op as u32).saturating_sub(0x0c);
                let pc_delta = adjusted / LNP_LINE_RANGE;
                let line_delta = (adjusted % LNP_LINE_RANGE) as i32 + LNP_LINE_BASE;
                if has_mid_local_event_between(pc, pc.saturating_add(pc_delta), &events, event_pos)
                {
                    append_pc_advance_with_events(
                        pc_delta,
                        &mut pc,
                        &events,
                        &mut event_pos,
                        any_type_off,
                        lnp_data,
                        cp,
                    );
                    emit_special_opcode(0, line_delta, lnp_data, cp);
                } else {
                    lnp_data.push(op);
                    pc = pc.saturating_add(pc_delta);
                }
            }
            _ => lnp_data.push(op),
        }
    }
    emit_mid_local_events_at_pc(pc, &events, &mut event_pos, any_type_off, lnp_data, cp);
}

fn split_line_ops(method_ops: &[u8]) -> Option<(&[u8], &[u8])> {
    let end_pos = method_ops.iter().position(|&b| b == 0x00)?;
    let after = end_pos + 1;
    if after + 4 > method_ops.len() {
        return Some((&method_ops[..end_pos], &[]));
    }
    let cp_len = u32::from_le_bytes(method_ops[after..after + 4].try_into().ok()?) as usize;
    let cp_start = after + 4;
    let cp_end = cp_start.checked_add(cp_len)?;
    if cp_end > method_ops.len() {
        return Some((&method_ops[..end_pos], &[]));
    }
    Some((&method_ops[..end_pos], &method_ops[cp_start..cp_end]))
}

fn append_pc_advance_with_events(
    delta: u32,
    pc: &mut u32,
    events: &[MidLocalEvent],
    event_pos: &mut usize,
    any_type_off: u32,
    lnp_data: &mut Vec<u8>,
    cp: &mut Vec<u8>,
) {
    let target = pc.saturating_add(delta);
    while *event_pos < events.len() && events[*event_pos].pc <= target {
        let event_pc = events[*event_pos].pc;
        if event_pc > *pc {
            emit_adv_pc(event_pc - *pc, lnp_data, cp);
            *pc = event_pc;
        }
        emit_mid_local_events_at_pc(event_pc, events, event_pos, any_type_off, lnp_data, cp);
    }
    if target > *pc {
        emit_adv_pc(target - *pc, lnp_data, cp);
        *pc = target;
    }
}

fn emit_adv_pc(delta: u32, lnp_data: &mut Vec<u8>, cp: &mut Vec<u8>) {
    if delta == 0 {
        return;
    }
    lnp_data.push(0x01);
    cp.extend_from_slice(&uleb128(delta as u64));
}

fn emit_special_opcode(pc_delta: u32, line_delta: i32, lnp_data: &mut Vec<u8>, cp: &mut Vec<u8>) {
    let adjusted_line = line_delta - LNP_LINE_BASE;
    if pc_delta < LNP_LINE_RANGE && adjusted_line >= 0 && (adjusted_line as u32) < LNP_LINE_RANGE {
        let op = pc_delta * LNP_LINE_RANGE + adjusted_line as u32 + 0x0c;
        if op <= u8::MAX as u32 {
            lnp_data.push(op as u8);
            return;
        }
    }
    if pc_delta > 0 {
        emit_adv_pc(pc_delta, lnp_data, cp);
    }
    lnp_data.push(0x02);
    cp.extend_from_slice(&encode_sleb128_bytes(line_delta));
    lnp_data.push(0x0c);
}

fn has_mid_local_event_between(
    start_pc: u32,
    end_pc: u32,
    events: &[MidLocalEvent],
    event_pos: usize,
) -> bool {
    events
        .get(event_pos)
        .is_some_and(|event| event.pc > start_pc && event.pc <= end_pc)
}

fn emit_mid_local_events_at_pc(
    pc: u32,
    events: &[MidLocalEvent],
    event_pos: &mut usize,
    any_type_off: u32,
    lnp_data: &mut Vec<u8>,
    cp: &mut Vec<u8>,
) {
    while *event_pos < events.len() && events[*event_pos].pc == pc {
        let event = &events[*event_pos];
        match event.kind {
            MidLocalKind::Start => {
                lnp_data.push(0x04);
                lnp_data.extend_from_slice(&encode_sleb128_bytes(event.reg));
                cp.extend_from_slice(&uleb128(event.name_off as u64));
                cp.extend_from_slice(&uleb128(any_type_off as u64));
                cp.extend_from_slice(&uleb128(any_type_off as u64));
            }
            MidLocalKind::End => {
                lnp_data.push(0x05);
                lnp_data.extend_from_slice(&encode_sleb128_bytes(event.reg));
            }
        }
        *event_pos += 1;
    }
}

fn read_uleb128_from(bytes: &[u8], mut pos: usize) -> Option<(u64, usize)> {
    let mut result = 0u64;
    let mut shift = 0u32;
    loop {
        let byte = *bytes.get(pos)?;
        pos += 1;
        result |= ((byte & 0x7f) as u64) << shift;
        if byte & 0x80 == 0 {
            return Some((result, pos));
        }
        shift += 7;
    }
}

fn read_sleb128_from(bytes: &[u8], mut pos: usize) -> Option<(i32, usize)> {
    let mut result = 0i32;
    let mut shift = 0u32;
    let byte = loop {
        let byte = *bytes.get(pos)?;
        pos += 1;
        result |= ((byte & 0x7f) as i32) << shift;
        shift += 7;
        if byte & 0x80 == 0 {
            break byte;
        }
    };
    if shift < 32 && byte & 0x40 != 0 {
        result |= !0 << shift;
    }
    Some((result, pos))
}

/// Encode a Panda `DebugInfoItem` with a pre-built constant_pool.
///
/// libpandafile `DebugInfoItem::Write` format:
/// ```text
/// line_num:           ULEB128        (1-based source line, or 0xFFFFFFFF sentinel)
/// num_parameters:     ULEB128        (number of formal parameters)
/// [param_off: ULEB128] * num_params  (file offset of each parameter name StringItem)
/// constant_pool_size: ULEB128        (byte count of the constant pool)
/// constant_pool:      raw bytes      (operands for LNP opcodes in the LNP data_ stream)
/// lnp_index:          ULEB128        (index into the LNP index table)
/// ```
///
/// The `constant_pool` contains ULEB128-encoded operands for opcodes in the LNP
/// data_ stream that need them (SET_FILE src_off, START_LOCAL_EXTENDED operands,
/// ADVANCE_PC delta, ADVANCE_LINE delta, etc.).
fn encode_debug_info_with_cp(
    _line_start: u32,
    param_name_offsets: &[u32],
    constant_pool: &[u8],
    lnp_index: u32,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + constant_pool.len());
    // es2abc leaves the source line in the LineNumberProgram and writes the
    // DebugInfo line_num field as NO_LINE_NUMBER.
    out.extend_from_slice(&uleb128(u32::MAX as u64));
    out.extend_from_slice(&uleb128(param_name_offsets.len() as u64));
    for &off in param_name_offsets {
        out.extend_from_slice(&uleb128(off as u64));
    }
    // constant_pool_size = byte count (not item count).
    out.extend_from_slice(&uleb128(constant_pool.len() as u64));
    out.extend_from_slice(constant_pool);
    out.extend_from_slice(&uleb128(lnp_index as u64));
    out
}

/// Encode a Panda `DebugInfoItem` with a minimal constant_pool containing only
/// the SET_FILE operand. Used as a fallback when no LNP tracking is available.
#[allow(dead_code)]
fn encode_debug_info(
    line_start: u32,
    param_name_offsets: &[u32],
    source_file_offset: u32,
    lnp_index: u32,
) -> Vec<u8> {
    // Minimal constant_pool: just the SET_FILE operand (ULEB128 source file offset).
    let cp = uleb128(source_file_offset as u64);
    encode_debug_info_with_cp(line_start, param_name_offsets, &cp, lnp_index)
}

fn debug_scope_start_pc(method: &MethodRecord) -> u32 {
    let mut pc = argument_copy_prologue_size(method);

    // `prepend_module_argument_copies` preserves a leading NewLexEnvWithName
    // before the copied parameters.  Upstream local-variable ranges start after
    // that hidden scope setup.
    if method.name == "func_main_0" && method.bytecode.first() == Some(&0xb6) {
        pc += 4;
    }

    // Function bodies that capture `this` in an arrow start with the function
    // `this` lexenv, then copied args, then a hidden `lda this; stlexvar`.
    if method.name != "func_main_0"
        && method.bytecode.first() == Some(&0xb6)
        && method.bytecode.get(1) == Some(&1)
    {
        pc += 4; // NewLexEnvWithName
        let this_src = method.num_vregs + MANDATORY_PARAM_THIS_REG as u16;
        if this_src <= u8::MAX as u16 {
            pc += 4; // lda v(this_src) + stlexvar 0, 0
        }
    }

    pc
}

fn argument_copy_prologue_size(method: &MethodRecord) -> u32 {
    let has_this_lexenv = method.name != "func_main_0"
        && method.bytecode.first() == Some(&0xb6)
        && method.bytecode.get(1) == Some(&1);
    let mut size = 0u32;
    for dst in 0..method.num_args {
        if has_this_lexenv && dst == MANDATORY_PARAM_THIS_REG as u16 {
            continue;
        }
        let src = method.num_vregs + dst;
        size += if dst <= 0x0f && src <= 0x0f {
            2
        } else if src <= u8::MAX as u16 {
            3
        } else {
            5
        };
    }
    size
}

impl Default for AbcFile {
    fn default() -> Self {
        Self::new()
    }
}
