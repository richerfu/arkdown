use rustc_hash::FxHashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arkdown_core::{ModuleType, OhosResolvedModule, PkgContext};
use arkdown_path::{
    ArtifactPathSerializer, ModuleSpecifier, ModuleSpecifierText, OhmSchema, OhmUrl, SpecifierKind,
};
use oxc_resolver::{
    EnforceExtension, ResolveOptions as OxcResolveOptions, Resolver as OxcResolver,
    TsconfigDiscovery, TsconfigOptions, TsconfigReferences,
};

use crate::error::ResolveError;
use crate::oh_modules::OhPackage;
use crate::plan::{PackageManifestSnapshot, PackageResolutionPlan};

/// Configuration for the OHOS module resolver.
pub struct ResolverConfig {
    /// Absolute path to the project root directory.
    pub project_root: PathBuf,
    /// Paths to SDK `api/` directories (for `@ohos/*` specifiers).
    pub sdk_api_dirs: Vec<PathBuf>,
    /// Paths to SDK `kits/` directories (for `@kit.*` specifiers).
    pub sdk_kits_dirs: Vec<PathBuf>,
    /// Paths to SDK `arkts/` directories (for `@arkts.*` specifiers).
    pub sdk_arkts_dirs: Vec<PathBuf>,
    /// Non-package lookup roots from hvigor `arkts-pack.getResolveModules()`.
    ///
    /// The active module's `src/main` root precedes its generated profile
    /// directory, allowing generated sources such as `BuildProfile.ets` to be
    /// imported through the same bare-specifier lookup used by hvigor.
    pub source_roots: Vec<PathBuf>,
    /// Absolute module lookup directories matching hvigor `getResolveModules()`.
    pub module_paths: Vec<PathBuf>,
    /// Immutable package-name/source-root index derived before hooks run.
    pub package_resolution: Arc<PackageResolutionPlan>,
    /// Whether to use normalized OHM URLs as module identifiers.
    pub use_normalized_ohm_url: bool,
    /// Whether to enforce case-sensitive import checks.
    pub case_sensitive_check: bool,
    /// Source-set overlay: when a non-default build target is active, resolved
    /// paths under `src/main/ets/` may be shadowed by a target-specific file.
    ///
    /// Each entry is `(src_main_ets_root, target_ets_root)`:
    /// - `src_main_ets_root`: the standard source root, e.g. `<module>/src/main/ets`
    /// - `target_ets_root`: the override root, e.g. `<module>/src/phone/ets`
    ///
    /// When resolution produces a path under `src_main_ets_root`, the resolver
    /// checks if a file with the same relative path exists under `target_ets_root`
    /// and redirects to it if found (target-specific file wins).
    ///
    /// hvigor calls this the "source-set overlay"; it allows target-specific `.ets`
    /// files to shadow the default `src/main/ets` versions without changing import
    /// specifiers in the source code.
    pub source_set_overlays: Vec<(PathBuf, PathBuf)>,
}

/// OHOS module resolver.
///
/// Maps import specifiers to [`OhosResolvedModule`] values following OHOS-specific
/// resolution rules (OHM URLs, SDK APIs, `oh_modules/`, relative imports).
pub struct OhosResolver {
    config: ResolverConfig,
    fs_resolver: OxcResolver,
    require_fs_resolver: OxcResolver,
    package_roots: FxHashMap<String, Arc<PackageRoot>>,
    package_roots_by_path: FxHashMap<PathBuf, Arc<PackageRoot>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolveImportKind {
    Import,
    Require,
}

#[derive(Debug, Clone)]
struct PackageRoot {
    path: PathBuf,
    package_name: String,
    manifest: Arc<PackageManifestSnapshot>,
    local_source: bool,
    source_roots: Vec<PathBuf>,
}

struct ResolverSpecifier<'a> {
    raw: &'a str,
    kind: SpecifierKind,
}

impl<'a> ResolverSpecifier<'a> {
    fn new(raw: &'a str) -> Self {
        Self {
            raw,
            kind: ModuleSpecifierText::new(raw).kind(),
        }
    }

    fn as_str(&self) -> &'a str {
        self.raw
    }

    fn ohm_url(&self) -> Option<OhmUrl> {
        matches!(self.kind, SpecifierKind::OhmUrl)
            .then(|| OhmUrl::parse(self.raw).ok())
            .flatten()
    }

    fn sdk_api_filenames(&self) -> Vec<String> {
        ModuleSpecifierText::new(self.raw).sdk_api_filenames()
    }

    fn sdk_kit_filenames(&self) -> Vec<String> {
        ModuleSpecifierText::new(self.raw).sdk_kit_filenames()
    }

    fn sdk_arkts_filenames(&self) -> Vec<String> {
        ModuleSpecifierText::new(self.raw).sdk_arkts_filenames()
    }

    fn is_relative(&self) -> bool {
        matches!(self.kind, SpecifierKind::Relative)
    }

    fn package_candidates(&self) -> Vec<arkdown_path::PackageSpecifierCandidate> {
        ModuleSpecifierText::new(self.raw).package_candidates()
    }

    fn as_relative_request(&self) -> String {
        if self.is_relative() {
            self.raw.to_string()
        } else {
            ModuleSpecifier::from_relative_path(Path::new(self.raw)).into_string()
        }
    }
}

impl OhosResolver {
    /// Create a new resolver with the given configuration.
    pub fn new(config: ResolverConfig) -> Self {
        let fs_resolver = OxcResolver::new(Self::oxc_resolve_options(
            &config,
            ResolveImportKind::Import,
        ));
        let require_fs_resolver = OxcResolver::new(Self::oxc_resolve_options(
            &config,
            ResolveImportKind::Require,
        ));
        let (package_roots, package_roots_by_path) = PackageRootIndex::build(&config).into_roots();
        OhosResolver {
            config,
            fs_resolver,
            require_fs_resolver,
            package_roots,
            package_roots_by_path,
        }
    }

    /// Resolve `specifier` imported from `importer`.
    ///
    /// Resolution order:
    /// 1. `@bundle:` / `@shared:` → external bundle / HSP reference
    /// 2. Bare source names from hvigor's module source/profile roots
    /// 3. Bare package names, including scoped `@ohos/*` packages, from
    ///    the package-resolution plan / `oh_modules/`
    /// 4. `@ohos/` → SDK API declaration
    /// 5. `@kit.` → SDK kit declaration
    /// 6. Relative (`./`, `../`) → local source file
    pub fn resolve(
        &self,
        specifier: &str,
        importer: &Path,
    ) -> Result<OhosResolvedModule, ResolveError> {
        self.resolve_with_kind(specifier, importer, ResolveImportKind::Import)
    }

