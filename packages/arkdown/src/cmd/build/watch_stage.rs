//! Watch mode coordinator.
//!
//! Wraps `arkdown-watcher`'s `FileWatcher` and drives incremental rebuilds
//! when source files change.  This is a thin coordinator that:
//!
//! 1. Determines which directories to watch based on the build mode
//!    (full watch vs preview/resource-only).
//! 2. Computes an importer map snapshot from the previous build's
//!    `importer_map.json` files for M2 hot-reload delta gating.
//! 3. Classifies each change batch as **hot** (ETS/TS source only) or **cold**
//!    (config/resource change) using `classify_watch_change`.
//!    - Hot → skips resource + profile stages (bundle-only rebuild).
//!    - Cold → full pipeline rebuild.
//! 4. Calls `BuildCommand::execute` with the appropriate `skip_*` flags set.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use rustc_hash::{FxHashMap, FxHashSet};

use arkdown_core::ArkdownConfig;
use arkdown_path::ArtifactPathSerializer;
use arkdown_watcher::{classify_watch_change, FileWatcher, WatchChangeKind};

use arkdown::plugins::factory::PackagePlugin;

use super::args::{AbcBackendChoice, BuildArgs, BuildCommand, ModuleAnalyzerChoice};
use super::layout::ModuleBuildLayout;

/// Input for the watch stage.
///
/// Individual fields are used rather than a `BuildArgs` reference because
/// `BuildArgs` does not implement `Clone` (it is a clap `Args` struct).
pub(super) struct WatchStageInput {
  pub(super) config: Arc<ArkdownConfig>,
  /// Build target string (e.g. "hap", "hsp", "har").
  pub(super) target: String,
  /// Build mode string (e.g. "debug", "release").
  pub(super) mode: String,
  /// Optional module filter.
  pub(super) module_filter: Option<String>,
  /// Optional product filter.
  pub(super) product_filter: Option<String>,
  /// Project root path.
  pub(super) project_root: PathBuf,
  /// ABC backend choice for incremental rebuilds.
  pub(super) abc_backend: AbcBackendChoice,
  pub(super) build_target_name: Option<String>,
  pub(super) coverage: bool,
  pub(super) ohos_debug_asan: bool,
  pub(super) ohos_enable_hwasan: bool,
  pub(super) disable_abc_cache: bool,
  pub(super) cache_file: Option<PathBuf>,
  pub(super) checker: Option<String>,
  pub(super) module_analyzer: ModuleAnalyzerChoice,
  pub(super) cache: bool,
  pub(super) package_plugins: Vec<PackagePlugin>,
  /// When `true`, only `src/main/resources/` directories are watched.
  pub(super) preview_mode: bool,
}

/// Watch stage coordinator.
pub(super) struct WatchStage;

