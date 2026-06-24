use std::path::{Path, PathBuf};
use std::process::Command;

use arkdown_core::SigningConfig;

use crate::config_material::SigningConfigMaterial;
use crate::error::SignError;
use crate::HapSigner;

pub trait SignBackend: Send + Sync {
    fn sign_or_skip(
        &self,
        input: &Path,
        output: &Path,
        config: Option<&SigningConfig>,
        compatible_sdk_version: u32,
    ) -> Result<(), SignError>;
}

#[derive(Debug, Clone)]
pub struct RustSignBackend;

impl SignBackend for RustSignBackend {
    fn sign_or_skip(
        &self,
        input: &Path,
        output: &Path,
        config: Option<&SigningConfig>,
        compatible_sdk_version: u32,
    ) -> Result<(), SignError> {
        HapSigner::sign_or_skip(input, output, config, compatible_sdk_version)
    }
}

#[derive(Debug, Clone)]
pub struct SdkSignBackend {
    hap_sign_tool: PathBuf,
}

impl SdkSignBackend {
    pub fn new(hap_sign_tool: PathBuf) -> Self {
        Self { hap_sign_tool }
    }
}

impl SignBackend for SdkSignBackend {
    fn sign_or_skip(
        &self,
        input: &Path,
        output: &Path,
        config: Option<&SigningConfig>,
        _compatible_sdk_version: u32,
    ) -> Result<(), SignError> {
        let Some(config) = config else {
            if input != output {
                std::fs::copy(input, output)?;
            }
            tracing::warn!(output = %output.display(), "no signing config; output is unsigned");
            return Ok(());
        };

        let material = SigningConfigMaterial::new(config);
        let store_password = material.store_password()?;
        let key_password = material.key_password()?;

        let mut command = Command::new("java");
        command
            .arg("-jar")
            .arg(&self.hap_sign_tool)
            .arg("sign-app")
            .arg("-mode")
            .arg("localSign")
            .arg("-keystoreFile")
            .arg(material.store_file())
            .arg("-keystorePwd")
            .arg(&store_password)
            .arg("-keyAlias")
            .arg(material.key_alias())
            .arg("-keyPwd")
            .arg(&key_password)
            .arg("-signAlg")
            .arg(material.sign_alg())
            .arg("-profileFile")
            .arg(material.profile_path())
            .arg("-appCertFile")
            .arg(material.cert_path())
            .arg("-inFile")
            .arg(input)
            .arg("-outFile")
            .arg(output);

        let status = command.status()?;
        if !status.success() {
            return Err(SignError::SigningFailed(format!(
                "SDK hap-sign-tool failed with status {status}"
            )));
        }
        Ok(())
    }
}

pub struct SignBackendRunner {
    backend: Box<dyn SignBackend>,
}

impl SignBackendRunner {
    pub fn default_sdk(hap_sign_tool: PathBuf) -> Self {
        Self {
            backend: Box::new(SdkSignBackend::new(hap_sign_tool)),
        }
    }

    pub fn rust() -> Self {
        Self {
            backend: Box::new(RustSignBackend),
        }
    }

    pub fn sign_or_skip(
        &self,
        input: &Path,
        output: &Path,
        config: Option<&SigningConfig>,
        compatible_sdk_version: u32,
    ) -> Result<(), SignError> {
        self.backend
            .sign_or_skip(input, output, config, compatible_sdk_version)
    }
}
