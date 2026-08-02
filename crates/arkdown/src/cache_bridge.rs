//! Bundle cache bridge.
//!
//! [`BundleCacheBridge`] connects the rolldown bundle stage to the rollup
//! cache service and the `shouldInvalidCache` plugin hook aggregation.
//!
//! # Upstream correspondence
//!
//! | Type | hvigor/SDK equivalent |
//! |------|-----------------------|
//! | `BundleCacheBridge` | `invalid-cache-plugin.js` + `rollup-cache-service.js` |
//! | `ShouldInvalidCacheRequest` | `wrapperShouldInvalidCache` input |
//! | `ShouldInvalidCacheResult` | `wrapperShouldInvalidCache` output |
//!
//! # Group-level invalidation
//!
//! hvigor's `invalid-cache-plugin.ts` maintains separate hash snapshots for
//! distinct groups of build inputs (`compileEnv`, `projectConfig`, `pkgJsonFileHash`,
//! `harNameOhmMap`, `routerMapHash`, `preloadSoFileSet`, `pluginIdentity`, etc.).
//! When any group's snapshot changes between builds, the affected module set is
//! marked for recompilation.  See [`arkdown_cache::CacheGroupKey`] for the
//! full group taxonomy.

use std::path::{Path, PathBuf};

use arkdown_cache::{CacheGroupDiff, CacheGroupSnapshot, CacheGroupsStore};
use arkdown_core::HvigorModuleSourceFile;
use arkdown_path::{ArtifactPathSerializer, RelPath};
use serde::{Deserialize, Serialize};

use crate::{PluginAstParseOptions, PluginAstParser};

/// A cached transformed module passed to latest-hvigor
/// `shouldInvalidCache({ ast, code, id })` hooks.
#[derive(Debug, Clone)]
pub struct PluginCacheModule {
    pub id: String,
    pub code: String,
}

impl PluginCacheModule {
    /// Serialize the exact object passed to latest-hvigor
    /// `shouldInvalidCache({ ast, code, id })` hooks.
    ///
    /// The AST envelope remains private transport metadata until the thin
    /// JavaScript bridge revives JSON-inexpressible literal values. A parser
    /// rejection leaves `ast` absent, matching hvigor's optional AST contract
    /// for transformed ArkTS syntax.
    pub fn hook_request_json(&self) -> anyhow::Result<String> {
        let mut request = serde_json::Map::from_iter([
            (
                "code".to_string(),
                serde_json::Value::String(self.code.clone()),
            ),
            ("id".to_string(), serde_json::Value::String(self.id.clone())),
        ]);
        if let Ok(parsed) =
            PluginAstParser::new(&self.code, PluginAstParseOptions::default()).parse()
        {
            request.insert("__arkdownOxcAst".to_string(), parsed.into_ast());
        }
        serde_json::to_string(&request).map_err(Into::into)
    }
}

/// Persistent module inventory for custom compile-plugin cache decisions.
///
/// Rolldown's Rust plugin trait does not currently expose Rollup's
/// `shouldTransformCachedModule` hook. Arkdown's coarse task cache can skip the
/// whole scan, so this inventory preserves the exact transformed module IDs
/// and code required by latest hvigor's custom `shouldInvalidCache` wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginCacheModuleInventory {
    version: u32,
    modules: Vec<PluginCacheModuleRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginCacheModuleRecord {
    id: String,
    source_path: String,
}

impl PluginCacheModuleInventory {
    const FILE_NAME: &'static str = "plugin_module_cache.json";
    const VERSION: u32 = 1;

