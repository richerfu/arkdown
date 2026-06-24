//! Manual chunk policy for the bundle stage.
//!
//! hvigor uses `arkts-pack.js::manualChunks` to group modules into named
//! output chunks.  [`ManualChunkPolicy`] implements this logic for Rolldown.
//!
//! # Upstream correspondence
//!
//! | Type | hvigor/SDK equivalent |
//! |------|-----------------------|
//! | `ManualChunkPolicy` | `arkts-pack.js::manualChunks` |
//! | `ManualChunkDecision` | chunk assignment result |
//! | `ChunkName` | output chunk file name |

use std::collections::BTreeMap;
use std::path::Path;

use arkdown_path::classify::PathClassifier;

// ---------------------------------------------------------------------------
// ChunkName
// ---------------------------------------------------------------------------

/// The name of an output chunk.
///
/// In OHOS module builds, the primary chunk is named after the module's
/// entry (e.g., `"entry"`, `"EntryAbility"`).  Common/vendor dependencies
/// may be grouped into a `"commons"` or `"vendors"` chunk.
///
/// # Upstream correspondence
///
/// Corresponds to `manualChunks` function return value in hvigor.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChunkName(pub String);

impl ChunkName {
    /// The common dependencies chunk name.
    pub const COMMONS: &'static str = "commons";
    /// The vendor dependencies chunk name.
    pub const VENDORS: &'static str = "vendors";

    /// Create a chunk name.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// The raw chunk name string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ChunkName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ---------------------------------------------------------------------------
// ManualChunkDecision
// ---------------------------------------------------------------------------

/// The result of a `manualChunks` decision for a single module.
///
/// # Upstream correspondence
///
/// Corresponds to the return value of hvigor's `manualChunks` function
/// for a given module ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManualChunkDecision {
    /// Assign the module to a named chunk.
    AssignTo(ChunkName),
    /// Let Rolldown decide the chunk for this module.
    Default,
}

// ---------------------------------------------------------------------------
// ManualChunkPolicy
// ---------------------------------------------------------------------------

/// Policy for grouping modules into named output chunks.
///
/// # Upstream correspondence
///
/// Corresponds to `arkts-pack.js::manualChunks` — the Rollup/Rolldown
/// `output.manualChunks` function.
pub struct ManualChunkPolicy {
    /// Explicit module ID → chunk name mappings.
    explicit: BTreeMap<String, ChunkName>,
    /// Pattern for common dependency modules (e.g., shared HAR modules).
    common_deps: CommonDependencyPatterns,
}

impl ManualChunkPolicy {
    /// Create from a `BundlePlan`.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `arkts-pack.js::manualChunks` configuration logic.
    pub fn from_plan(
        common_deps_prefixes: Vec<String>,
        explicit: BTreeMap<String, ChunkName>,
    ) -> Self {
        Self {
            explicit,
            common_deps: CommonDependencyPatterns::from_prefixes(common_deps_prefixes),
        }
    }

    /// Determine the chunk assignment for a module.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to the `manualChunks(id)` function in `arkts-pack.js`.
    pub fn chunk_for_module(&self, module_id: &str) -> ManualChunkDecision {
        // Check explicit assignments first.
        if let Some(chunk) = self.explicit.get(module_id) {
            return ManualChunkDecision::AssignTo(chunk.clone());
        }
        // Check common dependency prefixes.
        if self.is_common_dependency(module_id) {
            return ManualChunkDecision::AssignTo(ChunkName::new(ChunkName::COMMONS));
        }
        ManualChunkDecision::Default
    }

    /// Return `true` if the module is a common (shared) dependency.
    ///
    /// Common dependencies are modules under `oh_modules/` or modules
    /// whose IDs match a common-deps prefix pattern.
    pub fn is_common_dependency(&self, module_id: &str) -> bool {
        // Dependency module entries are common dependencies.
        if PathClassifier::new(Path::new(module_id)).is_inside_dependency_modules() {
            return true;
        }
        // Check explicit prefixes.
        self.common_deps.matches(module_id)
    }
}

#[derive(Debug, Clone, Default)]
struct CommonDependencyPatterns {
    prefixes: Vec<CommonDependencyPrefix>,
}

impl CommonDependencyPatterns {
    fn from_prefixes(prefixes: Vec<String>) -> Self {
        Self {
            prefixes: prefixes
                .into_iter()
                .map(CommonDependencyPrefix::new)
                .collect(),
        }
    }

    fn matches(&self, module_id: &str) -> bool {
        self.prefixes
            .iter()
            .any(|prefix| prefix.matches_module_id(module_id))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CommonDependencyPrefix {
    value: String,
}

impl CommonDependencyPrefix {
    fn new(value: String) -> Self {
        Self { value }
    }

    fn matches_module_id(&self, module_id: &str) -> bool {
        module_id.starts_with(self.value.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_assignment_takes_priority() {
        let mut explicit = BTreeMap::new();
        explicit.insert("src/utils.js".to_string(), ChunkName::new("utils"));
        let policy = ManualChunkPolicy::from_plan(vec![], explicit);
        assert_eq!(
            policy.chunk_for_module("src/utils.js"),
            ManualChunkDecision::AssignTo(ChunkName::new("utils"))
        );
    }

    #[test]
    fn default_for_normal_module() {
        let policy = ManualChunkPolicy::from_plan(vec![], BTreeMap::new());
        assert_eq!(
            policy.chunk_for_module("src/entry/Index.ets"),
            ManualChunkDecision::Default
        );
    }

    #[test]
    fn chunk_name_display() {
        let name = ChunkName::new("commons");
        assert_eq!(name.to_string(), "commons");
    }
}
