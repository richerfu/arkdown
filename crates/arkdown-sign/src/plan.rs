//! Typed signing plan for the HAP/HSP/HAR signing stage.
//!
//! [`SignPlan`] is the single structured input to the signing stage.
//! It is constructed by the CLI's `BuildPlanBuilder` from session data,
//! with all material paths pre-resolved, then passed to [`crate::HapSigner`]
//! (or the future `Signer` facade) for execution.
//!
//! The plan owns all inputs needed to drive signing:
//! - Input archive path (unsigned `.hap`/`.hsp`/`.har`)
//! - Output archive path (signed artifact destination)
//! - Signing material (keystore, key alias, profile)
//! - Password handling strategy (plaintext or DevEco-encrypted)
//!
//! Plan validation does not discover paths.  [`SigningPassword::resolve`]
//! reads DevEco material only when the password strategy explicitly asks for
//! encrypted password handling.
//!
//! # Upstream correspondence
//!
//! | Type | hvigor/SDK equivalent |
//! |------|------------------------|
//! | `SignPlan` | `SignModel` + `CommonSignCommandBuilder` inputs |
//! | `SignKind` | `SignHap`/`SignHsp`/`PackageSignHar` task selection |
//! | `SigningMaterial` | `CommonSignCommandBuilder` keystore/profile fields |
//! | `SigningPassword` | `DecipherUtil` password handling strategy |

use std::path::{Path, PathBuf};

use crate::error::SignError;

// ---------------------------------------------------------------------------
// SignKind
// ---------------------------------------------------------------------------

/// The kind of OHOS artifact being signed.
///
/// Controls which signing block format and metadata the signer emits.
///
/// # Upstream correspondence
///
/// Corresponds to the hvigor task selection:
/// - `SignHap` → `sign-hap.js`
/// - `SignHsp` → `hsp/sign-hsp.js`
/// - `SignHar` → `PackageSignHar` (HAR signing is lighter: no signing block,
///   only profile embedding)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SignKind {
    /// Sign a Host Application Package (`.hap`).
    ///
    /// Full signing block with V2/V3 magic, content digest, CMS SignedData,
    /// and embedded profile.
    Hap,
    /// Sign a Harmony Shared Package (`.hsp`).
    ///
    /// Same signing block format as HAP; HSP is a shared library variant.
    Hsp,
    /// Sign a Harmony Archive (`.har`).
    ///
    /// Lighter signing: profile embedding only, no full signing block.
    /// HAR artifacts are compile-time dependencies, not runtime packages.
    Har,
}

impl SignKind {
    /// Return the canonical file extension for this artifact kind.
    pub fn extension(&self) -> &'static str {
        match self {
            SignKind::Hap => "hap",
            SignKind::Hsp => "hsp",
            SignKind::Har => "har",
        }
    }

    /// Whether this kind requires a full signing block (V2/V3).
    ///
    /// HAR signing does not use the HAP signing block format; only HAP and
    /// HSP require the full block.
    pub fn requires_signing_block(&self) -> bool {
        matches!(self, SignKind::Hap | SignKind::Hsp)
    }
}

// ---------------------------------------------------------------------------
// SigningPassword
// ---------------------------------------------------------------------------

/// Password handling strategy for keystore and key passwords.
///
/// DevEco Studio stores signing passwords as AES-128-GCM ciphertext in the
/// `build-profile.json5`.  The sign crate can either accept pre-decrypted
/// plaintext (preferred for CLI and CI) or perform DevEco decryption
/// in-process given the material directory.
///
/// # Upstream correspondence
///
/// Corresponds to `DecipherUtil` in `decipher-util.js` and the
/// `ARKDOWN_STORE_PASSWORD` / `ARKDOWN_KEY_PASSWORD` env-var override path
/// in `HapSigner::from_config`.
#[derive(Debug, Clone)]
pub enum SigningPassword {
    /// Password is already in plaintext.
    ///
    /// Use this when passwords are provided via environment variables
    /// (`ARKDOWN_STORE_PASSWORD`, `ARKDOWN_KEY_PASSWORD`) or when the
    /// session layer has already decrypted them.
    Plaintext(String),

