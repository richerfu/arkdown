//! hvigor module path ownership: module path map, roots, and chunk sources.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use rustc_hash::{FxHashMap, FxHashSet};

use arkdown_core::{ArkdownError, ChunkSourceFile, HvigorModuleSourceFile, HvigorNpmEntry};
use arkdown_path::{
    lexical_normalize, strip_relative_prefix, AbsPath, ArtifactPathSerializer, OhmUrl,
};
use arkdown_resolver::OhPackage;

use crate::output;
use crate::plan::{BundleRuntimePackageContext, BundleRuntimePackageInfo};

use super::*;

pub(crate) struct HvigorNormalizedOhmUrl<'a> {
    pub(crate) context: &'a BundleRuntimePackageContext,
    pub(crate) project_file_path: &'a str,
}

impl<'a> HvigorNormalizedOhmUrl<'a> {
    pub(crate) fn new(
        context: &'a BundleRuntimePackageContext,
        project_file_path: &'a str,
    ) -> Self {
        Self {
            context,
            project_file_path,
        }
    }

    pub(crate) fn to_request(&self) -> String {
        let source_path = Path::new(self.project_file_path).with_extension("");
        let source_path = ArtifactPathSerializer::rel_to_slash(&source_path);
        let normalized_import = format!("{}/{}", self.context.package_name, source_path);
        OhmUrl::normalized(
            self.context.is_so,
            self.context.module_name.as_str(),
            self.context.bundle_name.as_str(),
            &normalized_import,
            self.context.version.as_str(),
        )
        .into_string()
    }

    pub(crate) fn to_record(&self) -> String {
        let source_path = Path::new(self.project_file_path).with_extension("");
        let source_path = ArtifactPathSerializer::rel_to_slash(&source_path);
        format!(
            "{}&{}/{}&{}",
            self.context.bundle_name, self.context.package_name, source_path, self.context.version
        )
    }
}

#[derive(Debug, Clone)]
pub(crate) struct HvigorModulePathMap {
    pub(crate) current_module_name: String,
    pub(crate) current_module_root: PathBuf,
    pub(crate) roots: Vec<HvigorModuleRoot>,
    pub(crate) module_meta_info_ohm_urls: FxHashMap<PathBuf, String>,
}

impl HvigorModulePathMap {
    pub(crate) fn current_module_root(&self) -> &Path {
        &self.current_module_root
    }

    pub(crate) fn new(
        current_module_name: &str,
        module_root: &Path,
        module_path_map: &[(String, PathBuf)],
    ) -> Self {
        let mut roots = module_path_map
            .iter()
            .map(|(name, root)| {
                HvigorModuleRoot::from_module_path_map(
                    current_module_name,
                    name.clone(),
                    module_root,
                    Self::canonical_root(root),
                )
            })
            .collect::<Vec<_>>();
        // The loader module root points at `<module>/src/main`, while
        // modulePathMap owns the complete module root. Do not add a more
        // specific fallback root in that case: it cannot load oh-package.json5
        // and would shadow the owning root with the synthetic `1.0.0` version.
        let has_current_module_owner = roots.iter().any(|root| {
            root.module_name == current_module_name
                && root.kind == HvigorModuleRootKind::SourceModule
                && module_root.starts_with(&root.path)
        });
        if !has_current_module_owner {
            roots.push(HvigorModuleRoot::source_root(
                current_module_name.to_string(),
                module_root.to_path_buf(),
            ));
        }
        let current_module_root = roots
            .iter()
            .find(|root| {
                root.module_name == current_module_name
                    && root.kind == HvigorModuleRootKind::SourceModule
                    && (root.path.join("oh-package.json5").is_file()
                        || root.path.join("package.json").is_file())
            })
            .map(|root| root.path.clone())
            .or_else(|| Self::package_root_for_compile_entry(module_root))
            .unwrap_or_else(|| Self::canonical_root(module_root));
        Self::from_roots(current_module_name.to_string(), current_module_root, roots)
    }

    pub(crate) fn add_compile_entry_package_roots(&mut self, entries: &[String]) {
        let mut roots = std::mem::take(&mut self.roots);
        for entry in entries {
            let entry_path = Path::new(entry);
            if !entry_path.has_root() || !arkdown_path::classify::is_inside_oh_modules(entry_path) {
                continue;
            }
            let Some(root_path) = Self::package_root_for_compile_entry(entry_path) else {
                continue;
            };
            if roots.iter().any(|root| root.path == root_path) {
                continue;
            }
            let Some(root) = HvigorModuleRoot::from_package_root(root_path) else {
                continue;
            };
            roots.push(root);
        }
        self.rebuild_roots(roots);
    }

    pub(crate) fn add_module_record_roots(&mut self, module_record_roots: &[(String, PathBuf)]) {
        let mut roots = std::mem::take(&mut self.roots);
        for (name, root) in module_record_roots {
            roots.push(HvigorModuleRoot::from_record_root(
                name.clone(),
                Self::canonical_root(root),
            ));
        }
        self.rebuild_roots(roots);
    }

