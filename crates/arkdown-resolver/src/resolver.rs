use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use arkdown_core::{ModuleType, OhosResolvedModule, PkgContext};
use oxc_resolver::{
    EnforceExtension, ResolveOptions as OxcResolveOptions, Resolver as OxcResolver,
};

use crate::error::ResolveError;
use crate::oh_modules::{
    is_bytecode_har, read_bytecode_abc_path, read_declaration_entry, read_file_dependencies,
    read_name_field, read_package_exports_import_field, read_package_main_field,
    read_package_version_field,
};

/// Extensions tried in order during resolution.
const EXTENSIONS: &[&str] = &[".ets", ".ts", ".js", ".d.ets", ".d.ts"];

/// Configuration for the OHOS module resolver.
pub struct ResolverConfig {
    /// Absolute path to the project root directory.
    pub project_root: PathBuf,
    /// Paths to SDK `api/` directories (for `@ohos/*` specifiers).
    pub sdk_api_dirs: Vec<PathBuf>,
    /// Paths to SDK `kits/` directories (for `@kit.*` specifiers).
    pub sdk_kits_dirs: Vec<PathBuf>,
    /// Map from package name → `PkgBriefInfo` (from `loader.json` `pkgBriefInfo`).
    pub pkg_name_to_pkg_info: HashMap<String, PkgBriefInfo>,
    /// Whether to use normalized OHM URLs as module identifiers.
    pub use_normalized_ohm_url: bool,
    /// Whether to enforce case-sensitive import checks.
    pub case_sensitive_check: bool,
    /// Per-module `oh_modules/` roots and project-root `oh_modules/`.
    pub oh_modules_roots: Vec<PathBuf>,
    /// Module source roots (for relative resolution fallback).
    pub module_src_roots: Vec<PathBuf>,
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
    package_roots: HashMap<String, PackageRoot>,
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
        let fs_resolver = OxcResolver::new(oxc_resolve_options(&config));
        let package_roots = build_package_roots(&config);
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
                let resolved = canonicalize_existing(resolution.into_path_buf());
                let module_type = infer_module_type(&resolved);
                return Ok(OhosResolvedModule::LocalSource {
                    path: resolved,
                    module_type,
                    pkg_context: None,
                });
            }

            let importer_dir = importer.parent().unwrap_or(importer);
            if let Some(resolved) = try_extensions(&importer_dir.join(specifier)) {
                let resolved = canonicalize_existing(resolved);
                let module_type = infer_module_type(&resolved);
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

        for candidate in package_candidates(specifier) {
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
                let resolved = canonicalize_existing(resolution.into_path_buf());
                let module_type = infer_module_type(&resolved);
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
        let oh_package_path = pkg_root.path.join("oh-package.json5");
        if oh_package_path.is_file() && is_bytecode_har(&oh_package_path) {
            let abc_path = read_bytecode_abc_path(&oh_package_path)
                .unwrap_or_else(|| pkg_root.path.join("ets/modules.abc"));
            let declaration_path = read_declaration_entry(&oh_package_path)
                .unwrap_or_else(|| pkg_root.path.join("index.d.ets"));
            return Some(OhosResolvedModule::BytecodeHar {
                abc_path,
                declaration_path,
                pkg_name: package_name.to_string(),
                ohm_url: format!("@bundle:{}", package_name),
            });
        }

        let requested = if subpath.is_empty() {
            read_package_exports_import_field(&pkg_root.path)
                .or_else(|| read_package_main_field(&pkg_root.path))
                .unwrap_or_else(|| "index".to_string())
        } else {
            subpath.to_string()
        };
        let resolved = self
            .resolve_package_path(&pkg_root.path, &requested)
            .or_else(|| self.resolve_package_path(&pkg_root.path, "index"))?;
        let module_type = infer_module_type(&resolved);

        Some(remote_or_local_package_module(
            resolved,
            module_type,
            package_name,
            pkg_root,
        ))
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
            .map(|resolution| canonicalize_existing(resolution.into_path_buf()))
            .or_else(|| {
                let request = request.trim_start_matches("./");
                try_extensions(&package_root.join(request)).map(canonicalize_existing)
            })
    }
}

