use std::collections::HashSet;
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

        eprintln!(
            "arkdown: watching {} director{} for changes",
            self.watch_dirs.iter().filter(|d| d.exists()).count(),
            if self.watch_dirs.len() == 1 { "y" } else { "ies" }
        );

        loop {
            let changed = collect_changes(&rx, self.debounce);
            if !changed.is_empty() {
                let start = Instant::now();
                eprintln!("arkdown: {} file(s) changed, rebuilding...", changed.len());
                if let Err(e) = on_change(&changed) {
                    eprintln!("arkdown: rebuild failed: {}", e);
                } else {
                    eprintln!(
                        "arkdown: rebuild done ({:.1}s)",
                        start.elapsed().as_secs_f32()
                    );
                }
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

    let mut paths: HashSet<PathBuf> = paths_from_event(first);

    // Collect additional events within the debounce window
    let deadline = Instant::now() + debounce;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match rx.recv_timeout(remaining) {
            Ok(Ok(evt)) => {
                paths.extend(paths_from_event(evt));
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

fn paths_from_event(evt: Event) -> HashSet<PathBuf> {
    evt.paths.into_iter().collect()
}

/// Build the list of directories to watch for an OHOS module.
pub fn watch_dirs_for_module(module_src_path: &Path) -> Vec<PathBuf> {
    vec![
        module_src_path.join("src/main/ets"),
        module_src_path.join("src/main/resources"),
    ]
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
                },
            },
            modules: vec![],
            sdk_config: arkdown_core::SdkConfig {
                ets_loader_path: PathBuf::from("/sdk"),
                es2abc_path: PathBuf::from("/sdk/es2abc"),
                restool_path: PathBuf::from("/sdk/restool"),
                compile_api_version: 12,
                has_rollup_plugin: true,
            },
        })
    }

    #[test]
    fn test_watch_dirs_for_module() {
        let dirs = watch_dirs_for_module(Path::new("/project/entry"));
        assert!(dirs.iter().any(|d| d.ends_with("src/main/ets")));
        assert!(dirs.iter().any(|d| d.ends_with("src/main/resources")));
    }

    #[test]
    fn test_watcher_creation() {
        let config = make_test_config();
        // Just verify construction doesn't panic
        let _watcher = FileWatcher::new(config, vec![]);
    }
}
