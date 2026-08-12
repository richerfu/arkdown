use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum SignError {
    #[error("signing configuration error: {0}")]
    Config(String),

    #[error("signing input '{path}' could not be read: {source}")]
    IoPath {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Signing(#[from] hapsigner::SignError),
}
