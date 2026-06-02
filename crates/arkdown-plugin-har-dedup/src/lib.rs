use std::borrow::Cow;
use rustc_hash::FxHashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rolldown_common::{ModuleInfo, NormalModule, ResolvedExternal};
use rolldown_plugin::{
    HookBuildEndArgs, HookNoopReturn, HookResolveIdArgs, HookResolveIdOutput, HookResolveIdReturn,
    HookUsage, Plugin, PluginContext,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HarDedupMode {
    CollectPass,
    RealBuild { excluded_files: FxHashSet<PathBuf> },
}

pub struct HarDeduplicatePlugin {
    mode: HarDedupMode,
    collect_file_path: PathBuf,
    collected: Arc<Mutex<Vec<String>>>,
}

impl std::fmt::Debug for HarDeduplicatePlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HarDeduplicatePlugin")
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}

impl HarDeduplicatePlugin {
    pub fn collect_pass(collect_file_path: PathBuf) -> Self {
        HarDeduplicatePlugin {
            mode: HarDedupMode::CollectPass,
            collect_file_path,
            collected: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn real_build(collect_file_path: PathBuf) -> Result<Self, HarDedupError> {
        let excluded = read_collect_file(&collect_file_path)?;
        Ok(HarDeduplicatePlugin {
            mode: HarDedupMode::RealBuild {
                excluded_files: excluded,
            },
            collect_file_path,
            collected: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub(crate) fn resolve_path(
        &self,
        specifier: &str,
    ) -> anyhow::Result<Option<HookResolveIdOutput>> {
        if let HarDedupMode::RealBuild { ref excluded_files } = self.mode {
            let path = PathBuf::from(specifier);
            if excluded_files.contains(&path) {
                return Ok(Some(HookResolveIdOutput {
                    id: specifier.into(),
                    external: Some(ResolvedExternal::Bool(true)),
                    ..Default::default()
                }));
            }
        }
        Ok(None)
    }

    pub(crate) fn flush_collect(&self) -> Result<(), HarDedupError> {
        if matches!(self.mode, HarDedupMode::CollectPass) {
            let files = self.collected.lock().unwrap().clone();
            write_collect_file(&self.collect_file_path, &files)?;
        }
        Ok(())
    }
}

impl Plugin for HarDeduplicatePlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("har-dedup")
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::ResolveId | HookUsage::ModuleParsed | HookUsage::BuildEnd
    }

    fn resolve_id(
        &self,
        _ctx: &PluginContext,
        args: &HookResolveIdArgs<'_>,
    ) -> impl std::future::Future<Output = HookResolveIdReturn> + Send {
        let specifier = args.specifier.to_owned();
        let result = self.resolve_path(&specifier);
        async move { result }
    }

    fn module_parsed(
        &self,
        _ctx: &PluginContext,
        module_info: Arc<ModuleInfo>,
        _normal_module: &NormalModule,
    ) -> impl std::future::Future<Output = HookNoopReturn> + Send {
        let id = module_info.id.as_str().to_owned();
        let collected = Arc::clone(&self.collected);
        let is_collect_pass = matches!(self.mode, HarDedupMode::CollectPass);
        async move {
            if is_collect_pass && id.contains("oh_modules") {
                collected.lock().unwrap().push(id);
            }
            Ok(())
        }
    }

    fn build_end(
        &self,
        _ctx: &PluginContext,
        _args: Option<&HookBuildEndArgs<'_>>,
    ) -> impl std::future::Future<Output = HookNoopReturn> + Send {
        let result = self.flush_collect();
        async move { result.map_err(|e| anyhow::anyhow!("har-dedup: {e}")) }
    }
}

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

fn read_collect_file(path: &Path) -> Result<FxHashSet<PathBuf>, HarDedupError> {
    if !path.exists() {
        return Ok(FxHashSet::default());
    }
    let content = fs_err::read_to_string(path)?;
    let data: CollectFile = serde_json::from_str(&content)?;
    Ok(data
        .source_har_files
        .into_iter()
        .map(PathBuf::from)
        .collect())
}

#[derive(Debug, thiserror::Error)]
pub enum HarDedupError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_file_roundtrip() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        drop(tmp);

        let files = vec!["a.ts".to_string(), "b.ets".to_string()];
        write_collect_file(&path, &files).unwrap();

        let read = read_collect_file(&path).unwrap();
        assert!(read.contains(&PathBuf::from("a.ts")));
        assert!(read.contains(&PathBuf::from("b.ets")));
    }

    #[test]
    fn test_empty_when_missing() {
        let result = read_collect_file(Path::new(
            "/nonexistent/arkdown-test-path-that-never-exists.json",
        ))
        .unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_real_build_excludes_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("collect.json");

        write_collect_file(&path, &["/project/oh_modules/lib/index.ets".to_string()]).unwrap();

        let plugin = HarDeduplicatePlugin::real_build(path).unwrap();
        let result = plugin
            .resolve_path("/project/oh_modules/lib/index.ets")
            .unwrap();

        let output = result.unwrap();
        assert!(matches!(
            output.external,
            Some(ResolvedExternal::Bool(true))
        ));
    }

    #[test]
    fn test_non_excluded_passes_through() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("collect.json");

        write_collect_file(&path, &["/other/file.ets".to_string()]).unwrap();

        let plugin = HarDeduplicatePlugin::real_build(path).unwrap();
        let result = plugin
            .resolve_path("/project/entry/src/main/ets/Index.ets")
            .unwrap();

        assert!(result.is_none());
    }
}
