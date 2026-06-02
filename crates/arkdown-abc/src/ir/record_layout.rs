use crate::emit::file::{LiteralArray, LiteralEntry};
use crate::emit::record::{FieldRecord, FieldValue, FIELD_OWNER_CLASS_IDX};

pub(super) struct RecordLayout<'a> {
    record_name: &'a str,
    source: &'a str,
}

impl<'a> RecordLayout<'a> {
    pub(super) fn new(record_name: &'a str, source: &'a str) -> Self {
        Self {
            record_name,
            source,
        }
    }

    pub(super) fn class_name(&self) -> String {
        // ArkRuntime finds modules by their OHM URL class name format:
        //   "L{bundleName}/{moduleName}/{pathFromEts};"
        if self.record_name.contains('/') {
            format!("L{};", self.record_name)
        } else {
            format!("L_{};", self.record_name)
        }
    }

    pub(super) fn module_name(&self) -> &str {
        self.record_name
            .split('/')
            .nth(1)
            .filter(|name| !name.is_empty())
            .unwrap_or("entry")
    }

    pub(super) fn module_fields(
        &self,
        scope_names_idx: usize,
        module_record_idx: usize,
    ) -> Vec<FieldRecord> {
        let module_name = self.module_name();
        vec![
            module_u8_field(format!("pkgName@{module_name}")),
            module_u8_field("isCommonjs"),
            module_u8_field("hasTopLevelAwait"),
            module_u8_field("isSharedModule"),
            FieldRecord {
                name: "scopeNames".to_string(),
                class_idx: FIELD_OWNER_CLASS_IDX,
                type_idx: 1,
                access_flags: 0,
                value: Some(FieldValue::LiteralArray(scope_names_idx)),
            },
            FieldRecord {
                name: "moduleRecordIdx".to_string(),
                class_idx: FIELD_OWNER_CLASS_IDX,
                type_idx: 1,
                access_flags: 0,
                value: Some(FieldValue::LiteralArray(module_record_idx)),
            },
        ]
    }

    pub(super) fn source_code_strings(&self) -> Vec<String> {
        let class_short = self
            .record_name
            .rsplit('/')
            .next()
            .unwrap_or(self.record_name);
        if self.record_name.contains("/ets/pages/") {
            return vec![format!("#{class_short}_Params")];
        }
        let params_type = format!("{class_short}_Params");
        if self.source.contains(&format!("interface {params_type}")) {
            return vec![format!("#{params_type}")];
        }
        if self.record_name.contains("/entryability/") {
            return vec![class_short.to_string(), "not supported".to_string()];
        }
        Vec::new()
    }

    pub(super) fn literal_indices(
        &self,
        mut indices: Vec<usize>,
        literal_arrays: &[LiteralArray],
    ) -> Vec<usize> {
        if self.record_name.contains("/entryability/") {
            indices.sort_by_key(|&idx| {
                let array = &literal_arrays[idx];
                (!array.has_method_affiliate(), idx)
            });
        } else if self.record_name.contains("/ets/pages/") {
            indices.sort_by_key(|&idx| (page_literal_rank(&literal_arrays[idx]), idx));
        }
        indices
    }
}

fn module_u8_field(name: impl Into<String>) -> FieldRecord {
    FieldRecord {
        name: name.into(),
        class_idx: FIELD_OWNER_CLASS_IDX,
        type_idx: 0,
        access_flags: 0,
        value: Some(FieldValue::U8(0)),
    }
}

fn page_literal_rank(array: &LiteralArray) -> u8 {
    if matches!(
        array.entries.as_slice(),
        [
            LiteralEntry::TaggedInteger(1),
            LiteralEntry::TaggedString(name),
            ..
        ] if name == "this"
    ) {
        return 0;
    }
    if array.has_method_affiliate() {
        return 1;
    }
    if array
        .entries
        .iter()
        .all(|entry| matches!(entry, LiteralEntry::TaggedString(_)))
    {
        return 2;
    }
    3
}
