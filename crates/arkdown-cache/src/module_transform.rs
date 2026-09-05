//! Versioned, cross-process cache for OXC module transform outputs.
//!
//! The cache persists only allocator-independent data: generated code, source
//! maps, diagnostics/facts serialized by the transform owner, and the resolved
//! module graph. A previous graph is used solely to invalidate the reverse
//! dependency closure; Rolldown's in-memory AST graph is never represented by
//! a module-id list.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use arkdown_path::RelPath;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::{CacheError, CacheFingerprint};

/// One allocator-independent transform result owned by the bundler adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleTransformCacheRecord<T> {
    pub module_id: String,
    /// Hash of the exact code received by the transform hook, after any
    /// package plugin positioned before the native OXC transform.
    pub input_hash: String,
    /// Hash of the BuildSession source plus generated side input (for example
    /// a `.visual` fragment). This drives graph-closure invalidation before
    /// Rolldown starts invoking hooks.
    pub source_snapshot_hash: String,
    /// Typed transform data encoded by the owning bundler adapter.
    pub payload: T,
}

/// Successful bundle facts committed together with transform records.
pub struct ModuleTransformCacheCommit<'a, T> {
    pub records: Vec<ModuleTransformCacheRecord<T>>,
    pub module_imports: &'a BTreeMap<String, Vec<String>>,
    pub active_module_ids: &'a [String],
    pub source_snapshot_hashes: &'a BTreeMap<String, String>,
}

/// Valid transform records loaded before Rolldown enters its hook hot path.
#[derive(Debug)]
pub struct LoadedModuleTransformCache<T> {
    records: BTreeMap<String, ModuleTransformCacheRecord<T>>,
}

impl<T> Default for LoadedModuleTransformCache<T> {
    fn default() -> Self {
        Self {
            records: BTreeMap::new(),
        }
    }
}

impl<T> LoadedModuleTransformCache<T> {
    pub fn into_records(self) -> BTreeMap<String, ModuleTransformCacheRecord<T>> {
        self.records
    }
}

