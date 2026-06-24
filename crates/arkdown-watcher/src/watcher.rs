//! Session-aware filesystem watcher.
//!
//! `SessionWatcher` watches a typed set of directories derived from
//! `BuildSession.input_inventory` (passed as `WatchInput`) rather than
//! guessing raw directory paths.
//!
//! Upstream: `arkts-pack.js::watch` mode.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crossbeam_channel::{bounded, Receiver, Sender};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use rustc_hash::FxHashSet;

use crate::WatchError;

// ---------------------------------------------------------------------------
// WatchEventKind
// ---------------------------------------------------------------------------

/// The kind of filesystem change that triggered a `WatchEvent`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchEventKind {
    /// A new file was created.
    Created,
    /// An existing file was modified.
    Modified,
    /// A file was deleted.
    Deleted,
}

// ---------------------------------------------------------------------------
// WatchEvent
// ---------------------------------------------------------------------------

/// A single debounce-eligible filesystem change event.
#[derive(Debug, Clone)]
pub struct WatchEvent {
    /// Absolute path of the changed file.
    pub path: PathBuf,
    /// Nature of the change.
    pub kind: WatchEventKind,
    /// Monotonic timestamp of when the event was received by the watcher.
    pub timestamp: Instant,
}

impl WatchEvent {
    /// Construct a new event.
    pub fn new(path: PathBuf, kind: WatchEventKind) -> Self {
        WatchEvent {
            path,
            kind,
            timestamp: Instant::now(),
        }
    }
}

// ---------------------------------------------------------------------------
// WatchInput
// ---------------------------------------------------------------------------

/// Typed input for `SessionWatcher`, constructed by the CLI from session data.
///
/// The CLI builds this from `BuildSession.input_inventory` so that the watcher
/// crate does not need to import `arkdown-session` directly.
#[derive(Debug, Clone, Default)]
pub struct WatchInput {
    /// Directories to watch recursively, from `SourceStore` root dirs.
    pub watch_dirs: Vec<PathBuf>,
    /// File extensions to accept, from `SdkSnapshot` support extensions.
    /// Empty means "accept all extensions".
    pub watch_extensions: Vec<String>,
    /// Initial fingerprints keyed by file path, used for change detection.
    /// Populated by the CLI before the first watch cycle.
    pub fingerprint_baseline: HashMap<PathBuf, u64>,
}

impl WatchInput {
    /// Create a new empty `WatchInput`.
    pub fn new() -> Self {
        WatchInput::default()
    }

    /// Set the directories to watch.
    pub fn with_dirs(mut self, dirs: Vec<PathBuf>) -> Self {
        self.watch_dirs = dirs;
        self
    }

    /// Set the extensions to filter on.
    pub fn with_extensions(mut self, exts: Vec<String>) -> Self {
        self.watch_extensions = exts;
        self
    }

    /// Set the initial fingerprint baseline.
    pub fn with_fingerprints(mut self, baseline: HashMap<PathBuf, u64>) -> Self {
        self.fingerprint_baseline = baseline;
        self
    }

    /// Return `true` if `path` passes the extension filter.
    ///
    /// When `watch_extensions` is empty every extension is accepted.
    pub fn accepts_extension(&self, path: &Path) -> bool {
        if self.watch_extensions.is_empty() {
            return true;
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        self.watch_extensions.iter().any(|e| e == ext)
    }
}

// ---------------------------------------------------------------------------
// SessionWatcher
// ---------------------------------------------------------------------------

/// Session-aware filesystem watcher backed by the `notify` crate.
///
/// Unlike the legacy `FileWatcher`, `SessionWatcher` receives its watch
/// directories from a `WatchInput` that is constructed by the CLI from
/// `BuildSession.input_inventory`.  This avoids hard-coded path guessing and
/// lets the session layer control what is observed.
///
/// Upstream: `arkts-pack.js::watch`.
pub struct SessionWatcher {
    input: WatchInput,
    /// Collected events (append-only from the watcher thread).
    events: Arc<Mutex<Vec<WatchEvent>>>,
    /// Channel used by `stop()` to signal the background thread to exit.
    stop_tx: Sender<()>,
    stop_rx: Receiver<()>,
}

impl SessionWatcher {
    /// Create a new watcher from the given session input.
    ///
    /// The watcher is not yet running — call `watch` or `poll` to start it.
    pub fn new(input: WatchInput) -> Self {
        let (stop_tx, stop_rx) = bounded::<()>(1);
        SessionWatcher {
            input,
            events: Arc::new(Mutex::new(Vec::new())),
            stop_tx,
            stop_rx,
        }
    }

