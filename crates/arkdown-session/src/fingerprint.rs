//! [`InputFingerprintSet`] — content hashes for all startup inputs.
//!
//! Cache keys must be built from `InputFingerprintSet` or explicit content
//! hashes. Placeholder hashes are forbidden.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arkdown_config::{ArkdownCustomConfig, RuntimeContextOverrides};
use arkdown_core::ArkdownConfig;
use arkdown_path::{lexical_normalize, to_slash};
use rayon::prelude::*;
use rustc_hash::FxHashMap;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::dependency_graph::snapshot::DependencyNodeType;
use crate::paths::ProjectPathLayout;
use crate::profiles::store::ProfileStore;
use crate::project::snapshot::ProjectSnapshot;
use crate::sdk::snapshot::SdkSnapshot;

/// A single content fingerprint (SHA-256 of file content).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Fingerprint(pub [u8; 32]);

impl Fingerprint {
    /// Compute the fingerprint of the given bytes.
    pub fn of_bytes(data: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&result);
        Self(bytes)
    }

    /// Compute the fingerprint of the given file.
    pub fn of_file(path: &std::path::Path) -> std::io::Result<Self> {
        let data = std::fs::read(path)?;
        Ok(Self::of_bytes(&data))
    }

    /// Serialize this fingerprint as a stable lowercase SHA-256 hex string.
    pub fn to_hex(&self) -> String {
        let mut output = String::with_capacity(self.0.len() * 2);
        for byte in self.0 {
            write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
        }
        output
    }
}

/// Content fingerprints scoped to one module's real compilation inputs.
///
/// Source and resource dependencies are included so a source HAR or resource
/// dependency change invalidates only consumers of that dependency instead of
/// every module in the project.
#[derive(Debug, Clone)]
pub struct ModuleInputFingerprints {
    /// Source files compiled or bundled for this module.
    pub sources: Fingerprint,
    /// Resource inputs linked into this module.
    pub resources: Fingerprint,
    /// Package manifests and lockfiles that define this module's resolution graph.
    pub dependencies: Fingerprint,
}

/// Errors produced while fingerprinting startup inputs.
#[derive(Debug, Error)]
pub enum FingerprintError {
    /// A file could not be read for hashing.
    #[error("failed to read fingerprint input {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// A directory walk failed.
    #[error("failed to scan fingerprint input {path}: {message}")]
    Walk { path: PathBuf, message: String },

    /// A structured startup value could not be serialized for hashing.
    #[error("failed to serialize fingerprint value {name}: {source}")]
    Serialize {
        name: &'static str,
        #[source]
        source: serde_json::Error,
    },
}

/// The set of all startup input fingerprints.
///
/// Used to build cache keys and to determine which stages need to re-run
/// when inputs change.
#[derive(Debug, Clone)]
pub struct InputFingerprintSet {
    /// Hash of the build configuration.
    pub config: Fingerprint,
    /// Hash of the dependency graph inputs (all `oh-package.json5` files).
    pub package_graph: Fingerprint,
    /// Hash of all profile JSON input files.
    pub profiles: Fingerprint,
    /// Hash of all resource input files.
    pub resources: Fingerprint,
    /// Hash of all source files.
    pub sources: Fingerprint,
    /// Hash of the selected SDK metadata.
    pub sdk: Fingerprint,
    /// Per-module content fingerprints used by stage cache keys.
    module_inputs: BTreeMap<String, ModuleInputFingerprints>,
}

impl InputFingerprintSet {
    /// Build the fingerprint set from normalized startup snapshots.
    pub fn build(
        config: &ArkdownConfig,
        project: &ProjectSnapshot,
        profiles: &ProfileStore,
        sdk: &SdkSnapshot,
    ) -> Result<Self, FingerprintError> {
        Self::build_with_runtime_overrides(config, project, profiles, sdk, None)
    }

