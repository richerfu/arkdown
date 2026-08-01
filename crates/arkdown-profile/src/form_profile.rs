//! Form profile discovery and source validation.
//!
//! Mirrors hvigor `TargetTaskService.getFormJsonArr()` and the pre-build
//! `validateFormSrc()` pass for stage model form extension abilities.

use indexmap::IndexMap;
use std::path::{Component, Path, PathBuf};

use crate::layout::{LegacyFormSourceLayout, ProfileModuleLayout};
use crate::resource_ref::ProfileResourceRef;
use arkdown_core::{
    ArkdownError, ExtensionAbilityEntry, ModuleConfig, ModuleJson5, OhosModuleType,
};

const FORM_METADATA_NAME: &str = "ohos.extension.form";

/// Input for form profile collection.
pub struct FormProfileInput<'a> {
    /// Module root, e.g. `/project/entry`.
    pub module_root: &'a Path,
    /// The module.json5 path used in diagnostics.
    pub module_json_path: Option<&'a Path>,
    /// Processed `extensionAbilities` entries from `module.json5`.
    pub extension_abilities: &'a [ExtensionAbilityEntry],
}

/// Project-aware form profile collection input.
///
/// This handles the hvigor form module pairing fields:
/// - HAP `formWidgetModule` points profile/source lookup at the HSP module.
/// - HSP `formExtensionModule` imports form extension metadata from the HAP module.
pub struct ModuleFormProfileInput<'a> {
    pub module_name: &'a str,
    pub module_root: &'a Path,
    pub module_type: &'a OhosModuleType,
    pub module_json_path: &'a Path,
    pub extension_abilities: &'a [ExtensionAbilityEntry],
    pub project_modules: &'a [ModuleConfig],
    pub compatible_api_version: Option<u32>,
}

/// One profile JSON referenced by a form extension ability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormProfileEntry {
    pub resource: String,
    pub profile_path: PathBuf,
}

/// One `forms[]` source entry from a referenced form profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormSourceEntry {
    pub profile_path: PathBuf,
    pub src: String,
    pub source_path: PathBuf,
    pub ui_syntax: Option<String>,
}

impl FormSourceEntry {
    pub fn is_arkts(&self) -> bool {
        self.ui_syntax.as_deref() == Some("arkts")
    }
}

#[derive(Debug, serde::Deserialize)]
struct FormConfigFile {
    #[serde(default)]
    forms: Vec<FormConfigEntry>,
}

#[derive(Debug, serde::Deserialize)]
struct FormConfigEntry {
    src: Option<String>,
    #[serde(rename = "uiSyntax")]
    ui_syntax: Option<String>,
}

/// Processes form profiles for a module.
///
/// Mirrors hvigor `TargetTaskService.getFormJsonArr()` and the pre-build
/// `validateFormSrc()` pass for stage model form extension abilities.
pub struct FormProfileProcessor;

impl FormProfileProcessor {
    /// Create a new processor.
    pub fn new() -> Self {
        FormProfileProcessor
    }

    /// Collect profile files referenced by form extension metadata.
    pub fn collect_form_profiles(
        &self,
        input: FormProfileInput<'_>,
    ) -> Result<Vec<FormProfileEntry>, ArkdownError> {
        let module_layout = ProfileModuleLayout::new(input.module_root);
        let module_json = Self::module_json_label(input.module_json_path);
        let mut profiles = Vec::new();

        for ability in input.extension_abilities {
            if ability.extension_type != "form" {
                continue;
            }
            if ability.metadata.is_empty() {
                return Err(ArkdownError::Bundler(format!(
                    "THE_METADATE_FIELD_IN_FORMEXTENSIONABILITY_CANNOT_BE_LEFT_BLANK_OR_AS_AN_EMPTY_ARRAY: {module_json}"
                )));
            }

            let has_form_metadata_resource = ability
                .metadata
                .iter()
                .any(|metadata| metadata.name == FORM_METADATA_NAME && metadata.resource.is_some());
            if !has_form_metadata_resource {
                return Err(ArkdownError::Bundler(format!(
                    "IN_FORMEXTENSIONABILITY_METADATA_MUST_CONTAIN_AN_OBJECT_WITH_ITS_NAME: {module_json}"
                )));
            }

            for metadata in &ability.metadata {
                let Some(resource) = metadata.resource.as_deref() else {
                    continue;
                };
                let Some(profile_ref) = ProfileResourceRef::parse(resource) else {
                    return Err(ArkdownError::Bundler(format!(
                        "CAN_NOT_RESOLVE_SYMBOL: {resource} in {module_json}; expected $profile:<name>"
                    )));
                };
                let profile_path = module_layout.profile_json(profile_ref.name());
                if !profile_path.exists() {
                    return Err(ArkdownError::Bundler(format!(
                        "CAN_NOT_RESOLVE_SYMBOL: {resource} in {module_json}; expected {}",
                        profile_path.display()
                    )));
                }
                profiles.push(FormProfileEntry {
                    resource: resource.to_string(),
                    profile_path,
                });
            }
        }

        Ok(profiles)
    }

