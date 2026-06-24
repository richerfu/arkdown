use std::path::PathBuf;

use thiserror::Error;

/// A structured diagnostic from the ts2abc stage.
///
/// Unlike [`Ts2abcError`], a `Ts2abcDiagnostic` is non-fatal; it records an
/// anomalous condition that should be logged but does not abort the build.
#[derive(Debug, Clone)]
pub struct Ts2abcDiagnostic {
    /// Human-readable message.
    pub message: String,
    /// The unsupported feature name, if applicable.
    pub feature: Option<String>,
}

impl Ts2abcDiagnostic {
    /// Create a diagnostic for an unsupported feature.
    pub fn unsupported(feature: impl Into<String>, reason: impl Into<String>) -> Self {
        let feature_str = feature.into();
        Self {
            message: format!("{}: {}", feature_str, reason.into()),
            feature: Some(feature_str),
        }
    }
}

/// An unsupported feature warning produced by the ts2abc stage.
///
/// Distinct from [`Ts2abcError::UnsupportedFeature`] (which is fatal):
/// `Ts2abcUnsupportedFeature` is a non-fatal variant recorded for parity
/// reporting.
#[derive(Debug, Clone)]
pub struct Ts2abcUnsupportedFeature {
    /// The feature name (e.g., `"AOT"`, `"dumpSymbolTable"`).
    pub feature: String,
    /// Human-readable explanation of why the feature is unsupported.
    pub reason: String,
    /// Whether the build should fail rather than continue.
    pub is_fatal: bool,
}

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
    #[error("SDK es2abc compiler does not exist: {0}")]
    SdkCompilerNotFound(PathBuf),
    #[error("SDK es2abc compiler is not executable: {0}")]
    SdkCompilerNotExecutable(PathBuf),
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
