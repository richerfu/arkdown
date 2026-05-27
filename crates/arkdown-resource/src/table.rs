//! Text writer for `ResourceTable.txt`.
//!
//! `ResourceTable.txt` is a human-readable index of every resource ID, type,
//! and name.  It is consumed by developer tools and by the compiler for symbol
//! resolution; it is NOT included in the final HAP archive.

use std::path::Path;
use crate::compiler::CompiledEntry;

/// Write a `ResourceTable.txt` file listing every entry in `entries`.
pub fn write_resource_table(
    entries: &[CompiledEntry],
    output: &Path,
) -> Result<(), std::io::Error> {
    let mut lines = Vec::with_capacity(entries.len() + 1);
    lines.push("ResourceTable".to_string());
    for e in entries {
        lines.push(format!(
            "  entry: 0x{:08X}  {}  {}",
            e.id,
            type_name(e.entry_type),
            e.name
        ));
    }
    fs_err::write(output, lines.join("\n"))?;
    Ok(())
}

fn type_name(t: u32) -> &'static str {
    match t {
        0x1 => "media",
        0x4 => "string",
        0x5 => "integer",
        0x6 => "float",
        0xA => "color",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::CompiledEntry;

    #[test]
    fn test_resource_table_contains_name() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("ResourceTable.txt");
        let entries = vec![CompiledEntry {
            id: 0x01000001,
            entry_type: 0x4,
            name: "app_name".to_string(),
            value: "My App".to_string(),
            qualifier: String::new(),
        }];
        write_resource_table(&entries, &out).unwrap();
        let content = std::fs::read_to_string(&out).unwrap();
        assert!(content.starts_with("ResourceTable"));
        assert!(content.contains("app_name"));
        assert!(content.contains("0x01000001"));
        assert!(content.contains("string"));
    }

    #[test]
    fn test_resource_table_header_only_when_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("ResourceTable.txt");
        write_resource_table(&[], &out).unwrap();
        let content = std::fs::read_to_string(&out).unwrap();
        assert_eq!(content.trim(), "ResourceTable");
    }
}
