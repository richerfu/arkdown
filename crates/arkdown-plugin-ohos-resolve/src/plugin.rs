use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use arkdown_core::{
    ArkdownPlugin, ModuleType, OhosResolvedModule, PluginResult, ResolveContext, ResolvedId,
};
use arkdown_resolver::{OhosResolver, ResolverConfig};

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

    async fn resolve_id(
        &self,
        _ctx: &ResolveContext,
        specifier: &str,
        importer: Option<&str>,
    ) -> PluginResult<Option<ResolvedId>> {
        let importer_path = importer
            .map(Path::new)
            .unwrap_or(&self.project_root);

        let resolved = match self.resolver.resolve(specifier, importer_path) {
            Ok(r) => r,
            Err(_) => return Ok(None), // let next plugin try
        };

        let result = match resolved {
            OhosResolvedModule::ExternalBundle { ohm_url } => ResolvedId {
                id: ohm_url,
                external: true,
                module_type: ModuleType::Ts,
            },
            OhosResolvedModule::ExternalHsp {
                bundle_name,
                module_name,
                entry_path,
            } => ResolvedId {
                id: format!("@shared:{}/{}/{}", bundle_name, module_name, entry_path),
                external: true,
                module_type: ModuleType::Ts,
            },
            OhosResolvedModule::SdkDeclaration { path, .. } => ResolvedId {
                id: path.to_string_lossy().to_string(),
                external: true,
                module_type: ModuleType::Declaration,
            },
            OhosResolvedModule::BytecodeHar { ohm_url, .. } => ResolvedId {
                id: ohm_url,
                external: true,
                module_type: ModuleType::BytecodeHar,
            },
            OhosResolvedModule::LocalSource {
                path, module_type, ..
            } => ResolvedId {
                id: path.to_string_lossy().to_string(),
                external: false,
                module_type,
            },
            OhosResolvedModule::RemotePackage {
                path, module_type, ..
            } => ResolvedId {
                id: path.to_string_lossy().to_string(),
                external: false,
                module_type,
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

    #[tokio::test]
    async fn test_bundle_ref_is_external() {
        let plugin = make_plugin();
        let result = plugin
            .resolve_id(
                &ResolveContext::default(),
                "@bundle:com.foo/entry/ets/Index",
                None,
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
                &ResolveContext::default(),
                "some-unknown-package",
                Some("/tmp/main.ets"),
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
                &ResolveContext::default(),
                "@shared:com.example/sharedlib",
                None,
            )
            .await
            .unwrap();
        let id = result.unwrap();
        assert!(id.external);
        assert!(id.id.starts_with("@shared:"));
    }
}
