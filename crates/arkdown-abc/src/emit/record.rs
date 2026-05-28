/// Encode `value` as an unsigned LEB-128 byte sequence.
pub(crate) fn uleb128(mut value: u64) -> Vec<u8> {
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

const METHOD_ITEM_SIZE: usize = 18;
pub const FIELD_OWNER_CLASS_IDX: u16 = u16::MAX;

/// Encode a Panda `StringItem` as an inline ULEB128-prefixed null-terminated string.
///
/// Panda wire format (from `libpandafile/file_items.h`):
/// ```text
/// ULEB128((utf16_length << 1) | is_ascii_flag)
/// [raw bytes: utf16_length bytes if ASCII, utf16_length*2 bytes if UTF-16LE]
/// 0x00  (null terminator, or 0x00 0x00 for UTF-16)
/// ```
///
/// This is used for class names (embedded inline at the start of a `ClassItem`)
/// and for standalone `StringItem`s referenced by file offset in `MethodItem.name_off`.
pub fn inline_string(s: &str) -> Vec<u8> {
    let bytes = s.as_bytes();
    let is_ascii = bytes.iter().all(|&b| b < 0x80);
    let mut out = Vec::with_capacity(bytes.len() + 3);
    if is_ascii {
        let prefix = ((bytes.len() as u64) << 1) | 1u64;
        out.extend_from_slice(&uleb128(prefix));
        out.extend_from_slice(bytes);
        out.push(0x00);
    } else {
        let utf16: Vec<u16> = s.encode_utf16().collect();
        let prefix = (utf16.len() as u64) << 1; // is_ascii = 0
        out.extend_from_slice(&uleb128(prefix));
        for unit in &utf16 {
            out.extend_from_slice(&unit.to_le_bytes());
        }
        // UTF-16 null terminator (2 bytes)
        out.push(0x00);
        out.push(0x00);
    }
    out
}

/// Encode a Panda `CodeItem`.
///
/// Binary layout:
/// ```text
/// ULEB128(num_vregs)   – number of virtual registers required
/// ULEB128(num_args)    – number of formal parameters (3 for module func_main_0)
/// ULEB128(num_insns)   – byte length of the bytecode that follows
/// ULEB128(tries_size)  – number of try-block records that follow bytecode
/// [bytecode: u8 * num_insns]
/// ```
///
/// No try-catch block footer is emitted (empty try-block list).  ArkRuntime
/// accepts this for functions without exception handlers.
pub fn encode_code_item(num_vregs: u16, num_args: u16, bytecode: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(7 + bytecode.len());
    out.extend_from_slice(&uleb128(num_vregs as u64));
    out.extend_from_slice(&uleb128(num_args as u64));
    out.extend_from_slice(&uleb128(bytecode.len() as u64));
    out.extend_from_slice(&uleb128(0));
    out.extend_from_slice(bytecode);
    out
}

/// A compiled method (function) within a class record.
///
/// In Panda bytecode, every function — including a module's top-level
/// `func_main_0` — is a method belonging to a class record.
#[derive(Debug)]
pub struct MethodRecord {
    /// Method name string (e.g. `"func_main_0"` for the module entry point).
    pub name: String,
    /// Encoded bytecode for the method body.
    pub bytecode: Vec<u8>,
    /// Number of virtual registers required by this method.
    pub num_vregs: u16,
    /// Number of formal parameters.
    ///
    /// ArkRuntime always passes **3** arguments when invoking `func_main_0`:
    /// `(JSThread*, JSTaggedValue this, JSTaggedValue[] args)` — set this to 3.
    pub num_args: u16,
    /// Optional method debug metadata. hvigor/es2abc emits this in non-stripped
    /// output so ArkRuntime can link bytecode spans back to source files.
    pub debug_info: Option<DebugInfoRecord>,
}

/// Minimal method debug info linked through a Panda LineNumberProgram.
#[derive(Debug, Clone)]
pub struct DebugInfoRecord {
    /// Initial 1-based source line for the method.
    pub line_start: u32,
    /// Source file path stored in the class SOURCE_FILE tag and LNP SET_FILE.
    pub source_file: String,
}

/// A field (instance or static) in a Panda `ClassItem`.
///
/// Used to emit the 6 module metadata fields that es2abc always writes:
/// `pkgName@`, `isCommonjs`, `hasTopLevelAwait`, `isSharedModule`,
/// `scopeNames`, `moduleRecordIdx`.
#[derive(Debug)]
pub struct FieldRecord {
    /// Field name string (stored as a StringItem in the file).
    pub name: String,
    /// Owning class index in the file IndexHeader class table.
    ///
    /// Use [`FIELD_OWNER_CLASS_IDX`] to let serialization fill in the enclosing
    /// class index after final class-table ordering is known.
    pub class_idx: u16,
    /// Field type index in the owning class. hvigor uses `0` for `u8` metadata
    /// fields and `1` for `u32` metadata fields.
    pub type_idx: u16,
    /// Access flags — 1 = ACC_PUBLIC.
    pub access_flags: u32,
    /// Optional constant value emitted in the field's tagged section.
    pub value: Option<FieldValue>,
}

/// Constant field values used by hvigor module metadata records.
#[derive(Debug, Clone, Copy)]
pub enum FieldValue {
    /// `u8` zero (`FieldTag::VALUE = 0x01`, one-byte payload).
    U8(u8),
    /// `u32` file offset (`FieldTag::VALUE = 0x02`, four-byte payload).
    U32(u32),
    /// `u32` file offset of a literal array, resolved during file layout.
    LiteralArray(usize),
}

/// A compiled module, represented as a Panda `ClassItem`.
///
/// One `ClassRecord` is produced per bundled JavaScript chunk file.
#[derive(Debug)]
pub struct ClassRecord {
    /// Class name (the OHM URL or record identifier for this chunk).
    pub name: String,
    /// All methods belonging to this record.
    pub methods: Vec<MethodRecord>,
    /// Access flags for this class (ULEB128-encoded in the binary).
    ///
    /// Regular JS module classes use `1` (ACC_PUBLIC).
    /// Annotation stub classes (API 13+) use `0x2001` (ACC_PUBLIC | ACC_ANNOTATION).
    pub access_flags: u32,
    /// Class and method source-language byte.
    ///
    /// hvigor's API 17 output uses `0x04` for user ArkTS records and `0x00`
    /// for fixed SDK/annotation ECMAScript records.
    pub source_lang: u8,
    /// Optional source file path written as ClassTag::SOURCE_FILE.
    pub source_file: Option<String>,
    /// Module specifiers referenced by `require("@ohos:xxx")` calls in this
    /// module's code.
    ///
    /// These strings are emitted through the compact `moduleRecordIdx` literal
    /// array so that ArkRuntime can locate each module import specifier by file
    /// offset.
    pub module_requests: Vec<String>,
    /// Module metadata fields — the 6 fixed fields that es2abc writes into every
    /// user class so that ArkRuntime knows how to load this module's dependencies.
    ///
    /// Annotation and SDK stub classes leave this empty (`vec![]`).
    pub fields: Vec<FieldRecord>,
}

impl ClassRecord {
    /// Compute the serialized byte size of this class item.
    ///
    /// This can be calculated without knowing the actual method-name or
    /// code-item offsets because those are all fixed-width `u32` fields.
    pub fn class_item_byte_size(&self) -> usize {
        // Inline class name string
        let name_bytes = inline_string(&self.name).len();
        // super_class_off: u32
        let super_off = 4;
        // access_flags ULEB128(self.access_flags) — size depends on value
        let acc_flags = uleb128(self.access_flags as u64).len();
        // fields_count ULEB128(self.fields.len())
        let fields_count = uleb128(self.fields.len() as u64).len();
        // methods_count: ULEB128 (1 byte for counts < 128)
        let methods_count = uleb128(self.methods.len() as u64).len();
        // ClassTag::SOURCE_LANG (1) + lang byte (1) + ClassTag::NOTHING (1)
        let class_tags = 3;
        let field_bytes = self.field_items_byte_size();
        // Per-method bytes:
        // class_idx(2) + proto_idx(2) + name_off(4) + access_flags_uleb128(2)
        // + MethodTag::CODE(1) + code_off(4)
        // + MethodTag::SOURCE_LANG(1) + lang(1) + MethodTag::NOTHING(1)
        // = 18 bytes per method
        name_bytes
            + super_off
            + acc_flags
            + fields_count
            + methods_count
            + class_tags
            + self.source_file.as_ref().map_or(0, |_| 1 + 4)
            + field_bytes
            + self
                .methods
                .iter()
                .map(MethodRecord::method_item_byte_size)
                .sum::<usize>()
    }

    /// Return absolute offsets for this class's inline MethodItem records.
    pub fn method_item_offsets(&self, class_item_offset: u32) -> Vec<u32> {
        let first_method = class_item_offset + self.method_items_start_offset() as u32;
        let mut offsets = Vec::with_capacity(self.methods.len());
        let mut cur = first_method;
        for method in &self.methods {
            offsets.push(cur);
            cur += method.method_item_byte_size() as u32;
        }
        offsets
    }

    fn method_items_start_offset(&self) -> usize {
        inline_string(&self.name).len()
            + 4
            + uleb128(self.access_flags as u64).len()
            + uleb128(self.fields.len() as u64).len()
            + uleb128(self.methods.len() as u64).len()
            + 3
            + self.source_file.as_ref().map_or(0, |_| 1 + 4)
            + self.field_items_byte_size()
    }

    fn field_items_byte_size(&self) -> usize {
        self.fields
            .iter()
            .map(|field| {
                2 + 2
                    + 4
                    + uleb128(field.access_flags as u64).len()
                    + field.value.map_or(0, FieldValue::encoded_len)
                    + 1
            })
            .sum()
    }

    /// Serialize this record into the correct Panda `ClassItem` binary format.
    ///
    /// ## Binary layout (from `libpandafile/file_items.h` and es2abc output)
    ///
    /// ```text
    /// ── ClassItem ──────────────────────────────────────────────────────────
    /// [class_name: inline StringItem — ULEB128((utf16_len<<1)|ascii) + bytes + \0]
    /// [super_class_off: u32 LE]             0 = no superclass
    /// [access_flags: ULEB128]               1 = ACC_PUBLIC
    /// [fields_count: ULEB128]               0 (JS classes have no static fields)
    /// [methods_count: ULEB128]
    /// [ClassTag::SOURCE_LANG = 0x02] [lang: u8 = 0x00 (ECMAScript)]
    /// [ClassTag::NOTHING = 0x00]
    ///
    /// ── Per MethodItem (inline, immediately after ClassItem tags) ──────────
    /// [class_idx: u16 LE]    index of this class in the sorted class list
    /// [proto_idx: u16 LE]    0 (prototype index, unused for JS functions)
    /// [name_off: u32 LE]     file offset of the method's StringItem
    /// [access_flags: ULEB128]  0x108 = NC_FUNCTION (kind=1) | STATIC (flags=8)
    /// [MethodTag::CODE = 0x01] [code_off: u32 LE]   file offset of CodeItem
    /// [MethodTag::SOURCE_LANG = 0x02] [lang: u8 = 0x00]
    /// [MethodTag::NOTHING = 0x00]
    /// ```
    ///
    /// ## Parameters
    ///
    /// - `class_idx`: Position of this class in the **sorted** (by name) class
    ///   list.  Used as the `class_idx` field in all `MethodItem`s within this
    ///   class.
    /// - `method_name_offsets`: File offsets of each method's `StringItem`,
    ///   parallel to `self.methods`.
    /// - `code_offsets`: File offsets of each method's `CodeItem`, parallel to
    ///   `self.methods`.
    pub fn serialize_class_item(
        &self,
        class_idx: u16,
        field_name_offsets: &[u32],
        source_file_offset: Option<u32>,
        method_name_offsets: &[u32],
        code_offsets: &[u32],
        debug_info_offsets: &[Option<u32>],
        literal_array_offsets: &[u32],
    ) -> Vec<u8> {
        debug_assert_eq!(field_name_offsets.len(), self.fields.len());
        debug_assert_eq!(method_name_offsets.len(), self.methods.len());
        debug_assert_eq!(code_offsets.len(), self.methods.len());
        debug_assert_eq!(debug_info_offsets.len(), self.methods.len());

        let mut buf = Vec::with_capacity(self.class_item_byte_size());

        // ── Class header ──────────────────────────────────────────────────────

        // Inline class name StringItem
        buf.extend_from_slice(&inline_string(&self.name));

        // super_class_off: u32 LE = 0 (no superclass)
        buf.extend_from_slice(&0u32.to_le_bytes());

        // access_flags: ULEB128 (1 = ACC_PUBLIC for user classes; 0x2001 = ACC_PUBLIC|ACC_ANNOTATION for stubs)
        buf.extend_from_slice(&uleb128(self.access_flags as u64));

        // fields_count: ULEB128
        buf.extend_from_slice(&uleb128(self.fields.len() as u64));

        // methods_count: ULEB128
        buf.extend_from_slice(&uleb128(self.methods.len() as u64));

        // ── ClassTag tagged section ───────────────────────────────────────────

        // ClassTag::SOURCE_LANG = 0x02.
        buf.push(0x02);
        buf.push(self.source_lang);
        if let Some(source_file_offset) = source_file_offset {
            buf.push(0x07);
            buf.extend_from_slice(&source_file_offset.to_le_bytes());
        }
        // ClassTag::NOTHING = 0x00 (terminates tagged section)
        buf.push(0x00);

        // ── FieldItem entries ────────────────────────────────────────────────
        //
        // Binary layout per FieldItem (from Panda ABC spec / es2abc output):
        //   [class_idx: u16 LE]      index of the owning class
        //   [type_idx: u16 LE]       0 = generic/Any type
        //   [name_off: u32 LE]       file offset of the field name StringItem
        //   [access_flags: ULEB128]  1 = ACC_PUBLIC
        //   [FieldTag::NOTHING = 0x00]

        for (field, &name_off) in self.fields.iter().zip(field_name_offsets.iter()) {
            // class_idx/type slot: u16 LE
            let field_class_idx = if field.class_idx == FIELD_OWNER_CLASS_IDX {
                class_idx
            } else {
                field.class_idx
            };
            buf.extend_from_slice(&field_class_idx.to_le_bytes());
            // type_idx: u16 LE
            buf.extend_from_slice(&field.type_idx.to_le_bytes());
            // name_off: u32 LE (file offset to field's StringItem)
            buf.extend_from_slice(&name_off.to_le_bytes());
            // access_flags: ULEB128
            buf.extend_from_slice(&uleb128(field.access_flags as u64));
            if let Some(value) = field.value {
                value.encode_into(&mut buf, literal_array_offsets);
            }
            // FieldTag::NOTHING = 0x00
            buf.push(0x00);
        }

        // ── MethodItem entries ────────────────────────────────────────────────

        for (i, _method) in self.methods.iter().enumerate() {
            let name_off = method_name_offsets[i];
            let code_off = code_offsets[i];

            // class_idx: u16 LE
            buf.extend_from_slice(&class_idx.to_le_bytes());
            // proto_idx: u16 LE = 0xffff.
            //
            // hvigor/es2abc uses this sentinel for JS/ArkTS methods without an
            // explicit ProtoItem.  Using 0 makes libpandafile resolve proto
            // index 0, but API 17 output has no proto index table.
            buf.extend_from_slice(&0xFFFFu16.to_le_bytes());
            // name_off: u32 LE (file offset to method's StringItem)
            buf.extend_from_slice(&name_off.to_le_bytes());
            // access_flags: ULEB128 = 0x108
            //   header_idx = 0x00, function_kind = 0x01 (NC_FUNCTION), flags = 0x08 (STATIC)
            //   0x108 = 264, encoded as [0x88, 0x02] in ULEB128
            buf.extend_from_slice(&uleb128(0x108));
            // MethodTag::CODE = 0x01
            buf.push(0x01);
            // code_off: u32 LE (file offset to CodeItem)
            buf.extend_from_slice(&code_off.to_le_bytes());
            // MethodTag::SOURCE_LANG = 0x02.
            buf.push(0x02);
            buf.push(self.source_lang);
            if let Some(debug_info_off) = debug_info_offsets[i] {
                buf.push(0x05);
                buf.extend_from_slice(&debug_info_off.to_le_bytes());
            }
            // MethodTag::NOTHING = 0x00
            buf.push(0x00);
        }

        buf
    }
}

impl MethodRecord {
    fn method_item_byte_size(&self) -> usize {
        METHOD_ITEM_SIZE + self.debug_info.as_ref().map_or(0, |_| 1 + 4)
    }
}

impl FieldValue {
    fn encoded_len(self) -> usize {
        match self {
            FieldValue::U8(_) => 2,
            FieldValue::U32(_) | FieldValue::LiteralArray(_) => 5,
        }
    }

    fn encode_into(self, buf: &mut Vec<u8>, literal_array_offsets: &[u32]) {
        match self {
            FieldValue::U8(value) => {
                buf.push(0x01);
                buf.push(value);
            }
            FieldValue::U32(value) => {
                buf.push(0x02);
                buf.extend_from_slice(&value.to_le_bytes());
            }
            FieldValue::LiteralArray(index) => {
                let value = literal_array_offsets
                    .get(index)
                    .copied()
                    .expect("field literal array index must be resolved");
                buf.push(0x02);
                buf.extend_from_slice(&value.to_le_bytes());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inline_string_ascii() {
        // "L_GLOBAL;" = 9 chars, ASCII
        // ULEB128((9<<1)|1) = ULEB128(19) = ULEB128(0x13) = [0x13]
        let s = inline_string("L_GLOBAL;");
        assert_eq!(s[0], 0x13, "ULEB128 prefix for 9-char ASCII = 0x13");
        assert_eq!(&s[1..10], b"L_GLOBAL;");
        assert_eq!(s[10], 0x00, "null terminator");
        assert_eq!(s.len(), 11);
    }

    #[test]
    fn test_inline_string_func_main_0() {
        // "func_main_0" = 11 chars, ASCII
        // ULEB128((11<<1)|1) = ULEB128(23) = ULEB128(0x17) = [0x17]
        let s = inline_string("func_main_0");
        assert_eq!(s[0], 0x17, "ULEB128 prefix for 11-char ASCII");
        assert_eq!(&s[1..12], b"func_main_0");
        assert_eq!(s[12], 0x00);
        assert_eq!(s.len(), 13);
    }

    #[test]
    fn test_encode_code_item_empty() {
        // Empty bytecode: four ULEB128 headers (vregs/args/code_size/tries_size).
        let ci = encode_code_item(0, 0, &[]);
        assert_eq!(ci, vec![0x00, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn test_encode_code_item_with_bytecode() {
        // num_vregs=0, num_args=3, bytecode=[0x65] (ReturnUndefined)
        let ci = encode_code_item(0, 3, &[0x65]);
        assert_eq!(ci, vec![0x00, 0x03, 0x01, 0x00, 0x65]);
    }

    #[test]
    fn test_uleb128_0x108() {
        // 0x108 = 264: expected ULEB128 = [0x88, 0x02]
        // 264 & 0x7F = 8, set continuation → 0x88
        // 264 >> 7 = 2, no continuation → 0x02
        let encoded = uleb128(0x108);
        assert_eq!(
            encoded,
            vec![0x88, 0x02],
            "ULEB128(0x108) must encode to [0x88, 0x02]"
        );
    }

    #[test]
    fn test_class_item_byte_size() {
        let record = ClassRecord {
            name: "L_GLOBAL;".to_string(),
            methods: vec![MethodRecord {
                name: "func_main_0".to_string(),
                bytecode: vec![0x65],
                num_vregs: 0,
                num_args: 3,
                debug_info: None,
            }],
            access_flags: 1,
            source_lang: 0,
            source_file: None,
            module_requests: vec![],
            fields: vec![],
        };
        // inline_string("L_GLOBAL;") = 11 bytes
        // super_off = 4
        // access_flags ULEB128(1) = 1
        // fields_count ULEB128(0) = 1
        // methods_count ULEB128(1) = 1
        // class tags = 3
        // method item = 18
        // total = 11 + 4 + 1 + 1 + 1 + 3 + 18 = 39
        assert_eq!(record.class_item_byte_size(), 39);
    }

    #[test]
    fn test_serialize_class_item_structure() {
        let record = ClassRecord {
            name: "L_GLOBAL;".to_string(),
            methods: vec![MethodRecord {
                name: "func_main_0".to_string(),
                bytecode: vec![0x65],
                num_vregs: 0,
                num_args: 3,
                debug_info: None,
            }],
            access_flags: 1,
            source_lang: 0,
            source_file: None,
            module_requests: vec![],
            fields: vec![],
        };
        let bytes = record.serialize_class_item(4, &[], None, &[0x12a], &[0x1a1], &[None], &[]);
        assert_eq!(bytes.len(), record.class_item_byte_size());

        // Verify inline class name starts with ULEB128(0x13) = "L_GLOBAL;"
        assert_eq!(bytes[0], 0x13, "class name ULEB128 prefix");
        assert_eq!(&bytes[1..10], b"L_GLOBAL;", "class name bytes");
        assert_eq!(bytes[10], 0x00, "class name null");

        // super_class_off = 0
        assert_eq!(&bytes[11..15], &[0x00, 0x00, 0x00, 0x00]);

        // access_flags = ULEB128(1) = [0x01]
        assert_eq!(bytes[15], 0x01);
        // fields_count = ULEB128(0) = [0x00]
        assert_eq!(bytes[16], 0x00);
        // methods_count = ULEB128(1) = [0x01]
        assert_eq!(bytes[17], 0x01);

        // ClassTag::SOURCE_LANG (0x02), lang (0x00), ClassTag::NOTHING (0x00)
        assert_eq!(bytes[18], 0x02, "ClassTag::SOURCE_LANG");
        assert_eq!(bytes[19], 0x00, "lang ECMAScript");
        assert_eq!(bytes[20], 0x00, "ClassTag::NOTHING");

        // Method[0]: class_idx=4, proto_idx=0xffff
        assert_eq!(&bytes[21..23], &[0x04, 0x00], "class_idx=4");
        assert_eq!(&bytes[23..25], &[0xff, 0xff], "proto_idx=0xffff");
        // name_off = 0x12a LE
        assert_eq!(&bytes[25..29], &[0x2a, 0x01, 0x00, 0x00], "name_off=0x12a");
        // access_flags = ULEB128(0x108) = [0x88, 0x02]
        assert_eq!(&bytes[29..31], &[0x88, 0x02], "access_flags=0x108");
        // MethodTag::CODE = 0x01
        assert_eq!(bytes[31], 0x01, "MethodTag::CODE");
        // code_off = 0x1a1 LE
        assert_eq!(&bytes[32..36], &[0xa1, 0x01, 0x00, 0x00], "code_off=0x1a1");
        // MethodTag::SOURCE_LANG = 0x02, lang = 0x00
        assert_eq!(bytes[36], 0x02, "MethodTag::SOURCE_LANG");
        assert_eq!(bytes[37], 0x00, "lang");
        // MethodTag::NOTHING = 0x00
        assert_eq!(bytes[38], 0x00, "MethodTag::NOTHING");
    }
}
