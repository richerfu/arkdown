//! [`DependencyGraphBuilder`] — builds one dependency graph per host module.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arkdown_core::OhosModuleType;
use arkdown_path::{
    canonicalize_or_owned, strip_relative_prefix, HvigorSourceFileFilter, ModuleSpecifierText,
    SourceFileName,
};
use indexmap::{IndexMap, IndexSet};
use rustc_hash::FxHashMap;
use serde::Deserialize;

use crate::dependency_graph::snapshot::{
    DependencyCompileInputSnapshot, DependencyEdgeKind, DependencyEdgeSnapshot,
    DependencyGraphSnapshot, DependencyModuleProfileSnapshot, DependencyNodeSnapshot,
    DependencyNodeType, DependencyParameterSnapshot,
};
use crate::package::snapshot::PackageSnapshot;
use crate::paths::ModulePathLayout;

/// Build-session inventory for dependency package and module-profile snapshots.
///
/// Latest hvigor keeps dependency nodes in its process-level `DEP_NODE` cache
/// and reuses their parsed package/profile objects across module dependency
/// managers. Arkdown keeps the equivalent cache scoped to one immutable build
/// session so later module graphs observe the same startup inputs without
/// re-reading manifests.
#[derive(Debug, Default)]
pub(crate) struct DependencySnapshotInventory {
    packages: FxHashMap<PathBuf, Option<Arc<PackageSnapshot>>>,
    module_profiles: FxHashMap<PathBuf, Option<DependencyModuleProfileSnapshot>>,
    compile_inputs: FxHashMap<PathBuf, Arc<DependencyCompileInputSnapshot>>,
    source_files: FxHashMap<PathBuf, Arc<[PathBuf]>>,
    resolved_roots: FxHashMap<(PathBuf, PathBuf, String), Option<PathBuf>>,
    canonical_roots: FxHashMap<PathBuf, PathBuf>,
}

#[derive(Clone, Copy)]
struct DependencyTraversalState {
    kind: DependencyEdgeKind,
    is_transitive: bool,
    is_dynamic: bool,
    is_runtime_only: bool,
}

struct DependencyCollectionState<'a> {
    queue: &'a mut VecDeque<(PathBuf, usize)>,
    seen_roots: &'a mut std::collections::BTreeSet<PathBuf>,
    graph: &'a mut DependencyGraphSnapshot,
    inventory: &'a mut DependencySnapshotInventory,
}

#[derive(Clone, Copy)]
enum DependencyPackageManager {
    Ohpm,
    Npm,
}

impl DependencyPackageManager {
    fn from_ohpm_project(use_ohpm: bool) -> Self {
        if use_ohpm {
            Self::Ohpm
        } else {
            Self::Npm
        }
    }

    fn modules_directory(self) -> &'static str {
        match self {
            Self::Ohpm => "oh_modules",
            Self::Npm => "node_modules",
        }
    }

    fn manifest_name(self) -> &'static str {
        match self {
            Self::Ohpm => "oh-package.json5",
            Self::Npm => "package.json",
        }
    }
}

impl DependencySnapshotInventory {
    pub(crate) fn insert_package(
        &mut self,
        package_root: &Path,
        package: Option<Arc<PackageSnapshot>>,
    ) {
        self.packages.insert(package_root.to_path_buf(), package);
    }

    fn canonical_root(&mut self, root: &Path) -> PathBuf {
        if let Some(canonical) = self.canonical_roots.get(root) {
            return canonical.clone();
        }
        let canonical = canonicalize_or_owned(root);
        self.canonical_roots
            .insert(root.to_path_buf(), canonical.clone());
        canonical
    }

    fn resolve_package_root(
        &mut self,
        issuer_root: &Path,
        project_root: &Path,
        alias: &str,
        package_manager: DependencyPackageManager,
    ) -> Option<PathBuf> {
        let key = (
            issuer_root.to_path_buf(),
            project_root.to_path_buf(),
            alias.to_string(),
        );
        if let Some(root) = self.resolved_roots.get(&key) {
            return root.clone();
        }

        // Hvigor's `BasePackageResolver.resolvePackagePath()` walks from the
        // issuer to the filesystem root through the package-manager-specific
        // installation directory. The raw version specifier is not a
        // fallback package root.
        let modules_directory = package_manager.modules_directory();
        let mut current = Some(issuer_root);
        let mut resolved = None;
        while let Some(root) = current {
            let search_root = root
                .file_name()
                .filter(|name| {
                    name.to_string_lossy()
                        .eq_ignore_ascii_case(modules_directory)
                })
                .and_then(|_| root.parent())
                .unwrap_or(root);
            let candidate = search_root.join(modules_directory).join(alias);
            if candidate.is_dir() {
                resolved = Some(self.canonical_root(&candidate));
                break;
            }
            current = search_root.parent();
        }
        self.resolved_roots.insert(key, resolved.clone());
        resolved
    }

    fn read_package(
        &mut self,
        package_root: &Path,
        package_manager: DependencyPackageManager,
    ) -> Option<Arc<PackageSnapshot>> {
        if let Some(package) = self.packages.get(package_root) {
            return package.clone();
        }

        let package = PackageSnapshot::from_manifest(
            package_root,
            &package_root.join(package_manager.manifest_name()),
        )
        .ok()
        .flatten()
        .map(Arc::new);
        self.packages
            .insert(package_root.to_path_buf(), package.clone());
        package
    }

