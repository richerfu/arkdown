//! Compile-environment cache material.
//!
//! The compile environment is the set of inputs that, when changed, require
//! ALL module cache entries to be discarded regardless of per-module content
//! changes.  These include SDK version, ETS loader path, obfuscation settings,
//! and build mode.
//!
//! # Upstream correspondence
//!
//! | Type | hvigor equivalent |
//! |------|-------------------|
//! | `CompileEnvCacheMaterial` | `arkts-pack.js::getCompileEnv` input struct |
//! | `CompileEnvFingerprint` | The `#`-joined key string from `getCompileEnv` |
//! | `CompileEnvCachePolicy` | Whether `useSourceHash`/`usePathPlaceholder` is active |

use serde::{Deserialize, Serialize};

use crate::key::{compiler_cache_path, fnv1a_hash, CompileEnv};

// ---------------------------------------------------------------------------
// CompileEnvCacheMaterial
// ---------------------------------------------------------------------------

/// The stable raw material fed into the compile-environment fingerprint.
///
/// All fields must be pre-resolved before constructing this type — no
/// filesystem reads are performed here.
///
/// # Upstream correspondence
///
/// Corresponds to the fields assembled by `arkts-pack.js::getCompileEnv`
/// and fed into `generateConfig.compileEnv`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompileEnvCacheMaterial {
    /// SDK `compileSdkVersion` (e.g., `"6.0.0.47"`).
    pub compile_sdk_version: String,
    /// `compatibleSdkVersion` from `build-profile.json5` product.
    pub compatible_sdk_version: String,
    /// Target runtime OS (e.g., `"OpenHarmony"`).
    pub runtime_os: String,
    /// Absolute path to the SDK `ets-loader` directory.
    pub ets_loader_path: String,
    /// Whether obfuscation is enabled for this build.
    pub obfuscation_enabled: bool,
    /// OHOS module type (e.g., `"Entry"`, `"Feature"`, `"Shared"`, `"Har"`).
    pub module_type: String,
    /// Absolute path to the cache root directory.
    pub cache_path: String,
    /// Build mode: `"debug"` or `"release"`.
    pub build_mode: String,
    /// Whether `useSourceHash` is active (affects cached-module storage).
    ///
    /// When `true`, Huawei's Rollup fork stores a source-content SHA-256 in
    /// `originalCode` and clears cached transformed `code`; invalidation is
    /// content-based in either mode.
    pub use_source_hash: bool,
    /// Whether `usePathPlaceholder` is active (replaces absolute paths with
    /// placeholders in cache keys for relocatable caches).
    pub use_path_placeholder: bool,
}

impl CompileEnvCacheMaterial {
    /// Return a stable string representation of the material.
    ///
    /// Field order matches hvigor's `getCompileEnv()` concatenation.
    pub fn stable_material(&self) -> String {
        format!(
            "{}#{}#{}#{}#{}#{}#{}#{}#{}#{}",
            self.compile_sdk_version,
            self.compatible_sdk_version,
            self.runtime_os,
            self.ets_loader_path,
            self.obfuscation_enabled,
            self.module_type,
            self.cache_path,
            self.build_mode,
            self.use_source_hash,
            self.use_path_placeholder,
        )
    }

    /// Convert to a [`CompileEnv`] (drops the extra flags).
    ///
    /// `cache_path` is resolved as `<cache_path>/compiler.cache` to match
    /// hvigor's `path.resolve(t.cachePath, "compiler.cache")` in `getCompileEnv`.
    pub fn to_compile_env(&self) -> CompileEnv {
        CompileEnv::from_plan(
            &self.compile_sdk_version,
            &self.compatible_sdk_version,
            &self.runtime_os,
            &self.ets_loader_path,
            self.obfuscation_enabled,
            &self.module_type,
            compiler_cache_path(&self.cache_path),
        )
    }
}