    pub(crate) fn add_module_meta_info_files(
        &mut self,
        files: &[crate::plan::BundleModuleMetaInfoFile],
    ) {
        self.module_meta_info_ohm_urls.reserve(files.len());
        for file in files {
            self.module_meta_info_ohm_urls
                .insert(Self::canonical_root(&file.file_path), file.ohm_url.clone());
        }
    }

    pub(crate) fn module_meta_info_ohm_url(&self, module_id: &str) -> Option<&str> {
        self.module_meta_info_ohm_urls
            .get(&Self::canonical_root(Path::new(module_id)))
            .map(String::as_str)
    }

    pub(crate) fn rebuild_roots(&mut self, roots: Vec<HvigorModuleRoot>) {
        let current_module_name = self.current_module_name.clone();
        let current_module_root = self.current_module_root.clone();
        let module_meta_info_ohm_urls = std::mem::take(&mut self.module_meta_info_ohm_urls);
        let mut rebuilt = Self::from_roots(current_module_name, current_module_root, roots);
        rebuilt.module_meta_info_ohm_urls = module_meta_info_ohm_urls;
        *self = rebuilt;
    }

    pub(crate) fn from_roots(
        current_module_name: String,
        current_module_root: PathBuf,
        mut roots: Vec<HvigorModuleRoot>,
    ) -> Self {
        roots.sort_by(|left, right| {
            right
                .path
                .components()
                .count()
                .cmp(&left.path.components().count())
                .then_with(|| left.path.cmp(&right.path))
                .then_with(|| {
                    left.module_name
                        .starts_with('@')
                        .cmp(&right.module_name.starts_with('@'))
                })
                .then_with(|| left.module_name.cmp(&right.module_name))
        });
        let mut merged_roots: Vec<HvigorModuleRoot> = Vec::with_capacity(roots.len());
        for root in roots {
            if let Some(existing) = merged_roots
                .iter_mut()
                .find(|existing| existing.path == root.path)
            {
                existing.dependency_aliases.extend(root.dependency_aliases);
                existing.dependency_aliases.sort();
                existing.dependency_aliases.dedup();
                continue;
            }
            merged_roots.push(root);
        }
        Self {
            current_module_name,
            current_module_root,
            roots: merged_roots,
            module_meta_info_ohm_urls: FxHashMap::default(),
        }
    }

    pub(crate) fn package_root_for_compile_entry(entry_path: &Path) -> Option<PathBuf> {
        let mut current = if entry_path.is_dir() {
            Some(entry_path)
        } else {
            entry_path.parent()
        };
        while let Some(path) = current {
            if path.join("oh-package.json5").is_file() || path.join("package.json").is_file() {
                return Some(Self::canonical_root(path));
            }
            current = path.parent();
        }
        None
    }

    pub(crate) fn source_file_for_entry(
        &self,
        entry: &str,
    ) -> Result<HvigorChunkSource, ArkdownError> {
        let entry_path = Path::new(entry);
        if !entry_path.has_root() {
            return Ok(HvigorChunkSource::source_module(
                self.current_module_name.clone(),
                self.current_module_name.clone(),
                "1.0.0".to_string(),
                strip_relative_prefix(entry).to_string(),
            ));
        }

        let entry_path = Self::canonical_root(entry_path);
        for root in &self.roots {
            if let Ok(relative) = entry_path.strip_prefix(&root.path) {
                return root.source_for_relative(&entry_path, relative);
            }
        }
        Ok(HvigorChunkSource::source_module(
            String::new(),
            String::new(),
            "1.0.0".to_string(),
            HvigorModuleRelativeSourceFile::new(&entry_path).source_file(),
        ))
    }

    pub(crate) fn package_root_for_entry(&self, entry: &str) -> Option<&Path> {
        let entry_path = Path::new(entry);
        if !entry_path.has_root() {
            return Some(self.current_module_root.as_path());
        }
        let entry_path = Self::canonical_root(entry_path);
        self.roots
            .iter()
            .find(|root| entry_path.starts_with(&root.path))
            .map(|root| root.path.as_path())
    }

    pub(crate) fn has_dependency_package_info(&self, module_id: &str) -> bool {
        let module_path = Self::canonical_root(Path::new(module_id));
        // `module-meta-info-plugin.js::addModuleInfoData` only attaches
        // dependencyPkgInfo when the module is outside projectConfig.modulePath.
        // A remote Source HAR installed below `<module>/oh_modules` therefore
        // keeps entry-package attribution in source-map keys even though its
        // ModuleInfo packageName is the HAR package name.
        !module_path.starts_with(&self.current_module_root)
    }

    pub(crate) fn current_package_identity(&self) -> Option<(&str, &str)> {
        self.roots
            .iter()
            .find(|root| {
                root.kind == HvigorModuleRootKind::SourceModule
                    && root.path == self.current_module_root
            })
            .or_else(|| {
                self.roots.iter().find(|root| {
                    root.kind == HvigorModuleRootKind::SourceModule
                        && root.module_name == self.current_module_name
                })
            })
            .map(|root| (root.package_name.as_str(), root.package_version.as_str()))
    }

