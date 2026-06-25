use std::path::Path;

use arkdown_core::SigningConfig;

use crate::error::SignError;
use crate::{HapSigner, DEFAULT_COMPATIBLE_VERSION};

pub trait SignBackend: Send + Sync {
    fn sign_or_skip(
        &self,
        input: &Path,
        output: &Path,
        config: Option<&SigningConfig>,
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
    ) -> Result<(), SignError> {
        HapSigner::sign_or_skip(input, output, config, DEFAULT_COMPATIBLE_VERSION)
    }
}

pub struct SignBackendRunner {
    backend: Box<dyn SignBackend>,
}

impl SignBackendRunner {
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
    ) -> Result<(), SignError> {
        self.backend.sign_or_skip(input, output, config)
    }
}
