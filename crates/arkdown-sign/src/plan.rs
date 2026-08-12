//! Typed inputs for Arkdown's local application-signing stage.
//!
//! Hvigor's `CommonSignCommandBuilder` resolves these values before invoking
//! `hap-sign-tool`. Arkdown follows the same boundary: project discovery stays
//! outside this crate, DevEco passwords are resolved here, and the loaded
//! material is handed to hapsigner without reimplementing signing formats.

use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use hapsigner::{
    FileSigningMaterial, SignOptions, SigningAlgorithm, SigningMaterial as HapsignerSigningMaterial,
};

use crate::error::SignError;

/// Password handling selected by Arkdown's build-profile adapter.
#[derive(Clone)]
pub enum SigningPassword {
    Plaintext(String),
    Encrypted {
        ciphertext: String,
        material_dir: PathBuf,
    },
}

impl fmt::Debug for SigningPassword {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Plaintext(_) => formatter.write_str("Plaintext([REDACTED])"),
            Self::Encrypted { material_dir, .. } => formatter
                .debug_struct("Encrypted")
                .field("ciphertext", &"[REDACTED]")
                .field("material_dir", material_dir)
                .finish(),
        }
    }
}

impl SigningPassword {
    pub fn plaintext(password: impl Into<String>) -> Self {
        Self::Plaintext(password.into())
    }

    pub fn encrypted(ciphertext: impl Into<String>, material_dir: PathBuf) -> Self {
        Self::Encrypted {
            ciphertext: ciphertext.into(),
            material_dir,
        }
    }

    pub fn resolve(&self) -> Result<String, SignError> {
        match self {
            Self::Plaintext(password) => Ok(password.clone()),
            Self::Encrypted {
                ciphertext,
                material_dir,
            } => DevEcoPasswordResolver::new(ciphertext, material_dir).resolve(),
        }
    }
}

/// Filesystem-backed local signing material resolved from Hvigor-compatible
/// signing configuration.
#[derive(Debug, Clone)]
pub struct SigningMaterial {
    pub keystore_path: PathBuf,
    pub key_alias: String,
    pub store_password: SigningPassword,
    pub key_password: SigningPassword,
    pub profile_path: PathBuf,
    pub cert_path: PathBuf,
    pub sign_alg: String,
    pub profile_signed: bool,
}

impl SigningMaterial {
    pub fn load(&self) -> Result<HapsignerSigningMaterial, SignError> {
        let algorithm = SigningAlgorithm::from_str(&self.sign_alg)?;
        self.validate_paths()?;
        let mut material = FileSigningMaterial {
            keystore_path: self.keystore_path.clone(),
            profile_path: self.profile_path.clone(),
            certificate_path: self.cert_path.clone(),
            key_alias: self.key_alias.clone().into_boxed_str(),
            store_password: self.store_password.resolve()?.into_boxed_str(),
            key_password: self.key_password.resolve()?.into_boxed_str(),
            profile_signed: self.profile_signed,
            algorithm,
        };
        match material.load() {
            Ok(loaded) => Ok(loaded),
            Err(error) => {
                let replacement = match &error {
                    hapsigner::SignError::KeyAliasNotFound { available, .. } => {
                        KeystoreAliasSelector::new(&self.key_alias).select(available)
                    }
                    _ => None,
                };
                let Some(replacement) = replacement else {
                    return Err(error.into());
                };
                // Hvigor forwards `keyAlias` unchanged to the Java signing
                // tool. Java's JKS/PKCS#12 KeyStore providers resolve aliases
                // case-insensitively; published hapsigner 0.2 exposes the
                // canonical stored alias and currently compares it exactly.
                material.key_alias = replacement.into_boxed_str();
                Ok(material.load()?)
            }
        }
    }

    fn validate_paths(&self) -> Result<(), SignError> {
        for (label, path) in [
            ("keystore", &self.keystore_path),
            ("profile", &self.profile_path),
            ("certificate", &self.cert_path),
        ] {
            if !path.is_file() {
                return Err(SignError::Config(format!(
                    "signing {label} is not a file: {}",
                    path.display()
                )));
            }
        }
        Ok(())
    }
}

