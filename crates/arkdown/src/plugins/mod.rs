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
//! | `module_meta` | Per-module OHOS metadata (was `arkdown-plugin-module-meta`) |
//! | `ohos_resolve` | Resolve plugin wrapping the OHOS resolver (was `arkdown-plugin-ohos-resolve`) |
//! | `har_dedup` | HAR deduplication for HSP builds (was `arkdown-plugin-har-dedup`) |
//! | `ets_transform` | ArkTS transform replacing SDK JS plugins (was `arkdown-plugin-ets-transform`) |
//! | `api_transform` | Latest SDK `apiTransform` source-map boundary after ETS emit |
//! | `wrapper_chunk` | Wrapper chunk emission for globalThis module registry |
//! | `import_check` | Import validity / cross-module import guard |
//! | `minify` | Minification pass (release builds) |

pub mod api_transform;
#[cfg(test)]
mod api_transform_tests;
pub mod duplicate_check;
pub mod ets_transform;
pub mod factory;
pub mod har_dedup;
#[cfg(test)]
mod har_dedup_tests;
pub mod hvigor_compile;
#[cfg(test)]
mod hvigor_compile_tests;
pub mod import_check;
pub mod json5;
pub mod minify;
pub mod module_meta;
pub mod ohos_resolve;
pub mod wrapper_chunk;
