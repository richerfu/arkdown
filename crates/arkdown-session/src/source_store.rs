//! Source file store.
//!
//! [`SourceStore`] is built once during session startup and provides cached
//! access to source file contents.  It tracks all source files inventoried
//! during startup and provides content fingerprints for cache invalidation.
//!
//! # Upstream correspondence
//!
//! | Type | hvigor/SDK equivalent |
//! |------|-----------------------|
//! | `SourceStore` | `arkts-pack.js` input object / source inventories |
//! | `SourceFile` | per-file entry in `compileEntry`/`otherCompileFiles` |
//! | `SourceFingerprint` | FNV-1a or SHA-256 content hash for cache |
//! | `CodeResourceInventory` | copy code resource inventory |
//! | `VisualSourceInventory` | `.visual` file to `.ets` association |
//! | `WorkerSourceInventory` | `loadWorker`/`readWorkerFile` inventories |

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use arkdown_path::{canonicalize_or_owned, lexical_normalize, HvigorSourceFileFilter};
use rayon::prelude::*;
use rustc_hash::FxHashMap;

use arkdown_core::SourceFileId;

/// Whether `ext` (without the leading dot) belongs to a source-inventory
/// file.
///
/// Must stay in sync with the watch trigger set in
/// `arkdown-watcher::invalidation::is_source_ext` so that every file the
/// watcher can invalidate on is also captured by the source store and the
/// project fingerprint inventory.
pub(crate) fn is_source_inventory_ext(ext: &str) -> bool {
    matches!(
        ext,
        "ets" | "ts" | "js" | "mjs" | "cjs" | "json" | "json5" | "visual"
    )
}

// ---------------------------------------------------------------------------
// SourceFingerprint
// ---------------------------------------------------------------------------

/// Content fingerprint for a source file.
///
/// Used to determine whether a source file has changed since the last build,
/// without reading the file again.
///
/// # Upstream correspondence
///
/// Corresponds to the `useSourceHash` fingerprinting in
/// `invalid-cache-plugin.js`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFingerprint {
    /// FNV-1a 16-char hex hash of the file content.
    pub content_hash: String,
    /// File size in bytes.
    pub size: u64,
}

// ---------------------------------------------------------------------------
// SourceContent
// ---------------------------------------------------------------------------

/// The text or binary content of a source file.
#[derive(Debug, Clone)]
pub enum SourceContent {
    /// UTF-8 text source (`.ets`, `.ts`, `.js`, `.json`).
    Text(Arc<str>),
    /// Binary source (`.png`, `.jpg`, resource files).
    Bytes(Arc<Vec<u8>>),
}

// ---------------------------------------------------------------------------
// SourceFile
// ---------------------------------------------------------------------------

/// A source file entry in the [`SourceStore`].
///
/// # Upstream correspondence
///
/// Corresponds to a single entry in `arkts-pack.js` input object
/// `compileFiles` / `otherCompileFiles`.
#[derive(Debug, Clone)]
pub struct SourceFile {
    /// Stable source file ID.
    pub id: SourceFileId,
    /// Absolute path to the source file.
    pub path: PathBuf,
    /// Content fingerprint (computed at store construction time).
    pub fingerprint: SourceFingerprint,
    /// Modification time captured with the source bytes during startup.
    ///
    /// Hvigor exposes this through `share.getCachedFileInfo()`. Keeping it on
    /// the immutable source record avoids a second `stat` in plugin hooks.
    pub modified_millis: Option<u64>,
    /// Content captured during session startup.
    content: Option<SourceContent>,
}

impl SourceFile {
    /// Return the content captured by the startup inventory.
    pub fn cached_content(&self) -> Option<&SourceContent> {
        self.content.as_ref()
    }
}

// ---------------------------------------------------------------------------
// CodeResourceInventory
// ---------------------------------------------------------------------------