    /// Source-map ownership for project-model source HARs is derived from
    /// `module-meta-info-plugin.js::addModuleInfoData`, not from their module
    /// record URL. Keep this list in the same most-specific-first root order
    /// used by ModuleMode source attribution.
    pub(crate) fn source_map_local_dependency_roots(
        &self,
    ) -> Vec<output::SourceMapLocalDependencyRoot> {
        self.roots
            .iter()
            .filter(|root| root.kind == HvigorModuleRootKind::LocalDependencyModule)
            .map(|root| output::SourceMapLocalDependencyRoot {
                root: root.path.clone(),
                package_name: root.package_name.clone(),
                package_version: root.package_version.clone(),
            })
            .collect()
    }

    pub(crate) fn canonical_root(path: &Path) -> PathBuf {
        AbsPath::canonicalize(path)
            .map(AbsPath::into_path_buf)
            .unwrap_or_else(|_| lexical_normalize(path))
    }

    /// Mirrors `ark_utils.getOhmUrlByFilepath` for ModuleMode builds where
    /// `useNormalizedOHMUrl` is disabled.
    pub(crate) fn non_normalized_record_name(
        &self,
        project_root: &Path,
        module_id: &str,
        bundle_name: &str,
        entry_module_name: &str,
        source: &HvigorChunkSource,
    ) -> Result<String, ArkdownError> {
        match &source.kind {
            HvigorChunkSourceKind::PackageModule(_) => {
                let module_path = Path::new(module_id);
                let record = self
                    .package_record_for_path(project_root, module_path)
                    .ok_or_else(|| {
                        ArkdownError::Bundler(format!(
                            "cannot derive hvigor package record for '{}'",
                            module_path.display()
                        ))
                    })?;
                Ok(ArtifactPathSerializer::rel_to_slash(
                    &Path::new(&record).with_extension(""),
                ))
            }
            HvigorChunkSourceKind::SourceModule
            | HvigorChunkSourceKind::LocalDependencyModule(_) => {
                let source_file =
                    HvigorModuleRelativeSourceFile::new(Path::new(source.source_file.as_str()))
                        .source_file();
                let source_file = ArtifactPathSerializer::rel_to_slash(
                    &Path::new(&source_file).with_extension(""),
                );
                // Files under the current module root belong to the entry
                // module even when the source ownership resolved them to a
                // configured module name (the `ohosTest` template tree lives
                // under `<module>/.test/...` but carries the test module name
                // `entry_test`; hvigor `getBelongModuleInfo` keys by the
                // modulePathMap entry).
                let belongs_to_current = source.module_name == entry_module_name
                    || Self::canonical_root(Path::new(module_id))
                        .starts_with(&self.current_module_root);
                let module_segment = if belongs_to_current {
                    entry_module_name.to_string()
                } else {
                    format!("{entry_module_name}@{}", source.module_name)
                };
                Ok(format!("{bundle_name}/{module_segment}/{source_file}"))
            }
        }
    }

    pub(crate) fn npm_entries(
        &self,
        project_root: &Path,
        bundle_name: &str,
        entry_module_ids: &[String],
        module_source_files: &[HvigorModuleSourceFile],
    ) -> Result<Vec<HvigorNpmEntry>, ArkdownError> {
        let active_records = module_source_files
            .iter()
            .map(|source| source.record_name.as_str())
            .collect::<FxHashSet<_>>();
        let mut entries = Vec::new();
        let mut active_package_roots = FxHashSet::default();
        let compile_entry_paths = entry_module_ids
            .iter()
            .map(|entry| Self::canonical_root(Path::new(entry)))
            .collect::<FxHashSet<_>>();

        for root in self
            .roots
            .iter()
            .filter(|root| root.kind == HvigorModuleRootKind::PackageModule)
        {
            let Some((entry_path, build_path)) = self.package_root_entry(project_root, root)?
            else {
                continue;
            };
            if !active_records.contains(build_path.as_str()) {
                continue;
            }
            active_package_roots.insert(entry_path.clone());
            entries.push(HvigorNpmEntry {
                entry_path,
                build_path,
            });
        }

        self.add_package_symlink_entries(project_root, &active_package_roots, &mut entries)?;
        self.add_local_dependency_entries(
            bundle_name,
            &compile_entry_paths,
            &active_records,
            &mut entries,
        )?;

        // `ModuleMode.generateNpmEntriesInfo` preserves Map insertion order.
        // Keep the graph/root traversal order and only collapse exact duplicate
        // mappings, instead of sorting an order-sensitive npm entry manifest.
        let mut seen_entries = FxHashSet::default();
        entries.retain(|entry| {
            seen_entries.insert((entry.entry_path.clone(), entry.build_path.clone()))
        });
        Ok(entries)
    }

