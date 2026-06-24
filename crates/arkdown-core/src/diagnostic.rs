//! Typed diagnostic messages for arkdown build stages.
//!
//! [`Diagnostic`] is the universal, structured error/warning type used across
//! all arkdown crates.  It replaces ad-hoc `String` error messages, scattered
//! stderr writes, and `ArkdownError` strings with a typed, attachable
//! diagnostic that traces to a hvigor logger call or hvigor error code.
//!
//! # Upstream correspondence
//!
//! | hvigor API | arkdown equivalent |
//! |------------|--------------------|
//! | `OhosLogger.printErrorExit(code, msg)` | `Diagnostic::error(code, msg).with_path(...)` |
//! | `OhosLogger.printWarnInfo(msg)` | `Diagnostic::warning(msg)` |
//! | `OhosLogger.printInfo(msg)` | `Diagnostic::info(msg)` |
//! | hvigor error codes (`ArkUI_XXXX`) | `DiagnosticCode::Arkui(u32)` |

use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// DiagnosticSeverity
// ---------------------------------------------------------------------------

/// Severity level of a [`Diagnostic`], matching hvigor's logger severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiagnosticSeverity {
    /// Informational message; does not fail the build.
    ///
    /// Corresponds to hvigor `OhosLogger.printInfo`.
    Info,
    /// Warning; does not fail the build but may indicate a problem.
    ///
    /// Corresponds to hvigor `OhosLogger.printWarnInfo`.
    Warning,
    /// Error; causes the build to fail.
    ///
    /// Corresponds to hvigor `OhosLogger.printErrorExit`.
    Error,
}

impl std::fmt::Display for DiagnosticSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            DiagnosticSeverity::Info => "info",
            DiagnosticSeverity::Warning => "warning",
            DiagnosticSeverity::Error => "error",
        })
    }
}

// ---------------------------------------------------------------------------
// DiagnosticCode
// ---------------------------------------------------------------------------

/// Structured hvigor error/warning code attached to a diagnostic.
///
/// Hvigor emits codes of the form `ArkUI_XXXX`, `ArkTS_XXXX`, or
/// `HvigorPlugin_XXXX`.  The numeric portion is embedded for exact
/// reproduction.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DiagnosticCode {
    /// An `ArkUI_XXXX` code (e.g., `ArkUI_0001`).
    ///
    /// Corresponds to `@ohos/hvigor-arkts-compose` error codes.
    Arkui(u32),
    /// An `ArkTS_XXXX` code (e.g., `ArkTS_0001`).
    ///
    /// Corresponds to ArkTS checker/validator error codes.
    Arkts(u32),
    /// A `HvigorPlugin_XXXX` code.
    ///
    /// Corresponds to hvigor-ohos-plugin error codes.
    HvigorPlugin(u32),
    /// A raw string code for codes not yet classified into the above variants.
    Raw(String),
}

impl std::fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiagnosticCode::Arkui(n) => write!(f, "ArkUI_{n:04}"),
            DiagnosticCode::Arkts(n) => write!(f, "ArkTS_{n:04}"),
            DiagnosticCode::HvigorPlugin(n) => write!(f, "HvigorPlugin_{n:04}"),
            DiagnosticCode::Raw(s) => f.write_str(s),
        }
    }
}

// ---------------------------------------------------------------------------
// DiagnosticAttachment
// ---------------------------------------------------------------------------

/// Supplementary information attached to a [`Diagnostic`].
///
/// Attachments let callers add contextual notes, code spans, or related file
/// references without folding them into the primary message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticAttachment {
    /// A plain-text note appended after the primary message.
    Note(String),
    /// A secondary source location (file path + optional line number).
    RelatedLocation { path: PathBuf, line: Option<u32> },
    /// A suggested fix or remediation hint.
    Hint(String),
}

// ---------------------------------------------------------------------------
// Diagnostic
// ---------------------------------------------------------------------------

/// A structured build diagnostic from any arkdown stage.
///
/// Diagnostics are collected during a build stage and emitted to the terminal
/// (or structured log) at the end of the stage.  Error-severity diagnostics
/// cause the build to fail; warning/info diagnostics do not.
///
/// # Relationship to hvigor
///
/// hvigor's `OhosLogger.printErrorExit(code, msg, [file])` call is replaced
/// by `Diagnostic::error(code, msg).with_path(file)`.
///
/// # Example
///
/// ```rust
/// use arkdown_core::diagnostic::{Diagnostic, DiagnosticCode, DiagnosticSeverity};
///
/// let d = Diagnostic::error("duplicate module name")
///     .with_path("entry/module.json5")
///     .with_note("a module named 'entry' already exists");
/// assert_eq!(d.severity, DiagnosticSeverity::Error);
/// ```
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// Severity level.
    pub severity: DiagnosticSeverity,
    /// Primary human-readable message.
    pub message: String,
    /// Optional source file path where the diagnostic originates.
    pub path: Option<PathBuf>,
    /// Optional 1-based line number within `path`.
    pub line: Option<u32>,
    /// Structured hvigor error/warning code, if applicable.
    pub code: Option<DiagnosticCode>,
    /// Zero or more supplementary attachments.
    pub attachments: Vec<DiagnosticAttachment>,
}