    pub fn resolve_with_kind(
        &self,
        specifier: &str,
        importer: &Path,
        import_kind: ResolveImportKind,
    ) -> Result<OhosResolvedModule, ResolveError> {
        let specifier = ResolverSpecifier::new(specifier);

        // 1. External cross-bundle / HSP references
        if let Some(ohm_url) = specifier.ohm_url() {
            match ohm_url.schema() {
                Some(OhmSchema::Bundle) => {
                    return Ok(OhosResolvedModule::ExternalBundle {
                        ohm_url: ohm_url.into_string(),
                    });
                }
                Some(OhmSchema::Shared) => {
                    let parts = ohm_url
                        .shared_parts()
                        .ok_or_else(|| ResolveError::NotFound {
                            specifier: specifier.as_str().to_string(),
                            importer: ResolverPathText::new(importer).diagnostic_string(),
                        })?;
                    return Ok(OhosResolvedModule::ExternalHsp {
                        bundle_name: parts.bundle_name.to_string(),
                        module_name: parts.module_name.to_string(),
                        entry_path: String::new(),
                    });
                }
                _ => {}
            }
        }

        // 2. `arkts-pack.getResolveModules()` places `src/main` and the
        // generated profile directory before both `oh_modules` roots. Resolve
        // those as ordinary local sources rather than inventing a virtual
        // BuildProfile module.
        if let Some(resolved) = self.resolve_from_source_roots(&specifier, import_kind) {
            return Ok(resolved);
        }

        // 3. Bare packages take precedence over SDK declarations. OHOS package
        // names often use the `@ohos/*` scope; hvigor resolves those through
        // dependency metadata/oh_modules before falling back to SDK APIs.
        if let Some(resolved) = self.resolve_bare_package(&specifier, importer, import_kind) {
            return Ok(resolved);
        }

        // 4. @ohos/* / @ohos.* → SDK API declaration
        let api_filenames = specifier.sdk_api_filenames();
        if !api_filenames.is_empty() {
            if let Some(path) =
                SdkDeclarationLookup::new(&self.config.sdk_api_dirs, &api_filenames).find()
            {
                return Ok(OhosResolvedModule::SdkDeclaration {
                    path,
                    specifier: specifier.as_str().to_string(),
                });
            }
            return Err(ResolveError::SdkModuleNotFound {
                specifier: specifier.as_str().to_string(),
            });
        }

        // 5. @kit.* → SDK kit declaration
        let kit_filenames = specifier.sdk_kit_filenames();
        if !kit_filenames.is_empty() {
            if let Some(path) =
                SdkDeclarationLookup::new(&self.config.sdk_kits_dirs, &kit_filenames).find()
            {
                return Ok(OhosResolvedModule::SdkDeclaration {
                    path,
                    specifier: specifier.as_str().to_string(),
                });
            }
            return Err(ResolveError::SdkModuleNotFound {
                specifier: specifier.as_str().to_string(),
            });
        }

        // 6. @arkts.* → SDK ArkTS declaration
        let arkts_filenames = specifier.sdk_arkts_filenames();
        if !arkts_filenames.is_empty() {
            if let Some(path) =
                SdkDeclarationLookup::new(&self.config.sdk_arkts_dirs, &arkts_filenames).find()
            {
                return Ok(OhosResolvedModule::SdkDeclaration {
                    path,
                    specifier: specifier.as_str().to_string(),
                });
            }
            return Err(ResolveError::SdkModuleNotFound {
                specifier: specifier.as_str().to_string(),
            });
        }

        // 7. Relative imports
        if specifier.is_relative() {
            if let Ok(resolution) = self
                .fs_resolver_for(import_kind)
                .resolve_file(importer, specifier.as_str())
            {
                let resolved_buf = resolution.into_path_buf();
                let resolved = arkdown_path::canonicalize_or_owned(&resolved_buf);

                // Source-set overlay: check if a target-specific file shadows this path.
                //
                // hvigor supports `src/<target>/ets/` overlaying `src/main/ets/`:
                // if the resolved path is under the base root, check if a file with
                // the same relative path exists under the target root.  Target-specific
                // files take priority (closest match wins).
                let resolved = self.apply_source_set_overlay(resolved);

                let module_type = ModuleType::infer(&resolved);
                return Ok(OhosResolvedModule::LocalSource {
                    path: resolved,
                    module_type,
                    pkg_context: None,
                });
            }

            return Err(ResolveError::NotFound {
                specifier: specifier.as_str().to_string(),
                importer: ResolverPathText::new(importer).diagnostic_string(),
            });
        }

        Err(ResolveError::NotFound {
            specifier: specifier.as_str().to_string(),
            importer: ResolverPathText::new(importer).diagnostic_string(),
        })
    }

    fn resolve_from_source_roots(
        &self,
        specifier: &ResolverSpecifier<'_>,
        import_kind: ResolveImportKind,
    ) -> Option<OhosResolvedModule> {
        if specifier.is_relative() || specifier.package_candidates().is_empty() {
            return None;
        }
        let request = specifier.as_relative_request();
        for root in &self.config.source_roots {
            let resolution = self
                .fs_resolver_for(import_kind)
                .resolve(root, &request)
                .ok();
            let Some(resolution) = resolution else {
                continue;
            };
            let resolved = arkdown_path::canonicalize_or_owned(&resolution.into_path_buf());
            let resolved = self.apply_source_set_overlay(resolved);
            return Some(OhosResolvedModule::LocalSource {
                module_type: ModuleType::infer(&resolved),
                path: resolved,
                pkg_context: None,
            });
        }
        None
    }

    fn resolve_bare_package(
        &self,
        specifier: &ResolverSpecifier<'_>,
        importer: &Path,
        import_kind: ResolveImportKind,
    ) -> Option<OhosResolvedModule> {
        let candidates = specifier.package_candidates();
        if candidates.is_empty() {
            return None;
        }

        let importer_package = self.package_for_importer(importer);
        for candidate in candidates {
            let is_self_reference = importer_package
                .as_ref()
                .is_some_and(|package| package.package_name == candidate.package_name());
            if !is_self_reference {
                if let Some(resolved) = importer_package.as_ref().and_then(|package| {
                    self.resolve_package_from_importer_modules(
                        candidate.package_name(),
                        candidate.subpath(),
                        package,
                        import_kind,
                    )
                }) {
                    return Some(resolved);
                }
            }
            if let Some(pkg_root) = self.package_root_for(candidate.package_name()) {
                if let Some(resolved) = self.resolve_package_entry(
                    candidate.package_name(),
                    candidate.subpath(),
                    &pkg_root,
                    import_kind,
                ) {
                    return Some(resolved);
                }
                if is_self_reference {
                    if let Some(resolved) = self.resolve_package_source_root(
                        candidate.package_name(),
                        candidate.subpath(),
                        &pkg_root,
                        import_kind,
                    ) {
                        return Some(resolved);
                    }
                }
            }
        }
        self.resolve_package_from_module_paths(specifier, importer, import_kind)
    }

    /// Find hvigor's importer `meta.pkgName` equivalent without filesystem
    /// discovery. Package roots are canonicalized when the plan is built; this
    /// loop only probes the immutable root index by path component.
    fn package_for_importer(&self, importer: &Path) -> Option<Arc<PackageRoot>> {
        let mut current = if importer.extension().is_some() {
            importer.parent()
        } else {
            Some(importer)
        };
        while let Some(path) = current {
            if let Some(package) = self.package_roots_by_path.get(path) {
                return Some(Arc::clone(package));
            }
            current = path.parent();
        }
        None
    }

    /// Resolve an undeclared package from the importing package's own
    /// `oh_modules` directory before host/project module paths. This matches
    /// node-resolve's `baseDir = dirname(importer)` walk and is required for
    /// source packages such as `msi_runtime` whose installed dependency links
    /// are more complete than their published manifest.
    fn resolve_package_from_importer_modules(
        &self,
        package_name: &str,
        subpath: &str,
        importer_package: &PackageRoot,
        import_kind: ResolveImportKind,
    ) -> Option<OhosResolvedModule> {
        let package_root = importer_package.path.join("oh_modules").join(package_name);
        if !package_root.is_dir() {
            return None;
        }
        let package_root = arkdown_path::canonicalize_or_owned(&package_root);
        let manifest = OhPackage::load_from_root(&package_root)
            .as_ref()
            .map(PackageManifestSnapshot::from)
            .unwrap_or_default();
        self.resolve_package_entry(
            package_name,
            subpath,
            &PackageRoot {
                path: package_root,
                package_name: package_name.to_string(),
                manifest: Arc::new(manifest),
                local_source: false,
                source_roots: Vec::new(),
            },
            import_kind,
        )
    }

    fn resolve_package_source_root(
        &self,
        package_name: &str,
        subpath: &str,
        pkg_root: &PackageRoot,
        import_kind: ResolveImportKind,
    ) -> Option<OhosResolvedModule> {
        let request = if subpath.is_empty() { "index" } else { subpath };
        for source_root in &pkg_root.source_roots {
            let Some(resolved) = self.resolve_package_path(source_root, request, import_kind)
            else {
                continue;
            };
            let module_type = ModuleType::infer(&resolved);
            return Some(pkg_root.to_resolved_module(resolved, module_type, package_name));
        }
        None
    }