    /// Build fingerprints while including in-memory hvigor loader values.
    pub fn build_with_runtime_overrides(
        config: &ArkdownConfig,
        project: &ProjectSnapshot,
        profiles: &ProfileStore,
        sdk: &SdkSnapshot,
        runtime_overrides: Option<&RuntimeContextOverrides>,
    ) -> Result<Self, FingerprintError> {
        Self::build_with_custom_config(
            config,
            project,
            profiles,
            sdk,
            runtime_overrides,
            &ArkdownCustomConfig::default(),
        )
    }

    /// Build fingerprints including the selected Arkdown-owned configuration
    /// file. Its content affects generated BuildProfile constants and bundler
    /// options, so it belongs in the global configuration fingerprint.
    pub fn build_with_custom_config(
        config: &ArkdownConfig,
        project: &ProjectSnapshot,
        profiles: &ProfileStore,
        sdk: &SdkSnapshot,
        runtime_overrides: Option<&RuntimeContextOverrides>,
        custom_config: &ArkdownCustomConfig,
    ) -> Result<Self, FingerprintError> {
        let project_inputs = ProjectFingerprintInventory::new(project).build()?;
        Ok(Self {
            config: Self::compute_config_hash(
                config,
                project,
                runtime_overrides,
                custom_config.source_path(),
            )?,
            package_graph: Self::compute_package_hash(project, runtime_overrides)?,
            profiles: profiles.fingerprint(),
            resources: project_inputs.resources,
            sources: project_inputs.sources,
            sdk: Self::compute_sdk_hash(sdk)?,
            module_inputs: project_inputs.modules,
        })
    }

    /// Return content fingerprints for one module's source, resources, and
    /// dependency metadata.
    pub fn module(&self, module_name: &str) -> Option<&ModuleInputFingerprints> {
        self.module_inputs.get(module_name)
    }

    fn compute_config_hash(
        config: &ArkdownConfig,
        project: &ProjectSnapshot,
        runtime_overrides: Option<&RuntimeContextOverrides>,
        custom_config_path: Option<&Path>,
    ) -> Result<Fingerprint, FingerprintError> {
        let mut builder = FingerprintBuilder::new("config", &project.root);
        let encoded = serde_json::to_vec(config).map_err(|source| FingerprintError::Serialize {
            name: "ArkdownConfig",
            source,
        })?;
        builder.add_value("normalized_config", &encoded);
        if let Some(runtime_overrides) = runtime_overrides {
            let encoded = serde_json::to_vec(runtime_overrides).map_err(|source| {
                FingerprintError::Serialize {
                    name: "RuntimeContextOverrides",
                    source,
                }
            })?;
            builder.add_value("runtime_context_overrides", &encoded);
        }
        builder.add_file_if_present(&project.root.join("build-profile.json5"))?;
        if let Some(path) = custom_config_path {
            builder.add_file(path)?;
        }
        builder
            .add_file_if_present(&ProjectPathLayout::new(&project.root).app_scope_app_json5())?;
        for module in project.modules.values() {
            if let Some(path) = &module.module_json_path {
                builder.add_file(path)?;
            }
        }
        Ok(builder.finish())
    }

    fn compute_package_hash(
        project: &ProjectSnapshot,
        runtime_overrides: Option<&RuntimeContextOverrides>,
    ) -> Result<Fingerprint, FingerprintError> {
        let mut builder = FingerprintBuilder::new("package_graph", &project.root);
        let mut path_inventory = FingerprintPathInventory::default();
        for path in path_inventory.named_files_under(&project.root, "oh-package.json5")? {
            builder.add_file(&path)?;
        }
        if let Some(runtime_overrides) = runtime_overrides {
            let encoded = serde_json::to_vec(runtime_overrides).map_err(|source| {
                FingerprintError::Serialize {
                    name: "RuntimeContextOverrides",
                    source,
                }
            })?;
            builder.add_value("runtime_context_overrides", &encoded);
        }
        Ok(builder.finish())
    }

