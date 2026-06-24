//! Effective target and product/source-set selection for a module.

use crate::module::source_set::SourceRootSet;

/// The effective build target for one module, after product/target selection.
#[derive(Debug, Clone)]
pub struct EffectiveTargetSnapshot {
    /// Target name.
    pub target_name: String,
    /// Product name.
    pub product_name: String,
    /// Resolved source roots with overlays applied.
    pub source_roots: SourceRootSet,
    /// Whether source-set overlays are active.
    pub has_source_set_overlay: bool,
}