    fn package_root_for(&self, package_name: &str) -> Option<Arc<PackageRoot>> {
        if let Some(root) = self.package_roots.get(package_name) {
            return Some(root.clone());
        }
        if self.config.case_sensitive_check {
            return None;
        }
        self.package_roots
            .get(&PackageNameCase::new(package_name).lowercase())
            .cloned()
    }

    fn resolve_package_from_module_paths(
        &self,
        specifier: &ResolverSpecifier<'_>,
        importer: &Path,
        import_kind: ResolveImportKind,
    ) -> Option<OhosResolvedModule> {
        if self.config.module_paths.is_empty() {
            return None;
        }
        let resolution = self
            .fs_resolver_for(import_kind)
            .resolve(importer, specifier.as_str())
            .ok()?;
        let resolved_buf = resolution.into_path_buf();
        let resolved = arkdown_path::canonicalize_or_owned(&resolved_buf);
        let package_root = PhysicalPackageRoot::new(&resolved).find()?;
        let package = OhPackage::load_from_root(&package_root);
        let package_name = package
            .as_ref()
            .and_then(|package| package.name().map(str::to_string))
            .filter(|name| !name.is_empty())
            .or_else(|| {
                specifier
                    .package_candidates()
                    .into_iter()
                    .next()
                    .map(|candidate| candidate.package_name().to_string())
            })?;
        let module_type = ModuleType::infer(&resolved);
        Some(
            PackageRoot {
                path: package_root,
                package_name: package_name.clone(),
                manifest: Arc::new(
                    package
                        .as_ref()
                        .map(PackageManifestSnapshot::from)
                        .unwrap_or_default(),
                ),
                local_source: false,
                source_roots: Vec::new(),
            }
            .to_resolved_module(resolved, module_type, &package_name),
        )
    }

    fn resolve_package_entry(
        &self,
        package_name: &str,
        subpath: &str,
        pkg_root: &PackageRoot,
        import_kind: ResolveImportKind,
    ) -> Option<OhosResolvedModule> {
        if let Some(abc_path) = pkg_root.manifest.bytecode_abc_path(&pkg_root.path) {
            let declaration_path = pkg_root.manifest.declaration_entry(&pkg_root.path);
            return Some(OhosResolvedModule::BytecodeHar {
                abc_path,
                declaration_path,
                pkg_name: package_name.to_string(),
                ohm_url: OhmUrl::bundle_package(package_name).into_string(),
            });
        }

        let requested = if subpath.is_empty() {
            match import_kind {
                ResolveImportKind::Import => pkg_root.manifest.exports_import_field(),
                ResolveImportKind::Require => pkg_root.manifest.exports_require_field(),
            }
            .or_else(|| pkg_root.manifest.hvigor_main_field().map(str::to_string))
            .unwrap_or_else(|| "index".to_string())
        } else {
            subpath.to_string()
        };
        let resolved = self
            .resolve_package_path(&pkg_root.path, &requested, import_kind)
            .or_else(|| {
                subpath
                    .is_empty()
                    .then(|| self.resolve_package_path(&pkg_root.path, "index", import_kind))
                    .flatten()
            })?;
        let module_type = ModuleType::infer(&resolved);

        Some(pkg_root.to_resolved_module(resolved, module_type, package_name))
    }

    fn resolve_package_path(
        &self,
        package_root: &Path,
        request: &str,
        import_kind: ResolveImportKind,
    ) -> Option<PathBuf> {
        let request = ResolverSpecifier::new(request).as_relative_request();
        self.fs_resolver_for(import_kind)
            .resolve(package_root, &request)
            .ok()
            .map(|resolution| {
                let buf = resolution.into_path_buf();
                arkdown_path::canonicalize_or_owned(&buf)
            })
    }

    /// Apply source-set overlay: if the resolved path is under a `src/main/ets`
    /// base root and a corresponding file exists in the target-specific overlay
    /// root (`src/<target>/ets`), return the target-specific path instead.
    ///
    /// hvigor's source-set overlay allows `src/<target>/ets/Foo.ets` to shadow
    /// `src/main/ets/Foo.ets` without changing import specifiers.  Target-specific
    /// files take full precedence; imports that resolve to files without a target
    /// override pass through unchanged.
    fn apply_source_set_overlay(&self, resolved: PathBuf) -> PathBuf {
        for (base_root, target_root) in &self.config.source_set_overlays {
            // Canonicalize the base root to match the already-canonicalized resolved path.
            let canonical_base = arkdown_path::canonicalize_or_owned(base_root);
            if let Ok(relative) = resolved.strip_prefix(&canonical_base) {
                let candidate = target_root.join(relative);
                if candidate.is_file() {
                    return arkdown_path::canonicalize_or_owned(&candidate);
                }
            }
        }
        resolved
    }

    fn fs_resolver_for(&self, import_kind: ResolveImportKind) -> &OxcResolver {
        match import_kind {
            ResolveImportKind::Import => &self.fs_resolver,
            ResolveImportKind::Require => &self.require_fs_resolver,
        }
    }

    fn oxc_resolve_options(
        config: &ResolverConfig,
        import_kind: ResolveImportKind,
    ) -> OxcResolveOptions {
        let mut modules: Vec<String> = config
            .module_paths
            .iter()
            .map(|path| {
                path.to_str()
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| path.display().to_string())
            })
            .collect();
        modules.push("oh_modules".to_string());

        // F23 — tscResolve: if tsconfig.json exists at the project root, enable
        // TypeScript paths-based resolution.  This mirrors hvigor's useTscResolve
        // option: when active, the TypeScript compiler's path-mapping rules in
        // tsconfig.json (compilerOptions.paths) are honoured during resolution.
        let tsconfig_path = config.project_root.join("tsconfig.json");
        let tsconfig = if tsconfig_path.exists() {
            Some(TsconfigDiscovery::Manual(TsconfigOptions {
                config_file: tsconfig_path,
                references: TsconfigReferences::Disabled,
            }))
        } else {
            None
        };

        OxcResolveOptions {
            cwd: Some(config.project_root.clone()),
            extensions: [
                ".ets", ".ts", ".js", ".mjs", ".cjs", ".json", ".d.ets", ".d.ts",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
            condition_names: match import_kind {
                ResolveImportKind::Import => vec!["import".to_string()],
                ResolveImportKind::Require => vec!["require".to_string()],
            },
            // hvigor-arkts-compose node-resolve `getMainFields()` prefers ESM
            // package entries before the CommonJS fallback.
            main_fields: ["module", "jsnext:main", "main"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            main_files: vec!["index".to_string()],
            modules,
            node_path: false,
            enforce_extension: EnforceExtension::Disabled,
            symlinks: true,
            tsconfig,
            ..OxcResolveOptions::default()
        }
    }
}

struct SdkDeclarationLookup<'a> {
    dirs: &'a [PathBuf],
    filenames: &'a [String],
}

impl<'a> SdkDeclarationLookup<'a> {
    fn new(dirs: &'a [PathBuf], filenames: &'a [String]) -> Self {
        Self { dirs, filenames }
    }