struct KeystoreAliasSelector<'a> {
    requested: &'a str,
}

impl<'a> KeystoreAliasSelector<'a> {
    fn new(requested: &'a str) -> Self {
        Self { requested }
    }

    fn select(&self, available: &[String]) -> Option<String> {
        let requested = self.requested.to_lowercase();
        let mut matching = available
            .iter()
            .filter(|alias| alias.to_lowercase() == requested);
        let selected = matching.next()?.clone();
        matching.next().is_none().then_some(selected)
    }
}

/// Complete, pre-resolved application-signing operation.
#[derive(Debug, Clone)]
pub struct SignPlan {
    input_path: PathBuf,
    output_path: PathBuf,
    material: SigningMaterial,
    options: SignOptions,
}

impl SignPlan {
    pub fn new(
        input_path: PathBuf,
        output_path: PathBuf,
        material: SigningMaterial,
        compatible_sdk_version: u32,
    ) -> Self {
        Self {
            input_path,
            output_path,
            material,
            options: SignOptions {
                compatible_version: compatible_sdk_version,
                code_signing: true,
            },
        }
    }

    pub fn input(&self) -> &Path {
        &self.input_path
    }

    pub fn output(&self) -> &Path {
        &self.output_path
    }

    pub fn material(&self) -> &SigningMaterial {
        &self.material
    }

    pub fn options(&self) -> SignOptions {
        self.options
    }

    pub fn validate(&self) -> Result<(), SignError> {
        if !self.input_path.is_absolute() {
            return Err(SignError::Config(format!(
                "signing input path must be absolute: {}",
                self.input_path.display()
            )));
        }
        if !self.output_path.is_absolute() {
            return Err(SignError::Config(format!(
                "signing output path must be absolute: {}",
                self.output_path.display()
            )));
        }
        if !self.input_path.is_file() {
            return Err(SignError::IoPath {
                path: self.input_path.clone(),
                source: std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "unsigned application archive was not found",
                ),
            });
        }
        SigningAlgorithm::from_str(&self.material.sign_alg)?;
        self.material.validate_paths()
    }
}

struct DevEcoPasswordResolver<'a> {
    ciphertext: &'a str,
    material_dir: &'a Path,
}

impl<'a> DevEcoPasswordResolver<'a> {
    const COMPONENT: [u8; 16] = [
        49, 243, 9, 115, 214, 175, 91, 184, 211, 190, 177, 88, 101, 131, 192, 119,
    ];
    const MIN_ENCRYPTED_HEX_LEN: usize = 16;
    const ENCRYPTED_HEADER_PREFIX: &'static str = "000000";

    fn new(ciphertext: &'a str, material_dir: &'a Path) -> Self {
        Self {
            ciphertext,
            material_dir,
        }
    }

    fn resolve(&self) -> Result<String, SignError> {
        if !self.requires_material_decryption() {
            return Ok(self.ciphertext.to_owned());
        }

        let material_root = self.material_dir.join("material");
        let fd_root = material_root.join("fd");
        let fd0 = self.read_component(&fd_root.join("0"))?;
        let fd1 = self.read_component(&fd_root.join("1"))?;
        let fd2 = self.read_component(&fd_root.join("2"))?;
        let mut combined = [0_u8; 16];
        for index in 0..combined.len() {
            combined[index] = fd0[index] ^ fd1[index] ^ fd2[index] ^ Self::COMPONENT[index];
        }

        let password_material = String::from_utf8_lossy(&combined);
        let salt = self.read_single_material_file(&material_root.join("ac"))?;
        let mut root_key = [0_u8; 16];
        pbkdf2::pbkdf2_hmac::<sha2::Sha256>(
            password_material.as_bytes(),
            &salt,
            10_000,
            &mut root_key,
        );

        let encrypted_work_key = self.read_single_material_file(&material_root.join("ce"))?;
        let work_key = self.decrypt(&root_key, &encrypted_work_key)?;
        let work_key: [u8; 16] = work_key
            .try_into()
            .map_err(|_| SignError::Config("DevEco work key is not 16 bytes".into()))?;
        let encrypted_password = hex::decode(self.ciphertext)
            .map_err(|error| SignError::Config(format!("invalid encrypted password: {error}")))?;
        let plaintext = self.decrypt(&work_key, &encrypted_password)?;
        String::from_utf8(plaintext).map_err(|error| {
            SignError::Config(format!("decrypted password is not valid UTF-8: {error}"))
        })
    }

