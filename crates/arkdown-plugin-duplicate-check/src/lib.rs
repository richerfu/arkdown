use std::borrow::Cow;
use rustc_hash::FxHashMap;
use std::path::Path;

use rolldown_common::Output;
use rolldown_plugin::{HookGenerateBundleArgs, HookNoopReturn, HookUsage, Plugin, PluginContext};

pub struct DuplicateCheckPlugin;

impl std::fmt::Debug for DuplicateCheckPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DuplicateCheckPlugin").finish()
    }
}

impl Plugin for DuplicateCheckPlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("duplicate-check")
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::GenerateBundle
    }

    fn generate_bundle(
        &self,
        _ctx: &PluginContext,
        args: &mut HookGenerateBundleArgs<'_>,
    ) -> impl std::future::Future<Output = HookNoopReturn> + Send {
        let mut seen: FxHashMap<String, String> = FxHashMap::default();
        for output in args.bundle.iter() {
            if let Output::Chunk(chunk) = output {
                let filename = chunk.filename.to_string();
                if let Some(existing) = seen.get(&filename) {
                    let err = format!(
                        "Duplicate chunk filename '{}' detected: first '{}', second '{}'. \
                         Rename one to avoid .abc record conflicts.",
                        filename, existing, filename
                    );
                    return std::future::ready(Err(anyhow::anyhow!(err)));
                }
                seen.insert(filename.clone(), filename);
            }
        }
        std::future::ready(Ok(()))
    }
}

/// Check duplicate file stems from a list of module IDs (file paths).
/// Called by the bundler after building the module graph.
pub fn check_duplicate_module_ids(module_ids: &[String]) -> anyhow::Result<()> {
    let mut seen: FxHashMap<String, String> = FxHashMap::default();
    for id in module_ids {
        let path = Path::new(id);
        let id_lower = id.to_lowercase();
        if id_lower.ends_with(".d.ts") || id_lower.ends_with(".d.ets") {
            continue;
        }
        if id.starts_with("@bundle:") || id.starts_with("@shared:") {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            if let Some(existing) = seen.insert(stem.to_string(), id.clone()) {
                anyhow::bail!(
                    "Duplicate filename '{}' detected:\n  {}\n  {}\n\
                     Rename one to avoid .abc record conflicts.",
                    stem,
                    existing,
                    id
                );
            }
        }
    }
    Ok(())
}

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
}
