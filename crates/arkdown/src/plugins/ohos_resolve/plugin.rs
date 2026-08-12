use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arkdown_core::OhosResolvedModule;
use arkdown_path::{ArtifactPathSerializer, ModuleSpecifier, SpecifierKind};
use arkdown_resolver::{OhosResolver, ResolveImportKind, ResolverConfig};
use rolldown_common::{ImportKind, ResolvedExternal};
use rolldown_plugin::SharedLoadPluginContext;
use rolldown_plugin::{
    HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
    HookResolveIdReturn, HookUsage, Plugin, PluginContext,
};

const HVIGOR_IGNORE_PREFIX: &str = "\0hvigor_ignore_";

/// Rolldown `resolve_id` plugin backed by [`OhosResolver`].
///
/// Handles:
/// - `@ohos:` / `@hms:` / `@system.` / `@native:` — externalized immediately
///   without touching the resolver (colon-form builtins).
/// - `@bundle:` / `@shared:` — forwarded to the resolver and returned as external.
/// - `@ohos/` / `@kit.` SDK declarations — resolved to declaration file paths
///   and externalized so rolldown does not try to bundle `.d.ts` files.
/// - Bare package specifiers — resolved through `oh_modules/` / `pkg_name_to_pkg_info`.
/// - Relative imports (`./`, `../`) — resolved through the filesystem resolver.
/// - Absolute paths — passed through unchanged (`None` return lets rolldown handle them).
///
/// Runtime OHM URLs are generated after bundling, when hvigor rewrites module
/// references for the CompileArkTS handoff. Source modules participating in the
/// Rolldown graph stay identified by filesystem paths so Rolldown can load
/// them directly.
pub struct OhosResolvePlugin {
    resolver: Arc<OhosResolver>,
    project_root: PathBuf,
    /// Mirror of [`ResolverConfig::use_normalized_ohm_url`].
    use_normalized_ohm_url: bool,
    #[allow(dead_code)]
    case_sensitive: bool,
}

impl std::fmt::Debug for OhosResolvePlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OhosResolvePlugin")
            .field("project_root", &self.project_root)
            .field("use_normalized_ohm_url", &self.use_normalized_ohm_url)
            .finish_non_exhaustive()
    }
}

impl OhosResolvePlugin {
    /// Construct a plugin from a [`ResolverConfig`].
    ///
    /// Reads `use_normalized_ohm_url` and `case_sensitive_check` from the
    /// config before handing ownership to [`OhosResolver::new`].
    pub fn new(
        resolver_config: ResolverConfig,
        project_root: PathBuf,
        case_sensitive: bool,
    ) -> Self {
        let use_normalized_ohm_url = resolver_config.use_normalized_ohm_url;
        Self::with_resolver(
            Arc::new(OhosResolver::new(resolver_config)),
            project_root,
            use_normalized_ohm_url,
            case_sensitive,
        )
    }

    /// Construct a plugin from a pre-built [`OhosResolver`] (useful in tests or
    /// when the resolver is shared with another component).
    pub fn with_resolver(
        resolver: Arc<OhosResolver>,
        project_root: PathBuf,
        use_normalized_ohm_url: bool,
        case_sensitive: bool,
    ) -> Self {
        OhosResolvePlugin {
            resolver,
            project_root,
            use_normalized_ohm_url,
            case_sensitive,
        }
    }

    /// Synchronous resolution helper used in unit tests.
    #[cfg(test)]
    pub(crate) fn resolve_sync(
        &self,
        specifier: &str,
        importer: Option<&str>,
    ) -> anyhow::Result<Option<HookResolveIdOutput>> {
        let specifier = ResolveHookSpecifier::new(specifier);
        if specifier.is_virtual() {
            return Ok(None);
        }
        if specifier.is_native_shared_object() {
            return Ok(Some(specifier.native_shared_object_output()));
        }
        if specifier.is_builtin_external() {
            return Ok(Some(specifier.unresolved_external_output()));
        }
        if specifier.is_absolute_path() {
            return Ok(None);
        }

        let importer_path_buf: PathBuf;
        let importer_path: &Path = if let Some(imp) = importer {
            importer_path_buf = PathBuf::from(imp);
            &importer_path_buf
        } else {
            self.project_root.as_path()
        };

        let resolved = match self.resolver.resolve_with_kind(
            specifier.as_str(),
            importer_path,
            ResolveImportKind::Import,
        ) {
            Ok(resolved) => resolved,
            Err(err) => return Err(err.into()),
        };

        let output = ResolveHookOutputMapper::new(&self.project_root).map_resolved(resolved)?;
        Ok(Some(output))
    }
}

