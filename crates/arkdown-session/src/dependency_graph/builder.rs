//! [`DependencyGraphBuilder`] — builds one dependency graph per host module.

use crate::dependency_graph::snapshot::DependencyGraphSnapshot;

/// Builds a dependency graph from package snapshot data.
pub struct DependencyGraphBuilder {
    host_module: String,
}

impl DependencyGraphBuilder {
    pub fn new(host_module: impl Into<String>) -> Self {
        Self { host_module: host_module.into() }
    }

    /// Build the dependency graph snapshot.
    pub fn build(self) -> DependencyGraphSnapshot {
        DependencyGraphSnapshot {
            host_module: self.host_module,
            ..Default::default()
        }
    }
}
