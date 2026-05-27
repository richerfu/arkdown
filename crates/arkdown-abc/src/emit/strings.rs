use std::collections::HashMap;

/// Encode `value` as an unsigned LEB-128 (ULEB128) byte sequence.
///
/// Used for the length prefix of Panda string items.  For values less than 128
/// this is a single byte, which covers the common case of short identifiers.
fn uleb128(mut value: u64) -> Vec<u8> {
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

/// Interned string table for the ABC file string section.
///
/// Every string referenced in bytecode (method names, global variable names,
/// property names, string literals) is stored here and referenced by a
/// zero-based index.  The index is used directly in the bytecode's 16-bit
/// `string_id` operands.
///
/// ## Wire format
///
/// Each string item is serialised as:
/// ```text
/// ULEB128((utf16_length << 1) | is_ascii)  ← length + ASCII flag packed
/// [raw bytes: utf16_length bytes if ASCII, utf16_length*2 if UTF-16LE]
/// 0x00                                      ← null terminator
/// ```
///
/// This matches the `StringItem` format in
/// `arkcompiler_runtime_core/libpandafile/file_items.h` and was verified against
/// real `modules.abc` output (see binary analysis in SPEC-009 notes).
///
/// The serialised form is preceded by a `u32 LE` count of all strings, which
/// our simplified reader uses to locate individual items.  The real Panda
/// format does *not* have a count header — strings are referenced directly by
/// file offset.  Full offset-based referencing is a SPEC-009 follow-up.
#[derive(Debug, Default)]
pub struct StringTable {
    strings: Vec<String>,
    index: HashMap<String, u32>,
}

impl StringTable {
    /// Intern `s` and return its stable zero-based index in the string table.
    ///
    /// Calling `intern` with the same string twice always returns the same index.
    pub fn intern(&mut self, s: &str) -> u32 {
        if let Some(&idx) = self.index.get(s) {
            return idx;
        }
        let idx = self.strings.len() as u32;
        self.strings.push(s.to_string());
        self.index.insert(s.to_string(), idx);
        idx
    }

    /// Retrieve the string at `idx`, if present.
    pub fn get(&self, idx: u32) -> Option<&str> {
        self.strings.get(idx as usize).map(|s| s.as_str())
    }

    /// Number of unique strings currently interned.
    pub fn len(&self) -> usize {
        self.strings.len()
    }

    /// Returns `true` if no strings have been interned.
    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }

    /// Serialise the string table.
    ///
    /// Layout:
    /// ```text
    /// [count: u32 LE]          ← number of string items
    /// For each string:
    ///   ULEB128((utf16_len << 1) | is_ascii)
    ///   [raw bytes]            ← utf16_len bytes (ASCII) or utf16_len*2 (UTF-16LE)
    ///   [0x00]                 ← null terminator
    /// ```
    ///
    /// The ULEB128 prefix packs the UTF-16 codeunit length and the ASCII flag
    /// into a single variable-length integer, exactly as the real Panda format
    /// does.  Verified against `modules.abc` at offset 0x400:
    /// ```text
    /// 0x400: 61                ← ULEB128: 0x61 = 97 = (48 << 1) | 1
    ///        4c 26 ... 3b      ← 48 ASCII bytes of "L&entry/.../EntryAbility&;"
    ///        00                ← null terminator
    /// ```
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        // Simplified count prefix (not part of the real Panda format, but used
        // by our reader to iterate string items).
        buf.extend_from_slice(&(self.strings.len() as u32).to_le_bytes());

        for s in &self.strings {
            let bytes = s.as_bytes();
            // Determine whether all bytes are 7-bit ASCII.
            let is_ascii = bytes.iter().all(|&b| b < 0x80);

            if is_ascii {
                // UTF-16 length = number of characters = byte length for ASCII.
                let utf16_len = bytes.len() as u64;
                let prefix = (utf16_len << 1) | 1u64; // is_ascii = 1
                buf.extend_from_slice(&uleb128(prefix));
                buf.extend_from_slice(bytes);
            } else {
                // Encode as UTF-16LE; count UTF-16 code units.
                let utf16: Vec<u16> = s.encode_utf16().collect();
                let utf16_len = utf16.len() as u64;
                let prefix = utf16_len << 1; // is_ascii = 0
                buf.extend_from_slice(&uleb128(prefix));
                for unit in &utf16 {
                    buf.extend_from_slice(&unit.to_le_bytes());
                }
            }
            buf.push(0x00); // null terminator
        }
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uleb128_single_byte() {
        assert_eq!(uleb128(0), vec![0x00]);
        assert_eq!(uleb128(1), vec![0x01]);
        assert_eq!(uleb128(127), vec![0x7F]);
    }

    #[test]
    fn test_uleb128_multi_byte() {
        // 128 = 0x80: encoded as [0x80, 0x01]
        assert_eq!(uleb128(128), vec![0x80, 0x01]);
        // 300 = 0x12C: encoded as [0xAC, 0x02]
        assert_eq!(uleb128(300), vec![0xAC, 0x02]);
    }

    #[test]
    fn test_string_prefix_ascii() {
        // "hello" = 5 ASCII chars → prefix = (5 << 1) | 1 = 11 = 0x0B
        let mut st = StringTable::default();
        st.intern("hello");
        let bytes = st.serialize();
        // count(4) + ULEB128(11=0x0B)(1) + b"hello"(5) + null(1) = 11 bytes
        assert_eq!(bytes.len(), 4 + 1 + 5 + 1);
        assert_eq!(bytes[4], 0x0B, "ULEB128 prefix for 5-char ASCII string");
        assert_eq!(&bytes[5..10], b"hello");
        assert_eq!(bytes[10], 0x00);
    }

    #[test]
    fn test_string_prefix_48_chars() {
        // Matches the EntryAbility class name verified in modules.abc:
        // 48-char ASCII → prefix = (48 << 1) | 1 = 97 = 0x61
        let name = "L&entry/src/main/ets/entryability/EntryAbility&;";
        assert_eq!(name.len(), 48);
        let mut st = StringTable::default();
        st.intern(name);
        let bytes = st.serialize();
        // count(4) + ULEB128(97=0x61)(1) + 48 bytes + null(1) = 54 bytes
        assert_eq!(bytes[4], 0x61, "ULEB128 prefix must be 0x61 = 97 for 48-char ASCII");
    }
}