impl Plugin for OhosResolvePlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("ohos-resolve")
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::ResolveId | HookUsage::Load
    }

    fn resolve_id(
        &self,
        _ctx: &PluginContext,
        args: &HookResolveIdArgs<'_>,
    ) -> impl std::future::Future<Output = HookResolveIdReturn> + Send {
        let specifier = args.specifier.to_owned();
        let importer = args.importer.map(str::to_owned);
        let resolver = Arc::clone(&self.resolver);
        let project_root = self.project_root.clone();
        let import_kind = args.kind;

        async move {
            let s = ResolveHookSpecifier::new(specifier.as_str());

            if s.is_virtual() {
                return Ok(None);
            }

            if s.is_native_shared_object() {
                return Ok(Some(s.native_shared_object_output()));
            }

            // Fast path: colon-form OHOS builtins are always external.
            if s.is_builtin_external() {
                return Ok(Some(s.unresolved_external_output()));
            }

            // Absolute paths: let rolldown's default resolver handle them.
            if s.is_absolute_path() {
                return Ok(None);
            }

            let importer_path_buf: PathBuf;
            let importer_path: &Path = if let Some(ref imp) = importer {
                importer_path_buf = PathBuf::from(imp.as_str());
                &importer_path_buf
            } else {
                project_root.as_path()
            };

            let resolved = match resolver.resolve_with_kind(
                s.as_str(),
                importer_path,
                resolve_import_kind(import_kind),
            ) {
                Ok(resolved) => resolved,
                Err(err) => return Err(err.into()),
            };

            let output =
                ResolveHookOutputMapper::new(project_root.as_path()).map_resolved(resolved)?;

            Ok(Some(output))
        }
    }

    fn load(
        &self,
        _ctx: SharedLoadPluginContext,
        args: &HookLoadArgs<'_>,
    ) -> impl std::future::Future<Output = HookLoadReturn> + Send {
        let id = args.id.to_owned();
        async move { HvigorIgnoreModuleLoader::new(&id).load() }
    }
}

fn resolve_import_kind(kind: ImportKind) -> ResolveImportKind {
    match kind {
        ImportKind::Require => ResolveImportKind::Require,
        ImportKind::Import
        | ImportKind::DynamicImport
        | ImportKind::AtImport
        | ImportKind::UrlImport
        | ImportKind::NewUrl
        | ImportKind::HotAccept => ResolveImportKind::Import,
    }
}

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

/// Convert a resolved OHOS module into a rolldown [`HookResolveIdOutput`].
///
/// ## Externalization rules
///
/// | Variant | external | id |
/// |---|---|---|
/// | `ExternalBundle` | `true` | OHM URL as-is |
/// | `ExternalHsp` | `true` | `@shared:<bundle>/<module>/<entry>` |
/// | `SdkDeclaration` | `true` | original specifier |
/// | `BytecodeHar` | `true` | OHM URL |
/// | `LocalSource` | `None` | absolute filesystem path |
/// | `RemotePackage` | `None` | absolute filesystem path |
struct ResolveHookOutputMapper<'a> {
    project_root: &'a Path,
}

impl<'a> ResolveHookOutputMapper<'a> {
    fn new(project_root: &'a Path) -> Self {
        Self { project_root }
    }

    fn map_resolved(&self, resolved: OhosResolvedModule) -> anyhow::Result<HookResolveIdOutput> {
        if let Some(output) = self.map_declaration_file(&resolved)? {
            return Ok(output);
        }

        // External variants — always externalized regardless of ohm_url flag.
        if let Some(ext_id) = resolved.external_id() {
            return Ok(HookResolveIdOutput {
                id: ext_id.into_owned().into(),
                external: Some(ResolvedExternal::Bool(true)),
                ..Default::default()
            });
        }

        // Bundled source modules use filesystem paths as Rolldown module IDs.
        let path = resolved
            .local_path()
            .expect("non-external OhosResolvedModule must have a local path");
        let path = if path.has_root() {
            path.clone()
        } else {
            self.project_root.join(path)
        };
        Ok(HookResolveIdOutput {
            id: ArtifactPathSerializer::path_to_slash_string(&path)?.into(),
            external: None,
            ..Default::default()
        })
    }