impl Diagnostic {
    /// Create a new error-severity diagnostic.
    ///
    /// Corresponds to hvigor `OhosLogger.printErrorExit`.
    pub fn error(message: impl Into<String>) -> Self {
        Self::new(DiagnosticSeverity::Error, message)
    }

    /// Create a new warning-severity diagnostic.
    ///
    /// Corresponds to hvigor `OhosLogger.printWarnInfo`.
    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(DiagnosticSeverity::Warning, message)
    }

    /// Create a new info-severity diagnostic.
    ///
    /// Corresponds to hvigor `OhosLogger.printInfo`.
    pub fn info(message: impl Into<String>) -> Self {
        Self::new(DiagnosticSeverity::Info, message)
    }

    fn new(severity: DiagnosticSeverity, message: impl Into<String>) -> Self {
        Self {
            severity,
            message: message.into(),
            path: None,
            line: None,
            code: None,
            attachments: Vec::new(),
        }
    }

    /// Attach a source file path.
    #[must_use]
    pub fn with_path(mut self, path: impl AsRef<Path>) -> Self {
        self.path = Some(path.as_ref().to_owned());
        self
    }

    /// Attach a 1-based source line number (requires `with_path` to be
    /// meaningful).
    #[must_use]
    pub fn with_line(mut self, line: u32) -> Self {
        self.line = Some(line);
        self
    }

    /// Attach a structured diagnostic code.
    ///
    /// Corresponds to the `code` parameter of hvigor's `printErrorExit`.
    #[must_use]
    pub fn with_diagnostic_code(mut self, code: DiagnosticCode) -> Self {
        self.code = Some(code);
        self
    }

    /// Append a plain-text note.
    ///
    /// Notes appear after the primary message in terminal output.
    #[must_use]
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.attachments
            .push(DiagnosticAttachment::Note(note.into()));
        self
    }

    /// Append a related source location.
    #[must_use]
    pub fn with_related_location(mut self, path: impl AsRef<Path>, line: Option<u32>) -> Self {
        self.attachments
            .push(DiagnosticAttachment::RelatedLocation {
                path: path.as_ref().to_owned(),
                line,
            });
        self
    }

    /// Append a hint / suggested fix.
    #[must_use]
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.attachments
            .push(DiagnosticAttachment::Hint(hint.into()));
        self
    }

    /// Returns `true` if the severity is `Error`.
    #[inline]
    pub fn is_error(&self) -> bool {
        self.severity == DiagnosticSeverity::Error
    }

    /// Returns `true` if the severity is `Warning` or higher.
    #[inline]
    pub fn is_at_least_warning(&self) -> bool {
        self.severity >= DiagnosticSeverity::Warning
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}]", self.severity)?;
        if let Some(code) = &self.code {
            write!(f, "[{code}]")?;
        }
        if let Some(path) = &self.path {
            write!(f, " {}:", path.display())?;
            if let Some(line) = self.line {
                write!(f, "{line}:")?;
            }
        }
        write!(f, " {}", self.message)?;
        for attachment in &self.attachments {
            match attachment {
                DiagnosticAttachment::Note(note) => write!(f, "\n  note: {note}")?,
                DiagnosticAttachment::RelatedLocation { path, line } => {
                    write!(f, "\n  --> {}", path.display())?;
                    if let Some(l) = line {
                        write!(f, ":{l}")?;
                    }
                }
                DiagnosticAttachment::Hint(hint) => write!(f, "\n  hint: {hint}")?,
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_has_error_severity() {
        let d = Diagnostic::error("something went wrong");
        assert!(d.is_error());
        assert!(d.is_at_least_warning());
    }

    #[test]
    fn warning_is_not_error() {
        let d = Diagnostic::warning("watch out");
        assert!(!d.is_error());
        assert!(d.is_at_least_warning());
    }

    #[test]
    fn info_is_not_at_least_warning() {
        let d = Diagnostic::info("build started");
        assert!(!d.is_error());
        assert!(!d.is_at_least_warning());
    }

    #[test]
    fn with_path_and_code_display() {
        let d = Diagnostic::error("missing field")
            .with_path("entry/module.json5")
            .with_line(42)
            .with_diagnostic_code(DiagnosticCode::Arkui(1))
            .with_note("field 'name' is required");
        let s = d.to_string();
        assert!(s.contains("[ArkUI_0001]"));
        assert!(s.contains("entry/module.json5"));
        assert!(s.contains("42"));
        assert!(s.contains("note: field 'name' is required"));
    }

    #[test]
    fn builder_methods_chain_without_consuming_twice() {
        let d = Diagnostic::warning("test")
            .with_path("a.ts")
            .with_hint("do X instead")
            .with_note("context");
        assert_eq!(d.attachments.len(), 2);
    }
}
