//! `arkdown-plugin-module-meta` — OHOS per-module metadata annotation.
//!
//! This plugin walks up the file tree from each module's path to locate the
//! nearest `oh-package.json5`, then populates [`arkdown_core::ModuleMeta`]
//! with package name, version, module name, and dependency classification.
//!
//! # Usage
//!
//! ```rust,no_run
//! use std::path::PathBuf;
//! use arkdown_plugin_module_meta::ModuleMetaPlugin;
//!
//! let plugin = ModuleMetaPlugin::new(PathBuf::from("/my/project"), "entry");
//! ```

pub mod meta_compute;
pub mod pkg_brief_info;
pub mod plugin;

pub use pkg_brief_info::{build_pkg_brief_info, PkgBriefInfo};
pub use plugin::ModuleMetaPlugin;
