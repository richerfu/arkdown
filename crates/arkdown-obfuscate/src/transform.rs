//! Source obfuscation execution boundary.
//!
//! Hvigor delegates source obfuscation semantics to SDK `arkguard`. This local
//! pass is intentionally limited to disabled configurations; active builds use
//! the module-scoped SDK adapter in [`crate::Obfuscator::obfuscate_module`].

use crate::rules::ObfuscationRules;
use crate::ObfuscationError;

/// Output of a single obfuscation pass.
#[derive(Debug)]
pub struct ObfuscationOutput {
    /// The processed JavaScript source.
    pub code: String,
    /// Source-map JSON string. `None` because this disabled-config pass does not
    /// transform source text.
    pub source_map: Option<String>,
}

impl ObfuscationRules {
    /// Apply these obfuscation rules to `source` text.
    ///
    /// Returns the original source unchanged. Active source obfuscation is
    /// handled at module scope by the SDK adapter.
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
