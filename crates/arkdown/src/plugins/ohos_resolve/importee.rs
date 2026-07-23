// Post-bundle importee processing — port of hvigor's `process-importee.js`.
//
// After rolldown scans the module graph, hvigor walks the resolvedIds map
// to categorize every imported module into:
//   • non-entry importees  — modules pulled in by entries but not entry chunks themselves
//   • compiled importees   — source files compiled by CompileArkTS (relative paths)
//
// The result is consumed by loader.json generation. Hvigor persists its
// incremental importee sets in the CompileArkTS cache as three conditional
// files; it does not emit a combined intermediates/importee JSON artifact.

use rustc_hash::{FxHashMap, FxHashSet};
use std::path::{Path, PathBuf};

use arkdown_core::HvigorModuleSourceFile;
use arkdown_path::{
    lexical_normalize, ArtifactPathSerializer, ModuleSpecifier, OhmUrl, SpecifierKind,
};
// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Configuration for the post-bundle importee processing pass.
pub struct ProcessImporteeConfig {
    /// Bundle name, e.g. `"com.example.app"`.
    pub bundle_name: String,
    /// Module name, e.g. `"entry"`.
    pub module_name: String,
    /// Absolute path to the module's `src/main` directory (used to derive
    /// relative source paths for `compiled_importees`).
    pub module_src_main: PathBuf,
}

/// Result of the post-bundle importee processing pass.
pub struct ProcessImporteeResult {
    /// OHM URL strings for non-entry imported modules, e.g.
    /// `"@bundle:com.example.app/entry/ets/utils/Helpers"`.
    pub non_entry_importees: Vec<String>,
    /// Source paths relative to the module `src/main` directory, e.g.
    /// `"./ets/entryability/EntryAbility.ets"`.
    pub compiled_importees: Vec<String>,
}

// ---------------------------------------------------------------------------
// Core algorithm
// ---------------------------------------------------------------------------

/// Walks the scanned Rolldown module graph and writes hvigor-compatible importee metadata.
pub struct ProcessImporteeRunner<'a> {
    module_source_files: &'a [HvigorModuleSourceFile],
    entry_module_ids: &'a [String],
    module_imports: &'a FxHashMap<String, Vec<String>>,
    config: &'a ProcessImporteeConfig,
}

impl<'a> ProcessImporteeRunner<'a> {
    pub fn new(
        module_source_files: &'a [HvigorModuleSourceFile],
        entry_module_ids: &'a [String],
        module_imports: &'a FxHashMap<String, Vec<String>>,
        config: &'a ProcessImporteeConfig,
    ) -> Self {
        Self {
            module_source_files,
            entry_module_ids,
            module_imports,
            config,
        }
    }

