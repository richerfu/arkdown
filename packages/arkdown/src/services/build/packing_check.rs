//! hvigor `packing-check.js` / `packing-check-har.js` validation.
//!
//! Mirrors `PackingCheck.doTaskAction()`: `.so` extension-ability srcEntry
//! files must exist for every configured ABI after native stripping, and the
//! packaged outputs must not contain stray source code files
//! (`packOptions.enableSourceCodeCheck`, default on).

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

use arkdown_core::{ArkdownConfig, ModuleConfig};
use arkdown_profile::LoaderJson;

/// Runs the packing-check validation for one module before archive assembly.
pub(super) struct PackingCheck<'a> {
  config: &'a ArkdownConfig,
  module: &'a ModuleConfig,
  loader_json: &'a LoaderJson,
  stripped_libs_root: PathBuf,
  packaged_resources_root: PathBuf,
}

impl<'a> PackingCheck<'a> {
  pub(super) fn new(
    config: &'a ArkdownConfig,
    module: &'a ModuleConfig,
    loader_json: &'a LoaderJson,
    stripped_libs_root: PathBuf,
    packaged_resources_root: PathBuf,
  ) -> Self {
    Self {
      config,
      module,
      loader_json,
      stripped_libs_root,
      packaged_resources_root,
    }
  }

  pub(super) fn validate(&self) -> Result<()> {
    self.check_so_files_exist()?;
    self.check_packaged_source_files();
    Ok(())
  }

  fn abi_filters(&self) -> Vec<String> {
    self
      .config
      .product
      .build_option
      .external_native_options
      .as_ref()
      .and_then(|options| (!options.abi_filters.is_empty()).then(|| options.abi_filters.clone()))
      .unwrap_or_else(|| vec!["arm64-v8a".to_string()])
  }

  /// `checkSoFilesExist`: every `.so` srcEntry in extensionAbilities must be
  /// present under `stripped_native_libs/<abi>/`.
  fn check_so_files_exist(&self) -> Result<()> {
    let module_json_path = self.module.src_path.join("src/main/module.json5");
    let Ok(content) = std::fs::read_to_string(&module_json_path) else {
      return Ok(());
    };
    let Ok(module_json) = json5::from_str::<serde_json::Value>(&content) else {
      return Ok(());
    };
    let Some(extensions) = module_json
      .get("module")
      .and_then(|m| m.get("extensionAbilities"))
      .and_then(serde_json::Value::as_array)
    else {
      return Ok(());
    };
    let abis = self.abi_filters();
    for extension in extensions {
      let Some(src_entry) = extension
        .get("srcEntry")
        .and_then(serde_json::Value::as_str)
      else {
        continue;
      };
      if !src_entry.ends_with(".so") {
        continue;
      }
      for abi in &abis {
        let so_path = self.stripped_libs_root.join(abi).join(src_entry);
        if !so_path.is_file() {
          return Err(anyhow!(
                        "MODULE_SRC_ENTRY_NOT_FOUND: '{}' is not present under stripped_native_libs/{abi} (referenced by {})",
                        src_entry,
                        module_json_path.display()
                    ));
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
    let ets_assets = PathBuf::from(&self.loader_json.ace_module_build);
    collect_source_files(&ets_assets, &source_suffixes, &mut offending);
    // Packaged resource roots (restool build resources).
    collect_source_files(
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
}

fn collect_source_files(dir: &Path, suffixes: &[&str], out: &mut Vec<String>) {
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
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
          let dotted = format!(".{ext}");
          if suffixes.iter().any(|suffix| *suffix == dotted) {
            out.push(path.display().to_string());
          }
        }
      }
    }
  }
}
