use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use arkdown_core::ArkdownConfig;

/// Fields that uniquely identify the compiler environment for a build.
///
/// Mirrors hvigor's `getCompileEnv()` which joins these fields with `"#"` to
/// produce a cache-invalidation key.  When any field changes between builds,
/// ALL per-module cache entries must be discarded.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompileEnv {
    pub compile_sdk_version: String,
    pub compatible_sdk_version: String,
    pub runtime_os: String,
    pub ets_loader_path: String,
    pub obfuscation_enabled: bool,
    pub module_type: String,
    pub cache_path: String,
}

/// Compute the `compileEnv` string from an [`ArkdownConfig`] and a cache path.
///
/// The returned string is the `#`-joined representation used as the cache
/// discriminator, matching hvigor's `getCompileEnv()` implementation.
pub fn compute_compile_env(config: &ArkdownConfig, cache_path: &str) -> String {
    let obfuscation_enabled = config
        .product
        .build_option
        .ark_options
        .as_ref()
        .and_then(|o| o.obfuscation.as_ref())
        .map(|o| o.enable_obfuscation)
        .unwrap_or(false);
    let module_type = config
        .modules
        .first()
        .map(|m| format!("{:?}", m.module_type))
        .unwrap_or_default();
    CompileEnv {
        compile_sdk_version: config.sdk_config.compile_sdk_version.clone(),
        compatible_sdk_version: config.product.compatible_sdk_version.clone(),
        runtime_os: config.product.runtime_os.clone(),
        ets_loader_path: config
            .sdk_config
            .ets_loader_path
            .to_string_lossy()
            .into_owned(),
        obfuscation_enabled,
        module_type,
        cache_path: cache_path.to_string(),
    }
    .to_key_string()
}

impl CompileEnv {
    /// Produce the `#`-joined key string used for cache invalidation.
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

/// Extended cache key that includes the compile-environment discriminator.
///
/// A [`BundleCacheKey`] is valid only when both the per-module [`CacheKey`]
/// fields AND the `compile_env` string match the stored values.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BundleCacheKey {
    pub inner: CacheKey,
    pub compile_env: String,
}

/// Key that uniquely identifies a valid cache entry for a module.
///
/// A cache entry is valid if and only if all three hash components match the
/// values computed from the current build inputs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CacheKey {
    /// FNV-1a hex digest of the source file content.
    /// (A real implementation would use SHA-256; FNV-1a avoids the extra dep.)
    pub content_hash: String,
    /// Fingerprint derived from build config: SDK version, build mode,
    /// `useNormalizedOHMUrl`, obfuscation settings, etc.
    pub config_hash: u64,
    /// Hash of all direct import specifiers' resolved IDs (dependency graph).
    pub dependency_hash: u64,
}

/// A single cached module entry persisted to disk between builds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedEntry {
    /// Resolved module ID (OHM URL or absolute path).
    pub id: String,
    /// The key used to validate this entry.
    pub key: CacheKey,
    /// Output of the transform pipeline for this module.
    pub transformed_code: String,
    /// Inline source map (JSON string), if generated.
    pub source_map: Option<String>,
}

/// Errors produced by cache operations.
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

/// Name of the file that persists the last-known `compileEnv` string.
const COMPILE_ENV_FILE: &str = ".compile_env";

/// Persistent incremental build cache.
///
/// Stores per-module [`CachedEntry`] snapshots on disk so that unchanged
/// modules can skip re-parsing, re-transforming, and re-bundling on the next
/// build.
///
/// # Storage layout
///
/// Each module gets its own JSON file under `cache_dir`:
/// `<fnv1a_hex_of_id>.json`
///
/// The compile-environment discriminator is stored in `.compile_env`.
///
/// # Usage
///
/// ```rust,no_run
/// # use arkdown_cache::{BuildCache, CacheKey, CachedEntry, compute_content_hash};
/// let mut cache = BuildCache::new(std::path::PathBuf::from("build/.arkdown_cache"));
/// cache.load_from_disk().unwrap();
///
/// let key = CacheKey {
///     content_hash: compute_content_hash("let x = 1;"),
///     config_hash: 0,
///     dependency_hash: 0,
/// };
/// if cache.is_valid("src/Index.ets", &key) {
///     // use cached output
/// }
/// ```
pub struct BuildCache {
    cache_dir: PathBuf,
    /// In-memory store for this build session (populated on first access).
    memory: HashMap<String, CachedEntry>,
}

