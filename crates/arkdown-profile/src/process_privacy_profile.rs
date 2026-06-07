//! `ProcessModulePrivacyProfile` — process module PAC/privacy data.
//!
//! Mirrors the hvigor task in
//! `hvigor-ohos-plugin/src/tasks/pac-process/process-module-privacy-profile.js`:
//! read `<module>/pac.json5`, check hvigor's basic enabling fields, and write
//! the processed file to:
//!
//! ```text
//! <module>/build/default/intermediates/privacy_profile/default/pac.json5
//! ```

use indexmap::{IndexMap, IndexSet};
use serde::Deserialize;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use arkdown_core::{ArkdownError, ModuleConfig, OhosModuleType};

const PAC_JSON: &str = "pac.json5";
const OH_PACKAGE_JSON5: &str = "oh-package.json5";
const PACKAGE_JSON: &str = "package.json";

#[derive(Deserialize)]
struct DependencyPackageJson {
    #[serde(default)]
    dependencies: IndexMap<String, String>,
}

/// Input for the module PAC/privacy processing task.
pub struct PrivacyProfileInput<'a> {
    /// Root directory of the module, e.g. `/project/entry`.
    pub module_root: &'a Path,
    /// Target name, normally `default`.
    pub target_name: &'a str,
    /// SDK PAC schema path. hvigor skips this task when the schema is absent.
    pub pac_schema_path: Option<&'a Path>,
}

/// Output of [`process_module_privacy_profile`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyProfileOutput {
    /// Source PAC file at `<module>/pac.json5`.
    pub source_path: PathBuf,
    /// Processed PAC file at `build/default/intermediates/privacy_profile/<target>/pac.json5`.
    pub processed_path: PathBuf,
}

/// Input for hvigor's project-level `ProcessProjectPrivacyProfile`.
pub struct ProjectPrivacyProfileInput<'a> {
    /// Root directory of the project.
    pub project_root: &'a Path,
    /// Product name, normally `default`.
    pub product_name: &'a str,
    /// Workspace modules known to the current arkdown config.
    pub modules: &'a [ModuleConfig],
    /// SDK PAC schema path. hvigor skips this task when the schema is absent.
    pub pac_schema_path: Option<&'a Path>,
}

/// Output of [`process_project_privacy_profile`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectPrivacyProfileOutput {
    /// Processed project PAC file at `build/outputs/<product>/pac.json`.
    pub processed_path: PathBuf,
}

/// Process a module PAC/privacy profile with hvigor's basic enablement check.
///
/// Returns `Ok(None)` when the source file does not exist, cannot be parsed as
/// JSON5, or does not contain hvigor's required enabling fields:
/// `dataProcess` or `specialAPIs`.
pub fn process_module_privacy_profile(
    input: PrivacyProfileInput<'_>,
) -> Result<Option<PrivacyProfileOutput>, ArkdownError> {
    let Some(schema_path) = existing_pac_schema_path(input.pac_schema_path) else {
        return Ok(None);
    };
    let source_path = input.module_root.join(PAC_JSON);
    if !pac_file_validate(&source_path, schema_path)? {
        return Ok(None);
    }

    let processed_path = input
        .module_root
        .join("build/default/intermediates/privacy_profile")
        .join(input.target_name)
        .join(PAC_JSON);
    if let Some(parent) = processed_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(&source_path, &processed_path)?;

    Ok(Some(PrivacyProfileOutput {
        source_path,
        processed_path,
    }))
}

