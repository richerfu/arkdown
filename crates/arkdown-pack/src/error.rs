#[derive(Debug, thiserror::Error)]
pub enum PackError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Directory traversal error: {0}")]
    WalkDir(#[from] walkdir::Error),
    #[error("Zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("Raw zip error: {0}")]
    RawZip(#[from] rawzip::Error),
    #[error("Missing required input: {0}")]
    MissingInput(String),
    #[error("Integrated HSP dependency '{alias}' is missing its pre-built archive: {path}")]
    MissingIntegratedHsp { alias: String, path: String },
    #[error("Malformed pack.info: {0}")]
    MalformedPackInfo(String),
    #[error("Invalid archive filter pattern '{pattern}': {message}")]
    InvalidPattern { pattern: String, message: String },
    #[error("Unsupported ZIP compression for {entry}: {method}")]
    UnsupportedZipCompression { entry: String, method: String },
    #[error("Invalid APP packaging plan: {0}")]
    InvalidAppPlan(String),
    #[error("Duplicate APP artifact output name: {0}")]
    DuplicateAppArtifactName(String),
    #[error("npm HAR packaging launcher does not exist: {0}")]
    MissingNpmLauncher(String),
    #[error("npm HAR packaging failed with status {status:?}; stdout: {stdout}; stderr: {stderr}")]
    NpmPackFailed {
        status: Option<i32>,
        stdout: String,
        stderr: String,
    },
    #[error("npm HAR packaging returned an invalid archive name: {0}")]
    InvalidNpmPackOutput(String),
    #[error("Invalid remote HSP layout at {path}: {message}")]
    InvalidRemoteHsp { path: String, message: String },
    #[error(
        "SDK APP packing tool failed with status {status:?}; stdout: {stdout}; stderr: {stderr}"
    )]
    AppPackingToolFailed {
        status: Option<i32>,
        stdout: String,
        stderr: String,
    },
    #[error(
        "SDK HQF packing tool failed with status {status:?}; stdout: {stdout}; stderr: {stderr}"
    )]
    HqfPackingToolFailed {
        status: Option<i32>,
        stdout: String,
        stderr: String,
    },
}
