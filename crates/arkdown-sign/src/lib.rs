//! Pure Rust HAP signing — no Java subprocess, no SDK binary.
//!
//! Ports `developtools_hapsigner` (Apache 2.0) from Java/BouncyCastle to Rust:
//! - PKCS#12 keystore loading via `p12` crate
//! - CMS/PKCS#7 SignedData generation via manual DER encoding
//! - ECDSA P-256 / RSA-PSS signing via `p256` / `rsa` crates
//! - ZIP sort/align (4096-byte for `.abc`/`.so`) and signing block insertion
//! - HAP-specific chunked SHA-256 content digest

pub mod digest;
pub mod error;
pub mod keystore;
pub mod pkcs7;
pub mod signing_block;
pub mod zip;

pub use error::SignError;
pub use keystore::SigningKey;

use arkdown_core::SigningConfig;
use std::path::Path;

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
        let p12_bytes = fs_err::read(&config.store_file)?;

        let store_password = std::env::var("ARKDOWN_STORE_PASSWORD")
            .unwrap_or_else(|_| config.store_password.clone());
        let key_password = std::env::var("ARKDOWN_KEY_PASSWORD")
            .unwrap_or_else(|_| config.key_password.clone());

        let signing_key = SigningKey::from_pkcs12(
            &p12_bytes,
            &store_password,
            &config.key_alias,
            &key_password,
        )?;

        let profile = fs_err::read(&config.profile_path)?;
        let alg_id = parse_alg_id(&config.sign_alg);

        Ok(HapSigner {
            signing_key,
            profile,
            alg_id,
            compatible_sdk_version,
        })
    }

    /// Sign an unsigned HAP (raw bytes) and return the signed HAP bytes.
    pub fn sign(&self, unsigned_hap: &[u8]) -> Result<Vec<u8>, SignError> {
        // 1. Parse and prepare ZIP
        let mut hap_zip = zip::HapZip::parse(unsigned_hap)?;
        hap_zip.sort_entries();
        hap_zip.align();

        // 2. Get sections for digest
        let (entries_bytes, cd_bytes, eocd_zeroed) = hap_zip.sections_for_signing();

        // 3. Compute content digest
        let content_digest =
            digest::compute_hap_digest(&entries_bytes, &[], &cd_bytes, &eocd_zeroed);

        // 4. Encode (alg_id, digest) pairs
        let digest_pairs =
            signing_block::encode_digest_pairs(&[(self.alg_id, &content_digest)]);

        // 5. Build CMS SignedData
        let cms_data = pkcs7::build_cms_signed_data(
            &digest_pairs,
            &self.signing_key.private_key_der,
            &self.signing_key.cert_chain,
            self.alg_id,
        )?;

        // 6. Build optional blocks: profile + main signature
        let mut blocks: Vec<(u32, Vec<u8>)> = Vec::new();
        blocks.push((signing_block::BLOCK_ID_PROFILE, self.profile.clone()));
        blocks.push((signing_block::BLOCK_ID_SIGNATURE_V1, cms_data));

        // 7. Assemble signing block
        let sb = signing_block::build_signing_block(&blocks, self.compatible_sdk_version);

        // 8. Write final HAP
        Ok(hap_zip.with_signing_block(&sb))
    }

    /// Sign an unsigned HAP file and write the signed output to `output`.
    pub fn sign_file(&self, input: &Path, output: &Path) -> Result<(), SignError> {
        let unsigned = fs_err::read(input)?;
        let signed = self.sign(&unsigned)?;
        fs_err::write(output, &signed)?;
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
                eprintln!(
                    "warning: no signing config — output is unsigned: {}",
                    output.display()
                );
                Ok(())
            }
            Some(cfg) => {
                let signer = Self::from_config(cfg, compatible_sdk_version)?;
                signer.sign_file(input, output)
            }
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

// ---------------------------------------------------------------------------
// SPEC-022: hvigorw compatibility wrapper
// ---------------------------------------------------------------------------

