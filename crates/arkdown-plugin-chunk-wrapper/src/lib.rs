//! IIFE chunk wrapper plugin for arkdown.
//!
//! Wraps every output chunk in `(function () { ... })()` and rewrites
//! cross-chunk `require()` calls to `globalThis[...]` lookups as required by
//! ArkRuntime (which cannot execute raw CommonJS or ESM directly).

pub mod plugin;
pub mod registry;
pub mod rewrite;
mod source_edit;

pub use plugin::ChunkWrapperPlugin;