fn remote_or_local_package_module(
    path: PathBuf,
    module_type: ModuleType,
    package_name: &str,
    pkg_root: &PackageRoot,
) -> OhosResolvedModule {
    if pkg_root.local_source {
        return OhosResolvedModule::LocalSource {
            path,
            module_type,
            pkg_context: Some(PkgContext {
                package_name: pkg_root.package_name.clone(),
                bundle_name: String::new(),
                module_name: String::new(),
                version: read_package_version_field(&pkg_root.path),
                pkg_path: pkg_root.path.clone(),
                entry_path: None,
            }),
        };
    }

    OhosResolvedModule::RemotePackage {
        path,
        module_type,
        pkg_name: package_name.to_string(),
        pkg_version: read_package_version_field(&pkg_root.path),
        ohm_url: None,
    }
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
        ..OxcResolveOptions::default()
    }
}

fn build_package_roots(config: &ResolverConfig) -> HashMap<String, PackageRoot> {
    let mut roots = HashMap::new();

    for (name, info) in &config.pkg_name_to_pkg_info {
        roots.insert(
            name.clone(),
            PackageRoot {
                path: canonicalize_existing(info.pkg_root.clone()),
                package_name: info.pkg_name.clone(),
                local_source: true,
            },
        );
    }

    collect_project_file_dependencies(&config.project_root, &mut roots);

    for root in &config.oh_modules_roots {
        collect_oh_modules_package_roots(root, &mut roots);
    }

    roots
}

fn collect_project_file_dependencies(
    project_root: &Path,
    roots: &mut HashMap<String, PackageRoot>,
) {
    let mut visited = HashSet::new();
    collect_project_file_dependencies_inner(project_root, roots, &mut visited);
}