/// Process project-level PAC/privacy data.
///
/// Mirrors hvigor `ProcessProjectPrivacyProfile` at the merge-rule level:
/// AppScope PAC, non-HAP module PAC files, and dependency PAC files are merged
/// into `build/outputs/<product>/pac.json`. Dependency package metadata is
/// appended under `dependencies` with `(name, version)` de-duplication.
///
/// Returns `Ok(None)` when no PAC data or dependency metadata is discoverable.
pub fn process_project_privacy_profile(
    input: ProjectPrivacyProfileInput<'_>,
) -> Result<Option<ProjectPrivacyProfileOutput>, ArkdownError> {
    let Some(schema_path) = existing_pac_schema_path(input.pac_schema_path) else {
        return Ok(None);
    };
    let mut pac_files = Vec::new();
    push_valid_pac(
        input.project_root.join("AppScope").join(PAC_JSON),
        schema_path,
        &mut pac_files,
    )?;

    for module in input.modules {
        if !matches!(
            &module.module_type,
            OhosModuleType::Entry | OhosModuleType::Feature
        ) {
            push_valid_pac(module.src_path.join(PAC_JSON), schema_path, &mut pac_files)?;
        }
    }

    let package_dirs = collect_dependency_package_dirs(input.project_root, input.modules);
    let mut dependency_packages = Vec::new();
    for package_dir in &package_dirs {
        push_valid_pac(package_dir.join(PAC_JSON), schema_path, &mut pac_files)?;
        if let Some(package_json) = read_package_metadata(package_dir)? {
            dependency_packages.push(package_json);
        }
    }

    if pac_files.is_empty() && dependency_packages.is_empty() {
        return Ok(None);
    }

    let mut pac_values = Vec::new();
    for path in &pac_files {
        if let Some(value) = read_json5_value(path)? {
            pac_values.push(value);
        }
    }
    let mut merged = merge_pac_values(&pac_values);
    merge_dependency_metadata(&dependency_packages, &mut merged);

    let processed_path = input
        .project_root
        .join("build/outputs")
        .join(input.product_name)
        .join("pac.json");
    if let Some(parent) = processed_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        &processed_path,
        serde_json::to_vec(&merged).expect("serializing project PAC should not fail"),
    )?;

    Ok(Some(ProjectPrivacyProfileOutput { processed_path }))
}

/// Check the same enablement condition as hvigor's
/// `checkPacJsonRequiredFields()`: a valid JSON5 object with either
/// `dataProcess` or `specialAPIs` at the top level.
pub fn privacy_profile_is_enabled(path: &Path) -> bool {
    let Some(value) = read_json5_value_lenient(path) else {
        return false;
    };
    pac_value_is_enabled(&value)
}

fn read_json5_value_lenient(path: &Path) -> Option<serde_json::Value> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return None;
    };
    json5::from_str::<serde_json::Value>(&content).ok()
}

fn pac_value_is_enabled(value: &serde_json::Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    object.contains_key("dataProcess") || object.contains_key("specialAPIs")
}

fn existing_pac_schema_path(path: Option<&Path>) -> Option<&Path> {
    path.filter(|path| path.is_file())
}

fn push_valid_pac(
    path: PathBuf,
    schema_path: &Path,
    pac_files: &mut Vec<PathBuf>,
) -> Result<(), ArkdownError> {
    if pac_file_validate(&path, schema_path)? {
        pac_files.push(path);
    }
    Ok(())
}

fn pac_file_validate(path: &Path, schema_path: &Path) -> Result<bool, ArkdownError> {
    if !path.is_file() || !schema_path.is_file() {
        return Ok(false);
    }
    let Some(value) = read_json5_value_lenient(path) else {
        return Ok(false);
    };
    if !pac_value_is_enabled(&value) {
        return Ok(false);
    }
    validate_pac_schema(path, schema_path, &value)?;
    Ok(true)
}

fn validate_pac_schema(
    path: &Path,
    schema_path: &Path,
    value: &serde_json::Value,
) -> Result<(), ArkdownError> {
    let schema = read_json5_value(schema_path)?
        .ok_or_else(|| pac_schema_error(path, "SDK PAC schema is missing"))?;
    validate_json_schema(path, value, &schema, "$")
}