/// Inventory of code resources to be copied to the output.
///
/// Code resources include non-ETS/TS files that are part of the module source
/// tree (e.g., worker JS files, config JSON files).
///
/// # Upstream correspondence
///
/// Corresponds to `copy_resources_util.js` and the source files that need
/// to be copied alongside the ABC output.
#[derive(Debug, Clone, Default)]
pub struct CodeResourceInventory {
    /// Map from source path → target relative path.
    pub files: BTreeMap<PathBuf, String>,
}

impl CodeResourceInventory {
    /// Return the number of entries.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Return `true` if there are no entries.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

// ---------------------------------------------------------------------------
// VisualSourceInventory
// ---------------------------------------------------------------------------

/// Inventory of `.visual` preview files and their associated `.ets` sources.
///
/// Visual files are used by the IDE for live preview.  Each `.visual` file
/// is associated with the `.ets` source file that defines the component.
///
/// # Upstream correspondence
///
/// Corresponds to `rollup-plugin-visual.js` visual file input and
/// `process_visual.js` transform.
#[derive(Debug, Clone, Default)]
pub struct VisualSourceInventory {
    /// Map from `.ets` source path → `.visual` file path.
    pub ets_to_visual: BTreeMap<PathBuf, PathBuf>,
}

impl VisualSourceInventory {
    /// Return the `.visual` file for a given `.ets` source, if one exists.
    pub fn visual_for_source(&self, ets_path: &std::path::Path) -> Option<&PathBuf> {
        self.ets_to_visual.get(ets_path)
    }

    /// Return all `.ets` sources that have associated `.visual` files.
    pub fn visual_files_for_source(&self, ets_path: &std::path::Path) -> Vec<&PathBuf> {
        self.ets_to_visual
            .get(ets_path)
            .map(|v| vec![v])
            .unwrap_or_default()
    }

    /// Return all source/Visual associations in deterministic path order.
    pub fn sources(&self) -> impl Iterator<Item = (&Path, &Path)> {
        self.ets_to_visual
            .iter()
            .map(|(source, visual)| (source.as_path(), visual.as_path()))
    }

