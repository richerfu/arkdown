use rustc_hash::FxHashMap;

use crate::ir::class_layout::ClassStaticInitializationMode;
use crate::ir::function_lexical::{
    FunctionLexicalBinding, FunctionLexicalBindingKind, FunctionLexicalInfo,
};
use crate::ir::module_lexical::ScopedClassLexicalLayout;
use crate::ir::module_record::ModuleRecordInfo;

const DEFAULT_TARGET_API_VERSION: u32 = 12;
const DEFAULT_TARGET_API_SUB_VERSION: &str = "beta1";
const DEFAULT_API_SUB_VERSION_1: &str = "beta1";
const DEFAULT_API_SUB_VERSION_2: &str = "beta2";

pub(super) fn is_default_api_version_for(
    target_api_version: u32,
    target_api_sub_version: Option<&str>,
) -> bool {
    let sub_api = target_api_sub_version.unwrap_or(DEFAULT_TARGET_API_SUB_VERSION);
    target_api_version < DEFAULT_TARGET_API_VERSION
        || (target_api_version == DEFAULT_TARGET_API_VERSION
            && (sub_api == DEFAULT_API_SUB_VERSION_1 || sub_api == DEFAULT_API_SUB_VERSION_2))
}

#[derive(Debug, Default)]
pub(super) struct LoweringContext {
    import_indices: FxHashMap<String, u8>,
    export_indices: FxHashMap<String, u8>,
    function_names_by_span: FxHashMap<(u32, u32), String>,
    default_constructor_names_by_class_span: FxHashMap<(u32, u32), String>,
    static_initializer_names_by_class_span: FxHashMap<(u32, u32), String>,
    function_lexical_infos: FxHashMap<(u32, u32), FunctionLexicalInfo>,
    scoped_class_lexical_layout: ScopedClassLexicalLayout,
    target_api_version: u32,
    target_api_sub_version: Option<String>,
    use_define_semantic: bool,
    static_initialization_mode: ClassStaticInitializationMode,
    emit_debugger: bool,
    standalone_ecmascript_input: bool,
    pub(super) module_lexical_names: Vec<String>,
}

impl LoweringContext {
    pub(super) fn from_module_record(
        module_record: &ModuleRecordInfo,
        module_lexical_names: Vec<String>,
        function_names_by_span: FxHashMap<(u32, u32), String>,
        default_constructor_names_by_class_span: FxHashMap<(u32, u32), String>,
        static_initializer_names_by_class_span: FxHashMap<(u32, u32), String>,
        function_lexical_infos: FxHashMap<(u32, u32), FunctionLexicalInfo>,
        scoped_class_lexical_layout: ScopedClassLexicalLayout,
        target_api_version: u32,
        target_api_sub_version: Option<String>,
        use_define_semantic: bool,
        static_initialization_mode: ClassStaticInitializationMode,
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
            default_constructor_names_by_class_span,
            static_initializer_names_by_class_span,
            function_lexical_infos,
            scoped_class_lexical_layout,
            target_api_version,
            target_api_sub_version,
            use_define_semantic,
            static_initialization_mode,
            emit_debugger,
            standalone_ecmascript_input,
            module_lexical_names,
        }
    }

    pub(super) fn is_default_api_version(&self) -> bool {
        is_default_api_version_for(
            self.target_api_version,
            self.target_api_sub_version.as_deref(),
        )
    }

    pub(super) fn target_api_version(&self) -> u32 {
        self.target_api_version
    }

    pub(super) fn use_define_semantic(&self) -> bool {
        self.use_define_semantic
    }

    pub(super) fn static_initialization_mode(&self) -> ClassStaticInitializationMode {
        self.static_initialization_mode
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

    pub(super) fn default_constructor_name(&self, start: u32, end: u32) -> Option<&str> {
        self.default_constructor_names_by_class_span
            .get(&(start, end))
            .map(String::as_str)
    }

    pub(super) fn static_initializer_name(&self, start: u32, end: u32) -> Option<&str> {
        self.static_initializer_names_by_class_span
            .get(&(start, end))
            .map(String::as_str)
    }

    pub(super) fn function_lexical_bindings(&self, span: (u32, u32)) -> &[FunctionLexicalBinding] {
        self.function_lexical_infos
            .get(&span)
            .map_or(&[], |info| info.bindings.as_slice())
    }

    pub(super) fn class_lexical_names(&self, class_name: &str) -> &[String] {
        self.scoped_class_lexical_layout.class_names(class_name)
    }

    pub(super) fn outer_lexical_envs(&self, span: (u32, u32)) -> Vec<Vec<FunctionLexicalBinding>> {
        let mut envs = self
            .function_lexical_infos
            .get(&span)
            .map(|info| {
                info.parent_spans
                    .iter()
                    .filter_map(|parent| {
                        let bindings = self.function_lexical_bindings(*parent);
                        (!bindings.is_empty()).then(|| bindings.to_vec())
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if let Some(names) = self
            .scoped_class_lexical_layout
            .function_class_names(span)
            .filter(|names| !names.is_empty())
        {
            envs.push(lexical_bindings_from_names(names));
        }
        if !self.module_lexical_names.is_empty() {
            envs.push(lexical_bindings_from_names(&self.module_lexical_names));
        }
        envs
    }
}

fn lexical_bindings_from_names(names: &[String]) -> Vec<FunctionLexicalBinding> {
    names
        .iter()
        .map(|name| FunctionLexicalBinding {
            name: name.clone(),
            kind: FunctionLexicalBindingKind::Lexical,
        })
        .collect()
}
