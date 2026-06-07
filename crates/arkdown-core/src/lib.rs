//! Core shared types for the arkdown bundler.
//!
//! All other crates depend on this crate. It contains shared type definitions,
//! trait declarations, and small format-contract helpers used across stages.
//!
//! # File structure (SPEC-048 target)
//!
//! | File            | Responsibility                                        |
//! |-----------------|-------------------------------------------------------|
//! | `config.rs`     | Build mode, output target, module type, config values |
//! | `error.rs`      | Shared `ArkdownError` and error conversions            |
//! | `module.rs`     | Module identity, source kind, module metadata structs  |
//! | `chunk.rs`      | Bundler chunk data structures shared with output writers |
//! | `graph.rs`      | Graph-neutral structs; no package graph discovery      |
//! | `manifest.rs`   | Artifact-neutral manifest structs                      |
//! | `artifact.rs`   | Artifact kind and metadata value objects               |
//! | `paths.rs`      | Domain-neutral path normalization helpers              |
//! | `source_maps.rs`| Source map options and data structs                   |
//! | `resolution.rs` | Keep for migration; types move to `arkdown-resolver`   |
//! | `plugin.rs`     | Legacy plugin API; exports removed after rolldown migration |

pub mod artifact;
pub mod chunk;
pub mod config;
pub mod error;
pub mod graph;
pub mod manifest;
pub mod module;
pub mod paths;
// Legacy modules kept during SPEC-048 migration.
//
// `plugin.rs` — contains `ArkdownPlugin` and hook types that remain for
// `arkdown-loader` compatibility. The wildcard `pub use plugin::*` has been
// removed per SPEC-048. Only `ImportKind` is still re-exported because
// `graph.rs` uses it as a domain type; it will move to `graph.rs` in Phase 8.
//
// `resolution.rs` — `OhosResolvedModule`/`PkgBriefInfo`/`PkgContext`: kept
// until `arkdown-resolver` absorbs them (Phase 3). Only those specific types
// are re-exported, not the whole module.
pub mod plugin;
pub mod resolution;
pub mod source_maps;

pub use artifact::{ArtifactKind, ArtifactMeta};
pub use chunk::*;
pub use config::*;
pub use error::*;
pub use graph::*;
pub use manifest::*;
pub use module::*;
pub use paths::lexical_normalize;
// Selective re-exports from plugin.rs — ArkdownPlugin and hook types are NOT
// re-exported. They are only accessible via `arkdown_core::plugin::*` for
// arkdown-loader compatibility tests.
pub use plugin::ImportKind;
// Selective re-exports from resolution.rs — only types still used by
// production callers (arkdown-resolver, arkdown-plugin-ohos-resolve,
// arkdown-profile, arkdown-plugin-module-meta).
pub use resolution::{OhosResolvedModule, PkgBriefInfo, PkgContext};
pub use source_maps::*;
