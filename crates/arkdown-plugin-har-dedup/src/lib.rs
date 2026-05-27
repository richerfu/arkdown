use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use arkdown_core::{
    ArkdownError, ArkdownPlugin, BuildContext, BuildError, ModuleParsedContext, ModuleType,
    PluginResult, ResolveContext, ResolvedId, Module,
};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Controls whether the plugin is in the first (collect) or second (exclude) pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HarDedupMode {
    /// First pass: collect all HAR module IDs compiled into the HSP.
    CollectPass,
    /// Second pass: exclude the collected HAR modules from the HAP build.
    RealBuild { excluded_files: HashSet<PathBuf> },
}

/// Plugin that deduplicates HAR files for HSP builds.
///
/// When an HSP and a HAP both depend on the same HAR, the HAR is compiled
/// into the HSP only. The HAP build marks those modules as external so they
/// are not bundled again.
///
/// # Two-pass workflow
///
/// 1. Run `HarDeduplicatePlugin::collect_pass(path)` during the **HSP** build.
///    At `build_end` it serialises the set of collected HAR module IDs to `path`.
///
/// 2. Run `HarDeduplicatePlugin::real_build(path)` during the **HAP** build.
///    It reads the file written in step 1 and marks those module IDs as external
///    in `resolve_id`, so the bundler skips bundling them.
pub struct HarDeduplicatePlugin {
    mode: HarDedupMode,
    collect_file_path: PathBuf,
    collected: Arc<Mutex<Vec<String>>>,
}

impl HarDeduplicatePlugin {
    /// Create a collect-pass instance (used for the HSP build).
    pub fn collect_pass(collect_file_path: PathBuf) -> Self {
        HarDeduplicatePlugin {
            mode: HarDedupMode::CollectPass,
            collect_file_path,
            collected: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Create a real-build instance (used for the HAP build).
    ///
    /// Reads the collect file produced by a prior `collect_pass` build.
    /// If the file does not exist, no modules are excluded.
    pub fn real_build(collect_file_path: PathBuf) -> Result<Self, HarDedupError> {
        let excluded = read_collect_file(&collect_file_path)?;
        Ok(HarDeduplicatePlugin {
            mode: HarDedupMode::RealBuild { excluded_files: excluded },
            collect_file_path,
            collected: Arc::new(Mutex::new(Vec::new())),
        })
    }
}

#[async_trait]
impl ArkdownPlugin for HarDeduplicatePlugin {
    fn name(&self) -> &str {
        "har-dedup"
    }

    /// In `RealBuild` mode, mark excluded HAR modules as external.
    async fn resolve_id(
        &self,
        _ctx: &ResolveContext,
        specifier: &str,
        _importer: Option<&str>,
    ) -> PluginResult<Option<ResolvedId>> {
        if let HarDedupMode::RealBuild { ref excluded_files } = self.mode {
            let path = PathBuf::from(specifier);
            if excluded_files.contains(&path) {
                return Ok(Some(ResolvedId {
                    id: specifier.to_string(),
                    external: true,
                    module_type: ModuleType::Ts,
                }));
            }
        }
        Ok(None)
    }

    /// In `CollectPass` mode, record every HAR module that is parsed.
    ///
    /// A module is considered a HAR module when its ID contains `oh_modules`
    /// (ohpm install path) or when the module-meta plugin has flagged it as a
    /// local dependency.
    async fn module_parsed(
        &self,
        _ctx: &ModuleParsedContext,
        module: &Module,
    ) -> PluginResult<()> {
        if matches!(self.mode, HarDedupMode::CollectPass) {
            let id = &module.id.id;
            if id.contains("oh_modules") || module.meta.is_local_dependency {
                self.collected
                    .lock()
                    .unwrap()
                    .push(id.clone());
            }
        }
        Ok(())
    }

    /// In `CollectPass` mode, flush collected IDs to disk when the build finishes.
    async fn build_end(
        &self,
        _ctx: &BuildContext,
        _error: Option<&BuildError>,
    ) -> PluginResult<()> {
        if matches!(self.mode, HarDedupMode::CollectPass) {
            let files = self.collected.lock().unwrap().clone();
            write_collect_file(&self.collect_file_path, &files).map_err(|e| {
                ArkdownError::PluginError {
                    plugin: "har-dedup".to_string(),
                    source: Box::new(e),
                }
            })?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Collect-file serialisation
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
struct CollectFile {
    source_har_files: Vec<String>,
}

fn write_collect_file(path: &Path, files: &[String]) -> Result<(), HarDedupError> {
    let data = CollectFile {
        source_har_files: files.to_vec(),
    };
    let json = serde_json::to_string_pretty(&data)?;
    if let Some(parent) = path.parent() {
        fs_err::create_dir_all(parent)?;
    }
    fs_err::write(path, json)?;
    Ok(())
}

fn read_collect_file(path: &Path) -> Result<HashSet<PathBuf>, HarDedupError> {
    if !path.exists() {
        return Ok(HashSet::new());
    }
    let content = fs_err::read_to_string(path)?;
    let data: CollectFile = serde_json::from_str(&content)?;
    Ok(data.source_har_files.into_iter().map(PathBuf::from).collect())
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum HarDedupError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_file_roundtrip() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        // Drop the guard so write_collect_file can overwrite it.
        drop(tmp);

        let files = vec!["a.ts".to_string(), "b.ets".to_string()];
        write_collect_file(&path, &files).unwrap();

        let read = read_collect_file(&path).unwrap();
        assert!(read.contains(&PathBuf::from("a.ts")));
        assert!(read.contains(&PathBuf::from("b.ets")));
    }

    #[test]
    fn test_empty_when_missing() {
        let result =
            read_collect_file(Path::new("/nonexistent/arkdown-test-path-that-never-exists.json"))
                .unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_real_build_excludes_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("collect.json");

        write_collect_file(
            &path,
            &["/project/oh_modules/lib/index.ets".to_string()],
        )
        .unwrap();

        let plugin = HarDeduplicatePlugin::real_build(path).unwrap();
        let result = plugin
            .resolve_id(
                &ResolveContext::default(),
                "/project/oh_modules/lib/index.ets",
                None,
            )
            .await
            .unwrap();

        assert!(result.unwrap().external);
    }

    #[tokio::test]
    async fn test_non_excluded_passes_through() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("collect.json");

        write_collect_file(&path, &["/other/file.ets".to_string()]).unwrap();

        let plugin = HarDeduplicatePlugin::real_build(path).unwrap();
        let result = plugin
            .resolve_id(
                &ResolveContext::default(),
                "/project/entry/src/main/ets/Index.ets",
                None,
            )
            .await
            .unwrap();

        assert!(result.is_none());
    }
}
