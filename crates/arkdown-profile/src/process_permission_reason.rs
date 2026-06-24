//! Permission reason resource processing.
//!
//! Mirrors the core of hvigor `MergeProfile.processResourceStr()`:
//!
//! - HSP/HAR modules collect `requestPermissions[].reason` string references
//!   and copy only the referenced string resources into `resource_str`.
//! - HAP modules merge dependency HSP/HAR string resources into their own
//!   `resource_str` intermediate.
//!
//! ```text
//! <module>/build/default/intermediates/res/<target>/resource_str
//! ```

use indexmap::IndexMap;
use rustc_hash::FxHashSet;
use serde::Deserialize;
use std::collections::VecDeque;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use arkdown_core::ArkdownError;

const STRING_REF_PREFIX: &str = "$string:";

/// Input for permission reason resource processing.
pub struct PermissionReasonResourceInput<'a> {
    /// Root directory of the module, e.g. `/project/shared`.
    pub module_root: &'a Path,
    /// Target name, normally `default`.
    pub target_name: &'a str,
    /// `module.requestPermissions` from `module.json5` or merged profile.
    pub request_permissions: &'a [serde_json::Value],
}

/// Input for HAP dependency `resource_str` merging.
pub struct DependencyPermissionReasonResourceInput<'a> {
    /// Root directory of the consuming module, e.g. `/project/entry`.
    pub module_root: &'a Path,
    /// Root directory of the project.
    pub project_root: &'a Path,
    /// Target name, normally `default`.
    pub target_name: &'a str,
}

#[derive(Deserialize)]
struct DependencyPackageJson {
    #[serde(default)]
    dependencies: IndexMap<String, String>,
}

/// Processes permission reason string resources for a module.
///
/// Mirrors hvigor `MergeProfile.processResourceStr()`:
/// - HSP/HAR modules collect `requestPermissions[].reason` string references
///   and copy only the referenced string resources into `resource_str`.
/// - HAP modules merge dependency HSP/HAR string resources into their own
///   `resource_str` intermediate.
pub struct PermissionReasonProcessor;

impl PermissionReasonProcessor {
    /// Create a new processor.
    pub fn new() -> Self {
        PermissionReasonProcessor
    }

    /// Copy only permission reason string resources to hvigor's `resource_str`
    /// intermediate directory.
    ///
    /// Returns `Ok(None)` when there are no `$string:` reason references or no
    /// matching string resources.
    pub fn process_permission_reason_resources(
        &self,
        input: PermissionReasonResourceInput<'_>,
    ) -> Result<Option<PathBuf>, ArkdownError> {
        let layout = PermissionReasonLayout::new(input.module_root, input.target_name);
        let output_dir = layout.resource_str_output_dir();

        if output_dir.exists() {
            std::fs::remove_dir_all(&output_dir)?;
        }

        let reason_names = PermissionReasonNames::new(input.request_permissions).collect();
        if reason_names.is_empty() {
            return Ok(None);
        }

        let resources_dir = layout.resources_dir();
        if !resources_dir.is_dir() {
            return Ok(None);
        }

        let reason_set: FxHashSet<_> = reason_names.iter().map(String::as_str).collect();
        let mut wrote_any = false;
        for entry in std::fs::read_dir(&resources_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let qualifier = entry.file_name();
            if ResourceQualifier::new(&qualifier).is_raw_or_res_file() {
                continue;
            }

            let source_root = entry.path();
            let source = ResourceStringLayout::new(&source_root).string_json();
            if !source.exists() {
                continue;
            }
            let Some(filtered) = StringJsonDocument::filter_reason_names(&source, &reason_set)?
            else {
                continue;
            };

            let output = ResourceStringLayout::new(&output_dir).qualified_string_json(&qualifier);
            if let Some(parent) = output.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(
                &output,
                serde_json::to_vec(&filtered)
                    .expect("serializing permission reason string resources should not fail"),
            )?;
            wrote_any = true;
        }

        Ok(wrote_any.then_some(output_dir))
    }

