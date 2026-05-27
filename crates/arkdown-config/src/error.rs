use std::path::PathBuf;

/// Errors produced by the config parser.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("JSON5 parse error in {file}: {message}")]
    ParseError { file: PathBuf, message: String },

    #[error("SDK not found. Set OHOS_SDK_HOME or install DevEco Studio.")]
    SdkNotFound,

    #[error("Product '{0}' not found in build-profile.json5")]
    ProductNotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
