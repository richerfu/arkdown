//! Cache key types.
//!
//! This module defines the types used to identify and validate cache entries.
//! Production cache keys are driven by [`CacheKeyMaterial`] which combines
//! content fingerprints from `InputFingerprintSet` with per-stage config
//! hashes.
//!
//! # Upstream correspondence
//!
//! | Type | hvigor equivalent |
//! |------|-------------------|
//! | `CompileEnv` | `arkts-pack.js::getCompileEnv` output |
//! | `BundleCacheKey` | bundle-level cache entry in `rollup-cache-service.js` |
//! | `CacheKey` | per-module cache key from `invalid-cache-plugin.js` |
//! | `CacheKeyMaterial` | fingerprint input fed into the above |

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Hvigor ArkTS msgpack compile cache file name.
///
/// Upstream:
/// `hvigor-arkts-base/dist/src/util/constants.ts::MSGPACK_COMPILE_CACHE_DIR_NAME`.
pub const MSGPACK_COMPILE_CACHE_DIR_NAME: &str = "compiler.cache";

/// Resolve the compiler cache file path for hvigor compile-env parity.
///
/// Upstream `hvigor-arkts-compose/dist/src/arkts-pack.ts::getCompileEnv`
/// uses `path.resolve(t.cachePath, MSGPACK_COMPILE_CACHE_DIR_NAME)` as the
/// seventh compile environment field.
pub fn compiler_cache_path(cache_path: impl AsRef<Path>) -> String {
    cache_path
        .as_ref()
        .join(MSGPACK_COMPILE_CACHE_DIR_NAME)
        .display()
        .to_string()
}

// ---------------------------------------------------------------------------
// FNV-1a hash primitive — internal only
// ---------------------------------------------------------------------------

/// FNV-1a 64-bit hash of a byte slice.
///
/// Used internally for computing stable short hashes for cache keys.
/// This is NOT exported as a public API — callers must use
/// [`CacheKeyMaterial::compute_content_hash`] instead.
pub(crate) fn fnv1a_hash_bytes(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// FNV-1a 64-bit hash of a string slice.
///
/// Internal implementation detail — use [`CacheKeyMaterial::compute_content_hash`]
/// for public API.
pub(crate) fn fnv1a_hash(s: &str) -> u64 {
    fnv1a_hash_bytes(s.as_bytes())
}

// ---------------------------------------------------------------------------
// CacheKeyMaterial
// ---------------------------------------------------------------------------

/// Material used to construct a [`CacheKey`] or [`BundleCacheKey`].
///
/// `CacheKeyMaterial` is the typed input to key construction.  Callers build
/// up material from source fingerprints, config hashes, and dependency graph
/// hashes, then call [`CacheKeyMaterial::build`] to produce the final key.
///
/// # Upstream correspondence
///
/// Corresponds to the per-module cache key inputs collected by
/// `invalid-cache-plugin.js` and passed to `shouldInvalidCache`.
#[derive(Debug, Clone, Default)]
pub struct CacheKeyMaterial {
    /// Content bytes of the source file (or pre-computed FNV-1a hex hash).
    pub content_hash: Option<String>,
    /// Hash of the build configuration (SDK version, mode, etc.).
    pub config_hash: u64,
    /// Hash of the resolved import graph for this module.
    pub dependency_hash: u64,
}

impl CacheKeyMaterial {
    /// Compute a deterministic FNV-1a content hash from `content`.
    ///
    /// The result is a 16-character lowercase hex string suitable for use as
    /// a [`CacheKey::content_hash`].
    ///
    /// # Upstream correspondence
    ///
    /// Replaces `arkdown_cache::compute_content_hash` (SPEC-048: free function
    /// → associated method).
    pub fn compute_content_hash(content: &str) -> String {
        format!("{:016x}", fnv1a_hash(content))
    }

    /// Compute a deterministic FNV-1a content hash from raw bytes.
    pub fn compute_content_hash_bytes(bytes: &[u8]) -> String {
        format!("{:016x}", fnv1a_hash_bytes(bytes))
    }

    /// Compute the raw FNV-1a u64 hash of a string.
    ///
    /// Use this when you need the raw integer value rather than a hex string
    /// (e.g., for comparing against stored numeric hashes or embedding in
    /// filenames that use decimal representation).
    ///
    /// # Note
    ///
    /// Prefer [`CacheKeyMaterial::compute_content_hash`] for cache keys, as
    /// it produces a stable hex string.  This method is for callers that need
    /// numeric hash compatibility (e.g., router-map cache, coverage var names).
    pub fn hash_u64(s: &str) -> u64 {
        fnv1a_hash(s)
    }

    /// Build a [`CacheKey`] from the accumulated material.
    pub fn build(self) -> CacheKey {
        let content_hash = self
            .content_hash
            .unwrap_or_else(|| "0000000000000000".to_string());
        CacheKey {
            content_hash,
            config_hash: self.config_hash,
            dependency_hash: self.dependency_hash,
        }
    }
}

// ---------------------------------------------------------------------------
// CompileEnv
// ---------------------------------------------------------------------------

/// Fields that uniquely identify the compiler environment for a build.
///
/// Mirrors hvigor's exact 7-field `getCompileEnv()` key:
/// `compileSdkVersion#compatibleSdkVersion#runtimeOS#etsLoaderPath#obfuscationEnabled#moduleType#cachePath`
///
/// where `cachePath` is `path.resolve(t.cachePath, "compiler.cache")`.
///
/// When any field changes between builds, ALL per-module cache entries must be
/// discarded.
///
/// # Upstream correspondence
///
/// Corresponds to `arkts-pack.js::getCompileEnv` and
/// `generateConfig.compileEnv` in hvigor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompileEnv {
    pub compile_sdk_version: String,
    pub compatible_sdk_version: String,
    pub runtime_os: String,
    pub ets_loader_path: String,
    pub obfuscation_enabled: bool,
    pub module_type: String,
    /// Resolved as `path.resolve(cache_path, "compiler.cache")` to match
    /// hvigor's `getCompileEnv` exact field value.
    pub cache_path: String,
}

