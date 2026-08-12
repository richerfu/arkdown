use std::path::Path;

use arkdown_core::SigningConfig;
use hapsigner::HapSigner;

use crate::config_material::SigningConfigMaterial;
use crate::error::SignError;
use crate::plan::SignPlan;

/// Sequential signing-stage runner backed by the published hapsigner crate.
#[derive(Debug, Default, Clone, Copy)]
pub struct SignBackendRunner;

impl SignBackendRunner {
    pub fn new() -> Self {
        Self
    }

    /// Execute an already resolved local-signing plan.
    pub fn run(&self, plan: &SignPlan) -> Result<(), SignError> {
        plan.validate()?;
        let material = plan.material().load()?;
        HapSigner::new(material, plan.options()).sign_file(plan.input(), plan.output())?;
        Ok(())
    }

    /// Sign using the selected build-profile entry, or preserve the unsigned
    /// archive when signing is not configured.
    pub fn sign_or_skip(
        &self,
        input: &Path,
        output: &Path,
        config: Option<&SigningConfig>,
    ) -> Result<(), SignError> {
        let Some(config) = config else {
            if input != output {
                if let Some(parent) = output.parent() {
                    fs_err::create_dir_all(parent)?;
                }
                fs_err::copy(input, output)?;
            }
            tracing::warn!(output = %output.display(), "no signing config; output is unsigned");
            return Ok(());
        };

        let plan = SigningConfigMaterial::new(config).plan(input, output)?;
        self.run(&plan)
    }
}