    /// Collect and validate `forms[].src` entries from form profiles.
    pub fn collect_form_sources(
        &self,
        input: FormProfileInput<'_>,
    ) -> Result<Vec<FormSourceEntry>, ArkdownError> {
        let module_root = input.module_root;
        let profiles = self.collect_form_profiles(input)?;
        let mut sources = Vec::new();

        for profile in profiles {
            let content = fs_err::read_to_string(&profile.profile_path)?;
            let form_config: FormConfigFile =
                json5::from_str(&content).map_err(|err| ArkdownError::ParseError {
                    file: profile.profile_path.display().to_string(),
                    message: err.to_string(),
                })?;

            for form in form_config.forms {
                let Some(src) = form.src else {
                    return Err(ArkdownError::Bundler(format!(
                        "FORM_SRC_FIELD_MISSING: {}",
                        profile.profile_path.display()
                    )));
                };
                let source_path = Self::form_source_path(module_root, &src);
                Self::validate_form_source(
                    module_root,
                    &profile.profile_path,
                    &src,
                    &source_path,
                    &form.ui_syntax,
                )?;
                sources.push(FormSourceEntry {
                    profile_path: profile.profile_path.clone(),
                    src,
                    source_path,
                    ui_syntax: form.ui_syntax,
                });
            }
        }

        Ok(sources)
    }

    /// Collect and validate form sources using project-level form module mappings.
    pub fn collect_module_form_sources(
        &self,
        input: ModuleFormProfileInput<'_>,
    ) -> Result<Vec<FormSourceEntry>, ArkdownError> {
        let current_module_json = match Self::read_module_json5(input.module_json_path) {
            Ok(module_json) => Some(module_json),
            Err(ArkdownError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error),
        };
        let mut resource_module_root = input.module_root.to_path_buf();
        let mut module_json_path_for_diagnostics = input.module_json_path.to_path_buf();
        let mut extension_abilities = input.extension_abilities.to_vec();

        if Self::is_hap_module(input.module_type) {
            if current_module_json.is_some()
                && Self::project_has_form_module_pair(input.project_modules)
            {
                Self::validate_no_hap_form_config(input.project_modules)?;
            }
            if let Some(widget_module_name) = current_module_json
                .as_ref()
                .and_then(|module_json| module_json.module.form_widget_module.as_deref())
            {
                Self::validate_form_module_api_version(
                    input.compatible_api_version,
                    input.module_json_path,
                )?;
                let widget_module = Self::find_project_module(
                    input.project_modules,
                    widget_module_name,
                    input.module_json_path,
                )?;
                if widget_module.module_type != OhosModuleType::Shared {
                    return Err(ArkdownError::Bundler(format!(
                        "FORM_WIDGET_MODULE_INVALID_TYPE: {widget_module_name} in {}",
                        input.module_json_path.display()
                    )));
                }
                let widget_json_path =
                    ProfileModuleLayout::new(&widget_module.src_path).module_json5();
                let widget_json = Self::read_module_json5(&widget_json_path)?;
                if widget_json.module.form_extension_module.is_none() {
                    return Err(ArkdownError::Bundler(format!(
                        "FORM_WIDGET_MODULE_INVALID_TYPE: {widget_module_name} in {}",
                        input.module_json_path.display()
                    )));
                }
                if widget_json.module.form_extension_module.as_deref() != Some(input.module_name) {
                    return Err(ArkdownError::Bundler(format!(
                        "FORM_MODULE_INVALID_MAPPING: {widget_module_name} must map back to {} in {}",
                        input.module_name,
                        widget_json_path.display()
                    )));
                }
                resource_module_root = widget_module.src_path.clone();
            }
        } else if input.module_type == &OhosModuleType::Shared {
            if let Some(extension_module_name) = current_module_json
                .as_ref()
                .and_then(|module_json| module_json.module.form_extension_module.as_deref())
            {
                Self::validate_form_module_api_version(
                    input.compatible_api_version,
                    input.module_json_path,
                )?;
                let extension_module = Self::find_project_module(
                    input.project_modules,
                    extension_module_name,
                    input.module_json_path,
                )?;
                if !Self::is_hap_module(&extension_module.module_type) {
                    return Err(ArkdownError::Bundler(format!(
                        "FORM_EXTENSION_MODULE_INVALID_TYPE: {extension_module_name} in {}",
                        input.module_json_path.display()
                    )));
                }
                let extension_json_path =
                    ProfileModuleLayout::new(&extension_module.src_path).module_json5();
                let extension_json = Self::read_module_json5(&extension_json_path)?;
                if extension_json.module.form_widget_module.is_none() {
                    return Err(ArkdownError::Bundler(format!(
                        "FORM_EXTENSION_MODULE_INVALID_TYPE: {extension_module_name} in {}",
                        input.module_json_path.display()
                    )));
                }
                if extension_json.module.form_widget_module.as_deref() != Some(input.module_name) {
                    return Err(ArkdownError::Bundler(format!(
                        "FORM_MODULE_INVALID_MAPPING: {extension_module_name} must map back to {} in {}",
                        input.module_name,
                        extension_json_path.display()
                    )));
                }
                module_json_path_for_diagnostics = extension_json_path.clone();
                extension_abilities.extend(extension_json.module.extension_abilities.iter().map(
                    |ability| ExtensionAbilityEntry {
                        name: ability.name.clone(),
                        src_entry: String::new(),
                        extension_type: ability.extension_type.clone(),
                        label: ability.label.clone(),
                        description: ability.description.clone(),
                        metadata: ability.metadata.clone(),
                    },
                ));
                Self::validate_no_hap_form_config(input.project_modules)?;
                Self::validate_no_hsp_form_widget_dependencies(
                    input.module_root,
                    input.project_modules,
                )?;
                Self::validate_delivery_with_install(&extension_json, &extension_json_path)?;
                Self::validate_delivery_with_install(
                    current_module_json.as_ref().unwrap(),
                    input.module_json_path,
                )?;
            }
        }

        self.collect_form_sources(FormProfileInput {
            module_root: &resource_module_root,
            module_json_path: Some(&module_json_path_for_diagnostics),
            extension_abilities: &extension_abilities,
        })
    }
}

