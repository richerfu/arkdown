use std::collections::HashMap;
use std::path::{Path, PathBuf};

use arkdown_core::{ModuleType, OhosResolvedModule, PkgContext};

use crate::error::ResolveError;
use crate::oh_modules::{
    is_bytecode_har, read_bytecode_abc_path, read_declaration_entry, read_main_field,
    read_version_field,
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
}

impl OhosResolver {
    /// Create a new resolver with the given configuration.
    pub fn new(config: ResolverConfig) -> Self {
        OhosResolver { config }
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
        if let Some(resolved) = self.resolve_bare_package(specifier) {
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
            let importer_dir = importer.parent().unwrap_or(importer);
            let base = importer_dir.join(specifier);
            if let Some(resolved) = try_extensions(&base) {
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

    fn resolve_bare_package(&self, specifier: &str) -> Option<OhosResolvedModule> {
        if let Some(pkg_info) = self.config.pkg_name_to_pkg_info.get(specifier) {
            for src_root in &pkg_info.source_roots {
                let base = src_root.join("index");
                if let Some(resolved) = try_extensions(&base) {
                    let module_type = infer_module_type(&resolved);
                    return Some(OhosResolvedModule::LocalSource {
                        path: resolved,
                        module_type,
                        pkg_context: Some(PkgContext {
                            package_name: pkg_info.pkg_name.clone(),
                            bundle_name: String::new(),
                            module_name: String::new(),
                            version: String::new(),
                            pkg_path: pkg_info.pkg_root.clone(),
                            entry_path: None,
                        }),
                    });
                }
            }
        }

        for root in &self.config.oh_modules_roots {
            let pkg_dir = root.join(specifier);
            let oh_package_path = pkg_dir.join("oh-package.json5");
            if !oh_package_path.is_file() {
                continue;
            }

            let version = read_version_field(&oh_package_path);

            if is_bytecode_har(&oh_package_path) {
                let abc_path = read_bytecode_abc_path(&oh_package_path)
                    .unwrap_or_else(|| pkg_dir.join("ets/modules.abc"));
                let declaration_path = read_declaration_entry(&oh_package_path)
                    .unwrap_or_else(|| pkg_dir.join("index.d.ets"));
                return Some(OhosResolvedModule::BytecodeHar {
                    abc_path,
                    declaration_path,
                    pkg_name: specifier.to_string(),
                    ohm_url: format!("@bundle:{}", specifier),
                });
            }

            // Normal remote package: resolve entry via `main` field
            if let Some(main) = read_main_field(&oh_package_path) {
                let entry_base = pkg_dir.join(&main);
                if let Some(resolved) = try_extensions(&entry_base) {
                    let module_type = infer_module_type(&resolved);
                    return Some(OhosResolvedModule::RemotePackage {
                        path: resolved,
                        module_type,
                        pkg_name: specifier.to_string(),
                        pkg_version: version,
                        ohm_url: None,
                    });
                }
            }

            // Fallback: try index files in pkg_dir
            let index_base = pkg_dir.join("index");
            if let Some(resolved) = try_extensions(&index_base) {
                let module_type = infer_module_type(&resolved);
                return Some(OhosResolvedModule::RemotePackage {
                    path: resolved,
                    module_type,
                    pkg_name: specifier.to_string(),
                    pkg_version: version,
                    ohm_url: None,
                });
            }
        }

        None
    }
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
                assert_eq!(path, entry);
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
                assert_eq!(path, helper);
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
