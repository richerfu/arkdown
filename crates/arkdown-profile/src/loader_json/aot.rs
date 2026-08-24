//! AOT/AN loader.json field resolution.

use super::*;

pub(crate) struct AotLoaderFields {
    pub(crate) an_build_out_put: Option<String>,
    pub(crate) an_build_mode: Option<String>,
    pub(crate) ap_path: Option<String>,
    pub(crate) fallback_an_build: bool,
}

#[derive(Deserialize)]
pub(crate) struct DependencyPackageJson {
    #[serde(default)]
    pub(crate) name: Option<String>,
    #[serde(default)]
    pub(crate) dependencies: IndexMap<String, String>,
    #[serde(default)]
    pub(crate) main: Option<String>,
    #[serde(default)]
    pub(crate) types: Option<String>,
    #[serde(default)]
    pub(crate) version: String,
    #[serde(default)]
    pub(crate) metadata: Option<DependencyPackageMetadata>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DependencyPackageMetadata {
    #[serde(default)]
    pub(crate) resource: Option<DependencyResourceMetadata>,
}

#[derive(Deserialize)]
pub(crate) struct DependencyResourceMetadata {
    #[serde(default)]
    pub(crate) directories: Vec<String>,
}

#[derive(Deserialize)]
pub(crate) struct DependencyModuleJson {
    pub(crate) module: DependencyModuleSection,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DependencyModuleSection {
    #[serde(default)]
    pub(crate) name: Option<String>,
    #[serde(default)]
    pub(crate) abilities: Vec<DependencyAbilityEntry>,
    #[serde(default)]
    pub(crate) extension_abilities: Vec<DependencyAbilityEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DependencyAbilityEntry {
    #[serde(default)]
    pub(crate) src_entry: Option<String>,
    #[serde(rename = "type", alias = "extensionType", default)]
    pub(crate) extension_type: Option<String>,
}

impl AotLoaderFields {
    pub(super) fn from_config(
        config: &ArkdownConfig,
        module_cfg: &ModuleConfig,
        manifest: &HapManifest,
        target_name: &str,
    ) -> Self {
        let fallback_an_build = matches!(manifest.module_type, OhosModuleType::Har);
        if fallback_an_build {
            return Self::empty(fallback_an_build);
        }

        // Preview builds do not run AOT compilation. hvigor skips
        // `anBuildMode`/`anBuildOutPut` for preview targets. Local unit-test
        // builds keep them (hvigor `handleAotFields` is target-agnostic).
        if matches!(config.output_target, arkdown_core::OutputTarget::Preview) {
            return Self::empty(false);
        }

        let api_version = config.sdk_config.compile_api_version;
        let an_build_mode = Self::resolve_an_build_mode(config);
        let ap_path = Self::resolve_ap_path(config, module_cfg);
        let ap_exists = ap_path.as_ref().is_some_and(|path| path.exists());
        let has_ap_path = ap_path.is_some();
        let ap_suffix_valid = ap_path
            .as_ref()
            .is_none_or(|path| path.extension().is_some_and(|ext| ext == "ap"));

        if api_version == 9 && an_build_mode != "partial" && has_ap_path {
            panic!(
                "MISMATCH_AOT_COMPILE_MODE: apPath is only valid with aotCompileMode=partial on API 9"
            );
        }
        if api_version == 9 && an_build_mode == "partial" && !has_ap_path {
            panic!("PARTIAL_EMPTY_PATH: aotCompileMode=partial requires apPath on API 9");
        }
        if !ap_suffix_valid {
            panic!("PARTIAL_INVALID_SUFFIX: apPath must end with .ap");
        }
        if an_build_mode == "partial" && !ap_exists {
            let display_path = ap_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_default();
            panic!("PARTIAL_INVALID_PATH: apPath does not exist: {display_path}");
        }
        if ((api_version == 9 && !has_ap_path) || api_version >= 10) && an_build_mode.is_empty() {
            return Self::empty(fallback_an_build);
        }

        let module_root = PathBuf::from(to_absolute_path_string(&module_cfg.src_path));
        let aot_layout = if matches!(config.output_target, OutputTarget::Test) {
            ProfileModuleLayout::for_test_product(&module_root, &config.product.name)
        } else if matches!(config.output_target, OutputTarget::Preview) {
            ProfileModuleLayout::for_preview_product(
                &module_root,
                &config.product.name,
                target_name,
            )
        } else if target_name != "default" {
            ProfileModuleLayout::for_target_product(&module_root, &config.product.name, target_name)
        } else {
            ProfileModuleLayout::for_product(&module_root, &config.product.name)
        };
        let an_build_out_put = normalize_path_string(&aot_layout.aot_loader_output_dir());

        let should_write_ap_path =
            ((api_version == 9 && an_build_mode == "partial") || api_version >= 10) && ap_exists;
        if should_write_ap_path {
            let ap_path = ap_path.expect("ap_exists implies ap_path");
            Self::copy_ap_file(module_cfg, &aot_layout, &ap_path);
            return Self {
                an_build_out_put: Some(an_build_out_put),
                an_build_mode: Some(an_build_mode),
                ap_path: Some(to_absolute_path_string(&ap_path)),
                fallback_an_build,
            };
        }

        let without_ap_path = (api_version == 9 && an_build_mode == "type" && !has_ap_path)
            || (api_version >= 10 && an_build_mode == "type" && !ap_exists);
        if without_ap_path {
            return Self {
                an_build_out_put: Some(an_build_out_put),
                an_build_mode: Some(an_build_mode),
                ap_path: None,
                fallback_an_build,
            };
        }

        Self::empty(fallback_an_build)
    }

    pub(super) fn empty(fallback_an_build: bool) -> Self {
        Self {
            an_build_out_put: None,
            an_build_mode: None,
            ap_path: None,
            fallback_an_build,
        }
    }

    pub(super) fn resolve_an_build_mode(config: &ArkdownConfig) -> String {
        let api_version = config.sdk_config.compile_api_version;
        if api_version == 9 {
            return config
                .product
                .build_option
                .aot_compile_mode
                .clone()
                .unwrap_or_default();
        }

        let host_pgo = config.product.build_option.host_pgo == Some(true);
        let is_openharmony = !arkdown_core::is_harmony_os(&config.product.runtime_os);
        if is_openharmony
            && !aot_fixed_sdk_version(&config.sdk_config.compile_sdk_version)
            && !host_pgo
        {
            return String::new();
        }

        if host_pgo {
            "partial".to_string()
        } else {
            "type".to_string()
        }
    }

    pub(super) fn resolve_ap_path(
        config: &ArkdownConfig,
        module_cfg: &ModuleConfig,
    ) -> Option<PathBuf> {
        if let Some(path) = config.product.build_option.ap_path.as_ref() {
            return Some(if path.has_root() {
                path.clone()
            } else {
                module_cfg.src_path.join(path)
            });
        }

        (config.sdk_config.compile_api_version >= 10)
            .then(|| ProfileModuleLayout::new(&module_cfg.src_path).modules_ap())
    }

    pub(super) fn copy_ap_file(
        module_cfg: &ModuleConfig,
        layout: &ProfileModuleLayout<'_>,
        source: &Path,
    ) {
        let dest_dir = layout.ap_intermediate_dir();
        std::fs::create_dir_all(&dest_dir).unwrap_or_else(|error| {
            panic!(
                "failed to create hvigor AOT intermediates directory {}: {error}",
                dest_dir.display()
            )
        });
        let dest = dest_dir.join(Path::new(&module_cfg.name).with_extension("ap"));
        std::fs::copy(source, &dest).unwrap_or_else(|error| {
            panic!(
                "failed to copy AOT profile {} to {}: {error}",
                source.display(),
                dest.display()
            )
        });
    }
}

fn aot_fixed_sdk_version(version: &str) -> bool {
    AotSdkVersionText::new(version).is_after_fixed_threshold()
}

pub(crate) struct AotSdkVersionText<'a> {
    pub(crate) raw: &'a str,
}

impl<'a> AotSdkVersionText<'a> {
    const FIXED_THRESHOLD: (u32, u32, u32, u32) = (4, 0, 8, 1);

    pub(super) fn new(raw: &'a str) -> Self {
        Self { raw }
    }

    pub(super) fn is_after_fixed_threshold(&self) -> bool {
        let raw = self.raw.trim();
        if raw.is_empty() {
            return true;
        }

        match self.version_tuple() {
            Some(current) => current > Self::FIXED_THRESHOLD,
            None => true,
        }
    }

    pub(super) fn version_tuple(&self) -> Option<(u32, u32, u32, u32)> {
        let mut parts = self.raw.trim().split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let revision = parts.next()?.parse().ok()?;
        let build = parts.next()?.parse().ok()?;
        parts
            .next()
            .is_none()
            .then_some((major, minor, revision, build))
    }
}
