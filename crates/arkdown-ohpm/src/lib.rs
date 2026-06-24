//! OHPM package acquisition mechanics for the arkdown bundler.
//!
//! This crate owns:
//! - `oh-package-lock.json5` parsing and resolved-version lookup.
//! - Global OHPM cache path construction and offline/prefer-offline lookup.
//! - npm-compatible OHPM registry metadata fetch and tarball download.
//! - SRI integrity verification.
//! - Safe `.tgz` extraction into the OHPM cache.
//! - Typed acquisition request/result API for `arkdown-session` orchestration.
//!
//! # What this crate does NOT own
//!
//! - Dependency graph classification (HAR, HSP, native SO, etc.)
//! - Profile processing
//! - Session snapshots
//! - Resolver indexes

pub mod acquisition;
pub mod cache;
pub mod error;
pub mod integrity;
pub mod lockfile;
pub mod package_name;
pub mod registry;
pub mod tarball;

pub use acquisition::{
    OhpmResolutionMode, OhpmResolver, PackageAcquisitionRequest, PackageAcquisitionResult,
    PackageAcquisitionSource,
};
pub use error::OhpmError;
pub use package_name::PackageName;
