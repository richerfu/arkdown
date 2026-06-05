use rustc_hash::FxHashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::{bounded, Receiver};
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};

use arkdown_core::ArkdownConfig;

#[derive(Debug, thiserror::Error)]
pub enum WatchError {
    #[error("Filesystem watcher error: {0}")]
    Notify(#[from] notify::Error),
    #[error("Build failed: {0}")]
    Build(String),
}

pub struct FileWatcher {
    config: Arc<ArkdownConfig>,
    watch_dirs: Vec<PathBuf>,
    debounce: Duration,
}

impl FileWatcher {
    pub fn new(config: Arc<ArkdownConfig>, watch_dirs: Vec<PathBuf>) -> Self {
        FileWatcher {
            config,
            watch_dirs,
            debounce: Duration::from_millis(100),
        }
    }

    /// Return the configuration this watcher was built from.
    pub fn config(&self) -> &ArkdownConfig {
        &self.config
    }

    /// Start watching. Calls `on_change` with a list of changed file paths
    /// whenever a debounced batch of changes occurs. Runs until the process exits.
    pub fn watch<F>(&self, mut on_change: F) -> Result<(), WatchError>
    where
        F: FnMut(&[PathBuf]) -> Result<(), String>,
    {
        let (tx, rx) = bounded::<notify::Result<Event>>(100);

        let mut watcher = RecommendedWatcher::new(
            move |evt| {
                let _ = tx.send(evt);
            },
            Config::default(),
        )?;

        for dir in &self.watch_dirs {
            if dir.exists() {
                watcher.watch(dir, RecursiveMode::Recursive)?;
            }
        }

        tracing::info!(
            watch_dir_count = self.watch_dirs.iter().filter(|d| d.exists()).count(),
            "arkdown watcher started"
        );

        loop {
            let changed = Self::collect_changes(&rx, self.debounce);
            if !changed.is_empty() {
                let start = Instant::now();
                tracing::info!(
                    changed_count = changed.len(),
                    "arkdown watcher rebuild started"
                );
                if let Err(e) = on_change(&changed) {
                    tracing::error!(error = %e, "arkdown watcher rebuild failed");
                } else {
                    tracing::info!(
                        elapsed_ms = start.elapsed().as_millis(),
                        "arkdown watcher rebuild finished"
                    );
                }
            }
        }
    }

    /// Collect filesystem events from the channel, debouncing with a wait window.
    /// Returns a deduplicated list of changed file paths.
    fn collect_changes(rx: &Receiver<notify::Result<Event>>, debounce: Duration) -> Vec<PathBuf> {
        // Wait for first event (blocking, up to 5 minutes)
        let first = match rx.recv_timeout(Duration::from_secs(300)) {
            Ok(Ok(evt)) => evt,
            _ => return vec![],
        };

        let mut paths: FxHashSet<PathBuf> = Self::paths_from_event(first);

        // Collect additional events within the debounce window
        let deadline = Instant::now() + debounce;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            match rx.recv_timeout(remaining) {
                Ok(Ok(evt)) => {
                    paths.extend(Self::paths_from_event(evt));
                }
                Ok(Err(_)) | Err(_) => break,
            }
        }

        // Filter to source files only
        paths
            .into_iter()
            .filter(|p| {
                let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
                matches!(ext, "ets" | "ts" | "js" | "json" | "json5")
            })
            .collect()
    }

    fn paths_from_event(evt: Event) -> FxHashSet<PathBuf> {
        evt.paths.into_iter().collect()
    }

    /// Build the list of directories to watch for an OHOS module.
    pub fn dirs_for_module(module_src_path: &Path) -> Vec<PathBuf> {
        vec![
            module_src_path.join("src/main/ets"),
            module_src_path.join("src/main/resources"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Arc;

    fn make_test_config() -> Arc<ArkdownConfig> {
        Arc::new(ArkdownConfig {
            project_root: PathBuf::from("/tmp"),
            build_mode: arkdown_core::BuildMode::Debug,
            output_target: arkdown_core::OutputTarget::Hap,
            product: arkdown_core::ProductConfig {
                name: "default".to_string(),
                bundle_name: "com.test".to_string(),
                compatible_sdk_version: "5.0.0(12)".to_string(),
                compatible_sdk_version_stage: None,
                runtime_os: "HarmonyOS".to_string(),
                signing_config: None,
                build_option: arkdown_core::BuildOption {
                    strict_mode: arkdown_core::StrictModeConfig {
                        case_sensitive_check: false,
                        use_normalized_ohm_url: false,
                    },
                    ark_options: None,
                    res_options: None,
                    source_option: None,
                    preload_system_so: false,
                    preload_so: vec![],
                    remove_permissions: vec![],
                    aot_compile_mode: None,
                    host_pgo: None,
                    ap_path: None,
                    abi_filters: vec![],
                },
            },
            modules: vec![],
            sdk_config: arkdown_core::SdkConfig {
                ets_loader_path: PathBuf::from("/sdk"),
                es2abc_path: PathBuf::from("/sdk/es2abc"),
                restool_path: PathBuf::from("/sdk/restool"),
                compile_api_version: 12,
                has_rollup_plugin: true,
                compile_sdk_version: "6.0.0.47".to_string(),
            },
        })
    }

    #[test]
    fn test_watch_dirs_for_module() {
        let dirs = FileWatcher::dirs_for_module(Path::new("/project/entry"));
        assert!(dirs.iter().any(|d| d.ends_with("src/main/ets")));
        assert!(dirs.iter().any(|d| d.ends_with("src/main/resources")));
    }

    #[test]
    fn test_watch_dirs_for_module_count() {
        // Should return exactly 2 directories: ets + resources
        let dirs = FileWatcher::dirs_for_module(Path::new("/project/entry"));
        assert_eq!(
            dirs.len(),
            2,
            "watch_dirs_for_module must return exactly 2 dirs"
        );
    }

    #[test]
    fn test_watcher_creation() {
        let config = make_test_config();
        // Just verify construction doesn't panic
        let _watcher = FileWatcher::new(config, vec![]);
    }

    #[test]
    fn test_debounce_is_100ms() {
        let config = make_test_config();
        let watcher = FileWatcher::new(config, vec![]);
        // hvigor buildDelay is 100ms — verify our debounce matches
        assert_eq!(
            watcher.debounce,
            Duration::from_millis(100),
            "debounce must be 100ms to match hvigor buildDelay"
        );
    }

    #[test]
    fn test_watcher_config_accessor() {
        let config = make_test_config();
        let watcher = FileWatcher::new(config.clone(), vec![]);
        assert_eq!(watcher.config().project_root, PathBuf::from("/tmp"));
    }
}
