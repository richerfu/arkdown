//! `ProcessShareConfig` — merge `crossAppSharedConfig` from HAR dependencies.
//!
//! Mirrors the hvigor `ProcessShareConfig` task
//! (`hvigor-ohos-plugin/src/tasks/process-share-config.js`).
//!
//! ## What it does
//!
//! 1. Consumes the HAR dependency roots selected by the build session.
//! 2. Reads each HAR's `module.json5` (or the pre-processed `module.json`) and
//!    extracts its `module.crossAppSharedConfig` array.
//! 3. Fills in `packageName` from the package directory name when the entry
//!    does not already carry one.
//! 4. Validates that no two entries share the same `uri` across different packages
//!    (duplicate URIs would cause a runtime routing conflict).
//! 5. Writes the merged result as `{"crossAppSharedConfig": [...]}` to the
//!    intermediates path supplied by the caller.
//!
//! ## Output path (hvigor convention)
//!
//! ```text
//! <module_root>/build/default/intermediates/share_config/default/temp-share-config.json
//! ```

use rustc_hash::{FxHashMap, FxHashSet};
use std::path::{Path, PathBuf};

use arkdown_core::{ArkdownError, CrossAppSharedConfigEntry};

use crate::layout::ProfileModuleLayout;
use crate::resource_ref::ProfileResourceRef;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Input for the share-config merge pass.
pub struct ShareConfigInput<'a> {
    /// Root directory of the consuming module (e.g. `/project/entry`).
    pub module_root: &'a Path,
    /// HAR dependencies returned by hvigor's `getHarDependencies()` equivalent.
    /// HSP dependencies are intentionally absent: `ProcessShareConfig` does
    /// not collect share config from `getHspDependencies()`.
    pub dependency_packages: Vec<ShareConfigDependencyInput>,
    /// `crossAppSharedConfig` entries declared by the module itself (may be empty
    /// for HAP/HAR consumers; non-empty for modules that export shared components).
    pub self_entries: Vec<CrossAppSharedConfigEntry>,
    /// Optional `$profile:<name>` reference declared by the module itself.
    /// Hvigor reads this profile and merges its `crossAppSharedConfig` array
    /// before collecting dependency HAR entries.
    pub self_profile_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ShareConfigDependencyInput {
    pub package_name: String,
    pub package_root: PathBuf,
    /// Whether hvigor resolved this dependency to a workspace module model.
    /// Remote/install-state HARs read their compiled `module.json` first;
    /// local modules read the source `module.json5` selected by the target.
    pub is_local: bool,
}

impl ShareConfigInput<'_> {
    /// Collect `crossAppSharedConfig` entries from the session-selected HAR
    /// dependencies, merge them with `self_entries`, validate for
    /// duplicate URIs, and return the merged list.
    ///
    /// Errors are returned for I/O failures only; missing or malformed `module.json5`
    /// files in individual dependencies are silently skipped (matching hvigor behaviour).
    pub fn merge(self) -> Result<ShareConfigOutput, ArkdownError> {
        let mut all_entries: Vec<CrossAppSharedConfigEntry> = Vec::new();

        // Include the module's own entries first.
        all_entries.extend(self.self_entries);
        if let Some(profile_name) = self.self_profile_name.as_deref() {
            all_entries.extend(Self::read_profile_entries_from_module_root(
                self.module_root,
                profile_name,
            ));
        }
        let self_package_name = Self::module_package_name(self.module_root);
        for entry in &mut all_entries {
            if entry.package_name.is_empty() {
                entry.package_name = self_package_name.clone();
            }
            if entry.module_node_dir.is_none() {
                entry.module_node_dir = Some(self.module_root.to_path_buf());
            }
        }

        for dependency in self.dependency_packages {
            let entries = Self::read_cross_app_shared_config_from_pkg(
                &dependency.package_root,
                &dependency.package_name,
                dependency.is_local,
            );
            all_entries.extend(entries);
        }

        // Validate: no two entries from different packages may share the same URI.
        // (Duplicate URIs within the same package are allowed — hvigor only errors
        // on cross-package URI collisions.)
        ShareConfigInput::check_duplicate_uris(&all_entries)?;

        Ok(ShareConfigOutput {
            entries: all_entries,
        })
    }
}

/// Merged `crossAppSharedConfig` result.
#[derive(Debug, Clone)]
pub struct ShareConfigOutput {
    /// All merged entries ready to be serialized to `temp-share-config.json`.
    pub entries: Vec<CrossAppSharedConfigEntry>,
}

