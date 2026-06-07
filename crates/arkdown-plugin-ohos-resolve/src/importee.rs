// Post-bundle importee processing — port of hvigor's `process-importee.js`.
//
// After rolldown finishes bundling, hvigor walks the bundle's `resolvedIds` map
// to categorize every imported module into:
//   • non-entry importees  — modules pulled in by entries but not entry chunks themselves
//   • compiled importees   — source files compiled into the bundle (relative paths)
//
// The results are written to loader.json and to an importee cache JSON file under
// `intermediates/importee/default/{module_name}/importee.json`.

use rustc_hash::{FxHashMap, FxHashSet};
use std::path::{Path, PathBuf};

use arkdown_core::{ArkChunk, ChunkGraph};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Configuration for the post-bundle importee processing pass.
pub struct ProcessImporteeConfig {
    /// Bundle name, e.g. `"com.example.app"`.
    pub bundle_name: String,
    /// Module name, e.g. `"entry"`.
    pub module_name: String,
    /// Module version string.  Empty string is acceptable.
    pub module_version: String,
    /// Absolute path to the project root.
    pub project_root: PathBuf,
    /// Absolute path to the module's `src/main` directory (used to derive
    /// relative source paths for `compiled_importees`).
    pub module_src_main: PathBuf,
    /// Absolute path to the module's `build/default/intermediates` directory.
    /// The importee cache JSON is written to
    /// `{output_dir}/importee/default/{module_name}/importee.json`.
    pub output_dir: PathBuf,
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
// Internal cache file schema
// ---------------------------------------------------------------------------

/// Cache file persisted to
/// `intermediates/importee/default/{module_name}/importee.json`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImporteeCache {
    non_entry_importees: Vec<String>,
    compiled_importees: Vec<String>,
}

// ---------------------------------------------------------------------------
// Core algorithm
// ---------------------------------------------------------------------------