fn collect_project_file_dependencies_inner(
    dir: &Path,
    roots: &mut HashMap<String, PackageRoot>,
    visited: &mut HashSet<PathBuf>,
) {
    let Ok(real_dir) = std::fs::canonicalize(dir) else {
        return;
    };
    if !visited.insert(real_dir.clone()) {
        return;
    }

    let package_path = dir.join("oh-package.json5");
    if package_path.is_file() {
        for (name, path) in read_file_dependencies(&package_path) {
            let real_path = canonicalize_existing(path);
            roots.entry(name.clone()).or_insert(PackageRoot {
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
            ".git" | "oh_modules" | "node_modules" | "build" | "target"
        ) {
            continue;
        }
        if path.is_dir() {
            collect_project_file_dependencies_inner(&path, roots, visited);
        }
    }
}

fn collect_oh_modules_package_roots(root: &Path, roots: &mut HashMap<String, PackageRoot>) {
    collect_package_manifest_roots(root, roots, false, 0, 8);
    collect_package_manifest_roots(&root.join(".ohpm"), roots, false, 0, 6);
}

fn collect_package_manifest_roots(
    dir: &Path,
    roots: &mut HashMap<String, PackageRoot>,
    local_source: bool,
    depth: usize,
    max_depth: usize,
) {
    if depth > max_depth || !dir.is_dir() {
        return;
    }

    let package_path = dir.join("oh-package.json5");
    if package_path.is_file() {
        if let Some(package_name) = read_name_field(&package_path) {
            roots.entry(package_name.clone()).or_insert(PackageRoot {
                path: canonicalize_existing(dir.to_path_buf()),
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
            collect_package_manifest_roots(&path, roots, local_source, depth + 1, max_depth);
        }
    }
}

fn package_root_in_oh_modules(root: &Path, package_name: &str) -> Option<PathBuf> {
    let candidates = [
        root.join(package_name),
        root.join(".ohpm/oh_modules").join(package_name),
    ];
    candidates
        .into_iter()
        .find(|path| path.join("oh-package.json5").is_file() || path.join("package.json").is_file())
        .map(canonicalize_existing)
}

#[derive(Debug)]
struct PackageCandidate {
    package_name: String,
    subpath: String,
}

fn package_candidates(specifier: &str) -> Vec<PackageCandidate> {
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
        candidates.push(PackageCandidate {
            package_name: parts[..end].join("/"),
            subpath: parts[end..].join("/"),
        });
    }
    candidates
}

fn canonicalize_existing(path: PathBuf) -> PathBuf {
    std::fs::canonicalize(&path).unwrap_or(path)
}

/// Try to resolve a path by trying the exact path, then each extension, then index variants.
pub fn try_extensions(base: &Path) -> Option<PathBuf> {
    // 1. Exact path as given (must be a file, not a directory)
    if base.is_file() {
        return Some(base.to_path_buf());
    }
    // 2. With each extension appended
    for ext in EXTENSIONS {
        let ext_clean = ext.trim_start_matches('.');
        // build a path like `base.ets` — don't replace any existing extension
        let mut s = base.to_string_lossy().into_owned();
        s.push_str(ext);
        let p = PathBuf::from(s);
        if p.is_file() {
            return Some(p);
        }
        // Also try replacing existing extension (handles `foo.ts` → `foo.ets`)
        let p2 = base.with_extension(ext_clean);
        if p2.is_file() {
            return Some(p2);
        }
    }
    // 3. As a directory with an index file
    for ext in &[".ets", ".ts", ".js"] {
        let p = base.join(format!("index{}", ext));
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// Infer the [`ModuleType`] from a file extension.
pub fn infer_module_type(path: &Path) -> ModuleType {
    match path.extension().and_then(|e| e.to_str()) {
        Some("ets") => ModuleType::Ets,
        Some("ts") => ModuleType::Ts,
        Some("js") | Some("mjs") | Some("cjs") => ModuleType::Js,
        _ => ModuleType::Js,
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
            pkg_name_to_pkg_info: HashMap::new(),
            use_normalized_ohm_url: true,
            case_sensitive_check: false,
            oh_modules_roots,
            module_src_roots: vec![],
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

    // Test 5: try_extensions — .ets tried first
    #[test]
    fn test_try_extensions_ets_first() {
        let dir = TempDir::new().unwrap();
        let ets_file = dir.path().join("MyComp.ets");
        let ts_file = dir.path().join("MyComp.ts");
        // Create both; .ets should be found first
        fs::write(&ets_file, "// ets").unwrap();
        fs::write(&ts_file, "// ts").unwrap();

        let base = dir.path().join("MyComp");
        let resolved = try_extensions(&base).unwrap();
        assert_eq!(resolved, ets_file);
    }

    // Test 5b: try_extensions — falls back to .ts when .ets absent
    #[test]
    fn test_try_extensions_ts_fallback() {
        let dir = TempDir::new().unwrap();
        let ts_file = dir.path().join("utils.ts");
        fs::write(&ts_file, "// ts").unwrap();

        let base = dir.path().join("utils");
        let resolved = try_extensions(&base).unwrap();
        assert_eq!(resolved, ts_file);
    }

    // Test 5c: try_extensions — index.ets via directory fallback
    #[test]
    fn test_try_extensions_index_fallback() {
        let dir = TempDir::new().unwrap();
        let pkg_dir = dir.path().join("mypkg");
        fs::create_dir_all(&pkg_dir).unwrap();
        let index_file = pkg_dir.join("index.ets");
        fs::write(&index_file, "// index").unwrap();

        let base = pkg_dir.clone();
        let resolved = try_extensions(&base).unwrap();
        assert_eq!(resolved, index_file);
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
}
