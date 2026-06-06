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

        let mut obj = format!(
            r#"{{"type":"{}","module":"{}","build_mode":"{}","ts":{}"#,
            self.type_str(),
            escape_json(module),
            escape_json(build_mode),
            ts
        );

        match self {
            HotReloadEvent::BuildStart => {}
            HotReloadEvent::BuildComplete {
                artifact,
                success,
                duration_ms,
            } => {
                obj.push_str(&format!(
                    r#","artifact":"{}","success":{},"duration_ms":{}"#,
                    escape_json(artifact),
                    success,
                    duration_ms
                ));
            }
            HotReloadEvent::BuildError { message } => {
                obj.push_str(&format!(r#","message":"{}""#, escape_json(message)));
            }
            HotReloadEvent::ResourceChange { files }
            | HotReloadEvent::SourceChange { files }
            | HotReloadEvent::ProfileChange { files } => {
                let files_json: Vec<String> =
                    files.iter().map(|f| format!("\"{}\"", escape_json(f))).collect();
                obj.push_str(&format!(r#","files":[{}]"#, files_json.join(",")));
            }
            HotReloadEvent::PreviewRefresh { entry } => {
                obj.push_str(&format!(r#","entry":"{}""#, escape_json(entry)));
            }
        }

        obj.push('}');
        obj
    }
}

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

/// Simple JSON string escape (handle common control characters).
fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufRead;

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
            let v: serde_json::Value = serde_json::from_str(line)
                .expect("each event line must be valid JSON");
            assert!(v.get("type").is_some());
            assert!(v.get("ts").is_some());
        }
    }

    #[test]
    fn test_json_escaping_handles_special_chars() {
        let json = escape_json("path\\to\\file \"quoted\"");
        assert_eq!(json, r#"path\\to\\file \"quoted\""#);
    }
}
