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

    #[error("Plugin '{plugin}' failed: {source}")]
    PluginError {
        plugin: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },

    #[error("ABC compilation failed: {message}")]
    AbcCompilationFailed { message: String },

    #[error("Bundler error: {0}")]
    Bundler(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type PluginResult<T> = Result<T, ArkdownError>;
