//! Bundle stage diagnostics.
//!
//! [`BundlerDiagnostic`] is a structured error/warning from the Rolldown
//! bundle stage.  It wraps Rolldown errors with OHOS-specific context.
//!
//! # Upstream correspondence
//!
//! | Type | hvigor/SDK equivalent |
//! |------|-----------------------|
//! | `BundlerDiagnostic` | hvigor Rollup error format (from `getRollupErrorMessage`) |
//! | `RolldownErrorKind` | Rolldown error kind classification |

use arkdown_core::Diagnostic;
use rolldown_error::{BatchedBuildDiagnostic, BuildDiagnostic, EventKind};

// ---------------------------------------------------------------------------
// RolldownErrorKind
// ---------------------------------------------------------------------------

/// Classification of errors from the Rolldown bundler.
///
/// # Upstream correspondence
///
/// Corresponds to `getRollupErrorMessage` error code classification in hvigor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RolldownErrorKind {
    /// A module could not be resolved.
    UnresolvedImport,
    /// A module's source could not be loaded.
    LoadFailed,
    /// A module failed to transform.
    TransformFailed,
    /// A parse error in a module's source.
    ParseError,
    /// A circular dependency in the module graph.
    CircularDependency,
    /// An output file conflict.
    OutputConflict,
    /// An unknown/uncategorized rolldown error.
    Other,
}

// ---------------------------------------------------------------------------
// BundlerDiagnostic
// ---------------------------------------------------------------------------

/// A structured diagnostic from the Rolldown bundle stage.
///
/// # Upstream correspondence
///
/// Corresponds to hvigor's Rollup error format produced by
/// `getRollupErrorMessage` in `arkts-pack.js`.
#[derive(Debug, Clone)]
pub struct BundlerDiagnostic {
    /// The Rolldown error kind.
    pub kind: RolldownErrorKind,
    /// The error message.
    pub message: String,
    /// The module ID where the error occurred.
    pub module_id: Option<String>,
    /// Line number within the module.
    pub line: Option<u32>,
    /// Column number within the module.
    pub column: Option<u32>,
}

impl BundlerDiagnostic {
    pub fn from_rolldown_diagnostic(diagnostic: &BuildDiagnostic) -> Self {
        // PluginError::message intentionally returns an empty string until
        // `on_diagnostic` injects the hook-owned error. Render the public
        // diagnostic instead of Display on the raw event so plugin failures
        // retain their actionable module and cause.
        let message = diagnostic.to_diagnostic().to_string();
        Self {
            kind: RolldownDiagnosticKind::new(diagnostic.kind()).kind(),
            message,
            module_id: diagnostic.id(),
            line: None,
            column: None,
        }
    }

    /// Create from a plugin error (diagnostic from a plugin hook).
    pub fn from_plugin_error(module_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind: RolldownErrorKind::TransformFailed,
            message: message.into(),
            module_id: Some(module_id.into()),
            line: None,
            column: None,
        }
    }

    /// Convert to a `Diagnostic` for display.
    pub fn to_diagnostic(&self) -> Diagnostic {
        let d = Diagnostic::error(&self.message);
        let d = if let Some(id) = &self.module_id {
            d.with_path(id)
        } else {
            d
        };
        if let Some(line) = self.line {
            d.with_line(line)
        } else {
            d
        }
    }
}

pub(crate) struct RolldownBuildDiagnostics {
    diagnostics: Vec<BundlerDiagnostic>,
}

impl RolldownBuildDiagnostics {
    pub(crate) fn new(error: BatchedBuildDiagnostic) -> Self {
        Self {
            diagnostics: error
                .into_vec()
                .into_iter()
                .map(|diagnostic| BundlerDiagnostic::from_rolldown_diagnostic(&diagnostic))
                .collect(),
        }
    }

    pub(crate) fn into_message(self) -> String {
        if self.diagnostics.is_empty() {
            return "rolldown failed without diagnostics".to_string();
        }
        self.diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.into_message())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl BundlerDiagnostic {
    fn into_message(self) -> String {
        match self.module_id {
            Some(module_id) => format!("{module_id}: {}", self.message),
            None => self.message,
        }
    }
}

struct RolldownDiagnosticKind {
    event_kind: EventKind,
}

impl RolldownDiagnosticKind {
    fn new(event_kind: EventKind) -> Self {
        Self { event_kind }
    }

    fn kind(&self) -> RolldownErrorKind {
        match self.event_kind {
            EventKind::UnresolvedEntry | EventKind::UnresolvedImport | EventKind::ResolveError => {
                RolldownErrorKind::UnresolvedImport
            }
            EventKind::UnloadableDependencyError => RolldownErrorKind::LoadFailed,
            EventKind::TransformError | EventKind::ToleratedTransform | EventKind::PluginError => {
                RolldownErrorKind::TransformFailed
            }
            EventKind::ParseError | EventKind::JsonParseError => RolldownErrorKind::ParseError,
            EventKind::CircularDependency | EventKind::CircularReexportError => {
                RolldownErrorKind::CircularDependency
            }
            EventKind::FilenameConflict => RolldownErrorKind::OutputConflict,
            _ => RolldownErrorKind::Other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_rolldown_diagnostic_classifies_unresolved_entry() {
        let error = BuildDiagnostic::unresolved_entry("missing", None);
        let diag = BundlerDiagnostic::from_rolldown_diagnostic(&error);
        assert_eq!(diag.kind, RolldownErrorKind::UnresolvedImport);
    }

    #[test]
    fn from_plugin_error_has_transform_kind() {
        let diag = BundlerDiagnostic::from_plugin_error("entry/Index.ets", "transform failed");
        assert_eq!(diag.kind, RolldownErrorKind::TransformFailed);
        assert_eq!(diag.module_id.as_deref(), Some("entry/Index.ets"));
    }
}
