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

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use arkdown_path::{canonicalize_or_owned, lexical_normalize, HvigorSourceFileFilter};
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};

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
    /// TypeScript canonical file name → startup snapshot file name.
    ///
    /// `ets_checker.ts::getCanonicalFileName` follows
    /// `ts.sys.useCaseSensitiveFileNames`; the native TypeScript port keeps
    /// the same contract in `tspath.GetCanonicalFileName`. On a
    /// case-insensitive filesystem the Program can therefore report
    /// `index.ets` for an inventoried `Index.ets`. This startup-only index
    /// restores the snapshot spelling without filesystem I/O in checker and
    /// transform hot paths.
    compiler_canonical_files: Option<FxHashMap<PathBuf, PathBuf>>,
    /// Code resource inventory.
    code_resources: CodeResourceInventory,
    /// Visual file inventory.
    visuals: VisualSourceInventory,
    /// Worker entry inventory.
    workers: WorkerSourceInventory,
    /// Canonical roots covered by this immutable inventory.
    inventory_roots: Arc<FxHashSet<PathBuf>>,
    /// Lexical caller roots mapped to their canonical startup identities.
    root_aliases: Arc<SourceRootAliases>,
}

/// Component trie: longest-root matching visits the path once, independent of
/// the number of dependency aliases. Component keys preserve platform spelling.
#[derive(Debug, Default)]
struct SourceRootAliases {
    children: FxHashMap<OsString, SourceRootAliases>,
    canonical: Option<PathBuf>,
}

impl SourceRootAliases {
    fn new(aliases: Vec<(PathBuf, PathBuf)>) -> Self {
        let mut roots = Self::default();
        for (alias, canonical) in aliases {
            let mut node = &mut roots;
            for part in alias.components() {
                node = node
                    .children
                    .entry(part.as_os_str().to_owned())
                    .or_default();
            }
            node.canonical = Some(canonical);
        }
        roots
    }

    fn resolve(&self, path: &Path) -> Option<PathBuf> {
        let mut node = self;
        let mut parts = path.components();
        let mut matched = self.canonical.as_ref().map(|root| (root, path));
        while let Some(part) = parts.next() {
            let Some(child) = node.children.get(part.as_os_str()) else {
                break;
            };
            node = child;
            if let Some(canonical) = &node.canonical {
                matched = Some((canonical, parts.as_path()));
            }
        }
        matched.map(|(canonical, relative)| canonical.join(relative))
    }
}

impl arkdown_core::SourceInventory for SourceStore {
    fn canonical_path(&self, path: &Path) -> Option<&Path> {
        if let Some(file) = self.files.get(path) {
            return Some(&file.path);
        }
        // A parent component after an uncaptured symlink requires realpath;
        // lexical normalization is not evidence of physical identity.
        if path
            .components()
            .any(|part| part == std::path::Component::ParentDir)
        {
            return None;
        }
        let path = self.lookup_path(path);
        self.files
            .get(path.as_ref())
            .map(|file| file.path.as_path())
            .or_else(|| {
                self.inventory_roots
                    .get(path.as_ref())
                    .map(PathBuf::as_path)
            })
    }

    fn source_text(&self, path: &Path) -> Option<Arc<str>> {
        match self
            .files
            .get(self.canonical_path(path)?)?
            .cached_content()?
        {
            SourceContent::Text(text) => Some(Arc::clone(text)),
            SourceContent::Bytes(_) => None,
        }
    }
}

/// Per-startup walk cache. WalkDir's non-followed entry names have the directory's
/// actual spelling; joining them to the physical parent preserves realpath identity.
#[derive(Debug, Default)]
pub(crate) struct SourceInventoryPaths {
    directories: FxHashMap<PathBuf, Option<PathBuf>>,
}

