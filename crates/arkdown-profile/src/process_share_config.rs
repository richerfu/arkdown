//! `ProcessShareConfig` — merge `crossAppSharedConfig` from HSP dependencies.
//!
//! Mirrors the hvigor `ProcessShareConfig` task
//! (`hvigor-ohos-plugin/src/tasks/process-share-config.js`).
//!
//! ## What it does
//!
//! 1. Walks host/project declared dependencies for installed HSP packages.
//! 2. Reads each HSP's `module.json5` (or the pre-processed `module.json`) and
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

use indexmap::IndexMap;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::Deserialize;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use arkdown_core::{ArkdownError, CrossAppSharedConfigEntry};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Input for the share-config merge pass.
pub struct ShareConfigInput<'a> {
    /// Root directory of the project containing the consuming module.
    pub project_root: &'a Path,
    /// Root directory of the consuming module (e.g. `/project/entry`).
    pub module_root: &'a Path,
    /// `crossAppSharedConfig` entries declared by the module itself (may be empty
    /// for HAP/HAR consumers; non-empty for HSP modules that export shared components).
    pub self_entries: Vec<CrossAppSharedConfigEntry>,
    /// Optional `$profile:<name>` reference declared by the module itself.
    /// Hvigor reads this profile and merges its `crossAppSharedConfig` array
    /// before collecting dependency HSP entries.
    pub self_profile_name: Option<String>,
}

#[derive(Deserialize)]
struct ShareDependencyPackageJson {
    #[serde(default)]
    dependencies: IndexMap<String, String>,
}

