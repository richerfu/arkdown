//! Dependency node classifiers: bytecode HAR, integrated HSP, native SO,
//! source HAR, and runtime-only classification.

use crate::dependency_graph::snapshot::{DependencyNodeSnapshot, DependencyNodeType};

/// Classify a dependency node by inspecting its package root content.
///
/// Uses the `node_type` field and `package_root` path to determine if the
/// package is a bytecode HAR, integrated HSP, etc.
pub fn classify_node(node: &mut DependencyNodeSnapshot) {
    let Some(root) = &node.package_root else {
        node.node_type = DependencyNodeType::Unknown;
        return;
    };

    // Bytecode HAR: dynamic and static frontends preserve distinct merged-ABC
    // names in the package.
    if ["modules.abc", "modules_static.abc"]
        .into_iter()
        .any(|file_name| root.join("ets").join(file_name).is_file())
    {
        node.node_type = DependencyNodeType::BytecodeHar;
        return;
    }

    // Source HAR: contains `oh-package.json5` with har package type.
    if let Some(snap) = &node.package {
        if snap.package_json.package_type.as_deref() == Some("har") {
            node.node_type = DependencyNodeType::SourceHar;
            return;
        }
    }

    node.node_type = DependencyNodeType::OhpmSource;
}
