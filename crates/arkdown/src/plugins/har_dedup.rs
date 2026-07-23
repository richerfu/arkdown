//! HAR deduplication plugin for HSP builds.
//!
//! Operates in two tasks in one HSP build, matching hvigor's
//! `CollectDuplicateHarEntriesArkCompile` followed by the real
//! `CompileArkTS` task:
//! - **collect_pass** records only modules owned by source HAR roots selected
//!   by the project-level duplicate dependency planner.
//! - **real_build** replaces those exact modules with empty virtual modules so
//!   Rolldown keeps import bindings stable without bundling duplicate code.
//!
//! After the bundle completes, call [`HarDedupPlugin::dedup_facts`] to obtain
//! the collected [`HarDedupFact`] records for downstream stages.
//!
//! Previously the standalone `arkdown-plugin-har-dedup` crate.

use rustc_hash::FxHashSet;
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use arkdown_path::canonicalize_or_owned;
use rolldown_common::{ModuleInfo, NormalModule};
use rolldown_plugin::{
    HookBuildEndArgs, HookLoadArgs, HookLoadOutput, HookLoadReturn, HookNoopReturn,
    HookResolveIdArgs, HookResolveIdOutput, HookResolveIdReturn, HookUsage, Plugin, PluginContext,
    SharedLoadPluginContext,
};

use crate::facts::HarDedupFact;

// ---------------------------------------------------------------------------
// Mode
// ---------------------------------------------------------------------------

/// Operating mode for [`HarDedupPlugin`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HarDedupMode {
    /// First task: collect modules beneath the selected duplicate Source HAR
    /// roots and write them to `collect_file_path`.
    CollectPass { source_har_roots: Vec<PathBuf> },
    /// Second task: read the collection written by the first task and replace
    /// those exact source modules with empty modules.
    RealBuild { excluded_files: FxHashSet<String> },
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/// HAR deduplication plugin for HSP builds.
///
/// Use [`HarDedupPlugin::collect_pass`] for the first bundle run and
/// [`HarDedupPlugin::real_build`] for subsequent runs.  After bundling, call
/// [`HarDedupPlugin::dedup_facts`] to obtain the [`HarDedupFact`] records.
pub struct HarDedupPlugin {
    mode: HarDedupMode,
    collect_file_path: PathBuf,
    /// Module IDs collected during the collect pass (oh_modules entries).
    collected: Arc<Mutex<Vec<String>>>,
}

impl std::fmt::Debug for HarDedupPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HarDedupPlugin")
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}

