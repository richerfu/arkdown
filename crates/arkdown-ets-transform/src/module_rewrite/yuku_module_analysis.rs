use std::path::{Path, PathBuf};
use std::ptr::NonNull;

use rustc_hash::{FxHashMap, FxHashSet};

use crate::source::{
    SourceClassifier, SourceFileKind, TransformSourceFile, TransformSourceSnapshot,
};

use super::{ModuleBindingKind, ModuleExportClassifier};

const MAGIC: u32 = 0x414d4b59;
const VERSION: u32 = 1;
const LANG_JS: u32 = 0;
const LANG_TS: u32 = 1;
const LANG_DTS: u32 = 2;
const LANG_ARKUI: u32 = 3;
const STATUS_OK: u32 = 0;
const EDGE_LOCAL: u32 = 0;
const EDGE_REEXPORT_NAMED: u32 = 1;
const EDGE_REEXPORT_STAR: u32 = 2;
const KIND_TYPE: u32 = 0;
const HEADER_U32S: usize = 5;
const IMPORT_ENTRY_U32S: usize = 7;
const EXPORT_ENTRY_U32S: usize = 11;

pub(super) struct YukuModuleExportClassifier<'a> {
    graph: YukuModuleGraphAnalyzer<'a>,
}

impl<'a> YukuModuleExportClassifier<'a> {
    pub(super) fn new(sources: &'a TransformSourceSnapshot) -> Self {
        Self {
            graph: YukuModuleGraphAnalyzer::new(sources),
        }
    }

    pub(super) fn analyze(
        &mut self,
        source: TransformSourceFile,
    ) -> Option<FxHashMap<String, ModuleBindingKind>> {
        self.graph.collect_source_exports(source)
    }
}

struct YukuModuleGraphAnalyzer<'a> {
    sources: &'a TransformSourceSnapshot,
    facts: FxHashMap<PathBuf, Option<YukuModuleFacts>>,
}

impl<'a> YukuModuleGraphAnalyzer<'a> {
    fn new(sources: &'a TransformSourceSnapshot) -> Self {
        Self {
            sources,
            facts: FxHashMap::default(),
        }
    }

    fn collect_source_exports(
        &mut self,
        source: TransformSourceFile,
    ) -> Option<FxHashMap<String, ModuleBindingKind>> {
        let mut visited = FxHashSet::default();
        self.collect_source_exports_with_visited(source, &mut visited)
    }

    fn collect_source_exports_with_visited(
        &mut self,
        source: TransformSourceFile,
        visited: &mut FxHashSet<PathBuf>,
    ) -> Option<FxHashMap<String, ModuleBindingKind>> {
        if SourceClassifier::classify_path(&source.path).is_declaration() {
            return ModuleExportClassifier::new(source)
                .with_sources(self.sources)
                .analyze_with_visited(visited);
        }
        if !visited.insert(source.path.clone()) {
            return None;
        }

        let facts = self.source_facts(&source)?;
        let mut exports = FxHashMap::default();
        for export in facts.exports {
            match export.edge {
                YukuExportEdge::Local => {
                    let kind =
                        self.local_export_kind(&source.path, &facts.imports, &export, visited);
                    ModuleBindingKind::record(&mut exports, export.name, kind);
                }
                YukuExportEdge::ReexportNamed => {
                    let kind = self.reexported_binding_kind(&source.path, &export, visited);
                    ModuleBindingKind::record(&mut exports, export.name, kind);
                }
                YukuExportEdge::ReexportStar => {
                    self.collect_export_star(&source.path, &export, visited, &mut exports);
                }
            }
        }

        visited.remove(&source.path);
        Some(exports)
    }

    fn source_facts(&mut self, source: &TransformSourceFile) -> Option<YukuModuleFacts> {
        if let Some(cached) = self.facts.get(&source.path) {
            return cached.clone();
        }
        let facts = YukuNativeModuleAnalyzer::analyze(source);
        self.facts.insert(source.path.clone(), facts.clone());
        facts
    }

