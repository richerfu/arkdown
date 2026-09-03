//! HSP HAR-dedup facts produced by the real two-pass deduplication stage.
//!
//! # Upstream correspondence
//!
//! | Type | hvigor/SDK equivalent |
//! |------|-----------------------|
//! | `HarDedupFact` | `plugin-har-dedup` dedup result |

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
