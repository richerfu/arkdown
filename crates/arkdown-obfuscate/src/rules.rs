//! Parser for OHOS-style obfuscation rule files.
//!
//! # Format
//!
//! ```text
//! # comment
//! -enable-obfuscation
//! -enable-property-obfuscation
//! -enable-toplevel-obfuscation
//! -compact
//! -remove-log
//!
//! -keep-property-name
//! propertyToKeep
//! anotherProperty
//!
//! -keep-global-name
//! GlobalClass
//! globalFunction
//! ```

use std::path::Path;

use rustc_hash::FxHashSet;

/// Parsed, merged set of obfuscation directives.
#[derive(Debug, Clone, Default)]
pub struct ObfuscationRules {
    pub enable_obfuscation: bool,
    pub enable_property_obfuscation: bool,
    pub enable_toplevel_obfuscation: bool,
    pub compact: bool,
    pub remove_log: bool,
    pub keep_property_names: FxHashSet<String>,
    pub keep_global_names: FxHashSet<String>,
}

/// Current `-keep-*` section being accumulated.
enum Section {
    None,
    KeepProperty,
    KeepGlobal,
}

impl ObfuscationRules {
    /// Parse rule file content from a string.
    pub fn parse(content: &str) -> Self {
        let mut rules = ObfuscationRules::default();
        let mut section = Section::None;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            match line {
                "-enable-obfuscation" => {
                    rules.enable_obfuscation = true;
                    section = Section::None;
                }
                "-disable-obfuscation" => {
                    rules.enable_obfuscation = false;
                    section = Section::None;
                }
                "-enable-property-obfuscation" => {
                    rules.enable_property_obfuscation = true;
                    section = Section::None;
                }
                "-disable-property-obfuscation" => {
                    rules.enable_property_obfuscation = false;
                    section = Section::None;
                }
                "-enable-toplevel-obfuscation" => {
                    rules.enable_toplevel_obfuscation = true;
                    section = Section::None;
                }
                "-disable-toplevel-obfuscation" => {
                    rules.enable_toplevel_obfuscation = false;
                    section = Section::None;
                }
                "-compact" => {
                    rules.compact = true;
                    section = Section::None;
                }
                "-remove-log" => {
                    rules.remove_log = true;
                    section = Section::None;
                }
                "-keep-property-name" => {
                    section = Section::KeepProperty;
                }
                "-keep-global-name" => {
                    section = Section::KeepGlobal;
                }
                name => match section {
                    Section::KeepProperty => {
                        rules.keep_property_names.insert(name.to_string());
                    }
                    Section::KeepGlobal => {
                        rules.keep_global_names.insert(name.to_string());
                    }
                    Section::None => {}
                },
            }
        }
        rules
    }

    /// Parse a rule file from disk.
    pub fn from_file(path: &Path) -> std::io::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(Self::parse(&content))
    }

    /// Merge two rule sets (logical OR for flags, union for keep-name sets).
    pub fn merge(self, other: Self) -> Self {
        ObfuscationRules {
            enable_obfuscation: self.enable_obfuscation || other.enable_obfuscation,
            enable_property_obfuscation: self.enable_property_obfuscation
                || other.enable_property_obfuscation,
            enable_toplevel_obfuscation: self.enable_toplevel_obfuscation
                || other.enable_toplevel_obfuscation,
            compact: self.compact || other.compact,
            remove_log: self.remove_log || other.remove_log,
            keep_property_names: self
                .keep_property_names
                .into_iter()
                .chain(other.keep_property_names)
                .collect(),
            keep_global_names: self
                .keep_global_names
                .into_iter()
                .chain(other.keep_global_names)
                .collect(),
        }
    }
}
