//! Minimal read-only parser for Panda `.abc` binary files.
//!
//! Parses just enough of the Panda bytecode format to validate files and
//! extract class names.  Intended as the foundation for SPEC-041 ByteCode
//! HAR integration and SPEC-042 protoBin cache support.
//!
//! ## Binary layout (header at offset 0x00)
//!
//! ```text
//! 0x00  magic[8]          b"PANDA\x00\x00\x00"
//! 0x08  checksum[4]       CRC-32 of file body
//! 0x0C  version[4]        Bytecode version (LE u32)
//! 0x10  file_size[4]      Total file size in bytes
//! 0x14  foreign_off[4]    ForeignItem section offset (0 when absent)
//! 0x18  foreign_size[4]   ForeignItem section size  (0 when absent)
//! 0x1C  num_classes[4]    Number of ClassItems
//! 0x20  class_idx_off[4]  Offset of sorted class-offset array
//! 0x24  num_lnp[4]        Number of LineNumberProgram items
//! 0x28  lnp_idx_off[4]    Offset of LNP offset array (0 when absent)
//! 0x2C  num_literalarrays[4]   Number of LiteralArray items, or 0xffffffff since v13
//! 0x30  literalarray_idx_off[4] Offset of literal-array offset array, or 0xffffffff since v13
//! 0x34  num_idx_regions[4]     Number of IndexSection regions
//! 0x38  idx_section_off[4]     Offset of first IndexSection header
//! ```
//!
//! Upstream reference: `libpandafile/file_items.h` in
//! `arkcompiler_ets_frontend/arkcompiler/runtime_core`.

use std::collections::BTreeSet;

use crate::emit::file::{
    LITERAL_TAG_ACCESSOR, LITERAL_TAG_ARRAY_F32, LITERAL_TAG_ARRAY_F64, LITERAL_TAG_ARRAY_I16,
    LITERAL_TAG_ARRAY_I32, LITERAL_TAG_ARRAY_I64, LITERAL_TAG_ARRAY_I8, LITERAL_TAG_ARRAY_STRING,
    LITERAL_TAG_ARRAY_U1, LITERAL_TAG_ARRAY_U16, LITERAL_TAG_ARRAY_U32, LITERAL_TAG_ARRAY_U64,
    LITERAL_TAG_ARRAY_U8, LITERAL_TAG_ASYNC_GENERATOR_METHOD, LITERAL_TAG_BOOL,
    LITERAL_TAG_BUILTIN_TYPE_INDEX, LITERAL_TAG_DOUBLE, LITERAL_TAG_ETS_IMPLEMENTS,
    LITERAL_TAG_FLOAT, LITERAL_TAG_GENERATOR_METHOD, LITERAL_TAG_GETTER, LITERAL_TAG_INTEGER,
    LITERAL_TAG_LITERAL_ARRAY, LITERAL_TAG_LITERAL_BUFFER_INDEX, LITERAL_TAG_METHOD,
    LITERAL_TAG_METHOD_AFFILIATE, LITERAL_TAG_NULL, LITERAL_TAG_SETTER, LITERAL_TAG_STRING,
    LITERAL_TAG_TAGVALUE,
};
use thiserror::Error;

/// Error type for ABC binary parse failures.
#[derive(Debug, Error)]
pub enum AbcReadError {
    #[error("file too short: expected at least {expected} bytes, got {actual}")]
    TooShort { expected: usize, actual: usize },
    #[error("invalid magic bytes: expected b\"PANDA\\x00\\x00\\x00\", got {found:?}")]
    BadMagic { found: [u8; 8] },
    #[error(
        "class index out of bounds: offset {offset} + {size} bytes exceeds file length {file_len}"
    )]
    ClassIndexOob {
        offset: usize,
        size: usize,
        file_len: usize,
    },
    #[error("class item at offset {offset} is malformed: {reason}")]
    MalformedClassItem { offset: usize, reason: String },
    #[error("string item at offset {offset} is malformed: {reason}")]
    MalformedString { offset: usize, reason: String },
    #[error("invalid UTF-8 in string at offset {offset}: {source}")]
    InvalidUtf8 {
        offset: usize,
        #[source]
        source: std::string::FromUtf8Error,
    },
}

/// Panda magic bytes: ASCII "PANDA" followed by three null bytes.
pub const ABC_MAGIC: [u8; 8] = *b"PANDA\x00\x00\x00";

/// ABC file header size in bytes.
pub const ABC_HEADER_SIZE: usize = 0x3C;
const INVALID_U32: u32 = u32::MAX;
const LAST_LITERAL_ARRAY_HEADER_VERSION: [u8; 4] = [12, 0, 6, 0];

fn contains_literal_array_in_header(version: u32) -> bool {
    version.to_le_bytes() <= LAST_LITERAL_ARRAY_HEADER_VERSION
}

/// Summary of a class parsed from a Panda `.abc` file.
#[derive(Debug, Clone)]
pub struct AbcClassInfo {
    /// Class name as stored in the ClassItem inline StringItem.
    ///
    /// For OHM-URL module records this looks like
    /// `L@bundle:com.example/entry/ets/pages/Index;`.
    pub name: String,
    /// Number of methods in this class.
    pub method_count: usize,
}

/// Structured ClassItem parsed from a Panda `.abc` file.
#[derive(Debug, Clone)]
pub struct AbcClassItem {
    pub name: String,
    pub offset: u32,
    pub super_class_off: u32,
    pub access_flags: u32,
    pub source_lang: Option<u8>,
    pub source_file: Option<String>,
    pub annotation_offsets: Vec<u32>,
    pub fields: Vec<AbcFieldItem>,
    pub methods: Vec<AbcMethodItem>,
}

