use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Ts2abcError {
    #[error("missing value for {0}")]
    MissingValue(String),
    #[error("invalid value for {flag}: {value}")]
    InvalidValue { flag: String, value: String },
    #[error("unsupported ts2abc/es2abc flag: {0}")]
    UnsupportedFlag(String),
    #[error("no input source files were provided")]
    MissingInput,
    #[error("missing --output path for ABC output")]
    MissingOutput,
    #[error("input source does not exist: {0}")]
    MissingSource(PathBuf),
    #[error("source-map output is not implemented by the Rust backend yet")]
    SourceMapNotImplemented,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Abc(#[from] arkdown_abc::AbcError),
}