impl ShareConfigOutput {
    /// Produce the runtime resource written by hvigor's
    /// `CompileResource.processShareConfig()`. Build-only ownership fields are
    /// deliberately excluded from the packaged profile.
    pub fn compiled_resource_profile(&self) -> serde_json::Value {
        serde_json::json!({
            "crossAppSharedConfig": self
                .entries
                .iter()
                .map(CompiledCrossAppSharedConfigEntry::from_entry)
                .collect::<Vec<_>>()
        })
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CompiledCrossAppSharedConfigEntry<'a> {
    uri: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    src_entry: Option<&'a str>,
    #[serde(flatten)]
    additional_fields: &'a serde_json::Map<String, serde_json::Value>,
}

impl<'a> CompiledCrossAppSharedConfigEntry<'a> {
    fn from_entry(entry: &'a CrossAppSharedConfigEntry) -> Self {
        Self {
            uri: &entry.uri,
            src_entry: entry.src_entry.as_deref(),
            additional_fields: &entry.additional_fields,
        }
    }
}

/// Processes `crossAppSharedConfig` entries from HAR dependencies.
///
/// Mirrors the hvigor `ProcessShareConfig` task
/// (`hvigor-ohos-plugin/src/tasks/process-share-config.js`):
/// consumes typed dependency roots, collects entries from HAR packages,
/// validates for duplicate URIs, and writes the result to intermediates.
pub struct ShareConfigProcessor;

impl ShareConfigProcessor {
    /// Create a new processor.
    pub fn new() -> Self {
        ShareConfigProcessor
    }

    /// Collect `crossAppSharedConfig` entries from every supplied HAR
    /// dependency, merge them with `self_entries`, validate for
    /// duplicate URIs, and return the merged list.
    ///
    /// Returns `Err` when two packages declare the same URI.
    pub fn merge_share_config(
        &self,
        input: ShareConfigInput<'_>,
    ) -> Result<ShareConfigOutput, ArkdownError> {
        input.merge()
    }

    /// Write the merged share config to `output_path` as
    /// `{"crossAppSharedConfig": [...]}`.
    ///
    /// Creates parent directories if they do not exist.
    pub fn write_share_config(
        &self,
        output: &ShareConfigOutput,
        output_path: &Path,
    ) -> Result<(), std::io::Error> {
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let value = serde_json::json!({ "crossAppSharedConfig": output.entries });
        let json =
            serde_json::to_string(&value).map_err(|e| std::io::Error::other(e.to_string()))?;
        std::fs::write(output_path, json)?;
        Ok(())
    }