/// Walk the bundle's [`ChunkGraph`] to categorize all imported modules.
///
/// Returns [`ProcessImporteeResult`] and writes an importee cache JSON to
/// `{config.output_dir}/importee/default/{config.module_name}/importee.json`.
///
/// ## Algorithm
///
/// 1. Build a `name → &ArkChunk` lookup map.
/// 2. Identify entry chunks (`is_entry || is_dynamic_entry`).
/// 3. BFS from entry chunks through `imports`/`dynamic_imports` to find all
///    reachable chunks.
/// 4. Non-entry importees = reachable chunks that are NOT themselves entries,
///    converted to OHM URL format via their `facade_module_id`.
/// 5. Compiled importees = all non-external `facade_module_id` values converted
///    to paths relative to `config.module_src_main`.
pub fn process_importees(
    chunk_graph: &ChunkGraph,
    config: &ProcessImporteeConfig,
) -> std::io::Result<ProcessImporteeResult> {
    // ── 1. Build name → chunk lookup ─────────────────────────────────────────
    let chunk_by_name: FxHashMap<&str, &ArkChunk> = chunk_graph
        .chunks
        .iter()
        .map(|c| (c.name.as_str(), c))
        .collect();

    // ── 2. Identify entry chunk names ────────────────────────────────────────
    let entry_names: FxHashSet<&str> = chunk_graph
        .chunks
        .iter()
        .filter(|c| c.is_entry || c.is_dynamic_entry)
        .map(|c| c.name.as_str())
        .collect();

    // ── 3. BFS to find all chunks reachable from entry chunks ────────────────
    let mut reachable: FxHashSet<&str> = FxHashSet::default();
    let mut queue: Vec<&str> = entry_names.iter().copied().collect();

    while let Some(name) = queue.pop() {
        if reachable.contains(name) {
            continue;
        }
        reachable.insert(name);

        if let Some(chunk) = chunk_by_name.get(name) {
            for imp in chunk.imports.iter().chain(chunk.dynamic_imports.iter()) {
                // imports are chunk output filenames like "utils.js" — strip the
                // .js extension to match against chunk names.
                let imp_name = strip_js_ext(imp);
                if chunk_by_name.contains_key(imp_name) && !reachable.contains(imp_name) {
                    queue.push(imp_name);
                }
            }
        }
    }

    // ── 4. Non-entry importees (OHM URL format) ──────────────────────────────
    let mut non_entry_importees: Vec<String> = reachable
        .iter()
        .filter(|&&name| !entry_names.contains(name))
        .filter_map(|&name| {
            let chunk = chunk_by_name.get(name)?;
            chunk_to_ohm_url(chunk, config)
        })
        .collect();
    non_entry_importees.sort();

    // ── 5. Compiled importees (relative source paths) ────────────────────────
    let mut compiled_importees: Vec<String> = chunk_graph
        .chunks
        .iter()
        .filter_map(|chunk| {
            let id = chunk.facade_module_id.as_deref()?;
            if is_external_specifier(id) {
                return None;
            }
            abs_to_relative_src(id, &config.module_src_main)
        })
        .collect();
    compiled_importees.sort();
    compiled_importees.dedup();

    // ── 6. Write importee cache JSON ─────────────────────────────────────────
    write_importee_cache(&non_entry_importees, &compiled_importees, config)?;

    Ok(ProcessImporteeResult {
        non_entry_importees,
        compiled_importees,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Strip the `.js` extension from a chunk import specifier to obtain the chunk
/// name used as the lookup key in `chunk_by_name`.
fn strip_js_ext(s: &str) -> &str {
    // Handle both "utils.js" and "./utils.js" → "utils".
    let basename = s.rsplit('/').next().unwrap_or(s);
    basename.strip_suffix(".js").unwrap_or(basename)
}

/// Return `true` if `specifier` is an external OHOS module reference that
/// should NOT appear in the importee lists.
fn is_external_specifier(s: &str) -> bool {
    s.starts_with("@ohos:")
        || s.starts_with("@ohos/")
        || s.starts_with("@kit.")
        || s.starts_with("@shared:")
        || s.starts_with("@bundle:")
        || s.starts_with("@system.")
        || s.starts_with("@hms.")
        || s.starts_with("@hms:")
}

/// Convert an `ArkChunk` to an OHM URL string.
///
/// Format: `@bundle:{bundle_name}/{module_name}/{path_without_ext}`
/// where the path is derived from `facade_module_id` relative to
/// `config.module_src_main`.
fn chunk_to_ohm_url(chunk: &ArkChunk, config: &ProcessImporteeConfig) -> Option<String> {
    let id = chunk.facade_module_id.as_deref()?;
    if is_external_specifier(id) {
        return None;
    }
    let relative = abs_to_relative_src(id, &config.module_src_main)?;
    // Strip leading "./" and source file extension.
    let clean = relative.trim_start_matches("./");
    let no_ext = strip_source_ext(clean);
    Some(format!(
        "@bundle:{}/{}/{}",
        config.bundle_name, config.module_name, no_ext
    ))
}

/// Strip common ArkTS/TS/JS source file extensions.
fn strip_source_ext(path: &str) -> &str {
    for ext in &[".ets", ".ts", ".js"] {
        if let Some(s) = path.strip_suffix(ext) {
            return s;
        }
    }
    path
}

/// Convert an absolute source file path to a relative path from `base`
/// (module `src/main`), prefixed with `"./"`.
///
/// Returns `None` if `abs` is not under `base`.
fn abs_to_relative_src(abs: &str, base: &Path) -> Option<String> {
    let abs_path = Path::new(abs);
    // First try a direct prefix strip (works when both are clean absolute paths).
    if let Ok(rel) = abs_path.strip_prefix(base) {
        return Some(format!("./{}", rel.to_string_lossy()));
    }
    // If that fails, try after canonicalizing both sides.
    let canon_abs = std::fs::canonicalize(abs_path).ok()?;
    let canon_base = std::fs::canonicalize(base).ok()?;
    let rel = canon_abs.strip_prefix(&canon_base).ok()?;
    Some(format!("./{}", rel.to_string_lossy()))
}

/// Write the importee cache JSON to the intermediates directory.
fn write_importee_cache(
    non_entry_importees: &[String],
    compiled_importees: &[String],
    config: &ProcessImporteeConfig,
) -> std::io::Result<()> {
    let cache_dir = config
        .output_dir
        .join("importee")
        .join("default")
        .join(&config.module_name);
    std::fs::create_dir_all(&cache_dir)?;

    let cache = ImporteeCache {
        non_entry_importees: non_entry_importees.to_vec(),
        compiled_importees: compiled_importees.to_vec(),
    };
    let json = serde_json::to_string_pretty(&cache)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(cache_dir.join("importee.json"), json.as_bytes())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use arkdown_core::{ArkChunk, ChunkGraph};
    use oxc_index::IndexVec;

    use super::*;

    fn make_config(tmp_dir: &Path) -> ProcessImporteeConfig {
        ProcessImporteeConfig {
            bundle_name: "com.example.app".to_string(),
            module_name: "entry".to_string(),
            module_version: "1.0.0".to_string(),
            project_root: PathBuf::from("/project"),
            module_src_main: PathBuf::from("/project/entry/src/main"),
            output_dir: tmp_dir.to_path_buf(),
        }
    }

    fn make_chunk_graph(chunks: Vec<ArkChunk>) -> ChunkGraph {
        let mut idx_vec: IndexVec<arkdown_core::ChunkIdx, ArkChunk> = IndexVec::new();
        for c in chunks {
            idx_vec.push(c);
        }
        ChunkGraph {
            chunks: idx_vec,
            full_module_ids: Vec::new(),
            full_module_is_commonjs: FxHashMap::default(),
            module_to_chunk: IndexVec::new(),
            entry_module_to_entry_chunk: FxHashMap::default(),
        }
    }

    #[test]
    fn test_strip_js_ext_plain() {
        assert_eq!(strip_js_ext("utils.js"), "utils");
    }

    #[test]
    fn test_strip_js_ext_with_slash() {
        assert_eq!(strip_js_ext("./utils.js"), "utils");
    }

    #[test]
    fn test_strip_js_ext_no_ext() {
        assert_eq!(strip_js_ext("utils"), "utils");
    }

    #[test]
    fn test_is_external_specifier() {
        assert!(is_external_specifier("@ohos:net.http"));
        assert!(is_external_specifier("@ohos/hyperium"));
        assert!(is_external_specifier("@kit.AbilityKit"));
        assert!(is_external_specifier("@bundle:com.foo/bar/index"));
        assert!(!is_external_specifier(
            "/project/entry/src/main/ets/utils.ets"
        ));
        assert!(!is_external_specifier("./utils.ets"));
    }

    #[test]
    fn test_strip_source_ext() {
        assert_eq!(strip_source_ext("ets/pages/Index.ets"), "ets/pages/Index");
        assert_eq!(strip_source_ext("ets/utils.ts"), "ets/utils");
        assert_eq!(strip_source_ext("ets/helper.js"), "ets/helper");
        assert_eq!(strip_source_ext("ets/noext"), "ets/noext");
    }

    #[test]
    fn test_abs_to_relative_src() {
        let base = Path::new("/project/entry/src/main");
        let abs = "/project/entry/src/main/ets/entryability/EntryAbility.ets";
        assert_eq!(
            abs_to_relative_src(abs, base),
            Some("./ets/entryability/EntryAbility.ets".to_string())
        );
    }

    #[test]
    fn test_abs_to_relative_src_not_under_base() {
        let base = Path::new("/project/entry/src/main");
        let abs = "/other/path/file.ets";
        // Not under base — should return None.
        assert_eq!(abs_to_relative_src(abs, base), None);
    }

    #[test]
    fn test_process_importees_empty_graph() {
        let tmp = tempfile::tempdir().unwrap();
        let config = make_config(tmp.path());
        let graph = make_chunk_graph(vec![]);
        let result = process_importees(&graph, &config).unwrap();
        assert!(result.non_entry_importees.is_empty());
        assert!(result.compiled_importees.is_empty());

        // Cache file should still be written.
        let cache_path = tmp.path().join("importee/default/entry/importee.json");
        assert!(cache_path.exists());
        let content = std::fs::read_to_string(&cache_path).unwrap();
        let parsed: ImporteeCache = serde_json::from_str(&content).unwrap();
        assert!(parsed.non_entry_importees.is_empty());
        assert!(parsed.compiled_importees.is_empty());
    }

    #[test]
    fn test_process_importees_single_entry_no_imports() {
        let tmp = tempfile::tempdir().unwrap();
        let config = make_config(tmp.path());

        let mut entry_chunk = ArkChunk::default();
        entry_chunk.name = "EntryAbility".to_string();
        entry_chunk.is_entry = true;
        entry_chunk.facade_module_id =
            Some("/project/entry/src/main/ets/entryability/EntryAbility.ets".to_string());

        let graph = make_chunk_graph(vec![entry_chunk]);
        let result = process_importees(&graph, &config).unwrap();

        // No non-entry importees (no shared chunks imported).
        assert!(result.non_entry_importees.is_empty());
        // The entry source file itself appears in compiled_importees.
        assert_eq!(
            result.compiled_importees,
            vec!["./ets/entryability/EntryAbility.ets".to_string()]
        );
    }

    #[test]
    fn test_process_importees_entry_with_shared_chunk() {
        let tmp = tempfile::tempdir().unwrap();
        let config = make_config(tmp.path());

        // Entry chunk imports a shared "utils" chunk.
        let mut entry_chunk = ArkChunk::default();
        entry_chunk.name = "EntryAbility".to_string();
        entry_chunk.is_entry = true;
        entry_chunk.facade_module_id =
            Some("/project/entry/src/main/ets/entryability/EntryAbility.ets".to_string());
        entry_chunk.imports = vec!["utils.js".to_string()];

        let mut utils_chunk = ArkChunk::default();
        utils_chunk.name = "utils".to_string();
        utils_chunk.is_entry = false;
        utils_chunk.facade_module_id =
            Some("/project/entry/src/main/ets/utils/Helpers.ets".to_string());

        let graph = make_chunk_graph(vec![entry_chunk, utils_chunk]);
        let result = process_importees(&graph, &config).unwrap();

        // "utils" chunk is a non-entry importee.
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
        let tmp = tempfile::tempdir().unwrap();
        let config = make_config(tmp.path());

        let mut entry_chunk = ArkChunk::default();
        entry_chunk.name = "EntryAbility".to_string();
        entry_chunk.is_entry = true;
        // facade_module_id is an external OHM URL — must be excluded from
        // compiled_importees.
        entry_chunk.facade_module_id = Some("@bundle:com.example.har/lib/index".to_string());

        let graph = make_chunk_graph(vec![entry_chunk]);
        let result = process_importees(&graph, &config).unwrap();

        assert!(result.compiled_importees.is_empty());
    }

    #[test]
    fn test_process_importees_unreachable_chunk_excluded() {
        let tmp = tempfile::tempdir().unwrap();
        let config = make_config(tmp.path());

        // Entry chunk does NOT import "orphan".
        let mut entry_chunk = ArkChunk::default();
        entry_chunk.name = "EntryAbility".to_string();
        entry_chunk.is_entry = true;
        entry_chunk.facade_module_id =
            Some("/project/entry/src/main/ets/entryability/EntryAbility.ets".to_string());

        let mut orphan_chunk = ArkChunk::default();
        orphan_chunk.name = "orphan".to_string();
        orphan_chunk.is_entry = false;
        orphan_chunk.facade_module_id =
            Some("/project/entry/src/main/ets/orphan/Orphan.ets".to_string());

        let graph = make_chunk_graph(vec![entry_chunk, orphan_chunk]);
        let result = process_importees(&graph, &config).unwrap();

        // Orphan chunk is not reachable → not in non_entry_importees.
        assert!(result.non_entry_importees.is_empty());
        // But it still appears in compiled_importees (it was compiled into the
        // bundle even if unreachable from entries).
        assert_eq!(result.compiled_importees.len(), 2);
    }

    #[test]
    fn test_cache_file_content() {
        let tmp = tempfile::tempdir().unwrap();
        let config = make_config(tmp.path());

        let mut entry_chunk = ArkChunk::default();
        entry_chunk.name = "EntryAbility".to_string();
        entry_chunk.is_entry = true;
        entry_chunk.facade_module_id =
            Some("/project/entry/src/main/ets/entryability/EntryAbility.ets".to_string());
        entry_chunk.imports = vec!["utils.js".to_string()];

        let mut utils_chunk = ArkChunk::default();
        utils_chunk.name = "utils".to_string();
        utils_chunk.is_entry = false;
        utils_chunk.facade_module_id =
            Some("/project/entry/src/main/ets/utils/Helpers.ets".to_string());

        let graph = make_chunk_graph(vec![entry_chunk, utils_chunk]);
        process_importees(&graph, &config).unwrap();

        let cache_path = tmp.path().join("importee/default/entry/importee.json");
        let content = std::fs::read_to_string(&cache_path).unwrap();
        let parsed: ImporteeCache = serde_json::from_str(&content).unwrap();
        assert_eq!(
            parsed.non_entry_importees,
            vec!["@bundle:com.example.app/entry/ets/utils/Helpers".to_string()]
        );
        assert_eq!(parsed.compiled_importees.len(), 2);
    }
}
