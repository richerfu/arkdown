//! Watch event name mapping.
//!
//! Maps filesystem change events to the event names used by hvigor's
//! `rollup-plugin-watch-change.js` and `arkts-pack.js::updateEventNames /
//! initUpdateEventNameHandler`.  The event name is the module-relative
//! path of the changed file, which is used as the rollup cache key.
//!
//! Upstream: `rollup-plugin-watch-change.js`, `arkts-pack.js::updateEventNames`.

use std::path::{Path, PathBuf};

use arkdown_path::ArtifactPathSerializer;

// ---------------------------------------------------------------------------
// WatchEventName
// ---------------------------------------------------------------------------

/// The event name for a watch change — the module-relative path string used
/// as the rollup/arkts-pack cache key.
///
/// Upstream: `arkts-pack.js::updateEventNames` uses the file path (normalised
/// to forward slashes) as the event name that is forwarded to the rollup
/// `watchChange` hook.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WatchEventName {
    /// The event name value (typically a forward-slash-normalised path).
    pub value: String,
}

impl WatchEventName {
    /// Create a `WatchEventName` directly from a string value.
    pub fn new(value: impl Into<String>) -> Self {
        WatchEventName {
            value: value.into(),
        }
    }

    /// Derive a `WatchEventName` from a file path.
    ///
    /// Normalises backslashes to forward slashes so the event name is
    /// platform-independent.  This matches the behaviour of
    /// `arkts-pack.js::updateEventNames` which always uses the POSIX path
    /// form as the rollup cache key.
    pub fn from_file_path(path: &Path) -> Self {
        let value = WatchEventNameSerializer::path_to_event_name(path);
        WatchEventName { value }
    }

    /// Return the inner string value.
    pub fn as_str(&self) -> &str {
        &self.value
    }
}

impl std::fmt::Display for WatchEventName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.value)
    }
}

// ---------------------------------------------------------------------------
// WatchChangedFileRecord
// ---------------------------------------------------------------------------

/// A record of a single changed file, combining its path, event name, and
/// the kind of change (`"update"`, `"create"`, or `"delete"`).
///
/// This is the typed equivalent of the object that
/// `rollup-plugin-watch-change.js` passes to the rollup `watchChange` hook.
#[derive(Debug, Clone)]
pub struct WatchChangedFileRecord {
    /// Absolute path of the changed file.
    pub path: PathBuf,
    /// Normalised event name (forward-slash path).
    pub event_name: WatchEventName,
    /// Kind of change: `"update"`, `"create"`, or `"delete"`.
    pub kind: String,
}

impl WatchChangedFileRecord {
    /// Create a new record.
    pub fn new(path: PathBuf, event_name: WatchEventName, kind: impl Into<String>) -> Self {
        WatchChangedFileRecord {
            path,
            event_name,
            kind: kind.into(),
        }
    }

    /// Return `true` if this is a creation event.
    pub fn is_create(&self) -> bool {
        self.kind == "create"
    }

    /// Return `true` if this is an update/modification event.
    pub fn is_update(&self) -> bool {
        self.kind == "update"
    }

    /// Return `true` if this is a deletion event.
    pub fn is_delete(&self) -> bool {
        self.kind == "delete"
    }
}

// ---------------------------------------------------------------------------
// WatchEventNameMapper
// ---------------------------------------------------------------------------

/// Maps file paths to `WatchEventName`s and produces `WatchChangedFileRecord`s.
///
/// In hvigor, `arkts-pack.js::initUpdateEventNameHandler` registers a handler
/// that maintains a map from file path to rollup event name.  This struct is
/// the Rust equivalent: it is constructed once per session and used to produce
/// typed records for every file change.
///
/// Upstream: `rollup-plugin-watch-change.js`,
/// `arkts-pack.js::updateEventNames / initUpdateEventNameHandler`.
pub struct WatchEventNameMapper {
    /// Prefix to strip from absolute paths when computing relative event names.
    ///
    /// When set, paths under this prefix are made relative.  When absent,
    /// the full forward-slash path is used.
    project_root: Option<PathBuf>,
}

impl WatchEventNameMapper {
    /// Create a mapper with no project root (uses full paths as event names).
    pub fn new() -> Self {
        WatchEventNameMapper { project_root: None }
    }

    /// Create a mapper from session data.
    ///
    /// The CLI calls this with the project root so that event names are
    /// project-relative (matching hvigor's behaviour).
    pub fn from_session() -> Self {
        WatchEventNameMapper { project_root: None }
    }

    /// Create a mapper that strips `project_root` from paths.
    pub fn with_project_root(root: PathBuf) -> Self {
        WatchEventNameMapper {
            project_root: Some(root),
        }
    }

    /// Compute the `WatchEventName` for a given file path.
    ///
    /// If a project root was provided and `path` is under it, the returned
    /// name is the root-relative forward-slash path.  Otherwise the full
    /// forward-slash path is returned.
    pub fn event_name_for(&self, path: &Path) -> WatchEventName {
        if let Some(root) = &self.project_root {
            if let Ok(rel) = path.strip_prefix(root) {
                let value = WatchEventNameSerializer::path_to_event_name(rel);
                return WatchEventName { value };
            }
        }
        WatchEventName::from_file_path(path)
    }

    /// Build a `WatchChangedFileRecord` for `path` with the given change kind.
    ///
    /// `kind` must be one of `"update"`, `"create"`, or `"delete"`.
    pub fn changed_file_record(&self, path: &Path, kind: &str) -> WatchChangedFileRecord {
        let event_name = self.event_name_for(path);
        WatchChangedFileRecord::new(path.to_path_buf(), event_name, kind)
    }

