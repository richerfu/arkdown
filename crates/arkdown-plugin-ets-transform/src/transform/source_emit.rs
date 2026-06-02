//! Source emit helpers for ArkTS transform lowering.
//!
//! Upstream ets-loader keeps original AST nodes as nodes and lets the TypeScript
//! printer preserve their source shape where possible. Until ViewPU lowering is
//! fully represented as generated OXC AST, centralize original-node text access
//! here instead of scattering span slicing through lowering rules.

use oxc_span::GetSpan;

pub(crate) fn original_node_text(node: &impl GetSpan, source_text: &str) -> String {
    source_text[node.span()].trim().to_string()
}
