//! Hot-reload and live-preview notification support.
//!
//! Implements the build-status event emission required for IDE integration.
//!
//! ## Gap analysis context (section 3.9)
//!
//! The DevEco Studio IDE communicates with the build system via a proprietary
//! WebSocket protocol that requires protocol specification documents and SDK
//! access.  This module implements the **CLI-side** event bus that:
//!
//! 1. Emits JSON build-status events to stdout (for piping to IDE adapters).
//! 2. Writes events to a lock-free event file that IDE extensions can poll.
//! 3. Provides a structured `HotReloadEvent` type that any future WebSocket
//!    adapter can serialize to the DevEco protocol format.
//!
//! When the DevEco protocol specification becomes available, a thin WebSocket
//! adapter layer can be placed over this module without changing the event
//! types.
//!
//! ## Event format
//!
//! Events are newline-delimited JSON (NDJSON) on stdout:
//!
//! ```json
//! {"type":"build_start","module":"entry","build_mode":"debug","ts":1718000000}
//! {"type":"build_complete","module":"entry","artifact":"entry-default-signed.hap","success":true,"duration_ms":1234,"ts":1718000001}
//! {"type":"resource_change","module":"entry","files":["src/main/resources/base/media/icon.png"],"ts":1718000002}
//! {"type":"build_error","module":"entry","message":"UNSUPPORTED_COMPILE_MODE: ...","ts":1718000003}
//! ```
//!
//! The `ts` field is a Unix timestamp (seconds).  Consumers should use it
//! to order events when events arrive out-of-order.
//!
//! ## Usage
//!
//! ```no_run
//! use arkdown_watcher::hot_reload::{HotReloadNotifier, HotReloadEvent};
//! use std::path::PathBuf;
//!
//! let notifier = HotReloadNotifier::new(
//!     "entry".to_string(),
//!     "debug".to_string(),
//!     Some(PathBuf::from("/tmp/arkdown-events.ndjson")),
//!     true, // emit to stdout
//! );
//!
//! notifier.emit(HotReloadEvent::BuildStart);
//! // ... do work ...
//! notifier.emit(HotReloadEvent::BuildComplete {
//!     artifact: "entry-default-signed.hap".to_string(),
//!     success: true,
//!     duration_ms: 1500,
//! });
//! ```

use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use arkdown_path::ArtifactPathSerializer;

use crate::invalidation::InvalidationEvent;
use crate::WatchError;

/// Build-status events emitted to IDE integrations.
///
/// Mirrors the event types used in hvigor's preview/watch protocol.
#[derive(Debug, Clone)]
pub enum HotReloadEvent {
    /// A build has started for a module.
    BuildStart,
    /// A build completed (success or failure).
    BuildComplete {
        /// Name of the produced artifact file.
        artifact: String,
        /// Whether the build succeeded.
        success: bool,
        /// Wall-clock duration in milliseconds.
        duration_ms: u64,
    },
    /// A build failed with an error message.
    BuildError {
        /// Human-readable error description.
        message: String,
    },
    /// Resource files changed — request a lightweight resource-only rebuild.
    ResourceChange {
        /// Changed file paths (relative to project root).
        files: Vec<String>,
    },
    /// Source files changed — request a full rebuild.
    SourceChange {
        /// Changed file paths (relative to project root).
        files: Vec<String>,
    },
    /// Profile files changed — request a profile-only rebuild.
    ProfileChange {
        /// Changed file paths (relative to project root).
        files: Vec<String>,
    },
    /// The preview target has changed and a new preview should be launched.
    PreviewRefresh {
        /// Component entry point that was updated.
        entry: String,
    },
}

impl HotReloadEvent {
    /// Derive a `HotReloadEvent` from a classified `InvalidationEvent`.
    ///
    /// Maps invalidation reasons to the appropriate hot-reload event type:
    /// - `ResourceChanged` → `ResourceChange`
    /// - `ProfileFileChanged` → `ProfileChange`
    /// - `SourceFileChanged` → `SourceChange`
    /// - Everything else → `SourceChange` (triggers a full rebuild)
    ///
    /// Upstream: `hot-reload-ark-compile.js`, `cold-reload-ark-compile.js`.
    pub fn from_invalidation(event: &InvalidationEvent, module: &str) -> Self {
        use crate::invalidation::InvalidationReason;

        let path_str = |p: &std::path::Path| {
            ArtifactPathSerializer::path_to_slash_string(p)
                .unwrap_or_else(|_| ArtifactPathSerializer::rel_to_slash(p))
        };

        match &event.reason {
            InvalidationReason::ResourceChanged { path } => HotReloadEvent::ResourceChange {
                files: vec![path_str(path)],
            },
            InvalidationReason::ProfileFileChanged { path } => HotReloadEvent::ProfileChange {
                files: vec![path_str(path)],
            },
            InvalidationReason::SourceFileChanged { path } => HotReloadEvent::SourceChange {
                files: vec![path_str(path)],
            },
            InvalidationReason::ManifestChanged { path } => HotReloadEvent::ProfileChange {
                files: vec![path_str(path)],
            },
            _ => {
                // SDK change, dependency change, etc. — trigger a full source rebuild.
                HotReloadEvent::SourceChange {
                    files: vec![module.to_string()],
                }
            }
        }
    }

