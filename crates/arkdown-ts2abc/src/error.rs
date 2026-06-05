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
    #[error("unsupported ts2abc/es2abc feature {flag}: {reason}")]
    UnsupportedFeature { flag: String, reason: String },
    #[error("no input source files were provided")]
    MissingInput,
    #[error("input source does not exist: {0}")]
    MissingSource(PathBuf),
    /// Upstream es2abc rejects `--module` and `--commonjs` together
    /// (options.cpp: "cannot use --module and --commonjs at the same time").
    #[error("cannot use --module and --commonjs at the same time")]
    ConflictingFlags,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Abc(#[from] arkdown_abc::AbcError),
}
