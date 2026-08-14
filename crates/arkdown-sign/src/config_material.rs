use std::path::{Path, PathBuf};

use arkdown_core::SigningConfig;
use hapsigner::DEFAULT_COMPATIBLE_VERSION;

use crate::error::SignError;
use crate::plan::{SignPlan, SigningMaterial, SigningPassword};

/// Maps Arkdown's resolved build-profile entry to the hapsigner input plan.
pub(crate) struct SigningConfigMaterial<'a> {
    config: &'a SigningConfig,
}

impl<'a> SigningConfigMaterial<'a> {
    pub(crate) fn new(config: &'a SigningConfig) -> Self {
        Self { config }
    }

    pub(crate) fn plan(&self, input: &Path, output: &Path) -> Result<SignPlan, SignError> {
        Ok(SignPlan::new(
            input.to_path_buf(),
            output.to_path_buf(),
            self.material()?,
            DEFAULT_COMPATIBLE_VERSION,
        ))
    }

    pub(crate) fn material(&self) -> Result<SigningMaterial, SignError> {
        let material_dir = self.material_dir();
        Ok(SigningMaterial {
            keystore_path: self.config.store_file.clone(),
            key_alias: self.config.key_alias.clone(),
            store_password: self.password(
                "ARKDOWN_STORE_PASSWORD",
                &self.config.store_password,
                &material_dir,
            )?,
            key_password: self.password(
                "ARKDOWN_KEY_PASSWORD",
                &self.config.key_password,
                &material_dir,
            )?,
            profile_path: self.config.profile_path.clone(),
            cert_path: self.config.cert_path.clone(),
            sign_alg: self.config.sign_alg.clone(),
            profile_signed: true,
        })
    }

    fn password(
        &self,
        env_var: &str,
        configured: &str,
        material_dir: &Path,
    ) -> Result<SigningPassword, SignError> {
        match std::env::var(env_var) {
            Ok(password) => Ok(SigningPassword::plaintext(password)),
            Err(std::env::VarError::NotPresent) => Ok(SigningPassword::encrypted(
                configured,
                material_dir.to_path_buf(),
            )),
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