impl CompileEnv {
    /// Construct from a typed plan input.
    ///
    /// `cache_path` must already be the resolved compiler cache path
    /// (`<loader.cachePath>/compiler.cache`).  All other fields must be
    /// pre-resolved; this method does not access the filesystem or
    /// configuration files.
    pub fn from_plan(
        compile_sdk_version: impl Into<String>,
        compatible_sdk_version: impl Into<String>,
        runtime_os: impl Into<String>,
        ets_loader_path: impl Into<String>,
        obfuscation_enabled: bool,
        module_type: impl Into<String>,
        cache_path: impl Into<String>,
    ) -> Self {
        Self {
            compile_sdk_version: compile_sdk_version.into(),
            compatible_sdk_version: compatible_sdk_version.into(),
            runtime_os: runtime_os.into(),
            ets_loader_path: ets_loader_path.into(),
            obfuscation_enabled,
            module_type: module_type.into(),
            cache_path: cache_path.into(),
        }
    }

    /// Produce the `#`-joined key string used for cache invalidation.
    ///
    /// Field order matches hvigor's `getCompileEnv()` concatenation (7 fields,
    /// 6 `#` separators).
    pub fn to_key_string(&self) -> String {
        format!(
            "{}#{}#{}#{}#{}#{}#{}",
            self.compile_sdk_version,
            self.compatible_sdk_version,
            self.runtime_os,
            self.ets_loader_path,
            self.obfuscation_enabled,
            self.module_type,
            self.cache_path,
        )
    }
}

// ---------------------------------------------------------------------------
// CacheKey
// ---------------------------------------------------------------------------

/// Key that uniquely identifies a valid cache entry for a module.
///
/// A cache entry is valid if and only if all three hash components match the
/// values computed from the current build inputs.
///
/// # Upstream correspondence
///
/// Corresponds to the per-module fingerprint check in
/// `invalid-cache-plugin.js::shouldInvalidCache`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CacheKey {
    /// FNV-1a hex digest of the source file content.
    pub content_hash: String,
    /// Fingerprint derived from build config: SDK version, build mode,
    /// `useNormalizedOHMUrl`, obfuscation settings, etc.
    pub config_hash: u64,
    /// Hash of all direct import specifiers' resolved IDs (dependency graph).
    pub dependency_hash: u64,
}

impl CacheKey {
    /// Compute a stable string hash of the key for use as a filesystem-safe
    /// cache identifier.
    ///
    /// The result is a 16-character hex string.
    pub fn stable_hash(&self) -> String {
        let combined = format!(
            "{}:{}:{}",
            self.content_hash, self.config_hash, self.dependency_hash
        );
        CacheKeyMaterial::compute_content_hash(&combined)
    }
}

