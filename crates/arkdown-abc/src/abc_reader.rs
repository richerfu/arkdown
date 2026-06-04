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
//! 0x2C  num_literalarrays[4]   Number of LiteralArray items
//! 0x30  literalarray_idx_off[4] Offset of literal-array offset array
//! 0x34  num_idx_regions[4]     Number of IndexSection regions
//! 0x38  idx_section_off[4]     Offset of first IndexSection header
//! ```
//!
//! Upstream reference: `libpandafile/file_items.h` in
//! `arkcompiler_ets_frontend/arkcompiler/runtime_core`.

use thiserror::Error;

/// Error type for ABC binary parse failures.
#[derive(Debug, Error)]
pub enum AbcReadError {
    #[error("file too short: expected at least {expected} bytes, got {actual}")]
    TooShort { expected: usize, actual: usize },
    #[error("invalid magic bytes: expected b\"PANDA\\x00\\x00\\x00\", got {found:?}")]
    BadMagic { found: [u8; 8] },
    #[error("class index out of bounds: offset {offset} + {size} bytes exceeds file length {file_len}")]
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
                        lnp_idx_off, lnp_end, data.len()
                    ),
                });
            }
        }

        // LiteralArray index bounds (offset 0x2C/0x30; absent when off == 0).
        let num_literal = reader.read_u32(0x2C)? as usize;
        let literal_idx_off = reader.read_u32(0x30)? as usize;
        if literal_idx_off != 0 {
            let lit_end = literal_idx_off.saturating_add(num_literal * 4);
            if lit_end > data.len() {
                return Err(AbcReadError::MalformedClassItem {
                    offset: 0x30,
                    reason: format!(
                        "LiteralArray index region [{:#x}, {:#x}) exceeds file length {}",
                        literal_idx_off, lit_end, data.len()
                    ),
                });
            }
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
        Ok(self.read_summary()?.classes.into_iter().map(|c| c.name).collect())
    }

    // ── Private helpers ──────────────────────────────────────────────────────

    fn read_class_info(&self, class_off: usize) -> Result<AbcClassInfo, AbcReadError> {
        // ClassItem starts with an inline StringItem for the class name.
        let (name, name_len) = self.read_inline_string(class_off).map_err(|e| {
            AbcReadError::MalformedClassItem {
                offset: class_off,
                reason: format!("class name string: {e}"),
            }
        })?;

        // After the inline class name:
        // super_class_off: u32 (4 bytes)
        let after_name = class_off + name_len;
        let after_super = after_name + 4;

        // access_flags: ULEB128
        let (_access_flags, acc_len) = self.read_uleb128(after_super).map_err(|e| {
            AbcReadError::MalformedClassItem {
                offset: class_off,
                reason: format!("access_flags: {e}"),
            }
        })?;
        let after_acc = after_super + acc_len;

        // fields_count: ULEB128
        let (_fields_count, fc_len) = self.read_uleb128(after_acc).map_err(|e| {
            AbcReadError::MalformedClassItem {
                offset: class_off,
                reason: format!("fields_count: {e}"),
            }
        })?;
        let after_fc = after_acc + fc_len;

        // methods_count: ULEB128
        let (methods_count, _) = self.read_uleb128(after_fc).map_err(|e| {
            AbcReadError::MalformedClassItem {
                offset: class_off,
                reason: format!("methods_count: {e}"),
            }
        })?;

        Ok(AbcClassInfo {
            name,
            method_count: methods_count as usize,
        })
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
            let s = String::from_utf8(bytes.to_vec()).map_err(|e| AbcReadError::InvalidUtf8 {
                offset,
                source: e,
            })?;
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
        Ok(u32::from_le_bytes(self.data[offset..offset + 4].try_into().unwrap()))
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
pub fn diff_class_names(
    base_data: &[u8],
    dep_data: &[u8],
) -> Result<Vec<String>, AbcReadError> {
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
            target_api_version: 12,
            target_api_sub_version: None,
            merge_abc: true,
            module: false,
            cache_path: None,
            npm_entries: None,
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
    fn test_abc_reader_is_valid_magic_helper() {
        assert!(!AbcReader::is_valid_magic(b"not abc"));
        assert!(AbcReader::is_valid_magic(b"PANDA\x00\x00\x00extra"));
        assert!(!AbcReader::is_valid_magic(b"PANDA\x00\x00")); // too short
    }
}
