use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use arkdown::plugins::ets_transform::{TransformResolvedStaticModule, TransformSourceSnapshot};
use arkdown_ets_transform::SourceClassifier;
use arkdown_path::{canonicalize_or_owned, lexical_normalize, HvigorSourceFileFilter};
use arkdown_resolver::OhPackage;
use arkdown_session::SourceStore;
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::layout::ModuleSourceLayout;

/// Builds the immutable source inventory shared by OXC transforms, Rolldown,
/// and the structural checker.
pub struct TransformSourceSnapshotBuilder<'a> {
    module_roots: &'a [(String, PathBuf)],
    dependency_source_roots: &'a [PathBuf],
    native_declaration_roots: &'a [PathBuf],
    sdk_declaration_roots: &'a [PathBuf],
    custom_type_declaration_inputs: &'a [PathBuf],
    source_set_overlays: &'a [(PathBuf, PathBuf)],
    package_roots: &'a [(String, PathBuf)],
    /// Local unit-test source roots (`<module>/.test/testability`,
    /// `<module>/src/test`) used by the JavaScript `test` command chain.
    test_source_roots: &'a [PathBuf],
    /// Immutable project/package source inventory captured by `BuildSession`.
    source_store: Option<&'a SourceStore>,
}

impl<'a> TransformSourceSnapshotBuilder<'a> {
    pub fn new(
        module_roots: &'a [(String, PathBuf)],
        dependency_source_roots: &'a [PathBuf],
        native_declaration_roots: &'a [PathBuf],
        source_set_overlays: &'a [(PathBuf, PathBuf)],
    ) -> Self {
        Self {
            module_roots,
            dependency_source_roots,
            native_declaration_roots,
            sdk_declaration_roots: &[],
            custom_type_declaration_inputs: &[],
            source_set_overlays,
            package_roots: &[],
            test_source_roots: &[],
            source_store: None,
        }
    }

    pub fn with_source_store(mut self, source_store: &'a SourceStore) -> Self {
        self.source_store = Some(source_store);
        self
    }

    pub fn with_test_source_roots(mut self, roots: &'a [PathBuf]) -> Self {
        self.test_source_roots = roots;
        self
    }

    pub fn with_package_roots(mut self, roots: &'a [(String, PathBuf)]) -> Self {
        self.package_roots = roots;
        self
    }

    pub fn with_custom_type_declaration_inputs(mut self, inputs: &'a [PathBuf]) -> Self {
        self.custom_type_declaration_inputs = inputs;
        self
    }

    pub fn with_sdk_declaration_roots(mut self, roots: &'a [PathBuf]) -> Self {
        self.sdk_declaration_roots = roots;
        self
    }

    pub fn build(&self) -> Result<TransformSourceSnapshot> {
        let mut files = FxHashMap::default();
        let mut runtime_source_paths = FxHashSet::default();
        let mut collected_roots = FxHashSet::default();
        for (_, module_root) in self.module_roots {
            self.collect_source_root(
                &ModuleSourceLayout::new(module_root).ets(),
                &mut files,
                &mut runtime_source_paths,
                &mut collected_roots,
            )?;
        }
        for root in self.dependency_source_roots {
            self.collect_source_root(
                root,
                &mut files,
                &mut runtime_source_paths,
                &mut collected_roots,
            )?;
        }
        for root in self.test_source_roots {
            self.collect_source_root(
                root,
                &mut files,
                &mut runtime_source_paths,
                &mut collected_roots,
            )?;
        }
        for root in self.native_declaration_roots {
            self.collect_declaration_root(
                root,
                &mut files,
                &mut runtime_source_paths,
                &mut collected_roots,
            )?;
        }
        for root in self.sdk_declaration_roots {
            self.collect_declaration_root(
                root,
                &mut files,
                &mut runtime_source_paths,
                &mut collected_roots,
            )?;
        }
        for input in self.custom_type_declaration_inputs {
            self.collect_declaration_input(
                input,
                &mut files,
                &mut runtime_source_paths,
                &mut collected_roots,
            )?;
        }
        for (base_root, target_root) in self.source_set_overlays {
            self.collect_source_root(
                base_root,
                &mut files,
                &mut runtime_source_paths,
                &mut collected_roots,
            )?;
            self.collect_source_root(
                target_root,
                &mut files,
                &mut runtime_source_paths,
                &mut collected_roots,
            )?;
        }
        let resolved_static_modules = self.collect_resolved_static_modules();
        Ok(
            TransformSourceSnapshot::with_resolved_static_modules_and_runtime_sources(
                files,
                resolved_static_modules,
                runtime_source_paths,
            )
            .with_path_aliases(self.source_path_aliases())
            .with_sdk_declaration_roots(self.sdk_declaration_roots.to_vec())
            .with_package_roots(
                self.package_roots
                    .iter()
                    .map(|(name, root)| (name.clone(), root.clone()))
                    .collect(),
            ),
        )
    }

