use crate::emit::file::{LiteralArray, LiteralEntry};
use crate::emit::record::{FieldRecord, FieldValue, FIELD_OWNER_CLASS_IDX};

pub(super) struct RecordLayout<'a> {
    record_name: &'a str,
    package_name: Option<&'a str>,
    standalone_ecmascript_input: bool,
    source: &'a str,
    is_commonjs: bool,
    is_shared_module: bool,
    has_top_level_await: bool,
}

impl<'a> RecordLayout<'a> {
    pub(super) fn new(
        record_name: &'a str,
        package_name: Option<&'a str>,
        standalone_ecmascript_input: bool,
        source: &'a str,
        is_commonjs: bool,
        is_shared_module: bool,
        has_top_level_await: bool,
    ) -> Self {
        Self {
            record_name,
            package_name,
            standalone_ecmascript_input,
            source,
            is_commonjs,
            is_shared_module,
            has_top_level_await,
        }
    }

    pub(super) fn class_name(&self) -> String {
        if self.record_name.starts_with('L') && self.record_name.ends_with(';') {
            return self.record_name.to_string();
        }
        if self.record_name.is_empty() {
            return "L_GLOBAL;".to_string();
        }
        format!("L{};", self.record_name)
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
        module_record_idx: Option<usize>,
    ) -> Vec<FieldRecord> {
        let module_name = self.package_name.unwrap_or_else(|| {
            if self.is_standalone_ecmascript_input() {
                ""
            } else {
                self.module_name()
            }
        });
        let mut fields = vec![
            module_u8_field(format!("pkgName@{module_name}")),
            module_u8_field_with_value("isCommonjs", u8::from(self.is_commonjs)),
            module_u8_field_with_value("hasTopLevelAwait", u8::from(self.has_top_level_await)),
            module_u8_field_with_value("isSharedModule", u8::from(self.is_shared_module)),
            FieldRecord {
                name: "scopeNames".to_string(),
                class_idx: FIELD_OWNER_CLASS_IDX,
                type_idx: 1,
                access_flags: 0,
                value: Some(FieldValue::LiteralArray(scope_names_idx)),
            },
        ];
        if let Some(module_record_idx) = module_record_idx {
            fields.push(FieldRecord {
                name: "moduleRecordIdx".to_string(),
                class_idx: FIELD_OWNER_CLASS_IDX,
                type_idx: 1,
                access_flags: 0,
                value: Some(FieldValue::LiteralArray(module_record_idx)),
            });
        }
        fields
    }

    pub(super) fn source_code_strings(&self) -> Vec<String> {
        if self.is_standalone_ecmascript_input() {
            return Vec::new();
        }
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
        } else if self.record_name.contains("/ets/pages/") && !self.is_standalone_ecmascript_input()
        {
            indices.sort_by_key(|&idx| (page_literal_rank(&literal_arrays[idx]), idx));
        }
        indices
    }

    fn is_standalone_ecmascript_input(&self) -> bool {
        self.standalone_ecmascript_input
    }
}

pub(super) fn is_explicit_ecmascript_source_file(source_file: &str) -> bool {
    !source_file.contains('|') && (source_file.ends_with(".js") || source_file.ends_with(".ts"))
}

fn module_u8_field(name: impl Into<String>) -> FieldRecord {
    module_u8_field_with_value(name, 0)
}

fn module_u8_field_with_value(name: impl Into<String>, value: u8) -> FieldRecord {
    FieldRecord {
        name: name.into(),
        class_idx: FIELD_OWNER_CLASS_IDX,
        type_idx: 0,
        access_flags: 0,
        value: Some(FieldValue::U8(value)),
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

#[cfg(test)]
mod tests {
    use super::RecordLayout;

    fn layout(record_name: &str) -> RecordLayout<'_> {
        RecordLayout::new(record_name, None, true, "", false, false, false)
    }

    #[test]
    fn class_name_matches_upstream_descriptor_shape() {
        assert_eq!(
            layout("entry.src.main.ets.a").class_name(),
            "Lentry.src.main.ets.a;"
        );
        assert_eq!(layout("test_record").class_name(), "Ltest_record;");
        assert_eq!(
            layout("com.example.app/entry/ets/pages/Index").class_name(),
            "Lcom.example.app/entry/ets/pages/Index;"
        );
    }

    #[test]
    fn class_name_preserves_descriptors_and_global_record() {
        assert_eq!(layout("Lentry/main;").class_name(), "Lentry/main;");
        assert_eq!(layout("").class_name(), "L_GLOBAL;");
    }
}
