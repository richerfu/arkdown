//! [`DependencyGraphSnapshot`] — immutable per-host-module dependency graph.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use thiserror::Error;

use arkdown_ohos_schema::RawOhPackage;
use arkdown_path::ArtifactPath;
use indexmap::{IndexMap, IndexSet};

use crate::package::snapshot::PackageSnapshot;

/// Stable module/profile identity loaded from an installed dependency.
///
/// Hvigor exposes these fields through `DependencyInterface.getModuleJsonObj()`
/// to `GeneratePkgContextInfo` and `ProcessProfile`. Keeping them on the
/// session node prevents those stages from re-reading dependency profiles.
#[derive(Debug, Clone, Default)]
pub struct DependencyModuleProfileSnapshot {
    /// `module.name` from `src/main/module.json[5]`.
    pub module_name: Option<String>,
    /// `module.type` (or legacy `module.distro.moduleType`).
    pub module_type: Option<arkdown_core::OhosModuleType>,
    /// `app.bundleName` for common-app remote HSP packages.
    pub bundle_name: Option<String>,
    /// `app.bundleType` for common-app remote HSP packages.
    pub bundle_type: Option<String>,
    /// `app.versionCode` when it is embedded in a packaged HSP profile.
    pub version_code: Option<u64>,
    /// Home ability source entries used as roots for `preloadSystemSo` graph
    /// traversal. Captured with the dependency profile so later stages do not
    /// re-read Source HAR `module.json5` files.
    pub home_ability_src_entries: Vec<String>,
}

/// Classification of a dependency node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyNodeType {
    /// Source HAR (bundled into the output).
    SourceHar,
    /// Bytecode HAR (pre-compiled `.abc` included as-is).
    BytecodeHar,
    /// Integrated HSP (shared package bundled into host).
    IntegratedHsp,
    /// External shared HSP (deployed separately on device).
    SharedHsp,
    /// Native shared library (`.so` file).
    NativeSo,
    /// Regular OHPM source package.
    OhpmSource,
    /// Unknown or unresolved package.
    Unknown,
}

/// Classification of a declared dependency edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyEdgeKind {
    /// `dependencies` entry from `oh-package.json5`.
    Dependency,
    /// `devDependencies` entry from `oh-package.json5`.
    DevDependency,
    /// `dynamicDependencies` entry from `oh-package.json5`.
    DynamicDependency,
}

/// A declared dependency edge in traversal order.
#[derive(Debug, Clone)]
pub struct DependencyEdgeSnapshot {
    /// Package root that declares the dependency.
    pub issuer_root: PathBuf,
    /// Alias (key in `oh-package.json5` dependencies).
    pub alias: String,
    /// Raw dependency specifier from the declaring `oh-package.json5`.
    pub version_spec: Option<String>,
    /// Resolved package name.
    pub package_name: String,
    /// Resolved package root directory.
    pub package_root: PathBuf,
    /// Installed `oh_modules`/`node_modules` alias when the final package
    /// component is a symbolic link. Captured once while the session graph is
    /// built for Hvigor's `share.symlinkMap` and module-record aliases.
    pub installed_alias_path: Option<PathBuf>,
    /// Declared dependency section.
    pub kind: DependencyEdgeKind,
    /// Whether the declaring package is below the module/project dependency
    /// roots in hvigor's breadth-first dependency tree.
    pub is_transitive: bool,
}

/// A node in the dependency graph.
#[derive(Debug, Clone)]
pub struct DependencyNodeSnapshot {
    /// Alias (key in `oh-package.json5` dependencies).
    pub alias: String,
    /// Resolved package name.
    pub package_name: String,
    /// Raw dependency specifier from the declaring `oh-package.json5`.
    pub version_spec: Option<String>,
    /// Resolved package root directory.
    pub package_root: Option<PathBuf>,
    /// Parsed package metadata.
    pub package: Option<Arc<PackageSnapshot>>,
    /// Parsed dependency module/profile identity, when the package ships one.
    pub module_profile: Option<DependencyModuleProfileSnapshot>,
    /// Dependency node classification.
    pub node_type: DependencyNodeType,
    /// Whether this is a runtime-only (not bundled) dependency.
    pub is_runtime_only: bool,
    /// Whether this is a dynamic import dependency.
    pub is_dynamic: bool,
    /// Whether this is a transitive (indirect) dependency.
    pub is_transitive: bool,
    /// Direct dependency aliases declared by this package.
    pub direct_deps: Vec<String>,
}

