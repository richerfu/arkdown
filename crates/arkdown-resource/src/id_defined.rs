use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::ResourceError;

/// Native owner for hvigor `ProcessIdDefinedFile`.
///
/// The task removes the previous intermediate first, then copies a configured
/// JSON5 seed as tab-indented JSON. A missing or unset source is a successful
/// no-op after stale-output cleanup.
pub struct IdDefinedFileProcessor<'a> {
    source: Option<&'a Path>,
    output: &'a Path,
}

impl<'a> IdDefinedFileProcessor<'a> {
    pub fn new(source: Option<&'a Path>, output: &'a Path) -> Self {
        Self { source, output }
    }

    pub fn process(&self) -> Result<Option<PathBuf>, ResourceError> {
        if self.output.exists() {
            fs_err::remove_file(self.output)?;
        }
        let Some(source) = self.source.filter(|source| source.is_file()) else {
            return Ok(None);
        };
        let content = fs_err::read_to_string(source)?;
        let value = json5::from_str::<serde_json::Value>(&content).map_err(|error| {
            ResourceError::Backend {
                message: format!(
                    "cannot parse idDefinedFilePath '{}': {error}",
                    source.display()
                ),
            }
        })?;
        if let Some(parent) = self.output.parent() {
            fs_err::create_dir_all(parent)?;
        }
        let output = fs_err::File::create(self.output)?;
        let formatter = serde_json::ser::PrettyFormatter::with_indent(b"\t");
        let mut serializer = serde_json::Serializer::with_formatter(output, formatter);
        value
            .serialize(&mut serializer)
            .map_err(|error| ResourceError::Backend {
                message: format!(
                    "cannot write id-defined intermediate '{}': {error}",
                    self.output.display()
                ),
            })?;
        Ok(Some(self.output.to_path_buf()))
    }
}