impl SourceInventoryPaths {
    pub(crate) fn canonical_file(&mut self, entry: &walkdir::DirEntry) -> PathBuf {
        // A root file's name came from the caller, rather than directory enumeration.
        if entry.depth() == 0 || entry.file_type().is_symlink() {
            return canonicalize_or_owned(entry.path());
        }
        let Some(parent) = entry.path().parent() else {
            return canonicalize_or_owned(entry.path());
        };
        match self
            .directories
            .entry(parent.to_path_buf())
            .or_insert_with(|| {
                arkdown_path::AbsPath::canonicalize(parent)
                    .ok()
                    .map(arkdown_path::AbsPath::into_path_buf)
            }) {
            Some(parent) => parent.join(entry.file_name()),
            None => canonicalize_or_owned(entry.path()),
        }
    }
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
        Self::discover_with_preinventoried_roots(source_roots, &[])
    }

    /// Discover module roots while reusing dependency path inventories frozen
    /// by [`crate::DependencyGraphSnapshot`].
    ///
    /// Each tuple contains the dependency package root and its complete source
    /// file list. The package root remains part of `covers()`/alias handling,
    /// but is not walked a second time.
    pub(crate) fn discover_with_preinventoried_roots(
        source_roots: &[PathBuf],
        preinventoried_roots: &[(PathBuf, Arc<[PathBuf]>)],
    ) -> Result<Self, std::io::Error> {
        let mut paths = BTreeSet::new();
        let mut visuals = VisualSourceInventory::default();
        let mut inventory_paths = SourceInventoryPaths::default();

        let mut walk_roots = source_roots
            .iter()
            .map(|root| lexical_normalize(root))
            .collect::<std::collections::BTreeSet<_>>();
        for root in source_roots {
            if root.file_name().is_some_and(|name| name == "ets") {
                if let Some(src_main) = root.parent() {
                    let visual_root = src_main.join("supervisual");
                    if visual_root.is_dir() {
                        walk_roots.insert(lexical_normalize(&visual_root));
                    }
                }
            }
        }
        let mut requested_roots = walk_roots.clone();
        requested_roots.extend(
            preinventoried_roots
                .iter()
                .map(|(root, _)| lexical_normalize(root)),
        );
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

        for (_, inventory) in preinventoried_roots {
            for path in inventory.iter() {
                // DependencySnapshotInventory already captured physical file identities.
                let path = path.clone();
                if HvigorSourceFileFilter::new(&path).is_ignored_macos_metadata() {
                    continue;
                }
                let extension = path.extension().and_then(|extension| extension.to_str());
                if !extension.is_some_and(is_source_inventory_ext) {
                    continue;
                }
                if extension == Some("visual") {
                    visuals.insert(&path);
                }
                paths.insert(path);
            }
        }

        let walk_roots = root_aliases
            .iter()
            .filter(|(root, _)| walk_roots.contains(root))
            .map(|(_, canonical)| canonical.clone())
            .collect::<BTreeSet<_>>();
        for root in &walk_roots {
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
                let path = inventory_paths.canonical_file(&entry);
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
        let compiler_canonical_files = (!Self::compiler_uses_case_sensitive_file_names())
            .then(|| Self::compiler_canonical_file_index(files.keys()));

        Ok(Self {
            files,
            compiler_canonical_files,
            code_resources: CodeResourceInventory::default(),
            visuals,
            workers: WorkerSourceInventory::default(),
            inventory_roots: Arc::new(inventory_roots.into_iter().collect()),
            root_aliases: Arc::new(SourceRootAliases::new(root_aliases)),
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
        self.files.get(self.lookup_path(path).as_ref())
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

    fn lookup_path<'a>(&'a self, path: &'a Path) -> Cow<'a, Path> {
        if let Some((canonical, _)) = self.files.get_key_value(path) {
            return Cow::Borrowed(canonical);
        }
        if let Some(root) = self.inventory_roots.get(path) {
            return Cow::Borrowed(root);
        }
        let path = lexical_normalize(path);
        if self.files.contains_key(&path) || self.inventory_roots.contains(&path) {
            return Cow::Owned(path);
        }
        Cow::Owned(self.snapshot_file_spelling(self.root_aliases.resolve(&path).unwrap_or(path)))
    }

    fn snapshot_file_spelling(&self, path: PathBuf) -> PathBuf {
        if self.files.contains_key(&path) {
            return path;
        }
        self.compiler_canonical_files
            .as_ref()
            .and_then(|files| Self::compiler_canonical_path(&path).and_then(|key| files.get(&key)))
            .cloned()
            .unwrap_or(path)
    }

    fn compiler_canonical_file_index<'a>(
        files: impl Iterator<Item = &'a PathBuf>,
    ) -> FxHashMap<PathBuf, PathBuf> {
        let mut index = FxHashMap::default();
        let mut ambiguous = FxHashSet::default();
        for path in files {
            let Some(key) = Self::compiler_canonical_path(path) else {
                continue;
            };
            if ambiguous.contains(&key) {
                continue;
            }
            if index.get(&key).is_some_and(|existing| existing != path) {
                index.remove(&key);
                ambiguous.insert(key);
            } else {
                index.insert(key, path.clone());
            }
        }
        index
    }

    fn compiler_canonical_path(path: &Path) -> Option<PathBuf> {
        let path = path.to_str()?;
        let mut canonical = String::with_capacity(path.len());
        for character in path.chars() {
            if character == '\u{0130}' {
                canonical.push(character);
            } else {
                canonical.extend(character.to_lowercase());
            }
        }
        Some(PathBuf::from(canonical))
    }

    fn compiler_uses_case_sensitive_file_names() -> bool {
        if cfg!(windows) {
            return false;
        }
        if cfg!(target_arch = "wasm32") {
            return true;
        }
        let Ok(executable) = std::env::current_exe() else {
            return true;
        };
        let Some(executable) = executable.to_str() else {
            return true;
        };
        let swapped = executable
            .chars()
            .map(|character| {
                let mut uppercase = character.to_uppercase();
                let upper = uppercase.next().unwrap_or(character);
                let simple_upper = uppercase.next().is_none().then_some(upper);
                if simple_upper.is_some_and(|upper| upper != character) {
                    upper
                } else {
                    let mut lowercase = character.to_lowercase();
                    let lower = lowercase.next().unwrap_or(character);
                    if lowercase.next().is_none() {
                        lower
                    } else {
                        character
                    }
                }
            })
            .collect::<String>();
        !Path::new(&swapped).exists()
    }

    /// Return the startup-captured text content of a source file.
    pub fn read_text(&self, path: &std::path::Path) -> Result<Arc<str>, std::io::Error> {
        let path = self.lookup_path(path);
        match self
            .files
            .get(path.as_ref())
            .and_then(SourceFile::cached_content)
        {
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
        match self
            .files
            .get(path.as_ref())
            .and_then(SourceFile::cached_content)
        {
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
            .get(self.lookup_path(path).as_ref())
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

    #[test]
    fn preinventoried_dependency_root_is_not_walked_again() {
        let temp = tempfile::tempdir().unwrap();
        let dependency_root = temp.path().join("dependency");
        std::fs::create_dir_all(&dependency_root).unwrap();
        let captured = dependency_root.join("captured.ets");
        let excluded = dependency_root.join("excluded.ets");
        std::fs::write(&captured, "export const captured = true;").unwrap();
        std::fs::write(&excluded, "export const excluded = true;").unwrap();

        let store = SourceStore::discover_with_preinventoried_roots(
            &[],
            &[(
                dependency_root.clone(),
                Arc::from([canonicalize_or_owned(&captured)]),
            )],
        )
        .unwrap();

        assert!(store.covers(&dependency_root));
        assert!(store.source_by_path(&captured).is_some());
        assert!(store.source_by_path(&excluded).is_none());
    }

    #[test]
    fn compiler_canonical_file_index_restores_snapshot_spelling_without_ambiguity() {
        let actual = PathBuf::from("/project/ShopCartWidget/Index.ets");
        let paths = [actual.clone()];
        let index = SourceStore::compiler_canonical_file_index(paths.iter());
        let compiler_path = PathBuf::from("/project/shopcartwidget/index.ets");
        let key = SourceStore::compiler_canonical_path(&compiler_path).unwrap();

        assert_eq!(index.get(&key), Some(&actual));

        let colliding = PathBuf::from("/project/ShopCartWidget/index.ets");
        let paths = [actual, colliding];
        let index = SourceStore::compiler_canonical_file_index(paths.iter());
        assert!(!index.contains_key(&key));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn source_lookup_matches_typescript_program_case_on_case_insensitive_macos() {
        if SourceStore::compiler_uses_case_sensitive_file_names() {
            return;
        }
        let temp = tempfile::tempdir().expect("tempdir");
        let source_dir = temp.path().join("ShopCartWidget");
        std::fs::create_dir_all(&source_dir).expect("create source dir");
        let actual = source_dir.join("Index.ets");
        std::fs::write(&actual, "export const value = 1;").expect("write source");
        let store = SourceStore::discover(std::slice::from_ref(&source_dir)).expect("discover");
        let compiler_path = source_dir.join("index.ets");

        assert_eq!(
            store
                .source_by_path(&compiler_path)
                .expect("TypeScript canonical path")
                .path,
            store.source_by_path(&actual).expect("actual path").path
        );
    }
    #[test]
    fn alias_trie_matches_longest_prefix_scan() {
        let mut aliases = vec![(PathBuf::new(), PathBuf::from("fallback"))];
        for n in 0..128 {
            aliases.push((
                PathBuf::from(format!("/root/pkg{n}")),
                PathBuf::from(format!("/physical/{n}")),
            ));
            aliases.push((
                PathBuf::from(format!("/root/pkg{n}/nested")),
                PathBuf::from(format!("/nested/{n}")),
            ));
        }
        aliases.sort_by_key(|(alias, _)| std::cmp::Reverse(alias.components().count()));
        let trie = SourceRootAliases::new(aliases.clone());
        for path in [
            "/root/pkg12/a.ets",
            "/root/pkg12/nested/a.ets",
            "/root/pkg1234/a.ets",
            "/outside/a.ets",
            "relative.ets",
            "",
        ] {
            let path = Path::new(path);
            let scanned = aliases.iter().find_map(|(alias, canonical)| {
                path.strip_prefix(alias)
                    .ok()
                    .map(|relative| canonical.join(relative))
            });
            assert_eq!(trie.resolve(path), scanned, "{}", path.display());
        }
    }

    #[cfg(unix)]
    #[test]
    fn nested_root_aliases_choose_longest_component_prefix_without_disk_reads() {
        use arkdown_core::SourceInventory;
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let outer = temp.path().join("Outer");
        let inner = temp.path().join("Inner");
        std::fs::create_dir(&outer).unwrap();
        std::fs::create_dir(&inner).unwrap();
        std::fs::write(outer.join("Same.ets"), "outer").unwrap();
        std::fs::write(inner.join("Same.ets"), "inner").unwrap();
        symlink(&inner, outer.join("nested")).unwrap();
        let alias = temp.path().join("Alias");
        symlink(&outer, &alias).unwrap();
        let store = SourceStore::discover(&[alias.clone(), alias.join("nested")]).unwrap();
        let expected = std::fs::canonicalize(inner.join("Same.ets")).unwrap();
        std::fs::remove_file(&alias).unwrap();
        assert_eq!(
            store.read_text(&alias.join("Same.ets")).unwrap().as_ref(),
            "outer"
        );
        assert_eq!(
            store
                .read_text(&alias.join("nested/Same.ets"))
                .unwrap()
                .as_ref(),
            "inner"
        );
        assert_eq!(
            store.canonical_path(&alias.join("nested/Same.ets")),
            Some(expected.as_path())
        );
        assert!(store
            .source_by_path(&temp.path().join("AliasOther/Same.ets"))
            .is_none());
        assert!(store
            .canonical_path(&alias.join("nested/../Same.ets"))
            .is_none());
    }

    #[cfg(unix)]
    #[test]
    fn directory_identity_matches_realpath_for_roots_files_and_symlinks() {
        use arkdown_core::SourceInventory;
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("ActualPackage");
        std::fs::create_dir(&root).unwrap();
        let file = root.join("MixedCase.ets");
        std::fs::write(&file, "export const value = 1;\r\n").unwrap();
        symlink(&file, root.join("linked.ets")).unwrap();
        let alias = temp.path().join("alias");
        symlink(&root, &alias).unwrap();
        let mut paths = SourceInventoryPaths::default();
        for entry in walkdir::WalkDir::new(&alias).follow_links(false) {
            let entry = entry.unwrap();
            assert_eq!(
                paths.canonical_file(&entry),
                canonicalize_or_owned(entry.path())
            );
        }
        let store = SourceStore::discover(std::slice::from_ref(&alias)).unwrap();
        let canonical = std::fs::canonicalize(&file).unwrap();
        assert_eq!(
            store.files.len(),
            1,
            "nested links remain excluded from discovery"
        );
        std::fs::remove_file(&file).unwrap();
        assert_eq!(
            store.canonical_path(&alias.join("MixedCase.ets")),
            Some(canonical.as_path())
        );
        assert_eq!(
            store.source_text(&canonical).unwrap().as_ref(),
            "export const value = 1;\r\n"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn directory_walk_preserves_realpath_case_from_case_insensitive_root() {
        if SourceStore::compiler_uses_case_sensitive_file_names() {
            return;
        }
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("ActualPackage");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("MixedCase.ets"), "export {};").unwrap();
        let mut paths = SourceInventoryPaths::default();
        let wrong_case_root = temp.path().join("actualpackage");
        for entry in walkdir::WalkDir::new(wrong_case_root) {
            let entry = entry.unwrap();
            assert_eq!(
                paths.canonical_file(&entry),
                canonicalize_or_owned(entry.path())
            );
        }
    }
}
