//! Rust-native OHPM install stage executed before [`arkdown_session::BuildSession`]
//! freezes the project filesystem.
//!
//! Upstream evidence: `docs/kb/06-hvigor调用链分析.md` records the wrapper's
//! `ohpm install` before hvigor builds its project/dependency model. Arkdown
//! invokes the `ohpm-core` library at the same boundary and then lets the
//! session resolve the materialized `oh_modules` graph.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use arkdown_core::ArkdownConfig;
use ohpm_core::config::Config;
use ohpm_core::install::{valid_cli_options, InstallOptions};
use ohpm_core::registry::RegistryClient;

pub(crate) struct OhpmInstallStage<'a> {
    config: &'a ArkdownConfig,
    parameter_file: Option<PathBuf>,
}

impl<'a> OhpmInstallStage<'a> {
    pub(crate) fn new(config: &'a ArkdownConfig, parameter_file: Option<&Path>) -> Self {
        Self {
            config,
            parameter_file: parameter_file.map(Path::to_path_buf),
        }
    }

    pub(crate) async fn run(&self) -> Result<()> {
        if !self.is_ohpm_project() {
            return Ok(());
        }

        let project_root = &self.config.project_root;
        let mut ohpm_config = Config::new();
        ohpm_config.load(project_root, None).with_context(|| {
            format!(
                "ARKDOWN_OHPM_CONFIG_FAILED: load OHPM configuration for {}",
                project_root.display()
            )
        })?;
        let options = InstallOptions {
            prefix: Some(project_root.clone()),
            parameter_file: self.parameter_file.clone(),
            ..InstallOptions::default()
        };
        valid_cli_options("install", &options)
            .context("ARKDOWN_OHPM_OPTIONS_INVALID: validate ohpm install options")?;
        let client = RegistryClient::from_config(&ohpm_config)
            .context("ARKDOWN_OHPM_CLIENT_FAILED: initialize OHPM registry client")?;
        let outcome =
            ohpm_core::install::install(&client, &ohpm_config, project_root, &[], &options)
                .await
                .with_context(|| {
                    format!(
                        "ARKDOWN_OHPM_INSTALL_FAILED: install dependencies for {}",
                        project_root.display()
                    )
                })?;
        tracing::info!(
            modules = outcome.module_roots.len(),
            installed = outcome.installed,
            "OHPM dependency graph materialized"
        );
        Ok(())
    }

    fn is_ohpm_project(&self) -> bool {
        self.config.sdk_config.compile_api_version >= 9
            && (self.config.project_root.join("oh-package.json5").is_file()
                || !self.config.project_root.join("package.json").is_file())
    }
}
