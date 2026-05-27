/// Errors produced by the resource compiler.
#[derive(Debug, thiserror::Error)]
pub enum ResourceError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parse error in {file}: {message}")]
    Parse { file: String, message: String },
}
