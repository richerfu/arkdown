//! Binary writer for `resources.index`.
//!
//! # Format
//!
//! This is a best-effort reconstruction of the format produced by OpenHarmony's
//! `restool` tool.  The authoritative source is:
//! <https://gitcode.com/openharmony/developtools_restool>.
//! If the HAP packager rejects the output, verify the exact byte layout against
//! that repository and update this implementation accordingly.
//!
//! ```text
//! Header (12 bytes):
//!   magic    : u32 LE = 0xF00F_1234
//!   version  : u32 LE = 0x0000_0001
//!   count    : u32 LE = number of entries
//!
//! Per-entry record:
//!   id           : u32 LE   — e.g. 0x0100_0001
//!   entry_type   : u32 LE   — 0x4=string, 0x5=integer, 0x6=float, 0xA=color, 0x1=media
//!   name_len     : u16 LE
//!   name         : name_len bytes (UTF-8)
//!   value_len    : u16 LE
//!   value        : value_len bytes (UTF-8)
//!   qualifier_len: u16 LE
//!   qualifier    : qualifier_len bytes (UTF-8, empty string = base qualifier)
//! ```

use std::path::Path;
use crate::compiler::CompiledEntry;

/// Magic number written at offset 0 of `resources.index`.
pub const MAGIC: u32 = 0xF00F_1234;
/// Format version written at offset 4.
pub const VERSION: u32 = 0x0000_0001;

/// Serialise `entries` and write the result to `output`.
pub fn write_resources_index(
    entries: &[CompiledEntry],
    output: &Path,
) -> Result<(), std::io::Error> {
    let mut buf: Vec<u8> = Vec::with_capacity(12 + entries.len() * 64);

    // Header
    buf.extend_from_slice(&MAGIC.to_le_bytes());
    buf.extend_from_slice(&VERSION.to_le_bytes());
    buf.extend_from_slice(&(entries.len() as u32).to_le_bytes());

    // Entries
    for entry in entries {
        buf.extend_from_slice(&entry.id.to_le_bytes());
        buf.extend_from_slice(&entry.entry_type.to_le_bytes());

        let name_bytes = entry.name.as_bytes();
        buf.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        buf.extend_from_slice(name_bytes);

        let value_bytes = entry.value.as_bytes();
        buf.extend_from_slice(&(value_bytes.len() as u16).to_le_bytes());
        buf.extend_from_slice(value_bytes);

        let qual_bytes = entry.qualifier.as_bytes();
        buf.extend_from_slice(&(qual_bytes.len() as u16).to_le_bytes());
        buf.extend_from_slice(qual_bytes);
    }

    fs_err::write(output, &buf)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::CompiledEntry;

    fn make_entry(id: u32, entry_type: u32, name: &str, value: &str, qualifier: &str) -> CompiledEntry {
        CompiledEntry {
            id,
            entry_type,
            name: name.to_string(),
            value: value.to_string(),
            qualifier: qualifier.to_string(),
        }
    }

    #[test]
    fn test_write_resources_index_header() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("resources.index");
        let entries = vec![make_entry(0x01000001, 0x4, "app_name", "My App", "")];
        write_resources_index(&entries, &out).unwrap();

        let bytes = std::fs::read(&out).unwrap();
        // magic
        assert_eq!(&bytes[0..4], &0xF00F_1234_u32.to_le_bytes());
        // version
        assert_eq!(&bytes[4..8], &0x0000_0001_u32.to_le_bytes());
        // count = 1
        assert_eq!(&bytes[8..12], &1_u32.to_le_bytes());
    }

    #[test]
    fn test_write_resources_index_entry_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("resources.index");
        let entries = vec![make_entry(0x01000001, 0x4, "greeting", "Hello", "en_US")];
        write_resources_index(&entries, &out).unwrap();

        let bytes = std::fs::read(&out).unwrap();
        // Skip 12-byte header
        let mut pos = 12_usize;
        let id = u32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap());
        assert_eq!(id, 0x01000001);
        pos += 4;
        let etype = u32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap());
        assert_eq!(etype, 0x4);
        pos += 4;
        let nlen = u16::from_le_bytes(bytes[pos..pos + 2].try_into().unwrap()) as usize;
        pos += 2;
        let name = std::str::from_utf8(&bytes[pos..pos + nlen]).unwrap();
        assert_eq!(name, "greeting");
        pos += nlen;
        let vlen = u16::from_le_bytes(bytes[pos..pos + 2].try_into().unwrap()) as usize;
        pos += 2;
        let value = std::str::from_utf8(&bytes[pos..pos + vlen]).unwrap();
        assert_eq!(value, "Hello");
        pos += vlen;
        let qlen = u16::from_le_bytes(bytes[pos..pos + 2].try_into().unwrap()) as usize;
        pos += 2;
        let qual = std::str::from_utf8(&bytes[pos..pos + qlen]).unwrap();
        assert_eq!(qual, "en_US");
    }

    #[test]
    fn test_empty_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("resources.index");
        write_resources_index(&[], &out).unwrap();
        let bytes = std::fs::read(&out).unwrap();
        assert_eq!(bytes.len(), 12);
        assert_eq!(&bytes[8..12], &0_u32.to_le_bytes());
    }
}
