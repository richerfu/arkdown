//! Binary writer for `resources.index`.
//!
//! # Format — restool 6.0 KVDB
//!
//! This implements the KVDB binary format produced by OpenHarmony `restool` 6.0.
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │ Offset 0x00: magic  b"Restool 6.0.0.001\x00"  (18 bytes)              │
//! │ Offset 0x12: null padding to 0x80               (110 bytes)            │
//! ├─────────────────────────────────────────────────────────────────────────┤
//! │ Offset 0x80: file_size      u32 LE   total file length                 │
//! │ Offset 0x84: cfg_count      u32 LE   number of qualifier configs (1)   │
//! │ Offset 0x88: b"KEYS"        [u8; 4]  section tag                       │
//! │ Offset 0x8C: cfg0_offset    u32 LE   absolute offset of first config   │
//! │ Offset 0x90: pad            u32 LE   = 0                               │
//! ├─────────────────────────────────────────────────────────────────────────┤
//! │ Offset 0x94: b"IDSS"        [u8; 4]  ID-section tag                    │
//! │ Offset 0x98: entry_count    u32 LE   number of records                 │
//! │ Offset 0x9C: entries[]      (id: u32 LE, offset: u32 LE) × count       │
//! ├─────────────────────────────────────────────────────────────────────────┤
//! │ Record data — at each entry's absolute offset:                          │
//! │   body_size : u32 LE  (bytes after this field)                         │
//! │   type      : u32 LE  resource type code                               │
//! │   id        : u32 LE  resource numeric ID                              │
//! │   value_len : u16 LE  length of value string incl. null terminator     │
//! │   value     : [u8; value_len]  null-terminated UTF-8                   │
//! │   key_len   : u16 LE  length of name string incl. null terminator      │
//! │   key       : [u8; key_len]    null-terminated UTF-8                   │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! Resource type codes observed in real `restool` output:
//! - 0x09 = string, 0x0E = color, 0x06 = float, 0x05 = integer
//! - 0x13 = media/image, 0x14 = profile

use std::path::Path;
use crate::compiler::CompiledEntry;

/// Magic bytes at offset 0x00: `b"Restool 6.0.0.001\x00"` (18 bytes).
pub const MAGIC: &[u8; 18] = b"Restool 6.0.0.001\x00";

/// Total header size including magic + null padding (128 bytes = 0x80).
pub const HEADER_TOTAL: usize = 0x80;

/// KVDB block size: 5 × u32 at 0x80–0x93.
const KVDB_BLOCK_SIZE: usize = 20;

/// IDSS section header: b"IDSS" (4) + count (4).
const IDSS_HEADER_SIZE: usize = 8;

/// Size of each IDSS entry: id (u32) + offset (u32).
const IDSS_ENTRY_SIZE: usize = 8;

