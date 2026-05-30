use std::path::PathBuf;

use index_vec::IndexVec;
use rustc_hash::FxHashMap;

use crate::module::{ModuleSource, OxcAstRef, ResolvedId};
use crate::plugin::ImportKind;

/// Opaque index into [`ModuleGraph::modules`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ModuleIdx(pub u32);

impl index_vec::Idx for ModuleIdx {
    fn from_usize(n: usize) -> Self {
        ModuleIdx(n as u32)
    }
    fn index(self) -> usize {
        self.0 as usize
    }
}

/// A fully parsed and transformed module in the build graph.
#[derive(Debug, Clone)]
pub struct NormalModule {
    /// This module's position in the graph's `IndexVec`.
    pub idx: ModuleIdx,
    /// Execution order assigned during linking (topological sort).
    pub exec_order: u32,
    pub id: ResolvedId,
    pub source: ModuleSource,
    /// Parsed abstract syntax tree (oxc AST reference).
    pub ast: OxcAstRef,
    /// Static import edges (resolved).
    pub imports: Vec<ImportEdge>,
    /// Dynamic import edges (resolved).
    pub dynamic_imports: Vec<ImportEdge>,
    /// Exported bindings.
    pub exports: Vec<ExportBinding>,
    /// OHOS-specific metadata collected by the module-meta plugin equivalent.
    pub meta: ModuleMeta,
    /// Whether this module is an entry point.
    pub is_entry: bool,
    /// Whether this module has side effects (used for tree-shaking).
    pub has_side_effects: bool,
}

/// A module that is referenced but not bundled (e.g. `@bundle:` / `@shared:` URLs).
#[derive(Debug, Clone)]
pub struct ExternalModule {
    pub idx: ModuleIdx,
    pub id: ResolvedId,
    /// Execution order (external modules are always last).
    pub exec_order: u32,
}

/// A module in the build graph — either a normal (bundled) module or an
/// external reference that is resolved but not included in the output.
#[derive(Debug, Clone)]
pub enum Module {
    Normal(Box<NormalModule>),
    External(Box<ExternalModule>),
}

impl Module {
    /// The resolved identifier of this module.
    pub fn id(&self) -> &ResolvedId {
        match self {
            Module::Normal(m) => &m.id,
            Module::External(m) => &m.id,
        }
    }

    /// The graph index of this module.
    pub fn idx(&self) -> ModuleIdx {
        match self {
            Module::Normal(m) => m.idx,
            Module::External(m) => m.idx,
        }
    }

    /// Whether this is an external module.
    pub fn is_external(&self) -> bool {
        matches!(self, Module::External(_))
    }

    /// Borrow the inner [`NormalModule`], or `None` if this is external.
    pub fn as_normal(&self) -> Option<&NormalModule> {
        if let Module::Normal(m) = self {
            Some(m)
        } else {
            None
        }
    }

    /// Mutably borrow the inner [`NormalModule`], or `None` if this is external.
    pub fn as_normal_mut(&mut self) -> Option<&mut NormalModule> {
        if let Module::Normal(m) = self {
            Some(m)
        } else {
            None
        }
    }

    // ── Convenience delegations to NormalModule ──────────────────────────────

    /// Static import edges, or empty slice for external modules.
    pub fn imports(&self) -> &[ImportEdge] {
        self.as_normal().map_or(&[], |m| &m.imports)
    }

    /// Whether this is an entry module (`false` for external modules).
    pub fn is_entry(&self) -> bool {
        self.as_normal().is_some_and(|m| m.is_entry)
    }

    /// Whether this module has observable side effects.
    /// External modules are conservatively treated as side-effect-free.
    pub fn has_side_effects(&self) -> bool {
        self.as_normal().is_some_and(|m| m.has_side_effects)
    }
}