    fn compute_sdk_hash(sdk: &SdkSnapshot) -> Result<Fingerprint, FingerprintError> {
        let mut builder = FingerprintBuilder::new("sdk", &sdk.sdk_root);
        builder.add_value("sdk_root", to_slash(&sdk.sdk_root).as_bytes());
        builder.add_value("api_version", sdk.api_version.to_string().as_bytes());
        builder.add_value(
            "component_api_version",
            sdk.component_api_version.to_string().as_bytes(),
        );
        builder.add_value("compile_sdk_version", sdk.compile_sdk_version.as_bytes());

        for relative in [
            "ets/oh-uni-package.json",
            "native/oh-uni-package.json",
            "toolchains/oh-uni-package.json",
            "js/oh-uni-package.json",
            "previewer/oh-uni-package.json",
            "ets/build-tools/ets-loader/package.json",
            "js/build-tools/js-loader/package.json",
        ] {
            builder.add_file_if_present(&sdk.sdk_root.join(relative))?;
        }

        for schema in [
            crate::sdk_schemas::SdkSchemaKind::App,
            crate::sdk_schemas::SdkSchemaKind::Module,
            crate::sdk_schemas::SdkSchemaKind::Page,
            crate::sdk_schemas::SdkSchemaKind::Form,
            crate::sdk_schemas::SdkSchemaKind::RouterMap,
            crate::sdk_schemas::SdkSchemaKind::ShareConfig,
            crate::sdk_schemas::SdkSchemaKind::AppStartup,
            crate::sdk_schemas::SdkSchemaKind::HarStartup,
            crate::sdk_schemas::SdkSchemaKind::HspStartup,
            crate::sdk_schemas::SdkSchemaKind::Syscap,
            crate::sdk_schemas::SdkSchemaKind::Pac,
            crate::sdk_schemas::SdkSchemaKind::Shortcuts,
            crate::sdk_schemas::SdkSchemaKind::InsightIntent,
        ] {
            if let Some(path) = sdk.schema_path(schema) {
                builder.add_file_if_present(path.as_path())?;
            }
        }

        Ok(builder.finish())
    }
}

#[derive(Debug, Default)]
struct ModuleInputPaths {
    sources: BTreeSet<PathBuf>,
    resources: BTreeSet<PathBuf>,
    dependencies: BTreeSet<PathBuf>,
}

#[derive(Debug)]
struct ProjectFingerprintInventory<'a> {
    project: &'a ProjectSnapshot,
    paths: FingerprintPathInventory,
}

impl<'a> ProjectFingerprintInventory<'a> {
    fn new(project: &'a ProjectSnapshot) -> Self {
        Self {
            project,
            paths: FingerprintPathInventory::default(),
        }
    }

    fn build(mut self) -> Result<ProjectFingerprintOutput, FingerprintError> {
        let mut paths_by_module = BTreeMap::new();
        for module_name in &self.project.module_order {
            paths_by_module.insert(module_name.clone(), self.module_paths(module_name)?);
        }

        let all_paths = paths_by_module
            .values()
            .flat_map(|paths| {
                paths
                    .sources
                    .iter()
                    .chain(paths.resources.iter())
                    .chain(paths.dependencies.iter())
            })
            .cloned()
            .collect::<BTreeSet<_>>();
        let content = ProjectContentFingerprints::read(all_paths)?;

        let mut global_sources = FingerprintBuilder::new("sources", &self.project.root);
        let mut global_resources = FingerprintBuilder::new("resources", &self.project.root);
        let mut modules = BTreeMap::new();
        for (module_name, paths) in paths_by_module {
            let mut source_builder = FingerprintBuilder::new("module_sources", &self.project.root);
            let mut resource_builder =
                FingerprintBuilder::new("module_resources", &self.project.root);
            let mut dependency_builder =
                FingerprintBuilder::new("module_dependencies", &self.project.root);

            content.add_all(&mut source_builder, &paths.sources);
            content.add_all(&mut resource_builder, &paths.resources);
            content.add_all(&mut dependency_builder, &paths.dependencies);
            content.add_all(&mut global_sources, &paths.sources);
            content.add_all(&mut global_resources, &paths.resources);

            modules.insert(
                module_name,
                ModuleInputFingerprints {
                    sources: source_builder.finish(),
                    resources: resource_builder.finish(),
                    dependencies: dependency_builder.finish(),
                },
            );
        }

        Ok(ProjectFingerprintOutput {
            sources: global_sources.finish(),
            resources: global_resources.finish(),
            modules,
        })
    }

