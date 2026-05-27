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

    #[error("Fatal ETS check error in {file}: {message}")]
    Fatal { file: String, message: String },
}