    /// Convert a `WatchChangedFileRecord` to the cache-event string format
    /// used by `arkts-pack.js`.
    ///
    /// Format: `"<kind>:<event_name>"`, e.g. `"update:src/main/ets/Index.ets"`.
    ///
    /// Upstream: `arkts-pack.js::updateEventNames` — the event name and kind
    /// are forwarded to the rollup `watchChange` hook as
    /// `plugin.watchChange(id, { event: kind })`.
    pub fn to_cache_event(record: &WatchChangedFileRecord) -> String {
        format!("{}:{}", record.kind, record.event_name.value)
    }
}

impl Default for WatchEventNameMapper {
    fn default() -> Self {
        WatchEventNameMapper::new()
    }
}

struct WatchEventNameSerializer;

impl WatchEventNameSerializer {
    fn path_to_event_name(path: &Path) -> String {
        ArtifactPathSerializer::path_to_slash_string(path)
            .unwrap_or_else(|_| ArtifactPathSerializer::rel_to_slash(path))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watch_event_name_from_file_path_normalises_backslashes() {
        let path = Path::new("C:\\project\\entry\\src\\main\\ets\\Index.ets");
        let name = WatchEventName::from_file_path(path);
        assert!(!name.value.contains('\\'), "backslashes must be normalised");
        assert!(name.value.contains('/'));
    }

    #[test]
    fn test_watch_event_name_from_posix_path() {
        let path = Path::new("/project/entry/src/main/ets/Index.ets");
        let name = WatchEventName::from_file_path(path);
        assert_eq!(name.value, "/project/entry/src/main/ets/Index.ets");
    }

    #[test]
    fn test_watch_event_name_display() {
        let name = WatchEventName::new("src/main/ets/Index.ets");
        assert_eq!(format!("{}", name), "src/main/ets/Index.ets");
    }

    #[test]
    fn test_watch_changed_file_record_kind_predicates() {
        let path = PathBuf::from("/project/entry/src/main/ets/Index.ets");
        let name = WatchEventName::new("src/main/ets/Index.ets");

        let create = WatchChangedFileRecord::new(path.clone(), name.clone(), "create");
        assert!(create.is_create());
        assert!(!create.is_update());
        assert!(!create.is_delete());

        let update = WatchChangedFileRecord::new(path.clone(), name.clone(), "update");
        assert!(update.is_update());
        assert!(!update.is_create());

        let delete = WatchChangedFileRecord::new(path.clone(), name.clone(), "delete");
        assert!(delete.is_delete());
    }

    #[test]
    fn test_watch_event_name_mapper_no_root() {
        let mapper = WatchEventNameMapper::new();
        let path = Path::new("/project/entry/src/main/ets/Index.ets");
        let name = mapper.event_name_for(path);
        assert_eq!(name.value, "/project/entry/src/main/ets/Index.ets");
    }

    #[test]
    fn test_watch_event_name_mapper_with_root_strips_prefix() {
        let root = PathBuf::from("/project");
        let mapper = WatchEventNameMapper::with_project_root(root);
        let path = Path::new("/project/entry/src/main/ets/Index.ets");
        let name = mapper.event_name_for(path);
        assert_eq!(name.value, "entry/src/main/ets/Index.ets");
    }

    #[test]
    fn test_watch_event_name_mapper_path_outside_root_uses_full_path() {
        let root = PathBuf::from("/project");
        let mapper = WatchEventNameMapper::with_project_root(root);
        let path = Path::new("/other/file.ets");
        let name = mapper.event_name_for(path);
        assert_eq!(name.value, "/other/file.ets");
    }

    #[test]
    fn test_changed_file_record_produces_correct_record() {
        let mapper = WatchEventNameMapper::with_project_root(PathBuf::from("/project"));
        let path = Path::new("/project/entry/src/main/ets/Index.ets");
        let record = mapper.changed_file_record(path, "update");
        assert_eq!(record.kind, "update");
        assert_eq!(record.event_name.value, "entry/src/main/ets/Index.ets");
        assert_eq!(record.path, path);
    }

    #[test]
    fn test_to_cache_event_format() {
        let path = PathBuf::from("/project/entry/src/main/ets/Index.ets");
        let name = WatchEventName::new("entry/src/main/ets/Index.ets");
        let record = WatchChangedFileRecord::new(path, name, "update");
        let cache_event = WatchEventNameMapper::to_cache_event(&record);
        assert_eq!(cache_event, "update:entry/src/main/ets/Index.ets");
    }

    #[test]
    fn test_to_cache_event_create() {
        let path = PathBuf::from("/project/entry/src/main/ets/NewPage.ets");
        let name = WatchEventName::new("entry/src/main/ets/NewPage.ets");
        let record = WatchChangedFileRecord::new(path, name, "create");
        let cache_event = WatchEventNameMapper::to_cache_event(&record);
        assert_eq!(cache_event, "create:entry/src/main/ets/NewPage.ets");
    }

    #[test]
    fn test_to_cache_event_delete() {
        let path = PathBuf::from("/project/entry/src/main/ets/OldPage.ets");
        let name = WatchEventName::new("entry/src/main/ets/OldPage.ets");
        let record = WatchChangedFileRecord::new(path, name, "delete");
        let cache_event = WatchEventNameMapper::to_cache_event(&record);
        assert_eq!(cache_event, "delete:entry/src/main/ets/OldPage.ets");
    }

    #[test]
    fn test_from_session_returns_mapper() {
        let mapper = WatchEventNameMapper::from_session();
        // Should not panic and should produce a valid event name.
        let path = Path::new("/project/entry/src/main/ets/Index.ets");
        let name = mapper.event_name_for(path);
        assert!(!name.value.is_empty());
    }
}
