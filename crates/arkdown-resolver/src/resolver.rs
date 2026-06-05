use rustc_hash::{FxHashMap, FxHashSet};
use std::path::{Path, PathBuf};

use arkdown_core::{ModuleType, OhosResolvedModule, PkgContext};
use oxc_resolver::{
    EnforceExtension, ResolveOptions as OxcResolveOptions, Resolver as OxcResolver,
    TsconfigDiscovery, TsconfigOptions, TsconfigReferences,
};

use crate::error::ResolveError;
use crate::oh_modules::{package_root_in_oh_modules, OhPackage};

/// Configuration for the OHOS module resolver.
pub struct ResolverConfig {
    /// Absolute path to the project root directory.
    pub project_root: PathBuf,
    /// Paths to SDK `api/` directories (for `@ohos/*` specifiers).
    pub sdk_api_dirs: Vec<PathBuf>,
    /// Paths to SDK `kits/` directories (for `@kit.*` specifiers).
    pub sdk_kits_dirs: Vec<PathBuf>,
    /// Map from package name → `PkgBriefInfo` (from `loader.json` `pkgBriefInfo`).
    pub pkg_name_to_pkg_info: FxHashMap<String, PkgBriefInfo>,
    /// Whether to use normalized OHM URLs as module identifiers.
    pub use_normalized_ohm_url: bool,
    /// Whether to enforce case-sensitive import checks.
    pub case_sensitive_check: bool,
    /// Per-module `oh_modules/` roots and project-root `oh_modules/`.
    pub oh_modules_roots: Vec<PathBuf>,
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

/// Brief package information extracted from `loader.json`.
#[derive(Debug, Clone)]
pub struct PkgBriefInfo {
    /// Absolute path to the package root directory.
    pub pkg_root: PathBuf,
    /// The package name (e.g. `"@ohos/axios"`).
    pub pkg_name: String,
    /// Source root directories within this package.
    pub source_roots: Vec<PathBuf>,
}

/// OHOS module resolver.
///
/// Maps import specifiers to [`OhosResolvedModule`] values following OHOS-specific
/// resolution rules (OHM URLs, SDK APIs, `oh_modules/`, relative imports).
pub struct OhosResolver {
    config: ResolverConfig,
    fs_resolver: OxcResolver,
    package_roots: FxHashMap<String, PackageRoot>,
}

#[derive(Debug, Clone)]
struct PackageRoot {
    path: PathBuf,
    package_name: String,
    local_source: bool,
}

impl OhosResolver {
    /// Create a new resolver with the given configuration.
    pub fn new(config: ResolverConfig) -> Self {
        let fs_resolver = OxcResolver::new(Self::oxc_resolve_options(&config));
        let package_roots = PackageRootIndex::build(&config).into_roots();
        OhosResolver {
            config,
            fs_resolver,
            package_roots,
        }
    }