    fn find(&self) -> Option<PathBuf> {
        for dir in self.dirs {
            for filename in self.filenames {
                let candidate = dir.join(filename);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
        None
    }
}

impl PackageRoot {
    fn to_resolved_module(
        &self,
        path: PathBuf,
        module_type: ModuleType,
        package_name: &str,
    ) -> OhosResolvedModule {
        if self.local_source {
            return OhosResolvedModule::LocalSource {
                path,
                module_type,
                pkg_context: Some(PkgContext {
                    package_name: self.package_name.clone(),
                    bundle_name: String::new(),
                    module_name: String::new(),
                    version: self.manifest.version.clone(),
                    pkg_path: self.path.clone(),
                    entry_path: None,
                }),
            };
        }

        OhosResolvedModule::RemotePackage {
            path,
            module_type,
            pkg_name: package_name.to_string(),
            pkg_version: self.manifest.version.clone(),
            ohm_url: None,
        }
    }
}

struct PackageRootIndex {
    roots: FxHashMap<String, Arc<PackageRoot>>,
    case_insensitive_roots: FxHashMap<String, Arc<PackageRoot>>,
}

impl PackageRootIndex {
    fn build(config: &ResolverConfig) -> Self {
        let mut index = Self {
            roots: FxHashMap::default(),
            case_insensitive_roots: FxHashMap::default(),
        };

        for (name, info) in config.package_resolution.sorted_entries() {
            let package_root = arkdown_path::canonicalize_or_owned(&info.pkg_root);
            let root = Arc::new(PackageRoot {
                source_roots: info
                    .source_roots
                    .iter()
                    .map(|source_root| {
                        let source_root = if source_root.has_root() {
                            source_root.clone()
                        } else {
                            package_root.join(source_root)
                        };
                        arkdown_path::canonicalize_or_owned(&source_root)
                    })
                    .collect(),
                path: package_root,
                package_name: info.pkg_name.clone(),
                manifest: Arc::clone(&info.manifest),
                local_source: info.local_source,
            });
            index
                .case_insensitive_roots
                .entry(PackageNameCase::new(name).lowercase())
                .or_insert_with(|| root.clone());
            index.roots.insert(name.to_string(), root);
        }

        index
    }

    fn into_roots(
        mut self,
    ) -> (
        FxHashMap<String, Arc<PackageRoot>>,
        FxHashMap<PathBuf, Arc<PackageRoot>>,
    ) {
        for (name, root) in self.case_insensitive_roots {
            self.roots.entry(name).or_insert(root);
        }
        let mut by_path = FxHashMap::default();
        for root in self.roots.values() {
            by_path
                .entry(root.path.clone())
                .or_insert_with(|| root.clone());
        }
        (self.roots, by_path)
    }
}

struct PackageNameCase<'a> {
    name: &'a str,
}

impl<'a> PackageNameCase<'a> {
    fn new(name: &'a str) -> Self {
        Self { name }
    }

    fn lowercase(&self) -> String {
        self.name.to_lowercase()
    }
}

struct PhysicalPackageRoot<'a> {
    resolved_path: &'a Path,
}

impl<'a> PhysicalPackageRoot<'a> {
    fn new(resolved_path: &'a Path) -> Self {
        Self { resolved_path }
    }

    fn find(&self) -> Option<PathBuf> {
        let mut current = if self.resolved_path.is_dir() {
            Some(self.resolved_path)
        } else {
            self.resolved_path.parent()
        };
        while let Some(path) = current {
            if path.join("oh-package.json5").is_file() {
                return Some(arkdown_path::canonicalize_or_owned(path));
            }
            current = path.parent();
        }
        None
    }
}

struct ResolverPathText<'a> {
    path: &'a Path,
}

impl<'a> ResolverPathText<'a> {
    fn new(path: &'a Path) -> Self {
        Self { path }
    }

    fn slash_string(&self) -> Option<String> {
        ArtifactPathSerializer::path_to_slash_string(self.path).ok()
    }