    pub fn from_module_sources(
        cache_dir: &Path,
        module_sources: &[HvigorModuleSourceFile],
    ) -> anyhow::Result<Self> {
        let mut modules = module_sources
            .iter()
            .map(|source| {
                let relative = source.path.strip_prefix(cache_dir).map_err(|_| {
                    anyhow::anyhow!(
                        "plugin cache source '{}' is outside cache root '{}'",
                        source.path.display(),
                        cache_dir.display()
                    )
                })?;
                Ok(PluginCacheModuleRecord {
                    id: source.module_id.clone(),
                    source_path: ArtifactPathSerializer::path_to_slash_string(relative)?,
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        modules.sort_by(|left, right| left.id.cmp(&right.id));
        modules.dedup_by(|left, right| left.id == right.id);
        Ok(Self {
            version: Self::VERSION,
            modules,
        })
    }

    pub fn load(cache_dir: &Path) -> anyhow::Result<Option<Self>> {
        let path = cache_dir.join(Self::FILE_NAME);
        let source = match fs_err::read_to_string(&path) {
            Ok(source) => source,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let inventory: Self = serde_json::from_str(&source)?;
        if inventory.version != Self::VERSION {
            anyhow::bail!(
                "unsupported plugin module cache version {}; expected {}",
                inventory.version,
                Self::VERSION
            );
        }
        Ok(Some(inventory))
    }

    pub fn save(&self, cache_dir: &Path) -> anyhow::Result<()> {
        fs_err::create_dir_all(cache_dir)?;
        let source = serde_json::to_vec_pretty(self)?;
        fs_err::write(cache_dir.join(Self::FILE_NAME), source)?;
        Ok(())
    }

    pub fn clear(cache_dir: &Path) -> anyhow::Result<()> {
        match fs_err::remove_file(cache_dir.join(Self::FILE_NAME)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }

    pub fn modules(&self) -> &[PluginCacheModuleRecord] {
        &self.modules
    }
}

impl PluginCacheModuleRecord {
    pub fn read(&self, cache_dir: &Path) -> anyhow::Result<PluginCacheModule> {
        let relative = RelPath::parse(PathBuf::from(&self.source_path))?;
        let code = fs_err::read_to_string(cache_dir.join(relative.as_path()))?;
        Ok(PluginCacheModule {
            id: self.id.clone(),
            code,
        })
    }
}

// ---------------------------------------------------------------------------
// ShouldInvalidCacheRequest
// ---------------------------------------------------------------------------

/// Input to the `shouldInvalidCache` plugin hook.
///
/// # Upstream correspondence
///
/// Corresponds to the request object passed to `wrapperShouldInvalidCache`
/// in `arkts-pack.js`.
#[derive(Debug, Clone)]
pub struct ShouldInvalidCacheRequest {
    /// The current compile environment key string.
    pub compile_env: String,
    /// Files that have changed since the last build.
    pub changed_files: Vec<PathBuf>,
    /// Module name for per-module invalidation decisions.
    pub module_name: Option<String>,
    /// Cache directory for the module currently being checked.
    pub cache_dir: Option<PathBuf>,
    /// Whether this is a watch-mode (incremental) build.
    pub is_watch: bool,
    /// Whether the coarse task cache matched before plugin invalidation ran.
    pub cache_hit: bool,
    /// Current build's group-level invalidation snapshots.
    ///
    /// Populated by the coordinator before the bundle stage.  Each entry
    /// represents one deterministic invalidation group (see
    /// [`arkdown_cache::CacheGroupKey`]).  An empty vec means group-level
    /// invalidation is not requested for this build.
    pub group_snapshots: Vec<CacheGroupSnapshot>,
    /// Importer module ids for the module being checked (per-module deps).
    ///
    /// Used to detect whether a module's transitive dependencies have changed.
    pub module_deps: Vec<String>,
}

// ---------------------------------------------------------------------------
// ShouldInvalidCacheResult
// ---------------------------------------------------------------------------

/// Result of the `shouldInvalidCache` plugin hook.
///
/// # Upstream correspondence
///
/// Corresponds to the merged result from all registered `shouldInvalidCache`
/// plugin hooks in `arkts-pack.js::wrapperShouldInvalidCache`.
#[derive(Debug, Clone)]
pub struct ShouldInvalidCacheResult {
    /// Whether any plugin voted to invalidate the cache.
    pub should_invalidate: bool,
    /// Whether the invalidation applies to ALL modules (global invalidation).
    pub global_invalidation: bool,
    /// Optional reason from the deciding plugin.
    pub reason: Option<String>,
    /// Which cache groups changed (if group-level invalidation was requested).
    pub changed_groups: CacheGroupDiff,
}

impl ShouldInvalidCacheResult {
    /// Create a "keep cache" result.
    pub fn keep() -> Self {
        Self {
            should_invalidate: false,
            global_invalidation: false,
            reason: None,
            changed_groups: CacheGroupDiff::default(),
        }
    }

    /// Create an "invalidate" result.
    pub fn invalidate(reason: impl Into<String>) -> Self {
        Self {
            should_invalidate: true,
            global_invalidation: false,
            reason: Some(reason.into()),
            changed_groups: CacheGroupDiff::default(),
        }
    }

    /// Create a "global invalidate" result.
    pub fn invalidate_all(reason: impl Into<String>) -> Self {
        Self {
            should_invalidate: true,
            global_invalidation: true,
            reason: Some(reason.into()),
            changed_groups: CacheGroupDiff::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// BundleCacheBridge
// ---------------------------------------------------------------------------

/// Connects the bundle stage to the rollup cache service.
///
/// `BundleCacheBridge` orchestrates:
/// 1. The `shouldInvalidCache` hook aggregation before each bundle run.
/// 2. Rollup cache restoration (reading the bundle cache from disk).
/// 3. Rollup cache commit (writing the bundle cache after a successful run).
/// 4. Group-level cache invalidation: comparing current group snapshots
///    against the persisted `group_keys.json` and marking affected modules.
///
/// # Upstream correspondence
///
/// Corresponds to `invalid-cache-plugin.js` + `rollup-cache-service.js` in
/// hvigor, plus the `wrapperShouldInvalidCache` plugin hook wrapper.
pub struct BundleCacheBridge {
    /// Compile environment key for invalidation checks.
    compile_env: String,
    /// Bundle cache directory path.
    cache_dir: Option<std::path::PathBuf>,
    /// Group-level invalidation diff computed at `from_plan` time.
    ///
    /// Populated when `group_snapshots` are provided; empty otherwise.
    group_diff: CacheGroupDiff,
}

impl BundleCacheBridge {
    /// Create from a bundle plan.
    ///
    /// If `group_snapshots` is non-empty, the persisted `group_keys.json` is
    /// loaded from `cache_dir` and diffed against the current snapshots.  The
    /// diff is stored and used in subsequent `ask_plugins` calls.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `rollup-cache-service.js::getCacheDirectory` and
    /// the compile env setup.
    pub fn from_plan(
        compile_env: impl Into<String>,
        cache_dir: Option<std::path::PathBuf>,
        group_snapshots: Vec<CacheGroupSnapshot>,
    ) -> Self {
        let group_diff = if !group_snapshots.is_empty() {
            if let Some(ref dir) = cache_dir {
                let store = CacheGroupsStore::load(dir);
                store.diff(&group_snapshots)
            } else {
                CacheGroupDiff::default()
            }
        } else {
            CacheGroupDiff::default()
        };

        Self {
            compile_env: compile_env.into(),
            cache_dir,
            group_diff,
        }
    }

    /// Return the group-level diff computed at construction time.
    pub fn group_diff(&self) -> &CacheGroupDiff {
        &self.group_diff
    }

    /// Ask all registered plugins whether the cache should be invalidated.
    ///
    /// Decision priority (matching hvigor's `shouldTransformCachedModule`):
    /// 1. Compile environment changed → global invalidate.
    /// 2. Changed files → module-level invalidate.
    /// 3. Any cache group changed → module-level invalidate with group reason.
    /// 4. Module deps indicate a changed transitive dependency → invalidate.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `arkts-pack.js::wrapperShouldInvalidCache`.
    pub fn ask_plugins(&self, request: &ShouldInvalidCacheRequest) -> ShouldInvalidCacheResult {
        // Priority 1: compile environment changed → global invalidation.
        if request.compile_env != self.compile_env {
            return ShouldInvalidCacheResult::invalidate_all("compile environment changed");
        }

        // Priority 2: files changed since last build.
        if !request.changed_files.is_empty() {
            return ShouldInvalidCacheResult::invalidate(format!(
                "{} file(s) changed",
                request.changed_files.len()
            ));
        }

        // Priority 3: any cache group changed.
        if !self.group_diff.is_clean() {
            return ShouldInvalidCacheResult {
                should_invalidate: true,
                global_invalidation: false,
                reason: Some(format!(
                    "cache group(s) changed: {}",
                    self.group_diff.explain()
                )),
                changed_groups: self.group_diff.clone(),
            };
        }

        // Priority 4: module dependency graph changed (per-module).
        //
        // `request.module_deps` contains the imported module ids for the
        // current module (populated from Rolldown ModuleInfo during
        // `shouldTransformCachedModule`).  A non-empty list combined with
        // a changed-file set indicates transitive dependency impact.
        if !request.module_deps.is_empty() && !request.changed_files.is_empty() {
            return ShouldInvalidCacheResult::invalidate(format!(
                "module dependency changed ({} importers)",
                request.module_deps.len()
            ));
        }

        ShouldInvalidCacheResult::keep()
    }

    /// Restore the rollup cache from disk.
    ///
    /// Returns the rollup cache JSON string if a valid cache exists.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `rollup-cache-service.js::readCacheWithEvent`.
    pub fn restore_rollup_cache(&self) -> Option<String> {
        let dir = self.cache_dir.as_deref()?;
        let cache_path = dir.join("rollup_cache.json");
        std::fs::read_to_string(&cache_path).ok()
    }

    /// Commit the current rollup cache and group snapshots to disk.
    ///
    /// Writes both `rollup_cache.json` and `group_keys.json` so subsequent
    /// builds can diff against the current group snapshots.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `rollup-cache-service.js::writeCache` + the per-hook
    /// `this.cache.set(…)` calls that persist group state in hvigor.
    pub fn commit_rollup_cache(
        &self,
        cache_json: &str,
        group_snapshots: &[CacheGroupSnapshot],
    ) -> Result<(), std::io::Error> {
        let dir = match &self.cache_dir {
            Some(d) => d,
            None => return Ok(()),
        };
        std::fs::create_dir_all(dir)?;
        std::fs::write(dir.join("rollup_cache.json"), cache_json)?;

        if !group_snapshots.is_empty() {
            let mut store = CacheGroupsStore::load(dir);
            store.update(group_snapshots);
            store.save(dir)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkdown_cache::CacheGroupKey;

    fn module_source(module_id: &str, path: PathBuf) -> HvigorModuleSourceFile {
        HvigorModuleSourceFile {
            module_id: module_id.to_string(),
            path,
            record_name: "entry/ets/pages/Index".to_string(),
            source_file: "entry/src/main/ets/pages/Index.ets".to_string(),
            package_name: Some("entry".to_string()),
            is_commonjs: false,
            is_shared_module: false,
            origin_source_lang: "ets".to_string(),
            imported_record_names: Vec::new(),
        }
    }

    fn empty_request(compile_env: &str) -> ShouldInvalidCacheRequest {
        ShouldInvalidCacheRequest {
            compile_env: compile_env.to_string(),
            changed_files: vec![],
            module_name: None,
            cache_dir: None,
            is_watch: false,
            cache_hit: true,
            group_snapshots: vec![],
            module_deps: vec![],
        }
    }

    #[test]
    fn keep_result_is_not_invalidation() {
        let result = ShouldInvalidCacheResult::keep();
        assert!(!result.should_invalidate);
    }

    #[test]
    fn plugin_module_inventory_round_trips_exact_transformed_code() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let cache_dir = tmp.path().join("cache");
        let source_path = cache_dir.join("entry/ets/pages/Index.js");
        fs_err::create_dir_all(source_path.parent().expect("source parent"))
            .expect("create source directory");
        fs_err::write(&source_path, "export const cached = true;").expect("write source");

        PluginCacheModuleInventory::from_module_sources(
            &cache_dir,
            &[module_source("/entry/Index.ets", source_path)],
        )
        .expect("inventory")
        .save(&cache_dir)
        .expect("save inventory");

        let loaded = PluginCacheModuleInventory::load(&cache_dir)
            .expect("load inventory")
            .expect("inventory exists");
        let module = loaded.modules()[0]
            .read(&cache_dir)
            .expect("read cached module");
        assert_eq!(module.id, "/entry/Index.ets");
        assert_eq!(module.code, "export const cached = true;");
    }

    #[test]
    fn plugin_module_inventory_rejects_sources_outside_cache_root() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let cache_dir = tmp.path().join("cache");
        let outside = tmp.path().join("outside.js");
        let error = PluginCacheModuleInventory::from_module_sources(
            &cache_dir,
            &[module_source("outside", outside)],
        )
        .expect_err("outside source must fail");
        assert!(error.to_string().contains("outside cache root"));
    }

    #[test]
    fn changed_compile_env_invalidates_all() {
        let bridge = BundleCacheBridge::from_plan("env_v1", None, vec![]);
        let request = ShouldInvalidCacheRequest {
            compile_env: "env_v2".to_string(),
            ..empty_request("env_v2")
        };
        let result = bridge.ask_plugins(&request);
        assert!(result.should_invalidate);
        assert!(result.global_invalidation);
    }

    #[test]
    fn same_env_no_changes_keeps_cache() {
        let bridge = BundleCacheBridge::from_plan("env_v1", None, vec![]);
        let result = bridge.ask_plugins(&empty_request("env_v1"));
        assert!(!result.should_invalidate);
    }

    #[test]
    fn changed_group_triggers_invalidation() {
        let tmp = tempfile::TempDir::new().unwrap();
        // Persist "old" hash for PackageHash group
        let mut store = CacheGroupsStore::default();
        let old_snap = CacheGroupSnapshot::from_value(CacheGroupKey::PackageHash, "old");
        store.set(CacheGroupKey::PackageHash, old_snap.hash);
        store.save(tmp.path()).unwrap();

        // Build bridge with "new" snapshot
        let new_snap = CacheGroupSnapshot::from_value(CacheGroupKey::PackageHash, "new");
        let bridge =
            BundleCacheBridge::from_plan("env_v1", Some(tmp.path().to_path_buf()), vec![new_snap]);

        let result = bridge.ask_plugins(&empty_request("env_v1"));
        assert!(result.should_invalidate);
        assert!(!result.global_invalidation);
        assert!(result.changed_groups.contains(CacheGroupKey::PackageHash));
    }

    #[test]
    fn unchanged_group_keeps_cache() {
        let tmp = tempfile::TempDir::new().unwrap();
        let snap = CacheGroupSnapshot::from_value(CacheGroupKey::RouterMap, "same-hash");
        let mut store = CacheGroupsStore::default();
        store.set(CacheGroupKey::RouterMap, snap.hash);
        store.save(tmp.path()).unwrap();

        let bridge = BundleCacheBridge::from_plan(
            "env_v1",
            Some(tmp.path().to_path_buf()),
            vec![CacheGroupSnapshot::from_value(
                CacheGroupKey::RouterMap,
                "same-hash",
            )],
        );

        let result = bridge.ask_plugins(&empty_request("env_v1"));
        assert!(!result.should_invalidate);
    }

    #[test]
    fn commit_persists_group_snapshots() {
        let tmp = tempfile::TempDir::new().unwrap();
        let bridge = BundleCacheBridge::from_plan("env_v1", Some(tmp.path().to_path_buf()), vec![]);

        let snap = CacheGroupSnapshot::from_value(CacheGroupKey::HarNameOhmMap, "har-v1");
        bridge.commit_rollup_cache("{}", &[snap.clone()]).unwrap();

        // Reload and verify
        let store = CacheGroupsStore::load(tmp.path());
        assert_eq!(store.get(CacheGroupKey::HarNameOhmMap), Some(snap.hash));
    }
}