// ---------------------------------------------------------------------------
// CompileEnvFingerprint
// ---------------------------------------------------------------------------

/// FNV-1a fingerprint of the compile environment.
///
/// When the fingerprint changes, all cache entries must be invalidated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompileEnvFingerprint {
    /// Hex fingerprint string.
    pub value: String,
    /// Human-readable key string (for diagnostics).
    pub key_string: String,
}

impl CompileEnvFingerprint {
    /// Compute the fingerprint from a material.
    pub fn from_bundle_compile_env(material: &CompileEnvCacheMaterial) -> Self {
        let key_string = material.stable_material();
        let value = format!("{:016x}", fnv1a_hash(&key_string));
        Self { value, key_string }
    }

    /// Return the fingerprint string for cache invalidation comparison.
    pub fn fingerprint(&self) -> &str {
        &self.value
    }
}

// ---------------------------------------------------------------------------
// CompileEnvCachePolicy
// ---------------------------------------------------------------------------

/// Policy governing how the compile environment affects cache invalidation.
///
/// `CompileEnvCachePolicy` encapsulates hvigor's `useSourceHash` and
/// `usePathPlaceholder` flags so that the cache key strategy can be adapted
/// for CI/CD environments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileEnvCachePolicy {
    /// When `true`, cached modules retain a content hash instead of source
    /// text. This is a storage policy, not an mtime-based invalidation mode.
    pub use_source_hash: bool,
    /// When `true`, absolute paths are replaced with placeholders to produce
    /// relocatable cache entries.
    pub use_path_placeholder: bool,
}

impl CompileEnvCachePolicy {
    /// Create a policy from the material fields.
    pub fn from_bundle_compile_env(material: &CompileEnvCacheMaterial) -> Self {
        Self {
            use_source_hash: material.use_source_hash,
            use_path_placeholder: material.use_path_placeholder,
        }
    }

    /// Returns `true` if this policy change affects existing cache entries.
    ///
    /// A policy change always affects the cache because the hash strategy or
    /// path encoding changes.
    pub fn affects_cache(&self, other: &Self) -> bool {
        self.use_source_hash != other.use_source_hash
            || self.use_path_placeholder != other.use_path_placeholder
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_material() -> CompileEnvCacheMaterial {
        CompileEnvCacheMaterial {
            compile_sdk_version: "6.0.0.47".to_string(),
            compatible_sdk_version: "12".to_string(),
            runtime_os: "OpenHarmony".to_string(),
            ets_loader_path: "/sdk/ets-loader".to_string(),
            obfuscation_enabled: false,
            module_type: "Entry".to_string(),
            cache_path: "/tmp/cache".to_string(),
            build_mode: "debug".to_string(),
            use_source_hash: false,
            use_path_placeholder: false,
        }
    }

    #[test]
    fn fingerprint_is_deterministic() {
        let m = make_material();
        let fp1 = CompileEnvFingerprint::from_bundle_compile_env(&m);
        let fp2 = CompileEnvFingerprint::from_bundle_compile_env(&m);
        assert_eq!(fp1.value, fp2.value);
    }

    #[test]
    fn different_sdk_produces_different_fingerprint() {
        let m1 = make_material();
        let mut m2 = make_material();
        m2.compile_sdk_version = "5.0.0.0".to_string();
        let fp1 = CompileEnvFingerprint::from_bundle_compile_env(&m1);
        let fp2 = CompileEnvFingerprint::from_bundle_compile_env(&m2);
        assert_ne!(fp1.value, fp2.value);
    }

    #[test]
    fn policy_affects_cache_when_source_hash_changes() {
        let p1 = CompileEnvCachePolicy {
            use_source_hash: false,
            use_path_placeholder: false,
        };
        let p2 = CompileEnvCachePolicy {
            use_source_hash: true,
            use_path_placeholder: false,
        };
        assert!(p1.affects_cache(&p2));
        assert!(!p1.affects_cache(&p1.clone()));
    }
}
