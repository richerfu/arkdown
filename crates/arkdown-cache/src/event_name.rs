//! Cache event name types.
//!
//! hvigor tracks "event names" (change-event identifiers) to manage
//! watch-mode cache invalidation.  When a file changes, the corresponding
//! event name is updated, and any bundle cache entry that stored the old event
//! name is invalidated.
//!
//! # Upstream correspondence
//!
//! | Type | hvigor equivalent |
//! |------|-------------------|
//! | `CacheEventName` | `arkts-pack.js::updateEventNames` output |
//! | `UpdateEventNameHandler` | `arkts-pack.js::initUpdateEventNameHandler` |
//! | `ChangedFileEvent` | watch change plugin changed-file record |

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::key::{fnv1a_hash, fnv1a_hash_bytes};

// ---------------------------------------------------------------------------
// CacheEventName
// ---------------------------------------------------------------------------

/// An event name used to identify a watch-mode change event.
///
/// Event names are opaque strings produced by hvigor's watch change plugin.
/// They are stored alongside the bundle cache so subsequent watch runs can
/// detect which cached bundle entries are still valid.
///
/// # Upstream correspondence
///
/// Corresponds to the event name computed by
/// `arkts-pack.js::updateEventNames` and `initUpdateEventNameHandler`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheEventName {
    /// The computed event name string.
    pub value: String,
}

impl CacheEventName {
    /// Compute an event name from the current compile environment string.
    ///
    /// In hvigor, the event name is derived from the compileEnv and a
    /// per-changed-file hash.  We use FNV-1a for a deterministic, compact
    /// identifier.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `arkts-pack.js::updateEventNames` and
    /// `initUpdateEventNameHandler`.
    pub fn from_compile_env(compile_env: &str) -> Self {
        let hash = fnv1a_hash(compile_env);
        Self {
            value: format!("env_{hash:016x}"),
        }
    }
}

// ---------------------------------------------------------------------------
// ChangedFileEvent
// ---------------------------------------------------------------------------

/// A single file-change event recorded by the watch change plugin.
///
/// # Upstream correspondence
///
/// Corresponds to `rollup.WatchEventType` combined with the file path, as
/// produced by the watch change plugin.
#[derive(Debug, Clone)]
pub struct ChangedFileEvent {
    /// Absolute path to the changed file.
    pub path: PathBuf,
    /// The kind of change: `"update"`, `"create"`, or `"delete"`.
    pub kind: String,
}

impl ChangedFileEvent {
    /// Create an "update" event for `path`.
    pub fn update(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            kind: "update".to_string(),
        }
    }

    /// Create a "create" event for `path`.
    pub fn create(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            kind: "create".to_string(),
        }
    }

    /// Create a "delete" event for `path`.
    pub fn delete(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            kind: "delete".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// UpdateEventNameHandler
// ---------------------------------------------------------------------------

/// Maintains the current event name state for watch-mode builds.
///
/// `UpdateEventNameHandler` is initialized once per watch session and updated
/// each time a file changes.  It produces the event name to store alongside
/// each bundle cache entry.
///
/// # Upstream correspondence
///
/// Corresponds to `arkts-pack.js::initUpdateEventNameHandler` and the
/// `updateEventNames` call inside each watch iteration.
pub struct UpdateEventNameHandler {
    /// Map from absolute file path → per-file event name.
    names: BTreeMap<PathBuf, CacheEventName>,
}

impl UpdateEventNameHandler {
    /// Create a new handler with no tracked files.
    pub fn new() -> Self {
        Self {
            names: BTreeMap::new(),
        }
    }

    /// Record a changed-file event and update the event name for that file.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `arkts-pack.js::updateEventNames`.
    pub fn update(&mut self, event: &ChangedFileEvent) {
        let path_key = CacheEventPathKey::new(&event.path);
        let hash = path_key.hash_with_kind(&event.kind);
        self.names.insert(
            event.path.clone(),
            CacheEventName {
                value: format!("file_{hash:016x}"),
            },
        );
    }

    /// Return the event name for a specific file path, if tracked.
    pub fn event_for_changed_file(&self, path: &Path) -> Option<&CacheEventName> {
        self.names.get(path)
    }

    /// Serialize all current event names to a string for storage alongside
    /// a bundle cache entry.
    pub fn serialize_event_name(&self) -> Vec<String> {
        self.names.values().map(|n| n.value.clone()).collect()
    }
}

impl Default for UpdateEventNameHandler {
    fn default() -> Self {
        Self::new()
    }
}

struct CacheEventPathKey<'a> {
    path: &'a Path,
}

impl<'a> CacheEventPathKey<'a> {
    fn new(path: &'a Path) -> Self {
        Self { path }
    }

    fn hash_with_kind(&self, kind: &str) -> u64 {
        let path_bytes = self.path.as_os_str().as_encoded_bytes();
        let mut bytes = Vec::with_capacity(path_bytes.len() + kind.len() + 1);
        bytes.extend_from_slice(path_bytes);
        bytes.push(b':');
        bytes.extend_from_slice(kind.as_bytes());
        fnv1a_hash_bytes(&bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_name_from_compile_env_is_deterministic() {
        let e1 = CacheEventName::from_compile_env("sdk12#OpenHarmony");
        let e2 = CacheEventName::from_compile_env("sdk12#OpenHarmony");
        assert_eq!(e1.value, e2.value);
    }

    #[test]
    fn different_compile_env_gives_different_event_names() {
        let e1 = CacheEventName::from_compile_env("sdk12");
        let e2 = CacheEventName::from_compile_env("sdk13");
        assert_ne!(e1.value, e2.value);
    }

    #[test]
    fn update_handler_records_changed_file() {
        let mut handler = UpdateEventNameHandler::new();
        handler.update(&ChangedFileEvent::update("/src/Index.ets"));
        let ev = handler.event_for_changed_file(std::path::Path::new("/src/Index.ets"));
        assert!(ev.is_some());
    }

    #[test]
    fn serialize_event_names_returns_all() {
        let mut handler = UpdateEventNameHandler::new();
        handler.update(&ChangedFileEvent::update("/a.ets"));
        handler.update(&ChangedFileEvent::create("/b.ets"));
        let names = handler.serialize_event_name();
        assert_eq!(names.len(), 2);
    }
}
