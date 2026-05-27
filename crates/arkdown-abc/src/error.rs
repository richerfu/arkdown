/// Errors produced by the ABC compiler.
#[derive(Debug, thiserror::Error)]
pub enum AbcError {
    #[error("Parse error in '{file}': {message}")]
    Parse { file: String, message: String },

    #[error("Unsupported construct '{construct}' in '{file}'")]
    UnsupportedConstruct { construct: String, file: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
