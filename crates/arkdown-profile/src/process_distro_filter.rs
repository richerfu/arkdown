//! Resource-phase rewrite for target `config.distributionFilter`.
//!
//! Hvigor writes target-level distribution filters into a resource profile and
//! wires that profile through `module.metadata`.

use std::path::{Path, PathBuf};

use arkdown_config::build_profile::RawBuildProfile;
use arkdown_config::ConfigError;
use arkdown_core::{ArkdownError, MetadataEntry};

use crate::layout::{ProfileModuleLayout, ProfileOutputLayout};
use crate::resource_ref::ProfileResourceRef;

const DEFAULT_DISTRO_FILTER_PROFILE: &str = "ohos_module_distro_filter";

pub struct DistroFilterProfileInput<'a> {
    pub project_root: &'a Path,
    pub module_name: &'a str,
    pub module_root: &'a Path,
    pub product_name: &'a str,
    /// Explicit module target selected by the build task.
    pub target_name: Option<&'a str>,
    /// Current build mode (`"debug"` or `"release"`), used to filter targets
    /// whose `buildModeSet` restricts them to specific modes.  Defaults to
    /// all-modes matching when `None`.
    pub build_mode_name: Option<&'a str>,
    pub compile_api_version: u32,
    pub metadata: &'a [MetadataEntry],
    pub output_profile_dir: &'a Path,
    pub module_json_path: Option<&'a Path>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DistroFilterProfileOutput {
    pub profile_name: String,
    pub profile_path: PathBuf,
    pub field_name: String,
}

#[derive(Debug, Clone)]
pub(crate) struct DistroFilterPlan {
    pub(crate) profile_name: String,
    pub(crate) field_name: String,
    pub(crate) value: serde_json::Value,
    pub(crate) has_existing_metadata: bool,
}

/// Processes distribution filter profiles for a module target.
///
/// Mirrors hvigor `process-distro-filter.js`: reads the target's
/// `config.distributionFilter` from `build-profile.json5`, writes it to a
/// resource profile JSON, and optionally updates `module.json` metadata.
pub struct DistroFilterProcessor;

impl DistroFilterProcessor {
    /// Create a new processor.
    pub fn new() -> Self {
        DistroFilterProcessor
    }

    /// Process the distro filter profile for the given input.
    ///
    /// Returns `Ok(None)` when there is no distribution filter configured for
    /// the target.
    pub fn process_distro_filter_profile(
        &self,
        input: DistroFilterProfileInput<'_>,
    ) -> Result<Option<DistroFilterProfileOutput>, ArkdownError> {
        let Some(plan) = build_distro_filter_plan(&input)? else {
            return Ok(None);
        };
        self.write_distro_filter_profile(input, plan)
    }

    /// Process a distribution filter already resolved by the build session.
    ///
    /// This is the production build-path entry point: it avoids re-reading
    /// `build-profile.json5` after startup and guarantees every stage consumes
    /// the same selected target.
    pub fn process_resolved_distro_filter_profile(
        &self,
        input: DistroFilterProfileInput<'_>,
        target_filter: Option<&serde_json::Value>,
    ) -> Result<Option<DistroFilterProfileOutput>, ArkdownError> {
        let Some(target_filter) = target_filter else {
            return Ok(None);
        };
        let plan = build_distro_filter_plan_from_value(
            input.module_root,
            input.compile_api_version,
            input.metadata,
            target_filter.clone(),
        )?;
        self.write_distro_filter_profile(input, plan)
    }

    fn write_distro_filter_profile(
        &self,
        input: DistroFilterProfileInput<'_>,
        plan: DistroFilterPlan,
    ) -> Result<Option<DistroFilterProfileOutput>, ArkdownError> {
        let has_existing_metadata = plan.has_existing_metadata;
        let profile_name = if has_existing_metadata {
            plan.profile_name
        } else {
            unique_profile_name(input.output_profile_dir, &plan.profile_name)?
        };
        let field_name = plan.field_name;
        let value = plan.value;

        let profile_path =
            ProfileOutputLayout::new(input.output_profile_dir).profile_json(&profile_name);
        if let Some(parent) = profile_path.parent() {
            fs_err::create_dir_all(parent)?;
        }
        let json = serde_json::json!({ field_name.clone(): value });
        let bytes = serde_json::to_vec(&json)
            .map_err(|e| ArkdownError::Io(std::io::Error::other(e.to_string())))?;
        fs_err::write(&profile_path, bytes)?;

        if !has_existing_metadata {
            if let Some(module_json_path) = input.module_json_path {
                append_profile_metadata_to_module_json(module_json_path, &profile_name)?;
            }
        }

        Ok(Some(DistroFilterProfileOutput {
            profile_name,
            profile_path,
            field_name,
        }))
    }
}

