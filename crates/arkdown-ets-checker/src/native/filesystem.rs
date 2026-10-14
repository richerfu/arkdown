//! Immutable input filesystem for one compiler snapshot, including deletion tombstones.
use super::{
    NativeCheckError, NativeProcessedSource, NativeSourcePreprocessor, options::CompilerPaths,
};
use arkdown_core::{SourceReadError, SourceTextHandle};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::SystemTime,
};
use tsrs_vfs::{Entries, FS, FileInfo, FileMode};

pub(super) struct SourceSnapshot {
    base: Arc<dyn FS>,
    sources: BTreeMap<String, Arc<SnapshotSource>>,
    disk_sources: Mutex<BTreeMap<String, Arc<SnapshotSource>>>,
    preprocessor: Option<Arc<dyn NativeSourcePreprocessor>>,
    deleted: BTreeSet<String>,
    directories: BTreeMap<String, PathBuf>,
    read_error: OnceLock<Arc<SourceReadError>>,
}
struct SnapshotSource {
    path: PathBuf,
    raw: SourceInput,
    processed: OnceLock<Result<SnapshotContent, Arc<SourceReadError>>>,
}
enum SourceInput {
    Captured(Arc<str>),
    Deferred(Arc<dyn SourceTextHandle>),
}
impl SourceInput {
    fn size_bytes(&self) -> u64 {
        match self {
            Self::Captured(text) => text.len() as u64,
            Self::Deferred(source) => source.size_bytes(),
        }
    }
}
struct SnapshotContent {
    // Preprocessing can create different text. Retain the original lease too,
    // so the store never evicts a source the program has actually read.
    _raw: Arc<str>,
    processed: NativeProcessedSource,
}
impl SnapshotSource {
    fn content(
        &self,
        preprocessor: Option<&dyn NativeSourcePreprocessor>,
    ) -> Result<&NativeProcessedSource, &Arc<SourceReadError>> {
        self.processed
            .get_or_init(|| {
                let raw = match &self.raw {
                    SourceInput::Captured(text) => Arc::clone(text),
                    SourceInput::Deferred(source) => source.read_text().map_err(Arc::new)?,
                };
                let processed = if Self::is_preprocess_source(&self.path)
                    && let Some(preprocessor) = preprocessor
                {
                    preprocessor.preprocess(&self.path, Arc::clone(&raw))
                } else {
                    NativeProcessedSource {
                        text: Arc::clone(&raw),
                        ..Default::default()
                    }
                };
                Ok(SnapshotContent {
                    _raw: raw,
                    processed,
                })
            })
            .as_ref()
            .map(|content| &content.processed)
    }
    fn is_preprocess_source(path: &Path) -> bool {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        !name.ends_with(".d.ts")
            && !name.ends_with(".d.ets")
            && matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("ets" | "ts")
            )
    }
}
impl SourceSnapshot {
    pub(super) fn new(
        base: Arc<dyn FS>,
        sources: &BTreeMap<PathBuf, Arc<str>>,
        deleted: &BTreeSet<PathBuf>,
        preprocessor: Option<Arc<dyn NativeSourcePreprocessor>>,
        previous: Option<&Self>,
        invalidated: &BTreeSet<PathBuf>,
    ) -> Result<Self, NativeCheckError> {
        let canonical = |path: &str| {
            tsrs_core::tspath::get_canonical_file_name(
                &tsrs_core::tspath::normalize_path(path),
                base.use_case_sensitive_file_names(),
            )
        };
        let sources = sources
            .iter()
            .map(|(p, text)| {
                let path = CompilerPaths::one(p)?;
                let key = canonical(&path);
                let source = previous
                    .and_then(|previous| previous.sources.get(&key))
                    .filter(|source| matches!(&source.raw, SourceInput::Captured(raw) if raw.as_ref() == text.as_ref()))
                    .map(Arc::clone)
                    .unwrap_or_else(|| {
                        Arc::new(SnapshotSource {
                            path: PathBuf::from(path),
                            raw: SourceInput::Captured(Arc::clone(text)),
                            processed: OnceLock::new(),
                        })
                    });
                Ok((key, source))
            })
            .collect::<Result<BTreeMap<_, _>, NativeCheckError>>()?;
        let deleted = deleted
            .iter()
            .map(|p| Ok(canonical(&CompilerPaths::one(p)?)))
            .collect::<Result<BTreeSet<_>, NativeCheckError>>()?;
        let directories = sources
            .values()
            .flat_map(|source| source.path.ancestors().skip(1).map(Path::to_path_buf))
            .map(|p| (canonical(p.to_str().expect("validated compiler path")), p))
            .collect();
        let invalidated = invalidated
            .iter()
            .map(|path| Ok(canonical(&CompilerPaths::one(path)?)))
            .collect::<Result<BTreeSet<_>, NativeCheckError>>()?;
        let disk_sources = previous
            .map(|previous| {
                previous
                    .disk_sources
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|(key, _)| !deleted.contains(*key) && !invalidated.contains(*key))
                    .map(|(key, source)| (key.clone(), Arc::clone(source)))
                    .collect()
            })
            .unwrap_or_default();
        Ok(Self {
            base,
            disk_sources: Mutex::new(disk_sources),
            preprocessor,
            sources,
            deleted,
            directories,
            read_error: OnceLock::new(),
        })
    }

    pub(super) fn with_source_handles(
        mut self,
        handles: &BTreeMap<PathBuf, Arc<dyn SourceTextHandle>>,
    ) -> Result<Self, NativeCheckError> {
        for (path, handle) in handles {
            let path = PathBuf::from(CompilerPaths::one(path)?);
            for directory in path.ancestors().skip(1) {
                self.directories.insert(
                    self.key(directory.to_str().unwrap()),
                    directory.to_path_buf(),
                );
            }
            self.sources.insert(
                self.key(path.to_str().unwrap()),
                Arc::new(SnapshotSource {
                    path,
                    raw: SourceInput::Deferred(Arc::clone(handle)),
                    processed: OnceLock::new(),
                }),
            );
        }
        Ok(self)
    }

    // tsrs FS returns Option, not Result. Preserve a failed captured read and
    // return its typed cause at the native API boundary, never a disk fallback
    // or an apparent "missing module" diagnostic.
    pub(super) fn check_result<T>(
        &self,
        result: tsrs_api::ApiResult<T>,
    ) -> Result<T, NativeCheckError> {
        if let Some(error) = self.read_error.get() {
            return Err(NativeCheckError::Source(Arc::clone(error)));
        }
        result.map_err(Into::into)
    }

    fn content(&self, source: &SnapshotSource) -> Option<Arc<str>> {
        match source.content(self.preprocessor.as_deref()) {
            Ok(processed) => Some(Arc::clone(&processed.text)),
            Err(error) => {
                let _ = self.read_error.set(Arc::clone(error));
                None
            }
        }
    }
    fn key(&self, path: &str) -> String {
        tsrs_core::tspath::get_canonical_file_name(
            &tsrs_core::tspath::normalize_path(path),
            self.base.use_case_sensitive_file_names(),
        )
    }
    fn is_deleted(&self, path: &str) -> bool {
        !self.deleted.is_empty()
            && (self.deleted.contains(&self.key(path))
                || self.deleted.contains(&self.key(&self.base.realpath(path))))
    }
    fn known_source(&self, path: &str) -> Option<&Arc<SnapshotSource>> {
        self.sources
            .get(&self.key(path))
            .or_else(|| self.sources.get(&self.key(&self.base.realpath(path))))
    }
    pub(super) fn read_sources(&self) -> BTreeMap<PathBuf, NativeProcessedSource> {
        let collect = |source: &Arc<SnapshotSource>| {
            source
                .processed
                .get()
                .and_then(|content| content.as_ref().ok())
                .map(|content| (source.path.clone(), content.processed.clone()))
        };
        self.sources
            .values()
            .filter_map(collect)
            .chain(
                self.disk_sources
                    .lock()
                    .unwrap()
                    .values()
                    .filter_map(collect),
            )
            .collect()
    }
}
impl FS for SourceSnapshot {
    fn use_case_sensitive_file_names(&self) -> bool {
        self.base.use_case_sensitive_file_names()
    }
    fn file_exists(&self, path: &str) -> bool {
        let p = self.key(path);
        !self.is_deleted(path) && (self.sources.contains_key(&p) || self.base.file_exists(path))
    }
    fn read_file(&self, path: &str) -> Option<String> {
        self.read_file_shared(path).map(|text| text.to_string())
    }
    fn read_file_shared(&self, path: &str) -> Option<Arc<str>> {
        let p = self.key(path);
        if self.is_deleted(path) {
            return None;
        }
        if let Some(source) = self.known_source(path) {
            return self.content(source);
        }
        if self.preprocessor.is_none() || !SnapshotSource::is_preprocess_source(Path::new(path)) {
            return self.base.read_file_shared(path);
        }
        let source = {
            let mut sources = self.disk_sources.lock().unwrap();
            if let Some(source) = sources.get(&p) {
                Arc::clone(source)
            } else {
                let raw = self.base.read_file_shared(path)?;
                let source = Arc::new(SnapshotSource {
                    path: PathBuf::from(path),
                    raw: SourceInput::Captured(raw),
                    processed: OnceLock::new(),
                });
                sources.insert(p, Arc::clone(&source));
                source
            }
        };
        self.content(&source)
    }
    fn directory_exists(&self, path: &str) -> bool {
        self.directories.contains_key(&self.key(path)) || self.base.directory_exists(path)
    }
    fn get_accessible_entries(&self, path: &str) -> Entries {
        let parent = Path::new(path);
        let mut entries = self.base.get_accessible_entries(path);
        entries.files.retain(|name| {
            !self
                .deleted
                .contains(&self.key(parent.join(name).to_str().expect("validated compiler path")))
        });
        for source in self.sources.values().filter(|source| {
            source
                .path
                .parent()
                .is_some_and(|p| self.key(p.to_str().unwrap()) == self.key(path))
        }) {
            if let Some(name) = source.path.file_name().and_then(|n| n.to_str()) {
                entries.files.push(name.into());
            }
        }
        for dir in self.directories.values().filter(|p| {
            p.parent()
                .is_some_and(|p| self.key(p.to_str().unwrap()) == self.key(path))
        }) {
            if let Some(name) = dir.file_name().and_then(|n| n.to_str()) {
                entries.directories.push(name.into());
            }
        }
        entries.files.sort_unstable();
        entries.files.dedup();
        entries.directories.sort_unstable();
        entries.directories.dedup();
        entries
    }
    fn stat(&self, path: &str) -> Option<FileInfo> {
        let p = self.key(path);
        if self.is_deleted(path) {
            return None;
        }
        if let Some(source) = self.sources.get(&p) {
            return Some(FileInfo {
                name: source.path.file_name()?.to_str()?.into(),
                size: source.raw.size_bytes() as i64,
                mode: FileMode::None,
                mod_time: None,
            });
        }
        if let Some(original) = self
            .directories
            .get(&p)
            .filter(|_| !self.base.directory_exists(path))
        {
            return Some(FileInfo {
                name: original.file_name()?.to_str()?.into(),
                size: 0,
                mode: FileMode::Dir,
                mod_time: None,
            });
        }
        self.base.stat(path)
    }
    fn realpath(&self, path: &str) -> String {
        if !self.base.file_exists(path)
            && let Some(source) = self.sources.get(&self.key(path))
        {
            return source
                .path
                .to_str()
                .expect("validated compiler path")
                .into();
        }
        self.base.realpath(path)
    }
    fn write_file(&self, _: &str, _: &str) -> Result<(), String> {
        Err("compiler snapshot is read-only".into())
    }
    fn append_file(&self, _: &str, _: &str) -> Result<(), String> {
        Err("compiler snapshot is read-only".into())
    }
    fn remove(&self, _: &str) -> Result<(), String> {
        Err("compiler snapshot is read-only".into())
    }
    fn chtimes(&self, _: &str, _: SystemTime, _: SystemTime) -> Result<(), String> {
        Err("compiler snapshot is read-only".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_snapshots_retain_shared_text_and_detect_changed_versions() {
        use tsrs_api::{
            SessionOptions,
            embedded::{Compiler, FileChanges},
        };
        use tsrs_core::{CompilerOptions, Tristate};
        let base: Arc<dyn FS> = Arc::new(tsrs_vfs::vfstest::from_map(
            std::iter::empty::<(&str, &str)>(),
            true,
        ));
        let compiler = Compiler::new(SessionOptions::new(
            "/".into(),
            String::new(),
            Arc::clone(&base),
            false,
        ))
        .unwrap();
        let path = PathBuf::from("/entry.ts");
        let text: Arc<str> = Arc::from("export const value: number = 'wrong';");
        let sources = BTreeMap::from([(path.clone(), Arc::clone(&text))]);
        let first = Arc::new(
            SourceSnapshot::new(
                Arc::clone(&base),
                &sources,
                &BTreeSet::new(),
                None,
                None,
                &BTreeSet::new(),
            )
            .unwrap(),
        );
        // Initialize the processed slot before counting: the next owner must be tsrs's file handle.
        assert!(Arc::ptr_eq(
            &first.read_file_shared("/entry.ts").unwrap(),
            &text
        ));
        let owners = Arc::strong_count(&text);
        let roots = vec!["/entry.ts".into()];
        let options = CompilerOptions {
            no_lib: Tristate::True,
            no_emit: Tristate::True,
            ..Default::default()
        };
        let old = compiler
            .create_program(&roots, options.clone(), first.clone())
            .unwrap();
        assert!(
            Arc::strong_count(&text) > owners,
            "tsrs must retain the original Arc, not an owned copy"
        );
        assert!(
            old.semantic_diagnostics()
                .unwrap()
                .iter()
                .any(|d| d.code == 2322)
        );
        let replacement: Arc<str> = Arc::from("export const value: number = 42;");
        let updated = BTreeMap::from([(path.clone(), Arc::clone(&replacement))]);
        let second = Arc::new(
            SourceSnapshot::new(
                base,
                &updated,
                &BTreeSet::new(),
                None,
                Some(&first),
                &BTreeSet::from([path]),
            )
            .unwrap(),
        );
        assert!(Arc::ptr_eq(
            &second.read_file_shared("/entry.ts").unwrap(),
            &replacement
        ));
        let owners = Arc::strong_count(&replacement);
        let next = compiler
            .update_program(
                &old,
                &roots,
                options,
                second,
                &FileChanges {
                    changed: roots.clone(),
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(Arc::strong_count(&replacement) > owners);
        assert!(
            !next
                .semantic_diagnostics()
                .unwrap()
                .iter()
                .any(|d| d.code == 2322)
        );
        assert!(
            old.semantic_diagnostics()
                .unwrap()
                .iter()
                .any(|d| d.code == 2322)
        );
        assert!(Arc::ptr_eq(
            &first.read_file_shared("/entry.ts").unwrap(),
            &text
        ));
    }

    #[test]
    fn snapshot_overlay_respects_host_case_rules_and_deletions() {
        for sensitive in [false, true] {
            let base: Arc<dyn FS> = Arc::new(tsrs_vfs::vfstest::from_map(
                [
                    ("/Project/Keep.ets", "disk"),
                    ("/Project/Remove.ets", "deleted disk"),
                ],
                sensitive,
            ));
            let sources = BTreeMap::from([
                (PathBuf::from("/Project/Keep.ets"), Arc::from("overlay")),
                (
                    PathBuf::from("/Project/Virtual/New.ets"),
                    Arc::from("virtual"),
                ),
            ]);
            let old = SourceSnapshot::new(
                Arc::clone(&base),
                &sources,
                &BTreeSet::new(),
                None,
                None,
                &BTreeSet::new(),
            )
            .unwrap();
            let new = SourceSnapshot::new(
                base,
                &sources,
                &BTreeSet::from([PathBuf::from("/Project/Remove.ets")]),
                None,
                None,
                &BTreeSet::new(),
            )
            .unwrap();
            assert_eq!(
                new.read_file("/Project/Keep.ets").as_deref(),
                Some("overlay")
            );
            assert_eq!(
                new.read_file("/project/KEEP.ets").as_deref(),
                if sensitive { None } else { Some("overlay") }
            );
            assert!(Arc::ptr_eq(
                &new.read_file_shared("/Project/Keep.ets").unwrap(),
                &sources[&PathBuf::from("/Project/Keep.ets")],
            ));
            assert!(new.directory_exists("/Project/Virtual"));
            assert_eq!(new.stat("/Project/Virtual/New.ets").unwrap().size, 7);
            assert!(!new.file_exists("/Project/Remove.ets"));
            assert!(!new.file_exists("/project/REMOVE.ets"));
            assert!(new.read_file("/Project/Remove.ets").is_none());
            assert!(old.file_exists("/Project/Remove.ets"));
            assert_eq!(new.get_accessible_entries("/Project").files, ["Keep.ets"]);
            let virtual_path = if sensitive {
                "/Project/Virtual/New.ets"
            } else {
                "/project/virtual/new.ets"
            };
            assert_eq!(new.realpath(virtual_path), "/Project/Virtual/New.ets");
            assert!(new.write_file("/Project/Keep.ets", "mutate").is_err());
        }
    }
}
