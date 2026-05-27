use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum SignError {
    // ZIP errors
    #[error("invalid ZIP file: {0}")]
    InvalidZip(&'static str),

    // Digest errors
    #[error("digest error: {0}")]
    DigestError(String),

    // Keystore errors
    #[error("keystore load failed: {0}")]
    KeystoreLoadFailed(&'static str),

    #[error("keystore parse error: {0}")]
    KeystoreParseError(String),

    #[error("no private key found in keystore")]
    NoPrivateKey,

    #[error("no certificate found in keystore")]
    NoCertificate,

    // Signing errors
    #[error("signing failed: {0}")]
    SigningFailed(String),

    #[error("PKCS#8 key loading failed: {0}")]
    Pkcs8Error(String),

    // PEM errors
    #[error("PEM decode error: {0}")]
    PemError(String),

    // DER / ASN.1 errors
    #[error("DER encoding error: {0}")]
    DerError(String),

    // IO errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("io error reading '{path}': {source}")]
    IoPath {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}
