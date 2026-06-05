use std::collections::BTreeSet;

use crate::emit::bytecode::{
    assign_inline_cache_slots, collect_bytecode_string_indices, patch_bytecode_index_operands,
};
use crate::emit::debug_info::{
    append_line_ops_with_mid_locals, debug_scope_start_pc, encode_debug_info_with_cp,
    encode_sleb128_bytes, infer_func_main_class_local_events, local_variable_debug_events,
    local_variable_debug_param_offsets,
};
use crate::emit::record::{
    encode_code_item, inline_string, uleb128, AnnotationRecord, AnnotationValue, ArrayValueRecord,
    ClassAnnotationOffsets, ClassRecord, DebugInfoRecord, FieldAnnotationOffsets, FieldType,
    FieldValue, MethodAnnotationOffsets, MethodKind, PandasmTypeRecord, ScalarValueData,
    ScalarValueRecord, VALUE_TYPE_ANNOTATION, VALUE_TYPE_ARRAY, VALUE_TYPE_ENUM, VALUE_TYPE_F32,
    VALUE_TYPE_F64, VALUE_TYPE_I16, VALUE_TYPE_I32, VALUE_TYPE_I64, VALUE_TYPE_I8,
    VALUE_TYPE_LITERALARRAY, VALUE_TYPE_METHOD, VALUE_TYPE_METHOD_HANDLE, VALUE_TYPE_RECORD,
    VALUE_TYPE_STRING, VALUE_TYPE_STRING_NULLPTR, VALUE_TYPE_U1, VALUE_TYPE_U16, VALUE_TYPE_U32,
    VALUE_TYPE_U64, VALUE_TYPE_U8,
};
use crate::emit::strings::StringTable;
use crate::names::full_method_name;
use rustc_hash::FxHashMap;

// ── LiteralArray types (Task 1) ───────────────────────────────────────────────

pub(crate) const LITERAL_TAG_TAGVALUE: u8 = 0x00;
pub(crate) const LITERAL_TAG_BOOL: u8 = 0x01;
pub(crate) const LITERAL_TAG_INTEGER: u8 = 0x02;
pub(crate) const LITERAL_TAG_FLOAT: u8 = 0x03;
pub(crate) const LITERAL_TAG_DOUBLE: u8 = 0x04;
pub(crate) const LITERAL_TAG_STRING: u8 = 0x05;
pub(crate) const LITERAL_TAG_METHOD: u8 = 0x06;
pub(crate) const LITERAL_TAG_GENERATOR_METHOD: u8 = 0x07;
pub(crate) const LITERAL_TAG_ACCESSOR: u8 = 0x08;
pub(crate) const LITERAL_TAG_METHOD_AFFILIATE: u8 = 0x09;
pub(crate) const LITERAL_TAG_ARRAY_U1: u8 = 0x0a;
pub(crate) const LITERAL_TAG_ARRAY_U8: u8 = 0x0b;
pub(crate) const LITERAL_TAG_ARRAY_I8: u8 = 0x0c;
pub(crate) const LITERAL_TAG_ARRAY_U16: u8 = 0x0d;
pub(crate) const LITERAL_TAG_ARRAY_I16: u8 = 0x0e;
pub(crate) const LITERAL_TAG_ARRAY_U32: u8 = 0x0f;
pub(crate) const LITERAL_TAG_ARRAY_I32: u8 = 0x10;
pub(crate) const LITERAL_TAG_ARRAY_U64: u8 = 0x11;
pub(crate) const LITERAL_TAG_ARRAY_I64: u8 = 0x12;
pub(crate) const LITERAL_TAG_ARRAY_F32: u8 = 0x13;
pub(crate) const LITERAL_TAG_ARRAY_F64: u8 = 0x14;
pub(crate) const LITERAL_TAG_ARRAY_STRING: u8 = 0x15;
pub(crate) const LITERAL_TAG_ASYNC_GENERATOR_METHOD: u8 = 0x16;
pub(crate) const LITERAL_TAG_LITERAL_BUFFER_INDEX: u8 = 0x17;
pub(crate) const LITERAL_TAG_LITERAL_ARRAY: u8 = 0x18;
pub(crate) const LITERAL_TAG_BUILTIN_TYPE_INDEX: u8 = 0x19;
pub(crate) const LITERAL_TAG_GETTER: u8 = 0x1a;
pub(crate) const LITERAL_TAG_SETTER: u8 = 0x1b;
pub(crate) const LITERAL_TAG_ETS_IMPLEMENTS: u8 = 0x1c;
pub(crate) const LITERAL_TAG_NULL: u8 = 0xff;

#[derive(Debug, Clone, Copy)]
enum PendingValueItem<'a> {
    Long(u64),
    Double(f64),
    Array(&'a ArrayValueRecord),
}