impl WatchStage {
  /// Start watching for file changes and drive incremental rebuilds.
  ///
  /// This call blocks until the user sends `Ctrl+C` or the watcher
  /// encounters a fatal error.
  pub(super) fn run(input: WatchStageInput) -> Result<()> {
    let config_for_watch = input.config.clone();
    let preview_mode = input.preview_mode;
    let target = input.target;
    let mode = input.mode;
    let module_filter = input.module_filter;
    let product_filter = input.product_filter;
    let project_root = input.project_root;
    let abc_backend = input.abc_backend;
    let build_target_name = input.build_target_name;
    let coverage = input.coverage;
    let ohos_debug_asan = input.ohos_debug_asan;
    let ohos_enable_hwasan = input.ohos_enable_hwasan;
    let disable_abc_cache = input.disable_abc_cache;
    let cache_file = input.cache_file;
    let checker = input.checker;
    let module_analyzer = input.module_analyzer;
    let cache = input.cache;
    let package_plugins = input.package_plugins;

    // Determine which directories to watch.
    let watch_dirs: Vec<PathBuf> = if preview_mode {
      config_for_watch
        .modules
        .iter()
        .map(|m| ModuleBuildLayout::new(&m.src_path).source().resources())
        .filter(|p| p.exists())
        .collect()
    } else {
      config_for_watch
        .modules
        .iter()
        .flat_map(|m| FileWatcher::dirs_for_module(&m.src_path))
        .collect()
    };

    // Snapshot importer maps for M2 hot-reload delta computation.
    let importer_maps: FxHashMap<String, Vec<String>> = config_for_watch
      .modules
      .iter()
      .filter_map(|m| {
        let map_path = ModuleBuildLayout::new(&m.src_path).importer_map_json();
        std::fs::read_to_string(&map_path)
          .ok()
          .and_then(|s| serde_json::from_str::<FxHashMap<String, Vec<String>>>(&s).ok())
      })
      .flatten()
      .collect();

    // Pre-compute module source roots for hot/cold classification.
    // Each element is `<module_root>/src/main` — the root under which
    // ETS/TS source files live.  Used by `classify_watch_change`.
    let module_source_roots: Vec<PathBuf> = config_for_watch
      .modules
      .iter()
      .map(|m| m.src_path.join("src").join("main"))
      .collect();

    let watcher = FileWatcher::new(config_for_watch.clone(), watch_dirs);

    tracing::info!("watching for changes");
    if preview_mode {
      tracing::info!("preview watch enabled for resource-only recompilation");
    }

    let rt = tokio::runtime::Handle::current();

    watcher
      .watch(move |changed| {
        // M2: Hot-reload delta computation.
        let changed_paths: Vec<WatchChangedPath> = changed
          .iter()
          .map(|path| WatchChangedPath::new(path, &project_root))
          .collect::<std::result::Result<Vec<_>, _>>()
          .map_err(|error| format!("serialize changed path failed: {error}"))?;

        let mut affected_chunks: FxHashSet<String> = FxHashSet::default();
        for changed_path in &changed_paths {
          for key in importer_maps.keys() {
            if changed_path.matches_importer_key(key) {
              affected_chunks.insert(key.clone());
            }
          }
        }

        if !importer_maps.is_empty() && affected_chunks.is_empty() {
          tracing::info!(
            changed_files = changed.len(),
            "changed files are not in module graph; skipping rebuild"
          );
          return Ok(());
        }

        // M3: Preview mode — write changed resource list for IDE polling.
        if preview_mode {
          let preview_json = serde_json::json!({
              "type": "resource_changed",
              "files": changed
                  .iter()
                  .map(|path| ArtifactPathSerializer::path_to_slash_string(path))
                  .collect::<std::result::Result<Vec<_>, _>>()
                  .map_err(|error| {
                      format!("serialize preview changed path failed: {error}")
                  })?,
          });
          for module in &config_for_watch.modules {
            let out_dir = ModuleBuildLayout::new(&module.src_path).outputs_root();
            let _ = std::fs::create_dir_all(&out_dir);
            let _ = std::fs::write(
              out_dir.join("preview_changes.json"),
              serde_json::to_string_pretty(&preview_json).unwrap_or_default(),
            );
          }
        }

        // Hot/cold classification: if ALL changed files are ETS/TS source
        // files under a known source root, this is a hot change and we can
        // skip resource + profile stages for a faster bundle-only rebuild.
        // If ANY file is cold (config/resource), do a full rebuild.
        //
        // In preview mode all changes are treated as Cold since resource
        // files are the only thing being watched.
        let change_kind = if preview_mode {
          WatchChangeKind::Cold
        } else {
          let source_root_refs: Vec<&Path> =
            module_source_roots.iter().map(|p| p.as_path()).collect();
          changed.iter().fold(WatchChangeKind::Hot, |acc, path| {
            match (acc, classify_watch_change(path, &source_root_refs)) {
              (WatchChangeKind::Cold, _) | (_, WatchChangeKind::Cold) => WatchChangeKind::Cold,
              _ => WatchChangeKind::Hot,
            }
          })
        };

        let mode_label = match change_kind {
          WatchChangeKind::Hot => "hot rebuild (bundle-only)",
          WatchChangeKind::Cold => {
            if preview_mode {
              "resource reload"
            } else {
              "full rebuild"
            }
          }
        };
        tracing::info!(
          changed_files = changed.len(),
          mode = mode_label,
          affected_chunks = if affected_chunks.is_empty() {
            "all".to_string()
          } else {
            affected_chunks.len().to_string()
          },
          "watch rebuild started"
        );

        // For hot changes, skip resource and profile stages to only
        // rerun the bundle → bytecode → package pipeline.
        let is_hot = matches!(change_kind, WatchChangeKind::Hot);
        let rebuild_args = BuildArgs {
          target: target.clone(),
          mode: mode.clone(),
          module: module_filter.clone(),
          product: product_filter.clone(),
          project: Some(project_root.clone()),
          build_target_name: build_target_name.clone(),
          watch: false,
          coverage,
          ohos_debug_asan,
          ohos_enable_hwasan,
          preview: false,
          disable_abc_cache,
          cache_file: cache_file.clone(),
          abc_backend,
          checker: checker.clone(),
          module_analyzer,
          cache,
          skip_resource: is_hot,
          skip_profile: is_hot,
        };
        if let Err(e) = rt.block_on(
          BuildCommand::new(rebuild_args)
            .with_package_plugins(package_plugins.clone())
            .execute(),
        ) {
          tracing::error!(error = %format!("{e:#}"), "watch rebuild failed");
        }
        Ok(())
      })
      .map_err(|e| anyhow::anyhow!("watch stage error: {e}"))
  }
}

