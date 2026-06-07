//! Dependency graph for OHOS projects.
//!
//! Implements the `DependencyGraphService` taxonomy from hvigor's
//! `DependencyManager`, which classifies dependency nodes into:
//!
//! - `SourceHar` — source HAR (`type: "har"` in `module.json5`)
//! - `BytecodeHar` — pre-compiled HAR (`byteCodeHar: true` in `oh-package.json5`)
//! - `SharedHsp` — shared HSP (`type: "shared"` in `module.json5`)
//! - `NativeSo` — native library package (contains `.so` files)
//! - `RuntimeOnly` — package listed under `oh-package.json5#metadata.runtimeOnly.packages`
//! - `Dynamic` — package imported via dynamic `import()` expression
//! - `Bundled` — package bundled inside another HAR's `oh_modules/`
//! - `RemoteOrUnknown` — package not found on disk (remote registry, network-only)
//!
//! ## Node Resolution Search Order
//!
//! For each declared dependency name, the graph searches for a physical
//! directory in this order (mirrors `ohpm` install / `DependencyManager`):
//!
//! 1. `<issuer_root>/oh_modules/<name>` — package-local install
//! 2. `<module_root>/oh_modules/<name>` — module-local install
//! 3. `<project_root>/oh_modules/<name>` — project-level install
//! 4. `<OHPM_HOME>/<name>@<version>/oh_modules/<name>` — OHPM global cache
//!
//! The OHPM global cache (`~/.ohpm/` by default, overridden by `$OHPM_HOME`)
//! is checked as a fallback for packages that are registered in the lockfile
//! but not materialised under `oh_modules/`.  This mirrors `DependencyManager`'s
//! treatment of cached remote packages — they are still classified by their
//! on-disk structure once found.

use std::path::{Path, PathBuf};

/// Classification of a dependency node in the dependency graph.
///
/// Mirrors hvigor's `DependencyManager.NodeType` taxonomy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyNodeType {
    /// Source HAR: `module.json5` with `type: "har"`.
    SourceHar,
    /// Pre-compiled ByteCode HAR: `byteCodeHar: true` in `oh-package.json5`.
    BytecodeHar,
    /// Shared HSP: `module.json5` with `type: "shared"`.
    SharedHsp,
    /// Native library package: contains `.so` files under `libs/`.
    NativeSo,
    /// Runtime-only dependency listed in `oh-package.json5#metadata.runtimeOnly.packages`.
    ///
    /// These packages are NOT bundled into the output artifact — they are
    /// expected to be present on the device at runtime.  Hvigor's
    /// `DependencyManager` tracks them separately so they do not appear in
    /// `modulePathMap` but ARE listed in `byteCodeHarInfo` when they are
    /// ByteCode HARs.
    RuntimeOnly,
    /// Dynamically imported dependency (`import('<name>')` expression in source).
    ///
    /// Hvigor resolves dynamic imports at bundle time and records them as
    /// `DynamicImport` nodes distinct from static `import` dependencies.
    Dynamic,
    /// Remote / unknown — not found on any local search path.
    ///
    /// This covers packages that require a network fetch (not yet in any
    /// local cache), corrupt installs, or package names that cannot be matched
    /// to a directory by the path-resolution rules.
    RemoteOrUnknown,
}

/// A single node in the dependency graph.
#[derive(Debug, Clone)]
pub struct DependencyNode {
    /// Package name (from `oh-package.json5#name` or directory alias).
    pub package_name: String,
    /// Absolute path to the package root directory.
    ///
    /// Empty `PathBuf` when `node_type == RemoteOrUnknown` and the package
    /// was not found on disk.
    pub pkg_root: PathBuf,
    /// Classified dependency type.
    pub node_type: DependencyNodeType,
    /// Names of direct dependencies (from `oh-package.json5#dependencies`).
    pub direct_deps: Vec<String>,
    /// Whether this node was introduced through a transitive (indirect) path.
    ///
    /// A node is transitive when it first appears as a dependency of a
    /// dependency rather than as a direct dependency of the module root.
    /// Hvigor's `DependencyManager` exposes this flag to determine packing
    /// and declaration inclusion rules.
    pub is_transitive: bool,
    /// Whether this package is physically located inside another package's
    /// `oh_modules/` directory (i.e. it was installed/bundled by a dependency,
    /// not at the project or module root level).
    ///
    /// Hvigor's `DependencyManager` distinguishes bundled from top-level
    /// packages to compute de-duplication rules for HAR/HAP artifacts.
    /// A bundled node still retains its artifact type classification
    /// (`SourceHar`, `SharedHsp`, etc.) — `is_bundled` is orthogonal.
    pub is_bundled: bool,
}

/// Type alias that aligns with hvigor's `DependencyGraphService` naming.
///
/// All new callers should use `DependencyGraphService` instead of
/// `LocalDependencyGraph`.  The underlying type is the same; the alias
/// provides API naming parity with hvigor's implementation.
pub type DependencyGraphService = LocalDependencyGraph;

/// Dependency graph service for OHOS projects.
///
/// Builds the complete dependency graph from declared `oh-package.json5`
/// entries using BFS traversal.  Covers:
/// - local `oh_modules/` packages at issuer, module, and project levels
/// - OHPM global cache packages (fallback for remote-fetched dependencies)
/// - `runtimeOnly.packages` entries classified as `RuntimeOnly` nodes
/// - `Bundled` classification for packages found inside another package's
///   local `oh_modules/`
///
/// Remote/cache packages that cannot be located on any search path are
/// retained as `RemoteOrUnknown` nodes so that callers can detect missing
/// installations.
///
/// This implements the local portion of hvigor's `DependencyGraphService`.
/// Full remote metadata (registry version manifests, signing status,
/// integrated HSP index) requires network access and is out of scope.
pub struct LocalDependencyGraph {
    /// Map from package name → node, in BFS insertion order.
    pub nodes: indexmap::IndexMap<String, DependencyNode>,
}

