//! Deterministic cache-group invalidation.
//!
//! hvigor's `invalid-cache-plugin.ts` maintains separate hash snapshots for
//! distinct groups of build inputs (compile env, project config, per-package
//! hash, mock config, HAR name→OHM URL map, router map, module deps, preload
//! SO list, and plugin identity).  When any group's snapshot changes between
//! builds, all modules in the affected set are marked for recompilation.
//!
//! # Upstream correspondence
//!
//! | Type | hvigor equivalent |
//! |------|-------------------|
//! | `CacheGroupKey` | constant keys in `invalid-cache-plugin.js` |
//! | `CacheGroupSnapshot` | `this.cache.set(key, hash)` calls |
//! | `CacheGroupsStore` | `group_keys.json` on-disk persistence |
//! | `CacheGroupDiff` | list of groups whose hash changed |

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::key::fnv1a_hash;

// ---------------------------------------------------------------------------
// CacheGroupKey
// ---------------------------------------------------------------------------

/// Identifies one deterministic invalidation group.
///
/// A group collects all build inputs that should trigger recompilation of
/// affected modules when they change.  Each variant maps to one or more
/// `this.cache.set(key, …)` calls in hvigor's `invalid-cache-plugin.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum CacheGroupKey {
    /// The compile environment discriminator (SDK version, mode, path, …).
    /// Corresponds to `getCompileEnv()` in hvigor.
    CompileEnv,
    /// Hash of `module.json5` + `build-profile.json5` bytes.
    ProjectConfig,
    /// Per-package `oh-package.json5` content hash (stable across modules).
    PackageHash,
    /// Hash of the mock-config file at `projectConfig.mockParams.mockConfigPath`.
    MockConfig,
    /// Snapshot of `hspNameOhmMap` from `aceBuildJson`.
    HarNameOhmMap,
    /// Hash of the router-map configuration (file paths + content).
    RouterMap,
    /// Per-module import-graph edges (importer → imported ids).  Compared at
    /// the module level inside `shouldTransformCachedModule`, not globally.
    ModuleDeps,
    /// Sorted list of preload SO file paths from `needPreloadSoFileSet`.
    PreloadSo,
    /// Concatenation of all `PackagePlugin::cache_identity()` strings.
    PluginIdentity,
    /// `isByteCodeHarOptimize` flag from project config.
    ByteCodeHarOptimize,
    /// `bundleName` from `AppScope/app.json5`.
    BundleName,
}

impl CacheGroupKey {
    /// Return a stable ASCII key string used as the JSON field name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CompileEnv => "compile_env",
            Self::ProjectConfig => "project_config",
            Self::PackageHash => "package_hash",
            Self::MockConfig => "mock_config",
            Self::HarNameOhmMap => "har_name_ohm_map",
            Self::RouterMap => "router_map",
            Self::ModuleDeps => "module_deps",
            Self::PreloadSo => "preload_so",
            Self::PluginIdentity => "plugin_identity",
            Self::ByteCodeHarOptimize => "byte_code_har_optimize",
            Self::BundleName => "bundle_name",
        }
    }
}

// ---------------------------------------------------------------------------
// CacheGroupSnapshot
// ---------------------------------------------------------------------------

/// A snapshot of one invalidation group's current value.
///
/// The `hash` is a stable FNV-1a 64-bit digest of the serialized group value.
/// When the persisted hash differs from the current hash, the group is
/// considered changed and affected modules must be recompiled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheGroupSnapshot {
    pub key: CacheGroupKey,
    pub hash: u64,
}

impl CacheGroupSnapshot {
    /// Compute a group snapshot from the serialized string representation of
    /// the group's current value.
    pub fn from_value(key: CacheGroupKey, serialized: &str) -> Self {
        Self {
            key,
            hash: fnv1a_hash(serialized),
        }
    }

    /// Helper: hash a pre-computed u64 value (e.g. a file mtime sum).
    pub fn from_u64(key: CacheGroupKey, value: u64) -> Self {
        Self { key, hash: value }
    }

    /// Helper: hash a boolean flag.
    pub fn from_bool(key: CacheGroupKey, value: bool) -> Self {
        Self::from_value(key, if value { "true" } else { "false" })
    }
}

// ---------------------------------------------------------------------------
// CacheGroupDiff
// ---------------------------------------------------------------------------

/// Groups that changed between the persisted and current snapshots.
#[derive(Debug, Clone, Default)]
pub struct CacheGroupDiff {
    /// Groups whose hash changed.  Sorted for deterministic diagnostics.
    pub changed: Vec<CacheGroupKey>,
}

impl CacheGroupDiff {
    /// Returns `true` when no groups changed.
    pub fn is_clean(&self) -> bool {
        self.changed.is_empty()
    }

    /// Returns `true` if the given group changed.
    pub fn contains(&self, key: CacheGroupKey) -> bool {
        self.changed.contains(&key)
    }

