//! Importee inventories written by hvigor's `processImportees` step.

/// Deterministic equivalent of the three importee arrays produced by latest
/// `@ohos/hvigor-arkts-base/process-importee`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HvigorHarImportees {
    /// Package subpath requests made by modules owned by the current HAR.
    pub non_entry: Vec<String>,
    /// All bare package requests observed across the compiled module graph.
    pub compiled: Vec<String>,
    /// All bare package requests made by modules owned by the current HAR.
    pub direct: Vec<String>,
}