impl LocalDependencyGraph {
    /// Build the dependency graph starting from `module_root` and
    /// `project_root` by traversing declared `oh-package.json5` dependencies.
    ///
    /// Also scans `oh-package.json5#metadata.runtimeOnly.packages` at both
    /// roots so that runtime-only ByteCode HARs are classified as
    /// `RuntimeOnly` rather than `RemoteOrUnknown`.
    pub fn build(module_root: &Path, project_root: &Path) -> Self {
        Self::build_with_config(module_root, project_root, None)
    }

    /// Build the dependency graph with explicit resolution configuration.
    ///
    /// Mirrors hvigor's `DependencyManager` construction API — the config
    /// controls offline mode, registry URL, and OHPM cache root.  When
    /// `config.offline` is `false` (the default), packages not found locally
    /// will be fetched from the OHPM registry (step 6 of resolution order).
    ///
    /// Section 3.4: "从 dependency graph 生成所有 package context" — this
    /// is the entry point that drives all remote/cache package resolution.
    pub fn build_with_config(
        module_root: &Path,
        project_root: &Path,
        config: Option<&DependencyResolutionConfig>,
    ) -> Self {
        let mut nodes = indexmap::IndexMap::new();
        let mut queue = std::collections::VecDeque::new();

        // Enqueue direct (non-transitive) dependencies from both roots.
        for dep_name in declared_dep_names(module_root)
            .into_iter()
            .chain(declared_dep_names(project_root))
        {
            queue.push_back((dep_name, module_root.to_path_buf(), false));
        }

        // Enqueue runtimeOnly packages as RuntimeOnly nodes (also non-transitive
        // from the perspective of the module root).
        for pkg_name in runtime_only_package_names(module_root)
            .into_iter()
            .chain(runtime_only_package_names(project_root))
        {
            queue.push_back((pkg_name, module_root.to_path_buf(), false));
        }

        while let Some((dep_name, issuer_root, is_transitive)) = queue.pop_front() {
            if nodes.contains_key(&dep_name) {
                continue;
            }

            // Is this package listed as a runtimeOnly package?
            let is_runtime_only = is_runtime_only_package(&dep_name, module_root, project_root);

            let Some(pkg_root) = resolve_dep_dir_with_config(
                &dep_name,
                &issuer_root,
                module_root,
                project_root,
                config,
            ) else {
                // Package not found locally on any path — mark remote/unknown.
                nodes.insert(
                    dep_name.clone(),
                    DependencyNode {
                        package_name: dep_name,
                        pkg_root: PathBuf::new(),
                        node_type: if is_runtime_only {
                            DependencyNodeType::RuntimeOnly
                        } else {
                            DependencyNodeType::RemoteOrUnknown
                        },
                        direct_deps: vec![],
                        is_transitive,
                        is_bundled: false, // RemoteOrUnknown nodes have no physical location.
                    },
                );
                continue;
            };

            // Determine whether this package is "bundled" — it lives inside
            // another package's oh_modules/ rather than at the project/module root.
            let is_bundled = is_bundled_package(&pkg_root, module_root, project_root);

            // Classify by artifact type regardless of bundled status.
            // RuntimeOnly overrides other types since it affects packing rules.
            let node_type = if is_runtime_only {
                DependencyNodeType::RuntimeOnly
            } else {
                classify_package(&pkg_root)
            };

            let direct_deps = declared_dep_names(&pkg_root);

            for child in &direct_deps {
                if !nodes.contains_key(child.as_str()) {
                    // Child deps of a resolved package are transitive from
                    // the module root's point of view (they are not direct
                    // deps of the module itself).
                    queue.push_back((child.clone(), pkg_root.clone(), true));
                }
            }

            nodes.insert(
                dep_name.clone(),
                DependencyNode {
                    package_name: dep_name,
                    pkg_root,
                    node_type,
                    direct_deps,
                    is_transitive,
                    is_bundled,
                },
            );
        }

        LocalDependencyGraph { nodes }
    }

    /// Return all nodes classified as a specific type, in BFS order.
    pub fn nodes_of_type(&self, ty: DependencyNodeType) -> Vec<&DependencyNode> {
        self.nodes.values().filter(|n| n.node_type == ty).collect()
    }

    /// Return source HAR nodes.
    pub fn source_hars(&self) -> Vec<&DependencyNode> {
        self.nodes_of_type(DependencyNodeType::SourceHar)
    }

    /// Return ByteCode HAR nodes.
    pub fn bytecode_hars(&self) -> Vec<&DependencyNode> {
        self.nodes_of_type(DependencyNodeType::BytecodeHar)
    }

    /// Return shared HSP nodes.
    pub fn shared_hsps(&self) -> Vec<&DependencyNode> {
        self.nodes_of_type(DependencyNodeType::SharedHsp)
    }

    /// Return runtime-only nodes.
    ///
    /// These are packages listed in `oh-package.json5#metadata.runtimeOnly.packages`.
    /// They are NOT bundled into the artifact but may still contain ByteCode HAR
    /// ABC files that need to be tracked in `byteCodeHarInfo`.
    pub fn runtime_only_nodes(&self) -> Vec<&DependencyNode> {
        self.nodes_of_type(DependencyNodeType::RuntimeOnly)
    }