struct WatchChangedPath {
  candidates: FxHashSet<String>,
}

impl WatchChangedPath {
  fn new(path: &Path, project_root: &Path) -> std::result::Result<Self, String> {
    let mut changed_path = Self {
      candidates: FxHashSet::default(),
    };
    changed_path.insert_path(path)?;
    if let Ok(relative) = ArtifactPathSerializer::source_file_key(project_root, path) {
      changed_path.insert_slash_path(relative.into_string());
    }
    Ok(changed_path)
  }

  fn matches_importer_key(&self, key: &str) -> bool {
    let importer_key = WatchImporterKey::new(key);
    self
      .candidates
      .iter()
      .any(|candidate| importer_key.matches_candidate(candidate))
  }

  fn insert_path(&mut self, path: &Path) -> std::result::Result<(), String> {
    let slash_path =
      ArtifactPathSerializer::path_to_slash_string(path).map_err(|error| error.to_string())?;
    self.insert_slash_path(slash_path);
    Ok(())
  }

  fn insert_slash_path(&mut self, slash_path: String) {
    if slash_path.is_empty() {
      return;
    }
    if let Some(extensionless) = WatchImporterKey::new(&slash_path).extensionless_path() {
      self.candidates.insert(extensionless);
    }
    self.candidates.insert(slash_path);
  }
}

struct WatchImporterKey<'a> {
  key: &'a str,
}

impl<'a> WatchImporterKey<'a> {
  fn new(key: &'a str) -> Self {
    Self { key }
  }

  fn matches_candidate(&self, candidate: &str) -> bool {
    if self.key.is_empty() || candidate.is_empty() {
      return false;
    }
    if self.key == candidate {
      return true;
    }
    let key_path = Path::new(self.key);
    let candidate_path = Path::new(candidate);
    key_path.ends_with(candidate_path) || candidate_path.ends_with(key_path)
  }

  fn extensionless_path(&self) -> Option<String> {
    let path = Path::new(self.key);
    path.extension()?;
    let file_stem = path.file_stem()?;
    let mut extensionless = path.to_path_buf();
    extensionless.set_file_name(file_stem);
    Some(ArtifactPathSerializer::rel_to_slash(&extensionless))
  }
}