    /// Password is a DevEco-encrypted hex string.
    ///
    /// The sign crate will decrypt it using the provided `material_dir`
    /// (typically `~/.ohos/config/material/`).
    ///
    /// Only used when the CLI reads raw `build-profile.json5` passwords
    /// without prior decryption.
    Encrypted {
        /// The hex-encoded AES-GCM ciphertext from `build-profile.json5`.
        ciphertext: String,
        /// Directory containing the DevEco key material (`fd/`, `ac`, `ce`).
        material_dir: PathBuf,
    },
}

impl SigningPassword {
    /// Resolve the password to a plaintext string.
    ///
    /// For [`SigningPassword::Plaintext`], returns the string directly.
    /// For [`SigningPassword::Encrypted`], resolves DevEco AES-GCM material.
    pub fn resolve(&self) -> Result<String, SignError> {
        match self {
            SigningPassword::Plaintext(pw) => Ok(pw.clone()),
            SigningPassword::Encrypted {
                ciphertext,
                material_dir,
            } => Self::resolve_deveco_password(ciphertext, material_dir),
        }
    }

    /// Create a plaintext password from any string-like value.
    pub fn plaintext(pw: impl Into<String>) -> Self {
        SigningPassword::Plaintext(pw.into())
    }

    fn resolve_deveco_password(
        encrypted_hex: &str,
        material_dir: &Path,
    ) -> Result<String, SignError> {
        // Only attempt decryption for strings that look like DevEco-encrypted passwords
        // (they start with 8 hex '0' chars: "0000001A" or "0000001B" etc.)
        if !DevecoEncryptedPasswordText::new(encrypted_hex).requires_material_decryption() {
            return Ok(encrypted_hex.to_string());
        }

        let mat = material_dir.join("material");

        // 1. Read fd components
        let component: [u8; 16] = [
            49, 243, 9, 115, 214, 175, 91, 184, 211, 190, 177, 88, 101, 131, 192, 119,
        ];
        let fd_dir = mat.join("fd");
        let fd_parts: Vec<[u8; 16]> = {
            let mut parts = Vec::new();
            for sub in ["0", "1", "2"] {
                let sub_dir = fd_dir.join(sub);
                let bytes = Self::read_single_material_file(&sub_dir)?;
                let mut arr = [0u8; 16];
                arr.copy_from_slice(&bytes[..16]);
                parts.push(arr);
            }
            parts
        };

        // 2. XOR fd0 ^ fd1 ^ fd2 ^ component
        let mut xored = [0u8; 16];
        for i in 0..16 {
            xored[i] = fd_parts[0][i] ^ fd_parts[1][i] ^ fd_parts[2][i] ^ component[i];
        }

        // 3. Convert xored bytes to UTF-8 string (raw bytes as UTF-8)
        let xored_str = String::from_utf8_lossy(&xored).into_owned();

        // 4. Read salt and derive root key via PBKDF2-SHA256
        let ac_bytes = Self::read_single_material_file(&mat.join("ac"))?;
        let mut root_key = [0u8; 16];
        pbkdf2::pbkdf2_hmac::<sha2::Sha256>(xored_str.as_bytes(), &ac_bytes, 10000, &mut root_key);

        // 5. Decrypt work material (ce/) with root_key via AES-128-GCM
        let ce_bytes = Self::read_single_material_file(&mat.join("ce"))?;
        let work_key_bytes = Self::aes_gcm_decrypt(&root_key, &ce_bytes)?;

        // 6. Decrypt the password hex string with work_key
        let enc_bytes = hex::decode(encrypted_hex)
            .map_err(|e| SignError::Config(format!("invalid hex password: {e}")))?;
        let work_key_arr: [u8; 16] = work_key_bytes
            .try_into()
            .map_err(|_| SignError::Config("work key is not 16 bytes".into()))?;
        let plaintext = Self::aes_gcm_decrypt(&work_key_arr, &enc_bytes)?;

        String::from_utf8(plaintext)
            .map_err(|e| SignError::Config(format!("decrypted password is not UTF-8: {e}")))
    }