impl ShareConfigInput<'_> {
    /// Walk declared dependencies, collect `crossAppSharedConfig` entries from
    /// every HSP dependency, merge them with `self_entries`, validate for
    /// duplicate URIs, and return the merged list.
    ///
    /// Errors are returned for I/O failures only; missing or malformed `module.json5`
    /// files in individual dependencies are silently skipped (matching hvigor behaviour
    /// where non-HSP packages simply have no `crossAppSharedConfig`).
    pub fn merge(self) -> Result<ShareConfigOutput, ArkdownError> {
        let mut all_entries: Vec<CrossAppSharedConfigEntry> = Vec::new();

        // Include the module's own entries first (for HSP modules that export shared components).
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
        }

        // Walk declared dependencies to find installed HSP packages.
        for pkg_dir in Self::collect_declared_dependency_roots(self.project_root, self.module_root)
        {
            let pkg_name = pkg_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            let entries = Self::read_cross_app_shared_config_from_pkg(&pkg_dir, &pkg_name);
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

/// Walk declared dependencies, collect `crossAppSharedConfig` entries from
/// every HSP dependency, merge them with `self_entries`, validate for
/// duplicate URIs, and return the merged list.
///
/// Returns `Err` when two packages declare the same URI — this is a hard build
/// error matching hvigor's `printErrorExit` call.
/// Missing or malformed `module.json5` files are silently skipped.
pub fn merge_share_config(input: ShareConfigInput<'_>) -> Result<ShareConfigOutput, ArkdownError> {
    input.merge()
}

/// Write the merged share config to `output_path` as
/// `{"crossAppSharedConfig": [...]}`.
///
/// Creates parent directories if they do not exist.
pub fn write_share_config(
    output: &ShareConfigOutput,
    output_path: &Path,
) -> Result<(), std::io::Error> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let value = serde_json::json!({ "crossAppSharedConfig": output.entries });
    let json = serde_json::to_string(&value).map_err(|e| std::io::Error::other(e.to_string()))?;
    std::fs::write(output_path, json)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Private associated functions on ShareConfigInput
// ---------------------------------------------------------------------------

impl ShareConfigInput<'_> {
    fn collect_declared_dependency_roots(project_root: &Path, module_root: &Path) -> Vec<PathBuf> {
        let mut roots = Vec::new();
        let mut queue = VecDeque::new();
        let mut seen = FxHashSet::default();
        queue.push_back(module_root.to_path_buf());
        if project_root != module_root {
            queue.push_back(project_root.to_path_buf());
        }

        while let Some(pkg_root) = queue.pop_front() {
            let Some(pkg) = Self::read_dependency_package_json(&pkg_root) else {
                continue;
            };
            for dep_name in pkg.dependencies.keys() {
                let Some(dep_root) =
                    Self::resolve_dependency_root(&pkg_root, project_root, dep_name)
                else {
                    continue;
                };
                let dep_key = arkdown_path::to_slash(&dep_root);
                if !seen.insert(dep_key) {
                    continue;
                }
                queue.push_back(dep_root.clone());
                roots.push(dep_root);
            }
        }

        roots
    }

    fn read_dependency_package_json(pkg_root: &Path) -> Option<ShareDependencyPackageJson> {
        let content = std::fs::read_to_string(pkg_root.join("oh-package.json5")).ok()?;
        json5::from_str::<ShareDependencyPackageJson>(&content).ok()
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

    /// Read `crossAppSharedConfig` from a single installed package directory.
    ///
    /// Tries `src/main/module.json5` first (source layout), then `module.json`
    /// (pre-built layout) at the package root.
    fn read_cross_app_shared_config_from_pkg(
        pkg_dir: &Path,
        pkg_name: &str,
    ) -> Vec<CrossAppSharedConfigEntry> {
        // Candidate paths in priority order.
        let candidates: &[PathBuf] = &[
            pkg_dir.join("src/main/module.json5"),
            pkg_dir.join("module.json5"),
            pkg_dir.join("module.json"),
        ];

        for candidate in candidates {
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

            // Only process HSP modules (type == "shared").
            let module_type = raw
                .pointer("/module/type")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if module_type != "shared" {
                return vec![];
            }

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
        let profile_path = module_root
            .join("src/main/resources/base/profile")
            .join(format!("{profile_name}.json"));
        Self::read_profile_entries(&profile_path)
    }

    fn read_profile_entries_from_pkg(
        pkg_dir: &Path,
        profile_ref: &str,
    ) -> Vec<CrossAppSharedConfigEntry> {
        let Some(profile_name) = Self::parse_profile_name(profile_ref) else {
            return vec![];
        };
        for base in [
            pkg_dir.join("src/main/resources/base/profile"),
            pkg_dir.join("resources/base/profile"),
            pkg_dir.join("build/default/intermediates/res/default/resources/base/profile"),
        ] {
            let profile_path = base.join(format!("{profile_name}.json"));
            if profile_path.is_file() {
                return Self::read_profile_entries(&profile_path);
            }
        }
        vec![]
    }

    fn parse_profile_name(profile_ref: &str) -> Option<&str> {
        let profile_name = profile_ref
            .strip_prefix("$profile:")
            .unwrap_or(profile_ref)
            .trim();
        (!profile_name.is_empty()).then_some(profile_name)
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

    #[test]
    fn test_merge_share_config_no_oh_modules() {
        let tmp = tempfile::tempdir().unwrap();
        let output = merge_share_config(ShareConfigInput {
            project_root: tmp.path(),
            module_root: tmp.path(),
            self_entries: vec![],
            self_profile_name: None,
        })
        .unwrap();
        assert!(output.entries.is_empty());
    }

    #[test]
    fn test_merge_share_config_collects_hsp_entries() {
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
                        { "uri": "myapp://shared/Foo", "srcEntry": "./ets/Foo.ets" }
                    ]
                }
            }"#,
        );

        let output = merge_share_config(ShareConfigInput {
            project_root: tmp.path(),
            module_root: tmp.path(),
            self_entries: vec![],
            self_profile_name: None,
        })
        .unwrap();

        assert_eq!(output.entries.len(), 1);
        assert_eq!(output.entries[0].uri, "myapp://shared/Foo");
        assert_eq!(output.entries[0].package_name, "my-hsp");
    }

    #[test]
    fn test_merge_share_config_skips_non_hsp() {
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
                        { "uri": "myapp://shared/Bar", "srcEntry": "./ets/Bar.ets" }
                    ]
                }
            }"#,
        );

        let output = merge_share_config(ShareConfigInput {
            project_root: tmp.path(),
            module_root: tmp.path(),
            self_entries: vec![],
            self_profile_name: None,
        })
        .unwrap();

        assert!(
            output.entries.is_empty(),
            "HAR packages must not contribute crossAppSharedConfig"
        );
    }

    #[test]
    fn test_merge_share_config_includes_self_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let self_entry = CrossAppSharedConfigEntry {
            package_name: "self-hsp".to_string(),
            uri: "myapp://shared/Self".to_string(),
            src_entry: Some("./ets/Self.ets".to_string()),
        };
        let output = merge_share_config(ShareConfigInput {
            project_root: tmp.path(),
            module_root: tmp.path(),
            self_entries: vec![self_entry.clone()],
            self_profile_name: None,
        })
        .unwrap();
        assert_eq!(output.entries.len(), 1);
        assert_eq!(output.entries[0], self_entry);
    }

    #[test]
    fn test_merge_share_config_reads_self_profile_ref() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("oh-package.json5"),
            r#"{ name: "self-hsp" }"#,
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

        let output = merge_share_config(ShareConfigInput {
            project_root: tmp.path(),
            module_root: tmp.path(),
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
        assert_eq!(output.entries[0].package_name, "self-hsp");
    }

    #[test]
    fn test_merge_share_config_reads_dependency_profile_ref() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("oh-package.json5"),
            r#"{ dependencies: { "profile-hsp": "1.0.0" } }"#,
        )
        .unwrap();
        let oh_modules = tmp.path().join("oh_modules");
        let hsp_pkg = oh_modules.join("profile-hsp");
        write_module_json5(
            &hsp_pkg,
            r#"{
                "module": {
                    "name": "profile-hsp",
                    "type": "shared",
                    "crossAppSharedConfig": "$profile:cross_app_shared_config"
                }
            }"#,
        );
        let profile_dir = hsp_pkg.join("src/main/resources/base/profile");
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

        let output = merge_share_config(ShareConfigInput {
            project_root: tmp.path(),
            module_root: tmp.path(),
            self_entries: vec![],
            self_profile_name: None,
        })
        .unwrap();

        assert_eq!(output.entries.len(), 1);
        assert_eq!(output.entries[0].uri, "myapp://shared/DepProfile");
        assert_eq!(output.entries[0].package_name, "profile-hsp");
    }

    #[test]
    fn test_write_share_config_creates_file() {
        let tmp = tempfile::tempdir().unwrap();
        let out_path = tmp
            .path()
            .join("intermediates/share_config/default/temp-share-config.json");

        let output = ShareConfigOutput {
            entries: vec![CrossAppSharedConfigEntry {
                package_name: "pkg-a".to_string(),
                uri: "myapp://shared/Comp".to_string(),
                src_entry: None,
            }],
        };
        write_share_config(&output, &out_path).unwrap();

        assert!(out_path.exists());
        let content = fs::read_to_string(&out_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let arr = parsed["crossAppSharedConfig"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["uri"], "myapp://shared/Comp");
        assert_eq!(arr[0]["packageName"], "pkg-a");
    }

    #[test]
    fn test_write_share_config_empty_array() {
        let tmp = tempfile::tempdir().unwrap();
        let out_path = tmp.path().join("temp-share-config.json");
        let output = ShareConfigOutput { entries: vec![] };
        write_share_config(&output, &out_path).unwrap();

        let content = fs::read_to_string(&out_path).unwrap();
        assert_eq!(content, r#"{"crossAppSharedConfig":[]}"#);
    }

    #[test]
    fn test_merge_share_config_fills_package_name_from_dir() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("oh-package.json5"),
            r#"{ dependencies: { "auto-named-hsp": "1.0.0" } }"#,
        )
        .unwrap();
        let oh_modules = tmp.path().join("oh_modules");
        let hsp_pkg = oh_modules.join("auto-named-hsp");
        write_module_json5(
            &hsp_pkg,
            r#"{
                "module": {
                    "name": "auto-named-hsp",
                    "type": "shared",
                    "crossAppSharedConfig": [
                        { "uri": "myapp://shared/X" }
                    ]
                }
            }"#,
        );

        let output = merge_share_config(ShareConfigInput {
            project_root: tmp.path(),
            module_root: tmp.path(),
            self_entries: vec![],
            self_profile_name: None,
        })
        .unwrap();

        assert_eq!(output.entries.len(), 1);
        assert_eq!(
            output.entries[0].package_name, "auto-named-hsp",
            "packageName must be filled from the oh_modules directory name"
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

        let scoped_hsp = source_har.join("oh_modules/@scope/shared-hsp");
        write_module_json5(
            &scoped_hsp,
            r#"{
                "module": {
                    "name": "shared-hsp",
                    "type": "shared",
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

        let output = merge_share_config(ShareConfigInput {
            project_root: tmp.path(),
            module_root: tmp.path(),
            self_entries: vec![],
            self_profile_name: None,
        })
        .unwrap();

        assert_eq!(output.entries.len(), 1);
        assert_eq!(output.entries[0].uri, "myapp://shared/Scoped");
        assert_eq!(output.entries[0].package_name, "shared-hsp");
    }
}
