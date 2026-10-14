//! Incremental build cache for the arkdown bundler.
//!
//! This crate provides the cache infrastructure used between builds to avoid
//! recomputing unchanged modules, bundles, resources, and ABC artifacts.
//!
//! # Architecture
//!
//! The cache is structured around the following concepts:
//!
//! - **Fingerprint** ([`CacheFingerprint`]): deterministic content identity
//!   for cache records.
//! - **Compile environment** ([`CompileEnv`]): the SDK/mode discriminator
//!   that, when changed, invalidates all module entries.
//! - **Module transform store** ([`ModuleTransformCacheStore`]): real OXC
//!   output plus Rolldown graph facts used across processes.
//!
//! # File structure
//!
//! | File | Responsibility |
//! |------|---------------|
//! | `error.rs` | `CacheError` |
//! | `key.rs` | `CompileEnv`, `CacheFingerprint` |
//! | `module_transform.rs` | OXC artifacts, graph manifest and atomic commit |

mod artifact;
pub mod error;
pub mod group_invalidation;
mod har_importee;
pub mod key;
pub mod module_transform;
pub mod plugin_context;

// ---------------------------------------------------------------------------
// Narrow re-exports
// ---------------------------------------------------------------------------

// error
pub use artifact::{ArtifactCacheStore, ArtifactInputHasher};
pub use error::CacheError;

// key — cache key types
pub use key::{compiler_cache_path, CacheFingerprint, CompileEnv, MSGPACK_COMPILE_CACHE_DIR_NAME};

pub use module_transform::{
    LoadedModuleTransformCache, ModuleTransformCacheCommit, ModuleTransformCacheCommitStats,
    ModuleTransformCacheRecord, ModuleTransformCacheStore,
};

// plugin_context — opaque V8 plugin state, with Rust-owned persistence
pub use plugin_context::PluginContextCacheStore;

// group_invalidation — deterministic cache-group invalidation
pub use group_invalidation::{CacheGroupDiff, CacheGroupKey, CacheGroupSnapshot, CacheGroupsStore};

// har_importee — ByteCode HAR importee handoff
pub use har_importee::{HarImporteeCache, HarImporteeCacheError, HvigorHarImportees};
