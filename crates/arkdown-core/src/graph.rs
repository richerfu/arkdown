use std::path::PathBuf;

use index_vec::IndexVec;
use rustc_hash::FxHashMap;

use crate::module::{ModuleSource, OxcAstRef, ResolvedId};

/// Opaque index into [`ModuleGraph::modules`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModuleIdx(u32);

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
pub struct Module {
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

/// OHOS-specific per-module metadata.
#[derive(Debug, Clone)]
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
