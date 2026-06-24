//! Per-module OHOS metadata annotation plugin.
//!
//! Walks up the file tree from each resolved module to locate the nearest
//! `oh-package.json5`, then populates `arkdown_core::ModuleMeta` with
//! package name, version, module name, and dependency classification.
//!
//! Previously the standalone `arkdown-plugin-module-meta` crate.

pub mod meta_compute;
pub mod pkg_brief_info;
pub mod plugin;

pub use pkg_brief_info::{PkgBriefInfo, PkgBriefInfoBuilder};
pub use plugin::ModuleMetaPlugin;