    pub(crate) fn package_root_entry(
        &self,
        project_root: &Path,
        root: &HvigorModuleRoot,
    ) -> Result<Option<(String, String)>, ArkdownError> {
        let Some(entry_path) = self.package_record_for_path(project_root, &root.path) else {
            return Ok(None);
        };
        let entry_rel = OhPackage::load_from_root(&root.path)
            .and_then(|package| {
                package
                    .exports_import_field()
                    .or_else(|| package.main.clone())
            })
            .unwrap_or_else(|| "index".to_string());
        let entry_rel = strip_relative_prefix(&entry_rel);
        // In legacy mode `ModuleMode.getPackageEntryInfo` writes the physical
        // `pkg_modules` path for both sides of a package entry mapping. It does
        // not use the normalized pkgContextInfo record that is valid only when
        // `useNormalizedOHMUrl` is enabled.
        let build_path = ArtifactPathSerializer::rel_to_slash(
            &Path::new(&entry_path).join(entry_rel).with_extension(""),
        );
        Ok(Some((entry_path, build_path)))
    }

    pub(crate) fn add_package_symlink_entries(
        &self,
        project_root: &Path,
        active_package_roots: &FxHashSet<String>,
        entries: &mut Vec<HvigorNpmEntry>,
    ) -> Result<(), ArkdownError> {
        let mut canonical_package_roots = FxHashMap::default();
        for root in self
            .roots
            .iter()
            .filter(|root| root.kind == HvigorModuleRootKind::PackageModule)
        {
            let Some(record) = self.package_record_for_path(project_root, &root.path) else {
                continue;
            };
            if !active_package_roots.contains(record.as_str()) {
                continue;
            }
            canonical_package_roots.insert(Self::canonical_root(&root.path), record);
        }

        for oh_modules_root in self.oh_modules_scan_roots(project_root) {
            self.scan_hvigor_oh_modules_dir(
                project_root,
                &oh_modules_root,
                &canonical_package_roots,
                entries,
            )?;
            let store_root = oh_modules_root.join(".ohpm");
            let Ok(store_entries) = fs_err::read_dir(&store_root) else {
                continue;
            };
            for store_entry in store_entries {
                let store_entry = store_entry?;
                let package_oh_modules = store_entry.path().join("oh_modules");
                self.scan_hvigor_oh_modules_dir(
                    project_root,
                    &package_oh_modules,
                    &canonical_package_roots,
                    entries,
                )?;
            }
        }
        Ok(())
    }

    /// Mirrors `resolve-ohmodules.js::resolveProjectOhModules`. Hvigor records
    /// only direct OHPM links from each project/module `oh_modules` directory,
    /// followed by the direct links in every immediate `.ohpm/*/oh_modules`
    /// directory. It deliberately does not recursively walk package trees.
    pub(crate) fn scan_hvigor_oh_modules_dir(
        &self,
        project_root: &Path,
        root: &Path,
        canonical_package_roots: &FxHashMap<PathBuf, String>,
        entries: &mut Vec<HvigorNpmEntry>,
    ) -> Result<(), ArkdownError> {
        let Ok(read_dir) = fs_err::read_dir(root) else {
            return Ok(());
        };
        for entry in read_dir {
            let entry = entry?;
            let entry_path = entry.path();
            if !fs_err::metadata(&entry_path)
                .map(|metadata| metadata.is_dir())
                .unwrap_or(false)
            {
                continue;
            }
            let entry_name = entry.file_name();
            let entry_name = entry_name.to_string_lossy();
            if entry_name == ".ohpm" {
                continue;
            }
            if entry_name.starts_with('@') {
                let Ok(scoped_entries) = fs_err::read_dir(&entry_path) else {
                    continue;
                };
                for scoped_entry in scoped_entries {
                    let scoped_path = scoped_entry?.path();
                    if !fs_err::metadata(&scoped_path)
                        .map(|metadata| metadata.is_dir())
                        .unwrap_or(false)
                    {
                        continue;
                    }
                    self.record_hvigor_symlink_entry(
                        project_root,
                        &scoped_path,
                        canonical_package_roots,
                        entries,
                    );
                }
                continue;
            }
            self.record_hvigor_symlink_entry(
                project_root,
                &entry_path,
                canonical_package_roots,
                entries,
            );
        }
        Ok(())
    }

    pub(crate) fn record_hvigor_symlink_entry(
        &self,
        project_root: &Path,
        link_path: &Path,
        canonical_package_roots: &FxHashMap<PathBuf, String>,
        entries: &mut Vec<HvigorNpmEntry>,
    ) {
        let canonical_link = Self::canonical_root(link_path);
        if canonical_link == lexical_normalize(link_path) {
            return;
        }
        let Some(build_path) = canonical_package_roots.get(&canonical_link) else {
            return;
        };
        let Some(entry_path) = self.package_record_for_path(project_root, link_path) else {
            return;
        };
        entries.push(HvigorNpmEntry {
            entry_path,
            build_path: build_path.clone(),
        });
    }

