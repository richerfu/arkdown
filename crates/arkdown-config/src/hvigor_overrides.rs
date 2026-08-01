use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use arkdown_core::{AppOptionOverride, ArkdownConfig, SigningConfig};
use serde::Deserialize;

use crate::ConfigError;

/// Project-level overrides evaluated from
/// `hvigorfile.*#config.ohos.overrides`.
///
/// Upstream: `AppOhosConfig.Overrides` in
/// `hvigor-ohos-plugin/src/options/build/app-ohos-config.d.ts`.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectHvigorOverrides {
    #[serde(default)]
    pub app_opt: Option<HvigorAppOptionOverride>,
    #[serde(default)]
    pub signing_config: Option<HvigorSigningConfigOverride>,
}

/// Module-root keyed build-option overrides evaluated by `runtime.js`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(transparent)]
pub struct ModuleHvigorOverrides(BTreeMap<PathBuf, serde_json::Value>);

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HvigorAppOptionOverride {
    #[serde(default)]
    pub vendor: Option<String>,
    #[serde(default)]
    pub version_code: Option<u64>,
    #[serde(default)]
    pub version_name: Option<String>,
    #[serde(default)]
    pub bundle_type: Option<String>,
    #[serde(default)]
    pub bundle_name: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HvigorSigningConfigOverride {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(rename = "type", default)]
    pub config_type: Option<String>,
    #[serde(default)]
    pub material: Option<HvigorSigningMaterialOverride>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HvigorSigningMaterialOverride {
    #[serde(default)]
    pub store_file: Option<String>,
    #[serde(default)]
    pub store_password: Option<String>,
    #[serde(default)]
    pub key_alias: Option<String>,
    #[serde(default)]
    pub key_password: Option<String>,
    #[serde(default)]
    pub sign_alg: Option<String>,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(rename = "certpath", alias = "certPath", default)]
    pub cert_path: Option<String>,
}

impl ProjectHvigorOverrides {
    /// Apply hvigorfile values after build-profile/AppScope resolution.
    ///
    /// Upstream `MergeProfile.getMergedAppOptObj` uses the same precedence,
    /// while `SignUtil.mergeSigningConfig` deep-merges `material`.
    pub fn apply_to(
        &self,
        config: &mut ArkdownConfig,
        source_path: &Path,
    ) -> Result<(), ConfigError> {
        if let Some(app_opt) = self.app_opt.as_ref() {
            app_opt.apply_to(config);
        }
        if let Some(signing_config) = self.signing_config.as_ref() {
            config.product.signing_config = Some(
                HvigorSigningConfigMerger::new(
                    config.product.signing_config.as_ref(),
                    signing_config,
                    &config.project_root,
                    source_path,
                )
                .merge()?,
            );
        }
        Ok(())
    }
}

