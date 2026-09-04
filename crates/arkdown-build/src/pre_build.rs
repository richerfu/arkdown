//! hvigor `pre-build.ts` build-time validation.
//!
//! Mirrors the JSON/schema and structural checks hvigor runs before any
//! compile task: schema validation against the SDK `toolchains/modulecheck`
//! schemas (app/module/form/page/startWindow/systemTheme/routerMap/shareConfig/
//! insightIntent/syscap), srcEntry/executable path existence, dynamic-import
//! (`runtimeOnly`) constraints, and API-version gates for options like
//! `useNormalizedOHMUrl` and `autoLazyImport`.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

use rustc_hash::FxHashSet;

use arkdown_core::{ModuleConfig, OhosModuleType};
use arkdown_session::BuildSession;

use crate::layout::ModuleBuildLayout;

/// Runs the pre-build validation set for one module target.
pub struct PreBuildValidator<'a> {
    config: &'a arkdown_core::ArkdownConfig,
    session: &'a BuildSession,
    module: &'a ModuleConfig,
    module_build_option: &'a arkdown_core::BuildOption,
}

impl<'a> PreBuildValidator<'a> {
    pub fn new(
        config: &'a arkdown_core::ArkdownConfig,
        session: &'a BuildSession,
        module: &'a ModuleConfig,
        module_build_option: &'a arkdown_core::BuildOption,
    ) -> Self {
        Self {
            config,
            session,
            module,
            module_build_option,
        }
    }

    pub fn validate(&self) -> Result<()> {
        self.validate_target_es_version()?;
        self.validate_module_src_entry()?;
        self.validate_executable_binary_paths()?;
        self.validate_ui_syntax()?;
        self.validate_duplicate_ability_names()?;
        self.validate_module_schema()?;
        self.validate_app_schema()?;
        self.validate_profile_references()?;
        self.validate_form_sources()?;
        self.validate_dynamic_import()?;
        self.validate_ohm_url_api()?;
        self.validate_auto_lazy_import_api()?;
        self.validate_main_element()?;
        self.validate_mock_config()?;
        self.validate_share_config_uri()?;
        self.validate_hybrid_device_types()?;
        self.validate_syscap_schema()?;
        self.validate_insight_intent_schema()?;
        Ok(())
    }

    fn module_json_path(&self) -> PathBuf {
        self.module.src_path.join("src/main/module.json5")
    }