    fn requires_material_decryption(&self) -> bool {
        self.ciphertext.len() >= Self::MIN_ENCRYPTED_HEX_LEN
            && self.ciphertext.starts_with(Self::ENCRYPTED_HEADER_PREFIX)
    }

    fn read_component(&self, directory: &Path) -> Result<[u8; 16], SignError> {
        let bytes = self.read_single_material_file(directory)?;
        bytes
            .get(..16)
            .ok_or_else(|| {
                SignError::Config(format!(
                    "DevEco material component is shorter than 16 bytes: {}",
                    directory.display()
                ))
            })?
            .try_into()
            .map_err(|_| {
                SignError::Config(format!(
                    "DevEco material component is invalid: {}",
                    directory.display()
                ))
            })
    }

    fn read_single_material_file(&self, directory: &Path) -> Result<Vec<u8>, SignError> {
        let entries = std::fs::read_dir(directory).map_err(|error| {
            SignError::Config(format!(
                "cannot read DevEco material directory {}: {error}",
                directory.display()
            ))
        })?;
        let mut paths = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| {
                SignError::Config(format!(
                    "cannot inspect DevEco material directory {}: {error}",
                    directory.display()
                ))
            })?;
            if entry.file_name() != ".DS_Store" {
                paths.push(entry.path());
            }
        }
        paths.sort();
        let path = paths.first().ok_or_else(|| {
            SignError::Config(format!(
                "DevEco material directory is empty: {}",
                directory.display()
            ))
        })?;
        fs_err::read(path).map_err(|source| SignError::IoPath {
            path: path.clone(),
            source,
        })
    }

    fn decrypt(&self, key: &[u8; 16], data: &[u8]) -> Result<Vec<u8>, SignError> {
        use aes_gcm::{aead::Aead, Aes128Gcm, KeyInit, Nonce};

        if data.len() < 20 {
            return Err(SignError::Config(
                "DevEco encrypted material is too short".into(),
            ));
        }
        let encrypted_region_len =
            u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
        let payload_len = data.len() - 4;
        if encrypted_region_len > payload_len {
            return Err(SignError::Config(
                "DevEco encrypted material length is invalid".into(),
            ));
        }
        let nonce_len = payload_len - encrypted_region_len;
        if nonce_len != 12 || encrypted_region_len < 16 {
            return Err(SignError::Config(
                "DevEco encrypted material has an invalid nonce or tag".into(),
            ));
        }
        let nonce = Nonce::from_slice(&data[4..4 + nonce_len]);
        let encrypted = &data[4 + nonce_len..];
        let cipher = Aes128Gcm::new_from_slice(key)
            .map_err(|error| SignError::Config(format!("invalid DevEco AES key: {error}")))?;
        cipher.decrypt(nonce, encrypted).map_err(|_| {
            SignError::Config(
                "DevEco AES-GCM decryption failed: wrong key or corrupt material".into(),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::KeystoreAliasSelector;

    #[test]
    fn java_keystore_alias_lookup_uses_the_unique_case_insensitive_alias() {
        let aliases = vec!["debugkey".to_string(), "release".to_string()];
        assert_eq!(
            KeystoreAliasSelector::new("debugKey").select(&aliases),
            Some("debugkey".to_string())
        );
    }

    #[test]
    fn java_keystore_alias_lookup_rejects_ambiguous_case_variants() {
        let aliases = vec!["debugkey".to_string(), "DEBUGKEY".to_string()];
        assert_eq!(KeystoreAliasSelector::new("debugKey").select(&aliases), None);
    }
}
