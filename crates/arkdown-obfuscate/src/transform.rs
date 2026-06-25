//! Source obfuscation execution boundary.
//!
//! Hvigor delegates source and bytecode obfuscation semantics to SDK
//! `arkguard`.  Until those semantics are ported in full, [`crate::Obfuscator`]
//! rejects active arkguard options during construction and this pass only
//! returns source unchanged.

use crate::rules::ObfuscationRules;
use crate::ObfuscationError;

/// Output of a single obfuscation pass.
pub struct ObfuscationOutput {
    /// The processed JavaScript source.
    pub code: String,
    /// Source-map JSON string. `None` because this pass does not transform
    /// source text after unsupported arkguard options are rejected.
    pub source_map: Option<String>,
}

impl ObfuscationRules {
    /// Apply these obfuscation rules to `source` text.
    ///
    /// Returns the original source unchanged. Active arkguard source/bytecode
    /// options are rejected by [`crate::Obfuscator`] before this pass is used.
    pub fn apply(
        &self,
        source: &str,
        _filename: &str,
    ) -> Result<ObfuscationOutput, ObfuscationError> {
        Ok(ObfuscationOutput {
            code: source.to_string(),
            source_map: None,
        })
    }
}
