//! hvigor `packing-check.js` / `packing-check-har.js` validation.
//!
//! Mirrors `PackingCheck.doTaskAction()`: `.so` extension-ability srcEntry
//! files must exist for every configured ABI after native stripping, and the
//! packaged outputs must not contain stray source code files
//! (`packOptions.enableSourceCodeCheck`, default on).

use std::path::{Path, PathBuf};

use arkdown_core::{ArkdownConfig, HapManifest, ModuleConfig};

use crate::PackError;

/// Runs the packing-check validation for one module before archive assembly.
pub struct PackingCheck<'a> {
    config: &'a ArkdownConfig,
    module: &'a ModuleConfig,
    manifest: &'a HapManifest,
    compiled_assets_root: PathBuf,
    stripped_libs_root: PathBuf,
    packaged_resources_root: PathBuf,
}

impl<'a> PackingCheck<'a> {
    pub fn new(
        config: &'a ArkdownConfig,
        module: &'a ModuleConfig,
        manifest: &'a HapManifest,
        compiled_assets_root: PathBuf,
        stripped_libs_root: PathBuf,
        packaged_resources_root: PathBuf,
    ) -> Self {
        Self {
            config,
            module,
            manifest,
            compiled_assets_root,
            stripped_libs_root,
            packaged_resources_root,
        }
    }

    pub fn validate(&self) -> Result<(), PackError> {
        self.check_so_files_exist()?;
        self.check_packaged_source_files();
        Ok(())
    }

    fn abi_filters(&self) -> Vec<String> {
        self.config
            .product
            .build_option
            .external_native_options
            .as_ref()
            .and_then(|options| {
                (!options.abi_filters.is_empty()).then(|| options.abi_filters.clone())
            })
            .unwrap_or_else(|| vec!["arm64-v8a".to_string()])
    }

    /// `checkSoFilesExist`: every `.so` srcEntry in extensionAbilities must be
    /// present under `stripped_native_libs/<abi>/`.
    fn check_so_files_exist(&self) -> Result<(), PackError> {
        let module_json_path = self.module.src_path.join("src/main/module.json5");
        let abis = self.abi_filters();
        for extension in &self.manifest.extension_abilities {
            let src_entry = extension.src_entry.as_str();
            if !src_entry.ends_with(".so") {
                continue;
            }
            for abi in &abis {
                let so_path = self.stripped_libs_root.join(abi).join(src_entry);
                if !so_path.is_file() {
                    return Err(PackError::ModuleSourceEntryNotFound {
                        source_entry: src_entry.to_string(),
                        abi: abi.clone(),
                        module_json: module_json_path.display().to_string(),
                    });
                }
            }
        }
        Ok(())
    }

    /// `PackingCheck.doTaskAction` source-code scan: warn when packaged
    /// outputs contain source files (gated by
    /// `packOptions.enableSourceCodeCheck`).
    fn check_packaged_source_files(&self) {
        let enabled = self.config.product.build_option.enable_source_code_check;
        if !enabled {
            return;
        }
        let source_suffixes = [
            ".c", ".h", ".cpp", ".cs", ".java", ".rs", ".py", ".go", ".ets", ".ts",
        ];
        let mut offending = Vec::new();
        // Compiled ETS output under loader_out.
        self.collect_source_files(&self.compiled_assets_root, &source_suffixes, &mut offending);
        // Packaged resource roots (restool build resources).
        self.collect_source_files(
            &self.packaged_resources_root,
            &source_suffixes,
            &mut offending,
        );
        if !offending.is_empty() {
            offending.sort();
            offending.dedup();
            tracing::warn!(
                "Unexpected source code files packaged in '{}': {}. Set buildOption/packOptions/enableSourceCodeCheck to false to disable this check.",
                self.module.name,
                offending.join("\n")
      );
        }
    }

    fn collect_source_files(&self, dir: &Path, suffixes: &[&str], out: &mut Vec<String>) {
        if !dir.is_dir() {
            return;
        }
        let mut directories = std::collections::VecDeque::from([dir.to_path_buf()]);
        while let Some(current) = directories.pop_front() {
            let Ok(entries) = std::fs::read_dir(&current) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    directories.push_back(path);
                } else if path.is_file() {
                    if let Some(ext) = path.extension().and_then(|extension| extension.to_str()) {
                        let dotted = format!(".{ext}");
                        if suffixes.iter().any(|suffix| *suffix == dotted) {
                            out.push(path.display().to_string());
                        }
                    }
                }
            }
        }
    }
}
