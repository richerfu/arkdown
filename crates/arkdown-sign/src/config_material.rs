use std::path::{Path, PathBuf};

use arkdown_core::SigningConfig;

use crate::error::SignError;
use crate::plan::SigningPassword;

pub(crate) struct SigningConfigMaterial<'a> {
    config: &'a SigningConfig,
}

impl<'a> SigningConfigMaterial<'a> {
    pub(crate) fn new(config: &'a SigningConfig) -> Self {
        Self { config }
    }

    pub(crate) fn cert_path(&self) -> &Path {
        &self.config.cert_path
    }

    pub(crate) fn key_alias(&self) -> &str {
        &self.config.key_alias
    }

    pub(crate) fn key_password(&self) -> Result<String, SignError> {
        self.resolve_password("ARKDOWN_KEY_PASSWORD", &self.config.key_password)
    }

    pub(crate) fn profile_path(&self) -> &Path {
        &self.config.profile_path
    }

    pub(crate) fn sign_alg(&self) -> &str {
        &self.config.sign_alg
    }

    pub(crate) fn store_file(&self) -> &Path {
        &self.config.store_file
    }

    pub(crate) fn store_password(&self) -> Result<String, SignError> {
        self.resolve_password("ARKDOWN_STORE_PASSWORD", &self.config.store_password)
    }

    fn resolve_password(&self, env_var: &str, ciphertext: &str) -> Result<String, SignError> {
        match std::env::var(env_var) {
            Ok(password) => Ok(password),
            Err(std::env::VarError::NotPresent) => SigningPassword::Encrypted {
                ciphertext: ciphertext.to_owned(),
                material_dir: self.material_dir(),
            }
            .resolve(),
            Err(std::env::VarError::NotUnicode(_)) => {
                Err(SignError::Config(format!("{env_var} must be valid UTF-8")))
            }
        }
    }

    fn material_dir(&self) -> PathBuf {
        self.config
            .store_file
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf()
    }
}