fn validate_json_schema(
    path: &Path,
    value: &serde_json::Value,
    schema: &serde_json::Value,
    value_path: &str,
) -> Result<(), ArkdownError> {
    validate_schema_type(path, value, schema, value_path)?;
    validate_schema_enum(path, value, schema, value_path)?;
    validate_schema_const(path, value, schema, value_path)?;

    if let Some(min_items) = schema.get("minItems").and_then(serde_json::Value::as_u64) {
        let Some(array) = value.as_array() else {
            return Err(pac_schema_error(
                path,
                &format!("{value_path} must be an array"),
            ));
        };
        if array.len() < min_items as usize {
            return Err(pac_schema_error(
                path,
                &format!("{value_path} must contain at least {min_items} item(s)"),
            ));
        }
    }

    if let Some(object) = value.as_object() {
        validate_property_names(path, object, schema, value_path)?;
        validate_required_fields(path, object, schema, value_path)?;
        if let Some(properties) = schema
            .get("properties")
            .and_then(serde_json::Value::as_object)
        {
            for (name, child_schema) in properties {
                if let Some(child_value) = object.get(name) {
                    validate_json_schema(
                        path,
                        child_value,
                        child_schema,
                        &format!("{value_path}.{name}"),
                    )?;
                }
            }
        }
    }

    if let (Some(items_schema), Some(array)) = (schema.get("items"), value.as_array()) {
        for (index, item) in array.iter().enumerate() {
            validate_json_schema(path, item, items_schema, &format!("{value_path}[{index}]"))?;
        }
    }

    if let Some(all_of) = schema.get("allOf").and_then(serde_json::Value::as_array) {
        for nested_schema in all_of {
            if let Some(if_schema) = nested_schema.get("if") {
                if json_schema_matches(value, if_schema) {
                    if let Some(then_schema) = nested_schema.get("then") {
                        validate_json_schema(path, value, then_schema, value_path)?;
                    }
                }
            } else {
                validate_json_schema(path, value, nested_schema, value_path)?;
            }
        }
    }

    Ok(())
}

fn validate_schema_type(
    path: &Path,
    value: &serde_json::Value,
    schema: &serde_json::Value,
    value_path: &str,
) -> Result<(), ArkdownError> {
    let Some(expected_type) = schema.get("type").and_then(serde_json::Value::as_str) else {
        return Ok(());
    };
    let valid = match expected_type {
        "array" => value.is_array(),
        "object" => value.is_object(),
        "string" => value.is_string(),
        _ => true,
    };
    if !valid {
        return Err(pac_schema_error(
            path,
            &format!("{value_path} must be {expected_type}"),
        ));
    }
    Ok(())
}

fn validate_schema_enum(
    path: &Path,
    value: &serde_json::Value,
    schema: &serde_json::Value,
    value_path: &str,
) -> Result<(), ArkdownError> {
    let Some(values) = schema.get("enum").and_then(serde_json::Value::as_array) else {
        return Ok(());
    };
    if !values.iter().any(|candidate| candidate == value) {
        return Err(pac_schema_error(
            path,
            &format!("{value_path} has value outside SDK PAC schema enum"),
        ));
    }
    Ok(())
}

fn validate_schema_const(
    path: &Path,
    value: &serde_json::Value,
    schema: &serde_json::Value,
    value_path: &str,
) -> Result<(), ArkdownError> {
    if let Some(expected) = schema.get("const") {
        if value != expected {
            return Err(pac_schema_error(
                path,
                &format!("{value_path} does not match SDK PAC schema const"),
            ));
        }
    }
    Ok(())
}

fn validate_property_names(
    path: &Path,
    object: &serde_json::Map<String, serde_json::Value>,
    schema: &serde_json::Value,
    value_path: &str,
) -> Result<(), ArkdownError> {
    let Some(property_names) = schema.get("propertyNames") else {
        return Ok(());
    };
    for name in object.keys() {
        validate_json_schema(
            path,
            &serde_json::Value::String(name.clone()),
            property_names,
            &format!("{value_path}.<propertyName>"),
        )?;
    }
    Ok(())
}

fn validate_required_fields(
    path: &Path,
    object: &serde_json::Map<String, serde_json::Value>,
    schema: &serde_json::Value,
    value_path: &str,
) -> Result<(), ArkdownError> {
    let Some(required) = schema.get("required").and_then(serde_json::Value::as_array) else {
        return Ok(());
    };
    for field in required.iter().filter_map(serde_json::Value::as_str) {
        if !object.contains_key(field) {
            return Err(pac_schema_error(
                path,
                &format!("{value_path}.{field} is required"),
            ));
        }
    }
    Ok(())
}

