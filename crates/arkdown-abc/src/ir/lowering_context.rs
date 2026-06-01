use std::collections::HashMap;

use crate::ir::module_record::ModuleRecordInfo;

#[derive(Debug, Default)]
pub(super) struct LoweringContext {
    import_indices: HashMap<String, u8>,
    export_indices: HashMap<String, u8>,
    lexical_indices: HashMap<String, u8>,
    pub(super) lexical_names: Vec<String>,
}

impl LoweringContext {
    pub(super) fn from_module_record(
        module_record: &ModuleRecordInfo,
        lexical_names: Vec<String>,
    ) -> Self {
        let mut import_indices: HashMap<String, u8> = module_record
            .regular_imports
            .iter()
            .filter_map(|import| {
                u8::try_from(import.module_request_idx)
                    .ok()
                    .map(|idx| (import.local_name.clone(), idx))
            })
            .collect();
        for ns in &module_record.namespace_imports {
            if let Ok(idx) = u8::try_from(ns.module_request_idx) {
                import_indices.entry(ns.local_name.clone()).or_insert(idx);
            }
        }
        let export_indices = module_record
            .local_exports
            .iter()
            .enumerate()
            .filter_map(|(idx, export)| {
                u8::try_from(idx)
                    .ok()
                    .map(|idx| (export.local_name.clone(), idx))
            })
            .collect();
        let lexical_indices = lexical_names
            .iter()
            .enumerate()
            .filter_map(|(idx, name)| u8::try_from(idx).ok().map(|idx| (name.clone(), idx)))
            .collect();
        Self {
            import_indices,
            export_indices,
            lexical_indices,
            lexical_names,
        }
    }

    pub(super) fn import_index(&self, name: &str) -> Option<u8> {
        self.import_indices.get(name).copied()
    }

    pub(super) fn export_index(&self, name: &str) -> Option<u8> {
        self.export_indices.get(name).copied()
    }

    pub(super) fn lexical_index(&self, name: &str) -> Option<u8> {
        self.lexical_indices.get(name).copied()
    }
}
