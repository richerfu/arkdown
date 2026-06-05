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

use crate::ir::opcodes::Insn;

pub const FIELD_OWNER_CLASS_IDX: u16 = u16::MAX;

/// Method access flags constants for MethodItem `access_flags`.
pub mod method_access {
    pub const INSTANCE: u32 = 0x008;
    pub const MODULE_OR_CTOR: u32 = 0x108;
    pub const ANON: u32 = 0x208;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodKind {
    Module,
    FunctionLiteral,
    Function,
    Constructor,
    Anonymous,
}

/// `panda_file::FunctionKind` values serialized by upstream protoPanda.
pub(crate) mod function_kind {
    pub(crate) const NONE: u32 = 0;
    pub(crate) const FUNCTION: u32 = 1;
    pub(crate) const NC_FUNCTION: u32 = 2;
    pub(crate) const GENERATOR_FUNCTION: u32 = 3;
    pub(crate) const ASYNC_FUNCTION: u32 = 4;
    pub(crate) const ASYNC_GENERATOR_FUNCTION: u32 = 5;
    pub(crate) const ASYNC_NC_FUNCTION: u32 = 6;
    pub(crate) const CONCURRENT_FUNCTION: u32 = 7;
    pub(crate) const SENDABLE_FUNCTION: u32 = 8;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodLabelStyle {
    Source,
    Abc2Program,
}

/// Return the correct `access_flags` value for a method based on its IR role.
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

/// A single try/catch region in a method's exception table.
///
/// Panda `TryBlock` format (serialized after bytecode in `CodeItem`):
/// ```text
/// start_pc: u32 LE   – byte offset of first try instruction
/// length:   u32 LE   – byte count of the try region
/// num_catches: u16 LE – number of catch handlers (always 1 here)
/// CatchBlock:
///   type_idx:   ULEB128 – exception type + 1 (0 = catch-all)
///   handler_pc: ULEB128 – byte offset of the catch handler entry
///   code_size:  ULEB128 – byte length of the catch handler
/// ```
#[derive(Debug, Clone, Default)]
pub struct CatchEntry {
    pub try_begin: u32,
    pub try_end: u32,
    pub catch_begin: u32,
    pub catch_end: u32,
}

/// A pandasm label emitted into upstream ProgramCache snapshots.
///
/// ABC bytecode stores branch targets as byte offsets, but protoPanda stores
/// labels as explicit `LabelIns` entries plus string references from jump and
/// catch records.  This metadata preserves generation-time label names and
/// insertion order without affecting the executable bytecode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramLabel {
    pub name: String,
    pub offset: u32,
    pub file_location: Option<FileLocationRecord>,
}

/// Source file position metadata carried by upstream protoPanda/pandasm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePositionRecord {
    pub line: u64,
    pub column: u64,
}

/// Source range metadata carried by upstream protoPanda/pandasm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLocationRecord {
    pub begin: SourcePositionRecord,
    pub end: SourcePositionRecord,
}

/// Parser file-location metadata used by upstream pandasm diagnostics/IDE helpers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileLocationRecord {
    pub whole_line: String,
    pub bound_left: u64,
    pub bound_right: u64,
    pub line_number: u64,
    pub is_defined: bool,
}

/// Generic upstream pandasm metadata attributes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MetadataRecord {
    pub set_attributes: Vec<String>,
    pub attributes: Vec<MetadataAttributeRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataAttributeRecord {
    pub key: String,
    pub values: Vec<String>,
}

/// Parser source metadata stored directly on upstream pandasm fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldSourceRecord {
    pub line_of_def: u64,
    pub whole_line: String,
    pub bound_left: u64,
    pub bound_right: u64,
    pub is_defined: bool,
}

impl Default for FieldSourceRecord {
    fn default() -> Self {
        Self {
            line_of_def: 0,
            whole_line: String::new(),
            bound_left: 0,
            bound_right: 0,
            is_defined: true,
        }
    }
}

/// Complete upstream pandasm type tuple serialized in protoPanda `Type`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PandasmTypeRecord {
    pub component_name: String,
    pub rank: u64,
    pub name: String,
    pub type_id: u32,
}

