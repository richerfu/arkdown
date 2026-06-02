use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use arkdown_core::OhosResolvedModule;
use arkdown_resolver::{OhosResolver, ResolverConfig};
use rolldown_common::ResolvedExternal;
use rolldown_plugin::{
    HookResolveIdArgs, HookResolveIdOutput, HookResolveIdReturn, HookUsage, Plugin, PluginContext,
};
use rustc_hash::FxHashMap;

pub type ResolvedImportMap = FxHashMap<String, FxHashMap<String, String>>;

pub struct OhosResolvePlugin {
    resolver: Arc<OhosResolver>,
    project_root: PathBuf,
    #[allow(dead_code)]
    case_sensitive: bool,
    resolved_imports: Arc<Mutex<ResolvedImportMap>>,
}

impl std::fmt::Debug for OhosResolvePlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OhosResolvePlugin")
            .field("project_root", &self.project_root)
            .finish_non_exhaustive()
    }
}

impl OhosResolvePlugin {
    pub fn new(
        resolver_config: ResolverConfig,
        project_root: PathBuf,
        case_sensitive: bool,
    ) -> Self {
        OhosResolvePlugin {
            resolver: Arc::new(OhosResolver::new(resolver_config)),
            project_root,
            case_sensitive,
            resolved_imports: Arc::new(Mutex::new(FxHashMap::default())),
        }
    }

    pub fn resolved_import_map_snapshot(&self) -> ResolvedImportMap {
        self.resolved_imports
            .lock()
            .map(|map| map.clone())
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub(crate) fn resolve_sync(
        &self,
        specifier: &str,
        importer: Option<&str>,
    ) -> anyhow::Result<Option<HookResolveIdOutput>> {
        if is_builtin_external_specifier(specifier) {
            return Ok(Some(unresolved_external_output(specifier)));
        }
        if Path::new(specifier).is_absolute() {
            return Ok(None);
        }

        let importer_path_buf: PathBuf;
        let importer_path: &Path = if let Some(imp) = importer {
            importer_path_buf = PathBuf::from(imp);
            &importer_path_buf
        } else {
            self.project_root.as_path()
        };

        let resolved = match self.resolver.resolve(specifier, importer_path) {
            Ok(resolved) => resolved,
            Err(_) if should_externalize_unresolved(specifier) => {
                return Ok(Some(unresolved_external_output(specifier)));
            }
            Err(err) => return Err(err.into()),
        };

        let output = resolved_module_to_hook_output(resolved);
        if let Some(importer) = importer {
            record_resolved_import(
                &self.resolved_imports,
                &importer,
                specifier,
                output.id.as_str(),
            );
        }
        Ok(Some(output))
    }
}

impl Plugin for OhosResolvePlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("ohos-resolve")
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::ResolveId
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
        let resolved_imports = Arc::clone(&self.resolved_imports);
        async move {
            let s = specifier.as_str();
            if is_builtin_external_specifier(s) {
                return Ok(Some(unresolved_external_output(s)));
            }
            if Path::new(s).is_absolute() {
                return Ok(None);
            }

            let importer_path_buf: PathBuf;
            let importer_path: &Path = if let Some(ref imp) = importer {
                importer_path_buf = PathBuf::from(imp);
                &importer_path_buf
            } else {
                project_root.as_path()
            };

            let resolved = match resolver.resolve(s, importer_path) {
                Ok(resolved) => resolved,
                Err(_) if should_externalize_unresolved(s) => {
                    return Ok(Some(unresolved_external_output(s)));
                }
                Err(err) => return Err(err.into()),
            };

            let output = resolved_module_to_hook_output(resolved);
            if let Some(importer) = importer {
                record_resolved_import(
                    &resolved_imports,
                    &importer,
                    &specifier,
                    output.id.as_str(),
                );
            }
            Ok(Some(output))
        }
    }
}

fn record_resolved_import(
    resolved_imports: &Arc<Mutex<ResolvedImportMap>>,
    importer: &str,
    specifier: &str,
    resolved_id: &str,
) {
    if let Ok(mut map) = resolved_imports.lock() {
        map.entry(importer.to_string())
            .or_default()
            .insert(specifier.to_string(), resolved_id.to_string());
    }
}

/// Convert a resolved OHOS module into a rolldown [`HookResolveIdOutput`].
///
/// External variants (bundle refs, HSP refs, SDK declarations, bytecode HARs)
/// are marked `external = true`; local source and remote packages are bundled
/// normally (`external = None`).
fn resolved_module_to_hook_output(resolved: OhosResolvedModule) -> HookResolveIdOutput {
    if let Some(ext_id) = resolved.external_id() {
        return HookResolveIdOutput {
            id: ext_id.into_owned().into(),
            external: Some(ResolvedExternal::Bool(true)),
            ..Default::default()
        };
    }
    let path = resolved
        .local_path()
        .expect("non-external OhosResolvedModule must have a local path");
    HookResolveIdOutput {
        id: path.to_string_lossy().as_ref().into(),
        external: None,
        ..Default::default()
    }
}

fn is_builtin_external_specifier(specifier: &str) -> bool {
    specifier.starts_with("@ohos:")
        || specifier.starts_with("@hms:")
        || specifier.starts_with("@hms.")
        || specifier.starts_with("@system.")
        || specifier.starts_with("@native:")
}

fn should_externalize_unresolved(specifier: &str) -> bool {
    !Path::new(specifier).is_absolute()
        && !specifier.starts_with("./")
        && !specifier.starts_with("../")
}

fn unresolved_external_output(specifier: &str) -> HookResolveIdOutput {
    HookResolveIdOutput {
        id: specifier.into(),
        external: Some(ResolvedExternal::Bool(true)),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustc_hash::FxHashMap;

    fn make_plugin() -> OhosResolvePlugin {
        OhosResolvePlugin::new(
            ResolverConfig {
                project_root: PathBuf::from("/tmp"),
                sdk_api_dirs: vec![],
                sdk_kits_dirs: vec![],
                pkg_name_to_pkg_info: FxHashMap::default(),
                use_normalized_ohm_url: false,
                case_sensitive_check: false,
                oh_modules_roots: vec![],
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
    fn test_unknown_bare_specifier_is_externalized() {
        let plugin = make_plugin();
        let result = plugin
            .resolve_sync("some-unknown-package", Some("/tmp/main.ets"))
            .unwrap()
            .unwrap();
        assert_eq!(result.id.as_str(), "some-unknown-package");
        assert!(matches!(
            result.external,
            Some(ResolvedExternal::Bool(true))
        ));
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
}