    fn module_paths(&mut self, module_name: &str) -> Result<ModuleInputPaths, FingerprintError> {
        let Some(module) = self.project.modules.get(module_name).cloned() else {
            return Ok(ModuleInputPaths::default());
        };
        let mut paths = ModuleInputPaths::default();
        self.paths
            .add_source_roots(&mut paths.sources, &module.source_roots)?;
        self.paths
            .add_resource_roots(&mut paths.resources, &module.resource_roots)?;
        if let Some(package_path) = &module.package_path {
            paths.dependencies.insert(package_path.clone());
        }
        if let Some(package) = module.package.as_deref() {
            if let Some(exports) = package.package_json.oh_exports.as_deref() {
                for entry in exports {
                    self.paths
                        .add_source_path(&mut paths.sources, &package.root.join(entry))?;
                }
            }
        }
        if let Some(module_json_path) = &module.module_json_path {
            paths.dependencies.insert(module_json_path.clone());
        }

        let project_layout = ProjectPathLayout::new(&self.project.root);
        for path in [
            self.project.root.join("oh-package.json5"),
            self.project.root.join("oh-package-lock.json5"),
            self.project.root.join("build-profile.json5"),
            project_layout.app_scope_app_json5(),
        ] {
            if path.is_file() {
                paths.dependencies.insert(path);
            }
        }

        if let Some(graph) = self.project.dependency_graphs.get(module_name).cloned() {
            for node in graph.nodes.values() {
                let Some(package_root) = node.package_root.as_ref() else {
                    continue;
                };
                for manifest in [
                    package_root.join("oh-package.json5"),
                    package_root.join("src").join("main").join("module.json5"),
                    package_root.join("src").join("main").join("module.json"),
                ] {
                    if manifest.is_file() {
                        paths.dependencies.insert(manifest);
                    }
                }
                if matches!(
                    node.node_type,
                    DependencyNodeType::SourceHar | DependencyNodeType::OhpmSource
                ) {
                    self.paths.add_source_roots(
                        &mut paths.sources,
                        &[package_root.join("src").join("main")],
                    )?;
                    if let Some(package) = &node.package {
                        for entry in [
                            package.package_json.main.as_deref(),
                            package.package_json.types.as_deref(),
                        ]
                        .into_iter()
                        .flatten()
                        {
                            self.paths
                                .add_source_path(&mut paths.sources, &package_root.join(entry))?;
                        }
                        if let Some(exports) = package.package_json.oh_exports.as_deref() {
                            for entry in exports {
                                self.paths.add_source_path(
                                    &mut paths.sources,
                                    &package_root.join(entry),
                                )?;
                            }
                        }
                    }
                }
                if matches!(
                    node.node_type,
                    DependencyNodeType::SourceHar
                        | DependencyNodeType::SharedHsp
                        | DependencyNodeType::IntegratedHsp
                ) {
                    self.paths.add_resource_roots(
                        &mut paths.resources,
                        &[package_root.join("src").join("main").join("resources")],
                    )?;
                }
            }
        }

        let app_scope_resources = project_layout.app_scope_resources();
        self.paths
            .add_resource_roots(&mut paths.resources, &[app_scope_resources])?;
        Ok(paths)
    }
}

/// Sorted filesystem path snapshots reused by every module fingerprint.
///
/// A large workspace often exposes the same source HAR roots through dozens
/// of host-module graphs. The first lookup freezes the directory contents for
/// this build session; later lookups reuse the immutable list instead of
/// walking the same tree again.
#[derive(Debug, Default)]
struct FingerprintPathInventory {
    files_by_root: FxHashMap<PathBuf, Arc<[PathBuf]>>,
}

