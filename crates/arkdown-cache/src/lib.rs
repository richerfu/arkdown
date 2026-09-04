//! Incremental build cache for the arkdown bundler.
//!
//! This crate provides the cache infrastructure used between builds to avoid
//! recomputing unchanged modules, bundles, resources, and ABC artifacts.
//!
//! # Architecture
//!
//! The cache is structured around the following concepts:
//!
//! - **Namespace** ([`CacheNamespace`]): a per-stage cache subdirectory.
//! - **Key** ([`CacheKey`], [`BundleCacheKey`]): a stable fingerprint that
//!   determines whether a cached artifact is still valid.
//! - **Compile environment** ([`CompileEnv`]): the SDK/mode discriminator
//!   that, when changed, invalidates all module entries.
//! - **Store** ([`BuildCache`]): the central entry store for a build session.
//! - **Manifest** ([`CacheManifest`]): the on-disk index of cached entries.
//! - **Plugin invalidation** ([`ShouldInvalidCacheBridge`]): aggregates
//!   plugin `shouldInvalidCache` hook decisions.
//! - **Event names** ([`CacheEventName`]): watch-mode change event tracking.
//!
//! # File structure (SPEC-048)
//!
//! | File | Responsibility |
//! |------|---------------|
//! | `error.rs` | `CacheError`, `CacheDiagnostic` |
//! | `key.rs` | `CompileEnv`, `CacheKey`, `BundleCacheKey`, `StageCacheKey`, `CacheKeyMaterial` |
//! | `manifest.rs` | `CacheManifest`, `CacheManifestEntry`, `CacheManifestVersion` |
//! | `store.rs` | `BuildCache`, `BuildCacheOpener`, `CacheStore`, `CacheHit`, `CacheCommit` |
//! | `namespace.rs` | `CacheNamespace`, `StageCacheNamespace` |
//! | `artifact.rs` | `CachedArtifact`, `CachedArtifactKind`, `CachedArtifactFingerprint` |
//! | `plugin_cache.rs` | `ShouldInvalidCacheBridge`, `PluginCacheDecision`, ... |
//! | `event_name.rs` | `CacheEventName`, `UpdateEventNameHandler`, `ChangedFileEvent` |

pub mod artifact;
pub mod error;
pub mod event_name;
pub mod group_invalidation;
pub mod key;
pub mod manifest;
pub mod namespace;
pub mod plugin_cache;
pub mod plugin_context;
pub mod store;

// ---------------------------------------------------------------------------
// Narrow re-exports (SPEC-048: no wildcard pub use)
// ---------------------------------------------------------------------------

// error
pub use error::{CacheDiagnostic, CacheError};

// key — cache key types
pub use key::{
    compiler_cache_path, BundleCacheKey, CacheKey, CacheKeyMaterial, CompileEnv, StageCacheKey,
    MSGPACK_COMPILE_CACHE_DIR_NAME,
};

// manifest — on-disk cache index
pub use manifest::{CacheManifest, CacheManifestEntry, CacheManifestVersion};

// store — cache entry store
pub use store::{BuildCache, BuildCacheOpener, CacheCommit, CacheHit, CacheStore, CachedEntry};

// namespace — cache directory namespaces
pub use namespace::{CacheNamespace, StageCacheNamespace};

// artifact — cached artifact types
pub use artifact::{CachedArtifact, CachedArtifactFingerprint, CachedArtifactKind};

// plugin_cache — plugin shouldInvalidCache bridge
pub use plugin_cache::{
    PluginCacheDecision, PluginInvalidationInput, PluginInvalidationOutput,
    ShouldInvalidCacheBridge,
};

// plugin_context — opaque V8 plugin state, with Rust-owned persistence
pub use plugin_context::PluginContextCacheStore;

// event_name — watch-mode event tracking
pub use event_name::{CacheEventName, ChangedFileEvent, UpdateEventNameHandler};

// group_invalidation — deterministic cache-group invalidation
pub use group_invalidation::{CacheGroupDiff, CacheGroupKey, CacheGroupSnapshot, CacheGroupsStore};