    fn insert(&mut self, visual_path: &std::path::Path) {
        let mut source_path = PathBuf::new();
        let mut replaced = false;
        for component in visual_path.components() {
            if !replaced && component.as_os_str() == "supervisual" {
                source_path.push("ets");
                replaced = true;
            } else {
                source_path.push(component.as_os_str());
            }
        }
        if replaced {
            source_path.set_extension("ets");
            self.ets_to_visual
                .insert(source_path, visual_path.to_path_buf());
        }
    }
}

// ---------------------------------------------------------------------------
// WorkerSourceInventory
// ---------------------------------------------------------------------------

/// Inventory of worker entry points.
///
/// Worker entries are compiled separately from the main bundle.  This
/// inventory tracks which files are worker entries and their relative paths.
///
/// # Upstream correspondence
///
/// Corresponds to `main.js::loadWorker/readWorkerFile/filterWorker` in the
/// SDK and `process-oh-package-json.js::addWorkerData`.
#[derive(Debug, Clone, Default)]
pub struct WorkerSourceInventory {
    /// List of worker entry source paths.
    pub workers: Vec<PathBuf>,
}

impl WorkerSourceInventory {
    /// Return all worker entry paths.
    pub fn worker_entries_for_module(&self) -> &[PathBuf] {
        &self.workers
    }
}

// ---------------------------------------------------------------------------
// SourceStore
// ---------------------------------------------------------------------------

/// Immutable source file inventory built once at session startup.
///
/// The `SourceStore` provides read-only access to all source files known to
/// the build session. Content and fingerprints are captured together during
/// `discover()`, so later transform and plugin stages never re-read a source
/// through this store.
///
/// # Upstream correspondence
///
/// Corresponds to the input object assembled in `arkts-pack.js` from
/// `compileEntry`, `otherCompileFiles`, `loadWorker`, and resource files.
#[derive(Debug)]
pub struct SourceStore {
    /// Map from absolute path → source file entry.
    files: FxHashMap<PathBuf, SourceFile>,
    /// Code resource inventory.
    code_resources: CodeResourceInventory,
    /// Visual file inventory.
    visuals: VisualSourceInventory,
    /// Worker entry inventory.
    workers: WorkerSourceInventory,
    /// Canonical roots covered by this immutable inventory.
    inventory_roots: Arc<[PathBuf]>,
    /// Lexical caller roots mapped to their canonical startup identities.
    root_aliases: Arc<[(PathBuf, PathBuf)]>,
}

impl SourceStore {
    /// Discover and fingerprint all source files in `source_roots`.
    ///
    /// This performs the initial scan and source reads at session startup.
    /// Paths are sorted before dense IDs are assigned, so discovery order and
    /// rayon scheduling cannot affect later output.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `compileEntry`/`otherCompileFiles` inventory in
    /// `GenerateLoaderJson` and `arkts-pack.js::init_config`.
    pub fn discover(source_roots: &[PathBuf]) -> Result<Self, std::io::Error> {
        let mut paths = BTreeSet::new();
        let mut visuals = VisualSourceInventory::default();

        let mut requested_roots = source_roots
            .iter()
            .map(|root| lexical_normalize(root))
            .collect::<std::collections::BTreeSet<_>>();
        for root in source_roots {
            if root.file_name().is_some_and(|name| name == "ets") {
                if let Some(src_main) = root.parent() {
                    let visual_root = src_main.join("supervisual");
                    if visual_root.is_dir() {
                        requested_roots.insert(lexical_normalize(&visual_root));
                    }
                }
            }
        }
        let mut root_aliases = requested_roots
            .into_iter()
            .map(|root| {
                let canonical = canonicalize_or_owned(&root);
                (root, canonical)
            })
            .collect::<Vec<_>>();
        root_aliases.sort_by(|left, right| {
            right
                .0
                .components()
                .count()
                .cmp(&left.0.components().count())
                .then_with(|| left.0.cmp(&right.0))
        });
        let inventory_roots = root_aliases
            .iter()
            .map(|(_, canonical)| canonical.clone())
            .collect::<BTreeSet<_>>();

        for root in &inventory_roots {
            if !root.exists() {
                continue;
            }
            for entry in walkdir::WalkDir::new(root.as_path())
                .follow_links(false)
                .into_iter()
                .filter_entry(|entry| !Self::is_generated_or_external_dir(entry.path(), root))
            {
                let entry = entry.map_err(|error| {
                    Error::other(format!(
                        "scan source inventory {}: {error}",
                        error.path().unwrap_or(root).display()
                    ))
                })?;
                if !entry.file_type().is_file() {
                    continue;
                }
                let path = canonicalize_or_owned(entry.path());
                if HvigorSourceFileFilter::new(&path).is_ignored_macos_metadata() {
                    continue;
                }
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if !is_source_inventory_ext(ext) {
                    continue;
                }
                if ext == "visual" {
                    visuals.insert(&path);
                }
                paths.insert(path);
            }
        }

        let paths = paths.into_iter().collect::<Vec<_>>();
        let contents = paths
            .par_iter()
            .map(|path| {
                let metadata = std::fs::metadata(path).map_err(|source| {
                    Error::new(
                        source.kind(),
                        format!("read metadata {}: {source}", path.display()),
                    )
                })?;
                let modified_millis = metadata
                    .modified()
                    .ok()
                    .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
                    .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64);
                let content = std::fs::read_to_string(path)
                    .map(Arc::<str>::from)
                    .map_err(|source| {
                        Error::new(source.kind(), format!("read {}: {source}", path.display()))
                    })?;
                Ok::<(Arc<str>, Option<u64>), std::io::Error>((content, modified_millis))
            })
            .collect::<Vec<_>>();
        let mut files = FxHashMap::default();
        for (index, (path, content)) in paths.into_iter().zip(contents).enumerate() {
            let (content, modified_millis) = content?;
            let size = content.len() as u64;
            let content_hash = format!("{:016x}", arkdown_core::fnv1a64(content.as_bytes()));
            files.insert(
                path.clone(),
                SourceFile {
                    id: SourceFileId::new(index),
                    path,
                    fingerprint: SourceFingerprint { content_hash, size },
                    modified_millis,
                    content: Some(SourceContent::Text(content)),
                },
            );
        }