/// Persistent cache owner for one module/target transform namespace.
pub struct ModuleTransformCacheStore {
    root: PathBuf,
    environment_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModuleTransformCacheManifest {
    version: u32,
    environment_key: String,
    modules: BTreeMap<String, ModuleTransformCacheManifestEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModuleTransformCacheManifestEntry {
    source_snapshot_hash: Option<String>,
    imports: Vec<String>,
    artifact: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModuleTransformCacheArtifact<T> {
    version: u32,
    record: ModuleTransformCacheRecord<T>,
}

impl ModuleTransformCacheStore {
    const VERSION: u32 = 1;
    const MANIFEST_FILE: &'static str = "manifest.json";
    const PREVIOUS_MANIFEST_FILE: &'static str = "manifest.previous.json";
    const ARTIFACT_DIR: &'static str = "artifacts";

    pub fn new(root: PathBuf, environment_key: String) -> Self {
        Self {
            root,
            environment_key,
        }
    }

    /// Load and validate every reusable artifact before plugin hooks run.
    /// Corrupt individual artifacts are ordinary misses. A malformed current
    /// manifest falls back to the previous successful commit; an error is
    /// returned only when neither manifest is usable.
    pub fn load<T: DeserializeOwned>(
        &self,
        current_source_hashes: &BTreeMap<String, String>,
    ) -> Result<LoadedModuleTransformCache<T>, CacheError> {
        let Some(manifest) = self.load_manifest()? else {
            return Ok(LoadedModuleTransformCache::default());
        };
        if manifest.version != Self::VERSION || manifest.environment_key != self.environment_key {
            return Ok(LoadedModuleTransformCache::default());
        }

        let invalidated = ModuleTransformInvalidation::new(&manifest, current_source_hashes)
            .reverse_dependency_closure();
        let mut records = BTreeMap::new();
        for (module_id, entry) in manifest.modules {
            if invalidated.contains(&module_id) {
                continue;
            }
            let Some(relative) = entry.artifact else {
                continue;
            };
            let Ok(relative) = RelPath::parse(PathBuf::from(relative)) else {
                continue;
            };
            let artifact_path = self.root.join(relative.as_path());
            let Ok(bytes) = fs_err::read(&artifact_path) else {
                continue;
            };
            let Ok(artifact) = serde_json::from_slice::<ModuleTransformCacheArtifact<T>>(&bytes)
            else {
                continue;
            };
            if artifact.version != Self::VERSION || artifact.record.module_id != module_id {
                continue;
            }
            records.insert(module_id, artifact.record);
        }
        Ok(LoadedModuleTransformCache { records })
    }

    /// Publish content-addressed artifacts first and atomically replace the
    /// manifest last. A failed build never calls this method, and a process
    /// crash before the final rename leaves the previous manifest recoverable.
    pub fn commit<T: Serialize>(
        &self,
        input: ModuleTransformCacheCommit<'_, T>,
    ) -> Result<(), CacheError> {
        fs_err::create_dir_all(self.root.join(Self::ARTIFACT_DIR))?;
        let mut artifact_by_module = BTreeMap::new();
        for record in input.records {
            let artifact = ModuleTransformCacheArtifact {
                version: Self::VERSION,
                record,
            };
            let bytes = serde_json::to_vec(&artifact)?;
            let content_hash = CacheFingerprint::content_hash_bytes(&bytes);
            let relative = PathBuf::from(Self::ARTIFACT_DIR).join(format!("{content_hash}.json"));
            let path = self.root.join(&relative);
            if !path.is_file() {
                AtomicCacheFile::new(&path).write(&bytes)?;
            }
            artifact_by_module.insert(
                artifact.record.module_id.clone(),
                arkdown_path::ArtifactPathSerializer::rel_to_slash(&relative),
            );
        }

        let mut module_ids = input
            .active_module_ids
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        module_ids.extend(input.module_imports.keys().cloned());
        module_ids.extend(artifact_by_module.keys().cloned());
        let modules = module_ids
            .into_iter()
            .map(|module_id| {
                let mut imports = input
                    .module_imports
                    .get(&module_id)
                    .cloned()
                    .unwrap_or_default();
                imports.sort();
                imports.dedup();
                let entry = ModuleTransformCacheManifestEntry {
                    source_snapshot_hash: input.source_snapshot_hashes.get(&module_id).cloned(),
                    imports,
                    artifact: artifact_by_module.remove(&module_id),
                };
                (module_id, entry)
            })
            .collect();
        let manifest = ModuleTransformCacheManifest {
            version: Self::VERSION,
            environment_key: self.environment_key.clone(),
            modules,
        };
        let bytes = serde_json::to_vec_pretty(&manifest)?;
        AtomicManifestFile::new(&self.root).replace(&bytes)
    }

    fn load_manifest(&self) -> Result<Option<ModuleTransformCacheManifest>, CacheError> {
        let current = self.root.join(Self::MANIFEST_FILE);
        match fs_err::read(&current) {
            Ok(bytes) => match serde_json::from_slice(&bytes) {
                Ok(manifest) => return Ok(Some(manifest)),
                Err(current_error) => {
                    let previous = self.root.join(Self::PREVIOUS_MANIFEST_FILE);
                    match fs_err::read(&previous) {
                        Ok(bytes) => {
                            return serde_json::from_slice(&bytes).map(Some).map_err(Into::into)
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                            return Err(current_error.into());
                        }
                        Err(error) => return Err(error.into()),
                    }
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let previous = self.root.join(Self::PREVIOUS_MANIFEST_FILE);
        match fs_err::read(previous) {
            Ok(bytes) => serde_json::from_slice(&bytes).map(Some).map_err(Into::into),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
}

struct ModuleTransformInvalidation<'a> {
    manifest: &'a ModuleTransformCacheManifest,
    current_source_hashes: &'a BTreeMap<String, String>,
}

impl<'a> ModuleTransformInvalidation<'a> {
    fn new(
        manifest: &'a ModuleTransformCacheManifest,
        current_source_hashes: &'a BTreeMap<String, String>,
    ) -> Self {
        Self {
            manifest,
            current_source_hashes,
        }
    }

    fn reverse_dependency_closure(&self) -> BTreeSet<String> {
        let mut reverse = BTreeMap::<&str, Vec<&str>>::new();
        for (importer, entry) in &self.manifest.modules {
            for importee in &entry.imports {
                reverse
                    .entry(importee.as_str())
                    .or_default()
                    .push(importer.as_str());
            }
        }
        let mut invalidated = self
            .manifest
            .modules
            .iter()
            .filter_map(|(module_id, entry)| {
                let current = self.current_source_hashes.get(module_id);
                (current != entry.source_snapshot_hash.as_ref()).then(|| module_id.clone())
            })
            .collect::<BTreeSet<_>>();
        let mut pending = invalidated.iter().cloned().collect::<VecDeque<_>>();
        while let Some(changed) = pending.pop_front() {
            let Some(importers) = reverse.get(changed.as_str()) else {
                continue;
            };
            for importer in importers {
                if invalidated.insert((*importer).to_string()) {
                    pending.push_back((*importer).to_string());
                }
            }
        }
        invalidated
    }
}

struct AtomicCacheFile<'a> {
    path: &'a Path,
}

impl<'a> AtomicCacheFile<'a> {
    fn new(path: &'a Path) -> Self {
        Self { path }
    }

    fn write(&self, bytes: &[u8]) -> Result<(), CacheError> {
        let temporary = self.temporary_path();
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        match fs_err::rename(&temporary, self.path) {
            Ok(()) => Ok(()),
            Err(_error) if self.path.is_file() => {
                let _ = fs_err::remove_file(temporary);
                Ok(())
            }
            Err(error) => Err(error.into()),
        }
    }

    fn temporary_path(&self) -> PathBuf {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        self.path
            .with_extension(format!("tmp-{}-{sequence}", std::process::id()))
    }
}

struct AtomicManifestFile<'a> {
    root: &'a Path,
}

impl<'a> AtomicManifestFile<'a> {
    fn new(root: &'a Path) -> Self {
        Self { root }
    }

    fn replace(&self, bytes: &[u8]) -> Result<(), CacheError> {
        fs_err::create_dir_all(self.root)?;
        let current = self.root.join(ModuleTransformCacheStore::MANIFEST_FILE);
        let previous = self
            .root
            .join(ModuleTransformCacheStore::PREVIOUS_MANIFEST_FILE);
        let temporary = AtomicCacheFile::new(&current).temporary_path();
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;

        if current.is_file() {
            if self.current_is_decodable(&current) {
                if previous.is_file() {
                    fs_err::remove_file(&previous)?;
                }
                fs_err::rename(&current, &previous)?;
            } else {
                fs_err::remove_file(&current)?;
            }
        }
        if let Err(error) = fs_err::rename(&temporary, &current) {
            if previous.is_file() && !current.exists() {
                let _ = fs_err::rename(&previous, &current);
            }
            return Err(error.into());
        }
        let _ = fs_err::remove_file(previous);
        Ok(())
    }

    fn current_is_decodable(&self, current: &Path) -> bool {
        fs_err::read(current)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<ModuleTransformCacheManifest>(&bytes).ok())
            .is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(
        id: &str,
        input: &str,
        snapshot: &str,
    ) -> ModuleTransformCacheRecord<serde_json::Value> {
        ModuleTransformCacheRecord {
            module_id: id.to_string(),
            input_hash: input.to_string(),
            source_snapshot_hash: snapshot.to_string(),
            payload: serde_json::json!({ "code": id }),
        }
    }

    #[test]
    fn changed_leaf_invalidates_its_transitive_importers_only() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = ModuleTransformCacheStore::new(temp.path().to_path_buf(), "env".to_string());
        let imports = BTreeMap::from([
            ("entry".to_string(), vec!["feature".to_string()]),
            ("feature".to_string(), vec!["leaf".to_string()]),
            ("unrelated".to_string(), Vec::new()),
        ]);
        let source_hashes = BTreeMap::from([
            ("entry".to_string(), "entry-v1".to_string()),
            ("feature".to_string(), "feature-v1".to_string()),
            ("leaf".to_string(), "leaf-v1".to_string()),
            ("unrelated".to_string(), "unrelated-v1".to_string()),
        ]);
        let records = source_hashes
            .iter()
            .map(|(id, hash)| record(id, hash, hash))
            .collect();
        store
            .commit(ModuleTransformCacheCommit {
                records,
                module_imports: &imports,
                active_module_ids: &source_hashes.keys().cloned().collect::<Vec<_>>(),
                source_snapshot_hashes: &source_hashes,
            })
            .expect("commit");

        let mut changed = source_hashes;
        changed.insert("leaf".to_string(), "leaf-v2".to_string());
        let loaded = store
            .load::<serde_json::Value>(&changed)
            .expect("load")
            .into_records();
        assert_eq!(loaded.keys().cloned().collect::<Vec<_>>(), ["unrelated"]);
    }

    #[test]
    fn environment_change_discards_all_transform_artifacts() {
        let temp = tempfile::tempdir().expect("tempdir");
        let hashes = BTreeMap::from([("entry".to_string(), "v1".to_string())]);
        ModuleTransformCacheStore::new(temp.path().to_path_buf(), "env-v1".to_string())
            .commit(ModuleTransformCacheCommit {
                records: vec![record("entry", "v1", "v1")],
                module_imports: &BTreeMap::new(),
                active_module_ids: &["entry".to_string()],
                source_snapshot_hashes: &hashes,
            })
            .expect("commit");
        let loaded =
            ModuleTransformCacheStore::new(temp.path().to_path_buf(), "env-v2".to_string())
                .load::<serde_json::Value>(&hashes)
                .expect("load");
        assert!(loaded.into_records().is_empty());
    }

    #[test]
    fn previous_manifest_recovers_interrupted_atomic_commit() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = ModuleTransformCacheStore::new(temp.path().to_path_buf(), "env".to_string());
        let hashes = BTreeMap::from([("entry".to_string(), "v1".to_string())]);
        store
            .commit(ModuleTransformCacheCommit {
                records: vec![record("entry", "v1", "v1")],
                module_imports: &BTreeMap::new(),
                active_module_ids: &["entry".to_string()],
                source_snapshot_hashes: &hashes,
            })
            .expect("commit");
        fs_err::rename(
            temp.path().join(ModuleTransformCacheStore::MANIFEST_FILE),
            temp.path()
                .join(ModuleTransformCacheStore::PREVIOUS_MANIFEST_FILE),
        )
        .expect("simulate interrupted commit");
        assert!(store
            .load::<serde_json::Value>(&hashes)
            .expect("recover previous manifest")
            .into_records()
            .contains_key("entry"));
    }

    #[test]
    fn corrupt_artifact_is_a_miss_without_invalidating_the_manifest() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = ModuleTransformCacheStore::new(temp.path().to_path_buf(), "env".to_string());
        let hashes = BTreeMap::from([("entry".to_string(), "v1".to_string())]);
        store
            .commit(ModuleTransformCacheCommit {
                records: vec![record("entry", "v1", "v1")],
                module_imports: &BTreeMap::new(),
                active_module_ids: &["entry".to_string()],
                source_snapshot_hashes: &hashes,
            })
            .expect("commit");
        let manifest = store
            .load_manifest()
            .expect("read manifest")
            .expect("manifest");
        let artifact = manifest.modules["entry"]
            .artifact
            .as_deref()
            .expect("artifact path");
        fs_err::write(temp.path().join(artifact), b"not-json").expect("corrupt artifact");

        assert!(store
            .load::<serde_json::Value>(&hashes)
            .expect("corrupt artifact degrades to miss")
            .into_records()
            .is_empty());
    }

    #[test]
    fn malformed_current_manifest_recovers_previous_commit() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = ModuleTransformCacheStore::new(temp.path().to_path_buf(), "env".to_string());
        let hashes = BTreeMap::from([("entry".to_string(), "v1".to_string())]);
        store
            .commit(ModuleTransformCacheCommit {
                records: vec![record("entry", "v1", "v1")],
                module_imports: &BTreeMap::new(),
                active_module_ids: &["entry".to_string()],
                source_snapshot_hashes: &hashes,
            })
            .expect("commit");
        let current = temp.path().join(ModuleTransformCacheStore::MANIFEST_FILE);
        let previous = temp
            .path()
            .join(ModuleTransformCacheStore::PREVIOUS_MANIFEST_FILE);
        fs_err::copy(&current, &previous).expect("retain previous commit");
        fs_err::write(&current, b"not-json").expect("corrupt current manifest");

        assert!(store
            .load::<serde_json::Value>(&hashes)
            .expect("recover previous manifest")
            .into_records()
            .contains_key("entry"));
    }
}
