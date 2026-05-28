//! Binary writer for `resources.index`.
//!
//! # Format — RestoolV2
//!
//! This implements the `RestoolV2` format produced by OpenHarmony's `restool` tool.
//! The authoritative source is:
//! <https://gitcode.com/openharmony/developtools_restool>.
//!
//! ```text
//! Header (18 bytes):
//!   magic    : [u8; 10] = b"RestoolV2 "   (0x52 65 73 74 6F 6F 6C 56 32 20)
//!   version  : u32 LE   = 1
//!   count    : u32 LE   = number of entries
//!
//! Per-entry record:
//!   id           : u32 LE   — e.g. 0x0100_0001
//!   entry_type   : u32 LE   — 0x1=media, 0x4=string, 0x5=integer, 0x6=float, 0xA=color
//!   config       : u32 LE   — qualifier config; 0 = base/default
//!   name_len     : u16 LE
//!   name         : name_len bytes (UTF-8)
//!   value_len    : u16 LE
//!   value        : value_len bytes (UTF-8)
//! ```

use std::path::Path;
use crate::compiler::CompiledEntry;

/// Magic bytes written at offset 0 of `resources.index` (RestoolV2 format).
pub const MAGIC: &[u8; 10] = b"RestoolV2 ";
/// Format version written immediately after the magic.
pub const VERSION: u32 = 0x0000_0001;
/// Total header size in bytes: 10 (magic) + 4 (version) + 4 (count).
pub const HEADER_SIZE: usize = 18;

/// Serialise `entries` in RestoolV2 format and write the result to `output`.
pub fn write_resources_index(
    entries: &[CompiledEntry],
    output: &Path,
) -> Result<(), std::io::Error> {
    let mut buf: Vec<u8> = Vec::with_capacity(HEADER_SIZE + entries.len() * 64);

    // Header: 10-byte magic + version u32 + count u32
    buf.extend_from_slice(MAGIC);
    buf.extend_from_slice(&VERSION.to_le_bytes());
    buf.extend_from_slice(&(entries.len() as u32).to_le_bytes());

    // Entries
    for entry in entries {
        buf.extend_from_slice(&entry.id.to_le_bytes());
        buf.extend_from_slice(&entry.entry_type.to_le_bytes());
        // qualifier config: 0 for the base/default qualifier
        let config: u32 = qualifier_to_config(&entry.qualifier);
        buf.extend_from_slice(&config.to_le_bytes());

        let name_bytes = entry.name.as_bytes();
        buf.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        buf.extend_from_slice(name_bytes);

        let value_bytes = entry.value.as_bytes();
        buf.extend_from_slice(&(value_bytes.len() as u16).to_le_bytes());
        buf.extend_from_slice(value_bytes);
    }

    fs_err::write(output, &buf)?;
    Ok(())
}

/// Map a qualifier string to its u32 config value.
/// The base qualifier (empty string) maps to 0.
/// Non-base qualifiers are not yet assigned specific values in this implementation;
/// they also map to 0 (safe default).
fn qualifier_to_config(qualifier: &str) -> u32 {
    if qualifier.is_empty() {
        0
    } else {
        // Non-base qualifiers: encode as 0 for now.
        // A full implementation would compute the qualifier bitmask from the
        // language/region/screen-size tokens (see ResourceUtil.cpp in restool).
        0
    }
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
        // RestoolV2 magic (10 bytes)
        assert_eq!(&bytes[0..10], b"RestoolV2 ");
        // version u32 LE = 1
        assert_eq!(&bytes[10..14], &1_u32.to_le_bytes());
        // count u32 LE = 1
        assert_eq!(&bytes[14..18], &1_u32.to_le_bytes());
        // Total file size >= HEADER_SIZE
        assert!(bytes.len() >= HEADER_SIZE);
    }

    #[test]
    fn test_write_resources_index_entry_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("resources.index");
        let entries = vec![make_entry(0x01000001, 0x4, "greeting", "Hello", "")];
        write_resources_index(&entries, &out).unwrap();

        let bytes = std::fs::read(&out).unwrap();
        // Skip 18-byte header
        let mut pos = HEADER_SIZE;
        let id = u32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap());
        assert_eq!(id, 0x01000001);
        pos += 4;
        let etype = u32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap());
        assert_eq!(etype, 0x4);
        pos += 4;
        // config (qualifier) = 0 for base
        let config = u32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap());
        assert_eq!(config, 0);
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
    }

    #[test]
    fn test_empty_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("resources.index");
        write_resources_index(&[], &out).unwrap();
        let bytes = std::fs::read(&out).unwrap();
        // Empty: only the 18-byte header
        assert_eq!(bytes.len(), HEADER_SIZE);
        // count = 0
        assert_eq!(&bytes[14..18], &0_u32.to_le_bytes());
    }

    #[test]
    fn test_magic_is_restool_v2() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("resources.index");
        write_resources_index(&[], &out).unwrap();
        let bytes = std::fs::read(&out).unwrap();
        assert_eq!(&bytes[0..10], b"RestoolV2 ");
    }
}
