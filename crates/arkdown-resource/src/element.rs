use serde_json::Value;
use std::path::Path;

/// A single resource entry parsed from an element JSON file.
#[derive(Debug, Clone)]
pub struct ResourceEntry {
    pub name: String,
    pub entry_type: EntryType,
    pub value: String,
}

/// The four element resource types that OpenHarmony supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryType {
    String,
    Color,
    Float,
    Integer,
}

impl EntryType {
    /// Maps to the resource type ID used in `resources.index`.
    ///
    /// Values mirror OpenHarmony restool `ResType`:
    /// `STRING = 9`, `COLOR = 14`, `FLOAT = 18`, `INTEGER = 8`.
    pub fn resource_type_id(self) -> u32 {
        match self {
            EntryType::String => 0x09,
            EntryType::Color => 0x0E,
            EntryType::Float => 0x12,
            EntryType::Integer => 0x08,
        }
    }
}

/// Parse an element JSON file and return all resource entries it contains.
///
/// The file stem determines the resource type.  Supported filenames:
/// `string.json`, `color.json`, `float.json`, `integer.json`.
///
/// Expected JSON structure (example for `string.json`):
/// ```json
/// { "string": [ { "name": "app_name", "value": "My App" } ] }
/// ```
///
/// Returns an empty `Vec` if the file cannot be read, parsed, or has an
/// unrecognised stem.
pub fn parse_element_file(path: &Path) -> Vec<ResourceEntry> {
    let content = match fs_err::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let v: Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    let type_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

    let entry_type = match type_name {
        "string" => EntryType::String,
        "color" => EntryType::Color,
        "float" => EntryType::Float,
        "integer" => EntryType::Integer,
        _ => return vec![],
    };

    v.get(type_name)
        .and_then(|arr| arr.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let name = item.get("name")?.as_str()?.to_string();
                    let value = item.get("value")?.as_str()?.to_string();
                    Some(ResourceEntry {
                        name,
                        entry_type,
                        value,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_parse_string_json() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("string.json");
        fs::write(
            &path,
            r#"{"string":[{"name":"app_name","value":"My App"},{"name":"ok","value":"OK"}]}"#,
        )
        .unwrap();
        let entries = parse_element_file(&path);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "app_name");
        assert_eq!(entries[0].value, "My App");
        assert_eq!(entries[0].entry_type, EntryType::String);
        assert_eq!(entries[0].entry_type.resource_type_id(), 0x09);
    }

    #[test]
    fn test_parse_color_json() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("color.json");
        fs::write(
            &path,
            r##"{"color":[{"name":"primary","value":"#FF0000"}]}"##,
        )
        .unwrap();
        let entries = parse_element_file(&path);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].entry_type, EntryType::Color);
        assert_eq!(entries[0].entry_type.resource_type_id(), 0x0E);
    }

    #[test]
    fn test_unknown_type_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("theme.json");
        fs::write(&path, r#"{"theme":[]}"#).unwrap();
        assert!(parse_element_file(&path).is_empty());
    }

    #[test]
    fn test_malformed_json_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("string.json");
        fs::write(&path, b"not json").unwrap();
        assert!(parse_element_file(&path).is_empty());
    }
}