    /// Return transitive (indirect) nodes.
    pub fn transitive_nodes(&self) -> Vec<&DependencyNode> {
        self.nodes.values().filter(|n| n.is_transitive).collect()
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn classify_package(pkg_root: &Path) -> DependencyNodeType {
    // ByteCode HAR: byteCodeHar flag in oh-package.json5.
    if is_bytecode_har(pkg_root) {
        return DependencyNodeType::BytecodeHar;
    }
    // Source/Shared: check module.json5 type field.
    if let Some(module_type) = read_module_type(pkg_root) {
        if module_type == "shared" {
            return DependencyNodeType::SharedHsp;
        }
        if module_type == "har" {
            return DependencyNodeType::SourceHar;
        }
    }
    // Native SO package: contains .so files.
    if has_native_so(pkg_root) {
        return DependencyNodeType::NativeSo;
    }
    DependencyNodeType::RemoteOrUnknown
}

/// Determine whether `pkg_root` is a "bundled" package — i.e. it lives inside
/// another package's `oh_modules/` rather than at the module or project root.
///
/// A bundled package's path contains an `oh_modules` segment that is NOT at
/// the module or project root level.
///
/// Both comparison paths are canonicalized (if possible) to avoid symlink
/// mismatches (e.g. `/tmp` → `/private/tmp` on macOS).
fn is_bundled_package(pkg_root: &Path, module_root: &Path, project_root: &Path) -> bool {
    let canon = |p: PathBuf| std::fs::canonicalize(&p).unwrap_or(p);

    let module_oh = canon(module_root.join("oh_modules"));
    let project_oh = canon(project_root.join("oh_modules"));

    // Strip the top-level oh_modules prefix.  A package is "bundled" when its
    // path still contains an `oh_modules` component AFTER the top-level prefix
    // is removed — meaning it lives inside another package's own oh_modules/.
    let remaining = if pkg_root.starts_with(&module_oh) {
        pkg_root.strip_prefix(&module_oh).ok()
    } else if pkg_root.starts_with(&project_oh) {
        pkg_root.strip_prefix(&project_oh).ok()
    } else {
        // Not under any known oh_modules root.
        return pkg_root.components().any(|c| c.as_os_str() == "oh_modules");
    };

    // The remaining path after the top-level oh_modules is `<pkg_name>/...`.
    // If it contains another oh_modules component, the package is bundled.
    remaining
        .map(|r| r.components().any(|c| c.as_os_str() == "oh_modules"))
        .unwrap_or(false)
}

fn is_bytecode_har(pkg_root: &Path) -> bool {
    let pkg_json = pkg_root.join("oh-package.json5");
    let Ok(content) = std::fs::read_to_string(&pkg_json) else {
        return false;
    };
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct PkgMinimal {
        #[serde(default)]
        byte_code_har: bool,
        #[serde(default)]
        metadata: PkgMeta,
    }
    #[derive(Default, serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct PkgMeta {
        #[serde(default)]
        byte_code_har: bool,
    }
    json5::from_str::<PkgMinimal>(&content)
        .map(|p| p.byte_code_har || p.metadata.byte_code_har)
        .unwrap_or(false)
}

fn read_module_type(pkg_root: &Path) -> Option<String> {
    for path in &["src/main/module.json5", "src/main/module.json"] {
        let file = pkg_root.join(path);
        if !file.is_file() {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&file) else {
            continue;
        };
        #[derive(serde::Deserialize)]
        struct ModuleJson {
            module: ModuleSection,
        }
        #[derive(serde::Deserialize)]
        struct ModuleSection {
            #[serde(rename = "type", default)]
            type_: String,
        }
        if let Ok(m) = json5::from_str::<ModuleJson>(&content) {
            return Some(m.module.type_);
        }
    }
    None
}

fn has_native_so(pkg_root: &Path) -> bool {
    for arm_dir in &[
        "libs/arm64-v8a",
        "libs/armeabi-v7a",
        "libs/x86_64",
        "libs/x86",
        "libs/armeabi",
    ] {
        let dir = pkg_root.join(arm_dir);
        if dir.is_dir() {
            if let Ok(mut entries) = std::fs::read_dir(&dir) {
                if entries.any(|e| {
                    e.ok()
                        .and_then(|entry| entry.path().extension().map(|ext| ext == "so"))
                        .unwrap_or(false)
                }) {
                    return true;
                }
            }
        }
    }
    false
}

fn declared_dep_names(pkg_root: &Path) -> Vec<String> {
    let pkg_json = pkg_root.join("oh-package.json5");
    let Ok(content) = std::fs::read_to_string(&pkg_json) else {
        return vec![];
    };
    #[derive(serde::Deserialize)]
    struct PkgDeps {
        #[serde(default)]
        dependencies: indexmap::IndexMap<String, String>,
    }
    json5::from_str::<PkgDeps>(&content)
        .map(|p| p.dependencies.into_keys().collect())
        .unwrap_or_default()
}

/// Extract the version string for a given dependency from the declared
/// `oh-package.json5#dependencies`.  Returns `None` when the package is not
/// declared or has no version string.
fn declared_dep_version(pkg_root: &Path, dep_name: &str) -> Option<String> {
    let pkg_json = pkg_root.join("oh-package.json5");
    let Ok(content) = std::fs::read_to_string(&pkg_json) else {
        return None;
    };
    #[derive(serde::Deserialize)]
    struct PkgDeps {
        #[serde(default)]
        dependencies: indexmap::IndexMap<String, String>,
    }
    json5::from_str::<PkgDeps>(&content)
        .ok()
        .and_then(|p| p.dependencies.get(dep_name).cloned())
}

/// Extract package names listed in `oh-package.json5#metadata.runtimeOnly.packages`.
///
/// hvigor's `DependencyManager` treats these as `RuntimeOnly` nodes — they are
/// not bundled into the artifact but may still contribute ByteCode HAR info.
fn runtime_only_package_names(pkg_root: &Path) -> Vec<String> {
    let pkg_json = pkg_root.join("oh-package.json5");
    let Ok(content) = std::fs::read_to_string(&pkg_json) else {
        return vec![];
    };
    #[derive(serde::Deserialize)]
    struct Pkg {
        #[serde(default)]
        metadata: PkgMetaRuntime,
    }
    #[derive(Default, serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct PkgMetaRuntime {
        #[serde(default)]
        runtime_only: RuntimeOnlySection,
    }
    #[derive(Default, serde::Deserialize)]
    struct RuntimeOnlySection {
        #[serde(default)]
        packages: Vec<String>,
    }
    json5::from_str::<Pkg>(&content)
        .map(|p| p.metadata.runtime_only.packages)
        .unwrap_or_default()
}

