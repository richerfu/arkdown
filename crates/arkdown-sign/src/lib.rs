//! HAP signing primitives for the Rust signing backend.
//!
//! The CLI may select an SDK signing backend for hvigor parity. This crate owns
//! the in-process Rust implementation when that backend is selected.
//!
//! Ports `developtools_hapsigner` (Apache 2.0) from Java/BouncyCastle to Rust:
//! - PKCS#12 keystore loading via `p12` crate
//! - CMS/PKCS#7 SignedData generation via manual DER encoding
//! - ECDSA P-256 / RSA-PSS signing via `p256` / `rsa` crates
//! - ZIP sort/align (4096-byte for `.abc`/`.so`) and signing block insertion
//! - HAP-specific chunked SHA-256 content digest

pub mod backend;
mod config_material;
pub mod digest;
pub mod error;
pub mod keystore;
pub mod pkcs7;
pub mod plan;
pub mod signing_block;
pub mod zip;

pub use backend::{RustSignBackend, SdkSignBackend, SignBackend, SignBackendRunner};
pub use error::SignError;
pub use keystore::SigningKey;
pub use plan::{SignKind, SignPlan, SigningMaterial, SigningPassword, SigningTool};

use arkdown_core::SigningConfig;
use std::path::Path;

use crate::config_material::SigningConfigMaterial;

// ---------------------------------------------------------------------------
// HapSigner — public signing API
// ---------------------------------------------------------------------------

/// Stateful HAP signer. Owns the loaded key + profile.
pub struct HapSigner {
    signing_key: SigningKey,
    profile: Vec<u8>,
    alg_id: u32,
    compatible_sdk_version: u32,
}

impl HapSigner {
    /// Create from a `SigningConfig` (loads the PKCS#12 keystore).
    ///
    /// Passwords are read from the config. Set `ARKDOWN_STORE_PASSWORD` /
    /// `ARKDOWN_KEY_PASSWORD` environment variables to override (needed when
    /// the project stores DevEco-encrypted passwords in the config file).
    pub fn from_config(
        config: &SigningConfig,
        compatible_sdk_version: u32,
    ) -> Result<Self, SignError> {
        let material = SigningConfigMaterial::new(config);
        let p12_bytes = fs_err::read(material.store_file())?;
        let store_password = material.store_password()?;
        let key_password = material.key_password()?;

        let signing_key = SigningKey::from_pkcs12(
            &p12_bytes,
            &store_password,
            material.key_alias(),
            &key_password,
        )?;

        let profile = fs_err::read(material.profile_path())?;
        let alg_id = Self::parse_alg_id(material.sign_alg());

        Ok(HapSigner {
            signing_key,
            profile,
            alg_id,
            compatible_sdk_version,
        })
    }

    /// Sign an unsigned HAP (raw bytes) and return the signed HAP bytes.
    pub fn sign(&self, unsigned_hap: &[u8]) -> Result<Vec<u8>, SignError> {
        let mut hap_zip = zip::HapZip::parse(unsigned_hap)?;
        let signing_block = self.signing_block_for_zip(&mut hap_zip)?;
        Ok(hap_zip.with_signing_block(&signing_block))
    }

    fn signing_block_for_zip(&self, hap_zip: &mut zip::HapZip) -> Result<Vec<u8>, SignError> {
        // 1. Prepare ZIP
        hap_zip.sort_entries();
        hap_zip.align();

        let content_digest = hap_zip.content_digest_for_signing()?;

        // 4. Encode (alg_id, digest) pairs
        let digest_pairs = signing_block::encode_digest_pairs(&[(self.alg_id, &content_digest)]);

        // 5. Build CMS SignedData
        let cms_data = pkcs7::build_cms_signed_data(
            &digest_pairs,
            &self.signing_key.private_key_der,
            &self.signing_key.cert_chain,
            self.alg_id,
        )?;

        // 6. Build optional blocks: profile + main signature
        let blocks: Vec<(u32, Vec<u8>)> = vec![
            (signing_block::BLOCK_ID_PROFILE, self.profile.clone()),
            (signing_block::BLOCK_ID_SIGNATURE_V1, cms_data),
        ];

        // 7. Assemble signing block
        Ok(signing_block::build_signing_block(
            &blocks,
            self.compatible_sdk_version,
        ))
    }

