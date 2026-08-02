//! Latest-hvigor schema validation for build-script runtime values.
//!
//! DevEco Studio 6.1.1 / `@ohos/hvigor-ohos-plugin` 6.24.3 rejects
//! declarative `arkOptions.compilePluginFile` fields. Compile plugins must be
//! registered from a module plugin through `OhosModuleContext::loadCompilePlugin`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const HVIGOR_PARITY_VERSION: &str = "6.24.3";

#[derive(Debug, Clone, Serialize, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[error("{message}")]
pub struct BuildScriptSchemaDiagnostic {
    pub code: String,
    pub message: String,
    pub source_file: PathBuf,
    pub field_path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildScriptBuildOptionValidationRequest {
    #[serde(default)]
    pub build_option: serde_json::Value,
    pub source_file: PathBuf,
    pub field_path: String,
}

/// Owns the build-profile schema paths observed in the latest installed
/// hvigor. Callers only provide already-parsed lossless JSON values.
pub struct BuildScriptSchemaValidator<'a> {
    source_file: &'a Path,
}

impl<'a> BuildScriptSchemaValidator<'a> {
    pub fn new(source_file: &'a Path) -> Self {
        Self { source_file }
    }

    pub fn validate_project_profile(
        &self,
        profile: &serde_json::Value,
    ) -> Result<(), BuildScriptSchemaDiagnostic> {
        for (index, product) in Self::array_at(profile, "/app/products").iter().enumerate() {
            self.validate_build_option(
                product.get("buildOption"),
                &format!("app.products[{index}].buildOption"),
            )?;
        }
        for (index, build_mode) in Self::array_at(profile, "/app/buildModeSet")
            .iter()
            .enumerate()
        {
            self.validate_build_option(
                build_mode.get("buildOption"),
                &format!("app.buildModeSet[{index}].buildOption"),
            )?;
        }
        Ok(())
    }

    pub fn validate_module_profile(
        &self,
        profile: &serde_json::Value,
    ) -> Result<(), BuildScriptSchemaDiagnostic> {
        self.validate_build_option(profile.get("buildOption"), "buildOption")?;
        for (index, build_option) in Self::array_at(profile, "/buildOptionSet")
            .iter()
            .enumerate()
        {
            self.validate_build_option(Some(build_option), &format!("buildOptionSet[{index}]"))?;
        }
        for (index, target) in Self::array_at(profile, "/targets").iter().enumerate() {
            self.validate_build_option(
                target.pointer("/config/buildOption"),
                &format!("targets[{index}].config.buildOption"),
            )?;
            self.validate_build_option(
                target.get("buildOption"),
                &format!("targets[{index}].buildOption"),
            )?;
        }
        Ok(())
    }

    pub fn validate_request(
        request: &'a BuildScriptBuildOptionValidationRequest,
    ) -> Result<(), BuildScriptSchemaDiagnostic> {
        Self::new(&request.source_file)
            .validate_build_option(Some(&request.build_option), &request.field_path)
    }

    fn validate_build_option(
        &self,
        build_option: Option<&serde_json::Value>,
        field_path: &str,
    ) -> Result<(), BuildScriptSchemaDiagnostic> {
        let Some(ark_options) = build_option
            .and_then(serde_json::Value::as_object)
            .and_then(|value| value.get("arkOptions"))
            .and_then(serde_json::Value::as_object)
        else {
            return Ok(());
        };
        if !ark_options.contains_key("compilePluginFile") {
            return Ok(());
        }

        let field_path = format!("{field_path}.arkOptions.compilePluginFile");
        Err(BuildScriptSchemaDiagnostic {
            code: "HVIGOR_SCHEMA_COMPILE_PLUGIN_FILE".to_string(),
            message: format!(
                "{field_path} is not accepted by the latest @ohos/hvigor-ohos-plugin \
                 {HVIGOR_PARITY_VERSION} schema in {}; register the .ts plugin from a module \
                 HvigorPlugin with OhosModuleContext.loadCompilePlugin(path).",
                self.source_file.display()
            ),
            source_file: self.source_file.to_path_buf(),
            field_path,
        })
    }

    fn array_at<'b>(value: &'b serde_json::Value, pointer: &str) -> &'b [serde_json::Value] {
        value
            .pointer(pointer)
            .and_then(serde_json::Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }
}