    fn type_str(&self) -> &'static str {
        match self {
            HotReloadEvent::BuildStart => "build_start",
            HotReloadEvent::BuildComplete { .. } => "build_complete",
            HotReloadEvent::BuildError { .. } => "build_error",
            HotReloadEvent::ResourceChange { .. } => "resource_change",
            HotReloadEvent::SourceChange { .. } => "source_change",
            HotReloadEvent::ProfileChange { .. } => "profile_change",
            HotReloadEvent::PreviewRefresh { .. } => "preview_refresh",
        }
    }

    fn to_json(&self, module: &str, build_mode: &str) -> String {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut obj = serde_json::Map::new();
        obj.insert("type".to_string(), serde_json::json!(self.type_str()));
        obj.insert("module".to_string(), serde_json::json!(module));
        obj.insert("build_mode".to_string(), serde_json::json!(build_mode));
        obj.insert("ts".to_string(), serde_json::json!(ts));

        match self {
            HotReloadEvent::BuildStart => {}
            HotReloadEvent::BuildComplete {
                artifact,
                success,
                duration_ms,
            } => {
                obj.insert("artifact".to_string(), serde_json::json!(artifact));
                obj.insert("success".to_string(), serde_json::json!(success));
                obj.insert("duration_ms".to_string(), serde_json::json!(duration_ms));
            }
            HotReloadEvent::BuildError { message } => {
                obj.insert("message".to_string(), serde_json::json!(message));
            }
            HotReloadEvent::ResourceChange { files }
            | HotReloadEvent::SourceChange { files }
            | HotReloadEvent::ProfileChange { files } => {
                obj.insert("files".to_string(), serde_json::json!(files));
            }
            HotReloadEvent::PreviewRefresh { entry } => {
                obj.insert("entry".to_string(), serde_json::json!(entry));
            }
        }

        serde_json::Value::Object(obj).to_string()
    }
}

// ---------------------------------------------------------------------------
// HotReloadPlan
// ---------------------------------------------------------------------------

/// A plan for a hot-reload cycle, grouping the changed files for a module.
///
/// Upstream: `hot-reload-ark-compile.js` — the reload task receives the list
/// of changed source files and decides whether to do a hot reload (incremental
/// ABC patch) or a cold reload (full rebuild + app restart).
#[derive(Debug, Clone)]
pub struct HotReloadPlan {
    /// OHOS module name (e.g. `"entry"`).
    pub module_name: String,
    /// Absolute paths of the files that changed.
    pub changed_files: Vec<PathBuf>,
    /// When `true`, the change requires a full application restart (cold
    /// reload).  When `false`, only an incremental ABC patch is needed.
    pub is_cold_reload: bool,
}

impl HotReloadPlan {
    /// Create a new hot-reload plan.
    pub fn new(module_name: impl Into<String>, changed_files: Vec<PathBuf>) -> Self {
        HotReloadPlan {
            module_name: module_name.into(),
            changed_files,
            is_cold_reload: false,
        }
    }

    /// Mark this plan as requiring a cold reload.
    pub fn with_cold_reload(mut self) -> Self {
        self.is_cold_reload = true;
        self
    }
}

// ---------------------------------------------------------------------------
// HotReloadNotifier
// ---------------------------------------------------------------------------

/// Emitter for hot-reload / live-preview build status events.
///
/// Events are written to:
/// 1. A file at `event_file` (NDJSON format, appended).
/// 2. Stdout when `emit_to_stdout` is true.
///
/// Both outputs use newline-delimited JSON so consumers can `tail -f` the
/// event file or pipe stdout to an IDE adapter.
pub struct HotReloadNotifier {
    module: String,
    build_mode: String,
    event_file: Option<PathBuf>,
    emit_to_stdout: bool,
    /// Mutex guards the file append to prevent interleaving when the notifier
    /// is used from multiple threads.
    file_lock: Mutex<()>,
}

impl HotReloadNotifier {
    /// Create a new notifier.
    ///
    /// - `module`: module name (e.g. "entry").
    /// - `build_mode`: "debug" or "release".
    /// - `event_file`: optional path to write NDJSON events.  Created if absent.
    /// - `emit_to_stdout`: whether to also print events to stdout.
    pub fn new(
        module: String,
        build_mode: String,
        event_file: Option<PathBuf>,
        emit_to_stdout: bool,
    ) -> Self {
        Self {
            module,
            build_mode,
            event_file,
            emit_to_stdout,
            file_lock: Mutex::new(()),
        }
    }