impl Default for DistroFilterProcessor {
    fn default() -> Self {
        DistroFilterProcessor::new()
    }
}

pub(crate) fn build_distro_filter_plan(
    input: &DistroFilterProfileInput<'_>,
) -> Result<Option<DistroFilterPlan>, ArkdownError> {
    let Some(target_filter) = read_target_distribution_filter(
        input.project_root,
        input.module_name,
        input.product_name,
        input.target_name,
        input.build_mode_name.unwrap_or("debug"),
    )?
    else {
        return Ok(None);
    };

    build_distro_filter_plan_from_value(
        input.module_root,
        input.compile_api_version,
        input.metadata,
        target_filter,
    )
    .map(Some)
}

fn build_distro_filter_plan_from_value(
    module_root: &Path,
    compile_api_version: u32,
    metadata: &[MetadataEntry],
    target_filter: serde_json::Value,
) -> Result<DistroFilterPlan, ArkdownError> {
    let default_field_name = distro_filter_field_name(compile_api_version);
    if let Some(existing) = find_existing_distro_filter_profile(module_root, metadata)? {
        let field_name = existing
            .value
            .as_object()
            .and_then(|object| object.keys().next().cloned())
            .unwrap_or_else(|| default_field_name.to_string());
        let existing_filter = existing
            .value
            .get("distroFilter")
            .cloned()
            .or_else(|| existing.value.get("distributionFilter").cloned())
            .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
        let merged = merge_json_objects(existing_filter, target_filter);
        return Ok(DistroFilterPlan {
            profile_name: existing.profile_name,
            field_name,
            value: merged,
            has_existing_metadata: true,
        });
    }

    Ok(DistroFilterPlan {
        profile_name: DEFAULT_DISTRO_FILTER_PROFILE.to_string(),
        field_name: default_field_name.to_string(),
        value: target_filter,
        has_existing_metadata: false,
    })
}

fn read_target_distribution_filter(
    project_root: &Path,
    module_name: &str,
    product_name: &str,
    target_name: Option<&str>,
    build_mode_name: &str,
) -> Result<Option<serde_json::Value>, ArkdownError> {
    let profile = match RawBuildProfile::load(project_root) {
        Ok(profile) => profile,
        Err(ConfigError::FileNotFound(_)) => return Ok(None),
        Err(e) => return Err(ArkdownError::Io(std::io::Error::other(e.to_string()))),
    };
    let Some(raw_module) = profile
        .modules
        .into_iter()
        .find(|module| module.name == module_name)
    else {
        return Ok(None);
    };
    let target = target_name
        .and_then(|name| raw_module.targets.iter().find(|target| target.name == name))
        .or_else(|| {
            arkdown_config::build_profile::pick_effective_raw_target(
                &raw_module.targets,
                product_name,
                build_mode_name,
            )
        });
    let Some(target) = target else {
        return Ok(None);
    };
    let Some(config) = target.config.as_ref() else {
        return Ok(None);
    };
    Ok(config
        .distribution_filter
        .clone()
        .or_else(|| config.distro_filter.clone()))
}

// Target selection now uses arkdown_config::build_profile::pick_effective_raw_target
// in read_target_distribution_filter above.

fn distro_filter_field_name(compile_api_version: u32) -> &'static str {
    if compile_api_version == 9 {
        "distroFilter"
    } else {
        "distributionFilter"
    }
}

#[derive(Debug)]
struct ExistingDistroFilterProfile {
    profile_name: String,
    value: serde_json::Value,
}