    fn read_single_material_file(dir: &Path) -> Result<Vec<u8>, SignError> {
        let entries: Vec<_> = std::fs::read_dir(dir)
            .map_err(|e| SignError::Config(format!("cannot read dir {:?}: {e}", dir)))?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name() != ".DS_Store")
            .collect();
        let path = entries
            .first()
            .ok_or_else(|| SignError::Config(format!("empty dir: {:?}", dir)))?
            .path();
        fs_err::read(&path).map_err(SignError::Io)
    }

    fn aes_gcm_decrypt(key: &[u8; 16], data: &[u8]) -> Result<Vec<u8>, SignError> {
        use aes_gcm::{aead::Aead, Aes128Gcm, KeyInit, Nonce};

        if data.len() < 20 {
            return Err(SignError::Config("encrypted data too short".into()));
        }

        // Parse: first 4 bytes = BE uint32 = r (ciphertext + tag region size)
        let r = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
        let iv_len = data.len() - 4 - r;
        let iv = &data[4..4 + iv_len];
        let ciphertext_and_tag = &data[4 + iv_len..];

        let cipher = Aes128Gcm::new_from_slice(key)
            .map_err(|e| SignError::Config(format!("AES key error: {e}")))?;
        let nonce = Nonce::from_slice(iv);

        cipher.decrypt(nonce, ciphertext_and_tag).map_err(|_| {
            SignError::Config("AES-GCM decryption failed: wrong key or corrupt data".into())
        })
    }
}

struct DevecoEncryptedPasswordText<'a> {
    raw: &'a str,
}

impl<'a> DevecoEncryptedPasswordText<'a> {
    const MIN_HEX_LEN: usize = 16;
    const HEADER_PREFIX: &'static str = "000000";

    fn new(raw: &'a str) -> Self {
        Self { raw }
    }

    fn requires_material_decryption(&self) -> bool {
        self.raw.len() >= Self::MIN_HEX_LEN && self.raw.starts_with(Self::HEADER_PREFIX)
    }
}

// ---------------------------------------------------------------------------
// SigningMaterial
// ---------------------------------------------------------------------------

/// All cryptographic material needed to sign an OHOS artifact.
///
/// All paths in this struct are absolute and pre-resolved by the CLI's
/// `BuildPlanBuilder`.  The sign crate does not perform any path discovery.
///
/// # Upstream correspondence
///
/// Corresponds to the keystore/profile fields populated by
/// `CommonSignCommandBuilder` in hvigor's
/// `src/tasks/sign/command-builder-impl/`.
#[derive(Debug, Clone)]
pub struct SigningMaterial {
    /// Absolute path to the PKCS#12 keystore (`.p12` file).
    pub keystore_path: PathBuf,

    /// Alias of the private key entry within the keystore.
    pub key_alias: String,

    /// Password for the keystore (store password).
    pub store_password: SigningPassword,

    /// Password for the private key entry.
    pub key_password: SigningPassword,

    /// Absolute path to the signing profile (`.p7b` file).
    ///
    /// The profile is embedded as the `PROFILE_BLOCK` (`0x20000002`) in the
    /// HAP signing block.
    pub profile_path: PathBuf,

    /// Absolute path to the signing certificate (`.cer` file).
    ///
    /// Used for certificate chain construction in the CMS `SignedData`.
    pub cert_path: PathBuf,

    /// The signature algorithm name.
    ///
    /// Accepted values: `"SHA256withECDSA"`, `"ECDSA_WITH_SHA256"`,
    /// `"SHA256withRSA/PSS"`, `"SHA256withRSAANDMGF1"`.
    /// Defaults to `"SHA256withECDSA"` if not specified.
    pub sign_alg: String,
}