impl BuildCache {
    /// Create a new cache instance pointing at `cache_dir`.
    /// Call [`load_from_disk`] to hydrate the in-memory store before the build.
    pub fn new(cache_dir: PathBuf) -> Self {
        BuildCache {
            cache_dir,
            memory: HashMap::new(),
        }
    }

    /// Scan `cache_dir` and load all valid JSON cache files into memory.
    /// Files that fail to parse are silently skipped (stale/corrupt entries).
    pub fn load_from_disk(&mut self) -> Result<(), CacheError> {
        if !self.cache_dir.exists() {
            return Ok(());
        }
        for entry in std::fs::read_dir(&self.cache_dir)?.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if let Ok(content) = fs_err::read_to_string(&path) {
                if let Ok(cached) = serde_json::from_str::<CachedEntry>(&content) {
                    self.memory.insert(cached.id.clone(), cached);
                }
            }
        }
        Ok(())
    }

    /// Look up a module by its resolved ID.
    pub fn get(&self, id: &str) -> Option<&CachedEntry> {
        self.memory.get(id)
    }

    /// Return `true` if the cached entry for `id` exists and its key matches `key`.
    pub fn is_valid(&self, id: &str, key: &CacheKey) -> bool {
        self.memory.get(id).map(|e| &e.key == key).unwrap_or(false)
    }

    /// Insert or update a cache entry both in memory and on disk.
    pub fn insert(&mut self, entry: CachedEntry) -> Result<(), CacheError> {
        let file_name = format!("{}.json", hash_id(&entry.id));
        let cache_file = self.cache_dir.join(file_name);
        fs_err::create_dir_all(&self.cache_dir)?;
        let json = serde_json::to_string_pretty(&entry)?;
        fs_err::write(&cache_file, &json)?;
        self.memory.insert(entry.id.clone(), entry);
        Ok(())
    }

    /// Remove all in-memory entries and delete all on-disk JSON files.
    /// Use this when a config change requires a full rebuild.
    pub fn invalidate_all(&mut self) -> Result<(), CacheError> {
        self.memory.clear();
        if self.cache_dir.exists() {
            for entry in std::fs::read_dir(&self.cache_dir)?.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    std::fs::remove_file(path).ok();
                }
            }
        }
        Ok(())
    }

    /// Compare `current_env` against the persisted compile-environment string.
    ///
    /// If they differ (or no stored value exists), ALL cache entries are
    /// invalidated and the new value is written to disk.  Returns `true` when
    /// the environment changed and a full rebuild is required.
    pub fn check_and_invalidate_compile_env(
        &mut self,
        current_env: &str,
    ) -> Result<bool, CacheError> {
        let env_file = self.cache_dir.join(COMPILE_ENV_FILE);
        let stored = if env_file.exists() {
            fs_err::read_to_string(&env_file).unwrap_or_default()
        } else {
            String::new()
        };
        if stored == current_env {
            return Ok(false);
        }
        self.invalidate_all()?;
        fs_err::create_dir_all(&self.cache_dir)?;
        fs_err::write(&env_file, current_env)?;
        Ok(true)
    }

    /// Number of entries currently loaded in memory.
    pub fn len(&self) -> usize {
        self.memory.len()
    }

    /// Returns `true` if no entries are loaded in memory.
    pub fn is_empty(&self) -> bool {
        self.memory.is_empty()
    }
}