    /// Merge dependency HSP/HAR `resource_str` data into a HAP module.
    ///
    /// This matches hvigor `MergeProfile.initResourceStr()` at the path/merge
    /// level: each dependency contributes `string.json`, and duplicate string
    /// names keep the first value already written.
    pub fn merge_dependency_permission_reason_resources(
        &self,
        input: DependencyPermissionReasonResourceInput<'_>,
    ) -> Result<Option<PathBuf>, ArkdownError> {
        let output_dir = PermissionReasonLayout::new(input.module_root, input.target_name)
            .resource_str_output_dir();
        if output_dir.exists() {
            std::fs::remove_dir_all(&output_dir)?;
        }

        let package_dirs =
            DependencyRootScanner::new(input.module_root, input.project_root).collect_roots();
        let mut wrote_any = false;
        for package_dir in package_dirs {
            for source_root in DependencyResourceLayout::new(&package_dir, input.target_name)
                .resource_str_sources()
            {
                wrote_any |= ResourceStrRootMerger::new(&source_root, &output_dir).merge()?;
            }
        }

        Ok(wrote_any.then_some(output_dir))
    }
}

impl Default for PermissionReasonProcessor {
    fn default() -> Self {
        PermissionReasonProcessor::new()
    }
}

struct PermissionReasonLayout<'a> {
    module_root: &'a Path,
    target_name: &'a str,
}

impl<'a> PermissionReasonLayout<'a> {
    fn new(module_root: &'a Path, target_name: &'a str) -> Self {
        Self {
            module_root,
            target_name,
        }
    }

    fn resource_str_output_dir(&self) -> PathBuf {
        self.module_root
            .join("build")
            .join("default")
            .join("intermediates")
            .join("res")
            .join(self.target_name)
            .join("resource_str")
    }

    fn resources_dir(&self) -> PathBuf {
        self.module_root.join("src").join("main").join("resources")
    }
}

struct ResourceStringLayout<'a> {
    root: &'a Path,
}

impl<'a> ResourceStringLayout<'a> {
    fn new(root: &'a Path) -> Self {
        Self { root }
    }

    fn string_json(&self) -> PathBuf {
        self.root.join("element").join("string.json")
    }

    fn qualified_string_json(&self, qualifier: &OsStr) -> PathBuf {
        self.root
            .join(qualifier)
            .join("element")
            .join("string.json")
    }
}

struct ResourceQualifier<'a> {
    name: &'a OsStr,
}

impl<'a> ResourceQualifier<'a> {
    fn new(name: &'a OsStr) -> Self {
        Self { name }
    }

    fn is_raw_or_res_file(&self) -> bool {
        self.name == OsStr::new("rawfile") || self.name == OsStr::new("resfile")
    }
}

struct PermissionReasonNames<'a> {
    request_permissions: &'a [serde_json::Value],
}

impl<'a> PermissionReasonNames<'a> {
    fn new(request_permissions: &'a [serde_json::Value]) -> Self {
        Self {
            request_permissions,
        }
    }

    fn collect(&self) -> Vec<String> {
        let mut seen = FxHashSet::default();
        let mut names = Vec::new();
        for permission in self.request_permissions {
            let Some(reason) = permission.get("reason").and_then(|value| value.as_str()) else {
                continue;
            };
            let Some(name) = reason.strip_prefix(STRING_REF_PREFIX) else {
                continue;
            };
            if seen.insert(name.to_string()) {
                names.push(name.to_string());
            }
        }
        names
    }
}

struct DependencyRootScanner<'a> {
    module_root: &'a Path,
    project_root: &'a Path,
}

impl<'a> DependencyRootScanner<'a> {
    fn new(module_root: &'a Path, project_root: &'a Path) -> Self {
        Self {
            module_root,
            project_root,
        }
    }

    fn collect_roots(&self) -> Vec<PathBuf> {
        let mut roots = Vec::new();
        let mut queue = VecDeque::new();
        let mut seen = FxHashSet::default();
        queue.push_back(self.module_root.to_path_buf());
        if self.project_root != self.module_root {
            queue.push_back(self.project_root.to_path_buf());
        }

        while let Some(pkg_root) = queue.pop_front() {
            let Some(pkg) = self.read_package_json(&pkg_root) else {
                continue;
            };
            for dep_name in pkg.dependencies.keys() {
                let Some(dep_root) = self.resolve_root(&pkg_root, dep_name) else {
                    continue;
                };
                if !seen.insert(dep_root.clone()) {
                    continue;
                }
                queue.push_back(dep_root.clone());
                roots.push(dep_root);
            }
        }

        roots
    }

    fn read_package_json(&self, pkg_root: &Path) -> Option<DependencyPackageJson> {
        let content = std::fs::read_to_string(pkg_root.join("oh-package.json5")).ok()?;
        json5::from_str::<DependencyPackageJson>(&content).ok()
    }

