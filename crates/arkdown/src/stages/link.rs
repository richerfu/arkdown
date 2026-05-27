use arkdown_core::ModuleGraph;

/// Resolve cross-module symbol references.
///
/// For now this is a no-op placeholder. The loader already resolves import
/// specifiers to `ResolvedId`s. Full symbol-level linking (binding resolution,
/// cycle detection) is needed for tree-shaking; for basic bundling the
/// loader-set `ImportEdge.resolved` values are sufficient.
pub fn link(_graph: &mut ModuleGraph) {
    // TODO: DFS cycle detection; validate all resolved IDs are present in the graph.
}