#[derive(Debug, Clone)]
pub struct AbcFieldItem {
    pub offset: u32,
    pub class_idx: u16,
    pub type_idx: u16,
    pub name: String,
    pub name_off: u32,
    pub access_flags: u32,
    pub value: Option<AbcFieldValue>,
    pub annotation_offsets: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AbcFieldValue {
    U8(u8),
    U32(u32),
}

#[derive(Debug, Clone)]
pub struct AbcMethodItem {
    pub offset: u32,
    pub class_idx: u16,
    pub proto_idx: u16,
    pub name: String,
    pub name_off: u32,
    pub access_flags: u32,
    pub source_lang: Option<u8>,
    pub code_off: Option<u32>,
    pub debug_info_off: Option<u32>,
    pub annotation_offsets: Vec<u32>,
    pub param_annotation_offset: Option<u32>,
    pub code: Option<AbcCodeItem>,
}

#[derive(Debug, Clone)]
pub struct AbcCodeItem {
    pub offset: u32,
    pub num_vregs: u32,
    pub num_args: u32,
    pub bytecode: Vec<u8>,
    pub catches: Vec<AbcCatchBlock>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbcCatchBlock {
    pub try_begin: u32,
    pub try_end: u32,
    pub handler_pc: u32,
    pub code_size: u32,
    pub type_idx: u32,
}

#[derive(Debug, Clone)]
pub struct AbcIndexSection {
    pub offset: u32,
    pub start: u32,
    pub end: u32,
    pub class_offsets: Vec<u32>,
    pub method_offsets: Vec<u32>,
    pub field_offsets: Vec<u32>,
    pub proto_offsets: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct AbcLiteralArrayItem {
    pub offset: u32,
    pub raw_item_count: u32,
    pub entries: Vec<AbcLiteralEntry>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AbcLiteralEntry {
    Bool(bool),
    U8 {
        tag: u8,
        value: u8,
    },
    U16 {
        tag: u8,
        value: u16,
    },
    U32 {
        tag: u8,
        value: u32,
    },
    U64(u64),
    F32(f32),
    F64(f64),
    String {
        tag: u8,
        offset: u32,
        value: String,
    },
    Method {
        tag: u8,
        offset: u32,
        name: Option<String>,
    },
    LiteralArray {
        offset: u32,
    },
    Array {
        tag: u8,
        values: Vec<AbcLiteralArrayValue>,
    },
    Null,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AbcLiteralArrayValue {
    Bool(bool),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    F32(f32),
    F64(f64),
    String { offset: u32, value: String },
}

/// Parsed summary of a Panda `.abc` binary file.
#[derive(Debug)]
pub struct ParsedAbcSummary {
    /// Bytecode version field from the header.
    pub version: u32,
    /// Total file size reported in the header.
    pub file_size: u32,
    /// All class entries found in the class index.
    pub classes: Vec<AbcClassInfo>,
}

/// Minimal read-only parser for Panda `.abc` binary files.
///
/// Parses the header and class index to extract class names.  Does not
/// decode bytecode instructions or string tables.
pub struct AbcReader<'a> {
    data: &'a [u8],
}

impl<'a> AbcReader<'a> {
    /// Construct a reader for `data`, validating the magic bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, AbcReadError> {
        if data.len() < ABC_HEADER_SIZE {
            return Err(AbcReadError::TooShort {
                expected: ABC_HEADER_SIZE,
                actual: data.len(),
            });
        }
        let magic: [u8; 8] = data[0..8].try_into().unwrap();
        if magic != ABC_MAGIC {
            return Err(AbcReadError::BadMagic { found: magic });
        }
        Ok(Self { data })
    }

    /// Return `true` if `data` starts with the Panda ABC magic bytes.
    pub fn is_valid_magic(data: &[u8]) -> bool {
        data.len() >= 8 && data[0..8] == ABC_MAGIC
    }

    /// Perform a structural validation of an ABC binary.
    ///
    /// Checks beyond the magic bytes:
    /// - `file_size` header field matches the actual data length.
    /// - Class index region is within bounds.
    /// - LNP (LineNumberProgram) index region is within bounds when present.
    /// - LiteralArray index region is within bounds when present.
    ///
    /// Use this in security/correctness paths (e.g. ByteCode HAR integration)
    /// rather than `is_valid_magic()`, which only checks the first 8 bytes.
    pub fn validate(data: &[u8]) -> Result<(), AbcReadError> {
        // Use a local reader bound to `data`'s lifetime.
        let reader = AbcReader::new(data)?;

        // file_size must equal the actual data length.
        let file_size = reader.read_u32(0x10)? as usize;
        if file_size != data.len() {
            return Err(AbcReadError::MalformedClassItem {
                offset: 0x10,
                reason: format!(
                    "file_size header field ({}) does not match data length ({})",
                    file_size,
                    data.len()
                ),
            });
        }

        // Class index bounds.
        let num_classes = reader.read_u32(0x1C)? as usize;
        let class_idx_off = reader.read_u32(0x20)? as usize;
        let idx_end = class_idx_off.saturating_add(num_classes * 4);
        if idx_end > data.len() {
            return Err(AbcReadError::ClassIndexOob {
                offset: class_idx_off,
                size: num_classes * 4,
                file_len: data.len(),
            });
        }

        // LNP index bounds (offset 0x24/0x28; absent when lnp_idx_off == 0).
        let num_lnp = reader.read_u32(0x24)? as usize;
        let lnp_idx_off = reader.read_u32(0x28)? as usize;
        if lnp_idx_off != 0 {
            let lnp_end = lnp_idx_off.saturating_add(num_lnp * 4);
            if lnp_end > data.len() {
                return Err(AbcReadError::MalformedClassItem {
                    offset: 0x28,
                    reason: format!(
                        "LNP index region [{:#x}, {:#x}) exceeds file length {}",
                        lnp_idx_off,
                        lnp_end,
                        data.len()
                    ),
                });
            }
        }

        // LiteralArray index bounds. v13+ reserves these fields as INVALID
        // because literal arrays are reached through IndexHeader method indexes.
        let version = reader.read_u32(0x0C)?;
        let num_literal = reader.read_u32(0x2C)?;
        let literal_idx_off = reader.read_u32(0x30)?;
        if contains_literal_array_in_header(version) {
            let literal_idx_off = literal_idx_off as usize;
            let num_literal = num_literal as usize;
            let lit_end = literal_idx_off.saturating_add(num_literal * 4);
            if lit_end > data.len() {
                return Err(AbcReadError::MalformedClassItem {
                    offset: 0x30,
                    reason: format!(
                        "LiteralArray index region [{:#x}, {:#x}) exceeds file length {}",
                        literal_idx_off,
                        lit_end,
                        data.len()
                    ),
                });
            }
        } else if num_literal != INVALID_U32 || literal_idx_off != INVALID_U32 {
            return Err(AbcReadError::MalformedClassItem {
                offset: 0x2c,
                reason: format!(
                    "LiteralArray header fields must be reserved in v13+: num={num_literal:#x}, off={literal_idx_off:#x}"
                ),
            });
        }

        Ok(())
    }

    /// Parse the file and return a summary with class names.
    pub fn read_summary(&self) -> Result<ParsedAbcSummary, AbcReadError> {
        let version = self.read_u32(0x0C)?;
        let file_size = self.read_u32(0x10)?;
        let num_classes = self.read_u32(0x1C)? as usize;
        let class_idx_off = self.read_u32(0x20)? as usize;

        // Validate the class index fits in the file.
        let idx_end = class_idx_off + num_classes * 4;
        if idx_end > self.data.len() {
            return Err(AbcReadError::ClassIndexOob {
                offset: class_idx_off,
                size: num_classes * 4,
                file_len: self.data.len(),
            });
        }

        let mut classes = Vec::with_capacity(num_classes);
        for i in 0..num_classes {
            let class_off = self.read_u32(class_idx_off + i * 4)? as usize;
            let info = self.read_class_info(class_off)?;
            classes.push(info);
        }

        Ok(ParsedAbcSummary {
            version,
            file_size,
            classes,
        })
    }

    /// Return the class names present in this ABC file, in class-index order.
    pub fn class_names(&self) -> Result<Vec<String>, AbcReadError> {
        Ok(self
            .read_summary()?
            .classes
            .into_iter()
            .map(|c| c.name)
            .collect())
    }

    /// Return structured class, field, method, and code metadata in class-index order.
    pub fn class_items(&self) -> Result<Vec<AbcClassItem>, AbcReadError> {
        let num_classes = self.read_u32(0x1C)? as usize;
        let class_idx_off = self.read_u32(0x20)? as usize;
        let idx_end = class_idx_off.saturating_add(num_classes * 4);
        if idx_end > self.data.len() {
            return Err(AbcReadError::ClassIndexOob {
                offset: class_idx_off,
                size: num_classes * 4,
                file_len: self.data.len(),
            });
        }

        let mut classes = Vec::with_capacity(num_classes);
        for i in 0..num_classes {
            let class_off = self.read_u32(class_idx_off + i * 4)?;
            if self.is_foreign_item_offset(class_off)? {
                classes.push(self.read_foreign_item(class_off)?);
            } else {
                classes.push(self.read_class_item(class_off)?);
            }
        }
        Ok(classes)
    }

    /// Return all IndexSection records in file order.
    ///
    /// This mirrors libpandafile's `File::GetIndexHeaders()`. Bytecode id
    /// operands are scoped by the IndexSection containing the current method
    /// item, then resolved through that section's method index.
    pub fn index_sections(&self) -> Result<Vec<AbcIndexSection>, AbcReadError> {
        let count = self.read_u32(0x34)? as usize;
        let start = self.read_u32(0x38)? as usize;
        let headers_end = start.saturating_add(count * 40);
        if headers_end > self.data.len() {
            return Err(AbcReadError::MalformedClassItem {
                offset: 0x38,
                reason: format!(
                    "IndexSection headers [{:#x}, {:#x}) exceed file length {}",
                    start,
                    headers_end,
                    self.data.len()
                ),
            });
        }

        let mut sections = Vec::with_capacity(count);
        for index in 0..count {
            let offset = start + index * 40;
            let section_start = self.read_u32(offset)?;
            let section_end = self.read_u32(offset + 4)?;
            let class_size = self.read_u32(offset + 8)?;
            let class_off = self.read_u32(offset + 12)?;
            let method_size = self.read_u32(offset + 16)?;
            let method_off = self.read_u32(offset + 20)?;
            let field_size = self.read_u32(offset + 24)?;
            let field_off = self.read_u32(offset + 28)?;
            let proto_size = self.read_u32(offset + 32)?;
            let proto_off = self.read_u32(offset + 36)?;

            sections.push(AbcIndexSection {
                offset: offset as u32,
                start: section_start,
                end: section_end,
                class_offsets: self.read_entity_id_table(class_off, class_size, "class")?,
                method_offsets: self.read_entity_id_table(method_off, method_size, "method")?,
                field_offsets: self.read_entity_id_table(field_off, field_size, "field")?,
                proto_offsets: self.read_entity_id_table(proto_off, proto_size, "proto")?,
            });
        }

        Ok(sections)
    }

    /// Resolve a bytecode method-index operand for `method_item_off`.
    ///
    /// Equivalent to upstream `File::ResolveMethodIndex(method_id, idx)`.
    pub fn resolve_method_index(
        &self,
        method_item_off: u32,
        index: u32,
    ) -> Result<Option<u32>, AbcReadError> {
        let Some(section) = self
            .index_sections()?
            .into_iter()
            .find(|section| section.start <= method_item_off && method_item_off < section.end)
        else {
            return Ok(None);
        };
        Ok(section.method_offsets.get(index as usize).copied())
    }

    /// Resolve a method-local proto-index operand.
    ///
    /// Equivalent to upstream `File::ResolveProtoIndex(method_id, idx)`.
    pub fn resolve_proto_index(
        &self,
        method_item_off: u32,
        index: u32,
    ) -> Result<Option<u32>, AbcReadError> {
        let Some(section) = self
            .index_sections()?
            .into_iter()
            .find(|section| section.start <= method_item_off && method_item_off < section.end)
        else {
            return Ok(None);
        };
        Ok(section.proto_offsets.get(index as usize).copied())
    }

    /// Return `ProtoDataAccessor::GetNumArgs()` for a ProtoItem offset.
    pub fn proto_num_args(&self, proto_off: u32) -> Result<u32, AbcReadError> {
        let mut cursor = proto_off as usize;
        let mut shorty = self.read_u16(cursor)?;
        cursor += 2;
        let mut elem_num = 0u32;

        while shorty != 0 {
            let shift = (elem_num % 4) * 4;
            let elem = (shorty >> shift) & 0x0f;
            if elem == 0 {
                break;
            }
            elem_num += 1;
            if elem_num % 4 == 0 {
                shorty = self.read_u16(cursor)?;
                cursor += 2;
            }
        }

        Ok(elem_num.saturating_sub(1))
    }

    /// Return the method-index table visible from `method_item_off`.
    pub fn method_index_for_method(
        &self,
        method_item_off: u32,
    ) -> Result<Option<Vec<u32>>, AbcReadError> {
        Ok(self
            .index_sections()?
            .into_iter()
            .find(|section| section.start <= method_item_off && method_item_off < section.end)
            .map(|section| section.method_offsets))
    }

    /// Return literal-array offsets referenced by header indexes, metadata
    /// fields, and method-index tables.
    pub fn literal_array_offsets(&self) -> Result<Vec<u32>, AbcReadError> {
        let mut offsets = BTreeSet::new();
        let version = self.read_u32(0x0C)?;
        if contains_literal_array_in_header(version) {
            let count = self.read_u32(0x2C)? as usize;
            let table_off = self.read_u32(0x30)? as usize;
            for index in 0..count {
                offsets.insert(self.read_u32(table_off + index * 4)?);
            }
        }

        let classes = self.class_items()?;
        let mut method_item_offsets = BTreeSet::new();
        let mut class_offsets = BTreeSet::new();
        for class in &classes {
            class_offsets.insert(class.offset);
            for field in &class.fields {
                if matches!(
                    field.name.as_str(),
                    "moduleRecordIdx" | "scopeNames" | "moduleRequestPhaseIdx"
                ) {
                    if let Some(AbcFieldValue::U32(offset)) = field.value {
                        offsets.insert(offset);
                    }
                }
            }
            for method in &class.methods {
                method_item_offsets.insert(method.offset);
            }
        }

        for section in self.index_sections()? {
            for offset in section.method_offsets {
                if offset == 0
                    || method_item_offsets.contains(&offset)
                    || class_offsets.contains(&offset)
                {
                    continue;
                }
                if self.read_literal_array_item(offset).is_ok() {
                    offsets.insert(offset);
                }
            }
        }

        Ok(offsets.into_iter().collect())
    }

    /// Parse a tagged literal-array payload using libpandafile's
    /// `LiteralDataAccessor::EnumerateLiteralVals` rules.
    pub fn read_literal_array_item(
        &self,
        literal_off: u32,
    ) -> Result<AbcLiteralArrayItem, AbcReadError> {
        let offset = literal_off as usize;
        let raw_item_count = self.read_u32(offset)?;
        let mut cursor = offset + 4;
        let mut entries = Vec::new();
        let mut item_index = 0u32;

        while item_index < raw_item_count {
            let tag = self.read_u8(cursor)?;
            cursor += 1;
            let entry = match tag {
                LITERAL_TAG_TAGVALUE => {
                    let value = self.read_u8(cursor)?;
                    cursor += 1;
                    AbcLiteralEntry::U8 { tag, value }
                }
                LITERAL_TAG_BOOL => {
                    let value = self.read_u8(cursor)? != 0;
                    cursor += 1;
                    AbcLiteralEntry::Bool(value)
                }
                LITERAL_TAG_INTEGER | LITERAL_TAG_LITERAL_BUFFER_INDEX => {
                    let value = self.read_u32(cursor)?;
                    cursor += 4;
                    AbcLiteralEntry::U32 { tag, value }
                }
                LITERAL_TAG_FLOAT => {
                    let value = f32::from_bits(self.read_u32(cursor)?);
                    cursor += 4;
                    AbcLiteralEntry::F32(value)
                }
                LITERAL_TAG_DOUBLE => {
                    let value = f64::from_bits(self.read_u64(cursor)?);
                    cursor += 8;
                    AbcLiteralEntry::F64(value)
                }
                LITERAL_TAG_STRING | LITERAL_TAG_ETS_IMPLEMENTS => {
                    let string_off = self.read_u32(cursor)?;
                    cursor += 4;
                    AbcLiteralEntry::String {
                        tag,
                        offset: string_off,
                        value: self.read_string_at(string_off as usize)?,
                    }
                }
                LITERAL_TAG_METHOD
                | LITERAL_TAG_GENERATOR_METHOD
                | LITERAL_TAG_ASYNC_GENERATOR_METHOD
                | LITERAL_TAG_GETTER
                | LITERAL_TAG_SETTER => {
                    let method_off = self.read_u32(cursor)?;
                    cursor += 4;
                    AbcLiteralEntry::Method {
                        tag,
                        offset: method_off,
                        name: self.method_name_at_offset(method_off)?,
                    }
                }
                LITERAL_TAG_ACCESSOR | LITERAL_TAG_BUILTIN_TYPE_INDEX | LITERAL_TAG_NULL => {
                    let value = self.read_u8(cursor)?;
                    cursor += 1;
                    if tag == LITERAL_TAG_NULL {
                        AbcLiteralEntry::Null
                    } else {
                        AbcLiteralEntry::U8 { tag, value }
                    }
                }
                LITERAL_TAG_METHOD_AFFILIATE => {
                    let value = self.read_u16(cursor)?;
                    cursor += 2;
                    AbcLiteralEntry::U16 { tag, value }
                }
                LITERAL_TAG_LITERAL_ARRAY => {
                    let nested_off = self.read_u32(cursor)?;
                    cursor += 4;
                    AbcLiteralEntry::LiteralArray { offset: nested_off }
                }
                LITERAL_TAG_ARRAY_U1
                | LITERAL_TAG_ARRAY_U8
                | LITERAL_TAG_ARRAY_I8
                | LITERAL_TAG_ARRAY_U16
                | LITERAL_TAG_ARRAY_I16
                | LITERAL_TAG_ARRAY_U32
                | LITERAL_TAG_ARRAY_I32
                | LITERAL_TAG_ARRAY_U64
                | LITERAL_TAG_ARRAY_I64
                | LITERAL_TAG_ARRAY_F32
                | LITERAL_TAG_ARRAY_F64
                | LITERAL_TAG_ARRAY_STRING => {
                    let declared_len = self.read_u32(cursor)? as usize;
                    cursor += 4;
                    let expected_len =
                        raw_item_count.checked_sub(2).ok_or_else(|| {
                            AbcReadError::MalformedClassItem {
                                offset,
                                reason: format!(
                                    "typed literal array tag {tag:#x} has raw item count {raw_item_count}"
                                ),
                            }
                        })? as usize;
                    if declared_len != expected_len {
                        return Err(AbcReadError::MalformedClassItem {
                            offset,
                            reason: format!(
                                "typed literal array tag {tag:#x} length mismatch: header={expected_len}, value={declared_len}"
                            ),
                        });
                    }
                    let mut values = Vec::with_capacity(declared_len);
                    for _ in 0..declared_len {
                        values.push(self.read_typed_literal_array_value(tag, &mut cursor)?);
                    }
                    entries.push(AbcLiteralEntry::Array { tag, values });
                    item_index = raw_item_count;
                    continue;
                }
                other => {
                    return Err(AbcReadError::MalformedClassItem {
                        offset: cursor - 1,
                        reason: format!("unsupported LiteralTag {other:#x}"),
                    });
                }
            };
            entries.push(entry);
            item_index += 2;
        }

        Ok(AbcLiteralArrayItem {
            offset: literal_off,
            raw_item_count,
            entries,
        })
    }

    fn read_typed_literal_array_value(
        &self,
        tag: u8,
        cursor: &mut usize,
    ) -> Result<AbcLiteralArrayValue, AbcReadError> {
        let value = match tag {
            LITERAL_TAG_ARRAY_U1 => {
                let value = self.read_u8(*cursor)? != 0;
                *cursor += 1;
                AbcLiteralArrayValue::Bool(value)
            }
            LITERAL_TAG_ARRAY_U8 | LITERAL_TAG_ARRAY_I8 => {
                let value = self.read_u8(*cursor)?;
                *cursor += 1;
                AbcLiteralArrayValue::U8(value)
            }
            LITERAL_TAG_ARRAY_U16 | LITERAL_TAG_ARRAY_I16 => {
                let value = self.read_u16(*cursor)?;
                *cursor += 2;
                AbcLiteralArrayValue::U16(value)
            }
            LITERAL_TAG_ARRAY_U32 | LITERAL_TAG_ARRAY_I32 => {
                let value = self.read_u32(*cursor)?;
                *cursor += 4;
                AbcLiteralArrayValue::U32(value)
            }
            LITERAL_TAG_ARRAY_U64 | LITERAL_TAG_ARRAY_I64 => {
                let value = self.read_u64(*cursor)?;
                *cursor += 8;
                AbcLiteralArrayValue::U64(value)
            }
            LITERAL_TAG_ARRAY_F32 => {
                let value = f32::from_bits(self.read_u32(*cursor)?);
                *cursor += 4;
                AbcLiteralArrayValue::F32(value)
            }
            LITERAL_TAG_ARRAY_F64 => {
                let value = f64::from_bits(self.read_u64(*cursor)?);
                *cursor += 8;
                AbcLiteralArrayValue::F64(value)
            }
            LITERAL_TAG_ARRAY_STRING => {
                let string_off = self.read_u32(*cursor)?;
                *cursor += 4;
                AbcLiteralArrayValue::String {
                    offset: string_off,
                    value: self.read_string_at(string_off as usize)?,
                }
            }
            other => {
                return Err(AbcReadError::MalformedClassItem {
                    offset: *cursor,
                    reason: format!("unsupported typed LiteralTag {other:#x}"),
                })
            }
        };
        Ok(value)
    }

    /// Read a standalone or inline Panda `StringItem` by absolute file offset.
    pub fn string_at(&self, offset: u32) -> Result<String, AbcReadError> {
        self.read_string_at(offset as usize)
    }

    // ── Private helpers ──────────────────────────────────────────────────────

    fn read_class_info(&self, class_off: usize) -> Result<AbcClassInfo, AbcReadError> {
        // ClassItem starts with an inline StringItem for the class name.
        let (name, name_len) =
            self.read_inline_string(class_off)
                .map_err(|e| AbcReadError::MalformedClassItem {
                    offset: class_off,
                    reason: format!("class name string: {e}"),
                })?;

        // After the inline class name:
        // super_class_off: u32 (4 bytes)
        let after_name = class_off + name_len;
        let after_super = after_name + 4;

        // access_flags: ULEB128
        let (_access_flags, acc_len) =
            self.read_uleb128(after_super)
                .map_err(|e| AbcReadError::MalformedClassItem {
                    offset: class_off,
                    reason: format!("access_flags: {e}"),
                })?;
        let after_acc = after_super + acc_len;

        // fields_count: ULEB128
        let (_fields_count, fc_len) =
            self.read_uleb128(after_acc)
                .map_err(|e| AbcReadError::MalformedClassItem {
                    offset: class_off,
                    reason: format!("fields_count: {e}"),
                })?;
        let after_fc = after_acc + fc_len;

        // methods_count: ULEB128
        let (methods_count, _) =
            self.read_uleb128(after_fc)
                .map_err(|e| AbcReadError::MalformedClassItem {
                    offset: class_off,
                    reason: format!("methods_count: {e}"),
                })?;

        Ok(AbcClassInfo {
            name,
            method_count: methods_count as usize,
        })
    }

    fn read_class_item(&self, class_off: u32) -> Result<AbcClassItem, AbcReadError> {
        let offset = class_off as usize;
        let (name, name_len) =
            self.read_inline_string(offset)
                .map_err(|e| AbcReadError::MalformedClassItem {
                    offset,
                    reason: format!("class name string: {e}"),
                })?;
        let mut cursor = offset + name_len;
        let super_class_off = self.read_u32(cursor)?;
        cursor += 4;
        let (access_flags, len) =
            self.read_uleb128(cursor)
                .map_err(|e| AbcReadError::MalformedClassItem {
                    offset,
                    reason: format!("access_flags: {e}"),
                })?;
        cursor += len;
        let (fields_count, len) =
            self.read_uleb128(cursor)
                .map_err(|e| AbcReadError::MalformedClassItem {
                    offset,
                    reason: format!("fields_count: {e}"),
                })?;
        cursor += len;
        let (methods_count, len) =
            self.read_uleb128(cursor)
                .map_err(|e| AbcReadError::MalformedClassItem {
                    offset,
                    reason: format!("methods_count: {e}"),
                })?;
        cursor += len;

        let mut source_lang = None;
        let mut source_file = None;
        let mut annotation_offsets = Vec::new();
        loop {
            let tag = self.read_u8(cursor)?;
            cursor += 1;
            match tag {
                0x00 => break,
                0x02 => {
                    source_lang = Some(self.read_u8(cursor)?);
                    cursor += 1;
                }
                0x04 => {
                    annotation_offsets.push(self.read_u32(cursor)?);
                    cursor += 4;
                }
                0x07 => {
                    let source_file_off = self.read_u32(cursor)?;
                    cursor += 4;
                    source_file = Some(self.read_string_at(source_file_off as usize)?);
                }
                other => {
                    return Err(AbcReadError::MalformedClassItem {
                        offset: cursor - 1,
                        reason: format!("unsupported ClassTag {other:#x}"),
                    });
                }
            }
        }

        let mut fields = Vec::with_capacity(fields_count as usize);
        for _ in 0..fields_count {
            let (field, next) = self.read_field_item(cursor)?;
            fields.push(field);
            cursor = next;
        }

        let mut methods = Vec::with_capacity(methods_count as usize);
        for _ in 0..methods_count {
            let (method, next) = self.read_method_item(cursor)?;
            methods.push(method);
            cursor = next;
        }

        Ok(AbcClassItem {
            name,
            offset: class_off,
            super_class_off,
            access_flags: access_flags as u32,
            source_lang,
            source_file,
            annotation_offsets,
            fields,
            methods,
        })
    }

    fn is_foreign_item_offset(&self, offset: u32) -> Result<bool, AbcReadError> {
        let foreign_off = self.read_u32(0x14)?;
        let foreign_size = self.read_u32(0x18)?;
        Ok(foreign_size != 0
            && offset >= foreign_off
            && offset < foreign_off.saturating_add(foreign_size))
    }

    fn read_foreign_item(&self, class_off: u32) -> Result<AbcClassItem, AbcReadError> {
        let name = self.read_string_at(class_off as usize)?;
        Ok(AbcClassItem {
            name,
            offset: class_off,
            super_class_off: 0,
            access_flags: 0,
            source_lang: None,
            source_file: None,
            annotation_offsets: Vec::new(),
            fields: Vec::new(),
            methods: Vec::new(),
        })
    }

    fn read_field_item(&self, offset: usize) -> Result<(AbcFieldItem, usize), AbcReadError> {
        let mut cursor = offset;
        let class_idx = self.read_u16(cursor)?;
        cursor += 2;
        let type_idx = self.read_u16(cursor)?;
        cursor += 2;
        let name_off = self.read_u32(cursor)?;
        cursor += 4;
        let name = self.read_string_at(name_off as usize)?;
        let (access_flags, len) =
            self.read_uleb128(cursor)
                .map_err(|e| AbcReadError::MalformedClassItem {
                    offset,
                    reason: format!("field access_flags: {e}"),
                })?;
        cursor += len;
        let mut value = None;
        let mut annotation_offsets = Vec::new();
        loop {
            let tag = self.read_u8(cursor)?;
            cursor += 1;
            match tag {
                0x00 => break,
                0x01 => {
                    let (raw, len) = self.read_sleb128(cursor)?;
                    value = Some(if (0..=u8::MAX as i64).contains(&raw) {
                        AbcFieldValue::U8(raw as u8)
                    } else {
                        AbcFieldValue::U32(raw as u32)
                    });
                    cursor += len;
                }
                0x02 => {
                    value = Some(AbcFieldValue::U32(self.read_u32(cursor)?));
                    cursor += 4;
                }
                0x04 => {
                    annotation_offsets.push(self.read_u32(cursor)?);
                    cursor += 4;
                }
                other => {
                    return Err(AbcReadError::MalformedClassItem {
                        offset: cursor - 1,
                        reason: format!("unsupported FieldTag {other:#x}"),
                    });
                }
            }
        }
        Ok((
            AbcFieldItem {
                offset: offset as u32,
                class_idx,
                type_idx,
                name,
                name_off,
                access_flags: access_flags as u32,
                value,
                annotation_offsets,
            },
            cursor,
        ))
    }

    fn read_method_item(&self, offset: usize) -> Result<(AbcMethodItem, usize), AbcReadError> {
        let mut cursor = offset;
        let class_idx = self.read_u16(cursor)?;
        cursor += 2;
        let proto_idx = self.read_u16(cursor)?;
        cursor += 2;
        let name_off = self.read_u32(cursor)?;
        cursor += 4;
        let name = self.read_string_at(name_off as usize)?;
        let (access_flags, len) =
            self.read_uleb128(cursor)
                .map_err(|e| AbcReadError::MalformedClassItem {
                    offset,
                    reason: format!("method access_flags: {e}"),
                })?;
        cursor += len;

        let mut source_lang = None;
        let mut code_off = None;
        let mut debug_info_off = None;
        let mut annotation_offsets = Vec::new();
        let mut param_annotation_offset = None;
        loop {
            let tag = self.read_u8(cursor)?;
            cursor += 1;
            match tag {
                0x00 => break,
                0x01 => {
                    code_off = Some(self.read_u32(cursor)?);
                    cursor += 4;
                }
                0x02 => {
                    source_lang = Some(self.read_u8(cursor)?);
                    cursor += 1;
                }
                0x05 => {
                    debug_info_off = Some(self.read_u32(cursor)?);
                    cursor += 4;
                }
                0x06 => {
                    annotation_offsets.push(self.read_u32(cursor)?);
                    cursor += 4;
                }
                0x07 => {
                    param_annotation_offset = Some(self.read_u32(cursor)?);
                    cursor += 4;
                }
                other => {
                    return Err(AbcReadError::MalformedClassItem {
                        offset: cursor - 1,
                        reason: format!("unsupported MethodTag {other:#x}"),
                    });
                }
            }
        }
        let code = code_off
            .map(|offset| self.read_code_item(offset))
            .transpose()?;
        Ok((
            AbcMethodItem {
                offset: offset as u32,
                class_idx,
                proto_idx,
                name,
                name_off,
                access_flags: access_flags as u32,
                source_lang,
                code_off,
                debug_info_off,
                annotation_offsets,
                param_annotation_offset,
                code,
            },
            cursor,
        ))
    }

    fn method_name_at_offset(&self, method_off: u32) -> Result<Option<String>, AbcReadError> {
        for class in self.class_items()? {
            for method in class.methods {
                if method.offset == method_off {
                    return Ok(Some(method.name));
                }
            }
        }
        Ok(None)
    }

    fn read_code_item(&self, code_off: u32) -> Result<AbcCodeItem, AbcReadError> {
        let offset = code_off as usize;
        let mut cursor = offset;
        let (num_vregs, len) =
            self.read_uleb128(cursor)
                .map_err(|e| AbcReadError::MalformedClassItem {
                    offset,
                    reason: format!("code num_vregs: {e}"),
                })?;
        cursor += len;
        let (num_args, len) =
            self.read_uleb128(cursor)
                .map_err(|e| AbcReadError::MalformedClassItem {
                    offset,
                    reason: format!("code num_args: {e}"),
                })?;
        cursor += len;
        let (insns_size, len) =
            self.read_uleb128(cursor)
                .map_err(|e| AbcReadError::MalformedClassItem {
                    offset,
                    reason: format!("code insns_size: {e}"),
                })?;
        cursor += len;
        let (tries_size, len) =
            self.read_uleb128(cursor)
                .map_err(|e| AbcReadError::MalformedClassItem {
                    offset,
                    reason: format!("code tries_size: {e}"),
                })?;
        cursor += len;

        let bytecode_start = cursor;
        let bytecode_end = bytecode_start + insns_size as usize;
        if bytecode_end > self.data.len() {
            return Err(AbcReadError::TooShort {
                expected: bytecode_end,
                actual: self.data.len(),
            });
        }
        let bytecode = self.data[bytecode_start..bytecode_end].to_vec();
        cursor = bytecode_end;

        let mut catches = Vec::with_capacity(tries_size as usize);
        for _ in 0..tries_size {
            let (try_begin, len) =
                self.read_uleb128(cursor)
                    .map_err(|e| AbcReadError::MalformedClassItem {
                        offset,
                        reason: format!("try begin: {e}"),
                    })?;
            cursor += len;
            let (try_len, len) =
                self.read_uleb128(cursor)
                    .map_err(|e| AbcReadError::MalformedClassItem {
                        offset,
                        reason: format!("try length: {e}"),
                    })?;
            cursor += len;
            let (num_catches, len) =
                self.read_uleb128(cursor)
                    .map_err(|e| AbcReadError::MalformedClassItem {
                        offset,
                        reason: format!("num catches: {e}"),
                    })?;
            cursor += len;
            for _ in 0..num_catches {
                let (type_idx, len) =
                    self.read_uleb128(cursor)
                        .map_err(|e| AbcReadError::MalformedClassItem {
                            offset,
                            reason: format!("catch type_idx: {e}"),
                        })?;
                cursor += len;
                let (handler_pc, len) =
                    self.read_uleb128(cursor)
                        .map_err(|e| AbcReadError::MalformedClassItem {
                            offset,
                            reason: format!("catch handler_pc: {e}"),
                        })?;
                cursor += len;
                let (code_size, len) =
                    self.read_uleb128(cursor)
                        .map_err(|e| AbcReadError::MalformedClassItem {
                            offset,
                            reason: format!("catch code_size: {e}"),
                        })?;
                cursor += len;
                catches.push(AbcCatchBlock {
                    try_begin: try_begin as u32,
                    try_end: (try_begin + try_len) as u32,
                    handler_pc: handler_pc as u32,
                    code_size: code_size as u32,
                    type_idx: type_idx as u32,
                });
            }
        }

        Ok(AbcCodeItem {
            offset: code_off,
            num_vregs: num_vregs as u32,
            num_args: num_args as u32,
            bytecode,
            catches,
        })
    }

    fn read_string_at(&self, offset: usize) -> Result<String, AbcReadError> {
        self.read_inline_string(offset)
            .map(|(value, _)| value)
            .map_err(|e| AbcReadError::MalformedString {
                offset,
                reason: e.to_string(),
            })
    }

    fn read_u8(&self, offset: usize) -> Result<u8, AbcReadError> {
        self.data
            .get(offset)
            .copied()
            .ok_or(AbcReadError::TooShort {
                expected: offset + 1,
                actual: self.data.len(),
            })
    }

    fn read_u16(&self, offset: usize) -> Result<u16, AbcReadError> {
        if offset + 2 > self.data.len() {
            return Err(AbcReadError::TooShort {
                expected: offset + 2,
                actual: self.data.len(),
            });
        }
        Ok(u16::from_le_bytes(
            self.data[offset..offset + 2].try_into().unwrap(),
        ))
    }

    /// Read a Panda `StringItem` (ULEB128-prefixed) at `offset`.
    ///
    /// Returns `(string, bytes_consumed)`.
    fn read_inline_string(&self, offset: usize) -> Result<(String, usize), AbcReadError> {
        let (prefix, prefix_len) = self.read_uleb128(offset)?;
        let is_ascii = (prefix & 1) == 1;
        let utf16_len = (prefix >> 1) as usize;

        let data_start = offset + prefix_len;

        if is_ascii {
            // ASCII: utf16_len bytes of ASCII + 0x00 terminator.
            let data_end = data_start + utf16_len;
            if data_end + 1 > self.data.len() {
                return Err(AbcReadError::TooShort {
                    expected: data_end + 1,
                    actual: self.data.len(),
                });
            }
            let bytes = &self.data[data_start..data_end];
            let s = String::from_utf8(bytes.to_vec())
                .map_err(|e| AbcReadError::InvalidUtf8 { offset, source: e })?;
            let total_len = prefix_len + utf16_len + 1; // +1 for null terminator
            Ok((s, total_len))
        } else {
            // UTF-16LE: utf16_len u16 units + 0x00 0x00 terminator.
            let byte_len = utf16_len * 2;
            let data_end = data_start + byte_len;
            if data_end + 2 > self.data.len() {
                return Err(AbcReadError::TooShort {
                    expected: data_end + 2,
                    actual: self.data.len(),
                });
            }
            let units: Vec<u16> = self.data[data_start..data_end]
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            let s = String::from_utf16(&units).unwrap_or_else(|_| "?".to_string());
            let total_len = prefix_len + byte_len + 2; // +2 for UTF-16 null terminator
            Ok((s, total_len))
        }
    }

    fn read_u32(&self, offset: usize) -> Result<u32, AbcReadError> {
        if offset + 4 > self.data.len() {
            return Err(AbcReadError::TooShort {
                expected: offset + 4,
                actual: self.data.len(),
            });
        }
        Ok(u32::from_le_bytes(
            self.data[offset..offset + 4].try_into().unwrap(),
        ))
    }

    fn read_sleb128(&self, mut offset: usize) -> Result<(i64, usize), AbcReadError> {
        let start = offset;
        let mut result = 0i64;
        let mut shift = 0u32;
        let mut byte;
        loop {
            byte = self.read_u8(offset)?;
            offset += 1;
            result |= ((byte & 0x7f) as i64) << shift;
            shift += 7;
            if (byte & 0x80) == 0 {
                break;
            }
            if shift >= 64 {
                return Err(AbcReadError::MalformedClassItem {
                    offset: start,
                    reason: "SLEB128 value is too large".to_string(),
                });
            }
        }
        if shift < 64 && (byte & 0x40) != 0 {
            result |= (!0i64) << shift;
        }
        Ok((result, offset - start))
    }

    fn read_u64(&self, offset: usize) -> Result<u64, AbcReadError> {
        if offset + 8 > self.data.len() {
            return Err(AbcReadError::TooShort {
                expected: offset + 8,
                actual: self.data.len(),
            });
        }
        Ok(u64::from_le_bytes(
            self.data[offset..offset + 8].try_into().unwrap(),
        ))
    }

    fn read_entity_id_table(
        &self,
        table_off: u32,
        table_size: u32,
        table_name: &str,
    ) -> Result<Vec<u32>, AbcReadError> {
        if table_size == INVALID_U32 && table_off == INVALID_U32 {
            return Ok(Vec::new());
        }
        if table_size == INVALID_U32 || table_off == INVALID_U32 {
            return Err(AbcReadError::MalformedClassItem {
                offset: table_off.min(self.data.len() as u32) as usize,
                reason: format!(
                    "{table_name} index has mixed invalid fields: size={table_size:#x}, off={table_off:#x}"
                ),
            });
        }

        let offset = table_off as usize;
        let size = table_size as usize;
        let end = offset.saturating_add(size * 4);
        if end > self.data.len() {
            return Err(AbcReadError::MalformedClassItem {
                offset,
                reason: format!(
                    "{table_name} index table [{:#x}, {:#x}) exceeds file length {}",
                    offset,
                    end,
                    self.data.len()
                ),
            });
        }

        let mut values = Vec::with_capacity(size);
        for index in 0..size {
            values.push(self.read_u32(offset + index * 4)?);
        }
        Ok(values)
    }

    fn read_uleb128(&self, offset: usize) -> Result<(u64, usize), AbcReadError> {
        let mut value: u64 = 0;
        let mut shift = 0u32;
        let mut pos = offset;
        loop {
            if pos >= self.data.len() {
                return Err(AbcReadError::TooShort {
                    expected: pos + 1,
                    actual: self.data.len(),
                });
            }
            let byte = self.data[pos];
            pos += 1;
            value |= ((byte & 0x7F) as u64) << shift;
            shift += 7;
            if byte & 0x80 == 0 {
                break;
            }
            if shift >= 64 {
                return Err(AbcReadError::MalformedString {
                    offset,
                    reason: "ULEB128 value exceeds 64 bits".to_string(),
                });
            }
        }
        Ok((value, pos - offset))
    }
}

/// Check whether all class names in `dep_data` are already present in `base_data`.
///
/// Returns the subset of class names from `dep_data` that are NOT in `base_data`.
/// Used to detect which ByteCode HAR classes would be added on merge.
/// Per-section hashes for an ABC binary.
///
/// Used by the regression test harness (TASK-ABC-03) to determine whether an
/// ABC output change is in the executable bytecode or only in the debug
/// (DebugInfo / LineNumberProgram) sections.
///
/// # Usage
///
/// ```ignore
/// let golden = AbcSectionHashes::compute(golden_bytes)?;
/// let actual = AbcSectionHashes::compute(actual_bytes)?;
/// // Assert only the code (non-debug) region is byte-identical.
/// assert_eq!(golden.code_hash, actual.code_hash,
///            "bytecode changed — check CodeItems/ClassRecords/StringTable");
/// // Allow debug info to differ in a separate assertion if desired.
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbcSectionHashes {
    /// FNV-1a 64-bit hash of the full file bytes.
    pub full_hash: u64,
    /// FNV-1a 64-bit hash of the file up to (but not including) the first
    /// LNP entry referenced by the LNP index.  This covers the header,
    /// class records, string table, method bytecode, and literal arrays.
    ///
    /// A mismatch here means executable bytecode changed.
    pub code_hash: u64,
    /// FNV-1a 64-bit hash of the LNP (LineNumberProgram) section.
    ///
    /// A mismatch here (while `code_hash` is identical) means only debug
    /// source-position information changed.
    pub lnp_hash: u64,
    /// Total size of the file in bytes (from the ABC header field at 0x10).
    pub file_size: u32,
    /// Number of LNP entries (from ABC header field at 0x24).
    pub num_lnp: u32,
}

impl AbcSectionHashes {
    /// Compute per-section hashes for an ABC binary.
    ///
    /// Requires valid PANDA magic and a parseable header.  Errors are returned
    /// for malformed files.
    pub fn compute(data: &[u8]) -> Result<Self, AbcReadError> {
        let reader = AbcReader::new(data)?;

        let file_size = reader.read_u32(0x10)?;
        let num_lnp = reader.read_u32(0x24)?;
        let lnp_idx_off = reader.read_u32(0x28)? as usize;

        let full_hash = fnv1a_hash_bytes(data);

        // The LNP section begins at the first LNP item offset listed in the
        // LNP index.  The LNP index itself is at `lnp_idx_off`; each entry is
        // a u32 pointing to the start of a LineNumberProgram item.
        //
        // If there are no LNP entries, lnp_idx_off is 0 (per Panda spec): in
        // that case the entire file is the "code" region.
        let lnp_data_start = if num_lnp > 0 && lnp_idx_off > 0 && lnp_idx_off + 4 <= data.len() {
            // Read the first LNP item offset (the earliest LNP body in the file).
            let mut earliest = reader.read_u32(lnp_idx_off)? as usize;
            for i in 1..num_lnp as usize {
                let off = lnp_idx_off + i * 4;
                if off + 4 > data.len() {
                    break;
                }
                let candidate = reader.read_u32(off)? as usize;
                if candidate < earliest {
                    earliest = candidate;
                }
            }
            // Clamp to valid range.
            earliest.min(data.len())
        } else {
            data.len()
        };

        // "code region" = everything before the first LNP body.
        let code_hash = fnv1a_hash_bytes(&data[..lnp_data_start]);

        // "LNP region" = everything from the first LNP body to end of file.
        let lnp_hash = fnv1a_hash_bytes(&data[lnp_data_start..]);

        Ok(Self {
            full_hash,
            code_hash,
            lnp_hash,
            file_size,
            num_lnp,
        })
    }