    /// Human-readable explanation of which groups changed.
    pub fn explain(&self) -> String {
        if self.changed.is_empty() {
            return "no cache group changes".to_string();
        }
        self.changed
            .iter()
            .map(|k| k.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

// ---------------------------------------------------------------------------
// CacheGroupsStore
// ---------------------------------------------------------------------------

/// On-disk persistence for group-level invalidation hashes.
///
/// Stored as `<cache_dir>/group_keys.json` — a simple JSON object mapping
/// `CacheGroupKey::as_str()` → u64 decimal hash.
///
/// # Upstream correspondence
///
/// Corresponds to the `this.cache.get/set("pkgJsonFileHash", …)` and similar
/// persistent-between-build cache entries in hvigor's `invalid-cache-plugin.ts`.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CacheGroupsStore {
    /// Persisted group hashes.  Key: `CacheGroupKey::as_str()`.
    #[serde(flatten)]
    hashes: BTreeMap<String, u64>,
}

impl CacheGroupsStore {
    /// Load from `<cache_dir>/group_keys.json`.  Returns an empty store if the
    /// file does not exist or cannot be parsed.
    pub fn load(cache_dir: &Path) -> Self {
        let path = cache_dir.join("group_keys.json");
        let Ok(content) = fs_err::read_to_string(&path) else {
            return Self::default();
        };
        serde_json::from_str(&content).unwrap_or_default()
    }

    /// Save to `<cache_dir>/group_keys.json`.
    pub fn save(&self, cache_dir: &Path) -> Result<(), std::io::Error> {
        std::fs::create_dir_all(cache_dir)?;
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(cache_dir.join("group_keys.json"), json)
    }

    /// Get the persisted hash for a group key, if any.
    pub fn get(&self, key: CacheGroupKey) -> Option<u64> {
        self.hashes.get(key.as_str()).copied()
    }

    /// Update a group's hash in memory.  Call `save()` to persist.
    pub fn set(&mut self, key: CacheGroupKey, hash: u64) {
        self.hashes.insert(key.as_str().to_string(), hash);
    }

    /// Compute which groups changed between `self` (persisted) and `current`
    /// (current build snapshots).
    pub fn diff(&self, current: &[CacheGroupSnapshot]) -> CacheGroupDiff {
        let mut changed = Vec::new();
        for snap in current {
            let persisted = self.get(snap.key);
            // If no persisted value → group is new → treat as changed so modules
            // are compiled fresh on the first build (matches hvigor cold-start).
            if persisted != Some(snap.hash) {
                changed.push(snap.key);
            }
        }
        changed.sort();
        CacheGroupDiff { changed }
    }

    /// Apply current snapshots to the in-memory store (in preparation for
    /// `save()`).
    pub fn update(&mut self, snapshots: &[CacheGroupSnapshot]) {
        for snap in snapshots {
            self.set(snap.key, snap.hash);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn snapshot_from_value_is_deterministic() {
        let s1 = CacheGroupSnapshot::from_value(CacheGroupKey::CompileEnv, "v1");
        let s2 = CacheGroupSnapshot::from_value(CacheGroupKey::CompileEnv, "v1");
        assert_eq!(s1.hash, s2.hash);
    }

    #[test]
    fn snapshot_different_values_differ() {
        let s1 = CacheGroupSnapshot::from_value(CacheGroupKey::CompileEnv, "v1");
        let s2 = CacheGroupSnapshot::from_value(CacheGroupKey::CompileEnv, "v2");
        assert_ne!(s1.hash, s2.hash);
    }

    #[test]
    fn diff_detects_changed_group() {
        let mut store = CacheGroupsStore::default();
        let old = CacheGroupSnapshot::from_value(CacheGroupKey::PackageHash, "old");
        store.set(CacheGroupKey::PackageHash, old.hash);

        let new = CacheGroupSnapshot::from_value(CacheGroupKey::PackageHash, "new");
        let diff = store.diff(&[new]);
        assert!(diff.contains(CacheGroupKey::PackageHash));
    }

    #[test]
    fn diff_clean_when_unchanged() {
        let mut store = CacheGroupsStore::default();
        let snap = CacheGroupSnapshot::from_value(CacheGroupKey::RouterMap, "same");
        store.set(CacheGroupKey::RouterMap, snap.hash);

        let diff = store.diff(&[snap]);
        assert!(diff.is_clean());
    }

    #[test]
    fn store_persist_and_reload() {
        let tmp = TempDir::new().unwrap();
        let mut store = CacheGroupsStore::default();
        store.set(CacheGroupKey::CompileEnv, 0xdeadbeef);
        store.save(tmp.path()).unwrap();

        let loaded = CacheGroupsStore::load(tmp.path());
        assert_eq!(loaded.get(CacheGroupKey::CompileEnv), Some(0xdeadbeef));
    }

    #[test]
    fn diff_treats_absent_key_as_changed() {
        let store = CacheGroupsStore::default(); // empty — nothing persisted
        let snap = CacheGroupSnapshot::from_value(CacheGroupKey::BundleName, "com.example.app");
        let diff = store.diff(&[snap]);
        // First-time build: no persisted value → treated as changed
        assert!(diff.contains(CacheGroupKey::BundleName));
    }
}