    fn read_module_profile(
        &mut self,
        package_root: &Path,
    ) -> Option<DependencyModuleProfileSnapshot> {
        if let Some(profile) = self.module_profiles.get(package_root) {
            return profile.clone();
        }

        #[derive(Deserialize)]
        struct DependencyProfile {
            #[serde(default)]
            app: Option<DependencyProfileApp>,
            module: DependencyProfileModule,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct DependencyProfileModule {
            #[serde(default)]
            name: Option<String>,
            #[serde(rename = "type", default)]
            module_type: Option<String>,
            #[serde(default)]
            distro: Option<DependencyProfileDistro>,
            #[serde(default)]
            abilities: Vec<DependencyProfileAbility>,
            #[serde(default)]
            extension_abilities: Vec<DependencyProfileAbility>,
            #[serde(default, rename = "routerMap")]
            router_map: Option<String>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct DependencyProfileAbility {
            #[serde(default)]
            src_entry: String,
            #[serde(default)]
            skills: Vec<DependencyProfileSkill>,
            #[serde(rename = "type", alias = "extensionType", default)]
            extension_type: Option<String>,
        }

        #[derive(Deserialize)]
        struct DependencyProfileSkill {
            #[serde(default)]
            actions: Vec<String>,
            #[serde(default)]
            entities: Vec<String>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct DependencyProfileDistro {
            #[serde(default)]
            module_type: Option<String>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct DependencyProfileApp {
            #[serde(default)]
            bundle_name: Option<String>,
            #[serde(default)]
            bundle_type: Option<String>,
            #[serde(default)]
            version_code: Option<u64>,
        }

        let profile = ModulePathLayout::new(package_root)
            .dependency_profile_candidates()
            .into_iter()
            .filter_map(|path| std::fs::read_to_string(path).ok())
            .filter_map(|content| json5::from_str::<DependencyProfile>(&content).ok())
            .next()
            .map(|profile| {
                let app = profile.app;
                let ability_src_entries = profile
                    .module
                    .abilities
                    .iter()
                    .chain(profile.module.extension_abilities.iter())
                    .filter(|ability| !ability.src_entry.is_empty())
                    .map(|ability| ability.src_entry.clone())
                    .collect();
                let work_scheduler_src_entries = profile
                    .module
                    .extension_abilities
                    .iter()
                    .filter(|ability| {
                        ability.extension_type.as_deref() == Some("workScheduler")
                            && !ability.src_entry.is_empty()
                    })
                    .map(|ability| ability.src_entry.clone())
                    .collect();
                let home_ability_src_entries = profile
                    .module
                    .abilities
                    .iter()
                    .filter(|ability| {
                        !ability.src_entry.is_empty()
                            && ability.skills.iter().any(|skill| {
                                skill
                                    .entities
                                    .iter()
                                    .any(|entity| entity == "entity.system.home")
                                    && skill.actions.iter().any(|action| {
                                        matches!(
                                            action.as_str(),
                                            "action.system.home" | "ohos.want.action.home"
                                        )
                                    })
                            })
                    })
                    .map(|ability| ability.src_entry.clone())
                    .collect();
                DependencyModuleProfileSnapshot {
                    module_name: profile.module.name,
                    module_type: profile
                        .module
                        .module_type
                        .or_else(|| profile.module.distro.and_then(|distro| distro.module_type))
                        .map(|module_type| OhosModuleType::from_type_str(&module_type)),
                    bundle_name: app.as_ref().and_then(|app| app.bundle_name.clone()),
                    bundle_type: app.as_ref().and_then(|app| app.bundle_type.clone()),
                    version_code: app.and_then(|app| app.version_code),
                    home_ability_src_entries,
                    ability_src_entries,
                    work_scheduler_src_entries,
                    router_map: profile.module.router_map,
                }
            });
        self.module_profiles
            .insert(package_root.to_path_buf(), profile.clone());
        profile
    }

    fn read_compile_inputs(
        &mut self,
        package_root: &Path,
        package: Option<&PackageSnapshot>,
        module_profile: Option<&DependencyModuleProfileSnapshot>,
        project_root: &Path,
        package_manager: DependencyPackageManager,
    ) -> Arc<DependencyCompileInputSnapshot> {
        if let Some(inputs) = self.compile_inputs.get(package_root) {
            return inputs.clone();
        }

        let layout = ModulePathLayout::new(package_root);
        let mut ability_source_files = IndexSet::new();
        let mut worker_source_files = IndexSet::new();
        let mut insight_intent_source_files = IndexSet::new();
        let source_inventory_files = self.read_source_inventory_files(package_root);
        let ets_source_root = layout.ets_source_root();
        let other_compile_source_files = source_inventory_files
            .iter()
            .filter(|path| path.starts_with(&ets_source_root))
            .filter(|path| Self::is_other_compile_code_file(path))
            .cloned()
            .collect::<Vec<_>>();

        if let Some(profile) = module_profile {
            for entry in &profile.ability_src_entries {
                let path = layout.src_main().join(strip_relative_prefix(entry));
                if Self::is_loader_code_file(&path) {
                    ability_source_files.insert(path);
                }
            }
            for entry in &profile.work_scheduler_src_entries {
                let path = package_root.join(strip_relative_prefix(entry));
                if Self::is_loader_code_file(&path) {
                    worker_source_files.insert(path);
                }
            }
        }

        if let Some(package) = package {
            for entry in Self::package_worker_entries(package) {
                let path = package_root.join(strip_relative_prefix(&entry));
                if Self::is_loader_code_file(&path) {
                    worker_source_files.insert(path);
                }
            }
        }
        for entry in Self::build_profile_worker_entries(package_root) {
            let path = package_root.join(strip_relative_prefix(&entry));
            if Self::is_loader_code_file(&path) {
                worker_source_files.insert(path);
            }
        }

        for resource_dir in Self::resource_directories(package_root, package) {
            let profile_path = resource_dir
                .join("base")
                .join("profile")
                .join("insight_intent.json");
            for entry in Self::insight_intent_entries(&profile_path) {
                let path = layout.src_main().join(strip_relative_prefix(&entry));
                if Self::is_loader_code_file(&path) {
                    insight_intent_source_files.insert(path);
                }
            }
        }

        let inputs = Arc::new(DependencyCompileInputSnapshot {
            ability_source_files: ability_source_files.into_iter().collect(),
            worker_source_files: worker_source_files.into_iter().collect(),
            insight_intent_source_files: insight_intent_source_files.into_iter().collect(),
            dynamic_import_entry_file: Self::dynamic_import_entry_file(package_root, package),
            other_compile_entry_file: Self::other_compile_entry_file(package_root, package),
            other_compile_source_files: Arc::from(other_compile_source_files),
            source_inventory_files,
            extra_importee_source_files: self.read_extra_importee_source_files(
                package,
                package_root,
                project_root,
                package_manager,
            ),
            router_map_profile: module_profile
                .and_then(|profile| profile.router_map.as_deref())
                .and_then(|reference| reference.strip_prefix("$profile:"))
                .and_then(|profile_name| {
                    std::fs::read_to_string(
                        layout.profile_root().join(format!("{profile_name}.json")),
                    )
                    .ok()
                })
                .and_then(|content| serde_json::from_str(&content).ok())
                .map(Arc::new),
        });
        self.compile_inputs
            .insert(package_root.to_path_buf(), inputs.clone());
        inputs
    }

    fn dynamic_import_entry_file(
        package_root: &Path,
        package: Option<&PackageSnapshot>,
    ) -> Option<PathBuf> {
        if let Some(package) = package {
            for entry in [
                package.package_json.main.as_deref(),
                package.package_json.types.as_deref(),
            ]
            .into_iter()
            .flatten()
            .filter(|entry| !entry.is_empty())
            {
                let candidate = package_root.join(entry);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
        let ets_root = ModulePathLayout::new(package_root).ets_source_root();
        for candidate_name in ["index.ets", "index.ts", "index.js"] {
            for candidate in [
                ets_root.join(candidate_name),
                package_root.join(candidate_name),
            ] {
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
        None
    }

    fn other_compile_entry_file(
        package_root: &Path,
        package: Option<&PackageSnapshot>,
    ) -> Option<PathBuf> {
        if let Some(package) = package {
            if let Some(main) = package
                .package_json
                .main
                .as_deref()
                .filter(|main| !main.is_empty())
            {
                let candidate = package_root.join(main);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
            let types = package.package_json.types.as_deref().or_else(|| {
                package
                    .package_json
                    .extra
                    .get("typings")
                    .and_then(serde_json::Value::as_str)
            });
            if let Some(types) = types.filter(|types| !types.is_empty()) {
                let candidate = package_root.join(types);
                if Self::is_other_compile_code_file(&candidate) {
                    return Some(candidate);
                }
            }
        }
        for relative in ["src/main/ets/index", "src/main/ets/Index", "index", "Index"] {
            let base = package_root.join(relative);
            if Self::is_other_compile_code_file(&base) {
                return Some(base);
            }
            for extension in ["ets", "ts", "js"] {
                let candidate = base.with_extension(extension);
                if Self::is_other_compile_code_file(&candidate) {
                    return Some(candidate);
                }
            }
        }
        None
    }

    fn read_source_inventory_files(&mut self, package_root: &Path) -> Arc<[PathBuf]> {
        if let Some(files) = self.source_files.get(package_root) {
            return Arc::clone(files);
        }
        let mut files = Vec::new();
        if !package_root.is_dir() {
            let files = Arc::from(files);
            self.source_files
                .insert(package_root.to_path_buf(), Arc::clone(&files));
            return files;
        }
        for entry in walkdir::WalkDir::new(package_root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|entry| {
                entry.path() == package_root
                    || !matches!(
                        entry.path().file_name().and_then(|value| value.to_str()),
                        Some(".git" | "build" | "out" | "node_modules" | "oh_modules")
                    )
            })
        {
            let Ok(entry) = entry else {
                continue;
            };
            if !entry.file_type().is_file() {
                continue;
            }
            let path = canonicalize_or_owned(entry.path());
            if HvigorSourceFileFilter::new(&path).is_ignored_macos_metadata() {
                continue;
            }
            let extension = path.extension().and_then(|extension| extension.to_str());
            if extension.is_some_and(crate::source_store::is_source_inventory_ext) {
                files.push(path);
            }
        }
        files.sort_unstable();
        let files = Arc::from(files);
        self.source_files
            .insert(package_root.to_path_buf(), Arc::clone(&files));
        files
    }

    fn read_extra_importee_source_files(
        &mut self,
        package: Option<&PackageSnapshot>,
        resolution_root: &Path,
        project_root: &Path,
        package_manager: DependencyPackageManager,
    ) -> IndexMap<String, PathBuf> {
        let importees = package
            .and_then(|package| package.package_json.extra.get("metadata"))
            .and_then(|metadata| metadata.get("extraImportees"))
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str);
        let mut resolved = IndexMap::new();
        for importee in importees {
            let candidates = ModuleSpecifierText::new(importee).package_candidates();
            let Some(candidate) = candidates.last() else {
                continue;
            };
            let Some(dependency_root) = self.resolve_package_root(
                resolution_root,
                project_root,
                candidate.package_name(),
                package_manager,
            ) else {
                continue;
            };
            let dependency_package = self.read_package(&dependency_root, package_manager);
            let subpath = Path::new(candidate.subpath());
            let path = if subpath.as_os_str().is_empty() {
                Self::other_compile_entry_file(&dependency_root, dependency_package.as_deref())
            } else {
                let base = canonicalize_or_owned(&dependency_root.join(subpath));
                let inventory = self.read_source_inventory_files(&dependency_root);
                std::iter::once(base.clone())
                    .chain(
                        ["ets", "ts", "js"]
                            .into_iter()
                            .map(|extension| base.with_extension(extension)),
                    )
                    .find(|candidate| inventory.iter().any(|path| path == candidate))
            };
            if let Some(path) = path {
                resolved.insert(importee.to_string(), path);
            }
        }
        resolved
    }

    fn package_worker_entries(package: &PackageSnapshot) -> Vec<String> {
        package
            .package_json
            .extra
            .get("metadata")
            .and_then(|metadata| metadata.get("workers"))
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .map(str::to_string)
            .collect()
    }

    fn build_profile_worker_entries(package_root: &Path) -> Vec<String> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct BuildProfileWorkers {
            #[serde(default)]
            build_option: BuildProfileOption,
        }

        #[derive(Default, Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct BuildProfileOption {
            #[serde(default)]
            source_option: BuildProfileSourceOption,
        }

        #[derive(Default, Deserialize)]
        struct BuildProfileSourceOption {
            #[serde(default)]
            workers: Vec<String>,
        }

        std::fs::read_to_string(package_root.join("build-profile.json5"))
            .ok()
            .and_then(|content| json5::from_str::<BuildProfileWorkers>(&content).ok())
            .map(|profile| profile.build_option.source_option.workers)
            .unwrap_or_default()
    }

    fn resource_directories(
        package_root: &Path,
        package: Option<&PackageSnapshot>,
    ) -> Vec<PathBuf> {
        let directories = package
            .and_then(|package| package.package_json.extra.get("metadata"))
            .and_then(|metadata| metadata.get("resource"))
            .and_then(|resource| resource.get("directories"))
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .map(|directory| package_root.join(strip_relative_prefix(directory)))
            .collect::<Vec<_>>();
        if directories.is_empty() {
            vec![ModulePathLayout::new(package_root).resources_root()]
        } else {
            directories
        }
    }

    fn insight_intent_entries(profile_path: &Path) -> Vec<String> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct InsightIntentProfile {
            #[serde(default)]
            insight_intents_src_entry: Vec<InsightIntentEntry>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct InsightIntentEntry {
            src_entry: Option<String>,
        }

        std::fs::read_to_string(profile_path)
            .ok()
            .and_then(|content| json5::from_str::<InsightIntentProfile>(&content).ok())
            .map(|profile| {
                profile
                    .insight_intents_src_entry
                    .into_iter()
                    .filter_map(|entry| entry.src_entry)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn is_loader_code_file(path: &Path) -> bool {
        path.is_file()
            && path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    matches!(extension.to_ascii_lowercase().as_str(), "ets" | "ts" | "js")
                })
    }

    fn is_other_compile_code_file(path: &Path) -> bool {
        path.is_file()
            && !SourceFileName::new(path).is_declaration()
            && path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    matches!(extension.to_ascii_lowercase().as_str(), "ets" | "ts" | "js")
                })
    }
}

/// Builds a dependency graph from package snapshot data.
pub struct DependencyGraphBuilder {
    host_module: String,
    project_root: PathBuf,
    host_root: PathBuf,
    parameter_file_override: Option<PathBuf>,
    dependency_parameters: Option<DependencyParameterSnapshot>,
    package_manager: DependencyPackageManager,
    configured_runtime_only_aliases: Vec<String>,
}

impl DependencyGraphBuilder {
    pub fn new(
        host_module: impl Into<String>,
        project_root: impl Into<PathBuf>,
        host_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            host_module: host_module.into(),
            project_root: project_root.into(),
            host_root: host_root.into(),
            parameter_file_override: None,
            dependency_parameters: None,
            package_manager: DependencyPackageManager::Ohpm,
            configured_runtime_only_aliases: Vec::new(),
        }
    }

    pub(crate) fn with_package_manager(mut self, use_ohpm: bool) -> Self {
        self.package_manager = DependencyPackageManager::from_ohpm_project(use_ohpm);
        self
    }

    /// Override the project `parameterFile` with hvigor's `-p parameterFile=`
    /// extra parameter.
    pub fn with_parameter_file_override(mut self, path: PathBuf) -> Self {
        self.parameter_file_override = Some(path);
        self
    }

    /// Reuse the project-level parameter snapshot loaded by
    /// [`crate::ProjectSnapshot`]. This prevents one parameter-file read per
    /// module while retaining the standalone graph-builder API.
    pub fn with_dependency_parameters(
        mut self,
        dependency_parameters: DependencyParameterSnapshot,
    ) -> Self {
        self.dependency_parameters = Some(dependency_parameters);
        self
    }

    /// Add runtime-only package aliases selected by the normalized build
    /// option. They participate in the same immutable dependency graph as
    /// manifest-declared runtime-only packages.
    pub fn with_runtime_only_aliases(mut self, aliases: impl IntoIterator<Item = String>) -> Self {
        self.configured_runtime_only_aliases.extend(aliases);
        self
    }

    /// Build the dependency graph snapshot.
    pub fn build(self) -> DependencyGraphSnapshot {
        let mut inventory = DependencySnapshotInventory::default();
        self.build_with_inventory(&mut inventory)
    }

    /// Build while reusing package/profile snapshots from the current session.
    pub(crate) fn build_with_inventory(
        self,
        inventory: &mut DependencySnapshotInventory,
    ) -> DependencyGraphSnapshot {
        let dependency_parameters = self.dependency_parameters.clone().unwrap_or_else(|| {
            let project_package = inventory.read_package(&self.project_root, self.package_manager);
            match self.package_manager {
                DependencyPackageManager::Ohpm => {
                    DependencyParameterSnapshot::from_project_deferred(
                        &self.project_root,
                        project_package.as_deref(),
                        self.parameter_file_override.as_deref(),
                    )
                }
                DependencyPackageManager::Npm => DependencyParameterSnapshot::default(),
            }
        });
        let mut graph = DependencyGraphSnapshot {
            host_module: self.host_module.clone(),
            host_root: inventory.canonical_root(&self.host_root),
            dependency_parameters,
            ..Default::default()
        };
        self.collect_dependencies(&mut graph, inventory);
        graph
    }

    fn collect_dependencies(
        &self,
        graph: &mut DependencyGraphSnapshot,
        inventory: &mut DependencySnapshotInventory,
    ) {
        let mut queue = VecDeque::new();
        let mut seen_roots = std::collections::BTreeSet::new();
        let mut root_runtime_only_aliases = std::collections::BTreeSet::new();
        root_runtime_only_aliases.extend(self.configured_runtime_only_aliases.iter().cloned());
        for root in [&self.host_root, &self.project_root] {
            if let Some(package) = inventory.read_package(root, self.package_manager) {
                root_runtime_only_aliases.extend(Self::runtime_only_aliases(&package));
            }
        }
        queue.push_back((self.host_root.clone(), 0usize));
        if self.project_root != self.host_root {
            queue.push_back((self.project_root.clone(), 0usize));
        }

        while let Some((package_root, depth)) = queue.pop_front() {
            let package = inventory.read_package(&package_root, self.package_manager);
            let mut runtime_only_aliases = package
                .as_deref()
                .map(Self::runtime_only_aliases)
                .unwrap_or_default();
            if depth == 0 {
                runtime_only_aliases.extend(root_runtime_only_aliases.iter().cloned());
                runtime_only_aliases.sort();
                runtime_only_aliases.dedup();
            }
            let Some(package) = package else {
                self.collect_runtime_only_dependencies(
                    &package_root,
                    depth,
                    runtime_only_aliases,
                    &std::collections::BTreeSet::new(),
                    &mut DependencyCollectionState {
                        queue: &mut queue,
                        seen_roots: &mut seen_roots,
                        graph,
                        inventory,
                    },
                );
                continue;
            };
            for (dependencies, edge_kind, is_dynamic) in [
                (
                    &package.package_json.dependencies,
                    DependencyEdgeKind::Dependency,
                    false,
                ),
                (
                    &package.package_json.dynamic_dependencies,
                    DependencyEdgeKind::DynamicDependency,
                    true,
                ),
                (
                    &package.package_json.dev_dependencies,
                    DependencyEdgeKind::DevDependency,
                    false,
                ),
            ] {
                for (alias, spec) in dependencies {
                    let Some(dep_root) = inventory.resolve_package_root(
                        &package_root,
                        &self.project_root,
                        alias,
                        self.package_manager,
                    ) else {
                        continue;
                    };
                    let dep_package = inventory.read_package(&dep_root, self.package_manager);
                    let traversal = DependencyTraversalState {
                        kind: edge_kind,
                        is_transitive: depth > 0,
                        is_dynamic,
                        is_runtime_only: runtime_only_aliases.contains(alias),
                    };
                    let canonical_issuer_root = inventory.canonical_root(&package_root);
                    let edge = self.edge_snapshot(
                        &canonical_issuer_root,
                        alias,
                        Some(spec),
                        &dep_root,
                        dep_package.as_deref(),
                        traversal,
                    );
                    if !seen_roots.insert(dep_root.clone()) {
                        if traversal.is_runtime_only {
                            if let Some(node) = graph.nodes.get_mut(&edge.package_name) {
                                node.is_runtime_only = true;
                            }
                        }
                        graph.edges.push(edge);
                        continue;
                    }
                    let node = self.node_snapshot(&edge, dep_package.clone(), traversal, inventory);
                    self.record_widget_extra_importees(graph, &node, inventory);
                    graph.roots_by_alias.insert(alias.clone(), dep_root.clone());
                    graph.nodes.insert(node.package_name.clone(), node);
                    queue.push_back((dep_root, depth + 1));
                    graph.edges.push(edge);
                }
            }

            let declared_aliases = package
                .package_json
                .dependencies
                .keys()
                .chain(package.package_json.dynamic_dependencies.keys())
                .chain(package.package_json.dev_dependencies.keys())
                .cloned()
                .collect::<std::collections::BTreeSet<_>>();
            self.collect_runtime_only_dependencies(
                &package_root,
                depth,
                runtime_only_aliases,
                &declared_aliases,
                &mut DependencyCollectionState {
                    queue: &mut queue,
                    seen_roots: &mut seen_roots,
                    graph,
                    inventory,
                },
            );
        }
    }

    fn collect_runtime_only_dependencies(
        &self,
        package_root: &Path,
        depth: usize,
        aliases: Vec<String>,
        declared_aliases: &std::collections::BTreeSet<String>,
        state: &mut DependencyCollectionState<'_>,
    ) {
        for alias in aliases {
            if declared_aliases.contains(&alias) {
                continue;
            }
            let Some(dep_root) = state.inventory.resolve_package_root(
                package_root,
                &self.project_root,
                &alias,
                self.package_manager,
            ) else {
                continue;
            };
            let dep_package = state
                .inventory
                .read_package(&dep_root, self.package_manager);
            let traversal = DependencyTraversalState {
                kind: DependencyEdgeKind::Dependency,
                is_transitive: depth > 0,
                is_dynamic: false,
                is_runtime_only: true,
            };
            let canonical_issuer_root = state.inventory.canonical_root(package_root);
            let edge = self.edge_snapshot(
                &canonical_issuer_root,
                &alias,
                None,
                &dep_root,
                dep_package.as_deref(),
                traversal,
            );
            if !state.seen_roots.insert(dep_root.clone()) {
                if let Some(node) = state.graph.nodes.get_mut(&edge.package_name) {
                    node.is_runtime_only = true;
                }
                continue;
            }
            let node = self.node_snapshot(&edge, dep_package, traversal, state.inventory);
            self.record_widget_extra_importees(state.graph, &node, state.inventory);
            state.graph.roots_by_alias.insert(alias, dep_root.clone());
            state.graph.nodes.insert(node.package_name.clone(), node);
            state.queue.push_back((dep_root, depth + 1));
            state.graph.edges.push(edge);
        }
    }

    fn runtime_only_aliases(package: &PackageSnapshot) -> Vec<String> {
        package
            .package_json
            .extra
            .get("metadata")
            .and_then(|metadata| metadata.get("runtimeOnly"))
            .and_then(|runtime_only| runtime_only.get("packages"))
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .map(str::to_string)
            .collect()
    }

    fn edge_snapshot(
        &self,
        issuer_root: &Path,
        alias: &str,
        spec: Option<&str>,
        root: &Path,
        package: Option<&PackageSnapshot>,
        traversal: DependencyTraversalState,
    ) -> DependencyEdgeSnapshot {
        DependencyEdgeSnapshot {
            // Keep issuer identity in the same canonical domain as host_root
            // and resolved package roots. This is observable on macOS where
            // `/tmp` aliases `/private/tmp`, and avoids equivalent Windows
            // drive/case spellings being misclassified as project globals.
            issuer_root: issuer_root.to_path_buf(),
            alias: alias.to_string(),
            version_spec: spec.map(str::to_string),
            package_name: self.package_name(alias, package),
            package_root: root.to_path_buf(),
            installed_alias_path: self.installed_alias_path(issuer_root, alias, root),
            kind: traversal.kind,
            is_transitive: traversal.is_transitive,
        }
    }

    fn installed_alias_path(
        &self,
        issuer_root: &Path,
        alias: &str,
        root: &Path,
    ) -> Option<PathBuf> {
        let modules_directory = self.package_manager.modules_directory();
        let mut current = Some(issuer_root);
        while let Some(issuer) = current {
            let search_root = issuer
                .file_name()
                .filter(|name| {
                    name.to_string_lossy()
                        .eq_ignore_ascii_case(modules_directory)
                })
                .and_then(|_| issuer.parent())
                .unwrap_or(issuer);
            let candidate = search_root.join(modules_directory).join(alias);
            if std::fs::symlink_metadata(&candidate)
                .is_ok_and(|metadata| metadata.file_type().is_symlink())
                && canonicalize_or_owned(&candidate) == canonicalize_or_owned(root)
            {
                return Some(candidate);
            }
            current = search_root.parent();
        }
        None
    }

    fn node_snapshot(
        &self,
        edge: &DependencyEdgeSnapshot,
        package: Option<Arc<PackageSnapshot>>,
        traversal: DependencyTraversalState,
        inventory: &mut DependencySnapshotInventory,
    ) -> DependencyNodeSnapshot {
        let alias = edge.alias.as_str();
        let root = &edge.package_root;
        let package_name = self.package_name(alias, package.as_deref());
        let module_profile = inventory.read_module_profile(root);
        let compile_inputs = inventory.read_compile_inputs(
            root,
            package.as_deref(),
            module_profile.as_ref(),
            &self.project_root,
            self.package_manager,
        );
        let direct_deps = package
            .as_ref()
            .map(|package| self.direct_dependency_aliases(package))
            .unwrap_or_default();
        DependencyNodeSnapshot {
            alias: alias.to_string(),
            package_name,
            version_spec: edge.version_spec.clone(),
            package_root: Some(root.to_path_buf()),
            node_type: self.classify_package(
                package.as_deref(),
                module_profile.as_ref(),
                self.package_manager,
            ),
            package,
            module_profile,
            compile_inputs,
            is_runtime_only: traversal.is_runtime_only,
            is_dynamic: traversal.is_dynamic,
            is_transitive: traversal.is_transitive,
            direct_deps,
        }
    }

    fn record_widget_extra_importees(
        &self,
        graph: &mut DependencyGraphSnapshot,
        node: &DependencyNodeSnapshot,
        inventory: &mut DependencySnapshotInventory,
    ) {
        let Some(package) = node.package.as_deref() else {
            return;
        };
        let files = inventory.read_extra_importee_source_files(
            Some(package),
            &self.host_root,
            &self.project_root,
            self.package_manager,
        );
        if !files.is_empty() {
            graph
                .widget_extra_importee_source_files
                .insert(node.package_name.clone(), files);
        }
    }

    fn package_name(&self, alias: &str, package: Option<&PackageSnapshot>) -> String {
        package
            .map(|package| package.name.clone())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| alias.to_string())
    }

    fn classify_package(
        &self,
        package: Option<&PackageSnapshot>,
        module_profile: Option<&DependencyModuleProfileSnapshot>,
        package_manager: DependencyPackageManager,
    ) -> DependencyNodeType {
        if package.is_some_and(|package| {
            Path::new(&package.name)
                .extension()
                .is_some_and(|ext| ext == "so")
        }) {
            return DependencyNodeType::NativeSo;
        }
        if package
            .and_then(|package| package.package_json.extra.get("metadata"))
            .and_then(|metadata| metadata.get("integratedHsp"))
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
        {
            return DependencyNodeType::IntegratedHsp;
        }
        if package
            .and_then(|package| package.package_json.extra.get("metadata"))
            .and_then(|metadata| metadata.get("byteCodeHar"))
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
        {
            return DependencyNodeType::BytecodeHar;
        }
        // npm packages are Harmony dependencies only when their package.json
        // carries the `ohos` marker. Without it Hvigor keeps the dependency as
        // an ordinary npm node even if a coincidental module profile exists.
        if matches!(package_manager, DependencyPackageManager::Npm)
            && !package.is_some_and(|package| package.package_json.extra.contains_key("ohos"))
        {
            return DependencyNodeType::OhpmSource;
        }
        // Mirrors hvigor-ohos-plugin's
        // `DependencyManager.getDependencyTypeByProfile`: packaged source HARs
        // normally declare their kind in src/main/module.json(.5), not in
        // oh-package.json5. Legacy profiles carry the same value under
        // module.distro.moduleType.
        if let Some(module_type) = module_profile.and_then(|profile| profile.module_type.as_ref()) {
            return match module_type {
                OhosModuleType::Har => DependencyNodeType::SourceHar,
                OhosModuleType::Shared => DependencyNodeType::SharedHsp,
                OhosModuleType::Entry | OhosModuleType::Feature => DependencyNodeType::OhpmSource,
            };
        }
        match package.and_then(|package| package.package_json.package_type.as_deref()) {
            Some("shared") | Some("hsp") => DependencyNodeType::SharedHsp,
            Some("har") => DependencyNodeType::SourceHar,
            _ => DependencyNodeType::OhpmSource,
        }
    }

    fn direct_dependency_aliases(&self, package: &PackageSnapshot) -> Vec<String> {
        let mut aliases = package
            .package_json
            .dependencies
            .keys()
            .chain(package.package_json.dynamic_dependencies.keys())
            .chain(package.package_json.dev_dependencies.keys())
            .cloned()
            .collect::<Vec<_>>();
        aliases.sort();
        aliases.dedup();
        aliases
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::*;

    struct DependencyGraphFixture {
        temp: tempfile::TempDir,
    }

    impl DependencyGraphFixture {
        fn new() -> Self {
            Self {
                temp: tempfile::tempdir().expect("tempdir"),
            }
        }

        fn root(&self) -> &Path {
            self.temp.path()
        }

        fn write_package(&self, relative: &str, manifest: &str) {
            let root = self.root().join(relative);
            fs::create_dir_all(&root).expect("package root");
            fs::write(root.join("oh-package.json5"), manifest).expect("oh-package");
        }

        fn write_dependency_profile(&self, relative: &str, file_name: &str, profile: &str) {
            let src_main = self.root().join(relative).join("src/main");
            fs::create_dir_all(&src_main).expect("dependency src/main");
            fs::write(src_main.join(file_name), profile).expect("dependency profile");
        }

        fn build_for(&self, host_module: &str) -> DependencyGraphSnapshot {
            DependencyGraphBuilder::new(host_module, self.root(), self.root().join(host_module))
                .build()
        }
    }

    #[test]
    fn collects_project_root_dev_dependencies() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package(
            "",
            r#"{
              "name": "app",
              "version": "1.0.0",
              "devDependencies": { "@ohos/hypium": "1.0.11" }
            }"#,
        );
        fixture.write_package("entry", r#"{ "name": "entry", "version": "1.0.0" }"#);
        fixture.write_package(
            "oh_modules/@ohos/hypium",
            r#"{ "name": "@ohos/hypium", "version": "1.0.11", "main": "index.js" }"#,
        );

        let graph = fixture.build_for("entry");

        let node = graph
            .nodes
            .get("@ohos/hypium")
            .expect("root dev dependency node");
        assert_eq!(node.alias, "@ohos/hypium");
        assert_eq!(node.version_spec.as_deref(), Some("1.0.11"));
        assert!(graph.roots_by_alias.contains_key("@ohos/hypium"));
    }

    #[test]
    fn freezes_root_runtime_only_packages_into_the_session_graph() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package(
            "",
            r#"{
              "name": "app",
              "version": "1.0.0",
              "metadata": { "runtimeOnly": { "packages": ["runtime-har"] } }
            }"#,
        );
        fixture.write_package(
            "entry",
            r#"{
              "name": "entry",
              "version": "1.0.0",
              "dependencies": { "runtime-har": "1.0.0" }
            }"#,
        );
        fixture.write_package(
            "entry/oh_modules/runtime-har",
            r#"{ "name": "runtime-har", "version": "1.0.0", "metadata": { "byteCodeHar": true } }"#,
        );

        let graph = fixture.build_for("entry");
        let node = graph.nodes.get("runtime-har").expect("runtime-only node");

        assert!(node.is_runtime_only);
        assert_eq!(node.node_type, DependencyNodeType::BytecodeHar);
    }

