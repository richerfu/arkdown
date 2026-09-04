//! Hvigor-compatible ArkTS importee cache handoff for HAR artifact processing.

use std::path::PathBuf;

use thiserror::Error;

const NON_ENTRY_FILE: &str = "non.entry.importee.json";
const COMPILED_FILE: &str = "compiled.importees.json";
const DIRECT_FILE: &str = "direct.importees.json";

/// Owns the three JSON arrays exchanged between latest hvigor's
/// `process-importee.js` build step and `ProcessHarArtifacts`.
pub struct HarImporteeCache {
    cache_dir: PathBuf,
}

/// Deterministic equivalent of the three importee arrays produced by latest
/// `@ohos/hvigor-arkts-base/process-importee`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HvigorHarImportees {
    /// Package subpath requests made by modules owned by the current HAR.
    pub non_entry: Vec<String>,
    /// All bare package requests observed across the compiled module graph.
    pub compiled: Vec<String>,
    /// All bare package requests made by modules owned by the current HAR.
    pub direct: Vec<String>,
}

#[derive(Debug, Error)]
pub enum HarImporteeCacheError {
    #[error("failed to {operation} HAR importee cache {path}: {source}")]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to {operation} HAR importee cache {path}: {source}")]
    Json {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

impl HarImporteeCache {
    pub fn new(cache_dir: impl Into<PathBuf>) -> Self {
        Self {
            cache_dir: cache_dir.into(),
        }
    }

    pub fn write_compile_output(
        &self,
        importees: &HvigorHarImportees,
        is_bytecode_har: bool,
        bundled_dependencies: bool,
        collect_direct_importees: bool,
    ) -> Result<(), HarImporteeCacheError> {
        // `processImportees` returns before touching the filesystem unless the
        // module is a ByteCode HAR or integrated optimization collects direct
        // importees. Preserve stale-file behavior on that inactive path.
        if !is_bytecode_har && !collect_direct_importees {
            return Ok(());
        }
        std::fs::create_dir_all(&self.cache_dir).map_err(|source| HarImporteeCacheError::Io {
            operation: "create directory for",
            path: self.cache_dir.clone(),
            source,
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

    pub fn read(&self) -> Result<HvigorHarImportees, HarImporteeCacheError> {
        Ok(HvigorHarImportees {
            non_entry: self.read_array(NON_ENTRY_FILE)?,
            compiled: self.read_array(COMPILED_FILE)?,
            direct: self.read_array(DIRECT_FILE)?,
        })
    }

    fn write_array(&self, file_name: &str, values: &[String]) -> Result<(), HarImporteeCacheError> {
        let path = self.cache_dir.join(file_name);
        let mut bytes =
            serde_json::to_vec(values).map_err(|source| HarImporteeCacheError::Json {
                operation: "serialize",
                path: path.clone(),
                source,
            })?;
        // fs-extra.writeJson appends the platform EOL; hvigor's observed cache on
        // macOS is compact JSON followed by one newline.
        bytes.push(b'\n');
        std::fs::write(&path, bytes).map_err(|source| HarImporteeCacheError::Io {
            operation: "write",
            path,
            source,
        })
    }

    fn read_array(&self, file_name: &str) -> Result<Vec<String>, HarImporteeCacheError> {
        let path = self.cache_dir.join(file_name);
        if !path.is_file() {
            return Ok(Vec::new());
        }
        let bytes = std::fs::read(&path).map_err(|source| HarImporteeCacheError::Io {
            operation: "read",
            path: path.clone(),
            source,
        })?;
        serde_json::from_slice(&bytes).map_err(|source| HarImporteeCacheError::Json {
            operation: "parse",
            path,
            source,
        })
    }
}