    fn diagnostic_string(&self) -> String {
        self.slash_string()
            .unwrap_or_else(|| self.path.display().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::PkgBriefInfo;
    use std::fs;
    use tempfile::TempDir;

    fn make_resolver(
        project_root: PathBuf,
        sdk_api_dirs: Vec<PathBuf>,
        sdk_kits_dirs: Vec<PathBuf>,
        packages: FxHashMap<String, PkgBriefInfo>,
    ) -> OhosResolver {
        OhosResolver::new(ResolverConfig {
            project_root,
            sdk_api_dirs,
            sdk_kits_dirs,
            sdk_arkts_dirs: vec![],
            source_roots: vec![],
            module_paths: vec![],
            package_resolution: Arc::new(packages.into()),
            use_normalized_ohm_url: true,
            case_sensitive_check: false,
            source_set_overlays: vec![],
        })
    }

    fn pkg_index(name: &str, root: &Path, local_source: bool) -> FxHashMap<String, PkgBriefInfo> {
        let mut map = FxHashMap::default();
        map.insert(
            name.to_string(),
            PkgBriefInfo {
                pkg_root: root.to_path_buf(),
                pkg_name: name.to_string(),
                manifest: Arc::new(
                    OhPackage::load_from_root(root)
                        .as_ref()
                        .map(PackageManifestSnapshot::from)
                        .unwrap_or_default(),
                ),
                local_source,
                source_roots: vec![],
            },
        );
        map
    }

    // Test 1: @bundle: → ExternalBundle
    #[test]
    fn test_external_bundle() {
        let dir = TempDir::new().unwrap();
        let resolver = make_resolver(
            dir.path().to_path_buf(),
            vec![],
            vec![],
            FxHashMap::default(),
        );
        let importer = dir.path().join("src/main.ets");
        let result = resolver
            .resolve("@bundle:com.example/entry/ets/pages/Index", &importer)
            .unwrap();
        match result {
            OhosResolvedModule::ExternalBundle { ohm_url } => {
                assert_eq!(ohm_url, "@bundle:com.example/entry/ets/pages/Index");
            }
            _ => panic!("expected ExternalBundle"),
        }
    }

    // Test 2: @ohos/hilog → SdkDeclaration
    #[test]
    fn test_ohos_sdk_declaration() {
        let dir = TempDir::new().unwrap();
        let sdk_api_dir = dir.path().join("sdk/api");
        fs::create_dir_all(&sdk_api_dir).unwrap();
        let decl_path = sdk_api_dir.join("@ohos.hilog.d.ts");
        fs::write(&decl_path, "// hilog decls").unwrap();

        let resolver = make_resolver(
            dir.path().to_path_buf(),
            vec![sdk_api_dir],
            vec![],
            FxHashMap::default(),
        );
        let importer = dir.path().join("src/main.ets");
        let result = resolver.resolve("@ohos/hilog", &importer).unwrap();
        match result {
            OhosResolvedModule::SdkDeclaration { path, specifier } => {
                assert_eq!(specifier, "@ohos/hilog");
                assert!(path.exists());
            }
            _ => panic!("expected SdkDeclaration"),
        }
    }

    #[test]
    fn test_ohos_sdk_declaration_d_ets_candidate() {
        let dir = TempDir::new().unwrap();
        let sdk_api_dir = dir.path().join("sdk/api");
        fs::create_dir_all(&sdk_api_dir).unwrap();
        let decl_path = sdk_api_dir.join("@ohos.file.PhotoPickerComponent.d.ets");
        fs::write(&decl_path, "// component decls").unwrap();

        let resolver = make_resolver(
            dir.path().to_path_buf(),
            vec![sdk_api_dir],
            vec![],
            FxHashMap::default(),
        );
        let importer = dir.path().join("src/main.ets");
        let result = resolver
            .resolve("@ohos.file.PhotoPickerComponent", &importer)
            .unwrap();
        match result {
            OhosResolvedModule::SdkDeclaration { path, specifier } => {
                assert_eq!(specifier, "@ohos.file.PhotoPickerComponent");
                assert_eq!(path, decl_path);
            }
            _ => panic!("expected SdkDeclaration"),
        }
    }

    #[test]
    fn test_ohos_scoped_package_precedes_sdk_declaration() {
        let dir = TempDir::new().unwrap();
        let sdk_api_dir = dir.path().join("sdk/api");
        fs::create_dir_all(&sdk_api_dir).unwrap();
        fs::write(sdk_api_dir.join("@ohos.utils.d.ts"), "// sdk utils").unwrap();

        let pkg_dir = dir.path().join("oh_modules/@ohos/utils");
        fs::create_dir_all(pkg_dir.join("src/main/ets")).unwrap();
        fs::write(
            pkg_dir.join("oh-package.json5"),
            r#"{ name: "@ohos/utils", main: "src/main/ets/index.ets" }"#,
        )
        .unwrap();
        let entry = pkg_dir.join("src/main/ets/index.ets");
        fs::write(&entry, "export const value = 1;").unwrap();

        let resolver = make_resolver(
            dir.path().to_path_buf(),
            vec![sdk_api_dir],
            vec![],
            pkg_index("@ohos/utils", &pkg_dir, false),
        );
        let importer = dir.path().join("src/main.ets");
        let result = resolver.resolve("@ohos/utils", &importer).unwrap();
        match result {
            OhosResolvedModule::RemotePackage { path, pkg_name, .. } => {
                assert_eq!(pkg_name, "@ohos/utils");
                assert_eq!(path, arkdown_path::canonicalize_or_owned(&entry));
            }
            _ => panic!("expected RemotePackage"),
        }
    }

    #[test]
    fn declared_package_manifest_is_frozen_before_resolver_hooks_run() {
        let dir = TempDir::new().unwrap();
        let pkg_dir = dir.path().join("oh_modules/frozen-package");
        fs::create_dir_all(pkg_dir.join("src/main/ets")).unwrap();
        let original_entry = pkg_dir.join("src/main/ets/original.ets");
        let mutated_entry = pkg_dir.join("src/main/ets/mutated.ets");
        fs::write(&original_entry, "export const value = 'original';").unwrap();
        fs::write(&mutated_entry, "export const value = 'mutated';").unwrap();
        fs::write(
            pkg_dir.join("oh-package.json5"),
            r#"{ name: "frozen-package", main: "src/main/ets/original.ets" }"#,
        )
        .unwrap();

        // BuildSession projects the manifest into PackageResolutionPlan once.
        // A later disk mutation must not alter resolver-hook behavior.
        let package_plan = pkg_index("frozen-package", &pkg_dir, false);
        fs::write(
            pkg_dir.join("oh-package.json5"),
            r#"{ name: "frozen-package", main: "src/main/ets/mutated.ets" }"#,
        )
        .unwrap();

        let resolver = make_resolver(dir.path().to_path_buf(), vec![], vec![], package_plan);
        let result = resolver
            .resolve("frozen-package", &dir.path().join("src/main.ets"))
            .unwrap();
        match result {
            OhosResolvedModule::RemotePackage { path, .. } => {
                assert_eq!(path, arkdown_path::canonicalize_or_owned(&original_entry));
            }
            _ => panic!("expected RemotePackage"),
        }
    }

    // Test 3: @kit.ArkUI → SdkDeclaration
    #[test]
    fn test_kit_sdk_declaration() {
        let dir = TempDir::new().unwrap();
        let kits_dir = dir.path().join("sdk/kits");
        fs::create_dir_all(&kits_dir).unwrap();
        let decl_path = kits_dir.join("@kit.ArkUI.d.ts");
        fs::write(&decl_path, "// ArkUI decls").unwrap();

        let resolver = make_resolver(
            dir.path().to_path_buf(),
            vec![],
            vec![kits_dir],
            FxHashMap::default(),
        );
        let importer = dir.path().join("src/main.ets");
        let result = resolver.resolve("@kit.ArkUI", &importer).unwrap();
        match result {
            OhosResolvedModule::SdkDeclaration { path, specifier } => {
                assert_eq!(specifier, "@kit.ArkUI");
                assert!(path.exists());
            }
            _ => panic!("expected SdkDeclaration"),
        }
    }

    #[test]
    fn test_arkts_sdk_declaration() {
        let dir = TempDir::new().unwrap();
        let arkts_dir = dir.path().join("sdk/ets/arkts");
        fs::create_dir_all(&arkts_dir).unwrap();
        let decl_path = arkts_dir.join("@arkts.utils.d.ets");
        fs::write(&decl_path, "// arkts utils decls").unwrap();

        let resolver = OhosResolver::new(ResolverConfig {
            project_root: dir.path().to_path_buf(),
            sdk_api_dirs: vec![],
            sdk_kits_dirs: vec![],
            sdk_arkts_dirs: vec![arkts_dir],
            source_roots: vec![],
            module_paths: vec![],
            package_resolution: Arc::new(PackageResolutionPlan::default()),
            use_normalized_ohm_url: true,
            case_sensitive_check: false,
            source_set_overlays: vec![],
        });
        let importer = dir.path().join("src/main.ets");
        let result = resolver.resolve("@arkts.utils", &importer).unwrap();
        match result {
            OhosResolvedModule::SdkDeclaration { path, specifier } => {
                assert_eq!(specifier, "@arkts.utils");
                assert_eq!(path, decl_path);
            }
            _ => panic!("expected SdkDeclaration"),
        }
    }

    // Test 4: parse_ohm_url round-trip
    #[test]
    fn test_parse_ohm_url_via_resolver() {
        use crate::ohm_url::OhmUrlParseResult;
        let url = "@bundle:com.example/entry/ets/pages/Index";
        let parsed = OhmUrlParseResult::parse(url).unwrap();
        assert_eq!(parsed.bundle, "com.example");
        assert_eq!(parsed.module, "entry");
        assert_eq!(parsed.path, "ets/pages/Index");
    }

    // Test 6: relative import resolution
    #[test]
    fn test_relative_import() {
        let dir = TempDir::new().unwrap();
        let src_dir = dir.path().join("src/ets");
        fs::create_dir_all(&src_dir).unwrap();
        let helper = src_dir.join("helper.ets");
        fs::write(&helper, "// helper").unwrap();

        let resolver = make_resolver(
            dir.path().to_path_buf(),
            vec![],
            vec![],
            FxHashMap::default(),
        );
        let importer = src_dir.join("main.ets");
        let result = resolver.resolve("./helper", &importer).unwrap();
        match result {
            OhosResolvedModule::LocalSource {
                path, module_type, ..
            } => {
                assert_eq!(path, arkdown_path::canonicalize_or_owned(&helper));
                assert_eq!(module_type, ModuleType::Ets);
            }
            _ => panic!("expected LocalSource"),
        }
    }

    #[test]
    fn test_relative_directory_self_import_resolves_index() {
        let dir = TempDir::new().unwrap();
        let src_dir = dir.path().join("src/ets");
        fs::create_dir_all(src_dir.join("feature")).unwrap();
        let index = src_dir.join("feature/index.ets");
        fs::write(&index, "export const value = 1;").unwrap();

        let resolver = make_resolver(
            dir.path().to_path_buf(),
            vec![],
            vec![],
            FxHashMap::default(),
        );
        let importer = src_dir.join("feature/main.ets");
        let result = resolver.resolve(".", &importer).unwrap();
        match result {
            OhosResolvedModule::LocalSource { path, .. } => {
                assert_eq!(path, arkdown_path::canonicalize_or_owned(&index));
            }
            _ => panic!("expected LocalSource"),
        }
    }

    #[test]
    fn test_relative_directory_parent_import_resolves_index() {
        let dir = TempDir::new().unwrap();
        let src_dir = dir.path().join("src/ets");
        fs::create_dir_all(src_dir.join("feature")).unwrap();
        let index = src_dir.join("index.ets");
        fs::write(&index, "export const value = 1;").unwrap();

        let resolver = make_resolver(
            dir.path().to_path_buf(),
            vec![],
            vec![],
            FxHashMap::default(),
        );
        let importer = src_dir.join("feature/main.ets");
        let result = resolver.resolve("..", &importer).unwrap();
        match result {
            OhosResolvedModule::LocalSource { path, .. } => {
                assert_eq!(path, arkdown_path::canonicalize_or_owned(&index));
            }
            _ => panic!("expected LocalSource"),
        }
    }

    // Test 7: bare specifier in oh_modules
    #[test]
    fn test_bare_specifier_oh_modules() {
        let dir = TempDir::new().unwrap();
        let oh_modules_dir = dir.path().join("oh_modules");
        let pkg_dir = oh_modules_dir.join("mylib");
        fs::create_dir_all(&pkg_dir).unwrap();
        // Write oh-package.json5
        fs::write(
            pkg_dir.join("oh-package.json5"),
            r#"{ "name": "mylib", "version": "1.2.3", "main": "index.ets" }"#,
        )
        .unwrap();
        // Write entry file
        fs::write(pkg_dir.join("index.ets"), "// mylib entry").unwrap();

        let resolver = make_resolver(
            dir.path().to_path_buf(),
            vec![],
            vec![],
            pkg_index("mylib", &pkg_dir, false),
        );
        let importer = dir.path().join("src/main.ets");
        let result = resolver.resolve("mylib", &importer).unwrap();
        match result {
            OhosResolvedModule::RemotePackage {
                pkg_name,
                pkg_version,
                ..
            } => {
                assert_eq!(pkg_name, "mylib");
                assert_eq!(pkg_version, "1.2.3");
            }
            _ => panic!("expected RemotePackage"),
        }
    }

    #[test]
    fn test_package_root_prefers_hvigor_esm_main_fields() {
        let dir = TempDir::new().unwrap();
        let pkg_dir = dir.path().join("oh_modules/dual-package");
        fs::create_dir_all(&pkg_dir).unwrap();
        fs::write(
            pkg_dir.join("oh-package.json5"),
            r#"{
              "name": "dual-package",
              "version": "1.0.0",
              "module": "index.esm.js",
              "main": "index.cjs.js"
            }"#,
        )
        .unwrap();
        let esm_entry = pkg_dir.join("index.esm.js");
        fs::write(&esm_entry, "export const value = 1;").unwrap();
        fs::write(
            pkg_dir.join("index.cjs.js"),
            "module.exports = { value: 1 };",
        )
        .unwrap();

        let resolver = make_resolver(
            dir.path().to_path_buf(),
            vec![],
            vec![],
            pkg_index("dual-package", &pkg_dir, false),
        );
        let importer = dir.path().join("src/main.ets");
        let result = resolver.resolve("dual-package", &importer).unwrap();

        match result {
            OhosResolvedModule::RemotePackage { path, .. } => {
                assert_eq!(path, arkdown_path::canonicalize_or_owned(&esm_entry));
            }
            _ => panic!("expected RemotePackage"),
        }
    }

    #[test]
    fn test_package_index_matches_case_insensitive_when_hvigor_case_check_disabled() {
        let dir = TempDir::new().unwrap();
        let pkg_dir = dir.path().join("oh_modules/@meituan/abtest");
        fs::create_dir_all(&pkg_dir).unwrap();
        fs::write(
            pkg_dir.join("oh-package.json5"),
            r#"{ "name": "@meituan/abtest", "version": "1.0.0", "main": "index.ets" }"#,
        )
        .unwrap();
        let entry = pkg_dir.join("index.ets");
        fs::write(&entry, "export const value = 1;").unwrap();

        let resolver = make_resolver(
            dir.path().to_path_buf(),
            vec![],
            vec![],
            pkg_index("@meituan/abtest", &pkg_dir, false),
        );
        let importer = dir.path().join("src/main.ets");
        let result = resolver.resolve("@meituan/abTest", &importer).unwrap();
        match result {
            OhosResolvedModule::RemotePackage { path, pkg_name, .. } => {
                assert_eq!(pkg_name, "@meituan/abTest");
                assert_eq!(path, arkdown_path::canonicalize_or_owned(&entry));
            }
            _ => panic!("expected RemotePackage"),
        }
    }

    #[test]
    fn test_bare_specifier_resolves_from_hvigor_module_paths() {
        let dir = TempDir::new().unwrap();
        let module_root = dir.path().join("entry");
        let pkg_dir = module_root.join("oh_modules/@ohos/hypium");
        fs::create_dir_all(&pkg_dir).unwrap();
        fs::write(
            pkg_dir.join("oh-package.json5"),
            r#"{ "name": "@ohos/hypium", "version": "1.0.11", "main": "index.js" }"#,
        )
        .unwrap();
        let entry = pkg_dir.join("index.js");
        fs::write(&entry, "export const describe = () => {};").unwrap();

        let resolver = OhosResolver::new(ResolverConfig {
            project_root: dir.path().to_path_buf(),
            sdk_api_dirs: vec![],
            sdk_kits_dirs: vec![],
            sdk_arkts_dirs: vec![],
            source_roots: vec![],
            module_paths: vec![module_root.join("oh_modules")],
            package_resolution: Arc::new(PackageResolutionPlan::default()),
            use_normalized_ohm_url: true,
            case_sensitive_check: false,
            source_set_overlays: vec![],
        });
        let importer = module_root.join("src/main.ets");
        let result = resolver.resolve("@ohos/hypium", &importer).unwrap();
        match result {
            OhosResolvedModule::RemotePackage {
                path,
                pkg_name,
                pkg_version,
                ..
            } => {
                assert_eq!(pkg_name, "@ohos/hypium");
                assert_eq!(pkg_version, "1.0.11");
                assert_eq!(path, arkdown_path::canonicalize_or_owned(&entry));
            }
            _ => panic!("expected RemotePackage"),
        }
    }

    #[test]
    fn hvigor_generated_profile_root_resolves_bare_build_profile_as_local_source() {
        let dir = TempDir::new().unwrap();
        let module_root = dir.path().join("entry");
        let source_root = module_root.join("src/main");
        let generated_profile_root = module_root.join("build/default/generated/profile/default");
        fs::create_dir_all(source_root.join("ets")).unwrap();
        fs::create_dir_all(&generated_profile_root).unwrap();
        let importer = source_root.join("ets/EntryAbility.ets");
        let generated_profile = generated_profile_root.join("BuildProfile.ets");
        fs::write(&importer, "import BuildProfile from 'BuildProfile';").unwrap();
        fs::write(&generated_profile, "export default class BuildProfile {}").unwrap();

        let resolver = OhosResolver::new(ResolverConfig {
            project_root: dir.path().to_path_buf(),
            sdk_api_dirs: vec![],
            sdk_kits_dirs: vec![],
            sdk_arkts_dirs: vec![],
            source_roots: vec![source_root, generated_profile_root],
            module_paths: vec![module_root.join("oh_modules")],
            package_resolution: Arc::new(PackageResolutionPlan::default()),
            use_normalized_ohm_url: true,
            case_sensitive_check: false,
            source_set_overlays: vec![],
        });

        let result = resolver.resolve("BuildProfile", &importer).unwrap();
        match result {
            OhosResolvedModule::LocalSource { path, .. } => {
                assert_eq!(
                    path,
                    arkdown_path::canonicalize_or_owned(&generated_profile)
                );
            }
            _ => panic!("expected generated BuildProfile to be a local source"),
        }
    }

    #[test]
    fn test_file_dependency_alias_subpath_resolves_like_hvigor_dep_info() {
        let dir = TempDir::new().unwrap();
        let app = dir.path().join("entry");
        let dep = dir.path().join("commons/network");
        fs::create_dir_all(&app).unwrap();
        fs::create_dir_all(&dep).unwrap();
        fs::write(
            app.join("oh-package.json5"),
            r#"{
              "name": "entry",
              "dependencies": { "@ohos/network": "file:../commons/network" }
            }"#,
        )
        .unwrap();
        fs::write(
            dep.join("oh-package.json5"),
            r#"{ "name": "network", "version": "1.0.0", "main": "Index.ets" }"#,
        )
        .unwrap();
        let request = dep.join("Request.ts");
        fs::write(&request, "export const ok = true;").unwrap();

        let resolver = make_resolver(
            dir.path().to_path_buf(),
            vec![],
            vec![],
            pkg_index("@ohos/network", &dep, true),
        );
        let importer = app.join("src/main/ets/Index.ets");
        let result = resolver
            .resolve("@ohos/network/Request", &importer)
            .unwrap();
        match result {
            OhosResolvedModule::LocalSource {
                path, pkg_context, ..
            } => {
                assert_eq!(path, arkdown_path::canonicalize_or_owned(&request));
                assert_eq!(
                    pkg_context.expect("pkg context").package_name,
                    "@ohos/network"
                );
            }
            _ => panic!("expected LocalSource"),
        }
    }