impl FingerprintPathInventory {
    fn named_files_under(
        &mut self,
        root: &Path,
        name: &str,
    ) -> Result<Vec<PathBuf>, FingerprintError> {
        Ok(self
            .files_under(root)?
            .iter()
            .filter(|path| path.file_name().and_then(|value| value.to_str()) == Some(name))
            .cloned()
            .collect())
    }

    fn add_source_path(
        &mut self,
        output: &mut BTreeSet<PathBuf>,
        path: &Path,
    ) -> Result<(), FingerprintError> {
        if path.is_dir() {
            return self.add_source_roots(output, &[path.to_path_buf()]);
        }
        if path.is_file() && Self::is_source_input(path) {
            output.insert(path.to_path_buf());
        }
        Ok(())
    }

    fn add_source_roots(
        &mut self,
        output: &mut BTreeSet<PathBuf>,
        roots: &[PathBuf],
    ) -> Result<(), FingerprintError> {
        for root in roots {
            for path in self.files_under(root)?.iter() {
                if Self::is_source_input(path) {
                    output.insert(path.clone());
                }
            }
        }
        Ok(())
    }

    fn add_resource_roots(
        &mut self,
        output: &mut BTreeSet<PathBuf>,
        roots: &[PathBuf],
    ) -> Result<(), FingerprintError> {
        for root in roots {
            output.extend(self.files_under(root)?.iter().cloned());
        }
        Ok(())
    }

    fn files_under(&mut self, root: &Path) -> Result<Arc<[PathBuf]>, FingerprintError> {
        if let Some(files) = self.files_by_root.get(root) {
            return Ok(Arc::clone(files));
        }
        if !root.is_dir() {
            let files = Arc::from([]);
            self.files_by_root
                .insert(root.to_path_buf(), Arc::clone(&files));
            return Ok(files);
        }
        let mut files = Vec::new();
        for entry in walkdir::WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|entry| !Self::is_generated_or_external_dir(entry.path(), root))
        {
            let entry = entry.map_err(|error| FingerprintError::Walk {
                path: error
                    .path()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| root.to_path_buf()),
                message: error.to_string(),
            })?;
            if entry.file_type().is_file() {
                files.push(entry.path().to_path_buf());
            }
        }
        files.sort();
        let files = Arc::from(files);
        self.files_by_root
            .insert(root.to_path_buf(), Arc::clone(&files));
        Ok(files)
    }

    fn is_generated_or_external_dir(path: &Path, root: &Path) -> bool {
        if path == root {
            return false;
        }
        matches!(
            path.file_name().and_then(|value| value.to_str()),
            Some(".git" | "build" | "out" | "node_modules")
        )
    }

    fn is_source_input(path: &Path) -> bool {
        path.extension()
            .and_then(|value| value.to_str())
            .is_some_and(crate::source_store::is_source_inventory_ext)
    }
}

#[derive(Debug)]
struct ProjectContentFingerprints {
    files: BTreeMap<PathBuf, Fingerprint>,
}

impl ProjectContentFingerprints {
    fn read(paths: BTreeSet<PathBuf>) -> Result<Self, FingerprintError> {
        let paths = paths.into_iter().collect::<Vec<_>>();
        let results = paths
            .par_iter()
            .map(|path| {
                Fingerprint::of_file(path)
                    .map(|fingerprint| (path.clone(), fingerprint))
                    .map_err(|source| FingerprintError::Io {
                        path: path.clone(),
                        source,
                    })
            })
            .collect::<Vec<_>>();
        let mut files = BTreeMap::new();
        for result in results {
            let (path, fingerprint) = result?;
            files.insert(path, fingerprint);
        }
        Ok(Self { files })
    }

