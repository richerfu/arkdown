//! Path-domain newtypes and helpers for the arkdown bundler.
//!
//! This crate owns all path-type distinctions. Every other crate that needs to
//! distinguish filesystem paths, module specifiers, OHM URLs, and artifact
//! slash-paths imports from here.
//!
//! # Domain separation
//!
//! | Type               | Invariant                                            |
//! |--------------------|------------------------------------------------------|
//! | [`AbsPath`]        | Absolute, lexically normalized filesystem path.      |
//! | [`RelPath`]        | Relative, no parent traversal above its base.        |
//! | [`ArtifactPath`]   | Slash-separated UTF-8 path for hvigor artifacts.     |
//! | [`ModuleSpecifier`]| Raw import/export specifier from source text.        |
//! | [`OhmUrl`]         | Runtime OHM URL (`@bundle:…`, `@shared:…`, …).       |

pub mod abs;
pub mod artifact;
pub mod classify;
pub mod compat;
pub mod hvigor;
pub mod native_library;
pub mod normalize;
pub mod ohm;
pub mod rel;
pub mod serializer;
pub mod source_file;
pub mod specifier;

pub use abs::AbsPath;
pub use artifact::ArtifactPath;
pub use classify::PathKind;
pub use compat::{canonicalize_or_owned, to_slash};
pub use hvigor::HvigorModulePath;
pub use native_library::HvigorNativeLibraryPattern;
pub use normalize::{lexical_normalize, strip_relative_prefix};
pub use ohm::{NormalizedOhmUrl, OhmSchema, OhmUrl};
pub use rel::RelPath;
pub use serializer::ArtifactPathSerializer;
pub use source_file::{HvigorSourceFileFilter, SourceFileName};
pub use specifier::{
    ModuleSpecifier, ModuleSpecifierText, PackageSpecifierCandidate, SpecifierKind,
};