    #[test]
    fn test_ohpm_shared_symlink_farm_resolves_oh_package_main() {
        let dir = TempDir::new().unwrap();
        let oh_modules_dir = dir.path().join("oh_modules");
        let pkg_dir = oh_modules_dir.join(".ohpm/bignumber.js@9.0.1/oh_modules/bignumber.js");
        fs::create_dir_all(&pkg_dir).unwrap();
        fs::write(
            pkg_dir.join("oh-package.json5"),
            r#"{ "name": "bignumber.js", "version": "9.0.1", "main": "bignumber" }"#,
        )
        .unwrap();
        let entry = pkg_dir.join("bignumber.js");
        fs::write(&entry, "export default function BigNumber() {}").unwrap();

        let resolver = make_resolver(
            dir.path().to_path_buf(),
            vec![],
            vec![],
            pkg_index("bignumber.js", &pkg_dir, false),
        );
        let importer = dir.path().join("src/main.ets");
        let result = resolver.resolve("bignumber.js", &importer).unwrap();
        match result {
            OhosResolvedModule::RemotePackage {
                path,
                pkg_name,
                pkg_version,
                ..
            } => {
                assert_eq!(path, arkdown_path::canonicalize_or_owned(&entry));
                assert_eq!(pkg_name, "bignumber.js");
                assert_eq!(pkg_version, "9.0.1");
            }
            _ => panic!("expected RemotePackage"),
        }
    }

