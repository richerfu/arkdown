//! Built-in rolldown plugins for the OpenHarmony OS build pipeline.
//!
//! These modules were previously separate workspace crates
//! (`arkdown-plugin-*`). They are now consolidated under the `arkdown`
//! bundler crate so that the full plugin set is available through a single
//! dependency and the inter-plugin API surface can be kept internal.
//!
//! # Sub-modules
//!
//! | Module | Responsibility |
//! |--------|---------------|
//! | `factory` | `PluginFactory` — assembles the ordered plugin set for a module build |
//! | `json5` | JSON/JSON5 import transform (was `arkdown-plugin-json5`) |
//! | `duplicate_check` | Filename uniqueness check (was `arkdown-plugin-duplicate-check`) |
//! Module/package metadata is materialized from the immutable package plan and
//! Rolldown scan output; it does not require a stateful hook adapter.
//! | `ohos_resolve` | Resolve plugin wrapping the OHOS resolver (was `arkdown-plugin-ohos-resolve`) |
//! | `har_dedup` | HAR deduplication for HSP builds (was `arkdown-plugin-har-dedup`) |
//! | `ets_transform` | ArkTS transform replacing SDK JS plugins (was `arkdown-plugin-ets-transform`) |
//! | `api_transform` | Latest SDK `apiTransform` source-map boundary after ETS emit |
//! | `import_check` | Import validity / cross-module import guard |
//!
//! Removed adapters: `wrapper_chunk` and `minify` (deleted 2026-08-17). Their
//! only hook (`render_chunk`) is unreachable on the production path —
//! `Bundler::bundle()` uses `scan_module_sources()` and never renders chunks.
//! ESMODULE release minification/obfuscation is owned by the obfuscation stage
//! (ArkGuard). Their `HvigorNativePluginSlot` ranks remain as JS-plugin anchors.

pub mod api_transform;
#[cfg(test)]
mod api_transform_tests;
pub mod duplicate_check;
pub mod ets_transform;
mod ets_transform_cache;
#[cfg(test)]
mod ets_transform_cache_tests;
pub mod factory;
pub mod har_dedup;
#[cfg(test)]
mod har_dedup_tests;
pub mod hvigor_compile;
#[cfg(test)]
mod hvigor_compile_tests;
pub mod import_check;
pub mod json5;
pub mod ohos_resolve;
