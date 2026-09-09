//! `arkdown` — bundler core for OpenHarmony OS projects.
//!
//! Orchestrates the full pipeline around rolldown: module loading, plugin
//! transforms, chunk graph conversion, and file output.
//!
//! # File structure (SPEC-048 Phase 4)
//!
//! | File | Responsibility |
//! |------|---------------|
//! | `plan.rs` | `BundlePlan`, `BundleInput`, `BundleOutputOptions`, `BundleCachePolicy` |
//! | `bundler.rs` | `Bundler`, `BundleOutput` — Rolldown orchestration |
//! | `manual_chunks.rs` | `ManualChunkPolicy`, `ManualChunkDecision`, `ChunkName` |
//! | `watch_config.rs` | `WatchConfigBridge`, `WatchInvalidationEvent` |
//! | `cache_bridge.rs` | `BundleCacheBridge`, `ShouldInvalidCacheRequest/Result` |
//! | `diagnostics.rs` | `BundlerDiagnostic`, `RolldownErrorKind` |
//! | `compile_entry.rs` | `CompileEntryNamer` — internal entry point helpers |
//! | `output.rs` | `WrittenOutput`, `SourceMapOptions` — output writing |

pub mod bundler;
pub mod cache_bridge;
mod chunk_module_request;
pub mod compile_entry;
pub mod diagnostics;
pub mod hvigor_json;
pub mod manual_chunks;
#[cfg(feature = "memory-diagnostics")]
pub mod memory_diagnostics;
pub mod output;
pub mod plan;
pub mod plugin_ast;
pub mod plugin_module;
pub mod plugins;
pub mod watch_config;

// ---------------------------------------------------------------------------
// Narrow re-exports (SPEC-048: no wildcard pub use)
// ---------------------------------------------------------------------------

// Core types re-exported for convenience.
pub use arkdown_core::source_maps;

// bundler — primary API
pub use bundler::{
    BundleOutput, Bundler, CrossplatformCollectionWriter, HvigorHarImportees,
    HvigorModuleRequestRewritePlan, InsightIntentModuleSource, PreloadSystemSoTask,
    SourceMapInputFingerprint,
};

// plan — typed bundle plan (SPEC-048 Phase 4 primary input)
pub use plan::{
    BundleCachePolicy, BundleInput, BundleMockConfig, BundleMockDependency,
    BundleModuleMetaInfoFile, BundleOutputOptions, BundlePlan, BundlePluginSet,
    BundleRuntimePackageContext, BundleRuntimePackageInfo, BundleSourceMapPackageInfo,
    LoaderResourceJoin, SourceMapPolicy,
};

// manual_chunks — chunk policy
pub use manual_chunks::{ChunkName, ManualChunkDecision, ManualChunkPolicy};

// watch_config — watch mode bridge
pub use watch_config::{WatchConfigBridge, WatchInvalidationEvent, WatchParityPolicy};

// cache_bridge — rollup cache bridge
pub use cache_bridge::{
    BundleCacheBridge, PluginCacheModule, PluginCacheModuleInventory, ShouldInvalidCacheRequest,
    ShouldInvalidCacheResult,
};

// diagnostics
pub use diagnostics::{BundlerDiagnostic, RolldownErrorKind};

// plugin_module — Rust/OXC/Rolldown TypeScript plugin compiler
pub use plugin_module::{PluginModuleCompileError, PluginModuleCompiler};

// hvigor_json — Rust/OXC implementation of hvigor's public JSON5 reader
pub use hvigor_json::{HvigorJsonParseError, HvigorJsonParser};

// plugin_ast — OXC-owned ESTree bridge for JavaScript compile plugins
pub use plugin_ast::{
    PluginAstParseError, PluginAstParseOptions, PluginAstParseResult, PluginAstParser,
};
