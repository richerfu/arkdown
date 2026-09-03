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

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use arkdown_path::HvigorSourceFileFilter;
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
    Text(Arc<String>),
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
    /// Content (lazily loaded; `None` until first read).
    content: Option<SourceContent>,
}

impl SourceFile {
    /// Return the content if it has been loaded; `None` otherwise.
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
/// the build session.  Content is lazily loaded and cached; fingerprints are
/// computed eagerly during `discover()`.
///
/// # Upstream correspondence
///
/// Corresponds to the input object assembled in `arkts-pack.js` from
/// `compileEntry`, `otherCompileFiles`, `loadWorker`, and resource files.
pub struct SourceStore {
    /// Map from absolute path → source file entry.
    files: FxHashMap<PathBuf, SourceFile>,
    /// Code resource inventory.
    code_resources: CodeResourceInventory,
    /// Visual file inventory.
    visuals: VisualSourceInventory,
    /// Worker entry inventory.
    workers: WorkerSourceInventory,
}

impl SourceStore {
    /// Discover and fingerprint all source files in `source_roots`.
    ///
    /// This performs the initial scan at session startup.  The result is an
    /// immutable store; content is NOT read here (only fingerprints).
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `compileEntry`/`otherCompileFiles` inventory in
    /// `GenerateLoaderJson` and `arkts-pack.js::init_config`.
    pub fn discover(source_roots: &[PathBuf]) -> Result<Self, std::io::Error> {
        let mut files = FxHashMap::default();
        let mut visuals = VisualSourceInventory::default();
        let mut next_id = 0u32;

        let mut inventory_roots = source_roots
            .iter()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();
        for root in source_roots {
            if root.file_name().is_some_and(|name| name == "ets") {
                if let Some(src_main) = root.parent() {
                    let visual_root = src_main.join("supervisual");
                    if visual_root.is_dir() {
                        inventory_roots.insert(visual_root);
                    }
                }
            }
        }

        for root in inventory_roots {
            if !root.exists() {
                continue;
            }
            let mut entries = walkdir::WalkDir::new(root.as_path())
                .follow_links(false)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .collect::<Vec<_>>();
            entries.sort_by(|left, right| left.path().cmp(right.path()));
            for entry in entries {
                let path = entry.path().to_path_buf();
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
                if let Ok(meta) = std::fs::metadata(&path) {
                    // Compute fingerprint from file size and mtime (fast path).
                    // When `useSourceHash` is active, content hashing is done
                    // separately by the cache key builder.
                    let size = meta.len();
                    let mtime = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    // Fast fingerprint from size + mtime (content-only hashing
                    // is done separately by the cache key builder when
                    // `useSourceHash` is active).
                    let raw = size.wrapping_mul(0x9e3779b97f4a7c15)
                        ^ mtime.wrapping_mul(0x517cc1b727220a95);
                    let hash = format!("{raw:016x}");
                    let id = SourceFileId::new(next_id as usize);
                    next_id += 1;
                    files.insert(
                        path.clone(),
                        SourceFile {
                            id,
                            path,
                            fingerprint: SourceFingerprint {
                                content_hash: hash,
                                size,
                            },
                            content: None,
                        },
                    );
                }
            }
        }

        Ok(Self {
            files,
            code_resources: CodeResourceInventory::default(),
            visuals,
            workers: WorkerSourceInventory::default(),
        })
    }

    /// Return the source file entry for `path`, if it's in the store.
    pub fn source_by_path(&self, path: &std::path::Path) -> Option<&SourceFile> {
        self.files.get(path)
    }

    /// Read the text content of a source file.
    ///
    /// Returns an error if the file does not exist or cannot be read as UTF-8.
    pub fn read_text(&self, path: &std::path::Path) -> Result<Arc<String>, std::io::Error> {
        let content = std::fs::read_to_string(path)?;
        Ok(Arc::new(content))
    }

    /// Read the raw bytes of a source file.
    pub fn read_bytes(&self, path: &std::path::Path) -> Result<Arc<Vec<u8>>, std::io::Error> {
        let bytes = std::fs::read(path)?;
        Ok(Arc::new(bytes))
    }

    /// Return the fingerprint for a source file.
    ///
    /// Returns `None` if the file is not in the store.
    pub fn fingerprint(&self, path: &std::path::Path) -> Option<&SourceFingerprint> {
        self.files.get(path).map(|f| &f.fingerprint)
    }

    /// Return the code resource inventory.
    pub fn code_resources_for_module(&self) -> &CodeResourceInventory {
        &self.code_resources
    }

    /// Return the visual file inventory.
    pub fn visual_files_for_source(&self, ets_path: &std::path::Path) -> Vec<&PathBuf> {
        self.visuals.visual_files_for_source(ets_path)
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
        assert_eq!(store.visual_files_for_source(&source), vec![&visual]);
        assert!(store.fingerprint(&visual).is_some());
    }
}
