use std::path::Path;

use crate::emit::record::{ClassRecord, FieldRecord, FieldType, FieldValue, FIELD_OWNER_CLASS_IDX};
use crate::error::AbcError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NpmEntry {
    pub record_name: String,
    pub field_name: String,
}

pub fn load_npm_entry_records(path: &Path) -> Result<Vec<ClassRecord>, AbcError> {
    let source = fs_err::read_to_string(path).map_err(AbcError::Io)?;
    Ok(parse_npm_entries(&source)
        .into_iter()
        .map(npm_entry_record)
        .collect())
}

fn parse_npm_entries(source: &str) -> Vec<NpmEntry> {
    source
        .lines()
        .map(|line| {
            let (record_name, field_name) = line.split_once(':').unwrap_or((line, line));
            NpmEntry {
                record_name: record_name.to_string(),
                field_name: field_name.to_string(),
            }
        })
        .collect()
}

fn npm_entry_record(entry: NpmEntry) -> ClassRecord {
    ClassRecord {
        name: pandasm_record_class_name(&entry.record_name),
        methods: vec![],
        string_indices: vec![],
        literal_indices: vec![],
        module_literal_indices: vec![],
        source_code_strings: vec![],
        access_flags: 1,
        source_lang: 0,
        standalone_ecmascript_input: false,
        source_file: None,
        annotations: vec![],
        module_requests: vec![],
        fields: vec![FieldRecord {
            name: entry.field_name,
            class_idx: FIELD_OWNER_CLASS_IDX,
            type_idx: 0,
            field_type: FieldType::U8,
            access_flags: 0,
            value: Some(FieldValue::U8(0)),
            annotations: vec![],
        }],
    }
}

fn pandasm_record_class_name(record_name: &str) -> String {
    if record_name.starts_with('L') && record_name.ends_with(';') {
        record_name.to_string()
    } else {
        format!("L{record_name};")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_lowers_npm_entries_like_es2panda() {
        let records = parse_npm_entries(
            "@normalized:N&entry/src/main/ets/pages/Index&:/pkg_modules/.ohpm/a/index\n",
        )
        .into_iter()
        .map(npm_entry_record)
        .collect::<Vec<_>>();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].name, "L@normalized;");
        assert!(records[0].methods.is_empty());
        assert_eq!(records[0].fields.len(), 1);
        assert_eq!(
            records[0].fields[0].name,
            "N&entry/src/main/ets/pages/Index&:/pkg_modules/.ohpm/a/index"
        );
        assert_eq!(records[0].fields[0].type_idx, 0);
        assert!(matches!(
            records[0].fields[0].value,
            Some(FieldValue::U8(0))
        ));
    }
}
