use std::collections::HashMap;
use std::path::Path;

use arkdown_core::{
    ArkdownError, ArkdownPlugin, ChunkGraph, HookUsage, PluginContext, PluginResult,
};
use async_trait::async_trait;

pub struct DuplicateCheckPlugin;

#[async_trait]
impl ArkdownPlugin for DuplicateCheckPlugin {
    fn name(&self) -> &str {
        "duplicate-check"
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::GenerateBundle
    }

    async fn generate_bundle(
        &self,
        _ctx: &PluginContext,
        bundle: &mut ChunkGraph,
        _is_write: bool,
    ) -> PluginResult<()> {
        // Collect all unique module IDs across all chunks
        for chunk in bundle.chunks.iter() {
            // chunk.modules contains ModuleIdx values, but we don't have the graph here.
            // Instead, scan using the chunk name and report based on what we have.
            // Note: Full duplicate checking requires the ModuleGraph — this is a
            // best-effort scan of chunk names for now.
            let _ = chunk; // suppress unused warning
        }
        // For the actual check, we look at chunk filenames since that's what es2abc sees
        check_duplicate_chunk_names(bundle)
    }
}

/// Check that no two chunks have the same name (which would create duplicate .abc records).
fn check_duplicate_chunk_names(bundle: &ChunkGraph) -> PluginResult<()> {
    let mut seen: HashMap<String, String> = HashMap::new();
    for chunk in bundle.chunks.iter() {
        let stem = chunk.name.clone();
        if let Some(existing) = seen.get(&stem) {
            return Err(ArkdownError::PluginError {
                plugin: "duplicate-check".to_string(),
                source: Box::new(DuplicateError {
                    stem,
                    first: existing.clone(),
                    second: chunk.name.clone(),
                }),
            });
        }
        seen.insert(stem, chunk.name.clone());
    }
    Ok(())
}

/// Check duplicate file stems from a list of module IDs (file paths).
/// Called by the bundler after building the module graph.
pub fn check_duplicate_module_ids(module_ids: &[String]) -> PluginResult<()> {
    let mut seen: HashMap<String, String> = HashMap::new();
    for id in module_ids {
        let path = Path::new(id);
        // Skip declaration files
        let id_lower = id.to_lowercase();
        if id_lower.ends_with(".d.ts") || id_lower.ends_with(".d.ets") {
            continue;
        }
        // Skip external references
        if id.starts_with("@bundle:") || id.starts_with("@shared:") {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            if let Some(existing) = seen.insert(stem.to_string(), id.clone()) {
                return Err(ArkdownError::PluginError {
                    plugin: "duplicate-check".to_string(),
                    source: Box::new(DuplicateError {
                        stem: stem.to_string(),
                        first: existing,
                        second: id.clone(),
                    }),
                });
            }
        }
    }
    Ok(())
}

#[derive(Debug)]
struct DuplicateError {
    stem: String,
    first: String,
    second: String,
}

impl std::fmt::Display for DuplicateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Duplicate filename '{}' detected:\n  {}\n  {}\nRename one to avoid .abc record conflicts.",
            self.stem, self.first, self.second
        )
    }
}

impl std::error::Error for DuplicateError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_duplicates() {
        let ids = vec![
            "/project/src/main/ets/utils/Logger.ets".to_string(),
            "/project/src/main/ets/pages/Index.ets".to_string(),
        ];
        assert!(check_duplicate_module_ids(&ids).is_ok());
    }

    #[test]
    fn test_detects_duplicate() {
        let ids = vec![
            "/project/src/main/ets/utils/Logger.ets".to_string(),
            "/project/src/main/ets/components/Logger.ets".to_string(),
        ];
        let result = check_duplicate_module_ids(&ids);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Logger") || err.contains("duplicate") || err.contains("Duplicate"));
    }

    #[test]
    fn test_declaration_files_excluded() {
        let ids = vec![
            "/sdk/api/hilog.d.ts".to_string(),
            "/sdk/api/hilog2.d.ts".to_string(),
            "/project/hilog.ets".to_string(),
        ];
        // .d.ts files should be excluded from the check
        assert!(check_duplicate_module_ids(&ids).is_ok());
    }

    #[test]
    fn test_external_refs_excluded() {
        let ids = vec![
            "@bundle:com.foo/entry/ets/Index".to_string(),
            "@bundle:com.foo/entry/ets/Index2".to_string(),
        ];
        assert!(check_duplicate_module_ids(&ids).is_ok());
    }

    #[tokio::test]
    async fn test_plugin_no_duplicate_chunks() {
        use index_vec::IndexVec;
        use rustc_hash::FxHashMap;
        let plugin = DuplicateCheckPlugin;
        let chunk_graph = ChunkGraph {
            chunks: IndexVec::new(),
            module_to_chunk: IndexVec::new(),
            entry_module_to_entry_chunk: FxHashMap::default(),
        };
        // Empty graph should pass
        let mut cg = chunk_graph;
        let result = plugin
            .generate_bundle(&PluginContext::default(), &mut cg, false)
            .await;
        assert!(result.is_ok());
    }
}