/// One per-host-module immutable dependency graph.
///
/// Consumers:
/// - share config merge
/// - startup config merge
/// - loader JSON `modulePathMap`, `hspNameOhmMap`, `harNameOhmMap`
/// - bytecode HAR scan
/// - integrated HSP packaging
/// - resolver package root lookup
/// - module meta package lookup
/// - CompileArkTS package metadata
#[derive(Debug, Clone, Default)]
pub struct DependencyGraphSnapshot {
    /// The host module name.
    pub host_module: String,
    /// Canonical package root of the host module. Project-root direct package
    /// nodes use a different issuer and are classified as hvigor globals.
    pub host_root: PathBuf,
    /// Project-level OHPM parameter values used to resolve `@param:`
    /// dependency versions during HAR artifact processing.
    pub dependency_parameters: DependencyParameterSnapshot,
    /// Nodes indexed by package name.
    pub nodes: BTreeMap<String, DependencyNodeSnapshot>,
    /// Declared dependency edges in graph traversal order.
    pub edges: Vec<DependencyEdgeSnapshot>,
    /// Package roots by alias.
    pub roots_by_alias: BTreeMap<String, PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct DependencyParameterSnapshot {
    pub source_path: Option<PathBuf>,
    pub values: Option<serde_json::Value>,
    pub load_error: Option<String>,
}

/// Parameter-resolved fields from hvigor's `OhosModuleTask.paramWhiteList`.
///
/// The generated HAR manifest keeps its original JSON key order; consumers
/// use this snapshot only to replace values in place.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedPackageParameterSnapshot {
    pub version: String,
    pub dependencies: IndexMap<String, String>,
    pub dev_dependencies: IndexMap<String, String>,
    pub dynamic_dependencies: IndexMap<String, String>,
}

#[derive(Debug, Error)]
pub enum DependencyParameterError {
    #[error(
        "cannot resolve dependency version '{version}': no project parameterFile is configured"
    )]
    MissingFile { version: String },
    #[error("cannot load dependency parameterFile {path}: {message}")]
    Load { path: PathBuf, message: String },
    #[error("dependency parameter key '{key}' was not found in {path}")]
    MissingKey { key: String, path: PathBuf },
    #[error("dependency parameter key '{key}' in {path} does not resolve to a string")]
    InvalidValue { key: String, path: PathBuf },
}

impl DependencyParameterSnapshot {
    /// Read the project parameter file once during session construction.
    ///
    /// Latest hvigor resolves the CLI `parameterFile` extra parameter before
    /// the project-level `oh-package.json5` declaration and rejects a missing
    /// configured file while creating the project model.
    pub fn from_project(
        project_root: &Path,
        project_package: Option<&PackageSnapshot>,
        parameter_file_override: Option<&Path>,
    ) -> Result<Self, DependencyParameterError> {
        let configured = parameter_file_override.map(Path::to_path_buf).or_else(|| {
            project_package
                .and_then(|package| package.package_json.extra.get("parameterFile"))
                .and_then(serde_json::Value::as_str)
                .map(PathBuf::from)
        });
        let Some(configured) = configured else {
            return Ok(Self::default());
        };
        let configured = project_package
            .map(|package| Self::interpolate_parameter_file(&configured, package))
            .unwrap_or(configured);
        let source_path = if configured.has_root() {
            configured
        } else {
            project_root.join(configured)
        };
        let content = std::fs::read_to_string(&source_path).map_err(|error| {
            DependencyParameterError::Load {
                path: source_path.clone(),
                message: error.to_string(),
            }
        })?;
        let values = json5::from_str::<serde_json::Value>(&content).map_err(|error| {
            DependencyParameterError::Load {
                path: source_path.clone(),
                message: error.to_string(),
            }
        })?;
        Ok(Self {
            source_path: Some(source_path),
            values: Some(values),
            load_error: None,
        })
    }

