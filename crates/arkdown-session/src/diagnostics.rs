//! Startup and planning diagnostics with stable file references.

use std::path::PathBuf;

/// Diagnostic severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// A single diagnostic message.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub file: Option<PathBuf>,
    pub hint: Option<String>,
}

/// Collection of diagnostics produced during session startup.
#[derive(Debug, Clone, Default)]
pub struct SessionDiagnostics {
    pub items: Vec<Diagnostic>,
}

impl SessionDiagnostics {
    /// Add an error diagnostic.
    pub fn error(&mut self, message: impl Into<String>, file: Option<PathBuf>) {
        self.items.push(Diagnostic {
            severity: Severity::Error,
            message: message.into(),
            file,
            hint: None,
        });
    }

    /// Add a warning diagnostic.
    pub fn warn(&mut self, message: impl Into<String>, file: Option<PathBuf>) {
        self.items.push(Diagnostic {
            severity: Severity::Warning,
            message: message.into(),
            file,
            hint: None,
        });
    }

    /// Return `true` if there are any error-severity diagnostics.
    pub fn has_errors(&self) -> bool {
        self.items
            .iter()
            .any(|d| d.severity == Severity::Error)
    }

    /// Merge another diagnostic collection into this one.
    pub fn extend(&mut self, other: SessionDiagnostics) {
        self.items.extend(other.items);
    }
}
