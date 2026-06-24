//! Watch configuration bridge for the bundle stage.
//!
//! [`WatchConfigBridge`] adapts hvigor's `getWatchConfig` and
//! `rollup-plugin-watch-change.js` to the Rolldown watch API.
//!
//! # Upstream correspondence
//!
//! | Type | hvigor/SDK equivalent |
//! |------|-----------------------|
//! | `WatchConfigBridge` | `arkts-pack.js::getWatchConfig` |
//! | `WatchInvalidationEvent` | watch change plugin change event |
//! | `WatchParityPolicy` | watch mode parity behavior policy |

use std::path::PathBuf;

// ---------------------------------------------------------------------------
// WatchInvalidationEvent
// ---------------------------------------------------------------------------

/// A file change event that may invalidate the bundle cache.
///
/// # Upstream correspondence
///
/// Corresponds to events produced by `rollup-plugin-watch-change.js` and
/// processed by `wrapperShouldInvalidCache` in `arkts-pack.js`.
#[derive(Debug, Clone)]
pub struct WatchInvalidationEvent {
    /// The file that changed.
    pub path: PathBuf,
    /// The kind of change: `"update"`, `"create"`, or `"delete"`.
    pub kind: String,
    /// Timestamp of the change (unix seconds).
    pub timestamp: u64,
}

impl WatchInvalidationEvent {
    /// Create an "update" event.
    pub fn update(path: impl Into<PathBuf>, timestamp: u64) -> Self {
        Self {
            path: path.into(),
            kind: "update".to_string(),
            timestamp,
        }
    }

    /// Create a "create" event.
    pub fn create(path: impl Into<PathBuf>, timestamp: u64) -> Self {
        Self {
            path: path.into(),
            kind: "create".to_string(),
            timestamp,
        }
    }

    /// Create a "delete" event.
    pub fn delete(path: impl Into<PathBuf>, timestamp: u64) -> Self {
        Self {
            path: path.into(),
            kind: "delete".to_string(),
            timestamp,
        }
    }
}

// ---------------------------------------------------------------------------
// WatchParityPolicy
// ---------------------------------------------------------------------------

/// Policy governing watch mode parity with hvigor.
///
/// # Upstream correspondence
///
/// Corresponds to hvigor's watch-mode behavior flags including
/// `shouldSkipBuildEnd` and incremental build policy.
#[derive(Debug, Clone)]
pub struct WatchParityPolicy {
    /// Whether to skip the `buildEnd` hook on cache-hit builds.
    ///
    /// hvigor skips `buildEnd` when the cache is still valid to avoid
    /// unnecessary ABC recompilation.
    pub skip_build_end_on_cache_hit: bool,
    /// Minimum file change size to trigger a rebuild (bytes).
    ///
    /// hvigor skips rebuilds for trivial whitespace-only changes.
    pub min_change_size_bytes: u64,
}

impl Default for WatchParityPolicy {
    fn default() -> Self {
        Self {
            skip_build_end_on_cache_hit: true,
            min_change_size_bytes: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// WatchConfigBridge
// ---------------------------------------------------------------------------

/// Adapts hvigor's watch configuration to Rolldown.
///
/// # Upstream correspondence
///
/// Corresponds to `arkts-pack.js::getWatchConfig` — the watch configuration
/// passed to Rolldown in watch mode builds.
pub struct WatchConfigBridge {
    /// Directories to watch for changes.
    pub watch_dirs: Vec<PathBuf>,
    /// File extensions to watch (e.g., `[".ets", ".ts", ".js"]`).
    pub extensions: Vec<String>,
    /// Watch parity policy.
    pub policy: WatchParityPolicy,
}

impl WatchConfigBridge {
    /// Create from a bundle plan (module root and source extensions).
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `arkts-pack.js::getWatchConfig`.
    pub fn from_plan(
        module_root: impl Into<PathBuf>,
        extensions: Vec<String>,
        policy: WatchParityPolicy,
    ) -> Self {
        let root = module_root.into();
        Self {
            watch_dirs: vec![root.join("src")],
            extensions,
            policy,
        }
    }

    /// Determine the cache invalidation event for a changed file.
    ///
    /// Returns `None` if the file change should be ignored.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `rollup-plugin-watch-change.js` event filtering.
    pub fn event_for_changed_file<'a>(
        &self,
        event: &'a WatchInvalidationEvent,
    ) -> Option<&'a WatchInvalidationEvent> {
        // Check if the extension is watched.
        let ext = event
            .path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let watched = self
            .extensions
            .iter()
            .any(|extension| WatchExtension::new(extension).matches_path_extension(ext));
        if !watched {
            return None;
        }
        // Check if the file is in a watched directory.
        let in_dir = self
            .watch_dirs
            .iter()
            .any(|dir| event.path.starts_with(dir));
        if !in_dir {
            return None;
        }
        Some(event)
    }

    /// Return `true` if the build end should be skipped on a cache hit.
    pub fn should_skip_build_end(&self) -> bool {
        self.policy.skip_build_end_on_cache_hit
    }

    /// Return the set of directories to watch as rolldown watch inputs.
    pub fn to_watcher_inputs(&self) -> Vec<PathBuf> {
        self.watch_dirs.clone()
    }
}

struct WatchExtension<'a> {
    raw: &'a str,
}

impl<'a> WatchExtension<'a> {
    fn new(raw: &'a str) -> Self {
        Self { raw }
    }

    fn matches_path_extension(&self, path_extension: &str) -> bool {
        self.normalized() == path_extension
    }

    fn normalized(&self) -> &'a str {
        self.raw.strip_prefix('.').unwrap_or(self.raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_for_ts_file_in_watched_dir() {
        let bridge = WatchConfigBridge::from_plan(
            "/project/entry",
            vec![".ets".to_string(), ".ts".to_string()],
            WatchParityPolicy::default(),
        );
        let event = WatchInvalidationEvent::update("/project/entry/src/pages/Index.ets", 0);
        assert!(bridge.event_for_changed_file(&event).is_some());
    }

    #[test]
    fn event_for_json_not_watched() {
        let bridge = WatchConfigBridge::from_plan(
            "/project/entry",
            vec![".ets".to_string(), ".ts".to_string()],
            WatchParityPolicy::default(),
        );
        let event = WatchInvalidationEvent::update("/project/entry/src/config.json", 0);
        assert!(bridge.event_for_changed_file(&event).is_none());
    }
}
