//! Arkdown-owned project configuration.
//!
//! `arkdown.config.toml` is the primary format. The earlier
//! `arkdown.config.json` format remains a compatibility fallback when the TOML
//! file is absent. When both files exist, TOML is authoritative.
//!
//! Build-profile constants use a direct table so they can be overlaid onto
//! hvigor's effective `buildOption.arkOptions.buildProfileFields` map:
//!
//! ```toml
//! [build-profile]
//! FEATURE_FLAG = true
//! API_HOST = "https://example.com"
//! ```

use std::path::{Path, PathBuf};

use crate::ConfigError;

const TOML_CONFIG_FILE: &str = "arkdown.config.toml";
const JSON_CONFIG_FILE: &str = "arkdown.config.json";

/// Arkdown-specific configuration loaded once during build-session startup.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ArkdownCustomConfig {
    /// Override `shimMissingExports` in the Rolldown plan.
    pub shim_missing_exports: Option<bool>,
    /// Force-enable or force-disable tree shaking.
    pub treeshake: Option<bool>,
    /// Override the Rolldown global context string.
    pub context: Option<String>,
    /// Plugin configuration overrides.
    pub plugins: ArkdownPluginsConfig,
    /// Constants that override the effective project/module
    /// `buildOption.arkOptions.buildProfileFields` values.
    #[serde(
        rename = "build-profile",
        alias = "buildProfile",
        alias = "build_profile",
        alias = "buildProfileFields"
    )]
    pub build_profile_fields: serde_json::Map<String, serde_json::Value>,
    /// Active source file, excluded from deserialization and configuration
    /// semantics but retained for cache/watch inventories.
    #[serde(skip)]
    source_path: Option<PathBuf>,
}

/// Built-in plugin configuration.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ArkdownPluginsConfig {
    /// Built-in plugin names disabled for this build.
    pub disabled: Vec<String>,
}

impl ArkdownCustomConfig {
    /// The configuration file selected by the startup loader.
    pub fn source_path(&self) -> Option<&Path> {
        self.source_path.as_deref()
    }
}

/// Loads the project-root Arkdown configuration with deterministic format
/// precedence.
pub struct ArkdownCustomConfigLoader {
    project_root: PathBuf,
}

impl ArkdownCustomConfigLoader {
    pub fn new(project_root: &Path) -> Self {
        Self {
            project_root: project_root.to_path_buf(),
        }
    }

    /// Load TOML first, then fall back to the legacy JSON format.
    pub fn load(&self) -> Result<ArkdownCustomConfig, ConfigError> {
        let toml_path = self.project_root.join(TOML_CONFIG_FILE);
        if toml_path.is_file() {
            return self.load_toml(&toml_path);
        }

        let json_path = self.project_root.join(JSON_CONFIG_FILE);
        if json_path.is_file() {
            return self.load_json(&json_path);
        }

        Ok(ArkdownCustomConfig::default())
    }

    fn load_toml(&self, path: &Path) -> Result<ArkdownCustomConfig, ConfigError> {
        let content = fs_err::read_to_string(path)?;
        let mut config = toml::from_str::<ArkdownCustomConfig>(&content).map_err(|error| {
            ConfigError::TomlParseError {
                file: path.to_path_buf(),
                message: error.to_string(),
            }
        })?;
        config.source_path = Some(path.to_path_buf());
        self.validate_build_profile_fields(&config, path)?;
        Ok(config)
    }

    fn load_json(&self, path: &Path) -> Result<ArkdownCustomConfig, ConfigError> {
        let content = fs_err::read_to_string(path)?;
        let mut config =
            serde_json::from_str::<ArkdownCustomConfig>(&content).map_err(|error| {
                ConfigError::ParseError {
                    file: path.to_path_buf(),
                    message: error.to_string(),
                }
            })?;
        config.source_path = Some(path.to_path_buf());
        self.validate_build_profile_fields(&config, path)?;
        Ok(config)
    }

    fn validate_build_profile_fields(
        &self,
        config: &ArkdownCustomConfig,
        path: &Path,
    ) -> Result<(), ConfigError> {
        for (name, value) in &config.build_profile_fields {
            if value.is_string() || value.is_number() || value.is_boolean() {
                continue;
            }
            return Err(ConfigError::ValidationError {
                file: path.to_path_buf(),
                message: format!(
                    "build-profile.{name} only supports string, number, or boolean values; got {}",
                    Self::value_kind(value)
                ),
            });
        }
        Ok(())
    }

    fn value_kind(value: &serde_json::Value) -> &'static str {
        match value {
            serde_json::Value::Null => "null",
            serde_json::Value::Bool(_) => "boolean",
            serde_json::Value::Number(_) => "number",
            serde_json::Value::String(_) => "string",
            serde_json::Value::Array(_) => "array",
            serde_json::Value::Object(_) => "object",
        }
    }
}