    fn add_all(&self, builder: &mut FingerprintBuilder, paths: &BTreeSet<PathBuf>) {
        for path in paths {
            if let Some(fingerprint) = self.files.get(path) {
                builder.add_file_fingerprint(path, fingerprint);
            }
        }
    }
}

#[derive(Debug)]
struct ProjectFingerprintOutput {
    sources: Fingerprint,
    resources: Fingerprint,
    modules: BTreeMap<String, ModuleInputFingerprints>,
}

/// Builder for deterministic, typed fingerprint inputs.
#[derive(Debug)]
pub struct FingerprintBuilder {
    base: PathBuf,
    hasher: Sha256,
    entries: usize,
}

impl FingerprintBuilder {
    /// Create a builder for one logical fingerprint kind.
    pub fn new(kind: &'static str, base: &Path) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(b"arkdown:fingerprint:v1");
        hasher.update([0]);
        hasher.update(kind.as_bytes());
        hasher.update([0]);
        Self {
            base: lexical_normalize(base),
            hasher,
            entries: 0,
        }
    }

    /// Add a structured value to the fingerprint.
    pub fn add_value(&mut self, name: &'static str, value: &[u8]) {
        self.hasher.update(b"value");
        self.hasher.update([0]);
        self.hasher.update(name.as_bytes());
        self.hasher.update([0]);
        self.hasher.update((value.len() as u64).to_le_bytes());
        self.hasher.update([0]);
        self.hasher.update(value);
        self.hasher.update([0xff]);
        self.entries += 1;
    }

    /// Add a file path and content to the fingerprint.
    pub fn add_file(&mut self, path: &Path) -> Result<(), FingerprintError> {
        let normalized = lexical_normalize(path);
        let data = std::fs::read(&normalized).map_err(|source| FingerprintError::Io {
            path: normalized.clone(),
            source,
        })?;
        let key = self.path_key(&normalized);
        self.hasher.update(b"file");
        self.hasher.update([0]);
        self.hasher.update(key.as_bytes());
        self.hasher.update([0]);
        self.hasher.update((data.len() as u64).to_le_bytes());
        self.hasher.update([0]);
        self.hasher.update(&data);
        self.hasher.update([0xff]);
        self.entries += 1;
        Ok(())
    }

    pub(crate) fn add_file_fingerprint(&mut self, path: &Path, fingerprint: &Fingerprint) {
        let normalized = lexical_normalize(path);
        let key = self.path_key(&normalized);
        self.hasher.update(b"file_fingerprint");
        self.hasher.update([0]);
        self.hasher.update(key.as_bytes());
        self.hasher.update([0]);
        self.hasher.update(fingerprint.0);
        self.hasher.update([0xff]);
        self.entries += 1;
    }

    fn add_file_if_present(&mut self, path: &Path) -> Result<(), FingerprintError> {
        if path.is_file() {
            self.add_file(path)?;
        }
        Ok(())
    }

    fn path_key(&self, path: &Path) -> String {
        path.strip_prefix(&self.base)
            .map(to_slash)
            .unwrap_or_else(|_| to_slash(path))
    }

    /// Finish the builder.
    pub fn finish(mut self) -> Fingerprint {
        if self.entries == 0 {
            self.hasher.update(b"empty");
        }
        let result = self.hasher.finalize();
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&result);
        Fingerprint(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::FingerprintPathInventory;

    #[test]
    fn path_inventory_freezes_one_sorted_walk_per_session_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("src");
        std::fs::create_dir_all(&root).expect("source root");
        std::fs::write(root.join("b.ets"), "b").expect("b source");
        std::fs::write(root.join("a.ets"), "a").expect("a source");

        let mut inventory = FingerprintPathInventory::default();
        let first = inventory.files_under(&root).expect("first inventory");
        assert_eq!(first.as_ref(), &[root.join("a.ets"), root.join("b.ets")]);

        std::fs::write(root.join("c.ets"), "c").expect("late source");
        let reused = inventory.files_under(&root).expect("reused inventory");
        assert_eq!(reused.as_ref(), first.as_ref());
    }
}
