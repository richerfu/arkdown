//! F24 — Custom build configuration from `arkdown.config.json`.
//!
//! Since hvigorfile.ts TypeScript execution is not available in pure Rust,
//! arkdown supports a JSON-based alternative: `arkdown.config.json` at the
//! project root.  This file can configure:
//!
//! - `shimMissingExports`: override rolldown's shimMissingExports option
//! - `treeshake`: force-enable or -disable tree-shaking
//! - `context`: set the rolldown globalThis context string
//! - `plugins.disabled`: list of built-in plugin names to disable
//! - `plugins.extra`: extra config passed to specific plugins
//!
//! Mirrors the subset of options that hvigorfile.ts projects commonly use.

use std::path::Path;

/// Custom configuration loaded from `arkdown.config.json`.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ArkdownCustomConfig {
    /// Override `shimMissingExports` rolldown option.
    pub shim_missing_exports: Option<bool>,
    /// Force-enable (`true`) or force-disable (`false`) tree-shaking.
    pub treeshake: Option<bool>,
    /// Global context string (defaults to `"globalThis"`).
    pub context: Option<String>,
    /// Plugin configuration overrides.
    pub plugins: ArkdownPluginsConfig,
}

/// Plugin configuration section in `arkdown.config.json`.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ArkdownPluginsConfig {
    /// List of built-in plugin names to disable for this build.
    pub disabled: Vec<String>,
}

/// Load `arkdown.config.json` from the project root.
///
/// Returns `None` silently when the file does not exist.
/// Logs a warning if the file exists but cannot be parsed.
pub fn load_custom_config(project_root: &Path) -> ArkdownCustomConfig {
    let config_path = project_root.join("arkdown.config.json");
    if !config_path.exists() {
        return ArkdownCustomConfig::default();
    }
    let Ok(content) = std::fs::read_to_string(&config_path) else {
        tracing::warn!(
            path = %config_path.display(),
            "F24: failed to read arkdown.config.json"
        );
        return ArkdownCustomConfig::default();
    };
    match serde_json::from_str::<ArkdownCustomConfig>(&content) {
        Ok(cfg) => {
            tracing::info!(
                path = %config_path.display(),
                "F24: loaded arkdown.config.json custom build configuration"
            );
            cfg
        }
        Err(e) => {
            tracing::warn!(
                path = %config_path.display(),
                error = %e,
                "F24: failed to parse arkdown.config.json; using defaults"
            );
            ArkdownCustomConfig::default()
        }
    }
}