    fn resolve_root(&self, issuer_root: &Path, dep_name: &str) -> Option<PathBuf> {
        let mut current = Some(issuer_root);
        while let Some(root) = current {
            let candidate = root.join("oh_modules").join(dep_name);
            if candidate.is_dir() {
                return Some(candidate);
            }
            if root == self.project_root {
                break;
            }
            current = root.parent();
        }
        let candidate = self.project_root.join("oh_modules").join(dep_name);
        candidate.is_dir().then_some(candidate)
    }
}

struct DependencyResourceLayout<'a> {
    package_dir: &'a Path,
    target_name: &'a str,
}

impl<'a> DependencyResourceLayout<'a> {
    fn new(package_dir: &'a Path, target_name: &'a str) -> Self {
        Self {
            package_dir,
            target_name,
        }
    }

    fn resource_str_sources(&self) -> Vec<PathBuf> {
        [
            PermissionReasonLayout::new(self.package_dir, self.target_name)
                .resource_str_output_dir(),
            self.package_dir.join("src").join("main").join("resource"),
            self.package_dir.join("src").join("main").join("resources"),
        ]
        .into_iter()
        .filter(|path| path.is_dir())
        .collect()
    }
}

struct ResourceStrRootMerger<'a> {
    source_root: &'a Path,
    output_dir: &'a Path,
}

impl<'a> ResourceStrRootMerger<'a> {
    fn new(source_root: &'a Path, output_dir: &'a Path) -> Self {
        Self {
            source_root,
            output_dir,
        }
    }

    fn merge(&self) -> Result<bool, ArkdownError> {
        let mut wrote_any = false;
        let mut entries = std::fs::read_dir(self.source_root)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let qualifier = entry.file_name();
            if ResourceQualifier::new(&qualifier).is_raw_or_res_file() {
                continue;
            }
            let entry_path = entry.path();
            let source = ResourceStringLayout::new(&entry_path).string_json();
            if !source.exists() {
                continue;
            }
            let source_json = StringJsonDocument::read(&source)?;
            let output =
                ResourceStringLayout::new(self.output_dir).qualified_string_json(&qualifier);
            let merged = if output.exists() {
                let existing_json = StringJsonDocument::read(&output)?;
                StringJsonDocument::merge(existing_json, source_json)
            } else {
                source_json
            };
            if let Some(parent) = output.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(
                &output,
                serde_json::to_vec(&merged)
                    .expect("serializing merged permission reason strings should not fail"),
            )?;
            wrote_any = true;
        }
        Ok(wrote_any)
    }
}

struct StringJsonDocument;

impl StringJsonDocument {
    fn filter_reason_names(
        source: &Path,
        reason_names: &FxHashSet<&str>,
    ) -> Result<Option<serde_json::Value>, ArkdownError> {
        let value = Self::read(source)?;
        let Some(strings) = value.get("string").and_then(|strings| strings.as_array()) else {
            return Ok(None);
        };
        let filtered = strings
            .iter()
            .filter(|entry| {
                entry
                    .get("name")
                    .and_then(|name| name.as_str())
                    .is_some_and(|name| reason_names.contains(name))
            })
            .cloned()
            .collect::<Vec<_>>();
        if filtered.is_empty() {
            return Ok(None);
        }
        Ok(Some(serde_json::json!({ "string": filtered })))
    }

    fn read(path: &Path) -> Result<serde_json::Value, ArkdownError> {
        let content = std::fs::read_to_string(path)?;
        let value = json5::from_str::<serde_json::Value>(&content)
            .map_err(|error| ArkdownError::Io(std::io::Error::other(error.to_string())))?;
        if !value
            .as_object()
            .is_some_and(|object| object.contains_key("string"))
        {
            return Err(ArkdownError::Io(std::io::Error::other(format!(
                "WRONG_STRING_JSON_FORMAT: {}",
                path.display()
            ))));
        }
        Ok(value)
    }

