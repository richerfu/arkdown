use oxc_span::Span;

/// Severity level for ETS checker diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

/// Errors and diagnostics produced by the ETS transform pipeline.
#[derive(Debug, Clone, thiserror::Error)]
pub enum EtsTransformError {
    #[error("{severity:?} in {file}: {message}")]
    Diagnostic {
        file: String,
        message: String,
        severity: DiagnosticSeverity,
    },

    #[error("{severity:?} [{code}] in {file}: {message}")]
    CodedDiagnostic {
        file: String,
        code: String,
        message: String,
        severity: DiagnosticSeverity,
        span: Option<Span>,
        solutions: Vec<String>,
    },

    #[error("Fatal ETS check error in {file}: {message}")]
    Fatal { file: String, message: String },
}

impl EtsTransformError {
    pub fn is_error(&self) -> bool {
        match self {
            Self::Diagnostic { severity, .. } | Self::CodedDiagnostic { severity, .. } => {
                matches!(severity, DiagnosticSeverity::Error)
            }
            Self::Fatal { .. } => true,
        }
    }
}