/// Compute a deterministic content hash from source text.
///
/// Uses FNV-1a 64-bit to avoid adding a SHA-256 dependency; the hex string
/// is stored in [`CacheKey::content_hash`].
pub fn compute_content_hash(content: &str) -> String {
    format!("{:016x}", fnv1a_hash(content))
}

/// Derive a stable file-system safe name from a module ID.
fn hash_id(id: &str) -> String {
    format!("{:016x}", fnv1a_hash(id))
}

fn fnv1a_hash(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

pub fn fnv1a_hash_bytes(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }

    // ── compile_env invalidation ─────────────────────────────────────────────

    #[test]
    fn test_compile_env_same_env_no_invalidation() {
        let tmp = std::env::temp_dir().join("arkdown_cache_compile_env_same");
        let _ = std::fs::remove_dir_all(&tmp);

        let env = "sdk12#12#OpenHarmony#/sdk/ets-loader#false#Entry#/tmp/cache";

        // First call: no stored env → reports changed and writes env file.
        let mut cache = BuildCache::new(tmp.clone());
        let changed_first = cache.check_and_invalidate_compile_env(env).unwrap();
        assert!(changed_first, "first call with no stored env must report changed");

        // Now insert an entry (cache is empty after invalidation above).
        cache.insert(make_entry("a.ts", "h1")).unwrap();
        assert_eq!(cache.len(), 1);

        // Second call with the same env: no invalidation.
        let changed = cache.check_and_invalidate_compile_env(env).unwrap();
        assert!(!changed, "same env must not invalidate");
        assert_eq!(cache.len(), 1, "entries must survive same-env check");

        // Third call with the same env: still no invalidation.
        let changed2 = cache.check_and_invalidate_compile_env(env).unwrap();
        assert!(!changed2, "repeated same env must not invalidate");
        assert_eq!(cache.len(), 1, "entries must still survive");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_compile_env_different_env_invalidates_all() {
        let tmp = std::env::temp_dir().join("arkdown_cache_compile_env_diff");
        let _ = std::fs::remove_dir_all(&tmp);

        let mut cache = BuildCache::new(tmp.clone());
        cache.insert(make_entry("a.ts", "h1")).unwrap();
        cache.insert(make_entry("b.ts", "h2")).unwrap();
        assert_eq!(cache.len(), 2);

        let changed = cache
            .check_and_invalidate_compile_env("sdk12#12#OpenHarmony#/sdk/ets-loader#false#Entry#/tmp/cache")
            .unwrap();
        assert!(changed, "first call with no stored env must report changed");
        assert_eq!(cache.len(), 0, "all entries must be invalidated on env change");

        // Reloading from disk must also show empty.
        let mut cache2 = BuildCache::new(tmp.clone());
        cache2.load_from_disk().unwrap();
        assert_eq!(cache2.len(), 0, "disk must be cleared after env invalidation");

        // Changing the env again must also invalidate.
        cache2.insert(make_entry("c.ts", "h3")).unwrap();
        let changed2 = cache2
            .check_and_invalidate_compile_env("sdk13#13#OpenHarmony#/sdk/ets-loader#true#Entry#/tmp/cache")
            .unwrap();
        assert!(changed2, "different env string must invalidate");
        assert_eq!(cache2.len(), 0);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_compile_env_key_string_format() {
        let env = CompileEnv {
            compile_sdk_version: "6.0.0.47".to_string(),
            compatible_sdk_version: "12".to_string(),
            runtime_os: "OpenHarmony".to_string(),
            ets_loader_path: "/sdk/ets-loader".to_string(),
            obfuscation_enabled: false,
            module_type: "Entry".to_string(),
            cache_path: "/tmp/cache".to_string(),
        };
        let key = env.to_key_string();
        assert_eq!(
            key,
            "6.0.0.47#12#OpenHarmony#/sdk/ets-loader#false#Entry#/tmp/cache"
        );
    }
}