impl Default for FormProfileProcessor {
    fn default() -> Self {
        FormProfileProcessor::new()
    }
}

impl FormProfileProcessor {
    fn validate_form_module_api_version(
        compatible_api_version: Option<u32>,
        module_json_path: &Path,
    ) -> Result<(), ArkdownError> {
        if compatible_api_version.is_some_and(|version| version < 20) {
            return Err(ArkdownError::Bundler(format!(
                "FORM_MODULE_INVALID_API_VERSION: requires API 20 or later in {}",
                module_json_path.display()
            )));
        }
        Ok(())
    }

    fn validate_delivery_with_install(
        module_json: &ModuleJson5,
        module_json_path: &Path,
    ) -> Result<(), ArkdownError> {
        if module_json.module.delivery_with_install != Some(true) {
            return Err(ArkdownError::Bundler(format!(
                "FORM_MODULE_INVALID_DELIVERY_WITH_INSTALL: {}",
                module_json_path.display()
            )));
        }
        Ok(())
    }

    fn validate_no_hsp_form_widget_dependencies(
        module_root: &Path,
        project_modules: &[ModuleConfig],
    ) -> Result<(), ArkdownError> {
        for dependency_name in Self::read_oh_package_dependency_names(module_root)? {
            if project_modules.iter().any(|module| {
                module.name == dependency_name && module.module_type == OhosModuleType::Shared
            }) {
                return Err(ArkdownError::Bundler(format!(
                    "FORM_WIDGET_MODULE_INVALID_DEPENDENCY: {dependency_name} in {}",
                    ProfileModuleLayout::new(module_root)
                        .oh_package_json5()
                        .display()
                )));
            }
        }
        Ok(())
    }

    fn read_oh_package_dependency_names(module_root: &Path) -> Result<Vec<String>, ArkdownError> {
        let package_path = ProfileModuleLayout::new(module_root).oh_package_json5();
        if !package_path.is_file() {
            return Ok(Vec::new());
        }
        #[derive(serde::Deserialize)]
        struct OhPackage {
            #[serde(default)]
            dependencies: IndexMap<String, serde_json::Value>,
            #[serde(rename = "dynamicDependencies", default)]
            dynamic_dependencies: IndexMap<String, serde_json::Value>,
        }
        let content = fs_err::read_to_string(&package_path)?;
        let package: OhPackage =
            json5::from_str(&content).map_err(|err| ArkdownError::ParseError {
                file: package_path.display().to_string(),
                message: err.to_string(),
            })?;
        Ok(package
            .dependencies
            .keys()
            .chain(package.dynamic_dependencies.keys())
            .cloned()
            .collect())
    }