fn find_existing_distro_filter_profile(
    module_root: &Path,
    metadata: &[MetadataEntry],
) -> Result<Option<ExistingDistroFilterProfile>, ArkdownError> {
    for metadata in metadata {
        let Some(profile_ref) = metadata
            .resource
            .as_deref()
            .and_then(ProfileResourceRef::parse)
        else {
            continue;
        };
        let profile_name = profile_ref.name();
        let profile_path = ProfileModuleLayout::new(module_root).profile_json(profile_name);
        let Ok(content) = fs_err::read_to_string(&profile_path) else {
            continue;
        };
        let value: serde_json::Value =
            json5::from_str(&content).map_err(|e| ArkdownError::ParseError {
                file: profile_path.display().to_string(),
                message: e.to_string(),
            })?;
        if value.get("distroFilter").is_some() || value.get("distributionFilter").is_some() {
            return Ok(Some(ExistingDistroFilterProfile {
                profile_name: profile_name.to_string(),
                value,
            }));
        }
    }
    Ok(None)
}

fn merge_json_objects(base: serde_json::Value, overlay: serde_json::Value) -> serde_json::Value {
    let mut merged = match base {
        serde_json::Value::Object(object) => object,
        _ => serde_json::Map::new(),
    };
    if let serde_json::Value::Object(overlay) = overlay {
        for (key, value) in overlay {
            merged.insert(key, value);
        }
    }
    serde_json::Value::Object(merged)
}

fn unique_profile_name(output_profile_dir: &Path, base_name: &str) -> Result<String, ArkdownError> {
    let entries = match fs_err::read_dir(output_profile_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(base_name.to_string());
        }
        Err(error) => return Err(ArkdownError::Io(error)),
    };

    let mut used_names = std::collections::HashSet::new();
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        if let Some(stem) = entry.path().file_stem().and_then(|stem| stem.to_str()) {
            used_names.insert(stem.to_string());
        }
    }

    if !used_names.contains(base_name) {
        return Ok(base_name.to_string());
    }

    for suffix in 1usize.. {
        let candidate = format!("{base_name}{suffix}");
        if !used_names.contains(&candidate) {
            return Ok(candidate);
        }
    }

    unreachable!("unbounded suffix search must find a free distro filter profile name")
}