impl PendingValueItem<'_> {
    fn alignment(&self) -> u32 {
        match self {
            Self::Long(_) | Self::Double(_) => 8,
            Self::Array(_) => 1,
        }
    }

    fn byte_size(&self) -> u32 {
        match self {
            Self::Long(_) | Self::Double(_) => 8,
            Self::Array(value) => {
                uleb128(value.values.len() as u64).len() as u32
                    + value.values.len() as u32 * array_component_byte_size(value.component_type)
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum ScalarBinaryValue {
    Integer(u32),
    Float(u32),
    Id(u32),
    Offset(u32),
}

impl ScalarBinaryValue {
    fn annotation_u32(self) -> u32 {
        match self {
            Self::Integer(value) | Self::Float(value) | Self::Id(value) | Self::Offset(value) => {
                value
            }
        }
    }

    fn needs_value_item(value: &ScalarValueRecord) -> Option<PendingValueItem<'_>> {
        match value.value_type {
            VALUE_TYPE_I64 | VALUE_TYPE_U64 => Some(PendingValueItem::Long(value.scalar_u64())),
            VALUE_TYPE_F64 => Some(PendingValueItem::Double(value.scalar_f64())),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct AnnotationElementKey {
    name_off: u32,
    value: u32,
    tag: u8,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct AnnotationItemKey {
    class_idx: u16,
    elements: Vec<AnnotationElementKey>,
}

impl AnnotationItemKey {
    fn byte_size(&self) -> u32 {
        2 + 2 + (self.elements.len() as u32 * (4 + 4)) + self.elements.len() as u32
    }
}

fn insert_annotation_key(
    annotation_offsets_by_key: &mut FxHashMap<AnnotationItemKey, u32>,
    annotation_emit_order: &mut Vec<AnnotationItemKey>,
    ann_cur: &mut u32,
    key: AnnotationItemKey,
) -> u32 {
    *annotation_offsets_by_key
        .entry(key.clone())
        .or_insert_with(|| {
            let off = *ann_cur;
            *ann_cur += key.byte_size();
            annotation_emit_order.push(key);
            off
        })
}

fn scalar_value_is_32bit_integer(value_type: u32) -> bool {
    matches!(
        value_type,
        VALUE_TYPE_U1
            | VALUE_TYPE_I8
            | VALUE_TYPE_U8
            | VALUE_TYPE_I16
            | VALUE_TYPE_U16
            | VALUE_TYPE_I32
            | VALUE_TYPE_U32
            | VALUE_TYPE_STRING_NULLPTR
    )
}

fn scalar_value_is_id(value_type: u32) -> bool {
    matches!(
        value_type,
        VALUE_TYPE_STRING
            | VALUE_TYPE_RECORD
            | VALUE_TYPE_METHOD
            | VALUE_TYPE_ENUM
            | VALUE_TYPE_ANNOTATION
            | VALUE_TYPE_METHOD_HANDLE
            | VALUE_TYPE_LITERALARRAY
    )
}

fn array_component_byte_size(component_type: u32) -> u32 {
    match component_type {
        VALUE_TYPE_U1 | VALUE_TYPE_I8 | VALUE_TYPE_U8 => 1,
        VALUE_TYPE_I16 | VALUE_TYPE_U16 => 2,
        VALUE_TYPE_I32 | VALUE_TYPE_U32 | VALUE_TYPE_F32 => 4,
        VALUE_TYPE_I64 | VALUE_TYPE_U64 | VALUE_TYPE_F64 => 8,
        VALUE_TYPE_STRING
        | VALUE_TYPE_RECORD
        | VALUE_TYPE_METHOD
        | VALUE_TYPE_ENUM
        | VALUE_TYPE_ANNOTATION
        | VALUE_TYPE_METHOD_HANDLE
        | VALUE_TYPE_LITERALARRAY => 4,
        VALUE_TYPE_ARRAY => 0,
        _ => 4,
    }
}

impl ScalarValueRecord {
    fn scalar_u64(&self) -> u64 {
        match &self.value {
            ScalarValueData::U64(value) => *value,
            ScalarValueData::F32(value) => value.to_bits() as u64,
            ScalarValueData::F64(value) => value.to_bits(),
            _ => 0,
        }
    }

    fn scalar_f32(&self) -> f32 {
        match &self.value {
            ScalarValueData::F32(value) => *value,
            ScalarValueData::U64(value) => f32::from_bits(*value as u32),
            _ => 0.0,
        }
    }

    fn scalar_f64(&self) -> f64 {
        match &self.value {
            ScalarValueData::F64(value) => *value,
            ScalarValueData::U64(value) => f64::from_bits(*value),
            _ => 0.0,
        }
    }
}

/// A single entry in a Panda `LiteralArray`.
///
/// Generic class/scope buffers are encoded as item pairs:
/// `[tag_value: u8] [value: variable]`. The array count is the raw item count,
/// so each tagged entry contributes two items. Module records are a special
/// compact libpandafile layout and store raw values without tag-value items.
#[derive(Debug, Clone, PartialEq)]
pub enum LiteralArrayElement {
    Bool(bool),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    F32(f32),
    F64(f64),
    String(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum LiteralEntry {
    TaggedBool(bool),
    TaggedInteger(u32),
    TaggedFloat(f32),
    TaggedDouble(f64),
    TaggedString(String),
    TaggedMethod(u32),
    TaggedMethodName(String),
    TaggedGeneratorMethod(u32),
    TaggedGeneratorMethodName(String),
    TaggedAsyncGeneratorMethod(u32),
    TaggedAsyncGeneratorMethodName(String),
    TaggedGetter(u32),
    TaggedGetterName(String),
    TaggedSetter(u32),
    TaggedSetterName(String),
    TaggedAccessor(u8),
    TaggedMethodAffiliate(u16),
    TaggedLiteralBufferIndex(u32),
    TaggedLiteralArrayIndex(usize),
    TaggedBuiltinTypeIndex(u8),
    TaggedEtsImplements(String),
    TaggedArray {
        component_tag: u8,
        values: Vec<LiteralArrayElement>,
    },
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
    /// Optional upstream ProgramCache literal-table key.
    pub explicit_key: Option<String>,
}

impl LiteralArray {
    /// Returns `true` if any entry is a `TaggedMethodAffiliate`.
    ///
    /// Used by both the emit stage and the IR record-layout stage to classify
    /// literal arrays that contain method-affiliate metadata.
    pub fn has_method_affiliate(&self) -> bool {
        self.entries
            .iter()
            .any(|entry| matches!(entry, LiteralEntry::TaggedMethodAffiliate(_)))
    }

    /// Returns `true` when this literal array is page route metadata.
    ///
    /// Heuristic: contains a `TaggedString("pageFullPath")` entry.
    pub(super) fn is_page_route_metadata(&self) -> bool {
        self.entries.iter().any(|entry| {
            matches!(
                entry,
                LiteralEntry::TaggedString(value) | LiteralEntry::RawString(value) if value == "pageFullPath"
            )
        })
    }

    /// Returns `true` when this literal array is a `this` object stub.
    pub(super) fn is_this_object(&self) -> bool {
        matches!(
            self.entries.as_slice(),
            [
                LiteralEntry::TaggedInteger(1),
                LiteralEntry::TaggedString(name),
                ..
            ] if name == "this"
        )
    }

    /// Returns `true` when this literal array is a page `Index` stub.
    pub(super) fn is_page_index_stub(&self) -> bool {
        matches!(
            self.entries.as_slice(),
            [
                LiteralEntry::TaggedInteger(1),
                LiteralEntry::TaggedString(name),
                ..
            ] if name == "Index"
        )
    }
}

impl LiteralEntry {
    fn item_count(&self) -> u32 {
        match self {
            LiteralEntry::TaggedBool(_)
            | LiteralEntry::TaggedInteger(_)
            | LiteralEntry::TaggedFloat(_)
            | LiteralEntry::TaggedDouble(_)
            | LiteralEntry::TaggedString(_)
            | LiteralEntry::TaggedMethod(_)
            | LiteralEntry::TaggedMethodName(_)
            | LiteralEntry::TaggedGeneratorMethod(_)
            | LiteralEntry::TaggedGeneratorMethodName(_)
            | LiteralEntry::TaggedAsyncGeneratorMethod(_)
            | LiteralEntry::TaggedAsyncGeneratorMethodName(_)
            | LiteralEntry::TaggedGetter(_)
            | LiteralEntry::TaggedGetterName(_)
            | LiteralEntry::TaggedSetter(_)
            | LiteralEntry::TaggedSetterName(_)
            | LiteralEntry::TaggedAccessor(_)
            | LiteralEntry::TaggedMethodAffiliate(_)
            | LiteralEntry::TaggedLiteralBufferIndex(_)
            | LiteralEntry::TaggedLiteralArrayIndex(_)
            | LiteralEntry::TaggedBuiltinTypeIndex(_)
            | LiteralEntry::TaggedEtsImplements(_)
            | LiteralEntry::TaggedNull => 2,
            LiteralEntry::TaggedArray { values, .. } => 2 + values.len() as u32,
            LiteralEntry::RawU32(_) | LiteralEntry::RawU16(_) | LiteralEntry::RawString(_) => 1,
        }
    }
}

/// Magic bytes at offset 0x00 of every `.abc` file.
///
/// Source: `arkcompiler_runtime_core/libpandafile/file_items.h` — the literal
/// ASCII string `"PANDA"` followed by three `\x00` bytes (8 bytes total).
pub const MAGIC: [u8; 8] = *b"PANDA\x00\x00\x00";

/// Default file format version at offset 0x0C.
///
/// hvigorw (DevEco Studio 6.0, target SDK 5.0.5(17)) produces version
/// `[0x0C, 0x00, 0x06, 0x00]` = `0x0006_000C` in little-endian.
/// This matches what the ArkRuntime at API 17 expects.
///
/// Compiler entry points should use [`version_for_target_api`] instead of this
/// default so API 12 projects emit `0x0002_000C` and API 12..17 release/current
/// projects emit `0x0006_000C`.
pub const VERSION: u32 = 0x0006_000C;

pub fn version_for_target_api(api: u32, sub_api: Option<&str>) -> u32 {
    match api {
        9 | 10 => 0x0000_0009,
        11 => 0x0002_000B,
        12 if matches!(sub_api, None | Some("beta1") | Some("beta2")) => 0x0002_000C,
        12..=17 => 0x0006_000C,
        _ => 0x0001_000D,
    }
}

/// Byte length of the fixed `.abc` file header.
///
/// The Panda header is **60 bytes** (0x3C).  The class index begins
/// immediately after the header at offset `class_idx_off = 0x3C`.
const HEADER_SIZE: usize = 0x3C;
const INVALID_U32: u32 = u32::MAX;
const LAST_LITERAL_ARRAY_HEADER_VERSION: [u8; 4] = [12, 0, 6, 0];

const SOURCE_LANG_ECMASCRIPT: u8 = 0x00;

fn align4(value: u32) -> u32 {
    (value + 3) & !3
}

fn align_to(value: u32, alignment: u32) -> u32 {
    if alignment <= 1 {
        value
    } else {
        (value + alignment - 1) & !(alignment - 1)
    }
}

fn contains_literal_array_in_header(version: u32) -> bool {
    version.to_le_bytes() <= LAST_LITERAL_ARRAY_HEADER_VERSION
}

/// Assembled Panda `.abc` file ready for serialization.
pub struct AbcFile {
    /// Interned string table for bytecode string operands (`lda.str`,
    /// `tryldglobalbyname`, etc.).  Referenced by 16-bit index in opcodes.
    pub strings: StringTable,
    /// Optional upstream Program.lang preserved for ProgramCache/protoBin
    /// roundtrips. Source compilation leaves it unset and derives lang from
    /// selected class records.
    pub program_lang: Option<u32>,
    /// Upstream Program.array_types preserved for ProgramCache/protoBin
    /// roundtrips. ABC binary emission does not use this table directly.
    pub array_types: Vec<PandasmTypeRecord>,
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
            program_lang: None,
            array_types: Vec::new(),
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
    /// ## Binary layout (dynamic ABC / DevEco Studio 6.0 / es2abc v13.0.1.0)
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
    /// 0x2C         4    num_literalarrays    = 0xffffffff for v13+
    /// 0x30         4    literalarray_idx_off = 0xffffffff for v13+
    /// 0x34         4    num_idx_regions  = 1
    /// 0x38         4    idx_section_off  = after class and literal-array indexes
    ///
    /// ── class index (at 0x3C) ─────────────────────────────────────────────
    ///      4*N    class_idx  [u32 file-offsets, sorted alphabetically by name]
    ///
    /// ── literal-array index (v12.0.6.0 and older only) ───────────────────
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
    /// API 13+ replaces old API 9 stubs. hvigor emits regular ClassItems with
    /// 0 fields, 0 methods, and access_flags = 0x2001 (ACC_PUBLIC |
    /// ACC_ANNOTATION). v13 adds `L_ESExpectedPropertyCountAnnotation;`.
    ///
    /// * `L_ESConcurrentModuleRequestsAnnotation;`
    /// * `L_ESExpectedPropertyCountAnnotation;` (v13+)
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
        self.serialize_with_version(VERSION)
    }

    pub fn serialize_with_version(&self, version: u32) -> Vec<u8> {
        let literal_arrays_in_header = contains_literal_array_in_header(version);

        // ── Annotation stub classes ──────────────────────────────────────────
        let mut annotation_stubs = Vec::with_capacity(if literal_arrays_in_header { 2 } else { 3 });
        annotation_stubs.push(ClassRecord {
            name: "L_ESConcurrentModuleRequestsAnnotation;".to_string(),
            methods: vec![],
            string_indices: vec![],
            literal_indices: vec![],
            module_literal_indices: vec![],
            source_code_strings: vec![],
            access_flags: 0x2001,
            source_lang: SOURCE_LANG_ECMASCRIPT,
            metadata_attrs: Default::default(),
            conflict: false,
            params_num: 0,
            body_presence: false,
            file_location: None,
            standalone_ecmascript_input: false,
            source_file: None,
            annotations: vec![],
            module_requests: vec![],
            fields: vec![],
        });
        if !literal_arrays_in_header {
            annotation_stubs.push(ClassRecord {
                name: "L_ESExpectedPropertyCountAnnotation;".to_string(),
                methods: vec![],
                string_indices: vec![],
                literal_indices: vec![],
                module_literal_indices: vec![],
                source_code_strings: vec![],
                access_flags: 0x2001,
                source_lang: SOURCE_LANG_ECMASCRIPT,
                metadata_attrs: Default::default(),
                conflict: false,
                params_num: 0,
                body_presence: false,
                file_location: None,
                standalone_ecmascript_input: false,
                source_file: None,
                annotations: vec![],
                module_requests: vec![],
                fields: vec![],
            });
        }
        annotation_stubs.push(ClassRecord {
            name: "L_ESSlotNumberAnnotation;".to_string(),
            methods: vec![],
            string_indices: vec![],
            literal_indices: vec![],
            module_literal_indices: vec![],
            source_code_strings: vec![],
            access_flags: 0x2001,
            source_lang: SOURCE_LANG_ECMASCRIPT,
            metadata_attrs: Default::default(),
            conflict: false,
            params_num: 0,
            body_presence: false,
            file_location: None,
            standalone_ecmascript_input: false,
            source_file: None,
            annotations: vec![],
            module_requests: vec![],
            fields: vec![],
        });

        let all_classes_owned: Vec<ClassRecord> = annotation_stubs
            .iter()
            .cloned()
            .chain(self.records.iter().cloned())
            .map(|cls| cls.with_upstream_method_order())
            .collect();
        let all_classes: Vec<&ClassRecord> = all_classes_owned.iter().collect();
        let mut foreign_class_name_set = BTreeSet::new();
        for class in &all_classes {
            for field in &class.fields {
                if field.field_type.requires_panda_string_record() {
                    foreign_class_name_set.insert("Lpanda.String;".to_string());
                }
                if let Some(descriptor) = field.field_type.foreign_descriptor() {
                    foreign_class_name_set.insert(descriptor);
                }
            }
        }
        let foreign_class_names = foreign_class_name_set.into_iter().collect::<Vec<_>>();
        let foreign_section_size = foreign_class_names
            .iter()
            .map(|name| inline_string(name).len())
            .sum::<usize>() as u32;

        let total_classes = all_classes.len() as u32 + foreign_class_names.len() as u32;
        let class_idx_off = HEADER_SIZE as u32; // 0x3C
        let method_index_count = all_classes
            .iter()
            .map(|class| {
                method_index_string_refs(class, &self.strings).len()
                    + class.method_pool_entry_indices().len()
                    + class.literal_indices.len()
            })
            .sum::<usize>() as u32;
        let has_expected_property_count_annotations = all_classes.iter().any(|class| {
            class
                .methods
                .iter()
                .any(|method| method.expected_property_count.is_some())
        });

        // hvigor places the LNP index at the very end of the file (after the literal-array
        // section), not between the class index and literal-array index.  We match that layout
        // so `lnp_idx_off` is patched into the header after `total_size` is known.
        let num_literal_arrays = self.literal_arrays.len() as u32;
        let header_num_literal_arrays = if literal_arrays_in_header {
            num_literal_arrays
        } else {
            INVALID_U32
        };
        let literalarray_idx_off = if literal_arrays_in_header {
            class_idx_off + total_classes * 4
        } else {
            INVALID_U32
        };

        // v13+ deletes the header-adjacent literal-array offset table. Literal arrays
        // are still referenced through the IndexHeader method index.
        let literalarray_idx_size = if literal_arrays_in_header {
            num_literal_arrays * 4
        } else {
            0
        };
        let idx_sect_off = class_idx_off + total_classes * 4 + literalarray_idx_size;
        let primitive_field_type_ids = primitive_field_type_ids(&all_classes);
        let index_class_count = primitive_field_type_ids.len() as u32
            + u32::from(has_expected_property_count_annotations)
            + 1
            + self.records.len() as u32
            + foreign_class_names.len() as u32;
        let index_header_size: u32 = 40;
        let index_class_idx_off = idx_sect_off + index_header_size;
        let index_method_idx_off = index_class_idx_off + index_class_count * 4;
        let idx_sect_size = index_header_size + index_class_count * 4 + method_index_count * 4;

        // libpandafile writes ClassItems immediately after the IndexSection.
        // The MethodItem/FieldItem string offsets inside those ClassItems may
        // point forward to StringItems emitted later.
        let foreign_section_start = idx_sect_off + idx_sect_size;
        let class_section_start = foreign_section_start + foreign_section_size;
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

        let foreign_class_offsets: Vec<u32> = {
            let mut offsets = Vec::with_capacity(foreign_class_names.len());
            let mut pos = foreign_section_start;
            for name in &foreign_class_names {
                offsets.push(pos);
                pos += inline_string(name).len() as u32;
            }
            offsets
        };

        // Build a map from class name → item file offset (for the class index).
        let mut name_to_class_offset: FxHashMap<&str, u32> = all_classes
            .iter()
            .zip(class_item_offsets.iter())
            .map(|(c, &off)| (c.name.as_str(), off))
            .collect();
        for (name, &off) in foreign_class_names.iter().zip(foreign_class_offsets.iter()) {
            name_to_class_offset.insert(name.as_str(), off);
        }

        let mut class_index_names: Vec<&str> = all_classes
            .iter()
            .map(|class| class.name.as_str())
            .collect();
        class_index_names.extend(foreign_class_names.iter().map(String::as_str));
        class_index_names.sort();

        let string_section_start =
            class_section_start + class_item_sizes.iter().sum::<usize>() as u32;

        // Build the class-OHM-URL prefix (strip "L" prefix and ";" suffix).
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
        let mut supplemental_string_offsets = FxHashMap::<(usize, String), u32>::default();
        let mut method_string_offsets: Vec<Vec<u32>> = all_classes
            .iter()
            .map(|class| vec![0; class.methods.len()])
            .collect();
        let mut field_name_string_offsets: Vec<Vec<u32>> = all_classes
            .iter()
            .map(|class| vec![0; class.fields.len()])
            .collect();
        let mut field_value_offsets: Vec<Vec<Option<u32>>> = all_classes
            .iter()
            .map(|class| vec![None; class.fields.len()])
            .collect();
        let mut class_source_file_string_offsets: Vec<Option<u32>> = vec![None; all_classes.len()];
        let mut method_source_file_string_offsets: Vec<Vec<Option<u32>>> = all_classes
            .iter()
            .map(|class| vec![None; class.methods.len()])
            .collect();
        let mut method_source_code_string_offsets: Vec<Vec<Option<u32>>> = all_classes
            .iter()
            .map(|class| vec![None; class.methods.len()])
            .collect();
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
            for method_idx in class.method_pool_entry_indices() {
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

            for annotation in &class.annotations {
                intern_annotation_strings(annotation, &mut string_items);
            }

            if let Some(source_file) = &class.source_file {
                let off = string_items.get_or_create(source_file);
                class_source_file_string_offsets[class_idx].get_or_insert(off);
            }

            for (field_idx, field) in class.fields.iter().enumerate() {
                field_name_string_offsets[class_idx][field_idx] =
                    string_items.get_or_create(&field.name);
                if let Some(FieldValue::String(value)) = &field.value {
                    field_value_offsets[class_idx][field_idx] =
                        Some(string_items.get_or_create(value));
                }
                if let Some(value) = &field.value {
                    intern_field_value_strings(value, &mut string_items);
                }
                for annotation in &field.annotations {
                    intern_annotation_strings(annotation, &mut string_items);
                }
            }

            let mut function_order: Vec<usize> = (0..class.methods.len()).collect();
            function_order.sort_by(|&a, &b| class.methods[a].name.cmp(&class.methods[b].name));
            for method_idx in function_order {
                let method = &class.methods[method_idx];
                method_string_offsets[class_idx][method_idx] =
                    string_items.get_or_create(&method.name);

                if let Some(debug) = &method.debug_info {
                    if debug_source_file_differs_from_record(class, debug) {
                        method_source_file_string_offsets[class_idx][method_idx] =
                            Some(string_items.get_or_create(&debug.source_file));
                    }
                    if let Some(source_code) = debug
                        .source_code
                        .as_deref()
                        .filter(|value| !value.is_empty())
                    {
                        method_source_code_string_offsets[class_idx][method_idx] =
                            Some(string_items.get_or_create(source_code));
                    }
                    let is_arrow = method.kind == MethodKind::Anonymous;
                    let has_structured_local_variables = !debug.local_variables.is_empty();
                    for local in &debug.local_variables {
                        string_items.get_or_create(&local.name);
                        if !local.is_parameter(method.num_vregs) {
                            string_items.get_or_create(&local.signature);
                            if !local.signature_type.is_empty() {
                                string_items.get_or_create(&local.signature_type);
                            }
                        }
                    }
                    if !class.standalone_ecmascript_input && !has_structured_local_variables {
                        if is_arrow {
                            string_items.get_or_create("0newTarget");
                            string_items.get_or_create("0this");
                            string_items.get_or_create("4funcObj");
                        } else {
                            string_items.get_or_create("4funcObj");
                            string_items.get_or_create("any");
                            string_items.get_or_create("4newTarget");
                            if method.kind == MethodKind::Constructor {
                                string_items.get_or_create("args");
                            }
                        }
                        let mut param_entries: Vec<(String, i32)> = debug
                            .param_names
                            .iter()
                            .enumerate()
                            .map(|(param_idx, param_name)| {
                                (param_name.clone(), 3 + param_idx as i32)
                            })
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
                }

                for annotation in &method.annotations {
                    intern_annotation_strings(annotation, &mut string_items);
                }
                for param_annotations in &method.param_annotations {
                    for annotation in param_annotations {
                        intern_annotation_strings(annotation, &mut string_items);
                    }
                }
            }

            for source_code in &class.source_code_strings {
                string_items.get_or_create(source_code);
            }

            for &array_idx in &class.module_literal_indices {
                let array = &self.literal_arrays[array_idx];
                for (entry_idx, entry) in array.entries.iter().enumerate() {
                    if let Some(off) = intern_literal_entry_strings(entry, &mut string_items) {
                        literal_string_offsets[array_idx][entry_idx] = Some(off);
                    }
                }
            }

            for &array_idx in &class.literal_indices {
                let array = &self.literal_arrays[array_idx];
                for (entry_idx, entry) in array.entries.iter().enumerate() {
                    if let Some(off) = intern_literal_entry_strings(entry, &mut string_items) {
                        literal_string_offsets[array_idx][entry_idx] = Some(off);
                    }
                }
            }
        }

        for (array_idx, array) in self.literal_arrays.iter().enumerate() {
            for (entry_idx, entry) in array.entries.iter().enumerate() {
                if literal_string_offsets[array_idx][entry_idx].is_some() {
                    continue;
                }
                if let Some(off) = intern_literal_entry_strings(entry, &mut string_items) {
                    literal_string_offsets[array_idx][entry_idx] = Some(off);
                }
            }
        }

        let mut per_lnp_param_name_offsets: Vec<Vec<(String, i32, u32)>> = Vec::new();
        for (class_idx, class) in all_classes.iter().enumerate() {
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

        let needs_scope_debug = all_classes.iter().any(|class| {
            !class.standalone_ecmascript_input
                && class
                    .methods
                    .iter()
                    .any(|method| method.debug_info.is_some())
        });
        let scope_offs_regular: Option<[u32; 3]> = needs_scope_debug.then(|| {
            [
                string_items.get_or_create("4funcObj"),
                string_items.get_or_create("4newTarget"),
                string_items.get_or_create("this"),
            ]
        });
        let scope_offs_arrow: Option<[u32; 3]> = needs_scope_debug.then(|| {
            [
                string_items.get_or_create("0newTarget"),
                string_items.get_or_create("0this"),
                string_items.get_or_create("4funcObj"),
            ]
        });
        let any_type_off = needs_scope_debug.then(|| string_items.get_or_create("any"));
        let expected_property_count_str_off = has_expected_property_count_annotations
            .then(|| string_items.get_or_create("ExpectedPropertyCount"));
        let slot_number_str_off = string_items.get_or_create("SlotNumber");
        let mut cur = string_items.cur();
        let mut pending_value_items = Vec::<(u32, PendingValueItem<'_>)>::new();
        let mut annotation_value_offsets = FxHashMap::<usize, u32>::default();
        for (class_idx, class) in all_classes.iter().enumerate() {
            for annotation in &class.annotations {
                collect_pending_annotation_offsets(
                    annotation,
                    &mut cur,
                    &mut pending_value_items,
                    &mut annotation_value_offsets,
                );
            }
            for (field_idx, field) in class.fields.iter().enumerate() {
                if let Some(FieldValue::RawScalar(value)) = field.value.as_ref() {
                    collect_pending_scalar_offsets(
                        value,
                        &mut cur,
                        &mut pending_value_items,
                        &mut annotation_value_offsets,
                    );
                }
                if let Some(item) = field_pending_value_item(field.value.as_ref()) {
                    let off = push_pending_value(&mut cur, &mut pending_value_items, item);
                    field_value_offsets[class_idx][field_idx] = Some(off);
                }
                for annotation in &field.annotations {
                    collect_pending_annotation_offsets(
                        annotation,
                        &mut cur,
                        &mut pending_value_items,
                        &mut annotation_value_offsets,
                    );
                }
            }
            for method in &class.methods {
                for annotation in &method.annotations {
                    collect_pending_annotation_offsets(
                        annotation,
                        &mut cur,
                        &mut pending_value_items,
                        &mut annotation_value_offsets,
                    );
                }
                for param_annotations in &method.param_annotations {
                    for annotation in param_annotations {
                        collect_pending_annotation_offsets(
                            annotation,
                            &mut cur,
                            &mut pending_value_items,
                            &mut annotation_value_offsets,
                        );
                    }
                }
            }
        }

        let mut method_name_offsets = FxHashMap::default();
        let mut method_entity_offsets = FxHashMap::<String, u32>::default();
        let mut field_entity_offsets = FxHashMap::<String, u32>::default();
        for (class, &class_off) in all_classes.iter().zip(class_item_offsets.iter()) {
            for (field, field_off) in class.fields.iter().zip(class.field_item_offsets(class_off)) {
                field_entity_offsets.insert(format!("{}.{}", class.name, field.name), field_off);
                field_entity_offsets.insert(field.name.clone(), field_off);
            }
            for (method, method_off) in class
                .methods
                .iter()
                .zip(class.method_item_offsets(class_off))
            {
                method_name_offsets.insert(method.name.as_str(), method_off);
                method_entity_offsets
                    .insert(full_method_name(&class.name, &method.name), method_off);
                method_entity_offsets.insert(method.name.clone(), method_off);
            }
        }

        // Build the IndexHeader class/type table used by FieldItem/MethodItem/
        // AnnotationItem `class_idx`/`type_idx` values. Primitive field types
        // are stored as scalar type ids at the front, followed by referenced
        // records/foreign records.
        let mut index_class_offsets = Vec::with_capacity(index_class_count as usize);
        index_class_offsets.extend(primitive_field_type_ids.iter().copied());
        let primitive_type_index: FxHashMap<u32, u16> = primitive_field_type_ids
            .iter()
            .enumerate()
            .map(|(idx, value)| (*value, idx as u16))
            .collect();
        let mut name_to_index_class_idx = FxHashMap::default();
        let expected_property_count_annotation = annotation_stubs
            .iter()
            .find(|class| class.name == "L_ESExpectedPropertyCountAnnotation;");
        let slot_annotation = annotation_stubs
            .iter()
            .find(|class| class.name == "L_ESSlotNumberAnnotation;")
            .expect("ESSlotNumber annotation stub must be emitted");
        let mut indexed_classes = Vec::with_capacity(
            index_class_count.saturating_sub(primitive_field_type_ids.len() as u32) as usize,
        );
        if has_expected_property_count_annotations {
            indexed_classes.push(
                expected_property_count_annotation
                    .expect("ESExpectedPropertyCount annotation stub must be emitted"),
            );
        }
        indexed_classes.push(slot_annotation);
        indexed_classes.extend(self.records.iter());
        for (i, class) in indexed_classes.into_iter().enumerate() {
            let off = *name_to_class_offset
                .get(class.name.as_str())
                .expect("indexed class name must have a ClassItem offset");
            index_class_offsets.push(off);
            name_to_index_class_idx.insert(
                class.name.as_str(),
                primitive_field_type_ids.len() as u16 + i as u16,
            );
        }
        for name in &foreign_class_names {
            let off = *name_to_class_offset
                .get(name.as_str())
                .expect("foreign class name must have a ForeignItem offset");
            let idx = index_class_offsets.len() as u16;
            index_class_offsets.push(off);
            name_to_index_class_idx.insert(name.as_str(), idx);
        }
        debug_assert_eq!(index_class_offsets.len() as u32, index_class_count);

        let field_type_indices: Vec<Vec<u16>> = all_classes
            .iter()
            .map(|class| {
                class
                    .fields
                    .iter()
                    .map(|field| {
                        field_type_index(
                            &field.field_type,
                            &primitive_type_index,
                            &name_to_index_class_idx,
                        )
                    })
                    .collect()
            })
            .collect();

        let method_pool_layouts = Self::build_method_pool_layouts(&all_classes, &self.strings);
        let literal_emit_order =
            Self::build_literal_emit_order(&all_classes, self.literal_arrays.len());
        let literal_header_order = if literal_arrays_in_header {
            Self::build_literal_header_order(&all_classes, &self.literal_arrays)
        } else {
            Vec::new()
        };

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
                        LiteralEntry::TaggedBool(value) => {
                            buf.push(LITERAL_TAG_BOOL);
                            buf.push(u8::from(*value));
                        }
                        LiteralEntry::TaggedInteger(v) => {
                            buf.push(LITERAL_TAG_INTEGER);
                            buf.extend_from_slice(&v.to_le_bytes());
                        }
                        LiteralEntry::TaggedFloat(value) => {
                            buf.push(LITERAL_TAG_FLOAT);
                            buf.extend_from_slice(&value.to_bits().to_le_bytes());
                        }
                        LiteralEntry::TaggedDouble(value) => {
                            buf.push(LITERAL_TAG_DOUBLE);
                            buf.extend_from_slice(&value.to_bits().to_le_bytes());
                        }
                        LiteralEntry::TaggedString(_) | LiteralEntry::TaggedEtsImplements(_) => {
                            let offset = literal_string_offsets[array_idx][entry_idx]
                                .expect("tagged string literal offset must be resolved");
                            let tag = match entry {
                                LiteralEntry::TaggedEtsImplements(_) => LITERAL_TAG_ETS_IMPLEMENTS,
                                _ => LITERAL_TAG_STRING,
                            };
                            buf.push(tag);
                            buf.extend_from_slice(&offset.to_le_bytes());
                        }
                        LiteralEntry::TaggedMethod(off) => {
                            buf.push(LITERAL_TAG_METHOD);
                            buf.extend_from_slice(&off.to_le_bytes());
                        }
                        LiteralEntry::TaggedMethodName(name)
                        | LiteralEntry::TaggedGeneratorMethodName(name)
                        | LiteralEntry::TaggedAsyncGeneratorMethodName(name)
                        | LiteralEntry::TaggedGetterName(name)
                        | LiteralEntry::TaggedSetterName(name) => {
                            let off = method_name_offsets
                                .get(name.as_str())
                                .copied()
                                .expect("tagged method literal offset must be resolved");
                            let tag = match entry {
                                LiteralEntry::TaggedGeneratorMethodName(_) => {
                                    LITERAL_TAG_GENERATOR_METHOD
                                }
                                LiteralEntry::TaggedAsyncGeneratorMethodName(_) => {
                                    LITERAL_TAG_ASYNC_GENERATOR_METHOD
                                }
                                LiteralEntry::TaggedGetterName(_) => LITERAL_TAG_GETTER,
                                LiteralEntry::TaggedSetterName(_) => LITERAL_TAG_SETTER,
                                _ => LITERAL_TAG_METHOD,
                            };
                            buf.push(tag);
                            buf.extend_from_slice(&off.to_le_bytes());
                        }
                        LiteralEntry::TaggedGeneratorMethod(off) => {
                            buf.push(LITERAL_TAG_GENERATOR_METHOD);
                            buf.extend_from_slice(&off.to_le_bytes());
                        }
                        LiteralEntry::TaggedAsyncGeneratorMethod(off) => {
                            buf.push(LITERAL_TAG_ASYNC_GENERATOR_METHOD);
                            buf.extend_from_slice(&off.to_le_bytes());
                        }
                        LiteralEntry::TaggedGetter(off) => {
                            buf.push(LITERAL_TAG_GETTER);
                            buf.extend_from_slice(&off.to_le_bytes());
                        }
                        LiteralEntry::TaggedSetter(off) => {
                            buf.push(LITERAL_TAG_SETTER);
                            buf.extend_from_slice(&off.to_le_bytes());
                        }
                        LiteralEntry::TaggedAccessor(value) => {
                            buf.push(LITERAL_TAG_ACCESSOR);
                            buf.push(*value);
                        }
                        LiteralEntry::TaggedMethodAffiliate(value) => {
                            buf.push(LITERAL_TAG_METHOD_AFFILIATE);
                            buf.extend_from_slice(&value.to_le_bytes());
                        }
                        LiteralEntry::TaggedLiteralBufferIndex(value) => {
                            buf.push(LITERAL_TAG_LITERAL_BUFFER_INDEX);
                            buf.extend_from_slice(&value.to_le_bytes());
                        }
                        LiteralEntry::TaggedLiteralArrayIndex(index) => {
                            let offset = offsets
                                .get(*index)
                                .copied()
                                .expect("literal array index must be resolved");
                            buf.push(LITERAL_TAG_LITERAL_ARRAY);
                            buf.extend_from_slice(&offset.to_le_bytes());
                        }
                        LiteralEntry::TaggedBuiltinTypeIndex(value) => {
                            buf.push(LITERAL_TAG_BUILTIN_TYPE_INDEX);
                            buf.push(*value);
                        }
                        LiteralEntry::TaggedArray {
                            component_tag,
                            values,
                        } => {
                            buf.push(*component_tag);
                            buf.extend_from_slice(&(values.len() as u32).to_le_bytes());
                            write_typed_literal_array_values(
                                &mut buf,
                                *component_tag,
                                values,
                                &string_items,
                            );
                        }
                        LiteralEntry::TaggedNull => {
                            buf.push(LITERAL_TAG_NULL);
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
        let literal_key_to_offset: FxHashMap<String, u32> = self
            .literal_arrays
            .iter()
            .zip(literal_array_offsets.iter())
            .filter_map(|(array, &offset)| {
                array.explicit_key.as_ref().map(|key| (key.clone(), offset))
            })
            .collect();

        let mut slot_counts_per_class: Vec<Vec<u32>> = all_classes
            .iter()
            .map(|class| vec![0; class.methods.len()])
            .collect();
        for (class_idx, class) in all_classes.iter().enumerate() {
            let layout = &method_pool_layouts[class_idx];
            let mut defineclass_cursor = 0usize;
            let mut definefunc_cursor = 0usize;
            for (method_idx, method) in class.methods.iter().enumerate() {
                let mut bytecode = patch_bytecode_index_operands(
                    &method.bytecode,
                    &layout.string_pool_indices,
                    &layout.literal_pool_indices,
                    &layout.ctor_pool_indices,
                    &layout.definefunc_pool_indices,
                    &layout.definemethod_pool_indices_by_name,
                    &mut defineclass_cursor,
                    &mut definefunc_cursor,
                );
                slot_counts_per_class[class_idx][method_idx] = method
                    .slots_num
                    .unwrap_or_else(|| assign_inline_cache_slots(&mut bytecode));
            }
        }

        // ── 4. Pre-compute AnnotationItem section ────────────────────────────
        //
        // es2panda emits function annotations before CodeItems. FunctionEmitter
        // appends ExpectedPropertyCount first (when non-zero) and SlotNumber
        // for every method, and libpandafile preserves that per-method order.
        //
        // AnnotationItem format (libpandafile annotation_data_accessor.h):
        //   class_idx: u16 LE  (2 = L_ESSlotNumberAnnotation in IndexSection)
        //   count:     u16 LE  (1 element)
        //   elements:  [(name_off: u32 LE, value: u32 LE)] * count
        //   tags:      [tag: u8] * count  (0x37 = 'U32' type tag, after ALL elements)
        // = 2+2+8+1 = 13 bytes per annotation
        let annotation_section_start = cur;
        let annotation_order_per_class: Vec<Vec<usize>> = all_classes
            .iter()
            .map(|class| {
                let mut order: Vec<usize> = (0..class.methods.len()).collect();
                order.sort_by(|&a, &b| class.methods[a].name.cmp(&class.methods[b].name));
                order
            })
            .collect();
        let expected_property_count_annotation_class_idx =
            expected_property_count_str_off.map(|_| {
                *name_to_index_class_idx
                    .get("L_ESExpectedPropertyCountAnnotation;")
                    .expect("ExpectedPropertyCount annotation class must be indexed")
            });
        let slot_number_annotation_class_idx = *name_to_index_class_idx
            .get("L_ESSlotNumberAnnotation;")
            .expect("SlotNumber annotation class must be indexed");
        let entity_offsets = EntityOffsetMaps {
            class_offsets: &name_to_class_offset,
            method_offsets: &method_entity_offsets,
            field_offsets: &field_entity_offsets,
            literal_offsets: &literal_key_to_offset,
        };

        let mut class_annotation_offsets_per_class: Vec<ClassAnnotationOffsets> =
            Vec::with_capacity(all_classes.len());
        let mut field_annotation_offsets_per_class: Vec<Vec<FieldAnnotationOffsets>> =
            Vec::with_capacity(all_classes.len());
        let mut annotation_offsets_per_class: Vec<Vec<MethodAnnotationOffsets>> =
            Vec::with_capacity(all_classes.len());
        let mut param_annotation_offsets_per_class: Vec<Vec<Option<Vec<Vec<u32>>>>> =
            Vec::with_capacity(all_classes.len());
        let mut annotation_emit_order = Vec::new();
        let mut annotation_offsets_by_key = FxHashMap::<AnnotationItemKey, u32>::default();
        let mut annotation_offsets_by_ptr = FxHashMap::<usize, u32>::default();
        let mut ann_cur = annotation_section_start;
        {
            for (class_idx, class) in all_classes.iter().enumerate() {
                let mut class_annotation_offsets = ClassAnnotationOffsets::default();
                for annotation in &class.annotations {
                    let off = insert_annotation_record(
                        annotation,
                        &name_to_index_class_idx,
                        &string_items,
                        &entity_offsets,
                        &literal_array_offsets,
                        &annotation_value_offsets,
                        &mut annotation_offsets_by_key,
                        &mut annotation_offsets_by_ptr,
                        &mut annotation_emit_order,
                        &mut ann_cur,
                    );
                    class_annotation_offsets.custom.push(off);
                }
                class_annotation_offsets_per_class.push(class_annotation_offsets);

                let mut field_annotation_offsets = Vec::with_capacity(class.fields.len());
                field_annotation_offsets
                    .resize(class.fields.len(), FieldAnnotationOffsets::default());
                for (field_idx, field) in class.fields.iter().enumerate() {
                    if let Some(FieldValue::RawScalar(value)) = field.value.as_ref() {
                        register_nested_scalar_annotation(
                            value,
                            &name_to_index_class_idx,
                            &string_items,
                            &entity_offsets,
                            &literal_array_offsets,
                            &annotation_value_offsets,
                            &mut annotation_offsets_by_key,
                            &mut annotation_offsets_by_ptr,
                            &mut annotation_emit_order,
                            &mut ann_cur,
                        );
                    }
                    for annotation in &field.annotations {
                        let off = insert_annotation_record(
                            annotation,
                            &name_to_index_class_idx,
                            &string_items,
                            &entity_offsets,
                            &literal_array_offsets,
                            &annotation_value_offsets,
                            &mut annotation_offsets_by_key,
                            &mut annotation_offsets_by_ptr,
                            &mut annotation_emit_order,
                            &mut ann_cur,
                        );
                        field_annotation_offsets[field_idx].custom.push(off);
                    }
                }
                field_annotation_offsets_per_class.push(field_annotation_offsets);

                let mut method_ann_offs = Vec::with_capacity(class.methods.len());
                method_ann_offs.resize(class.methods.len(), MethodAnnotationOffsets::default());
                let mut method_param_ann_offsets = Vec::with_capacity(class.methods.len());
                method_param_ann_offsets.resize(class.methods.len(), None);
                for &method_idx in &annotation_order_per_class[class_idx] {
                    let method = &class.methods[method_idx];
                    for annotation in &method.annotations {
                        let off = insert_annotation_record(
                            annotation,
                            &name_to_index_class_idx,
                            &string_items,
                            &entity_offsets,
                            &literal_array_offsets,
                            &annotation_value_offsets,
                            &mut annotation_offsets_by_key,
                            &mut annotation_offsets_by_ptr,
                            &mut annotation_emit_order,
                            &mut ann_cur,
                        );
                        method_ann_offs[method_idx].custom.push(off);
                    }

                    if let Some(expected_property_count) = method.expected_property_count {
                        let key = AnnotationItemKey {
                            class_idx: expected_property_count_annotation_class_idx
                                .expect("ExpectedPropertyCount annotation class must be indexed"),
                            elements: vec![AnnotationElementKey {
                                name_off: expected_property_count_str_off
                                    .expect("ExpectedPropertyCount string must be interned"),
                                value: expected_property_count,
                                tag: b'7',
                            }],
                        };
                        let off = insert_annotation_key(
                            &mut annotation_offsets_by_key,
                            &mut annotation_emit_order,
                            &mut ann_cur,
                            key,
                        );
                        method_ann_offs[method_idx].expected_property_count = Some(off);
                    }

                    let key = AnnotationItemKey {
                        class_idx: slot_number_annotation_class_idx,
                        elements: vec![AnnotationElementKey {
                            name_off: slot_number_str_off,
                            value: slot_counts_per_class[class_idx][method_idx],
                            tag: b'7',
                        }],
                    };
                    let off = insert_annotation_key(
                        &mut annotation_offsets_by_key,
                        &mut annotation_emit_order,
                        &mut ann_cur,
                        key,
                    );
                    method_ann_offs[method_idx].slot_number = off;

                    if method.has_param_annotations() {
                        let param_count =
                            usize::max(method.num_args as usize, method.param_annotations.len());
                        let mut param_offsets = vec![Vec::new(); param_count];
                        for (param_idx, param_annotations) in
                            method.param_annotations.iter().enumerate()
                        {
                            for annotation in param_annotations {
                                let off = insert_annotation_record(
                                    annotation,
                                    &name_to_index_class_idx,
                                    &string_items,
                                    &entity_offsets,
                                    &literal_array_offsets,
                                    &annotation_value_offsets,
                                    &mut annotation_offsets_by_key,
                                    &mut annotation_offsets_by_ptr,
                                    &mut annotation_emit_order,
                                    &mut ann_cur,
                                );
                                param_offsets[param_idx].push(off);
                            }
                        }
                        method_param_ann_offsets[method_idx] = Some(param_offsets);
                    }
                }
                annotation_offsets_per_class.push(method_ann_offs);
                param_annotation_offsets_per_class.push(method_param_ann_offsets);
            }
        }
        let annotation_section_size = annotation_emit_order
            .iter()
            .map(AnnotationItemKey::byte_size)
            .sum::<u32>();
        cur += annotation_section_size;

        let param_annotation_section_start = cur;
        let mut param_annotation_section = Vec::new();
        for (class_idx, _class) in all_classes.iter().enumerate() {
            for &method_idx in &annotation_order_per_class[class_idx] {
                let Some(param_annotation_offsets) =
                    param_annotation_offsets_per_class[class_idx][method_idx].as_ref()
                else {
                    continue;
                };
                annotation_offsets_per_class[class_idx][method_idx].param_annotations = Some(cur);
                param_annotation_section
                    .extend_from_slice(&(param_annotation_offsets.len() as u32).to_le_bytes());
                cur += 4;
                for annotations in param_annotation_offsets {
                    param_annotation_section
                        .extend_from_slice(&(annotations.len() as u32).to_le_bytes());
                    cur += 4;
                    for &annotation_off in annotations {
                        param_annotation_section.extend_from_slice(&annotation_off.to_le_bytes());
                        cur += 4;
                    }
                }
            }
        }
        for (class_idx, class) in all_classes.iter().enumerate() {
            for (field_idx, field) in class.fields.iter().enumerate() {
                if field_value_offsets[class_idx][field_idx].is_some() {
                    continue;
                }
                if let Some(FieldValue::RawScalar(value)) = field.value.as_ref() {
                    if scalar_value_is_id(value.value_type) {
                        field_value_offsets[class_idx][field_idx] =
                            Some(resolve_scalar_id_existing(
                                value,
                                &string_items,
                                &entity_offsets,
                                &mut annotation_offsets_by_ptr,
                            ));
                    }
                }
            }
        }
        let field_value_section = encode_pending_value_section(
            string_items.cur(),
            &pending_value_items,
            &string_items,
            &entity_offsets,
            &mut annotation_offsets_by_ptr,
        );

        // ── 5. Compute CodeItem start offsets ─────────────────────────────────
        let code_section_start: u32 = cur;

        let mut code_offsets: Vec<Vec<u32>> = Vec::new();
        let mut all_code_items: Vec<Vec<u8>> = Vec::new();

        let mut code_cur = code_section_start;
        for (class_idx, class) in all_classes.iter().enumerate() {
            let mut class_code_offs = vec![0u32; class.methods.len()];
            let mut class_code_items = Vec::with_capacity(class.methods.len());
            let layout = &method_pool_layouts[class_idx];
            let mut defineclass_cursor = 0usize;
            let mut definefunc_cursor = 0usize;
            for method in &class.methods {
                let bytecode = patch_bytecode_index_operands(
                    &method.bytecode,
                    &layout.string_pool_indices,
                    &layout.literal_pool_indices,
                    &layout.ctor_pool_indices,
                    &layout.definefunc_pool_indices,
                    &layout.definemethod_pool_indices_by_name,
                    &mut defineclass_cursor,
                    &mut definefunc_cursor,
                );
                let mut bytecode = bytecode;
                let slot_count = assign_inline_cache_slots(&mut bytecode);
                slot_counts_per_class[class_idx][class_code_items.len()] =
                    method.slots_num.unwrap_or(slot_count);
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
        // Each debug method gets an LNP with optional SET_FILE/SET_SOURCE_CODE
        // prefixes, advance sequences, and END_SEQUENCE.  libpandafile only
        // emits SET_FILE when the function source file differs from the owner
        // record source file; otherwise ClassTag::SOURCE_FILE is the default.
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
            struct PendingDebugMethod {
                class_idx: usize,
                method_idx: usize,
                lnp_data: Vec<u8>,
                constant_pool: Vec<u8>,
                line_start: u32,
                param_name_offsets: Vec<u32>,
                emit_scope_locals: bool,
            }

            struct UniqueLnp {
                data: Vec<u8>,
                ref_count: u32,
                representative_class_idx: usize,
                representative_method_idx: usize,
                representative_order: usize,
                index: u32,
                offset: u32,
            }

            let mut per_class_offsets: Vec<Vec<Option<u32>>> = all_classes
                .iter()
                .map(|class| vec![None; class.methods.len()])
                .collect();
            let mut pending_debug_methods = Vec::new();
            let mut lnp_param_cursor = 0usize;

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

            for (class_idx, class) in all_classes.iter().enumerate() {
                let mut method_order: Vec<usize> = (0..class.methods.len()).collect();
                method_order.sort_by(|&a, &b| class.methods[a].name.cmp(&class.methods[b].name));
                for method_idx in method_order {
                    let method = &class.methods[method_idx];
                    if let Some(debug) = &method.debug_info {
                        let method_source_file_off =
                            method_source_file_string_offsets[class_idx][method_idx];
                        let method_source_code_off =
                            method_source_code_string_offsets[class_idx][method_idx];

                        let param_offs = per_lnp_param_name_offsets
                            .get(lnp_param_cursor)
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
                        let is_arrow = method.kind == MethodKind::Anonymous;
                        let has_structured_local_variables = !debug.local_variables.is_empty();
                        let emit_scope_locals =
                            !class.standalone_ecmascript_input && !has_structured_local_variables;
                        let debug_param_name_offsets = if has_structured_local_variables {
                            local_variable_debug_param_offsets(method, &string_items)
                        } else {
                            Vec::new()
                        };
                        let mut param_offs = param_offs;
                        let is_generated_constructor = method.kind == MethodKind::Constructor
                            && param_offs.is_empty()
                            && method.num_args > 3;
                        if emit_scope_locals && is_generated_constructor {
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
                        //   [SC → cp: source_code_off, SF → cp: method_source_file_off]
                        //     only when function source file differs from record source file
                        //   PC(num_args×2) → cp: prologue_size  [arg-copy prologue]
                        //   SLE(reg) × (3+params) → cp: name,type,typesig per var
                        //   [body from line_ops: LN+SP+SC+PC per instruction]
                        //   EL(reg) × (3+params) → cp: closing_pc=0
                        //   END

                        if let Some(src_off) = method_source_file_off {
                            if let Some(source_code_off) = method_source_code_off {
                                lnp_data.push(0x0a);
                                cp.extend_from_slice(&uleb128(source_code_off as u64));
                            }

                            // SET_FILE
                            lnp_data.push(0x09);
                            cp.extend_from_slice(&uleb128(src_off as u64));
                        }

                        // ADV_PC(prologue_size): skip hidden entry instructions
                        // before the source-visible function scope starts.
                        // es2panda emits the real mov.v4/mov.v8/mov.v16 argument
                        // copy width, and module entries keep a leading
                        // newlexenvwithname before those copies.
                        let prologue_size = debug_scope_start_pc(method);
                        if emit_scope_locals {
                            lnp_data.push(0x01); // ADV_PC
                            cp.extend_from_slice(&uleb128(prologue_size as u64));
                        }

                        // START_LOCAL_EXTENDED for scope vars.  Non-arrow
                        // functions emit `this` after regular locals because
                        // es2panda records function-scope bindings first, then
                        // the mandatory `this` binding.
                        let pre_scope_locals: Vec<(String, i32, u32)> = if method.name
                            == "func_main_0"
                            && emit_scope_locals
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
                        let any_type_off = any_type_off.unwrap_or_default();
                        if emit_scope_locals {
                            for &(_, reg, name_off) in &pre_scope_locals {
                                lnp_data.push(0x04);
                                lnp_data.extend_from_slice(&encode_sleb128_bytes(reg));
                                cp.extend_from_slice(&uleb128(name_off as u64));
                                cp.extend_from_slice(&uleb128(any_type_off as u64));
                                cp.extend_from_slice(&uleb128(any_type_off as u64));
                            }

                            let scope_offs_regular = scope_offs_regular
                                .expect("regular scope offsets must exist when scope debug emits");
                            let scope_offs_arrow = scope_offs_arrow
                                .expect("arrow scope offsets must exist when scope debug emits");
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
                        }
                        // START_LOCAL_EXTENDED for locals/params, sorted like
                        // es2panda's scope binding map but keeping original
                        // register assignments.
                        let mut local_offs = param_offs.clone();
                        if emit_scope_locals && !is_arrow {
                            if !debug.lexical_this {
                                let scope_offs_regular = scope_offs_regular.expect(
                                    "regular scope offsets must exist when scope debug emits",
                                );
                                local_offs.push(("this".to_string(), 2, scope_offs_regular[2]));
                            }
                            local_offs.sort_by(|a, b| a.0.cmp(&b.0));
                        }
                        if emit_scope_locals {
                            for &(_, reg, param_off) in &local_offs {
                                lnp_data.push(0x04);
                                lnp_data.extend_from_slice(&encode_sleb128_bytes(reg));
                                cp.extend_from_slice(&uleb128(param_off as u64));
                                cp.extend_from_slice(&uleb128(any_type_off as u64));
                                cp.extend_from_slice(&uleb128(any_type_off as u64));
                            }
                        }

                        // Body opcodes + cp from line_ops, interleaving local
                        // variable events that start/end inside the instruction
                        // stream. This matches libpandafile
                        // BuildLineNumberProgram, which flushes pending PC
                        // before local start/end events.
                        if has_structured_local_variables {
                            let mid_locals = local_variable_debug_events(method, &string_items);
                            append_line_ops_with_mid_locals(
                                method_ops,
                                0,
                                &mid_locals,
                                &mut lnp_data,
                                &mut cp,
                            );
                        } else if emit_scope_locals {
                            let mid_locals = infer_func_main_class_local_events(
                                method,
                                class,
                                &string_items,
                                any_type_off,
                            );
                            append_line_ops_with_mid_locals(
                                method_ops,
                                prologue_size,
                                &mid_locals,
                                &mut lnp_data,
                                &mut cp,
                            );
                        } else {
                            append_line_ops_with_mid_locals(
                                method_ops,
                                0,
                                &[],
                                &mut lnp_data,
                                &mut cp,
                            );
                        }

                        // END_LOCAL in the same order as START_LOCAL.
                        if emit_scope_locals {
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
                        }

                        lnp_data.push(0x00); // END_SEQUENCE

                        pending_debug_methods.push(PendingDebugMethod {
                            class_idx,
                            method_idx,
                            lnp_data,
                            constant_pool: cp,
                            line_start: debug.line_start,
                            param_name_offsets: debug_param_name_offsets,
                            emit_scope_locals: !class.standalone_ecmascript_input,
                        });
                        lnp_param_cursor += 1;
                    }
                }
            }

            let mut unique_by_data: FxHashMap<Vec<u8>, usize> = FxHashMap::default();
            let mut unique_lnps: Vec<UniqueLnp> = Vec::new();
            let mut pending_unique_indices = Vec::with_capacity(pending_debug_methods.len());
            for pending in &pending_debug_methods {
                let representative_order = pending.class_idx * 1_000_000 + pending.method_idx;
                let unique_idx = if let Some(&idx) = unique_by_data.get(&pending.lnp_data) {
                    unique_lnps[idx].ref_count += 1;
                    if representative_order < unique_lnps[idx].representative_order {
                        unique_lnps[idx].representative_class_idx = pending.class_idx;
                        unique_lnps[idx].representative_method_idx = pending.method_idx;
                        unique_lnps[idx].representative_order = representative_order;
                    }
                    idx
                } else {
                    let idx = unique_lnps.len();
                    unique_by_data.insert(pending.lnp_data.clone(), idx);
                    unique_lnps.push(UniqueLnp {
                        data: pending.lnp_data.clone(),
                        ref_count: 1,
                        representative_class_idx: pending.class_idx,
                        representative_method_idx: pending.method_idx,
                        representative_order,
                        index: 0,
                        offset: 0,
                    });
                    idx
                };
                pending_unique_indices.push(unique_idx);
            }

            let mut lnp_index_order: Vec<usize> = (0..unique_lnps.len()).collect();
            lnp_index_order.sort_by(|&a, &b| {
                unique_lnps[b]
                    .ref_count
                    .cmp(&unique_lnps[a].ref_count)
                    .then_with(|| {
                        unique_lnps[a]
                            .representative_order
                            .cmp(&unique_lnps[b].representative_order)
                    })
            });
            for (index, &unique_idx) in lnp_index_order.iter().enumerate() {
                unique_lnps[unique_idx].index = index as u32;
            }

            let mut section_buf = Vec::new();
            let mut cur = code_cur;
            let mut emitted_unique_lnps = vec![false; unique_lnps.len()];
            for (pending, &unique_idx) in pending_debug_methods
                .iter()
                .zip(pending_unique_indices.iter())
            {
                if !emitted_unique_lnps[unique_idx]
                    && pending.class_idx == unique_lnps[unique_idx].representative_class_idx
                    && pending.method_idx == unique_lnps[unique_idx].representative_method_idx
                {
                    unique_lnps[unique_idx].offset = cur;
                    section_buf.extend_from_slice(&unique_lnps[unique_idx].data);
                    cur += unique_lnps[unique_idx].data.len() as u32;
                    emitted_unique_lnps[unique_idx] = true;
                }

                per_class_offsets[pending.class_idx][pending.method_idx] = Some(cur);
                let di_bytes = encode_debug_info_with_cp(
                    pending.line_start,
                    &pending.param_name_offsets,
                    &pending.constant_pool,
                    unique_lnps[unique_idx].index,
                    pending.emit_scope_locals,
                );
                section_buf.extend_from_slice(&di_bytes);
                cur += di_bytes.len() as u32;
            }

            let mut lnp_offsets = vec![0u32; unique_lnps.len()];
            for unique in &unique_lnps {
                lnp_offsets[unique.index as usize] = unique.offset;
            }
            (per_class_offsets, section_buf, lnp_offsets)
        };
        let num_lnp = lnp_offsets.len() as u32;

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
        let entity_region_start = if foreign_section_size == 0 {
            first_class_off
        } else {
            foreign_section_start
        };

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
                    &class_annotation_offsets_per_class[ci],
                    &field_name_string_offsets[ci],
                    class_source_file_string_offsets[ci],
                    &field_annotation_offsets_per_class[ci],
                    &method_string_offsets[ci],
                    &code_offsets[ci],
                    &debug_info_offsets[ci],
                    &annotation_offsets_per_class[ci],
                    &literal_array_offsets,
                    &field_type_indices[ci],
                    &field_value_offsets[ci],
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
        file.extend_from_slice(&version.to_le_bytes()); // 0x0C version
        file.extend_from_slice(&0u32.to_le_bytes()); // 0x10 file_size (patched)
        file.extend_from_slice(
            &(if foreign_section_size == 0 {
                0
            } else {
                foreign_section_start
            })
            .to_le_bytes(),
        ); // 0x14 foreign_off
        file.extend_from_slice(&foreign_section_size.to_le_bytes()); // 0x18 foreign_size
        file.extend_from_slice(&total_classes.to_le_bytes()); // 0x1C num_classes
        file.extend_from_slice(&class_idx_off.to_le_bytes()); // 0x20 class_idx_off = 0x3C
        file.extend_from_slice(&num_lnp.to_le_bytes()); // 0x24 num_lnp
        file.extend_from_slice(&lnp_idx_off.to_le_bytes()); // 0x28 lnp_idx_off (at file end)
        file.extend_from_slice(&header_num_literal_arrays.to_le_bytes()); // 0x2C num_literalarrays
        file.extend_from_slice(&literalarray_idx_off.to_le_bytes()); // 0x30 literalarray_idx_off
        file.extend_from_slice(&1u32.to_le_bytes()); // 0x34 num_idx_regions = 1
        file.extend_from_slice(&idx_sect_off.to_le_bytes()); // 0x38 idx_section_off
        debug_assert_eq!(file.len(), HEADER_SIZE, "header must be exactly 0x3C bytes");

        // ── Class index (at 0x3C, sorted by class name) ───────────────────────
        for class_name in &class_index_names {
            let off = *name_to_class_offset
                .get(*class_name)
                .expect("class name must be in class index");
            file.extend_from_slice(&off.to_le_bytes());
        }
        debug_assert_eq!(
            file.len() as u32,
            class_idx_off + total_classes * 4,
            "class index must end at the upstream class index boundary"
        );

        // ── LiteralArray index (v12.0.6.0 and older only) ────────────────────
        //
        // v13.0.0.0 deleted this offsets-array. Literal arrays are reachable through
        // the IndexHeader method index, matching libpandafile's v13 ItemContainer.
        if literal_arrays_in_header {
            for &literal_idx in &literal_header_order {
                file.extend_from_slice(&literal_array_offsets[literal_idx].to_le_bytes());
            }
        }
        debug_assert_eq!(
            file.len() as u32,
            idx_sect_off,
            "pre-IndexSection header indexes must end exactly at idx_sect_off"
        );

        // ── IndexSection ─────────────────────────────────────────────────────
        //
        // libpandafile expects a 40-byte IndexHeader followed by EntityId
        // arrays.  The entity region spans from the first ClassItem to the end
        // of the complete file (including the trailing LNP index).
        file.extend_from_slice(&entity_region_start.to_le_bytes()); // start
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

        debug_assert_eq!(file.len() as u32, foreign_section_start);

        // ── ForeignItems ─────────────────────────────────────────────────────
        for name in &foreign_class_names {
            file.extend_from_slice(&inline_string(name));
        }

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

        file.extend_from_slice(&field_value_section);

        if let Some(first_literal_array_off) = literal_array_offsets.iter().min().copied() {
            debug_assert_eq!(file.len() as u32, first_literal_array_off);
        }

        // ── LiteralArray section ──────────────────────────────────────────────
        file.extend_from_slice(&lit_arr_section);

        debug_assert_eq!(file.len() as u32, annotation_section_start);

        // ── AnnotationItems ───────────────────────────────────────────────────
        //
        // Format per annotation (libpandafile AnnotationDataAccessor):
        //   class_idx: u16 LE  (annotation class in IndexSection)
        //   count:     u16 LE  (1 element)
        //   elements:  [(name_off: u32 LE, value: u32 LE)] * count
        //   tags:      [tag: u8] * count  (0x37 = 'U32' type tag)
        // = 2+2+8+1 = 13 bytes per annotation
        //
        for annotation in &annotation_emit_order {
            file.extend_from_slice(&annotation.class_idx.to_le_bytes());
            file.extend_from_slice(&(annotation.elements.len() as u16).to_le_bytes());
            for element in &annotation.elements {
                file.extend_from_slice(&element.name_off.to_le_bytes());
                file.extend_from_slice(&element.value.to_le_bytes());
            }
            for element in &annotation.elements {
                file.push(element.tag);
            }
        }
        debug_assert_eq!(file.len() as u32, param_annotation_section_start);

        // ── ParamAnnotationsItems ────────────────────────────────────────────
        //
        // Format per libpandafile ParamAnnotationsItem:
        //   params_count: u32
        //   repeated params_count times:
        //     annotations_count: u32
        //     annotation_off: u32 * annotations_count
        file.extend_from_slice(&param_annotation_section);
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
        let checksum = Self::adler32(&file[12..]);
        file[0x08..0x0C].copy_from_slice(&checksum.to_le_bytes());

        file
    }
}

fn primitive_field_type_ids(classes: &[&ClassRecord]) -> Vec<u32> {
    let mut ids = BTreeSet::new();
    for class in classes {
        for field in &class.fields {
            if let Some(id) = field.field_type.binary_type_id() {
                ids.insert(id);
            }
        }
    }
    ids.into_iter().collect()
}

fn field_type_index(
    field_type: &FieldType,
    primitive_type_index: &FxHashMap<u32, u16>,
    name_to_index_class_idx: &FxHashMap<&str, u16>,
) -> u16 {
    if let Some(id) = field_type.binary_type_id() {
        return *primitive_type_index
            .get(&id)
            .expect("primitive field type must be present in IndexSection");
    }
    if let FieldType::Record { component_name } = field_type {
        let descriptor = pandasm_reference_descriptor(component_name);
        return *name_to_index_class_idx
            .get(descriptor.as_str())
            .expect("record field type must be present in IndexSection");
    }
    if let Some(descriptor) = field_type.foreign_descriptor() {
        return *name_to_index_class_idx
            .get(descriptor.as_str())
            .expect("foreign field type must be present in IndexSection");
    }
    match field_type {
        FieldType::U1
        | FieldType::I8
        | FieldType::U8
        | FieldType::I16
        | FieldType::U16
        | FieldType::I32
        | FieldType::U32
        | FieldType::F32
        | FieldType::F64
        | FieldType::I64
        | FieldType::U64
        | FieldType::Any => unreachable!(),
        FieldType::PandaString | FieldType::Record { .. } | FieldType::Array { .. } => {
            unreachable!()
        }
    }
}

fn pandasm_reference_descriptor(component_name: &str) -> String {
    let mut descriptor = String::with_capacity(component_name.len() + 2);
    descriptor.push('L');
    descriptor.push_str(component_name);
    descriptor.push(';');
    descriptor
}

#[derive(Debug, Default)]
struct MethodPoolLayout {
    string_refs: Vec<MethodIndexStringRef>,
    string_pool_indices: FxHashMap<u16, u16>,
    literal_pool_indices: FxHashMap<u16, u16>,
    definemethod_pool_indices_by_name: FxHashMap<u16, u16>,
    method_indices: Vec<usize>,
    definefunc_pool_indices: Vec<u16>,
    ctor_pool_indices: Vec<u16>,
}

pub(super) struct StringItemWriter {
    cur: u32,
    offsets: FxHashMap<String, u32>,
    items: Vec<Vec<u8>>,
}

impl StringItemWriter {
    fn new(start: u32) -> Self {
        Self {
            cur: start,
            offsets: FxHashMap::default(),
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

    pub(super) fn id_of(&self, value: &str) -> Option<u32> {
        self.offsets.get(value).copied()
    }

    fn cur(&self) -> u32 {
        self.cur
    }

    fn items(&self) -> &[Vec<u8>] {
        &self.items
    }
}

fn intern_literal_entry_strings(
    entry: &LiteralEntry,
    string_items: &mut StringItemWriter,
) -> Option<u32> {
    match entry {
        LiteralEntry::TaggedString(value)
        | LiteralEntry::TaggedEtsImplements(value)
        | LiteralEntry::RawString(value) => Some(string_items.get_or_create(value)),
        LiteralEntry::TaggedArray {
            component_tag: LITERAL_TAG_ARRAY_STRING,
            values,
        } => {
            for value in values {
                if let LiteralArrayElement::String(value) = value {
                    string_items.get_or_create(value);
                }
            }
            None
        }
        _ => None,
    }
}

fn write_typed_literal_array_values(
    buf: &mut Vec<u8>,
    component_tag: u8,
    values: &[LiteralArrayElement],
    string_items: &StringItemWriter,
) {
    for value in values {
        match (component_tag, value) {
            (LITERAL_TAG_ARRAY_U1, LiteralArrayElement::Bool(value)) => {
                buf.push(u8::from(*value));
            }
            (
                LITERAL_TAG_ARRAY_U8 | LITERAL_TAG_ARRAY_I8,
                LiteralArrayElement::U8(value),
            ) => {
                buf.push(*value);
            }
            (
                LITERAL_TAG_ARRAY_U16 | LITERAL_TAG_ARRAY_I16,
                LiteralArrayElement::U16(value),
            ) => {
                buf.extend_from_slice(&value.to_le_bytes());
            }
            (
                LITERAL_TAG_ARRAY_U32 | LITERAL_TAG_ARRAY_I32,
                LiteralArrayElement::U32(value),
            ) => {
                buf.extend_from_slice(&value.to_le_bytes());
            }
            (
                LITERAL_TAG_ARRAY_U64 | LITERAL_TAG_ARRAY_I64,
                LiteralArrayElement::U64(value),
            ) => {
                buf.extend_from_slice(&value.to_le_bytes());
            }
            (LITERAL_TAG_ARRAY_F32, LiteralArrayElement::F32(value)) => {
                buf.extend_from_slice(&value.to_bits().to_le_bytes());
            }
            (LITERAL_TAG_ARRAY_F64, LiteralArrayElement::F64(value)) => {
                buf.extend_from_slice(&value.to_bits().to_le_bytes());
            }
            (LITERAL_TAG_ARRAY_STRING, LiteralArrayElement::String(value)) => {
                let offset = string_items
                    .id_of(value)
                    .unwrap_or_else(|| panic!("typed literal array string {value} must be interned"));
                buf.extend_from_slice(&offset.to_le_bytes());
            }
            _ => panic!(
                "typed literal array value {value:?} does not match component tag {component_tag:#x}"
            ),
        }
    }
}

fn intern_field_value_strings(value: &FieldValue, string_items: &mut StringItemWriter) {
    match value {
        FieldValue::String(value) => {
            string_items.get_or_create(value);
        }
        FieldValue::RawScalar(value) => intern_scalar_value_strings(value, string_items),
        FieldValue::U8(_)
        | FieldValue::Bool(_)
        | FieldValue::U32(_)
        | FieldValue::LiteralArray(_)
        | FieldValue::F64(_) => {}
    }
}

fn intern_annotation_strings(annotation: &AnnotationRecord, string_items: &mut StringItemWriter) {
    for element in &annotation.elements {
        string_items.get_or_create(&element.name);
        intern_annotation_value_strings(&element.value, string_items);
    }
}

fn intern_annotation_value_strings(value: &AnnotationValue, string_items: &mut StringItemWriter) {
    match value {
        AnnotationValue::String(value) => {
            string_items.get_or_create(value);
        }
        AnnotationValue::RawScalar(value) => intern_scalar_value_strings(value, string_items),
        AnnotationValue::Array(value) => intern_array_value_strings(value, string_items),
        AnnotationValue::Bool(_)
        | AnnotationValue::U32(_)
        | AnnotationValue::F64(_)
        | AnnotationValue::LiteralArray { .. } => {}
    }
}

fn intern_scalar_value_strings(value: &ScalarValueRecord, string_items: &mut StringItemWriter) {
    match &value.value {
        ScalarValueData::String(string) if value.value_type == VALUE_TYPE_STRING => {
            string_items.get_or_create(string);
        }
        ScalarValueData::Annotation(annotation) => {
            intern_annotation_strings(annotation, string_items);
        }
        _ => {}
    }
}

fn intern_array_value_strings(value: &ArrayValueRecord, string_items: &mut StringItemWriter) {
    for element in &value.values {
        if value.component_type == VALUE_TYPE_STRING {
            if let ScalarValueData::String(value) = &element.value {
                string_items.get_or_create(value);
            }
        }
        if let ScalarValueData::Annotation(annotation) = &element.value {
            intern_annotation_strings(annotation, string_items);
        }
    }
}

fn push_pending_value<'a>(
    cur: &mut u32,
    pending_items: &mut Vec<(u32, PendingValueItem<'a>)>,
    item: PendingValueItem<'a>,
) -> u32 {
    let aligned = align_to(*cur, item.alignment());
    let off = aligned;
    *cur = aligned + item.byte_size();
    pending_items.push((off, item));
    off
}

fn collect_pending_annotation_offsets<'a>(
    annotation: &'a AnnotationRecord,
    cur: &mut u32,
    pending_items: &mut Vec<(u32, PendingValueItem<'a>)>,
    annotation_value_offsets: &mut FxHashMap<usize, u32>,
) {
    for element in &annotation.elements {
        collect_pending_annotation_value_offsets(
            &element.value,
            cur,
            pending_items,
            annotation_value_offsets,
        );
    }
}

fn collect_pending_annotation_value_offsets<'a>(
    value: &'a AnnotationValue,
    cur: &mut u32,
    pending_items: &mut Vec<(u32, PendingValueItem<'a>)>,
    annotation_value_offsets: &mut FxHashMap<usize, u32>,
) {
    if let Some(item) = pending_annotation_value_item(value) {
        let off = push_pending_value(cur, pending_items, item);
        annotation_value_offsets.insert(annotation_value_offset_key(value), off);
    }

    match value {
        AnnotationValue::RawScalar(value) => {
            collect_pending_scalar_offsets(value, cur, pending_items, annotation_value_offsets);
        }
        AnnotationValue::Array(value) => {
            collect_pending_array_offsets(value, cur, pending_items, annotation_value_offsets);
        }
        AnnotationValue::Bool(_)
        | AnnotationValue::U32(_)
        | AnnotationValue::F64(_)
        | AnnotationValue::String(_)
        | AnnotationValue::LiteralArray { .. } => {}
    }
}

fn collect_pending_scalar_offsets<'a>(
    value: &'a ScalarValueRecord,
    cur: &mut u32,
    pending_items: &mut Vec<(u32, PendingValueItem<'a>)>,
    annotation_value_offsets: &mut FxHashMap<usize, u32>,
) {
    if let ScalarValueData::Annotation(annotation) = &value.value {
        collect_pending_annotation_offsets(
            annotation,
            cur,
            pending_items,
            annotation_value_offsets,
        );
    }
}

fn collect_pending_array_offsets<'a>(
    value: &'a ArrayValueRecord,
    cur: &mut u32,
    pending_items: &mut Vec<(u32, PendingValueItem<'a>)>,
    annotation_value_offsets: &mut FxHashMap<usize, u32>,
) {
    for element in &value.values {
        collect_pending_scalar_offsets(element, cur, pending_items, annotation_value_offsets);
    }
}

fn annotation_value_offset_key(value: &AnnotationValue) -> usize {
    value as *const AnnotationValue as usize
}

fn annotation_record_offset_key(value: &AnnotationRecord) -> usize {
    value as *const AnnotationRecord as usize
}

fn field_pending_value_item(value: Option<&FieldValue>) -> Option<PendingValueItem<'_>> {
    match value {
        Some(FieldValue::F64(value)) => Some(PendingValueItem::Double(*value)),
        Some(FieldValue::RawScalar(value)) => ScalarBinaryValue::needs_value_item(value),
        _ => None,
    }
}

fn pending_annotation_value_item(value: &AnnotationValue) -> Option<PendingValueItem<'_>> {
    match value {
        AnnotationValue::F64(value) => Some(PendingValueItem::Double(*value)),
        AnnotationValue::RawScalar(value) => ScalarBinaryValue::needs_value_item(value),
        AnnotationValue::Array(value) => Some(PendingValueItem::Array(value)),
        _ => None,
    }
}

struct EntityOffsetMaps<'a> {
    class_offsets: &'a FxHashMap<&'a str, u32>,
    method_offsets: &'a FxHashMap<String, u32>,
    field_offsets: &'a FxHashMap<String, u32>,
    literal_offsets: &'a FxHashMap<String, u32>,
}

fn insert_annotation_record(
    annotation: &AnnotationRecord,
    name_to_index_class_idx: &FxHashMap<&str, u16>,
    string_items: &StringItemWriter,
    entities: &EntityOffsetMaps<'_>,
    literal_array_offsets: &[u32],
    annotation_value_offsets: &FxHashMap<usize, u32>,
    annotation_offsets_by_key: &mut FxHashMap<AnnotationItemKey, u32>,
    annotation_offsets_by_ptr: &mut FxHashMap<usize, u32>,
    annotation_emit_order: &mut Vec<AnnotationItemKey>,
    ann_cur: &mut u32,
) -> u32 {
    let ptr_key = annotation_record_offset_key(annotation);
    if let Some(offset) = annotation_offsets_by_ptr.get(&ptr_key).copied() {
        return offset;
    }

    for element in &annotation.elements {
        register_nested_annotation_values(
            &element.value,
            name_to_index_class_idx,
            string_items,
            entities,
            literal_array_offsets,
            annotation_value_offsets,
            annotation_offsets_by_key,
            annotation_offsets_by_ptr,
            annotation_emit_order,
            ann_cur,
        );
    }

    let key = annotation_item_key(
        annotation,
        name_to_index_class_idx,
        string_items,
        entities,
        literal_array_offsets,
        annotation_value_offsets,
        annotation_offsets_by_key,
        annotation_offsets_by_ptr,
        annotation_emit_order,
        ann_cur,
    );
    let off = insert_annotation_key(
        annotation_offsets_by_key,
        annotation_emit_order,
        ann_cur,
        key,
    );
    annotation_offsets_by_ptr.insert(ptr_key, off);
    off
}

fn register_nested_annotation_values(
    value: &AnnotationValue,
    name_to_index_class_idx: &FxHashMap<&str, u16>,
    string_items: &StringItemWriter,
    entities: &EntityOffsetMaps<'_>,
    literal_array_offsets: &[u32],
    annotation_value_offsets: &FxHashMap<usize, u32>,
    annotation_offsets_by_key: &mut FxHashMap<AnnotationItemKey, u32>,
    annotation_offsets_by_ptr: &mut FxHashMap<usize, u32>,
    annotation_emit_order: &mut Vec<AnnotationItemKey>,
    ann_cur: &mut u32,
) {
    match value {
        AnnotationValue::RawScalar(value) => register_nested_scalar_annotation(
            value,
            name_to_index_class_idx,
            string_items,
            entities,
            literal_array_offsets,
            annotation_value_offsets,
            annotation_offsets_by_key,
            annotation_offsets_by_ptr,
            annotation_emit_order,
            ann_cur,
        ),
        AnnotationValue::Array(value) => {
            for element in &value.values {
                register_nested_scalar_annotation(
                    element,
                    name_to_index_class_idx,
                    string_items,
                    entities,
                    literal_array_offsets,
                    annotation_value_offsets,
                    annotation_offsets_by_key,
                    annotation_offsets_by_ptr,
                    annotation_emit_order,
                    ann_cur,
                );
            }
        }
        AnnotationValue::Bool(_)
        | AnnotationValue::U32(_)
        | AnnotationValue::F64(_)
        | AnnotationValue::String(_)
        | AnnotationValue::LiteralArray { .. } => {}
    }
}

fn register_nested_scalar_annotation(
    value: &ScalarValueRecord,
    name_to_index_class_idx: &FxHashMap<&str, u16>,
    string_items: &StringItemWriter,
    entities: &EntityOffsetMaps<'_>,
    literal_array_offsets: &[u32],
    annotation_value_offsets: &FxHashMap<usize, u32>,
    annotation_offsets_by_key: &mut FxHashMap<AnnotationItemKey, u32>,
    annotation_offsets_by_ptr: &mut FxHashMap<usize, u32>,
    annotation_emit_order: &mut Vec<AnnotationItemKey>,
    ann_cur: &mut u32,
) {
    if let ScalarValueData::Annotation(annotation) = &value.value {
        insert_annotation_record(
            annotation,
            name_to_index_class_idx,
            string_items,
            entities,
            literal_array_offsets,
            annotation_value_offsets,
            annotation_offsets_by_key,
            annotation_offsets_by_ptr,
            annotation_emit_order,
            ann_cur,
        );
    }
}

fn annotation_item_key(
    annotation: &AnnotationRecord,
    name_to_index_class_idx: &FxHashMap<&str, u16>,
    string_items: &StringItemWriter,
    entities: &EntityOffsetMaps<'_>,
    literal_array_offsets: &[u32],
    annotation_value_offsets: &FxHashMap<usize, u32>,
    annotation_offsets_by_key: &mut FxHashMap<AnnotationItemKey, u32>,
    annotation_offsets_by_ptr: &mut FxHashMap<usize, u32>,
    annotation_emit_order: &mut Vec<AnnotationItemKey>,
    ann_cur: &mut u32,
) -> AnnotationItemKey {
    let annotation_class_name = format!("L{};", annotation.record_name);
    let annotation_class_idx = *name_to_index_class_idx
        .get(annotation_class_name.as_str())
        .unwrap_or_else(|| {
            panic!("custom annotation record {annotation_class_name} must be indexed")
        });
    let elements = annotation
        .elements
        .iter()
        .map(|element| {
            let name_off = string_items
                .id_of(&element.name)
                .expect("annotation element name must be interned");
            let value = annotation_value_binary_u32(
                &element.value,
                annotation_value_offsets
                    .get(&annotation_value_offset_key(&element.value))
                    .copied(),
                name_to_index_class_idx,
                string_items,
                entities,
                literal_array_offsets,
                annotation_value_offsets,
                annotation_offsets_by_key,
                annotation_offsets_by_ptr,
                annotation_emit_order,
                ann_cur,
            );
            AnnotationElementKey {
                name_off,
                value,
                tag: element.value.tag(),
            }
        })
        .collect();
    AnnotationItemKey {
        class_idx: annotation_class_idx,
        elements,
    }
}

fn annotation_value_binary_u32(
    value: &AnnotationValue,
    pending_offset: Option<u32>,
    name_to_index_class_idx: &FxHashMap<&str, u16>,
    string_items: &StringItemWriter,
    entities: &EntityOffsetMaps<'_>,
    literal_array_offsets: &[u32],
    annotation_value_offsets: &FxHashMap<usize, u32>,
    annotation_offsets_by_key: &mut FxHashMap<AnnotationItemKey, u32>,
    annotation_offsets_by_ptr: &mut FxHashMap<usize, u32>,
    annotation_emit_order: &mut Vec<AnnotationItemKey>,
    ann_cur: &mut u32,
) -> u32 {
    match value {
        AnnotationValue::Bool(value) => u32::from(*value),
        AnnotationValue::U32(value) => *value,
        AnnotationValue::F64(_) => pending_offset.expect("annotation f64 value offset must exist"),
        AnnotationValue::String(value) => string_items
            .id_of(value)
            .expect("annotation string value must be interned"),
        AnnotationValue::LiteralArray { index, .. } => literal_array_offsets[*index],
        AnnotationValue::RawScalar(value) => scalar_binary_value_u32(
            value,
            pending_offset,
            name_to_index_class_idx,
            string_items,
            entities,
            literal_array_offsets,
            annotation_value_offsets,
            annotation_offsets_by_key,
            annotation_offsets_by_ptr,
            annotation_emit_order,
            ann_cur,
        )
        .annotation_u32(),
        AnnotationValue::Array(_) => {
            pending_offset.expect("annotation array value offset must exist")
        }
    }
}

fn scalar_binary_value_u32(
    value: &ScalarValueRecord,
    pending_offset: Option<u32>,
    name_to_index_class_idx: &FxHashMap<&str, u16>,
    string_items: &StringItemWriter,
    entities: &EntityOffsetMaps<'_>,
    literal_array_offsets: &[u32],
    annotation_value_offsets: &FxHashMap<usize, u32>,
    annotation_offsets_by_key: &mut FxHashMap<AnnotationItemKey, u32>,
    annotation_offsets_by_ptr: &mut FxHashMap<usize, u32>,
    annotation_emit_order: &mut Vec<AnnotationItemKey>,
    ann_cur: &mut u32,
) -> ScalarBinaryValue {
    if scalar_value_is_32bit_integer(value.value_type) {
        return ScalarBinaryValue::Integer(value.scalar_u64() as u32);
    }
    if value.value_type == VALUE_TYPE_F32 {
        return ScalarBinaryValue::Float(value.scalar_f32().to_bits());
    }
    if matches!(
        value.value_type,
        VALUE_TYPE_I64 | VALUE_TYPE_U64 | VALUE_TYPE_F64
    ) {
        return ScalarBinaryValue::Offset(
            pending_offset.expect("non-32-bit scalar value offset must exist"),
        );
    }
    if scalar_value_is_id(value.value_type) {
        return ScalarBinaryValue::Id(resolve_scalar_id(
            value,
            name_to_index_class_idx,
            string_items,
            entities,
            literal_array_offsets,
            annotation_value_offsets,
            annotation_offsets_by_key,
            annotation_offsets_by_ptr,
            annotation_emit_order,
            ann_cur,
        ));
    }
    panic!(
        "unsupported protoPanda scalar value type {} for ABC binary emission",
        value.value_type
    );
}

fn resolve_scalar_id(
    value: &ScalarValueRecord,
    name_to_index_class_idx: &FxHashMap<&str, u16>,
    string_items: &StringItemWriter,
    entities: &EntityOffsetMaps<'_>,
    literal_array_offsets: &[u32],
    annotation_value_offsets: &FxHashMap<usize, u32>,
    annotation_offsets_by_key: &mut FxHashMap<AnnotationItemKey, u32>,
    annotation_offsets_by_ptr: &mut FxHashMap<usize, u32>,
    annotation_emit_order: &mut Vec<AnnotationItemKey>,
    ann_cur: &mut u32,
) -> u32 {
    match (&value.value, value.value_type) {
        (ScalarValueData::String(value), VALUE_TYPE_STRING) => string_items
            .id_of(value)
            .expect("raw scalar string value must be interned"),
        (ScalarValueData::String(value), VALUE_TYPE_METHOD) => *entities
            .method_offsets
            .get(value)
            .unwrap_or_else(|| panic!("raw scalar method {value} must be indexed")),
        (ScalarValueData::String(value), VALUE_TYPE_ENUM) => *entities
            .field_offsets
            .get(value)
            .unwrap_or_else(|| panic!("raw scalar enum field {value} must be indexed")),
        (ScalarValueData::String(value), VALUE_TYPE_LITERALARRAY) => *entities
            .literal_offsets
            .get(value)
            .unwrap_or_else(|| panic!("raw scalar literal array {value} must be indexed")),
        (ScalarValueData::Type(value), VALUE_TYPE_RECORD) => {
            let descriptor = pandasm_type_binary_descriptor(value);
            *entities
                .class_offsets
                .get(descriptor.as_str())
                .unwrap_or_else(|| panic!("raw scalar record {descriptor} must be indexed"))
        }
        (ScalarValueData::U64(_), VALUE_TYPE_STRING_NULLPTR) => 0,
        (ScalarValueData::Annotation(annotation), VALUE_TYPE_ANNOTATION) => {
            insert_annotation_record(
                annotation,
                name_to_index_class_idx,
                string_items,
                entities,
                literal_array_offsets,
                annotation_value_offsets,
                annotation_offsets_by_key,
                annotation_offsets_by_ptr,
                annotation_emit_order,
                ann_cur,
            )
        }
        (_, VALUE_TYPE_METHOD_HANDLE) => {
            panic!("method handle annotation binary emission is not supported by upstream")
        }
        _ => panic!(
            "unsupported protoPanda scalar id value type {} for ABC binary emission",
            value.value_type
        ),
    }
}

fn scalar_binary_value_u32_existing(
    value: &ScalarValueRecord,
    pending_offset: Option<u32>,
    string_items: &StringItemWriter,
    entities: &EntityOffsetMaps<'_>,
    annotation_offsets_by_ptr: &FxHashMap<usize, u32>,
) -> ScalarBinaryValue {
    if scalar_value_is_32bit_integer(value.value_type) {
        return ScalarBinaryValue::Integer(value.scalar_u64() as u32);
    }
    if value.value_type == VALUE_TYPE_F32 {
        return ScalarBinaryValue::Float(value.scalar_f32().to_bits());
    }
    if matches!(
        value.value_type,
        VALUE_TYPE_I64 | VALUE_TYPE_U64 | VALUE_TYPE_F64
    ) {
        return ScalarBinaryValue::Offset(
            pending_offset.expect("non-32-bit scalar value offset must exist"),
        );
    }
    if scalar_value_is_id(value.value_type) {
        return ScalarBinaryValue::Id(resolve_scalar_id_existing(
            value,
            string_items,
            entities,
            annotation_offsets_by_ptr,
        ));
    }
    panic!(
        "unsupported protoPanda scalar value type {} for ABC binary emission",
        value.value_type
    );
}

fn resolve_scalar_id_existing(
    value: &ScalarValueRecord,
    string_items: &StringItemWriter,
    entities: &EntityOffsetMaps<'_>,
    annotation_offsets_by_ptr: &FxHashMap<usize, u32>,
) -> u32 {
    match (&value.value, value.value_type) {
        (ScalarValueData::String(value), VALUE_TYPE_STRING) => string_items
            .id_of(value)
            .expect("raw scalar string value must be interned"),
        (ScalarValueData::String(value), VALUE_TYPE_METHOD) => *entities
            .method_offsets
            .get(value)
            .unwrap_or_else(|| panic!("raw scalar method {value} must be indexed")),
        (ScalarValueData::String(value), VALUE_TYPE_ENUM) => *entities
            .field_offsets
            .get(value)
            .unwrap_or_else(|| panic!("raw scalar enum field {value} must be indexed")),
        (ScalarValueData::String(value), VALUE_TYPE_LITERALARRAY) => *entities
            .literal_offsets
            .get(value)
            .unwrap_or_else(|| panic!("raw scalar literal array {value} must be indexed")),
        (ScalarValueData::Type(value), VALUE_TYPE_RECORD) => {
            let descriptor = pandasm_type_binary_descriptor(value);
            *entities
                .class_offsets
                .get(descriptor.as_str())
                .unwrap_or_else(|| panic!("raw scalar record {descriptor} must be indexed"))
        }
        (ScalarValueData::U64(_), VALUE_TYPE_STRING_NULLPTR) => 0,
        (ScalarValueData::Annotation(annotation), VALUE_TYPE_ANNOTATION) => {
            annotation_offsets_by_ptr
                .get(&annotation_record_offset_key(annotation))
                .copied()
                .expect("annotation scalar must be registered")
        }
        (_, VALUE_TYPE_METHOD_HANDLE) => {
            panic!("method handle annotation binary emission is not supported by upstream")
        }
        _ => panic!(
            "unsupported protoPanda scalar id value type {} for ABC binary emission",
            value.value_type
        ),
    }
}

fn encode_pending_value_section(
    section_start: u32,
    pending_items: &[(u32, PendingValueItem<'_>)],
    string_items: &StringItemWriter,
    entities: &EntityOffsetMaps<'_>,
    annotation_offsets_by_ptr: &FxHashMap<usize, u32>,
) -> Vec<u8> {
    let mut buf = Vec::new();
    for (offset, item) in pending_items {
        while section_start + (buf.len() as u32) < *offset {
            buf.push(0);
        }
        match item {
            PendingValueItem::Long(value) => buf.extend_from_slice(&value.to_le_bytes()),
            PendingValueItem::Double(value) => buf.extend_from_slice(&value.to_le_bytes()),
            PendingValueItem::Array(value) => {
                buf.extend_from_slice(&uleb128(value.values.len() as u64));
                for element in &value.values {
                    encode_array_scalar_element(
                        &mut buf,
                        value.component_type,
                        element,
                        string_items,
                        entities,
                        annotation_offsets_by_ptr,
                    );
                }
            }
        }
    }
    buf
}

fn encode_array_scalar_element(
    buf: &mut Vec<u8>,
    component_type: u32,
    value: &ScalarValueRecord,
    string_items: &StringItemWriter,
    entities: &EntityOffsetMaps<'_>,
    annotation_offsets_by_ptr: &FxHashMap<usize, u32>,
) {
    match component_type {
        VALUE_TYPE_U1 | VALUE_TYPE_I8 | VALUE_TYPE_U8 => {
            buf.push(value.scalar_u64() as u8);
        }
        VALUE_TYPE_I16 | VALUE_TYPE_U16 => {
            buf.extend_from_slice(&(value.scalar_u64() as u16).to_le_bytes());
        }
        VALUE_TYPE_I32 | VALUE_TYPE_U32 => {
            buf.extend_from_slice(&(value.scalar_u64() as u32).to_le_bytes());
        }
        VALUE_TYPE_F32 => {
            buf.extend_from_slice(&value.scalar_f32().to_bits().to_le_bytes());
        }
        VALUE_TYPE_I64 | VALUE_TYPE_U64 => {
            buf.extend_from_slice(&value.scalar_u64().to_le_bytes());
        }
        VALUE_TYPE_F64 => {
            buf.extend_from_slice(&value.scalar_f64().to_le_bytes());
        }
        VALUE_TYPE_STRING
        | VALUE_TYPE_RECORD
        | VALUE_TYPE_METHOD
        | VALUE_TYPE_ENUM
        | VALUE_TYPE_LITERALARRAY
        | VALUE_TYPE_METHOD_HANDLE => {
            let id = resolve_scalar_id_existing(
                value,
                string_items,
                entities,
                annotation_offsets_by_ptr,
            );
            buf.extend_from_slice(&id.to_le_bytes());
        }
        VALUE_TYPE_ANNOTATION => {
            let ScalarValueData::Annotation(annotation) = &value.value else {
                panic!("annotation array element must carry annotation data")
            };
            let id = annotation_offsets_by_ptr
                .get(&annotation_record_offset_key(annotation))
                .copied()
                .expect("annotation array element must be registered");
            buf.extend_from_slice(&id.to_le_bytes());
        }
        VALUE_TYPE_ARRAY => {
            panic!("nested array annotation element is not supported by upstream")
        }
        _ => {
            let scalar = scalar_binary_value_u32_existing(
                value,
                None,
                string_items,
                entities,
                annotation_offsets_by_ptr,
            )
            .annotation_u32();
            buf.extend_from_slice(&scalar.to_le_bytes());
        }
    }
}

fn pandasm_type_binary_descriptor(value: &PandasmTypeRecord) -> String {
    if value.rank > 0 {
        let mut descriptor = "[".repeat(value.rank as usize);
        if let Some(shorty) = primitive_descriptor_for_name(&value.component_name) {
            descriptor.push(shorty);
        } else {
            descriptor.push('L');
            descriptor.push_str(&value.component_name);
            descriptor.push(';');
        }
        return descriptor;
    }
    if value.name.starts_with('L') && value.name.ends_with(';') {
        value.name.clone()
    } else if let Some(shorty) = primitive_descriptor_for_name(&value.name) {
        shorty.to_string()
    } else {
        format!("L{};", value.name)
    }
}

fn primitive_descriptor_for_name(value: &str) -> Option<char> {
    match value {
        "void" => Some('V'),
        "u1" => Some('Z'),
        "i8" => Some('B'),
        "u8" => Some('H'),
        "i16" => Some('S'),
        "u16" => Some('C'),
        "i32" => Some('I'),
        "u32" => Some('U'),
        "f32" => Some('F'),
        "f64" => Some('D'),
        "i64" => Some('J'),
        "u64" => Some('Q'),
        "any" => Some('A'),
        _ => None,
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

impl ClassRecord {
    fn with_upstream_method_order(mut self) -> Self {
        self.methods.sort_by(|a, b| {
            let a_len = a.name.encode_utf16().count();
            let b_len = b.name.encode_utf16().count();
            a_len.cmp(&b_len).then_with(|| a.name.cmp(&b.name))
        });
        self
    }
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
    let use_page_method_pool_supplements =
        class.name.contains("/ets/pages/") && !class.standalone_ecmascript_input;
    if use_page_method_pool_supplements {
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
    if use_page_method_pool_supplements {
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

fn debug_source_file_differs_from_record(class: &ClassRecord, debug: &DebugInfoRecord) -> bool {
    !debug.source_file.is_empty()
        && class.source_file.as_deref() != Some(debug.source_file.as_str())
}

impl Default for AbcFile {
    fn default() -> Self {
        Self::new()
    }
}

impl AbcFile {
    fn build_method_pool_layouts(
        classes: &[&ClassRecord],
        strings: &StringTable,
    ) -> Vec<MethodPoolLayout> {
        let mut next_pool_index = 0u16;
        let mut global_string_pool_indices = FxHashMap::<u16, u16>::default();
        classes
            .iter()
            .map(|class| {
                let mut layout = MethodPoolLayout::default();
                let mut all_bytecode_string_indices = Vec::new();
                for method in &class.methods {
                    collect_bytecode_string_indices(
                        &method.bytecode,
                        &mut all_bytecode_string_indices,
                    );
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

                layout.method_indices = class.method_pool_entry_indices();
                let mut method_pool_indices_by_method_idx = FxHashMap::<usize, u16>::default();
                let mut definefunc_methods = Vec::new();
                for &method_idx in &layout.method_indices {
                    let pool_index = next_pool_index;
                    let method = &class.methods[method_idx];
                    method_pool_indices_by_method_idx.insert(method_idx, pool_index);
                    if matches!(
                        method.kind,
                        MethodKind::Anonymous | MethodKind::FunctionLiteral
                    ) {
                        if let Some(order) = method.definefunc_order {
                            definefunc_methods.push((order, pool_index));
                        }
                    }
                    if let Some(name_id) = strings.id_of(&method.name) {
                        layout
                            .definemethod_pool_indices_by_name
                            .insert(name_id as u16, pool_index);
                    }
                    next_pool_index = next_pool_index.saturating_add(1);
                }
                definefunc_methods.sort_by_key(|(order, _)| *order);
                layout.definefunc_pool_indices = definefunc_methods
                    .into_iter()
                    .map(|(_, pool_index)| pool_index)
                    .collect();
                layout.ctor_pool_indices = class
                    .methods
                    .iter()
                    .enumerate()
                    .filter_map(|(method_idx, method)| {
                        (method.kind == MethodKind::Constructor && method.method_pool_entry)
                            .then(|| method_pool_indices_by_method_idx.get(&method_idx).copied())
                            .flatten()
                    })
                    .collect();

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
                rank[literal_idx] = if is_page && array.has_method_affiliate() {
                    0
                } else if is_page && array.is_page_route_metadata() {
                    4
                } else if is_page && array.is_this_object() {
                    6
                } else if is_page && array.is_page_index_stub() {
                    1
                } else if is_entry && array.has_method_affiliate() {
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

    // Compute Adler-32 checksum for the ABC file checksum field at offset 0x08.
    // Verified against `es2abc` output for API 9 SDK.
    fn adler32(data: &[u8]) -> u32 {
        const MOD: u32 = 65521;
        let (mut a, mut b) = (1u32, 0u32);
        for &byte in data {
            a = (a + byte as u32) % MOD;
            b = (b + a) % MOD;
        }
        (b << 16) | a
    }
}