/// Check whether `dep_name` appears in any `runtimeOnly.packages` list
/// reachable from `module_root` or `project_root`.
fn is_runtime_only_package(dep_name: &str, module_root: &Path, project_root: &Path) -> bool {
    runtime_only_package_names(module_root)
        .iter()
        .chain(runtime_only_package_names(project_root).iter())
        .any(|n| n == dep_name)
}

/// Resolve a dependency name to its physical directory.
///
/// Search order (mirrors ohpm / DependencyManager):
/// 1. `<issuer_root>/oh_modules/<dep_name>`
/// 2. `<module_root>/oh_modules/<dep_name>`
/// 3. `<project_root>/oh_modules/<dep_name>`
/// 4. OHPM global cache: `$OHPM_HOME/<dep_name>@<ver>/oh_modules/<dep_name>`
///    where `$OHPM_HOME` defaults to `~/.ohpm` and `<ver>` is read from the
///    issuer's declared dependency version.
/// 5. Offline cache lookup via `oh-package-lock.json5` at `project_root`.
///    Uses [`crate::ohpm_registry::OfflineOhpmCache::resolve_from_lockfile`]
///    to find packages that are in the global cache but whose version is only
/// Configuration for dependency resolution in `DependencyGraphService`.
///
/// Controls offline mode, registry URL overrides, and automatic remote fetch.
/// Mirrors hvigor's `DependencyManager` resolution configuration.
#[derive(Debug, Clone, Default)]
pub struct DependencyResolutionConfig {
    /// When `true`, no network requests are made.  Packages not found locally
    /// or in the OHPM global cache are classified as `RemoteOrUnknown`.
    /// Equivalent to `ohpm install --offline`.
    pub offline: bool,
    /// Override the OHPM registry URL (default: `https://registry.ohpm.openharmony.cn`).
    pub registry_url: Option<String>,
    /// Override the OHPM global cache root (default: `~/.ohpm`).
    pub ohpm_cache_root: Option<PathBuf>,
}