    /// Resolve `specifier` imported from `importer`.
    ///
    /// Resolution order:
    /// 1. `@bundle:` / `@shared:` → external bundle / HSP reference
    /// 2. Bare package names, including scoped `@ohos/*` packages, from
    ///    `pkg_name_to_pkg_info` / `oh_modules/`
    /// 3. `@ohos/` → SDK API declaration
    /// 4. `@kit.` → SDK kit declaration
    /// 5. Relative (`./`, `../`) → local source file
    pub fn resolve(
        &self,
        specifier: &str,
        importer: &Path,
    ) -> Result<OhosResolvedModule, ResolveError> {
        // 1. External cross-bundle / HSP references
        if specifier.starts_with("@bundle:") {
            return Ok(OhosResolvedModule::ExternalBundle {
                ohm_url: specifier.to_string(),
            });
        }

        if let Some(rest) = specifier.strip_prefix("@shared:") {
            // Format: @shared:<bundleName>/<moduleName>
            let slash = rest.find('/').ok_or_else(|| ResolveError::NotFound {
                specifier: specifier.to_string(),
                importer: importer.to_string_lossy().into_owned(),
            })?;
            let bundle_name = rest[..slash].to_string();
            let module_name = rest[slash + 1..].to_string();
            return Ok(OhosResolvedModule::ExternalHsp {
                bundle_name,
                module_name,
                entry_path: String::new(),
            });
        }

        // 2. Bare packages take precedence over SDK declarations. OHOS package
        // names often use the `@ohos/*` scope; hvigor resolves those through
        // dependency metadata/oh_modules before falling back to SDK APIs.
        if let Some(resolved) = self.resolve_bare_package(specifier, importer) {
            return Ok(resolved);
        }

        // 3. @ohos/* → SDK API declaration
        if let Some(api_name) = specifier.strip_prefix("@ohos/") {
            let filename = format!("@ohos.{}.d.ts", api_name);
            for api_dir in &self.config.sdk_api_dirs {
                let candidate = api_dir.join(&filename);
                if candidate.is_file() {
                    return Ok(OhosResolvedModule::SdkDeclaration {
                        path: candidate,
                        specifier: specifier.to_string(),
                    });
                }
            }
            return Err(ResolveError::SdkModuleNotFound {
                specifier: specifier.to_string(),
            });
        }

        // 4. @kit.* → SDK kit declaration
        if specifier.starts_with("@kit.") {
            let filename = format!("{}.d.ts", specifier);
            for kits_dir in &self.config.sdk_kits_dirs {
                let candidate = kits_dir.join(&filename);
                if candidate.is_file() {
                    return Ok(OhosResolvedModule::SdkDeclaration {
                        path: candidate,
                        specifier: specifier.to_string(),
                    });
                }
            }
            return Err(ResolveError::SdkModuleNotFound {
                specifier: specifier.to_string(),
            });
        }

        // 5. Relative imports
        if specifier.starts_with("./") || specifier.starts_with("../") {
            if let Ok(resolution) = self.fs_resolver.resolve_file(importer, specifier) {
                let resolved_buf = resolution.into_path_buf();
                let resolved = arkdown_utils::canonicalize_or_owned(&resolved_buf);

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
                specifier: specifier.to_string(),
                importer: importer.to_string_lossy().into_owned(),
            });
        }

        Err(ResolveError::NotFound {
            specifier: specifier.to_string(),
            importer: importer.to_string_lossy().into_owned(),
        })
    }

    fn resolve_bare_package(&self, specifier: &str, importer: &Path) -> Option<OhosResolvedModule> {
        if specifier.starts_with("./") || specifier.starts_with("../") || specifier.starts_with('/')
        {
            return None;
        }

        for candidate in PackageCandidate::for_specifier(specifier) {
            if let Some(pkg_root) = self.package_root_for(&candidate.package_name, importer) {
                if let Some(resolved) = self.resolve_package_entry(
                    &candidate.package_name,
                    &candidate.subpath,
                    &pkg_root,
                ) {
                    return Some(resolved);
                }
            }
        }

        let importer_dir = importer.parent().unwrap_or(importer);
        self.fs_resolver
            .resolve(importer_dir, specifier)
            .ok()
            .map(|resolution| {
                let resolved_buf = resolution.into_path_buf();
                let resolved = arkdown_utils::canonicalize_or_owned(&resolved_buf);
                let module_type = ModuleType::infer(&resolved);
                OhosResolvedModule::RemotePackage {
                    path: resolved,
                    module_type,
                    pkg_name: specifier.to_string(),
                    pkg_version: String::new(),
                    ohm_url: None,
                }
            })
    }

    fn package_root_for(&self, package_name: &str, importer: &Path) -> Option<PackageRoot> {
        if let Some(root) = self.package_roots.get(package_name) {
            return Some(root.clone());
        }

        let mut current = importer.parent().unwrap_or(importer);
        loop {
            let root = current.join("oh_modules");
            if let Some(path) = package_root_in_oh_modules(&root, package_name) {
                return Some(PackageRoot {
                    path,
                    package_name: package_name.to_string(),
                    local_source: false,
                });
            }
            if current == self.config.project_root {
                break;
            }
            let Some(parent) = current.parent() else {
                break;
            };
            current = parent;
        }

        None
    }