fn json_schema_matches(value: &serde_json::Value, schema: &serde_json::Value) -> bool {
    if let Some(expected) = schema.get("const") {
        return value == expected;
    }

    if let Some(expected_type) = schema.get("type").and_then(serde_json::Value::as_str) {
        let valid = match expected_type {
            "array" => value.is_array(),
            "object" => value.is_object(),
            "string" => value.is_string(),
            _ => true,
        };
        if !valid {
            return false;
        }
    }

    if let Some(values) = schema.get("enum").and_then(serde_json::Value::as_array) {
        if !values.iter().any(|candidate| candidate == value) {
            return false;
        }
    }

    if let (Some(properties), Some(object)) = (
        schema
            .get("properties")
            .and_then(serde_json::Value::as_object),
        value.as_object(),
    ) {
        for (name, child_schema) in properties {
            if let Some(child_value) = object.get(name) {
                if !json_schema_matches(child_value, child_schema) {
                    return false;
                }
            }
        }
    }

    true
}

/// Generate a PAC schema validation error matching hvigor's pacFileValidate() error format.
///
/// hvigor uses `PRIVACY_PROFILE_VALIDATION_FAILED` as the primary error code
/// for pac.json5 schema validation failures (from ValidateUtil.validatePacFileSchema()).
fn pac_schema_error(path: &Path, message: &str) -> ArkdownError {
    ArkdownError::Bundler(format!(
        "PRIVACY_PROFILE_VALIDATION_FAILED: {}: {message}",
        path.display()
    ))
}

fn read_json5_value(path: &Path) -> Result<Option<serde_json::Value>, ArkdownError> {
    if !path.is_file() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(path)?;
    let value = json5::from_str::<serde_json::Value>(&content).map_err(|err| {
        ArkdownError::Bundler(format!("failed to parse {}: {err}", path.display()))
    })?;
    Ok(Some(value))
}

