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
//! Previously the standalone `arkdown-plugin-har-dedup` crate.

use rustc_hash::FxHashSet;
use std::borrow::Cow;
use std::path::{Path, PathBuf};

use arkdown_path::canonicalize_or_owned;
use rolldown_plugin::{
    HookBuildEndArgs, HookLoadArgs, HookLoadOutput, HookLoadReturn, HookNoopReturn,
    HookResolveIdArgs, HookResolveIdOutput, HookResolveIdReturn, HookUsage, Plugin, PluginContext,
    SharedLoadPluginContext,
};

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
pub struct HarDedupPlugin {
    mode: HarDedupMode,
    collect_file_path: PathBuf,
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
        })
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

    /// `hsp-collect-har-files.js::buildEnd/collectSourceFiles` reads the final
    /// Rollup graph through `this.getModuleIds()`. Collecting here avoids a
    /// shared write on every concurrent `moduleParsed` hook and preserves the
    /// same graph boundary.
    pub(crate) fn flush_collect<I, S>(&self, module_ids: I) -> Result<(), HarDedupError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        if matches!(self.mode, HarDedupMode::CollectPass { .. }) {
            let files = module_ids
                .into_iter()
                .filter_map(|id| {
                    let id = id.as_ref();
                    self.should_collect_source(id).then(|| {
                        canonicalize_or_owned(Path::new(id.strip_prefix('\0').unwrap_or(id)))
                            .to_string_lossy()
                            .into_owned()
                    })
                })
                .collect::<Vec<_>>();
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
        if id.starts_with('\0') {
            return false;
        }
        let path = canonicalize_or_owned(Path::new(id.strip_prefix('\0').unwrap_or(id)));
        source_har_roots.iter().any(|root| path.starts_with(root))
    }
}

impl Plugin for HarDedupPlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("har-dedup")
    }

    fn register_hook_usage(&self) -> HookUsage {
        match self.mode {
            HarDedupMode::CollectPass { .. } => HookUsage::BuildEnd,
            HarDedupMode::RealBuild { .. } => HookUsage::ResolveId | HookUsage::Load,
        }
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

    fn build_end(
        &self,
        ctx: &PluginContext,
        _args: Option<&HookBuildEndArgs<'_>>,
    ) -> impl std::future::Future<Output = HookNoopReturn> + Send {
        let result = self.flush_collect(ctx.get_module_ids());
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
// Error type
// ---------------------------------------------------------------------------

/// Errors produced by the HAR dedup plugin.
#[derive(Debug, thiserror::Error)]
pub enum HarDedupError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