    /// Categorize all imported modules for loader generation.
    ///
    /// ## Algorithm
    ///
    /// 1. Identify entry module IDs from the Rolldown scan input.
    /// 2. BFS from entry modules through scanned resolved import edges.
    /// 3. Non-entry importees = reachable modules that are not entries, converted
    ///    to OHM URL format from their module ID.
    /// 4. Compiled importees = all non-external module source files converted
    ///    to paths relative to `config.module_src_main`.
    pub fn process(&self) -> ProcessImporteeResult {
        let compiled_module_ids = self
            .module_source_files
            .iter()
            .map(|file| file.module_id.as_str())
            .collect::<FxHashSet<_>>();
        let entry_module_ids = self
            .entry_module_ids
            .iter()
            .map(String::as_str)
            .collect::<FxHashSet<_>>();

        let mut reachable = FxHashSet::<String>::default();
        let mut queue = self.entry_module_ids.to_vec();

        while let Some(module_id) = queue.pop() {
            if !reachable.insert(module_id.clone()) {
                continue;
            }
            if let Some(imports) = self.module_imports.get(module_id.as_str()) {
                for imported_id in imports {
                    if compiled_module_ids.contains(imported_id.as_str())
                        && !reachable.contains(imported_id)
                    {
                        queue.push(imported_id.clone());
                    }
                }
            }
        }

        // ── 3. Non-entry importees (OHM URL format) ──────────────────────────
        let mut non_entry_importees: Vec<String> = reachable
            .iter()
            .filter(|module_id| !entry_module_ids.contains(module_id.as_str()))
            .filter_map(|module_id| {
                ImporteeModuleUrl::new(module_id.as_str(), self.config).ohm_url()
            })
            .collect();
        non_entry_importees.sort();

        // ── 4. Compiled importees (relative source paths) ────────────────────
        let mut compiled_importees: Vec<String> = self
            .module_source_files
            .iter()
            .filter_map(|source| {
                let id = source.module_id.as_str();
                if ImporteeModuleId::new(id).is_external() {
                    return None;
                }
                ImporteePathRelativizer::new(&self.config.module_src_main).relative_specifier(id)
            })
            .collect();
        compiled_importees.sort();
        compiled_importees.dedup();

        ProcessImporteeResult {
            non_entry_importees,
            compiled_importees,
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

struct ImporteeModuleId<'a> {
    raw: &'a str,
    kind: SpecifierKind,
}

impl<'a> ImporteeModuleId<'a> {
    fn new(raw: &'a str) -> Self {
        Self {
            raw,
            kind: ModuleSpecifier::new(raw).kind(),
        }
    }

    /// Return `true` if this id is an external OHOS module reference that
    /// should NOT appear in importee lists.
    fn is_external(&self) -> bool {
        self.is_runtime_builtin() || self.is_sdk_dot_namespace() || self.is_cross_bundle_ohm_url()
    }

    fn is_runtime_builtin(&self) -> bool {
        ModuleSpecifier::new(self.raw).is_runtime_builtin()
    }

    fn is_sdk_dot_namespace(&self) -> bool {
        self.raw.starts_with("@ohos.")
            || self.raw.starts_with("@kit.")
            || self.raw.starts_with("@arkts.")
    }

    fn is_cross_bundle_ohm_url(&self) -> bool {
        matches!(self.kind, SpecifierKind::OhmUrl)
            && OhmUrl::parse(self.raw)
                .ok()
                .is_some_and(|url| url.is_cross_bundle_reference())
    }
}

struct ImporteeModuleUrl<'a> {
    module_id: &'a str,
    config: &'a ProcessImporteeConfig,
}

impl<'a> ImporteeModuleUrl<'a> {
    fn new(module_id: &'a str, config: &'a ProcessImporteeConfig) -> Self {
        Self { module_id, config }
    }

    /// Convert a resolved module ID to an OHM URL string.
    ///
    /// Format: `@bundle:{bundle_name}/{module_name}/{path_without_ext}`
    /// where the path is derived from the module ID relative to
    /// `config.module_src_main`.
    fn ohm_url(&self) -> Option<String> {
        let id = self.module_id;
        if ImporteeModuleId::new(id).is_external() {
            return None;
        }
        let relative =
            ImporteePathRelativizer::new(&self.config.module_src_main).relative_source_path(id)?;
        let no_ext = ImporteeSourcePath::new(&relative).without_source_extension();
        let rel_path = ArtifactPathSerializer::rel_to_slash(&no_ext);
        Some(
            OhmUrl::bundle(
                &self.config.bundle_name,
                &self.config.module_name,
                &rel_path,
            )
            .into_string(),
        )
    }
}

struct ImporteeSourcePath<'a> {
    path: &'a Path,
}

impl<'a> ImporteeSourcePath<'a> {
    fn new(path: &'a Path) -> Self {
        Self { path }
    }

    fn without_source_extension(&self) -> PathBuf {
        let mut path = self.path.to_path_buf();
        if ImporteeSourceExtension::from_path(&path).is_source() {
            path.set_extension("");
        }
        path
    }
}

enum ImporteeSourceExtension {
    ArkTs,
    TypeScript,
    JavaScript,
    Other,
}

