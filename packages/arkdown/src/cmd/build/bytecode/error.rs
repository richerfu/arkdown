use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Ts2abcError {
  /// The typed plan is invalid (missing fields or inconsistent configuration).
  ///
  /// # Upstream correspondence
  ///
  /// Corresponds to hvigor's `genAbc` plan validation errors before
  /// launching `es2abc`.
  #[error("invalid ts2abc plan: {0}")]
  InvalidPlan(String),
  #[error("missing value for {0}")]
  MissingValue(String),
  #[error("no input source files were provided")]
  MissingInput,
  #[error("SDK es2abc compiler does not exist: {0}")]
  SdkCompilerNotFound(PathBuf),
  #[error("SDK es2abc compiler is not executable: {0}")]
  SdkCompilerNotExecutable(PathBuf),
  #[error("failed to inspect SDK es2abc options for {program}: {reason}")]
  SdkCompilerOptionProbeFailed { program: PathBuf, reason: String },
  #[error("SDK es2abc {program} does not support command options: {options}")]
  UnsupportedSdkCompilerOptions { program: PathBuf, options: String },
  #[error("failed to run SDK es2abc {program}: {source}")]
  SdkCompilerIo {
    program: String,
    #[source]
    source: std::io::Error,
  },
  #[error("SDK es2abc failed with status {status}: {stderr}")]
  SdkCompilerFailed { status: String, stderr: String },
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[error(transparent)]
  Path(#[from] arkdown_path::serializer::SerializerError),
}
