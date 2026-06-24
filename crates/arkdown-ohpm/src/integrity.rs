//! SRI integrity verification for OHPM package tarballs.
//!
//! Supports `sha512-<base64>` and `sha256-<base64>` SRI hash formats.

use base64::Engine as _;
use sha2::{Digest, Sha256, Sha512};
use thiserror::Error;

/// An error produced during integrity verification.
#[derive(Debug, Error)]
pub enum IntegrityError {
    /// The integrity string uses an unsupported algorithm.
    #[error("unsupported integrity algorithm in: {0}")]
    UnsupportedAlgorithm(String),
    /// The base64 payload is malformed.
    #[error("malformed base64 in integrity hash: {0}")]
    MalformedBase64(String),
    /// The computed hash does not match the expected hash.
    #[error("integrity mismatch: expected {expected}, computed {computed}")]
    Mismatch { expected: String, computed: String },
}

/// Verify that `data` matches the SRI `integrity` string.
///
/// The integrity string must be in the format `<alg>-<base64>` where `<alg>`
/// is `sha512` or `sha256`.
///
/// # Errors
///
/// Returns an [`IntegrityError`] if the algorithm is unsupported, the base64
/// is malformed, or the hash does not match.
pub fn verify(data: &[u8], integrity: &str) -> Result<(), IntegrityError> {
    let parsed = IntegrityText::new(integrity).parse()?;

    let expected_bytes = base64::engine::general_purpose::STANDARD
        .decode(parsed.hash_b64)
        .map_err(|_| IntegrityError::MalformedBase64(integrity.to_string()))?;

    let computed_bytes = parsed.algorithm.digest(data);

    if computed_bytes != expected_bytes {
        let computed = format!(
            "{}-{}",
            parsed.algorithm.as_str(),
            base64::engine::general_purpose::STANDARD.encode(&computed_bytes)
        );
        return Err(IntegrityError::Mismatch {
            expected: integrity.to_string(),
            computed,
        });
    }

    Ok(())
}

struct IntegrityText<'a> {
    raw: &'a str,
}

impl<'a> IntegrityText<'a> {
    fn new(raw: &'a str) -> Self {
        Self { raw }
    }

    fn parse(&self) -> Result<ParsedIntegrity<'a>, IntegrityError> {
        let (algorithm, hash_b64) = self
            .raw
            .split_once('-')
            .ok_or_else(|| IntegrityError::UnsupportedAlgorithm(self.raw.to_string()))?;
        let algorithm = IntegrityAlgorithm::parse(algorithm)?;
        Ok(ParsedIntegrity {
            algorithm,
            hash_b64,
        })
    }
}

struct ParsedIntegrity<'a> {
    algorithm: IntegrityAlgorithm,
    hash_b64: &'a str,
}

#[derive(Clone, Copy)]
enum IntegrityAlgorithm {
    Sha512,
    Sha256,
}

impl IntegrityAlgorithm {
    fn parse(raw: &str) -> Result<Self, IntegrityError> {
        match raw {
            "sha512" => Ok(Self::Sha512),
            "sha256" => Ok(Self::Sha256),
            _ => Err(IntegrityError::UnsupportedAlgorithm(raw.to_string())),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Sha512 => "sha512",
            Self::Sha256 => "sha256",
        }
    }

    fn digest(self, data: &[u8]) -> Vec<u8> {
        match self {
            Self::Sha512 => {
                let mut hasher = Sha512::new();
                hasher.update(data);
                hasher.finalize().to_vec()
            }
            Self::Sha256 => {
                let mut hasher = Sha256::new();
                hasher.update(data);
                hasher.finalize().to_vec()
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha512};

    fn sha512_b64(data: &[u8]) -> String {
        let mut h = Sha512::new();
        h.update(data);
        format!(
            "sha512-{}",
            base64::engine::general_purpose::STANDARD.encode(h.finalize())
        )
    }

    #[test]
    fn test_verify_sha512_ok() {
        let data = b"hello world";
        let integrity = sha512_b64(data);
        assert!(verify(data, &integrity).is_ok());
    }

    #[test]
    fn test_verify_sha512_mismatch() {
        let data = b"hello world";
        let integrity = sha512_b64(b"different content");
        let err = verify(data, &integrity).unwrap_err();
        assert!(matches!(err, IntegrityError::Mismatch { .. }));
    }

    #[test]
    fn test_verify_unsupported_algorithm() {
        let err = verify(b"data", "md5-abc123").unwrap_err();
        assert!(matches!(err, IntegrityError::UnsupportedAlgorithm(_)));
    }

    #[test]
    fn test_verify_malformed_base64() {
        let err = verify(b"data", "sha512-!!!not_valid_base64!!!").unwrap_err();
        assert!(matches!(err, IntegrityError::MalformedBase64(_)));
    }

    #[test]
    fn test_verify_missing_separator() {
        let err = verify(b"data", "sha512xyz").unwrap_err();
        assert!(matches!(err, IntegrityError::UnsupportedAlgorithm(_)));
    }
}
