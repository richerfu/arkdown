use rustc_hash::FxHashMap;

use crate::ir::function_lexical::FunctionLexicalInfo;
use crate::ir::module_record::ModuleRecordInfo;

const DEFAULT_TARGET_API_VERSION: u32 = 12;
const DEFAULT_TARGET_API_SUB_VERSION: &str = "beta1";
const DEFAULT_API_SUB_VERSION_1: &str = "beta1";
const DEFAULT_API_SUB_VERSION_2: &str = "beta2";

#[derive(Debug, Default)]
pub(super) struct LoweringContext {
    import_indices: FxHashMap<String, u8>,
    export_indices: FxHashMap<String, u8>,
    function_names_by_span: FxHashMap<(u32, u32), String>,
    function_lexical_infos: FxHashMap<(u32, u32), FunctionLexicalInfo>,
    target_api_version: u32,
    target_api_sub_version: Option<String>,
    emit_debugger: bool,
    standalone_ecmascript_input: bool,
    pub(super) module_lexical_names: Vec<String>,
}

impl LoweringContext {
    pub(super) fn from_module_record(
        module_record: &ModuleRecordInfo,
        module_lexical_names: Vec<String>,
        function_names_by_span: FxHashMap<(u32, u32), String>,
        function_lexical_infos: FxHashMap<(u32, u32), FunctionLexicalInfo>,
        target_api_version: u32,
        target_api_sub_version: Option<String>,
        emit_debugger: bool,
        standalone_ecmascript_input: bool,
    ) -> Self {
        let regular_imports = module_record.regular_imports_in_external_order();
        let mut import_indices: FxHashMap<String, u8> = regular_imports
            .iter()
            .enumerate()
            .filter_map(|(idx, import)| {
                u8::try_from(idx)
                    .ok()
                    .map(|idx| (import.local_name.clone(), idx))
            })
            .collect();
        let namespace_base = regular_imports.len();
        for (idx, ns) in module_record
            .namespace_imports_in_external_order()
            .into_iter()
            .enumerate()
        {
            if let Ok(idx) = u8::try_from(namespace_base + idx) {
                import_indices.entry(ns.local_name.clone()).or_insert(idx);
            }
        }
        let export_indices = module_record
            .local_exports_in_external_order()
            .into_iter()
            .enumerate()
            .filter_map(|(idx, export)| {
                u8::try_from(idx)
                    .ok()
                    .map(|idx| (export.local_name.clone(), idx))
            })
            .collect();
        Self {
            import_indices,
            export_indices,
            function_names_by_span,
            function_lexical_infos,
            target_api_version,
            target_api_sub_version,
            emit_debugger,
            standalone_ecmascript_input,
            module_lexical_names,
        }
    }

    pub(super) fn is_default_api_version(&self) -> bool {
        let sub_api = self
            .target_api_sub_version
            .as_deref()
            .unwrap_or(DEFAULT_TARGET_API_SUB_VERSION);
        self.target_api_version < DEFAULT_TARGET_API_VERSION
            || (self.target_api_version == DEFAULT_TARGET_API_VERSION
                && (sub_api == DEFAULT_API_SUB_VERSION_1 || sub_api == DEFAULT_API_SUB_VERSION_2))
    }

    pub(super) fn target_api_version(&self) -> u32 {
        self.target_api_version
    }

    pub(super) fn emit_debugger(&self) -> bool {
        self.emit_debugger
    }

    pub(super) fn standalone_ecmascript_input(&self) -> bool {
        self.standalone_ecmascript_input
    }

    pub(super) fn import_index(&self, name: &str) -> Option<u8> {
        self.import_indices.get(name).copied()
    }

    pub(super) fn export_index(&self, name: &str) -> Option<u8> {
        self.export_indices.get(name).copied()
    }

    pub(super) fn function_name(&self, start: u32, end: u32) -> Option<&str> {
        self.function_names_by_span
            .get(&(start, end))
            .map(String::as_str)
    }

    pub(super) fn function_lexical_names(&self, span: (u32, u32)) -> &[String] {
        self.function_lexical_infos
            .get(&span)
            .map_or(&[], |info| info.names.as_slice())
    }

    pub(super) fn outer_lexical_envs(&self, span: (u32, u32)) -> Vec<Vec<String>> {
        let mut envs = self
            .function_lexical_infos
            .get(&span)
            .map(|info| {
                info.parent_spans
                    .iter()
                    .filter_map(|parent| {
                        let names = self.function_lexical_names(*parent);
                        (!names.is_empty()).then(|| names.to_vec())
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if !self.module_lexical_names.is_empty() {
            envs.push(self.module_lexical_names.clone());
        }
        envs
    }
}
