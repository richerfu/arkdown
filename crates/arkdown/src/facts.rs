//! Bundle stage facts — cross-plugin observable outputs.
//!
//! [`BundleFacts`] aggregates the facts collected by various plugins during
//! the bundle run.  These facts are consumed by downstream stages (ABC
//! compilation, packaging) and are NOT part of the rolldown output itself.
//!
//! # Upstream correspondence
//!
//! | Type | hvigor/SDK equivalent |
//! |------|-----------------------|
//! | `BundleFacts` | global facts maps from various plugins |
//! | `ModuleFact` | `module-meta-info-plugin.js` per-module fact |
//! | `ImporteeFact` | `collect-importers-plugin.js` importee map |
//! | `HarDedupFact` | `plugin-har-dedup` dedup result |
//! | `DuplicateOutputFact` | `duplicate-check-plugin.js` warning |

use std::collections::BTreeMap;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// ModuleFact
// ---------------------------------------------------------------------------

/// Per-module fact collected by the module-meta plugin.
///
/// # Upstream correspondence
///
/// Corresponds to `module-meta-info-plugin.js` per-module metadata output.
#[derive(Debug, Clone)]
pub struct ModuleFact {
    /// The module ID (OHM URL or absolute path).
    pub module_id: String,
    /// Package name (from `oh-package.json5`).
    pub package_name: Option<String>,
    /// Package version.
    pub package_version: Option<String>,
    /// Whether this module is the package entry.
    pub is_entry: bool,
    /// OHOS module name this source belongs to.
    pub module_name: Option<String>,
}

// ---------------------------------------------------------------------------
// ImporteeFact
// ---------------------------------------------------------------------------

/// Facts about module importers (reverse dependency map).
///
/// # Upstream correspondence
///
/// Corresponds to `collect-importers-plugin.js` importee tracking output.
#[derive(Debug, Clone)]
pub struct ImporteeFact {
    /// The module ID.
    pub module_id: String,
    /// IDs of all modules that import this module.
    pub imported_by: Vec<String>,
}

// ---------------------------------------------------------------------------
// HarDedupFact
// ---------------------------------------------------------------------------

/// HAR deduplication fact from the HAR dedup plugin.
///
/// # Upstream correspondence
///
/// Corresponds to `plugin-har-dedup.js` output — modules that are
/// deduplicated across HAR packages.
#[derive(Debug, Clone)]
pub struct HarDedupFact {
    /// The duplicated module ID.
    pub module_id: String,
    /// HAR packages that contain this module.
    pub source_hars: Vec<String>,
    /// The HAR package chosen as the canonical source.
    pub canonical_har: String,
}

// ---------------------------------------------------------------------------
// DuplicateOutputFact
// ---------------------------------------------------------------------------

/// Warning fact from the duplicate output check plugin.
///
/// # Upstream correspondence
///
/// Corresponds to `duplicate-check-plugin.js` warning output — multiple
/// modules that map to the same output path.
#[derive(Debug, Clone)]
pub struct DuplicateOutputFact {
    /// The output file path that has multiple source candidates.
    pub output_path: PathBuf,
    /// The module IDs that map to this output path.
    pub conflicting_modules: Vec<String>,
}

// ---------------------------------------------------------------------------
// BundleFacts
// ---------------------------------------------------------------------------

/// Aggregated facts from the bundle run.
///
/// `BundleFacts` is populated by plugin-side fact collectors during the
/// Rolldown bundle run and consumed by downstream stages.
///
/// # Upstream correspondence
///
/// Corresponds to the combination of global facts objects maintained by
/// `collect-importers-plugin`, `module-meta-info-plugin`, and others.
#[derive(Debug, Clone, Default)]
pub struct BundleFacts {
    /// Per-module facts indexed by module ID.
    pub module_facts: BTreeMap<String, ModuleFact>,
    /// Importee facts indexed by module ID.
    pub importee_facts: BTreeMap<String, ImporteeFact>,
    /// HAR deduplication facts.
    pub har_dedup: Vec<HarDedupFact>,
    /// Duplicate output warnings.
    pub duplicate_outputs: Vec<DuplicateOutputFact>,
}

impl BundleFacts {
    /// Merge plugin facts from multiple plugin outputs.
    ///
    /// Last write wins for module/importee facts per module ID.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to the global facts accumulation across plugin hook runs.
    pub fn merge_plugin_facts(&mut self, other: BundleFacts) {
        self.module_facts.extend(other.module_facts);
        self.importee_facts.extend(other.importee_facts);
        self.har_dedup.extend(other.har_dedup);
        self.duplicate_outputs.extend(other.duplicate_outputs);
    }

    /// Return facts for a specific module.
    pub fn facts_by_module(&self, module_id: &str) -> Option<&ModuleFact> {
        self.module_facts.get(module_id)
    }

    /// Return all diagnostics (duplicate warnings as strings).
    pub fn diagnostics(&self) -> Vec<String> {
        self.duplicate_outputs
            .iter()
            .map(|d| {
                format!(
                    "duplicate output {}: {:?}",
                    d.output_path.display(),
                    d.conflicting_modules
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_adds_module_facts() {
        let mut facts = BundleFacts::default();
        let mut other = BundleFacts::default();
        other.module_facts.insert(
            "entry/Index".to_string(),
            ModuleFact {
                module_id: "entry/Index".to_string(),
                package_name: Some("entry".to_string()),
                package_version: None,
                is_entry: true,
                module_name: Some("entry".to_string()),
            },
        );
        facts.merge_plugin_facts(other);
        assert!(facts.facts_by_module("entry/Index").is_some());
    }
}