    /// Start watching all directories in `WatchInput.watch_dirs`.
    ///
    /// Calls `on_events` with each debounced batch of `WatchEvent`s.  Runs
    /// until `stop()` is called or the process exits.
    ///
    /// Upstream: `arkts-pack.js::watch`.
    pub fn watch<F>(&self, mut on_events: F) -> Result<(), WatchError>
    where
        F: FnMut(&[WatchEvent]) -> Result<(), String>,
    {
        let (tx, rx) = bounded::<notify::Result<Event>>(256);

        let mut watcher = RecommendedWatcher::new(
            move |evt| {
                let _ = tx.send(evt);
            },
            Config::default(),
        )?;

        for dir in &self.input.watch_dirs {
            if dir.exists() {
                watcher.watch(dir, RecursiveMode::Recursive)?;
                tracing::debug!(dir = %dir.display(), "session-watcher: watching dir");
            } else {
                tracing::debug!(
                    dir = %dir.display(),
                    "session-watcher: skipping non-existent dir"
                );
            }
        }

        tracing::info!(
            dir_count = self.input.watch_dirs.iter().filter(|d| d.exists()).count(),
            ext_filter = ?self.input.watch_extensions,
            "session-watcher started"
        );

        let debounce = Duration::from_millis(150);

        loop {
            // Check for stop signal (non-blocking).
            if self.stop_rx.try_recv().is_ok() {
                tracing::info!("session-watcher: stop signal received");
                break;
            }

            let batch = Self::collect_events(&rx, debounce, &self.input);
            if !batch.is_empty() {
                // Append to collected events.
                if let Ok(mut guard) = self.events.lock() {
                    guard.extend(batch.iter().cloned());
                }
                if let Err(e) = on_events(&batch) {
                    tracing::error!(error = %e, "session-watcher: on_events callback failed");
                }
            }
        }

        Ok(())
    }

    /// Non-blocking poll: drain any events that have been collected since the
    /// last call.
    ///
    /// Returns all events collected so far (does NOT reset the internal buffer).
    /// Use this when you want to drive the watcher from an external event loop
    /// rather than via the blocking `watch` callback.
    pub fn poll(&self) -> Vec<WatchEvent> {
        self.events.lock().map(|g| g.clone()).unwrap_or_default()
    }

    /// Signal the background watch loop to stop.
    ///
    /// The `watch` call will return on the next iteration after this is called.
    pub fn stop(&self) {
        let _ = self.stop_tx.try_send(());
    }

    /// Return all events collected since the watcher was created.
    pub fn events(&self) -> Vec<WatchEvent> {
        self.events.lock().map(|g| g.clone()).unwrap_or_default()
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Collect and classify notify events within a debounce window.
    fn collect_events(
        rx: &Receiver<notify::Result<Event>>,
        debounce: Duration,
        input: &WatchInput,
    ) -> Vec<WatchEvent> {
        // Wait for first event (blocking, up to 5 minutes idle timeout).
        let first = match rx.recv_timeout(Duration::from_secs(300)) {
            Ok(Ok(evt)) => evt,
            _ => return vec![],
        };

        let mut seen: FxHashSet<PathBuf> = FxHashSet::default();
        let mut events: Vec<WatchEvent> = Vec::new();

        Self::classify_event(first, input, &mut seen, &mut events);

        // Collect additional events within the debounce window.
        let deadline = Instant::now() + debounce;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            match rx.recv_timeout(remaining) {
                Ok(Ok(evt)) => {
                    Self::classify_event(evt, input, &mut seen, &mut events);
                }
                Ok(Err(_)) | Err(_) => break,
            }
        }

        events
    }

