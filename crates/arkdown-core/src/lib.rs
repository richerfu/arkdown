//! Core shared types for the arkdown bundler.
//!
//! All other crates depend on this crate. It contains shared type definitions,
//! trait declarations, and small format-contract helpers used across stages.
//!
//! # File structure (SPEC-048 target)
//!
//! | File            | Responsibility                                          |
//! |-----------------|---------------------------------------------------------|
//! | `ids.rs`        | Typed build-context IDs: ProjectId, ModuleId, etc.     |
//! | `error.rs`      | Shared `ArkdownError` and error conversions             |
//! | `diagnostic.rs` | Typed `Diagnostic` / `DiagnosticCode` / severity       |
//! | `module.rs`     | Module identity, source kind, module metadata structs   |
//! | `chunk.rs`      | Bundler chunk data structures shared with output writers |
//! | `graph.rs`      | Graph-neutral structs; no package graph discovery       |
//! | `manifest.rs`   | Artifact-neutral manifest structs (→ arkdown-ohos-schema) |
//! | `artifact.rs`   | Artifact kind and metadata value objects               |
//! | `source_maps.rs`| Source map options and data structs                    |
//! | `resolution.rs` | Keep for migration; types move to `arkdown-resolver`    |
//! |                 |                                                         |
//! | SPEC-048 migration targets (to be moved out):           |            |
//! | `config.rs`     | Build config types → `arkdown-config` (Phase 3+)       |
//! | `paths.rs`      | `lexical_normalize` → `arkdown-path::normalize`         |

pub mod artifact;
pub mod chunk;
pub mod config;
pub mod diagnostic;
pub mod error;
pub mod graph;
pub mod ids;
pub mod manifest;
pub mod module;
pub mod paths;
// SPEC-048: resolution.rs — OhosResolvedModule / PkgBriefInfo / PkgContext
// are kept here until arkdown-resolver absorbs them (Phase 3).  Only those
// specific types are re-exported.
pub mod resolution;
pub mod source_maps;

// ---------------------------------------------------------------------------
// Narrow re-exports (SPEC-048: no wildcard pub use)
// ---------------------------------------------------------------------------

// artifact
pub use artifact::{ArtifactKind, ArtifactMeta, StageOutputKind, StageOutputMeta};

// chunk — Bundler chunk / graph types used by bundler and plugins.
pub use chunk::{ArkChunk, ChunkGraph, ChunkIdx, ChunkSourceFile, CrossChunkImportItem};

// config — Normalized build config types.
// SPEC-048: these will migrate to `arkdown-config` in Phase 3; kept here for
// migration period.
pub use config::{
    ArkOptions, ArkdownConfig, BuildMode, BuildOption, CompileMode, CopyCodeResource,
    ExternalNativeOptions, HarArtifactType, ModuleConfig, NativeDebugSymbolOption,
    NativeLibFilterOption, NativeLibOption, NativeLibPackageSelection, ObfuscationConfig,
    OhosModuleType, OutputTarget, ProductConfig, ResOptions, RuntimeOnlyConfig, SdkConfig,
    SigningConfig, SourceOption, StrictModeConfig,
};

// diagnostic
pub use diagnostic::{Diagnostic, DiagnosticAttachment, DiagnosticCode, DiagnosticSeverity};

// error
pub use error::ArkdownError;

// graph — Module graph, edges, indices.  `ImportKind` is defined in graph.rs.
pub use graph::{
    ExportBinding, ExternalModule, HostModuleInfo, ImportEdge, ImportKind, ImportedName, Module,
    ModuleGraph, ModuleIdx, ModuleMeta, NormalModule,
};

// ids — Typed build-context IDs (SPEC-048 Phase 3+).
pub use ids::{
    ChunkId, DependencyNodeId, ModuleId, PackEntryId, PackageId, ProfileFileId, ProjectId,
    ResourceFileId, SourceFileId, TargetId,
};

// manifest — Artifact-neutral manifest / profile structs.
// SPEC-048: raw OHOS schema types will migrate to arkdown-ohos-schema; kept
// here during migration period.
pub use manifest::{
    AbcArtifact, AbilityConfig, AbilityEntry, AppStartupProfile, BytecodeHarInfo,
    CrossAppSharedConfig, CrossAppSharedConfigEntry, ExtensionAbilityConfig, ExtensionAbilityEntry,
    HapManifest, HspDependency, MergedStartupConfig, MetadataEntry, ModuleJson5, ModuleJson5Module,
    OhPackageJson5, OhosResource, ResourceProfile, SkillConfig, SkillUri, StartupTask,
};

// module
pub use module::{
    HvigorModuleSourceFile, HvigorNpmEntry, ModuleDefFormat, ModuleSource, ModuleType, OxcAstRef,
    ResolvedId, SourceMap,
};

// resolution — kept for migration.
pub use resolution::{OhosResolvedModule, PkgBriefInfo, PkgContext};

// source_maps
pub use source_maps::{
    HvigorSourceMapEntry, HvigorSourceMapOptions, HvigorSourceMapScope, HvigorSourceMapWriter,
};