/// OHOS-specific per-module metadata.
#[derive(Debug, Clone, Default)]
pub struct ModuleMeta {
    /// ohpm package name (e.g., `"my-har"`, `"@ohos/lib"`).
    pub pkg_name: Option<String>,
    /// ohpm package version.
    pub pkg_version: Option<String>,
    /// OHOS module name from `module.json5` (e.g., `"entry"`, `"hsp-lib"`).
    pub module_name: Option<String>,
    /// Absolute path to the module's `oh-package.json5` directory.
    pub pkg_path: Option<PathBuf>,
    /// Whether this is a local (non-ohpm) dependency.
    pub is_local_dependency: bool,
    /// Whether this file is the main entry file of its package.
    pub is_node_entry_file: bool,
    /// Modules that import this module (for hot-reload tracking).
    pub host_modules_info: Vec<HostModuleInfo>,
    /// Whether this module is tagged as needing preloaded SO.
    pub need_preload_so: bool,
    /// Path to the project root containing this module.
    pub belong_project_path: Option<PathBuf>,
    /// Path to the OHOS module root containing this module.
    pub belong_module_path: Option<PathBuf>,
}

/// A reverse-dependency entry (who imports this module).
#[derive(Debug, Clone)]
pub struct HostModuleInfo {
    /// Package name of the importing module.
    pub host_dependency_name: String,
    /// OHOS module name of the importing module.
    pub host_module_name: String,
}

/// A resolved import edge between modules.
#[derive(Debug, Clone)]
pub struct ImportEdge {
    /// The raw specifier as written in source (`"./utils/helper"`, `"@ohos/hilog"`).
    pub specifier: String,
    /// Resolved target module ID.
    pub resolved: ResolvedId,
    /// Imported binding names (empty for namespace/side-effect imports).
    pub imported_names: Vec<ImportedName>,
    /// The kind of import (static, dynamic, require, etc.).
    pub kind: ImportKind,
}

/// A named binding brought in by an import.
#[derive(Debug, Clone)]
pub struct ImportedName {
    /// The name as exported by the source module.
    pub imported: String,
    /// The local alias used in the importing module.
    pub local: String,
}

/// An exported binding from a module.
#[derive(Debug, Clone)]
pub struct ExportBinding {
    pub exported_name: String,
    pub local_name: String,
}

/// The full module graph for one OHOS module build.
#[derive(Debug, Clone)]
pub struct ModuleGraph {
    /// All modules, indexed by [`ModuleIdx`].
    pub modules: IndexVec<ModuleIdx, Module>,
    /// Map from `ResolvedId.id` → [`ModuleIdx`].
    pub id_to_idx: FxHashMap<String, ModuleIdx>,
    /// Entry modules (from `loader.json` `compileEntry`).
    pub entries: Vec<ModuleIdx>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::{
        ExportBinding, ExternalModule, ImportEdge, ImportedName, Module, ModuleIdx, ModuleMeta,
        NormalModule,
    };
    use crate::module::{ModuleSource, ResolvedId};
    use crate::plugin::ImportKind;
    use index_vec::Idx as _;

    fn make_normal_module(idx: u32) -> NormalModule {
        NormalModule {
            idx: ModuleIdx(idx),
            exec_order: idx,
            id: ResolvedId::default(),
            source: ModuleSource {
                id: ResolvedId::default(),
                content: String::new(),
                source_map: None,
            },
            ast: (),
            imports: vec![],
            dynamic_imports: vec![],
            exports: vec![],
            meta: ModuleMeta::default(),
            is_entry: false,
            has_side_effects: false,
        }
    }

    fn make_external_module(idx: u32) -> ExternalModule {
        ExternalModule {
            idx: ModuleIdx(idx),
            id: ResolvedId::default(),
            exec_order: u32::MAX,
        }
    }

    // ── NormalModule ─────────────────────────────────────────────────────────

    #[test]
    fn test_normal_module_construction() {
        let m = make_normal_module(0);
        assert_eq!(m.idx, ModuleIdx(0));
        assert_eq!(m.exec_order, 0);
        assert!(!m.is_entry);
        assert!(!m.has_side_effects);
        assert!(m.imports.is_empty());
        assert!(m.exports.is_empty());
    }