    fn merge(mut existing: serde_json::Value, incoming: serde_json::Value) -> serde_json::Value {
        let mut seen = FxHashSet::default();
        let mut strings = Vec::new();
        if let Some(existing_strings) = existing
            .get_mut("string")
            .and_then(|value| value.as_array_mut())
        {
            for item in existing_strings.drain(..) {
                if let Some(name) = item.get("name").and_then(|name| name.as_str()) {
                    seen.insert(name.to_string());
                }
                strings.push(item);
            }
        }
        if let Some(incoming_strings) = incoming.get("string").and_then(|value| value.as_array()) {
            for item in incoming_strings {
                let Some(name) = item.get("name").and_then(|name| name.as_str()) else {
                    continue;
                };
                if seen.insert(name.to_string()) {
                    strings.push(item.clone());
                }
            }
        }
        serde_json::json!({ "string": strings })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_permission_reason_resources_filters_strings() {
        let tmp = tempfile::tempdir().unwrap();
        let string_dir = tmp.path().join("src/main/resources/base/element");
        std::fs::create_dir_all(&string_dir).unwrap();
        std::fs::write(
            string_dir.join("string.json"),
            r#"{
                string: [
                    { name: "camera_reason", value: "Need camera" },
                    { name: "unused_reason", value: "Unused" },
                ]
            }"#,
        )
        .unwrap();

        let output = PermissionReasonProcessor::new()
            .process_permission_reason_resources(PermissionReasonResourceInput {
                module_root: tmp.path(),
                target_name: "default",
                request_permissions: &[serde_json::json!({
                    "name": "ohos.permission.CAMERA",
                    "reason": "$string:camera_reason",
                })],
            })
            .unwrap()
            .expect("matching reason should produce resource_str");

        assert!(output.ends_with(Path::new(
            "build/default/intermediates/res/default/resource_str"
        )));
        let filtered = std::fs::read_to_string(output.join("base/element/string.json")).unwrap();
        assert_eq!(
            filtered,
            r#"{"string":[{"name":"camera_reason","value":"Need camera"}]}"#
        );
    }

    #[test]
    fn test_process_permission_reason_resources_clears_stale_output() {
        let tmp = tempfile::tempdir().unwrap();
        let stale = tmp
            .path()
            .join("build/default/intermediates/res/default/resource_str/base/element");
        std::fs::create_dir_all(&stale).unwrap();
        std::fs::write(stale.join("string.json"), r#"{"string":[]}"#).unwrap();

        let output = PermissionReasonProcessor::new()
            .process_permission_reason_resources(PermissionReasonResourceInput {
                module_root: tmp.path(),
                target_name: "default",
                request_permissions: &[],
            })
            .unwrap();

        assert!(output.is_none());
        assert!(!tmp
            .path()
            .join("build/default/intermediates/res/default/resource_str")
            .exists());
    }

    #[test]
    fn test_merge_dependency_permission_reason_resources_merges_and_deduplicates() {
        let tmp = tempfile::tempdir().unwrap();
        let module_root = tmp.path().join("entry");
        let dep_a = module_root.join("oh_modules/dep-a");
        let dep_b = module_root.join("oh_modules/dep-b");
        std::fs::create_dir_all(
            dep_a.join("build/default/intermediates/res/default/resource_str/base/element"),
        )
        .unwrap();
        std::fs::create_dir_all(dep_b.join("src/main/resources/base/element")).unwrap();
        std::fs::write(
            module_root.join("oh-package.json5"),
            r#"{ dependencies: { "dep-a": "1.0.0", "dep-b": "1.0.0" } }"#,
        )
        .unwrap();
        std::fs::write(
            dep_a.join(
                "build/default/intermediates/res/default/resource_str/base/element/string.json",
            ),
            r#"{
                string: [
                    { name: "shared_reason", value: "from dep a" },
                    { name: "camera_reason", value: "camera" },
                ]
            }"#,
        )
        .unwrap();
        std::fs::write(
            dep_b.join("src/main/resources/base/element/string.json"),
            r#"{
                string: [
                    { name: "shared_reason", value: "from dep b" },
                    { name: "location_reason", value: "location" },
                ]
            }"#,
        )
        .unwrap();

        let output = PermissionReasonProcessor::new()
            .merge_dependency_permission_reason_resources(DependencyPermissionReasonResourceInput {
                module_root: &module_root,
                project_root: tmp.path(),
                target_name: "default",
            })
            .unwrap()
            .expect("dependency resources should produce resource_str");