/// Full dependency directory resolution with optional network fetch.
///
/// Implements hvigor's `DependencyManager` resolution order:
/// 1. issuer oh_modules/ — package-local install
/// 2. module oh_modules/ — module-level install
/// 3. project oh_modules/ — project-level install
/// 4. OHPM global cache `~/.ohpm/<name>@<version>/` (from declared version)
/// 5. OHPM global cache via lockfile version
/// 6. OHPM registry fetch (when not offline) — download, verify, install to cache
///
/// Step 6 is the key difference from a purely local graph: hvigor's
/// `DependencyManager` will download packages from the OHPM registry when
/// they are not found locally.  This enables the `RemoteOrUnknown` nodes
/// to be resolved to real package directories.
fn resolve_dep_dir_with_config(
    dep_name: &str,
    issuer_root: &Path,
    module_root: &Path,
    project_root: &Path,
    config: Option<&DependencyResolutionConfig>,
) -> Option<PathBuf> {
    for base in &[
        issuer_root.join("oh_modules"),
        module_root.join("oh_modules"),
        project_root.join("oh_modules"),
    ] {
        let candidate = base.join(dep_name);
        if candidate.is_dir() {
            return Some(std::fs::canonicalize(&candidate).unwrap_or(candidate));
        }
    }

    // Fallback: OHPM global cache.
    // ohpm stores packages under `$OHPM_HOME/<name>@<version>/oh_modules/<name>/`.
    // Uses config.ohpm_cache_root when set (enables test isolation), else
    // falls back to $OHPM_HOME / ~/.ohpm.
    if let Some(cache_root) = config
        .and_then(|c| c.ohpm_cache_root.clone())
        .or_else(ohpm_global_cache_root)
    {
        // Try with the declared version first.
        if let Some(version) = declared_dep_version(issuer_root, dep_name) {
            let versioned = cache_root
                .join(format!("{}@{}", dep_name, version))
                .join("oh_modules")
                .join(dep_name);
            if versioned.is_dir() {
                return Some(std::fs::canonicalize(&versioned).unwrap_or(versioned));
            }
        }
        // Try the scoped package format (@scope+name@version).
        // ohpm may store @scope/pkg as `@scope+pkg@version` on disk.
        if dep_name.starts_with('@') {
            let encoded = dep_name.replacen('/', "+", 1);
            if let Some(version) = declared_dep_version(issuer_root, dep_name) {
                let scoped = cache_root
                    .join(format!("{}@{}", encoded, version))
                    .join("oh_modules")
                    .join(dep_name);
                if scoped.is_dir() {
                    return Some(std::fs::canonicalize(&scoped).unwrap_or(scoped));
                }
            }
        }

        // Step 5: offline cache lookup via lockfile.
        //
        // When the declared version in `oh-package.json5` doesn't match what
        // is actually cached (e.g. the lockfile pins a different resolved
        // version), load the lockfile from `project_root` and ask
        // `OfflineOhpmCache` to resolve the path from it.  This covers the
        // common case where the developer has run `ohpm install` once and the
        // packages are present in the global cache, but the `oh_modules/`
        // directories haven't been re-materialised (e.g. after a `git clean`).
        let lockfile = crate::lockfile::OhPackageLockfile::load(project_root);
        if !lockfile.is_empty() {
            // Use an empty version string so resolve_from_lockfile falls back
            // to the lockfile's recorded version.
            if let Some(path) = crate::ohpm_registry::OfflineOhpmCache::resolve_from_lockfile(
                &lockfile,
                dep_name,
                "",
                &cache_root,
            ) {
                return Some(path);
            }
        }

        // Step 6: OHPM registry fetch (mirrors hvigor's DependencyManager
        // remote node resolution — the key step that makes remote packages
        // concrete instead of RemoteOrUnknown).
        //
        // When a package is not found in any local or cached location, and
        // we are not in offline mode, attempt to fetch it from the OHPM
        // registry, install it to the global cache, and return the installed
        // path.  This matches hvigor's `DependencyManager.resolveRemotePackage()`
        // behavior.
        let is_offline = config.map(|c| c.offline).unwrap_or(false);
        if !is_offline {
            // Determine the package version to fetch.
            let version = declared_dep_version(issuer_root, dep_name)
                .or_else(|| lockfile.resolved_version(dep_name).map(str::to_string));

            if let Some(version) = version {
                let registry_url = config
                    .and_then(|c| c.registry_url.as_deref())
                    .unwrap_or(crate::ohpm_registry::DEFAULT_REGISTRY);

                // Attempt to fetch and install from the OHPM registry.
                match crate::ohpm_registry::OhpmRegistryClient::with_registry(registry_url) {
                    Ok(client) => {
                        let opts = crate::ohpm_registry::OhpmInstallOptions {
                            offline: false,
                            prefer_offline: false,
                            registry_url: None,
                        };
                        tracing::debug!(
                            package = %dep_name,
                            version = %version,
                            registry = %registry_url,
                            "DependencyManager: fetching remote package from OHPM registry"
                        );
                        match client.install(dep_name, &version, &cache_root, &opts) {
                            Ok(installed_path) => {
                                tracing::info!(
                                    package = %dep_name,
                                    version = %version,
                                    path = %installed_path.display(),
                                    "DependencyManager: remote package installed from OHPM registry"
                                );
                                return Some(installed_path);
                            }
                            Err(e) => {
                                tracing::debug!(
                                    package = %dep_name,
                                    error = %e,
                                    "DependencyManager: OHPM registry fetch failed, classifying as RemoteOrUnknown"
                                );
                            }
                        }
                    }
                    Err(e) => {
                        tracing::debug!(
                            error = %e,
                            "DependencyManager: failed to create OHPM registry client"
                        );
                    }
                }
            }
        }
    }

    None
}

/// Return the OHPM global cache root directory.
///
/// Checks `$OHPM_HOME` first, then falls back to `~/.ohpm`.
/// Returns `None` when neither path can be determined (e.g. no `$HOME`).
fn ohpm_global_cache_root() -> Option<PathBuf> {
    if let Some(ohpm_home) = std::env::var_os("OHPM_HOME") {
        let p = PathBuf::from(ohpm_home);
        if p.is_dir() {
            return Some(p);
        }
    }
    let home = home_dir()?;
    let p = home.join(".ohpm");
    if p.is_dir() {
        Some(p)
    } else {
        Some(p)
    } // always return the path, even if not yet created
}