        Ok(Self {
            files,
            code_resources: CodeResourceInventory::default(),
            visuals,
            workers: WorkerSourceInventory::default(),
            inventory_roots: Arc::from(inventory_roots.into_iter().collect::<Vec<_>>()),
            root_aliases: Arc::from(root_aliases),
        })
    }

    fn is_generated_or_external_dir(path: &Path, root: &Path) -> bool {
        path != root
            && matches!(
                path.file_name().and_then(|value| value.to_str()),
                Some(".git" | "build" | "out" | "node_modules" | "oh_modules")
            )
    }

    /// Whether `root` is wholly covered by one of the startup inventory roots.
    pub fn covers(&self, root: &Path) -> bool {
        let root = self.lookup_path(root);
        self.inventory_roots
            .iter()
            .any(|inventory_root| root.starts_with(inventory_root))
    }

    /// Return startup-captured text sources below `root` in path order.
    pub fn text_sources_under(&self, root: &Path) -> Vec<(&Path, Arc<str>)> {
        let root = self.lookup_path(root);
        let mut sources = self
            .files
            .values()
            .filter(|file| file.path.starts_with(&root))
            .filter_map(|file| match file.cached_content() {
                Some(SourceContent::Text(content)) => {
                    Some((file.path.as_path(), Arc::clone(content)))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        sources.sort_by(|left, right| left.0.cmp(right.0));
        sources
    }

    /// Return the source file entry for `path`, if it's in the store.
    pub fn source_by_path(&self, path: &std::path::Path) -> Option<&SourceFile> {
        self.files.get(&self.lookup_path(path))
    }

    /// Return the startup source inventory in stable path order.
    ///
    /// Callers receive references into the immutable store; source text is
    /// never cloned merely to expose file metadata to another build stage.
    pub fn source_files(&self) -> Vec<&SourceFile> {
        let mut files = self.files.values().collect::<Vec<_>>();
        files.sort_unstable_by(|left, right| left.path.cmp(&right.path));
        files
    }

    fn lookup_path(&self, path: &Path) -> PathBuf {
        let path = lexical_normalize(path);
        if self.files.contains_key(&path) || self.inventory_roots.iter().any(|root| root == &path) {
            return path;
        }
        for (alias, canonical) in self.root_aliases.iter() {
            if let Ok(relative) = path.strip_prefix(alias) {
                return lexical_normalize(&canonical.join(relative));
            }
        }
        path
    }

    /// Return the startup-captured text content of a source file.
    pub fn read_text(&self, path: &std::path::Path) -> Result<Arc<str>, std::io::Error> {
        let path = self.lookup_path(path);
        match self.files.get(&path).and_then(SourceFile::cached_content) {
            Some(SourceContent::Text(content)) => Ok(Arc::clone(content)),
            Some(SourceContent::Bytes(_)) => Err(Error::new(
                ErrorKind::InvalidData,
                format!("source is not UTF-8 text: {}", path.display()),
            )),
            None => Err(Error::new(
                ErrorKind::NotFound,
                format!(
                    "source was not captured at session startup: {}",
                    path.display()
                ),
            )),
        }
    }

    /// Read the raw bytes of a source file.
    pub fn read_bytes(&self, path: &std::path::Path) -> Result<Arc<Vec<u8>>, std::io::Error> {
        let path = self.lookup_path(path);
        match self.files.get(&path).and_then(SourceFile::cached_content) {
            Some(SourceContent::Bytes(content)) => Ok(Arc::clone(content)),
            Some(SourceContent::Text(content)) => Ok(Arc::new(content.as_bytes().to_vec())),
            None => Err(Error::new(
                ErrorKind::NotFound,
                format!(
                    "source was not captured at session startup: {}",
                    path.display()
                ),
            )),
        }
    }

    /// Return the fingerprint for a source file.
    ///
    /// Returns `None` if the file is not in the store.
    pub fn fingerprint(&self, path: &std::path::Path) -> Option<&SourceFingerprint> {
        self.files
            .get(&self.lookup_path(path))
            .map(|f| &f.fingerprint)
    }

    /// Return the code resource inventory.
    pub fn code_resources_for_module(&self) -> &CodeResourceInventory {
        &self.code_resources
    }

    /// Return the visual file inventory.
    pub fn visual_files_for_source(&self, ets_path: &std::path::Path) -> Vec<&PathBuf> {
        self.visuals
            .visual_files_for_source(&self.lookup_path(ets_path))
    }

    /// Return startup-captured Visual associations in deterministic order.
    pub fn visual_sources(&self) -> impl Iterator<Item = (&Path, &Path)> {
        self.visuals.sources()
    }

    /// Return the worker entry inventory.
    pub fn worker_entries_for_module(&self) -> &[PathBuf] {
        self.workers.worker_entries_for_module()
    }

    /// Return the total number of source files in the store.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Return `true` if the store has no files.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_store_from_nonexistent_root() {
        let store = SourceStore::discover(&[std::path::PathBuf::from("/nonexistent/path")])
            .expect("discover should not fail for nonexistent path");
        assert!(store.is_empty());
    }

    #[test]
    fn discover_skips_hvigor_ignored_macos_metadata_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source_dir = temp.path().join("entry/src/main/ets");
        std::fs::create_dir_all(&source_dir).expect("create source dir");
        let real_source = source_dir.join("Index.ets");
        let apple_double = source_dir.join("._BuildProfile.ets");
        let ds_store = source_dir.join(".DS_Store");
        std::fs::write(&real_source, "export const value = 1;").expect("write source");
        std::fs::write(&apple_double, "metadata").expect("write apple double");
        std::fs::write(&ds_store, "metadata").expect("write ds store");

        let store = SourceStore::discover(&[temp.path().to_path_buf()]).expect("discover");

        assert_eq!(store.len(), 1);
        assert!(store.source_by_path(&real_source).is_some());
        assert!(store.source_by_path(&apple_double).is_none());
        assert!(store.source_by_path(&ds_store).is_none());
    }

    #[test]
    fn discover_indexes_sibling_supervisual_inputs_for_ets_sources() {
        let temp = tempfile::tempdir().expect("tempdir");
        let src_main = temp.path().join("entry/src/main");
        let source_dir = src_main.join("ets/pages");
        let visual_dir = src_main.join("supervisual/pages");
        std::fs::create_dir_all(&source_dir).expect("create source dir");
        std::fs::create_dir_all(&visual_dir).expect("create visual dir");
        let source = source_dir.join("Index.ets");
        let visual = visual_dir.join("Index.visual");
        std::fs::write(&source, "@Component struct Index { build() {} }").expect("write source");
        std::fs::write(&visual, "{}").expect("write visual");

        let store = SourceStore::discover(&[src_main.join("ets")]).expect("discover");

        assert_eq!(store.len(), 2);
        let canonical_visual = &store.source_by_path(&visual).expect("visual source").path;
        assert_eq!(
            store.visual_files_for_source(&source),
            vec![canonical_visual]
        );
        assert!(store.fingerprint(&visual).is_some());
    }

    #[test]
    fn source_content_and_hash_are_frozen_at_session_startup() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source_dir = temp.path().join("entry/src/main/ets");
        std::fs::create_dir_all(&source_dir).expect("create source dir");
        let source = source_dir.join("Index.ets");
        std::fs::write(&source, "export const value = 1;").expect("write source");

        let store = SourceStore::discover(std::slice::from_ref(&source_dir)).expect("discover");
        let frozen_hash = store
            .fingerprint(&source)
            .expect("fingerprint")
            .content_hash
            .clone();
        std::fs::write(&source, "export const value = 2;").expect("mutate source after startup");

        assert_eq!(
            store.read_text(&source).expect("cached source").as_ref(),
            "export const value = 1;"
        );
        assert_eq!(
            frozen_hash,
            format!("{:016x}", arkdown_core::fnv1a64(b"export const value = 1;"))
        );
    }
}