    #[test]
    fn test_esm_package_root_uses_exports_import_before_main() {
        let dir = TempDir::new().unwrap();
        let oh_modules_dir = dir.path().join("oh_modules");
        let pkg_dir = oh_modules_dir.join(".ohpm/long@5.2.1/oh_modules/long");
        fs::create_dir_all(pkg_dir.join("umd")).unwrap();
        fs::write(
            pkg_dir.join("oh-package.json5"),
            r#"{
              "name": "long",
              "version": "5.2.1",
              "type": "module",
              "main": "umd/index.js",
              "exports": {
                ".": {
                  "import": "./index.js",
                  "require": "./umd/index.js"
                }
              }
            }"#,
        )
        .unwrap();
        let esm_entry = pkg_dir.join("index.js");
        let cjs_entry = pkg_dir.join("umd/index.js");
        fs::write(&esm_entry, "export default class Long {}").unwrap();
        fs::write(&cjs_entry, "module.exports = function Long() {}").unwrap();

        let resolver = make_resolver(
            dir.path().to_path_buf(),
            vec![],
            vec![],
            pkg_index("long", &pkg_dir, false),
        );
        let importer = dir.path().join("src/main.ets");
        let result = resolver.resolve("long", &importer).unwrap();
        match result {
            OhosResolvedModule::RemotePackage { path, .. } => {
                assert_eq!(path, arkdown_path::canonicalize_or_owned(&esm_entry));
            }
            _ => panic!("expected RemotePackage"),
        }