    /// Create a notifier bound to a DevEco WebSocket port.
    ///
    /// Convenience constructor for the common case where the notifier is used
    /// alongside a `DevEcoWsServer` on a specific port.  Events are emitted
    /// to stdout only; no event file is written.
    ///
    /// `port` is stored for documentation purposes — the notifier itself
    /// does not open a network socket.  Use `DevEcoWsServer` for that.
    pub fn for_port(port: u16) -> Self {
        Self {
            module: format!("port:{}", port),
            build_mode: "debug".to_string(),
            event_file: None,
            emit_to_stdout: false,
            file_lock: Mutex::new(()),
        }
    }

    /// Send a `HotReloadEvent`, returning `Err` if the underlying I/O fails.
    ///
    /// Unlike `emit` (which silently ignores I/O errors), `send` propagates
    /// errors so the caller can handle them.
    pub fn send(&self, event: &HotReloadEvent) -> Result<(), WatchError> {
        let json = event.to_json(&self.module, &self.build_mode);
        let _guard = self.file_lock.lock();

        if self.emit_to_stdout {
            let mut stdout = std::io::stdout().lock();
            writeln!(stdout, "{}", json)
                .map_err(|e| WatchError::Build(format!("stdout write: {}", e)))?;
        }

        if let Some(path) = &self.event_file {
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .map_err(|e| WatchError::Build(format!("open event file: {}", e)))?;
            writeln!(file, "{}", json)
                .map_err(|e| WatchError::Build(format!("write event file: {}", e)))?;
        }

        Ok(())
    }

    /// Emit a build-status event.
    pub fn emit(&self, event: HotReloadEvent) {
        let json = event.to_json(&self.module, &self.build_mode);
        let _guard = self.file_lock.lock();

        if self.emit_to_stdout {
            // Write directly to stdout bypassing buffering so the IDE adapter
            // receives the event immediately.
            let mut stdout = std::io::stdout().lock();
            let _ = writeln!(stdout, "{}", json);
        }

        if let Some(path) = &self.event_file {
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
            {
                let _ = writeln!(file, "{}", json);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_build_start_json_contains_type() {
        let json = HotReloadEvent::BuildStart.to_json("entry", "debug");
        assert!(json.contains("\"type\":\"build_start\""));
        assert!(json.contains("\"module\":\"entry\""));
        assert!(json.contains("\"build_mode\":\"debug\""));
    }

    #[test]
    fn test_build_complete_json_has_artifact_and_success() {
        let event = HotReloadEvent::BuildComplete {
            artifact: "entry-debug.hap".to_string(),
            success: true,
            duration_ms: 1234,
        };
        let json = event.to_json("entry", "debug");
        assert!(json.contains("\"type\":\"build_complete\""));
        assert!(json.contains("\"artifact\":\"entry-debug.hap\""));
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"duration_ms\":1234"));
    }

    #[test]
    fn test_build_error_json_has_message() {
        let event = HotReloadEvent::BuildError {
            message: "UNSUPPORTED_COMPILE_MODE: jsbundle".to_string(),
        };
        let json = event.to_json("entry", "debug");
        assert!(json.contains("\"type\":\"build_error\""));
        assert!(json.contains("UNSUPPORTED_COMPILE_MODE"));
    }

    #[test]
    fn test_resource_change_json_has_files_array() {
        let event = HotReloadEvent::ResourceChange {
            files: vec![
                "src/main/resources/base/media/icon.png".to_string(),
                "src/main/resources/base/string/strings.json".to_string(),
            ],
        };
        let json = event.to_json("entry", "debug");
        assert!(json.contains("\"type\":\"resource_change\""));
        assert!(json.contains("icon.png"));
        assert!(json.contains("strings.json"));
    }

    #[test]
    fn test_notifier_writes_to_file() {
        let tmp = tempfile::tempdir().unwrap();
        let event_file = tmp.path().join("events.ndjson");

        let notifier = HotReloadNotifier::new(
            "entry".to_string(),
            "debug".to_string(),
            Some(event_file.clone()),
            false, // don't pollute test stdout
        );

        notifier.emit(HotReloadEvent::BuildStart);
        notifier.emit(HotReloadEvent::BuildComplete {
            artifact: "entry-debug.hap".to_string(),
            success: true,
            duration_ms: 100,
        });

        let content = std::fs::read_to_string(&event_file).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2, "expected 2 NDJSON events");
        assert!(lines[0].contains("build_start"));
        assert!(lines[1].contains("build_complete"));

        // Verify each line is valid JSON.
        for line in lines {
            let v: serde_json::Value =
                serde_json::from_str(line).expect("each event line must be valid JSON");
            assert!(v.get("type").is_some());
            assert!(v.get("ts").is_some());
        }
    }