    fn local_export_kind(
        &mut self,
        importer: &Path,
        imports: &FxHashMap<String, YukuImportFact>,
        export: &YukuExportFact,
        visited: &mut FxHashSet<PathBuf>,
    ) -> ModuleBindingKind {
        if export.kind == ModuleBindingKind::Type {
            return ModuleBindingKind::Type;
        }
        if !export.local_is_import {
            return ModuleBindingKind::Value;
        }
        imports
            .get(&export.local)
            .and_then(|import| self.classify_import(importer, import, visited, false))
            .unwrap_or(ModuleBindingKind::Value)
    }

    fn reexported_binding_kind(
        &mut self,
        importer: &Path,
        export: &YukuExportFact,
        visited: &mut FxHashSet<PathBuf>,
    ) -> ModuleBindingKind {
        if export.kind == ModuleBindingKind::Type {
            return ModuleBindingKind::Type;
        }
        let Some(target) = self
            .sources
            .resolve_static_with_declarations(importer, &export.specifier)
        else {
            return ModuleBindingKind::Value;
        };
        self.collect_source_exports_with_visited(target, visited)
            .and_then(|exports| exports.get(&export.from).copied())
            .unwrap_or(ModuleBindingKind::Value)
    }

    fn collect_export_star(
        &mut self,
        importer: &Path,
        export: &YukuExportFact,
        visited: &mut FxHashSet<PathBuf>,
        exports: &mut FxHashMap<String, ModuleBindingKind>,
    ) {
        let Some(target) = self
            .sources
            .resolve_static_with_declarations(importer, &export.specifier)
        else {
            return;
        };
        let Some(target_exports) = self.collect_source_exports_with_visited(target, visited) else {
            return;
        };
        for (name, kind) in target_exports {
            if name == "default" {
                continue;
            }
            let export_kind = if export.kind == ModuleBindingKind::Type {
                ModuleBindingKind::Type
            } else {
                kind
            };
            ModuleBindingKind::record(exports, name, export_kind);
        }
    }

    fn classify_import(
        &mut self,
        importer: &Path,
        import: &YukuImportFact,
        visited: &mut FxHashSet<PathBuf>,
        declaration_fallback: bool,
    ) -> Option<ModuleBindingKind> {
        if import.kind == ModuleBindingKind::Type {
            return Some(ModuleBindingKind::Type);
        }
        if import.imported == "*" {
            return Some(ModuleBindingKind::Value);
        }

        let target = self
            .sources
            .resolve_static_with_declarations(importer, &import.specifier)?;
        let is_declaration = SourceClassifier::classify_path(&target.path).is_declaration();
        let exports = self.collect_source_exports_with_visited(target, visited)?;
        exports.get(&import.imported).copied().or({
            if declaration_fallback && is_declaration {
                Some(ModuleBindingKind::Type)
            } else {
                None
            }
        })
    }
}

#[derive(Clone)]
struct YukuModuleFacts {
    imports: FxHashMap<String, YukuImportFact>,
    exports: Vec<YukuExportFact>,
}

#[derive(Clone)]
struct YukuImportFact {
    imported: String,
    specifier: String,
    kind: ModuleBindingKind,
}

#[derive(Clone)]
struct YukuExportFact {
    edge: YukuExportEdge,
    name: String,
    local: String,
    from: String,
    specifier: String,
    kind: ModuleBindingKind,
    local_is_import: bool,
}

#[derive(Clone, Copy)]
enum YukuExportEdge {
    Local,
    ReexportNamed,
    ReexportStar,
}

struct YukuNativeModuleAnalyzer;

impl YukuNativeModuleAnalyzer {
    fn analyze(source: &TransformSourceFile) -> Option<YukuModuleFacts> {
        let buffer =
            NativeAnalysisBuffer::analyze(source.source.as_bytes(), Self::lang(&source.path))?;
        YukuFactsDecoder::new(buffer.as_slice()).decode()
    }

    fn lang(path: &Path) -> u32 {
        match SourceClassifier::classify_path(path) {
            SourceFileKind::Ets => LANG_ARKUI,
            SourceFileKind::Ts => LANG_TS,
            SourceFileKind::Js | SourceFileKind::Other => LANG_JS,
            SourceFileKind::Declaration => {
                match path.extension().and_then(|extension| extension.to_str()) {
                    Some("ets") => LANG_ARKUI,
                    Some("ts") => LANG_DTS,
                    _ => LANG_DTS,
                }
            }
        }
    }
}