    fn source_path_aliases(&self) -> Vec<(PathBuf, PathBuf)> {
        let source_paths = self
            .module_roots
            .iter()
            .map(|(_, root)| root)
            .chain(self.dependency_source_roots.iter())
            .chain(self.native_declaration_roots.iter())
            .chain(self.sdk_declaration_roots.iter())
            .chain(self.custom_type_declaration_inputs.iter())
            .chain(
                self.source_set_overlays
                    .iter()
                    .flat_map(|(base, target)| [base, target]),
            )
            .chain(self.package_roots.iter().map(|(_, root)| root))
            .chain(self.test_source_roots.iter())
            .map(|path| lexical_normalize(path))
            .collect::<BTreeSet<_>>();
        let ancestor_paths = source_paths
            .iter()
            .flat_map(|path| path.ancestors().map(Path::to_path_buf))
            .collect::<BTreeSet<_>>();
        let mut candidates = ancestor_paths
            .into_iter()
            .filter_map(|alias| {
                let canonical = canonicalize_or_owned(&alias);
                (canonical != alias).then_some((alias, canonical))
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        candidates.sort_by_key(|(alias, _)| alias.components().count());

        let mut aliases = Vec::<(PathBuf, PathBuf)>::new();
        for (alias, canonical) in candidates {
            let covered = aliases.iter().any(|(parent_alias, parent_canonical)| {
                alias.strip_prefix(parent_alias).is_ok_and(|relative| {
                    lexical_normalize(&parent_canonical.join(relative)) == canonical
                })
            });
            if !covered {
                aliases.push((alias, canonical));
            }
        }
        aliases
    }

    fn collect_source_root(
        &self,
        root: &Path,
        files: &mut FxHashMap<PathBuf, Arc<str>>,
        runtime_source_paths: &mut FxHashSet<PathBuf>,
        collected_roots: &mut FxHashSet<PathBuf>,
    ) -> Result<()> {
        self.collect_root(
            root,
            files,
            runtime_source_paths,
            collected_roots,
            SourceRootMode::TransformAndDeclarations,
        )
    }

    fn collect_declaration_root(
        &self,
        root: &Path,
        files: &mut FxHashMap<PathBuf, Arc<str>>,
        runtime_source_paths: &mut FxHashSet<PathBuf>,
        collected_roots: &mut FxHashSet<PathBuf>,
    ) -> Result<()> {
        self.collect_root(
            root,
            files,
            runtime_source_paths,
            collected_roots,
            SourceRootMode::DeclarationsOnly,
        )
    }

    fn collect_declaration_input(
        &self,
        input: &Path,
        files: &mut FxHashMap<PathBuf, Arc<str>>,
        runtime_source_paths: &mut FxHashSet<PathBuf>,
        collected_roots: &mut FxHashSet<PathBuf>,
    ) -> Result<()> {
        if input.is_dir() {
            return self.collect_declaration_root(
                input,
                files,
                runtime_source_paths,
                collected_roots,
            );
        }
        if input.is_file() && SourceClassifier::classify_path(input).is_declaration() {
            let path = canonicalize_or_owned(input);
            let source = Arc::<str>::from(fs_err::read_to_string(&path)?);
            files.insert(path, source);
        }
        Ok(())
    }

    fn collect_root(
        &self,
        root: &Path,
        files: &mut FxHashMap<PathBuf, Arc<str>>,
        runtime_source_paths: &mut FxHashSet<PathBuf>,
        collected_roots: &mut FxHashSet<PathBuf>,
        mode: SourceRootMode,
    ) -> Result<()> {
        if !root.is_dir() {
            return Ok(());
        }
        let root = canonicalize_or_owned(root);
        if !collected_roots.insert(lexical_normalize(&root)) {
            return Ok(());
        }

        if let Some(source_store) = self.source_store.filter(|store| store.covers(&root)) {
            for (path, source) in source_store.text_sources_under(&root) {
                let kind = SourceClassifier::classify_path(path);
                if matches!(mode, SourceRootMode::TransformAndDeclarations)
                    && kind.is_javascript_runtime()
                {
                    runtime_source_paths.insert(path.to_path_buf());
                }
                let accepts = match mode {
                    SourceRootMode::TransformAndDeclarations => {
                        kind.is_ets_transform_input()
                            || kind.is_declaration()
                            || matches!(
                                path.extension().and_then(|extension| extension.to_str()),
                                Some("json" | "json5")
                            )
                    }
                    SourceRootMode::DeclarationsOnly => kind.is_declaration(),
                };
                if accepts {
                    files.insert(path.to_path_buf(), source);
                }
            }
            return Ok(());
        }

        let mut paths = Vec::new();
        for entry in walkdir::WalkDir::new(&root)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().is_file())
        {
            if HvigorSourceFileFilter::new(entry.path()).is_ignored_macos_metadata() {
                continue;
            }
            let kind = SourceClassifier::classify_path(entry.path());
            if matches!(mode, SourceRootMode::TransformAndDeclarations)
                && kind.is_javascript_runtime()
            {
                runtime_source_paths.insert(canonicalize_or_owned(entry.path()));
            }
            let accepts = match mode {
                SourceRootMode::TransformAndDeclarations => {
                    kind.is_ets_transform_input()
                        || kind.is_declaration()
                        || matches!(
                            entry
                                .path()
                                .extension()
                                .and_then(|extension| extension.to_str()),
                            Some("json" | "json5")
                        )
                }
                SourceRootMode::DeclarationsOnly => kind.is_declaration(),
            };
            if accepts {
                paths.push(entry.into_path());
            }
        }
        paths.sort();

        let results = paths
            .par_iter()
            .map(|path| {
                fs_err::read_to_string(path)
                    .map(|source| (canonicalize_or_owned(path), Arc::<str>::from(source)))
            })
            .collect::<Vec<_>>();
        for result in results {
            let (path, source) = result?;
            files.insert(path, source);
        }
        Ok(())
    }

    fn collect_resolved_static_modules(&self) -> Vec<TransformResolvedStaticModule> {
        let mut package_roots = BTreeSet::new();
        for (_, module_root) in self.module_roots {
            package_roots.insert(canonicalize_or_owned(module_root));
        }
        for root in self.dependency_source_roots {
            package_roots.insert(canonicalize_or_owned(root));
        }

        let mut modules = Vec::new();
        for package_root in package_roots {
            self.collect_package_resolved_static_modules(&package_root, &mut modules);
        }
        modules
    }

    fn collect_package_resolved_static_modules(
        &self,
        package_root: &Path,
        modules: &mut Vec<TransformResolvedStaticModule>,
    ) {
        let Some(package) = OhPackage::load_from_root(package_root) else {
            return;
        };
        for (specifier, dependency_root) in package.file_dependencies() {
            let dependency_root = canonicalize_or_owned(&dependency_root);
            let Some(dependency_package) = OhPackage::load_from_root(&dependency_root) else {
                continue;
            };
            let Some(declaration_entry) = dependency_package.existing_declaration_entry() else {
                continue;
            };
            modules.push(TransformResolvedStaticModule::new(
                package_root,
                specifier,
                declaration_entry,
            ));
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum SourceRootMode {
    TransformAndDeclarations,
    DeclarationsOnly,
}
