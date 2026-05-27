// Post-bundle import path rewriting (process-importee equivalent).
// Stub — full implementation in a follow-up when ChunkGraph has import metadata.

/// Rewrite `@bundle:` references in chunk code to point at pre-compiled `.abc` paths.
///
/// Currently a no-op stub; the full implementation is deferred until
/// `ChunkGraph` exposes the per-import metadata needed to perform the rewrite.
pub fn rewrite_bytecode_har_imports(_chunk_code: &str) -> String {
    // TODO: rewrite @bundle: references to point to pre-compiled .abc paths
    _chunk_code.to_string()
}
