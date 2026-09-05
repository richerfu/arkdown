use std::path::PathBuf;

use crate::{ObfuscationError, ObfuscationRules};

/// Writes HAR consumer obfuscation rules using arkguard's exported
/// `obfuscation.txt` format.
pub struct ConsumerObfuscationFileWriter {
    self_rule_files: Vec<PathBuf>,
    dependency_rule_files: Vec<PathBuf>,
    output_path: PathBuf,
}

impl ConsumerObfuscationFileWriter {
    pub fn new(self_rule_files: Vec<PathBuf>, output_path: PathBuf) -> Self {
        Self {
            self_rule_files,
            dependency_rule_files: Vec::new(),
            output_path,
        }
    }

    pub fn with_dependency_rule_files(mut self, dependency_rule_files: Vec<PathBuf>) -> Self {
        self.dependency_rule_files = dependency_rule_files;
        self
    }

    pub fn write(&self) -> Result<(), ObfuscationError> {
        let mut merged = ObfuscationRules::default();
        for path in self
            .self_rule_files
            .iter()
            .chain(self.dependency_rule_files.iter())
        {
            if !path.exists() {
                return Err(ObfuscationError::RuleFileNotFound(path.clone()));
            }
            merged = merged.merge(ObfuscationRules::from_file(path)?);
        }
        merged.resolve_keep_dts_names()?;

        if let Some(parent) = self.output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.output_path, merged.serialize_consumer_config())?;
        Ok(())
    }
}