    fn resolve_package_entry(
        &self,
        package_name: &str,
        subpath: &str,
        pkg_root: &PackageRoot,
    ) -> Option<OhosResolvedModule> {
        let pkg = OhPackage::load_from_root(&pkg_root.path);

        if let Some(ref pkg) = pkg {
            if pkg.is_bytecode_har() {
                let abc_path = pkg
                    .bytecode_abc_path()
                    .unwrap_or_else(|| pkg_root.path.join("ets/modules.abc"));
                let declaration_path = pkg
                    .declaration_entry()
                    .unwrap_or_else(|| pkg_root.path.join("index.d.ets"));
                return Some(OhosResolvedModule::BytecodeHar {
                    abc_path,
                    declaration_path,
                    pkg_name: package_name.to_string(),
                    ohm_url: format!("@bundle:{}", package_name),
                });
            }
        }

        let requested = if subpath.is_empty() {
            pkg.as_ref()
                .and_then(|p| p.exports_import_field())
                .or_else(|| {
                    pkg.as_ref()
                        .and_then(|p| p.main_field().map(str::to_string))
                })
                .unwrap_or_else(|| "index".to_string())
        } else {
            subpath.to_string()
        };
        let resolved = self
            .resolve_package_path(&pkg_root.path, &requested)
            .or_else(|| self.resolve_package_path(&pkg_root.path, "index"))?;
        let module_type = ModuleType::infer(&resolved);

        Some(pkg_root.to_resolved_module(resolved, module_type, package_name))
    }

    fn resolve_package_path(&self, package_root: &Path, request: &str) -> Option<PathBuf> {
        let request = if request.starts_with("./") || request.starts_with("../") {
            request.to_string()
        } else {
            format!("./{}", request)
        };
        self.fs_resolver
            .resolve(package_root, &request)
            .ok()
            .map(|resolution| {
                let buf = resolution.into_path_buf();
                arkdown_utils::canonicalize_or_owned(&buf)
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
            let canonical_base = arkdown_utils::canonicalize_or_owned(base_root);
            if let Ok(relative) = resolved.strip_prefix(&canonical_base) {
                let candidate = target_root.join(relative);
                if candidate.is_file() {
                    return arkdown_utils::canonicalize_or_owned(&candidate);
                }
            }
        }
        resolved
    }

    fn oxc_resolve_options(config: &ResolverConfig) -> OxcResolveOptions {
        let mut modules: Vec<String> = Vec::new();
        for root in &config.oh_modules_roots {
            modules.push(root.to_string_lossy().into_owned());
            modules.push(root.join(".ohpm/oh_modules").to_string_lossy().into_owned());
        }
        if modules.is_empty() {
            modules.push("oh_modules".to_string());
        }

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
            condition_names: vec!["import".to_string()],
            main_fields: vec!["main".to_string(), "module".to_string()],
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
                    version: OhPackage::load_from_root(&self.path)
                        .map(|p| p.version().to_string())
                        .unwrap_or_default(),
                    pkg_path: self.path.clone(),
                    entry_path: None,
                }),
            };
        }

        OhosResolvedModule::RemotePackage {
            path,
            module_type,
            pkg_name: package_name.to_string(),
            pkg_version: OhPackage::load_from_root(&self.path)
                .map(|p| p.version().to_string())
                .unwrap_or_default(),
            ohm_url: None,
        }
    }
}

struct PackageRootIndex {
    roots: FxHashMap<String, PackageRoot>,
}

impl PackageRootIndex {
    fn build(config: &ResolverConfig) -> Self {
        let mut index = Self {
            roots: FxHashMap::default(),
        };

        for (name, info) in &config.pkg_name_to_pkg_info {
            index.roots.insert(
                name.clone(),
                PackageRoot {
                    path: arkdown_utils::canonicalize_or_owned(&info.pkg_root),
                    package_name: info.pkg_name.clone(),
                    local_source: true,
                },
            );
        }

        index.collect_project_file_dependencies(&config.project_root);

        for root in &config.oh_modules_roots {
            index.collect_oh_modules_package_roots(root);
        }

        index
    }