    #[test]
    fn configured_runtime_only_package_is_frozen_without_host_manifest() {
        let fixture = DependencyGraphFixture::new();
        fs::create_dir_all(fixture.root().join("entry")).expect("host module");
        fixture.write_package(
            "oh_modules/runtime-package",
            r#"{ "name": "runtime-package", "version": "1.0.0" }"#,
        );

        let graph =
            DependencyGraphBuilder::new("entry", fixture.root(), fixture.root().join("entry"))
                .with_runtime_only_aliases(["runtime-package".to_string()])
                .build();

        assert!(graph.nodes["runtime-package"].is_runtime_only);
        assert_eq!(graph.edges[0].alias, "runtime-package");
        assert_eq!(graph.edges[0].version_spec, None);
    }

    #[test]
    fn freezes_dependency_loader_compile_inputs_at_session_startup() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package("", r#"{ "name": "app", "version": "1.0.0" }"#);
        fixture.write_package(
            "entry",
            r#"{
              "name": "entry",
              "version": "1.0.0",
              "dependencies": { "source-har": "1.0.0" }
            }"#,
        );
        fixture.write_package(
            "entry/oh_modules/source-har",
            r#"{
              "name": "source-har",
              "version": "1.0.0",
              "main": "src/main/ets/index.ets",
              "metadata": {
                "workers": ["./src/main/ets/workers/PackageWorker.ts"],
                "extraImportees": ["extra-pkg/src/feature"]
              }
            }"#,
        );
        fixture.write_package(
            "entry/oh_modules/extra-pkg",
            r#"{ "name": "extra-pkg", "version": "1.0.0" }"#,
        );
        fixture.write_dependency_profile(
            "entry/oh_modules/source-har",
            "module.json5",
            r#"{
              module: {
                name: "source-har",
                type: "har",
                routerMap: "$profile:router_map",
                abilities: [{ srcEntry: "ets/abilities/MainAbility.ets" }],
                extensionAbilities: [
                  { type: "workScheduler", srcEntry: "./src/main/ets/workers/SchedulerWorker.ets" },
                  { type: "service", srcEntry: "ets/extensions/ServiceExtension.ets" }
                ]
              }
            }"#,
        );
        let dep_root = fixture.root().join("entry/oh_modules/source-har");
        fs::write(
            dep_root.join("build-profile.json5"),
            r#"{
              buildOption: {
                sourceOption: { workers: ["./src/main/ets/workers/ProfileWorker.js"] }
              }
            }"#,
        )
        .expect("dependency build profile");
        let insight_profile = dep_root.join("src/main/resources/base/profile");
        fs::create_dir_all(&insight_profile).expect("insight profile directory");
        fs::write(
            insight_profile.join("insight_intent.json"),
            r#"{
              insightIntentsSrcEntry: [{ srcEntry: "ets/insight/IntentEntry.ts" }]
            }"#,
        )
        .expect("insight profile");
        fs::write(
            insight_profile.join("router_map.json"),
            r#"{ "routerMap": [{ "name": "detail", "pageSourceFile": "ets/pages/Detail.ets" }] }"#,
        )
        .expect("router map profile");
        for relative in [
            "src/main/ets/abilities/MainAbility.ets",
            "src/main/ets/extensions/ServiceExtension.ets",
            "src/main/ets/workers/SchedulerWorker.ets",
            "src/main/ets/workers/PackageWorker.ts",
            "src/main/ets/workers/ProfileWorker.js",
            "src/main/ets/insight/IntentEntry.ts",
            "src/main/ets/index.ets",
        ] {
            let path = dep_root.join(relative);
            fs::create_dir_all(path.parent().expect("source parent")).expect("source directory");
            fs::write(path, "").expect("dependency source");
        }
        let extra_importee = fixture
            .root()
            .join("entry/oh_modules/extra-pkg/src/feature.ets");
        fs::create_dir_all(extra_importee.parent().expect("extra importee parent"))
            .expect("extra importee directory");
        fs::write(&extra_importee, "").expect("extra importee source");

        let graph = fixture.build_for("entry");
        let inputs = &graph.nodes["source-har"].compile_inputs;
        let dep_root = canonicalize_or_owned(&dep_root);

        assert_eq!(
            inputs.ability_source_files,
            vec![
                dep_root.join("src/main/ets/abilities/MainAbility.ets"),
                dep_root.join("src/main/ets/extensions/ServiceExtension.ets"),
            ]
        );
        assert_eq!(
            inputs.worker_source_files,
            vec![
                dep_root.join("src/main/ets/workers/SchedulerWorker.ets"),
                dep_root.join("src/main/ets/workers/PackageWorker.ts"),
                dep_root.join("src/main/ets/workers/ProfileWorker.js"),
            ]
        );
        assert_eq!(
            inputs.insight_intent_source_files,
            vec![dep_root.join("src/main/ets/insight/IntentEntry.ts")]
        );
        assert_eq!(
            inputs.dynamic_import_entry_file,
            Some(dep_root.join("src/main/ets/index.ets"))
        );
        assert!(inputs
            .source_inventory_files
            .contains(&dep_root.join("src/main/ets/index.ets")));
        assert_eq!(
            inputs.extra_importee_source_files["extra-pkg/src/feature"],
            canonicalize_or_owned(&extra_importee)
        );
        assert_eq!(
            inputs
                .router_map_profile
                .as_deref()
                .and_then(|profile| profile.pointer("/routerMap/0/name"))
                .and_then(serde_json::Value::as_str),
            Some("detail")
        );
    }

    #[test]
    fn runtime_only_package_does_not_require_a_duplicate_dependency_entry() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package("", r#"{ "name": "app", "version": "1.0.0" }"#);
        fixture.write_package(
            "entry",
            r#"{
              "name": "entry",
              "version": "1.0.0",
              "metadata": { "runtimeOnly": { "packages": ["runtime-har"] } }
            }"#,
        );
        fixture.write_package(
            "entry/oh_modules/runtime-har",
            r#"{
              "name": "runtime-har",
              "version": "1.0.0",
              "metadata": { "byteCodeHar": true },
              "dependencies": { "nested": "1.0.0" }
            }"#,
        );
        fixture.write_package(
            "entry/oh_modules/runtime-har/oh_modules/nested",
            r#"{ "name": "nested", "version": "1.0.0" }"#,
        );

        let graph = fixture.build_for("entry");
        let node = graph.nodes.get("runtime-har").expect("runtime-only node");

        assert!(node.is_runtime_only);
        assert_eq!(node.version_spec, None);
        assert!(graph.nodes["nested"].is_transitive);
        assert_eq!(
            graph
                .edges
                .iter()
                .filter(|edge| edge.alias == "runtime-har")
                .count(),
            1
        );
    }

    #[test]
    fn transitive_package_resolves_without_redundant_project_declaration() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package("", r#"{ "name": "app", "version": "1.0.0" }"#);
        fixture.write_package(
            "meituan",
            r#"{
              "name": "meituan",
              "version": "1.0.0",
              "dependencies": { "@msi/jsi": "1.0.0" }
            }"#,
        );
        fixture.write_package(
            "meituan/oh_modules/@msi/jsi",
            r#"{
              "name": "@msi/jsi",
              "version": "1.0.0",
              "dependencies": { "@msi/jsi_interface": "1.0.0" }
            }"#,
        );
        fixture.write_package(
            "meituan/oh_modules/@msi/jsi/oh_modules/@msi/jsi_interface",
            r#"{
              "name": "@msi/jsi_interface",
              "version": "1.0.0",
              "main": "index.ets"
            }"#,
        );

        let graph = fixture.build_for("meituan");
        let interface = graph
            .nodes
            .get("@msi/jsi_interface")
            .expect("transitive interface package");
        let expected_root = canonicalize_or_owned(
            &fixture
                .root()
                .join("meituan/oh_modules/@msi/jsi/oh_modules/@msi/jsi_interface"),
        );

        assert!(interface.is_transitive);
        assert_eq!(
            interface.package_root.as_deref(),
            Some(expected_root.as_path())
        );
        assert_eq!(
            graph.roots_by_alias.get("@msi/jsi_interface"),
            Some(&expected_root)
        );
        assert!(graph.edges.iter().any(|edge| {
            edge.alias == "@msi/jsi_interface"
                && edge.issuer_root
                    == canonicalize_or_owned(&fixture.root().join("meituan/oh_modules/@msi/jsi"))
        }));
    }

    #[test]
    fn includes_dev_and_dynamic_aliases_in_direct_deps() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package("", r#"{ "name": "app", "version": "1.0.0" }"#);
        fixture.write_package(
            "entry",
            r#"{
              "name": "entry",
              "version": "1.0.0",
              "dependencies": { "runtime-lib": "1.0.0" },
              "devDependencies": { "dev-lib": "1.0.0" },
              "dynamicDependencies": { "dynamic-lib": "1.0.0" }
            }"#,
        );
        for name in ["runtime-lib", "dev-lib", "dynamic-lib"] {
            fixture.write_package(
                &format!("entry/oh_modules/{name}"),
                &format!(r#"{{ "name": "{name}", "version": "1.0.0" }}"#),
            );
        }

        let builder =
            DependencyGraphBuilder::new("entry", fixture.root(), fixture.root().join("entry"));
        let package = PackageSnapshot::from_root(&fixture.root().join("entry"))
            .expect("read entry package")
            .expect("entry package");

        assert_eq!(
            builder.direct_dependency_aliases(&package),
            vec!["dev-lib", "dynamic-lib", "runtime-lib"]
        );

        let graph = fixture.build_for("entry");

        assert!(graph.nodes.contains_key("dev-lib"));
        assert!(graph.nodes.contains_key("dynamic-lib"));
        assert!(graph.nodes["dynamic-lib"].is_dynamic);
        assert_eq!(
            graph
                .edges
                .iter()
                .map(|edge| (edge.alias.as_str(), edge.kind))
                .collect::<Vec<_>>(),
            vec![
                ("runtime-lib", DependencyEdgeKind::Dependency),
                ("dynamic-lib", DependencyEdgeKind::DynamicDependency),
                ("dev-lib", DependencyEdgeKind::DevDependency),
            ]
        );
    }

    #[test]
    fn shared_inventory_reuses_hvigor_dep_node_package_and_profile_snapshots() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package("", r#"{ "name": "app", "version": "1.0.0" }"#);
        for module in ["first", "second"] {
            fixture.write_package(
                module,
                r#"{
                  "name": "host",
                  "version": "1.0.0",
                  "dependencies": { "shared-har": "1.0.0" }
                }"#,
            );
        }
        fixture.write_package(
            "oh_modules/shared-har",
            r#"{ "name": "shared-har", "version": "1.0.0" }"#,
        );
        fixture.write_dependency_profile(
            "oh_modules/shared-har",
            "module.json5",
            r#"{ "module": { "name": "shared", "type": "har" } }"#,
        );

        let mut inventory = DependencySnapshotInventory::default();
        let first =
            DependencyGraphBuilder::new("first", fixture.root(), fixture.root().join("first"))
                .build_with_inventory(&mut inventory);

        fixture.write_package(
            "oh_modules/shared-har",
            r#"{ "name": "changed-after-snapshot", "version": "2.0.0" }"#,
        );
        fixture.write_dependency_profile(
            "oh_modules/shared-har",
            "module.json5",
            r#"{ "module": { "name": "changed", "type": "shared" } }"#,
        );

        let second =
            DependencyGraphBuilder::new("second", fixture.root(), fixture.root().join("second"))
                .build_with_inventory(&mut inventory);

        for graph in [&first, &second] {
            let node = graph.nodes.get("shared-har").expect("cached dependency");
            assert_eq!(node.package.as_ref().unwrap().version, "1.0.0");
            assert_eq!(node.node_type, DependencyNodeType::SourceHar);
            assert_eq!(
                node.module_profile
                    .as_ref()
                    .and_then(|profile| profile.module_name.as_deref()),
                Some("shared")
            );
        }
    }

    #[test]
    fn marks_only_nested_dependencies_as_transitive() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package("", r#"{ "name": "app", "version": "1.0.0" }"#);
        fixture.write_package(
            "entry",
            r#"{
              "name": "entry",
              "version": "1.0.0",
              "dependencies": { "direct-har": "1.0.0" }
            }"#,
        );
        fixture.write_package(
            "entry/oh_modules/direct-har",
            r#"{
              "name": "direct-har",
              "version": "1.0.0",
              "dependencies": { "nested-har": "1.0.0" }
            }"#,
        );
        fixture.write_package(
            "entry/oh_modules/direct-har/oh_modules/nested-har",
            r#"{ "name": "nested-har", "version": "1.0.0" }"#,
        );

        let graph = fixture.build_for("entry");

        assert!(!graph.nodes["direct-har"].is_transitive);
        assert!(graph.nodes["nested-har"].is_transitive);
    }

    #[test]
    fn file_dependency_requires_installed_oh_modules_alias_like_hvigor() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package("", r#"{ "name": "app", "version": "1.0.0" }"#);
        fixture.write_package(
            "entry",
            r#"{
              "name": "entry",
              "version": "1.0.0",
              "dependencies": { "native-types.so": "file:./types/native" }
            }"#,
        );
        fixture.write_package(
            "entry/types/native",
            r#"{ "name": "native-types.so", "version": "1.0.0", "types": "Index.d.ts" }"#,
        );

        let graph_without_install = fixture.build_for("entry");
        assert!(
            !graph_without_install.nodes.contains_key("native-types.so"),
            "a declared file path is not an installed OHPM package alias"
        );

        fixture.write_package(
            "entry/oh_modules/native-types.so",
            r#"{ "name": "native-types.so", "version": "1.0.0", "types": "Index.d.ts" }"#,
        );
        let graph_with_install = fixture.build_for("entry");
        assert!(graph_with_install.nodes.contains_key("native-types.so"));
    }

    #[cfg(unix)]
    #[test]
    fn captures_installed_symlink_alias_once_in_the_session_graph() {
        use std::os::unix::fs::symlink;

        let fixture = DependencyGraphFixture::new();
        fixture.write_package("", r#"{ "name": "app", "version": "1.0.0" }"#);
        fixture.write_package(
            "entry",
            r#"{
              "name": "entry",
              "version": "1.0.0",
              "dependencies": { "linked": "file:../packages/linked" }
            }"#,
        );
        fixture.write_package(
            "packages/linked",
            r#"{ "name": "linked", "version": "1.0.0" }"#,
        );
        let modules = fixture.root().join("entry/oh_modules");
        fs::create_dir_all(&modules).expect("oh_modules");
        symlink("../../packages/linked", modules.join("linked")).expect("package symlink");

        let graph = fixture.build_for("entry");
        let edge = graph
            .edges
            .iter()
            .find(|edge| edge.alias == "linked")
            .expect("linked dependency edge");

        assert_eq!(
            edge.installed_alias_path.as_deref(),
            Some(graph.host_root.join("oh_modules/linked").as_path())
        );
        assert_eq!(
            edge.package_root,
            canonicalize_or_owned(&fixture.root().join("packages/linked"))
        );
        let linked_change = graph.host_root.join("oh_modules/linked/src/Index.ets");
        assert_eq!(
            graph.changed_package_names_for_paths([linked_change.as_path()]),
            Some(vec!["linked".to_string()])
        );
        let project_change = fixture.root().join("entry/src/main/ets/Index.ets");
        assert_eq!(
            graph.changed_package_names_for_paths([project_change.as_path()]),
            None
        );
    }

    #[cfg(unix)]
    #[test]
    fn materialized_local_path_roots_enter_the_frozen_graph() {
        use std::os::unix::fs::symlink;

        let fixture = DependencyGraphFixture::new();
        fixture.write_package("", r#"{ "name": "app", "version": "1.0.0" }"#);
        fixture.write_package(
            "entry",
            r#"{
              "name": "entry",
              "version": "1.0.0",
              "dependencies": { "native-types.so": "file:./types/native" }
            }"#,
        );
        fixture.write_package(
            "entry/types/native",
            r#"{ "name": "native-types.so", "version": "1.0.0", "types": "Index.d.ts" }"#,
        );

        let modules = fixture.root().join("entry/oh_modules");
        fs::create_dir_all(&modules).expect("oh_modules");
        symlink("../types/native", modules.join("native-types.so"))
            .expect("materialized local dependency symlink");
        let mut inventory = DependencySnapshotInventory::default();
        let graph =
            DependencyGraphBuilder::new("entry", fixture.root(), fixture.root().join("entry"))
                .build_with_inventory(&mut inventory);

        let dependency = graph
            .nodes
            .get("native-types.so")
            .expect("materialized package must enter dependency graph");
        assert_eq!(dependency.node_type, DependencyNodeType::NativeSo);
        assert_eq!(
            dependency.package_root,
            Some(canonicalize_or_owned(
                &fixture.root().join("entry/types/native")
            ))
        );
    }

    #[test]
    fn preserves_dependency_edge_declaration_order() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package("", r#"{ "name": "app", "version": "1.0.0" }"#);
        fixture.write_package(
            "entry",
            r#"{
              "name": "entry",
              "version": "1.0.0",
              "dependencies": {
                "dep-b": "1.0.0",
                "dep-a": "1.0.0"
              }
            }"#,
        );
        for name in ["dep-a", "dep-b"] {
            fixture.write_package(
                &format!("entry/oh_modules/{name}"),
                &format!(r#"{{ "name": "{name}", "version": "1.0.0" }}"#),
            );
        }

        let graph = fixture.build_for("entry");

        assert_eq!(
            graph
                .edges
                .iter()
                .map(|edge| edge.alias.as_str())
                .collect::<Vec<_>>(),
            vec!["dep-b", "dep-a"]
        );
    }

    #[test]
    fn bytecode_har_artifact_view_preserves_manifest_traversal_order() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package("", r#"{ "name": "app", "version": "1.0.0" }"#);
        fixture.write_package(
            "entry",
            r#"{
              "name": "entry",
              "version": "1.0.0",
              "dependencies": {
                "z-bytecode": "1.0.0",
                "a-bytecode": "1.0.0"
              }
            }"#,
        );
        for name in ["a-bytecode", "z-bytecode"] {
            fixture.write_package(
                &format!("entry/oh_modules/{name}"),
                &format!(
                    r#"{{ "name": "{name}", "version": "1.0.0", "metadata": {{ "byteCodeHar": true }} }}"#
                ),
            );
        }

        let graph = fixture.build_for("entry");
        assert_eq!(
            graph
                .bytecode_har_nodes_in_traversal_order()
                .into_iter()
                .map(|node| node.package_name.as_str())
                .collect::<Vec<_>>(),
            ["z-bytecode", "a-bytecode"]
        );
    }

    #[test]
    fn bytecode_har_ohm_view_matches_latest_hvigor_tree_barriers_and_section_order() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package("", r#"{ "name": "app", "version": "1.0.0" }"#);
        fixture.write_package(
            "entry",
            r#"{
              "name": "entry",
              "version": "1.0.0",
              "dependencies": {
                "source-har": "1.0.0",
                "direct-byte": "1.0.0",
                "direct-other": "1.0.0",
                "direct-hsp": "1.0.0"
              },
              "dynamicDependencies": { "dynamic-other": "1.0.0" },
              "devDependencies": { "dev-other": "1.0.0" }
            }"#,
        );
        fixture.write_package(
            "entry/oh_modules/source-har",
            r#"{
              "name": "source-har",
              "version": "1.0.0",
              "dependencies": { "nested-byte": "1.0.0" }
            }"#,
        );
        fixture.write_dependency_profile(
            "entry/oh_modules/source-har",
            "module.json",
            r#"{ "module": { "name": "source", "type": "har" } }"#,
        );
        fixture.write_package(
            "entry/oh_modules/direct-byte",
            r#"{ "name": "direct-byte", "version": "1.0.0", "metadata": { "byteCodeHar": true } }"#,
        );
        fixture.write_package(
            "entry/oh_modules/direct-other",
            r#"{
              "name": "direct-other",
              "version": "1.0.0",
              "dependencies": { "hidden-byte": "1.0.0" }
            }"#,
        );
        fixture.write_package(
            "entry/oh_modules/direct-hsp",
            r#"{ "name": "direct-hsp", "version": "1.0.0" }"#,
        );
        fixture.write_dependency_profile(
            "entry/oh_modules/direct-hsp",
            "module.json",
            r#"{ "module": { "name": "shared", "type": "shared" } }"#,
        );
        for name in ["dynamic-other", "dev-other"] {
            fixture.write_package(
                &format!("entry/oh_modules/{name}"),
                &format!(r#"{{ "name": "{name}", "version": "1.0.0" }}"#),
            );
        }
        fixture.write_package(
            "entry/oh_modules/source-har/oh_modules/nested-byte",
            r#"{ "name": "nested-byte", "version": "1.0.0", "metadata": { "byteCodeHar": true } }"#,
        );
        fixture.write_package(
            "entry/oh_modules/direct-other/oh_modules/hidden-byte",
            r#"{ "name": "hidden-byte", "version": "1.0.0", "metadata": { "byteCodeHar": true } }"#,
        );

        let graph = fixture.build_for("entry");
        let names = |bundled, optimized| {
            graph
                .bytecode_har_ohm_nodes_in_traversal_order(bundled, optimized)
                .into_iter()
                .map(|node| node.package_name.as_str())
                .collect::<Vec<_>>()
        };

        assert_eq!(
            names(true, true),
            [
                "direct-byte",
                "direct-other",
                "dynamic-other",
                "dev-other",
                "nested-byte",
            ]
        );
        assert_eq!(
            names(true, false),
            [
                "direct-byte",
                "direct-other",
                "dynamic-other",
                "dev-other",
                "nested-byte",
                "hidden-byte",
            ]
        );
        assert_eq!(
            names(false, true),
            [
                "source-har",
                "direct-byte",
                "direct-other",
                "dynamic-other",
                "dev-other",
            ]
        );
        assert_eq!(
            names(false, false),
            ["source-har", "direct-byte", "direct-other", "dynamic-other",]
        );
    }

    #[test]
    fn bytecode_har_dependency_versions_match_direct_and_bundled_hvigor_views() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package("", r#"{ "name": "app", "version": "1.0.0" }"#);
        fixture.write_package(
            "entry",
            r#"{
              "name": "entry",
              "version": "1.0.0",
              "dependencies": {
                "direct-other": "2.0.0",
                "direct-har": "1.0.0",
                "shared": "3.0.0"
              }
            }"#,
        );
        fixture.write_package(
            "entry/oh_modules/direct-other",
            r#"{
              "name": "direct-other",
              "version": "2.0.0",
              "dependencies": { "nested-other": "4.0.0" }
            }"#,
        );
        fixture.write_package(
            "entry/oh_modules/direct-other/oh_modules/nested-other",
            r#"{ "name": "nested-other", "version": "4.0.0" }"#,
        );
        fixture.write_package(
            "entry/oh_modules/direct-har",
            r#"{ "name": "direct-har", "version": "1.0.0" }"#,
        );
        fixture.write_dependency_profile(
            "entry/oh_modules/direct-har",
            "module.json",
            r#"{ "module": { "name": "directhar", "type": "har" } }"#,
        );
        fixture.write_package(
            "entry/oh_modules/shared",
            r#"{ "name": "shared", "version": "3.0.0", "metadata": { "integratedHsp": true } }"#,
        );

        let graph = fixture.build_for("entry");
        assert_eq!(
            graph
                .bytecode_har_dependency_versions(
                    false,
                    false,
                    &Default::default(),
                    &Default::default(),
                )
                .expect("direct dependency versions")
                .into_iter()
                .collect::<Vec<_>>(),
            [
                ("direct-other".to_string(), "2.0.0".to_string()),
                ("direct-har".to_string(), "1.0.0".to_string()),
            ]
        );

        let compiled = ["direct-har".to_string()].into_iter().collect();
        assert_eq!(
            graph
                .bytecode_har_dependency_versions(true, false, &compiled, &Default::default(),)
                .expect("bundled dependency versions")
                .into_iter()
                .collect::<Vec<_>>(),
            [
                ("direct-other".to_string(), "2.0.0".to_string()),
                ("nested-other".to_string(), "4.0.0".to_string()),
            ]
        );
    }

    #[test]
    fn classifies_latest_metadata_integrated_hsp_key() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package("", r#"{ "name": "app", "version": "1.0.0" }"#);
        fixture.write_package(
            "entry",
            r#"{
              "name": "entry",
              "version": "1.0.0",
              "dependencies": { "shared": "1.0.0" }
            }"#,
        );
        fixture.write_package(
            "entry/oh_modules/shared",
            r#"{
              "name": "shared",
              "version": "1.0.0",
              "metadata": { "integratedHsp": true }
            }"#,
        );

        let graph = fixture.build_for("entry");

        assert_eq!(
            graph.nodes["shared"].node_type,
            DependencyNodeType::IntegratedHsp
        );
    }

    #[test]
    fn classifies_source_har_from_packaged_module_json() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package("", r#"{ "name": "app", "version": "1.0.0" }"#);
        fixture.write_package(
            "entry",
            r#"{
              "name": "entry",
              "version": "1.0.0",
              "dependencies": { "source-har": "1.0.0" }
            }"#,
        );
        fixture.write_package(
            "entry/oh_modules/source-har",
            r#"{ "name": "source-har", "version": "1.0.0" }"#,
        );
        fixture.write_dependency_profile(
            "entry/oh_modules/source-har",
            "module.json",
            r#"{ "module": { "name": "sourcehar", "type": "har" } }"#,
        );

        let graph = fixture.build_for("entry");

        assert_eq!(
            graph.nodes["source-har"].node_type,
            DependencyNodeType::SourceHar
        );
    }

    #[test]
    fn snapshots_source_har_home_ability_entries_for_preload_system_so() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package("", r#"{ "name": "app", "version": "1.0.0" }"#);
        fixture.write_package(
            "entry",
            r#"{
              "name": "entry",
              "version": "1.0.0",
              "dependencies": { "source-har": "1.0.0" }
            }"#,
        );
        fixture.write_package(
            "entry/oh_modules/source-har",
            r#"{ "name": "source-har", "version": "1.0.0" }"#,
        );
        fixture.write_dependency_profile(
            "entry/oh_modules/source-har",
            "module.json",
            r#"{
              "module": {
                "name": "sourcehar",
                "type": "har",
                "abilities": [
                  {
                    "srcEntry": "ets/HomeAbility.ets",
                    "skills": [{
                      "entities": ["entity.system.home"],
                      "actions": ["ohos.want.action.home"]
                    }]
                  },
                  {
                    "srcEntry": "ets/OtherAbility.ets",
                    "skills": []
                  }
                ]
              }
            }"#,
        );

        let graph = fixture.build_for("entry");

        assert_eq!(
            graph.nodes["source-har"]
                .module_profile
                .as_ref()
                .expect("dependency module profile")
                .home_ability_src_entries,
            ["ets/HomeAbility.ets"]
        );
    }

    #[test]
    fn classifies_legacy_har_from_distro_module_type() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package("", r#"{ "name": "app", "version": "1.0.0" }"#);
        fixture.write_package(
            "entry",
            r#"{
              "name": "entry",
              "version": "1.0.0",
              "dependencies": { "legacy-har": "1.0.0" }
            }"#,
        );
        fixture.write_package(
            "entry/oh_modules/legacy-har",
            r#"{ "name": "legacy-har", "version": "1.0.0" }"#,
        );
        fixture.write_dependency_profile(
            "entry/oh_modules/legacy-har",
            "config.json",
            r#"{ "module": { "distro": { "moduleType": "har" } } }"#,
        );

        let graph = fixture.build_for("entry");

        assert_eq!(
            graph.nodes["legacy-har"].node_type,
            DependencyNodeType::SourceHar
        );
    }

    #[test]
    fn bundled_har_importees_and_resources_follow_process_har_artifacts() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package("", r#"{ "name": "app", "version": "1.0.0" }"#);
        fixture.write_package(
            "entry",
            r#"{
              "name": "entry",
              "version": "1.0.0",
              "dependencies": { "source-har": "1.0.0", "plain-pkg": "2.0.0" }
            }"#,
        );
        fixture.write_package(
            "entry/oh_modules/source-har",
            r#"{
              "name": "source-har",
              "version": "1.0.0",
              "metadata": { "resource": { "directories": ["src/main/custom"] } }
            }"#,
        );
        fixture.write_dependency_profile(
            "entry/oh_modules/source-har",
            "module.json",
            r#"{ "module": { "name": "sourcehar", "type": "har" } }"#,
        );
        fixture.write_package(
            "entry/oh_modules/plain-pkg",
            r#"{ "name": "plain-pkg", "version": "2.0.0" }"#,
        );

        let graph = fixture.build_for("entry");
        let metadata = graph.har_importee_metadata(
            &[],
            &["source-har".into(), "plain-pkg/feature".into()],
            &["source-har".into(), "plain-pkg/feature".into()],
            true,
            true,
            &[],
        );

        assert_eq!(metadata.compiled_package_names, ["source-har"]);
        assert_eq!(metadata.extra_importees, ["plain-pkg/feature"]);
        assert!(metadata
            .affects_compilation_dependency_keys
            .contains(&"source-har".to_string()));
        let resources = graph.bundled_har_resources();
        assert_eq!(resources.len(), 1);
        assert_eq!(
            resources[0].metadata_directory.as_str(),
            "src/main/custom-source-har"
        );
        assert_eq!(
            resources[0].archive_directory.as_str(),
            "src/main/custom-source-har"
        );
    }

    #[test]
    fn optimized_importees_mark_project_root_direct_dependencies_global() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package(
            "",
            r#"{
              "name": "app",
              "version": "1.0.0",
              "dependencies": { "global-lib": "1.0.0" }
            }"#,
        );
        fixture.write_package("entry", r#"{ "name": "entry", "version": "1.0.0" }"#);
        fixture.write_package(
            "oh_modules/global-lib",
            r#"{ "name": "global-lib", "version": "1.0.0" }"#,
        );

        let graph = fixture.build_for("entry");
        let metadata = graph.har_importee_metadata(
            &[],
            &["global-lib".into()],
            &["global-lib".into()],
            true,
            true,
            &[],
        );

        assert_eq!(metadata.global_dependencies, ["global-lib"]);
    }

    #[test]
    fn bundled_bytecode_har_keep_dependency_false_merges_then_filters_like_hvigor() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package("", r#"{ "name": "app", "version": "1.0.0" }"#);
        fixture.write_package(
            "entry",
            r#"{
              "name": "entry",
              "version": "1.0.0",
              "dependencies": {
                "source-har": "1.0.0",
                "byte-har": "1.0.0"
              },
              "devDependencies": { "dev-source": "1.0.0" },
              "dynamicDependencies": {
                "dyn-source": "1.0.0",
                "dyn-byte": "1.0.0"
              }
            }"#,
        );
        fixture.write_package(
            "entry/oh_modules/source-har",
            r#"{
              "name": "source-har",
              "version": "1.0.0",
              "dependencies": { "byte-har": "2.0.0" }
            }"#,
        );
        fixture.write_dependency_profile(
            "entry/oh_modules/source-har",
            "module.json",
            r#"{ "module": { "name": "source-har", "type": "har" } }"#,
        );
        fixture.write_package(
            "entry/oh_modules/byte-har",
            r#"{
              "name": "byte-har",
              "version": "1.0.0",
              "metadata": { "byteCodeHar": true }
            }"#,
        );
        fixture.write_package(
            "entry/oh_modules/dev-source",
            r#"{ "name": "dev-source", "version": "1.0.0" }"#,
        );
        fixture.write_package(
            "entry/oh_modules/dyn-source",
            r#"{
              "name": "dyn-source",
              "version": "1.0.0",
              "dynamicDependencies": { "dyn-byte": "2.0.0" }
            }"#,
        );
        fixture.write_package(
            "entry/oh_modules/dyn-byte",
            r#"{
              "name": "dyn-byte",
              "version": "1.0.0",
              "metadata": { "byteCodeHar": true }
            }"#,
        );

        let graph = fixture.build_for("entry");
        let metadata = graph.har_importee_metadata(
            &[],
            &["source-har".into()],
            &["source-har".into()],
            true,
            true,
            &[],
        );
        assert_eq!(metadata.compiled_package_names, ["source-har"]);
        assert!(metadata.global_dependencies.is_empty());
        let versions = graph
            .bytecode_har_dependency_versions(
                true,
                true,
                &metadata.compiled_package_names.iter().cloned().collect(),
                &metadata
                    .affects_compilation_dependency_keys
                    .iter()
                    .cloned()
                    .collect(),
            )
            .expect("collect optimized bundled dependency versions");
        assert!(versions.is_empty());
        let package = PackageSnapshot::from_root(&fixture.root().join("entry"))
            .expect("read entry package")
            .expect("entry package");
        let retained = graph
            .bundled_har_retained_dependencies(&package.package_json)
            .expect("rewrite bundled dependencies");

        assert_eq!(
            retained.dependencies.into_iter().collect::<Vec<_>>(),
            vec![("byte-har".to_string(), "2.0.0".to_string())]
        );
        assert_eq!(
            retained
                .dynamic_dependencies
                .into_iter()
                .collect::<Vec<_>>(),
            vec![("dyn-byte".to_string(), "2.0.0".to_string())]
        );
    }

    #[test]
    fn bytecode_har_dependency_versions_resolve_project_parameter_file() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package(
            "",
            r#"{
              "name": "app",
              "version": "1.0.0",
              "parameterFile": "${paths.parameters}",
              "properties": { "paths": { "parameters": "ohpm-params.json5" } }
            }"#,
        );
        fs::write(
            fixture.root().join("ohpm-params.json5"),
            r#"{ "versions.lib": "^1.2.3" }"#,
        )
        .expect("parameter file");
        fixture.write_package(
            "entry",
            r#"{
              "name": "entry",
              "version": "1.0.0",
              "dependencies": { "lib": "@param:versions.lib" }
            }"#,
        );
        fixture.write_package(
            "entry/oh_modules/lib",
            r#"{ "name": "lib", "version": "1.2.3" }"#,
        );

        let graph = fixture.build_for("entry");
        let versions = graph
            .bytecode_har_dependency_versions(
                false,
                false,
                &Default::default(),
                &Default::default(),
            )
            .expect("resolve parameterized dependency version");

        assert_eq!(versions["lib"], "^1.2.3");
        let package = PackageSnapshot::from_root(&fixture.root().join("entry"))
            .expect("read entry package")
            .expect("entry package");
        let resolved = graph
            .dependency_parameters
            .resolve_package_parameters(&package.package_json)
            .expect("resolve generated package parameters");
        assert_eq!(resolved.dependencies["lib"], "^1.2.3");
    }
}
