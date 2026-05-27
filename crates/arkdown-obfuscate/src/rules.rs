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

/// Parse a rule file from its string content.
pub fn parse_rules_str(content: &str) -> ObfuscationRules {
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
                Section::None => {
                    // Unknown directive — ignore silently for forward-compatibility.
                }
            },
        }
    }
    rules
}

/// Parse a rule file from disk.
pub fn parse_rules_file(path: &std::path::Path) -> std::io::Result<ObfuscationRules> {
    let content = std::fs::read_to_string(path)?;
    Ok(parse_rules_str(&content))
}

/// Merge two rule sets, treating each `enable_*` as a logical OR and merging
/// keep-name sets with union.
pub fn merge_rules(base: ObfuscationRules, extra: ObfuscationRules) -> ObfuscationRules {
    ObfuscationRules {
        enable_obfuscation: base.enable_obfuscation || extra.enable_obfuscation,
        enable_property_obfuscation: base.enable_property_obfuscation
            || extra.enable_property_obfuscation,
        enable_toplevel_obfuscation: base.enable_toplevel_obfuscation
            || extra.enable_toplevel_obfuscation,
        compact: base.compact || extra.compact,
        remove_log: base.remove_log || extra.remove_log,
        keep_property_names: base
            .keep_property_names
            .into_iter()
            .chain(extra.keep_property_names)
            .collect(),
        keep_global_names: base
            .keep_global_names
            .into_iter()
            .chain(extra.keep_global_names)
            .collect(),
    }
}