impl PandasmTypeRecord {
    pub fn any() -> Self {
        Self {
            component_name: "any".to_string(),
            rank: 0,
            name: "any".to_string(),
            type_id: 0x0e,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScalarValueRecord {
    pub value_type: u32,
    pub value: ScalarValueData,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScalarValueData {
    U64(u64),
    F32(f32),
    F64(f64),
    String(String),
    Type(PandasmTypeRecord),
    Annotation(Box<AnnotationRecord>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArrayValueRecord {
    pub value_type: u32,
    pub component_type: u32,
    pub values: Vec<ScalarValueRecord>,
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
/// [TryBlock * tries_size]
/// ```
///
/// Each `TryBlock` is ULEB128-encoded:
/// `start_pc + length + num_catches + [type_idx + handler_pc + code_size]`.
pub fn encode_code_item(
    num_vregs: u16,
    num_args: u16,
    bytecode: &[u8],
    catch_entries: &[CatchEntry],
) -> Vec<u8> {
    let try_table_len = catch_entries.len() * 8;
    let mut out = Vec::with_capacity(7 + bytecode.len() + try_table_len);
    out.extend_from_slice(&uleb128(num_vregs as u64));
    out.extend_from_slice(&uleb128(num_args as u64));
    out.extend_from_slice(&uleb128(bytecode.len() as u64));
    out.extend_from_slice(&uleb128(catch_entries.len() as u64));
    out.extend_from_slice(bytecode);
    for entry in catch_entries {
        let length = entry.try_end.saturating_sub(entry.try_begin);
        let catch_size = entry.catch_end.saturating_sub(entry.catch_begin);
        out.extend_from_slice(&uleb128(entry.try_begin as u64));
        out.extend_from_slice(&uleb128(length as u64));
        out.extend_from_slice(&uleb128(1));
        out.extend_from_slice(&uleb128(0));
        out.extend_from_slice(&uleb128(entry.catch_begin as u64));
        out.extend_from_slice(&uleb128(catch_size as u64));
    }
    out
}

/// A compiled method (function) within a class record.
///
/// In Panda bytecode, every function — including a module's top-level
/// `func_main_0` — is a method belonging to a class record.
#[derive(Debug, Clone)]
pub struct MethodRecord {
    /// Method name string (e.g. `"func_main_0"` for the module entry point).
    pub name: String,
    /// Encoded bytecode for the method body.
    pub bytecode: Vec<u8>,
    /// Structured pandasm instruction stream used by upstream ProgramCache.
    ///
    /// This must describe the same instruction stream as `bytecode`, after
    /// argument-register physicalization and bytecode optimization. Hand-written
    /// tests may leave it empty; ProgramCache serialization then refuses to
    /// cache that method instead of emitting an invalid protoPanda program.
    pub pandasm_insns: Vec<Insn>,
    /// Per-pandasm-instruction debug position serialized through
    /// protoPanda `Ins.insDebug`.
    pub pandasm_debug: Vec<InstructionDebugInfo>,
    /// Number of virtual registers required by this method.
    pub num_vregs: u16,
    /// Number of formal parameters.
    ///
    /// ArkRuntime always passes **3** arguments when invoking `func_main_0`:
    /// `(JSThread*, JSTaggedValue this, JSTaggedValue[] args)` — set this to 3.
    pub num_args: u16,
    /// protoPanda parser helper for the highest `aN` parameter register seen in
    /// pandasm source. ABC stores `num_vregs`/`num_args`, so this is preserved
    /// only for ProgramCache/protoBin roundtrips.
    pub value_of_first_param: i64,
    /// Upstream `Function::slots_num` / `_ESSlotNumberAnnotation` value.
    /// Source compilation computes this from inline-cache operands; proto/ABC
    /// inputs keep the upstream value here so ProgramCache snapshots do not
    /// lose metadata when an instruction is not modeled by local IR yet.
    pub slots_num: Option<u32>,
    /// Optional method debug metadata. hvigor/es2abc emits this in non-stripped
    /// output so ArkRuntime can link bytecode spans back to source files.
    pub debug_info: Option<DebugInfoRecord>,
    /// Encoded Panda LineNumberProgram opcodes (ADVANCE_PC, ADVANCE_LINE,
    /// SPECIAL_OPCODE, END_SEQUENCE).  Non-empty only when line tracking was
    /// performed during compilation (i.e. when a source text was available).
    /// Written verbatim into the LNP body after the SET_FILE preamble.
    pub line_ops: Vec<u8>,
    /// Exception handler table for this method.  Each entry maps a try region
    /// (by bytecode byte offset) to its catch handler entry point.  Empty for
    /// methods with no `try`/`catch` blocks.
    pub catch_entries: Vec<CatchEntry>,
    /// Ordered pandasm labels for ProgramCache/protoBin materialization.
    pub program_labels: Vec<ProgramLabel>,
    /// Upstream `Function::label_table` entries, preserving parser
    /// file-location metadata independently from executable label positions.
    pub label_table: Vec<ProgramLabel>,
    pub kind: MethodKind,
    /// Raw upstream `panda_file::FunctionKind` value serialized in protoPanda.
    pub function_kind: u32,
    /// Module request indices captured for `"use concurrent"` functions.
    pub concurrent_module_requests: Vec<u32>,
    pub method_pool_entry: bool,
    pub expected_property_count: Option<u32>,
    pub annotations: Vec<AnnotationRecord>,
    /// Upstream generic pandasm metadata attributes.
    pub metadata_attrs: MetadataRecord,
    /// Custom annotations attached to each Panda method parameter.
    ///
    /// The outer index is the proto parameter index used by libpandafile's
    /// MethodItem. Empty vectors mean the parameter has no custom annotations.
    pub param_annotations: Vec<Vec<AnnotationRecord>>,
    /// Source-order key for `definefunc` operands. MethodItems may be sorted for
    /// upstream file layout, but anonymous `definefunc` opcodes are resolved in
    /// AST discovery order.
    pub definefunc_order: Option<u32>,
    /// Label naming style used when materializing protoPanda Program snapshots.
    pub label_style: MethodLabelStyle,
    /// Raw upstream parameter types. Source compilation leaves this empty and
    /// ProgramCache writes `any` for each parameter, matching current behavior.
    pub param_types: Vec<PandasmTypeRecord>,
    /// Raw upstream return type. Source compilation writes `any`.
    pub return_type: Option<PandasmTypeRecord>,
    /// Raw upstream function body-presence flag. Source compilation derives it
    /// from bytecode presence.
    pub body_presence: Option<bool>,
    /// Raw upstream function body source location. Source compilation uses the
    /// upstream default zero location.
    pub body_location: Option<SourceLocationRecord>,
    /// Raw upstream function parser file location.
    pub file_location: Option<FileLocationRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstructionDebugInfo {
    pub line_number: u32,
    pub column_number: u32,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct MethodAnnotationOffsets {
    pub(crate) custom: Vec<u32>,
    pub(crate) expected_property_count: Option<u32>,
    pub(crate) slot_number: u32,
    pub(crate) param_annotations: Option<u32>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ClassAnnotationOffsets {
    pub(crate) custom: Vec<u32>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct FieldAnnotationOffsets {
    pub(crate) custom: Vec<u32>,
}

/// Minimal method debug info linked through a Panda LineNumberProgram.
#[derive(Debug, Clone)]
pub struct DebugInfoRecord {
    /// Initial 1-based source line for the method.
    pub line_start: u32,
    /// Source file path stored in the LNP SET_FILE debug opcode.
    pub source_file: String,
    /// Optional source text stored through the LNP SET_SOURCE_CODE opcode.
    pub source_code: Option<String>,
    /// Formal parameter names, parallel to the method's parameter list.
    /// Empty for methods whose params are unnamed or not tracked.
    pub param_names: Vec<String>,
    /// The function's `this` binding is captured into a lexical environment.
    ///
    /// Upstream `GenScopeVariableInfo()` skips local-variable debug entries for
    /// `LexicalBound()` variables, so such methods must not emit a `this` local.
    pub lexical_this: bool,
    /// Upstream protoPanda local-variable debug records. Their `start` and
    /// `length` fields are instruction indexes, matching pandasm
    /// `Function::local_variable_debug`.
    pub local_variables: Vec<LocalVariableDebugRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalVariableDebugRecord {
    pub name: String,
    pub signature: String,
    pub signature_type: String,
    pub reg: i32,
    pub start: u32,
    pub length: u32,
}

impl LocalVariableDebugRecord {
    pub(crate) fn is_parameter(&self, num_vregs: u16) -> bool {
        self.reg >= i32::from(num_vregs)
    }
}

/// A field (instance or static) in a Panda `ClassItem`.
///
/// Used to emit the 6 module metadata fields that es2abc always writes:
/// `pkgName@`, `isCommonjs`, `hasTopLevelAwait`, `isSharedModule`,
/// `scopeNames`, `moduleRecordIdx`.
#[derive(Debug, Clone)]
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
    /// Structured Panda field type used by ProgramCache and by the ABC
    /// IndexSection type table.
    pub field_type: FieldType,
    /// Upstream `FieldMetadata::field_type`, preserved separately from the
    /// top-level pandasm field type.
    pub metadata_field_type: Option<PandasmTypeRecord>,
    /// Access flags — 1 = ACC_PUBLIC.
    pub access_flags: u32,
    /// Optional constant value emitted in the field's tagged section.
    pub value: Option<FieldValue>,
    /// Custom annotation metadata attached to this field.
    pub annotations: Vec<AnnotationRecord>,
    /// Upstream generic pandasm metadata attributes.
    pub metadata_attrs: MetadataRecord,
    /// Upstream pandasm parser source metadata for ProgramCache/protoBin
    /// roundtrips.
    pub source: FieldSourceRecord,
}

/// Panda field type carried by a [`FieldRecord`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FieldType {
    U1,
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    F32,
    F64,
    I64,
    U64,
    PandaString,
    Array { component_name: String, rank: u32 },
    Any,
}

impl FieldType {
    pub(crate) fn from_legacy_idx(type_idx: u16) -> Self {
        match type_idx {
            0 => Self::U8,
            1 => Self::U32,
            _ => Self::Any,
        }
    }

    pub(crate) fn binary_type_id(&self) -> Option<u32> {
        match self {
            Self::U1 => Some(0),
            Self::I8 => Some(1),
            Self::U8 => Some(2),
            Self::I16 => Some(3),
            Self::U16 => Some(4),
            Self::I32 => Some(5),
            Self::U32 => Some(6),
            Self::F32 => Some(7),
            Self::F64 => Some(8),
            Self::I64 => Some(9),
            Self::U64 => Some(10),
            Self::PandaString | Self::Array { .. } | Self::Any => None,
        }
    }

    pub(crate) fn component_name(&self) -> &str {
        match self {
            Self::U1 => "u1",
            Self::I8 => "i8",
            Self::U8 => "u8",
            Self::I16 => "i16",
            Self::U16 => "u16",
            Self::I32 => "i32",
            Self::U32 => "u32",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::I64 => "i64",
            Self::U64 => "u64",
            Self::PandaString => "panda.String",
            Self::Array { component_name, .. } => component_name,
            Self::Any => "any",
        }
    }

    pub(crate) fn rank(&self) -> u32 {
        match self {
            Self::Array { rank, .. } => *rank,
            _ => 0,
        }
    }

    pub(crate) fn pandasm_name(&self) -> String {
        let mut name = self.component_name().to_string();
        for _ in 0..self.rank() {
            name.push_str("[]");
        }
        name
    }

    pub(crate) fn proto_type_id(&self) -> u32 {
        if self.rank() > 0 {
            return 0x0d;
        }
        match self.component_name() {
            "void" => 0x01,
            "u1" => 0x02,
            "i8" => 0x03,
            "u8" => 0x04,
            "i16" => 0x05,
            "u16" => 0x06,
            "i32" => 0x07,
            "u32" => 0x08,
            "f32" => 0x09,
            "f64" => 0x0a,
            "i64" => 0x0b,
            "u64" => 0x0c,
            "any" => 0x0e,
            _ => 0x0d,
        }
    }

    pub(crate) fn foreign_descriptor(&self) -> Option<String> {
        match self {
            Self::PandaString => Some("Lpanda.String;".to_string()),
            Self::Array {
                component_name,
                rank,
            } if *rank > 0 => {
                let mut descriptor = "[".repeat(*rank as usize);
                if let Some(shorty) = primitive_descriptor(component_name) {
                    descriptor.push(shorty);
                } else {
                    descriptor.push('L');
                    descriptor.push_str(component_name);
                    descriptor.push(';');
                }
                Some(descriptor)
            }
            _ => None,
        }
    }

    pub(crate) fn requires_panda_string_record(&self) -> bool {
        self.component_name() == "panda.String"
    }
}

fn primitive_descriptor(component_name: &str) -> Option<char> {
    match component_name {
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

/// Constant field values used by hvigor module metadata records.
#[derive(Debug, Clone)]
pub enum FieldValue {
    /// `u8` zero (`FieldTag::VALUE = 0x01`, one-byte payload).
    U8(u8),
    /// `u1` boolean (`FieldTag::VALUE = 0x01`, one-byte payload).
    Bool(bool),
    /// `u32` file offset (`FieldTag::VALUE = 0x02`, four-byte payload).
    U32(u32),
    /// `u32` file offset of a literal array, resolved during file layout.
    LiteralArray(usize),
    /// `u32` file offset of a StringItem default value.
    String(String),
    /// `u32` file offset of an inline f64 default value.
    F64(f64),
    /// Upstream protoPanda scalar value not covered by the legacy ABC field
    /// encoder yet. Preserved for ProgramCache/protoBin roundtrips.
    RawScalar(ScalarValueRecord),
}

/// Custom annotation metadata attached to a Panda class, method, or field.
#[derive(Debug, Clone, PartialEq)]
pub struct AnnotationRecord {
    pub record_name: String,
    pub elements: Vec<AnnotationElementRecord>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnnotationElementRecord {
    pub name: String,
    pub value: AnnotationValue,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AnnotationValue {
    Bool(bool),
    U32(u32),
    F64(f64),
    String(String),
    LiteralArray { index: usize, key: String },
    RawScalar(ScalarValueRecord),
    Array(ArrayValueRecord),
}

impl AnnotationValue {
    pub(crate) fn tag(&self) -> u8 {
        match self {
            Self::Bool(_) => b'1',
            Self::U32(_) => b'7',
            Self::F64(_) => b'B',
            Self::String(_) => b'C',
            Self::LiteralArray { .. } => b'#',
            Self::RawScalar(value) => scalar_value_type_tag(value.value_type),
            Self::Array(value) if value.values.is_empty() => b'H',
            Self::Array(value) => array_component_type_tag(value.component_type),
        }
    }
}

pub(crate) fn scalar_value_type_tag(value_type: u32) -> u8 {
    match value_type {
        0 => b'1',
        1 => b'2',
        2 => b'3',
        3 => b'4',
        4 => b'5',
        5 => b'6',
        6 => b'7',
        7 => b'8',
        8 => b'9',
        9 => b'A',
        10 => b'B',
        11 => b'C',
        12 => b'*',
        13 => b'D',
        14 => b'E',
        15 => b'F',
        16 => b'G',
        17 => b'H',
        18 => b'I',
        19 => b'J',
        20 => b'#',
        _ => b'0',
    }
}

pub(crate) fn array_component_type_tag(component_type: u32) -> u8 {
    match component_type {
        0 => b'K',
        1 => b'L',
        2 => b'M',
        3 => b'N',
        4 => b'O',
        5 => b'P',
        6 => b'Q',
        7 => b'R',
        8 => b'S',
        9 => b'T',
        10 => b'U',
        11 => b'V',
        13 => b'W',
        14 => b'X',
        15 => b'Y',
        16 => b'Z',
        19 => b'@',
        _ => b'0',
    }
}

/// A compiled module, represented as a Panda `ClassItem`.
///
/// One `ClassRecord` is produced per bundled JavaScript chunk file.
#[derive(Debug, Clone)]
pub struct ClassRecord {
    /// Class name (the OHM URL or record identifier for this chunk).
    pub name: String,
    /// All methods belonging to this record.
    pub methods: Vec<MethodRecord>,
    /// Global [`StringTable`] indices introduced while compiling this record.
    ///
    /// Panda bytecode string operands index into the file IndexHeader method
    /// pool, not directly into a global string table.  Upstream es2abc groups
    /// method-pool entries by record: this record's strings, method literals,
    /// then literal arrays.  Keeping the per-record string indices lets the
    /// serializer rebuild that pool and rewrite bytecode operands accordingly.
    pub string_indices: Vec<u32>,
    /// Global literal-array indices introduced while compiling this record.
    pub literal_indices: Vec<usize>,
    /// Literal arrays used by module metadata fields (`moduleRecordIdx`,
    /// `scopeNames`).  Upstream emits these literal strings with the owning
    /// program, but they are not entries in that record's method index pool.
    pub module_literal_indices: Vec<usize>,
    /// Function source-code strings created while MakeFunctionItems runs.
    pub source_code_strings: Vec<String>,
    /// Access flags for this class (ULEB128-encoded in the binary).
    ///
    /// Regular JS module classes use `1` (ACC_PUBLIC).
    /// Annotation stub classes (API 13+) use `0x2001` (ACC_PUBLIC | ACC_ANNOTATION).
    pub access_flags: u32,
    /// Class and method source-language byte.
    ///
    /// Matches libpandafile `SourceLang`: ECMASCRIPT=0, JAVASCRIPT=2,
    /// TYPESCRIPT=3, ARKTS=4.
    pub source_lang: u8,
    /// Upstream generic pandasm metadata attributes.
    pub metadata_attrs: MetadataRecord,
    /// Raw upstream `Record::conflict` flag.
    pub conflict: bool,
    /// Raw upstream `Record::params_num` value.
    pub params_num: u64,
    /// Raw upstream `Record::body_presence` flag.
    pub body_presence: bool,
    /// Raw upstream record parser file location.
    pub file_location: Option<FileLocationRecord>,
    /// True when the record comes from a direct es2abc-style `.js`/`.ts`
    /// source-file input rather than hvigor's filesInfo handoff.
    pub standalone_ecmascript_input: bool,
    /// Optional source file path written as ClassTag::SOURCE_FILE.
    pub source_file: Option<String>,
    /// Custom annotation metadata attached to this class/record.
    pub annotations: Vec<AnnotationRecord>,
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
        // ClassTag::SOURCE_LANG + custom ClassTag::ANNOTATION entries +
        // optional ClassTag::SOURCE_FILE + ClassTag::NOTHING.
        let class_tags = self.class_tagged_data_byte_size();
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
            + field_bytes
            + self
                .methods
                .iter()
                .map(MethodRecord::method_item_byte_size)
                .sum::<usize>()
    }

    /// Return absolute offsets for this class's inline FieldItem records.
    pub fn field_item_offsets(&self, class_item_offset: u32) -> Vec<u32> {
        let first_field = class_item_offset + self.field_items_start_offset() as u32;
        let mut offsets = Vec::with_capacity(self.fields.len());
        let mut cur = first_field;
        for field in &self.fields {
            offsets.push(cur);
            cur += (2
                + 2
                + 4
                + uleb128(field.access_flags as u64).len()
                + field.value.as_ref().map_or(0, FieldValue::encoded_len)
                + field.annotations.len() * (1 + 4)
                + 1) as u32;
        }
        offsets
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

    fn field_items_start_offset(&self) -> usize {
        inline_string(&self.name).len()
            + 4
            + uleb128(self.access_flags as u64).len()
            + uleb128(self.fields.len() as u64).len()
            + uleb128(self.methods.len() as u64).len()
            + self.class_tagged_data_byte_size()
    }

    fn method_items_start_offset(&self) -> usize {
        self.field_items_start_offset() + self.field_items_byte_size()
    }

    fn class_tagged_data_byte_size(&self) -> usize {
        // ClassTag::SOURCE_LANG + custom ClassTag::ANNOTATION entries +
        // optional ClassTag::SOURCE_FILE + ClassTag::NOTHING.
        2 + self.annotations.len() * (1 + 4) + self.source_file.as_ref().map_or(0, |_| 1 + 4) + 1
    }

    fn field_items_byte_size(&self) -> usize {
        self.fields
            .iter()
            .map(|field| {
                2 + 2
                    + 4
                    + uleb128(field.access_flags as u64).len()
                    + field.value.as_ref().map_or(0, FieldValue::encoded_len)
                    + field.annotations.len() * (1 + 4)
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
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn serialize_class_item(
        &self,
        class_idx: u16,
        class_annotation_offsets: &ClassAnnotationOffsets,
        field_name_offsets: &[u32],
        source_file_offset: Option<u32>,
        field_annotation_offsets: &[FieldAnnotationOffsets],
        method_name_offsets: &[u32],
        code_offsets: &[u32],
        debug_info_offsets: &[Option<u32>],
        annotation_offsets: &[MethodAnnotationOffsets],
        literal_array_offsets: &[u32],
        field_type_indices: &[u16],
        field_value_offsets: &[Option<u32>],
    ) -> Vec<u8> {
        debug_assert_eq!(
            class_annotation_offsets.custom.len(),
            self.annotations.len()
        );
        debug_assert_eq!(field_name_offsets.len(), self.fields.len());
        debug_assert_eq!(field_annotation_offsets.len(), self.fields.len());
        debug_assert_eq!(field_type_indices.len(), self.fields.len());
        debug_assert_eq!(field_value_offsets.len(), self.fields.len());
        debug_assert_eq!(method_name_offsets.len(), self.methods.len());
        debug_assert_eq!(code_offsets.len(), self.methods.len());
        debug_assert_eq!(debug_info_offsets.len(), self.methods.len());
        debug_assert_eq!(annotation_offsets.len(), self.methods.len());

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
        // ClassTag::ANNOTATION = 0x04.
        for &annotation_off in &class_annotation_offsets.custom {
            buf.push(0x04);
            buf.extend_from_slice(&annotation_off.to_le_bytes());
        }
        // ClassTag::SOURCE_FILE = 0x07.
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

        for (field_idx, (field, &name_off)) in self
            .fields
            .iter()
            .zip(field_name_offsets.iter())
            .enumerate()
        {
            // class_idx/type slot: u16 LE
            let field_class_idx = if field.class_idx == FIELD_OWNER_CLASS_IDX {
                class_idx
            } else {
                field.class_idx
            };
            buf.extend_from_slice(&field_class_idx.to_le_bytes());
            // type_idx: u16 LE
            buf.extend_from_slice(&field_type_indices[field_idx].to_le_bytes());
            // name_off: u32 LE (file offset to field's StringItem)
            buf.extend_from_slice(&name_off.to_le_bytes());
            // access_flags: ULEB128
            buf.extend_from_slice(&uleb128(field.access_flags as u64));
            if let Some(value) = &field.value {
                value.encode_into(
                    &mut buf,
                    literal_array_offsets,
                    field_value_offsets[field_idx],
                );
            }
            // FieldTag::ANNOTATION = 0x04.
            for &annotation_off in &field_annotation_offsets[field_idx].custom {
                buf.push(0x04);
                buf.extend_from_slice(&annotation_off.to_le_bytes());
            }
            // FieldTag::NOTHING = 0x00
            buf.push(0x00);
        }

        // ── MethodItem entries ────────────────────────────────────────────────

        for (i, method) in self.methods.iter().enumerate() {
            let name_off = method_name_offsets[i];
            let code_off = code_offsets[i];
            let ann_offsets = &annotation_offsets[i];

            // class_idx: u16 LE
            buf.extend_from_slice(&class_idx.to_le_bytes());
            // proto_idx: u16 LE = 0xffff.
            buf.extend_from_slice(&0xFFFFu16.to_le_bytes());
            // name_off: u32 LE (file offset to method's StringItem)
            buf.extend_from_slice(&name_off.to_le_bytes());
            // access_flags: ULEB128 — per-method type (INSTANCE=0x008, MODULE_OR_CTOR=0x108, ANON=0x208)
            buf.extend_from_slice(&uleb128(method.access_flags() as u64));
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
            for &custom_off in &ann_offsets.custom {
                buf.push(0x06);
                buf.extend_from_slice(&custom_off.to_le_bytes());
            }
            if let Some(expected_off) = ann_offsets.expected_property_count {
                buf.push(0x06);
                buf.extend_from_slice(&expected_off.to_le_bytes());
            }
            // MethodTag::ANNOTATION = 0x06 + annotation offset. Every method
            // gets SlotNumber, and ExpectedPropertyCount (when non-zero) is
            // emitted before it, matching FunctionEmitter::Generate().
            buf.push(0x06);
            buf.extend_from_slice(&ann_offsets.slot_number.to_le_bytes());
            if let Some(param_annotations_off) = ann_offsets.param_annotations {
                buf.push(0x07);
                buf.extend_from_slice(&param_annotations_off.to_le_bytes());
            }
            // MethodTag::NOTHING = 0x00
            buf.push(0x00);
        }

        buf
    }
    /// Indices of methods in `self.methods` that have `method_pool_entry` set,
    /// sorted by method name.  Used when building `method_index` items in the
    /// ABC file format.
    pub(crate) fn method_pool_entry_indices(&self) -> Vec<usize> {
        let mut indices: Vec<usize> = self
            .methods
            .iter()
            .enumerate()
            .filter_map(|(idx, method)| method.method_pool_entry.then_some(idx))
            .collect();
        indices.sort_by(|&a, &b| self.methods[a].name.cmp(&self.methods[b].name));
        indices
    }
}

impl MethodRecord {
    /// Compute the Panda access flags for this method.
    ///
    /// Matches upstream es2panda's access flag assignment:
    /// - `MODULE` / `CONSTRUCTOR` → `MODULE_OR_CTOR` (0x108)
    /// - `ANONYMOUS` → `ANON` (0x208)
    /// - `INSTANCE` / other → `INSTANCE` (0x008)
    pub(crate) fn access_flags(&self) -> u32 {
        match self.kind {
            MethodKind::Module => method_access::MODULE_OR_CTOR,
            MethodKind::Constructor
                if self.is_generated_default_base_constructor()
                    || self.is_generated_default_derived_constructor() =>
            {
                method_access::MODULE_OR_CTOR
            }
            MethodKind::FunctionLiteral => method_access::MODULE_OR_CTOR,
            MethodKind::Anonymous => method_access::ANON,
            MethodKind::Function | MethodKind::Constructor => method_access::INSTANCE,
        }
    }

    fn is_generated_default_derived_constructor(&self) -> bool {
        // GENERATED_CONSTRUCTOR follows upstream es2panda's default derived class
        // path: FunctionEnter, CopyRestArgs, SuperCallSpread, implicit this checks.
        self.bytecode.starts_with(&[
            0x44, 0x60, // mov v0, v6
            0x44, 0x71, // mov v1, v7
            0x44, 0x82, // mov v2, v8
            0x44, 0x93, // mov v3, v9
            0xcf, 0x00, // copyrestargs 0
        ])
    }

    fn is_generated_default_base_constructor(&self) -> bool {
        // Direct es2abc standalone inputs synthesize a default base constructor
        // as `lda this; return` and mark it with constructor/module flags.
        self.num_args == 3
            && (self.bytecode == [0x60, 0x02, 0x64]
                || self.bytecode == [0x3c, 0x00, 0xfb, 0x06, 0x00, 0x02, 0x60, 0x02, 0x64])
    }

    pub(crate) fn method_item_byte_size(&self) -> usize {
        // class_idx(2) + proto_idx(2) + name_off(4) + access_flags(variable)
        // + MethodTag::CODE(1) + code_off(4) + MethodTag::SOURCE_LANG(1) + lang(1)
        // + MethodTag::ANNOTATION(1) + ann_off(4)  ← always emitted
        // + MethodTag::DEBUG_INFO(1) + debug_off(4) ← only when debug_info present
        // + MethodTag::NOTHING(1)
        let access_flags_len = uleb128(self.access_flags() as u64).len();
        16 + access_flags_len
            + (1 + 4) // MethodTag::ANNOTATION always emitted
            + self.annotations.len() * (1 + 4)
            + self.expected_property_count.map_or(0, |_| 1 + 4)
            + self.has_param_annotations().then_some(1 + 4).unwrap_or(0)
            + self.debug_info.as_ref().map_or(0, |_| 1 + 4)
    }

    pub(crate) fn has_param_annotations(&self) -> bool {
        self.param_annotations
            .iter()
            .any(|annotations| !annotations.is_empty())
    }
}

impl FieldValue {
    pub(crate) fn encoded_len(&self) -> usize {
        match self {
            FieldValue::U8(_) | FieldValue::Bool(_) => 2,
            FieldValue::U32(_)
            | FieldValue::LiteralArray(_)
            | FieldValue::String(_)
            | FieldValue::F64(_) => 5,
            FieldValue::RawScalar(value) if scalar_field_value_is_integer(value.value_type) => {
                1 + sleb128_i32(value.scalar_u32_bits() as i32).len()
            }
            FieldValue::RawScalar(_) => 5,
        }
    }

    fn encode_into(
        &self,
        buf: &mut Vec<u8>,
        literal_array_offsets: &[u32],
        field_value_offset: Option<u32>,
    ) {
        match self {
            FieldValue::U8(value) => {
                buf.push(0x01);
                buf.push(*value);
            }
            FieldValue::Bool(value) => {
                buf.push(0x01);
                buf.push(u8::from(*value));
            }
            FieldValue::U32(value) => {
                buf.push(0x02);
                buf.extend_from_slice(&value.to_le_bytes());
            }
            FieldValue::LiteralArray(index) => {
                let value = literal_array_offsets
                    .get(*index)
                    .copied()
                    .expect("field literal array index must be resolved");
                buf.push(0x02);
                buf.extend_from_slice(&value.to_le_bytes());
            }
            FieldValue::String(_) | FieldValue::F64(_) => {
                let value = field_value_offset.expect("field value offset must be resolved");
                buf.push(0x02);
                buf.extend_from_slice(&value.to_le_bytes());
            }
            FieldValue::RawScalar(value) if scalar_field_value_is_integer(value.value_type) => {
                buf.push(0x01);
                buf.extend_from_slice(&sleb128_i32(value.scalar_u32_bits() as i32));
            }
            FieldValue::RawScalar(value) if value.value_type == VALUE_TYPE_F32 => {
                buf.push(0x02);
                buf.extend_from_slice(&value.scalar_f32_bits().to_le_bytes());
            }
            FieldValue::RawScalar(_) => {
                let value = field_value_offset.expect("raw field value offset must be resolved");
                buf.push(0x02);
                buf.extend_from_slice(&value.to_le_bytes());
            }
        }
    }
}

pub(crate) const VALUE_TYPE_U1: u32 = 0;
pub(crate) const VALUE_TYPE_I8: u32 = 1;
pub(crate) const VALUE_TYPE_U8: u32 = 2;
pub(crate) const VALUE_TYPE_I16: u32 = 3;
pub(crate) const VALUE_TYPE_U16: u32 = 4;
pub(crate) const VALUE_TYPE_I32: u32 = 5;
pub(crate) const VALUE_TYPE_U32: u32 = 6;
pub(crate) const VALUE_TYPE_I64: u32 = 7;
pub(crate) const VALUE_TYPE_U64: u32 = 8;
pub(crate) const VALUE_TYPE_F32: u32 = 9;
pub(crate) const VALUE_TYPE_F64: u32 = 10;
pub(crate) const VALUE_TYPE_STRING: u32 = 11;
pub(crate) const VALUE_TYPE_STRING_NULLPTR: u32 = 12;
pub(crate) const VALUE_TYPE_RECORD: u32 = 13;
pub(crate) const VALUE_TYPE_METHOD: u32 = 14;
pub(crate) const VALUE_TYPE_ENUM: u32 = 15;
pub(crate) const VALUE_TYPE_ANNOTATION: u32 = 16;
pub(crate) const VALUE_TYPE_ARRAY: u32 = 17;
pub(crate) const VALUE_TYPE_METHOD_HANDLE: u32 = 19;
pub(crate) const VALUE_TYPE_LITERALARRAY: u32 = 20;

fn scalar_field_value_is_integer(value_type: u32) -> bool {
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

impl ScalarValueRecord {
    pub(crate) fn scalar_u32_bits(&self) -> u32 {
        match &self.value {
            ScalarValueData::U64(value) => *value as u32,
            ScalarValueData::F32(value) => value.to_bits(),
            _ => 0,
        }
    }

    pub(crate) fn scalar_f32_bits(&self) -> u32 {
        match &self.value {
            ScalarValueData::F32(value) => value.to_bits(),
            ScalarValueData::U64(value) => *value as u32,
            _ => 0,
        }
    }
}

fn sleb128_i32(mut value: i32) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let byte = (value as u8) & 0x7f;
        value >>= 7;
        let done = (value == 0 && (byte & 0x40) == 0) || (value == -1 && (byte & 0x40) != 0);
        out.push(if done { byte } else { byte | 0x80 });
        if done {
            break;
        }
    }
    out
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
        let ci = encode_code_item(0, 0, &[], &[]);
        assert_eq!(ci, vec![0x00, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn test_encode_code_item_with_bytecode() {
        // num_vregs=0, num_args=3, bytecode=[0x65] (ReturnUndefined)
        let ci = encode_code_item(0, 3, &[0x65], &[]);
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
                pandasm_insns: Vec::new(),
                pandasm_debug: Vec::new(),
                num_vregs: 0,
                num_args: 3,
                value_of_first_param: -1,
                slots_num: None,
                debug_info: None,
                line_ops: vec![],
                catch_entries: vec![],
                program_labels: vec![],
                label_table: vec![],
                kind: MethodKind::Module,
                function_kind: function_kind::FUNCTION,
                concurrent_module_requests: Vec::new(),
                method_pool_entry: false,
                expected_property_count: None,
                annotations: Vec::new(),
                metadata_attrs: Default::default(),
                param_annotations: Vec::new(),
                definefunc_order: None,
                label_style: MethodLabelStyle::Source,
                param_types: Vec::new(),
                return_type: None,
                body_presence: None,
                body_location: None,
                file_location: None,
            }],
            string_indices: vec![],
            literal_indices: vec![],
            module_literal_indices: vec![],
            source_code_strings: vec![],
            access_flags: 1,
            source_lang: 0,
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
        };
        // inline_string("L_GLOBAL;") = 11 bytes
        // super_off = 4
        // access_flags ULEB128(1) = 1
        // fields_count ULEB128(0) = 1
        // methods_count ULEB128(1) = 1
        // class tags = 3
        // method item: 16 (fixed) + 2 (access_flags 0x108 ULEB128) + 5 (annotation tag) = 23
        // total = 11 + 4 + 1 + 1 + 1 + 3 + 23 = 44
        assert_eq!(record.class_item_byte_size(), 44);
    }

    #[test]
    fn test_serialize_class_item_structure() {
        let record = ClassRecord {
            name: "L_GLOBAL;".to_string(),
            methods: vec![MethodRecord {
                name: "func_main_0".to_string(),
                bytecode: vec![0x65],
                pandasm_insns: Vec::new(),
                pandasm_debug: Vec::new(),
                num_vregs: 0,
                num_args: 3,
                value_of_first_param: -1,
                slots_num: None,
                debug_info: None,
                line_ops: vec![],
                catch_entries: vec![],
                program_labels: vec![],
                label_table: vec![],
                kind: MethodKind::Module,
                function_kind: function_kind::FUNCTION,
                concurrent_module_requests: Vec::new(),
                method_pool_entry: false,
                expected_property_count: None,
                annotations: Vec::new(),
                metadata_attrs: Default::default(),
                param_annotations: Vec::new(),
                definefunc_order: None,
                label_style: MethodLabelStyle::Source,
                param_types: Vec::new(),
                return_type: None,
                body_presence: None,
                body_location: None,
                file_location: None,
            }],
            string_indices: vec![],
            literal_indices: vec![],
            module_literal_indices: vec![],
            source_code_strings: vec![],
            access_flags: 1,
            source_lang: 0,
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
        };
        // annotation_offset = 0x200 (dummy annotation offset for test)
        let bytes = record.serialize_class_item(
            4,
            &ClassAnnotationOffsets::default(),
            &[],
            None,
            &[],
            &[0x12a],
            &[0x1a1],
            &[None],
            &[MethodAnnotationOffsets {
                custom: Vec::new(),
                expected_property_count: None,
                slot_number: 0x200,
                param_annotations: None,
            }],
            &[],
            &[],
            &[],
        );
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
        // MethodTag::ANNOTATION = 0x06, ann_off = 0x200 LE
        assert_eq!(bytes[38], 0x06, "MethodTag::ANNOTATION");
        assert_eq!(&bytes[39..43], &[0x00, 0x02, 0x00, 0x00], "ann_off=0x200");
        // MethodTag::NOTHING = 0x00
        assert_eq!(bytes[43], 0x00, "MethodTag::NOTHING");
    }
}