fn merge_pac_values(values: &[serde_json::Value]) -> serde_json::Value {
    let mut data_processes: IndexMap<String, IndexMap<String, IndexSet<String>>> = IndexMap::new();
    let mut special_apis: IndexMap<String, IndexSet<String>> = IndexMap::new();

    for value in values {
        if let Some(entries) = value.get("dataProcess").and_then(|value| value.as_array()) {
            for entry in entries {
                let Some(data_type) = entry.get("dataType").and_then(|value| value.as_str()) else {
                    continue;
                };
                let labels = data_processes.entry(data_type.to_string()).or_default();
                let Some(data_labels) = entry.get("dataLabels").and_then(|value| value.as_array())
                else {
                    continue;
                };
                for data_label in data_labels {
                    let Some(label) = data_label.get("label").and_then(|value| value.as_str())
                    else {
                        continue;
                    };
                    let purposes = labels.entry(label.to_string()).or_default();
                    if let Some(values) = data_label
                        .get("purposes")
                        .and_then(|value| value.as_array())
                    {
                        for purpose in values {
                            if let Some(purpose) = purpose.as_str() {
                                purposes.insert(purpose.to_string());
                            }
                        }
                    }
                }
            }
        }

        if let Some(entries) = value.get("specialAPIs").and_then(|value| value.as_array()) {
            for entry in entries {
                let Some(api_type) = entry.get("apiType").and_then(|value| value.as_str()) else {
                    continue;
                };
                let reasons = special_apis.entry(api_type.to_string()).or_default();
                if let Some(values) = entry.get("reasons").and_then(|value| value.as_array()) {
                    for reason in values {
                        if let Some(reason) = reason.as_str() {
                            reasons.insert(reason.to_string());
                        }
                    }
                }
            }
        }
    }

    let data_process = data_processes
        .into_iter()
        .map(|(data_type, labels)| {
            let data_labels = labels
                .into_iter()
                .map(|(label, purposes)| {
                    serde_json::json!({
                        "label": label,
                        "purposes": purposes.into_iter().collect::<Vec<_>>(),
                    })
                })
                .collect::<Vec<_>>();
            serde_json::json!({
                "dataType": data_type,
                "dataLabels": data_labels,
            })
        })
        .collect::<Vec<_>>();

    let special_apis = special_apis
        .into_iter()
        .map(|(api_type, reasons)| {
            serde_json::json!({
                "apiType": api_type,
                "reasons": reasons.into_iter().collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();

    serde_json::json!({
        "dataProcess": data_process,
        "specialAPIs": special_apis,
    })
}

fn merge_dependency_metadata(packages: &[serde_json::Value], merged: &mut serde_json::Value) {
    let mut seen = IndexSet::new();
    let dependencies = merged
        .as_object_mut()
        .expect("merged PAC must be an object")
        .entry("dependencies")
        .or_insert_with(|| serde_json::Value::Array(Vec::new()));
    let dependencies = dependencies
        .as_array_mut()
        .expect("merged PAC dependencies must be an array");

    for package in packages {
        let Some(name) = package.get("name").and_then(|value| value.as_str()) else {
            continue;
        };
        let version = package
            .get("version")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        if !seen.insert(format!("{name}\0{version}")) {
            continue;
        }
        dependencies.push(serde_json::json!({
            "author": package.get("author").and_then(|value| value.as_str()).unwrap_or_default(),
            "name": name,
            "description": package.get("description").and_then(|value| value.as_str()).unwrap_or_default(),
            "version": version,
        }));
    }
}

fn collect_dependency_package_dirs(project_root: &Path, modules: &[ModuleConfig]) -> Vec<PathBuf> {
    let mut seen = IndexSet::new();
    let mut packages = Vec::new();
    let mut queue = VecDeque::new();
    queue.push_back(project_root.to_path_buf());
    for module in modules {
        if module.src_path != project_root {
            queue.push_back(module.src_path.clone());
        }
    }

    while let Some(pkg_root) = queue.pop_front() {
        let Some(pkg) = read_dependency_package_json(&pkg_root) else {
            continue;
        };
        for dep_name in pkg.dependencies.keys() {
            let Some(dep_root) = resolve_dependency_root(&pkg_root, project_root, dep_name) else {
                continue;
            };
            let key = arkdown_path::to_slash(&dep_root);
            if !seen.insert(key) {
                continue;
            }
            queue.push_back(dep_root.clone());
            packages.push(dep_root);
        }
    }

    packages
}

fn read_dependency_package_json(pkg_root: &Path) -> Option<DependencyPackageJson> {
    let content = std::fs::read_to_string(pkg_root.join(OH_PACKAGE_JSON5)).ok()?;
    json5::from_str::<DependencyPackageJson>(&content).ok()
}

fn resolve_dependency_root(
    issuer_root: &Path,
    project_root: &Path,
    dep_name: &str,
) -> Option<PathBuf> {
    let mut current = Some(issuer_root);
    while let Some(root) = current {
        let candidate = root.join("oh_modules").join(dep_name);
        if candidate.is_dir() {
            return Some(candidate);
        }
        if root == project_root {
            break;
        }
        current = root.parent();
    }
    let candidate = project_root.join("oh_modules").join(dep_name);
    candidate.is_dir().then_some(candidate)
}

fn read_package_metadata(package_dir: &Path) -> Result<Option<serde_json::Value>, ArkdownError> {
    for file_name in [OH_PACKAGE_JSON5, PACKAGE_JSON] {
        let path = package_dir.join(file_name);
        if let Some(value) = read_json5_value(&path)? {
            return Ok(Some(value));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkdown_core::ModuleConfig;

    fn write_schema(dir: &Path) -> PathBuf {
        let schema = dir.join("pacSchema.json");
        std::fs::write(
            &schema,
            r#"{
                "type": "object",
                "propertyNames": { "enum": ["dataProcess", "specialAPIs"] },
                "properties": {
                    "dataProcess": {
                        "type": "array",
                        "minItems": 1,
                        "items": {
                            "type": "object",
                            "propertyNames": { "enum": ["dataType", "dataLabels"] },
                            "required": ["dataType", "dataLabels"],
                            "properties": {
                                "dataType": {
                                    "type": "string",
                                    "enum": ["Location information"]
                                },
                                "dataLabels": {
                                    "type": "array",
                                    "minItems": 1,
                                    "items": {
                                        "type": "object",
                                        "propertyNames": { "enum": ["label", "purposes"] },
                                        "required": ["label", "purposes"],
                                        "properties": {
                                            "label": {
                                                "type": "string",
                                                "enum": ["Network location", "GPS location"]
                                            },
                                            "purposes": {
                                                "type": "array",
                                                "minItems": 1,
                                                "items": {
                                                    "type": "string",
                                                    "enum": ["App functionality", "Analytics"]
                                                }
                                            }
                                        }
                                    }
                                }
                            },
                            "allOf": [{
                                "if": {
                                    "properties": {
                                        "dataType": { "const": "Location information" }
                                    }
                                },
                                "then": {
                                    "properties": {
                                        "dataLabels": {
                                            "items": {
                                                "properties": {
                                                    "label": {
                                                        "enum": ["Network location", "GPS location"]
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }]
                        }
                    },
                    "specialAPIs": {
                        "type": "array",
                        "minItems": 1,
                        "items": {
                            "type": "object",
                            "propertyNames": { "enum": ["apiType", "reasons"] },
                            "required": ["apiType", "reasons"],
                            "properties": {
                                "apiType": {
                                    "type": "string",
                                    "enum": ["File timestamp APIs", "Disk space APIs"]
                                },
                                "reasons": {
                                    "type": "array",
                                    "minItems": 1,
                                    "items": {
                                        "type": "string",
                                        "enum": [
                                            "Display to user on-device",
                                            "Access file metadata in-app",
                                            "Write or delete file on-device",
                                            "User-initiated bug report"
                                        ]
                                    }
                                }
                            },
                            "allOf": [{
                                "if": {
                                    "properties": {
                                        "apiType": { "const": "File timestamp APIs" }
                                    }
                                },
                                "then": {
                                    "properties": {
                                        "reasons": {
                                            "items": {
                                                "enum": [
                                                    "Display to user on-device",
                                                    "Access file metadata in-app"
                                                ]
                                            }
                                        }
                                    }
                                }
                            }, {
                                "if": {
                                    "properties": {
                                        "apiType": { "const": "Disk space APIs" }
                                    }
                                },
                                "then": {
                                    "properties": {
                                        "reasons": {
                                            "items": {
                                                "enum": [
                                                    "Display to user on-device",
                                                    "Write or delete file on-device",
                                                    "User-initiated bug report"
                                                ]
                                            }
                                        }
                                    }
                                }
                            }]
                        }
                    }
                }
            }"#,
        )
        .unwrap();
        schema
    }

    fn valid_special_api_pac() -> &'static str {
        r#"{
            specialAPIs: [{
                apiType: "File timestamp APIs",
                reasons: ["Display to user on-device"],
            }],
        }"#
    }

    #[test]
    fn test_privacy_profile_is_enabled_by_data_process() {
        let tmp = tempfile::tempdir().unwrap();
        let pac = tmp.path().join(PAC_JSON);
        std::fs::write(
            &pac,
            r#"{
                // JSON5 syntax is accepted by hvigor.
                dataProcess: [],
            }"#,
        )
        .unwrap();

        assert!(privacy_profile_is_enabled(&pac));
    }

    #[test]
    fn test_privacy_profile_requires_known_fields() {
        let tmp = tempfile::tempdir().unwrap();
        let pac = tmp.path().join(PAC_JSON);
        std::fs::write(&pac, r#"{ "dependencies": [] }"#).unwrap();

        assert!(!privacy_profile_is_enabled(&pac));
    }

    #[test]
    fn test_process_module_privacy_profile_copies_to_hvigor_intermediate() {
        let tmp = tempfile::tempdir().unwrap();
        let schema = write_schema(tmp.path());
        let pac = tmp.path().join(PAC_JSON);
        std::fs::write(&pac, valid_special_api_pac()).unwrap();

        let output = process_module_privacy_profile(PrivacyProfileInput {
            module_root: tmp.path(),
            target_name: "default",
            pac_schema_path: Some(&schema),
        })
        .unwrap()
        .expect("PAC file should be enabled");

        assert!(output.processed_path.ends_with(Path::new(
            "build/default/intermediates/privacy_profile/default/pac.json5"
        )));
        assert_eq!(
            std::fs::read_to_string(output.processed_path).unwrap(),
            valid_special_api_pac()
        );
    }

    #[test]
    fn test_process_module_privacy_profile_skips_when_sdk_schema_missing() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(PAC_JSON), valid_special_api_pac()).unwrap();

        let output = process_module_privacy_profile(PrivacyProfileInput {
            module_root: tmp.path(),
            target_name: "default",
            pac_schema_path: None,
        })
        .unwrap();

        assert!(output.is_none());
    }

    #[test]
    fn test_process_module_privacy_profile_rejects_schema_shape_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let schema = write_schema(tmp.path());
        std::fs::write(tmp.path().join(PAC_JSON), r#"{ specialAPIs: [] }"#).unwrap();

        let error = process_module_privacy_profile(PrivacyProfileInput {
            module_root: tmp.path(),
            target_name: "default",
            pac_schema_path: Some(&schema),
        })
        .unwrap_err();

        assert!(error
            .to_string()
            .contains("PRIVACY_PROFILE_VALIDATION_FAILED"));
    }

    #[test]
    fn test_process_module_privacy_profile_rejects_conditional_schema_enum_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let schema = write_schema(tmp.path());
        std::fs::write(
            tmp.path().join(PAC_JSON),
            r#"{
                specialAPIs: [{
                    apiType: "File timestamp APIs",
                    reasons: ["Write or delete file on-device"],
                }],
            }"#,
        )
        .unwrap();

        let error = process_module_privacy_profile(PrivacyProfileInput {
            module_root: tmp.path(),
            target_name: "default",
            pac_schema_path: Some(&schema),
        })
        .unwrap_err();

        assert!(error
            .to_string()
            .contains("PRIVACY_PROFILE_VALIDATION_FAILED"));
    }

    #[test]
    fn test_process_project_privacy_profile_merges_pac_files_and_dependencies() {
        let tmp = tempfile::tempdir().unwrap();
        let schema = write_schema(tmp.path());
        std::fs::write(
            tmp.path().join(OH_PACKAGE_JSON5),
            r#"{ dependencies: { "dep": "1.0.0" } }"#,
        )
        .unwrap();
        let app_scope = tmp.path().join("AppScope");
        std::fs::create_dir_all(&app_scope).unwrap();
        std::fs::write(
            app_scope.join(PAC_JSON),
            r#"{
                dataProcess: [{
                    dataType: "Location information",
                    dataLabels: [{ label: "GPS location", purposes: ["App functionality"] }],
                }],
                specialAPIs: [{
                    apiType: "File timestamp APIs",
                    reasons: ["Display to user on-device"],
                }],
            }"#,
        )
        .unwrap();

        let shared = tmp.path().join("shared");
        std::fs::create_dir_all(&shared).unwrap();
        std::fs::write(
            shared.join(PAC_JSON),
            r#"{
                dataProcess: [{
                    dataType: "Location information",
                    dataLabels: [{
                        label: "GPS location",
                        purposes: ["App functionality", "Analytics"],
                    }],
                }],
                specialAPIs: [{
                    apiType: "File timestamp APIs",
                    reasons: [
                        "Display to user on-device",
                        "Access file metadata in-app",
                    ],
                }],
            }"#,
        )
        .unwrap();

        let dep = tmp.path().join("oh_modules/dep");
        std::fs::create_dir_all(&dep).unwrap();
        std::fs::write(
            dep.join(OH_PACKAGE_JSON5),
            r#"{ name: "dep", version: "1.0.0", author: "team", description: "desc" }"#,
        )
        .unwrap();
        std::fs::write(
            dep.join(PAC_JSON),
            r#"{
                specialAPIs: [{
                    apiType: "File timestamp APIs",
                    reasons: ["Access file metadata in-app"],
                }],
            }"#,
        )
        .unwrap();

        let modules = vec![ModuleConfig {
            name: "shared".to_string(),
            src_path: shared,
            module_type: OhosModuleType::Shared,
        }];
        let output = process_project_privacy_profile(ProjectPrivacyProfileInput {
            project_root: tmp.path(),
            product_name: "default",
            modules: &modules,
            pac_schema_path: Some(&schema),
        })
        .unwrap()
        .expect("project PAC should be written");

        assert!(output
            .processed_path
            .ends_with(Path::new("build/outputs/default/pac.json")));
        let value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(output.processed_path).unwrap()).unwrap();
        assert_eq!(
            value["dataProcess"][0]["dataLabels"][0]["purposes"],
            serde_json::json!(["App functionality", "Analytics"])
        );
        assert_eq!(
            value["specialAPIs"][0]["reasons"],
            serde_json::json!(["Display to user on-device", "Access file metadata in-app"])
        );
        assert_eq!(
            value["dependencies"],
            serde_json::json!([{
                "author": "team",
                "name": "dep",
                "description": "desc",
                "version": "1.0.0",
            }])
        );
    }

    #[test]
    fn test_process_project_privacy_profile_follows_declared_transitive_and_skips_undeclared() {
        let tmp = tempfile::tempdir().unwrap();
        let schema = write_schema(tmp.path());
        std::fs::write(
            tmp.path().join(OH_PACKAGE_JSON5),
            r#"{ dependencies: { "source-har": "1.0.0" } }"#,
        )
        .unwrap();

        let source_har = tmp.path().join("oh_modules/source-har");
        std::fs::create_dir_all(&source_har).unwrap();
        std::fs::write(
            source_har.join(OH_PACKAGE_JSON5),
            r#"{ dependencies: { "@scope/shared-hsp": "1.0.0" } }"#,
        )
        .unwrap();

        let shared_hsp = source_har.join("oh_modules/@scope/shared-hsp");
        std::fs::create_dir_all(&shared_hsp).unwrap();
        std::fs::write(
            shared_hsp.join(OH_PACKAGE_JSON5),
            r#"{ name: "@scope/shared-hsp", version: "1.0.0", author: "team", description: "scoped" }"#,
        )
        .unwrap();
        std::fs::write(
            shared_hsp.join(PAC_JSON),
            r#"{
                specialAPIs: [{
                    apiType: "File timestamp APIs",
                    reasons: ["Access file metadata in-app"],
                }],
            }"#,
        )
        .unwrap();

        let undeclared_hsp = tmp.path().join("oh_modules/undeclared-hsp");
        std::fs::create_dir_all(&undeclared_hsp).unwrap();
        std::fs::write(
            undeclared_hsp.join(OH_PACKAGE_JSON5),
            r#"{ name: "undeclared-hsp", version: "1.0.0" }"#,
        )
        .unwrap();
        std::fs::write(
            undeclared_hsp.join(PAC_JSON),
            r#"{
                specialAPIs: [{
                    apiType: "File timestamp APIs",
                    reasons: ["Display to user on-device"],
                }],
            }"#,
        )
        .unwrap();

        let output = process_project_privacy_profile(ProjectPrivacyProfileInput {
            project_root: tmp.path(),
            product_name: "default",
            modules: &[],
            pac_schema_path: Some(&schema),
        })
        .unwrap()
        .expect("project PAC should be written from transitive dependency");

        let value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(output.processed_path).unwrap()).unwrap();
        assert_eq!(
            value["specialAPIs"][0]["reasons"],
            serde_json::json!(["Access file metadata in-app"])
        );
        assert_eq!(
            value["dependencies"],
            serde_json::json!([{
                "author": "team",
                "name": "@scope/shared-hsp",
                "description": "scoped",
                "version": "1.0.0",
            }])
        );
    }

    #[test]
    fn test_process_project_privacy_profile_skips_when_sdk_schema_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let app_scope = tmp.path().join("AppScope");
        std::fs::create_dir_all(&app_scope).unwrap();
        std::fs::write(app_scope.join(PAC_JSON), valid_special_api_pac()).unwrap();

        let output = process_project_privacy_profile(ProjectPrivacyProfileInput {
            project_root: tmp.path(),
            product_name: "default",
            modules: &[],
            pac_schema_path: None,
        })
        .unwrap();

        assert!(output.is_none());
    }
}
