#[derive(Debug, thiserror::Error)]
pub enum PackError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("Raw zip error: {0}")]
    RawZip(#[from] rawzip::Error),
    #[error("Missing required input: {0}")]
    MissingInput(String),
    #[error("Malformed pack.info: {0}")]
    MalformedPackInfo(String),
    #[error("Unsupported ZIP compression for {entry}: {method}")]
    UnsupportedZipCompression { entry: String, method: String },
}