    fn project_has_form_module_pair(project_modules: &[ModuleConfig]) -> bool {
        project_modules.iter().any(|module| {
            let module_json_path = ProfileModuleLayout::new(&module.src_path).module_json5();
            let Ok(module_json) = Self::read_module_json5(&module_json_path) else {
                return false;
            };
            module_json.module.form_widget_module.is_some()
                || module_json.module.form_extension_module.is_some()
        })
    }

    fn validate_no_hap_form_config(project_modules: &[ModuleConfig]) -> Result<(), ArkdownError> {
        for module in project_modules {
            if !Self::is_hap_module(&module.module_type) {
                continue;
            }
            let Some(profile_path) = Self::first_form_config_profile(&module.src_path)? else {
                continue;
            };
            let module_json_path = ProfileModuleLayout::new(&module.src_path).module_json5();
            return Err(ArkdownError::Bundler(format!(
                "NO_FORM_ABILITY_ALLOWED: {} in {}",
                profile_path.display(),
                module_json_path.display()
            )));
        }
        Ok(())
    }

    fn first_form_config_profile(module_root: &Path) -> Result<Option<PathBuf>, ArkdownError> {
        let profile_dir = ProfileModuleLayout::new(module_root).profile_dir();
        let entries = match fs_err::read_dir(&profile_dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(ArkdownError::Io(error.into())),
        };
        for entry in entries {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let content = fs_err::read_to_string(&path)?;
            let value: serde_json::Value =
                json5::from_str(&content).map_err(|err| ArkdownError::ParseError {
                    file: path.display().to_string(),
                    message: err.to_string(),
                })?;
            if value.get("forms").is_some() {
                return Ok(Some(path));
            }
        }
        Ok(None)
    }

    fn is_hap_module(module_type: &OhosModuleType) -> bool {
        matches!(module_type, OhosModuleType::Entry | OhosModuleType::Feature)
    }