    pub(crate) fn add_local_dependency_entries(
        &self,
        bundle_name: &str,
        compile_entry_paths: &FxHashSet<PathBuf>,
        active_records: &FxHashSet<&str>,
        entries: &mut Vec<HvigorNpmEntry>,
    ) -> Result<(), ArkdownError> {
        let current_root = self
            .roots
            .iter()
            .find(|root| {
                root.kind == HvigorModuleRootKind::SourceModule
                    && root.package_name == self.current_module_name
            })
            .or_else(|| {
                self.roots
                    .iter()
                    .find(|root| root.kind == HvigorModuleRootKind::SourceModule)
            });
        let Some(current_root) = current_root else {
            return Ok(());
        };
        let current_package = OhPackage::load_from_root(&current_root.path);
        let dependency_aliases = current_package
            .as_ref()
            .map(|package| package.file_dependencies())
            .unwrap_or_default();

        for root in self
            .roots
            .iter()
            .filter(|root| root.kind == HvigorModuleRootKind::LocalDependencyModule)
            .filter(|root| root.path != current_root.path)
        {
            let Some(alias) = dependency_aliases
                .iter()
                .find_map(|(alias, path)| {
                    let canonical = Self::canonical_root(path);
                    if canonical == root.path {
                        return Some(alias.as_str());
                    }
                    OhPackage::load_from_root(&canonical).and_then(|package| {
                        (package.name() == Some(root.package_name.as_str()))
                            .then_some(alias.as_str())
                    })
                })
                .or_else(|| root.dependency_aliases.first().map(String::as_str))
            else {
                continue;
            };
            let entry_source = OhPackage::load_from_root(&root.path)
                .and_then(|package| package.main_field().map(str::to_string))
                .filter(|main| !main.is_empty())
                .unwrap_or_else(|| "Index.ets".to_string());
            // `ModuleMode.getPackageEntryInfo` only receives a local package
            // entry when the original module metadata marks it as a node
            // entry. In the compose pipeline that corresponds to the
            // compile-entry inventory, not every active local HAR module.
            // HmosWorld, for example, compiles `conference/Index.ets` through
            // graph traversal but does not add a bare `@ohos/conference`
            // npm-entry mapping.
            let source_entry = Self::canonical_root(&root.path.join(&entry_source));
            if !compile_entry_paths.contains(&source_entry) {
                continue;
            }
            let build_record = self.local_dependency_record_name(bundle_name, root, &entry_source);
            if !active_records.contains(build_record.as_str()) {
                continue;
            }
            entries.push(HvigorNpmEntry {
                entry_path: format!("pkg_modules@{}/{}", self.current_module_name, alias),
                build_path: build_record.clone(),
            });
            entries.push(HvigorNpmEntry {
                entry_path: alias.to_string(),
                build_path: format!("@bundle.{build_record}"),
            });
        }
        Ok(())
    }

    pub(crate) fn local_dependency_record_name(
        &self,
        bundle_name: &str,
        root: &HvigorModuleRoot,
        entry_source: &str,
    ) -> String {
        let source_file =
            HvigorModuleRelativeSourceFile::new(Path::new(entry_source)).source_file();
        let source_file =
            ArtifactPathSerializer::rel_to_slash(&Path::new(&source_file).with_extension(""));
        let module_segment = if root.module_name == self.current_module_name {
            self.current_module_name.clone()
        } else {
            format!("{}@{}", self.current_module_name, root.module_name)
        };
        format!("{bundle_name}/{module_segment}/{source_file}")
    }

    pub(crate) fn oh_modules_scan_roots(&self, project_root: &Path) -> Vec<PathBuf> {
        let mut roots = vec![Self::canonical_root(project_root).join("oh_modules")];
        roots.extend(
            self.roots
                .iter()
                .filter(|root| root.kind.is_project_oh_modules_owner())
                .map(|root| root.path.join("oh_modules")),
        );
        roots.sort();
        roots.dedup();
        roots
    }

    pub(crate) fn package_record_for_path(
        &self,
        project_root: &Path,
        path: &Path,
    ) -> Option<String> {
        let project_oh_modules = Self::canonical_root(project_root).join("oh_modules");
        if let Some(relative) = Self::relative_to_root_preserving_links(path, &project_oh_modules) {
            if relative.starts_with(Path::new(".ohpm").join("oh_modules")) {
                return None;
            }
            return Some(Self::pkg_modules_record("pkg_modules", &relative));
        }

        for root in self
            .roots
            .iter()
            .filter(|root| root.kind.is_project_oh_modules_owner())
        {
            let oh_modules = root.path.join("oh_modules");
            if let Some(relative) = Self::relative_to_root_preserving_links(path, &oh_modules) {
                return Some(Self::pkg_modules_record(
                    &format!("pkg_modules@{}", root.module_name),
                    &relative,
                ));
            }
        }
        None
    }

