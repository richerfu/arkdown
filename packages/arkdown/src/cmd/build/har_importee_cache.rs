//! Hvigor-compatible ArkTS importee cache handoff for HAR artifact processing.

use std::path::PathBuf;

use anyhow::{Context, Result};
use arkdown::HvigorHarImportees;

const NON_ENTRY_FILE: &str = "non.entry.importee.json";
const COMPILED_FILE: &str = "compiled.importees.json";
const DIRECT_FILE: &str = "direct.importees.json";

/// Owns the three JSON arrays exchanged between latest hvigor's
/// `process-importee.js` build step and `ProcessHarArtifacts`.
pub(super) struct HarImporteeCache {
  cache_dir: PathBuf,
}

impl HarImporteeCache {
  pub(super) fn new(cache_dir: impl Into<PathBuf>) -> Self {
    Self {
      cache_dir: cache_dir.into(),
    }
  }

  pub(super) fn write_compile_output(
    &self,
    importees: &HvigorHarImportees,
    is_bytecode_har: bool,
    bundled_dependencies: bool,
    collect_direct_importees: bool,
  ) -> Result<()> {
    // `processImportees` returns before touching the filesystem unless the
    // module is a ByteCode HAR or integrated optimization collects direct
    // importees. Preserve stale-file behavior on that inactive path.
    if !is_bytecode_har && !collect_direct_importees {
      return Ok(());
    }
    std::fs::create_dir_all(&self.cache_dir).with_context(|| {
      format!(
        "create HAR importee cache directory {}",
        self.cache_dir.display()
      )
    })?;
    if is_bytecode_har {
      self.write_array(NON_ENTRY_FILE, &importees.non_entry)?;
    }
    if bundled_dependencies {
      self.write_array(COMPILED_FILE, &importees.compiled)?;
    }
    if collect_direct_importees {
      self.write_array(DIRECT_FILE, &importees.direct)?;
    }
    Ok(())
  }

  pub(super) fn read(&self) -> Result<HvigorHarImportees> {
    Ok(HvigorHarImportees {
      non_entry: self.read_array(NON_ENTRY_FILE)?,
      compiled: self.read_array(COMPILED_FILE)?,
      direct: self.read_array(DIRECT_FILE)?,
    })
  }

  fn write_array(&self, file_name: &str, values: &[String]) -> Result<()> {
    let path = self.cache_dir.join(file_name);
    let mut bytes = serde_json::to_vec(values)
      .with_context(|| format!("serialize HAR importee cache {}", path.display()))?;
    // fs-extra.writeJson appends the platform EOL; hvigor's observed cache on
    // macOS is compact JSON followed by one newline.
    bytes.push(b'\n');
    std::fs::write(&path, bytes)
      .with_context(|| format!("write HAR importee cache {}", path.display()))
  }

  fn read_array(&self, file_name: &str) -> Result<Vec<String>> {
    let path = self.cache_dir.join(file_name);
    if !path.is_file() {
      return Ok(Vec::new());
    }
    let bytes = std::fs::read(&path)
      .with_context(|| format!("read HAR importee cache {}", path.display()))?;
    serde_json::from_slice(&bytes)
      .with_context(|| format!("parse HAR importee cache {}", path.display()))
  }
}