/// Portable home-directory resolution.
fn home_dir() -> Option<PathBuf> {
    // Try $HOME first (Unix / CI environments).
    if let Some(h) = std::env::var_os("HOME") {
        return Some(PathBuf::from(h));
    }
    // Windows: try $USERPROFILE.
    if let Some(h) = std::env::var_os("USERPROFILE") {
        return Some(PathBuf::from(h));
    }
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_pkg(dir: &Path, name: &str, extra: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(
            dir.join("oh-package.json5"),
            format!(r#"{{ "name": "{name}", "dependencies": {{}} {extra} }}"#),
        )
        .unwrap();
    }

    fn write_module_json(pkg: &Path, ty: &str) {
        let src_main = pkg.join("src/main");
        fs::create_dir_all(&src_main).unwrap();
        fs::write(
            src_main.join("module.json5"),
            format!(r#"{{ "module": {{ "name": "mod", "type": "{ty}" }} }}"#),
        )
        .unwrap();
    }

    #[test]
    fn test_classify_source_har() {
        let tmp = tempfile::tempdir().unwrap();
        let pkg = tmp.path().join("oh_modules/my-har");
        write_pkg(&pkg, "my-har", "");
        write_module_json(&pkg, "har");
        fs::write(
            tmp.path().join("oh-package.json5"),
            r#"{ "dependencies": { "my-har": "1.0.0" } }"#,
        )
        .unwrap();

        let graph = LocalDependencyGraph::build(tmp.path(), tmp.path());
        let node = graph.nodes.get("my-har").expect("my-har must be in graph");
        assert_eq!(node.node_type, DependencyNodeType::SourceHar);
        assert!(
            !node.is_transitive,
            "direct dependency must not be transitive"
        );
    }

    #[test]
    fn test_classify_bytecode_har() {
        let tmp = tempfile::tempdir().unwrap();
        let pkg = tmp.path().join("oh_modules/bc-har");
        fs::create_dir_all(&pkg).unwrap();
        fs::write(
            pkg.join("oh-package.json5"),
            r#"{ "name": "bc-har", "byteCodeHar": true, "dependencies": {} }"#,
        )
        .unwrap();
        fs::create_dir_all(pkg.join("ets")).unwrap();
        fs::write(pkg.join("ets/modules.abc"), b"PANDA\x00\x00\x00").unwrap();
        fs::write(
            tmp.path().join("oh-package.json5"),
            r#"{ "dependencies": { "bc-har": "1.0.0" } }"#,
        )
        .unwrap();

        let graph = LocalDependencyGraph::build(tmp.path(), tmp.path());
        let node = graph.nodes.get("bc-har").expect("bc-har must be in graph");
        assert_eq!(node.node_type, DependencyNodeType::BytecodeHar);
    }

    #[test]
    fn test_classify_shared_hsp() {
        let tmp = tempfile::tempdir().unwrap();
        let pkg = tmp.path().join("oh_modules/my-hsp");
        write_pkg(&pkg, "my-hsp", "");
        write_module_json(&pkg, "shared");
        fs::write(
            tmp.path().join("oh-package.json5"),
            r#"{ "dependencies": { "my-hsp": "1.0.0" } }"#,
        )
        .unwrap();

        let graph = LocalDependencyGraph::build(tmp.path(), tmp.path());
        let node = graph.nodes.get("my-hsp").expect("my-hsp must be in graph");
        assert_eq!(node.node_type, DependencyNodeType::SharedHsp);
    }

    #[test]
    fn test_classify_remote_or_unknown() {
        let tmp = tempfile::tempdir().unwrap();
        // No oh_modules directory — package not found locally.
        fs::write(
            tmp.path().join("oh-package.json5"),
            r#"{ "dependencies": { "remote-pkg": "1.0.0" } }"#,
        )
        .unwrap();

        let graph = LocalDependencyGraph::build(tmp.path(), tmp.path());
        let node = graph
            .nodes
            .get("remote-pkg")
            .expect("remote-pkg must be in graph");
        assert_eq!(node.node_type, DependencyNodeType::RemoteOrUnknown);
    }

    /// runtimeOnly packages must be classified as RuntimeOnly, not RemoteOrUnknown,
    /// even when they cannot be found on disk (they live on the device at runtime).
    #[test]
    fn test_runtime_only_package_classified_as_runtime_only() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("oh-package.json5"),
            r#"{
                "dependencies": {},
                "metadata": {
                    "runtimeOnly": {
                        "packages": ["@runtime/deviceLib"]
                    }
                }
            }"#,
        )
        .unwrap();

        let graph = LocalDependencyGraph::build(tmp.path(), tmp.path());
        let node = graph
            .nodes
            .get("@runtime/deviceLib")
            .expect("runtimeOnly pkg must be in graph");
        assert_eq!(
            node.node_type,
            DependencyNodeType::RuntimeOnly,
            "runtimeOnly packages must be classified as RuntimeOnly"
        );
    }

    /// A package that is installed locally AND listed in runtimeOnly should
    /// be classified as RuntimeOnly (RuntimeOnly overrides other classifications).
    #[test]
    fn test_runtime_only_overrides_local_classification() {
        let tmp = tempfile::tempdir().unwrap();
        let pkg = tmp.path().join("oh_modules/rt-har");
        write_pkg(&pkg, "rt-har", "");
        write_module_json(&pkg, "har");
        fs::write(
            tmp.path().join("oh-package.json5"),
            r#"{
                "dependencies": { "rt-har": "1.0.0" },
                "metadata": { "runtimeOnly": { "packages": ["rt-har"] } }
            }"#,
        )
        .unwrap();

        let graph = LocalDependencyGraph::build(tmp.path(), tmp.path());
        let node = graph.nodes.get("rt-har").expect("rt-har must be in graph");
        assert_eq!(
            node.node_type,
            DependencyNodeType::RuntimeOnly,
            "RuntimeOnly must override SourceHar classification"
        );
    }

    /// Transitive dependencies (deps of deps) must have is_transitive = true.
    #[test]
    fn test_transitive_flag_on_indirect_deps() {
        let tmp = tempfile::tempdir().unwrap();
        let oh = tmp.path().join("oh_modules");

        // Direct dep: dep-a, which declares dep-b as its own dependency.
        let dep_a = oh.join("dep-a");
        fs::create_dir_all(&dep_a).unwrap();
        write_module_json(&dep_a, "har");
        fs::write(
            dep_a.join("oh-package.json5"),
            r#"{ "name": "dep-a", "dependencies": { "dep-b": "1.0.0" } }"#,
        )
        .unwrap();

        // dep-b: installed inside dep-a's oh_modules (bundled).
        let dep_b = dep_a.join("oh_modules/dep-b");
        write_pkg(&dep_b, "dep-b", "");
        write_module_json(&dep_b, "har");

        fs::write(
            tmp.path().join("oh-package.json5"),
            r#"{ "dependencies": { "dep-a": "1.0.0" } }"#,
        )
        .unwrap();

        let graph = LocalDependencyGraph::build(tmp.path(), tmp.path());

        let a = graph.nodes.get("dep-a").expect("dep-a in graph");
        assert!(!a.is_transitive, "direct dep must not be transitive");

        let b = graph.nodes.get("dep-b").expect("dep-b in graph");
        assert!(b.is_transitive, "dep-of-dep must be transitive");
    }

    /// A package nested inside another package's oh_modules/ must have
    /// `is_bundled = true` but still retain its artifact type classification
    /// (SourceHar), because hvigor's DependencyManager classifies by artifact
    /// type and tracks bundled status as a separate property.
    #[test]
    fn test_bundled_package_has_is_bundled_flag() {
        let tmp = tempfile::tempdir().unwrap();
        let oh = tmp.path().join("oh_modules");

        let outer = oh.join("outer");
        fs::create_dir_all(&outer).unwrap();
        write_module_json(&outer, "har");
        // outer depends on inner, which is nested inside outer's oh_modules.
        fs::write(
            outer.join("oh-package.json5"),
            r#"{ "name": "outer", "dependencies": { "inner": "1.0.0" } }"#,
        )
        .unwrap();

        let inner = outer.join("oh_modules/inner");
        write_pkg(&inner, "inner", "");
        write_module_json(&inner, "har");

        fs::write(
            tmp.path().join("oh-package.json5"),
            r#"{ "dependencies": { "outer": "1.0.0" } }"#,
        )
        .unwrap();

        let graph = LocalDependencyGraph::build(tmp.path(), tmp.path());
        let inner_node = graph.nodes.get("inner").expect("inner in graph");
        // The nested package is still a SourceHar by artifact type.
        assert_eq!(
            inner_node.node_type,
            DependencyNodeType::SourceHar,
            "bundled package must retain its artifact type classification"
        );
        // But it is flagged as bundled (lives inside another package's oh_modules/).
        assert!(
            inner_node.is_bundled,
            "nested package must have is_bundled = true"
        );
        // The outer package is NOT bundled.
        let outer_node = graph.nodes.get("outer").expect("outer in graph");
        assert!(
            !outer_node.is_bundled,
            "top-level oh_modules package must not be bundled"
        );
    }

    #[test]
    fn test_bfs_insertion_order() {
        let tmp = tempfile::tempdir().unwrap();
        let oh_modules = tmp.path().join("oh_modules");

        for name in &["alpha", "beta", "gamma"] {
            let pkg = oh_modules.join(name);
            write_pkg(&pkg, name, "");
            write_module_json(&pkg, "har");
        }
        fs::write(
            tmp.path().join("oh-package.json5"),
            r#"{ "dependencies": { "alpha": "1.0.0", "beta": "1.0.0", "gamma": "1.0.0" } }"#,
        )
        .unwrap();

        let graph = LocalDependencyGraph::build(tmp.path(), tmp.path());
        let names: Vec<_> = graph.nodes.keys().cloned().collect();
        assert_eq!(
            names,
            vec!["alpha", "beta", "gamma"],
            "BFS insertion order must be preserved"
        );
    }

    /// helper_methods: source_hars(), bytecode_hars(), shared_hsps(), runtime_only_nodes()
    #[test]
    fn test_helper_methods_by_node_type() {
        let tmp = tempfile::tempdir().unwrap();
        let oh = tmp.path().join("oh_modules");

        let src_pkg = oh.join("src-har");
        write_pkg(&src_pkg, "src-har", "");
        write_module_json(&src_pkg, "har");

        let hsp_pkg = oh.join("my-hsp");
        write_pkg(&hsp_pkg, "my-hsp", "");
        write_module_json(&hsp_pkg, "shared");

        fs::write(
            tmp.path().join("oh-package.json5"),
            r#"{
                "dependencies": { "src-har": "1.0.0", "my-hsp": "1.0.0" },
                "metadata": { "runtimeOnly": { "packages": ["rt-pkg"] } }
            }"#,
        )
        .unwrap();

        let graph = LocalDependencyGraph::build(tmp.path(), tmp.path());
        assert_eq!(graph.source_hars().len(), 1);
        assert_eq!(graph.shared_hsps().len(), 1);
        assert_eq!(graph.runtime_only_nodes().len(), 1);
    }

    /// When `offline=true`, packages not found locally must be classified as
    /// `RemoteOrUnknown` rather than triggering a network fetch.
    ///
    /// This mirrors hvigor's `--offline` mode where `DependencyManager` skips
    /// the remote resolution step and returns `RemoteOrUnknown` nodes for
    /// packages that are not installed locally or in the OHPM global cache.
    #[test]
    fn test_dependency_resolution_config_offline_mode() {
        let tmp = tempfile::tempdir().unwrap();
        // Only declare a dependency without installing it.
        fs::write(
            tmp.path().join("oh-package.json5"),
            r#"{ "dependencies": { "not-installed": "1.0.0" } }"#,
        )
        .unwrap();

        let offline_config = DependencyResolutionConfig {
            offline: true,
            registry_url: None,
            ohpm_cache_root: Some(tmp.path().join("empty-cache")), // non-existent cache
        };
        let graph =
            LocalDependencyGraph::build_with_config(tmp.path(), tmp.path(), Some(&offline_config));
        let node = graph
            .nodes
            .get("not-installed")
            .expect("node must be in graph");
        assert_eq!(
            node.node_type,
            DependencyNodeType::RemoteOrUnknown,
            "offline mode must classify unfound packages as RemoteOrUnknown, not attempt registry fetch"
        );
    }

    /// Integration test: DependencyManager remote resolution (step 6) using a
    /// real local HTTP server.
    ///
    /// This test verifies that `build_with_config()` with `offline=false`
    /// ACTUALLY fetches and installs a package from an OHPM-compatible registry
    /// when the package is not found locally.  It uses a local TCP server so
    /// the test is deterministic and does not require internet access.
    ///
    /// The test asserts `SourceHar` unconditionally — if the HTTP server is
    /// up and serving, the DependencyManager MUST successfully install the
    /// package and classify it.  Any fallback to `RemoteOrUnknown` is a
    /// test failure.  This mirrors hvigor's `DependencyManager.resolveRemotePackage()`
    /// contract: given a reachable registry, remote nodes are always resolved.
    #[test]
    fn test_dependency_resolution_config_remote_fetch_via_local_registry() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            Arc as StdArc,
        };

        let tmp_project = tempfile::tempdir().unwrap();
        let tmp_cache = tempfile::tempdir().unwrap();
        let pkg_tgz = create_minimal_har_tgz("test-remote-pkg", "1.0.0");

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let registry_url = format!("http://{}", addr);

        // Track whether the tarball was actually served (proof of HTTP roundtrip).
        let tarball_served = StdArc::new(AtomicBool::new(false));
        let tarball_served_clone = tarball_served.clone();

        let tgz_bytes = pkg_tgz.clone();
        std::thread::spawn(move || {
            for _ in 0..4 {
                let Ok((mut stream, _)) = listener.accept() else {
                    break;
                };
                let mut buf = [0u8; 8192];
                let n = stream.read(&mut buf).unwrap_or(0);
                let req = std::str::from_utf8(&buf[..n]).unwrap_or("");

                if req.contains("/test-remote-pkg") && !req.contains(".tgz") {
                    let body = format!(
                        r#"{{"name":"test-remote-pkg","versions":{{"1.0.0":{{"version":"1.0.0","dist":{{"tarball":"http://{addr}/test-remote-pkg/-/test-remote-pkg-1.0.0.tgz"}}}}}}}}"#
                    );
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(), body
                    );
                    let _ = stream.write_all(response.as_bytes());
                } else if req.contains(".tgz") {
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        tgz_bytes.len()
                    );
                    let _ = stream.write_all(response.as_bytes());
                    let _ = stream.write_all(&tgz_bytes);
                    tarball_served_clone.store(true, Ordering::Relaxed);
                }
            }
        });

        std::thread::sleep(std::time::Duration::from_millis(100));

        fs::write(
            tmp_project.path().join("oh-package.json5"),
            r#"{ "dependencies": { "test-remote-pkg": "1.0.0" } }"#,
        )
        .unwrap();

        let config = DependencyResolutionConfig {
            offline: false,
            registry_url: Some(registry_url),
            ohpm_cache_root: Some(tmp_cache.path().to_path_buf()),
        };

        let graph = LocalDependencyGraph::build_with_config(
            tmp_project.path(),
            tmp_project.path(),
            Some(&config),
        );

        let node = graph
            .nodes
            .get("test-remote-pkg")
            .expect("node must be in graph");

        if tarball_served.load(Ordering::Relaxed) {
            // The HTTP server DID serve the tarball — verify the DependencyManager
            // resolved the package correctly.  This is the hvigor-equivalent
            // assertion: resolveRemotePackage() must produce a concrete node type.
            assert_eq!(
                node.node_type,
                DependencyNodeType::SourceHar,
                "After tarball was served and extracted, package must be SourceHar \
                 (module.json5 type=har).  Got {:?}.  \
                 This means the DependencyManager did NOT reclassify the remote package \
                 after installation, which breaks hvigor DependencyManager equivalence.",
                node.node_type
            );
            assert!(
                !node.pkg_root.as_os_str().is_empty(),
                "installed package must have a non-empty pkg_root path"
            );
            assert!(
                node.pkg_root.is_dir(),
                "pkg_root must point to an actual directory: {}",
                node.pkg_root.display()
            );
        } else {
            // The HTTP server did NOT serve the tarball (reqwest connection issue
            // or race condition).  Fail with a diagnostic rather than silently
            // passing — the test is intended to exercise the HTTP path.
            panic!(
                "DependencyManager integration test FAILED: the local HTTP server \
                 did not serve the tarball.  The remote resolution path (step 6) \
                 was not exercised.  \
                 node_type = {:?}, pkg_root = {:?}",
                node.node_type, node.pkg_root
            );
        }
    }

    /// Create a minimal .tgz archive representing a source HAR package.
    /// Used by the remote resolution integration test.
    fn create_minimal_har_tgz(name: &str, _version: &str) -> Vec<u8> {
        use flate2::{write::GzEncoder, Compression};

        let mut tar_builder = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::default()));

        // Add oh-package.json5.
        let pkg_json = format!(r#"{{"name": "{}", "version": "1.0.0"}}"#, name);
        let pkg_bytes = pkg_json.as_bytes();
        let mut header = tar::Header::new_gnu();
        header.set_path("package/oh-package.json5").unwrap();
        header.set_size(pkg_bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar_builder.append(&header, pkg_bytes).unwrap();

        // Add src/main/module.json5 with type "har" so it's classified as SourceHar.
        let module_json = r#"{"module":{"name":"test-remote-pkg","type":"har"}}"#;
        let module_bytes = module_json.as_bytes();
        let mut header2 = tar::Header::new_gnu();
        header2.set_path("package/src/main/module.json5").unwrap();
        header2.set_size(module_bytes.len() as u64);
        header2.set_mode(0o644);
        header2.set_cksum();
        tar_builder.append(&header2, module_bytes).unwrap();

        tar_builder.into_inner().unwrap().finish().unwrap()
    }
}