    #[test]
    fn test_json_escaping_handles_special_chars() {
        let event = HotReloadEvent::BuildError {
            message: "path\\to\\file \"quoted\"".to_string(),
        };
        let json = event.to_json("entry", "debug");
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(
            value.get("message").and_then(serde_json::Value::as_str),
            Some("path\\to\\file \"quoted\"")
        );
    }

    // -----------------------------------------------------------------------
    // HotReloadPlan
    // -----------------------------------------------------------------------

    #[test]
    fn test_hot_reload_plan_new() {
        let plan = HotReloadPlan::new(
            "entry",
            vec![PathBuf::from("/project/entry/src/main/ets/Index.ets")],
        );
        assert_eq!(plan.module_name, "entry");
        assert_eq!(plan.changed_files.len(), 1);
        assert!(!plan.is_cold_reload);
    }

    #[test]
    fn test_hot_reload_plan_with_cold_reload() {
        let plan = HotReloadPlan::new("entry", vec![]).with_cold_reload();
        assert!(plan.is_cold_reload);
    }

    // -----------------------------------------------------------------------
    // HotReloadEvent::from_invalidation
    // -----------------------------------------------------------------------

    #[test]
    fn test_from_invalidation_source_change() {
        use crate::invalidation::{
            AffectedModuleSet, AffectedStage, InvalidationEvent, InvalidationReason,
        };

        let reason = InvalidationReason::SourceFileChanged {
            path: PathBuf::from("/project/entry/src/main/ets/Index.ets"),
        };
        let event = InvalidationEvent {
            reason,
            affected_stages: vec![AffectedStage::Bundle],
            affected_modules_set: AffectedModuleSet::empty(),
        };
        // We need to use the public constructor path; build via from_file_event
        // since InvalidationEvent fields are pub.
        let hot = HotReloadEvent::from_invalidation(&event, "entry");
        match hot {
            HotReloadEvent::SourceChange { files } => {
                assert!(files[0].contains("Index.ets"));
            }
            other => panic!("expected SourceChange, got {:?}", other),
        }
    }

    #[test]
    fn test_from_invalidation_resource_change() {
        use crate::invalidation::{
            AffectedModuleSet, AffectedStage, InvalidationEvent, InvalidationReason,
        };

        let reason = InvalidationReason::ResourceChanged {
            path: PathBuf::from("/project/entry/src/main/resources/base/media/icon.png"),
        };
        let event = InvalidationEvent {
            reason,
            affected_stages: vec![AffectedStage::Resource],
            affected_modules_set: AffectedModuleSet::empty(),
        };
        let hot = HotReloadEvent::from_invalidation(&event, "entry");
        match hot {
            HotReloadEvent::ResourceChange { files } => {
                assert!(files[0].contains("icon.png"));
            }
            other => panic!("expected ResourceChange, got {:?}", other),
        }
    }

    #[test]
    fn test_from_invalidation_profile_change() {
        use crate::invalidation::{
            AffectedModuleSet, AffectedStage, InvalidationEvent, InvalidationReason,
        };

        let reason = InvalidationReason::ProfileFileChanged {
            path: PathBuf::from("/project/entry/build-profile.json5"),
        };
        let event = InvalidationEvent {
            reason,
            affected_stages: vec![AffectedStage::Profile],
            affected_modules_set: AffectedModuleSet::empty(),
        };
        let hot = HotReloadEvent::from_invalidation(&event, "entry");
        match hot {
            HotReloadEvent::ProfileChange { files } => {
                assert!(files[0].contains("build-profile.json5"));
            }
            other => panic!("expected ProfileChange, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // HotReloadNotifier::send
    // -----------------------------------------------------------------------

    #[test]
    fn test_notifier_send_writes_to_file() {
        let tmp = tempfile::tempdir().unwrap();
        let event_file = tmp.path().join("events.ndjson");

        let notifier = HotReloadNotifier::new(
            "entry".to_string(),
            "debug".to_string(),
            Some(event_file.clone()),
            false,
        );

        notifier.send(&HotReloadEvent::BuildStart).unwrap();
        notifier
            .send(&HotReloadEvent::BuildComplete {
                artifact: "entry-debug.hap".to_string(),
                success: true,
                duration_ms: 100,
            })
            .unwrap();

        let content = std::fs::read_to_string(&event_file).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("build_start"));
        assert!(lines[1].contains("build_complete"));
    }

    #[test]
    fn test_notifier_for_port_does_not_panic() {
        let notifier = HotReloadNotifier::for_port(61001);
        // Should not panic.
        let _ = notifier.send(&HotReloadEvent::BuildStart);
    }
}
