//! Immutable input filesystem for one compiler snapshot, including deletion tombstones.
use super::{NativeCheckError, options::CompilerPaths};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};
use tsrs_vfs::{Entries, FS, FileInfo, FileMode};

pub(super) struct SourceSnapshot {
    base: Arc<dyn FS>,
    sources: BTreeMap<String, (PathBuf, Arc<str>)>,
    deleted: BTreeSet<String>,
    directories: BTreeMap<String, PathBuf>,
}
impl SourceSnapshot {
    pub(super) fn new(
        base: Arc<dyn FS>,
        sources: &BTreeMap<PathBuf, Arc<str>>,
        deleted: &BTreeSet<PathBuf>,
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
                Ok((canonical(&path), (PathBuf::from(path), Arc::clone(text))))
            })
            .collect::<Result<BTreeMap<_, _>, NativeCheckError>>()?;
        let deleted = deleted
            .iter()
            .map(|p| Ok(canonical(&CompilerPaths::one(p)?)))
            .collect::<Result<BTreeSet<_>, NativeCheckError>>()?;
        let directories = sources
            .values()
            .flat_map(|(p, _)| p.ancestors().skip(1).map(Path::to_path_buf))
            .map(|p| (canonical(p.to_str().expect("validated compiler path")), p))
            .collect();
        Ok(Self {
            base,
            sources,
            deleted,
            directories,
        })
    }
    fn key(&self, path: &str) -> String {
        tsrs_core::tspath::get_canonical_file_name(
            &tsrs_core::tspath::normalize_path(path),
            self.base.use_case_sensitive_file_names(),
        )
    }
}
impl FS for SourceSnapshot {
    fn use_case_sensitive_file_names(&self) -> bool {
        self.base.use_case_sensitive_file_names()
    }
    fn file_exists(&self, path: &str) -> bool {
        let p = self.key(path);
        !self.deleted.contains(&p) && (self.sources.contains_key(&p) || self.base.file_exists(path))
    }
    fn read_file(&self, path: &str) -> Option<String> {
        let p = self.key(path);
        if self.deleted.contains(&p) {
            return None;
        }
        self.sources
            .get(&p)
            .map(|(_, text)| text.to_string())
            .or_else(|| self.base.read_file(path))
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
        for (file, _) in self.sources.values().filter(|(p, _)| {
            p.parent()
                .is_some_and(|p| self.key(p.to_str().unwrap()) == self.key(path))
        }) {
            if let Some(name) = file.file_name().and_then(|n| n.to_str()) {
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
        if self.deleted.contains(&p) {
            return None;
        }
        if let Some((original, text)) = self.sources.get(&p) {
            return Some(FileInfo {
                name: original.file_name()?.to_str()?.into(),
                size: text.len() as i64,
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
            && let Some((original, _)) = self.sources.get(&self.key(path))
        {
            return original.to_str().expect("validated compiler path").into();
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
            let old = SourceSnapshot::new(Arc::clone(&base), &sources, &BTreeSet::new()).unwrap();
            let new = SourceSnapshot::new(
                base,
                &sources,
                &BTreeSet::from([PathBuf::from("/Project/Remove.ets")]),
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