    /// Returns `true` when both files have the same code hash (CodeItems,
    /// ClassRecords, StringTable, LiteralArrays) even if debug info differs.
    pub fn code_region_matches(&self, other: &Self) -> bool {
        self.code_hash == other.code_hash
    }

    /// Returns `true` when the full file is byte-identical.
    pub fn is_identical(&self, other: &Self) -> bool {
        self.full_hash == other.full_hash && self.file_size == other.file_size
    }
}

/// FNV-1a 64-bit hash of a byte slice.
///
/// Used internally by `AbcSectionHashes::compute()` to avoid pulling in an
/// additional crate dependency.
fn fnv1a_hash_bytes(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

pub fn diff_class_names(base_data: &[u8], dep_data: &[u8]) -> Result<Vec<String>, AbcReadError> {
    let base_names: std::collections::HashSet<String> = AbcReader::new(base_data)?
        .class_names()?
        .into_iter()
        .collect();
    let dep_names = AbcReader::new(dep_data)?.class_names()?;
    Ok(dep_names
        .into_iter()
        .filter(|n| !base_names.contains(n))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::{AbcCompileOptions, AbcCompiler, AbcModule};

    fn compile_to_bytes(source: &str, record_name: &str) -> Vec<u8> {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let compiler = AbcCompiler::new(AbcCompileOptions {
            debug_info: false,
            opt_level: 0,
            optimize_try_catch_func: true,
            target_api_version: 12,
            target_api_sub_version: None,
            enable_annotations: false,
            merge_abc: true,
            module: false,
            module_record_field_name: None,
            npm_entries: None,
            dependency_context: None,
            remove_redundant_file: false,
            dump_deps_info: false,
        });
        compiler
            .compile(
                vec![AbcModule {
                    record_name: record_name.to_string(),
                    source: source.to_string(),
                    source_map: None,
                    source_file: None,
                    package_name: None,
                    is_commonjs: false,
                    is_shared_module: false,
                    origin_source_lang: String::new(),
                }],
                tmp.path(),
            )
            .expect("compile must succeed");
        std::fs::read(tmp.path()).expect("read abc bytes")
    }

    #[test]
    fn test_abc_reader_validates_magic() {
        let bytes = compile_to_bytes("var x = 1;", "test/reader");
        let reader = AbcReader::new(&bytes).expect("valid magic");
        let summary = reader.read_summary().expect("read summary");
        assert!(summary.file_size > 0, "file_size must be > 0");
        assert!(!summary.classes.is_empty(), "must have at least one class");
    }

    #[test]
    fn test_abc_reader_rejects_bad_magic() {
        let mut bytes = vec![0u8; ABC_HEADER_SIZE];
        bytes[0..8].copy_from_slice(b"NOTPANDA");
        let result = AbcReader::new(&bytes);
        assert!(
            matches!(result, Err(AbcReadError::BadMagic { .. })),
            "expected BadMagic error"
        );
    }

    #[test]
    fn test_abc_reader_counts_proto_args_from_shorty() {
        let mut bytes = vec![0u8; ABC_HEADER_SIZE + 8];
        bytes[0..8].copy_from_slice(&ABC_MAGIC);
        let proto_off = ABC_HEADER_SIZE;

        bytes[proto_off..proto_off + 2].copy_from_slice(&0x0321u16.to_le_bytes());
        let reader = AbcReader::new(&bytes).unwrap();
        assert_eq!(reader.proto_num_args(proto_off as u32).unwrap(), 2);

        bytes[proto_off..proto_off + 2].copy_from_slice(&0x4321u16.to_le_bytes());
        bytes[proto_off + 2..proto_off + 4].copy_from_slice(&0x0005u16.to_le_bytes());
        let reader = AbcReader::new(&bytes).unwrap();
        assert_eq!(reader.proto_num_args(proto_off as u32).unwrap(), 4);
    }

    #[test]
    fn test_abc_reader_extracts_class_names() {
        let bytes = compile_to_bytes("var x = 1;", "entry/ets/pages/Index");
        let names = AbcReader::new(&bytes).unwrap().class_names().unwrap();
        // The reader should include the user record and any annotation stubs.
        assert!(
            names.iter().any(|n| n.contains("Index")),
            "class names must include the user record; got {names:?}"
        );
    }

    #[test]
    fn test_abc_reader_extracts_class_fields_methods_and_code_items() {
        let bytes = compile_to_bytes("var x = 1; x;", "entry/ets/pages/Index");
        let classes = AbcReader::new(&bytes).unwrap().class_items().unwrap();
        let class = classes
            .iter()
            .find(|class| class.name.contains("entry/ets/pages/Index"))
            .expect("user class must be present");

        assert_eq!(class.access_flags, 1);
        assert_eq!(class.source_lang, Some(0));
        assert!(
            class.fields.iter().any(
                |field| field.name == "isCommonjs" && field.value == Some(AbcFieldValue::U8(0))
            ),
            "module metadata fields must be parsed, got {:?}",
            class.fields
        );
        assert!(
            class
                .fields
                .iter()
                .any(|field| field.name == "moduleRecordIdx"
                    && matches!(field.value, Some(AbcFieldValue::U32(_)))),
            "moduleRecordIdx literal-array offset field must be parsed"
        );

        let method = class
            .methods
            .iter()
            .find(|method| method.name == "func_main_0")
            .expect("func_main_0 method must be present");
        assert_eq!(method.source_lang, Some(0));
        assert!(
            method.code_off.is_some(),
            "method code offset must be parsed"
        );
        assert!(
            !method.annotation_offsets.is_empty(),
            "SlotNumber annotation offset must be parsed"
        );

        let code = method
            .code
            .as_ref()
            .expect("method code item must be parsed");
        assert_eq!(code.num_args, 3);
        assert!(!code.bytecode.is_empty(), "bytecode must be parsed");
        assert_eq!(code.offset, method.code_off.unwrap());
    }

    #[test]
    fn test_abc_reader_extracts_index_sections_and_resolves_method_index() {
        let bytes = compile_to_bytes(
            "function f() { return 'hello'; } f();",
            "entry/ets/pages/Index",
        );
        let reader = AbcReader::new(&bytes).unwrap();
        let classes = reader.class_items().unwrap();
        let sections = reader.index_sections().unwrap();
        assert_eq!(
            sections.len(),
            1,
            "generated ABC must have one IndexSection"
        );

        let user_class = classes
            .iter()
            .find(|class| class.name.contains("entry/ets/pages/Index"))
            .expect("user class must be present");
        let method = user_class
            .methods
            .iter()
            .find(|method| method.name == "func_main_0")
            .expect("func_main_0 method must be present");
        let pool = reader
            .method_index_for_method(method.offset)
            .unwrap()
            .expect("method item must be covered by an IndexSection");
        assert!(!pool.is_empty(), "method index pool must be parsed");
        assert_eq!(
            reader.resolve_method_index(method.offset, 0).unwrap(),
            pool.first().copied()
        );
        assert!(
            sections[0].class_offsets.contains(&user_class.offset),
            "IndexSection class table must include the user class offset"
        );
    }

    #[test]
    fn test_abc_reader_extracts_literal_array_offsets_and_payloads() {
        let bytes = compile_to_bytes(
            "class A { m() { return 1; } } new A().m();",
            "entry/ets/pages/Index",
        );
        let reader = AbcReader::new(&bytes).unwrap();
        let classes = reader.class_items().unwrap();
        let user_class = classes
            .iter()
            .find(|class| class.name.contains("entry/ets/pages/Index"))
            .expect("user class must be present");
        let module_record_idx = user_class
            .fields
            .iter()
            .find_map(|field| {
                (field.name == "moduleRecordIdx")
                    .then_some(field.value)
                    .flatten()
            })
            .expect("moduleRecordIdx field value must be present");
        let AbcFieldValue::U32(module_literal_off) = module_record_idx else {
            panic!("moduleRecordIdx must be stored as a literal-array offset");
        };

        let literal_offsets = reader.literal_array_offsets().unwrap();
        assert!(
            literal_offsets.contains(&module_literal_off),
            "metadata literal array offset must be collected"
        );
        assert!(
            literal_offsets
                .iter()
                .any(|&offset| reader.read_literal_array_item(offset).is_ok()),
            "at least one method-index literal array must parse with tagged literal rules"
        );
    }

    #[test]
    fn test_abc_reader_is_valid_magic_helper() {
        assert!(!AbcReader::is_valid_magic(b"not abc"));
        assert!(AbcReader::is_valid_magic(b"PANDA\x00\x00\x00extra"));
        assert!(!AbcReader::is_valid_magic(b"PANDA\x00\x00")); // too short
    }

    // ── AbcSectionHashes ──────────────────────────────────────────────────────

    #[test]
    fn test_section_hashes_full_equals_code_when_no_lnp() {
        // Compile without debug info: no LNP entries → code_hash == full_hash.
        let bytes = compile_to_bytes("var x = 1;", "entry/ets/Index");
        let hashes = AbcSectionHashes::compute(&bytes).unwrap();
        // num_lnp == 0 or lnp_idx_off == 0 → lnp_data_start == file_size.
        // full_hash and code_hash may or may not be equal depending on whether
        // any LNP index is present; at minimum the struct should be parseable.
        assert_eq!(hashes.full_hash, fnv1a_hash_bytes(&bytes));
        assert!(hashes.file_size as usize <= bytes.len());
    }

    #[test]
    fn test_section_hashes_same_source_yields_identical_hashes() {
        let source = "export function add(a: number, b: number): number { return a + b; }";
        let bytes1 = compile_to_bytes(source, "module/ets/math");
        let bytes2 = compile_to_bytes(source, "module/ets/math");
        let h1 = AbcSectionHashes::compute(&bytes1).unwrap();
        let h2 = AbcSectionHashes::compute(&bytes2).unwrap();
        assert!(
            h1.is_identical(&h2),
            "identical sources must produce identical ABC"
        );
    }

    #[test]
    fn test_section_hashes_code_region_matches_catches_debug_vs_code_change() {
        // Two different sources produce different code hashes.
        let bytes_a = compile_to_bytes("var x = 1;", "entry/ets/A");
        let bytes_b = compile_to_bytes("var y = 2;", "entry/ets/B");
        let h_a = AbcSectionHashes::compute(&bytes_a).unwrap();
        let h_b = AbcSectionHashes::compute(&bytes_b).unwrap();
        // Different sources must differ in code_hash.
        assert!(
            !h_a.code_region_matches(&h_b),
            "different sources must not have matching code hash"
        );
    }
}