    pub(crate) fn from_project_deferred(
        project_root: &Path,
        project_package: Option<&PackageSnapshot>,
        parameter_file_override: Option<&Path>,
    ) -> Self {
        match Self::from_project(project_root, project_package, parameter_file_override) {
            Ok(snapshot) => snapshot,
            Err(DependencyParameterError::Load { path, message }) => Self {
                source_path: Some(path),
                values: None,
                load_error: Some(message),
            },
            Err(error) => Self {
                source_path: None,
                values: None,
                load_error: Some(error.to_string()),
            },
        }
    }

    pub fn resolve_version(&self, version: &str) -> Result<String, DependencyParameterError> {
        let Some(key) = Self::parameter_key(version) else {
            return Ok(version.to_string());
        };
        let path = self.source_path.clone().unwrap_or_default();
        if let Some(message) = self.load_error.as_ref() {
            return Err(DependencyParameterError::Load {
                path,
                message: message.clone(),
            });
        }
        let values = self
            .values
            .as_ref()
            .ok_or_else(|| DependencyParameterError::MissingFile {
                version: version.to_string(),
            })?;
        let segments = key.split('.').collect::<Vec<_>>();
        let value = Self::longest_prefix_value(values, &segments).ok_or_else(|| {
            DependencyParameterError::MissingKey {
                key: key.to_string(),
                path: path.clone(),
            }
        })?;
        value
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| DependencyParameterError::InvalidValue {
                key: key.to_string(),
                path,
            })
    }

    /// Resolve every field processed by latest hvigor's
    /// `OhosModuleTask.paramWhiteList`: `version`, `dependencies`,
    /// `devDependencies`, and `dynamicDependencies`.
    pub fn resolve_package_parameters(
        &self,
        package: &RawOhPackage,
    ) -> Result<ResolvedPackageParameterSnapshot, DependencyParameterError> {
        let mut resolved = ResolvedPackageParameterSnapshot {
            version: self.resolve_version(&package.version)?,
            ..Default::default()
        };
        for (name, version) in &package.dependencies {
            resolved
                .dependencies
                .insert(name.clone(), self.resolve_version(version)?);
        }
        for (name, version) in &package.dev_dependencies {
            resolved
                .dev_dependencies
                .insert(name.clone(), self.resolve_version(version)?);
        }
        for (name, version) in &package.dynamic_dependencies {
            resolved
                .dynamic_dependencies
                .insert(name.clone(), self.resolve_version(version)?);
        }
        Ok(resolved)
    }

    fn parameter_key(version: &str) -> Option<&str> {
        let start = version.find("@param:")? + "@param:".len();
        let tail = &version[start..];
        let end = tail.find(char::is_whitespace).unwrap_or(tail.len());
        (end > 0).then_some(&tail[..end])
    }

    fn longest_prefix_value<'a>(
        value: &'a serde_json::Value,
        segments: &[&str],
    ) -> Option<&'a serde_json::Value> {
        let object = value.as_object()?;
        for split in (1..=segments.len()).rev() {
            let key = segments[..split].join(".");
            let Some(candidate) = object.get(&key) else {
                continue;
            };
            if candidate.is_string() {
                return Some(candidate);
            }
            if split != segments.len() {
                if let Some(resolved) = Self::longest_prefix_value(candidate, &segments[split..]) {
                    return Some(resolved);
                }
            }
        }
        None
    }

    fn interpolate_parameter_file(configured: &Path, package: &PackageSnapshot) -> PathBuf {
        let Some(configured) = configured.to_str() else {
            return configured.to_path_buf();
        };
        let Some(properties) = package
            .package_json
            .extra
            .get("properties")
            .and_then(serde_json::Value::as_object)
        else {
            return configured.into();
        };
        let mut resolved = String::with_capacity(configured.len());
        let mut tail = configured;
        while let Some(start) = tail.find("${") {
            resolved.push_str(&tail[..start]);
            let parameter = &tail[start + 2..];
            let Some(end) = parameter.find('}') else {
                resolved.push_str(&tail[start..]);
                return resolved.into();
            };
            let key = parameter[..end].trim();
            match Self::property_value(properties, key) {
                Some(value) => resolved.push_str(&value),
                None => resolved.push_str(&tail[start..start + 3 + end]),
            }
            tail = &parameter[end + 1..];
        }
        resolved.push_str(tail);
        resolved.into()
    }

    fn property_value(
        properties: &serde_json::Map<String, serde_json::Value>,
        key: &str,
    ) -> Option<String> {
        let mut segments = key.split('.');
        let mut value = properties.get(segments.next()?)?;
        for segment in segments {
            value = value.as_object()?.get(segment)?;
        }
        match value {
            serde_json::Value::String(value) => Some(value.clone()),
            serde_json::Value::Number(value) => Some(value.to_string()),
            serde_json::Value::Bool(value) => Some(value.to_string()),
            _ => None,
        }
    }
}