    fn read_module_json(&self) -> Result<serde_json::Value> {
        let path = self.module_json_path();
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("read module.json5 at {}", path.display()))?;
        json5::from_str(&content).map_err(|e| anyhow!("parse {}: {e}", path.display()))
    }

    fn sdk_schema(&self, file_name: &str) -> Option<PathBuf> {
        self.config
            .sdk_config
            .modulecheck_schema_path_for_runtime(&self.config.product.runtime_os, file_name)
    }

    /// `pre-build.ts::targetESVerisonValidate`: tscConfig.targetESVersion is
    /// only supported on API >= 12.
    fn validate_target_es_version(&self) -> Result<()> {
        // `tscConfig.targetESVersion` is rejected below API 12
        // (`pre-build.ts::targetESVerisonValidate`).
        let explicit = self
            .module_build_option
            .ark_options
            .as_ref()
            .map(|options| options.tsc_config.target_es_version)
            .unwrap_or_default();
        if explicit == arkdown_core::ArkTscTarget::Es2017 && self.api_versions_below(12) {
            return Err(anyhow!(
        "UNSUPPORTED_API_VERSION_TARGET_ES_VERSION: targetESVersion is only supported on API >= 12"
      ));
        }
        Ok(())
    }

    /// `validateModuleSrcEntry`: `module.srcEntry`/`srcEntrance` must exist.
    fn validate_module_src_entry(&self) -> Result<()> {
        let module_json = self.read_module_json()?;
        let Some(module) = module_json.get("module") else {
            return Ok(());
        };
        let entry = module
            .get("srcEntry")
            .and_then(serde_json::Value::as_str)
            .or_else(|| {
                module
                    .get("srcEntrance")
                    .and_then(serde_json::Value::as_str)
            });
        let Some(entry) = entry else {
            return Ok(());
        };
        let source_set_root = self.module.src_path.join("src/main");
        let resolved = if Path::new(entry).is_absolute() {
            PathBuf::from(entry)
        } else {
            source_set_root.join(entry)
        };
        if !resolved.is_file() {
            return Err(anyhow!(
                "MODULE_SRC_ENTRY_NOT_FOUND: {} referenced by {}",
                resolved.display(),
                self.module_json_path().display()
            ));
        }
        Ok(())
    }

    /// `validateExecutableBinaryPaths`: executable paths must be relative,
    /// point to a file, and live under `libs/<abi>`.
    fn validate_executable_binary_paths(&self) -> Result<()> {
        let module_json = self.read_module_json()?;
        let Some(paths) = module_json
            .get("module")
            .and_then(|m| m.get("executableBinaryPaths"))
            .and_then(serde_json::Value::as_array)
        else {
            return Ok(());
        };
        let allowed = ["libs/arm64-v8a", "libs/x86_64", "libs/armeabi-v7a"];
        for path_value in paths {
            let Some(entry) = path_value.get("path").and_then(serde_json::Value::as_str) else {
                continue;
            };
            if Path::new(entry).is_absolute() {
                return Err(anyhow!(
                    "EXECUTABLE_BINARY_PATH_SHOULD_BE_RELATIVE_PATH: {entry}"
                ));
            }
            let resolved = self.module.src_path.join(entry);
            if !resolved.is_file() {
                return Err(anyhow!("EXECUTABLE_BINARY_PATH_FILE_IS_INVALID: {entry}"));
            }
            let relative = resolved
                .strip_prefix(&self.module.src_path)
                .unwrap_or(&resolved);
            let in_allowed = allowed.iter().any(|dir| relative.starts_with(dir));
            if !in_allowed {
                return Err(anyhow!("EXECUTABLE_BINARY_PATH_NOT_FOUND_IN_LIBS: {entry}"));
            }
        }
        Ok(())
    }

    /// `moduleJson5Validate` uiSyntax gate. Hvigor derives this from the actual
    /// ETS component version, not the project API level: versions below 3.2.2.5
    /// require the field, except the 3.2.1.4 compatibility release.
    fn validate_ui_syntax(&self) -> Result<()> {
        let module_json = self.read_module_json()?;
        let ui_syntax = module_json
            .get("module")
            .and_then(|module| module.get("uiSyntax"));
        let requires_ui_syntax = self
            .session
            .sdk
            .compile_plugin()
            .component_metadata()
            .is_some_and(arkdown_session::ComponentMetadata::requires_ui_syntax);
        if ui_syntax.is_none() && requires_ui_syntax {
            return Err(anyhow!(
                "UNSUPPORTED_SDK_VERSION_UI_SYNTAX: {}",
                self.module_json_path().display()
            ));
        }
        Ok(())
    }

    /// `ValidateUtil.validateDuplicatedName(module.abilities, "ability")`.
    fn validate_duplicate_ability_names(&self) -> Result<()> {
        let module_json = self.read_module_json()?;
        let Some(abilities) = module_json
            .get("module")
            .and_then(|m| m.get("abilities"))
            .and_then(serde_json::Value::as_array)
        else {
            return Ok(());
        };
        let mut seen = FxHashSet::default();
        for ability in abilities {
            if let Some(name) = ability.get("name").and_then(serde_json::Value::as_str) {
                if !seen.insert(name.to_string()) {
                    return Err(anyhow!(
                        "ABILITY_NAME_DUPLICATED: duplicate ability name '{name}' in {}",
                        self.module_json_path().display()
                    ));
                }
            }
        }
        Ok(())
    }

    /// `appJson5Validate`: validate `AppScope/app.json5` against the SDK
    /// `app.json` schema when present.
    ///
    /// hvigor passes `changeAppSchema = (moduleType === Har)`, which narrows
    /// the required fields to `bundleName`/`versionCode`/`versionName` — HAR
    /// modules have no app icon/label of their own.
    fn validate_app_schema(&self) -> Result<()> {
        let Some(schema_path) = self.sdk_schema("app.json") else {
            return Ok(());
        };
        let app_path = self.config.project_root.join("AppScope").join("app.json5");
        if !app_path.is_file() {
            return Ok(());
        }
        let is_har = matches!(self.module.module_type, OhosModuleType::Har);
        let mut schema = Self::read_schema(&schema_path)?;
        if is_har {
            if let Some(app) = schema
                .get_mut("properties")
                .and_then(|p| p.get_mut("app"))
                .and_then(|a| a.as_object_mut())
            {
                app.insert(
                    "required".into(),
                    serde_json::json!(["bundleName", "versionCode", "versionName"]),
                );
            }
        }
        let instance = Self::parse_json5_file(&app_path)?;
        validate_schema(&app_path, &instance, &schema, "AppScope/app.json5", false)
    }

    /// `moduleJson5Validate`: validate `module.json5` against the SDK
    /// `module.json` schema. `removeLegacyPhoneSupport` drops deviceTypes
    /// errors for the legacy `"phone"` device.
    fn validate_module_schema(&self) -> Result<()> {
        let Some(schema_path) = self.sdk_schema("module.json") else {
            return Ok(());
        };
        let schema = Self::read_schema(&schema_path)?;
        let instance = self.read_module_json()?;
        let legacy_phone = instance
            .get("module")
            .and_then(|m| m.get("deviceTypes"))
            .and_then(serde_json::Value::as_array)
            .is_some_and(|types| types.iter().any(|t| t.as_str() == Some("phone")));
        validate_schema(
            &self.module_json_path(),
            &instance,
            &schema,
            "module.json5",
            legacy_phone,
        )
    }

    /// `pagesJsonValidate` / `startWindowJsonValidate` /
    /// `systemThemeJsonValidate` / `routerMapJsonValidate` /
    /// `shareConfigJsonValidate` / `checkResReferenceConfig`: referenced
    /// profile files must exist and satisfy their SDK schemas.
    fn validate_profile_references(&self) -> Result<()> {
        let module_json = self.read_module_json()?;
        let Some(module) = module_json.get("module") else {
            return Ok(());
        };
        let profile_base = self.module.src_path.join("src/main/resources/base/profile");

        // pages
        if let Some(pages) = module.get("pages").and_then(serde_json::Value::as_str) {
            self.validate_profile_file(&profile_base, pages, "page.json", "pages")?;
        }
        // systemTheme
        if let Some(theme) = module
            .get("systemTheme")
            .and_then(serde_json::Value::as_str)
        {
            self.validate_profile_file(&profile_base, theme, "systemTheme.json", "systemTheme")?;
        }
        // startWindow per ability
        if let Some(abilities) = module
            .get("abilities")
            .and_then(serde_json::Value::as_array)
        {
            for ability in abilities {
                if let Some(window) = ability
                    .get("startWindow")
                    .and_then(serde_json::Value::as_str)
                {
                    self.validate_profile_file(
                        &profile_base,
                        window,
                        "startWindow.json",
                        "startWindow",
                    )?;
                }
            }
        }
        // routerMap
        if let Some(router) = module.get("routerMap").and_then(serde_json::Value::as_str) {
            self.validate_profile_file(&profile_base, router, "routerMap.json", "routerMap")?;
        }
        // shareConfig
        if let Some(share) = module
            .get("shareConfig")
            .and_then(serde_json::Value::as_str)
        {
            self.validate_profile_file(&profile_base, share, "shareConfig.json", "shareConfig")?;
        }
        // appStartup
        if let Some(startup) = module.get("appStartup").and_then(serde_json::Value::as_str) {
            self.validate_profile_file(&profile_base, startup, "appStartup.json", "appStartup")?;
        }
        Ok(())
    }

    fn validate_profile_file(
        &self,
        profile_base: &Path,
        reference: &str,
        schema_file: &str,
        field: &str,
    ) -> Result<()> {
        let Some(file_name) = profile_file_name(reference) else {
            return Ok(());
        };
        let profile_path = profile_base.join(file_name);
        if !profile_path.is_file() {
            return Err(anyhow!(
                "FIELD_FILE_NOT_FOUND: {field} references '{}' which does not exist",
                profile_path.display()
            ));
        }
        let Some(schema_path) = self.sdk_schema(schema_file) else {
            return Ok(());
        };
        let schema = Self::read_schema(&schema_path)?;
        let instance = Self::parse_json5_file(&profile_path)?;
        validate_schema(&profile_path, &instance, &schema, field, false)
    }

    /// `validateFormSrc`: every form entry must have `src`, the resolved path
    /// must stay inside the module, and the file must exist.
    fn validate_form_sources(&self) -> Result<()> {
        for form_path in self.collect_form_config_paths() {
            if !form_path.is_file() {
                continue;
            }
            let instance = Self::parse_json5_file(&form_path)?;
            if let Some(schema_path) = self.sdk_schema("form.json") {
                let schema = Self::read_schema(&schema_path)?;
                validate_schema(&form_path, &instance, &schema, "form_config", false)?;
            }
            let Some(forms) = instance.get("forms").and_then(serde_json::Value::as_array) else {
                continue;
            };
            let module_root = self.module.src_path.clone();
            for form in forms {
                let Some(src) = form.get("src").and_then(serde_json::Value::as_str) else {
                    return Err(anyhow!(
                        "FORM_SRC_FIELD_MISSING: form entry in {} lacks 'src'",
                        form_path.display()
                    ));
                };
                let resolved = module_root.join("src/main").join(src);
                let normalized = lexical_normalize(&resolved);
                if !normalized.starts_with(lexical_normalize(&module_root)) {
                    return Err(anyhow!(
                        "FORM_SRC_INVALID: '{}' escapes the module root in {}",
                        src,
                        form_path.display()
                    ));
                }
                if !normalized.is_file() {
                    return Err(anyhow!(
                        "FORM_SRC_NOT_FOUND: '{}' referenced by {}",
                        src,
                        form_path.display()
                    ));
                }
            }
        }
        Ok(())
    }

    fn collect_form_config_paths(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        let profile_dir = self.module.src_path.join("src/main/resources/base/profile");
        let Ok(entries) = std::fs::read_dir(&profile_dir) else {
            return paths;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json")
                && path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .is_some_and(|stem| stem.contains("form"))
            {
                paths.push(path);
            }
        }
        paths
    }

    /// `checkDynamicImportFile`: runtimeOnly sources/packages constraints.
    fn validate_dynamic_import(&self) -> Result<()> {
        let Some(runtime_only) = self
            .module_build_option
            .ark_options
            .as_ref()
            .and_then(|options| options.runtime_only.as_ref())
        else {
            return Ok(());
        };
        if self.api_versions_below(11) {
            return Err(anyhow!(
                "UNSUPPORTED_API_VERSION_DYNAMIC_IMPORT: runtimeOnly requires API >= 11"
            ));
        }
        let sources = &runtime_only.sources;
        let module_root = self.module.src_path.clone();
        let mut missing = Vec::new();
        let mut cross_module = Vec::new();
        let mut bad_suffix = Vec::new();
        for source in sources {
            let resolved = module_root.join(source);
            if !resolved.exists() {
                missing.push(source.clone());
            } else if resolved.is_file()
                && !["ets", "ts"]
                    .iter()
                    .any(|ext| source.ends_with(&format!(".{ext}")))
            {
                bad_suffix.push(source.clone());
            }
            if !resolved.starts_with(&module_root) {
                cross_module.push(source.clone());
            }
        }
        if !missing.is_empty() {
            return Err(anyhow!(
                "SOURCES_NOT_FOUND: runtimeOnly.sources {} do not exist in module '{}'",
                missing.join(", "),
                self.module.name
            ));
        }
        if !bad_suffix.is_empty() {
            return Err(anyhow!(
                "SOURCES_SUFFIX_INCORRECT: {} must be .ets/.ts files",
                bad_suffix.join(", ")
            ));
        }
        if !cross_module.is_empty() {
            return Err(anyhow!(
                "DYNAMIC_IMPORT_ACROSS_MODULE_PROHIBITED: {} in module '{}'",
                cross_module.join(", "),
                self.module.name
            ));
        }
        Ok(())
    }

    /// `checkOHMURL`: useNormalizedOHMUrl requires API >= 11.
    fn validate_ohm_url_api(&self) -> Result<()> {
        if self.module_build_option.strict_mode.use_normalized_ohm_url
            && self.api_versions_below(11)
        {
            return Err(anyhow!(
        "USENORMAILZEDOHMURL_CAN_ONLY_BE_USED_IN_API12: useNormalizedOHMUrl requires API >= 11"
      ));
        }
        Ok(())
    }

    /// `checkSdkVersionMatchLazyImport`: autoLazyImport requires API >= 12
    /// (excluding early beta stages).
    fn validate_auto_lazy_import_api(&self) -> Result<()> {
        let auto_lazy = self
            .module_build_option
            .ark_options
            .as_ref()
            .and_then(|options| options.auto_lazy_import)
            .unwrap_or(false);
        if auto_lazy && self.api_versions_below(12) {
            return Err(anyhow!(
                "AUTO_LAZY_IMPORT_CANNOT_BE_CONFIGURED_FOR_THE_CURRENT_API_VERSION: autoLazyImport requires API >= 12"
            ));
        }
        Ok(())
    }

    /// `validateMainElement`: installationFree modules need mainElement.
    fn validate_main_element(&self) -> Result<()> {
        let module_json = self.read_module_json()?;
        let Some(module) = module_json.get("module") else {
            return Ok(());
        };
        let installation_free = module
            .get("installationFree")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let has_main_element = module.get("mainElement").is_some();
        if installation_free && !has_main_element {
            return Err(anyhow!(
                "EMPTY_MAIN_ELEMENT: installationFree module must declare mainElement in {}",
                self.module_json_path().display()
            ));
        }
        Ok(())
    }

    /// `validateMockConfig`: mock-config.json5 `source` entries must exist.
    fn validate_mock_config(&self) -> Result<()> {
        // `CoreModuleModelImpl.getMockConfigPath()` owns the fixed
        // `<module>/src/mock/mock-config.json5` location.
        let mock_path = ModuleBuildLayout::new(&self.module.src_path)
            .source()
            .mock_config_json5();
        if !mock_path.is_file() {
            return Ok(());
        }
        let instance = Self::parse_json5_file(&mock_path)?;
        if let Some(object) = instance.as_object() {
            for (_, value) in object {
                let Some(source) = value.get("source").and_then(serde_json::Value::as_str) else {
                    return Err(anyhow!(
                        "SOURCE_FILED_EMPTY: mock entry in {} lacks 'source'",
                        mock_path.display()
                    ));
                };
                let resolved = self.module.src_path.join(source);
                if !resolved.exists() {
                    return Err(anyhow!(
                        "SOURCE_NOT_FOUND: mock source '{}' does not exist",
                        resolved.display()
                    ));
                }
            }
        }
        Ok(())
    }

    /// `checkShareConfigUri`: duplicate share-config URIs are hard errors.
    fn validate_share_config_uri(&self) -> Result<()> {
        let share_dir = self.module.src_path.join("src/main/resources/base/profile");
        let Ok(entries) = std::fs::read_dir(&share_dir) else {
            return Ok(());
        };
        let mut uris = FxHashSet::default();
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if !name.contains("share") {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(value) = json5::from_str::<serde_json::Value>(&content) else {
                continue;
            };
            let Some(configs) = value
                .get("crossAppSharedConfig")
                .and_then(serde_json::Value::as_array)
            else {
                continue;
            };
            for config in configs {
                if let Some(uri) = config.get("uri").and_then(serde_json::Value::as_str) {
                    if !uris.insert(uri.to_string()) {
                        return Err(anyhow!(
                            "SHARE_CONFIG_URI_UNIQUE: duplicate share config uri '{uri}'"
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    /// `PreCheckSyscap::doTaskAction`: pre-check the module `syscap.json`
    /// against `module.json5#module.deviceTypes` for OpenHarmony targets
    /// (skipped for HarmonyOS, like hvigor's `isHarmonyOS()` gate).
    /// `JSON_PROFILE_MISSING` when deviceTypes is empty and `src/main/syscap.json`
    /// is absent; `SYSCAP_FIELD_NOT_EMPTY` when deviceTypes is empty but
    /// `devices.general` is non-empty; `DEVICE_TYPE_NOT_INCONSISTENT` when
    /// `devices.general` differs from deviceTypes (element-wise after sort,
    /// `array-util.js::checkArrayElementIsSame`).
    pub fn validate_pre_check_syscap(&self) -> Result<()> {
        if arkdown_core::is_harmony_os(&self.config.product.runtime_os) {
            return Ok(());
        }
        let module_json = self.read_module_json()?;
        let device_types = Self::module_device_types(&module_json);
        let syscap_path = self.module.src_path.join("src/main/syscap.json");
        if device_types.is_empty() && !syscap_path.is_file() {
            return Err(anyhow!(
                "JSON_PROFILE_MISSING: module '{}' declares no deviceTypes in {} and has no {}",
                self.module.name,
                self.module_json_path().display(),
                syscap_path.display()
            ));
        }
        if !syscap_path.is_file() {
            return Ok(());
        }
        let value = Self::parse_json5_file(&syscap_path)?;
        let general = value
            .pointer("/devices/general")
            .and_then(serde_json::Value::as_array)
            .map(|devices| {
                devices
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if device_types.is_empty() {
            if !general.is_empty() {
                return Err(anyhow!(
                    "SYSCAP_FIELD_NOT_EMPTY: module '{}' declares no deviceTypes but {} defines devices.general",
                    self.module.name,
                    syscap_path.display()
                ));
            }
            return Ok(());
        }
        if general.is_empty() {
            return Ok(());
        }
        let mut sorted_general = general.clone();
        sorted_general.sort();
        let mut sorted_types = device_types.clone();
        sorted_types.sort();
        if sorted_general != sorted_types {
            return Err(anyhow!(
                "DEVICE_TYPE_NOT_INCONSISTENT: devices.general {:?} in {} does not match module deviceTypes {:?}",
                general,
                syscap_path.display(),
                device_types
            ));
        }
        Ok(())
    }

    /// `ValidateUtil.validateHybridDeviceTypes`: deviceTypes must not mix rich
    /// (`phone`/`tablet`/`tv`/`wearable`/`car`) and lite (`lite Wearable`/
    /// `smart Vision`/`router`) device types (`INCONSISTENT_DEVICE_TYPES`), and
    /// lite-only configurations may list at most one device type
    /// (`UNEQUAL_MINI_SYSTEM_DEVICE_TYPES`).
    fn validate_hybrid_device_types(&self) -> Result<()> {
        let module_json = self.read_module_json()?;
        let device_types = Self::module_device_types(&module_json);
        const RICH: &[&str] = &["phone", "tablet", "tv", "wearable", "car"];
        const LITE: &[&str] = &["lite Wearable", "smart Vision", "router"];
        let has_rich = device_types
            .iter()
            .any(|device| RICH.contains(&device.as_str()));
        let has_lite = device_types
            .iter()
            .any(|device| LITE.contains(&device.as_str()));
        if has_rich && has_lite {
            return Err(anyhow!(
                "INCONSISTENT_DEVICE_TYPES: module '{}' mixes rich and lite device types {:?}",
                self.module.name,
                device_types
            ));
        }
        let all_lite = !device_types.is_empty()
            && device_types
                .iter()
                .all(|device| LITE.contains(&device.as_str()));
        if all_lite && device_types.len() > 1 {
            return Err(anyhow!(
        "UNEQUAL_MINI_SYSTEM_DEVICE_TYPES: module '{}' lists multiple lite device types {:?}",
        self.module.name,
        device_types
      ));
        }
        Ok(())
    }

    /// Raw `module.json5#module.deviceTypes` (hvigor reads the same profile
    /// field in both `pre-build.ts` and `pre-check-syscap.ts`).
    fn module_device_types(module_json: &serde_json::Value) -> Vec<String> {
        module_json
            .get("module")
            .and_then(|module| module.get("deviceTypes"))
            .and_then(serde_json::Value::as_array)
            .map(|types| {
                types
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// `sysCapValidate`: the `syscap.json` profile must satisfy the SDK
    /// syscap schema for non-HarmonyOS targets.
    fn validate_syscap_schema(&self) -> Result<()> {
        if arkdown_core::is_harmony_os(&self.config.product.runtime_os) {
            return Ok(());
        }
        let syscap_dir = self.module.src_path.join("src/main/resources/base/profile");
        let syscap_path = syscap_dir.join("syscap.json");
        if !syscap_path.is_file() {
            return Ok(());
        }
        let Some(schema_path) = self.sdk_schema("syscap.json") else {
            return Ok(());
        };
        let schema = Self::read_schema(&schema_path)?;
        let instance = Self::parse_json5_file(&syscap_path)?;
        validate_schema(&syscap_path, &instance, &schema, "syscap.json", false)
    }

    /// `insightIntentJsonValidate` + `buildProfileJsonValidate`: the
    /// insightIntent.json profile must exist and satisfy the SDK schema
    /// (API >= 11).
    fn validate_insight_intent_schema(&self) -> Result<()> {
        if self.api_versions_below(11) {
            return Ok(());
        }
        let profile_dir = self.module.src_path.join("src/main/resources/base/profile");
        let insight_path = profile_dir.join("insightIntent.json");
        if !insight_path.is_file() {
            return Ok(());
        }
        let Some(schema_path) = self.sdk_schema("insightIntent.json") else {
            return Ok(());
        };
        let schema = Self::read_schema(&schema_path)?;
        let instance = Self::parse_json5_file(&insight_path)?;
        validate_schema(
            &insight_path,
            &instance,
            &schema,
            "insightIntent.json",
            false,
        )
    }

    fn api_versions_below(&self, version: u32) -> bool {
        self.config.sdk_config.compile_api_version < version
            || self.session.sdk.api_version < version
    }

    fn read_schema(path: &Path) -> Result<serde_json::Value> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("read schema {}", path.display()))?;
        serde_json::from_str(&content).map_err(|e| anyhow!("parse schema {}: {e}", path.display()))
    }

    fn parse_json5_file(path: &Path) -> Result<serde_json::Value> {
        let content =
            std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        json5::from_str(&content).map_err(|e| anyhow!("parse {}: {e}", path.display()))
    }
}

/// Validate `instance` against the JSON Schema at `schema`.
fn validate_schema(
    file: &Path,
    instance: &serde_json::Value,
    schema: &serde_json::Value,
    field: &str,
    legacy_phone: bool,
) -> Result<()> {
    let validator = jsonschema::validator_for(schema)
        .map_err(|e| anyhow!("invalid SDK schema for {field}: {e}"))?;
    if let Err(error) = validator.validate(instance) {
        // `removeLegacyPhoneSupport`: hvigor filters deviceTypes errors when
        // the module still lists the legacy "phone" device.
        if legacy_phone && error.instance_path().to_string().contains("deviceTypes") {
            return Ok(());
        }
        return Err(anyhow!(
            "SCHEMA_VALIDATE_FAILED: {field} in {}: {}",
            file.display(),
            error
        ));
    }
    Ok(())
}

fn profile_file_name(reference: &str) -> Option<String> {
    let name = reference.trim_start_matches("$profile:");
    if name.is_empty() || name == reference {
        return None;
    }
    let name = name.trim_end_matches(".json");
    Some(format!("{name}.json"))
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}