// ---------------------------------------------------------------------------
// BundleCacheKey
// ---------------------------------------------------------------------------

/// Extended cache key that includes the compile-environment discriminator.
///
/// A [`BundleCacheKey`] is valid only when both the per-module [`CacheKey`]
/// fields AND the `compile_env` string match the stored values.
///
/// # Upstream correspondence
///
/// Corresponds to the bundle-level cache key in `rollup-cache-service.js`
/// that gates the entire bundle output.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BundleCacheKey {
    pub inner: CacheKey,
    pub compile_env: String,
}

impl BundleCacheKey {
    /// Build a bundle cache key from a set of fingerprints and a compile env.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to constructing the bundle-level cache key in
    /// `arkts-pack.js::generateConfig` and `rollup-cache-service.js`.
    pub fn from_fingerprints(
        content_hash: impl Into<String>,
        config_hash: u64,
        dependency_hash: u64,
        compile_env: impl Into<String>,
    ) -> Self {
        Self {
            inner: CacheKey {
                content_hash: content_hash.into(),
                config_hash,
                dependency_hash,
            },
            compile_env: compile_env.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// StageCacheKey
// ---------------------------------------------------------------------------

/// Cache key scoped to a specific build stage (e.g., profile, resource, ABC).
///
/// `StageCacheKey` adds a `stage_name` discriminant to the inner [`CacheKey`]
/// so that cache entries from different stages do not collide even if they
/// share the same module input fingerprint.
///
/// # Upstream correspondence
///
/// Corresponds to task-level cache keys in hvigor's task cache service.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StageCacheKey {
    /// The stage name (e.g., `"profile"`, `"resource"`, `"abc"`).
    pub stage_name: String,
    /// The underlying module-level key.
    pub inner: CacheKey,
}

impl StageCacheKey {
    /// Create a stage-scoped cache key.
    pub fn new(stage_name: impl Into<String>, inner: CacheKey) -> Self {
        Self {
            stage_name: stage_name.into(),
            inner,
        }
    }

    /// Compute a stable filesystem-safe identifier for this key.
    pub fn stable_hash(&self) -> String {
        let combined = format!("{}:{}", self.stage_name, self.inner.stable_hash());
        CacheKeyMaterial::compute_content_hash(&combined)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_content_hash_is_16_hex_chars() {
        let h = CacheKeyMaterial::compute_content_hash("test");
        assert_eq!(h.len(), 16);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn compute_content_hash_deterministic() {
        assert_eq!(
            CacheKeyMaterial::compute_content_hash("hello"),
            CacheKeyMaterial::compute_content_hash("hello")
        );
        assert_ne!(
            CacheKeyMaterial::compute_content_hash("hello"),
            CacheKeyMaterial::compute_content_hash("world")
        );
    }

    #[test]
    fn cache_key_stable_hash_differs_for_different_keys() {
        let k1 = CacheKey {
            content_hash: "a".to_string(),
            config_hash: 0,
            dependency_hash: 0,
        };
        let k2 = CacheKey {
            content_hash: "b".to_string(),
            config_hash: 0,
            dependency_hash: 0,
        };
        assert_ne!(k1.stable_hash(), k2.stable_hash());
    }

    #[test]
    fn compile_env_key_string_format() {
        let env = CompileEnv::from_plan(
            "6.0.0.47",
            "12",
            "OpenHarmony",
            "/sdk/ets-loader",
            false,
            "Entry",
            "/tmp/cache/compiler.cache",
        );
        assert_eq!(
            env.to_key_string(),
            "6.0.0.47#12#OpenHarmony#/sdk/ets-loader#false#Entry#/tmp/cache/compiler.cache"
        );
    }

    #[test]
    fn material_build_uses_default_hash_when_unset() {
        let material = CacheKeyMaterial {
            content_hash: None,
            config_hash: 0,
            dependency_hash: 0,
        };
        let key = material.build();
        assert_eq!(key.content_hash, "0000000000000000");
    }

    #[test]
    fn bundle_cache_key_from_fingerprints() {
        let k = BundleCacheKey::from_fingerprints("abc", 1, 2, "env_str");
        assert_eq!(k.inner.content_hash, "abc");
        assert_eq!(k.compile_env, "env_str");
    }
}
