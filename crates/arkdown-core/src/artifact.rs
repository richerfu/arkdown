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

impl ArtifactMeta {
    /// Create a new artifact metadata record.
    pub fn new(kind: ArtifactKind, artifact_path: impl Into<String>) -> Self {
        Self {
            kind,
            artifact_path: artifact_path.into(),
            content_hash: None,
            in_package: false,
        }
    }

    /// Return the expected file extension for this artifact kind.
    ///
    /// Corresponds to hvigor's artifact type → file extension mapping in
    /// `BuildDirConst` and package assembly tasks.
    pub fn extension(&self) -> &'static str {
        match self.kind {
            ArtifactKind::Hap => "hap",
            ArtifactKind::Hsp => "hsp",
            ArtifactKind::Har => "har",
            ArtifactKind::Abc => "abc",
            ArtifactKind::ResourceIndex => "index",
            ArtifactKind::ResourceTable => "txt",
            ArtifactKind::LoaderJson | ArtifactKind::ModuleJson | ArtifactKind::PackInfo => "json",
            ArtifactKind::JsChunk => "js",
            ArtifactKind::SourceMap => "map",
        }
    }
}

// ---------------------------------------------------------------------------
// Stage output kinds
// ---------------------------------------------------------------------------

/// The kind of intermediate or final output produced by a build stage.
///
/// `StageOutputKind` categorizes the outputs of each pipeline stage so that
/// subsequent stages and the build service can locate and consume them without
/// hard-coding filesystem paths.
///
/// # Upstream correspondence
///
/// Corresponds to hvigor `TaskService.getArtifacts` output classification and
/// `PathInfo` / `BuildDirConst` intermediates categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StageOutputKind {
    /// `loader.json` — the bundler's compile-entry manifest.
    LoaderJson,
    /// `pkgContextInfo.json` — package context information.
    PkgContextInfo,
    /// `module.json` — processed module manifest.
    ModuleJson,
    /// `resources.index` — compiled resource index.
    ResourceIndex,
    /// `ets/modules.abc` — compiled Panda bytecode.
    Abc,
    /// Source-map JSON files.
    SourceMap,
    /// Rolldown/Rollup bundle output (JS chunks).
    BundleChunks,
    /// Signed output artifact (`.hap`, `.hsp`, `.har`).
    SignedPackage,
    /// Unsigned output artifact before signing.
    UnsignedPackage,
    /// HAR declarations directory.
    HarDeclarations,
    /// Proto binary output for Panda cache.
    ProtoBin,
}

/// Metadata about a stage's intermediate output file.
///
/// Combines the [`StageOutputKind`] with a concrete path (relative to the
/// module output root) so that downstream stages can locate the file without
/// knowing the filesystem layout policy.
///
/// # Upstream correspondence
///
/// Corresponds to hvigor `PathInfo` output file records and `BuildDirConst`
/// directory constants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageOutputMeta {
    /// The kind of stage output.
    pub kind: StageOutputKind,
    /// Slash-separated path relative to the module output root.
    pub path: String,
}

impl StageOutputMeta {
    /// Create a stage output record for a given kind and relative path.
    pub fn for_kind(kind: StageOutputKind, path: impl Into<String>) -> Self {
        Self {
            kind,
            path: path.into(),
        }
    }
}