    /// Apply hvigor's `CompileResource.writeShareConfig()` handoff: write the
    /// runtime-only profile and point the compiled `module.json` at it.
    pub fn write_compiled_share_config(
        &self,
        output: &ShareConfigOutput,
        profile_name: &str,
        profile_path: &Path,
        module_json_path: &Path,
    ) -> Result<(), ArkdownError> {
        if let Some(parent) = profile_path.parent() {
            fs_err::create_dir_all(parent)?;
        }
        let profile = output.compiled_resource_profile();
        let profile_json =
            serde_json::to_vec(&profile).map_err(|error| ArkdownError::ParseError {
                file: profile_path.display().to_string(),
                message: error.to_string(),
            })?;
        fs_err::write(profile_path, profile_json)?;

        let content = fs_err::read_to_string(module_json_path)?;
        let mut module_json: serde_json::Value =
            serde_json::from_str(&content).map_err(|error| ArkdownError::ParseError {
                file: module_json_path.display().to_string(),
                message: error.to_string(),
            })?;
        let module = module_json
            .get_mut("module")
            .and_then(serde_json::Value::as_object_mut)
            .ok_or_else(|| ArkdownError::ParseError {
                file: module_json_path.display().to_string(),
                message: "missing object field 'module'".to_string(),
            })?;
        module.insert(
            "crossAppSharedConfig".to_string(),
            serde_json::Value::String(format!("$profile:{profile_name}")),
        );
        let module_json =
            serde_json::to_vec(&module_json).map_err(|error| ArkdownError::ParseError {
                file: module_json_path.display().to_string(),
                message: error.to_string(),
            })?;
        fs_err::write(module_json_path, module_json)?;
        Ok(())
    }
}

impl Default for ShareConfigProcessor {
    fn default() -> Self {
        ShareConfigProcessor::new()
    }
}

// ---------------------------------------------------------------------------
// Private associated functions on ShareConfigInput
// ---------------------------------------------------------------------------

impl ShareConfigInput<'_> {
    /// Read `crossAppSharedConfig` from a single installed package directory.
    ///
    /// Tries `src/main/module.json5` first (source layout), then `module.json`
    /// (pre-built layout) at the package root.
    fn read_cross_app_shared_config_from_pkg(
        pkg_dir: &Path,
        pkg_name: &str,
        is_local: bool,
    ) -> Vec<CrossAppSharedConfigEntry> {
        // Installed hvigor 6.24.3 uses the project module model for local
        // dependencies, but `getModuleObjByRemoteDependency()` selects the
        // compiled src/main/module.json before module.json5 for installed HARs.
        let layout = ProfileModuleLayout::new(pkg_dir);
        let candidates = if is_local {
            [
                layout.module_json5(),
                layout.module_json(),
                layout.package_module_json5(),
                layout.package_module_json(),
            ]
        } else {
            [
                layout.module_json(),
                layout.module_json5(),
                layout.package_module_json(),
                layout.package_module_json5(),
            ]
        };

        for candidate in &candidates {
            if !candidate.exists() {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(candidate) else {
                continue;
            };
            // Parse as JSON5 (also valid for plain JSON).
            let Ok(raw): Result<serde_json::Value, _> = json5::from_str(&content) else {
                continue;
            };

            let Some(cross_app_shared_config) = raw.pointer("/module/crossAppSharedConfig") else {
                return vec![];
            };

            let mut entries = if let Some(arr) = cross_app_shared_config.as_array() {
                Self::entries_from_array(arr)
            } else if let Some(profile_ref) = cross_app_shared_config.as_str() {
                Self::read_profile_entries_from_pkg(pkg_dir, profile_ref)
            } else {
                vec![]
            };

            // Fill in packageName from the directory name when absent.
            for entry in &mut entries {
                if entry.package_name.is_empty() {
                    entry.package_name = pkg_name.to_string();
                }
                entry.module_node_dir = Some(pkg_dir.to_path_buf());
            }

            return entries;
        }

        vec![]
    }

    fn entries_from_array(arr: &[serde_json::Value]) -> Vec<CrossAppSharedConfigEntry> {
        arr.iter()
            .filter_map(|item| {
                serde_json::from_value::<CrossAppSharedConfigEntry>(item.clone()).ok()
            })
            .collect()
    }

    fn read_profile_entries_from_module_root(
        module_root: &Path,
        profile_ref: &str,
    ) -> Vec<CrossAppSharedConfigEntry> {
        let Some(profile_name) = Self::parse_profile_name(profile_ref) else {
            return vec![];
        };
        let profile_path = ProfileModuleLayout::new(module_root).profile_json(profile_name);
        Self::read_profile_entries(&profile_path)
    }

    fn read_profile_entries_from_pkg(
        pkg_dir: &Path,
        profile_ref: &str,
    ) -> Vec<CrossAppSharedConfigEntry> {
        let Some(profile_name) = Self::parse_profile_name(profile_ref) else {
            return vec![];
        };
        let layout = ProfileModuleLayout::new(pkg_dir);
        for profile_path in [
            layout.profile_json(profile_name),
            layout.prebuilt_profile_json(profile_name),
            layout.compiled_res_profile_json(profile_name),
        ] {
            if profile_path.is_file() {
                return Self::read_profile_entries(&profile_path);
            }
        }
        vec![]
    }

    fn parse_profile_name(profile_ref: &str) -> Option<&str> {
        ProfileResourceRef::profile_name_or_bare(profile_ref)
    }

    fn read_profile_entries(profile_path: &Path) -> Vec<CrossAppSharedConfigEntry> {
        let Ok(content) = std::fs::read_to_string(profile_path) else {
            return vec![];
        };
        let Ok(raw): Result<serde_json::Value, _> = serde_json::from_str(&content) else {
            return vec![];
        };
        raw.get("crossAppSharedConfig")
            .and_then(|value| value.as_array())
            .map(|arr| Self::entries_from_array(arr))
            .unwrap_or_default()
    }

    fn module_package_name(module_root: &Path) -> String {
        let pkg_path = module_root.join("oh-package.json5");
        if let Ok(content) = std::fs::read_to_string(pkg_path) {
            let parsed: Result<serde_json::Value, _> = json5::from_str(&content);
            if let Ok(value) = parsed {
                if let Some(name) = value.get("name").and_then(|name| name.as_str()) {
                    return name.to_string();
                }
            }
        }
        module_root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string()
    }

    /// Returns `Err` when two entries from different packages share the same URI.
    /// Hvigor calls `printErrorExit` here; arkdown now matches that hard-error behaviour.
    fn check_duplicate_uris(entries: &[CrossAppSharedConfigEntry]) -> Result<(), ArkdownError> {
        // Map URI → list of package names that declare it.
        let mut uri_to_packages: FxHashMap<&str, Vec<&str>> = FxHashMap::default();
        for entry in entries {
            uri_to_packages
                .entry(entry.uri.as_str())
                .or_default()
                .push(entry.package_name.as_str());
        }

        for (uri, packages) in &uri_to_packages {
            if packages.len() > 1 {
                // Deduplicate package names before emitting the error.
                let mut unique: Vec<&&str> = packages
                    .iter()
                    .collect::<FxHashSet<_>>()
                    .into_iter()
                    .collect();
                unique.sort();
                return Err(ArkdownError::Bundler(format!(
                    "ProcessShareConfig: duplicate crossAppSharedConfig URI '{}' declared by packages: {:?}",
                    uri, unique
                )));
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_module_json5(dir: &Path, content: &str) {
        let src_main = dir.join("src/main");
        fs::create_dir_all(&src_main).unwrap();
        fs::write(src_main.join("module.json5"), content).unwrap();
    }

    fn dependency(package_name: &str, package_root: &Path) -> ShareConfigDependencyInput {
        ShareConfigDependencyInput {
            package_name: package_name.to_string(),
            package_root: package_root.to_path_buf(),
            is_local: false,
        }
    }

    #[test]
    fn test_merge_share_config_no_oh_modules() {
        let tmp = tempfile::tempdir().unwrap();
        let output = ShareConfigProcessor::new()
            .merge_share_config(ShareConfigInput {
                module_root: tmp.path(),
                dependency_packages: vec![],
                self_entries: vec![],
                self_profile_name: None,
            })
            .unwrap();
        assert!(output.entries.is_empty());
    }

    #[test]
    fn test_merge_share_config_collects_har_entries() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("oh-package.json5"),
            r#"{ dependencies: { "my-har": "1.0.0" } }"#,
        )
        .unwrap();
        let oh_modules = tmp.path().join("oh_modules");
        let har_pkg = oh_modules.join("my-har");
        write_module_json5(
            &har_pkg,
            r#"{
                "module": {
                    "name": "my-har",
                    "type": "har",
                    "crossAppSharedConfig": [
                        { "uri": "myapp://shared/Foo", "srcEntry": "./ets/Foo.ets" }
                    ]
                }
            }"#,
        );

        let output = ShareConfigProcessor::new()
            .merge_share_config(ShareConfigInput {
                module_root: tmp.path(),
                dependency_packages: vec![dependency("my-har", &har_pkg)],
                self_entries: vec![],
                self_profile_name: None,
            })
            .unwrap();

        assert_eq!(output.entries.len(), 1);
        assert_eq!(output.entries[0].uri, "myapp://shared/Foo");
        assert_eq!(output.entries[0].package_name, "my-har");
    }

    #[test]
    fn test_merge_share_config_does_not_rediscover_hsp_dependencies() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("oh-package.json5"),
            r#"{ dependencies: { "my-hsp": "1.0.0" } }"#,
        )
        .unwrap();
        let oh_modules = tmp.path().join("oh_modules");
        let hsp_pkg = oh_modules.join("my-hsp");
        write_module_json5(
            &hsp_pkg,
            r#"{
                "module": {
                    "name": "my-hsp",
                    "type": "shared",
                    "crossAppSharedConfig": [
                        { "uri": "myapp://shared/Bar", "srcEntry": "./ets/Bar.ets" }
                    ]
                }
            }"#,
        );

        let output = ShareConfigProcessor::new()
            .merge_share_config(ShareConfigInput {
                module_root: tmp.path(),
                dependency_packages: vec![],
                self_entries: vec![],
                self_profile_name: None,
            })
            .unwrap();

        assert!(
            output.entries.is_empty(),
            "HSP packages are not part of hvigor ProcessShareConfig's HAR dependency input"
        );
    }

    #[test]
    fn test_merge_share_config_includes_self_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let self_entry = CrossAppSharedConfigEntry {
            uri: "myapp://shared/Self".to_string(),
            src_entry: Some("./ets/Self.ets".to_string()),
            additional_fields: serde_json::Map::new(),
            module_node_dir: None,
            package_name: "self-har".to_string(),
        };
        let output = ShareConfigProcessor::new()
            .merge_share_config(ShareConfigInput {
                module_root: tmp.path(),
                dependency_packages: vec![],
                self_entries: vec![self_entry.clone()],
                self_profile_name: None,
            })
            .unwrap();
        assert_eq!(output.entries.len(), 1);
        assert_eq!(output.entries[0].uri, self_entry.uri);
        assert_eq!(output.entries[0].src_entry, self_entry.src_entry);
        assert_eq!(output.entries[0].package_name, self_entry.package_name);
        assert_eq!(
            output.entries[0].module_node_dir.as_deref(),
            Some(tmp.path())
        );
    }

    #[test]
    fn test_merge_share_config_reads_self_profile_ref() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("oh-package.json5"),
            r#"{ name: "self-har" }"#,
        )
        .unwrap();

        let profile_dir = tmp.path().join("src/main/resources/base/profile");
        fs::create_dir_all(&profile_dir).unwrap();
        fs::write(
            profile_dir.join("cross_app_shared_config.json"),
            r#"{
                "crossAppSharedConfig": [
                    { "uri": "myapp://shared/Profile", "srcEntry": "./ets/Profile.ets" }
                ]
            }"#,
        )
        .unwrap();

        let output = ShareConfigProcessor::new()
            .merge_share_config(ShareConfigInput {
                module_root: tmp.path(),
                dependency_packages: vec![],
                self_entries: vec![],
                self_profile_name: Some("cross_app_shared_config".to_string()),
            })
            .unwrap();

        assert_eq!(output.entries.len(), 1);
        assert_eq!(output.entries[0].uri, "myapp://shared/Profile");
        assert_eq!(
            output.entries[0].src_entry.as_deref(),
            Some("./ets/Profile.ets")
        );
        assert_eq!(output.entries[0].package_name, "self-har");
    }

    #[test]
    fn test_merge_share_config_reads_dependency_profile_ref() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("oh-package.json5"),
            r#"{ dependencies: { "profile-har": "1.0.0" } }"#,
        )
        .unwrap();
        let oh_modules = tmp.path().join("oh_modules");
        let har_pkg = oh_modules.join("profile-har");
        write_module_json5(
            &har_pkg,
            r#"{
                "module": {
                    "name": "profile-har",
                    "type": "har",
                    "crossAppSharedConfig": "$profile:cross_app_shared_config"
                }
            }"#,
        );
        let profile_dir = har_pkg.join("src/main/resources/base/profile");
        fs::create_dir_all(&profile_dir).unwrap();
        fs::write(
            profile_dir.join("cross_app_shared_config.json"),
            r#"{
                "crossAppSharedConfig": [
                    { "uri": "myapp://shared/DepProfile" }
                ]
            }"#,
        )
        .unwrap();

        let output = ShareConfigProcessor::new()
            .merge_share_config(ShareConfigInput {
                module_root: tmp.path(),
                dependency_packages: vec![dependency("profile-har", &har_pkg)],
                self_entries: vec![],
                self_profile_name: None,
            })
            .unwrap();

        assert_eq!(output.entries.len(), 1);
        assert_eq!(output.entries[0].uri, "myapp://shared/DepProfile");
        assert_eq!(output.entries[0].package_name, "profile-har");
    }

    #[test]
    fn remote_har_uses_compiled_module_json_before_shipped_source_profile() {
        let tmp = tempfile::tempdir().unwrap();
        let har_pkg = tmp.path().join("oh_modules/remote-har");
        write_module_json5(
            &har_pkg,
            r#"{
                "module": {
                    "name": "remote-har",
                    "type": "har",
                    "crossAppSharedConfig": "$profile:cross_app_shared_config"
                }
            }"#,
        );
        fs::write(
            har_pkg.join("src/main/module.json"),
            r#"{"module":{"name":"remote-har","type":"har"}}"#,
        )
        .unwrap();
        let profile_dir = har_pkg.join("src/main/resources/base/profile");
        fs::create_dir_all(&profile_dir).unwrap();
        fs::write(
            profile_dir.join("cross_app_shared_config.json"),
            r#"{"crossAppSharedConfig":[{"uri":"arkdown://remote/source-only"}]}"#,
        )
        .unwrap();

        let remote_output = ShareConfigProcessor::new()
            .merge_share_config(ShareConfigInput {
                module_root: tmp.path(),
                dependency_packages: vec![ShareConfigDependencyInput {
                    package_name: "remote-har".to_string(),
                    package_root: har_pkg.clone(),
                    is_local: false,
                }],
                self_entries: vec![],
                self_profile_name: None,
            })
            .unwrap();
        assert!(
            remote_output.entries.is_empty(),
            "DependencyManager.getModuleObjByRemoteDependency treats compiled module.json as authoritative"
        );

        let local_output = ShareConfigProcessor::new()
            .merge_share_config(ShareConfigInput {
                module_root: tmp.path(),
                dependency_packages: vec![ShareConfigDependencyInput {
                    package_name: "remote-har".to_string(),
                    package_root: har_pkg,
                    is_local: true,
                }],
                self_entries: vec![],
                self_profile_name: None,
            })
            .unwrap();
        assert_eq!(local_output.entries.len(), 1);
    }

    #[test]
    fn test_write_share_config_creates_file() {
        let tmp = tempfile::tempdir().unwrap();
        let out_path = tmp
            .path()
            .join("intermediates/share_config/default/temp-share-config.json");

        let output = ShareConfigOutput {
            entries: vec![CrossAppSharedConfigEntry {
                uri: "myapp://shared/Comp".to_string(),
                src_entry: None,
                additional_fields: serde_json::Map::new(),
                module_node_dir: Some(PathBuf::from("/workspace/oh_modules/pkg-a")),
                package_name: "pkg-a".to_string(),
            }],
        };
        ShareConfigProcessor::new()
            .write_share_config(&output, &out_path)
            .unwrap();

        assert!(out_path.exists());
        let content = fs::read_to_string(&out_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let arr = parsed["crossAppSharedConfig"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["uri"], "myapp://shared/Comp");
        assert_eq!(arr[0]["packageName"], "pkg-a");
        assert_eq!(
            arr[0]["moduleNodeDir"],
            serde_json::Value::String("/workspace/oh_modules/pkg-a".to_string())
        );
    }

    #[test]
    fn test_write_compiled_share_config_strips_ownership_and_updates_module_profile_ref() {
        let tmp = tempfile::tempdir().unwrap();
        let profile_path = tmp
            .path()
            .join("resources/base/profile/default_share_config.json");
        let module_json_path = tmp.path().join("module.json");
        fs::write(
            &module_json_path,
            r#"{"module":{"name":"entry","type":"entry"}}"#,
        )
        .unwrap();
        let output = ShareConfigOutput {
            entries: vec![CrossAppSharedConfigEntry {
                uri: "myapp://shared/Comp".to_string(),
                src_entry: Some("./ets/Comp.ets".to_string()),
                additional_fields: serde_json::Map::from_iter([(
                    "allowList".to_string(),
                    serde_json::json!(["com.example.consumer"]),
                )]),
                module_node_dir: Some(PathBuf::from("/workspace/oh_modules/pkg-a")),
                package_name: "pkg-a".to_string(),
            }],
        };

        ShareConfigProcessor::new()
            .write_compiled_share_config(
                &output,
                "default_share_config",
                &profile_path,
                &module_json_path,
            )
            .unwrap();

        let profile: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(profile_path).unwrap()).unwrap();
        assert_eq!(
            profile,
            serde_json::json!({
                "crossAppSharedConfig": [{
                    "uri": "myapp://shared/Comp",
                    "srcEntry": "./ets/Comp.ets",
                    "allowList": ["com.example.consumer"]
                }]
            })
        );
        let entry = profile["crossAppSharedConfig"][0].as_object().unwrap();
        assert!(!entry.contains_key("moduleNodeDir"));
        assert!(!entry.contains_key("packageName"));

        let module_json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(module_json_path).unwrap()).unwrap();
        assert_eq!(
            module_json["module"]["crossAppSharedConfig"],
            "$profile:default_share_config"
        );
    }

    #[test]
    fn test_write_share_config_empty_array() {
        let tmp = tempfile::tempdir().unwrap();
        let out_path = tmp.path().join("temp-share-config.json");
        let output = ShareConfigOutput { entries: vec![] };
        ShareConfigProcessor::new()
            .write_share_config(&output, &out_path)
            .unwrap();

        let content = fs::read_to_string(&out_path).unwrap();
        assert_eq!(content, r#"{"crossAppSharedConfig":[]}"#);
    }

    #[test]
    fn test_merge_share_config_fills_package_name_from_dir() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("oh-package.json5"),
            r#"{ dependencies: { "auto-named-har": "1.0.0" } }"#,
        )
        .unwrap();
        let oh_modules = tmp.path().join("oh_modules");
        let har_pkg = oh_modules.join("auto-named-har");
        write_module_json5(
            &har_pkg,
            r#"{
                "module": {
                    "name": "auto-named-har",
                    "type": "har",
                    "crossAppSharedConfig": [
                        { "uri": "myapp://shared/X" }
                    ]
                }
            }"#,
        );

        let output = ShareConfigProcessor::new()
            .merge_share_config(ShareConfigInput {
                module_root: tmp.path(),
                dependency_packages: vec![dependency("auto-named-har", &har_pkg)],
                self_entries: vec![],
                self_profile_name: None,
            })
            .unwrap();

        assert_eq!(output.entries.len(), 1);
        assert_eq!(
            output.entries[0].package_name, "auto-named-har",
            "packageName must come from the typed dependency input"
        );
    }

    #[test]
    fn test_merge_share_config_follows_declared_transitive_scoped_dependencies() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("oh-package.json5"),
            r#"{ dependencies: { "source-har": "1.0.0" } }"#,
        )
        .unwrap();

        let source_har = tmp.path().join("oh_modules/source-har");
        fs::create_dir_all(&source_har).unwrap();
        fs::write(
            source_har.join("oh-package.json5"),
            r#"{ dependencies: { "@scope/shared-hsp": "1.0.0" } }"#,
        )
        .unwrap();
        write_module_json5(
            &source_har,
            r#"{
                "module": {
                    "name": "source-har",
                    "type": "har"
                }
            }"#,
        );

        let scoped_har = source_har.join("oh_modules/@scope/shared-har");
        write_module_json5(
            &scoped_har,
            r#"{
                "module": {
                    "name": "shared-har",
                    "type": "har",
                    "crossAppSharedConfig": [
                        { "uri": "myapp://shared/Scoped", "srcEntry": "./ets/Scoped.ets" }
                    ]
                }
            }"#,
        );

        let undeclared_hsp = tmp.path().join("oh_modules/undeclared-hsp");
        write_module_json5(
            &undeclared_hsp,
            r#"{
                "module": {
                    "name": "undeclared-hsp",
                    "type": "shared",
                    "crossAppSharedConfig": [
                        { "uri": "myapp://shared/Undeclared" }
                    ]
                }
            }"#,
        );

        let output = ShareConfigProcessor::new()
            .merge_share_config(ShareConfigInput {
                module_root: tmp.path(),
                dependency_packages: vec![dependency("@scope/shared-har", &scoped_har)],
                self_entries: vec![],
                self_profile_name: None,
            })
            .unwrap();

        assert_eq!(output.entries.len(), 1);
        assert_eq!(output.entries[0].uri, "myapp://shared/Scoped");
        assert_eq!(output.entries[0].package_name, "@scope/shared-har");
    }

    #[test]
    fn test_share_config_processor_new_and_default() {
        let _p1 = ShareConfigProcessor::new();
        let _p2 = ShareConfigProcessor;
    }

    #[test]
    fn test_share_config_processor_merge_no_deps() {
        let tmp = tempfile::tempdir().unwrap();
        let processor = ShareConfigProcessor::new();
        let output = processor
            .merge_share_config(ShareConfigInput {
                module_root: tmp.path(),
                dependency_packages: vec![],
                self_entries: vec![],
                self_profile_name: None,
            })
            .unwrap();
        assert!(output.entries.is_empty());
    }

    #[test]
    fn test_share_config_processor_write() {
        let tmp = tempfile::tempdir().unwrap();
        let out_path = tmp.path().join("temp-share-config.json");
        let output = ShareConfigOutput { entries: vec![] };
        let processor = ShareConfigProcessor::new();
        processor.write_share_config(&output, &out_path).unwrap();
        let content = fs::read_to_string(&out_path).unwrap();
        assert_eq!(content, r#"{"crossAppSharedConfig":[]}"#);
    }
}