impl ModuleHvigorOverrides {
    /// Bind runtime-discovered module roots to the normalized module names used
    /// by the Rust build plan. Unknown workspace folders are intentionally not
    /// build inputs, matching hvigor's configured module model.
    pub fn bind_to_modules(&self, config: &ArkdownConfig) -> BTreeMap<String, serde_json::Value> {
        let configured = config
            .modules
            .iter()
            .map(|module| {
                (
                    module
                        .src_path
                        .canonicalize()
                        .unwrap_or_else(|_| module.src_path.clone()),
                    module.name.as_str(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        self.0
            .iter()
            .filter_map(|(root, build_option)| {
                let root = root.canonicalize().unwrap_or_else(|_| root.clone());
                configured
                    .get(&root)
                    .map(|module_name| ((*module_name).to_string(), build_option.clone()))
            })
            .collect()
    }
}

impl HvigorAppOptionOverride {
    fn apply_to(&self, config: &mut ArkdownConfig) {
        if let Some(bundle_name) = self.bundle_name.as_ref() {
            config.product.bundle_name.clone_from(bundle_name);
        }
        if let Some(version_code) = self.version_code {
            config.product.version_code = Some(version_code);
        }
        if let Some(version_name) = self.version_name.as_ref() {
            config.product.version_name = Some(version_name.clone());
        }
        let app_options = &mut config.product.app_options;
        Self::replace(&mut app_options.vendor, self.vendor.as_ref());
        Self::replace(&mut app_options.bundle_type, self.bundle_type.as_ref());
        Self::replace(&mut app_options.icon, self.icon.as_ref());
        Self::replace(&mut app_options.label, self.label.as_ref());
    }

    fn replace(target: &mut Option<String>, value: Option<&String>) {
        if let Some(value) = value {
            *target = Some(value.clone());
        }
    }
}

struct HvigorSigningConfigMerger<'a> {
    base: Option<&'a SigningConfig>,
    overlay: &'a HvigorSigningConfigOverride,
    project_root: &'a Path,
    source_path: &'a Path,
}

impl<'a> HvigorSigningConfigMerger<'a> {
    fn new(
        base: Option<&'a SigningConfig>,
        overlay: &'a HvigorSigningConfigOverride,
        project_root: &'a Path,
        source_path: &'a Path,
    ) -> Self {
        Self {
            base,
            overlay,
            project_root,
            source_path,
        }
    }

    fn merge(&self) -> Result<SigningConfig, ConfigError> {
        let material = self.overlay.material.as_ref();
        Ok(SigningConfig {
            cert_path: self.required_path(
                "material.certpath",
                material.and_then(|value| value.cert_path.as_deref()),
                self.base.map(|value| value.cert_path.as_path()),
            )?,
            key_alias: self.required_string(
                "material.keyAlias",
                material.and_then(|value| value.key_alias.as_deref()),
                self.base.map(|value| value.key_alias.as_str()),
            )?,
            key_password: self.required_string(
                "material.keyPassword",
                material.and_then(|value| value.key_password.as_deref()),
                self.base.map(|value| value.key_password.as_str()),
            )?,
            profile_path: self.required_path(
                "material.profile",
                material.and_then(|value| value.profile.as_deref()),
                self.base.map(|value| value.profile_path.as_path()),
            )?,
            sign_alg: self.required_string(
                "material.signAlg",
                material.and_then(|value| value.sign_alg.as_deref()),
                self.base.map(|value| value.sign_alg.as_str()),
            )?,
            store_file: self.required_path(
                "material.storeFile",
                material.and_then(|value| value.store_file.as_deref()),
                self.base.map(|value| value.store_file.as_path()),
            )?,
            store_password: self.required_string(
                "material.storePassword",
                material.and_then(|value| value.store_password.as_deref()),
                self.base.map(|value| value.store_password.as_str()),
            )?,
        })
    }

    fn required_string(
        &self,
        field: &str,
        overlay: Option<&str>,
        base: Option<&str>,
    ) -> Result<String, ConfigError> {
        overlay
            .or(base)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .ok_or_else(|| self.missing_field(field))
    }

    fn required_path(
        &self,
        field: &str,
        overlay: Option<&str>,
        base: Option<&Path>,
    ) -> Result<PathBuf, ConfigError> {
        if let Some(value) = overlay.filter(|value| !value.is_empty()) {
            let path = PathBuf::from(value);
            return Ok(if path.is_absolute() {
                path
            } else {
                self.project_root.join(path)
            });
        }
        base.filter(|path| !path.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .ok_or_else(|| self.missing_field(field))
    }

    fn missing_field(&self, field: &str) -> ConfigError {
        ConfigError::ValidationError {
            file: self.source_path.to_path_buf(),
            message: format!(
                "config.ohos.overrides.signingConfig.{field} is required after merging the selected product signing config"
            ),
        }
    }
}

impl From<&HvigorAppOptionOverride> for AppOptionOverride {
    fn from(value: &HvigorAppOptionOverride) -> Self {
        Self {
            vendor: value.vendor.clone(),
            bundle_type: value.bundle_type.clone(),
            icon: value.icon.clone(),
            label: value.label.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use arkdown_core::{
        BuildMode, ModuleConfig, OhosModuleType, OutputTarget, ProductConfig, SdkConfig,
    };

    use super::*;

    fn config(project_root: &Path) -> ArkdownConfig {
        let source_path = project_root.join("build-profile.json5");
        let build_option =
            crate::build_profile::BuildOptionDecoder::new(None, project_root, &source_path)
                .decode()
                .unwrap();
        ArkdownConfig {
            project_root: project_root.to_path_buf(),
            build_mode: BuildMode::Debug,
            output_target: OutputTarget::Hap,
            product: ProductConfig {
                name: "default".to_string(),
                bundle_name: "com.example.product".to_string(),
                compatible_sdk_version: "12".to_string(),
                compatible_sdk_version_stage: None,
                target_sdk_version: None,
                runtime_os: "HarmonyOS".to_string(),
                signing_config: None,
                build_option,
                version_code: Some(100),
                version_name: Some("1.0.0".to_string()),
                build_version: None,
                app_options: AppOptionOverride {
                    vendor: Some("product-vendor".to_string()),
                    ..AppOptionOverride::default()
                },
            },
            modules: vec![],
            sdk_config: SdkConfig {
                ets_loader_path: project_root.join("sdk/ets-loader"),
                es2abc_path: project_root.join("sdk/es2abc"),
                restool_path: project_root.join("sdk/restool"),
                compile_api_version: 12,
                sdk_component_api_version: 12,
                has_rollup_plugin: true,
                compile_sdk_version: "6.0.0.47".to_string(),
            },
        }
    }

    #[test]
    fn project_app_opt_overrides_product_at_highest_precedence() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = config(temp.path());
        let overrides: ProjectHvigorOverrides = serde_json::from_value(serde_json::json!({
            "appOpt": {
                "vendor": "script-vendor",
                "versionCode": 200,
                "versionName": "2.0.0",
                "bundleType": "atomicService",
                "bundleName": "com.example.script",
                "icon": "$media:script",
                "label": "$string:script"
            }
        }))
        .unwrap();

        overrides
            .apply_to(&mut config, &temp.path().join("hvigorfile.ts"))
            .unwrap();

        assert_eq!(config.product.bundle_name, "com.example.script");
        assert_eq!(config.product.version_code, Some(200));
        assert_eq!(config.product.version_name.as_deref(), Some("2.0.0"));
        assert_eq!(
            config.product.app_options,
            AppOptionOverride {
                vendor: Some("script-vendor".to_string()),
                bundle_type: Some("atomicService".to_string()),
                icon: Some("$media:script".to_string()),
                label: Some("$string:script".to_string()),
            }
        );
    }

    #[test]
    fn project_signing_override_deep_merges_product_material() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = config(temp.path());
        config.product.signing_config = Some(SigningConfig {
            cert_path: PathBuf::from("/base/app.cer"),
            key_alias: "base-key".to_string(),
            key_password: "base-key-password".to_string(),
            profile_path: PathBuf::from("/base/app.p7b"),
            sign_alg: "SHA256withECDSA".to_string(),
            store_file: PathBuf::from("/base/app.p12"),
            store_password: "base-store-password".to_string(),
        });
        let overrides: ProjectHvigorOverrides = serde_json::from_value(serde_json::json!({
            "signingConfig": {
                "name": "script",
                "type": "HarmonyOS",
                "material": {
                    "keyAlias": "script-key",
                    "storeFile": "signing/script.p12"
                }
            }
        }))
        .unwrap();

        overrides
            .apply_to(&mut config, &temp.path().join("hvigorfile.ts"))
            .unwrap();

        let signing = config.product.signing_config.unwrap();
        assert_eq!(signing.key_alias, "script-key");
        assert_eq!(signing.store_file, temp.path().join("signing/script.p12"));
        assert_eq!(signing.cert_path, PathBuf::from("/base/app.cer"));
        assert_eq!(signing.profile_path, PathBuf::from("/base/app.p7b"));
        assert_eq!(signing.key_password, "base-key-password");
    }

    #[test]
    fn standalone_partial_signing_override_is_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = config(temp.path());
        let overrides: ProjectHvigorOverrides = serde_json::from_value(serde_json::json!({
            "signingConfig": {
                "material": { "keyAlias": "only-key" }
            }
        }))
        .unwrap();

        let error = overrides
            .apply_to(&mut config, &temp.path().join("hvigorfile.ts"))
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("signingConfig.material.certpath is required"));
    }

    #[test]
    fn module_overrides_bind_absolute_roots_to_configured_module_names() {
        let temp = tempfile::tempdir().unwrap();
        let entry = temp.path().join("entry");
        let unrelated = temp.path().join("tools");
        std::fs::create_dir_all(&entry).unwrap();
        std::fs::create_dir_all(&unrelated).unwrap();
        let mut config = config(temp.path());
        config.modules.push(ModuleConfig {
            name: "entry".to_string(),
            src_path: entry.clone(),
            module_type: OhosModuleType::Entry,
        });
        let mut raw = serde_json::Map::new();
        raw.insert(
            entry.to_string_lossy().into_owned(),
            serde_json::json!({ "arkOptions": { "hostPGO": true } }),
        );
        raw.insert(
            unrelated.to_string_lossy().into_owned(),
            serde_json::json!({ "arkOptions": { "hostPGO": false } }),
        );
        let overrides: ModuleHvigorOverrides =
            serde_json::from_value(serde_json::Value::Object(raw)).unwrap();

        let bound = overrides.bind_to_modules(&config);
        assert_eq!(bound.len(), 1);
        assert_eq!(bound["entry"]["arkOptions"]["hostPGO"], true);
    }
}