    #[test]
    fn test_normal_module_with_entry_flag() {
        let mut m = make_normal_module(1);
        m.is_entry = true;
        assert!(m.is_entry);
        assert_eq!(m.idx.index(), 1);
    }

    // ── ExternalModule ───────────────────────────────────────────────────────

    #[test]
    fn test_external_module_exec_order_is_max() {
        let ext = make_external_module(5);
        assert_eq!(ext.exec_order, u32::MAX);
        assert_eq!(ext.idx, ModuleIdx(5));
    }

    // ── Module enum — Normal variant ─────────────────────────────────────────

    #[test]
    fn test_module_normal_id_idx_is_not_external() {
        let inner = make_normal_module(0);
        let m = Module::Normal(Box::new(inner));
        // id() and idx() delegate correctly
        assert_eq!(m.idx(), ModuleIdx(0));
        assert!(!m.is_external());
    }

    #[test]
    fn test_module_normal_as_normal_returns_some() {
        let m = Module::Normal(Box::new(make_normal_module(0)));
        assert!(m.as_normal().is_some());
    }

    #[test]
    fn test_module_normal_is_entry_delegates() {
        let mut inner = make_normal_module(0);
        inner.is_entry = true;
        let m = Module::Normal(Box::new(inner));
        assert!(m.is_entry());
    }

    #[test]
    fn test_module_normal_imports_slice() {
        let m = Module::Normal(Box::new(make_normal_module(0)));
        assert_eq!(m.imports().len(), 0);
    }

    // ── Module enum — External variant ──────────────────────────────────────

    #[test]
    fn test_module_external_is_external() {
        let m = Module::External(Box::new(make_external_module(2)));
        assert!(m.is_external());
        assert_eq!(m.idx(), ModuleIdx(2));
    }

    #[test]
    fn test_module_external_as_normal_returns_none() {
        let m = Module::External(Box::new(make_external_module(0)));
        assert!(m.as_normal().is_none());
    }

    #[test]
    fn test_module_external_is_not_entry() {
        let m = Module::External(Box::new(make_external_module(0)));
        assert!(!m.is_entry());
    }

    #[test]
    fn test_module_external_imports_is_empty_slice() {
        let m = Module::External(Box::new(make_external_module(0)));
        assert_eq!(m.imports().len(), 0);
    }

    // ── ImportEdge ───────────────────────────────────────────────────────────

    #[test]
    fn test_import_edge_dynamic_import_kind() {
        let edge = ImportEdge {
            specifier: "./lazy-page".to_string(),
            resolved: ResolvedId::default(),
            imported_names: vec![],
            kind: ImportKind::DynamicImport,
        };
        assert_eq!(edge.kind, ImportKind::DynamicImport);
        assert_eq!(edge.specifier, "./lazy-page");
    }

    #[test]
    fn test_import_edge_static_import_with_names() {
        let edge = ImportEdge {
            specifier: "@ohos/hilog".to_string(),
            resolved: ResolvedId::default(),
            imported_names: vec![ImportedName {
                imported: "hilog".to_string(),
                local: "hilog".to_string(),
            }],
            kind: ImportKind::Import,
        };
        assert_eq!(edge.imported_names.len(), 1);
        assert_eq!(edge.imported_names[0].imported, "hilog");
    }

    // ── ExportBinding ────────────────────────────────────────────────────────

    #[test]
    fn test_export_binding_construction() {
        let eb = ExportBinding {
            exported_name: "default".to_string(),
            local_name: "_default".to_string(),
        };
        assert_eq!(eb.exported_name, "default");
        assert_eq!(eb.local_name, "_default");
    }

    // ── ModuleIdx ────────────────────────────────────────────────────────────

    #[test]
    fn test_module_idx_from_usize_round_trip() {
        let idx = ModuleIdx::from_usize(42);
        assert_eq!(idx.index(), 42);
        assert_eq!(idx, ModuleIdx(42));
    }

    #[test]
    fn test_module_idx_default_is_zero() {
        let idx: ModuleIdx = Default::default();
        assert_eq!(idx.index(), 0);
    }
}