    /// Convert a raw notify event into zero or more `WatchEvent`s, filtering
    /// by extension and deduplicating by path.
    fn classify_event(
        evt: Event,
        input: &WatchInput,
        seen: &mut FxHashSet<PathBuf>,
        out: &mut Vec<WatchEvent>,
    ) {
        let kind = match evt.kind {
            EventKind::Create(_) => WatchEventKind::Created,
            EventKind::Modify(_) => WatchEventKind::Modified,
            EventKind::Remove(_) => WatchEventKind::Deleted,
            _ => return, // Access, Other, Any — not actionable
        };

        for path in evt.paths {
            if !input.accepts_extension(&path) {
                continue;
            }
            if seen.insert(path.clone()) {
                out.push(WatchEvent::new(path, kind));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watch_input_default_accepts_all_extensions() {
        let input = WatchInput::new();
        assert!(input.accepts_extension(Path::new("foo.ets")));
        assert!(input.accepts_extension(Path::new("bar.json")));
        assert!(input.accepts_extension(Path::new("baz.xyz")));
    }

    #[test]
    fn test_watch_input_extension_filter() {
        let input = WatchInput::new().with_extensions(vec!["ets".to_string(), "ts".to_string()]);
        assert!(input.accepts_extension(Path::new("foo.ets")));
        assert!(input.accepts_extension(Path::new("foo.ts")));
        assert!(!input.accepts_extension(Path::new("foo.json")));
        assert!(!input.accepts_extension(Path::new("foo.png")));
    }

    #[test]
    fn test_watch_input_no_extension_rejected_when_filter_set() {
        let input = WatchInput::new().with_extensions(vec!["ets".to_string()]);
        // A path with no extension should be rejected when a filter is set.
        assert!(!input.accepts_extension(Path::new("Makefile")));
    }

    #[test]
    fn test_watch_event_kind_values() {
        let e = WatchEvent::new(PathBuf::from("/a/b.ets"), WatchEventKind::Created);
        assert_eq!(e.kind, WatchEventKind::Created);
        let e2 = WatchEvent::new(PathBuf::from("/a/b.ets"), WatchEventKind::Modified);
        assert_eq!(e2.kind, WatchEventKind::Modified);
        let e3 = WatchEvent::new(PathBuf::from("/a/b.ets"), WatchEventKind::Deleted);
        assert_eq!(e3.kind, WatchEventKind::Deleted);
    }

    #[test]
    fn test_session_watcher_new_does_not_panic() {
        let input = WatchInput::new().with_dirs(vec![PathBuf::from("/tmp")]);
        let _watcher = SessionWatcher::new(input);
    }

    #[test]
    fn test_session_watcher_poll_empty_initially() {
        let input = WatchInput::new();
        let watcher = SessionWatcher::new(input);
        assert!(watcher.poll().is_empty());
    }

    #[test]
    fn test_session_watcher_events_empty_initially() {
        let input = WatchInput::new();
        let watcher = SessionWatcher::new(input);
        assert!(watcher.events().is_empty());
    }

    #[test]
    fn test_session_watcher_stop_does_not_panic() {
        let input = WatchInput::new();
        let watcher = SessionWatcher::new(input);
        watcher.stop(); // Should not panic even when not watching.
    }

    #[test]
    fn test_watch_input_with_fingerprints() {
        let mut baseline = HashMap::new();
        baseline.insert(
            PathBuf::from("/project/entry/src/main/ets/Index.ets"),
            12345u64,
        );
        let input = WatchInput::new().with_fingerprints(baseline.clone());
        assert_eq!(
            input
                .fingerprint_baseline
                .get(Path::new("/project/entry/src/main/ets/Index.ets")),
            Some(&12345u64)
        );
    }

    #[test]
    fn test_watch_input_builder_chaining() {
        let input = WatchInput::new()
            .with_dirs(vec![PathBuf::from("/project/entry/src/main/ets")])
            .with_extensions(vec!["ets".to_string(), "ts".to_string()])
            .with_fingerprints(HashMap::new());
        assert_eq!(input.watch_dirs.len(), 1);
        assert_eq!(input.watch_extensions.len(), 2);
    }
}