struct YukuFactsDecoder<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> YukuFactsDecoder<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn decode(mut self) -> Option<YukuModuleFacts> {
        let magic = self.read_u32()?;
        let version = self.read_u32()?;
        if magic != MAGIC || version != VERSION {
            return None;
        }

        let import_count = self.read_u32()? as usize;
        let export_count = self.read_u32()? as usize;
        let string_len = self.read_u32()? as usize;
        let entries_len = import_count
            .checked_mul(IMPORT_ENTRY_U32S * 4)?
            .checked_add(export_count.checked_mul(EXPORT_ENTRY_U32S * 4)?)?;
        let strings_start = HEADER_U32S * 4 + entries_len;
        let strings_end = strings_start.checked_add(string_len)?;
        if self.bytes.len() != strings_end {
            return None;
        }
        let strings = &self.bytes[strings_start..strings_end];

        let mut imports = FxHashMap::default();
        for _ in 0..import_count {
            let local = self.read_string(strings)?;
            let imported = self.read_string(strings)?;
            let specifier = self.read_string(strings)?;
            let kind = self.read_kind()?;
            imports.insert(
                local,
                YukuImportFact {
                    imported,
                    specifier,
                    kind,
                },
            );
        }

        let mut exports = Vec::with_capacity(export_count);
        for _ in 0..export_count {
            let edge = self.read_edge()?;
            let kind = self.read_kind()?;
            let local_is_import = self.read_u32()? != 0;
            let name = self.read_string(strings)?;
            let local = self.read_string(strings)?;
            let from = self.read_string(strings)?;
            let specifier = self.read_string(strings)?;
            exports.push(YukuExportFact {
                edge,
                name,
                local,
                from,
                specifier,
                kind,
                local_is_import,
            });
        }

        Some(YukuModuleFacts { imports, exports })
    }

    fn read_edge(&mut self) -> Option<YukuExportEdge> {
        match self.read_u32()? {
            EDGE_LOCAL => Some(YukuExportEdge::Local),
            EDGE_REEXPORT_NAMED => Some(YukuExportEdge::ReexportNamed),
            EDGE_REEXPORT_STAR => Some(YukuExportEdge::ReexportStar),
            _ => None,
        }
    }

    fn read_kind(&mut self) -> Option<ModuleBindingKind> {
        match self.read_u32()? {
            KIND_TYPE => Some(ModuleBindingKind::Type),
            _ => Some(ModuleBindingKind::Value),
        }
    }

    fn read_string(&mut self, strings: &[u8]) -> Option<String> {
        let start = self.read_u32()? as usize;
        let len = self.read_u32()? as usize;
        if len == 0 {
            return Some(String::new());
        }
        let end = start.checked_add(len)?;
        let bytes = strings.get(start..end)?;
        std::str::from_utf8(bytes).ok().map(ToOwned::to_owned)
    }

    fn read_u32(&mut self) -> Option<u32> {
        let bytes = self.bytes.get(self.offset..self.offset.checked_add(4)?)?;
        self.offset += 4;
        Some(u32::from_le_bytes(bytes.try_into().ok()?))
    }
}

struct NativeAnalysisBuffer {
    ptr: NonNull<u8>,
    len: usize,
}

impl NativeAnalysisBuffer {
    fn analyze(source: &[u8], lang: u32) -> Option<Self> {
        let result = unsafe { arkdown_yuku_module_analyze(source.as_ptr(), source.len(), lang) };
        if result.status != STATUS_OK || result.ptr.is_null() || result.len == 0 {
            return None;
        }
        Some(Self {
            ptr: NonNull::new(result.ptr)?,
            len: result.len,
        })
    }

    fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }
}

impl Drop for NativeAnalysisBuffer {
    fn drop(&mut self) {
        unsafe {
            arkdown_yuku_module_analysis_free(self.ptr.as_ptr(), self.len);
        }
    }
}

#[repr(C)]
struct NativeAnalysisResult {
    ptr: *mut u8,
    len: usize,
    status: u32,
}

extern "C" {
    fn arkdown_yuku_module_analyze(
        source_ptr: *const u8,
        source_len: usize,
        lang: u32,
    ) -> NativeAnalysisResult;

    fn arkdown_yuku_module_analysis_free(ptr: *mut u8, len: usize);
}
