//! Text writer for `ResourceTable.txt`.
//!
//! `ResourceTable.txt` is a human-readable index of every resource ID, type,
//! and name.  It is consumed by developer tools and by the compiler for symbol
//! resolution; it is NOT included in the final HAP archive.

use crate::compiler::CompiledEntry;
use std::path::Path;

/// Write a `ResourceTable.txt` file listing every entry in `entries`.
pub fn write_resource_table(
    entries: &[CompiledEntry],
    output: &Path,
) -> Result<(), std::io::Error> {
    let mut sorted = entries.iter().collect::<Vec<_>>();
    sorted.sort_by(|a, b| a.id.cmp(&b.id));
    sorted.dedup_by(|a, b| a.id == b.id);
    sorted.sort_by(|a, b| {
        type_sort_order(a.entry_type)
            .cmp(&type_sort_order(b.entry_type))
            .then_with(|| a.name.cmp(&b.name))
    });

    let mut lines = Vec::with_capacity(sorted.len());
    for e in sorted {
        lines.push(format!(
            "{} {} 0x{:08x}",
            type_name(e.entry_type),
            e.name,
            e.id
        ));
    }
    fs_err::write(output, lines.join("\n"))?;
    Ok(())
}

pub(crate) fn type_name(t: u32) -> &'static str {
    match t {
        0x08 => "integer",
        0x12 => "float",
        0x09 => "string",
        0x0E => "color",
        0x13 => "media",
        0x14 => "profile",
        _ => "unknown",
    }
}

fn type_sort_order(t: u32) -> u8 {
    match t {
        0x09 => 0, // string
        0x0E => 1, // color
        0x12 => 2, // float
        0x08 => 3, // integer
        0x13 => 4, // media
        0x14 => 5, // profile
        _ => 9,
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
            entry_type: 0x09,
            name: "app_name".to_string(),
            value: "My App".to_string(),
            qualifier: String::new(),
        }];
        write_resource_table(&entries, &out).unwrap();
        let content = std::fs::read_to_string(&out).unwrap();
        assert_eq!(content, "string app_name 0x01000001");
    }

    #[test]
    fn test_resource_table_header_only_when_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("ResourceTable.txt");
        write_resource_table(&[], &out).unwrap();
        let content = std::fs::read_to_string(&out).unwrap();
        assert!(content.is_empty());
    }
}