    /// Sign an unsigned HAP file and write the signed output to `output`.
    pub fn sign_file(&self, input: &Path, output: &Path) -> Result<(), SignError> {
        let mut hap_zip = zip::HapZip::parse_file(input)?;
        let signing_block = self.signing_block_for_zip(&mut hap_zip)?;
        let mut output_file = fs_err::File::create(output)?;
        hap_zip.write_with_signing_block(&mut output_file, &signing_block)?;
        Ok(())
    }

    /// Sign or skip:
    /// - `None` config → copy unsigned file to output with a warning
    /// - `Some(cfg)` → load keystore and sign
    pub fn sign_or_skip(
        input: &Path,
        output: &Path,
        config: Option<&SigningConfig>,
        compatible_sdk_version: u32,
    ) -> Result<(), SignError> {
        match config {
            None => {
                if input != output {
                    std::fs::copy(input, output)?;
                }
                tracing::warn!(output = %output.display(), "no signing config; output is unsigned");
                Ok(())
            }
            Some(cfg) => {
                let signer = Self::from_config(cfg, compatible_sdk_version)?;
                signer.sign_file(input, output)
            }
        }
    }

    fn parse_alg_id(sign_alg: &str) -> u32 {
        match sign_alg {
            "SHA256withECDSA" | "ECDSA_WITH_SHA256" => signing_block::ALG_ECDSA_SHA256,
            "SHA256withRSA/PSS" | "SHA256withRSA" => signing_block::ALG_RSA_PSS_SHA256,
            _ => signing_block::ALG_ECDSA_SHA256, // default
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signing_block_magic_v2() {
        let block = signing_block::build_signing_block(&[(0x20000000, vec![1, 2, 3, 4])], 5);
        let len = block.len();
        let magic_start = len - 4 - 16; // 4 for version, 16 for magic
        assert_eq!(&block[magic_start..magic_start + 16], b"HAP Sig Block 42");
    }

    #[test]
    fn test_signing_block_magic_v3() {
        let block = signing_block::build_signing_block(&[(0x20000000, vec![1, 2, 3, 4])], 12);
        let len = block.len();
        let magic_start = len - 4 - 16;
        assert_eq!(&block[magic_start..magic_start + 16], b"<hap sign block>");
    }

    #[test]
    fn test_chunk_digest_deterministic() {
        let d1 = digest::compute_hap_digest(b"hello", &[], b"world", b"eocd");
        let d2 = digest::compute_hap_digest(b"hello", &[], b"world", b"eocd");
        assert_eq!(d1, d2);
    }

    #[test]
    fn test_encode_digest_pairs() {
        let digest_val = [0xabu8; 32];
        let encoded = signing_block::encode_digest_pairs(&[(0x201, &digest_val)]);
        assert_eq!(&encoded[0..4], &0x201u32.to_le_bytes());
        assert_eq!(&encoded[4..8], &32u32.to_le_bytes());
        assert_eq!(&encoded[8..40], &digest_val[..]);
    }

    #[test]
    fn test_signing_block_structure() {
        // Verify the total_size field is correct.
        let data = vec![0xFFu8; 100];
        let block = signing_block::build_signing_block(&[(0x20000000, data)], 10);
        // total = 1*12 (headers) + 100 (data) + 4 (count) + 8 (size) + 16 (magic) + 4 (version)
        let expected_total: u64 = (1 * 12 + 100 + 4 + 8 + 16 + 4) as u64;
        // total_size field is at: block.len() - 4 (version) - 16 (magic) - 8 (size)
        let total_start = block.len() - 4 - 16 - 8;
        let total = u64::from_le_bytes(block[total_start..total_start + 8].try_into().unwrap());
        assert_eq!(total, expected_total);
    }
}
