use std::path::PathBuf;

/// Errors produced by the config parser.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("JSON5 parse error in {file}: {message}")]
    ParseError { file: PathBuf, message: String },

    #[error("TOML parse error in {file}: {message}")]
    TomlParseError { file: PathBuf, message: String },

    #[error("Schema validate failed in {file}: {message}")]
    ValidationError { file: PathBuf, message: String },

    #[error("Unsupported build feature '{feature}' in {file}: {message}")]
    UnsupportedFeature {
        feature: &'static str,
        file: PathBuf,
        message: String,
    },

    #[error("SDK not found. Set OHOS_SDK_HOME or install DevEco Studio.")]
    SdkNotFound,

    #[error("Product '{0}' not found in build-profile.json5")]
    ProductNotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Parse JSON5 text into `T`, reporting failures against `file`.
///
/// Single parse entry for every JSON5 read in this crate so error formatting
/// cannot drift between call sites.
pub(crate) fn parse_json5<T: serde::de::DeserializeOwned>(
    content: &str,
    file: &std::path::Path,
) -> Result<T, ConfigError> {
    json5::from_str::<T>(content).map_err(|e| ConfigError::ParseError {
        file: file.to_path_buf(),
        message: e.to_string(),
    })
}