/// One source-HAR resource directory copied by
/// `ProcessHarArtifacts.processBundleDepRes`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundledHarResourceSnapshot {
    pub package_name: String,
    pub source_dir: PathBuf,
    /// Manifest-relative directory written to `metadata.resource.directories`.
    pub metadata_directory: ArtifactPath,
    /// Directory used inside the HAR staging tree.
    pub archive_directory: ArtifactPath,
}

/// Dependency metadata derived from hvigor's three importee cache arrays.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HarImporteeMetadataSnapshot {
    pub compiled_package_names: Vec<String>,
    pub extra_importees: Vec<String>,
    pub affects_compilation_dependency_keys: Vec<String>,
    pub global_dependencies: Vec<String>,
}

/// Final dependency sections produced by latest hvigor when
/// `hvigor.keepDependency` is falsy for a bundled ByteCode HAR.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BundledHarDependencySnapshot {
    pub dependencies: IndexMap<String, String>,
    pub dynamic_dependencies: IndexMap<String, String>,
}

impl DependencyGraphSnapshot {
    /// Resolve a watcher batch to package names only when every changed path
    /// belongs to a known dependency root or its installed symbolic-link
    /// alias. The deepest matching root wins for nested dependencies.
    ///
    /// `None` means the batch also contains project/configuration changes and
    /// cannot safely use Hvigor's dependency-only incremental checker path.
    pub fn changed_package_names_for_paths<'a>(
        &self,
        paths: impl IntoIterator<Item = &'a Path>,
    ) -> Option<Vec<String>> {
        let mut packages = std::collections::BTreeSet::new();
        let mut saw_path = false;
        for path in paths {
            saw_path = true;
            let matched = self
                .edges
                .iter()
                .filter_map(|edge| {
                    let matched_root = std::iter::once(&edge.package_root)
                        .chain(edge.installed_alias_path.iter())
                        .filter(|root| path.starts_with(root))
                        .max_by_key(|root| root.components().count())?;
                    Some((matched_root.components().count(), &edge.package_name))
                })
                .max_by_key(|(depth, _)| *depth);
            let (_, package_name) = matched?;
            packages.insert(package_name.clone());
        }
        saw_path.then(|| packages.into_iter().collect())
    }

    /// Merge every resolved dependency manifest into the host sections and
    /// retain only HSP and ByteCode HAR aliases.
    ///
    /// This mirrors `ProcessHarArtifacts.deleteOhPkgDep`: JavaScript
    /// `Object.assign` replaces an existing value without moving its key and
    /// appends newly discovered transitive keys in dependency traversal order.
    pub fn bundled_har_retained_dependencies(
        &self,
        host_package: &RawOhPackage,
    ) -> Result<BundledHarDependencySnapshot, DependencyParameterError> {
        let host = self
            .dependency_parameters
            .resolve_package_parameters(host_package)?;
        let mut output = BundledHarDependencySnapshot {
            dependencies: host.dependencies,
            dynamic_dependencies: host.dynamic_dependencies,
        };
        let mut visited_roots = std::collections::BTreeSet::new();
        for edge in &self.edges {
            let Some(node) = self.nodes.get(&edge.package_name) else {
                continue;
            };
            let Some(package) = node.package.as_deref() else {
                continue;
            };
            if !visited_roots.insert(package.root.clone()) {
                continue;
            }
            let resolved = self
                .dependency_parameters
                .resolve_package_parameters(&package.package_json)?;
            for (name, version) in resolved.dependencies {
                output.dependencies.insert(name, version);
            }
            for (name, version) in resolved.dynamic_dependencies {
                output.dynamic_dependencies.insert(name, version);
            }
        }

        let mut retained = IndexSet::new();
        for edge in &self.edges {
            let Some(node) = self.nodes.get(&edge.package_name) else {
                continue;
            };
            if matches!(
                node.node_type,
                DependencyNodeType::BytecodeHar
                    | DependencyNodeType::IntegratedHsp
                    | DependencyNodeType::SharedHsp
            ) {
                retained.insert(edge.alias.as_str());
                retained.insert(node.alias.as_str());
                retained.insert(node.package_name.as_str());
            }
        }
        output
            .dependencies
            .retain(|name, _| retained.contains(name.as_str()));
        output
            .dynamic_dependencies
            .retain(|name, _| retained.contains(name.as_str()));
        Ok(output)
    }

    /// Return an iterator over all HAR nodes (source and bytecode).
    pub fn har_nodes(&self) -> impl Iterator<Item = &DependencyNodeSnapshot> {
        self.nodes.values().filter(|n| {
            matches!(
                n.node_type,
                DependencyNodeType::SourceHar | DependencyNodeType::BytecodeHar
            )
        })
    }

    /// Return an iterator over bytecode HAR nodes.
    pub fn bytecode_har_nodes(&self) -> impl Iterator<Item = &DependencyNodeSnapshot> {
        self.nodes
            .values()
            .filter(|n| matches!(n.node_type, DependencyNodeType::BytecodeHar))
    }

    /// Return ByteCode HAR nodes in declared dependency traversal order.
    ///
    /// Hvigor's `DependencyManager` preserves manifest insertion order when
    /// `GenerateLoaderJson.processByteCodeHar` builds its maps. The node index
    /// is a `BTreeMap` for lookup stability, so artifact writers must recover
    /// semantic order from the edge inventory instead of serializing key order.
    pub fn bytecode_har_nodes_in_traversal_order(&self) -> Vec<&DependencyNodeSnapshot> {
        let mut seen = std::collections::BTreeSet::new();
        self.edges
            .iter()
            .filter_map(|edge| {
                let node = self.nodes.get(&edge.package_name)?;
                (matches!(node.node_type, DependencyNodeType::BytecodeHar)
                    && seen.insert(node.package_name.as_str()))
                .then_some(node)
            })
            .collect()
    }

    /// Return the packages serialized by latest hvigor into a ByteCode HAR's
    /// `harNameOhmMap`, preserving dependency-tree order.
    ///
    /// This mirrors installed hvigor 6.24.3
    /// `GenerateLoaderJson.getHarNameOhmMap()` and
    /// `TaskService.getAffectsCompilationDependencies()`:
    ///
    /// - bundled + optimized walks through source HARs, collects ByteCode HAR
    ///   and OTHER barriers, and does not descend through those barriers;
    /// - bundled + legacy collects every ByteCode HAR and OTHER dependency;
    /// - unbundled + optimized keeps every direct non-HSP dependency;
    /// - unbundled + legacy excludes direct `devDependencies` and HSPs.
    pub fn bytecode_har_ohm_nodes_in_traversal_order(
        &self,
        bundled_dependencies: bool,
        integrated_optimization: bool,
    ) -> Vec<&DependencyNodeSnapshot> {
        let mut selected = Vec::new();
        let mut seen = IndexSet::new();

        if !bundled_dependencies {
            for edge in &self.edges {
                if edge.is_transitive
                    || (!integrated_optimization
                        && matches!(edge.kind, DependencyEdgeKind::DevDependency))
                {
                    continue;
                }
                let Some(node) = self.nodes.get(&edge.package_name) else {
                    continue;
                };
                if matches!(
                    node.node_type,
                    DependencyNodeType::SharedHsp | DependencyNodeType::IntegratedHsp
                ) || !seen.insert(node.package_name.as_str())
                {
                    continue;
                }
                selected.push(node);
            }
            return selected;
        }

        if !integrated_optimization {
            for edge in &self.edges {
                let Some(node) = self.nodes.get(&edge.package_name) else {
                    continue;
                };
                if !Self::is_bytecode_har_ohm_barrier(node)
                    || !seen.insert(node.package_name.as_str())
                {
                    continue;
                }
                selected.push(node);
            }
            return selected;
        }

        let mut expandable_roots = IndexSet::new();
        for edge in &self.edges {
            if edge.is_transitive && !expandable_roots.contains(&edge.issuer_root) {
                continue;
            }
            let Some(node) = self.nodes.get(&edge.package_name) else {
                continue;
            };
            if matches!(node.node_type, DependencyNodeType::SourceHar) {
                expandable_roots.insert(edge.package_root.clone());
                continue;
            }
            if Self::is_bytecode_har_ohm_barrier(node) && seen.insert(node.package_name.as_str()) {
                selected.push(node);
            }
        }
        selected
    }

    fn is_bytecode_har_ohm_barrier(node: &DependencyNodeSnapshot) -> bool {
        matches!(
            node.node_type,
            DependencyNodeType::BytecodeHar
                | DependencyNodeType::NativeSo
                | DependencyNodeType::OhpmSource
                | DependencyNodeType::Unknown
        )
    }

    /// Collect dependency versions serialized into a ByteCode HAR.
    ///
    /// This is the typed equivalent of hvigor 6.24.3
    /// `ProcessHarArtifacts.setDependencyPkgVersion`: a normal ByteCode HAR
    /// records direct HAR and "other" packages, while `bundledDependencies`
    /// walks the full graph and omits packages already compiled into the ABC.
    /// Declaration order follows the manifest edge inventory.
    pub fn bytecode_har_dependency_versions(
        &self,
        bundled_dependencies: bool,
        integrated_optimization: bool,
        compiled_package_names: &std::collections::BTreeSet<String>,
        affects_compilation_dependency_keys: &std::collections::BTreeSet<String>,
    ) -> Result<IndexMap<String, String>, DependencyParameterError> {
        let mut versions = IndexMap::new();
        for edge in &self.edges {
            let Some(node) = self.nodes.get(&edge.package_name) else {
                continue;
            };
            if matches!(
                node.node_type,
                DependencyNodeType::SharedHsp | DependencyNodeType::IntegratedHsp
            ) {
                continue;
            }
            if integrated_optimization {
                let affected = affects_compilation_dependency_keys.contains(&edge.alias)
                    || affects_compilation_dependency_keys.contains(&node.package_name);
                if !affected {
                    continue;
                }
                if bundled_dependencies {
                    // setOptimizeDependencyPkgVersion keeps only ByteCode HAR
                    // and OTHER dependencies from the affected closure.
                    if !matches!(
                        node.node_type,
                        DependencyNodeType::BytecodeHar
                            | DependencyNodeType::OhpmSource
                            | DependencyNodeType::Unknown
                            | DependencyNodeType::NativeSo
                    ) {
                        continue;
                    }
                } else if node.is_transitive {
                    continue;
                }
            } else {
                if !bundled_dependencies && node.is_transitive {
                    continue;
                }
                if bundled_dependencies
                    && (compiled_package_names.contains(&edge.alias)
                        || compiled_package_names.contains(&node.package_name))
                {
                    continue;
                }
            }
            let version = edge
                .version_spec
                .as_ref()
                .or(node.version_spec.as_ref())
                .cloned();
            if let Some(version) = version {
                let version = self.dependency_parameters.resolve_version(&version)?;
                versions.entry(edge.alias.clone()).or_insert(version);
            }
        }
        Ok(versions)
    }

    /// Resolve the three latest-hvigor importee inventories against this
    /// immutable dependency graph.
    pub fn har_importee_metadata(
        &self,
        non_entry_importees: &[String],
        compiled_importees: &[String],
        direct_importees: &[String],
        bundled_dependencies: bool,
        integrated_optimization: bool,
        runtime_only_packages: &[String],
    ) -> HarImporteeMetadataSnapshot {
        let mut affected = IndexSet::new();
        if integrated_optimization {
            affected.extend(runtime_only_packages.iter().cloned());
            // ProcessHarArtifacts runs processDirectImportees first, but a
            // bundled build immediately replaces that inventory in
            // processAllImportees with packages actually present in
            // compiled.importees.json. It does not recursively mark every
            // dependency of an imported source HAR as compilation-affecting.
            let (importees, direct_only) = if bundled_dependencies {
                (compiled_importees, false)
            } else {
                (direct_importees, true)
            };
            for request in importees {
                if let Some((edge, node)) = self.dependency_for_request(request, direct_only, None)
                {
                    affected.insert(edge.alias.clone());
                    affected.insert(node.package_name.clone());
                    if bundled_dependencies && node.node_type == DependencyNodeType::SourceHar {
                        affected.extend(Self::runtime_only_packages(node));
                    }
                }
            }
        }

        let mut compiled_package_names = IndexSet::new();
        let mut extra_importees = IndexSet::new();
        if bundled_dependencies {
            for request in compiled_importees {
                let allowed = integrated_optimization.then_some(&affected);
                let Some((edge, node)) = self.dependency_for_request(request, false, allowed)
                else {
                    continue;
                };
                match node.node_type {
                    DependencyNodeType::SourceHar => {
                        compiled_package_names.insert(request.clone());
                    }
                    DependencyNodeType::OhpmSource
                    | DependencyNodeType::Unknown
                    | DependencyNodeType::NativeSo => {
                        if request != &edge.alias && request != &node.package_name {
                            extra_importees.insert(request.clone());
                        }
                    }
                    DependencyNodeType::BytecodeHar
                    | DependencyNodeType::IntegratedHsp
                    | DependencyNodeType::SharedHsp => {}
                }
            }
        } else {
            for request in non_entry_importees {
                let Some((edge, node)) = self.dependency_for_request(request, true, None) else {
                    continue;
                };
                if matches!(
                    node.node_type,
                    DependencyNodeType::OhpmSource
                        | DependencyNodeType::Unknown
                        | DependencyNodeType::NativeSo
                ) && request != &edge.alias
                    && request != &node.package_name
                {
                    extra_importees.insert(request.clone());
                }
            }
        }

        let mut compiled_package_names = compiled_package_names.into_iter().collect::<Vec<_>>();
        let mut extra_importees = extra_importees.into_iter().collect::<Vec<_>>();
        let mut affects_compilation_dependency_keys = affected.into_iter().collect::<Vec<_>>();
        compiled_package_names.sort();
        extra_importees.sort();
        affects_compilation_dependency_keys.sort();
        // ModuleDependencyInfo selects one direct package node per resolved
        // root, with module declarations taking precedence over project and
        // transitive declarations. Looking at every edge would incorrectly
        // classify a module-direct package as global when a source HAR also
        // declares that same package later in the BFS inventory.
        let mut selected_roots = std::collections::BTreeSet::new();
        let mut global_dependencies = self
            .edges
            .iter()
            .filter(|edge| selected_roots.insert(edge.package_root.clone()))
            .filter(|edge| edge.issuer_root != self.host_root)
            .filter_map(|edge| {
                let node = self.nodes.get(&edge.package_name)?;
                (affects_compilation_dependency_keys.contains(&edge.alias)
                    || affects_compilation_dependency_keys.contains(&node.package_name))
                .then(|| edge.alias.clone())
            })
            .collect::<Vec<_>>();
        global_dependencies.sort();
        global_dependencies.dedup();
        HarImporteeMetadataSnapshot {
            compiled_package_names,
            extra_importees,
            affects_compilation_dependency_keys,
            global_dependencies,
        }
    }

    /// Source-HAR resource directories in declared graph traversal order.
    pub fn bundled_har_resources(&self) -> Vec<BundledHarResourceSnapshot> {
        let mut resources = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for edge in &self.edges {
            let Some(node) = self.nodes.get(&edge.package_name) else {
                continue;
            };
            if node.node_type != DependencyNodeType::SourceHar
                || !seen.insert(node.package_name.as_str())
            {
                continue;
            }
            let Some(package) = node.package.as_deref() else {
                continue;
            };
            for directory in Self::resource_directories(package) {
                let directory = ArtifactPath::from_slash(directory);
                let suffixed_name = format!("{}-{}", directory.file_name(), node.package_name);
                resources.push(BundledHarResourceSnapshot {
                    package_name: node.package_name.clone(),
                    source_dir: package.root.join(directory.as_str()),
                    metadata_directory: ArtifactPath::from_slash(format!(
                        "{}-{}",
                        directory.as_str(),
                        node.package_name
                    )),
                    archive_directory: ArtifactPath::from_slash("src/main").join(&suffixed_name),
                });
            }
        }
        resources
    }

    fn dependency_for_request<'a>(
        &'a self,
        request: &str,
        direct_only: bool,
        allowed: Option<&IndexSet<String>>,
    ) -> Option<(&'a DependencyEdgeSnapshot, &'a DependencyNodeSnapshot)> {
        self.edges.iter().find_map(|edge| {
            let node = self.nodes.get(&edge.package_name)?;
            if direct_only && node.is_transitive {
                return None;
            }
            if allowed.is_some_and(|allowed| {
                !allowed.contains(&edge.alias) && !allowed.contains(&node.package_name)
            }) {
                return None;
            }
            let matches = Self::request_matches_package(request, &edge.alias)
                || Self::request_matches_package(request, &node.package_name);
            matches.then_some((edge, node))
        })
    }

    fn request_matches_package(request: &str, package: &str) -> bool {
        request == package
            || request
                .strip_prefix(package)
                .is_some_and(|suffix| suffix.starts_with('/'))
    }

    fn runtime_only_packages(node: &DependencyNodeSnapshot) -> Vec<String> {
        node.package
            .as_ref()
            .and_then(|package| package.package_json.extra.get("metadata"))
            .and_then(|metadata| metadata.get("runtimeOnly"))
            .and_then(|runtime_only| runtime_only.get("packages"))
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .map(str::to_string)
            .collect()
    }

    fn resource_directories(package: &PackageSnapshot) -> Vec<String> {
        let configured = package
            .package_json
            .extra
            .get("metadata")
            .and_then(|metadata| metadata.get("resource"))
            .and_then(|resource| resource.get("directories"))
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .map(|directory| directory.replace('\\', "/"))
            .collect::<Vec<_>>();
        if configured.is_empty() {
            vec!["src/main/resources".to_string()]
        } else {
            configured
        }
    }

    /// Return an iterator over integrated HSP nodes.
    pub fn integrated_hsp_nodes(&self) -> impl Iterator<Item = &DependencyNodeSnapshot> {
        self.nodes
            .values()
            .filter(|n| matches!(n.node_type, DependencyNodeType::IntegratedHsp))
    }

    /// Return `true` if a path is inside this graph's `oh_modules` ownership.
    ///
    /// Uses the `roots_by_alias` map; no substring matching.
    pub fn owns_path(&self, path: &std::path::Path) -> bool {
        self.roots_by_alias
            .values()
            .any(|root| path.starts_with(root))
    }
}
