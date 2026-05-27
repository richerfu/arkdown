use std::collections::HashMap;

/// Interned string table for the ABC file string section.
///
/// Every string referenced in bytecode (method names, global variable names,
/// property names, string literals) is stored here and referenced by index.
#[derive(Debug, Default)]
pub struct StringTable {
    strings: Vec<String>,
    index: HashMap<String, u32>,
}

impl StringTable {
    /// Intern `s` and return its stable index in the string table.
    ///
    /// If `s` was already interned the existing index is returned without
    /// allocating a new entry.
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

    /// Serialize the string table as a length-prefixed sequence of UTF-8 entries.
    ///
    /// Format:
    /// ```text
    /// [count: u32 LE]
    /// For each string:
    ///   [len: u16 LE] [bytes: u8 * len]
    /// ```
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&(self.strings.len() as u32).to_le_bytes());
        for s in &self.strings {
            let bytes = s.as_bytes();
            buf.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
            buf.extend_from_slice(bytes);
        }
        buf
    }
}