/// Serialise `entries` in the restool 6.0 KVDB binary format and write to `output`.
///
/// All entries are placed under a single qualifier config (the `base`/default qualifier).
pub fn write_resources_index(
    entries: &[CompiledEntry],
    output: &Path,
) -> Result<(), std::io::Error> {
    let count = entries.len();

    // Absolute offset where the first record body begins.
    let records_start: u32 =
        (HEADER_TOTAL + KVDB_BLOCK_SIZE + IDSS_HEADER_SIZE + count * IDSS_ENTRY_SIZE) as u32;

    // Build per-record bytes and collect (id, absolute_offset) pairs for the IDSS table.
    let mut record_blobs: Vec<Vec<u8>> = Vec::with_capacity(count);
    let mut idss_table: Vec<(u32, u32)> = Vec::with_capacity(count);
    let mut current_offset = records_start;

    for entry in entries {
        let val_bytes = entry.value.as_bytes();
        // +1 for null terminator
        let val_len = val_bytes.len() + 1;
        let key_bytes = entry.name.as_bytes();
        let key_len = key_bytes.len() + 1;

        // body_size does NOT include the 4-byte body_size field itself
        let body_size: u32 = (4 + 4 + 2 + val_len + 2 + key_len) as u32;

        let record_total = 4 + body_size as usize;
        let mut rec = Vec::with_capacity(record_total);
        rec.extend_from_slice(&body_size.to_le_bytes());
        rec.extend_from_slice(&entry.entry_type.to_le_bytes());
        rec.extend_from_slice(&entry.id.to_le_bytes());
        rec.extend_from_slice(&(val_len as u16).to_le_bytes());
        rec.extend_from_slice(val_bytes);
        rec.push(0u8); // null terminator for value
        rec.extend_from_slice(&(key_len as u16).to_le_bytes());
        rec.extend_from_slice(key_bytes);
        rec.push(0u8); // null terminator for key

        idss_table.push((entry.id, current_offset));
        current_offset += rec.len() as u32;
        record_blobs.push(rec);
    }

    let file_size = current_offset;

    // Assemble the final buffer.
    let mut buf: Vec<u8> = Vec::with_capacity(file_size as usize);

    // ── 1. Magic + null padding to 0x80 ─────────────────────────────────────
    buf.extend_from_slice(MAGIC);
    buf.resize(HEADER_TOTAL, 0u8);

    // ── 2. KVDB block (0x80–0x93) ───────────────────────────────────────────
    buf.extend_from_slice(&file_size.to_le_bytes());
    buf.extend_from_slice(&1u32.to_le_bytes()); // cfg_count = 1 (base qualifier)
    buf.extend_from_slice(b"KEYS");
    let cfg0_offset = (HEADER_TOTAL + KVDB_BLOCK_SIZE) as u32; // = 0x94
    buf.extend_from_slice(&cfg0_offset.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes()); // end-of-configs marker

    // ── 3. IDSS section (at 0x94) ───────────────────────────────────────────
    buf.extend_from_slice(b"IDSS");
    buf.extend_from_slice(&(count as u32).to_le_bytes());
    for (id, offset) in &idss_table {
        buf.extend_from_slice(&id.to_le_bytes());
        buf.extend_from_slice(&offset.to_le_bytes());
    }

    // ── 4. Record data ───────────────────────────────────────────────────────
    for rec in &record_blobs {
        buf.extend_from_slice(rec);
    }

    debug_assert_eq!(buf.len(), file_size as usize, "file_size mismatch");

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
        let entries = vec![make_entry(0x01000001, 0x9, "app_name", "My App", "")];
        write_resources_index(&entries, &out).unwrap();

        let bytes = std::fs::read(&out).unwrap();
        // Restool 6.0 magic (18 bytes)
        assert_eq!(&bytes[0..18], b"Restool 6.0.0.001\x00");
        // Padding at 0x12 = 0
        assert_eq!(bytes[0x12], 0);
        // file_size at 0x80 should be the actual file length
        let file_size = u32::from_le_bytes(bytes[0x80..0x84].try_into().unwrap()) as usize;
        assert_eq!(file_size, bytes.len());
        // cfg_count at 0x84 = 1
        let cfg_count = u32::from_le_bytes(bytes[0x84..0x88].try_into().unwrap());
        assert_eq!(cfg_count, 1);
        // "KEYS" tag at 0x88
        assert_eq!(&bytes[0x88..0x8C], b"KEYS");
    }

    #[test]
    fn test_write_resources_index_idss_structure() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("resources.index");
        let entries = vec![make_entry(0x01000000, 0x9, "greeting", "Hello", "")];
        write_resources_index(&entries, &out).unwrap();

        let bytes = std::fs::read(&out).unwrap();
        // IDSS tag at 0x94
        assert_eq!(&bytes[0x94..0x98], b"IDSS");
        // count = 1
        let count = u32::from_le_bytes(bytes[0x98..0x9C].try_into().unwrap());
        assert_eq!(count, 1);
        // first IDSS entry: id = 0x01000000
        let id = u32::from_le_bytes(bytes[0x9C..0xA0].try_into().unwrap());
        assert_eq!(id, 0x01000000);
        // offset should point into the record data area
        let offset = u32::from_le_bytes(bytes[0xA0..0xA4].try_into().unwrap()) as usize;
        assert!(offset < bytes.len(), "record offset {offset:#x} must be within file");
    }

    #[test]
    fn test_write_resources_index_record_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("resources.index");
        let entries = vec![make_entry(0x01000001, 0x9, "greeting", "Hello", "")];
        write_resources_index(&entries, &out).unwrap();

        let bytes = std::fs::read(&out).unwrap();

        // Locate the record via the IDSS table offset
        let offset = u32::from_le_bytes(bytes[0xA0..0xA4].try_into().unwrap()) as usize;
        let body_size = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
        assert!(body_size > 0);
        let etype = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap());
        assert_eq!(etype, 0x9);
        let id = u32::from_le_bytes(bytes[offset + 8..offset + 12].try_into().unwrap());
        assert_eq!(id, 0x01000001);

        let val_len = u16::from_le_bytes(bytes[offset + 12..offset + 14].try_into().unwrap()) as usize;
        let val = std::str::from_utf8(&bytes[offset + 14..offset + 14 + val_len - 1]).unwrap();
        assert_eq!(val, "Hello");

        let key_off = offset + 14 + val_len;
        let key_len = u16::from_le_bytes(bytes[key_off..key_off + 2].try_into().unwrap()) as usize;
        let key = std::str::from_utf8(&bytes[key_off + 2..key_off + 2 + key_len - 1]).unwrap();
        assert_eq!(key, "greeting");
    }

    #[test]
    fn test_empty_entries_file_size() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("resources.index");
        write_resources_index(&[], &out).unwrap();
        let bytes = std::fs::read(&out).unwrap();
        // Empty: header (0x80) + KVDB block (20) + IDSS header (8) + 0 entries = 156
        assert_eq!(bytes.len(), HEADER_TOTAL + KVDB_BLOCK_SIZE + IDSS_HEADER_SIZE);
        // count in IDSS = 0
        let count = u32::from_le_bytes(bytes[0x98..0x9C].try_into().unwrap());
        assert_eq!(count, 0);
    }

    #[test]
    fn test_magic_is_restool_6() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("resources.index");
        write_resources_index(&[], &out).unwrap();
        let bytes = std::fs::read(&out).unwrap();
        assert_eq!(&bytes[0..18], b"Restool 6.0.0.001\x00");
    }

    #[test]
    fn test_file_size_field_matches_actual_size() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("resources.index");
        let entries = vec![
            make_entry(0x01000000, 0x9, "app_name", "TestApp", ""),
            make_entry(0x01000001, 0x13, "icon", "media/icon.png", ""),
        ];
        write_resources_index(&entries, &out).unwrap();
        let bytes = std::fs::read(&out).unwrap();
        let file_size = u32::from_le_bytes(bytes[0x80..0x84].try_into().unwrap()) as usize;
        assert_eq!(file_size, bytes.len());
    }
}