impl HarDedupPlugin {
    /// Create a plugin instance for the collect pass.
    ///
    /// During this pass every `oh_modules` module ID is recorded and written
    /// to `collect_file_path` when the build ends.
    pub fn collect_pass(collect_file_path: PathBuf, source_har_roots: Vec<PathBuf>) -> Self {
        let mut source_har_roots = source_har_roots
            .into_iter()
            .map(|root| canonicalize_or_owned(&root))
            .collect::<Vec<_>>();
        source_har_roots.sort();
        source_har_roots.dedup();
        HarDedupPlugin {
            mode: HarDedupMode::CollectPass { source_har_roots },
            collect_file_path,
            collected: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Create a plugin instance for the real build pass.
    ///
    /// Reads the collect file written by a previous [`collect_pass`] run and
    /// replaces those exact modules with empty virtual modules during load.
    ///
    /// [`collect_pass`]: HarDedupPlugin::collect_pass
    pub fn real_build(collect_file_path: PathBuf) -> Result<Self, HarDedupError> {
        let excluded = CollectFile::read(&collect_file_path)?;
        Ok(HarDedupPlugin {
            mode: HarDedupMode::RealBuild {
                excluded_files: excluded,
            },
            collect_file_path,
            collected: Arc::new(Mutex::new(Vec::new())),
        })
    }

    /// Return [`HarDedupFact`] records for every module that was deduplicated.
    ///
    /// In `CollectPass` mode this returns the modules collected during the
    /// current run (each marked as its own canonical source).  In `RealBuild`
    /// mode this returns facts for every module that was excluded as external,
    /// using the collect file path stem as the HAR package name.
    pub fn dedup_facts(&self) -> Vec<HarDedupFact> {
        match &self.mode {
            HarDedupMode::CollectPass { .. } => {
                let ids = self.collected.lock().unwrap_or_else(|p| p.into_inner());
                let har_name = CollectFilePath::new(&self.collect_file_path).har_name();
                ids.iter()
                    .map(|id| HarDedupFact {
                        module_id: id.clone(),
                        source_hars: vec![har_name.clone()],
                        canonical_har: har_name.clone(),
                    })
                    .collect()
            }
            HarDedupMode::RealBuild { excluded_files } => {
                let har_name = CollectFilePath::new(&self.collect_file_path).har_name();
                excluded_files
                    .iter()
                    .map(|module_id| HarDedupFact {
                        module_id: module_id.clone(),
                        source_hars: vec![har_name.clone()],
                        canonical_har: har_name.clone(),
                    })
                    .collect()
            }
        }
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    pub(crate) fn resolve_path(
        &self,
        specifier: &str,
    ) -> anyhow::Result<Option<HookResolveIdOutput>> {
        if let HarDedupMode::RealBuild { ref excluded_files } = self.mode {
            if excluded_files.contains(specifier) {
                return Ok(Some(HookResolveIdOutput {
                    id: specifier.into(),
                    ..Default::default()
                }));
            }
        }
        Ok(None)
    }

    pub(crate) fn flush_collect(&self) -> Result<(), HarDedupError> {
        if matches!(self.mode, HarDedupMode::CollectPass { .. }) {
            let files = self
                .collected
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .clone();
            CollectFile::write(&self.collect_file_path, &files)?;
        }
        Ok(())
    }

    pub(crate) fn load_module(&self, id: &str) -> HookLoadReturn {
        let HarDedupMode::RealBuild { excluded_files } = &self.mode else {
            return Ok(None);
        };
        let normalized_id = id.strip_prefix('\0').unwrap_or(id);
        if !excluded_files.contains(normalized_id) && !excluded_files.contains(id) {
            return Ok(None);
        }
        Ok(Some(HookLoadOutput {
            code: "".into(),
            ..Default::default()
        }))
    }

    pub(crate) fn should_collect_source(&self, id: &str) -> bool {
        let HarDedupMode::CollectPass { source_har_roots } = &self.mode else {
            return false;
        };
        let path = canonicalize_or_owned(Path::new(id.strip_prefix('\0').unwrap_or(id)));
        source_har_roots.iter().any(|root| path.starts_with(root))
    }
}

impl Plugin for HarDedupPlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("har-dedup")
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::ResolveId | HookUsage::Load | HookUsage::ModuleParsed | HookUsage::BuildEnd
    }

    fn load(
        &self,
        _ctx: SharedLoadPluginContext,
        args: &HookLoadArgs<'_>,
    ) -> impl std::future::Future<Output = HookLoadReturn> + Send {
        let result = self.load_module(args.id);
        async move { result }
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
        let should_collect = self.should_collect_source(&id);
        async move {
            if should_collect {
                let path = canonicalize_or_owned(Path::new(id.strip_prefix('\0').unwrap_or(&id)));
                collected
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .push(path.to_string_lossy().into_owned());
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

// ---------------------------------------------------------------------------
// Collect file (newline-separated manifest on disk)
// ---------------------------------------------------------------------------

struct CollectFile {
    source_har_files: FxHashSet<String>,
}

impl CollectFile {
    fn write(path: &Path, files: &[String]) -> Result<(), HarDedupError> {
        let mut source_har_files = files.to_vec();
        source_har_files.sort();
        source_har_files.dedup();
        if let Some(parent) = path.parent() {
            fs_err::create_dir_all(parent)?;
        }
        fs_err::write(path, source_har_files.join("\n"))?;
        Ok(())
    }

    fn read(path: &Path) -> Result<FxHashSet<String>, HarDedupError> {
        if !path.exists() {
            return Ok(FxHashSet::default());
        }
        let content = fs_err::read_to_string(path)?;
        Ok(CollectFile {
            source_har_files: content
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_string)
                .collect(),
        }
        .source_har_files)
    }
}

// ---------------------------------------------------------------------------
// Collect file paths
// ---------------------------------------------------------------------------

pub(crate) struct CollectFilePath<'a> {
    path: &'a Path,
}

impl<'a> CollectFilePath<'a> {
    pub(crate) fn new(path: &'a Path) -> Self {
        Self { path }
    }

    pub(crate) fn default_for_module(module_root: &Path) -> PathBuf {
        module_root
            .join("build")
            .join("default")
            .join("intermediates")
            .join("har_files")
            .join("default")
            .join("har_files.json")
    }

    /// Derive a short HAR package name from the collect file path.
    ///
    /// Uses the file stem of the collect file (e.g. `"har_files"` from
    /// `intermediates/har_dedup/har_files.json`) as a stable identifier.
    fn har_name(&self) -> String {
        self.path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown_har")
            .to_string()
    }
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors produced by the HAR dedup plugin.
#[derive(Debug, thiserror::Error)]
pub enum HarDedupError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