        let merged = std::fs::read_to_string(output.join("base/element/string.json")).unwrap();
        assert_eq!(
            merged,
            r#"{"string":[{"name":"shared_reason","value":"from dep a"},{"name":"camera_reason","value":"camera"},{"name":"location_reason","value":"location"}]}"#
        );
    }

    #[test]
    fn test_merge_dependency_permission_reason_resources_handles_scoped_packages() {
        let tmp = tempfile::tempdir().unwrap();
        let module_root = tmp.path().join("entry");
        let dep = module_root.join("oh_modules/@scope/dep");
        std::fs::create_dir_all(dep.join("src/main/resource/base/element")).unwrap();
        std::fs::write(
            module_root.join("oh-package.json5"),
            r#"{ dependencies: { "@scope/dep": "1.0.0" } }"#,
        )
        .unwrap();
        std::fs::write(
            dep.join("src/main/resource/base/element/string.json"),
            r#"{"string":[{"name":"scoped_reason","value":"scoped"}]}"#,
        )
        .unwrap();

        let output = PermissionReasonProcessor::new()
            .merge_dependency_permission_reason_resources(DependencyPermissionReasonResourceInput {
                module_root: &module_root,
                project_root: tmp.path(),
                target_name: "default",
            })
            .unwrap()
            .expect("scoped dependency resources should produce resource_str");

        let merged = std::fs::read_to_string(output.join("base/element/string.json")).unwrap();
        assert_eq!(
            merged,
            r#"{"string":[{"name":"scoped_reason","value":"scoped"}]}"#
        );
    }

    #[test]
    fn test_merge_dependency_permission_reason_resources_follows_transitive_and_skips_undeclared() {
        let tmp = tempfile::tempdir().unwrap();
        let module_root = tmp.path().join("entry");
        let source_har = module_root.join("oh_modules/source-har");
        let transitive_hsp = source_har.join("oh_modules/@scope/shared-hsp");
        let undeclared_hsp = module_root.join("oh_modules/undeclared-hsp");
        std::fs::create_dir_all(&source_har).unwrap();
        std::fs::create_dir_all(transitive_hsp.join("src/main/resource/base/element")).unwrap();
        std::fs::create_dir_all(undeclared_hsp.join("src/main/resource/base/element")).unwrap();
        std::fs::write(
            module_root.join("oh-package.json5"),
            r#"{ dependencies: { "source-har": "1.0.0" } }"#,
        )
        .unwrap();
        std::fs::write(
            source_har.join("oh-package.json5"),
            r#"{ dependencies: { "@scope/shared-hsp": "1.0.0" } }"#,
        )
        .unwrap();
        std::fs::write(
            transitive_hsp.join("src/main/resource/base/element/string.json"),
            r#"{"string":[{"name":"transitive_reason","value":"transitive"}]}"#,
        )
        .unwrap();
        std::fs::write(
            undeclared_hsp.join("src/main/resource/base/element/string.json"),
            r#"{"string":[{"name":"undeclared_reason","value":"undeclared"}]}"#,
        )
        .unwrap();

        let output = PermissionReasonProcessor::new()
            .merge_dependency_permission_reason_resources(DependencyPermissionReasonResourceInput {
                module_root: &module_root,
                project_root: tmp.path(),
                target_name: "default",
            })
            .unwrap()
            .expect("transitive dependency resources should produce resource_str");

        let merged = std::fs::read_to_string(output.join("base/element/string.json")).unwrap();
        assert_eq!(
            merged,
            r#"{"string":[{"name":"transitive_reason","value":"transitive"}]}"#
        );
    }

    #[test]
    fn test_permission_reason_processor_new_and_default() {
        let _p1 = PermissionReasonProcessor::new();
        let _p2 = PermissionReasonProcessor::default();
    }

    #[test]
    fn test_permission_reason_processor_process_returns_none_when_no_reasons() {
        let tmp = tempfile::tempdir().unwrap();
        let processor = PermissionReasonProcessor::new();
        let result = processor
            .process_permission_reason_resources(PermissionReasonResourceInput {
                module_root: tmp.path(),
                target_name: "default",
                request_permissions: &[],
            })
            .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_permission_reason_processor_merge_returns_none_when_no_deps() {
        let tmp = tempfile::tempdir().unwrap();
        let processor = PermissionReasonProcessor::new();
        let result = processor
            .merge_dependency_permission_reason_resources(DependencyPermissionReasonResourceInput {
                module_root: tmp.path(),
                project_root: tmp.path(),
                target_name: "default",
            })
            .unwrap();
        assert!(result.is_none());
    }
}