    fn into_roots(self) -> FxHashMap<String, PackageRoot> {
        self.roots
    }

    fn collect_project_file_dependencies(&mut self, project_root: &Path) {
        let mut visited = FxHashSet::default();
        self.collect_project_file_dependencies_inner(project_root, &mut visited);
    }

    fn collect_project_file_dependencies_inner(
        &mut self,
        dir: &Path,
        visited: &mut FxHashSet<PathBuf>,
    ) {
        let Ok(real_dir) = std::fs::canonicalize(dir) else {
            return;
        };
        if !visited.insert(real_dir.clone()) {
            return;
        }

        if let Some(pkg) = OhPackage::load_from_root(dir) {
            for (name, path) in pkg.file_dependencies() {
                let real_path = arkdown_utils::canonicalize_or_owned(&path);
                self.roots.entry(name.clone()).or_insert(PackageRoot {
                    path: real_path,
                    package_name: name,
                    local_source: true,
                });
            }
        }

        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if matches!(
                name.as_ref(),
                ".git" | ".hvigor" | "oh_modules" | "node_modules" | "build" | "target"
            ) {
                continue;
            }
            if path.is_dir() {
                self.collect_project_file_dependencies_inner(&path, visited);
            }
        }
    }

    fn collect_oh_modules_package_roots(&mut self, root: &Path) {
        self.collect_package_manifest_roots(root, false, 0, 8);
        self.collect_package_manifest_roots(&root.join(".ohpm"), false, 0, 6);
    }

    fn collect_package_manifest_roots(
        &mut self,
        dir: &Path,
        local_source: bool,
        depth: usize,
        max_depth: usize,
    ) {
        if depth > max_depth || !dir.is_dir() {
            return;
        }

        if let Some(pkg) = OhPackage::load_from_root(dir) {
            if let Some(package_name) = pkg.name().map(str::to_string) {
                self.roots
                    .entry(package_name.clone())
                    .or_insert(PackageRoot {
                        path: arkdown_utils::canonicalize_or_owned(dir),
                        package_name,
                        local_source,
                    });
            }
        }

        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                self.collect_package_manifest_roots(&path, local_source, depth + 1, max_depth);
            }
        }
    }
}

#[derive(Debug)]
struct PackageCandidate {
    package_name: String,
    subpath: String,
}

