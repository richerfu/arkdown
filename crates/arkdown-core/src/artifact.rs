//! Artifact kind and artifact metadata value objects.
//!
//! These types describe the output artifacts produced by the build pipeline
//! without encoding filesystem layout policy. Layout policy belongs to
//! `arkdown-session::artifacts::layout::ArtifactLayout`.

use serde::{Deserialize, Serialize};

/// The kind of build artifact produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArtifactKind {
    /// HAP — Harmony Ability Package (deployable app module).
    Hap,
    /// HSP — Harmony Shared Package (shared library deployed once on-device).
    Hsp,
    /// HAR — Harmony Archive (source or bytecode library package).
    Har,
    /// `.abc` — Panda bytecode file.
    Abc,
    /// `resources.index` — compiled resource index.
    ResourceIndex,
    /// `ResourceTable.txt` — resource ID table for linking.
    ResourceTable,
    /// `loader.json` — module entry descriptor consumed by the bundler.
    LoaderJson,
    /// `module.json` — final HAP module manifest.
    ModuleJson,
    /// `pack.info` — HAP packaging metadata.
    PackInfo,
    /// JavaScript chunk file.
    JsChunk,
    /// Source map file.
    SourceMap,
}

/// Metadata about a produced build artifact.
///
/// Artifact paths are stored as `String` here for display and logging.
/// Stage executors use `ArtifactLayout` for concrete paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactMeta {
    /// The artifact kind.
    pub kind: ArtifactKind,
    /// Slash-separated path relative to the module output root.
    ///
    /// Used for display and for embedding in `loader.json` / `pkgContextInfo.json`.
    pub artifact_path: String,
    /// Content hash of the artifact, if computed.
    pub content_hash: Option<String>,
    /// Whether this artifact is included in the final package.
    pub in_package: bool,
}
