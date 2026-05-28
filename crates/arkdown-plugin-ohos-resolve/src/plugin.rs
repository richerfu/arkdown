use std::path::{Path, PathBuf};
use std::sync::Arc;

use arkdown_core::{
    ArkdownPlugin, HookResolveIdArgs, HookResolveIdOutput, HookUsage, ModuleType,
    OhosResolvedModule, PluginContext, PluginResult,
};
use arkdown_resolver::{OhosResolver, ResolverConfig};
use async_trait::async_trait;

pub struct OhosResolvePlugin {
    resolver: Arc<OhosResolver>,
    project_root: PathBuf,
    #[allow(dead_code)]
    case_sensitive: bool,
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
}

#[async_trait]
impl ArkdownPlugin for OhosResolvePlugin {
    fn name(&self) -> &str {
        "ohos-resolve"
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::ResolveId
    }

    async fn resolve_id(
        &self,
        _ctx: &PluginContext,
        args: &HookResolveIdArgs<'_>,
    ) -> PluginResult<Option<HookResolveIdOutput>> {
        // SDK built-in modules that ArkRuntime provides natively must stay as
        // ESM imports in the bundler output so es2abc sees normal import
        // statements rather than CJS require() + interop helpers.
        // The ETS kit-rewrite produces "@ohos:xxx" specifiers (colon); the
        // resolver uses "@ohos/xxx" (slash).  Handle both forms here.
        let s = args.specifier;
        if s.starts_with("@ohos:")
            || s.starts_with("@ohos/")
            || s.starts_with("@system.")
            || s.starts_with("@native:")
        {
            return Ok(Some(HookResolveIdOutput {
                id: s.to_string(),
                external: true,
                module_type: ModuleType::Ts,
                side_effects: None,
            }));
        }

        let importer_path = args.importer.map(Path::new).unwrap_or(&self.project_root);

        let resolved = match self.resolver.resolve(args.specifier, importer_path) {
            Ok(r) => r,
            Err(_) => return Ok(None), // let next plugin try
        };

        let result = match resolved {
            OhosResolvedModule::ExternalBundle { ohm_url } => HookResolveIdOutput {
                id: ohm_url,
                external: true,
                module_type: ModuleType::Ts,
                side_effects: None,
            },
            OhosResolvedModule::ExternalHsp {
                bundle_name,
                module_name,
                entry_path,
            } => HookResolveIdOutput {
                id: format!("@shared:{}/{}/{}", bundle_name, module_name, entry_path),
                external: true,
                module_type: ModuleType::Ts,
                side_effects: None,
            },
            OhosResolvedModule::SdkDeclaration { path, .. } => HookResolveIdOutput {
                id: path.to_string_lossy().to_string(),
                external: true,
                module_type: ModuleType::Declaration,
                side_effects: None,
            },
            OhosResolvedModule::BytecodeHar { ohm_url, .. } => HookResolveIdOutput {
                id: ohm_url,
                external: true,
                module_type: ModuleType::BytecodeHar,
                side_effects: None,
            },
            OhosResolvedModule::LocalSource {
                path, module_type, ..
            } => HookResolveIdOutput {
                id: path.to_string_lossy().to_string(),
                external: false,
                module_type,
                side_effects: None,
            },
            OhosResolvedModule::RemotePackage {
                path, module_type, ..
            } => HookResolveIdOutput {
                id: path.to_string_lossy().to_string(),
                external: false,
                module_type,
                side_effects: None,
            },
        };

        Ok(Some(result))
    }
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

    fn make_args<'a>(specifier: &'a str, importer: Option<&'a str>) -> HookResolveIdArgs<'a> {
        use arkdown_core::ImportKind;
        HookResolveIdArgs {
            specifier,
            importer,
            is_entry: importer.is_none(),
            kind: ImportKind::Import,
        }
    }

    #[tokio::test]
    async fn test_bundle_ref_is_external() {
        let plugin = make_plugin();
        let result = plugin
            .resolve_id(
                &PluginContext::default(),
                &make_args("@bundle:com.foo/entry/ets/Index", None),
            )
            .await
            .unwrap();
        let id = result.unwrap();
        assert!(id.external);
        assert!(id.id.contains("com.foo"));
    }

    #[tokio::test]
    async fn test_unknown_specifier_returns_none() {
        let plugin = make_plugin();
        let result = plugin
            .resolve_id(
                &PluginContext::default(),
                &make_args("some-unknown-package", Some("/tmp/main.ets")),
            )
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_shared_ref_is_external() {
        let plugin = make_plugin();
        let result = plugin
            .resolve_id(
                &PluginContext::default(),
                &make_args("@shared:com.example/sharedlib", None),
            )
            .await
            .unwrap();
        let id = result.unwrap();
        assert!(id.external);
        assert!(id.id.starts_with("@shared:"));
    }
}