    fn map_declaration_file(
        &self,
        resolved: &OhosResolvedModule,
    ) -> anyhow::Result<Option<HookResolveIdOutput>> {
        let path = match resolved {
            OhosResolvedModule::LocalSource { path, .. }
            | OhosResolvedModule::RemotePackage { path, .. }
            | OhosResolvedModule::SdkDeclaration { path, .. } => path,
            _ => return Ok(None),
        };
        let declaration = ResolvedDeclarationFile::new(path);
        if !declaration.is_types_declaration() {
            return Ok(None);
        }
        Ok(Some(HookResolveIdOutput {
            id: HvigorIgnoreModuleId::from_path(path)?.id.into(),
            external: Some(ResolvedExternal::Bool(true)),
            ..Default::default()
        }))
    }
}

// ---------------------------------------------------------------------------
// Resolve hook specifier classification
// ---------------------------------------------------------------------------

struct ResolveHookSpecifier<'a> {
    raw: &'a str,
    kind: SpecifierKind,
}

impl<'a> ResolveHookSpecifier<'a> {
    fn new(raw: &'a str) -> Self {
        Self {
            raw,
            kind: ModuleSpecifier::new(raw).kind(),
        }
    }

    fn as_str(&self) -> &'a str {
        self.raw
    }

    fn is_virtual(&self) -> bool {
        self.raw.contains('\0')
    }

    fn is_native_shared_object(&self) -> bool {
        NativeSharedObjectSpecifier::new(self.raw).matches_hvigor_rule()
    }

    fn native_shared_object_output(&self) -> HookResolveIdOutput {
        HookResolveIdOutput {
            id: HvigorIgnoreModuleId::from_specifier(self.raw).id.into(),
            external: Some(ResolvedExternal::Bool(true)),
            ..Default::default()
        }
    }

    /// Returns `true` for colon-form OHOS built-in specifiers that are always
    /// external and do not need to go through the resolver.
    ///
    /// These are SDK-provided module namespaces injected by ArkRuntime:
    /// - `@ohos:` — OpenHarmony system APIs (e.g. `@ohos:hilog`)
    /// - `@hms:` / `@hms.` — Huawei Mobile Services APIs
    /// - `@system.` — legacy system module namespace
    /// - `@native:` — native C/C++ bridge modules
    fn is_builtin_external(&self) -> bool {
        ModuleSpecifier::new(self.raw).is_runtime_builtin()
    }

    fn is_absolute_path(&self) -> bool {
        matches!(self.kind, SpecifierKind::AbsolutePath)
    }

    /// Build a [`HookResolveIdOutput`] that marks this specifier as an
    /// unresolved external.
    fn unresolved_external_output(&self) -> HookResolveIdOutput {
        HookResolveIdOutput {
            id: self.raw.into(),
            external: Some(ResolvedExternal::Bool(true)),
            ..Default::default()
        }
    }
}

struct NativeSharedObjectSpecifier<'a> {
    raw: &'a str,
}

impl<'a> NativeSharedObjectSpecifier<'a> {
    fn new(raw: &'a str) -> Self {
        Self { raw }
    }

    fn matches_hvigor_rule(&self) -> bool {
        let Some(index) = self.raw.find(".so") else {
            return false;
        };
        index > "lib".len()
            && self.raw.starts_with("lib")
            && self.raw["lib".len()..index]
                .chars()
                .all(|character| !character.is_whitespace())
    }
}

struct ResolvedDeclarationFile<'a> {
    path: &'a Path,
}

impl<'a> ResolvedDeclarationFile<'a> {
    fn new(path: &'a Path) -> Self {
        Self { path }
    }

    fn is_types_declaration(&self) -> bool {
        let name = match self.path.file_name().and_then(|name| name.to_str()) {
            Some(name) => name,
            None => return false,
        };
        name.ends_with(".d.ts") || name.ends_with(".d.ets")
    }
}

struct HvigorIgnoreModuleId {
    id: String,
}

impl HvigorIgnoreModuleId {
    fn from_specifier(specifier: &str) -> Self {
        Self {
            id: format!("{HVIGOR_IGNORE_PREFIX}{}", Self::sanitize(specifier)),
        }
    }

    fn from_path(path: &Path) -> anyhow::Result<Self> {
        Ok(Self::from_specifier(
            ArtifactPathSerializer::path_to_slash_string(path)?.as_str(),
        ))
    }

