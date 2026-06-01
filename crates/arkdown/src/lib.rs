//! `arkdown` — bundler core for OpenHarmony OS projects.
//!
//! Orchestrates the full pipeline around rolldown: module loading, plugin
//! transforms, chunk graph conversion, and file output.
//!
//! ## Quick start
//! ```rust,ignore
//! use std::sync::Arc;
//! use arkdown::{Bundler, BundleOutput};
//! use arkdown_core::ArkdownConfig;
//! use arkdown_profile::LoaderJson;
//!
//! let config = ArkdownConfig { /* ... */ };
//! let bundler = Bundler::new(config, vec![]);
//! let loader_json: LoaderJson = /* ... */;
//! let output: BundleOutput = bundler.bundle(&loader_json).await?;
//! ```

pub mod bundler;
pub mod output;

pub use bundler::{BundleOutput, Bundler};
