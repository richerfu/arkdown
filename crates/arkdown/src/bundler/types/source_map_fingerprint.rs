//! Source-map input fingerprint for incremental map ordering.

/// One source-map entry's pre-transform source fingerprint.
///
/// This is internal incremental metadata, not an artifact serialized into the
/// HAP/HSP/HAR. It mirrors the source identity Rollup uses to decide which
/// `SourceMapGenerator.sourceMaps` entries belong to the current pass.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SourceMapInputFingerprint {
    pub key: String,
    pub content_hash: String,
}
