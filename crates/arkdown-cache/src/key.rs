//! Cache key types.
//!
//! Production cache keys use [`CacheFingerprint`] for deterministic content
//! identity and [`CompileEnv`] for compiler-wide invalidation.
//!
//! # Upstream correspondence
//!
//! | Type | hvigor equivalent |
//! |------|-------------------|
//! | `CompileEnv` | `arkts-pack.js::getCompileEnv` output |
//! | `CacheFingerprint` | deterministic content identity for owned cache records |

use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

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
/// Used internally for computing stable short hashes for cache keys. This is
/// not exported; callers use [`CacheFingerprint::content_hash`] instead.
pub(crate) fn fnv1a_hash_bytes(bytes: &[u8]) -> u64 {
    arkdown_core::fnv1a64(bytes)
}

// ---------------------------------------------------------------------------
// CacheFingerprint
// ---------------------------------------------------------------------------

/// Deterministic fingerprints for content-addressed cache records.
pub struct CacheFingerprint;

impl CacheFingerprint {
    /// Compute a deterministic SHA-256 content hash from `content`.
    ///
    /// The result is a 64-character lowercase hex string suitable for use as
    /// a content-addressed cache identifier.
    pub fn content_hash(content: &str) -> String {
        Self::content_hash_bytes(content.as_bytes())
    }

    /// Compute a deterministic SHA-256 content hash from raw bytes.
    pub fn content_hash_bytes(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    /// Compute a fast FNV-1a fingerprint for an already versioned cache field.
    ///
    /// Use this only when the serialized cache schema already carries the
    /// collision boundary; content-addressed artifacts use SHA-256 above.
    ///
    pub fn fast_hash_bytes(bytes: &[u8]) -> u64 {
        fnv1a_hash_bytes(bytes)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_hash_is_sha256_hex() {
        let h = CacheFingerprint::content_hash("test");
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn content_hash_deterministic() {
        assert_eq!(
            CacheFingerprint::content_hash("hello"),
            CacheFingerprint::content_hash("hello")
        );
        assert_ne!(
            CacheFingerprint::content_hash("hello"),
            CacheFingerprint::content_hash("world")
        );
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
}