    fn find_project_module<'a>(
        modules: &'a [ModuleConfig],
        name: &str,
        module_json_path: &Path,
    ) -> Result<&'a ModuleConfig, ArkdownError> {
        modules
            .iter()
            .find(|module| module.name == name)
            .ok_or_else(|| {
                ArkdownError::Bundler(format!(
                    "MODULE_NOT_FOUND: {name} in {}",
                    module_json_path.display()
                ))
            })
    }

    fn read_module_json5(path: &Path) -> Result<ModuleJson5, ArkdownError> {
        let content = fs_err::read_to_string(path)?;
        json5::from_str(&content).map_err(|err| ArkdownError::ParseError {
            file: path.display().to_string(),
            message: err.to_string(),
        })
    }

    fn form_source_path(module_root: &Path, src: &str) -> PathBuf {
        let src_path = Path::new(src);
        if src_path.has_root() {
            src_path.to_path_buf()
        } else {
            ProfileModuleLayout::new(module_root).source_from_module_src(src_path)
        }
    }

    fn validate_form_source(
        module_root: &Path,
        profile_path: &Path,
        src: &str,
        source_path: &Path,
        ui_syntax: &Option<String>,
    ) -> Result<(), ArkdownError> {
        let normalized_module_root = Self::normalize_path(module_root);
        let normalized_source_path = Self::normalize_path(source_path);
        if !normalized_source_path.starts_with(&normalized_module_root) {
            return Err(ArkdownError::Bundler(format!(
                "FORM_SRC_INVALID: {src} escapes {} in {}",
                module_root.display(),
                profile_path.display()
            )));
        }

        let source_exists = if ui_syntax.as_deref() == Some("arkts") {
            source_path.exists()
        } else {
            Self::legacy_form_source_exists(source_path)
        };
        if !source_exists {
            return Err(ArkdownError::Bundler(format!(
                "FORM_SRC_NOT_FOUND: {src} in {}",
                profile_path.display()
            )));
        }
        Ok(())
    }

    fn legacy_form_source_exists(source_path: &Path) -> bool {
        let legacy_layout = LegacyFormSourceLayout::new(source_path);
        if legacy_layout.hml_candidate().exists() {
            return true;
        }
        legacy_layout.supervisual_candidate().exists()
    }

    fn normalize_path(path: &Path) -> PathBuf {
        let mut normalized = PathBuf::new();
        for component in path.components() {
            match component {
                Component::CurDir => {}
                Component::ParentDir => {
                    normalized.pop();
                }
                _ => normalized.push(component.as_os_str()),
            }
        }
        normalized
    }

    fn module_json_label(path: Option<&Path>) -> String {
        path.map(|path| path.display().to_string())
            .unwrap_or_else(|| "<module.json5>".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkdown_core::{ExtensionAbilityEntry, MetadataEntry, ModuleConfig, OhosModuleType};

    fn metadata(name: &str, resource: Option<&str>) -> MetadataEntry {
        MetadataEntry {
            name: name.to_string(),
            value: None,
            resource: resource.map(str::to_string),
        }
    }

    fn form_ability(metadata: Vec<MetadataEntry>) -> ExtensionAbilityEntry {
        ExtensionAbilityEntry {
            name: "EntryFormAbility".to_string(),
            src_entry: "./ets/form/EntryFormAbility.ets".to_string(),
            extension_type: "form".to_string(),
            label: None,
            description: None,
            metadata,
        }
    }

    fn input<'a>(
        module_root: &'a Path,
        extension_abilities: &'a [ExtensionAbilityEntry],
    ) -> FormProfileInput<'a> {
        FormProfileInput {
            module_root,
            module_json_path: None,
            extension_abilities,
        }
    }

    fn module_config(name: &str, src_path: PathBuf, module_type: OhosModuleType) -> ModuleConfig {
        ModuleConfig {
            name: name.to_string(),
            src_path,
            module_type,
        }
    }

    fn write_widget_form_files(widget_root: &Path) -> PathBuf {
        let profile_dir = widget_root.join("src/main/resources/base/profile");
        let form_src = widget_root.join("src/main/ets/forms/Card.ets");
        std::fs::create_dir_all(&profile_dir).unwrap();
        std::fs::create_dir_all(form_src.parent().unwrap()).unwrap();
        std::fs::write(&form_src, "export default struct Card {}").unwrap();
        std::fs::write(
            profile_dir.join("form_config.json"),
            r#"{ forms: [{ name: "Card", src: "./ets/forms/Card.ets", uiSyntax: "arkts" }] }"#,
        )
        .unwrap();
        form_src
    }

    #[test]
    fn test_collect_form_sources_uses_declared_profile() {
        let tmp = tempfile::tempdir().unwrap();
        let module_root = tmp.path().join("entry");
        let profile_dir = module_root.join("src/main/resources/base/profile");
        let form_src = module_root.join("src/main/ets/forms/Card.ets");
        std::fs::create_dir_all(&profile_dir).unwrap();
        std::fs::create_dir_all(form_src.parent().unwrap()).unwrap();
        std::fs::write(&form_src, "export default struct Card {}").unwrap();
        std::fs::write(
            profile_dir.join("form_config.json"),
            r#"{ forms: [{ name: "Card", src: "./ets/forms/Card.ets", uiSyntax: "arkts" }] }"#,
        )
        .unwrap();
        std::fs::write(
            profile_dir.join("unused_form.json"),
            r#"{ forms: [{ name: "Unused", src: "./ets/forms/Unused.ets", uiSyntax: "arkts" }] }"#,
        )
        .unwrap();

        let abilities = vec![form_ability(vec![metadata(
            FORM_METADATA_NAME,
            Some("$profile:form_config"),
        )])];
        let sources = FormProfileProcessor::new()
            .collect_form_sources(input(&module_root, &abilities))
            .unwrap();

        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].src, "./ets/forms/Card.ets");
        assert_eq!(sources[0].source_path, form_src);
        assert!(sources[0].is_arkts());
    }

    #[test]
    fn test_collect_form_profiles_rejects_empty_metadata() {
        let tmp = tempfile::tempdir().unwrap();
        let module_root = tmp.path().join("entry");
        let abilities = vec![form_ability(vec![])];

        let error = FormProfileProcessor::new()
            .collect_form_profiles(input(&module_root, &abilities))
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("THE_METADATE_FIELD_IN_FORMEXTENSIONABILITY"));
    }

    #[test]
    fn test_collect_form_profiles_requires_named_form_metadata() {
        let tmp = tempfile::tempdir().unwrap();
        let module_root = tmp.path().join("entry");
        let abilities = vec![form_ability(vec![metadata(
            "ohos.extension.other",
            Some("$profile:form_config"),
        )])];

        let error = FormProfileProcessor::new()
            .collect_form_profiles(input(&module_root, &abilities))
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("IN_FORMEXTENSIONABILITY_METADATA_MUST_CONTAIN_AN_OBJECT_WITH_ITS_NAME"));
    }

    #[test]
    fn test_collect_form_profiles_rejects_missing_profile() {
        let tmp = tempfile::tempdir().unwrap();
        let module_root = tmp.path().join("entry");
        let abilities = vec![form_ability(vec![metadata(
            FORM_METADATA_NAME,
            Some("$profile:missing_form"),
        )])];

        let error = FormProfileProcessor::new()
            .collect_form_profiles(input(&module_root, &abilities))
            .unwrap_err();
        assert!(error.to_string().contains("CAN_NOT_RESOLVE_SYMBOL"));
    }

    #[test]
    fn test_collect_form_sources_rejects_missing_src_field() {
        let tmp = tempfile::tempdir().unwrap();
        let module_root = tmp.path().join("entry");
        let profile_dir = module_root.join("src/main/resources/base/profile");
        std::fs::create_dir_all(&profile_dir).unwrap();
        std::fs::write(profile_dir.join("form_config.json"), r#"{ forms: [{}] }"#).unwrap();
        let abilities = vec![form_ability(vec![metadata(
            FORM_METADATA_NAME,
            Some("$profile:form_config"),
        )])];

        let error = FormProfileProcessor::new()
            .collect_form_sources(input(&module_root, &abilities))
            .unwrap_err();
        assert!(error.to_string().contains("FORM_SRC_FIELD_MISSING"));
    }

    #[test]
    fn test_collect_form_sources_rejects_escaped_src() {
        let tmp = tempfile::tempdir().unwrap();
        let module_root = tmp.path().join("entry");
        let profile_dir = module_root.join("src/main/resources/base/profile");
        std::fs::create_dir_all(&profile_dir).unwrap();
        std::fs::write(
            profile_dir.join("form_config.json"),
            r#"{ forms: [{ src: "../../../outside/Card.ets", uiSyntax: "arkts" }] }"#,
        )
        .unwrap();
        let abilities = vec![form_ability(vec![metadata(
            FORM_METADATA_NAME,
            Some("$profile:form_config"),
        )])];

        let error = FormProfileProcessor::new()
            .collect_form_sources(input(&module_root, &abilities))
            .unwrap_err();
        assert!(error.to_string().contains("FORM_SRC_INVALID"));
    }

    #[test]
    fn test_collect_form_sources_rejects_missing_source_file() {
        let tmp = tempfile::tempdir().unwrap();
        let module_root = tmp.path().join("entry");
        let profile_dir = module_root.join("src/main/resources/base/profile");
        std::fs::create_dir_all(&profile_dir).unwrap();
        std::fs::write(
            profile_dir.join("form_config.json"),
            r#"{ forms: [{ src: "./ets/forms/Missing.ets", uiSyntax: "arkts" }] }"#,
        )
        .unwrap();
        let abilities = vec![form_ability(vec![metadata(
            FORM_METADATA_NAME,
            Some("$profile:form_config"),
        )])];

        let error = FormProfileProcessor::new()
            .collect_form_sources(input(&module_root, &abilities))
            .unwrap_err();
        assert!(error.to_string().contains("FORM_SRC_NOT_FOUND"));
    }

    #[test]
    fn test_collect_module_form_sources_uses_form_widget_module_resources() {
        let tmp = tempfile::tempdir().unwrap();
        let entry_root = tmp.path().join("entry");
        let widget_root = tmp.path().join("widget");
        std::fs::create_dir_all(entry_root.join("src/main")).unwrap();
        std::fs::create_dir_all(widget_root.join("src/main")).unwrap();
        std::fs::write(
            entry_root.join("src/main/module.json5"),
            r#"{
              module: {
                name: "entry",
                type: "entry",
                deliveryWithInstall: true,
                formWidgetModule: "widget",
                extensionAbilities: [{
                  name: "EntryFormAbility",
                  srcEntry: "./ets/form/EntryFormAbility.ets",
                  type: "form",
                  metadata: [{ name: "ohos.extension.form", resource: "$profile:form_config" }]
                }]
              }
            }"#,
        )
        .unwrap();
        std::fs::write(
            widget_root.join("src/main/module.json5"),
            r#"{
              module: {
                name: "widget",
                type: "shared",
                deliveryWithInstall: true,
                formExtensionModule: "entry"
              }
            }"#,
        )
        .unwrap();
        let form_src = write_widget_form_files(&widget_root);
        let abilities = vec![form_ability(vec![metadata(
            FORM_METADATA_NAME,
            Some("$profile:form_config"),
        )])];
        let modules = vec![
            module_config("entry", entry_root.clone(), OhosModuleType::Entry),
            module_config("widget", widget_root.clone(), OhosModuleType::Shared),
        ];

        let sources = FormProfileProcessor::new()
            .collect_module_form_sources(ModuleFormProfileInput {
                module_name: "entry",
                module_root: &entry_root,
                module_type: &OhosModuleType::Entry,
                module_json_path: &entry_root.join("src/main/module.json5"),
                extension_abilities: &abilities,
                project_modules: &modules,
                compatible_api_version: Some(20),
            })
            .unwrap();

        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].source_path, form_src);
    }

    #[test]
    fn test_collect_module_form_sources_merges_form_extension_module_abilities() {
        let tmp = tempfile::tempdir().unwrap();
        let entry_root = tmp.path().join("entry");
        let widget_root = tmp.path().join("widget");
        std::fs::create_dir_all(entry_root.join("src/main")).unwrap();
        std::fs::create_dir_all(widget_root.join("src/main")).unwrap();
        std::fs::write(
            entry_root.join("src/main/module.json5"),
            r#"{
              module: {
                name: "entry",
                type: "entry",
                deliveryWithInstall: true,
                formWidgetModule: "widget",
                extensionAbilities: [{
                  name: "EntryFormAbility",
                  srcEntry: "./ets/form/EntryFormAbility.ets",
                  type: "form",
                  metadata: [{ name: "ohos.extension.form", resource: "$profile:form_config" }]
                }]
              }
            }"#,
        )
        .unwrap();
        std::fs::write(
            widget_root.join("src/main/module.json5"),
            r#"{
              module: {
                name: "widget",
                type: "shared",
                deliveryWithInstall: true,
                formExtensionModule: "entry"
              }
            }"#,
        )
        .unwrap();
        let form_src = write_widget_form_files(&widget_root);
        let modules = vec![
            module_config("entry", entry_root.clone(), OhosModuleType::Entry),
            module_config("widget", widget_root.clone(), OhosModuleType::Shared),
        ];

        let sources = FormProfileProcessor::new()
            .collect_module_form_sources(ModuleFormProfileInput {
                module_name: "widget",
                module_root: &widget_root,
                module_type: &OhosModuleType::Shared,
                module_json_path: &widget_root.join("src/main/module.json5"),
                extension_abilities: &[],
                project_modules: &modules,
                compatible_api_version: Some(20),
            })
            .unwrap();

        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].source_path, form_src);
    }

    #[test]
    fn test_collect_module_form_sources_rejects_api_below_20() {
        let tmp = tempfile::tempdir().unwrap();
        let entry_root = tmp.path().join("entry");
        let widget_root = tmp.path().join("widget");
        std::fs::create_dir_all(entry_root.join("src/main")).unwrap();
        std::fs::create_dir_all(widget_root.join("src/main")).unwrap();
        std::fs::write(
            entry_root.join("src/main/module.json5"),
            r#"{
              module: {
                name: "entry",
                type: "entry",
                deliveryWithInstall: true,
                formWidgetModule: "widget"
              }
            }"#,
        )
        .unwrap();
        std::fs::write(
            widget_root.join("src/main/module.json5"),
            r#"{
              module: {
                name: "widget",
                type: "shared",
                deliveryWithInstall: true,
                formExtensionModule: "entry"
              }
            }"#,
        )
        .unwrap();
        let modules = vec![
            module_config("entry", entry_root.clone(), OhosModuleType::Entry),
            module_config("widget", widget_root, OhosModuleType::Shared),
        ];

        let error = FormProfileProcessor::new()
            .collect_module_form_sources(ModuleFormProfileInput {
                module_name: "entry",
                module_root: &entry_root,
                module_type: &OhosModuleType::Entry,
                module_json_path: &entry_root.join("src/main/module.json5"),
                extension_abilities: &[],
                project_modules: &modules,
                compatible_api_version: Some(19),
            })
            .unwrap_err();

        assert!(error
            .to_string()
            .contains("FORM_MODULE_INVALID_API_VERSION"));
    }

    #[test]
    fn test_collect_module_form_sources_rejects_missing_delivery_with_install() {
        let tmp = tempfile::tempdir().unwrap();
        let entry_root = tmp.path().join("entry");
        let widget_root = tmp.path().join("widget");
        std::fs::create_dir_all(entry_root.join("src/main")).unwrap();
        std::fs::create_dir_all(widget_root.join("src/main")).unwrap();
        std::fs::write(
            entry_root.join("src/main/module.json5"),
            r#"{
              module: {
                name: "entry",
                type: "entry",
                deliveryWithInstall: true,
                formWidgetModule: "widget"
              }
            }"#,
        )
        .unwrap();
        std::fs::write(
            widget_root.join("src/main/module.json5"),
            r#"{
              module: {
                name: "widget",
                type: "shared",
                formExtensionModule: "entry"
              }
            }"#,
        )
        .unwrap();
        let modules = vec![
            module_config("entry", entry_root.clone(), OhosModuleType::Entry),
            module_config("widget", widget_root.clone(), OhosModuleType::Shared),
        ];

        let error = FormProfileProcessor::new()
            .collect_module_form_sources(ModuleFormProfileInput {
                module_name: "widget",
                module_root: &widget_root,
                module_type: &OhosModuleType::Shared,
                module_json_path: &widget_root.join("src/main/module.json5"),
                extension_abilities: &[],
                project_modules: &modules,
                compatible_api_version: Some(20),
            })
            .unwrap_err();

        assert!(error
            .to_string()
            .contains("FORM_MODULE_INVALID_DELIVERY_WITH_INSTALL"));
    }

    #[test]
    fn test_collect_module_form_sources_rejects_hap_form_config_when_pair_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let entry_root = tmp.path().join("entry");
        let widget_root = tmp.path().join("widget");
        std::fs::create_dir_all(entry_root.join("src/main/resources/base/profile")).unwrap();
        std::fs::create_dir_all(widget_root.join("src/main")).unwrap();
        std::fs::write(
            entry_root.join("src/main/module.json5"),
            r#"{
              module: {
                name: "entry",
                type: "entry",
                deliveryWithInstall: true,
                formWidgetModule: "widget"
              }
            }"#,
        )
        .unwrap();
        std::fs::write(
            entry_root.join("src/main/resources/base/profile/local_form.json"),
            r#"{ forms: [{ src: "./ets/forms/Local.ets", uiSyntax: "arkts" }] }"#,
        )
        .unwrap();
        std::fs::write(
            widget_root.join("src/main/module.json5"),
            r#"{
              module: {
                name: "widget",
                type: "shared",
                deliveryWithInstall: true,
                formExtensionModule: "entry"
              }
            }"#,
        )
        .unwrap();
        let modules = vec![
            module_config("entry", entry_root.clone(), OhosModuleType::Entry),
            module_config("widget", widget_root, OhosModuleType::Shared),
        ];

        let error = FormProfileProcessor::new()
            .collect_module_form_sources(ModuleFormProfileInput {
                module_name: "entry",
                module_root: &entry_root,
                module_type: &OhosModuleType::Entry,
                module_json_path: &entry_root.join("src/main/module.json5"),
                extension_abilities: &[],
                project_modules: &modules,
                compatible_api_version: Some(20),
            })
            .unwrap_err();

        assert!(error.to_string().contains("NO_FORM_ABILITY_ALLOWED"));
    }

    #[test]
    fn test_collect_module_form_sources_rejects_hsp_dependency_for_widget_module() {
        let tmp = tempfile::tempdir().unwrap();
        let entry_root = tmp.path().join("entry");
        let widget_root = tmp.path().join("widget");
        let other_hsp_root = tmp.path().join("otherHsp");
        std::fs::create_dir_all(entry_root.join("src/main")).unwrap();
        std::fs::create_dir_all(widget_root.join("src/main")).unwrap();
        std::fs::create_dir_all(other_hsp_root.join("src/main")).unwrap();
        std::fs::write(
            entry_root.join("src/main/module.json5"),
            r#"{
              module: {
                name: "entry",
                type: "entry",
                deliveryWithInstall: true,
                formWidgetModule: "widget"
              }
            }"#,
        )
        .unwrap();
        std::fs::write(
            widget_root.join("src/main/module.json5"),
            r#"{
              module: {
                name: "widget",
                type: "shared",
                deliveryWithInstall: true,
                formExtensionModule: "entry"
              }
            }"#,
        )
        .unwrap();
        std::fs::write(
            widget_root.join("oh-package.json5"),
            r#"{ name: "widget", dependencies: { otherHsp: "1.0.0" } }"#,
        )
        .unwrap();
        let modules = vec![
            module_config("entry", entry_root.clone(), OhosModuleType::Entry),
            module_config("widget", widget_root.clone(), OhosModuleType::Shared),
            module_config("otherHsp", other_hsp_root, OhosModuleType::Shared),
        ];

        let error = FormProfileProcessor::new()
            .collect_module_form_sources(ModuleFormProfileInput {
                module_name: "widget",
                module_root: &widget_root,
                module_type: &OhosModuleType::Shared,
                module_json_path: &widget_root.join("src/main/module.json5"),
                extension_abilities: &[],
                project_modules: &modules,
                compatible_api_version: Some(20),
            })
            .unwrap_err();

        assert!(error
            .to_string()
            .contains("FORM_WIDGET_MODULE_INVALID_DEPENDENCY"));
    }

    #[test]
    fn test_form_profile_processor_collect_form_profiles_delegates() {
        let tmp = tempfile::tempdir().unwrap();
        let module_root = tmp.path().join("entry");
        let abilities = vec![form_ability(vec![])];
        let processor = FormProfileProcessor::new();
        let error = processor
            .collect_form_profiles(input(&module_root, &abilities))
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("THE_METADATE_FIELD_IN_FORMEXTENSIONABILITY"));
    }

    #[test]
    fn test_form_profile_processor_default() {
        let tmp = tempfile::tempdir().unwrap();
        let module_root = tmp.path().join("entry");
        let processor = FormProfileProcessor::default();
        let result = processor.collect_form_profiles(FormProfileInput {
            module_root: &module_root,
            module_json_path: None,
            extension_abilities: &[],
        });
        assert!(result.unwrap().is_empty());
    }
}