impl ImporteeSourceExtension {
    fn from_path(path: &Path) -> Self {
        match path.extension().and_then(|extension| extension.to_str()) {
            Some("ets") => Self::ArkTs,
            Some("ts") => Self::TypeScript,
            Some("js") => Self::JavaScript,
            _ => Self::Other,
        }
    }

    fn is_source(&self) -> bool {
        matches!(self, Self::ArkTs | Self::TypeScript | Self::JavaScript)
    }
}

struct ImporteePathRelativizer<'a> {
    base: &'a Path,
}

impl<'a> ImporteePathRelativizer<'a> {
    fn new(base: &'a Path) -> Self {
        Self { base }
    }

    /// Convert an absolute source file path to a relative specifier from
    /// `base` (module `src/main`), prefixed with `"./"`.
    ///
    /// Returns `None` if `abs` is not under `base`.
    fn relative_specifier(&self, abs: &str) -> Option<String> {
        let rel = self.relative_source_path(abs)?;
        Some(ModuleSpecifier::from_relative_path(&rel).into_string())
    }

    fn relative_source_path(&self, abs: &str) -> Option<PathBuf> {
        let abs_path = lexical_normalize(Path::new(abs));
        let base = lexical_normalize(self.base);
        let rel = abs_path.strip_prefix(base).ok()?;
        Some(rel.to_path_buf())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn make_config() -> ProcessImporteeConfig {
        ProcessImporteeConfig {
            bundle_name: "com.example.app".to_string(),
            module_name: "entry".to_string(),
            module_src_main: PathBuf::from("/project/entry/src/main"),
        }
    }

    fn module_source(module_id: &str) -> HvigorModuleSourceFile {
        HvigorModuleSourceFile {
            module_id: module_id.to_string(),
            path: PathBuf::from(module_id),
            record_name: module_id.to_string(),
            source_file: module_id.to_string(),
            package_name: Some("entry".to_string()),
            is_commonjs: false,
            is_shared_module: false,
            origin_source_lang: "ets".to_string(),
            imported_record_names: Vec::new(),
        }
    }

    fn module_imports(edges: &[(&str, &[&str])]) -> FxHashMap<String, Vec<String>> {
        edges
            .iter()
            .map(|(importer, imports)| {
                (
                    (*importer).to_string(),
                    imports.iter().map(|import| (*import).to_string()).collect(),
                )
            })
            .collect()
    }

    #[test]
    fn test_is_external_specifier() {
        assert!(ImporteeModuleId::new("@ohos:net.http").is_external());
        assert!(!ImporteeModuleId::new("@ohos/hypium").is_external());
        assert!(ImporteeModuleId::new("@kit.AbilityKit").is_external());
        assert!(ImporteeModuleId::new("@bundle:com.foo/bar/index").is_external());
        assert!(!ImporteeModuleId::new("/project/entry/src/main/ets/utils.ets").is_external());
        assert!(!ImporteeModuleId::new("./utils.ets").is_external());
    }

    #[test]
    fn test_source_path_without_extension() {
        assert_eq!(
            ImporteeSourcePath::new(Path::new("ets/pages/Index.ets")).without_source_extension(),
            PathBuf::from("ets/pages/Index")
        );
        assert_eq!(
            ImporteeSourcePath::new(Path::new("ets/utils.ts")).without_source_extension(),
            PathBuf::from("ets/utils")
        );
        assert_eq!(
            ImporteeSourcePath::new(Path::new("ets/helper.js")).without_source_extension(),
            PathBuf::from("ets/helper")
        );
        assert_eq!(
            ImporteeSourcePath::new(Path::new("ets/noext")).without_source_extension(),
            PathBuf::from("ets/noext")
        );
    }

    #[test]
    fn test_importee_path_relativizer_relative_specifier() {
        let base = Path::new("/project/entry/src/main");
        let abs = "/project/entry/src/main/ets/entryability/EntryAbility.ets";
        assert_eq!(
            ImporteePathRelativizer::new(base).relative_specifier(abs),
            Some("./ets/entryability/EntryAbility.ets".to_string())
        );
    }

    #[test]
    fn test_importee_path_relativizer_uses_lexical_normalization() {
        let base = Path::new("/project/entry/src/main");
        let abs = "/project/entry/src/main/ets/./utils/../pages/Index.ets";
        assert_eq!(
            ImporteePathRelativizer::new(base).relative_specifier(abs),
            Some("./ets/pages/Index.ets".to_string())
        );
    }

    #[test]
    fn test_importee_path_relativizer_rejects_path_outside_base() {
        let base = Path::new("/project/entry/src/main");
        let abs = "/other/path/file.ets";
        // Not under base — should return None.
        assert_eq!(
            ImporteePathRelativizer::new(base).relative_specifier(abs),
            None
        );
    }

    #[test]
    fn test_process_importees_empty_graph() {
        let config = make_config();
        let files = Vec::new();
        let entries = Vec::new();
        let imports = FxHashMap::default();
        let result = ProcessImporteeRunner::new(&files, &entries, &imports, &config).process();
        assert!(result.non_entry_importees.is_empty());
        assert!(result.compiled_importees.is_empty());
    }

    #[test]
    fn test_process_importees_single_entry_no_imports() {
        let config = make_config();
        let entry = "/project/entry/src/main/ets/entryability/EntryAbility.ets".to_string();
        let files = vec![module_source(&entry)];
        let entries = vec![entry];
        let imports = FxHashMap::default();
        let result = ProcessImporteeRunner::new(&files, &entries, &imports, &config).process();

        // No non-entry importees (the entry imports no compiled module).
        assert!(result.non_entry_importees.is_empty());
        // The entry source file itself appears in compiled_importees.
        assert_eq!(
            result.compiled_importees,
            vec!["./ets/entryability/EntryAbility.ets".to_string()]
        );
    }

    #[test]
    fn test_process_importees_entry_with_shared_chunk() {
        let config = make_config();
        let entry = "/project/entry/src/main/ets/entryability/EntryAbility.ets".to_string();
        let utils = "/project/entry/src/main/ets/utils/Helpers.ets".to_string();
        let files = vec![module_source(&entry), module_source(&utils)];
        let entries = vec![entry.clone()];
        let imports = module_imports(&[(&entry, &[&utils])]);

        let result = ProcessImporteeRunner::new(&files, &entries, &imports, &config).process();

        assert_eq!(
            result.non_entry_importees,
            vec!["@bundle:com.example.app/entry/ets/utils/Helpers".to_string()]
        );
        // Both source files appear in compiled_importees.
        assert_eq!(result.compiled_importees.len(), 2);
        assert!(result
            .compiled_importees
            .contains(&"./ets/entryability/EntryAbility.ets".to_string()));
        assert!(result
            .compiled_importees
            .contains(&"./ets/utils/Helpers.ets".to_string()));
    }

    #[test]
    fn test_process_importees_external_facade_excluded() {
        let config = make_config();
        let external = "@bundle:com.example.har/lib/index".to_string();
        let files = vec![module_source(&external)];
        let entries = vec![external];
        let imports = FxHashMap::default();
        let result = ProcessImporteeRunner::new(&files, &entries, &imports, &config).process();

        assert!(result.compiled_importees.is_empty());
    }

    #[test]
    fn test_process_importees_unreachable_chunk_excluded() {
        let config = make_config();
        let entry = "/project/entry/src/main/ets/entryability/EntryAbility.ets".to_string();
        let orphan = "/project/entry/src/main/ets/orphan/Orphan.ets".to_string();
        let files = vec![module_source(&entry), module_source(&orphan)];
        let entries = vec![entry];
        let imports = FxHashMap::default();
        let result = ProcessImporteeRunner::new(&files, &entries, &imports, &config).process();

        // Orphan module is not reachable from an entry → not in non_entry_importees.
        assert!(result.non_entry_importees.is_empty());
        // But it still appears in compiled_importees because CompileArkTS compiled it.
        assert_eq!(result.compiled_importees.len(), 2);
    }
}
