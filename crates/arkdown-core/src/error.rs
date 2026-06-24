use thiserror::Error;

/// Top-level error type for all arkdown operations.
#[derive(Debug, Error)]
pub enum ArkdownError {
    #[error("Resolution failed for '{specifier}' from '{importer:?}': {reason}")]
    ResolutionFailed {
        specifier: String,
        importer: Option<String>,
        reason: String,
    },

    #[error("Parse error in '{file}': {message}")]
    ParseError { file: String, message: String },

    #[error("ABC compilation failed: {message}")]
    AbcCompilationFailed { message: String },

    #[error("Bundler error: {0}")]
    Bundler(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