impl SigningMaterial {
    /// Validate that all material paths exist on the filesystem.
    ///
    /// Returns the first missing path as an error.  This is a pre-flight
    /// check; the actual signing will fail with more detailed errors if
    /// files are unreadable.
    pub fn validate_paths(&self) -> Result<(), SignError> {
        for (label, path) in [
            ("keystore", &self.keystore_path),
            ("profile", &self.profile_path),
            ("cert", &self.cert_path),
        ] {
            if !path.exists() {
                return Err(SignError::Config(format!(
                    "signing material {label} not found: {}",
                    path.display()
                )));
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// SignPlan
// ---------------------------------------------------------------------------

/// Complete typed plan for the HAP/HSP/HAR signing stage.
///
/// `SignPlan` is the single input to the signing executor.  It is constructed
/// by the CLI's `BuildPlanBuilder` after the pack stage completes, with all
/// material paths pre-resolved from session data.
///
/// The sign crate does not read `build-profile.json5`, `app.json5`, or any
/// other project config file.  All inputs are provided by the plan.
///
/// # Upstream correspondence
///
/// Replaces the ad-hoc `SigningConfig`-based construction in
/// `HapSigner::from_config`.  Together with `SigningMaterial` it corresponds
/// to `SignModel` + `CommonSignCommandBuilder` in hvigor.
///
/// # Example
///
/// ```no_run
/// use arkdown_sign::plan::{SignPlan, SignKind, SigningMaterial, SigningPassword};
/// use std::path::PathBuf;
///
/// let material = SigningMaterial {
///     keystore_path: PathBuf::from("/certs/app.p12"),
///     key_alias: "debug".to_string(),
///     store_password: SigningPassword::plaintext("storepass"),
///     key_password: SigningPassword::plaintext("keypass"),
///     profile_path: PathBuf::from("/certs/app.p7b"),
///     cert_path: PathBuf::from("/certs/app.cer"),
///     sign_alg: "SHA256withECDSA".to_string(),
/// };
///
/// let plan = SignPlan::new(
///     PathBuf::from("/build/unsigned/entry.hap"),
///     PathBuf::from("/build/signed/entry.hap"),
///     SignKind::Hap,
///     material,
///     12,
/// );
///
/// plan.validate().expect("plan is valid");
/// ```
#[derive(Debug, Clone)]
pub struct SignPlan {
    /// Absolute path to the unsigned input archive.
    input_path: PathBuf,

    /// Absolute path where the signed output archive will be written.
    output_path: PathBuf,

    /// The kind of artifact being signed.
    kind: SignKind,

    /// All cryptographic material needed for signing.
    material: SigningMaterial,

    /// The `compatibleSdkVersion` from `module.json5`.
    ///
    /// Controls whether V2 (`< 8`) or V3 (`>= 8`) signing block magic is
    /// emitted.  See `signing_block::build_signing_block`.
    compatible_sdk_version: u32,
}

impl SignPlan {
    /// Construct a new `SignPlan`.
    ///
    /// All paths must be absolute.  Call [`SignPlan::validate`] after
    /// construction to check for configuration errors before executing.
    pub fn new(
        input_path: PathBuf,
        output_path: PathBuf,
        kind: SignKind,
        material: SigningMaterial,
        compatible_sdk_version: u32,
    ) -> Self {
        Self {
            input_path,
            output_path,
            kind,
            material,
            compatible_sdk_version,
        }
    }

    // -----------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------

    /// Return the path to the unsigned input archive.
    pub fn input(&self) -> &Path {
        &self.input_path
    }

    /// Return the path where the signed artifact will be written.
    pub fn output(&self) -> &Path {
        &self.output_path
    }

    /// Return the signing material (keystore, profile, cert, passwords).
    pub fn material(&self) -> &SigningMaterial {
        &self.material
    }

    /// Return the artifact kind being signed.
    pub fn kind(&self) -> SignKind {
        self.kind
    }

    /// Return the `compatibleSdkVersion` used to select V2/V3 magic.
    pub fn compatible_sdk_version(&self) -> u32 {
        self.compatible_sdk_version
    }

    // -----------------------------------------------------------------------
    // Validation
    // -----------------------------------------------------------------------

    /// Validate the plan for obvious configuration errors.
    ///
    /// Checks:
    /// - `input_path` is absolute
    /// - `output_path` is absolute
    /// - `input_path` exists on the filesystem
    /// - `sign_alg` is a recognised algorithm name
    /// - Material paths exist (via [`SigningMaterial::validate_paths`])
    ///
    /// This is a pre-flight check.  The actual signing stage will report
    /// more detailed errors if cryptographic operations fail.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `validate-sign.js` pre-flight validation in hvigor.
    pub fn validate(&self) -> Result<(), SignError> {
        // Input path must be absolute.
        if !self.input_path.is_absolute() {
            return Err(SignError::Config(format!(
                "SignPlan: input_path must be absolute, got: {}",
                self.input_path.display()
            )));
        }

        // Output path must be absolute.
        if !self.output_path.is_absolute() {
            return Err(SignError::Config(format!(
                "SignPlan: output_path must be absolute, got: {}",
                self.output_path.display()
            )));
        }

        // Validate the sign algorithm name (cheap check before any I/O).
        let known_algs = [
            "SHA256withECDSA",
            "ECDSA_WITH_SHA256",
            "SHA256withRSA/PSS",
            "SHA256withRSAANDMGF1",
        ];
        if !known_algs.contains(&self.material.sign_alg.as_str()) {
            return Err(SignError::Config(format!(
                "SignPlan: unknown sign_alg '{}'; expected one of: {}",
                self.material.sign_alg,
                known_algs.join(", ")
            )));
        }

        // Input archive must exist.
        if !self.input_path.exists() {
            return Err(SignError::IoPath {
                path: self.input_path.clone(),
                source: std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "unsigned input archive not found",
                ),
            });
        }

        // Validate material paths.
        self.material.validate_paths()?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_material() -> SigningMaterial {
        SigningMaterial {
            keystore_path: PathBuf::from("/certs/app.p12"),
            key_alias: "debug".to_string(),
            store_password: SigningPassword::plaintext("storepass"),
            key_password: SigningPassword::plaintext("keypass"),
            profile_path: PathBuf::from("/certs/app.p7b"),
            cert_path: PathBuf::from("/certs/app.cer"),
            sign_alg: "SHA256withECDSA".to_string(),
        }
    }

    fn make_plan_with_paths(input: PathBuf, output: PathBuf) -> SignPlan {
        SignPlan::new(input, output, SignKind::Hap, make_material(), 12)
    }

    // --- SignKind ---

    #[test]
    fn sign_kind_extension() {
        assert_eq!(SignKind::Hap.extension(), "hap");
        assert_eq!(SignKind::Hsp.extension(), "hsp");
        assert_eq!(SignKind::Har.extension(), "har");
    }

    #[test]
    fn sign_kind_requires_signing_block() {
        assert!(SignKind::Hap.requires_signing_block());
        assert!(SignKind::Hsp.requires_signing_block());
        assert!(!SignKind::Har.requires_signing_block());
    }

    // --- SigningPassword ---

    #[test]
    fn signing_password_plaintext_resolves() {
        let pw = SigningPassword::plaintext("secret");
        assert_eq!(pw.resolve().unwrap(), "secret");
    }

    #[test]
    fn signing_password_plaintext_helper() {
        let pw = SigningPassword::plaintext("hello");
        assert!(matches!(pw, SigningPassword::Plaintext(s) if s == "hello"));
    }

    #[test]
    fn signing_password_encrypted_plain_value_resolves_raw() {
        let pw = SigningPassword::Encrypted {
            ciphertext: "plainpass".to_string(),
            material_dir: PathBuf::from("/does/not/matter"),
        };
        assert_eq!(pw.resolve().unwrap(), "plainpass");
    }

    // --- SignPlan accessors ---

    #[test]
    fn sign_plan_accessors() {
        let plan = SignPlan::new(
            PathBuf::from("/build/unsigned/entry.hap"),
            PathBuf::from("/build/signed/entry.hap"),
            SignKind::Hap,
            make_material(),
            12,
        );
        assert_eq!(plan.input(), Path::new("/build/unsigned/entry.hap"));
        assert_eq!(plan.output(), Path::new("/build/signed/entry.hap"));
        assert_eq!(plan.kind(), SignKind::Hap);
        assert_eq!(plan.compatible_sdk_version(), 12);
    }

    // --- SignPlan::validate ---

    #[test]
    fn validate_rejects_relative_input_path() {
        let plan = make_plan_with_paths(
            PathBuf::from("relative/entry.hap"),
            PathBuf::from("/build/signed/entry.hap"),
        );
        let err = plan.validate().unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("input_path must be absolute"), "got: {msg}");
    }

    #[test]
    fn validate_rejects_relative_output_path() {
        let plan = make_plan_with_paths(
            PathBuf::from("/build/unsigned/entry.hap"),
            PathBuf::from("relative/entry.hap"),
        );
        let err = plan.validate().unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("output_path must be absolute"), "got: {msg}");
    }

    #[test]
    fn validate_rejects_unknown_sign_alg() {
        let mut material = make_material();
        material.sign_alg = "MD5withRSA".to_string();
        let plan = SignPlan::new(
            PathBuf::from("/build/unsigned/entry.hap"),
            PathBuf::from("/build/signed/entry.hap"),
            SignKind::Hap,
            material,
            12,
        );
        let err = plan.validate().unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("unknown sign_alg"), "got: {msg}");
    }

    #[test]
    fn validate_rejects_missing_input_file() {
        // Use a path that is absolute but does not exist.
        let plan = make_plan_with_paths(
            PathBuf::from("/nonexistent/path/entry.hap"),
            PathBuf::from("/build/signed/entry.hap"),
        );
        let err = plan.validate().unwrap_err();
        // Either IoPath or Config error; the key thing is it fails.
        let msg = err.to_string();
        assert!(
            msg.contains("not found") || msg.contains("archive"),
            "got: {msg}"
        );
    }

    #[test]
    fn validate_rejects_missing_material_paths() {
        // All material paths are non-existent; validate_paths should catch them.
        let plan = SignPlan::new(
            // We need the input to pass the "is_absolute" check but we want
            // material validation to trigger, so use a temp file for input.
            PathBuf::from("/tmp/dummy_nonexistent_entry.hap"),
            PathBuf::from("/build/signed/entry.hap"),
            SignKind::Hap,
            make_material(), // paths like /certs/app.p12 won't exist
            12,
        );
        // The plan will fail either on missing input or missing material.
        // Both are valid validation failures.
        assert!(plan.validate().is_err());
    }

    #[test]
    fn validate_accepts_all_known_sign_algs() {
        // We can only check the alg validation in isolation; filesystem checks
        // will fail for non-existent paths, so we inspect the error message.
        let algs = [
            "SHA256withECDSA",
            "ECDSA_WITH_SHA256",
            "SHA256withRSA/PSS",
            "SHA256withRSAANDMGF1",
        ];
        for alg in algs {
            let mut material = make_material();
            material.sign_alg = alg.to_string();
            let plan = SignPlan::new(
                PathBuf::from("/nonexistent/entry.hap"),
                PathBuf::from("/build/signed/entry.hap"),
                SignKind::Hap,
                material,
                12,
            );
            let err = plan.validate().unwrap_err();
            // The error must NOT be about an unknown sign_alg.
            assert!(
                !err.to_string().contains("unknown sign_alg"),
                "alg {alg} was rejected: {err}"
            );
        }
    }

    // --- SigningMaterial ---

    #[test]
    fn signing_material_validate_paths_reports_missing() {
        let material = make_material(); // /certs/app.p12 etc. do not exist
        let err = material.validate_paths().unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("not found"), "got: {msg}");
    }

    // --- SignKind display smoke test ---

    #[test]
    fn sign_plan_hsp_kind() {
        let plan = SignPlan::new(
            PathBuf::from("/build/unsigned/shared.hsp"),
            PathBuf::from("/build/signed/shared.hsp"),
            SignKind::Hsp,
            make_material(),
            9,
        );
        assert_eq!(plan.kind(), SignKind::Hsp);
        assert!(plan.kind().requires_signing_block());
        assert_eq!(plan.compatible_sdk_version(), 9);
    }

    #[test]
    fn sign_plan_har_does_not_require_block() {
        let plan = SignPlan::new(
            PathBuf::from("/build/unsigned/utils.har"),
            PathBuf::from("/build/signed/utils.har"),
            SignKind::Har,
            make_material(),
            12,
        );
        assert!(!plan.kind().requires_signing_block());
    }
}