    pub(crate) fn relative_to_root_preserving_links(path: &Path, root: &Path) -> Option<PathBuf> {
        let lexical_path = lexical_normalize(path);
        let lexical_root = lexical_normalize(root);
        if let Ok(relative) = lexical_path.strip_prefix(&lexical_root) {
            return Some(relative.to_path_buf());
        }

        // `/var` -> `/private/var` on macOS is a common project-root alias.
        // Resolve that alias only for non-link terminal paths; resolving a
        // package link here would erase the OHPM alias that ModuleMode stores
        // in npmEntries.
        let is_terminal_link = fs_err::symlink_metadata(path)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false);
        if is_terminal_link {
            return None;
        }
        let canonical_path = Self::canonical_root(path);
        let canonical_root = Self::canonical_root(root);
        canonical_path
            .strip_prefix(canonical_root)
            .ok()
            .map(Path::to_path_buf)
    }

    pub(crate) fn pkg_modules_record(prefix: &str, relative: &Path) -> String {
        let mut path = PathBuf::from(prefix);
        for component in relative.components() {
            let value = component.as_os_str();
            if value == "oh_modules" {
                path.push("pkg_modules");
            } else {
                path.push(value);
            }
        }
        ArtifactPathSerializer::rel_to_slash(&path)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct HvigorModuleRoot {
    pub(crate) module_name: String,
    pub(crate) package_name: String,
    pub(crate) package_version: String,
    pub(crate) dependency_aliases: Vec<String>,
    pub(crate) path: PathBuf,
    pub(crate) kind: HvigorModuleRootKind,
}

impl HvigorModuleRoot {
    pub(crate) fn source_root(module_name: String, path: PathBuf) -> Self {
        let package = OhPackage::load_from_root(&path);
        Self::with_package(
            module_name,
            path,
            HvigorModuleRootKind::SourceModule,
            package,
        )
    }

    pub(crate) fn from_module_path_map(
        current_module_name: &str,
        module_name: String,
        current_module_root: &Path,
        path: PathBuf,
    ) -> Self {
        let kind = HvigorModuleRootKind::from_root_path(&path);
        let package = OhPackage::load_from_root(&path);
        let kind = if kind == HvigorModuleRootKind::SourceModule
            && module_name != current_module_name
            && path != current_module_root
            && package.is_some()
        {
            HvigorModuleRootKind::LocalDependencyModule
        } else {
            kind
        };
        Self::with_package(module_name, path, kind, package)
    }

    pub(crate) fn from_package_root(path: PathBuf) -> Option<Self> {
        let package = OhPackage::load_from_root(&path)?;
        let module_name = package.name().map(str::to_string)?;
        let kind = HvigorModuleRootKind::from_root_path(&path);
        Some(Self::with_package(module_name, path, kind, Some(package)))
    }

    pub(crate) fn from_record_root(module_name: String, path: PathBuf) -> Self {
        let package = OhPackage::load_from_root(&path);
        Self::with_package(
            module_name,
            path,
            HvigorModuleRootKind::PackageModule,
            package,
        )
    }

    pub(crate) fn with_package(
        module_name: String,
        path: PathBuf,
        kind: HvigorModuleRootKind,
        package: Option<OhPackage>,
    ) -> Self {
        let package_name = package
            .as_ref()
            .and_then(|package| package.name())
            .unwrap_or(module_name.as_str())
            .to_string();
        let package_version = package
            .as_ref()
            .map(|package| package.version())
            .filter(|version| !version.is_empty())
            .unwrap_or("1.0.0")
            .to_string();
        Self {
            dependency_aliases: module_name
                .starts_with('@')
                .then(|| module_name.clone())
                .into_iter()
                .collect(),
            module_name,
            package_name,
            package_version,
            path,
            kind,
        }
    }

    pub(crate) fn source_for_relative(
        &self,
        absolute_path: &Path,
        relative: &Path,
    ) -> Result<HvigorChunkSource, ArkdownError> {
        match self.kind {
            HvigorModuleRootKind::SourceModule => Ok(HvigorChunkSource::source_module(
                self.package_name.clone(),
                self.module_name.clone(),
                self.package_version.clone(),
                HvigorModuleRelativeSourceFile::new(relative).source_file(),
            )),
            HvigorModuleRootKind::PackageModule => {
                let package =
                    HvigorPackageModuleSource::new(absolute_path, &self.path, &self.package_name)?;
                Ok(HvigorChunkSource::package_module(
                    self.package_name.clone(),
                    self.module_name.clone(),
                    self.package_version.clone(),
                    ArtifactPathSerializer::rel_to_slash(relative),
                    package,
                ))
            }
            HvigorModuleRootKind::LocalDependencyModule => {
                let package =
                    HvigorPackageModuleSource::new(absolute_path, &self.path, &self.package_name)?;
                Ok(HvigorChunkSource::local_dependency_module(
                    self.package_name.clone(),
                    self.module_name.clone(),
                    self.package_version.clone(),
                    ArtifactPathSerializer::rel_to_slash(relative),
                    package,
                ))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HvigorModuleRootKind {
    SourceModule,
    LocalDependencyModule,
    PackageModule,
}

impl HvigorModuleRootKind {
    pub(crate) fn from_root_path(path: &Path) -> Self {
        if arkdown_path::classify::is_inside_oh_modules(path) {
            Self::PackageModule
        } else {
            Self::SourceModule
        }
    }

    pub(crate) fn is_project_oh_modules_owner(self) -> bool {
        matches!(self, Self::SourceModule | Self::LocalDependencyModule)
    }
}

pub(crate) struct HvigorChunkSource {
    pub(crate) module_name: String,
    pub(crate) cache_module_name: String,
    pub(crate) package_version: String,
    pub(crate) source_file: String,
    pub(crate) kind: HvigorChunkSourceKind,
}

pub(crate) enum HvigorChunkSourceKind {
    SourceModule,
    LocalDependencyModule(HvigorPackageModuleSource),
    PackageModule(HvigorPackageModuleSource),
}

impl HvigorChunkSource {
    pub(crate) fn source_module(
        module_name: String,
        cache_module_name: String,
        package_version: String,
        source_file: String,
    ) -> Self {
        Self {
            cache_module_name,
            module_name,
            package_version,
            source_file,
            kind: HvigorChunkSourceKind::SourceModule,
        }
    }

    pub(crate) fn package_module(
        module_name: String,
        cache_module_name: String,
        package_version: String,
        source_file: String,
        package: HvigorPackageModuleSource,
    ) -> Self {
        Self {
            cache_module_name,
            module_name,
            package_version,
            source_file,
            kind: HvigorChunkSourceKind::PackageModule(package),
        }
    }

    pub(crate) fn local_dependency_module(
        module_name: String,
        cache_module_name: String,
        package_version: String,
        source_file: String,
        package: HvigorPackageModuleSource,
    ) -> Self {
        Self {
            cache_module_name,
            module_name,
            package_version,
            source_file,
            kind: HvigorChunkSourceKind::LocalDependencyModule(package),
        }
    }

    pub(crate) fn with_chunk_name(self, chunk_name: String) -> ChunkSourceFile {
        ChunkSourceFile {
            chunk_name,
            module_name: self.module_name,
            source_file: self.source_file,
        }
    }

    pub(crate) fn normalized_record_name(
        &self,
        runtime_packages: &BundleRuntimePackageInfo,
    ) -> Result<String, ArkdownError> {
        match &self.kind {
            HvigorChunkSourceKind::SourceModule => {
                let context = self.runtime_context(runtime_packages)?;
                let project_file_path = self.source_project_file_path();
                Ok(HvigorNormalizedOhmUrl::new(context, &project_file_path).to_record())
            }
            HvigorChunkSourceKind::PackageModule(package) => {
                package.to_normalized_record(runtime_packages)
            }
            HvigorChunkSourceKind::LocalDependencyModule(package) => {
                package.to_normalized_record(runtime_packages)
            }
        }
    }

    pub(crate) fn to_normalized_request(
        &self,
        runtime_packages: &BundleRuntimePackageInfo,
        _entry_module_name: &str,
    ) -> Result<String, ArkdownError> {
        match &self.kind {
            HvigorChunkSourceKind::SourceModule => {
                let context = self.runtime_context(runtime_packages)?;
                let project_file_path = self.source_project_file_path();
                Ok(HvigorNormalizedOhmUrl::new(context, &project_file_path).to_request())
            }
            HvigorChunkSourceKind::PackageModule(package)
            | HvigorChunkSourceKind::LocalDependencyModule(package) => {
                package.to_normalized_request(runtime_packages)
            }
        }
    }

    pub(crate) fn runtime_context<'a>(
        &self,
        runtime_packages: &'a BundleRuntimePackageInfo,
    ) -> Result<&'a BundleRuntimePackageContext, ArkdownError> {
        runtime_packages
            .packages
            .get(self.module_name.as_str())
            .ok_or_else(|| {
                ArkdownError::Bundler(format!(
                    "pkgContextInfo is missing runtime package context for '{}'",
                    self.module_name
                ))
            })
    }

    pub(crate) fn source_project_file_path(&self) -> String {
        ArtifactPathSerializer::rel_to_slash(
            &HvigorModuleSourceFileAttribution::project_source_file_for(&self.source_file),
        )
    }

    pub(crate) fn file_info_source_file(
        &self,
        entry_package_name: &str,
        entry_package_version: &str,
        has_dependency_package_info: bool,
        emits_javascript: bool,
    ) -> String {
        let mut source_file = match &self.kind {
            HvigorChunkSourceKind::SourceModule => {
                HvigorModuleSourceFileAttribution::project_source_file_for(&self.source_file)
            }
            HvigorChunkSourceKind::PackageModule(_) => {
                HvigorModuleSourceFileAttribution::package_source_file_for(&self.source_file)
            }
            HvigorChunkSourceKind::LocalDependencyModule(_) => {
                HvigorModuleSourceFileAttribution::package_source_file_for(&self.source_file)
            }
        };
        // SDK ModuleMode.processModuleInfos always writes `.mjs` and `.cjs`
        // modules to a `.js` cache path. addModuleInfoItem then derives the
        // filesInfo source attribution from that rewritten cache path.
        let module_mode_rewrites_extension = matches!(
            Path::new(&self.source_file)
                .extension()
                .and_then(|extension| extension.to_str()),
            Some("mjs" | "cjs")
        );
        if emits_javascript || module_mode_rewrites_extension {
            source_file.set_extension("js");
        }
        let source_file = ArtifactPathSerializer::rel_to_slash(&source_file);
        let (package_name, package_version) = if has_dependency_package_info {
            (self.module_name.as_str(), self.package_version.as_str())
        } else {
            (entry_package_name, entry_package_version)
        };
        format!(
            "{}|{}|{}|{}",
            entry_package_name, package_name, package_version, source_file
        )
    }
}

pub(crate) struct HvigorPackageModuleSource {
    pub(crate) package_name: String,
    pub(crate) package_root: PathBuf,
    pub(crate) project_file_path: String,
}

impl HvigorPackageModuleSource {
    pub(crate) fn new(
        absolute_path: &Path,
        package_root: &Path,
        package_name: &str,
    ) -> Result<Self, ArkdownError> {
        let project_file_path = absolute_path.strip_prefix(package_root).map_err(|error| {
            ArkdownError::Bundler(format!(
                "hvigor package module source is outside package root: {} not under {}: {}",
                absolute_path.display(),
                package_root.display(),
                error
            ))
        })?;
        let project_file_path = ArtifactPathSerializer::rel_to_slash(project_file_path);
        let package = OhPackage::load_from_root(package_root).ok_or_else(|| {
            ArkdownError::Bundler(format!(
                "hvigor package root for {package_name} does not contain package metadata: {}",
                package_root.display()
            ))
        })?;
        if package.version().is_empty() {
            return Err(ArkdownError::Bundler(format!(
                "hvigor package root for {package_name} does not contain package version metadata: {}",
                package_root.display()
            )));
        }
        Ok(Self {
            package_name: package_name.to_string(),
            package_root: package_root.to_path_buf(),
            project_file_path,
        })
    }

    /// `utils.genTemporaryPath` removes `meta.belongProjectPath` for package
    /// modules. For nested `.ohpm` paths this is the owner of the outermost
    /// `oh_modules`, which may be a local dependency project outside the host
    /// application root.
    pub(crate) fn belong_project_path(&self) -> &Path {
        self.package_root
            .ancestors()
            .filter(|ancestor| {
                ancestor
                    .file_name()
                    .is_some_and(|name| name == "oh_modules")
            })
            .last()
            .and_then(Path::parent)
            .unwrap_or(&self.package_root)
    }

    pub(crate) fn normalized_project_file_path<'a>(
        &'a self,
        _context: &BundleRuntimePackageContext,
    ) -> Cow<'a, str> {
        Cow::Borrowed(&self.project_file_path)
    }

    pub(crate) fn to_normalized_record(
        &self,
        runtime_packages: &BundleRuntimePackageInfo,
    ) -> Result<String, ArkdownError> {
        let context = self.runtime_context(runtime_packages)?;
        let project_file_path = self.normalized_project_file_path(context);
        Ok(HvigorNormalizedOhmUrl::new(context, project_file_path.as_ref()).to_record())
    }

    pub(crate) fn to_normalized_request(
        &self,
        runtime_packages: &BundleRuntimePackageInfo,
    ) -> Result<String, ArkdownError> {
        let context = self.runtime_context(runtime_packages)?;
        let project_file_path = self.normalized_project_file_path(context);
        Ok(HvigorNormalizedOhmUrl::new(context, project_file_path.as_ref()).to_request())
    }

    pub(crate) fn runtime_context<'a>(
        &self,
        runtime_packages: &'a BundleRuntimePackageInfo,
    ) -> Result<&'a BundleRuntimePackageContext, ArkdownError> {
        runtime_packages
            .packages
            .get(self.package_name.as_str())
            .ok_or_else(|| {
                ArkdownError::Bundler(format!(
                    "pkgContextInfo is missing runtime package context for '{}'",
                    self.package_name
                ))
            })
    }
}