    fn is_virtual_id(id: &str) -> bool {
        id.starts_with(HVIGOR_IGNORE_PREFIX)
    }

    fn sanitize(value: &str) -> String {
        value.replace(['\\', '/'], "_")
    }
}

struct HvigorIgnoreModuleLoader<'a> {
    id: &'a str,
}

impl<'a> HvigorIgnoreModuleLoader<'a> {
    fn new(id: &'a str) -> Self {
        Self { id }
    }

    fn load(&self) -> HookLoadReturn {
        if !HvigorIgnoreModuleId::is_virtual_id(self.id) {
            return Ok(None);
        }
        Ok(Some(HookLoadOutput {
            code: "".into(),
            ..Default::default()
        }))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use arkdown_resolver::PkgBriefInfo;
    use rustc_hash::FxHashMap;

    fn make_plugin() -> OhosResolvePlugin {
        OhosResolvePlugin::new(
            ResolverConfig {
                project_root: PathBuf::from("/tmp"),
                sdk_api_dirs: vec![],
                sdk_kits_dirs: vec![],
                sdk_arkts_dirs: vec![],
                source_roots: vec![],
                module_paths: vec![],
                pkg_name_to_pkg_info: FxHashMap::default(),
                use_normalized_ohm_url: false,
                case_sensitive_check: false,
                source_set_overlays: vec![],
            },
            PathBuf::from("/tmp"),
            false,
        )
    }

    #[test]
    fn test_bundle_ref_is_external() {
        let plugin = make_plugin();
        let result = plugin
            .resolve_sync("@bundle:com.foo/entry/ets/Index", None)
            .unwrap();
        let id = result.unwrap();
        assert!(matches!(id.external, Some(ResolvedExternal::Bool(true))));
        assert!(id.id.contains("com.foo"));
    }

    #[test]
    fn test_unknown_bare_specifier_is_reported() {
        let plugin = make_plugin();
        let result = plugin.resolve_sync("some-unknown-package", Some("/tmp/main.ets"));
        assert!(result.is_err());
    }

    #[test]
    fn test_native_shared_object_uses_hvigor_ignore_virtual_id() {
        let plugin = make_plugin();
        let result = plugin
            .resolve_sync("libmtlauncher.so", Some("/tmp/main.ets"))
            .unwrap()
            .unwrap();
        assert_eq!(result.id.as_str(), "\0hvigor_ignore_libmtlauncher.so");
        assert!(matches!(
            result.external,
            Some(ResolvedExternal::Bool(true))
        ));
    }

    #[test]
    fn test_hvigor_ignore_virtual_id_loads_empty_module() {
        let output = HvigorIgnoreModuleLoader::new("\0hvigor_ignore_libmtlauncher.so")
            .load()
            .unwrap()
            .unwrap();
        assert_eq!(output.code.as_str(), "");
        assert!(HvigorIgnoreModuleLoader::new("libmtlauncher.so")
            .load()
            .unwrap()
            .is_none());
    }

    #[test]
    fn test_unknown_relative_specifier_returns_error() {
        let plugin = make_plugin();
        let err = plugin
            .resolve_sync("./missing", Some("/tmp/main.ets"))
            .expect_err("relative resolution must stay strict");
        assert!(err.to_string().contains("Module not found"));
    }

    #[test]
    fn test_shared_ref_is_external() {
        let plugin = make_plugin();
        let result = plugin
            .resolve_sync("@shared:com.example/sharedlib", None)
            .unwrap();
        let id = result.unwrap();
        assert!(matches!(id.external, Some(ResolvedExternal::Bool(true))));
        assert!(id.id.starts_with("@shared:"));
    }

    #[test]
    fn test_builtin_ohos_specifier_is_external() {
        let plugin = make_plugin();
        let result = plugin
            .resolve_sync("@ohos:hilog", Some("/tmp/main.ets"))
            .unwrap()
            .unwrap();
        assert!(matches!(
            result.external,
            Some(ResolvedExternal::Bool(true))
        ));
        assert_eq!(result.id.as_str(), "@ohos:hilog");
    }

    #[test]
    fn test_ohos_scoped_package_resolves_from_hvigor_module_paths() {
        let dir = tempfile::tempdir().unwrap();
        let project_root = dir.path();
        let module_root = project_root.join("meituan");
        let pkg_dir = project_root.join("oh_modules/@ohos/hypium");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        std::fs::write(
            pkg_dir.join("oh-package.json5"),
            r#"{ "name": "@ohos/hypium", "version": "1.0.11", "main": "index.js" }"#,
        )
        .unwrap();
        std::fs::write(
            pkg_dir.join("index.js"),
            "export const DEFAULT = 'default';",
        )
        .unwrap();
        std::fs::write(
            pkg_dir.join("index.d.ts"),
            "export declare const DEFAULT: string;",
        )
        .unwrap();
        let importer = module_root.join("src/main/ets/Index.ets");

        let plugin = OhosResolvePlugin::new(
            ResolverConfig {
                project_root: project_root.to_path_buf(),
                sdk_api_dirs: vec![],
                sdk_kits_dirs: vec![],
                sdk_arkts_dirs: vec![],
                source_roots: vec![],
                module_paths: vec![project_root.join("oh_modules")],
                pkg_name_to_pkg_info: FxHashMap::default(),
                use_normalized_ohm_url: false,
                case_sensitive_check: false,
                source_set_overlays: vec![],
            },
            project_root.to_path_buf(),
            false,
        );

        let result = plugin
            .resolve_sync("@ohos/hypium", Some(importer.to_string_lossy().as_ref()))
            .unwrap()
            .unwrap();

        assert!(result.external.is_none());
        assert!(result.id.ends_with("/oh_modules/@ohos/hypium/index.js"));
    }

    #[test]
    fn test_remote_package_declaration_uses_hvigor_ignore_virtual_id() {
        let dir = tempfile::tempdir().unwrap();
        let pkg_dir = dir.path().join("oh_modules/decl-only");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        std::fs::write(pkg_dir.join("oh-package.json5"), r#"{ name: "decl-only" }"#).unwrap();
        let declaration = pkg_dir.join("index.d.ts");
        std::fs::write(&declaration, "export declare const value: string;").unwrap();
        let mut pkg_name_to_pkg_info = FxHashMap::default();
        pkg_name_to_pkg_info.insert(
            "decl-only".to_string(),
            PkgBriefInfo {
                pkg_root: pkg_dir.clone(),
                pkg_name: "decl-only".to_string(),
                local_source: false,
                source_roots: vec![],
            },
        );
        let plugin = OhosResolvePlugin::new(
            ResolverConfig {
                project_root: dir.path().to_path_buf(),
                sdk_api_dirs: vec![],
                sdk_kits_dirs: vec![],
                sdk_arkts_dirs: vec![],
                source_roots: vec![],
                module_paths: vec![],
                pkg_name_to_pkg_info,
                use_normalized_ohm_url: false,
                case_sensitive_check: false,
                source_set_overlays: vec![],
            },
            dir.path().to_path_buf(),
            false,
        );

        let result = plugin
            .resolve_sync("decl-only", Some("/tmp/main.ets"))
            .unwrap()
            .unwrap();
        assert!(result.id.starts_with(HVIGOR_IGNORE_PREFIX));
        assert!(result.id.ends_with("_oh_modules_decl-only_index.d.ts"));
        assert!(matches!(
            result.external,
            Some(ResolvedExternal::Bool(true))
        ));
    }

    #[test]
    fn test_sdk_declaration_uses_hvigor_ignore_virtual_id() {
        let dir = tempfile::tempdir().unwrap();
        let kits_dir = dir.path().join("sdk/kits");
        std::fs::create_dir_all(&kits_dir).unwrap();
        std::fs::write(kits_dir.join("@kit.ArkData.d.ts"), "export {};").unwrap();
        let plugin = OhosResolvePlugin::new(
            ResolverConfig {
                project_root: dir.path().to_path_buf(),
                sdk_api_dirs: vec![],
                sdk_kits_dirs: vec![kits_dir],
                sdk_arkts_dirs: vec![],
                source_roots: vec![],
                module_paths: vec![],
                pkg_name_to_pkg_info: FxHashMap::default(),
                use_normalized_ohm_url: false,
                case_sensitive_check: false,
                source_set_overlays: vec![],
            },
            dir.path().to_path_buf(),
            false,
        );

        let result = plugin
            .resolve_sync("@kit.ArkData", Some("/tmp/main.ets"))
            .unwrap()
            .unwrap();
        assert!(matches!(
            result.external,
            Some(ResolvedExternal::Bool(true))
        ));
        assert!(result.id.starts_with(HVIGOR_IGNORE_PREFIX));
        assert!(result.id.ends_with("_sdk_kits_@kit.ArkData.d.ts"));
    }
}
