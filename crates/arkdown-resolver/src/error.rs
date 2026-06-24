/// Errors that can occur during module resolution.
#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error("Module not found: '{specifier}' from '{importer}'")]
    NotFound { specifier: String, importer: String },
    #[error("SDK api dir not found for '{specifier}'")]
    SdkModuleNotFound { specifier: String },
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