fn append_profile_metadata_to_module_json(
    module_json_path: &Path,
    profile_name: &str,
) -> Result<(), ArkdownError> {
    let content = fs_err::read_to_string(module_json_path)?;
    let mut value: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| ArkdownError::ParseError {
            file: module_json_path.display().to_string(),
            message: e.to_string(),
        })?;
    let module = value
        .get_mut("module")
        .and_then(|value| value.as_object_mut())
        .ok_or_else(|| ArkdownError::ParseError {
            file: module_json_path.display().to_string(),
            message: "module.json is missing a module object".to_string(),
        })?;

    let resource = ProfileResourceRef::resource_name(profile_name);
    let metadata = module
        .entry("metadata".to_string())
        .or_insert_with(|| serde_json::Value::Array(Vec::new()));
    let serde_json::Value::Array(entries) = metadata else {
        return Err(ArkdownError::ParseError {
            file: module_json_path.display().to_string(),
            message: "module.metadata must be an array".to_string(),
        });
    };

    if !entries.iter().any(|entry| {
        entry
            .get("resource")
            .and_then(|value| value.as_str())
            .is_some_and(|existing| existing == resource)
    }) {
        entries.push(serde_json::json!({ "resource": resource }));
    }

    let json = serde_json::to_string_pretty(&value)
        .map_err(|e| ArkdownError::Io(std::io::Error::other(e.to_string())))?;
    fs_err::write(module_json_path, json.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata(resource: &str) -> MetadataEntry {
        MetadataEntry {
            name: String::new(),
            value: None,
            resource: Some(resource.to_string()),
        }
    }

    #[test]
    fn test_process_distro_filter_writes_new_profile() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let module_root = tmp.path().join("entry");
        fs_err::create_dir_all(module_root.join("src/main")).expect("create module root");
        fs_err::write(
            tmp.path().join("build-profile.json5"),
            r#"{
              app: { products: [{ name: "default" }] },
              modules: [{
                name: "entry",
                srcPath: "./entry",
                targets: [{
                  name: "default",
                  applyToProducts: ["default"],
                  config: {
                    distributionFilter: {
                      apiVersion: { policy: "include", value: [12] }
                    }
                  }
                }]
              }]
            }"#,
        )
        .expect("write build-profile");

        let out_dir = tmp.path().join("build/res/profile");
        let output = DistroFilterProcessor::new()
            .process_distro_filter_profile(DistroFilterProfileInput {
                project_root: tmp.path(),
                module_name: "entry",
                module_root: &module_root,
                product_name: "default",
                target_name: None,
                build_mode_name: Some("debug"),
                compile_api_version: 12,
                metadata: &[],
                output_profile_dir: &out_dir,
                module_json_path: None,
            })
            .expect("process distro")
            .expect("output");

        assert_eq!(output.profile_name, "ohos_module_distro_filter");
        let written = fs_err::read_to_string(output.profile_path).expect("read profile");
        let json: serde_json::Value = serde_json::from_str(&written).expect("json");
        assert_eq!(
            json["distributionFilter"]["apiVersion"]["value"],
            serde_json::json!([12])
        );
    }

    #[test]
    fn test_process_distro_filter_reuses_existing_profile() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let module_root = tmp.path().join("entry");
        let profile_dir = module_root.join("src/main/resources/base/profile");
        fs_err::create_dir_all(&profile_dir).expect("create profile dir");
        fs_err::write(
            profile_dir.join("existing_distro.json"),
            r#"{
              "distributionFilter": {
                "screenShape": { "policy": "include", "value": ["circle"] }
              }
            }"#,
        )
        .expect("write existing profile");
        fs_err::write(
            tmp.path().join("build-profile.json5"),
            r#"{
              app: { products: [{ name: "default" }] },
              modules: [{
                name: "entry",
                srcPath: "./entry",
                targets: [{
                  name: "default",
                  applyToProducts: ["default"],
                  config: {
                    distributionFilter: {
                      apiVersion: { policy: "include", value: [12] }
                    }
                  }
                }]
              }]
            }"#,
        )
        .expect("write build-profile");

        let out_dir = tmp.path().join("build/res/profile");
        let output = DistroFilterProcessor::new()
            .process_distro_filter_profile(DistroFilterProfileInput {
                project_root: tmp.path(),
                module_name: "entry",
                module_root: &module_root,
                product_name: "default",
                target_name: None,
                build_mode_name: Some("debug"),
                compile_api_version: 12,
                metadata: &[metadata("$profile:existing_distro")],
                output_profile_dir: &out_dir,
                module_json_path: None,
            })
            .expect("process distro")
            .expect("output");

        assert_eq!(output.profile_name, "existing_distro");
        let written = fs_err::read_to_string(output.profile_path).expect("read profile");
        let json: serde_json::Value = serde_json::from_str(&written).expect("json");
        assert_eq!(
            json["distributionFilter"]["screenShape"]["value"],
            serde_json::json!(["circle"])
        );
        assert_eq!(
            json["distributionFilter"]["apiVersion"]["value"],
            serde_json::json!([12])
        );
    }

    #[test]
    fn test_process_distro_filter_uses_unique_profile_name_and_updates_module_json() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let module_root = tmp.path().join("entry");
        fs_err::create_dir_all(module_root.join("src/main")).expect("create module root");
        fs_err::write(
            tmp.path().join("build-profile.json5"),
            r#"{
              app: { products: [{ name: "default" }] },
              modules: [{
                name: "entry",
                srcPath: "./entry",
                targets: [{
                  name: "default",
                  applyToProducts: ["default"],
                  config: {
                    distributionFilter: {
                      apiVersion: { policy: "include", value: [12] }
                    }
                  }
                }]
              }]
            }"#,
        )
        .expect("write build-profile");
        let out_dir = tmp.path().join("build/res/profile");
        fs_err::create_dir_all(&out_dir).expect("create out dir");
        fs_err::write(out_dir.join("ohos_module_distro_filter.json"), "{}")
            .expect("write existing output profile");
        let module_json_path = tmp.path().join("module.json");
        fs_err::write(&module_json_path, r#"{ "module": { "name": "entry" } }"#)
            .expect("write module json");

        let output = DistroFilterProcessor::new()
            .process_distro_filter_profile(DistroFilterProfileInput {
                project_root: tmp.path(),
                module_name: "entry",
                module_root: &module_root,
                product_name: "default",
                target_name: None,
                build_mode_name: Some("debug"),
                compile_api_version: 12,
                metadata: &[],
                output_profile_dir: &out_dir,
                module_json_path: Some(&module_json_path),
            })
            .expect("process distro")
            .expect("output");

        assert_eq!(output.profile_name, "ohos_module_distro_filter1");
        assert!(out_dir.join("ohos_module_distro_filter1.json").exists());
        let module_json: serde_json::Value =
            serde_json::from_str(&fs_err::read_to_string(module_json_path).expect("read module"))
                .expect("json");
        assert_eq!(
            module_json["module"]["metadata"][0]["resource"].as_str(),
            Some("$profile:ohos_module_distro_filter1")
        );
    }

    /// Verify that when a target has a specific `buildModeSet`, the distro filter
    /// only applies for that build mode (not for other modes).
    ///
    /// Addresses section 3.3: 'distro 多 target 校验/strict oracle'
    #[test]
    fn test_process_distro_filter_respects_build_mode_set() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let module_root = tmp.path().join("entry");
        fs_err::create_dir_all(module_root.join("src/main")).expect("create module root");
        fs_err::write(
            tmp.path().join("build-profile.json5"),
            r#"{
              app: { products: [{ name: "default" }] },
              modules: [{
                name: "entry",
                srcPath: "./entry",
                targets: [
                  {
                    name: "release-only",
                    applyToProducts: ["default"],
                    buildModeSet: [{ name: "release" }],
                    config: {
                      distributionFilter: {
                        apiVersion: { policy: "include", value: [12] }
                      }
                    }
                  }
                ]
              }]
            }"#,
        )
        .expect("write build-profile");

        let out_dir = tmp.path().join("build/res/profile");
        fs_err::create_dir_all(&out_dir).expect("create out dir");

        // Debug mode: the release-only target should NOT apply → no distro filter.
        let debug_result = DistroFilterProcessor::new()
            .process_distro_filter_profile(DistroFilterProfileInput {
                project_root: tmp.path(),
                module_name: "entry",
                module_root: &module_root,
                product_name: "default",
                target_name: None,
                build_mode_name: Some("debug"),
                compile_api_version: 12,
                metadata: &[],
                output_profile_dir: &out_dir,
                module_json_path: None,
            })
            .expect("process distro");
        assert!(
            debug_result.is_none(),
            "debug mode must not apply release-only distro filter; got: {:?}",
            debug_result
        );

        // Release mode: the release-only target SHOULD apply → distro filter written.
        let release_result = DistroFilterProcessor::new()
            .process_distro_filter_profile(DistroFilterProfileInput {
                project_root: tmp.path(),
                module_name: "entry",
                module_root: &module_root,
                product_name: "default",
                target_name: None,
                build_mode_name: Some("release"),
                compile_api_version: 12,
                metadata: &[],
                output_profile_dir: &out_dir,
                module_json_path: None,
            })
            .expect("process distro")
            .expect("distro filter must apply for release mode");
        assert_eq!(release_result.profile_name, "ohos_module_distro_filter");
    }

    #[test]
    fn test_distro_filter_processor_new_and_default() {
        let _p1 = DistroFilterProcessor::new();
        let _p2 = DistroFilterProcessor;
    }

    #[test]
    fn test_distro_filter_processor_returns_none_without_build_profile() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let module_root = tmp.path().join("entry");
        fs_err::create_dir_all(module_root.join("src/main")).expect("create module root");
        let out_dir = tmp.path().join("build/res/profile");
        let processor = DistroFilterProcessor::new();
        let result = processor
            .process_distro_filter_profile(DistroFilterProfileInput {
                project_root: tmp.path(),
                module_name: "entry",
                module_root: &module_root,
                product_name: "default",
                target_name: None,
                build_mode_name: None,
                compile_api_version: 12,
                metadata: &[],
                output_profile_dir: &out_dir,
                module_json_path: None,
            })
            .expect("process distro");
        assert!(result.is_none());
    }
}