pub(crate) struct HvigorModuleRelativeSourceFile<'a> {
    pub(crate) relative: &'a Path,
}

impl<'a> HvigorModuleRelativeSourceFile<'a> {
    pub(crate) fn new(relative: &'a Path) -> Self {
        Self { relative }
    }

    pub(crate) fn source_file(&self) -> String {
        let source = self
            .after_src_main()
            .unwrap_or_else(|| self.relative.to_path_buf());
        ArtifactPathSerializer::rel_to_slash(&source)
    }

    pub(crate) fn after_src_main(&self) -> Option<PathBuf> {
        // hvigor `ark_utils.getOhmUrlByFilepath` matches
        // `/src/(?:main|ohosTest)/(ets|js|mock)/` and maps the remainder to
        // `ets/...` — the `ohosTest` template tree
        // (`.test/<product>/intermediates/src/ohosTest/ets/...`) hits the
        // `src/ohosTest` arm.
        let components = self.relative.components().collect::<Vec<_>>();
        let src_main_index = components.windows(2).position(|window| {
            matches!(window[0], std::path::Component::Normal(component) if component == "src")
                && matches!(
                    window[1],
                    std::path::Component::Normal(component)
                        if component == "main" || component == "ohosTest"
                )
        })?;
        let mut path = PathBuf::new();
        for component in components.into_iter().skip(src_main_index + 2) {
            path.push(component.as_os_str());
        }
        (!path.as_os_str().is_empty()).then_some(path)
    }
}
