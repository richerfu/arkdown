use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arkdown_core::OhosResolvedModule;
use arkdown_resolver::{OhosResolver, ResolverConfig};
use rolldown_common::ResolvedExternal;
use rolldown_plugin::{
    HookResolveIdArgs, HookResolveIdOutput, HookResolveIdReturn, HookUsage, Plugin, PluginContext,
};

pub struct OhosResolvePlugin {
    resolver: Arc<OhosResolver>,
    project_root: PathBuf,
    #[allow(dead_code)]
    case_sensitive: bool,
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
        }
    }

    pub(crate) fn resolve_sync(
        &self,
        specifier: &str,
        importer: Option<&str>,
    ) -> anyhow::Result<Option<HookResolveIdOutput>> {
        let s = specifier;
        if is_builtin_external_specifier(s) {
            return Ok(Some(HookResolveIdOutput {
                id: s.into(),
                external: Some(ResolvedExternal::Bool(true)),
                ..Default::default()
            }));
        }
        if Path::new(s).is_absolute() {
            return Ok(None);
        }

        let importer_path_buf: PathBuf;
        let importer_path: &Path = if let Some(imp) = importer {
            importer_path_buf = PathBuf::from(imp);
            &importer_path_buf
        } else {
            self.project_root.as_path()
        };

        let resolved = self.resolver.resolve(specifier, importer_path)?;

        let output = match resolved {
            OhosResolvedModule::ExternalBundle { ohm_url } => HookResolveIdOutput {
                id: ohm_url.into(),
                external: Some(ResolvedExternal::Bool(true)),
                ..Default::default()
            },
            OhosResolvedModule::ExternalHsp {
                bundle_name,
                module_name,
                entry_path,
            } => HookResolveIdOutput {
                id: format!("@shared:{}/{}/{}", bundle_name, module_name, entry_path).into(),
                external: Some(ResolvedExternal::Bool(true)),
                ..Default::default()
            },
            OhosResolvedModule::SdkDeclaration { path, .. } => HookResolveIdOutput {
                id: path.to_string_lossy().as_ref().into(),
                external: Some(ResolvedExternal::Bool(true)),
                ..Default::default()
            },
            OhosResolvedModule::BytecodeHar { ohm_url, .. } => HookResolveIdOutput {
                id: ohm_url.into(),
                external: Some(ResolvedExternal::Bool(true)),
                ..Default::default()
            },
            OhosResolvedModule::LocalSource { path, .. } => HookResolveIdOutput {
                id: path.to_string_lossy().as_ref().into(),
                external: None,
                ..Default::default()
            },
            OhosResolvedModule::RemotePackage { path, .. } => HookResolveIdOutput {
                id: path.to_string_lossy().as_ref().into(),
                external: None,
                ..Default::default()
            },
        };

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
        async move {
            let s = specifier.as_str();
            if is_builtin_external_specifier(s) {
                return Ok(Some(HookResolveIdOutput {
                    id: s.into(),
                    external: Some(ResolvedExternal::Bool(true)),
                    ..Default::default()
                }));
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

            let resolved = resolver.resolve(s, importer_path)?;

            let output = match resolved {
                OhosResolvedModule::ExternalBundle { ohm_url } => HookResolveIdOutput {
                    id: ohm_url.into(),
                    external: Some(ResolvedExternal::Bool(true)),
                    ..Default::default()
                },
                OhosResolvedModule::ExternalHsp {
                    bundle_name,
                    module_name,
                    entry_path,
                } => HookResolveIdOutput {
                    id: format!("@shared:{}/{}/{}", bundle_name, module_name, entry_path).into(),
                    external: Some(ResolvedExternal::Bool(true)),
                    ..Default::default()
                },
                OhosResolvedModule::SdkDeclaration { path, .. } => HookResolveIdOutput {
                    id: path.to_string_lossy().as_ref().into(),
                    external: Some(ResolvedExternal::Bool(true)),
                    ..Default::default()
                },
                OhosResolvedModule::BytecodeHar { ohm_url, .. } => HookResolveIdOutput {
                    id: ohm_url.into(),
                    external: Some(ResolvedExternal::Bool(true)),
                    ..Default::default()
                },
                OhosResolvedModule::LocalSource { path, .. } => HookResolveIdOutput {
                    id: path.to_string_lossy().as_ref().into(),
                    external: None,
                    ..Default::default()
                },
                OhosResolvedModule::RemotePackage { path, .. } => HookResolveIdOutput {
                    id: path.to_string_lossy().as_ref().into(),
                    external: None,
                    ..Default::default()
                },
            };

            Ok(Some(output))
        }
    }
}

fn is_builtin_external_specifier(specifier: &str) -> bool {
    specifier.starts_with("@ohos:")
        || specifier.starts_with("@system.")
        || specifier.starts_with("@native:")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_plugin() -> OhosResolvePlugin {
        OhosResolvePlugin::new(
            ResolverConfig {
                project_root: PathBuf::from("/tmp"),
                sdk_api_dirs: vec![],
                sdk_kits_dirs: vec![],
                pkg_name_to_pkg_info: HashMap::new(),
                use_normalized_ohm_url: false,
                case_sensitive_check: false,
                oh_modules_roots: vec![],
                module_src_roots: vec![],
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
    fn test_unknown_specifier_returns_error() {
        let plugin = make_plugin();
        let result = plugin
            .resolve_sync("some-unknown-package", Some("/tmp/main.ets"))
            .expect_err("unknown package must fail resolution");
        assert!(result.to_string().contains("Module not found"));
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
}