        let result = resolver
            .resolve_with_kind("long", &importer, ResolveImportKind::Require)
            .unwrap();
        match result {
            OhosResolvedModule::RemotePackage { path, .. } => {
                assert_eq!(path, arkdown_path::canonicalize_or_owned(&cjs_entry));
            }
            _ => panic!("expected RemotePackage"),
        }
    }

    #[test]
    fn importer_local_oh_modules_resolves_symlink_dependency_absent_from_host_plan() {
        let dir = TempDir::new().unwrap();
        let oh_modules_dir = dir.path().join("oh_modules");
        let cloud_root = oh_modules_dir
            .join(".ohpm")
            .join("@hw-agconnect+cloud@yzqapoubp9vkiyjnrivxkgyixmfolam5fmb1z55bw2a=")
            .join("oh_modules")
            .join("@hw-agconnect")
            .join("cloud");
        let base64_root = oh_modules_dir
            .join(".ohpm")
            .join("@protobufjs+base64@clcadirmo9vkiyjnrivxkgyixmfolam5fmb1z55bw2a=")
            .join("oh_modules")
            .join("@protobufjs")
            .join("base64");
        fs::create_dir_all(cloud_root.join("src/main/ets/database/utils")).unwrap();
        fs::create_dir_all(&base64_root).unwrap();
        fs::write(
            cloud_root.join("oh-package.json5"),
            r#"{
              "name": "@hw-agconnect/cloud",
              "version": "1.0.2",
              "main": "index.ets",
              "dependencies": {
                "@protobufjs/base64": "1.1.2"
              }
            }"#,
        )
        .unwrap();
        fs::write(
            base64_root.join("oh-package.json5"),
            r#"{ "name": "@protobufjs/base64", "version": "1.1.2", "main": "index.js" }"#,
        )
        .unwrap();
        let base64_entry = base64_root.join("index.js");
        fs::write(&base64_entry, "export default {};").unwrap();

        let scoped_nested_root = cloud_root.join("oh_modules/@protobufjs");
        fs::create_dir_all(&scoped_nested_root).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&base64_root, scoped_nested_root.join("base64")).unwrap();
        #[cfg(windows)]
        junction::create(&base64_root, scoped_nested_root.join("base64")).unwrap();

        let resolver = make_resolver(
            dir.path().to_path_buf(),
            vec![],
            vec![],
            pkg_index("@hw-agconnect/cloud", &cloud_root, true),
        );
        let importer = cloud_root.join("src/main/ets/database/utils/Serializable.ts");
        fs::write(&importer, "import base64 from '@protobufjs/base64';").unwrap();
        let importer = arkdown_path::canonicalize_or_owned(&importer);
        let result = resolver.resolve("@protobufjs/base64", &importer).unwrap();
        match result {
            OhosResolvedModule::RemotePackage {
                path,
                pkg_name,
                pkg_version,
                ..
            } => {
                assert_eq!(path, arkdown_path::canonicalize_or_owned(&base64_entry));
                assert_eq!(pkg_name, "@protobufjs/base64");
                assert_eq!(pkg_version, "1.1.2");
            }
            _ => panic!("expected RemotePackage"),
        }
    }

    #[test]
    fn hvigor_self_last_package_plan_resolves_current_main_without_symlink_bounce() {
        let dir = TempDir::new().unwrap();
        let current_root = dir.path().join("msi_runtime");
        let installed_root = dir.path().join("oh_modules/@msi/runtime");
        fs::create_dir_all(current_root.join("src/main/ets/util")).unwrap();
        fs::create_dir_all(&installed_root).unwrap();
        fs::write(
            current_root.join("oh-package.json5"),
            r#"{ "name": "@msi/runtime", "version": "1.0.0", "main": "Index.ets" }"#,
        )
        .unwrap();
        fs::write(
            installed_root.join("oh-package.json5"),
            r#"{ "name": "@msi/runtime", "version": "0.9.0", "main": "Index.ets" }"#,
        )
        .unwrap();
        let current_entry = current_root.join("Index.ets");
        fs::write(&current_entry, "export const current = true;").unwrap();
        fs::write(
            installed_root.join("Index.ets"),
            "export const installed = true;",
        )
        .unwrap();
        let importer = current_root.join("src/main/ets/util/NetworkTypeUtil.ets");
        fs::write(&importer, "import { current } from '@msi/runtime';").unwrap();

        let mut builder = crate::plan::PackageResolutionPlanBuilder::new();
        builder.insert_dependency(
            "@msi/runtime",
            PkgBriefInfo {
                pkg_root: installed_root,
                pkg_name: "@msi/runtime".to_string(),
                manifest: Arc::new(PackageManifestSnapshot {
                    version: "0.9.0".to_string(),
                    main: Some("Index.ets".to_string()),
                    ..PackageManifestSnapshot::default()
                }),
                local_source: false,
                source_roots: vec![],
            },
        );
        builder.insert_current(PkgBriefInfo {
            pkg_root: current_root.clone(),
            pkg_name: "@msi/runtime".to_string(),
            manifest: Arc::new(PackageManifestSnapshot {
                version: "1.0.0".to_string(),
                main: Some("Index.ets".to_string()),
                ..PackageManifestSnapshot::default()
            }),
            local_source: true,
            source_roots: vec![current_root.join("src/main")],
        });
        let resolver = OhosResolver::new(ResolverConfig {
            project_root: dir.path().to_path_buf(),
            sdk_api_dirs: vec![],
            sdk_kits_dirs: vec![],
            sdk_arkts_dirs: vec![],
            source_roots: vec![],
            module_paths: vec![dir.path().join("oh_modules")],
            package_resolution: Arc::new(builder.build()),
            use_normalized_ohm_url: false,
            case_sensitive_check: false,
            source_set_overlays: vec![],
        });

        let result = resolver.resolve("@msi/runtime", &importer).unwrap();
        match result {
            OhosResolvedModule::LocalSource { path, .. } => {
                assert_eq!(path, arkdown_path::canonicalize_or_owned(&current_entry));
            }
            _ => panic!("expected current package local source"),
        }
    }

    #[test]
    fn hvigor_self_subpath_uses_source_roots_only_for_owning_package() {
        let dir = TempDir::new().unwrap();
        let package_root = dir.path().join("msi_runtime");
        let source_root = package_root.join("src/main");
        let importer = source_root.join("ets/util/NetworkTypeUtil.ets");
        let target = source_root.join("ets/util/GrayscaleUtils.ets");
        fs::create_dir_all(importer.parent().unwrap()).unwrap();
        fs::write(
            package_root.join("oh-package.json5"),
            r#"{ "name": "@msi/runtime", "main": "Index.ets" }"#,
        )
        .unwrap();
        fs::write(package_root.join("Index.ets"), "export {};").unwrap();
        fs::write(&importer, "export {};").unwrap();
        fs::write(&target, "export const grayscale = true;").unwrap();
        let importer = arkdown_path::canonicalize_or_owned(&importer);

        let mut builder = crate::plan::PackageResolutionPlanBuilder::new();
        builder.insert_current(PkgBriefInfo {
            pkg_root: package_root.clone(),
            pkg_name: "@msi/runtime".to_string(),
            manifest: Arc::new(PackageManifestSnapshot {
                main: Some("Index.ets".to_string()),
                ..PackageManifestSnapshot::default()
            }),
            local_source: true,
            source_roots: vec![source_root],
        });
        let resolver = OhosResolver::new(ResolverConfig {
            project_root: dir.path().to_path_buf(),
            sdk_api_dirs: vec![],
            sdk_kits_dirs: vec![],
            sdk_arkts_dirs: vec![],
            source_roots: vec![],
            module_paths: vec![],
            package_resolution: Arc::new(builder.build()),
            use_normalized_ohm_url: false,
            case_sensitive_check: false,
            source_set_overlays: vec![],
        });

        let result = resolver
            .resolve("@msi/runtime/ets/util/GrayscaleUtils", &importer)
            .unwrap();
        match result {
            OhosResolvedModule::LocalSource { path, .. } => {
                assert_eq!(path, arkdown_path::canonicalize_or_owned(&target));
            }
            _ => panic!("expected self source-root resolution"),
        }

        let outside_importer = dir.path().join("entry/src/main/ets/Index.ets");
        fs::create_dir_all(outside_importer.parent().unwrap()).unwrap();
        fs::write(&outside_importer, "export {};").unwrap();
        let outside_importer = arkdown_path::canonicalize_or_owned(&outside_importer);
        assert!(resolver
            .resolve("@msi/runtime/ets/util/GrayscaleUtils", &outside_importer,)
            .is_err());
    }

    // Test 8: @shared: → ExternalHsp
    #[test]
    fn test_external_hsp() {
        let dir = TempDir::new().unwrap();
        let resolver = make_resolver(
            dir.path().to_path_buf(),
            vec![],
            vec![],
            FxHashMap::default(),
        );
        let importer = dir.path().join("src/main.ets");
        let result = resolver
            .resolve("@shared:com.example/sharedlib", &importer)
            .unwrap();
        match result {
            OhosResolvedModule::ExternalHsp {
                bundle_name,
                module_name,
                ..
            } => {
                assert_eq!(bundle_name, "com.example");
                assert_eq!(module_name, "sharedlib");
            }
            _ => panic!("expected ExternalHsp"),
        }
    }

    /// Test 9: source-set overlay — target-specific file shadows src/main/ets equivalent.
    ///
    /// When a file `src/<target>/ets/Foo.ets` exists alongside `src/main/ets/Foo.ets`,
    /// a relative import that resolves to the main version should be redirected to the
    /// target-specific version.
    #[test]
    fn test_source_set_overlay_target_shadows_main() {
        let dir = TempDir::new().unwrap();
        let base_ets = dir.path().join("src/main/ets");
        let target_ets = dir.path().join("src/phone/ets");
        fs::create_dir_all(&base_ets).unwrap();
        fs::create_dir_all(&target_ets).unwrap();

        // Create both the base and target versions of the same file.
        fs::write(base_ets.join("Utils.ets"), "export const base = 1;").unwrap();
        fs::write(target_ets.join("Utils.ets"), "export const target = 1;").unwrap();

        // Create the importer in src/main/ets.
        let importer = base_ets.join("Index.ets");
        fs::write(&importer, "import { Utils } from './Utils';").unwrap();

        // Build resolver WITH overlay: target/ets should shadow main/ets.
        let resolver = OhosResolver::new(ResolverConfig {
            project_root: dir.path().to_path_buf(),
            sdk_api_dirs: vec![],
            sdk_kits_dirs: vec![],
            sdk_arkts_dirs: vec![],
            source_roots: vec![],
            module_paths: vec![],
            package_resolution: Arc::new(PackageResolutionPlan::default()),
            use_normalized_ohm_url: false,
            case_sensitive_check: false,
            source_set_overlays: vec![(base_ets.clone(), target_ets.clone())],
        });

        let result = resolver.resolve("./Utils", &importer).unwrap();
        match result {
            OhosResolvedModule::LocalSource { path, .. } => {
                let canonical = arkdown_path::canonicalize_or_owned(&path);
                let target_canonical =
                    arkdown_path::canonicalize_or_owned(&target_ets.join("Utils.ets"));
                assert_eq!(
                    canonical, target_canonical,
                    "source-set overlay must redirect to target-specific file; got: {:?}",
                    canonical
                );
            }
            _ => panic!("expected LocalSource"),
        }

        // Without overlay: should resolve to the base file.
        let resolver_no_overlay = OhosResolver::new(ResolverConfig {
            project_root: dir.path().to_path_buf(),
            sdk_api_dirs: vec![],
            sdk_kits_dirs: vec![],
            sdk_arkts_dirs: vec![],
            source_roots: vec![],
            module_paths: vec![],
            package_resolution: Arc::new(PackageResolutionPlan::default()),
            use_normalized_ohm_url: false,
            case_sensitive_check: false,
            source_set_overlays: vec![],
        });

        let result2 = resolver_no_overlay.resolve("./Utils", &importer).unwrap();
        match result2 {
            OhosResolvedModule::LocalSource { path, .. } => {
                let canonical = arkdown_path::canonicalize_or_owned(&path);
                let base_canonical =
                    arkdown_path::canonicalize_or_owned(&base_ets.join("Utils.ets"));
                assert_eq!(
                    canonical, base_canonical,
                    "without overlay, must resolve to base file; got: {:?}",
                    canonical
                );
            }
            _ => panic!("expected LocalSource"),
        }
    }
}