/// The hvigorw compatibility shell script template.
///
/// Drop this at the project root as `hvigorw` to make the project buildable
/// with the standard `./hvigorw assembleHap` invocation.
pub const HVIGORW_SCRIPT: &str = r#"#!/bin/bash
# hvigorw — drop-in replacement using arkdown
# Generated by: arkdown init --compat

ARKDOWN_BIN="${ARKDOWN_HOME:-$(which arkdown 2>/dev/null)}"
if [ -z "$ARKDOWN_BIN" ]; then
    echo "ERROR: arkdown not found. Install arkdown or set ARKDOWN_HOME." >&2
    exit 2
fi

CMD="${1:-default}"; shift 2>/dev/null || true
ARGS=()
MODE="debug"
MODULE=""
PRODUCT=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --mode)    MODE="$2";    shift 2 ;;
        -m)        MODULE="$2";  shift 2 ;;
        --product) PRODUCT="$2"; shift 2 ;;
        -p)        ARGS+=("-p" "$2"); shift 2 ;;
        *)         ARGS+=("$1"); shift ;;
    esac
done

TARGET_ARGS=()
[[ -n "$MODULE" ]]  && TARGET_ARGS+=("--module"  "$MODULE")
[[ -n "$PRODUCT" ]] && TARGET_ARGS+=("--product" "$PRODUCT")

case "$CMD" in
    assembleHap)  exec "$ARKDOWN_BIN" build --target hap --mode "$MODE" "${TARGET_ARGS[@]}" "${ARGS[@]}" ;;
    assembleHsp)  exec "$ARKDOWN_BIN" build --target hsp --mode "$MODE" "${TARGET_ARGS[@]}" "${ARGS[@]}" ;;
    assembleHar)  exec "$ARKDOWN_BIN" build --target har --mode "$MODE" "${TARGET_ARGS[@]}" "${ARGS[@]}" ;;
    assembleApp)  exec "$ARKDOWN_BIN" build --target app --mode "$MODE" "${TARGET_ARGS[@]}" "${ARGS[@]}" ;;
    clean)        exec "$ARKDOWN_BIN" clean "${TARGET_ARGS[@]}" "${ARGS[@]}" ;;
    test)         exec "$ARKDOWN_BIN" test  "${TARGET_ARGS[@]}" "${ARGS[@]}" ;;
    default|"")   exec "$ARKDOWN_BIN" build "${TARGET_ARGS[@]}" "${ARGS[@]}" ;;
    *)
        echo "ERROR: Unknown hvigorw command: $CMD" >&2
        echo "Supported: assembleHap, assembleHsp, assembleHar, assembleApp, clean, test" >&2
        exit 2
        ;;
esac
"#;

/// Write the hvigorw compatibility script to the project root.
pub fn write_hvigorw_script(project_root: &Path) -> std::io::Result<()> {
    let script_path = project_root.join("hvigorw");
    std::fs::write(&script_path, HVIGORW_SCRIPT)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&script_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script_path, perms)?;
    }

    println!("Written: {}", script_path.display());
    Ok(())
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
    fn test_hvigorw_script_present() {
        assert!(HVIGORW_SCRIPT.contains("assembleHap"));
    }

    #[test]
    fn test_hvigorw_script_contains_all_commands() {
        assert!(HVIGORW_SCRIPT.contains("assembleHsp"));
        assert!(HVIGORW_SCRIPT.contains("assembleHar"));
        assert!(HVIGORW_SCRIPT.contains("assembleApp"));
        assert!(HVIGORW_SCRIPT.contains("clean"));
        assert!(HVIGORW_SCRIPT.contains("--mode"));
    }

    #[test]
    fn test_write_hvigorw_script() {
        let tmp = tempfile::tempdir().unwrap();
        write_hvigorw_script(tmp.path()).unwrap();
        let script = tmp.path().join("hvigorw");
        assert!(script.exists());
        let content = std::fs::read_to_string(&script).unwrap();
        assert!(content.contains("assembleHap"));
        assert!(content.contains("arkdown"));
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