impl PackageCandidate {
    fn for_specifier(specifier: &str) -> Vec<Self> {
        if specifier.starts_with("./")
            || specifier.starts_with("../")
            || specifier.starts_with('/')
            || specifier.starts_with('@') && !specifier.contains('/')
        {
            return Vec::new();
        }

        let parts: Vec<&str> = specifier.split('/').collect();
        let min_parts = if specifier.starts_with('@') { 2 } else { 1 };
        if parts.len() < min_parts {
            return Vec::new();
        }

        let mut candidates = Vec::new();
        for end in (min_parts..=parts.len()).rev() {
            candidates.push(Self {
                package_name: parts[..end].join("/"),
                subpath: parts[end..].join("/"),
            });
        }
        candidates
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_resolver(
        project_root: PathBuf,
        sdk_api_dirs: Vec<PathBuf>,
        sdk_kits_dirs: Vec<PathBuf>,
        oh_modules_roots: Vec<PathBuf>,
    ) -> OhosResolver {
        OhosResolver::new(ResolverConfig {
            project_root,
            sdk_api_dirs,
            sdk_kits_dirs,
            pkg_name_to_pkg_info: FxHashMap::default(),
            use_normalized_ohm_url: true,
            case_sensitive_check: false,
            oh_modules_roots,
            source_set_overlays: vec![],
        })
    }

    // Test 1: @bundle: → ExternalBundle
    #[test]
    fn test_external_bundle() {
        let dir = TempDir::new().unwrap();
        let resolver = make_resolver(dir.path().to_path_buf(), vec![], vec![], vec![]);
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

        let resolver = make_resolver(dir.path().to_path_buf(), vec![sdk_api_dir], vec![], vec![]);
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
            vec![dir.path().join("oh_modules")],
        );
        let importer = dir.path().join("src/main.ets");
        let result = resolver.resolve("@ohos/utils", &importer).unwrap();
        match result {
            OhosResolvedModule::RemotePackage { path, pkg_name, .. } => {
                assert_eq!(pkg_name, "@ohos/utils");
                assert_eq!(path, std::fs::canonicalize(&entry).unwrap());
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

        let resolver = make_resolver(dir.path().to_path_buf(), vec![], vec![kits_dir], vec![]);
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

    // Test 4: parse_ohm_url round-trip
    #[test]
    fn test_parse_ohm_url_via_resolver() {
        use crate::ohm_url::parse_ohm_url;
        let url = "@bundle:com.example/entry/ets/pages/Index";
        let (bundle, module, path) = parse_ohm_url(url).unwrap();
        assert_eq!(bundle, "com.example");
        assert_eq!(module, "entry");
        assert_eq!(path, "ets/pages/Index");
    }

    // Test 6: relative import resolution
    #[test]
    fn test_relative_import() {
        let dir = TempDir::new().unwrap();
        let src_dir = dir.path().join("src/ets");
        fs::create_dir_all(&src_dir).unwrap();
        let helper = src_dir.join("helper.ets");
        fs::write(&helper, "// helper").unwrap();

        let resolver = make_resolver(dir.path().to_path_buf(), vec![], vec![], vec![]);
        let importer = src_dir.join("main.ets");
        let result = resolver.resolve("./helper", &importer).unwrap();
        match result {
            OhosResolvedModule::LocalSource {
                path, module_type, ..
            } => {
                assert_eq!(path, std::fs::canonicalize(&helper).unwrap());
                assert_eq!(module_type, ModuleType::Ets);
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
            vec![oh_modules_dir],
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

        let resolver = make_resolver(dir.path().to_path_buf(), vec![], vec![], vec![]);
        let importer = app.join("src/main/ets/Index.ets");
        let result = resolver
            .resolve("@ohos/network/Request", &importer)
            .unwrap();
        match result {
            OhosResolvedModule::LocalSource {
                path, pkg_context, ..
            } => {
                assert_eq!(path, std::fs::canonicalize(&request).unwrap());
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
            vec![oh_modules_dir],
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
                assert_eq!(path, std::fs::canonicalize(&entry).unwrap());
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
            vec![oh_modules_dir],
        );
        let importer = dir.path().join("src/main.ets");
        let result = resolver.resolve("long", &importer).unwrap();
        match result {
            OhosResolvedModule::RemotePackage { path, .. } => {
                assert_eq!(path, std::fs::canonicalize(&esm_entry).unwrap());
            }
            _ => panic!("expected RemotePackage"),
        }
    }

    // Test 8: @shared: → ExternalHsp
    #[test]
    fn test_external_hsp() {
        let dir = TempDir::new().unwrap();
        let resolver = make_resolver(dir.path().to_path_buf(), vec![], vec![], vec![]);
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
            pkg_name_to_pkg_info: FxHashMap::default(),
            use_normalized_ohm_url: false,
            case_sensitive_check: false,
            oh_modules_roots: vec![],
            source_set_overlays: vec![(base_ets.clone(), target_ets.clone())],
        });

        let result = resolver.resolve("./Utils", &importer).unwrap();
        match result {
            OhosResolvedModule::LocalSource { path, .. } => {
                let canonical = std::fs::canonicalize(&path).unwrap_or(path.clone());
                let target_canonical =
                    std::fs::canonicalize(&target_ets.join("Utils.ets")).unwrap();
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
            pkg_name_to_pkg_info: FxHashMap::default(),
            use_normalized_ohm_url: false,
            case_sensitive_check: false,
            oh_modules_roots: vec![],
            source_set_overlays: vec![],
        });

        let result2 = resolver_no_overlay.resolve("./Utils", &importer).unwrap();
        match result2 {
            OhosResolvedModule::LocalSource { path, .. } => {
                let canonical = std::fs::canonicalize(&path).unwrap_or(path.clone());
                let base_canonical =
                    std::fs::canonicalize(&base_ets.join("Utils.ets")).unwrap();
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
